# errors Specification

## Purpose

Keeps failure legible and recoverable: when the TV cannot play a file, when Developer Mode or the sandbox blocks the decoder, or when the app moves to the background, the viewer gets a clear message and browsing stays available.

## Requirements

### Requirement: Error handling and sandbox limits

The System SHALL surface playback failures as named failure classes and SHALL keep browsing usable after any playback failure.

#### Scenario: Decoder unavailable / sandbox block

- **WHEN** StarfishMediaAPIs fails to load or the sandbox withholds the decoder device (known k5lp/k3lp Developer Mode issue)
- **THEN** the System shows a native-video-unavailable panel naming the failure class and keeps browsing available

#### Scenario: Demux or decode fails after play is requested

- **WHEN** demux or decode fails after play is requested
- **THEN** the System unloads the pipeline, returns to the previous screen, and shows a toast with the failure class (unsupported codec, network, file missing, decoder)

#### Scenario: Background and screensaver

- **WHEN** the application moves to the background
- **THEN** the System pauses playback; while in the foreground and playing, it blocks the screensaver

#### Scenario: Playback event log

- **WHEN** playback events occur (load, playing, seek, error)
- **THEN** the System logs them to an application log file under the app's runtime directory
