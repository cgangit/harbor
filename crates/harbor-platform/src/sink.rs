#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedResult {
    Ok,
    BufferFull,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoPlane {
    Unbound,
    Acb,
    Exported,
}

pub trait VideoSink {
    fn load(&mut self, payload: &str) -> Result<(), String>;
    fn feed(&mut self, bytes: &[u8], pts_ns: i64, es: i32) -> FeedResult;
    fn pause(&mut self);
    fn play(&mut self);
    fn flush(&mut self);
    fn set_playtime(&mut self, ns: i64);
    fn unload(&mut self);
    fn window_id(&self) -> Option<&str> {
        None
    }
    fn set_window_id(&mut self, _id: &str) {}
    fn acb_bind(&mut self, _media_id: &str) {}
    fn acb_send_video_data(&mut self, _source_info: &[u8]) {}
    fn acb_start(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    fn acb_unload(&mut self) {}
    fn plane(&self) -> VideoPlane {
        VideoPlane::Unbound
    }
}

pub struct NullSink;

impl VideoSink for NullSink {
    fn load(&mut self, _payload: &str) -> Result<(), String> {
        Ok(())
    }
    fn feed(&mut self, _bytes: &[u8], _pts_ns: i64, _es: i32) -> FeedResult {
        FeedResult::Ok
    }
    fn pause(&mut self) {}
    fn play(&mut self) {}
    fn flush(&mut self) {}
    fn set_playtime(&mut self, _ns: i64) {}
    fn unload(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_sink_load_and_feed() {
        let mut s = NullSink;
        assert!(s.load("{}").is_ok());
        assert_eq!(s.feed(&[0, 1], 0, 1), FeedResult::Ok);
        assert_eq!(s.plane(), VideoPlane::Unbound);
    }
}
