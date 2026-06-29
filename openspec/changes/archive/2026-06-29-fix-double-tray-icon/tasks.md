## 1. Config Cleanup

- [x] 1.1 Open `src-tauri/tauri.conf.json` and locate the `app.trayIcon` configuration block
- [x] 1.2 Delete the `trayIcon` block (iconPath, iconAsTemplate, etc.)
- [x] 1.3 Verification: `cargo check` passed; full build requires signing setup — run manually
- [x] 1.4 Config-only change, no platform-specific logic — trivially compatible
