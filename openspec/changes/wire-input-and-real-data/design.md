# Design

## Context

See proposal.md for motivation. The facts that shape this design:

- `harbor_app::run()` builds `Catalog::demo()` once at boot and passes it to `harbor_screens::handle` / `draw_lang` for the life of the process. `View` already owns `SourceStore`, `CredentialVault`, `Settings`, and `ResumeStore`, but none of them feed the catalog.
- `harbor_gfx::Window::poll_keys` accepts only `Event::KeyDown { scancode: Some(..) }`; everything else (mouse buttons, keycode-only key events, quit variants) is dropped. `harbor_platform::from_sdl_scancode(i32)` maps raw scancodes without depending on sdl2 — that indirection must be preserved (harbor-platform has no sdl2 dependency).
- `harbor_sources` provides `discover() -> Vec<LocalSource>` and the `Source::list(path) -> Result<Page, SourceError>` trait with `Entry { id, parent, name, kind, size, mtime, url }`. It is currently referenced only from tests.
- `ResumeStore` exposes `get`/`put`/`clear` but no iteration, so "Continue Watching" cannot be built from it yet.
- Host loop is `make sim` / `make check`; device build is `make ipk` on an ARM builder. Nothing in this change needs the NDK to develop.

## Goals / Non-Goals

**Goals:**

- Every `Key` the UI handles can be produced on a real webOS remote (arrows, OK, Back) and by mouse click in the host simulator.
- Home/Browser content is derived at runtime from `SourceStore` + local mount discovery + the resume store; demo titles never reach a running app.
- Source add/remove is reflected without restarting.
- `make check` stays green; new behaviour is covered by host tests.

**Non-Goals:**

- Playback wiring (`harbor-media` Engine / StarfishSink / HostSink) — separate change.
- Hover-to-focus from Magic Remote pointer motion (spec only requires that hover is not *required*).
- Thumbnail generation, A–Z index build cost tuning, paging of large directories.
- DLNA/SMB background browsing of *nested* folders beyond the first level (first `list()` on source open is enough for this change).

## Decisions

**D1 — Input: one mapping layer, three event shapes.**
`Window::poll_keys` normalizes three event shapes into `Key`: (a) `KeyDown` with scancode (existing path), (b) `KeyDown` with only `keycode` — mapped through a new `from_sdl_keycode(i32)` in `harbor-platform`, mirroring `from_sdl_scancode` and keeping sdl2 out of that crate, (c) `MouseButtonDown` with the left button → `Key::Ok`, so a Magic Remote click activates the focused item exactly like OK (spec scenario "Pointer click activates like OK"). Repeats stay ignored; `Scancode::Q` keeps its host-quit meaning.
*Alternative considered:* handle raw events in `harbor-app` instead — rejected, because it would leak sdl2 event types above the gfx layer and duplicate the existing scancode indirection.

**D2 — Catalog becomes derived state with an explicit refresh.**
Add `refresh_catalog(&mut View)` in `harbor-screens`: rebuilds `View.catalog` from `View.sources` (configured sources → names), `harbor_sources::discover()` (local mounts, jail-safe roots), and `View.store` (Continue Watching). The app loop calls it at boot and whenever `View` sets a `needs_refresh` flag — set whenever `SourceStore` mutates (today: `handle_add`; the flag is the single choke point, so a future remove/edit flow gets refresh for free). Navigation into Home/Browser also triggers it so enumeration staleness is bounded to a screen transition.
*Alternative considered:* rebuild every frame — rejected (filesystem enumeration at 60 fps); rebuild inside `handle()` — rejected (it takes `&View` by design and would do I/O inside a pure reducer, breaking its unit tests' assumptions).

**D3 — Network sources list lazily, off the main thread. (Deferred during implementation — see note.)**
Local enumeration is synchronous (fast, no network). DLNA/SMB `list()` was to run on a `std::thread` with the result delivered back through a channel drained once per frame; until the first page arrives the Browser shows a loading row. The 60 fps present loop must never block on SMB/DLNA.
*Implementation note:* the premise that `Source::list` performs network I/O turned out to be false — `DlnaSource::list` only reads a page cache (`browse_soap` builds a SOAP body but nothing POSTs it), `SmbSource::list` only reads an in-memory map, and SSDP discovery has no socket code at all. Landing the worker on top of that would ship a loading row that resolves to an empty list. The mechanism is therefore deferred together with the network stack to the follow-up change `network-source-browsing`; this change ships synchronous local discovery only, which is what `refresh_catalog` does.
*Alternative considered:* implementing only the worker here (tasks 4.1/4.2 as originally written) — rejected: it would verify against fixtures while being a no-op on a real TV.
*Alternative considered:* synchronous `list()` on source open — rejected: SMB connect alone allows a 10 s timeout, which would freeze input and violate the 60 fps loop.

**D4 — Demo data becomes test-only.**
`Catalog::demo()` is renamed to `Catalog::fixture()` and moved behind `#[cfg(test)]` (or kept as a documented test helper). Production code has no path that can construct it. Existing tests keep compiling with a one-line rename.

**D5 — Resume store gains iteration.**
Add `ResumeStore::iter()` (and a `list()` convenience) over its in-memory map / log entries, returning `Vec<Resume>` sorted by `updated_unix` desc, for the Continue Watching shelf. Existing log format is append-only; iteration replays it.

**D6 — Drop undecodable event types at the queue (added during implementation).**
The host's `libSDL2-2.0.so.0` is sdl2-compat (SDL2 API over SDL3) and leaks SDL3-native window event numbers (`0x202..0x20F`; `0x207` = PIXEL_SIZE_CHANGED observed at boot). sdl2-rs 0.37's `EventType::try_from` transmutes the raw type and aborts on an invalid discriminant, so `make smoke` crashed on HEAD before this change. `Window::open` installs an `SDL_SetEventFilter` callback that admits only the SDL2 event types sdl2-rs can decode (`is_decodable_event_type`, unit-tested) and drops the rest — fail-closed, since nothing we consume lives outside the allowlist. This also makes the raw-event diagnostics (D1) see only input-relevant events.

## Risks / Trade-offs

- [Unknown webOS SDL event shapes — the actual keycode/scancode values the remote reports are unverified] → the mapping layer logs the first N raw events (class + code) to the app log on device; adding a mapping is then a one-line change in `harbor-platform`. This is the single highest-risk unknown and is why device verification is an explicit task.
- [Local discovery may be slow or empty under the app jail] → discovery runs once per refresh, results are cached in `View`, and an empty result degrades to the existing empty-state panel rather than an error.
- [Async `list()` introduces cross-thread state] → deferred with D3 to `network-source-browsing`; when it lands, only owned `Page` results cross the thread boundary and the worker never touches `View` or GL, matching the `Engine`/`Shared` discipline in `harbor-media`.
- [Refresh on every transition could make Back feel laggy if a mount is slow] → refresh is bounded to local mounts; network listings are not part of this change (D3 deferred).
- [Removing `demo()` from production may expose screens that silently assumed demo rows] → `rows()`/focus tests already cover `Catalog::empty()`; a smoke test in the sim paints all screens from a fixture-free catalog.

## Migration Plan

1. Land D1 (input) first — independently testable on host with keyboard + synthetic mouse events, immediately useful on device.
2. Land D4 + D5 (test-only fixture, resume iteration), then D2 (refresh) with unit tests over fixtures.
3. Land D6 (queue event filter) — it unblocks `make smoke`, which had been crashing on HEAD.
4. D3 (async network listing) moves to the follow-up change `network-source-browsing`.
5. Host green (`make check`), then build the IPK and verify on a real TV; rollback is a revert of the change — no data-format migrations are involved (`sources.json`, resume log, and vault formats are unchanged).

## Open Questions

- Exact scancode/keycode values the webOS remote reports through the NDK's SDL are unknown until the device run; D1's event logging is designed to answer this without another code change. Does not affect specs or task breakdown.
