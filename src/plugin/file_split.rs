use crate::appender::{Command, FastLogRecord, LogAppender};
use crate::consts::LogSize;
use crate::error::LogError;
use crate::plugin::file_name::FileName;
use crate::{chan, Receiver, Sender, WaitGroup};
use fastdate::DateTime;
use std::cell::RefCell;
use std::fs::{DirEntry, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// must impl pack_name,is_allow,do_pack method
pub trait Packer: Send + Sync {
    fn pack_name(&self) -> &'static str;

    ///return bool: remove_log_file
    fn do_pack(&self, log_file: File, log_file_path: &str) -> Result<bool, LogError>;

    /// default 0 is not retry pack. if retry > 0 ,it will trying rePack
    fn retry(&self) -> i32 {
        return 0;
    }
}

impl Packer for Box<dyn Packer> {
    fn pack_name(&self) -> &'static str {
        self.deref().pack_name()
    }

    fn do_pack(&self, log_file: File, log_file_path: &str) -> Result<bool, LogError> {
        self.deref().do_pack(log_file, log_file_path)
    }
}

/// is can do pack?
pub trait CanRollingPack: Send {
    /// is it can do rolling? just return Some(log_file_name).notice the log_file_name must have prefix of temp_name.
    /// if you return None, it's not do rolling now.
    fn can(
        &mut self,
        appender: &dyn Packer,
        temp_name: &str,
        temp_size: usize,
        arg: &FastLogRecord,
    ) -> Option<String>;
}

/// keep logs, for example keep by log num or keep by log create time.
/// that do not meet the retention conditions will be deleted
/// you can use KeepType or RollingType::All
pub trait Keep: Send {
    /// return removed nums
    fn do_keep(&self, dir: &str, temp_name: &str) -> i64;
    fn read_paths(&self, dir: &str, temp_name: &str) -> Vec<DirEntry> {
        let base_name = get_base_name(temp_name);
        let paths = std::fs::read_dir(dir);
        if let Ok(paths) = paths {
            //let mut temp_file = None;
            let mut paths_vec = vec![];
            for path in paths {
                match path {
                    Ok(path) => {
                        if let Some(v) = path.file_name().to_str() {
                            if v == temp_name {
                                continue;
                            }
                            if !v.starts_with(&base_name) {
                                continue;
                            }
                        }
                        paths_vec.push(path);
                    }
                    _ => {}
                }
            }
            paths_vec.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
            return paths_vec;
        }
        return vec![];
    }
}

pub trait SplitFile: Send {
    fn new(path: &str) -> Result<Self, LogError>
    where
        Self: Sized;
    fn seek(&self, pos: SeekFrom) -> std::io::Result<u64>;
    fn write(&self, buf: &[u8]) -> std::io::Result<usize>;
    fn truncate(&self) -> std::io::Result<()>;
    fn flush(&self);
    fn len(&self) -> usize;
    fn offset(&self) -> usize;
}

///only use File
pub struct RawFile {
    pub inner: RefCell<File>,
}

impl From<File> for RawFile {
    fn from(value: File) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
}

impl SplitFile for RawFile {
    fn new(path: &str) -> Result<Self, LogError>
    where
        Self: Sized,
    {
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;
        Ok(Self {
            inner: RefCell::new(file),
        })
    }

    fn seek(&self, pos: SeekFrom) -> std::io::Result<u64> {
        self.inner.borrow_mut().seek(pos)
    }

    fn write(&self, buf: &[u8]) -> std::io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }

    fn truncate(&self) -> std::io::Result<()> {
        self.inner.borrow_mut().set_len(0)?;
        self.inner.borrow_mut().flush()?;
        self.inner.borrow_mut().seek(SeekFrom::Start(0))?;
        Ok(())
    }

    fn flush(&self) {
        let _ = self.inner.borrow_mut().flush();
    }

    fn len(&self) -> usize {
        if let Ok(v) = self.inner.borrow_mut().metadata() {
            v.len() as usize
        } else {
            0
        }
    }

    fn offset(&self) -> usize {
        let mut offset = self.len();
        if offset > 0 {
            offset = offset - 1;
        }
        offset
    }
}

pub enum DateType {
    Sec,
    Hour,
    Minute,
    Day,
    Month,
    Year,
}

impl Default for DateType {
    fn default() -> Self {
        Self::Day
    }
}

#[allow(dead_code)]
pub struct DurationType {
    last: DateTime,
    pub start_time: DateTime,
    pub duration: Duration,
}

impl DurationType {
    pub fn new(d: Duration) -> Self {
        let now = DateTime::now();
        Self {
            last: now.clone(),
            start_time: now,
            duration: d,
        }
    }
}

pub struct Rolling {
    last: SystemTime,
    pub how: RollingType,
}

impl Rolling {
    pub fn new(how: RollingType) -> Self {
        Self {
            last: SystemTime::now(),
            how: how,
        }
    }
}

///log rolling type
pub enum RollingType {
    ByDate(DateType),
    BySize(LogSize),
    ByDuration((DateTime, Duration)),
}

impl CanRollingPack for Rolling {
    fn can(
        &mut self,
        _appender: &dyn Packer,
        temp_name: &str,
        temp_size: usize,
        arg: &FastLogRecord,
    ) -> Option<String> {
        let last_time = self.last.clone();
        self.last = arg.now.clone();
        return match &mut self.how {
            RollingType::ByDate(date_type) => {
                let last_time = DateTime::from_system_time(last_time, fastdate::offset_sec());
                let log_time = DateTime::from_system_time(arg.now, fastdate::offset_sec());
                let diff = match date_type {
                    DateType::Sec => log_time.sec() != last_time.sec(),
                    DateType::Hour => log_time.hour() != last_time.hour(),
                    DateType::Minute => log_time.minute() != last_time.minute(),
                    DateType::Day => log_time.day() != last_time.day(),
                    DateType::Month => log_time.mon() != last_time.mon(),
                    DateType::Year => log_time.year() != last_time.year(),
                };
                if diff {
                    let log_name = {
                        if let Some(idx) = temp_name.rfind(".") {
                            let suffix = &temp_name[idx..];
                            temp_name.replace(
                                suffix,
                                &last_time.format(&format!("YYYY-MM-DDThh-mm-ss.000000{}", suffix)),
                            )
                        } else {
                            let mut temp_name = temp_name.to_string();
                            temp_name.push_str(&last_time.format("YYYY-MM-DDThh-mm-ss.000000"));
                            temp_name
                        }
                    };
                    Some(log_name)
                } else {
                    None
                }
            }
            RollingType::BySize(limit) => {
                if temp_size >= limit.get_len() {
                    let log_name = {
                        let last_time =
                            DateTime::from_system_time(last_time, fastdate::offset_sec());
                        if let Some(idx) = temp_name.rfind(".") {
                            let suffix = &temp_name[idx..];
                            temp_name.replace(
                                suffix,
                                &last_time.format(&format!("YYYY-MM-DDThh-mm-ss.000000{}", suffix)),
                            )
                        } else {
                            let mut temp_name = temp_name.to_string();
                            temp_name.push_str(&last_time.format("YYYY-MM-DDThh-mm-ss.000000"));
                            temp_name
                        }
                    };
                    Some(log_name)
                } else {
                    None
                }
            }
            RollingType::ByDuration((start_time, duration)) => {
                let log_time = DateTime::from_system_time(arg.now, fastdate::offset_sec());
                let next = start_time.clone().add(duration.clone());
                if log_time >= next {
                    let now = DateTime::now();
                    let last_time = DateTime::from_system_time(last_time, fastdate::offset_sec());
                    let log_name = {
                        if let Some(idx) = temp_name.rfind(".") {
                            let suffix = &temp_name[idx..];
                            temp_name.replace(
                                suffix,
                                &last_time.format(&format!("YYYY-MM-DDThh-mm-ss.000000{}", suffix)),
                            )
                        } else {
                            let mut temp_name = temp_name.to_string();
                            temp_name.push_str(&last_time.format("YYYY-MM-DDThh-mm-ss.000000"));
                            temp_name
                        }
                    };
                    *start_time = now;
                    Some(log_name)
                } else {
                    None
                }
            }
        };
    }
}

/// split log file allow pack compress log
/// Memory space swop running time , reduces the number of repeated queries for IO
pub struct FileSplitAppender {
    file: Box<dyn SplitFile>,
    packer: Arc<Box<dyn Packer>>,
    dir_path: String,
    sender: Sender<LogPack>,
    can_pack: Box<dyn CanRollingPack>,
    //cache data
    temp_bytes: AtomicUsize,
    temp_name: String,
}

impl FileSplitAppender {
    pub fn new<F: SplitFile + 'static>(
        file_path: &str,
        rolling: Box<dyn CanRollingPack>,
        keeper: Box<dyn Keep>,
        packer: Box<dyn Packer>,
    ) -> Result<FileSplitAppender, LogError> {
        let temp_name = {
            let mut name = file_path.extract_file_name().to_string();
            if name.is_empty() {
                name = "temp.log".to_string();
            }
            name
        };
        let mut dir_path = file_path.trim_end_matches(&temp_name).to_string();
        if dir_path.is_empty() {
            if let Ok(v) = std::env::current_dir() {
                dir_path = v.to_str().unwrap_or_default().to_string();
            }
        }
        let _ = std::fs::create_dir_all(&dir_path);
        let mut sp = "";
        if !dir_path.is_empty() {
            sp = "/";
        }
        let temp_file = format!("{}{}{}", dir_path, sp, temp_name);
        let temp_bytes = AtomicUsize::new(0);
        let file = F::new(&temp_file)?;
        let mut offset = file.offset();
        if offset != 0 {
            offset += 1;
        }
        temp_bytes.store(offset, Ordering::Relaxed);
        let _ = file.seek(SeekFrom::Start(temp_bytes.load(Ordering::Relaxed) as u64));
        let (sender, receiver) = chan(None);
        let arc_packer = Arc::new(packer);
        spawn_saver(temp_name.clone(), receiver, keeper, arc_packer.clone());
        Ok(Self {
            temp_bytes,
            dir_path: dir_path.to_string(),
            file: Box::new(file) as Box<dyn SplitFile>,
            sender,
            can_pack: rolling,
            temp_name,
            packer: arc_packer,
        })
    }
    /// send data make an pack,and truncate data when finish.
    pub fn send_pack(&self, new_log_name: String, wg: Option<WaitGroup>) {
        let mut sp = "";
        if !self.dir_path.is_empty() && !self.dir_path.ends_with("/") {
            sp = "/";
        }
        let first_file_path = format!("{}{}{}", self.dir_path, sp, &self.temp_name);
        let new_log_path = first_file_path.replace(&self.temp_name, &new_log_name);
        self.file.flush();
        let _ = std::fs::copy(&first_file_path, &new_log_path);
        let _ = self.sender.send(LogPack {
            dir: self.dir_path.clone(),
            new_log_name: new_log_path,
            wg,
        });
        self.truncate();
    }

    pub fn truncate(&self) {
        //reset data
        let _ = self.file.truncate();
        self.temp_bytes.store(0, Ordering::SeqCst);
    }

    pub fn temp_name(&self) -> &str {
        &self.temp_name
    }
}

///log data pack
pub struct LogPack {
    pub dir: String,
    pub new_log_name: String,
    pub wg: Option<WaitGroup>,
}

impl LogPack {
    /// write an Pack to zip file
    pub fn do_pack(&self, packer: &Box<dyn Packer>) -> Result<bool, LogError> {
        let log_file_path = self.new_log_name.as_str();
        if log_file_path.is_empty() {
            return Err(LogError::from("log_file_path.is_empty"));
        }
        let log_file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(log_file_path)
            .map_err(|e| {
                LogError::from(format!("open(log_file_path={}) fail={}", log_file_path, e))
            })?;
        //make
        let r = packer.do_pack(log_file, log_file_path);
        if r.is_err() && packer.retry() > 0 {
            let mut retry = 1;
            while let Err(_packs) = self.do_pack(packer) {
                retry += 1;
                if retry > packer.retry() {
                    break;
                }
            }
        }
        if let Ok(b) = r {
            return Ok(b);
        }
        return Ok(false);
    }
}

///rolling keep type
#[derive(Copy, Clone, Debug)]
pub enum KeepType {
    /// keep All of log packs
    All,
    /// keep by Time Duration,
    /// for example:
    /// // keep one day log pack
    /// (Duration::from_secs(24 * 3600))
    KeepTime(Duration),
    /// keep log pack num(.log,.gzip..more)
    KeepNum(i64),
}

impl Keep for KeepType {
    fn do_keep(&self, dir: &str, temp_name: &str) -> i64 {
        let mut removed = 0;
        match self {
            KeepType::All => {
                //do nothing
            }
            KeepType::KeepNum(n) => {
                let paths_vec = self.read_paths(dir, temp_name);
                for index in 0..paths_vec.len() {
                    if index >= (*n) as usize {
                        let item = &paths_vec[index];
                        let _ = std::fs::remove_file(item.path());
                        removed += 1;
                    }
                }
            }
            KeepType::KeepTime(duration) => {
                let paths_vec = self.read_paths(dir, temp_name);
                let now = DateTime::now();
                for index in 0..paths_vec.len() {
                    let item = &paths_vec[index];
                    if let Ok(m) = item.metadata() {
                        if let Ok(c) = m.created() {
                            let time = DateTime::from(c);
                            if now.clone().sub(duration.clone()) > time {
                                let _ = std::fs::remove_file(item.path());
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }
        removed
    }
}

impl LogAppender for FileSplitAppender {
    fn do_logs(&mut self, records: &[FastLogRecord]) {
        //if temp_bytes is full,must send pack
        let mut cap = 0;
        if records.len() != 0 {
            cap = 0;
            for x in records {
                cap += x.formated.len();
            }
        }
        let mut temp = String::with_capacity(cap);
        for x in records {
            match x.command {
                Command::CommandRecord => {
                    let current_temp_size = self.temp_bytes.load(Ordering::Relaxed)
                        + temp.as_bytes().len()
                        + x.formated.as_bytes().len();
                    if let Some(new_log_name) = self.can_pack.can(
                        self.packer.deref(),
                        &self.temp_name,
                        current_temp_size,
                        x,
                    ) {
                        self.temp_bytes.fetch_add(
                            {
                                let w = self.file.write(temp.as_bytes());
                                if let Ok(w) = w {
                                    w
                                } else {
                                    0
                                }
                            },
                            Ordering::SeqCst,
                        );
                        temp.clear();
                        self.send_pack(new_log_name, None);
                    }
                    temp.push_str(x.formated.as_str());
                }
                Command::CommandExit => {}
                Command::CommandFlush(ref w) => {
                    let current_temp_size = self.temp_bytes.load(Ordering::Relaxed);
                    if let Some(new_log_name) = self.can_pack.can(
                        self.packer.deref(),
                        &self.temp_name,
                        current_temp_size,
                        x,
                    ) {
                        self.temp_bytes.fetch_add(
                            {
                                let w = self.file.write(temp.as_bytes());
                                if let Ok(w) = w {
                                    w
                                } else {
                                    0
                                }
                            },
                            Ordering::SeqCst,
                        );
                        temp.clear();
                        self.send_pack(new_log_name, Some(w.clone()));
                    }
                }
            }
        }
        if !temp.is_empty() {
            let _ = self.temp_bytes.fetch_add(
                {
                    let w = self.file.write(temp.as_bytes());
                    if let Ok(w) = w {
                        w
                    } else {
                        0
                    }
                },
                Ordering::SeqCst,
            );
        }
    }
}

///spawn an saver thread to save log file or zip file
fn spawn_saver(
    temp_name: String,
    r: Receiver<LogPack>,
    rolling_type: Box<dyn Keep>,
    packer: Arc<Box<dyn Packer>>,
) {
    std::thread::spawn(move || {
        loop {
            if let Ok(pack) = r.recv() {
                if pack.wg.is_some() {
                    return;
                }
                let log_file_path = pack.new_log_name.clone();
                //do save pack
                let remove = pack.do_pack(packer.as_ref());
                if let Ok(remove) = remove {
                    if remove {
                        let _ = std::fs::remove_file(log_file_path);
                    }
                }
                //do rolling
                rolling_type.do_keep(&pack.dir, &temp_name);
            } else {
                break;
            }
        }
    });
}

fn get_base_name(path: &str) -> String {
    let file_name = path.extract_file_name();
    let p = file_name.rfind(".");
    match p {
        None => file_name,
        Some(i) => file_name[0..i].to_string(),
    }
}
