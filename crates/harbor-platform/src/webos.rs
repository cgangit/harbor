#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaPath {
    Acb,
    WindowId,
}

pub fn major_version(raw: &str) -> Option<u32> {
    let digits: String = raw
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

pub fn is_webos5(version: &str) -> bool {
    major_version(version).map(|n| n >= 5).unwrap_or(false)
}

pub fn media_path(version: &str) -> MediaPath {
    if is_webos5(version) {
        MediaPath::WindowId
    } else {
        MediaPath::Acb
    }
}

pub fn inject_window_id(payload: &str, window_id: &str) -> String {
    let id = escape_json(window_id);
    let trimmed = payload.trim();
    if trimmed.contains("windowId") {
        return trimmed.to_string();
    }
    if trimmed.is_empty() || trimmed == "{}" {
        return format!(r#"{{"option":{{"windowId":"{id}"}}}}"#);
    }
    if let Some(body) = trimmed.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        let body = body.trim();
        if body.is_empty() {
            return format!(r#"{{"option":{{"windowId":"{id}"}}}}"#);
        }
        return format!(r#"{{{body},"option":{{"windowId":"{id}"}}}}"#);
    }
    format!(r#"{{"option":{{"windowId":"{id}"}}}}"#)
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_x_is_acb_five_plus_is_window_id() {
        assert!(!is_webos5("4.5.0"));
        assert_eq!(media_path("4.4.2"), MediaPath::Acb);
        assert!(is_webos5("5.0.0"));
        assert!(is_webos5("7.3.1"));
        assert_eq!(media_path("5.2"), MediaPath::WindowId);
        assert_eq!(major_version("webOS 5.0"), Some(5));
    }

    #[test]
    fn injects_option_window_id() {
        assert_eq!(
            inject_window_id("{}", "42"),
            r#"{"option":{"windowId":"42"}}"#
        );
        let merged = inject_window_id(r#"{"uri":"file://a.mkv"}"#, "sdl-1");
        assert!(merged.contains(r#""uri":"file://a.mkv""#));
        assert!(merged.contains(r#""windowId":"sdl-1""#));
        let already = r#"{"option":{"windowId":"keep"}}"#;
        assert_eq!(inject_window_id(already, "other"), already);
    }
}
