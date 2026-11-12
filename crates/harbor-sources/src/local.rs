use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use harbor_platform::{classify, Mount, PathClass};

use crate::entry::{ext_of, is_video_name, Entry, EntryKind, Page, Probe};
use crate::error::SourceError;
use crate::handle::MediaHandle;
use crate::{Kind, Source, SourceId};

#[derive(Clone, Debug)]
pub struct LocalSource {
    pub id: SourceId,
    pub name: String,
    pub root: PathBuf,
    pub class: PathClass,
}

impl LocalSource {
    pub fn from_mount(mount: Mount) -> Self {
        let id = format!("local:{}", mount.path.display());
        Self {
            id: SourceId(id),
            name: mount.name,
            root: mount.path,
            class: mount.class,
        }
    }

    fn resolve(&self, rel: &str) -> Result<PathBuf, SourceError> {
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() {
            return Ok(self.root.clone());
        }
        let joined = self.root.join(rel);
        let root = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        match joined.canonicalize() {
            Ok(abs) => {
                if abs.starts_with(&root) {
                    Ok(abs)
                } else {
                    Err(SourceError::Escape {
                        path: joined.display().to_string(),
                    })
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(SourceError::denied(self.class, joined.display().to_string()))
            }
            Err(_) => {
                if joined.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                    Err(SourceError::Escape {
                        path: joined.display().to_string(),
                    })
                } else {
                    Ok(joined)
                }
            }
        }
    }
}

impl Source for LocalSource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn kind(&self) -> Kind {
        Kind::Local
    }

    fn display_name(&self) -> &str {
        &self.name
    }

    fn list(&self, path: &str) -> Result<Page, SourceError> {
        let dir = self.resolve(path)?;
        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                return Err(SourceError::denied(self.class, dir.display().to_string()));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SourceError::NotFound {
                    path: dir.display().to_string(),
                });
            }
            Err(e) => return Err(SourceError::Io(e.to_string())),
        };
        let parent = if path.is_empty() || path == "/" {
            String::new()
        } else {
            path.trim_start_matches('/').to_string()
        };
        let mut entries = Vec::new();
        for item in rd.flatten() {
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let meta = match item.metadata() {
                Ok(m) => m,
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    continue;
                }
                Err(_) => continue,
            };
            let kind = if meta.is_dir() {
                EntryKind::Dir
            } else if is_video_name(&name) {
                EntryKind::Video
            } else {
                EntryKind::Other
            };
            if kind == EntryKind::Other {
                continue;
            }
            let rel = if parent.is_empty() {
                name.clone()
            } else {
                format!("{parent}/{name}")
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64);
            entries.push(Entry {
                id: rel,
                parent: parent.clone(),
                name,
                kind,
                size: if meta.is_file() {
                    Some(meta.len())
                } else {
                    None
                },
                mtime,
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
        let path = self.resolve(&entry.id)?;
        MediaHandle::open_path(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SourceError::denied(self.class, path.display().to_string())
            } else if e.kind() == std::io::ErrorKind::NotFound {
                SourceError::NotFound {
                    path: path.display().to_string(),
                }
            } else {
                SourceError::Io(e.to_string())
            }
        })
    }

    fn probe(&self, entry: &Entry) -> Result<Probe, SourceError> {
        let path = self.resolve(&entry.id)?;
        let meta = fs::metadata(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                SourceError::denied(self.class, path.display().to_string())
            } else {
                SourceError::Io(e.to_string())
            }
        })?;
        Ok(Probe {
            container: ext_of(&entry.name),
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration_ns: None,
            size: Some(meta.len()),
            bit_depth: None,
            dovi: None,
        })
    }
}

pub fn discover() -> Vec<LocalSource> {
    harbor_platform::discover_roots()
        .into_iter()
        .map(LocalSource::from_mount)
        .collect()
}

pub fn from_dir(path: impl AsRef<Path>, name: &str) -> LocalSource {
    let path = path.as_ref().to_path_buf();
    LocalSource {
        id: SourceId(format!("local:{}", path.display())),
        name: name.to_string(),
        class: classify(&path),
        root: path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "harbor-local-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::create_dir_all(dir.join("Movies"));
        let _ = fs::create_dir_all(dir.join("Shows"));
        fs::write(dir.join("Movies/the-river.mkv"), b"fake-mkv").unwrap();
        fs::write(dir.join("Movies/notes.txt"), b"ignore").unwrap();
        fs::write(dir.join("harbor-demo.mp4"), b"fake-mp4").unwrap();
        dir
    }

    #[test]
    fn lists_dirs_and_videos_skips_other() {
        let root = tree("list");
        let src = from_dir(&root, "Host");
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
    }

    #[test]
    fn open_read_at_and_probe() {
        let root = tree("open");
        let src = from_dir(&root, "Host");
        let page = src.list("Movies").unwrap();
        let entry = &page.entries[0];
        let mut h = src.open(entry).unwrap();
        let mut buf = [0u8; 4];
        assert_eq!(h.read_at(0, &mut buf).unwrap(), 4);
        assert_eq!(&buf, b"fake");
        let p = src.probe(entry).unwrap();
        assert_eq!(p.container.as_deref(), Some("mkv"));
        assert_eq!(p.size, Some(8));
    }

    #[test]
    fn parent_dir_escape_rejected() {
        let root = tree("esc");
        let src = from_dir(&root, "Host");
        let err = src.list("../").unwrap_err();
        match err {
            SourceError::Escape { .. } => {}
            other => panic!("expected escape, got {other:?}"),
        }
    }

    #[test]
    fn denied_message_names_usb() {
        let src = LocalSource {
            id: SourceId("local:/tmp/usb/sda1".into()),
            name: "USB".into(),
            root: PathBuf::from("/tmp/usb/sda1"),
            class: PathClass::Usb,
        };
        let err = SourceError::denied(src.class, "/tmp/usb/sda1/secret");
        assert_eq!(err.message(), "USB path denied: /tmp/usb/sda1/secret");
    }
}
