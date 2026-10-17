//! Minimal file logger: %ProgramData%\fw-ams\service.log (truncated at 1 MB)

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn data_dir() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("fw-ams")
}

struct FileLogger {
    file: Mutex<Option<std::fs::File>>,
    console: bool,
}

impl log::Log for FileLogger {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.level() <= log::Level::Info
    }

    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        let line = format!(
            "{} {:5} {}\n",
            fw_ams_tuner::now_ms() / 1000,
            r.level(),
            r.args()
        );
        if self.console {
            eprint!("{line}");
        }
        if let Some(f) = self.file.lock().unwrap().as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {}
}

pub fn init(console: bool) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("service.log");
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > 1_000_000) {
        let _ = std::fs::remove_file(&path);
    }
    let file = std::fs::OpenOptions::new().create(true).append(true).open(path).ok();
    let logger = Box::leak(Box::new(FileLogger {
        file: Mutex::new(file),
        console,
    }));
    let _ = log::set_logger(logger).map(|()| log::set_max_level(log::LevelFilter::Info));
}
