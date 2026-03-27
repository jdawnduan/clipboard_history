//! macOS-specific functionality
//! 
//! Future enhancements:
//! - Native menu bar integration
//! - Keyboard shortcuts via Accessibility API
//! - LaunchAgent for auto-start

#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
#[cfg(target_os = "macos")]
use objc2_foundation::MainThreadMarker;
#[cfg(target_os = "macos")]
use macos_accessibility_client::accessibility;
#[cfg(target_os = "macos")]
use objc2::msg_send;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // Hide application from Dock and Cmd-Tab
    #[cfg(target_os = "macos")]
    {
        if let Some(mtm) = MainThreadMarker::new() {
            let app = NSApplication::sharedApplication(mtm);
            app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        }

        if !check_accessibility_permissions() {
            println!("WARNING: Accessibility permissions not granted. Auto-paste will not work.");
            println!("Please grant Accessibility permissions to this app in System Settings > Privacy & Security > Accessibility.");
            // Request permissions (this will trigger a system dialog if not already granted)
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
            unsafe { app.deactivate(); }
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
