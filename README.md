## Installation

### Homebrew (macOS)

You can install `clipboard-history` via Homebrew by creating a local tap or using the provided formula.

1.  **Install via the personal tap:**
    ```bash
    brew tap jdawnduan/tap
    brew install jdawnduan/tap/clipboard-history
    ```

2.  **Start the daemon as a service:**
    ```bash
    brew services start clipboard-history
    ```

### Managing the Service

Once installed as a service, you can manage it using `brew services`:

- **Check status:**
  ```bash
  brew services info clipboard-history
  ```

- **Stop the service:**
  ```bash
  brew services stop clipboard-history
  ```

- **Restart the service:**
  ```bash
  brew services restart clipboard-history
  ```

- **View logs:**
  ```bash
  tail -f $(brew --prefix)/var/log/clipboard-history.log
  ```

### Permissions (macOS)

The first time you start the daemon (either via `brew services` or manually), macOS will ask for **Accessibility** permissions. This is required for:
1.  Global hotkey support (`Cmd+Option+v`).
2.  Select from 1 to 10 (0 means 10) past entries.
2.  `Cmd+v` to do paste into your active application.

If it doesn't work, ensure `clipboard-history` (or your terminal/Homebrew) is enabled in **System Settings > Privacy & Security > Accessibility**.

Once the service is started and permissions are granted, you can use the `clipboard-history` CLI from anywhere.

---

## Learn How It Works

Interested in understanding the codebase? There's an interactive course that teaches how clipboard-history works — no coding knowledge required.

Switch to the `docs` branch or check out the [docs/README.md](docs/README.md) for instructions on viewing the course.
