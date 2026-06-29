## Purpose

Define the system tray icon behavior for PaseBoard, ensuring exactly one icon appears across all supported platforms.

## Requirements

### Requirement: Single tray icon
The system SHALL display exactly one tray icon in the system tray after startup.

#### Scenario: Startup creates single icon
- **WHEN** PaseBoard starts on any supported platform
- **THEN** exactly one tray icon appears in the system tray

#### Scenario: No duplicate icons
- **WHEN** the user inspects the system tray after startup
- **THEN** there SHALL NOT be duplicate PaseBoard icons
