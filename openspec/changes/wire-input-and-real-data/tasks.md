# Tasks

## 1. Input mapping (design D1)

- [x] 1.1 Add `from_sdl_keycode(i32) -> Key` to `harbor-platform/src/keys.rs` (Return→Ok, Escape/Backspace→Back, arrows→directions, media keys) with unit tests; verify with `cargo test -p harbor-platform`.
- [x] 1.2 Extract a pure event→`Key` mapping helper in `harbor-gfx/src/window.rs` covering the three shapes (scancode keydown, keycode-only keydown, left `MouseButtonDown`→`Key::Ok`), keep `poll_keys` as a thin loop over it; verify with unit tests in `harbor-gfx` (`cargo test -p harbor-gfx`).
- [x] 1.3 Add raw-event diagnostics: log the first 20 event class + code pairs to the app log on boot so unknown webOS remote codes are visible on device; verify with a unit test on the logging helper (`cargo test -p harbor-gfx`).
- [ ] 1.4 Confirm in the host sim that keyboard arrows/Enter/Escape and a synthetic mouse click all drive navigation; verify by running `make smoke` plus a manual `make sim` session. (smoke passes, exit 0; the manual session needs the user — no X input-injection tool exists on this host)
- [x] 1.5 Install a queue-level SDL event filter that drops event types sdl2-rs 0.37 cannot decode (host `libSDL2` is sdl2-compat over SDL3 and leaks e.g. `0x207`), which crashed `make smoke` on HEAD before this change; verify with `cargo test -p harbor-gfx` (allowlist tests) and `make smoke` exiting 0.

## 2. Test-only fixtures and resume iteration (design D4, D5)

- [x] 2.1 Rename `Catalog::demo()` to `Catalog::fixture()` and gate it to tests; verify `make check` passes and `grep -rn "demo()" crates/` shows no non-test call sites.
- [x] 2.2 Add `ResumeStore::iter()` returning resumes sorted by `updated_unix` descending, with round-trip and ordering tests; verify with `cargo test -p harbor-session`.

## 3. Catalog from real data (design D2)

- [x] 3.1 Add `needs_refresh: bool` to `View` and `refresh_catalog(&mut View)` in `harbor-screens` that rebuilds `sources` from `SourceStore` and `continue_watching` from `ResumeStore`; set the flag in `handle_add` after a source is saved; verify with unit tests over in-memory stores (`cargo test -p harbor-screens`).
- [x] 3.2 Make `refresh_catalog` enumerate real local sources via `harbor_sources::discover()` (honouring `HARBOR_MEDIA` on the host) into `recently_added`, degrading to the empty state when nothing is found; verify with a test using a fixture directory (`cargo test -p harbor-screens`). (implemented as a bounded BFS into subfolders so `Movies/*.mkv` counts as Recently Added)
- [x] 3.3 Clear `needs_refresh` and re-run `refresh_catalog` on transitions into Home/Browser so staleness is bounded to a screen change; verify with a nav test that adds a source and sees the new name on the next Home draw.
- [x] 3.4 Wire the app loop: call `refresh_catalog` at boot and when the flag is set, drop `Catalog::demo()` from `harbor-app`; verify with `HARBOR_MEDIA=<fixture dir> HARBOR_SMOKE_FRAMES=3 make smoke` and confirm the boot log reports a non-zero discovered-entry count. (boot log: `catalog[boot] sources=1 files=2 recent=2 resume=0`)

## 4. Lazy network listing (design D3) — deferred

Moved out of this change: design D3's premise (`Source::list` does network I/O) was falsified during implementation — DLNA/SMB listing and SSDP discovery have no socket code yet. See "Workflow follow-up"; the design note in design.md records the finding.

## 5. Integration and device verification

- [x] 5.1 Run `make check` (workspace tests + `ci/test_mkipk.py`) and keep it green — this is the gate for every group above.
- [x] 5.2 Update `docs/design.md` with the refresh/async-input additions and verify the documented commands run as written. (documented `make check`, `make smoke`, `HARBOR_MEDIA=… make smoke`, `make sim`; first three re-run green with the documented output)
- [ ] 5.3 On a real webOS 4.0+ TV: confirm arrows/OK/Back operate the UI, pointer click activates the focused item, Home lists real local files (no demo titles), and the raw-event log captures the remote's codes; record the observed codes in `docs/design.md`.

## Workflow follow-up

- Deferred tasks 4.1/4.2 (worker-thread `list()` path + non-blocking proof) belong to the new change `network-source-browsing`, together with what implementation exposed: real DLNA browse (HTTP SOAP POST), SSDP discovery over UDP, an SMB protocol client, and the loading row in the Browser.
- Open a separate change for playback wiring (`harbor-media` Engine / StarfishSink / HostSink) — see proposal.md "What Changes".
- Open a separate change for the missing Settings flows (source edit/remove, test-connection), which `settings` already requires but this change does not implement.
- Archive this change after implementation and device verification are complete.
