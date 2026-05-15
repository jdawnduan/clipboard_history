# Homebrew Formula Update — v0.2.0

## What Changed

Starting from v0.2.0, `clipboard-history` ships as a macOS `.app` bundle with a
consistent `CFBundleIdentifier` (`com.jdawnduan.clipboard-history`). This is
required so that macOS TCC (accessibility) permissions persist across upgrades.

## Required Formula Changes

The Homebrew formula (in `jdawnduan/homebrew-tap/Formula/clipboard-history.rb`)
must be updated to:

### 1. Create the `.app` bundle after `cargo install`

```ruby
def install
  system "cargo", "install", *std_cargo_args

  # --- NEW: Create .app bundle for persistent TCC permissions ---
  app_name = "Clipboard History.app"
  app_dir = prefix / app_name / "Contents"
  macos_dir = app_dir / "MacOS"
  macos_dir.mkpath

  # Copy the binary into the bundle
  cp bin/"clipboard-history", macos_dir/"clipboard-history"

  # Write Info.plist with stable bundle identifier
  (app_dir / "Info.plist").write <<~PLIST
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
      <key>CFBundleExecutable</key>
      <string>clipboard-history</string>
      <key>CFBundleIdentifier</key>
      <string>com.jdawnduan.clipboard-history</string>
      <key>CFBundleName</key>
      <string>Clipboard History</string>
      <key>CFBundleVersion</key>
      <string>#{version}</string>
      <key>CFBundleShortVersionString</key>
      <string>#{version}</string>
      <key>CFBundlePackageType</key>
      <string>APPL</string>
      <key>LSUIElement</key>
      <true/>
      <key>NSAccessibilityUsageDescription</key>
      <string>Clipboard History needs accessibility access to simulate Cmd+V for pasting clipboard entries into your active application.</string>
    </dict>
    </plist>
  PLIST
  # -----------------------------------------------------------

  # Symlink the bundle's binary so `brew services` finds it
  bin.install_symlink macos_dir/"clipboard-history" => "clipboard-history"
end
```

### 2. (Optional) Bump the revision on upgrade to clear stale TCC entries

macOS caches TCC decisions per bundle ID. If users have old entries from
pre-bundle installations cluttering their permissions list, they can run:

```bash
tccutil reset Accessibility com.jdawnduan.clipboard-history
```

This can be called in the formula's `caveats` section:

```ruby
def caveats
  <<~EOS
    Clipboard History is now installed as a .app bundle.
    If you have stale permissions from a previous version,
    run this once to clean them up:
      tccutil reset Accessibility com.jdawnduan.clipboard-history
  EOS
end
```

Or automatically in `post_install`:

```ruby
def post_install
  system "tccutil", "reset", "Accessibility", "com.jdawnduan.clipboard-history"
end
```

> Using `post_install` resets the permission on EVERY upgrade, which means
> users must re-grant access after each update. The `caveats` approach is
> friendlier — it only runs once on install.

## Verification

After updating the formula:

```bash
# Test install
brew reinstall clipboard-history

# Verify the bundle exists and has correct structure
ls -la $(brew --prefix)/Caskroom/clipboard-history/*/Clipboard\ History.app/Contents/MacOS/clipboard-history

# Verify the binary works from PATH
clipboard-history --help

# Start service
brew services restart clipboard-history
```

## Rollback

If something goes wrong, users can revert to the non-bundle version:

```bash
brew install jdawnduan/tap/clipboard-history@0.1.2
```

But v0.2.0+ should always be installed via the bundle to maintain TCC
permissions correctly.
