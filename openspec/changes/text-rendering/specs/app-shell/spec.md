# Spec Delta

## ADDED Requirements

### Requirement: Bundled rendering font

The System SHALL ship its text-rendering font as part of the application payload, so that drawing text never depends on a system font or an asset installed separately from the application.

#### Scenario: Font ships with the package

- **WHEN** the IPK is built and installed
- **THEN** everything needed to render text is contained in the application payload itself, with no separate font installation step

#### Scenario: No system font lookup on device

- **WHEN** the app draws a label on a webOS television
- **THEN** rendering consults no OS font API and reads no font file outside the application's own payload

#### Scenario: Simulator uses the same font

- **WHEN** the host simulator and the device build render the same screen
- **THEN** both render with the same bundled font, so host output matches the device
