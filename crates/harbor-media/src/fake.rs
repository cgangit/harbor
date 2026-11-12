use harbor_platform::{FeedResult, VideoPlane, VideoSink};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    Load(String),
    Feed { n: usize, pts_ns: i64, es: i32 },
    Pause,
    Play,
    Flush,
    SetPlaytime(i64),
    Unload,
    AcbBind(String),
    AcbSend(Vec<u8>),
    AcbStart { x: i32, y: i32, w: i32, h: i32 },
    AcbUnload,
}

pub struct FakeSink {
    pub calls: Vec<Call>,
    full_next: usize,
}

impl Default for FakeSink {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            full_next: 0,
        }
    }
}

impl FakeSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn acb_seq(&self) -> Vec<Call> {
        self.calls
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Call::AcbBind(_)
                        | Call::AcbSend(_)
                        | Call::AcbStart { .. }
                        | Call::Play
                        | Call::AcbUnload
                )
            })
            .cloned()
            .collect()
    }
}

impl VideoSink for FakeSink {
    fn load(&mut self, payload: &str) -> Result<(), String> {
        self.calls.push(Call::Load(payload.to_string()));
        Ok(())
    }

    fn feed(&mut self, bytes: &[u8], pts_ns: i64, es: i32) -> FeedResult {
        if self.full_next > 0 {
            self.full_next -= 1;
            return FeedResult::BufferFull;
        }
        self.calls.push(Call::Feed {
            n: bytes.len(),
            pts_ns,
            es,
        });
        FeedResult::Ok
    }

    fn pause(&mut self) {
        self.calls.push(Call::Pause);
    }

    fn play(&mut self) {
        self.calls.push(Call::Play);
    }

    fn flush(&mut self) {
        self.calls.push(Call::Flush);
    }

    fn set_playtime(&mut self, ns: i64) {
        self.calls.push(Call::SetPlaytime(ns));
    }

    fn unload(&mut self) {
        self.calls.push(Call::Unload);
    }

    fn acb_bind(&mut self, media_id: &str) {
        self.calls.push(Call::AcbBind(media_id.to_string()));
    }

    fn acb_send_video_data(&mut self, source_info: &[u8]) {
        self.calls.push(Call::AcbSend(source_info.to_vec()));
    }

    fn acb_start(&mut self, x: i32, y: i32, w: i32, h: i32) {
        self.calls.push(Call::AcbStart { x, y, w, h });
    }

    fn acb_unload(&mut self) {
        self.calls.push(Call::AcbUnload);
    }

    fn plane(&self) -> VideoPlane {
        VideoPlane::Acb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_acb_verbs() {
        let mut s = FakeSink::new();
        s.acb_bind("m1");
        s.acb_send_video_data(b"raw");
        s.acb_start(0, 0, 1920, 1080);
        s.play();
        assert_eq!(
            s.acb_seq(),
            vec![
                Call::AcbBind("m1".into()),
                Call::AcbSend(b"raw".to_vec()),
                Call::AcbStart {
                    x: 0,
                    y: 0,
                    w: 1920,
                    h: 1080
                },
                Call::Play,
            ]
        );
    }
}
