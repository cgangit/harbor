use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Mutex;

use crate::au::AuQueue;

pub struct Shared {
    pub playhead_ns: AtomicI64,
    pub frames: AtomicU64,
    pub load_completed: AtomicBool,
    pub duration_ns: AtomicI64,
    pub seek_byte: AtomicU64,
    pub stop: AtomicBool,
    pub media_id: Mutex<String>,
    pub source_info: Mutex<Vec<u8>>,
    pub queue: AuQueue,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            playhead_ns: AtomicI64::new(0),
            frames: AtomicU64::new(0),
            load_completed: AtomicBool::new(false),
            duration_ns: AtomicI64::new(0),
            seek_byte: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            media_id: Mutex::new(String::new()),
            source_info: Mutex::new(Vec::new()),
            queue: AuQueue::new(32),
        }
    }

    pub fn publish_seek_byte(&self, byte: u64) {
        self.seek_byte.store(byte, Ordering::Release);
    }

    pub fn take_seek_byte(&self) -> u64 {
        self.seek_byte.load(Ordering::Acquire)
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as StdMutex};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn seek_byte_release_before_close() {
        let shared = Arc::new(Shared::new());
        let log = Arc::new(StdMutex::new(Vec::new()));
        let log_w = log.clone();
        let sh = shared.clone();
        let worker = thread::spawn(move || {
            while !sh.stop.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
            log_w.lock().unwrap().push("close");
        });
        shared.publish_seek_byte(4096);
        log.lock().unwrap().push("seek");
        shared.stop.store(true, Ordering::Release);
        worker.join().unwrap();
        assert_eq!(shared.take_seek_byte(), 4096);
        assert_eq!(*log.lock().unwrap(), ["seek", "close"]);
    }
}
