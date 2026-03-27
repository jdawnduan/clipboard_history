//! Linux-specific functionality
//!
//! Supports both X11 and Wayland via arboard
//! 
//! Future enhancements:
//! - D-Bus integration for system tray
//! - Systemd user service for auto-start

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // Check for display server availability
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        println!("Using Wayland clipboard");
    } else if std::env::var("DISPLAY").is_ok() {
        println!("Using X11 clipboard");
    } else {
        return Err("No display server found (X11 or Wayland required)".into());
    }
    Ok(())
}
