# sources Specification

## Purpose

Defines the three content origins Harbor v1 supports — local filesystem mounts (USB / internal), DLNA/UPnP MediaServers, and SMB/CIFS shares — and how each one is configured, listed, opened for playback, and reported when it fails. Every source ultimately hands the playback engine a `MediaHandle` with random access for seeking.

## Requirements

### Requirement: Local filesystem source

The System SHALL play videos from USB storage and the TV's internal storage so the viewer can watch files already on the device.

#### Scenario: USB volume as a Local source

- **WHEN** a USB mass-storage volume is mounted
- **THEN** the System lists that volume as a Local source whose root is the volume mount point

#### Scenario: Permitted mount paths

- **WHEN** the System enumerates local media
- **THEN** it scans `/tmp/usb/` (or the platform-equivalent USB mount) and `/media/` paths that the application jail permits

#### Scenario: Play a local video file

- **WHEN** the user selects a video file under a Local source
- **THEN** the System opens that file through the playback engine using a `file://` URI or an open file descriptor

#### Scenario: Jail denies a path

- **WHEN** the jail denies a path
- **THEN** the System shows a readable error naming the path class (USB, internal, other) and keeps the rest of the Library usable

#### Scenario: Video candidates during enumeration

- **WHEN** local files are enumerated
- **THEN** the containers `.mkv`, `.mp4`, `.m4v`, `.mov`, `.avi`, `.ts`, `.m2ts`, `.webm` are treated as video candidates

### Requirement: DLNA / UPnP source

The System SHALL browse and play from DLNA MediaServers on the local network so a NAS or PC share can be watched without copying files.

#### Scenario: Discover MediaServers

- **WHEN** the user chooses Add Source → DLNA
- **THEN** the System discovers MediaServers via SSDP (`urn:schemas-upnp-org:device:MediaServer:1`) and lists each server's friendly name

#### Scenario: Browse ContentDirectory

- **WHEN** the user selects a discovered MediaServer
- **THEN** the System browses its ContentDirectory (`Browse` / `BrowseDirectChildren`) as a folder tree

#### Scenario: Play a DLNA item

- **WHEN** the user selects a DLNA video item
- **THEN** the System plays the item's HTTP resource URL through the playback engine

#### Scenario: Discovery finds nothing

- **WHEN** discovery yields zero servers after 8 seconds
- **THEN** the System shows a retry action and a short explanation that the TV and the server must share a LAN

#### Scenario: ContentDirectory or media request fails

- **WHEN** a ContentDirectory request or HTTP media request fails
- **THEN** the System shows the UPnP error code or HTTP status and leaves the Source list intact

### Requirement: SMB / CIFS source

The System SHALL play a NAS library that is not published over DLNA by adding a Samba share with a host, share name, and optional credentials.

#### Scenario: Add SMB source form

- **WHEN** the user chooses Add Source → SMB
- **THEN** the System accepts host (or IP), share name, optional username, optional password, and optional domain

#### Scenario: Persist and mount a valid SMB source

- **WHEN** the user confirms a valid SMB source
- **THEN** the System persists the source (password stored in the webOS secure store when available, otherwise in an application-private file with mode 0600) and mounts or streams that share as a folder tree

#### Scenario: Play a file from an SMB source

- **WHEN** the user selects a video file on an SMB source
- **THEN** the System plays it through the playback engine using an SMB-backed reader (libsmb2 or equivalent) that supports HTTP-Range-like random access for seeking

#### Scenario: Authentication fails

- **WHEN** SMB authentication fails
- **THEN** the System shows an authentication-failed state and offers to edit credentials

#### Scenario: Host unreachable

- **WHEN** the SMB host is unreachable
- **THEN** the System shows a network-unreachable state after a 10-second connect timeout
