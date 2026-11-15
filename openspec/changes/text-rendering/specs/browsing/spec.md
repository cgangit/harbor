# Spec Delta

## ADDED Requirements

### Requirement: Readable glyph text

The System SHALL render every text label drawn on any surface as readable glyph text, rasterized from a font bundled with the application, at the size the screen requests.

#### Scenario: Labels render as glyphs

- **WHEN** any screen (Home, browser, Settings, Player, HUD, Add dialog, loading or error rows) draws a text label
- **THEN** the label appears as readable glyph text at the requested role size, not as a placeholder shape

#### Scenario: Both UI languages render

- **WHEN** the UI language is English or Simplified Chinese
- **THEN** every translated string of that language is drawn as glyphs the viewer can read, because the bundled font covers the i18n dictionary's characters

#### Scenario: Characters outside the font

- **WHEN** a label contains a character the bundled font does not cover
- **THEN** a visible replacement glyph is drawn for that character and the rest of the label still renders, with no panic and no silently blank label
