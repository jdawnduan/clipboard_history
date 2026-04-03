# Changelog

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
