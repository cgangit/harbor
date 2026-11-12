use std::collections::BTreeMap;
use std::time::Duration;

use crate::entry::{ext_of, is_video_name, Entry, EntryKind, Page, Probe};
use crate::error::SourceError;
use crate::handle::MediaHandle;
use crate::{Kind, Source, SourceId};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const CONNECT_TIMEOUT_MS: u64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmbCreds {
    pub host: String,
    pub share: String,
    pub username: String,
    pub domain: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmbDirent {
    pub name: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct SmbSource {
    pub id: SourceId,
    pub name: String,
    pub creds: SmbCreds,
    dirs: BTreeMap<String, Vec<SmbDirent>>,
    files: BTreeMap<String, Vec<u8>>,
    fail: Option<SourceError>,
}

impl SmbCreds {
    pub fn new(host: &str, share: &str, username: &str, domain: &str, password: &str) -> Self {
        Self {
            host: host.to_string(),
            share: share.to_string(),
            username: username.to_string(),
            domain: domain.to_string(),
            password: password.to_string(),
        }
    }
}

impl SmbSource {
    pub fn new(creds: SmbCreds) -> Self {
        let id = format!("smb:{}/{}", creds.host, creds.share);
        let name = if creds.username.is_empty() {
            format!("{}/{}", creds.host, creds.share)
        } else {
            format!("{}@{}/{}", creds.username, creds.host, creds.share)
        };
        let mut dirs = BTreeMap::new();
        dirs.insert(String::new(), Vec::new());
        Self {
            id: SourceId(id),
            name,
            creds,
            dirs,
            files: BTreeMap::new(),
            fail: None,
        }
    }

    pub fn fixture(creds: SmbCreds) -> Self {
        let mut src = Self::new(creds);
        src.add_dir("", "Movies");
        src.add_dir("", "Shows");
        src.add_file("Movies", "the-river.mkv", b"fake-mkv");
        src.add_file("Movies", "notes.txt", b"ignore");
        src.add_file("", "harbor-demo.mp4", b"fake-mp4");
        src
    }

    pub fn auth_fail(creds: SmbCreds) -> Self {
        let mut src = Self::new(creds);
        src.fail = Some(SourceError::Auth {
            source: src.id.0.clone(),
        });
        src
    }

    pub fn timed_out(creds: SmbCreds) -> Self {
        let mut src = Self::new(creds);
        src.fail = Some(SourceError::Timeout {
            what: "SMB connect".into(),
        });
        src
    }

    pub fn add_dir(&mut self, parent: &str, name: &str) {
        let parent = canonical_path(parent).unwrap_or_default();
        let child = join_path(&parent, name);
        self.dirs.entry(parent.clone()).or_default().push(SmbDirent {
            name: name.to_string(),
            kind: EntryKind::Dir,
            size: None,
            mtime: None,
        });
        self.dirs.entry(child).or_default();
    }

    pub fn add_file(&mut self, parent: &str, name: &str, data: &[u8]) {
        let parent = canonical_path(parent).unwrap_or_default();
        let child = join_path(&parent, name);
        let kind = if is_video_name(name) {
            EntryKind::Video
        } else {
            EntryKind::Other
        };
        self.dirs.entry(parent).or_default().push(SmbDirent {
            name: name.to_string(),
            kind,
            size: Some(data.len() as u64),
            mtime: None,
        });
        self.files.insert(child, data.to_vec());
    }

    fn check(&self) -> Result<(), SourceError> {
        match &self.fail {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

impl Source for SmbSource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn kind(&self) -> Kind {
        Kind::Smb
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn list(&self, path: &str) -> Result<Page, SourceError> {
        self.check()?;
        let path = canonical_path(path)?;
        let dirents = self.dirs.get(&path).ok_or_else(|| SourceError::NotFound {
            path: path.clone(),
        })?;
        let mut entries = Vec::new();
        for d in dirents {
            if d.kind == EntryKind::Other {
                continue;
            }
            let id = join_path(&path, &d.name);
            entries.push(Entry {
                id,
                parent: path.clone(),
                name: d.name.clone(),
                kind: d.kind,
                size: d.size,
                mtime: d.mtime,
                url: None,
            });
        }
        entries.sort_by(|a, b| match (a.kind, b.kind) {
            (EntryKind::Dir, EntryKind::Dir) | (EntryKind::Video, EntryKind::Video) => {
                a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase())
            }
            (EntryKind::Dir, _) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Greater,
        });
        Ok(Page { entries })
    }

    fn open(&self, entry: &Entry) -> Result<MediaHandle, SourceError> {
        self.check()?;
        let path = canonical_path(&entry.id)?;
        let data = self
            .files
            .get(&path)
            .cloned()
            .ok_or_else(|| SourceError::NotFound { path })?;
        Ok(MediaHandle::open_smb(data))
    }

    fn probe(&self, entry: &Entry) -> Result<Probe, SourceError> {
        self.check()?;
        let path = canonical_path(&entry.id)?;
        let size = self.files.get(&path).map(|d| d.len() as u64).or(entry.size);
        Ok(Probe {
            container: ext_of(&entry.name),
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration_ns: None,
            size,
            bit_depth: None,
            dovi: None,
        })
    }
}

pub fn canonical_path(path: &str) -> Result<String, SourceError> {
    let mut parts: Vec<&str> = Vec::new();
    for raw in path.split(['/', '\\']) {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw == ".." {
            return Err(SourceError::Escape {
                path: path.to_string(),
            });
        }
        if raw.contains('\0') {
            return Err(SourceError::Escape {
                path: path.to_string(),
            });
        }
        parts.push(raw);
    }
    Ok(parts.join("/"))
}

pub fn join_path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

pub fn classify_nt_status(status: u32) -> Option<SourceError> {
    match status {
        0xC000006D | 0xC000006A | 0xC0000072 | 0xC0000234 => Some(SourceError::Auth {
            source: "smb".into(),
        }),
        _ => None,
    }
}

pub fn classify_connect(elapsed: Duration, budget: Duration, timed_out: bool) -> Option<SourceError> {
    if timed_out || elapsed >= budget {
        Some(SourceError::Timeout {
            what: "SMB connect".into(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn creds() -> SmbCreds {
        SmbCreds::new("nas", "media", "alice", "HOME", "s3cret")
    }

    #[test]
    fn canonicalizes_and_rejects_parent() {
        assert_eq!(canonical_path("").unwrap(), "");
        assert_eq!(canonical_path("/").unwrap(), "");
        assert_eq!(canonical_path("Movies").unwrap(), "Movies");
        assert_eq!(canonical_path("/Movies/foo.mkv").unwrap(), "Movies/foo.mkv");
        assert_eq!(
            canonical_path("Movies\\foo.mkv").unwrap(),
            "Movies/foo.mkv"
        );
        assert_eq!(
            canonical_path("//Movies//./a.mkv").unwrap(),
            "Movies/a.mkv"
        );
        match canonical_path("Movies/../secret") {
            Err(SourceError::Escape { .. }) => {}
            other => panic!("{other:?}"),
        }
        match canonical_path("..") {
            Err(SourceError::Escape { .. }) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn lists_dirs_and_videos_skips_other() {
        let src = SmbSource::fixture(creds());
        assert_eq!(src.kind(), Kind::Smb);
        assert_eq!(src.display_name(), "alice@nas/media");
        let page = src.list("").unwrap();
        let names: Vec<_> = page.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Movies"));
        assert!(names.contains(&"Shows"));
        assert!(names.contains(&"harbor-demo.mp4"));
        assert!(!names.contains(&"notes.txt"));
        let movies = src.list("Movies").unwrap();
        assert_eq!(movies.entries.len(), 1);
        assert_eq!(movies.entries[0].name, "the-river.mkv");
        assert_eq!(movies.entries[0].kind, EntryKind::Video);
        assert_eq!(movies.entries[0].id, "Movies/the-river.mkv");
    }

    #[test]
    fn open_read_at_and_probe() {
        let src = SmbSource::fixture(creds());
        let page = src.list("Movies").unwrap();
        let entry = &page.entries[0];
        let mut h = src.open(entry).unwrap();
        assert_eq!(h.len(), 8);
        let mut buf = [0u8; 4];
        assert_eq!(h.read_at(0, &mut buf).unwrap(), 4);
        assert_eq!(&buf, b"fake");
        assert_eq!(h.read_at(5, &mut buf).unwrap(), 3);
        assert_eq!(&buf[..3], b"mkv");
        let p = src.probe(entry).unwrap();
        assert_eq!(p.container.as_deref(), Some("mkv"));
        assert_eq!(p.size, Some(8));
    }

    #[test]
    fn parent_dir_escape_rejected() {
        let src = SmbSource::fixture(creds());
        let err = src.list("../").unwrap_err();
        match err {
            SourceError::Escape { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn auth_fail_and_timeout() {
        let err = SmbSource::auth_fail(creds()).list("").unwrap_err();
        match err {
            SourceError::Auth { ref source } => assert_eq!(source, "smb:nas/media"),
            other => panic!("{other:?}"),
        }
        assert_eq!(err.message(), "authentication failed: smb:nas/media");
        let err = SmbSource::timed_out(creds()).open(&Entry {
            id: "a.mkv".into(),
            parent: String::new(),
            name: "a.mkv".into(),
            kind: EntryKind::Video,
            size: None,
            mtime: None,
            url: None,
        });
        let err = err.unwrap_err();
        match err {
            SourceError::Timeout { what } => assert_eq!(what, "SMB connect"),
            other => panic!("{other:?}"),
        }
        assert_eq!(CONNECT_TIMEOUT, Duration::from_millis(CONNECT_TIMEOUT_MS));
        assert_eq!(CONNECT_TIMEOUT.as_secs(), 10);
        let mapped = classify_nt_status(0xC000006D).unwrap();
        match mapped {
            SourceError::Auth { .. } => {}
            other => panic!("{other:?}"),
        }
        let t = classify_connect(Duration::from_secs(10), CONNECT_TIMEOUT, true).unwrap();
        match t {
            SourceError::Timeout { what } => assert_eq!(what, "SMB connect"),
            other => panic!("{other:?}"),
        }
        assert!(classify_connect(Duration::from_secs(1), CONNECT_TIMEOUT, false).is_none());
    }
}
