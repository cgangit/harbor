use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CredentialVault {
    pub path: PathBuf,
    items: Vec<(String, String)>,
}

impl CredentialVault {
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

    pub fn get(&self, source_id: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(id, _)| id == source_id)
            .map(|(_, p)| p.as_str())
    }

    pub fn put(&mut self, source_id: &str, password: &str) {
        if password.is_empty() {
            self.clear(source_id);
            return;
        }
        if let Some((_, existing)) = self.items.iter_mut().find(|(id, _)| id == source_id) {
            existing.clear();
            existing.push_str(password);
        } else {
            self.items
                .push((source_id.to_string(), password.to_string()));
        }
        self.flush();
    }

    pub fn clear(&mut self, source_id: &str) {
        let before = self.items.len();
        self.items.retain(|(id, _)| id != source_id);
        if self.items.len() != before {
            self.flush();
        }
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
            for (id, password) in &self.items {
                let _ = writeln!(f, "{}\t{}", escape(id), escape(password));
            }
            let _ = f.flush();
            let _ = fs::rename(&tmp, &self.path);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
        }
    }
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

fn load(path: &Path) -> Vec<(String, String)> {
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut items = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.is_empty() {
            continue;
        }
        let Some((id, password)) = line.split_once('\t') else {
            continue;
        };
        items.push((unescape(id), unescape(password)));
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_mode_0600_empty_clears() {
        let dir = std::env::temp_dir().join(format!("harbor-vault-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("vault");
        let _ = fs::remove_file(&path);
        let mut vault = CredentialVault::open(&path);
        vault.put("smb:nas/media", "s3cret");
        let reopened = CredentialVault::open(&path);
        assert_eq!(reopened.get("smb:nas/media"), Some("s3cret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("s3cret"));
        let mut vault = reopened;
        vault.put("smb:nas/media", "");
        assert!(vault.get("smb:nas/media").is_none());
    }
}
