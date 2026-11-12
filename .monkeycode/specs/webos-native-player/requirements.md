# Requirements Document

## Introduction

Harbor is a native (non-WebView) media player for LG webOS TVs. Working application id: `com.harbor.player`. The application SHALL run as a `type: native` IPK, draw its own GPU UI at 1920x1080 / 60 fps, and hand video to the television's hardware decoder through StarfishMediaAPIs and ACB (webOS 4.x) or the exported-window video path (webOS 5+). Minimum supported platform is webOS 4.0. Content sources are local storage (USB / internal), DLNA/UPnP, and SMB/CIFS. The product is a general-purpose player, independent of Plex.

The player architecture, UI language, and playback pipeline are informed by [PlxNative](https://github.com/GLinnik21/plx-native). This specification describes an independent implementation. The System SHALL contain no PlxNative source, assets, or trademarks. Implementation language is Rust (application crates) plus a thin C seam for StarfishMediaAPIs and ACB.

## Glossary

- **System**: The Harbor application (`com.harbor.player`) packaged as an IPK.
- **Native app**: A webOS application with `appinfo.json` `"type": "native"`. The process is a 32-bit ARM ELF. The UI is drawn with SDL2 + OpenGL ES. There is no Chromium, JavaScript, or WebView in the playback or browse path.
- **StarfishMediaAPIs**: LG's in-process media API (`libplayerAPIs`). The System loads, feeds, pauses, seeks, and unloads through this API.
- **ACB**: Adaptive Content Binder (`libAcb` / `libAcbAPI` → `com.webos.service.acb`). On webOS 4.x this is the only app-facing way to place the hardware video plane. On webOS 5+ the System uses an SDL exported window (`windowId`) instead.
- **Buffer-feed path**: FFmpeg demuxes the container; the System feeds elementary access units into StarfishMediaAPIs. This is the default playback path, matching PlxNative.
- **URI path**: The System hands a URL to StarfishMediaAPIs / `com.webos.media` and lets the firmware fetch, demux, and decode. Used as a fallback when the buffer-feed path cannot handle a format.
- **Source**: A configured origin of media: a local filesystem mount, a DLNA/UPnP MediaServer, or an SMB/CIFS share.
- **Library**: The browsable catalogue the System builds from one or more Sources.
- **Direct play**: Playback that uses the television's hardware decoder without transcoding. The System never transcodes; unsupported streams are refused with a visible reason.
- **webOS 4.x**: Platform release 4.0 through 4.x inclusive. The 4.x media path has no `windowId`; picture placement is ACB + SoC plane (`dvovideosink` / `mvsink`).
- **Host simulator**: A desktop build (Linux/macOS) that runs the UI and a software video sink. It does not prove Starfish, ACB, text rasterization, or frame rate.

## Requirements

### Requirement 1: Native runtime and packaging

**User Story:** AS a webOS TV owner, I want a native application I can install as an IPK, so that playback and browsing run without Chromium.

#### Acceptance Criteria

1. The System SHALL package as a webOS IPK whose `appinfo.json` sets `"type"` to `"native"`, `"resolution"` to `"1920x1080"`, and `"main"` to the native binary name.
2. The System SHALL target 32-bit ARM (`armv7-unknown-linux-gnueabi`) and SHALL run on webOS 4.0 and newer.
3. WHEN the user launches the application from the webOS launcher, the System SHALL create an SDL2 + OpenGL ES window, enter a 60 fps present loop, and accept Magic Remote / IR remote key events.
4. The System SHALL draw every browse, detail, settings, and player overlay surface with GPU primitives. The System SHALL keep Chromium, a WebView, and a JavaScript runtime out of those surfaces.
5. The System SHALL ship a host simulator binary that runs the same UI code path on Linux (X11/Wayland) for development.

### Requirement 2: Home and source browser

**User Story:** AS a viewer, I want a remote-first home screen that lists my sources and recent files, so that I can start watching from the couch.

#### Acceptance Criteria

1. WHEN the System starts with at least one configured Source, the System SHALL show a Home screen with a hero area and horizontal shelves for Continue Watching and Recently Added.
2. WHEN the System starts with zero configured Sources, the System SHALL show an empty-state panel whose primary action opens Add Source.
3. WHILE a shelf or grid has focus, the System SHALL move focus with Left/Right/Up/Down, activate the focused item with OK, and return to the previous screen with Back.
4. WHEN the user opens a Source, the System SHALL list that Source's folders and media files in a poster-or-list browser with A–Z jump, sort (name, date, size), and filter (video / audio / all).
5. The System SHALL size ordinary content text at 24 px or larger (couch caption floor) on the 1920x1080 surface.

### Requirement 3: Local filesystem source

**User Story:** AS a viewer, I want to play videos from USB storage and the TV's internal storage, so that I can watch files I already have on the device.

#### Acceptance Criteria

1. WHEN a USB mass-storage volume is mounted, the System SHALL list that volume as a Local source whose root is the volume mount point.
2. The System SHALL enumerate `/tmp/usb/` (or the platform-equivalent USB mount) and `/media/` paths that the application jail permits.
3. WHEN the user selects a video file under a Local source, the System SHALL open that file through the playback engine using a `file://` URI or an open file descriptor.
4. IF the jail denies a path, the System SHALL show a readable error naming the path class (USB, internal, other) and SHALL keep the rest of the Library usable.
5. The System SHALL treat containers `.mkv`, `.mp4`, `.m4v`, `.mov`, `.avi`, `.ts`, `.m2ts`, `.webm` as video candidates during enumeration.

### Requirement 4: DLNA / UPnP source

**User Story:** AS a viewer, I want to browse and play from DLNA MediaServers on my LAN, so that I can watch a NAS or PC share without copying files.

#### Acceptance Criteria

1. WHEN the user chooses Add Source → DLNA, the System SHALL discover MediaServers on the local network via SSDP (`urn:schemas-upnp-org:device:MediaServer:1`) and list each server's friendly name.
2. WHEN the user selects a discovered MediaServer, the System SHALL browse its ContentDirectory (`Browse` / `BrowseDirectChildren`) as a folder tree.
3. WHEN the user selects a DLNA video item, the System SHALL play the item's HTTP resource URL through the playback engine.
4. IF discovery yields zero servers after 8 seconds, the System SHALL show a retry action and a short explanation that the TV and the server must share a LAN.
5. IF a ContentDirectory request or HTTP media request fails, the System SHALL show the UPnP error code or HTTP status and SHALL leave the Source list intact.

### Requirement 5: SMB / CIFS source

**User Story:** AS a viewer, I want to add a Samba share with a host, share name, and optional credentials, so that I can play a NAS library that is not published over DLNA.

#### Acceptance Criteria

1. WHEN the user chooses Add Source → SMB, the System SHALL accept host (or IP), share name, optional username, optional password, and optional domain.
2. WHEN the user confirms a valid SMB source, the System SHALL persist the source (password stored in the webOS secure store when available, otherwise in an application-private file with mode 0600) and SHALL mount or stream that share as a folder tree.
3. WHEN the user selects a video file on an SMB source, the System SHALL play it through the playback engine using an SMB-backed reader (libsmb2 or equivalent) that supports HTTP-Range-like random access for seeking.
4. IF authentication fails, the System SHALL show an authentication-failed state and SHALL offer to edit credentials.
5. IF the host is unreachable, the System SHALL show a network-unreachable state after a 10-second connect timeout.

### Requirement 6: Hardware playback pipeline

**User Story:** AS a viewer, I want the television's own decoder to play the file, so that 4K HEVC and similar formats play at full quality.

#### Acceptance Criteria

1. The System SHALL play video through StarfishMediaAPIs (`libplayerAPIs`) in-process. Default topology SHALL be the buffer-feed path: demux on CPU, feed access units to the hardware decoder.
2. WHILE running on webOS 4.x, the System SHALL bind the hardware video plane with ACB (`appId` + `pipelineId`, `setDisplayWindow`, `setMediaVideoData`, `setState`) using the order: bind → wait until two frames have presented → send sourceInfo verbatim → set display window → set state PLAYING.
3. WHILE running on webOS 5 or newer, the System SHALL bind the picture with an SDL exported window id passed in the load `option.windowId` (or the documented 5+ equivalent).
4. The System SHALL consult `/etc/umediaserver/device_codec_capability_config.json` when that file exists, and SHALL offer Direct Play only for codecs the table lists.
5. WHEN the selected file's video codec is H.264 or HEVC (8-bit or 10-bit) and the capability table allows it, the System SHALL Direct Play.
6. WHEN the file includes Dolby Vision profile 5 or 8 and the capability table lists DV, the System SHALL Direct Play that stream.
7. IF the capability table is absent, the System SHALL attempt Direct Play of H.264 and 8-bit HEVC and SHALL surface a decode-failed error for anything else.

### Requirement 7: Player transport and overlays

**User Story:** AS a viewer, I want pause, seek, audio, and subtitles on a HUD drawn over the hardware video, so that I can control playback with the remote.

#### Acceptance Criteria

1. WHEN playback starts, the System SHALL show a transport HUD for 4 seconds, then hide it until the user presses a key.
2. WHEN the user presses OK during playback, the System SHALL toggle pause and show the HUD.
3. WHEN the user presses Left or Right during playback, the System SHALL seek by 10 seconds (Left backward, Right forward) and SHALL show the HUD with a scrub bar.
4. WHEN the user holds Left or Right, the System SHALL enter scrub mode; releasing OK SHALL seek to the scrub position.
5. The System SHALL list audio tracks and subtitle tracks in overlay menus, and SHALL apply the user's selection to the current session.
6. The System SHALL render text subtitles (SRT, VTT, ASS/SSA) on the GL overlay. Image subtitles (PGS) SHALL be rendered when the demuxer exposes them.
7. WHEN playback reaches the end of the file, the System SHALL stop, unload the decoder, and return to the previous browse screen.
8. The HUD SHALL composite over the hardware video plane. The HUD SHALL leave the video pixels to the decoder.

### Requirement 8: Seeking and buffering

**User Story:** AS a viewer, I want seeks to land near the requested time, so that I can skip intros and recaps.

#### Acceptance Criteria

1. WHEN the user seeks on the buffer-feed path, the System SHALL flush the decoder, reopen the reader at the nearest keyframe (cue/index, else byte-interpolated), rebase timestamps, and resume feeding.
2. WHILE the decoder reports BufferFull, the System SHALL hold the pending access unit and retry on the next pump tick.
3. IF Starfish `Seek` is available and succeeds on the current firmware, the System SHALL use it for in-place seeks; IF that call fails, the System SHALL restart the stream at the new position.
4. The System SHALL display playback position and duration once the demuxer has published duration.

### Requirement 9: Resume and continue watching

**User Story:** AS a viewer, I want the System to remember where I stopped, so that I can continue later.

#### Acceptance Criteria

1. WHILE a file is playing, the System SHALL persist playhead position at least every 10 seconds and on pause, seek, and stop.
2. WHEN the user opens a file whose persisted playhead is between 10 seconds and 90% of duration, the System SHALL offer Continue and Start over.
3. WHEN the user chooses Continue, the System SHALL start playback at the persisted playhead.
4. WHEN playback passes 90% of duration, the System SHALL clear that file's resume point.

### Requirement 10: UI language (PlxNative-inspired)

**User Story:** AS a viewer, I want a 60 fps, focus-first interface with glass panels over artwork, so that browsing feels like a native TV app.

#### Acceptance Criteria

1. The System SHALL draw on a fixed 1920x1080 surface with a 60 fps present loop.
2. The System SHALL use a role-based colour theme (text primary, accent, control fill, scrim) rather than per-screen colour literals.
3. Overlay menus SHALL use a translucent glass panel, a 30% scrim, and a 1 px rim, over a frozen snapshot of the page behind them.
4. Focus SHALL be a first-class state: the focused card scales up, and captions appear in a band under the focused card.
5. Input SHALL come from the webOS remote (keys and Magic Remote pointer). Hover SHALL not be required to discover actions.

### Requirement 11: Settings and source management

**User Story:** AS a viewer, I want to add, edit, and remove sources and set player defaults, so that the Library matches my network.

#### Acceptance Criteria

1. The Settings screen SHALL list configured Sources with type, host or path, and a test-connection action.
2. The user SHALL add, edit, and remove a Source from Settings. Removing a Source SHALL delete its cached metadata and resume points.
3. Settings SHALL expose: default subtitle language, default audio language, subtitle size, seek step (10 / 30 seconds), and UI language (at least English and Simplified Chinese).
4. WHEN the user presses Back on the top-level Settings screen, the System SHALL return to Home.

### Requirement 12: Error handling and sandbox limits

**User Story:** AS a viewer, I want a clear message when the TV cannot play a file or when Developer Mode blocks the decoder, so that I know what to do next.

#### Acceptance Criteria

1. IF StarfishMediaAPIs fails to load or the sandbox withholds the decoder device (known k5lp/k3lp Developer Mode issue), the System SHALL show a native-video-unavailable panel naming the failure class, and SHALL keep browsing available.
2. IF demux or decode fails after play is requested, the System SHALL unload the pipeline, return to the previous screen, and show a toast with the failure class (unsupported codec, network, file missing, decoder).
3. WHEN the application moves to the background, the System SHALL pause playback and SHALL block the screensaver while in the foreground and playing.
4. The System SHALL log playback events (load, playing, seek, error) to an application log file under the app's runtime directory.

### Requirement 13: Host tests and device verification

**User Story:** AS a developer, I want a host unit suite and a documented device loop, so that playback changes can be gated before a television is available.

#### Acceptance Criteria

1. The repository SHALL provide `make check` (or equivalent) that runs host unit tests for demux helpers, SMB URL parsing, DLNA DIDL-Lite parsing, resume store, and UI focus reducers.
2. The host simulator SHALL play a local sample file through a software sink so UI HUD tests can run without a television.
3. Device verification of Starfish, ACB, and frame rate SHALL be documented as requiring a real webOS 4.0+ television; host green SHALL not be claimed as decoder proof.

## Out of scope (v1)

- Plex, Emby, Jellyfin, or any account-based media server protocol.
- Live TV, DVR, music library, photo library.
- Server-side transcoding. The System plays what the TV can decode, or refuses.
- HDR10+ dynamic metadata beyond what Starfish already applies.
- webOS 3.x as a supported target (may work; untested; no v1 commitment).
- LG Content Store publication. Distribution is Developer Mode / Homebrew Channel IPK.

## Implementation phases (summary)

Phases are expanded in `design.md`. Requirement mapping:

| Phase | Name | Requirements |
|---|---|---|
| 0 | Toolchain, native shell, IPK, host simulator | 1, 13 |
| 1 | UI kit, Home, file browser, Settings shell | 2, 10, 11 |
| 2 | Starfish buffer-feed + ACB player, local files | 3, 6, 7, 8, 12 |
| 3 | Resume, tracks, subtitles | 7, 9 |
| 4 | DLNA discovery and browse/play | 4 |
| 5 | SMB source and random-access reader | 5 |
| 6 | Codec table, DV, webOS 5+ window path, polish | 6, 12 |
