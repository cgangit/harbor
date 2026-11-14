use harbor_session::{Resume, SourceStore};
use harbor_sources::{LocalSource, Source};

use crate::nav::View;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub continue_watching: Vec<String>,
    pub recently_added: Vec<String>,
    pub sources: Vec<String>,
    pub files: Vec<String>,
}

impl Catalog {
    /// Test-only sample data. Production catalogs come from
    /// `harbor_screens::refresh_catalog` (real sources + resume store).
    #[cfg(test)]
    pub fn fixture() -> Self {
        Self {
            continue_watching: vec!["The River".into(), "Night Train".into()],
            recently_added: vec![
                "Harbor Demo".into(),
                "Coastline".into(),
                "Archive Tape".into(),
            ],
            sources: vec![
                "USB Drive".into(),
                "Living Room NAS".into(),
                "SMB Share".into(),
            ],
            files: vec![
                "Movies".into(),
                "Shows".into(),
                "the-river.mkv".into(),
                "night-train.mp4".into(),
                "harbor-demo.mkv".into(),
            ],
        }
    }

    pub fn empty() -> Self {
        Self {
            continue_watching: Vec::new(),
            recently_added: Vec::new(),
            sources: Vec::new(),
            files: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }
}

pub const SORTS: [&str; 3] = ["Name", "Date", "Size"];

/// Rebuild `view.catalog` from the view's own state: configured sources,
/// discovered local mounts, and the resume store (design D2). Clears
/// `needs_refresh`. Local discovery only — network sources list lazily on a
/// worker thread (design D3), never here.
pub fn refresh_catalog(view: &mut View) {
    let mounts = harbor_sources::discover();
    view.catalog = build_catalog(&view.sources, &view.store.iter(), &mounts);
    view.needs_refresh = false;
    // The rebuilt catalog describes the local view again: a drained network
    // page or a stale failure class must not outlive it (the next listing
    // re-sets both through `browser::start` / `browser::drain`).
    view.browse_error = None;
    view.page = None;
}

/// Pure catalog construction — separated from `refresh_catalog` so tests can
/// feed fixture mounts without touching the process environment.
pub fn build_catalog(
    sources: &SourceStore,
    resumes: &[Resume],
    mounts: &[LocalSource],
) -> Catalog {
    let mut names: Vec<String> = sources.list().iter().map(|c| c.name.clone()).collect();
    // (mtime, name) so Recently Added can sort newest first
    let mut videos: Vec<(i64, String)> = Vec::new();
    for mount in mounts {
        let display = mount.display_name().to_string();
        if !names.contains(&display) {
            names.push(display);
        }
        collect_videos(mount, &mut videos);
    }
    videos.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let recently_added: Vec<String> = videos.iter().take(8).map(|(_, n)| n.clone()).collect();
    let files: Vec<String> = videos.into_iter().map(|(_, n)| n).collect();
    let continue_watching: Vec<String> = resumes.iter().map(|r| r.entry_id.clone()).collect();
    Catalog {
        continue_watching,
        recently_added,
        sources: names,
        files,
    }
}

/// Bounded breadth-first walk of one local mount: Recently Added has to see
/// videos inside `Movies/`-style folders, but must never scan an unbounded
/// tree on the UI thread (design D3 keeps network sources out of here entirely).
fn collect_videos(mount: &LocalSource, videos: &mut Vec<(i64, String)>) {
    const MAX_DEPTH: usize = 4;
    const MAX_VIDEOS: usize = 200;
    let mut queue: Vec<(String, usize)> = vec![(String::new(), 0)];
    while let Some((path, depth)) = queue.pop() {
        if videos.len() >= MAX_VIDEOS {
            return;
        }
        let Ok(page) = mount.list(&path) else {
            continue;
        };
        for entry in page.entries {
            match entry.kind {
                harbor_sources::EntryKind::Dir if depth < MAX_DEPTH => {
                    queue.push((entry.id, depth + 1));
                }
                harbor_sources::EntryKind::Video => {
                    videos.push((entry.mtime.unwrap_or(0), entry.name));
                }
                _ => {}
            }
            if videos.len() >= MAX_VIDEOS {
                return;
            }
        }
    }
}
pub const AZ: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
    "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
];
pub const CHROME: [&str; 3] = ["Home", "Browser", "Settings"];

#[cfg(test)]
mod tests {
    use super::*;
    use harbor_session::{Resume, SourceConfig};

    #[test]
    fn demo_has_sources_empty_does_not() {
        assert!(!Catalog::fixture().is_empty());
        assert!(Catalog::empty().is_empty());
        assert_eq!(Catalog::fixture().continue_watching.len(), 2);
    }

    #[test]
    fn refresh_rebuilds_sources_and_continue_watching() {
        let mut view = View::default();
        view.sources.put(SourceConfig::smb(
            "smb:nas/media",
            "nas",
            "media",
            "alice",
            "HOME",
        ));
        view.store.put(Resume {
            source_id: "demo".into(),
            entry_id: "the-river.mkv".into(),
            position_ns: 12_000_000_000,
            duration_ns: 90_000_000_000,
            updated_unix: 0,
        });
        view.needs_refresh = true;

        refresh_catalog(&mut view);

        assert!(!view.needs_refresh, "refresh must clear the flag");
        assert!(
            view.catalog.sources.iter().any(|n| n.contains("nas/media")),
            "configured source missing from {:?}",
            view.catalog.sources
        );
        assert_eq!(
            view.catalog.continue_watching,
            vec!["the-river.mkv".to_string()],
            "resume store must feed Continue Watching"
        );
    }

    #[test]
    fn build_catalog_lists_video_files_from_fixture_mount() {
        let dir = std::env::temp_dir().join(format!("harbor-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Movies")).unwrap();
        std::fs::write(dir.join("Movies/the-river.mkv"), b"fake").unwrap();
        std::fs::write(dir.join("harbor-demo.mp4"), b"fake").unwrap();
        std::fs::write(dir.join("notes.txt"), b"text").unwrap();

        let mount = harbor_sources::from_dir(&dir, "USB Drive");
        let cat = build_catalog(&SourceStore::in_memory(), &[], &[mount]);

        assert!(cat.sources.contains(&"USB Drive".to_string()));
        assert!(cat.files.contains(&"the-river.mkv".to_string()));
        assert!(cat.files.contains(&"harbor-demo.mp4".to_string()));
        assert!(
            !cat.files.iter().any(|f| f.ends_with(".txt")),
            "non-video entries must not be listed: {:?}",
            cat.files
        );
        assert_eq!(cat.recently_added.len(), 2, "videos feed Recently Added");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_catalog_degrades_to_empty_when_nothing_is_found() {
        let empty = build_catalog(&SourceStore::in_memory(), &[], &[]);
        assert_eq!(empty, Catalog::empty());
    }
}
