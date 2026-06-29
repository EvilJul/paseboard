## ADDED Requirements

### Requirement: Hide Dock icon on macOS
The system SHALL hide the macOS Dock icon when running on macOS.

#### Scenario: Startup hides Dock icon
- **WHEN** PaseBoard starts on macOS
- **THEN** the Dock icon SHALL NOT appear

#### Scenario: Tray icon still visible
- **WHEN** PaseBoard starts on macOS with Dock icon hidden
- **THEN** the system tray icon SHALL still be visible and interactive

#### Scenario: Window show/hide unaffected
- **WHEN** user toggles the main window (tray click or shortcut)
- **THEN** the window SHALL show/hide normally regardless of Dock icon visibility

#### Scenario: No effect on Windows/Linux
- **WHEN** PaseBoard starts on Windows or Linux
- **THEN** the activation policy SHALL NOT be modified
