#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub continue_watching: Vec<String>,
    pub recently_added: Vec<String>,
    pub sources: Vec<String>,
    pub files: Vec<String>,
}

impl Catalog {
    pub fn demo() -> Self {
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
pub const AZ: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
    "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
];
pub const CHROME: [&str; 3] = ["Home", "Browser", "Settings"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_has_sources_empty_does_not() {
        assert!(!Catalog::demo().is_empty());
        assert!(Catalog::empty().is_empty());
        assert_eq!(Catalog::demo().continue_watching.len(), 2);
    }
}
