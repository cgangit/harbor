# Proposal

## Why

The `sources` spec requires real DLNA discovery/browse and SMB listing, but the code behind `DlnaSource::list` and `SmbSource::list` has no network I/O at all — `browse_soap()` only formats a SOAP body, SSDP helpers are pure functions with no sockets, and `SmbSource::list` reads an in-memory map filled by fixtures. Add Source → DLNA shows hard-coded server names. Change `wire-input-and-real-data` deferred its worker-thread listing tasks (4.1/4.2) here for exactly this reason: a loading row over a cache that never fills is a no-op on a real TV.

## What Changes

- **SSDP discovery**: multicast M-SEARCH (`239.255.255.250:1900`) with the 8-second timeout the spec already requires, device-description fetch, and real server names fed into Add Source → DLNA instead of the hard-coded list.
- **DLNA browse**: HTTP GET of the device description and a SOAP `ContentDirectory:1` `Browse` POST over `TcpStream`, parsed with the existing `parse_browse_response` / DIDL-Lite code; `DlnaSource::list` gains a network path with a page cache.
- **SMB listing**: a real client behind `SmbSource::list`, chosen by a spike (pure-Rust crate vs `libsmb2` FFI) — see design.md D2.
- **Async browse**: worker thread posts the first `Page` back through a queue drained once per frame into `View`; the Browser shows a loading row until it arrives and input stays responsive (this is the deferred tasks 4.1/4.2).
- **No playback changes**: opening a listed file still uses the current player path; playback wiring remains its own change.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `browsing`: "Home and source browser" gains scenarios for the loading state while a network source is being listed, and for listing never blocking input — both are user-visible contracts the current spec does not state.

## Impact

- `crates/harbor-sources/src/dlna.rs` (discovery + browse go from pure helpers to sockets), `smb.rs` (client behind `list()`), possibly a new `http.rs` for the minimal GET/POST helper shared with `handle.rs`.
- `crates/harbor-ui-kit/src/add_source.rs` (discovering step consumes real results), `crates/harbor-screens/src/nav.rs` + `catalog.rs` (browse queue, loading row, drain).
- Dependencies: possibly one SMB crate or the `libsmb2` C library (host + armv7 builds), decided by spike.
- Verification: `make check` stays green with local `TcpListener` fixtures; live checks need a MiniDLNA/ReadyMedia host and a Samba share on the LAN, plus the device run.
