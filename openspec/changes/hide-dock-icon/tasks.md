## 1. Implementation

- [x] 1.1 Add `#[cfg(target_os = "macos")] app.handle().set_activation_policy(tauri::ActivationPolicy::Accessory)?;` in `main.rs` setup closure

## 2. Verification

- [x] 2.1 `cargo check` passed on macOS
