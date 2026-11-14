# Spec Delta

## MODIFIED Requirements

### Requirement: Home and source browser

The System SHALL provide a couch-oriented Home screen that lists configured sources and recent files, and a Source browser for folders and media files.

#### Scenario: Home with configured sources

- **WHEN** the System starts with at least one configured Source
- **THEN** it shows a Home screen with a hero area and horizontal shelves for Continue Watching and Recently Added

#### Scenario: Home with zero sources

- **WHEN** the System starts with zero configured Sources
- **THEN** it shows an empty-state panel whose primary action opens Add Source

#### Scenario: Remote navigation

- **WHEN** a shelf or grid has focus
- **THEN** the System moves focus with Left/Right/Up/Down, activates the focused item with OK, and returns to the previous screen with Back

#### Scenario: Source browser

- **WHEN** the user opens a Source
- **THEN** the System lists that Source's folders and media files in a poster-or-list browser with A–Z jump, sort (name, date, size), and filter (video / audio / all)

#### Scenario: Couch caption floor

- **WHEN** ordinary content text is laid out on the 1920x1080 surface
- **THEN** it is sized at 24 px or larger

#### Scenario: Shelves show real source content

- **WHEN** the Home screen renders with at least one configured Source
- **THEN** Recently Added lists entries actually enumerated from the configured Sources and Continue Watching lists entries from the resume store, with no hard-coded demo titles

#### Scenario: Source changes reflect without restart

- **WHEN** the set of configured Sources changes (for example the user adds one in Settings)
- **THEN** the Home screen and the Source browser reflect that change the next time they are shown, without restarting the System

### Requirement: UI language (PlxNative-inspired)

The System SHALL draw a 60 fps, focus-first interface with glass panels over artwork, so that browsing feels like a native TV app.

#### Scenario: Fixed surface and present loop

- **WHEN** the UI renders
- **THEN** it draws on a fixed 1920x1080 surface with a 60 fps present loop

#### Scenario: Role-based colour theme

- **WHEN** any surface chooses colours
- **THEN** it uses a role-based colour theme (text primary, accent, control fill, scrim) rather than per-screen colour literals

#### Scenario: Glass overlay menus

- **WHEN** an overlay menu is shown
- **THEN** it uses a translucent glass panel, a 30% scrim, and a 1 px rim over a frozen snapshot of the page behind it

#### Scenario: Focus as a first-class state

- **WHEN** an item receives focus
- **THEN** the focused card scales up and captions appear in a band under the focused card

#### Scenario: Remote-first input

- **WHEN** the user provides input
- **THEN** it comes from the webOS remote (keys and Magic Remote pointer), and hover is not required to discover actions

#### Scenario: Pointer click activates like OK

- **WHEN** the user clicks the Magic Remote pointer on the focused item, or presses OK on the remote
- **THEN** the System activates the focused item identically in both cases
