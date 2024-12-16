[![Rust](https://github.com/helicex-rs/fastlog/actions/workflows/rust.yml/badge.svg)](https://github.com/helicex-rs/fastlog/actions/workflows/rust.yml)

# fastlog

A log implementation for extreme speed, using Crossbeam/channel ,once Batch write logs,fast log date, Appender
architecture, appender per thread

* High performance,Low overhead, logs auto merge, Full APPEND mode file writing
* Built-in `ZIP`,`LZ4` compression
* Support use ```log::logger().flush()``` method wait to flush disk
* Support custom file(impl Trait)
* Support rolling log(`ByDate`,`BySize`,`ByDuration`)
* Support Keep log(`All`,`KeepTime`,`KeepNum`) Delete old logs,Prevent logs from occupying the disk
* uses `#![forbid(unsafe_code)]` 100% Safe Rust.

```
              -----------------
log data->    | main channel(crossbeam)  |   ->          
              ----------------- 
                                        ----------------                                    ----------------------
                                  ->    |thread channel|  -> background thread  |    appender1  |
                                        ----------------                                    ----------------------

                                        ----------------                                    ----------------------
                                  ->    |thread channel|  -> background thread  |    appender2  |
                                        ----------------                                    ----------------------
```

* How fast is?

* no flush(chan_len=1000000) benches/log.rs

```bash
# aws c7g.large (2C4G)
$ cargo bench --bench log
bench_log               time:   [935.11 ns 1.2231 µs 1.5810 µs]
Found 6 outliers among 100 measurements (6.00%)
  2 (2.00%) high mild
  4 (4.00%) high severe
```

* all log flush into file(chan_len=1000000) benches/log_file.rs

```bash
# aws c7g.large (2C4G)
$ cargo bench --bench log_file
bench_log_file          time:   [445.26 ns 614.35 ns 814.19 ns]
Found 13 outliers among 100 measurements (13.00%)
  5 (5.00%) high mild
  8 (8.00%) high severe
```

* how to use?

```toml
log = "~0.4"
fastlog = { git = "https://github.com/helicex-rs/fastlog.git", tag = "v1.7.6" }
```

or enable zip/lz4/gzip Compression library

```toml
log = "~0.4"
# "lz4","zip","gzip"
fastlog = { git = "https://github.com/helicex-rs/fastlog.git", tag = "v1.7.6", features = ["lz4", "zip", "gzip"] }
```

#### Performance optimization(important)

* use ```chan_len(Some(100000))``` Preallocating channel memory reduces the overhead of memory allocation，for example:

```rust
use log::{error, info, warn};
fn main() {
    fastlog::init(Config::new().file("target/test.log").chan_len(Some(100000))).unwrap();
    log::info!("Commencing yak shaving{}", 0);
}
```

#### Use Log(Console)

```rust
use log::{error, info, warn};
fn main() {
    fastlog::init(Config::new().console().chan_len(Some(100000))).unwrap();
    log::info!("Commencing yak shaving{}", 0);
}
```

#### Use Log(Console Print)

```rust
use log::{error, info, warn};
fn main() {
    fastlog::init(Config::new().console().chan_len(Some(100000))).unwrap();
    fastlog::print("Commencing print\n".into());
}
```

#### Use Log(File)

```rust
use fastlog::{init_log};
use log::{error, info, warn};
fn main() {
    fastlog::init(Config::new().file("target/test.log").chan_len(Some(100000))).unwrap();
    log::info!("Commencing yak shaving{}", 0);
    info!("Commencing yak shaving");
}
```

#### Split Log(ByLogDate)

```rust
use fastlog::config::Config;
use fastlog::plugin::file_split::{RollingType, KeepType, DateType, Rolling};
use std::thread::sleep;
use std::time::Duration;
use fastlog::plugin::packer::LogPacker;
fn main() {
    fastlog::init(Config::new().chan_len(Some(100000)).console().file_split(
        "target/logs/",
        Rolling::new(RollingType::ByDate(DateType::Day)),
        KeepType::KeepNum(2),
        LogPacker {},
    ))
        .unwrap();
    for _ in 0..60 {
        sleep(Duration::from_secs(1));
        log::info!("Commencing yak shaving");
    }
    log::logger().flush();
    println!("you can see log files in path: {}", "target/logs/")
}

```

#### Split Log(ByLogSize)

```rust
use fastlog::config::Config;
use fastlog::consts::LogSize;
use fastlog::plugin::file_split::{RollingType, KeepType, Rolling};
use fastlog::plugin::packer::LogPacker;
fn main() {
    fastlog::init(Config::new().chan_len(Some(100000)).console().file_split(
        "target/logs/",
        Rolling::new(RollingType::BySize(LogSize::KB(500))),
        KeepType::KeepNum(2),
        LogPacker {},
    ))
        .unwrap();
    for _ in 0..40000 {
        log::info!("Commencing yak shaving");
    }
    log::logger().flush();
    println!("you can see log files in path: {}", "target/logs/")
}

```

##### Custom Log(impl do_log method)

```rust
use fastlog::appender::{FastLogRecord, LogAppender};
use fastlog::config::Config;
use fastdate::DateTime;
use log::Level;

struct CustomLog {}

impl LogAppender for CustomLog {
    fn do_logs(&mut self, records: &[FastLogRecord]) {
        for record in records {
            let now = DateTime::from(record.now);
            let data;
            match record.level {
                Level::Warn | Level::Error => {
                    data = format!(
                        "{} {} {} - {}  {}\n",
                        now, record.level, record.module_path, record.args, record.formated
                    );
                }
                _ => {
                    data = format!(
                        "{} {} {} - {}\n",
                        &now, record.level, record.module_path, record.args
                    );
                }
            }
            print!("{}", data);
        }
    }
}

fn main() {
    fastlog::init(Config::new().custom(CustomLog {})).unwrap();
    log::info!("Commencing yak shaving");
    log::error!("Commencing error");
    log::logger().flush();
}

```
