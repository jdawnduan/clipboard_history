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
            unsafe {
                let nil: *mut objc2::runtime::AnyObject = std::ptr::null_mut();
                let _: () = msg_send![&app, hide: nil];
            }
        }

        if !check_accessibility_permissions() {
            println!("WARNING: Accessibility permissions not granted. Auto-paste will not work.");
            println!("Please grant Accessibility permissions to this app in System Settings > Privacy & Security > Accessibility.");
            accessibility::application_is_trusted_with_prompt();
        }
    }

    Ok(())
}

/// Deactivate the application so focus returns to the previous one
pub fn deactivate_app() {
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            // hide: method on NSApplication
            unsafe {
                let _: () = msg_send![&app, hide: 0 as *mut objc2::runtime::AnyObject];
            }
            // Also try to explicitly deactivate
            unsafe {
                app.deactivate();
            }
        }
    }
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
