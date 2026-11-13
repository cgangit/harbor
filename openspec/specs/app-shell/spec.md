# app-shell Specification

## Purpose

Runs Harbor (`com.harbor.player`) as a native, non-WebView webOS application: the ARM IPK packaging, the SDL2 + OpenGL ES 1920x1080 / 60 fps UI shell with remote input, the host simulator used for daily development, and the host/device verification loop that gates playback changes. v1 excludes account-based media servers, live TV, transcoding, and LG Content Store publication.

## Requirements

### Requirement: Native runtime and packaging

The System SHALL package as a webOS IPK with `"type": "native"` and render every surface with GPU primitives, with no Chromium, WebView, or JavaScript runtime in the browse or playback path.

#### Scenario: IPK metadata

- **WHEN** the System is packaged as a webOS IPK
- **THEN** `appinfo.json` sets `"type"` to `"native"`, `"resolution"` to `"1920x1080"`, and `"main"` to the native binary name

#### Scenario: Target platform

- **WHEN** the IPK is built
- **THEN** it targets 32-bit ARM (`armv7-unknown-linux-gnueabi`) and runs on webOS 4.0 and newer

#### Scenario: Launch from the webOS launcher

- **WHEN** the user launches the application from the webOS launcher
- **THEN** the System creates an SDL2 + OpenGL ES window, enters a 60 fps present loop, and accepts Magic Remote / IR remote key events

#### Scenario: GPU-only surfaces

- **WHEN** the System draws any browse, detail, settings, or player overlay surface
- **THEN** it draws that surface with GPU primitives and keeps Chromium, a WebView, and a JavaScript runtime out of it

#### Scenario: Host simulator binary

- **WHEN** the System is built for the host
- **THEN** it ships a host simulator binary that runs the same UI code path on Linux (X11/Wayland) for development

### Requirement: Host tests and device verification

The System SHALL provide a host test suite and a documented device verification loop so playback changes can be gated before a television is available.

#### Scenario: Host unit suite

- **WHEN** a developer runs `make check` (or equivalent)
- **THEN** it runs host unit tests for demux helpers, SMB URL parsing, DLNA DIDL-Lite parsing, the resume store, and UI focus reducers

#### Scenario: Simulator playback without a television

- **WHEN** the host simulator runs
- **THEN** it plays a local sample file through a software sink so HUD tests can run without a television

#### Scenario: Host green is not decoder proof

- **WHEN** claims are made about Starfish, ACB, or frame rate
- **THEN** they are documented as requiring a real webOS 4.0+ television, and a green host suite is not reported as decoder proof
