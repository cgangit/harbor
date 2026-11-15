# Design

## Context

`Surface for Frame::text` in `harbor-gfx` currently fills a solid rectangle — there is no font, glyph, or texture code anywhere in the workspace, so every screen renders as placeholder bars (the "skeleton" observed on device and in the host simulator alike). The interface above it is sound: screens emit `fill`/`text` ops through `harbor_ui::Surface`, `Frame` is the only real implementation, and the SDL canvas already has alpha blending enabled. The change therefore replaces exactly one function body and everything upstream stays untouched. Motivation is in proposal.md — Why.

Constraints shaping the approach:

- webOS device: no system font API allowed in the browse path (app-shell: GPU-only surfaces), 32-bit ARM, IPK already ~900 KB after the `smb2` addition — the font must not add megabytes.
- The i18n dictionary (English + Simplified Chinese) is fixed in `crates/harbor-platform/src/i18n.rs`; the UI also draws literals (e.g. `A–Z`) and, later, network-sourced names of arbitrary content.
- Toolchain reality on this host: no system pip/fonttools initially (fonttools installed user-local for this work), no `hb-subset`, and the typst `subsetter` crate is unusable because it strips the `cmap` table (PDF-embedding only).

## Goals / Non-Goals

**Goals:**

- Real glyph text on every surface through the existing `Surface::text` contract, with zero per-screen changes.
- Pure-Rust rendering path at runtime (rasterizer is a Rust crate embedded in the binary); no SDL_ttf, no OS font APIs, no extra native libraries in the IPK.
- Coverage of the whole i18n dictionary in both languages, with visible, non-crashing degradation for everything else.
- Deterministic, re-runnable font subsetting so the dictionary can grow without hand work.

**Non-Goals:**

- Text shaping: no kerning pairs, ligatures, or complex-script layout — labels are short and mostly Latin/CJK; legibility at ≥24 px does not depend on them.
- Multi-line layout, wrapping, ellipsis truncation — every current call site is single-line.
- Full CJK coverage for arbitrary network-sourced names (server, share, file names): out-of-subset characters get the replacement glyph; widening coverage is a re-run of the subset script, not a design change.
- Cursor drawing for the Magic Remote pointer, text effects (stroke/shadow), dynamic font scaling beyond the existing role sizes.

## Decisions

**D1 — Rasterizer: `fontdue`.**
Pure Rust (MIT OR Apache-2.0 OR Zlib, 0.9.5, maintained through 2026-10), simple per-glyph API: `Font::from_bytes` → `rasterize(ch, px)` returns an alpha mask plus metrics — exactly what a texture-backed label needs. *Alternatives considered:* `ab_glyph` (equally pure Rust; rejected on no material advantage and a slightly less direct raster API); SDL_ttf (rejected: ships a new native `.so` in the IPK, may be absent from the webOS NDK sysroot, and breaks the minimal-native-surface stance of the previous change's D1/D2); OS font APIs (rejected: platform-specific and outside the GPU-only browse path). Shaping is explicitly a non-goal, so a shaping-capable stack buys nothing here.

**D2 — One font: Noto Sans SC (OFL), subset to the drawn-character set, embedded with `include_bytes!`.**
A single Simplified-Chinese font covers Latin too, so there is one raster path and no fallback stack. The source (~10 MB language-level OTF, reachable from `notofonts/noto-cjk`, with the jsDelivr CDN as the primary mirror — raw GitHub resets long transfers) is subset with `pyftsubset` (fonttools 4.68, verified working, handles CFF) to: printable ASCII + every character in the i18n dictionary + UI literals (`–`, `…`, etc. — computed by scanning drawn string literals in the workspace) + common typography including U+25A1. The verified result is 233 characters in **36 KB** — two orders of magnitude under the full font. The subset and `OFL.txt` are checked in under `crates/harbor-gfx/assets/`; the source font is downloaded into a gitignored cache by the regeneration script (`ci/subset_font.py`), never committed. *Alternatives considered:* typst `subsetter` crate (rejected: removes `cmap`, output is PDF-only — verified from its own docs); full unsubset font (rejected: multi-MB IPK for glyphs we never draw); a font file in the IPK payload loaded from disk (rejected: adds `mkipk.py`/staging changes and a missing-file runtime mode for no benefit — embedding makes the font unlosable and keeps the CI path filter on `crates/**` effective).

**D3 — Draw path: per-glyph mask cache + per-string texture cache.**
Two levels: (1) `(char, px)` → rasterized alpha mask, computed once per character/size; (2) `(text, px, color)` → RGBA texture composited from those masks, uploaded once and then `canvas.copy`-ed per frame. The cache lives in `Window` and is handed to `Frame` (the existing `unsafe_textures` feature lets textures outlive the creator). The string cache is bounded (clear-all at a few hundred entries): labels are stable across frames so eviction thrash is unlikely, while the per-second HUD clock can never grow it without bound. Placement uses fontdue's line metrics so text occupies the same box the placeholder bars used (`y` = top of the text box), keeping every existing layout coordinate valid. Alpha blending is already enabled on the canvas, so antialiased edges composite correctly over glass cards. *Alternatives considered:* one GPU glyph atlas with per-glyph blits (rejected for v1: packing and eviction bookkeeping for ~50 stable labels per frame — revisit only if profiling shows copy cost matters); re-rasterizing every label each frame (rejected: 60 fps texture uploads); drawing glyphs as `fill_rect` pixel grids (rejected: thousands of rects per frame).

**D4 — Failure modes degrade, never panic.**
The font bytes are compiled in, so the only realistic load failure is a corrupt build artifact: if fontdue cannot parse it at startup, log to `events.log` and fall back to today's bar rendering — the app boots and the diagnosis is in the log. A character outside the subset renders as **U+25A1 WHITE SQUARE**, the replacement glyph the subset includes (Noto Sans SC itself has no U+FFFD — verified against the source cmap — so the square is the visible "uncovered character" marker), or, if even that is somehow missing, a drawn hollow box at the correct advance width; the rest of the label still draws. Neither path may panic or blank the whole label (spec: "Characters outside the font").

**D5 — Verification at three levels.**
Unit (host, `make check`): rasterizer loads the embedded font; glyph masks are non-empty for `A`/`中`, empty for space; a coverage test fails if any character in the i18n dictionary is missing from the subset (regenerate when it fires); a pure `render_to_buffer` helper asserts real pixel coverage for a known string. Visual (host): `make sim` shows readable text — this also makes the pending task 1.4 session meaningful. Device: CI IPK rebuild, install, and the task 5.3 run finally observes loading/error rows and focus captions on the TV.

## Risks / Trade-offs

- [Subset misses a character a future UI string introduces] → the dictionary-coverage test fails in `make check` before merge; runtime surprises fall back to the replacement glyph.
- [Network-sourced names contain Chinese outside the dictionary subset] → replacement glyph per spec; widening coverage is `pyftsubset` with a larger char set (script is deterministic) — a deliberate size/readability tradeoff, not an outage.
- [fontdue quality/compatibility on the CFF-flavoured Noto subset] → verified in the first implementation task against the actual subset bytes; fallback is `ab_glyph` (same shape of API, contained to one module).
- [Texture cache growth from dynamic strings] → bounded clear-all cache (D3); worst case a full re-upload after overflow, invisible at these label counts.
- [IPK grows by the subset size (≤ a few hundred KB)] → accepted; same order as the earlier `smb2` growth and far below the 16 MB full-font option.

## Migration Plan

Visual-only change with no data or API migration: land behind the normal `make check` → CI IPK → device-install loop. Rollback is a plain revert; nothing else depends on the placeholder behaviour (tests assert on `Recorder` ops, which are unchanged).

## Open Questions

None — rasterizer, font, subset tooling, coverage policy, and cache strategy are all decided above.
