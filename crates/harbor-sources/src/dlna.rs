use std::net::{ToSocketAddrs, UdpSocket};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::entry::{ext_of, is_video_name, Entry, EntryKind, Page, Probe};
use crate::error::SourceError;
use crate::handle::MediaHandle;
use crate::http;
use crate::{Kind, Source, SourceId};

pub const MEDIA_SERVER_ST: &str = "urn:schemas-upnp-org:device:MediaServer:1";
pub const CONTENT_DIRECTORY_ST: &str = "urn:schemas-upnp-org:service:ContentDirectory:1";
pub const DISCOVER_TIMEOUT_MS: u64 = 8_000;
pub const OBJECT_ID_ROOT: &str = "0";
/// SSDP multicast group (`msearch_packet` addresses it).
pub const SSDP_MULTICAST: &str = "239.255.255.250:1900";
/// Stop collecting this long after the most recent reply. LAN servers answer
/// a M-SEARCH within milliseconds; the full budget is reserved for the
/// empty-network case the spec pins at `DISCOVER_TIMEOUT_MS`.
pub const SSDP_QUIET_MS: u64 = 1_000;
/// Per-device budget for fetching the device description over HTTP.
pub const DESCRIBE_BUDGET: Duration = Duration::from_secs(2);
/// Budget for a single ContentDirectory Browse: the DLNA-side 8 s family
/// (design D1), enforced as connect/read/write timeouts by `http::request`.
pub const BROWSE_TIMEOUT: Duration = Duration::from_secs(8);
/// First-page cap so one Browse can never stream an unbounded `Result` at
/// the UI (design D3 risk: large listings freezing the Browser draw).
pub const BROWSE_PAGE_CAP: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SsdpDevice {
    pub location: String,
    pub usn: String,
    pub st: String,
    pub server: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaServer {
    pub udn: String,
    pub friendly_name: String,
    pub location: String,
    pub control_url: String,
}

#[derive(Debug)]
pub struct DlnaSource {
    pub id: SourceId,
    pub server: MediaServer,
    /// `object-id` → page cache. Filled by fixtures (`with_page`) and by the
    /// network path in `list`. A mutex (not a plain `Vec`) because
    /// `Source::list` takes `&self` and a browse worker fills it from
    /// another thread (design D3).
    pages: Mutex<Vec<(String, Page)>>,
}

impl Clone for DlnaSource {
    fn clone(&self) -> Self {
        let pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        Self {
            id: self.id.clone(),
            server: self.server.clone(),
            pages: Mutex::new(pages.clone()),
        }
    }
}

impl DlnaSource {
    pub fn from_server(server: MediaServer) -> Self {
        Self {
            id: SourceId(format!("dlna:{}", server.udn)),
            server,
            pages: Mutex::new(Vec::new()),
        }
    }

    pub fn with_page(self, object_id: &str, page: Page) -> Self {
        self.pages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push((object_id.to_string(), page));
        self
    }

    fn object_id(path: &str) -> String {
        if path.is_empty() || path == "/" {
            OBJECT_ID_ROOT.to_string()
        } else {
            path.trim_start_matches('/').to_string()
        }
    }

    /// A cached page, if this object was listed before (offline path).
    fn cached(&self, id: &str) -> Option<Page> {
        self.pages
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, p)| p.clone())
    }

    fn cache(&self, id: &str, page: Page) {
        let mut pages = self.pages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(slot) = pages.iter_mut().find(|(k, _)| k == id) {
            slot.1 = page;
        } else {
            pages.push((id.to_string(), page));
        }
    }
}

impl Source for DlnaSource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn kind(&self) -> Kind {
        Kind::Dlna
    }

    fn display_name(&self) -> &str {
        &self.server.friendly_name
    }

    /// First the page cache (a fixture or an earlier browse is served
    /// offline); on a miss, browse the ContentDirectory over HTTP and fill
    /// the cache (task 2.2). A UPnP "No such object" fault reads as
    /// `NotFound`, matching the local source; other faults keep their code.
    fn list(&self, path: &str) -> Result<Page, SourceError> {
        let id = Self::object_id(path);
        if let Some(page) = self.cached(&id) {
            return Ok(page);
        }
        match browse_page(&self.server.control_url, &id, BROWSE_TIMEOUT) {
            Ok(page) => {
                self.cache(&id, page.clone());
                Ok(page)
            }
            Err(SourceError::Upnp { code: 701, .. }) => Err(SourceError::NotFound {
                path: id,
            }),
            Err(e) => Err(e),
        }
    }

    fn open(&self, entry: &Entry) -> Result<MediaHandle, SourceError> {
        match entry.url.as_deref() {
            Some(url) => MediaHandle::open_http(url),
            None => Err(SourceError::NotFound {
                path: entry.id.clone(),
            }),
        }
    }

    fn probe(&self, entry: &Entry) -> Result<Probe, SourceError> {
        Ok(Probe {
            container: ext_of(&entry.name),
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration_ns: None,
            size: entry.size,
            bit_depth: None,
            dovi: None,
        })
    }
}

pub fn msearch_packet() -> String {
    format!(
        concat!(
            "M-SEARCH * HTTP/1.1\r\n",
            "HOST: 239.255.255.250:1900\r\n",
            "MAN: \"ssdp:discover\"\r\n",
            "MX: 3\r\n",
            "ST: {}\r\n",
            "\r\n"
        ),
        MEDIA_SERVER_ST
    )
}

pub fn parse_ssdp(packet: &str) -> Option<SsdpDevice> {
    let first = packet.lines().next()?.to_ascii_uppercase();
    if !(first.starts_with("HTTP/1.1 200") || first.starts_with("NOTIFY")) {
        return None;
    }
    let mut location = None;
    let mut usn = None;
    let mut st = None;
    let mut nts = None;
    let mut server = None;
    for line in packet.lines().skip(1) {
        let Some((k, v)) = split_header(line) else {
            continue;
        };
        match k.as_str() {
            "location" => location = Some(v),
            "usn" => usn = Some(v),
            "st" | "nt" => st = Some(v),
            "nts" => nts = Some(v),
            "server" => server = Some(v),
            _ => {}
        }
    }
    if nts.as_deref() == Some("ssdp:byebye") {
        return None;
    }
    let st = st?;
    if !st_is_media_server(&st) {
        return None;
    }
    Some(SsdpDevice {
        location: location?,
        usn: usn.unwrap_or_default(),
        st,
        server,
    })
}

pub fn parse_ssdp_many(packets: &[&str]) -> Vec<SsdpDevice> {
    let mut out = Vec::new();
    for p in packets {
        if let Some(dev) = parse_ssdp(p) {
            if !out.iter().any(|d: &SsdpDevice| d.usn == dev.usn && d.location == dev.location) {
                out.push(dev);
            }
        }
    }
    out
}

fn st_is_media_server(st: &str) -> bool {
    let s = st.to_ascii_lowercase();
    s.contains("mediaserver") || s == "upnp:rootdevice"
}

fn split_header(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once(':')?;
    Some((k.trim().to_ascii_lowercase(), v.trim().to_string()))
}

pub fn parse_device_description(xml: &str, location: &str) -> Option<MediaServer> {
    let device = inner_tag(xml, "device").unwrap_or(xml);
    let device_type = text_of(device, "deviceType").unwrap_or_default();
    if !device_type.to_ascii_lowercase().contains("mediaserver")
        && inner_tag(xml, "deviceType").is_some()
        && !xml.to_ascii_lowercase().contains("mediaserver")
    {
        return None;
    }
    let udn = text_of(device, "UDN")?;
    let friendly_name = text_of(device, "friendlyName").unwrap_or_else(|| udn.clone());
    let control_url = content_directory_control_url(xml, location)?;
    Some(MediaServer {
        udn,
        friendly_name,
        location: location.to_string(),
        control_url,
    })
}

fn content_directory_control_url(xml: &str, location: &str) -> Option<String> {
    let mut rest = xml;
    while let Some(svc) = inner_tag(rest, "service") {
        let st = text_of(svc, "serviceType").unwrap_or_default();
        if st.to_ascii_lowercase().contains("contentdirectory") {
            let ctrl = text_of(svc, "controlURL")?;
            return Some(join_url(location, &ctrl));
        }
        if let Some(idx) = rest.find("</service>") {
            rest = &rest[idx + "</service>".len()..];
        } else {
            break;
        }
    }
    None
}

pub fn join_url(base: &str, rel: &str) -> String {
    if rel.starts_with("http://") || rel.starts_with("https://") {
        return rel.to_string();
    }
    if rel.starts_with('/') {
        if let Some(scheme_end) = base.find("://") {
            let after = &base[scheme_end + 3..];
            let host = after.split('/').next().unwrap_or(after);
            return format!("{}://{}{}", &base[..scheme_end], host, rel);
        }
    }
    let trimmed = base.rsplit_once('/').map(|(a, _)| a).unwrap_or(base);
    format!("{trimmed}/{}", rel.trim_start_matches('/'))
}

/// Discover MediaServers via SSDP (design D5): send a M-SEARCH to `target`,
/// re-send once while nothing answers, and collect replies until `budget`
/// expires or replies have been quiet for `SSDP_QUIET_MS`. Each responder's
/// device description is then fetched over HTTP for its friendly name and
/// ContentDirectory control URL; responders that cannot be described are
/// skipped (a MediaServer without a control URL cannot be browsed).
pub fn discover_media_servers(
    target: &str,
    budget: Duration,
) -> Result<Vec<MediaServer>, SourceError> {
    let addr = target
        .to_socket_addrs()
        .map_err(|e| SourceError::Io(format!("ssdp target {target}: {e}")))?
        .next()
        .ok_or_else(|| SourceError::Io(format!("ssdp target {target}: no address")))?;
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| SourceError::Io(format!("ssdp bind: {e}")))?;
    let msearch = msearch_packet();
    socket
        .send_to(msearch.as_bytes(), addr)
        .map_err(|e| SourceError::Io(format!("ssdp send: {e}")))?;
    let mut sends = 1;
    let mut last_send = Instant::now();
    let quiet = Duration::from_millis(SSDP_QUIET_MS);
    let deadline = Instant::now() + budget;
    let mut packets: Vec<String> = Vec::new();
    let mut last_reply: Option<Instant> = None;
    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if let Some(t) = last_reply {
            if now.duration_since(t) >= quiet {
                break;
            }
        }
        // One re-send while the network stays silent: a single M-SEARCH can
        // be dropped, and the empty-network case still ends at `budget`.
        if sends < 2 && last_reply.is_none() && now.duration_since(last_send) >= Duration::from_millis(250) {
            let _ = socket.send_to(msearch.as_bytes(), addr);
            sends += 1;
            last_send = now;
        }
        let slice = deadline.saturating_duration_since(now).min(Duration::from_millis(100));
        socket
            .set_read_timeout(Some(slice))
            .map_err(|e| SourceError::Io(format!("ssdp timeout: {e}")))?;
        let mut buf = [0u8; 4096];
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                packets.push(String::from_utf8_lossy(&buf[..n]).into_owned());
                last_reply = Some(Instant::now());
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(SourceError::Io(e.to_string())),
        }
    }
    let refs: Vec<&str> = packets.iter().map(String::as_str).collect();
    let mut servers = Vec::new();
    for device in parse_ssdp_many(&refs) {
        match describe(&device.location, DESCRIBE_BUDGET) {
            Some(server) => servers.push(server),
            None => harbor_base::log(&format!(
                "discovery: no description from {}",
                device.location
            )),
        }
    }
    harbor_base::log(&format!("discovery: {} server(s)", servers.len()));
    Ok(servers)
}

/// Fetch a device description over HTTP and parse its identity (design D1).
/// Returns `None` for transport errors, non-200 responses, and documents
/// without a ContentDirectory control URL — none of those can be browsed.
pub fn describe(location: &str, budget: Duration) -> Option<MediaServer> {
    let resp = http::get(location, budget).ok()?;
    if resp.status != 200 {
        return None;
    }
    let xml = resp.body_text().ok()?;
    parse_device_description(xml, location)
}

/// POST a ContentDirectory `Browse` for `object_id` through the HTTP helper
/// and parse the DIDL page (design D1). Only the first page is requested —
/// `RequestedCount` capped at [`BROWSE_PAGE_CAP`] so one Browse cannot
/// stream an unbounded result. UPnP faults arrive as HTTP 500 with a
/// `s:Fault` body; `parse_browse_response` turns those into
/// `SourceError::Upnp`.
pub fn browse_page(
    control_url: &str,
    object_id: &str,
    budget: Duration,
) -> Result<Page, SourceError> {
    let envelope = browse_soap_paged(object_id, 0, BROWSE_PAGE_CAP);
    let resp = http::post_soap(
        control_url,
        "urn:schemas-upnp-org:service:ContentDirectory:1#Browse",
        &envelope,
        budget,
    )?;
    let body = resp.body_text()?.to_string();
    parse_browse_response(&body)
}

pub fn browse_soap(object_id: &str) -> String {
    browse_soap_paged(object_id, 0, 0)
}

/// Paged variant: `count = 0` asks the server for everything, while the
/// listing path always caps at [`BROWSE_PAGE_CAP`].
pub fn browse_soap_paged(object_id: &str, start: usize, count: usize) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" ",
            "s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">",
            "<s:Body>",
            "<u:Browse xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\">",
            "<ObjectID>{}</ObjectID>",
            "<BrowseFlag>BrowseDirectChildren</BrowseFlag>",
            "<Filter>*</Filter>",
            "<StartingIndex>{}</StartingIndex>",
            "<RequestedCount>{}</RequestedCount>",
            "<SortCriteria></SortCriteria>",
            "</u:Browse>",
            "</s:Body>",
            "</s:Envelope>"
        ),
        xml_escape(object_id),
        start,
        count
    )
}

pub fn parse_browse_response(soap: &str) -> Result<Page, SourceError> {
    if let Some(code) = upnp_fault(soap) {
        return Err(code);
    }
    let result = text_of(soap, "Result").ok_or_else(|| SourceError::Io("Browse missing Result".into()))?;
    let didl = xml_unescape(&result);
    Ok(Page {
        entries: parse_didl(&didl),
    })
}

fn upnp_fault(soap: &str) -> Option<SourceError> {
    let code = text_of(soap, "errorCode")?.parse().ok()?;
    let description = text_of(soap, "errorDescription").unwrap_or_default();
    Some(SourceError::Upnp { code, description })
}

pub fn parse_didl(xml: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    collect_containers(xml, &mut entries);
    collect_items(xml, &mut entries);
    entries
}

fn collect_containers(xml: &str, out: &mut Vec<Entry>) {
    let mut rest = xml;
    while let Some((tag, after)) = next_element(rest, "container") {
        if let Some(entry) = container_entry(&tag) {
            out.push(entry);
        }
        rest = after;
    }
}

fn collect_items(xml: &str, out: &mut Vec<Entry>) {
    let mut rest = xml;
    while let Some((tag, after)) = next_element(rest, "item") {
        if let Some(entry) = item_entry(&tag) {
            out.push(entry);
        }
        rest = after;
    }
}

fn container_entry(tag: &str) -> Option<Entry> {
    let id = attr(tag, "id")?;
    let parent = attr(tag, "parentID").unwrap_or_default();
    let name = text_of(tag, "title").unwrap_or_else(|| id.clone());
    Some(Entry {
        id,
        parent,
        name,
        kind: EntryKind::Dir,
        size: None,
        mtime: None,
        url: None,
    })
}

fn item_entry(tag: &str) -> Option<Entry> {
    let id = attr(tag, "id")?;
    let parent = attr(tag, "parentID").unwrap_or_default();
    let name = text_of(tag, "title").unwrap_or_else(|| id.clone());
    let class = text_of(tag, "class").unwrap_or_default();
    let res = first_res(tag);
    let url = res.as_ref().map(|(u, _)| u.clone());
    let size = res.as_ref().and_then(|(_, s)| *s);
    let kind = if class.to_ascii_lowercase().contains("video")
        || url.as_deref().map(is_video_name).unwrap_or(false)
        || is_video_name(&name)
    {
        EntryKind::Video
    } else {
        EntryKind::Other
    };
    if kind == EntryKind::Other {
        return None;
    }
    Some(Entry {
        id,
        parent,
        name,
        kind,
        size,
        mtime: None,
        url,
    })
}

fn first_res(tag: &str) -> Option<(String, Option<u64>)> {
    let (elem, _) = next_element(tag, "res")?;
    let body = inner_tag(&elem, "res")?;
    let url = xml_unescape(body.trim());
    if url.is_empty() {
        return None;
    }
    let size = attr(&elem, "size").and_then(|s| s.parse().ok());
    Some((url, size))
}

fn local_name(raw: &str) -> &str {
    raw.rsplit(':').next().unwrap_or(raw)
}

fn find_open(xml: &str, name: &str) -> Option<(usize, usize, bool)> {
    let mut i = 0;
    while let Some(rel) = xml[i..].find('<') {
        let start = i + rel;
        let rest = &xml[start + 1..];
        if rest.starts_with('/') || rest.starts_with('!') || rest.starts_with('?') {
            i = start + 1;
            continue;
        }
        let name_end = rest.find(|c: char| c == ' ' || c == '>' || c == '/' || c == '\n' || c == '\t' || c == '\r')?;
        let raw = &rest[..name_end];
        if local_name(raw) == name {
            let gt = xml[start..].find('>')?;
            let self_closing = xml.as_bytes().get(start + gt - 1) == Some(&b'/');
            return Some((start, start + gt + 1, self_closing));
        }
        i = start + 1;
    }
    None
}

fn next_element<'a>(xml: &'a str, name: &str) -> Option<(String, &'a str)> {
    let (start, inner_start, self_closing) = find_open(xml, name)?;
    if self_closing {
        return Some((xml[start..inner_start].to_string(), &xml[inner_start..]));
    }
    let mut depth = 1;
    let mut i = inner_start;
    while depth > 0 {
        let next_lt = xml[i..].find('<')?;
        let pos = i + next_lt;
        if xml[pos..].starts_with("</") {
            let rest = &xml[pos + 2..];
            let name_end = rest.find(|c: char| c == ' ' || c == '>' || c == '\n' || c == '\t' || c == '\r')?;
            let raw = &rest[..name_end];
            if local_name(raw) == name {
                depth -= 1;
                if depth == 0 {
                    let gt = xml[pos..].find('>')?;
                    let end = pos + gt + 1;
                    return Some((xml[start..end].to_string(), &xml[end..]));
                }
            }
            i = pos + 1;
        } else if xml[pos..].starts_with("<?") || xml[pos..].starts_with("<!") {
            i = pos + 1;
        } else {
            let rest = &xml[pos + 1..];
            let name_end = rest.find(|c: char| c == ' ' || c == '>' || c == '/' || c == '\n' || c == '\t' || c == '\r')?;
            let raw = &rest[..name_end];
            let gt = xml[pos..].find('>')?;
            let self_c = xml.as_bytes().get(pos + gt - 1) == Some(&b'/');
            if local_name(raw) == name && !self_c {
                depth += 1;
            }
            i = pos + gt + 1;
        }
    }
    None
}

fn inner_tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let (start, inner_start, self_closing) = find_open(xml, name)?;
    if self_closing {
        return Some("");
    }
    let (elem, _) = next_element(&xml[start..], name)?;
    let close = elem.rfind("</")?;
    let inner_len = close.saturating_sub(inner_start - start);
    Some(&xml[inner_start..inner_start + inner_len])
}

fn text_of(xml: &str, name: &str) -> Option<String> {
    let inner = inner_tag(xml, name)?;
    let t = inner.trim();
    if t.is_empty() {
        None
    } else {
        Some(xml_unescape(t))
    }
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let head = tag.split('>').next()?;
    for quote in ['"', '\''] {
        let key = format!("{name}={quote}");
        if let Some(i) = head.find(&key) {
            let rest = &head[i + key.len()..];
            let end = rest.find(quote)?;
            return Some(xml_unescape(&rest[..end]));
        }
    }
    None
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub const SAMPLE_SSDP_OK: &str = concat!(
    "HTTP/1.1 200 OK\r\n",
    "CACHE-CONTROL: max-age=1800\r\n",
    "LOCATION: http://192.168.1.10:8200/rootDesc.xml\r\n",
    "SERVER: Linux/5.10 UPnP/1.0 MiniDLNA/1.3\r\n",
    "ST: urn:schemas-upnp-org:device:MediaServer:1\r\n",
    "USN: uuid:minidlna-living::urn:schemas-upnp-org:device:MediaServer:1\r\n",
    "\r\n"
);

pub const SAMPLE_SSDP_NOTIFY: &str = concat!(
    "NOTIFY * HTTP/1.1\r\n",
    "HOST: 239.255.255.250:1900\r\n",
    "NT: urn:schemas-upnp-org:device:MediaServer:1\r\n",
    "NTS: ssdp:alive\r\n",
    "LOCATION: http://192.168.1.11:8200/rootDesc.xml\r\n",
    "USN: uuid:nas-office::urn:schemas-upnp-org:device:MediaServer:1\r\n",
    "\r\n"
);

pub const SAMPLE_SSDP_RENDERER: &str = concat!(
    "HTTP/1.1 200 OK\r\n",
    "LOCATION: http://192.168.1.20:1400/xml/device_description.xml\r\n",
    "ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\n",
    "USN: uuid:renderer-1::urn:schemas-upnp-org:device:MediaRenderer:1\r\n",
    "\r\n"
);

pub const SAMPLE_DEVICE_XML: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaServer:1</deviceType>
    <friendlyName>Living Room NAS</friendlyName>
    <UDN>uuid:minidlna-living</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ContentDirectory:1</serviceType>
        <serviceId>urn:upnp-org:serviceId:ContentDirectory</serviceId>
        <controlURL>/ctl/ContentDir</controlURL>
        <eventSubURL>/evt/ContentDir</eventSubURL>
        <SCPDURL>/ContentDir.xml</SCPDURL>
      </service>
    </serviceList>
  </device>
</root>
"#;

pub const SAMPLE_DIDL: &str = r#"<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/">
  <container id="1" parentID="0" restricted="1">
    <dc:title>Movies</dc:title>
    <upnp:class>object.container.storageFolder</upnp:class>
  </container>
  <container id="2" parentID="0" restricted="1">
    <dc:title>Shows</dc:title>
    <upnp:class>object.container.storageFolder</upnp:class>
  </container>
  <item id="64" parentID="1" restricted="1">
    <dc:title>The River</dc:title>
    <upnp:class>object.item.videoItem.movie</upnp:class>
    <res protocolInfo="http-get:*:video/x-matroska:*" size="1048576">http://192.168.1.10:8200/MediaItems/64.mkv</res>
  </item>
  <item id="65" parentID="1" restricted="1">
    <dc:title>notes.txt</dc:title>
    <upnp:class>object.item.textItem</upnp:class>
    <res protocolInfo="http-get:*:text/plain:*">http://192.168.1.10:8200/MediaItems/65.txt</res>
  </item>
</DIDL-Lite>
"#;

pub const SAMPLE_BROWSE_SOAP: &str = concat!(
    "<?xml version=\"1.0\"?>",
    "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">",
    "<s:Body>",
    "<u:BrowseResponse xmlns:u=\"urn:schemas-upnp-org:service:ContentDirectory:1\">",
    "<Result>",
    "&lt;DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\"&gt;",
    "&lt;container id=\"1\" parentID=\"0\" restricted=\"1\"&gt;",
    "&lt;dc:title&gt;Movies&lt;/dc:title&gt;",
    "&lt;upnp:class&gt;object.container.storageFolder&lt;/upnp:class&gt;",
    "&lt;/container&gt;",
    "&lt;item id=\"64\" parentID=\"1\" restricted=\"1\"&gt;",
    "&lt;dc:title&gt;The River&lt;/dc:title&gt;",
    "&lt;upnp:class&gt;object.item.videoItem.movie&lt;/upnp:class&gt;",
    "&lt;res protocolInfo=\"http-get:*:video/x-matroska:*\" size=\"1048576\"&gt;http://192.168.1.10:8200/MediaItems/64.mkv&lt;/res&gt;",
    "&lt;/item&gt;",
    "&lt;/DIDL-Lite&gt;",
    "</Result>",
    "<NumberReturned>2</NumberReturned>",
    "<TotalMatches>2</TotalMatches>",
    "<UpdateID>1</UpdateID>",
    "</u:BrowseResponse>",
    "</s:Body>",
    "</s:Envelope>"
);

pub const SAMPLE_BROWSE_FAULT: &str = concat!(
    "<?xml version=\"1.0\"?>",
    "<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\">",
    "<s:Body>",
    "<s:Fault>",
    "<detail>",
    "<UPnPError xmlns=\"urn:schemas-upnp-org:control-1-0\">",
    "<errorCode>701</errorCode>",
    "<errorDescription>No such object</errorDescription>",
    "</UPnPError>",
    "</detail>",
    "</s:Fault>",
    "</s:Body>",
    "</s:Envelope>"
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpStream;

    /// Read one HTTP request (headers + Content-Length body) before the
    /// fixture replies — replying mid-body resets the client's write.
    fn read_full_request(s: &mut TcpStream) -> String {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 4096];
        loop {
            let Ok(n) = s.read(&mut tmp) else { break };
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
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

    #[test]
    fn msearch_targets_mediaserver() {
        let p = msearch_packet();
        assert!(p.starts_with("M-SEARCH * HTTP/1.1"));
        assert!(p.contains(MEDIA_SERVER_ST));
        assert!(p.contains("ssdp:discover"));
    }

    /// End-to-end discovery against local fixtures: a UDP responder answers
    /// the M-SEARCH with two known devices, and a TCP server serves their
    /// device descriptions. Budget is short because the loop also honours
    /// the quiet window (the fixture replies immediately).
    #[test]
    fn discover_media_servers_returns_two_known_devices() {
        use std::io::Write;
        use std::net::{TcpListener, UdpSocket};
        use std::thread;

        let device_xml = |friendly: &str, udn: &str| {
            SAMPLE_DEVICE_XML
                .replace("Living Room NAS", friendly)
                .replace("uuid:minidlna-living", udn)
        };

        // HTTP fixture: route each path to one known device description.
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let http_port = http.local_addr().unwrap().port();
        thread::spawn(move || {
            for conn in http.incoming().flatten() {
                let mut conn = conn;
                let mut buf = [0u8; 2048];
                let n = conn.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let (friendly, udn) = if req.starts_with("GET /dev1.xml") {
                    ("Living Room NAS", "uuid:minidlna-living")
                } else {
                    ("Office NAS", "uuid:office-nas")
                };
                let body = device_xml(friendly, udn);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = conn.write_all(resp.as_bytes());
            }
        });

        // UDP fixture: answer the M-SEARCH with both devices' locations.
        let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let udp_port = udp.local_addr().unwrap().port();
        thread::spawn(move || {
            let mut buf = [0u8; 2048];
            if let Ok((_, from)) = udp.recv_from(&mut buf) {
                for path in ["/dev1.xml", "/dev2.xml"] {
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\n\
                         LOCATION: http://127.0.0.1:{http_port}{path}\r\n\
                         ST: {MEDIA_SERVER_ST}\r\n\
                         USN: uuid:{path}::{MEDIA_SERVER_ST}\r\n\r\n"
                    );
                    let _ = udp.send_to(reply.as_bytes(), from);
                }
            }
        });

        let servers =
            discover_media_servers(&format!("127.0.0.1:{udp_port}"), Duration::from_millis(400))
                .unwrap();
        let mut names: Vec<&str> = servers
            .iter()
            .map(|s| s.friendly_name.as_str())
            .collect();
        names.sort_unstable();
        assert_eq!(names, vec!["Living Room NAS", "Office NAS"], "{servers:?}");
        let office = servers
            .iter()
            .find(|s| s.friendly_name == "Office NAS")
            .unwrap();
        assert_eq!(office.udn, "uuid:office-nas");
        assert_eq!(
            office.control_url,
            format!("http://127.0.0.1:{http_port}/ctl/ContentDir")
        );
    }

    #[test]
    fn ssdp_ok_and_notify_keep_mediaserver() {
        let a = parse_ssdp(SAMPLE_SSDP_OK).unwrap();
        assert_eq!(a.location, "http://192.168.1.10:8200/rootDesc.xml");
        assert!(a.usn.contains("minidlna-living"));
        let b = parse_ssdp(SAMPLE_SSDP_NOTIFY).unwrap();
        assert_eq!(b.location, "http://192.168.1.11:8200/rootDesc.xml");
        assert!(parse_ssdp(SAMPLE_SSDP_RENDERER).is_none());
        let many = parse_ssdp_many(&[SAMPLE_SSDP_OK, SAMPLE_SSDP_OK, SAMPLE_SSDP_NOTIFY]);
        assert_eq!(many.len(), 2);
    }

    #[test]
    fn device_xml_yields_control_url() {
        let s = parse_device_description(SAMPLE_DEVICE_XML, "http://192.168.1.10:8200/rootDesc.xml")
            .unwrap();
        assert_eq!(s.friendly_name, "Living Room NAS");
        assert_eq!(s.udn, "uuid:minidlna-living");
        assert_eq!(s.control_url, "http://192.168.1.10:8200/ctl/ContentDir");
    }

    #[test]
    fn didl_folders_and_videos_skip_text() {
        let entries = parse_didl(SAMPLE_DIDL);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Movies"));
        assert!(names.contains(&"Shows"));
        assert!(names.contains(&"The River"));
        assert!(!names.contains(&"notes.txt"));
        let video = entries.iter().find(|e| e.name == "The River").unwrap();
        assert_eq!(video.kind, EntryKind::Video);
        assert_eq!(
            video.url.as_deref(),
            Some("http://192.168.1.10:8200/MediaItems/64.mkv")
        );
        assert_eq!(video.size, Some(1_048_576));
    }

    #[test]
    fn browse_soap_and_fault() {
        let body = browse_soap("0");
        assert!(body.contains("<ObjectID>0</ObjectID>"));
        assert!(body.contains("BrowseDirectChildren"));
        let page = parse_browse_response(SAMPLE_BROWSE_SOAP).unwrap();
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].name, "Movies");
        assert_eq!(page.entries[1].kind, EntryKind::Video);
        let err = parse_browse_response(SAMPLE_BROWSE_FAULT).unwrap_err();
        match err {
            SourceError::Upnp { code, description } => {
                assert_eq!(code, 701);
                assert_eq!(description, "No such object");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn source_lists_fixture_page() {
        let page = parse_browse_response(SAMPLE_BROWSE_SOAP).unwrap();
        let mut server = parse_device_description(
            SAMPLE_DEVICE_XML,
            "http://192.168.1.10:8200/rootDesc.xml",
        )
        .unwrap();
        // A dead control URL proves the cached page is served without any
        // network I/O (task 2.2: cached fixture page offline).
        server.control_url = "http://127.0.0.1:1/ctl/ContentDir".into();
        let src = DlnaSource::from_server(server).with_page(OBJECT_ID_ROOT, page);
        assert_eq!(src.kind(), Kind::Dlna);
        assert_eq!(src.display_name(), "Living Room NAS");
        let listed = src.list("").unwrap();
        assert_eq!(listed.entries.len(), 2);
        let video = listed.entries.iter().find(|e| e.kind == EntryKind::Video).unwrap();
        assert!(video.url.as_deref().unwrap().starts_with("http://"));
        // A cache miss takes the network path — and the dead URL fails.
        let err = src.list("missing").unwrap_err();
        assert!(matches!(err, SourceError::Io(_)), "{err:?}");
    }

    /// The network path (task 2.2): an uncached `list` POSTs Browse to the
    /// local server and fills the page cache, so the next call is offline
    /// even after the server is gone.
    #[test]
    fn network_path_fills_cache_from_local_server() {
        use std::io::Write;
        use std::net::TcpListener;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let served = Arc::new(AtomicUsize::new(0));
        let counter = served.clone();
        let request = Arc::new(std::sync::Mutex::new(String::new()));
        let request_clone = request.clone();
        thread::spawn(move || {
            // One connection only: a second Browse attempt would be refused.
            if let Ok((mut s, _)) = listener.accept() {
                counter.fetch_add(1, Ordering::SeqCst);
                *request_clone.lock().unwrap() = read_full_request(&mut s);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    SAMPLE_BROWSE_SOAP.len(),
                    SAMPLE_BROWSE_SOAP
                );
                let _ = s.write_all(resp.as_bytes());
            }
        });

        let server = MediaServer {
            udn: "uuid:net-path".into(),
            friendly_name: "Net Path".into(),
            location: format!("http://127.0.0.1:{port}/rootDesc.xml"),
            control_url: format!("http://127.0.0.1:{port}/ctl/ContentDir"),
        };
        let src = DlnaSource::from_server(server);
        let listed = src.list("").unwrap();
        assert_eq!(listed.entries.len(), 2);
        assert_eq!(served.load(Ordering::SeqCst), 1, "one Browse POST");
        // First page only: RequestedCount capped at BROWSE_PAGE_CAP.
        let req = request.lock().unwrap();
        assert!(
            req.contains("<StartingIndex>0</StartingIndex>")
                && req.contains(&format!(
                    "<RequestedCount>{BROWSE_PAGE_CAP}</RequestedCount>"
                )),
            "{req}"
        );
        drop(req);
        // Second call: served from the page cache, not the (now closed) server.
        let cached = src.list("").unwrap();
        assert_eq!(cached.entries.len(), 2);
        assert_eq!(served.load(Ordering::SeqCst), 1, "cache must not re-post");
    }

    /// UPnP 701 "No such object" is a missing path, not a transport failure.
    #[test]
    fn missing_object_fault_reads_as_not_found() {
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else {
                return;
            };
            let _ = read_full_request(&mut s);
            let resp = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                SAMPLE_BROWSE_FAULT.len(),
                SAMPLE_BROWSE_FAULT
            );
            let _ = s.write_all(resp.as_bytes());
        });

        let server = MediaServer {
            udn: "uuid:faulty".into(),
            friendly_name: "Faulty".into(),
            location: format!("http://127.0.0.1:{port}/rootDesc.xml"),
            control_url: format!("http://127.0.0.1:{port}/ctl/ContentDir"),
        };
        let src = DlnaSource::from_server(server);
        match src.list("99").unwrap_err() {
            SourceError::NotFound { path } => assert_eq!(path, "99"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn empty_discovery_timeout_copy() {
        let err = SourceError::Timeout {
            what: "DLNA discovery".into(),
        };
        assert_eq!(err.message(), "timed out: DLNA discovery");
        assert_eq!(DISCOVER_TIMEOUT_MS, 8_000);
    }

    /// One local server, two connections: first the device description
    /// (`describe`), then the ContentDirectory Browse POST answered with the
    /// recorded `SAMPLE_BROWSE_SOAP` (task 2.1).
    #[test]
    fn describe_and_browse_page_round_trip_over_http() {
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for _ in 0..2 {
                let Ok((mut s, _)) = listener.accept() else {
                    break;
                };
                requests.push(read_full_request(&mut s));
                let body = if requests.len() == 1 {
                    SAMPLE_DEVICE_XML.to_string()
                } else {
                    SAMPLE_BROWSE_SOAP.to_string()
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = s.write_all(resp.as_bytes());
            }
            requests
        });

        let base = format!("http://127.0.0.1:{port}");
        let budget = Duration::from_secs(2);
        let server_desc = describe(&format!("{base}/rootDesc.xml"), budget).unwrap();
        assert_eq!(server_desc.friendly_name, "Living Room NAS");
        assert_eq!(
            server_desc.control_url,
            format!("{base}/ctl/ContentDir")
        );

        let page = browse_page(&server_desc.control_url, OBJECT_ID_ROOT, budget).unwrap();
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].name, "Movies");
        assert_eq!(page.entries[1].kind, EntryKind::Video);

        let requests = server.join().unwrap();
        let post = requests.get(1).expect("second request is the Browse POST");
        assert!(post.starts_with("POST /ctl/ContentDir HTTP/1.1"), "{post}");
        assert!(
            post.contains("SOAPAction: \"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\""),
            "{post}"
        );
        assert!(post.contains("<ObjectID>0</ObjectID>"), "{post}");
    }

    /// UPnP faults travel as HTTP 500 bodies; `browse_page` must surface
    /// them as `SourceError::Upnp`, not as a transport failure.
    #[test]
    fn browse_page_keeps_upnp_fault_status() {
        use std::io::Write;
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let Ok((mut s, _)) = listener.accept() else {
                return;
            };
            let _ = read_full_request(&mut s);
            let resp = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                SAMPLE_BROWSE_FAULT.len(),
                SAMPLE_BROWSE_FAULT
            );
            let _ = s.write_all(resp.as_bytes());
        });

        let url = format!("http://127.0.0.1:{port}/ctl/ContentDir");
        match browse_page(&url, "99", Duration::from_secs(2)) {
            Err(SourceError::Upnp { code, .. }) => assert_eq!(code, 701),
            other => panic!("expected Upnp fault, got {other:?}"),
        }
    }
}
