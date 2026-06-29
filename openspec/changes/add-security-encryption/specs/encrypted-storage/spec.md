## ADDED Requirements

### Requirement: Database is encrypted with SQLCipher
The clipboard history database SHALL use SQLCipher for transparent encryption at rest.

#### Scenario: Database created encrypted
- **WHEN** PaseBoard runs for the first time
- **THEN** the system SHALL create `~/.paseboard/history.db` as a SQLCipher encrypted database
- **THEN** the encryption key SHALL NOT be stored in the same directory as the database

#### Scenario: Database opened with correct key
- **WHEN** PaseBoard starts and `~/.paseboard/history.db` exists
- **THEN** the system SHALL retrieve the encryption key from the platform keychain
- **THEN** the system SHALL open the database with the correct key using `PRAGMA key`
- **THEN** queries SHALL function as before (encryption is transparent to the query layer)

#### Scenario: Database opened with wrong key
- **WHEN** the encryption key cannot be retrieved or is incorrect
- **THEN** the system SHALL log the error
- **THEN** the system SHALL start with an empty history (cannot decrypt existing data)
- **THEN** the UI SHALL show "加密数据库无法打开" warning

### Requirement: Encryption key is managed by platform keychain
The database encryption key SHALL be stored and retrieved via the system's platform keychain service.

#### Scenario: Key stored on first run
- **WHEN** the database is created for the first time
- **THEN** the system SHALL generate a random 32-byte encryption key
- **THEN** the system SHALL store the key in the platform keychain with service name "PaseBoard" and account "db-key"
- **THEN** the system SHALL use this key to initialize the SQLCipher database

#### Scenario: Key retrieved on subsequent runs
- **WHEN** PaseBoard starts and the database file exists
- **THEN** the system SHALL retrieve the encryption key from the platform keychain
- **THEN** the system SHALL use the retrieved key to open the database

#### Scenario: Keychain unavailable fallback
- **WHEN** the platform keychain is unavailable (headless server, WSL, container)
- **THEN** the system SHALL fall back to a key file at `~/.paseboard/db.key`
- **THEN** the system SHALL log a warning about reduced security
- **THEN** the UI SHALL show a non-blocking notification "密码存储在文件中，建议升级到有密钥链的平台"

### Requirement: Capacity management works with encrypted database
The existing capacity management (1000 record limit, FIFO eviction) SHALL work identically with the encrypted database.

#### Scenario: Capacity eviction on encrypted DB
- **WHEN** the clipboard history reaches 1000 records in an encrypted database
- **THEN** the oldest 100 records SHALL be deleted
- **THEN** the count and query operations SHALL function identically to unencrypted mode
