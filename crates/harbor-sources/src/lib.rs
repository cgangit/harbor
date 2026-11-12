pub mod dlna;
pub mod entry;
pub mod error;
pub mod handle;
pub mod local;
pub mod smb;

use crate::error::SourceError;
use crate::handle::MediaHandle;

pub use dlna::{
    browse_soap, msearch_packet, parse_browse_response, parse_device_description, parse_didl,
    parse_ssdp, parse_ssdp_many, DlnaSource, MediaServer, SsdpDevice, CONTENT_DIRECTORY_ST,
    DISCOVER_TIMEOUT_MS, MEDIA_SERVER_ST, OBJECT_ID_ROOT,
};
pub use entry::{is_video_name, Entry, EntryKind, Page, Probe, VIDEO_EXTS};
pub use error::SourceError as Error;
pub use handle::{parse_content_range, parse_http_url, range_header};
pub use local::{discover, from_dir, LocalSource};
pub use smb::{
    canonical_path, classify_connect, classify_nt_status, SmbCreds, SmbSource, CONNECT_TIMEOUT,
    CONNECT_TIMEOUT_MS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Local,
    Dlna,
    Smb,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceId(pub String);

pub trait Source {
    fn id(&self) -> &SourceId;
    fn kind(&self) -> Kind;
    fn display_name(&self) -> &str;
    fn list(&self, path: &str) -> Result<Page, SourceError>;
    fn open(&self, entry: &Entry) -> Result<MediaHandle, SourceError>;
    fn probe(&self, entry: &Entry) -> Result<Probe, SourceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_distinct() {
        assert_ne!(Kind::Local, Kind::Dlna);
        assert_ne!(Kind::Dlna, Kind::Smb);
    }
}
