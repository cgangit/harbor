# Proposal

## Why

The IPK installed on a real TV renders only a demo skeleton: the main loop boots a hard-coded `Catalog::demo()`, never enumerates a real source, and the SDL event loop discards every event that is not a `KeyDown` with a scancode — so Magic Remote pointer clicks and any keycode-only events are dropped and the UI cannot be operated. Real sources (local mounts, DLNA, SMB) and the settings the user saves exist in the codebase but are never wired into what the user sees.

## What Changes

- **Input**: accept the full event set a webOS TV actually delivers — `KeyDown` with keycode-only events, Magic Remote pointer/click (mouse button) events, and repeat-safe handling — and map them onto the existing `Key` enum so OK/Back/arrows work on the remote and in the host simulator.
- **Real data**: replace the boot-time `Catalog::demo()` with a catalog built from real state — configured `SourceStore` entries and local mount discovery (`harbor_sources::discover()`), refreshed when a source changes or the user returns to Home/Browser, without restarting the app.
- **Shelves**: Home "Continue Watching" comes from the real resume store; "Recently Added" comes from real enumerated entries (including files inside subfolders) instead of hard-coded titles.
- **No playback changes**: the player screen keeps its current HUD behaviour in this change; wiring `harbor-media`'s `Engine`/`StarfishSink` is a separate follow-up change.
- **No network listing in this change**: DLNA/SMB `list()` has no network implementation behind it yet (page cache / in-memory map only), so worker-thread browsing and SSDP discovery move to the follow-up change `network-source-browsing`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `browsing`: "Home and source browser" gains scenarios for shelves built from real source content and for the UI reflecting source add/remove without a restart; "UI language (PlxNative-inspired)" gains an activation scenario for the Magic Remote pointer click alongside its existing remote-input scenario.

## Impact

- `crates/harbor-gfx/src/window.rs` — event filtering (`poll_keys`), pointer/click handling.
- `crates/harbor-platform/src/keys.rs` — key mapping extended for webOS-reported events.
- `crates/harbor-app/src/lib.rs` — catalog construction and refresh in the main loop.
- `crates/harbor-screens/src/catalog.rs`, `nav.rs` — catalog built from `SourceStore` + `harbor_sources` instead of `demo()`.
- Host tests: `make check` must stay green; new unit tests cover key/event mapping and catalog building from fixtures.
- Device verification: remote arrows/OK/Back and pointer click must operate the UI on a webOS 4.0+ TV (manual, documented).
