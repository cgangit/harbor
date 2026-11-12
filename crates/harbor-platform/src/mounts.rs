use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathClass {
    Usb,
    Internal,
    Host,
    Other,
}

impl PathClass {
    pub fn label(self) -> &'static str {
        match self {
            PathClass::Usb => "USB",
            PathClass::Internal => "internal",
            PathClass::Host => "host",
            PathClass::Other => "other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    pub path: PathBuf,
    pub class: PathClass,
    pub name: String,
}

pub fn discover_roots() -> Vec<Mount> {
    discover_roots_with(
        std::env::var_os("HARBOR_MEDIA").map(PathBuf::from),
        Path::new("/tmp/usb"),
        Path::new("/media"),
    )
}

pub fn discover_roots_with(
    harbor_media: Option<PathBuf>,
    usb: &Path,
    media: &Path,
) -> Vec<Mount> {
    if let Some(dir) = harbor_media {
        if dir.is_dir() {
            return vec![Mount {
                name: volume_name(&dir),
                path: dir,
                class: PathClass::Host,
            }];
        }
        return Vec::new();
    }
    let mut out = Vec::new();
    collect_volumes(usb, PathClass::Usb, &mut out);
    collect_volumes(media, PathClass::Usb, &mut out);
    out
}

pub fn classify(path: &Path) -> PathClass {
    let s = path.to_string_lossy();
    if s.starts_with("/tmp/usb") {
        PathClass::Usb
    } else if s.starts_with("/media") {
        PathClass::Usb
    } else {
        PathClass::Other
    }
}

fn collect_volumes(base: &Path, class: PathClass, out: &mut Vec<Mount>) {
    if !base.is_dir() {
        return;
    }
    let mut kids = Vec::new();
    if let Ok(rd) = fs::read_dir(base) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                kids.push(p);
            }
        }
    }
    if kids.is_empty() {
        out.push(Mount {
            path: base.to_path_buf(),
            class,
            name: volume_name(base),
        });
        return;
    }
    for k in kids {
        out.push(Mount {
            name: volume_name(&k),
            path: k,
            class,
        });
    }
}

fn volume_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Local")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "harbor-mnt-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn harbor_media_wins() {
        let host = scratch("host");
        let usb = scratch("usb");
        let found = discover_roots_with(Some(host.clone()), &usb, Path::new("/nope"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].class, PathClass::Host);
        assert_eq!(found[0].path, host);
    }

    #[test]
    fn missing_harbor_media_is_empty() {
        let found = discover_roots_with(
            Some(PathBuf::from("/no/such/harbor-media")),
            Path::new("/nope-usb"),
            Path::new("/nope-media"),
        );
        assert!(found.is_empty());
    }

    #[test]
    fn usb_children_are_volumes() {
        let usb = scratch("vols");
        let a = usb.join("sda1");
        let b = usb.join("sdb1");
        let _ = fs::create_dir_all(&a);
        let _ = fs::create_dir_all(&b);
        let found = discover_roots_with(None, &usb, Path::new("/nope-media"));
        let names: Vec<_> = found.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"sda1"));
        assert!(names.contains(&"sdb1"));
        assert!(found.iter().all(|m| m.class == PathClass::Usb));
    }

    #[test]
    fn classify_usb_prefix() {
        assert_eq!(classify(Path::new("/tmp/usb/sda1")), PathClass::Usb);
        assert_eq!(classify(Path::new("/media/usb")), PathClass::Usb);
        assert_eq!(classify(Path::new("/home/media")), PathClass::Other);
    }
}
