use crate::appender::{Command, FastLogRecord, RecordFormat};
use log::LevelFilter;

pub enum TimeType {
    Local,
    //default
    Utc,
}

impl Default for TimeType {
    fn default() -> Self {
        TimeType::Local
    }
}

pub struct FastLogFormat {
    // show line level
    pub display_line_level: LevelFilter,
    pub time_type: TimeType,
}

impl RecordFormat for FastLogFormat {
    fn do_format(&self, arg: &mut FastLogRecord) {
        match &arg.command {
            Command::CommandRecord => {
                let now = match self.time_type {
                    TimeType::Local => fastdate::DateTime::from(arg.now)
                        .set_offset(fastdate::offset_sec())
                        .display_stand_ms(),
                    TimeType::Utc => fastdate::DateTime::from(arg.now).display_stand_ms(),
                };
                if arg.level.to_level_filter() <= self.display_line_level {
                    arg.formated = format!(
                        "{} {} [{}:{}] {}\n",
                        &now,
                        arg.level,
                        arg.file,
                        arg.line.unwrap_or_default(),
                        arg.args,
                    );
                } else {
                    arg.formated = format!("{} {} {}\n", &now, arg.level, arg.args);
                }
            }
            Command::CommandExit => {}
            Command::CommandFlush(_) => {}
        }
    }
}

impl FastLogFormat {
    pub fn new() -> FastLogFormat {
        Self {
            display_line_level: LevelFilter::Info,
            time_type: TimeType::default(),
        }
    }

    ///show line level
    pub fn set_display_line_level(mut self, level: LevelFilter) -> Self {
        self.display_line_level = level;
        self
    }

    /// set time_type
    pub fn set_time_type(mut self, time_type: TimeType) -> Self {
        self.time_type = time_type;
        self
    }
}

pub struct FastLogFormatJson {
    pub time_type: TimeType,
}

impl Default for FastLogFormatJson {
    fn default() -> Self {
        Self {
            time_type: TimeType::default(),
        }
    }
}

impl RecordFormat for FastLogFormatJson {
    fn do_format(&self, arg: &mut FastLogRecord) {
        match &arg.command {
            Command::CommandRecord => {
                let now = match self.time_type {
                    TimeType::Local => fastdate::DateTime::from(arg.now)
                        .add_sub_sec(fastdate::offset_sec() as i64)
                        .display_stand_ms(),
                    TimeType::Utc => fastdate::DateTime::from(arg.now).display_stand_ms(),
                };
                //{"args":"Commencing yak shaving","date":"2022-08-19 09:53:47.798674","file":"example/src/split_log.rs","level":"INFO","line":21}
                let args = arg.args.replace("\"", "\\\"");
                let file = arg.file.replace("\\", "/");
                arg.formated = format!(
                    "{}\"args\":\"{}\",\"date\":\"{}\",\"file\":\"{}\",\"level\":\"{}\",\"line\":{}{}",
                    "{",
                    args,
                    now,
                    file,
                    arg.level,
                    arg.line.unwrap_or_default(),
                    "}\n"
                );
            }
            Command::CommandExit => {}
            Command::CommandFlush(_) => {}
        }
    }
}

impl FastLogFormatJson {
    pub fn new() -> FastLogFormatJson {
        Self::default()
    }
}
