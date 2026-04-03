#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    macos::init()?;

    #[cfg(target_os = "linux")]
    linux::init()?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn deactivate_app() {
    macos::deactivate_app();
}

#[cfg(not(target_os = "macos"))]
pub fn deactivate_app() {}

#[cfg(target_os = "macos")]
pub fn acquire_single_instance() -> Result<bool, Box<dyn std::error::Error>> {
    macos::acquire_single_instance()
}

#[cfg(not(target_os = "macos"))]
pub fn acquire_single_instance() -> Result<bool, Box<dyn std::error::Error>> {
    Ok(true)
}

#[cfg(target_os = "macos")]
pub fn release_single_instance() -> Result<(), Box<dyn std::error::Error>> {
    macos::release_single_instance()
}

#[cfg(not(target_os = "macos"))]
pub fn release_single_instance() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
