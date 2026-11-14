# Tasks

## 1. HTTP helper and DLNA discovery (design D1, D5)

- [x] 1.1 Extract a minimal `http::{get, post_soap}` helper from `handle.rs`'s Range code (redirect cap 3, read/write timeouts at the spec's 8 s / 10 s budgets); verify with `TcpListener` fixture tests covering body, redirect, and timeout classification (`cargo test -p harbor-sources`).
- [x] 1.2 Implement SSDP discovery: multicast M-SEARCH to `239.255.255.250:1900`, collect responses for 8 s, parse with the existing `parse_ssdp` / `parse_device_description`; verify with a local UDP responder fixture test that returns two known devices (`cargo test -p harbor-sources`). (`dlna::discover_media_servers(target, budget)`; quiet-window early exit so a live LAN resolves in ~1 s while an empty network still ends at the budget; one silent re-send; fixture test drives UDP responder + HTTP description server)
- [x] 1.3 Feed real discovery results into `AddSource::discovering` (replacing the hard-coded server names) and turn the chosen server into a `SourceConfig` through the existing `Added` action; verify with a test that a discovered server lands in `SourceStore` (`cargo test -p harbor-screens`). (`discovery::poll_discovery` runs each frame beside `tick_add` on a worker thread; `View.discovery: DiscoveryHandle` shared with the thread; `SourceConfig::dlna(&MediaServer)` persists `dlna:{udn}` via `Added`; tests: `discovery_results_feed_the_dialog_and_land_in_source_store`, `late_results_reopen_the_empty_state`)

## 2. DLNA browse (design D1)

- [x] 2.1 Fetch the device description and POST the ContentDirectory `Browse` (`browse_soap`) through the new helper, parsing with `parse_browse_response`; verify against a `TcpListener` serving the recorded `SAMPLE_BROWSE_SOAP` (`cargo test -p harbor-sources`). (`dlna::describe(location, budget)` + `dlna::browse_page(control_url, object_id, budget)`; tests: `describe_and_browse_page_round_trip_over_http` (GET description → POST Browse with SOAPAction + ObjectID assertions), `browse_page_keeps_upnp_fault_status`)
- [x] 2.2 Give `DlnaSource::list` a network path with a page cache (first page, `RequestedCount` capped at 200); verify that a cached fixture page is still served offline and that the network path fills the cache from the local server (`cargo test -p harbor-sources`). (`pages` became a `Mutex<Vec<(String, Page)>>` with a manual `Clone`; cache hit returns offline, miss calls `browse_page` with `BROWSE_TIMEOUT` 8 s / `BROWSE_PAGE_CAP` 200 (`browse_soap_paged`), UPnP 701 → `NotFound`; tests: `source_lists_fixture_page` (dead control URL), `network_path_fills_cache_from_local_server` (one POST, cached second read), `missing_object_fault_reads_as_not_found`)

## 3. SMB client (design D2)

- [x] 3.1 Time-boxed spike: evaluate a pure-Rust SMB2 crate against `libsmb2` FFI — NTLMv2 auth with and without a password, positional `read_at`, and an `armv7-unknown-linux-gnueabi` build (this host cannot cross-compile; verify the ARM side in CI via a push or `workflow_dispatch` on `build-ipk.yml`); verify by recording the verdict and its evidence (test output) in design.md D2. (**Verdict: `smb2` 0.43.0 wins**, recorded with full test output in design.md D2 — guest + NTLMv2 pass against a containerized Samba, wrong password → `STATUS_LOGON_FAILURE`, bad share → `STATUS_BAD_NETWORK_NAME`, `read_at` independent offsets, `cargo check --target armv7-unknown-linux-gnueabi` clean; ARM link rides CI after task 3.2's push)
- [x] 3.2 Implement connect + readdir behind `SmbSource::list` with errors routed through the existing `classify_connect` / `classify_nt_status`, keeping the fixture constructors untouched; verify with offline unit tests plus an opt-in live test (`HARBOR_SMB_LIVE=1`, local Samba) (`cargo test -p harbor-sources`). (`SmbSource::networked(creds)` sets the new `network` flag — `fixture`/`auth_fail`/`timed_out` untouched; `dirs` became a `Mutex<BTreeMap>` (manual `Clone`) reused as the listing cache; `readdir` = `SmbClient::connect` → `connect_share` → `list_directory` under `futures_lite::block_on`, errors through `classify_smb_failure(status, timed_out, auth, …)` → `classify_nt_status` / `classify_connect`, NT 701-family codes → `NotFound`, dot entries filtered in `page_from`; tests: `classify_smb_failure_routes_status_budget_and_auth`, `smb_addr_defaults_port_and_keeps_explicit`, `networked_list_dials_and_classifies_dead_host`, `networked_cache_serves_without_network`, opt-in `live_smb_connect_readdir_and_cache` (guest + subdir + NTLMv2 + wrong password + missing share, green against containerized Samba on `127.0.0.1:1445`))

## 4. Async browse, loading row, wiring (deferred tasks 4.1/4.2 of `wire-input-and-real-data`; design D3, D4)

- [ ] 4.1 Add the browse queue (`Arc<Mutex<Vec<BrowseResult>>>`) plus `View.loading` and `View.browse_error`, with a drain step that clears them as it fills `catalog.files`; verify with a test that posts a fixture page and finds it in `View` after one drain (`cargo test -p harbor-screens`).
- [ ] 4.2 Spawn the worker on DLNA/SMB source open and draw the Browser loading row (and the failure row) with `rows()` counting them so focus stays valid; verify with a stub source whose `list()` blocks briefly that `handle` keeps responding and the loading row shows until the post arrives (`cargo test -p harbor-screens`).
- [ ] 4.3 Wire the trigger: selecting a configured source (Settings source row, sources shelf) starts a listing; verify with a nav test that selecting an SMB source sets `loading` and the drained page reaches `catalog.files` (`cargo test -p harbor-screens`).

## 5. Integration and verification

- [ ] 5.1 Run `make check` (workspace tests + `ci/test_mkipk.py`) and keep it green — the gate for every group above.
- [ ] 5.2 Run the LAN checklist: browse MiniDLNA/ReadyMedia and a Samba share (with and without password) from this machine, both through the new async path; verify the documented commands in `docs/design.md` run as written and record the results there.
- [ ] 5.3 On a real webOS 4.0+ TV: discover a server, browse it, list a share, and confirm input stays responsive during the 8 s SSDP and 10 s SMB waits; verify by reading the app log on device and record the outcome in `docs/design.md`.

## Workflow follow-up

- Archive this change after implementation plus the LAN and device checks are complete.
- Playback wiring (`harbor-media` Engine / StarfishSink / HostSink) and the Settings edit/remove flows remain separate changes, shared with `wire-input-and-real-data`.
