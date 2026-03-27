## Installation

### Homebrew (macOS)

You can install `clipboard-history` via Homebrew by creating a local tap or using the provided formula.

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/jdawnduan/clipboard_history.git
    cd clipboard_history
    ```

2.  **Install via the local formula:**
    ```bash
    brew install --build-from-source ./clipboard-history.rb
    ```

3.  **Start the daemon as a service:**
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

### Creating a Tap (Optional)

If you want to share this formula with others, you can create a Homebrew Tap:

1.  Create a new repository on GitHub named `homebrew-tap`.
2.  Move the `clipboard-history.rb` into that repository.
3.  Others can then install it with:
    ```bash
    brew tap <your-username>/tap
    brew install clipboard-history
    ```

Note: To use the formula without `--build-from-source`, you should create a GitHub release and update the `url` and `sha256` in `clipboard-history.rb`.
