use harbor_platform::{media_path, FeedResult, MediaPath, VideoPlane, VideoSink};

pub struct StarfishSink {
    created: bool,
    version: String,
    window_id: Option<String>,
}

impl StarfishSink {
    pub fn new() -> Self {
        Self::with_version("4.0.0")
    }

    pub fn with_version(version: &str) -> Self {
        Self {
            created: false,
            version: version.to_string(),
            window_id: None,
        }
    }

    pub fn try_create(&mut self) -> Result<(), String> {
        let rc = unsafe { crate::ffi::sf_create() };
        if rc != 0 {
            return Err("native-video-unavailable".into());
        }
        let _ = unsafe { crate::ffi::acb_create() };
        self.created = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_create_is_unavailable() {
        let mut s = StarfishSink::new();
        let err = s.try_create().unwrap_err();
        assert_eq!(err, "native-video-unavailable");
        assert_eq!(s.plane(), VideoPlane::Acb);
    }

    #[test]
    fn webos5_uses_exported_window() {
        let mut s = StarfishSink::with_version("5.2.0");
        s.set_window_id("sdl-export-1");
        assert_eq!(s.plane(), VideoPlane::Exported);
        assert_eq!(s.window_id(), Some("sdl-export-1"));
        assert_eq!(StarfishSink::with_version("4.9").plane(), VideoPlane::Acb);
    }
}

impl Default for StarfishSink {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoSink for StarfishSink {
    fn load(&mut self, payload: &str) -> Result<(), String> {
        if !self.created {
            self.try_create()?;
        }
        let c = std::ffi::CString::new(payload).map_err(|e| e.to_string())?;
        let rc = unsafe { crate::ffi::sf_load(c.as_ptr()) };
        if rc != 0 {
            Err("load failed".into())
        } else {
            Ok(())
        }
    }

    fn feed(&mut self, bytes: &[u8], pts_ns: i64, es: i32) -> FeedResult {
        let rc = unsafe {
            crate::ffi::sf_feed(bytes.as_ptr(), bytes.len() as i32, pts_ns, es)
        };
        match rc {
            0 => FeedResult::Ok,
            1 => FeedResult::BufferFull,
            _ => FeedResult::Error,
        }
    }

    fn pause(&mut self) {
        unsafe {
            let _ = crate::ffi::sf_pause();
        }
    }

    fn play(&mut self) {
        unsafe {
            let _ = crate::ffi::sf_play();
        }
    }

    fn flush(&mut self) {
        unsafe {
            let _ = crate::ffi::sf_flush();
        }
    }

    fn set_playtime(&mut self, _ns: i64) {}

    fn unload(&mut self) {
        unsafe {
            let _ = crate::ffi::sf_unload();
            crate::ffi::acb_unload();
        }
    }

    fn acb_bind(&mut self, media_id: &str) {
        if let Ok(c) = std::ffi::CString::new(media_id) {
            unsafe {
                let _ = crate::ffi::acb_bind(c.as_ptr());
            }
        }
    }

    fn acb_send_video_data(&mut self, source_info: &[u8]) {
        unsafe {
            let _ = crate::ffi::acb_send_video_data(
                source_info.as_ptr(),
                source_info.len() as i32,
            );
        }
    }

    fn acb_start(&mut self, x: i32, y: i32, w: i32, h: i32) {
        unsafe {
            let _ = crate::ffi::acb_start(x, y, w, h);
        }
    }

    fn acb_unload(&mut self) {
        unsafe {
            let _ = crate::ffi::acb_unload();
        }
    }

    fn window_id(&self) -> Option<&str> {
        self.window_id.as_deref()
    }

    fn set_window_id(&mut self, id: &str) {
        self.window_id = Some(id.to_string());
    }

    fn plane(&self) -> VideoPlane {
        match media_path(&self.version) {
            MediaPath::WindowId => VideoPlane::Exported,
            MediaPath::Acb => VideoPlane::Acb,
        }
    }
}
