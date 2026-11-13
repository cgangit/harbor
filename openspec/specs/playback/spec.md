# playback Specification

## Purpose

Drives video through the television's own decoder: the StarfishMediaAPIs buffer-feed pipeline, ACB (webOS 4.x) or the exported-window path (webOS 5+) for picture placement, the transport HUD composited over the hardware plane, and seeking/buffering behaviour. The System never transcodes — it plays what the TV can decode or refuses with a visible reason.

## Requirements

### Requirement: Hardware playback pipeline

The System SHALL play video through StarfishMediaAPIs (`libplayerAPIs`) in-process, defaulting to the buffer-feed path: demux on CPU, feed access units to the hardware decoder.

#### Scenario: Buffer-feed default

- **WHEN** the System plays a file
- **THEN** it demuxes on CPU and feeds access units to StarfishMediaAPIs (`libplayerAPIs`) in-process

#### Scenario: Picture placement on webOS 4.x

- **WHEN** the System runs on webOS 4.x
- **THEN** it binds the hardware video plane with ACB (`appId` + `pipelineId`, `setDisplayWindow`, `setMediaVideoData`, `setState`) in the order: bind → wait until two frames have presented → send sourceInfo verbatim → set display window → set state PLAYING

#### Scenario: Picture placement on webOS 5+

- **WHEN** the System runs on webOS 5 or newer
- **THEN** it binds the picture with an SDL exported window id passed in the load `option.windowId` (or the documented 5+ equivalent)

#### Scenario: Consult the codec capability table

- **WHEN** `/etc/umediaserver/device_codec_capability_config.json` exists
- **THEN** the System consults it and offers Direct Play only for codecs the table lists

#### Scenario: Direct Play H.264 and HEVC

- **WHEN** the selected file's video codec is H.264 or HEVC (8-bit or 10-bit) and the capability table allows it
- **THEN** the System Direct Plays

#### Scenario: Direct Play Dolby Vision

- **WHEN** the file includes Dolby Vision profile 5 or 8 and the capability table lists DV
- **THEN** the System Direct Plays that stream

#### Scenario: Capability table absent

- **WHEN** the capability table is absent
- **THEN** the System attempts Direct Play of H.264 and 8-bit HEVC and surfaces a decode-failed error for anything else

### Requirement: Player transport and overlays

The System SHALL expose pause, seek, audio, and subtitles on a HUD drawn over the hardware video so the viewer can control playback with the remote.

#### Scenario: HUD auto-hide

- **WHEN** playback starts
- **THEN** the System shows a transport HUD for 4 seconds, then hides it until the user presses a key

#### Scenario: OK toggles pause

- **WHEN** the user presses OK during playback
- **THEN** the System toggles pause and shows the HUD

#### Scenario: Left/Right seek

- **WHEN** the user presses Left or Right during playback
- **THEN** the System seeks by 10 seconds (Left backward, Right forward) and shows the HUD with a scrub bar

#### Scenario: Hold to scrub

- **WHEN** the user holds Left or Right
- **THEN** the System enters scrub mode, and releasing OK seeks to the scrub position

#### Scenario: Track selection

- **WHEN** the user opens the overlay menus
- **THEN** the System lists audio tracks and subtitle tracks and applies the user's selection to the current session

#### Scenario: Subtitle rendering

- **WHEN** playback has text subtitles (SRT, VTT, ASS/SSA)
- **THEN** the System renders them on the GL overlay, and renders image subtitles (PGS) when the demuxer exposes them

#### Scenario: End of file

- **WHEN** playback reaches the end of the file
- **THEN** the System stops, unloads the decoder, and returns to the previous browse screen

#### Scenario: HUD composites over the video plane

- **WHEN** the HUD is drawn
- **THEN** it composites over the hardware video plane and leaves the video pixels to the decoder

### Requirement: Seeking and buffering

The System SHALL land seeks near the requested time so the viewer can skip intros and recaps.

#### Scenario: Seek on the buffer-feed path

- **WHEN** the user seeks on the buffer-feed path
- **THEN** the System flushes the decoder, reopens the reader at the nearest keyframe (cue/index, else byte-interpolated), rebases timestamps, and resumes feeding

#### Scenario: Decoder reports BufferFull

- **WHEN** the decoder reports BufferFull
- **THEN** the System holds the pending access unit and retries on the next pump tick

#### Scenario: In-place seek when available

- **WHEN** Starfish `Seek` is available and succeeds on the current firmware
- **THEN** the System uses it for in-place seeks; **IF** that call fails, the System restarts the stream at the new position

#### Scenario: Position and duration display

- **WHEN** the demuxer has published duration
- **THEN** the System displays playback position and duration
