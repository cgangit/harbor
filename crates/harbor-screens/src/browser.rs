//! The Browser surface and its listing pipeline (design D3/D4): a worker
//! thread posts the first `Page` (or a failure class) into a shared queue,
//! the app loop drains it into `View` once per frame, and `draw`/`rows`
//! show a loading or failure row while a listing is in flight.

use std::sync::{Arc, Mutex};

use harbor_platform::{t, Lang};
use harbor_session::{CredentialVault, SourceConfig};
use harbor_sources::{DlnaSource, Kind, Page, SmbSource, Source};
use harbor_ui::draw::Surface;
use harbor_ui::glass::card;
use harbor_ui::label::Label;
use harbor_ui::theme::{SHELF_CARD_W, SHELF_GAP};
use harbor_ui::Focus;

use crate::catalog::{Catalog, AZ, SORTS};
use crate::nav::View;

/// One listing outcome: which source it belongs to and what arrived —
/// the first `Page`, or the failure class for the error row (D3/D4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowseResult {
    pub source_id: String,
    pub outcome: Result<Page, String>,
}

/// Shared browse queue (D3): a newtype because `Mutex<T>` never gets `Eq`,
/// which `View` derives; equality compares the queued results (the same-arc
/// shortcut avoids locking a mutex twice). Chosen over an `mpsc::Receiver`
/// inside `View` for exactly those derives.
#[derive(Clone, Debug)]
pub struct BrowseQueue(pub Arc<Mutex<Vec<BrowseResult>>>);

impl BrowseQueue {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }

    pub fn push(&self, result: BrowseResult) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(result);
    }

    /// Everything posted so far, emptied in one step — the drain per frame.
    pub fn take(&self) -> Vec<BrowseResult> {
        std::mem::take(&mut *self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl Default for BrowseQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for BrowseQueue {
    fn eq(&self, other: &Self) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
            == *other.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Eq for BrowseQueue {}

/// The source a listing runs against: `Send + Sync` so a worker thread can
/// hold it, with identity-based `Debug`/`PartialEq` so `View` keeps its
/// derives (same reasoning as `DiscoveryHandle`).
pub struct SourceHandle(pub Arc<dyn Source + Send + Sync>);

impl Clone for SourceHandle {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl std::fmt::Debug for SourceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SourceHandle").field(&self.0.id().0).finish()
    }
}

impl PartialEq for SourceHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.id() == other.0.id()
    }
}

impl Eq for SourceHandle {}

/// What the Browser shows besides the library list (D4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BrowseInfo<'a> {
    /// A first-page listing is in flight — its page has not landed yet.
    pub loading: bool,
    /// The failure class of the last listing, until the next start or a
    /// successful drain.
    pub error: Option<&'a str>,
}

impl<'a> BrowseInfo<'a> {
    pub fn of(view: &'a View) -> Self {
        Self {
            loading: !view.loading.is_empty(),
            error: view.browse_error.as_deref(),
        }
    }
}

/// Fold every posted result into `View` (D3): clear `loading` for that
/// source, fill `catalog.files` (and keep `page` for folder kinds/ids that
/// the flat name list loses), or record the failure class. Called once per
/// frame by the app loop; tests call it directly after posting.
pub fn drain(view: &mut View) {
    let Some(queue) = view.browse.clone() else {
        return;
    };
    for result in queue.take() {
        view.loading.retain(|id| *id != result.source_id);
        match result.outcome {
            Ok(page) => {
                view.browse_error = None;
                view.catalog.files = page.entries.iter().map(|e| e.name.clone()).collect();
                view.page = Some(page);
            }
            Err(class) => {
                view.browse_error = Some(class);
                view.page = None;
                view.catalog.files.clear();
            }
        }
    }
}

/// Spawn the first-page listing worker for `source` at `path` ("" is the
/// share/server root) — D3: the present loop never waits on the network;
/// D4: the id joins `loading` so the Browser draws the loading row until
/// the worker's post is drained.
pub fn start(view: &mut View, source: SourceHandle, path: &str) {
    let id = source.0.id().0.clone();
    view.loading.retain(|l| *l != id);
    view.loading.push(id.clone());
    view.browse_error = None;
    view.browsing = Some(source.clone());
    let queue = view.browse.get_or_insert_with(BrowseQueue::default).clone();
    let path = path.to_string();
    std::thread::spawn(move || {
        let outcome = match source.0.list(&path) {
            Ok(page) => Ok(page),
            Err(e) => Err(e.message()),
        };
        queue.push(BrowseResult {
            source_id: id,
            outcome,
        });
    });
}

/// The live handle for a configured source: DLNA and SMB list over the
/// network; local mounts are listed synchronously by `refresh_catalog`, so
/// they need no worker.
pub fn source_for(cfg: &SourceConfig, vault: &CredentialVault) -> Option<SourceHandle> {
    match cfg.kind {
        Kind::Dlna => cfg
            .dlna_server()
            .map(|server| SourceHandle(Arc::new(DlnaSource::from_server(server)))),
        Kind::Smb => cfg
            .smb_creds(vault.get(&cfg.id).unwrap_or(""))
            .map(|creds| SourceHandle(Arc::new(SmbSource::networked(creds)))),
        Kind::Local => None,
    }
}

/// The configured network source displayed as `name` — the lookup both
/// selection triggers share (Settings source row, Home sources shelf).
pub fn network_source(view: &View, name: &str) -> Option<SourceHandle> {
    let cfg = view.sources.list().iter().find(|c| c.name == name)?;
    source_for(cfg, &view.vault)
}

pub fn draw(s: &mut impl Surface, catalog: &Catalog, focus: Focus, browse: BrowseInfo) {
    draw_lang(s, catalog, focus, browse, Lang::En);
}

pub fn draw_lang(
    s: &mut impl Surface,
    catalog: &Catalog,
    focus: Focus,
    browse: BrowseInfo,
    lang: Lang,
) {
    Label::new(80, 36, t(lang, "browser")).headline().draw(s);
    Label::new(80, 96, t(lang, "sort")).caption().muted().draw(s);
    let sorts = [t(lang, "name"), t(lang, "date"), t(lang, "size")];
    for (i, name) in sorts.iter().enumerate() {
        let x = 80 + i as i32 * 180;
        card(s, x, 130, 160, 56, focus.row == 0 && focus.col == i);
        Label::new(x + 16, 144, name).caption().draw(s);
    }
    Label::new(80, 210, "A-Z").caption().muted().draw(s);
    for (i, letter) in AZ.iter().enumerate() {
        let x = 80 + i as i32 * 68;
        card(s, x, 244, 52, 52, focus.row == 1 && focus.col == i);
        Label::new(x + 14, 256, letter).caption().draw(s);
    }
    Label::new(80, 320, t(lang, "library")).headline().draw(s);
    // D4: the failure row wins, then the loading row stands in for the
    // library list while the first page is on its way.
    if let Some(err) = browse.error {
        card(s, 80, 370, SHELF_CARD_W * 2, 56, focus.row == 2 && focus.col == 0);
        let text: String = err.chars().take(64).collect();
        Label::new(100, 384, &text).caption().draw(s);
    } else if browse.loading {
        card(s, 80, 370, SHELF_CARD_W * 2, 56, focus.row == 2 && focus.col == 0);
        Label::new(100, 384, t(lang, "loading")).caption().draw(s);
    } else {
        for (i, name) in catalog.files.iter().enumerate() {
            let y = 370 + i as i32 * (56 + SHELF_GAP / 2);
            card(
                s,
                80,
                y,
                SHELF_CARD_W * 2,
                56,
                focus.row == 2 && focus.col == i,
            );
            Label::new(100, y + 14, name).caption().draw(s);
        }
    }
}

/// The focus grid for this screen: the library row counts whichever of the
/// page / loading / failure rows is showing, so focus can never leave it.
pub fn rows(catalog: &Catalog, browse: BrowseInfo) -> Vec<usize> {
    let library = if browse.error.is_some() || browse.loading {
        1
    } else {
        catalog.files.len().max(1)
    };
    vec![SORTS.len(), AZ.len(), library]
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use harbor_sources::error::SourceError;
    use harbor_sources::{Entry, EntryKind, SourceId};
    use harbor_ui::draw::Recorder;

    fn video(name: &str) -> Entry {
        Entry {
            id: name.to_string(),
            parent: String::new(),
            name: name.to_string(),
            kind: EntryKind::Video,
            size: Some(9),
            mtime: None,
            url: None,
        }
    }

    fn folder(name: &str) -> Entry {
        Entry {
            kind: EntryKind::Dir,
            size: None,
            ..video(name)
        }
    }

    fn page_of(entries: Vec<Entry>) -> Page {
        Page { entries }
    }

    #[test]
    fn browser_paints_sort_az_and_files() {
        let mut r = Recorder::default();
        draw(
            &mut r,
            &Catalog::fixture(),
            Focus { row: 2, col: 0 },
            BrowseInfo::default(),
        );
        let texts = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Name"));
        assert!(texts.contains(&"Date"));
        assert!(texts.contains(&"A"));
        assert!(texts.contains(&"Z"));
        assert!(texts.contains(&"the-river.mkv"));
    }

    /// Task 4.1: a posted fixture page reaches `View` after one drain, and
    /// a posted failure becomes the error class with `loading` cleared.
    #[test]
    fn posted_results_land_in_view_after_one_drain() {
        let mut v = View::default();
        let queue = BrowseQueue::new();
        v.browse = Some(queue.clone());
        v.loading.push("smb:nas/media".into());

        queue.push(BrowseResult {
            source_id: "smb:nas/media".into(),
            outcome: Ok(page_of(vec![folder("Movies"), video("a.mkv")])),
        });
        drain(&mut v);

        assert!(v.loading.is_empty());
        assert!(v.browse_error.is_none());
        assert_eq!(v.catalog.files, vec!["Movies", "a.mkv"]);
        let page = v.page.as_ref().expect("page kept for folder kinds");
        assert_eq!(page.entries[0].kind, EntryKind::Dir);
        assert_eq!(page.entries[1].kind, EntryKind::Video);

        // A failure post clears the page and records the class instead.
        v.loading.push("smb:nas/media".into());
        queue.push(BrowseResult {
            source_id: "smb:nas/media".into(),
            outcome: Err("authentication failed: smb:nas/media".into()),
        });
        drain(&mut v);
        assert!(v.loading.is_empty());
        assert_eq!(
            v.browse_error.as_deref(),
            Some("authentication failed: smb:nas/media")
        );
        assert!(v.page.is_none());
        assert!(v.catalog.files.is_empty());
    }

    /// A stub source whose `list` blocks briefly (task 4.2): `handle` keeps
    /// answering while the worker sleeps, the loading row stands in for the
    /// library list and counts in `rows()` until the post arrives, then the
    /// drained page replaces it.
    #[test]
    fn loading_row_shows_until_post_and_input_keeps_responding() {
        struct StubSource {
            id: SourceId,
            delay: Duration,
            page: Page,
        }
        impl Source for StubSource {
            fn id(&self) -> &SourceId {
                &self.id
            }
            fn kind(&self) -> Kind {
                Kind::Smb
            }
            fn display_name(&self) -> &str {
                "stub"
            }
            fn list(&self, _path: &str) -> Result<Page, SourceError> {
                std::thread::sleep(self.delay);
                Ok(self.page.clone())
            }
            fn open(&self, _entry: &Entry) -> Result<harbor_sources::handle::MediaHandle, SourceError> {
                Err(SourceError::NotFound {
                    path: String::new(),
                })
            }
            fn probe(&self, entry: &Entry) -> Result<harbor_sources::Probe, SourceError> {
                Ok(harbor_sources::Probe {
                    container: None,
                    video_codec: None,
                    audio_codec: None,
                    width: None,
                    height: None,
                    duration_ns: None,
                    size: entry.size,
                    bit_depth: None,
                    dovi: None,
                })
            }
        }

        let mut v = View::with_catalog(Catalog::fixture());
        let stub = SourceHandle(Arc::new(StubSource {
            id: SourceId("stub:one".into()),
            delay: Duration::from_millis(250),
            page: page_of(vec![video("stub.mkv")]),
        }));
        start(&mut v, stub, "");
        assert_eq!(v.loading, vec!["stub:one"]);

        // The listing runs on its worker: input keeps responding meanwhile.
        let moved = crate::handle(&v, harbor_platform::Key::Down);
        assert_ne!(moved.focus, v.focus);

        // Loading row: visible in draw, counted as the single library row.
        let info = BrowseInfo::of(&v);
        assert!(info.loading);
        assert!(info.error.is_none());
        assert_eq!(rows(&v.catalog, info), vec![SORTS.len(), AZ.len(), 1]);
        let mut r = Recorder::default();
        crate::draw(
            &mut r,
            crate::Screen::Browser,
            &v.catalog,
            v.focus,
            None,
            None,
            Some(info),
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t == "Loading..."),
            "loading row missing: {texts:?}"
        );

        // Wait for the worker's post; each drain is one frame's worth.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !v.loading.is_empty() {
            assert!(Instant::now() < deadline, "worker never posted");
            drain(&mut v);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(v.catalog.files, vec!["stub.mkv"]);
        assert!(v.browse_error.is_none());
        // The loading row is gone: the library row counts the page again.
        assert_eq!(
            rows(&v.catalog, BrowseInfo::of(&v)),
            vec![SORTS.len(), AZ.len(), 1]
        );
    }

    /// The failure row (D4): the class is drawn and `rows()` counts it, so
    /// focus stays valid while the list is replaced.
    #[test]
    fn failure_row_shows_the_class_and_counts() {
        let mut v = View::default();
        v.browse_error = Some("timed out: SMB connect".into());
        let info = BrowseInfo::of(&v);
        assert_eq!(info.error, Some("timed out: SMB connect"));
        assert_eq!(rows(&v.catalog, info), vec![SORTS.len(), AZ.len(), 1]);

        let mut r = Recorder::default();
        crate::draw(
            &mut r,
            crate::Screen::Browser,
            &v.catalog,
            v.focus,
            None,
            None,
            Some(info),
        );
        let texts: Vec<_> = r
            .ops
            .iter()
            .filter_map(|op| match op {
                harbor_ui::DrawOp::Text { text, .. } => Some(text.to_string()),
                _ => None,
            })
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("timed out: SMB connect")),
            "error row missing: {texts:?}"
        );
    }

    /// Task 5.2 LAN checklist, opt-in with `HARBOR_LAN_LIVE=1`: every
    /// network source goes through the real async path (`network_source` →
    /// `start` → `drain`) against a live MiniDLNA — SSDP discovery, or
    /// `HARBOR_DLN_LOCATION` to describe a known URL — and a live Samba
    /// (`HARBOR_SMB_ADDR`, default `127.0.0.1:1445`): guest share without a
    /// password, authenticated share with one, and a wrong password that
    /// must arrive as a failure class.
    #[test]
    fn lan_live_browses_dlna_and_smb_through_async_path() {
        if std::env::var("HARBOR_LAN_LIVE").as_deref() != Ok("1") {
            return;
        }

        fn wait_for_drain(v: &mut View, budget: Duration) {
            let deadline = Instant::now() + budget;
            while !v.loading.is_empty() {
                assert!(
                    Instant::now() < deadline,
                    "listing never posted: loading={:?} error={:?}",
                    v.loading,
                    v.browse_error
                );
                drain(v);
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        fn browse(view: &mut View, name: &str, path: &str, budget: Duration) {
            let source = network_source(view, name).expect("configured name resolves to a handle");
            start(view, source, path);
            assert!(!view.loading.is_empty());
            wait_for_drain(view, budget);
        }

        fn store_name(view: &View, id: &str) -> String {
            view.sources
                .get(id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| panic!("no source {id}"))
        }

        /// A page carrying a video resource URL (DLNA names may drop the
        /// extension — MiniDLNA titles are extension-less — so the `res`
        /// URL is what says "media here").
        fn has_media(v: &View) -> bool {
            v.page.as_ref().is_some_and(|p| {
                p.entries.iter().any(|e| {
                    e.url
                        .as_deref()
                        .is_some_and(|u| u.to_ascii_lowercase().ends_with(".mkv") || u.to_ascii_lowercase().ends_with(".mp4"))
                })
            })
        }

        // --- DLNA: live SSDP discovery, then the discovered server lists.
        let server = match std::env::var("HARBOR_DLN_LOCATION") {
            Ok(location) => harbor_sources::describe(&location, Duration::from_secs(5))
                .expect("HARBOR_DLN_LOCATION describes a MediaServer"),
            Err(_) => {
                let mut found = harbor_sources::discover_media_servers(
                    harbor_sources::SSDP_MULTICAST,
                    Duration::from_secs(8),
                )
                .expect("live SSDP discovery");
                assert!(!found.is_empty(), "no MediaServer answered M-SEARCH");
                // Other devices on a real LAN may answer first; the
                // checklist pins the test server by friendly name when it
                // says so.
                if let Ok(want) = std::env::var("HARBOR_DLN_NAME") {
                    found.retain(|s| s.friendly_name == want);
                    assert!(
                        !found.is_empty(),
                        "no discovered MediaServer named {want:?}"
                    );
                }
                found.into_iter().next().unwrap()
            }
        };
        let mut v = View::default();
        v.sources.put(SourceConfig::dlna(&server));
        v.catalog = crate::build_catalog(&v.sources, &[], &[]);
        let dlna_name = server.friendly_name.clone();
        browse(&mut v, &dlna_name, "", Duration::from_secs(15));
        assert!(
            v.browse_error.is_none(),
            "DLNA browse failed: {:?}",
            v.browse_error
        );
        // MiniDLNA puts its content behind a "Browse Folders" container and
        // titles entries extension-less; other servers list media at the
        // root. Walk down (the follow-up `list(path)` path) until a video
        // resource shows up — at most four levels deep.
        for _ in 0..4 {
            if has_media(&v) {
                break;
            }
            let folder = v
                .page
                .as_ref()
                .and_then(|p| p.entries.iter().find(|e| e.kind == EntryKind::Dir))
                .map(|e| e.id.clone());
            let Some(folder) = folder else {
                break;
            };
            browse(&mut v, &dlna_name, &folder, Duration::from_secs(15));
            assert!(
                v.browse_error.is_none(),
                "DLNA drill-down failed: {:?}",
                v.browse_error
            );
        }
        assert!(
            has_media(&v),
            "no .mkv/.mp4 resource within four folders: {:?} (page {:?})",
            v.catalog.files,
            v.page.as_ref().map(|p| p.entries.len())
        );
        assert!(v.page.is_some(), "page kept for folder drill-down");

        // --- SMB without password: the guest share lists.
        let addr = std::env::var("HARBOR_SMB_ADDR").unwrap_or_else(|_| "127.0.0.1:1445".into());
        let guest_id = harbor_session::smb_id(&addr, "guest");
        let mut g = View::default();
        g.sources
            .put(SourceConfig::smb(&guest_id, &addr, "guest", "", ""));
        g.catalog = crate::build_catalog(&g.sources, &[], &[]);
        let guest_name = store_name(&g, &guest_id);
        browse(&mut g, &guest_name, "", Duration::from_secs(15));
        assert!(
            g.browse_error.is_none(),
            "guest browse failed: {:?}",
            g.browse_error
        );
        assert!(
            g.catalog.files.iter().any(|f| f == "movie.mkv"),
            "guest page missing movie.mkv: {:?}",
            g.catalog.files
        );

        // --- SMB with password: the private share lists for `harbor`.
        let priv_id = harbor_session::smb_id(&addr, "private");
        let mut p = View::default();
        p.sources.put(SourceConfig::smb(
            &priv_id,
            &addr,
            "private",
            "harbor",
            "",
        ));
        p.vault.put(&priv_id, "harborpass");
        p.catalog = crate::build_catalog(&p.sources, &[], &[]);
        let priv_name = store_name(&p, &priv_id);
        browse(&mut p, &priv_name, "", Duration::from_secs(15));
        assert!(
            p.browse_error.is_none(),
            "authenticated browse failed: {:?}",
            p.browse_error
        );
        assert!(
            p.catalog.files.iter().any(|f| f == "private-movie.mkv"),
            "private page missing private-movie.mkv: {:?}",
            p.catalog.files
        );

        // --- Wrong password: the failure class arrives through the same
        // path, with no page left behind.
        let bad_id = harbor_session::smb_id(&addr, "bad-pass");
        let mut b = View::default();
        b.sources.put(SourceConfig::smb(
            &bad_id,
            &addr,
            "private",
            "harbor",
            "",
        ));
        b.vault.put(&bad_id, "wrong");
        b.catalog = crate::build_catalog(&b.sources, &[], &[]);
        let bad_name = store_name(&b, &bad_id);
        browse(&mut b, &bad_name, "", Duration::from_secs(15));
        assert!(
            b.browse_error
                .as_deref()
                .is_some_and(|e| e.contains("authentication failed")),
            "expected an auth failure class, got {:?}",
            b.browse_error
        );
        assert!(b.catalog.files.is_empty());
        assert!(b.page.is_none());
    }
}
