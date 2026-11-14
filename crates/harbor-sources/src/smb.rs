use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use smb2::{ClientConfig, DirectoryEntry, Error as Smb2Error, ErrorKind as Smb2Kind, SmbClient};

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

#[derive(Debug)]
pub struct SmbSource {
    pub id: SourceId,
    pub name: String,
    pub creds: SmbCreds,
    /// `path` → dirents. Fixture constructors fill it up front; a networked
    /// source fills it from `list` results. A mutex (not a plain map) because
    /// `Source::list` takes `&self` and browse workers list from another
    /// thread (design D3, same reason as `DlnaSource::pages`).
    dirs: Mutex<BTreeMap<String, Vec<SmbDirent>>>,
    files: BTreeMap<String, Vec<u8>>,
    fail: Option<SourceError>,
    /// Set only by [`SmbSource::networked`]: `list` then connects over SMB2
    /// instead of reading the in-memory fixture maps (task 3.2).
    network: bool,
}

impl Clone for SmbSource {
    fn clone(&self) -> Self {
        let dirs = self.dirs.lock().unwrap_or_else(|e| e.into_inner());
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            creds: self.creds.clone(),
            dirs: Mutex::new(dirs.clone()),
            files: self.files.clone(),
            fail: self.fail.clone(),
            network: self.network,
        }
    }
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
            dirs: Mutex::new(dirs),
            files: BTreeMap::new(),
            fail: None,
            network: false,
        }
    }

    /// A live SMB source (task 3.2): `list` connects to the server and
    /// readdirs over SMB2, caching each listing in `dirs`. The empty root
    /// entry `new` seeds for fixtures is dropped so the first root listing
    /// is a cache miss, not a cached empty page.
    pub fn networked(creds: SmbCreds) -> Self {
        let mut src = Self::new(creds);
        src.dirs.lock().unwrap_or_else(|e| e.into_inner()).clear();
        src.network = true;
        src
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
        let mut dirs = self.dirs.lock().unwrap_or_else(|e| e.into_inner());
        dirs.entry(parent.clone()).or_default().push(SmbDirent {
            name: name.to_string(),
            kind: EntryKind::Dir,
            size: None,
            mtime: None,
        });
        dirs.entry(child).or_default();
    }

    pub fn add_file(&mut self, parent: &str, name: &str, data: &[u8]) {
        let parent = canonical_path(parent).unwrap_or_default();
        let child = join_path(&parent, name);
        let kind = if is_video_name(name) {
            EntryKind::Video
        } else {
            EntryKind::Other
        };
        let mut dirs = self.dirs.lock().unwrap_or_else(|e| e.into_inner());
        dirs.entry(parent).or_default().push(SmbDirent {
            name: name.to_string(),
            kind,
            size: Some(data.len() as u64),
            mtime: None,
        });
        drop(dirs);
        self.files.insert(child, data.to_vec());
    }

    fn check(&self) -> Result<(), SourceError> {
        match &self.fail {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    /// The `ClientConfig` for this source's credentials (task 3.2). The
    /// form's host may omit the port; SMB lives on 445 (tests pass an
    /// explicit `host:port`).
    fn client_config(&self) -> ClientConfig {
        ClientConfig {
            addr: smb_addr(&self.creds.host),
            username: self.creds.username.clone(),
            password: self.creds.password.clone(),
            domain: self.creds.domain.clone(),
            timeout: CONNECT_TIMEOUT,
            ..ClientConfig::default()
        }
    }

    /// Connect, tree-connect, and readdir one path over SMB2, caching the
    /// result for the next `list` (task 3.2). Runs on the caller's thread —
    /// the browse queue (group 4) is what keeps it off the present loop.
    fn readdir(&self, path: &str) -> Result<Vec<SmbDirent>, SourceError> {
        let started = Instant::now();
        let found = futures_lite::future::block_on(async {
            let mut client = SmbClient::connect(self.client_config())
                .await
                .map_err(|e| self.classify_smb(&e, path, started.elapsed()))?;
            let mut tree = client
                .connect_share(&self.creds.share)
                .await
                .map_err(|e| self.classify_smb(&e, path, started.elapsed()))?;
            client
                .list_directory(&mut tree, path)
                .await
                .map_err(|e| self.classify_smb(&e, path, started.elapsed()))
        })?;
        let dirents: Vec<SmbDirent> = found.iter().map(dirent_of).collect();
        self.dirs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(path.to_string(), dirents.clone());
        Ok(dirents)
    }

    /// Route an `smb2::Error` through the existing classifiers (task 3.2)
    /// by extracting its primitive signals first — the crate's `Error` is
    /// `#[non_exhaustive]` and cannot be built in tests, so the routing
    /// rules live in [`classify_smb_failure`].
    fn classify_smb(&self, err: &Smb2Error, path: &str, elapsed: Duration) -> SourceError {
        let timed_out = match err {
            Smb2Error::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ),
            other => other.kind() == Smb2Kind::TimedOut,
        };
        let auth = matches!(
            err.kind(),
            Smb2Kind::AuthRequired | Smb2Kind::AccessDenied | Smb2Kind::GuestSessionRefused
        );
        classify_smb_failure(
            err.status().map(|s| s.0),
            timed_out,
            auth,
            err.to_string(),
            path,
            elapsed,
            &self.id.0,
        )
    }
}

/// The routing rules for one SMB failure (task 3.2), as primitives so they
/// are unit-testable: NTSTATUS first (`classify_nt_status`), then the
/// connect budget (`classify_connect`), then the auth flag, and finally the
/// crate's own message.
fn classify_smb_failure(
    status: Option<u32>,
    timed_out: bool,
    auth: bool,
    message: String,
    path: &str,
    elapsed: Duration,
    source: &str,
) -> SourceError {
    if let Some(code) = status {
        if let Some(mapped) = classify_nt_status(code) {
            return mapped;
        }
        // STATUS_OBJECT_NAME_NOT_FOUND / STATUS_OBJECT_PATH_NOT_FOUND /
        // STATUS_BAD_NETWORK_NAME: the path or share simply isn't there.
        if matches!(code, 0xC000_0034 | 0xC000_003A | 0xC000_00CC) {
            return SourceError::NotFound {
                path: path.to_string(),
            };
        }
    }
    if let Some(mapped) = classify_connect(elapsed, CONNECT_TIMEOUT, timed_out) {
        return mapped;
    }
    if auth {
        return SourceError::Auth {
            source: source.to_string(),
        };
    }
    SourceError::Io(message)
}

/// `host` from the Add form may omit the port; SMB's default is 445. An
/// explicit `host:port` (tests, non-standard ports) passes through.
fn smb_addr(host: &str) -> String {
    let has_port = host.rsplit_once(':').is_some_and(|(_, port)| {
        !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
    });
    if has_port {
        host.to_string()
    } else {
        format!("{host}:445")
    }
}

/// One server dirent → the in-memory shape both listing paths share.
fn dirent_of(e: &DirectoryEntry) -> SmbDirent {
    let kind = if e.is_directory {
        EntryKind::Dir
    } else if is_video_name(&e.name) {
        EntryKind::Video
    } else {
        EntryKind::Other
    };
    SmbDirent {
        name: e.name.clone(),
        kind,
        size: (!e.is_directory).then_some(e.size),
        mtime: if e.is_directory {
            None
        } else {
            e.modified
                .to_system_time()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
        },
    }
}

/// Dirents → the `Page` both listing paths return: dot entries and
/// non-media (`EntryKind::Other`) skipped, dirs first, then name order.
fn page_from(path: &str, dirents: &[SmbDirent]) -> Page {
    let mut entries = Vec::new();
    for d in dirents {
        if d.kind == EntryKind::Other || d.name == "." || d.name == ".." {
            continue;
        }
        entries.push(Entry {
            id: join_path(path, &d.name),
            parent: path.to_string(),
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
    Page { entries }
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
        if self.network {
            // Cache first (a repeat or back-navigation is offline), then the
            // network path: connect + readdir, errors classified (task 3.2).
            let cached = self
                .dirs
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&path)
                .cloned();
            let dirents = match cached {
                Some(dirents) => dirents,
                None => self.readdir(&path)?,
            };
            return Ok(page_from(&path, &dirents));
        }
        let dirents = self
            .dirs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&path)
            .cloned()
            .ok_or_else(|| SourceError::NotFound {
                path: path.clone(),
            })?;
        Ok(page_from(&path, &dirents))
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

    /// The routing rules of task 3.2: NTSTATUS through `classify_nt_status`,
    /// the budget through `classify_connect`, auth next, message last.
    /// (`smb2::Error` is `#[non_exhaustive]`, so the rules take primitives.)
    #[test]
    fn classify_smb_failure_routes_status_budget_and_auth() {
        let elapsed = Duration::from_millis(3);
        // STATUS_LOGON_FAILURE → the existing NT-status classifier.
        let auth = classify_smb_failure(
            Some(0xC000_006D),
            false,
            false,
            "login".into(),
            "",
            elapsed,
            "smb:nas/media",
        );
        match auth {
            SourceError::Auth { source } => assert_eq!(source, "smb"),
            other => panic!("{other:?}"),
        }
        // STATUS_BAD_NETWORK_NAME → missing share, not a transport failure.
        let gone = classify_smb_failure(
            Some(0xC000_00CC),
            false,
            false,
            "tree".into(),
            "share",
            elapsed,
            "smb",
        );
        match gone {
            SourceError::NotFound { path } => assert_eq!(path, "share"),
            other => panic!("{other:?}"),
        }
        // Timed out inside the 10 s budget → the existing connect classifier.
        let slow = classify_smb_failure(
            None,
            true,
            false,
            "slow".into(),
            "",
            elapsed,
            "smb",
        );
        match slow {
            SourceError::Timeout { what } => assert_eq!(what, "SMB connect"),
            other => panic!("{other:?}"),
        }
        // Guest/permission refusals without a mapped status → Auth.
        let refused = classify_smb_failure(
            None,
            false,
            true,
            "refused".into(),
            "",
            elapsed,
            "smb:nas/media",
        );
        match refused {
            SourceError::Auth { source } => assert_eq!(source, "smb:nas/media"),
            other => panic!("{other:?}"),
        }
        // Anything else keeps the crate's message.
        match classify_smb_failure(None, false, false, "boom".into(), "", elapsed, "smb") {
            SourceError::Io(msg) => assert_eq!(msg, "boom"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smb_addr_defaults_port_and_keeps_explicit() {
        assert_eq!(smb_addr("nas"), "nas:445");
        assert_eq!(smb_addr("nas.local"), "nas.local:445");
        assert_eq!(smb_addr("127.0.0.1:1445"), "127.0.0.1:1445");
        assert_eq!(smb_addr("192.168.1.9:1"), "192.168.1.9:1");
    }

    /// The network path exists (task 3.2): a dead port surfaces a classified
    /// error — a fixture would answer `NotFound` from the empty map without
    /// dialing anything.
    #[test]
    fn networked_list_dials_and_classifies_dead_host() {
        let src = SmbSource::networked(SmbCreds::new("127.0.0.1:1", "share", "u", "", "p"));
        match src.list("") {
            Err(SourceError::Io(_)) => {}
            other => panic!("expected classified Io from a dead host, got {other:?}"),
        }
    }

    /// Cached listings are served without dialing (the dead host would
    /// error); only a cache miss takes the network path.
    #[test]
    fn networked_cache_serves_without_network() {
        let mut src = SmbSource::networked(SmbCreds::new("127.0.0.1:1", "share", "u", "", "p"));
        src.add_dir("", "Movies");
        let page = src.list("").unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].name, "Movies");
        // Cache miss on an uncached path dials the (dead) server.
        match src.list("Elsewhere") {
            Err(SourceError::Io(_)) => {}
            other => panic!("expected Io on a cache miss, got {other:?}"),
        }
    }

    /// Opt-in against a local Samba (task 3.2): `HARBOR_SMB_LIVE=1`, with
    /// `HARBOR_SMB_ADDR=host:port` if it is not on `127.0.0.1:1445`.
    /// Covers guest readdir, NTLMv2 with a password, wrong-password
    /// classification, and a missing share — the spike criteria end to end.
    #[test]
    fn live_smb_connect_readdir_and_cache() {
        if std::env::var("HARBOR_SMB_LIVE").as_deref() != Ok("1") {
            return;
        }
        let addr = std::env::var("HARBOR_SMB_ADDR").unwrap_or_else(|_| "127.0.0.1:1445".into());

        // Guest (no password): the share lists, dot entries are filtered.
        let guest = SmbSource::networked(SmbCreds::new(&addr, "guest", "", "", ""));
        let page = guest.list("").unwrap();
        let names: Vec<_> = page.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"movie.mkv"), "{names:?}");
        assert!(page.entries.iter().all(|e| e.name != "." && e.name != ".."));
        // Second listing comes from the cache.
        assert_eq!(guest.list("").unwrap(), page);
        // A subdirectory readdirs over the wire.
        let movies = guest.list("Movies").unwrap();
        assert!(
            movies.entries.iter().any(|e| e.name == "episode.mkv"),
            "{movies:?}"
        );

        // NTLMv2 with a password: the private share opens for `harbor`.
        let ok = SmbSource::networked(SmbCreds::new(
            &addr,
            "private",
            "harbor",
            "",
            "harborpass",
        ));
        let priv_page = ok.list("").unwrap();
        assert!(
            priv_page.entries.iter().any(|e| e.name == "private-movie.mkv"),
            "{priv_page:?}"
        );
        // Wrong password → STATUS_LOGON_FAILURE routed to Auth.
        let bad = SmbSource::networked(SmbCreds::new(&addr, "private", "harbor", "", "wrong"));
        match bad.list("") {
            Err(SourceError::Auth { source }) => assert_eq!(source, "smb"),
            other => panic!("expected Auth, got {other:?}"),
        }
        // Missing share → STATUS_BAD_NETWORK_NAME routed to NotFound.
        let nosuch = SmbSource::networked(SmbCreds::new(
            &addr,
            "no-such-share",
            "harbor",
            "",
            "harborpass",
        ));
        match nosuch.list("") {
            Err(SourceError::NotFound { path }) => assert_eq!(path, ""),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }
}
