//! macOS-specific functionality
//!
//! Future enhancements:
//! - Native menu bar integration
//! - LaunchAgent for auto-start

#[cfg(target_os = "macos")]
use macos_accessibility_client::accessibility;
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
#[cfg(target_os = "macos")]
use objc2_foundation::MainThreadMarker;

#[cfg(target_os = "macos")]
use std::fs;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::time::Duration;

#[cfg(target_os = "macos")]
fn lock_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_dir = dirs::data_local_dir().ok_or("Could not find local data directory")?;
    Ok(data_dir.join("clipboard-history").join("daemon.pid"))
}

#[cfg(target_os = "macos")]
pub fn acquire_single_instance() -> Result<bool, Box<dyn std::error::Error>> {
    let lock_path = lock_file_path()?;

    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if lock_path.exists() {
        if let Ok(pid_str) = fs::read_to_string(&lock_path) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                use std::process::Command;
                let output = Command::new("ps").args(["-p", &pid.to_string()]).output();

                if output.is_ok() && output.unwrap().status.success() {
                    println!("Found existing daemon with PID {}. Terminating it...", pid);
                    let kill_result = Command::new("kill")
                        .arg("-TERM".to_string())
                        .arg(pid.to_string())
                        .output();

                    if kill_result.is_err() {
                        eprintln!("Failed to send SIGTERM to PID {}", pid);
                    } else {
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            }
            let _ = fs::remove_file(&lock_path);
        }
    }

    let current_pid = std::process::id();
    fs::write(&lock_path, current_pid.to_string())?;
    println!(
        "Acquired lock file: {} (PID: {})",
        lock_path.display(),
        current_pid
    );

    Ok(true)
}

#[cfg(not(target_os = "macos"))]
pub fn acquire_single_instance() -> Result<bool, Box<dyn std::error::Error>> {
    Ok(true)
}

#[cfg(not(target_os = "macos"))]
pub fn release_single_instance() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn release_single_instance() -> Result<(), Box<dyn std::error::Error>> {
    let lock_path = lock_file_path()?;
    if lock_path.exists() {
        let _ = fs::remove_file(&lock_path);
    }
    Ok(())
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        }

        if !check_accessibility_permissions() {
            println!("WARNING: Accessibility permissions not granted. Auto-paste will not work.");
            println!("Please grant Accessibility permissions to this app in System Settings > Privacy & Security > Accessibility.");
            accessibility::application_is_trusted_with_prompt();
        }
    }

    Ok(())
}

/// Deactivate the application so focus returns to the previous one.
/// Calls hide: to let macOS switch focus to the previous app, then deactivates.
/// Paired with unhide_app() before showing the popup again.
pub fn deactivate_app() {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            unsafe {
                let _: () = msg_send![&app, hide: 0 as *mut objc2::runtime::AnyObject];
                app.deactivate();
            }
        }
    }
}

/// Unhide the app before showing the popup, so macOS doesn't re-hide our window.
/// Safe to call even if the app is not hidden.
/// NOTE: selector is `unhideWithoutActivation` (no colon, no argument).
pub fn unhide_app() {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            unsafe {
                let _: () = msg_send![&app, unhideWithoutActivation];
            }
        }
    }
}

/// Hide the eframe window directly via NSWindow orderOut:.
/// More reliable than egui's ViewportCommand::Visible(false) because it
/// acts on the native window immediately, before the frame is rendered.
/// Called on first frame to suppress the startup black window.
pub fn hide_main_window() {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            unsafe {
                let windows: *mut objc2::runtime::AnyObject = msg_send![&app, windows];
                let count: u64 = msg_send![windows, count];
                if count == 0 {
                    return;
                }
                let ns_window: *mut objc2::runtime::AnyObject = msg_send![windows, firstObject];
                if ns_window.is_null() {
                    return;
                }
                let _: () = msg_send![ns_window, setIsVisible: false];
            }
        }
    }
}

/// Elevate the single application window above full-screen content and center it
/// on the cursor's display. Called once each time the popup is shown.
pub fn setup_popup_window() {
    #[cfg(target_os = "macos")]
    unsafe {
        // Get the application's only window (the hidden eframe popup) via [NSApp windows]
        let mtm = match MainThreadMarker::new() {
            Some(mtm) => mtm,
            None => return,
        };
        let app = NSApplication::sharedApplication(mtm);
        let windows: *mut objc2::runtime::AnyObject = msg_send![&app, windows];
        let count: u64 = msg_send![windows, count];
        if count == 0 {
            return;
        }
        let ns_window: *mut objc2::runtime::AnyObject = msg_send![windows, firstObject];
        if ns_window.is_null() {
            return;
        }
        let ns_window = &*ns_window;

        // Use the shielding window level so we appear above full-screen apps
        let level = core_graphics::display::CGShieldingWindowLevel();
        let _: () = msg_send![ns_window, setLevel: level as i64];

        // Center on the display containing the cursor
        center_on_cursor_screen(ns_window);
    }
}

#[cfg(target_os = "macos")]
unsafe fn center_on_cursor_screen(ns_window: &objc2::runtime::AnyObject) {
    use objc2_foundation::{CGPoint, CGRect, CGFloat};

    // Get mouse location in screen coordinates via [NSEvent mouseLocation]
    let event_class = match objc2::runtime::AnyClass::get("NSEvent") {
        Some(cls) => cls,
        None => return,
    };
    let mouse_location: CGPoint = msg_send![event_class, mouseLocation];

    // Get all screens via [NSScreen screens]
    let screens_class = match objc2::runtime::AnyClass::get("NSScreen") {
        Some(cls) => cls,
        None => return,
    };
    let screens: *mut objc2::runtime::AnyObject = msg_send![screens_class, screens];
    let count: u64 = msg_send![screens, count];

    // Default to the window's current screen
    let mut target_screen: *mut objc2::runtime::AnyObject = msg_send![ns_window, screen];

    // Find which screen contains the cursor
    for i in 0..count {
        let screen: *mut objc2::runtime::AnyObject = msg_send![screens, objectAtIndex: i];
        let frame: CGRect = msg_send![screen, frame];

        if mouse_location.x >= frame.origin.x
            && mouse_location.x < frame.origin.x + frame.size.width
            && mouse_location.y >= frame.origin.y
            && mouse_location.y < frame.origin.y + frame.size.height
        {
            target_screen = screen;
            break;
        }
    }

    // Center the popup window on that screen
    let screen_frame: CGRect = msg_send![target_screen, frame];
    let window_frame: CGRect = msg_send![ns_window, frame];

    let x: CGFloat = screen_frame.origin.x
        + (screen_frame.size.width - window_frame.size.width) / 2.0;
    let y: CGFloat = screen_frame.origin.y
        + (screen_frame.size.height - window_frame.size.height) / 2.0;

    let _: () = msg_send![ns_window, setFrameOrigin: CGPoint::new(x, y)];
}

/// Check if the app has accessibility permissions
pub fn check_accessibility_permissions() -> bool {
    #[cfg(target_os = "macos")]
    {
        accessibility::application_is_trusted()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}
