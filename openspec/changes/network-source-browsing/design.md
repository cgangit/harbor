# Design

## Context

See proposal.md for motivation. Facts that shape this design:

- `harbor-sources` already owns working **parsers**: `msearch_packet`/`parse_ssdp` (SSDP), `parse_device_description`, `browse_soap` (SOAP body only), `parse_browse_response` + DIDL-Lite, `SmbCreds`/`canonical_path`. All are pure and fixture-tested; none touch a socket.
- `handle.rs` is the only networking in the crate: `MediaHandle::open_http` does a real `TcpStream` GET with `Range` (tested against a local `TcpListener`).
- `DlnaSource::list` reads a `pages` cache; `SmbSource::list` reads a `dirs` map that `new()` leaves empty. `SmbSource::fixture/auth_fail/timed_out` are how tests populate them today.
- `AddSource::discovering(servers)` takes a `Vec<String>` — the caller decides what discovery returned.
- `View` (from `wire-input-and-real-data`) already owns `catalog` and `needs_refresh`; `refresh_catalog` is synchronous local discovery only.
- Spec timeouts to honour: SSDP gives up after 8 s, SMB connect after 10 s (`CONNECT_TIMEOUT_MS`), both already coded as constants.

## Goals / Non-Goals

**Goals:**

- A configured DLNA/SMB source lists its first page from the real network, and the result reaches the Browser without blocking a single frame.
- Add Source → DLNA discovers servers on the LAN for real.
- Every failure mode the `sources` spec names (UPnP fault, HTTP status, auth, timeout) surfaces as a visible class, not a hang.
- `make check` stays green with local-fixture tests; live LAN checks are a documented, optional step.

**Non-Goals:**

- Nested folder browsing depth beyond what the first page + follow-up `list(path)` calls give (no paging UI yet; `StartingIndex` stays 0 with a generous `RequestedCount`).
- Playback wiring (its own change), Settings source edit/remove (its own change).
- Thumbnails, transcode, SMB multichannel/DFS, Kerberos.
- Replacing the existing pure parsers or fixture constructors — they stay as-is and remain the test bed.

## Decisions

**D1 — One minimal HTTP helper over `TcpStream`, no HTTP crate.**
`handle.rs` already proves the pattern (connect → write request → read headers → body, `Content-Length`/`Range` handling, `TcpListener` fixtures). Extract it as `harbor-sources::http::{get, post_soap}` and reuse it for the device-description GET and the ContentDirectory SOAP POST. *Alternative considered:* add `ureq`/`minreq` — rejected: a dependency buys redirects/chunking we do not need, and the ARM build already has enough third-party surface. Redirects (30x on `LOCATION`/control URLs) get handled explicitly, capped at 3 hops.

**D2 — SMB client by spike, with a fallback already chosen.**
Task 1 runs a time-boxed spike: (a) a pure-Rust SMB2 crate that supports NTLMv2 auth **and** positional reads (`read_at` is required by the seek path), (b) `libsmb2` C FFI, which the original design named. Selection criteria: passes against a local Samba with and without a password, cross-compiles to `armv7-unknown-linux-gnueabi` with the webOS NDK, and no runtime outside the app jail. If the crate fails any criterion, `libsmb2` is the fallback and the FFI seam mirrors `src/starfish.c` (thin C, `dlopen`-free static link). *Risk note:* whichever wins, `SmbSource::new` must gain a real "connect + readdir" path while keeping its fixture constructors for tests.

**D3 — Browse queue: `Arc<Mutex<Vec<BrowseResult>>>` drained once per frame.**
A worker thread holds a clone of the queue plus the source handle and posts the first `Page` (or an error class); the app loop drains it every frame into `View` (clearing `loading`, setting `catalog.files`). Chosen over `mpsc::Receiver` inside `View` because `View` derives `Clone + Debug + PartialEq + Eq` and a `Receiver` is none of those; a shared `Vec` keeps every derive intact and is trivially testable without threads. *Alternative considered:* `crossbeam` channel — rejected for the same derive problem plus a dependency.

**D4 — Loading and failure live in `View`, drawn by the Browser.**
`View.loading: Vec<String>` (source ids in flight) and `View.browse_error: Option<String>` (failure class). The Browser renders a loading row when `loading` is non-empty and no page has landed, and the error row when `browse_error` is set; `rows()` counts whichever is showing so focus stays valid. Trigger for a listing: selecting a configured source (Settings source row, and the sources shelf once navigation reaches it).

**D5 — Discovery drives `AddSource`, not the catalog.**
SSDP results populate `AddSource::discovering(names)` exactly as the current UI expects, so the existing step machine, timeout display, and tests keep working; a chosen server becomes a `SourceConfig` through the existing `Added` action. Local mounts stay in `refresh_catalog` (synchronous) — discovery runs only while the Add Source dialog is open, bounded by the 8 s timeout.

## Risks / Trade-offs

- [SMB crate may not support the dialect/auth the target NAS uses] → D2's spike tests the two realistic cases (passwordless and NTLMv2) before any integration code exists; `libsmb2` fallback is already scoped.
- [Multicast SSDP may be filtered on some LANs / TV Wi-Fi power-save] → the 8 s empty-state with Retry is already in the `sources` spec; the worker returns an empty list rather than an error, so the dialog shows that state.
- [Blocking reads inside the worker still tie up a thread] → one thread per open action, joined or detached after the first page; timeouts are enforced with `set_read_timeout`/`set_write_timeout` at the budget constants (8 s / 10 s), never left to the OS default.
- [Large listings freeze the Browser draw] → first page only (`RequestedCount` capped, e.g. 200 entries), sorted client-side like the local path.
- [Live-network behaviour is untestable in CI] → all CI tests use `TcpListener`/fixture constructors; live MiniDLNA/Samba checks are an explicit device/lan task, and `make check` never needs a network.
- [`View` grows more derived state] → `loading`/`browse_error` are cleared by the same drain step that fills the page, so they cannot drift from `catalog.files`.

## Migration Plan

1. HTTP helper + DLNA discovery/browse behind existing parsers (fixture tests first, then a local `TcpListener`).
2. SMB spike → client integration behind `SmbSource::list`, fixture constructors untouched.
3. Browse queue + loading row wired into `View` and the Browser.
4. `make check` green at every step; then the LAN checklist (MiniDLNA + Samba) and the device run.
5. Rollback is a revert — no stored format changes (`sources.json`, vault, resume log are untouched; DLNA pages are a runtime cache).

## Open Questions

- Which SMB implementation wins the spike (D2) affects only the integration task's shape, not the specs or the task breakdown: both paths need "connect, auth, readdir, read_at, classify errors" verified the same way. Resolved inside task 1, recorded in this file when it lands.
