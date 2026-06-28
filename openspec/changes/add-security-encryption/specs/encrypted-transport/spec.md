## ADDED Requirements

### Requirement: WebSocket messages are encrypted with AES-256-GCM
All clipboard content and control messages transmitted over WebSocket SHALL be encrypted with AES-256-GCM before sending and decrypted upon receipt.

#### Scenario: Outbound message encrypted
- **WHEN** the system sends a pasteboard message to a remote device
- **THEN** the message payload SHALL be encrypted with AES-256-GCM
- **THEN** the encrypted message SHALL include: 12-byte nonce, ciphertext, sender's ephemeral public key
- **THEN** the plaintext SHALL never be written to the WebSocket send buffer

#### Scenario: Inbound message decrypted
- **WHEN** the system receives an encrypted WebSocket message
- **THEN** it SHALL compute the shared secret using ECDH (own private key + sender's public key)
- **THEN** it SHALL derive the AES-256-GCM key via HKDF-SHA256
- **THEN** it SHALL decrypt the ciphertext
- **THEN** the decrypted payload SHALL be processed as a standard `Message`

#### Scenario: Decryption failure handled
- **WHEN** AES-GCM decryption fails (authentication tag mismatch)
- **THEN** the system SHALL log the failure with remote device info
- **THEN** the system SHALL close the WebSocket connection
- **THEN** the system SHALL mark the device as untrusted

### Requirement: Shared key is derived per-connection
Each WebSocket connection SHALL derive an independent AES-256-GCM key using ECDH + HKDF.

#### Scenario: Unique key per connection
- **WHEN** Device A connects to Device B
- **THEN** the shared key SHALL be derived from X25519(DeviceA_priv, DeviceB_pub)
- **WHEN** Device A also connects to Device C
- **THEN** the shared key with Device C SHALL be different from the key with Device B

#### Scenario: Reconnection produces new key
- **WHEN** Device A disconnects and reconnects to Device B
- **THEN** a new ECDH exchange SHALL occur
- **THEN** the new shared key SHALL be independent of the previous session key

### Requirement: Message size limit applies to encrypted payload
The 10MB size limit SHALL be checked on the plaintext payload BEFORE encryption.

#### Scenario: Plaintext size checked before encryption
- **WHEN** a clipboard message exceeds 10MB plaintext size
- **THEN** the message SHALL be rejected before encryption
- **THEN** the error SHALL be reported to the sending device
