use std::fs;
use std::path::Path;

pub const DEFAULT_PATH: &str = "/etc/umediaserver/device_codec_capability_config.json";
pub const UNSUPPORTED: &str = "unsupported_codec";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecTable {
    pub present: bool,
    pub h264: bool,
    pub hevc8: bool,
    pub hevc10: bool,
    pub dv5: bool,
    pub dv8: bool,
}

impl CodecTable {
    pub fn absent() -> Self {
        Self {
            present: false,
            h264: true,
            hevc8: true,
            hevc10: false,
            dv5: false,
            dv8: false,
        }
    }

    pub fn from_json(json: &str) -> Self {
        parse(json)
    }

    pub fn load() -> Self {
        Self::load_path(Path::new(DEFAULT_PATH))
    }

    pub fn load_path(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => parse(&text),
            Err(_) => Self::absent(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    pub codec: String,
    pub bit_depth: Option<u8>,
    pub dovi: Option<u8>,
}

impl Stream {
    pub fn new(codec: &str) -> Self {
        Self {
            codec: codec.to_string(),
            bit_depth: None,
            dovi: None,
        }
    }

    pub fn hevc(bit_depth: u8) -> Self {
        Self {
            codec: "hevc".into(),
            bit_depth: Some(bit_depth),
            dovi: None,
        }
    }

    pub fn dv(profile: u8) -> Self {
        Self {
            codec: "dv".into(),
            bit_depth: None,
            dovi: Some(profile),
        }
    }
}

pub fn allow(stream: &Stream, table: &CodecTable) -> Result<(), String> {
    match decide(stream, table) {
        None => Ok(()),
        Some(name) => Err(refuse_message(&name)),
    }
}

pub fn refuse_message(codec: &str) -> String {
    format!("{UNSUPPORTED}: This TV cannot decode {codec}")
}

pub fn decide(stream: &Stream, table: &CodecTable) -> Option<String> {
    let kind = classify(&stream.codec);
    match kind.as_str() {
        "h264" => flag(table.h264, "H.264"),
        "hevc" => {
            let ten = stream.bit_depth.unwrap_or(8) >= 10;
            if ten {
                flag(table.hevc10, "HEVC 10-bit")
            } else {
                flag(table.hevc8, "HEVC")
            }
        }
        "dv" => match stream.dovi.unwrap_or(0) {
            5 => flag(table.dv5, "Dolby Vision profile 5"),
            8 => flag(table.dv8, "Dolby Vision profile 8"),
            other => Some(format!("Dolby Vision profile {other}")),
        },
        other => Some(display_name(other)),
    }
}

fn flag(ok: bool, name: &str) -> Option<String> {
    if ok {
        None
    } else {
        Some(name.to_string())
    }
}

fn display_name(kind: &str) -> String {
    match kind {
        "vp9" => "VP9".into(),
        "av1" => "AV1".into(),
        "mpeg2" => "MPEG-2".into(),
        "vc1" => "VC-1".into(),
        other => other.to_ascii_uppercase(),
    }
}

pub fn classify(raw: &str) -> String {
    let s = raw.trim().to_ascii_lowercase().replace(['.', '-', '_', ' '], "");
    if s.contains("dolbyvision") || s == "dv" || s == "dovi" || s.starts_with("dvhe") || s.starts_with("dvh1")
    {
        return "dv".into();
    }
    if s.contains("hevc") || s.contains("h265") {
        return "hevc".into();
    }
    if s.contains("h264") || s == "avc" || s.contains("avc1") {
        return "h264".into();
    }
    if s.contains("vp9") {
        return "vp9".into();
    }
    if s.contains("av1") {
        return "av1".into();
    }
    if s.contains("mpeg2") || s == "mp2v" {
        return "mpeg2".into();
    }
    if s.contains("vc1") {
        return "vc1".into();
    }
    s
}

fn parse(json: &str) -> CodecTable {
    let tokens = quoted_tokens(json);
    let mut table = CodecTable {
        present: true,
        h264: false,
        hevc8: false,
        hevc10: false,
        dv5: false,
        dv8: false,
    };
    let lower = json.to_ascii_lowercase();
    for tok in &tokens {
        match classify(tok).as_str() {
            "h264" => table.h264 = true,
            "hevc" => {
                table.hevc8 = true;
                if tok.to_ascii_lowercase().contains("10")
                    || tok.to_ascii_lowercase().contains("main10")
                {
                    table.hevc10 = true;
                }
            }
            "dv" => {
                table.dv5 = true;
                table.dv8 = true;
            }
            _ => {}
        }
    }
    if table.hevc8 && (lower.contains("main10") || lower.contains("\"10\"") || lower.contains("bitdepth\":10") || lower.contains("bit_depth\":10") || lower.contains("maxbitdepth\":10"))
    {
        table.hevc10 = true;
    }
    if tokens.iter().any(|t| classify(t) == "dv") || lower.contains("dolbyvision") {
        if lower.contains("5") {
            table.dv5 = true;
        }
        if lower.contains("8") {
            table.dv8 = true;
        }
        if !table.dv5 && !table.dv8 {
            table.dv5 = true;
            table.dv8 = true;
        }
    }
    table
}

fn quoted_tokens(json: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = json.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            i += 1;
            let mut s = String::new();
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' if i + 1 < bytes.len() => {
                        s.push(bytes[i + 1] as char);
                        i += 2;
                    }
                    b'"' => {
                        i += 1;
                        break;
                    }
                    c => {
                        s.push(c as char);
                        i += 1;
                    }
                }
            }
            if !s.is_empty() {
                out.push(s);
            }
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &str = r#"{
        "video": [
            {"codec": "H264"},
            {"codec": "HEVC", "profile": "main10", "bitDepth": 10},
            {"codec": "DV", "profiles": [5, 8]}
        ]
    }"#;

    #[test]
    fn absent_allows_h264_and_8bit_hevc_only() {
        let t = CodecTable::absent();
        assert!(!t.present);
        assert!(allow(&Stream::new("h264"), &t).is_ok());
        assert!(allow(&Stream::new("avc"), &t).is_ok());
        assert!(allow(&Stream::hevc(8), &t).is_ok());
        let e = allow(&Stream::hevc(10), &t).unwrap_err();
        assert!(e.starts_with("unsupported_codec:"));
        assert!(e.contains("This TV cannot decode HEVC 10-bit"));
        assert!(allow(&Stream::dv(5), &t).is_err());
        assert!(allow(&Stream::new("vp9"), &t).is_err());
        assert!(allow(&Stream::new("av1"), &t).is_err());
    }

    #[test]
    fn present_table_direct_plays_listed() {
        let t = CodecTable::from_json(TABLE);
        assert!(t.present);
        assert!(t.h264 && t.hevc8 && t.hevc10 && t.dv5 && t.dv8);
        assert!(allow(&Stream::new("H.264"), &t).is_ok());
        assert!(allow(&Stream::hevc(8), &t).is_ok());
        assert!(allow(&Stream::hevc(10), &t).is_ok());
        assert!(allow(&Stream::dv(5), &t).is_ok());
        assert!(allow(&Stream::dv(8), &t).is_ok());
        let e = allow(&Stream::new("VP9"), &t).unwrap_err();
        assert!(e.contains("VP9"));
        assert!(allow(&Stream::dv(7), &t).is_err());
    }

    #[test]
    fn missing_file_is_absent() {
        let t = CodecTable::load_path(Path::new("/no/such/device_codec_capability_config.json"));
        assert_eq!(t, CodecTable::absent());
    }

    #[test]
    fn load_path_reads_fixture() {
        let path = std::env::temp_dir().join(format!("harbor-codec-{}", std::process::id()));
        fs::write(&path, TABLE).unwrap();
        let t = CodecTable::load_path(&path);
        assert!(t.present && t.h264 && t.dv5);
    }
}
