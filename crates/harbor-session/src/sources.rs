use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use harbor_sources::{Kind, SmbCreds};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceConfig {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub local_root: Option<String>,
    pub dlna_udn: Option<String>,
    pub dlna_control_url: Option<String>,
    pub smb_host: Option<String>,
    pub smb_share: Option<String>,
    pub smb_username: Option<String>,
    pub smb_domain: Option<String>,
}

impl SourceConfig {
    pub fn smb(id: &str, host: &str, share: &str, username: &str, domain: &str) -> Self {
        let name = if username.is_empty() {
            format!("{host}/{share}")
        } else {
            format!("{username}@{host}/{share}")
        };
        Self {
            id: id.to_string(),
            kind: Kind::Smb,
            name,
            local_root: None,
            dlna_udn: None,
            dlna_control_url: None,
            smb_host: Some(host.to_string()),
            smb_share: Some(share.to_string()),
            smb_username: empty_to_none(username),
            smb_domain: empty_to_none(domain),
        }
    }

    pub fn smb_creds(&self, password: &str) -> Option<SmbCreds> {
        Some(SmbCreds::new(
            self.smb_host.as_deref()?,
            self.smb_share.as_deref()?,
            self.smb_username.as_deref().unwrap_or(""),
            self.smb_domain.as_deref().unwrap_or(""),
            password,
        ))
    }
}

fn empty_to_none(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

pub fn validate_smb(host: &str, share: &str) -> Result<(), &'static str> {
    let host = host.trim();
    let share = share.trim();
    if host.is_empty() {
        return Err("Host is required");
    }
    if share.is_empty() {
        return Err("Share is required");
    }
    if host.contains('/') || host.contains('\\') {
        return Err("Host must be a name or IP");
    }
    if share.contains('/') || share.contains('\\') {
        return Err("Share name cannot contain a slash");
    }
    Ok(())
}

pub fn smb_id(host: &str, share: &str) -> String {
    format!("smb:{}/{}", host.trim(), share.trim())
}

pub fn parse_smb_url(url: &str) -> Option<(String, String, String)> {
    let rest = url.strip_prefix("smb://")?;
    let (auth_host, share) = rest.split_once('/')?;
    let share = share.trim_matches('/').to_string();
    if share.is_empty() {
        return None;
    }
    let (username, host) = match auth_host.split_once('@') {
        Some((u, h)) => (u.to_string(), h.to_string()),
        None => (String::new(), auth_host.to_string()),
    };
    if host.is_empty() {
        return None;
    }
    Some((host, share, username))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceStore {
    pub path: PathBuf,
    items: Vec<SourceConfig>,
}

impl SourceStore {
    pub fn open(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let items = load(&path);
        Self { path, items }
    }

    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            items: Vec::new(),
        }
    }

    pub fn list(&self) -> &[SourceConfig] {
        &self.items
    }

    pub fn put(&mut self, cfg: SourceConfig) {
        if let Some(existing) = self.items.iter_mut().find(|c| c.id == cfg.id) {
            *existing = cfg;
        } else {
            self.items.push(cfg);
        }
        self.flush();
    }

    pub fn get(&self, id: &str) -> Option<&SourceConfig> {
        self.items.iter().find(|c| c.id == id)
    }

    fn flush(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let tmp = self.path.with_extension("tmp");
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp)
        {
            for c in &self.items {
                let _ = writeln!(f, "{}", encode(c));
            }
            let _ = f.flush();
            let _ = fs::rename(&tmp, &self.path);
        }
    }
}

fn kind_tag(kind: Kind) -> &'static str {
    match kind {
        Kind::Local => "local",
        Kind::Dlna => "dlna",
        Kind::Smb => "smb",
    }
}

fn parse_kind(s: &str) -> Option<Kind> {
    match s {
        "local" => Some(Kind::Local),
        "dlna" => Some(Kind::Dlna),
        "smb" => Some(Kind::Smb),
        _ => None,
    }
}

fn encode(c: &SourceConfig) -> String {
    format!(
        "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        escape(&c.id),
        kind_tag(c.kind),
        escape(&c.name),
        escape(c.local_root.as_deref().unwrap_or("")),
        escape(c.dlna_udn.as_deref().unwrap_or("")),
        escape(c.dlna_control_url.as_deref().unwrap_or("")),
        escape(c.smb_host.as_deref().unwrap_or("")),
        escape(c.smb_share.as_deref().unwrap_or("")),
        escape(c.smb_username.as_deref().unwrap_or("")),
        escape(c.smb_domain.as_deref().unwrap_or(""))
    )
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\t', "\\t").replace('\n', "\\n")
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn load(path: &Path) -> Vec<SourceConfig> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut items = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 10 {
            continue;
        }
        let Some(kind) = parse_kind(&unescape(parts[1])) else {
            continue;
        };
        items.push(SourceConfig {
            id: unescape(parts[0]),
            kind,
            name: unescape(parts[2]),
            local_root: empty_to_none(&unescape(parts[3])),
            dlna_udn: empty_to_none(&unescape(parts[4])),
            dlna_control_url: empty_to_none(&unescape(parts[5])),
            smb_host: empty_to_none(&unescape(parts[6])),
            smb_share: empty_to_none(&unescape(parts[7])),
            smb_username: empty_to_none(&unescape(parts[8])),
            smb_domain: empty_to_none(&unescape(parts[9])),
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_and_share_required() {
        assert_eq!(validate_smb("", "media"), Err("Host is required"));
        assert_eq!(validate_smb("nas", ""), Err("Share is required"));
        assert_eq!(validate_smb("nas", "media"), Ok(()));
        assert!(validate_smb("nas/bad", "media").is_err());
    }

    #[test]
    fn smb_url_and_id() {
        assert_eq!(
            parse_smb_url("smb://nas/media"),
            Some(("nas".into(), "media".into(), String::new()))
        );
        assert_eq!(
            parse_smb_url("smb://alice@192.168.1.5/videos"),
            Some(("192.168.1.5".into(), "videos".into(), "alice".into()))
        );
        assert!(parse_smb_url("smb://nas").is_none());
        assert_eq!(smb_id("nas", "media"), "smb:nas/media");
    }

    #[test]
    fn sources_json_has_no_password() {
        let path = std::env::temp_dir().join(format!("harbor-sources-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut store = SourceStore::open(&path);
        store.put(SourceConfig::smb("smb:nas/media", "nas", "media", "alice", "HOME"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.to_ascii_lowercase().contains("password"));
        assert!(!text.contains("s3cret"));
        let reopened = SourceStore::open(&path);
        let got = reopened.get("smb:nas/media").unwrap();
        assert_eq!(got.kind, Kind::Smb);
        assert_eq!(got.smb_host.as_deref(), Some("nas"));
        assert_eq!(got.smb_share.as_deref(), Some("media"));
        assert_eq!(got.smb_username.as_deref(), Some("alice"));
        assert_eq!(got.smb_domain.as_deref(), Some("HOME"));
        let creds = got.smb_creds("s3cret").unwrap();
        assert_eq!(creds.host, "nas");
        assert_eq!(creds.share, "media");
        assert_eq!(creds.username, "alice");
        assert_eq!(creds.domain, "HOME");
        assert_eq!(creds.password, "s3cret");
    }
}
