## ADDED Requirements

### Requirement: Pairing lifecycle
The system SHALL manage device pairing through request, confirm, and unpair stages.

#### Scenario: New device requests pairing
- **WHEN** an unpaired device connects for the first time
- **THEN** the system SHALL send a `PairingRequest` message to the connected device
- **AND** the receiving side SHALL show a confirmation prompt to the user

#### Scenario: User accepts pairing
- **WHEN** user clicks "接受" on the pairing confirmation prompt
- **THEN** the system SHALL store the device ID in `paired_devices` table
- **AND** the system SHALL send `PairingResponse { accepted: true }` to the requesting device
- **AND** subsequent connections from this device SHALL auto-accept

#### Scenario: User rejects pairing
- **WHEN** user clicks "拒绝" on the pairing confirmation prompt
- **THEN** the system SHALL NOT store the device ID
- **AND** the system SHALL send `PairingResponse { accepted: false }` to the requesting device
- **AND** the requesting device SHALL enter cooldown state for 30 minutes

#### Scenario: Already paired device connects
- **WHEN** a previously paired device connects
- **THEN** the system SHALL auto-accept the connection without user prompt
- **AND** the system SHALL NOT send a new pairing request

#### Scenario: User unpairs a device
- **WHEN** user removes a device from the paired list
- **THEN** the system SHALL delete the device from `paired_devices` table
- **AND** the next connection from this device SHALL trigger a new pairing request

#### Scenario: Cooldown prevents repeated requests
- **WHEN** a device was rejected within the last 30 minutes
- **THEN** the system SHALL NOT show a new pairing prompt
- **AND** the system SHALL automatically reject the connection

### Requirement: Pairing status display
The system SHALL display pairing status for each discovered device.

#### Scenario: Device list shows status
- **WHEN** user views the device list
- **THEN** each device SHALL show its pairing status: "已配对", "待配对", or "未配对"

#### Scenario: Pairing notification
- **WHEN** receiving a `PairingRequest` from a new device
- **THEN** the system SHALL show a notification with "接受" and "拒绝" options
