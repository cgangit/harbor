use harbor_platform::PathClass;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    Denied { class: PathClass, path: String },
    NotFound { path: String },
    Io(String),
    Escape { path: String },
    Upnp { code: i32, description: String },
    Http { status: u16, url: String },
    Timeout { what: String },
    Auth { source: String },
}

impl SourceError {
    pub fn denied(class: PathClass, path: impl Into<String>) -> Self {
        Self::Denied {
            class,
            path: path.into(),
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::Denied { class, path } => {
                format!("{} path denied: {path}", class.label())
            }
            Self::NotFound { path } => format!("not found: {path}"),
            Self::Io(e) => e.clone(),
            Self::Escape { path } => format!("path outside source: {path}"),
            Self::Upnp { code, description } => format!("UPnP {code}: {description}"),
            Self::Http { status, url } => format!("HTTP {status}: {url}"),
            Self::Timeout { what } => format!("timed out: {what}"),
            Self::Auth { source } => format!("authentication failed: {source}"),
        }
    }
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_names_class() {
        let e = SourceError::denied(PathClass::Usb, "/tmp/usb/secret");
        assert_eq!(e.message(), "USB path denied: /tmp/usb/secret");
        let e = SourceError::denied(PathClass::Internal, "/var/jail");
        assert!(e.message().starts_with("internal path denied:"));
        let e = SourceError::Upnp {
            code: 701,
            description: "No such object".into(),
        };
        assert_eq!(e.message(), "UPnP 701: No such object");
        let e = SourceError::Http {
            status: 404,
            url: "http://nas/item".into(),
        };
        assert_eq!(e.message(), "HTTP 404: http://nas/item");
        let e = SourceError::Timeout {
            what: "DLNA discovery".into(),
        };
        assert_eq!(e.message(), "timed out: DLNA discovery");
        let e = SourceError::Auth {
            source: "smb:nas/media".into(),
        };
        assert_eq!(e.message(), "authentication failed: smb:nas/media");
        let e = SourceError::Timeout {
            what: "SMB connect".into(),
        };
        assert_eq!(e.message(), "timed out: SMB connect");
    }
}
