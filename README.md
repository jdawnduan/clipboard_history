## Installation

### Homebrew (macOS)

You can install `clipboard-history` via Homebrew from the personal tap.

1.  **Install:**
    ```bash
    brew tap jdawnduan/tap
    brew install jdawnduan/tap/clipboard-history
    ```

2.  **Start the daemon as a service:**
    ```bash
    brew services start clipboard-history
    ```

### Building from Source

```bash
# Build the release binary
cargo build --release

# (macOS only) Wrap in a .app bundle for consistent macOS permissions
./scripts/build_app_bundle.sh

# Launch directly:
./target/release/clipboard-history daemon
```

On macOS, the `.app` bundle wraps the binary with a stable bundle identifier
(`com.jdawnduan.clipboard-history`). macOS tracks accessibility permissions by
bundle ID, not binary path — so granting permission once survives all future
upgrades.

### Managing the Service

- **Status:**
  ```bash
  brew services info clipboard-history
  ```

- **Stop:**
  ```bash
  brew services stop clipboard-history
  ```

- **Restart:**
  ```bash
  brew services restart clipboard-history
  ```

- **Logs:**
  ```bash
  tail -f $(brew --prefix)/var/log/clipboard-history.log
  ```

### Permissions (macOS)

The first time you invoke the clipboard popup (`Cmd+Option+V`), macOS will ask for
**Accessibility** permissions. This is required for simulating `Cmd+V` to paste
into your active application.

**Grant the permission** in **System Settings > Privacy & Security > Accessibility**.

Make sure the entry shown is **Clipboard History** (not your terminal app).
If you installed via `brew`, the binary lives inside the `.app` bundle at:
`/usr/local/Caskroom/clipboard-history/.../Clipboard History.app`

> **Permissions survive upgrades.** Starting from v0.2.0, the app uses a stable
> bundle identifier (`com.jdawnduan.clipboard-history`). Grant permission once
> and it carries across future `brew upgrade` runs. No need to re-grant.

---

## For Homebrew Formula Maintainers

The formula in [`jdawnduan/homebrew-tap`](https://github.com/jdawnduan/homebrew-tap)
must be updated to create the `.app` bundle at install time.

See [docs/homebrew-formula-update.md](docs/homebrew-formula-update.md) for the
complete recipe — it covers:

- Running `cargo install` to get the binary
- Creating the `.app` bundle with the correct `Info.plist`
- Symlinking the bundle's binary back to `bin/` for `brew services`
- Optionally running `tccutil reset Accessibility` on upgrade

## Learn How It Works

Interested in understanding the codebase? There's an interactive course that teaches how clipboard-history works — no coding knowledge required.

Switch to the `docs` branch or check out the [docs/README.md](docs/README.md) for instructions on viewing the course.
