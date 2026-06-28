## ADDED Requirements

### Requirement: Device generates Ed25519 key pair on first run
The system SHALL generate an Ed25519 key pair when PaseBoard runs for the first time and no existing key is found.

#### Scenario: Fresh install generates key pair
- **WHEN** PaseBoard starts with no existing key file
- **THEN** the system SHALL generate a new Ed25519 key pair
- **THEN** the system SHALL persist the private key to disk at `~/.paseboard/identity.pem`
- **THEN** the system SHALL derive a device ID from the public key fingerprint (SHA256 of public key bytes)

#### Scenario: Existing key loaded on restart
- **WHEN** PaseBoard starts and `~/.paseboard/identity.pem` exists
- **THEN** the system SHALL load the existing key pair
- **THEN** the system SHALL use the same device ID as previous session

### Requirement: Device broadcasts public key for discovery
The system SHALL include the Ed25519 public key in mDNS TXT records and UDP broadcast payloads.

#### Scenario: Public key in mDNS TXT record
- **WHEN** PaseBoard registers its mDNS service
- **THEN** the TXT record SHALL include a `pk` field with the base64-encoded Ed25519 public key

#### Scenario: Public key in UDP broadcast
- **WHEN** PaseBoard sends a UDP discovery packet
- **THEN** the JSON payload SHALL include a `public_key` field with the base64-encoded Ed25519 public key

#### Scenario: Device ID derived from public key
- **WHEN** the system reads a remote device's mDNS/UDP discovery
- **THEN** it SHALL compute the device ID as the SHA256 fingerprint of the remote public key
- **THEN** the device ID field from the discovery message SHALL be verified against this fingerprint

### Requirement: Device identity verification via ECDH
The system SHALL verify remote device identity through successful ECDH key exchange: only a device holding the private key corresponding to its advertised public key can establish an encrypted session.

#### Scenario: Valid identity accepted
- **WHEN** a remote device completes ECDH key exchange and sends a valid encrypted message
- **THEN** the local system SHALL accept the device as authenticated
- **THEN** the device SHALL appear as "connected" in the device list

#### Scenario: Invalid identity rejected
- **WHEN** a remote device's encrypted message fails AES-GCM decryption
- **THEN** the local system SHALL disconnect the WebSocket connection
- **THEN** the device SHALL remain in the device list with status "authentication failed"
- **THEN** the system SHALL log the authentication failure
