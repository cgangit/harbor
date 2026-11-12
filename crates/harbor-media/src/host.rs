use harbor_platform::{FeedResult, VideoPlane, VideoSink};

pub struct HostSink {
    pub loaded: bool,
    pub paused: bool,
    pub playtime_ns: i64,
    pub feeds: Vec<(Vec<u8>, i64, i32)>,
    pub last_frame: Vec<u8>,
    pub payload: String,
    pub window_id: Option<String>,
    full_next: usize,
}

impl Default for HostSink {
    fn default() -> Self {
        Self {
            loaded: false,
            paused: false,
            playtime_ns: 0,
            feeds: Vec::new(),
            last_frame: Vec::new(),
            payload: String::new(),
            window_id: Some("host-sdl".into()),
            full_next: 0,
        }
    }
}

impl HostSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn stall_next(&mut self, n: usize) {
        self.full_next = n;
    }
}

impl VideoSink for HostSink {
    fn load(&mut self, payload: &str) -> Result<(), String> {
        self.loaded = true;
        self.payload = payload.to_string();
        self.feeds.clear();
        self.last_frame.clear();
        Ok(())
    }

    fn feed(&mut self, bytes: &[u8], pts_ns: i64, es: i32) -> FeedResult {
        if self.full_next > 0 {
            self.full_next -= 1;
            return FeedResult::BufferFull;
        }
        self.feeds.push((bytes.to_vec(), pts_ns, es));
        self.last_frame = bytes.to_vec();
        self.playtime_ns = pts_ns;
        FeedResult::Ok
    }

    fn pause(&mut self) {
        self.paused = true;
    }

    fn play(&mut self) {
        self.paused = false;
    }

    fn flush(&mut self) {
        self.feeds.clear();
        self.last_frame.clear();
    }

    fn set_playtime(&mut self, ns: i64) {
        self.playtime_ns = ns;
    }

    fn unload(&mut self) {
        self.loaded = false;
        self.feeds.clear();
        self.last_frame.clear();
    }

    fn window_id(&self) -> Option<&str> {
        self.window_id.as_deref()
    }

    fn set_window_id(&mut self, id: &str) {
        self.window_id = Some(id.to_string());
    }

    fn plane(&self) -> VideoPlane {
        VideoPlane::Exported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exported_plane_skips_acb() {
        let s = HostSink::new();
        assert_eq!(s.plane(), VideoPlane::Exported);
        assert_eq!(s.window_id(), Some("host-sdl"));
    }

    #[test]
    fn buffer_full_then_ok() {
        let mut s = HostSink::new();
        s.stall_next(1);
        assert_eq!(s.feed(&[1], 0, 1), FeedResult::BufferFull);
        assert_eq!(s.feed(&[1], 0, 1), FeedResult::Ok);
        assert_eq!(s.feeds.len(), 1);
    }
}
