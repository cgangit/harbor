use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    Video,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub parent: String,
    pub name: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ns: Option<i64>,
    pub size: Option<u64>,
    pub bit_depth: Option<u8>,
    pub dovi: Option<u8>,
}

pub const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "avi", "ts", "m2ts", "webm",
];

pub fn is_video_name(name: &str) -> bool {
    ext_of(name)
        .map(|e| VIDEO_EXTS.iter().any(|v| *v == e))
        .unwrap_or(false)
}

pub fn ext_of(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_containers() {
        for name in [
            "a.mkv", "b.MP4", "c.m4v", "d.mov", "e.avi", "f.ts", "g.m2ts", "h.webm",
        ] {
            assert!(is_video_name(name), "{name}");
        }
        assert!(!is_video_name("notes.txt"));
        assert!(!is_video_name("folder"));
    }
}
