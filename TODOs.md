# Clipboard History — Improvement Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Test each change, commit after each task.

**Goal:** Fix 4 quality-of-life pain points: floating panel over full-screen, latency reduction, CJK character rendering, and macOS permission cleanup on upgrade.

**Architecture:** Single-binary Rust app using eframe/egui for GUI, global-hotkey for hotkey, and arboard for clipboard access. The daemon runs as a background process with an always-hidden eframe window that wakes up on hotkey. This plan adds an in-memory shared history state, event-driven hotkey thread, async disk writes, system CJK font fallback, a native-level floating panel (CGShieldingWindow), and a macOS .app bundle wrapper.

**Tech Stack:** Rust, egui/eframe 0.29, objc2, global-hotkey, enigo, arboard, Homebrew

---

## Task 1: In-Memory Shared Clipboard History

**Why:** Every hotkey press currently reads JSON from disk (`ClipboardHistory::load()`). With max 20 entries × 128 KB = 2.5 MB worst case, keeping it in memory costs ~0 but saves ~5-15ms on every hotkey press.

**Files:**
- Modify: `src/history.rs`
- Modify: `src/main.rs`
- Test: manual (observe no degredation)

**Step 1: Add `Arc<Mutex<>>` sharing to `ClipboardHistory`**

Wrap `ClipboardHistory` in `pub type SharedHistory = Arc<Mutex<ClipboardHistory>>`. Add helper methods on `SharedHistory` for thread-safe read/write.

```rust
// In src/history.rs
use std::sync::{Arc, Mutex};

pub type SharedClipboardHistory = Arc<Mutex<ClipboardHistory>>;

pub fn new_shared() -> SharedClipboardHistory {
    Arc::new(Mutex::new(ClipboardHistory::load().unwrap_or_default()))
}
```

**Step 2: Refactor `main.rs` to pass shared history everywhere**

- In `daemon()`: create `SharedHistory` before spawning threads
- Pass clone to `monitor_clipboard()`
- Store in `DaemonApp` struct (instead of `Vec<String>`)
- On hotkey, read from shared history directly, no file I/O

Change `DaemonApp.entries: Vec<String>` to `DaemonApp.history: SharedClipboardHistory`. In `update()`, lock and read entries:

```rust
if let Ok(history) = self.history.lock() {
    self.entries = history.entries().iter().take(10).map(|e| e.content.clone()).collect();
}
```

**Step 3: Update `monitor_clipboard()` signature**

```rust
async fn monitor_clipboard(
    skip_next: Arc<AtomicBool>,
    history: SharedClipboardHistory,
) -> Result<(), Box<dyn std::error::Error>>
```

Inside, lock to add entries AND save to disk after each addition (for now — Task 3 makes this async).

**Step 4: Verify**

```bash
cargo build
cargo run -- daemon &
# Press Cmd+Option+V → window appears instantly
# Confirm entries show correctly
kill %1
```

**Step 5: Commit**

```bash
git add -A && git commit -m "perf: keep clipboard history in memory, remove file read on hotkey"
```

---

## Task 2: Event-Driven Hotkey Listener

**Why:** Current code polls `GlobalHotKeyEvent::receiver().try_recv()` once per eframe update cycle (every 100ms). Worst-case hotkey→window latency is ~100ms. With a dedicated listener thread + blocking `recv()` + `ctx.request_repaint()`, worst case drops to ~1-5ms.

**Files:**
- Modify: `src/main.rs`

**Step 1: Add hotkey listener thread**

After creating the `GlobalHotKeyManager`, spawn a thread that does **blocking** `recv()`:

```rust
let hotkey_triggered = Arc::new(AtomicBool::new(false));
let hotkey_triggered_clone = hotkey_triggered.clone();

std::thread::spawn(move || {
    loop {
        if let Ok(event) = GlobalHotKeyEvent::receiver().recv() {
            if event.id == hotkey_id {
                hotkey_triggered_clone.store(true, Ordering::SeqCst);
            }
        }
    }
});
```

**Step 2: Pass context to hotkey thread for `request_repaint()`**

Store `egui::Context` in an `Arc<Mutex<Option<egui::Context>>>`. Set it during `update()`'s first call. The hotkey thread locks it and calls `request_repaint()` when the trigger fires.

```rust
// In DaemonApp::new, create empty holder
let ctx_holder: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
let ctx_holder_clone = ctx_holder.clone();

// In hotkey listener thread, after setting flag:
if let Ok(mut ctx_opt) = ctx_holder_clone.lock() {
    if let Some(ctx) = ctx_opt.as_ref() {
        ctx.request_repaint();
    }
}

// In update() first call, store ctx
*self.ctx_holder.lock().unwrap() = Some(ctx.clone());
```

**Step 3: Simplify `update()` hotkey check**

Replace the `GlobalHotKeyEvent::receiver().try_recv()` block with checking the atomic flag:

```rust
if self.hotkey_triggered.swap(false, Ordering::SeqCst) && !self.popup_open {
    // Load history and show popup (same code as before)
}
```

**Step 4: Still keep `request_repaint_after` for frame updates but at a lower rate**

Reduce from 100ms to 200ms for idle frame checks, since hotkey wakes via explicit `request_repaint()`. But we still need it in case user wants animation etc.

**Step 5: Verify**

```bash
cargo run -- daemon &
# Press Cmd+Option+V → feel the difference, window appears instantly
kill %1
```

**Step 6: Commit**

```bash
git add -A && git commit -m "perf: event-driven hotkey listener, sub-ms wake instead of 100ms polling"
```

---

## Task 3: Async Disk Writes

**Why:** Clipboard monitoring thread blocks on `history.save()` which writes JSON to disk. For large entries (~128 KB), this can stall the monitor loop by ~1-2ms. Async write means the monitor never waits.

**Files:**
- Modify: `src/main.rs`
- Modify: `src/history.rs`

**Step 1: Add a background save channel**

Use a `tokio::sync::mpsc` channel. The monitor sends "save request" messages. A background task receives and writes to disk.

```rust
// In daemon()
let (save_tx, mut save_rx) = tokio::sync::mpsc::channel::<()>(32);

// Background saver task
tokio::spawn(async move {
    while save_rx.recv().await.is_some() {
        // Lock, serialize, write — all sequential but non-blocking for monitor
        if let Ok(history) = shared_history.lock() {
            if let Err(e) = history.save() {
                eprintln!("Failed to save history: {}", e);
            }
        }
    }
});
```

**Step 2: Send save signal after each new entry**

In `monitor_clipboard()`, send on the channel: `let _ = save_tx.send(()).await;`

**Step 3: Remove direct `save()` calls from monitor**

Comment out or remove `history.save()?;` in the monitor loop. The background saver handles it.

**Step 4: Rate-limit saves**

If entries arrive faster than disk can write (unlikely but possible), add a debounce with `tokio::time::sleep(Duration::from_millis(50))` in the saver before writing, so rapid clipboard changes coalesce into one save.

**Step 5: Verify**

```bash
cargo run -- daemon &
# Copy several items in rapid succession
# History.json should eventually reflect all entries
kill %1
cat ~/Library/Application\ Support/clipboard-history/history.json | head
```

**Step 6: Commit**

```bash
git add -A && git commit -m "perf: async disk writes, clipboard monitor no longer blocks on I/O"
```

---

## Task 4: CJK Font Rendering in Popup

**Why:** egui's default font (Helvetica Neue) lacks CJK glyphs. Chinese/Japanese/Korean characters render as boxes in the preview but paste correctly.

**Files:**
- Modify: `src/main.rs` (add `FontDefinitions` configuration)

**Step 1: Configure font fallbacks in `DaemonApp::new()` or at eframe init**

In `main.rs` where eframe options are set, or inside `DaemonApp::update()` (once), configure fonts:

```rust
use egui::FontDefinitions;

fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    
    // Prepend macOS CJK system fonts as fallbacks for the proportional family
    #[cfg(target_os = "macos")]
    {
        let families = fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap();
        // PingFang SC for Chinese
        families.insert(0, "PingFang SC".to_owned());
        // Hiragino Sans GB as additional fallback
        families.insert(1, "Hiragino Sans GB".to_owned());
        // Apple SD Gothic Neo for Korean
        families.insert(2, "Apple SD Gothic Neo".to_owned());
    }
    
    #[cfg(target_os = "linux")]
    {
        let families = fonts.families.get_mut(&egui::FontFamily::Proportional).unwrap();
        families.insert(0, "Noto Sans CJK SC".to_owned());
        families.insert(1, "WenQuanYi Micro Hei".to_owned());
    }
    
    ctx.set_fonts(fonts);
}
```

**Step 2: Call the setup once after the application starts**

```rust
// In DaemonApp::update(), first call
static FONTS_SET: std::sync::Once = std::sync::Once::new();
FONTS_SET.call_once(|| {
    setup_cjk_fonts(ctx);
});
```

**Step 3: Test with various CJK characters**

Copy some Chinese text, Japanese text, emojis. Open popup. Confirm glyphs render correctly.

| Input | Expected |
|---|---|
| `你好世界` | 你好世界 (not ☐☐☐☐) |
| `こんにちは` | こんにちは |
| `😊🍕🎉` | emojis render |
| `ãéîøü` | accented Latin renders |

**Step 4: Fallback if system fonts aren't available**

If the above doesn't work, fall back to embedding a lightweight CJK font. PingFang is always present on macOS >= 10.11, so this should be reliable.

**Step 5: Commit**

```bash
git add -A && git commit -m "fix: add CJK system font fallbacks for clipboard popup rendering"
```

---

## Task 5: Floating Panel Over Full-Screen + Center on Cursor + Focus Return

**Why:** Current `.with_always_on_top()` uses a normal window level that sits below full-screen apps (which occupy their own space at `CGShieldingWindowLevel`). Need to raise the popup to `NSFloatingWindowLevel` or `CGShieldingWindowLevel - 1` to appear over full-screen content.

**Files:**
- Modify: `src/platform/macos.rs` (add floating-panel window mojo)
- Modify: `src/main.rs` (call platform code after window creation)

**Step 1: Understand the macOS window level hierarchy**

Full-screen apps live at roughly `CGShieldingWindowLevel` (~24 or ~2000 CGWindowLevel). Our window needs to be at `CGShieldingWindowLevel - 1` or `NSFloatingWindowLevel` to appear above them but below system overlays.

```
CGWindowLevelKey:
    kCGNormalWindowLevelKey      = 0
    kCGFloatingWindowLevelKey    = 3    (NSFloatingWindowLevel)
    kCGShieldingWindowLevelKey   = 24   (full-screen app level)
    
We want: kCGFloatingWindowLevelKey (or kCGShieldingWindowLevelKey - 1)
```

In CoreGraphics: `CGShieldingWindowLevel()` returns `CGWindowLevel` for shielding. On most systems it's ~2000. `NSFloatingWindowLevel` is ~4. We need something >= `CGShieldingWindowLevel()`.

Actually, looking more carefully at macOS:
- Full-screen apps use `kCGFullScreenWindowLevel` = `kCGShieldingWindowLevel` + 1
- We want to be *above* full-screen apps, which means `kCGMainMenuWindowLevel` or higher
- But that risks overlapping system UI

The commonly recommended value is `CGShieldingWindowLevel()` for truly covering full-screen. Some apps use `CGWindowLevelForKey(.floatingWindow)` + 100.

Let me check what value actually works. The safest approach is to use `CGShieldingWindowLevel` which is about 2000 on modern macOS.

**Step 2: Add platform function to set window level**

In `src/platform/macos.rs`:

```rust
pub fn set_floating_window_level(ns_window: &objc2::runtime::AnyObject) {
    unsafe {
        let level: i32 = core_graphics::base::CGShieldingWindowLevel();
        let _: () = msg_send![ns_window, setLevel: level];
    }
}
```

But we need to get the NSWindow handle from eframe. eframe creates the window internally. We need to hook into the creation.

**Step 2a: Get NSWindow from eframe**

In eframe 0.29, we can use the `native_options` or access the window via the egui context when running on macOS. The easiest way is to store a callback or use `Frame::native_window_id()` and then get the NSWindow from there.

Actually, looking at eframe's API: `eframe::Frame::native_window_id()` returns `Option<NonZeroU64>` on macOS which is the NSWindow pointer. We can cast it:

```rust
// In DaemonApp::update(), after window is shown:
#[cfg(target_os = "macos")]
if let Some(native_id) = _frame.native_window_id() {
    let ns_window = native_id.get() as *mut objc2::runtime::AnyObject;
    if let Some(window) = unsafe { ns_window.as_ref() } {
        platform::set_floating_window_level(window);
    }
}
```

But this needs to be done when the window becomes visible (or right after creation). Let me think...

Actually, we want to set the level once when the window is created. We could use `eframe::NativeOptions`'s `created` callback if it exists, or set it lazily when first shown.

Simpler: set it every time the window is shown (in the hotkey handler, right after `send_viewport_cmd(Visible(true))`). Overhead is negligible.

**Step 3: Center on cursor's display**

Instead of relying on eframe's `centered: true` (which centers on the main display), compute center of the display where the cursor is:

```rust
// In platform/macos.rs
pub fn center_window_on_cursor_screen(ns_window: &objc2::runtime::AnyObject) {
    use core_foundation::base::TCFType;
    
    // Get cursor position
    let mut mouse_location = CGPoint::new(0.0, 0.0);
    unsafe {
        mouse_location = CGEvent.new_location();
        // Actually: CGEventGetLocation or CGEvent.new_location()
    }
    
    // Or simpler: use NSScreen
    // Get window frame size, compute center for the screen containing cursor
}
```

This gets involved on the macOS objc2 side. Alternative approach: Use `NSScreen.screens` + `NSEvent.mouseLocation`, but that requires more objc2 work.

**Simpler plan:** Use eframe's built-in centering but override position after window creation on macOS:

```rust
// Compute center of screen with cursor
let screens = ... // get NSScreen array
for screen in screens {
    let frame = screen.frame();
    let cursor = ... // get mouse location in screen coordinates
    if /* cursor is in this screen's frame */ {
        let window_frame = window.frame();
        let x = frame.mid_x - window_frame.size.width / 2.0;
        let y = frame.mid_y - window_frame.size.height / 2.0;
        window.set_frame_origin(NSPoint { x, y });
    }
}
```

This is substantial objc2 code. Let me keep it focused and practical: use `CGDisplayBounds` + `CGEvent` to get cursor position.

**Step 4: Ensure focus returns to previous app after paste**

Current code calls `platform::deactivate_app()` which calls `[NSApp hide:]` and `deactivate`. This should return focus to the previously active application since our app is running as `NSAccessory`.

But we need to make sure this works smoothly. The sequence after selection should be:

1. Skip next clipboard monitor update
2. Set clipboard content
3. Hide window immediately
4. Simulate Cmd+V
5. Drop activation (let previous app become active again)

Testing this sequence is important.

**Step 5: Put it all together in `paste_entry()`**

Actually, re-reading the code: `paste_entry()` already does most of this. The key change is the window level and centering.

**Implementation approach for Step 2+3 (concise):**

In `src/main.rs`, add a helper that's called once when the window is first made visible:

```rust
fn setup_floating_window(frame: &eframe::Frame) {
    #[cfg(target_os = "macos")]
    {
        if let Some(native_id) = frame.native_window_id() {
            let ptr = native_id.get() as *mut objc2::runtime::AnyObject;
            if let Some(ns_window) = unsafe { ptr.as_ref() } {
                crate::platform::macos::make_floating_panel(ns_window);
            }
        }
    }
}
```

In `src/platform/macos.rs`:

```rust
pub fn make_floating_panel(ns_window: &objc2::runtime::AnyObject) {
    unsafe {
        // Set window level to floating (above full-screen apps)
        let level = core_graphics::base::CGShieldingWindowLevel();
        let _: () = msg_send![ns_window, setLevel: level as i64]; // CoreGraphics level is CGWindowLevel (i32 on 64-bit)
        
        // Make it a floating panel that doesn't activate its app
        let _: () = msg_send![ns_window, setFloatingPanel: true];
        
        // Center on cursor's display
        center_on_cursor_display(ns_window);
    }
}

fn center_on_cursor_display(ns_window: &objc2::runtime::AnyObject) {
    unsafe {
        // Get screen for window
        let screen: *mut objc2::runtime::AnyObject = msg_send![ns_window, screen];
        if screen.is_null() { return; }
        
        let screen_frame: CGRect = msg_send![screen, frame];
        let window_frame: CGRect = msg_send![ns_window, frame];
        
        let center_x = screen_frame.origin.x + (screen_frame.size.width - window_frame.size.width) / 2.0;
        let center_y = screen_frame.origin.y + (screen_frame.size.height - window_frame.size.height) / 2.0;
        
        let _: () = msg_send![ns_window, setFrameOrigin: NSPoint::new(center_x, center_y)];
    }
}
```

Wait, there's a subtlety: the `screen` property on NSWindow gives us the screen the window is currently on (which defaults to the main screen). We need to specify *which* screen to center on - the one with the cursor.

Better:

```rust
fn center_on_cursor_display(ns_window: &objc2::runtime::AnyObject) {
    unsafe {
        // Get cursor location
        let event_class = objc2::runtime::AnyClass::get("NSEvent").unwrap();
        let mouse_location: NSPoint = msg_send![event_class, mouseLocation];
        
        // Find which screen contains the cursor
        let screens_class = objc2::runtime::AnyClass::get("NSScreen").unwrap();
        let screens: *mut objc2::runtime::AnyObject = msg_send![screens_class, screens];
        let count: NSUInteger = msg_send![screens, count];
        
        let mut target_screen = screen; // default to current window screen
        for i in 0..count {
            let screen: *mut objc2::runtime::AnyObject = msg_send![screens, objectAtIndex: i];
            let frame: CGRect = msg_send![screen, frame];
            if mouse_location.x >= frame.origin.x 
                && mouse_location.x <= frame.origin.x + frame.size.width
                && mouse_location.y >= frame.origin.y 
                && mouse_location.y <= frame.origin.y + frame.size.height 
            {
                target_screen = screen;
                break;
            }
        }
        
        let screen_frame: CGRect = msg_send![target_screen, frame];
        let window_frame: CGRect = msg_send![ns_window, frame];
        
        let x = screen_frame.origin.x + (screen_frame.size.width - window_frame.size.width) / 2.0;
        let y = screen_frame.origin.y + (screen_frame.size.height - window_frame.size.height) / 2.0;
        
        let _: () = msg_send![ns_window, setFrameOrigin: NSPoint::new(x, y)];
    }
}
```

This is complex objc2 code. Let me note that each call needs `MainThreadMarker` since NSWindow operations must be on the main thread.

**Step 6: Verify**

```bash
cargo run -- daemon &
# Enter full-screen app
# Press Cmd+Option+V → window appears over full-screen, centered on cursor
# Select entry → window hides, paste happens, focus returns
```

**Step 7: Commit**

```bash
git add -A && git commit -m "feat: floating window over full-screen, center on cursor screen, focus return"
```

---

## Task 6: macOS .app Bundle for Consistent Permissions

**Why:** Each time `brew upgrade` replaces the binary, its path changes → macOS TCC sees a "new" app and asks for permissions again. Old paths clutter the permissions list. Wrapping in a `.app` bundle with a consistent `CFBundleIdentifier` solves this: TCC tracks by bundle ID, not binary path.

**Files:**
- Create: `scripts/build_app_bundle.sh`
- Create: `macos/Info.plist`
- Modify: `Cargo.toml` (add build.rs or script instructions)
- Modify: `CHANGELOG.md`
- Modify: (upstream) Homebrew formula to install `.app` bundle

**Step 1: Create `macos/Info.plist`**

```xml
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
    <string>0.1.2</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.2</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>LSUIElement</key>
    <true/>
    <key>NSAccessibilityUsageDescription</key>
    <string>Clipboard History needs accessibility access to simulate Cmd+V for pasting clipboard entries into your active application.</string>
</dict>
</plist>
```

Key items:
- `CFBundleIdentifier: com.jdawnduan.clipboard-history` — the stable identity
- `LSUIElement: true` — no dock icon (equivalent to current `NSAccessory` policy)

**Step 2: Create `scripts/build_app_bundle.sh`**

```bash
#!/bin/bash
# Creates macOS .app bundle wrapping the clipboard-history binary.
# Run after `cargo build --release`.

set -euo pipefail

BINARY="target/release/clipboard-history"
APP_NAME="Clipboard History.app"
APP_DIR="$APP_NAME/Contents"
MACOS_DIR="$APP_DIR/MacOS"

mkdir -p "$MACOS_DIR"
cp "$BINARY" "$MACOS_DIR/clipboard-history"
cp macos/Info.plist "$APP_DIR/Info.plist"

# Optionally codesign:
# codesign --force --sign - "$APP_NAME"

echo "Bundle created: $APP_NAME"
echo "Binary inside: $MACOS_DIR/clipboard-history"
```

**Step 3: Update Homebrew formula steps** (documentation, not code in this repo)

The brew formula (`jdawnduan/tap/clipboard-history`) needs to:
- `cargo install --path .` → binary goes to `$CARGO_HOME/bin`
- Then wrap it: create `.app` bundle at `#{prefix}/Clipboard History.app`
- Symlink or point `brew services` at the binary inside the bundle
- OR: install the .app bundle to `/Applications/` and have the brew service launch the binary inside it

```
def install
  system "cargo", "install", *std_cargo_args
  
  # Create .app bundle
  app_dir = prefix/"Clipboard History.app/Contents"
  macos_dir = app_dir/"MacOS"
  macos_dir.mkpath
  
  cp bin/"clipboard-history", macos_dir/"clipboard-history"
  
  # Write Info.plist
  (app_dir/"Info.plist").write <<~EOS
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC ...>
    <plist version="1.0">
      <dict>
        <key>CFBundleIdentifier</key>
        <string>com.jdawnduan.clipboard-history</string>
        ...
      </dict>
    </plist>
  EOS
  
  # Use the bundle's binary for the service
  bin.install_symlink macos_dir/"clipboard-history" => "clipboard-history"
end
```

The key: `brew services` should still work because the binary is symlinked to somewhere brew knows. The `.app` bundle exists alongside it. When macOS prompts for accessibility, it'll record `com.jdawnduan.clipboard-history`, and future upgrades with the same bundle ID won't re-prompt.

**Step 4: Verify**

```bash
cargo build --release
./scripts/build_app_bundle.sh
open "Clipboard History.app"  # Should start daemon, no dock icon
# Check permissions: System Settings > Privacy & Security > Accessibility
# Should show "Clipboard History" with a consistent entry
```

**Step 5: Document in CHANGELOG.md**

```markdown
## [0.2.0] - [date]

### Added
- macOS .app bundle with consistent bundle identifier (`com.jdawnduan.clipboard-history`).
  Permissions now survive upgrades — grant accessibility once, never again.

### Fixed
- Floating panel mode: clipboard popup now appears over full-screen apps.
- CJK character rendering: Chinese, Japanese, and Korean text now displays correctly in the popup.

### Changed
- Performance improvements: in-memory clipboard history, event-driven hotkey, and async
  disk writes reduce latency across all operations.
```

**Step 6: Bump version in Cargo.toml to 0.2.0**

**Step 7: Commit**

```bash
git add -A && git commit -m "feat: macOS .app bundle with consistent bundle ID for TCC permissions"
```

---

## Measurement & Tuning (Optional Follow-Up)

After Tasks 1-3 are done, TIME AND PROFILE the remaining slowness:

```bash
# Add timestamp logging to paste_entry():
fn paste_entry(&mut self, index: usize, ctx: &egui::Context) {
    let t0 = std::time::Instant::now();
    // ... step through key phases
    let t1 = std::time::Instant::now();
    println!("Phase C-D: set_clipboard: {:?}", t1 - t0);
    // ... simulate paste
    let t2 = std::time::Instant::now();
    println!("Phase D: cmd+v simulation: {:?}", t2 - t1);
}
```

If C→D (key press → paste completes) is still slow, implement **Option C from discussion**: pre-set clipboard content when the window opens. Then when user selects, only the Cmd+V simulation runs — the clipboard is already populated.

---

## Task Order Summary

| # | Task | Est. Time | Dependencies |
|---|---|---|---|
| 1 | In-memory history | 15 min | None |
| 2 | Event-driven hotkey | 15 min | None |
| 3 | Async disk writes | 20 min | Task 1 (shared history type) |
| 4 | CJK font rendering | 10 min | None |
| 5 | Floating panel over full-screen | 45 min | None (macOS-specific) |
| 6 | .app bundle | 30 min | None (standalone) |

**Recommended order:** 1 → 2 → 3 → 4 → 5 → 6

Tasks 1-3 can be batched (speed improvements), then 4 and 5 (UI fixes), then 6 (build/deploy).

---

## Execution Options

**Plan complete. Two execution options:**

**1. Subagent-Driven (this session)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Parallel Session (separate)** — Open a new session with `executing-plans`, batch execution with checkpoints.

**Which approach?**
