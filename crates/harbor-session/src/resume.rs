use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const OFFER_AFTER_NS: i64 = 10_000_000_000;
pub const CLEAR_RATIO_NUM: i64 = 9;
pub const CLEAR_RATIO_DEN: i64 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resume {
    pub source_id: String,
    pub entry_id: String,
    pub position_ns: i64,
    pub duration_ns: i64,
    pub updated_unix: i64,
}

impl Resume {
    pub fn key(&self) -> (&str, &str) {
        (&self.source_id, &self.entry_id)
    }
}

pub fn should_offer(position_ns: i64, duration_ns: i64) -> bool {
    position_ns >= OFFER_AFTER_NS && !past_ninety(position_ns, duration_ns)
}

pub fn past_ninety(position_ns: i64, duration_ns: i64) -> bool {
    duration_ns > 0 && position_ns * CLEAR_RATIO_DEN >= duration_ns * CLEAR_RATIO_NUM
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ResumeStore {
    pub path: PathBuf,
    items: Vec<Resume>,
}

impl ResumeStore {
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

    pub fn get(&self, source_id: &str, entry_id: &str) -> Option<&Resume> {
        self.items
            .iter()
            .find(|r| r.source_id == source_id && r.entry_id == entry_id)
    }

    pub fn put(&mut self, mut rec: Resume) {
        rec.updated_unix = now_unix();
        if past_ninety(rec.position_ns, rec.duration_ns) {
            self.clear(&rec.source_id, &rec.entry_id);
            return;
        }
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|r| r.source_id == rec.source_id && r.entry_id == rec.entry_id)
        {
            *existing = rec;
        } else {
            self.items.push(rec);
        }
        self.flush();
    }

    pub fn clear(&mut self, source_id: &str, entry_id: &str) {
        let before = self.items.len();
        self.items
            .retain(|r| !(r.source_id == source_id && r.entry_id == entry_id));
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
            for r in &self.items {
                let _ = writeln!(
                    f,
                    "{}\t{}\t{}\t{}\t{}",
                    escape(&r.source_id),
                    escape(&r.entry_id),
                    r.position_ns,
                    r.duration_ns,
                    r.updated_unix
                );
            }
            let _ = f.flush();
            let _ = fs::rename(&tmp, &self.path);
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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

fn load(path: &Path) -> Vec<Resume> {
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
        if parts.len() != 5 {
            continue;
        }
        let Ok(position_ns) = parts[2].parse() else {
            continue;
        };
        let Ok(duration_ns) = parts[3].parse() else {
            continue;
        };
        let Ok(updated_unix) = parts[4].parse() else {
            continue;
        };
        items.push(Resume {
            source_id: unescape(parts[0]),
            entry_id: unescape(parts[1]),
            position_ns,
            duration_ns,
            updated_unix,
        });
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_window_is_ten_seconds_to_ninety() {
        let dur = 100_000_000_000;
        assert!(!should_offer(0, dur));
        assert!(!should_offer(9_999_999_999, dur));
        assert!(should_offer(10_000_000_000, dur));
        assert!(should_offer(89_000_000_000, dur));
        assert!(!should_offer(90_000_000_000, dur));
        assert!(past_ninety(90_000_000_000, dur));
    }

    #[test]
    fn round_trip_and_ninety_clears() {
        let path = std::env::temp_dir().join(format!("harbor-resume-{}", std::process::id()));
        let _ = fs::remove_file(&path);
        let mut store = ResumeStore::open(&path);
        store.put(Resume {
            source_id: "demo".into(),
            entry_id: "the-river.mkv".into(),
            position_ns: 12_000_000_000,
            duration_ns: 90_000_000_000,
            updated_unix: 0,
        });
        let reopened = ResumeStore::open(&path);
        let got = reopened.get("demo", "the-river.mkv").unwrap();
        assert_eq!(got.position_ns, 12_000_000_000);
        let mut store = reopened;
        store.put(Resume {
            source_id: "demo".into(),
            entry_id: "the-river.mkv".into(),
            position_ns: 81_000_000_000,
            duration_ns: 90_000_000_000,
            updated_unix: 0,
        });
        assert!(store.get("demo", "the-river.mkv").is_none());
    }
}
