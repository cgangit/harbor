# Harbor implementation task list

Feature: webos-native-player
Updated: 2026-11-05

## Phase 0 — Native shell

- [x] 0.1 Workspace, MIT LICENSE, .gitignore, pkg/appinfo.json, placeholder icons/splash
- [x] 0.2 `harbor-base`: log, paths, unit tests
- [x] 0.3 `harbor-platform`: remote key map, `VideoSink` trait stub, unit tests
- [x] 0.4 `harbor-gfx`: size/colour tokens, SDL2 + GLES window, clear + present
- [x] 0.5 Remaining crate stubs matching the design graph
- [x] 0.6 `harbor-app` 60 fps loop + `harbor-sim` binary (arrow keys, `HARBOR_SMOKE_FRAMES`)
- [x] 0.7 `src/main.c` / `src/starfish.c` placeholders, Makefile (`sim`, `check`, `ipk`)

Exit: `make check` passes; `make sim` builds `harbor-sim`; `make ipk` exits with a clear NDK message.

## Phase 1 — UI kit, Home, browser, Settings

- [x] 1.1 Theme, Label, Shelf, glass, focus engine
- [x] 1.2 Home / Browser / Settings / empty-state screens with fake entries
- [x] 1.3 Back stack; size-scale unit test

## Phase 2 — Local files + buffer-feed

- [x] 2.1 Local source enumeration
- [x] 2.2 Demux + AU queue + HostSink
- [x] 2.3 Starfish C seam + FakeSink ACB order tests
- [x] 2.4 Player HUD, pause, seek, sandbox panel

## Phase 3 — Resume, tracks, subtitles

- [x] 3.1 Resume store and Continue prompt
- [x] 3.2 Audio/subtitle menus; SRT/VTT overlay

## Phase 4 — DLNA

- [x] 4.1 SSDP + ContentDirectory + DIDL-Lite
- [x] 4.2 HTTP Range MediaHandle; Add Source flow

## Phase 5 — SMB

- [x] 5.1 Source form and credential vault
- [x] 5.2 libsmb2 list + read_at

## Phase 6 — Capability table, DV, webOS 5+

- [x] 6.1 Codec table parse and refuse path
- [x] 6.2 webOS 5+ windowId; i18n en + zh-Hans

## Phase 7 — ARM IPK recipe

- [x] 7.1 `ci/mkipk.py` deterministic ar/tar packager; host tests (`make check-ipk`)
- [x] 7.2 Makefile `setup-env` (webosbrew NDK) + ARM `cargo rustc` + C link + `make ipk`
- [x] 7.3 GitHub Actions `ubuntu-24.04-arm` workflow to produce `pkg/com.harbor.player_*_arm.ipk`

Exit: `make check` stays host-green; `make ipk` on x86_64 Linux prints the NDK gap; ARM builder runs `make setup-env && make ipk`.
