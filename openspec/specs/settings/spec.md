# settings Specification

## Purpose

Lets the viewer add, edit, test, and remove Sources and set player defaults so the Library matches their network and language preferences.

## Requirements

### Requirement: Settings and source management

The System SHALL provide a Settings screen that manages configured Sources and exposes player defaults.

#### Scenario: Source list with test action

- **WHEN** the Settings screen opens
- **THEN** it lists configured Sources with type, host or path, and a test-connection action

#### Scenario: Add, edit, and remove a Source

- **WHEN** the user adds, edits, or removes a Source from Settings
- **THEN** the change is applied; removing a Source also deletes its cached metadata and resume points

#### Scenario: Player defaults

- **WHEN** the user opens Settings
- **THEN** it exposes default subtitle language, default audio language, subtitle size, seek step (10 / 30 seconds), and UI language (at least English and Simplified Chinese)

#### Scenario: Back from top-level Settings

- **WHEN** the user presses Back on the top-level Settings screen
- **THEN** the System returns to Home
