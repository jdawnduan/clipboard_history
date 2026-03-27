#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

// Platform-specific initialization (for future expansion)
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    macos::init()?;

    #[cfg(target_os = "linux")]
    linux::init()?;

    Ok(())
}
