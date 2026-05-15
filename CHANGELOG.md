# Changelog

## [0.2.0] - 2026-05-15

### Added
- **Floating panel over full-screen:** popup window appears above full-screen apps
  and auto-centers on the display containing the mouse cursor.
- **macOS .app bundle** (`scripts/build_app_bundle.sh`) with stable bundle identifier
  (`com.jdawnduan.clipboard-history`). Permissions now survive upgrades.

### Fixed
- **CJK font rendering:** Chinese, Japanese, and Korean characters now display
  correctly in the popup preview instead of rendering as boxes.

### Changed
- **Performance:** clipboard history kept in memory (no disk read on hotkey),
  event-driven hotkey listener (sub-ms wake instead of 100ms polling),
  async disk writes (clipboard monitor never blocks on file I/O).

## [0.1.2] - 2026-04-03

### Added
- Single-instance daemon support: automatically terminates existing daemon when starting a new one
- Full Unicode clipboard support: Chinese, Japanese, German, French characters and emojis now work correctly
- Dock icon hidden when running the daemon

### Changed
- Improved process management with PID lock file

## [0.1.1] - Initial release

### Features
- Clipboard history monitoring with configurable size limit
- Global hotkey support (Cmd+Option+V on macOS, Ctrl+Option+V on Linux)
- Popup UI for selecting and pasting history entries
- CLI commands for managing clipboard history
- Service/daemon support for auto-start
