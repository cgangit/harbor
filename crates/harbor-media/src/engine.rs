use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::JoinHandle;

use harbor_gfx::{SCR_H, SCR_W};
use harbor_platform::{allow_direct_play, CodecTable, FeedResult, Stream, VideoPlane, VideoSink};
use harbor_sources::handle::MediaHandle;
use harbor_sources::Probe;

use crate::au::AccessUnit;
use crate::demux;
use crate::shared::Shared;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Idle,
    Loading,
    Playing,
    Bound,
    Streaming,
}

pub struct Engine<S: VideoSink> {
    pub stage: Stage,
    pending: Option<AccessUnit>,
    worker: Option<JoinHandle<()>>,
    pub shared: Arc<Shared>,
    pub sink: S,
}

impl<S: VideoSink> Engine<S> {
    pub fn new(sink: S) -> Self {
        Self {
            stage: Stage::Idle,
            pending: None,
            worker: None,
            shared: Arc::new(Shared::new()),
            sink,
        }
    }

    pub fn load_stream(
        &mut self,
        handle: MediaHandle,
        payload: &str,
        stream: &Stream,
        table: &CodecTable,
    ) -> Result<(), String> {
        allow_direct_play(stream, table)?;
        self.load_handle(handle, payload)
    }

    pub fn load_probe(
        &mut self,
        handle: MediaHandle,
        payload: &str,
        probe: &Probe,
        table: &CodecTable,
    ) -> Result<(), String> {
        if let Some(stream) = stream_from_probe(probe) {
            self.load_stream(handle, payload, &stream, table)
        } else {
            self.load_handle(handle, payload)
        }
    }

    pub fn load_handle(&mut self, handle: MediaHandle, payload: &str) -> Result<(), String> {
        self.unload();
        let payload = match (self.sink.plane(), self.sink.window_id()) {
            (VideoPlane::Exported, Some(id)) => harbor_platform::inject_window_id(payload, id),
            _ => payload.to_string(),
        };
        self.sink.load(&payload)?;
        self.shared.load_completed.store(false, Ordering::Release);
        self.shared.stop.store(false, Ordering::Release);
        self.shared.publish_seek_byte(0);
        let shared = self.shared.clone();
        self.worker = Some(std::thread::spawn(move || demux::run_worker(handle, shared)));
        self.stage = Stage::Loading;
        Ok(())
    }

    pub fn pump(&mut self) {
        if matches!(
            self.stage,
            Stage::Loading | Stage::Playing | Stage::Streaming | Stage::Bound
        ) {
            self.feed_one();
        }
        match self.sink.plane() {
            VideoPlane::Acb => self.walk_acb(),
            VideoPlane::Exported => {
                if self.stage == Stage::Loading
                    && (self.shared.frames.load(Ordering::Relaxed) > 0
                        || self.shared.load_completed.load(Ordering::Acquire))
                {
                    self.sink.play();
                    self.stage = Stage::Playing;
                }
            }
            VideoPlane::Unbound => {}
        }
    }

    fn walk_acb(&mut self) {
        if self.stage == Stage::Loading {
            let id = self
                .shared
                .media_id
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if !id.is_empty() {
                self.sink.acb_bind(&id);
                self.stage = Stage::Bound;
            }
        }
        if self.stage == Stage::Bound && self.shared.frames.load(Ordering::Relaxed) >= 2 {
            let info = self
                .shared
                .source_info
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if !info.is_empty() {
                self.sink.acb_send_video_data(&info);
                self.sink.acb_start(0, 0, SCR_W as i32, SCR_H as i32);
                self.sink.play();
                self.stage = Stage::Playing;
            }
        }
    }

    pub fn pause(&mut self) {
        self.sink.pause();
    }

    pub fn play(&mut self) {
        self.sink.play();
        if self.stage == Stage::Idle {
            return;
        }
        self.stage = Stage::Playing;
    }

    pub fn note_firmware(&self, media_id: &str, source_info: &[u8]) {
        *self.shared.media_id.lock().unwrap_or_else(|e| e.into_inner()) = media_id.to_string();
        *self.shared.source_info.lock().unwrap_or_else(|e| e.into_inner()) = source_info.to_vec();
    }

    pub fn seek_byte(&mut self, byte: u64, handle: MediaHandle) {
        self.shared.publish_seek_byte(byte);
        self.stop_worker();
        self.pending = None;
        self.shared.queue.clear();
        self.sink.flush();
        self.shared.stop.store(false, Ordering::Release);
        let shared = self.shared.clone();
        self.worker = Some(std::thread::spawn(move || demux::run_worker(handle, shared)));
        self.stage = Stage::Loading;
    }

    pub fn unload(&mut self) {
        self.stop_worker();
        self.pending = None;
        self.shared.queue.clear();
        self.sink.unload();
        self.stage = Stage::Idle;
    }

    fn stop_worker(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }

    fn feed_one(&mut self) {
        let au = match self.pending.take().or_else(|| self.shared.queue.pop()) {
            Some(au) => au,
            None => return,
        };
        match self.sink.feed(&au.bytes, au.pts_ns, au.es) {
            FeedResult::Ok => {
                self.shared.playhead_ns.store(au.pts_ns, Ordering::Release);
                self.shared.frames.fetch_add(1, Ordering::Relaxed);
            }
            FeedResult::BufferFull => {
                self.pending = Some(au);
            }
            FeedResult::Error => {
                self.stage = Stage::Idle;
            }
        }
    }
}

pub fn stream_from_probe(probe: &Probe) -> Option<Stream> {
    let codec = probe.video_codec.as_deref()?;
    Some(Stream {
        codec: codec.to_string(),
        bit_depth: probe.bit_depth,
        dovi: probe.dovi,
    })
}

impl<S: VideoSink> Drop for Engine<S> {
    fn drop(&mut self) {
        self.stop_worker();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demux::write_fixture;
    use crate::host::HostSink;
    use harbor_sources::handle::MediaHandle;
    use std::time::{Duration, Instant};

    fn fixture_path(tag: &str) -> std::path::PathBuf {
        let aus: Vec<_> = (0..3)
            .map(|i| AccessUnit {
                bytes: vec![i; 4],
                pts_ns: i as i64 * 10_000_000,
                es: 1,
            })
            .collect();
        let path = std::env::temp_dir().join(format!(
            "harbor-eng-{}-{}",
            std::process::id(),
            tag
        ));
        write_fixture(&path, &aus).unwrap();
        path
    }

    fn wait_frames(eng: &mut Engine<HostSink>, n: u64) {
        let t = Instant::now();
        while eng.shared.frames.load(Ordering::Relaxed) < n {
            eng.pump();
            if t.elapsed() > Duration::from_secs(2) {
                panic!("timeout waiting for {n} frames");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn pumps_fixture_into_host_sink() {
        let path = fixture_path("play");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(HostSink::new());
        eng.load_handle(handle, "{}").unwrap();
        wait_frames(&mut eng, 3);
        assert_eq!(eng.sink.feeds.len(), 3);
        assert_eq!(eng.sink.plane(), harbor_platform::VideoPlane::Exported);
        assert!(eng.sink.payload.contains("windowId"));
        assert!(eng.sink.payload.contains("host-sdl"));
        eng.unload();
        assert_eq!(eng.stage, Stage::Idle);
    }

    #[test]
    fn acb_order_bind_frames_sourceinfo_window_play() {
        use crate::fake::{Call, FakeSink};
        let path = fixture_path("acb");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(FakeSink::new());
        eng.load_handle(handle, "{}").unwrap();
        let raw = b"{\"keep\":1,\"w\":1920}";
        eng.note_firmware("pipeline-1", raw);
        let t = Instant::now();
        while eng.stage != Stage::Playing {
            eng.pump();
            if t.elapsed() > Duration::from_secs(2) {
                panic!("timeout stage={:?}", eng.stage);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            eng.sink.acb_seq(),
            vec![
                Call::AcbBind("pipeline-1".into()),
                Call::AcbSend(raw.to_vec()),
                Call::AcbStart {
                    x: 0,
                    y: 0,
                    w: 1920,
                    h: 1080
                },
                Call::Play,
            ]
        );
        assert!(eng.shared.frames.load(Ordering::Relaxed) >= 2);
    }

    #[test]
    fn buffer_full_retries_same_au() {
        let path = fixture_path("full");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut sink = HostSink::new();
        sink.stall_next(2);
        let mut eng = Engine::new(sink);
        eng.load_handle(handle, "{}").unwrap();
        wait_frames(&mut eng, 3);
        assert_eq!(eng.sink.feeds.len(), 3);
        assert_eq!(eng.sink.feeds[0].0, vec![0, 0, 0, 0]);
    }

    #[test]
    fn refuses_unsupported_before_load() {
        let path = fixture_path("refuse");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(HostSink::new());
        let err = eng
            .load_stream(handle, "{}", &Stream::hevc(10), &CodecTable::absent())
            .unwrap_err();
        assert!(err.starts_with("unsupported_codec:"));
        assert!(err.contains("HEVC 10-bit"));
        assert!(!eng.sink.loaded);
        assert_eq!(eng.stage, Stage::Idle);
    }

    #[test]
    fn listed_codec_loads() {
        let path = fixture_path("allow");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(HostSink::new());
        let table = CodecTable::from_json(
            r#"{"video":[{"codec":"H264"},{"codec":"HEVC","profile":"main10"}]}"#,
        );
        eng.load_stream(handle, "{}", &Stream::hevc(10), &table)
            .unwrap();
        assert!(eng.sink.loaded);
        assert_eq!(eng.stage, Stage::Loading);
        eng.unload();
    }

    #[test]
    fn probe_without_codec_still_loads() {
        let path = fixture_path("unknown");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(HostSink::new());
        let probe = Probe {
            container: Some("mkv".into()),
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration_ns: None,
            size: None,
            bit_depth: None,
            dovi: None,
        };
        eng.load_probe(handle, "{}", &probe, &CodecTable::absent())
            .unwrap();
        assert!(eng.sink.loaded);
        eng.unload();
    }

    #[test]
    fn webos5_payload_has_window_id_four_x_does_not() {
        use crate::fake::FakeSink;
        let path = fixture_path("win");
        let handle = MediaHandle::open_path(&path).unwrap();
        let mut eng = Engine::new(FakeSink::new());
        eng.load_handle(handle, r#"{"uri":"file://a.mkv"}"#).unwrap();
        let load = eng
            .sink
            .calls
            .iter()
            .find_map(|c| match c {
                crate::fake::Call::Load(p) => Some(p.as_str()),
                _ => None,
            })
            .expect("load");
        assert!(!load.contains("windowId"));
        eng.unload();
    }
}
