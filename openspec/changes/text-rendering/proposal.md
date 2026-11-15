# Proposal

## Why

Harbor's UI draws every label as a solid rectangle: `Surface for Frame::text` in `harbor-gfx` fills a coloured bar instead of rendering glyphs, so every screen — host simulator and device alike — looks like a skeleton screen with no readable text. The specs already require readable text (browsing: couch caption floor, captions in a band under the focused card; settings: English and Simplified Chinese UI), and without glyphs the device verification loop cannot be observed at all: loading/error rows, focus captions, and navigation feedback are invisible. This was found during the first real-TV run of the `network-source-browsing` IPKs.

## What Changes

- Replace the placeholder `text` draw op in `harbor-gfx` with real glyph rendering: rasterize glyphs with a pure-Rust rasterizer, cache them, and blit them through the existing SDL canvas path — no new native libraries, no SDL_ttf, no OS font APIs (consistent with the minimal-native-surface decisions D1/D2 of the previous change).
- Bundle an open-licensed font subset covering ASCII plus every character in the i18n dictionary (English and Simplified Chinese), embedded in the binary so the IPK needs no runtime font file lookup.
- Preserve the `Surface::text` contract: every screen (Home, Browser, Settings, Player, HUD, Add dialog, discovery/loading/error rows) gains readable text with zero per-screen changes.
- Define visible degradation for characters the font cannot draw: a replacement glyph, never a panic and never a silently blank label.

## Capabilities

### New Capabilities

(none — text rendering belongs to the existing UI-language and packaging capabilities)

### Modified Capabilities

- `browsing`: labels on every surface render as readable glyphs from the application's bundled font at the existing role sizes, covering both UI languages; characters outside the font degrade to a visible replacement glyph.
- `app-shell`: the IPK payload ships the bundled font with the application (embedded in the binary), so rendering text on a device performs no runtime file access.

## Impact

- `crates/harbor-gfx`: `Frame::text` stops filling rectangles and renders glyphs; new glyph-rasterization/caching module; new pure-Rust rasterizer dependency (`fontdue` or equivalent) in the workspace and crate manifests; font asset, its license, and a subset-regeneration script live under the crate (inside `crates/**`, which the ARM CI path filter already watches).
- `crates/harbor-ui` and all screens: no API change — `Surface::text` signature and every `Label` call site stay as-is; Recorder-based tests are unaffected.
- Build/artifacts: IPK grows by the subset font size (hundreds of KB, not megabytes); `mkipk.py` needs no change because the font is compiled in.
- Unblocks: meaningful visual device verification of `network-source-browsing` (task 5.3 — loading and error rows become visible) and the pending `make sim` session (task 1.4).
