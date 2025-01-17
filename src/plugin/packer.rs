use crate::error::LogError;
use crate::plugin::file_split::Packer;
use std::fs::File;

/// keep temp{date}.log
#[derive(Clone)]
pub struct LogPacker {}
impl Packer for LogPacker {
    fn pack_name(&self) -> &'static str {
        "log"
    }

    fn do_pack(&self, _log_file: File, _log_file_path: &str) -> Result<bool, LogError> {
        //do nothing,and not remove file
        return Ok(false);
    }
}

#[cfg(feature = "gzip")]
use flate2::write::GzEncoder;
#[cfg(feature = "gzip")]
use flate2::Compression;

#[cfg(feature = "gzip")]
pub struct GZipPacker {}

#[cfg(feature = "gzip")]
impl Packer for GZipPacker {
    fn pack_name(&self) -> &'static str {
        "gz"
    }

    fn do_pack(&self, mut log_file: File, log_file_path: &str) -> Result<bool, LogError> {
        use std::io::Write;
        let zip_path = log_file_path.replace(".log", ".gz");
        let zip_file = File::create(&zip_path)
            .map_err(|e| LogError::from(format!("[fastlog] create(&{}) fail:{}", zip_path, e)))?;
        //write zip bytes data
        let mut zip = GzEncoder::new(zip_file, Compression::default());
        std::io::copy(&mut log_file, &mut zip).map_err(|e| LogError::from(e.to_string()))?;
        zip.flush().map_err(|e| LogError::from(e.to_string()))?;
        let finish = zip.finish();
        if finish.is_err() {
            return Err(LogError::from(format!(
                "[fastlog] try zip fail{:?}",
                finish.err()
            )));
        }
        return Ok(true);
    }
}
