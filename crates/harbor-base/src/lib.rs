use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const APP_ID: &str = "com.harbor.player";

static LOG: Mutex<Option<std::fs::File>> = Mutex::new(None);

pub fn app_id() -> &'static str {
    APP_ID
}

pub fn app_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("HARBOR_APP_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        return Path::new(&dir).join("harbor");
    }
    std::env::temp_dir().join("harbor")
}

pub fn rundir() -> PathBuf {
    let dir = app_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn log(msg: &str) {
    eprintln!("harbor: {msg}");
    let dir = rundir();
    let path = dir.join("events.log");
    let mut guard = LOG.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
    }
    if let Some(file) = guard.as_mut() {
        let _ = writeln!(file, "{msg}");
        let _ = file.flush();
    }
}

pub fn hex_u8(n: u8) -> String {
    format!("{n:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_is_harbor() {
        assert_eq!(app_id(), "com.harbor.player");
    }

    #[test]
    fn rundir_creates_directory() {
        let dir = rundir();
        assert!(dir.exists());
    }

    #[test]
    fn hex_u8_pads() {
        assert_eq!(hex_u8(0), "00");
        assert_eq!(hex_u8(255), "ff");
    }
}
