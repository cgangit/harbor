use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use crate::error::SourceError;

#[derive(Debug)]
enum Inner {
    File { file: File, len: u64 },
    Http { url: String, len: u64 },
    Smb { data: Vec<u8> },
}

#[derive(Debug)]
pub struct MediaHandle {
    inner: Inner,
}

impl MediaHandle {
    pub fn open_path(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self {
            inner: Inner::File { file, len },
        })
    }

    pub fn open_smb(data: Vec<u8>) -> Self {
        Self {
            inner: Inner::Smb { data },
        }
    }

    pub fn open_http(url: &str) -> Result<Self, SourceError> {
        let (status, total, _) = http_range(url, 0, 1)?;
        if status == 200 || status == 206 {
            Ok(Self {
                inner: Inner::Http {
                    url: url.to_string(),
                    len: total,
                },
            })
        } else {
            Err(SourceError::Http {
                status,
                url: url.to_string(),
            })
        }
    }

    pub fn len(&self) -> u64 {
        match &self.inner {
            Inner::File { len, .. } | Inner::Http { len, .. } => *len,
            Inner::Smb { data } => data.len() as u64,
        }
    }

    pub fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            Inner::File { file, len } => {
                if offset >= *len {
                    return Ok(0);
                }
                file.seek(SeekFrom::Start(offset))?;
                file.read(buf)
            }
            Inner::Smb { data } => {
                let len = data.len() as u64;
                if offset >= len || buf.is_empty() {
                    return Ok(0);
                }
                let start = offset as usize;
                let n = buf.len().min(data.len() - start);
                buf[..n].copy_from_slice(&data[start..start + n]);
                Ok(n)
            }
            Inner::Http { url, len } => {
                if offset >= *len || buf.is_empty() {
                    return Ok(0);
                }
                let want = (buf.len() as u64).min(*len - offset) as usize;
                let (status, _, body) = http_range(url, offset, want).map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, e.message())
                })?;
                if status != 200 && status != 206 {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("HTTP {status}"),
                    ));
                }
                let n = body.len().min(buf.len());
                buf[..n].copy_from_slice(&body[..n]);
                Ok(n)
            }
        }
    }
}

pub fn range_header(offset: u64, end: u64) -> String {
    format!("bytes={offset}-{end}")
}

pub fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let rest = value.trim().strip_prefix("bytes")?;
    let rest = rest.trim().trim_start_matches('*').trim();
    let (span, total) = rest.split_once('/')?;
    let total = if total == "*" {
        0
    } else {
        total.parse().ok()?
    };
    let (start, end) = span.split_once('-')?;
    Some((start.parse().ok()?, end.parse().ok()?, total))
}

pub fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (hostport, path) = match rest.split_once('/') {
        Some((h, p)) => (h, format!("/{p}")),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match hostport.split_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().ok()?),
        None => (hostport.to_string(), 80),
    };
    Some((host, port, path))
}

fn http_range(url: &str, offset: u64, want: usize) -> Result<(u16, u64, Vec<u8>), SourceError> {
    let (host, port, path) = parse_http_url(url).ok_or_else(|| SourceError::Io(format!("bad url: {url}")))?;
    let end = offset.saturating_add(want.saturating_sub(1) as u64);
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nRange: {}\r\nConnection: close\r\n\r\n",
        range_header(offset, end)
    );
    let mut stream = TcpStream::connect((host.as_str(), port)).map_err(|e| SourceError::Io(e.to_string()))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| SourceError::Io(e.to_string()))?;
    stream
        .write_all(req.as_bytes())
        .map_err(|e| SourceError::Io(e.to_string()))?;
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| SourceError::Io(e.to_string()))?;
    parse_http_response(url, offset, want, &raw)
}

fn parse_http_response(
    url: &str,
    offset: u64,
    want: usize,
    raw: &[u8],
) -> Result<(u16, u64, Vec<u8>), SourceError> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| SourceError::Io("truncated HTTP response".into()))?;
    let head = std::str::from_utf8(&raw[..sep]).map_err(|_| SourceError::Io("HTTP headers".into()))?;
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SourceError::Io("HTTP status".into()))?;
    if status >= 400 {
        return Err(SourceError::Http {
            status,
            url: url.to_string(),
        });
    }
    let mut total = 0u64;
    let mut content_len = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if k == "content-range" {
                if let Some((_, _, t)) = parse_content_range(v) {
                    total = t;
                }
            }
            if k == "content-length" {
                content_len = v.parse().ok();
            }
        }
    }
    let mut body = raw[sep + 4..].to_vec();
    if total == 0 {
        total = content_len.unwrap_or(body.len() as u64).max(offset + body.len() as u64);
    }
    if status == 200 && offset > 0 {
        let start = offset as usize;
        if start >= body.len() {
            body.clear();
        } else {
            let end = (start + want).min(body.len());
            body = body[start..end].to_vec();
        }
    }
    Ok((status, total, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn read_at_skips() {
        let path = std::env::temp_dir().join(format!("harbor-h-{}", std::process::id()));
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(b"abcdef").unwrap();
        }
        let mut h = MediaHandle::open_path(&path).unwrap();
        assert_eq!(h.len(), 6);
        let mut buf = [0u8; 3];
        assert_eq!(h.read_at(2, &mut buf).unwrap(), 3);
        assert_eq!(&buf, b"cde");
    }

    #[test]
    fn smb_read_at() {
        let mut h = MediaHandle::open_smb(b"abcdef".to_vec());
        assert_eq!(h.len(), 6);
        let mut buf = [0u8; 3];
        assert_eq!(h.read_at(2, &mut buf).unwrap(), 3);
        assert_eq!(&buf, b"cde");
        assert_eq!(h.read_at(6, &mut buf).unwrap(), 0);
    }

    #[test]
    fn range_helpers() {
        assert_eq!(range_header(2, 4), "bytes=2-4");
        assert_eq!(parse_content_range("bytes 2-4/6"), Some((2, 4, 6)));
        assert_eq!(
            parse_http_url("http://127.0.0.1:9/MediaItems/64.mkv"),
            Some(("127.0.0.1".into(), 9, "/MediaItems/64.mkv".into()))
        );
    }

    fn spawn_range_server(body: &'static [u8], status_override: Option<u16>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for _ in 0..8 {
                let Ok((mut s, _)) = listener.accept() else {
                    break;
                };
                let mut buf = [0u8; 2048];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                if let Some(code) = status_override {
                    let _ = write!(s, "HTTP/1.1 {code} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    continue;
                }
                let range = req
                    .lines()
                    .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                    .and_then(|l| l.split_once(':'))
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default();
                let (start, end) = range
                    .strip_prefix("bytes=")
                    .and_then(|r| r.split_once('-'))
                    .and_then(|(a, b)| Some((a.parse::<u64>().ok()?, b.parse::<u64>().ok()?)))
                    .unwrap_or((0, body.len() as u64 - 1));
                let start = start as usize;
                let end = (end as usize + 1).min(body.len()).max(start);
                let slice = &body[start..end];
                let _ = write!(
                    s,
                    "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    end.saturating_sub(1),
                    body.len(),
                    slice.len()
                );
                let _ = s.write_all(slice);
            }
        });
        format!("http://127.0.0.1:{}/clip.mkv", addr.port())
    }

    #[test]
    fn http_range_read_at() {
        let url = spawn_range_server(b"abcdef", None);
        let mut h = MediaHandle::open_http(&url).unwrap();
        assert_eq!(h.len(), 6);
        let mut buf = [0u8; 3];
        assert_eq!(h.read_at(2, &mut buf).unwrap(), 3);
        assert_eq!(&buf, b"cde");
    }

    #[test]
    fn http_404_is_status() {
        let url = spawn_range_server(b"x", Some(404));
        let err = MediaHandle::open_http(&url).unwrap_err();
        match err {
            SourceError::Http { status, .. } => assert_eq!(status, 404),
            other => panic!("{other:?}"),
        }
        assert!(err.message().starts_with("HTTP 404:"));
    }
}
