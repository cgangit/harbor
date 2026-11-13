# resume Specification

## Purpose

Remembers where the viewer stopped so playback can continue later, without resurrecting files that were watched to the end.

## Requirements

### Requirement: Resume and continue watching

The System SHALL persist playhead positions and offer to continue from them when the same file is opened again.

#### Scenario: Persist the playhead

- **WHEN** a file is playing
- **THEN** the System persists the playhead position at least every 10 seconds and on pause, seek, and stop

#### Scenario: Offer Continue or Start over

- **WHEN** the user opens a file whose persisted playhead is between 10 seconds and 90% of duration
- **THEN** the System offers Continue and Start over

#### Scenario: Continue from the persisted playhead

- **WHEN** the user chooses Continue
- **THEN** the System starts playback at the persisted playhead

#### Scenario: Clear the resume point near the end

- **WHEN** playback passes 90% of duration
- **THEN** the System clears that file's resume point
