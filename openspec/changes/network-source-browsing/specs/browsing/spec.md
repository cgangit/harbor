# Spec Delta

## ADDED Requirements

### Requirement: Non-blocking source listing

The System SHALL list a selected source's first page without blocking the present loop, showing progress until the entries arrive.

#### Scenario: Loading state while a network source lists

- **WHEN** the user opens a DLNA or SMB source and its first page has not arrived yet
- **THEN** the Browser shows a loading row in place of the library list until the page arrives or the listing fails

#### Scenario: Listing never blocks input

- **WHEN** a source listing is in progress, including against a host that does not answer for the full connect timeout
- **THEN** focus movement, OK, and Back keep responding and the present loop keeps running at 60 fps

#### Scenario: Listing failure is visible

- **WHEN** a source listing fails (authentication failure, unreachable host, UPnP fault)
- **THEN** the Browser shows the failure class and the configured Source list stays intact
