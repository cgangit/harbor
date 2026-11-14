//! Minimal HTTP/1.1 client shared by the DLNA paths (device description
//! GET, ContentDirectory SOAP POST). Deliberately not an HTTP crate: the
//! only peers we talk to are LAN UPnP devices, and `handle.rs` already
//! proves the raw `TcpStream` pattern (design D1 of
//! `network-source-browsing`).

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::error::SourceError;
use crate::handle::parse_http_url;

pub const MAX_REDIRECTS: usize = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

impl Response {
    pub fn body_text(&self) -> Result<&str, SourceError> {
        std::str::from_utf8(&self.body).map_err(|_| SourceError::Io("HTTP body is not UTF-8".into()))
    }
}

/// GET `url`, giving up after `budget` (the caller's spec budget — e.g. the
/// 8 s discovery window for device descriptions). Any status is returned to
/// the caller: UPnP faults arrive as HTTP 500 with a parseable body, so only
/// transport failures are errors here.
pub fn get(url: &str, budget: Duration) -> Result<Response, SourceError> {
    request("GET", url, &[], None, budget, 0)
}

/// POST a SOAP envelope. `soap_action` is the UPnP `SOAPAction` header value
/// (unquoted; this fn adds the quotes devices expect).
pub fn post_soap(
    url: &str,
    soap_action: &str,
    body: &str,
    budget: Duration,
) -> Result<Response, SourceError> {
    let headers = [
        "Content-Type: text/xml; charset=\"utf-8\"".to_string(),
        format!("SOAPAction: \"{soap_action}\""),
    ];
    request("POST", url, &headers, Some(body.as_bytes()), budget, 0)
}

fn request(
    method: &str,
    url: &str,
    headers: &[String],
    body: Option<&[u8]>,
    budget: Duration,
    depth: usize,
) -> Result<Response, SourceError> {
    if depth > MAX_REDIRECTS {
        return Err(SourceError::Io(format!(
            "too many redirects ({depth}) fetching {url}"
        )));
    }
    let (host, port, path) =
        parse_http_url(url).ok_or_else(|| SourceError::Io(format!("bad url: {url}")))?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| SourceError::Io(e.to_string()))?
        .next()
        .ok_or_else(|| SourceError::Io(format!("no address for {host}:{port}")))?;
    let mut stream = TcpStream::connect_timeout(&addr, budget).map_err(|e| {
        if e.kind() == std::io::ErrorKind::TimedOut {
            SourceError::Timeout {
                what: format!("HTTP connect {host}:{port}"),
            }
        } else {
            SourceError::Io(e.to_string())
        }
    })?;
    stream
        .set_read_timeout(Some(budget))
        .map_err(|e| SourceError::Io(e.to_string()))?;
    stream
        .set_write_timeout(Some(budget))
        .map_err(|e| SourceError::Io(e.to_string()))?;

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\n");
    for h in headers {
        req.push_str(h);
        req.push_str("\r\n");
    }
    match body {
        Some(b) => req.push_str(&format!("Content-Length: {}\r\n", b.len())),
        None => req.push_str("Content-Length: 0\r\n"),
    }
    req.push_str("Connection: close\r\n\r\n");
    stream
        .write_all(req.as_bytes())
        .and_then(|_| stream.write_all(body.unwrap_or(b"")))
        .map_err(|e| classify_io(e, method))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| classify_io(e, method))?;
    let response = parse_response(&raw)?;
    if is_redirect(response.status) {
        let location = response
            .headers
            .get("location")
            .cloned()
            .ok_or_else(|| SourceError::Io(format!("redirect without Location from {url}")))?;
        let next = resolve_url(url, &location)
            .ok_or_else(|| SourceError::Io(format!("bad redirect target: {location}")))?;
        return request(method, &next, headers, body, budget, depth + 1);
    }
    Ok(Response {
        status: response.status,
        body: response.body,
    })
}

fn classify_io(e: std::io::Error, method: &str) -> SourceError {
    match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => SourceError::Timeout {
            what: format!("HTTP {method}"),
        },
        _ => SourceError::Io(e.to_string()),
    }
}

struct Parsed {
    status: u16,
    headers: std::collections::HashMap<String, String>,
    body: Vec<u8>,
}

fn parse_response(raw: &[u8]) -> Result<Parsed, SourceError> {
    let sep = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| SourceError::Io("truncated HTTP response".into()))?;
    let head =
        std::str::from_utf8(&raw[..sep]).map_err(|_| SourceError::Io("HTTP headers".into()))?;
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| SourceError::Io("HTTP status".into()))?;
    let mut headers = std::collections::HashMap::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let mut body = raw[sep + 4..].to_vec();
    if headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        body = dechunk(&body)?;
    }
    Ok(Parsed {
        status,
        headers,
        body,
    })
}

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

/// Resolve a `Location` value (absolute, root-relative, or path-relative)
/// against the URL that produced it. Non-HTTP schemes are rejected.
fn resolve_url(base: &str, location: &str) -> Option<String> {
    if location.contains("://") && !location.starts_with("http://") {
        return None;
    }
    if location.starts_with("http://") {
        return Some(location.to_string());
    }
    let rest = base.strip_prefix("http://")?;
    let authority_end = rest.find('/').unwrap_or(rest.len());
    let origin = format!("http://{}", &rest[..authority_end]);
    if location.starts_with('/') {
        return Some(format!("{origin}{location}"));
    }
    let base_path = &rest[authority_end..];
    let dir = base_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    Some(format!("{origin}{dir}/{location}"))
}

fn dechunk(data: &[u8]) -> Result<Vec<u8>, SourceError> {
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let line_end = data[i..]
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|p| i + p)
            .ok_or_else(|| SourceError::Io("chunked body: no size line".into()))?;
        let size_line = std::str::from_utf8(&data[i..line_end])
            .map_err(|_| SourceError::Io("chunked body: bad size".into()))?;
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|_| SourceError::Io("chunked body: bad size".into()))?;
        if size == 0 {
            return Ok(out);
        }
        let start = line_end + 2;
        let end = start
            .checked_add(size)
            .filter(|e| *e <= data.len())
            .ok_or_else(|| SourceError::Io("chunked body: short chunk".into()))?;
        out.extend_from_slice(&data[start..end]);
        i = end + 2.min(data.len() - end); // skip trailing CRLF when present
        if i >= data.len() {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;

    const BUDGET: Duration = Duration::from_secs(2);

    /// Serve canned responses in order; records each raw request (headers +
    /// body, read until Content-Length is satisfied) on the channel.
    fn spawn_server(responses: Vec<String>) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            for response in responses {
                let Ok((mut s, _)) = listener.accept() else {
                    return;
                };
                let req = read_full_request(&mut s);
                let _ = tx.send(req);
                let _ = s.write_all(response.as_bytes());
            }
        });
        (format!("http://127.0.0.1:{}", addr.port()), rx)
    }

    fn read_full_request(s: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 8192];
        loop {
            match s.read(&mut tmp) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&tmp[..n]),
            }
            if buf.len() > 1 << 20 {
                break;
            }
            let Some(sep) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                continue;
            };
            let head = String::from_utf8_lossy(&buf[..sep]).into_owned();
            let content_len = head
                .lines()
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    k.eq_ignore_ascii_case("content-length")
                        .then(|| v.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buf.len() >= sep + 4 + content_len {
                break;
            }
        }
        String::from_utf8_lossy(&buf).into_owned()
    }

    fn response(status: &str, headers: &[(&str, &str)], body: &str) -> String {
        let mut out = format!("HTTP/1.1 {status}\r\n");
        for (k, v) in headers {
            out.push_str(&format!("{k}: {v}\r\n"));
        }
        out.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));
        out.push_str(body);
        out
    }

    #[test]
    fn get_returns_status_and_body() {
        let (base, rx) = spawn_server(vec![response(
            "200 OK",
            &[],
            "<root/>",
        )]);
        let r = get(&format!("{base}/desc.xml"), BUDGET).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body_text().unwrap(), "<root/>");
        let req = rx.recv().unwrap();
        assert!(req.starts_with("GET /desc.xml HTTP/1.1"), "{req}");
        assert!(req.to_ascii_lowercase().contains("host:"), "{req}");
    }

    #[test]
    fn get_follows_root_relative_redirect() {
        let (base, rx) = spawn_server(vec![
            response("302 Found", &[("Location", "/real.xml")], ""),
            response("200 OK", &[], "<device/>"),
        ]);
        let r = get(&format!("{base}/rootDesc.xml"), BUDGET).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.body_text().unwrap(), "<device/>");
        assert!(rx.recv().unwrap().starts_with("GET /rootDesc.xml"), "first request");
        assert!(rx.recv().unwrap().starts_with("GET /real.xml"), "redirect target");
    }

    #[test]
    fn redirect_cap_is_enforced() {
        let (base, _rx) = spawn_server(vec![
            response("301 Moved", &[("Location", "/a")], ""),
            response("301 Moved", &[("Location", "/b")], ""),
            response("301 Moved", &[("Location", "/c")], ""),
            response("301 Moved", &[("Location", "/d")], ""),
            response("301 Moved", &[("Location", "/e")], ""),
        ]);
        let err = get(&format!("{base}/x"), BUDGET).unwrap_err();
        assert!(err.message().contains("too many redirects"), "{err}");
    }

    #[test]
    fn slow_server_is_a_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                // hold the connection open, never answer
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                thread::sleep(Duration::from_millis(500));
            }
        });
        let err = get(&format!("http://127.0.0.1:{}/slow", addr.port()), Duration::from_millis(80))
            .unwrap_err();
        assert!(
            matches!(err, SourceError::Timeout { .. }),
            "expected timeout, got {err:?}"
        );
    }

    #[test]
    fn post_soap_carries_action_and_keeps_fault_status() {
        let (base, rx) = spawn_server(vec![response(
            "500 Internal Server Error",
            &[],
            "<s:Envelope><s:Body><s:Fault/></s:Body></s:Envelope>",
        )]);
        let envelope = browse_envelope();
        let r = post_soap(
            &format!("{base}/ContentDirectory/control"),
            "urn:schemas-upnp-org:service:ContentDirectory:1#Browse",
            &envelope,
            BUDGET,
        )
        .unwrap();
        // a SOAP fault arrives as 500 and must reach the parser intact
        assert_eq!(r.status, 500);
        assert!(r.body_text().unwrap().contains("<s:Fault"), "fault must reach the parser: {:?}", r.body_text());

        let req = rx.recv().unwrap();
        assert!(
            req.starts_with("POST /ContentDirectory/control HTTP/1.1"),
            "{req}"
        );
        assert!(
            req.contains("SOAPAction: \"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""),
            "{req}"
        );
        assert!(req.contains(&format!("Content-Length: {}", envelope.len())), "{req}");
        assert!(req.ends_with(&envelope), "body must follow headers: {req}");
    }

    #[test]
    fn chunked_bodies_are_dechunked() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        let parsed = parse_response(raw).unwrap();
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.body, b"hello world");
    }

    #[test]
    fn redirect_resolution() {
        assert_eq!(
            resolve_url("http://nas:8200/rootDesc.xml", "http://nas:8080/x").as_deref(),
            Some("http://nas:8080/x")
        );
        assert_eq!(
            resolve_url("http://nas:8200/rootDesc.xml", "/ctl").as_deref(),
            Some("http://nas:8200/ctl")
        );
        assert_eq!(
            resolve_url("http://nas:8200/a/b.xml", "c.xml").as_deref(),
            Some("http://nas:8200/a/c.xml")
        );
        assert_eq!(resolve_url("http://nas/x", "ftp://no"), None);
        assert_eq!(resolve_url("noscheme", "/x"), None);
    }

    fn browse_envelope() -> String {
        crate::dlna::browse_soap(crate::dlna::OBJECT_ID_ROOT)
    }
}
