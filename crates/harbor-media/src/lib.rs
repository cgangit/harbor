pub mod au;
pub mod demux;
pub mod engine;
pub mod fake;
pub mod ffi;
pub mod host;
pub mod shared;
pub mod starfish;
pub mod subs;

pub use au::{AccessUnit, AuQueue};
pub use engine::{stream_from_probe, Engine, Stage};
pub use fake::{Call, FakeSink};
pub use harbor_platform::{
    allow_direct_play, CodecTable, FeedResult, NullSink, Stream, VideoPlane, VideoSink,
};
pub use host::HostSink;
pub use shared::Shared;
pub use starfish::StarfishSink;
pub use subs::{active_text, parse as parse_subs, Cue};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_is_start() {
        assert_eq!(Stage::Idle as u8, 0);
    }
}
