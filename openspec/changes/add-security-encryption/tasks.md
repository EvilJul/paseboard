## 1. Dependencies & Config

- [ ] 1.1 Add `ed25519-dalek`, `x25519-dalek`, `aes-gcm`, `hkdf`, `rand`, `keyring` crates to Cargo.toml
- [ ] 1.2 Switch `rusqlite` to `sqlcipher` feature
- [ ] 1.3 Add `base64` crate for public key serialization
- [ ] 1.4 Run `cargo check` to verify dependency resolution

## 2. Device Identity

- [ ] 2.1 Implement `IdentityManager` in `src/network/identity.rs`: Ed25519 key generation, loading, persistence
- [ ] 2.2 Add `device_id` derivation from public key fingerprint (SHA256)
- [ ] 2.3 Persist key pair to `~/.paseboard/identity.pem` on first run
- [ ] 2.4 Integrate `IdentityManager` into `App` initialization flow
- [ ] 2.5 Add unit tests for key generation, loading, device ID derivation

## 3. Public Key Broadcast

- [ ] 3.1 Add `public_key` field to mDNS TXT records in `MdnsService::register()`
- [ ] 3.2 Add `public_key` field to UDP broadcast JSON payload
- [ ] 3.3 Parse remote public key from mDNS/UDP discovery in `parse_service_info`
- [ ] 3.4 Derive device ID from remote public key fingerprint (SHA256)
- [ ] 3.5 Add unit tests for public key parsing and fingerprint derivation

## 4. Encrypted Transport

- [ ] 4.1 Implement ECDH key exchange helper: `x25519` shared secret derivation
- [ ] 4.2 Implement `HKDF-SHA256` key derivation for AES-256-GCM key
- [ ] 4.3 Implement `EncryptedMessage` struct (nonce, ciphertext, public_key)
- [ ] 4.4 Add encrypt/decrypt methods to `WebSocketClient` send/recv pipeline
- [ ] 4.5 Add decrypt in `WebSocketServer` message receive path
- [ ] 4.6 Handle decryption failure: disconnect + log + mark device untrusted
- [ ] 4.7 Add message size check (10MB) on plaintext BEFORE encryption
- [ ] 4.8 Add unit tests for ECDH, encrypt/decrypt, failure handling

## 5. Encrypted Storage

- [ ] 5.1 Replace `Connection::open` with SQLCipher `PRAGMA key` in `HistoryStorage::new()`
- [ ] 5.2 Implement keychain integration: `keyring::Credential` for "PaseBoard/db-key"
- [ ] 5.3 Implement fallback key file at `~/.paseboard/db.key` for headless environments
- [ ] 5.4 Generate random 32-byte key on first DB creation
- [ ] 5.5 Handle key retrieval failure (keychain unavailable, corrupted key)
- [ ] 5.6 Add UI warning when using file-based key fallback
- [ ] 5.7 Add integration tests for encrypted DB operations

## 6. Backward Compatibility

- [ ] 6.1 Add `crypto_version` field to mDNS/UDP discovery messages
- [ ] 6.2 Detect incompatible devices and show "不兼容" status in device list
- [ ] 6.3 Prevent WebSocket connection attempts to incompatible devices
- [ ] 6.4 Add integration test for version incompatibility detection
