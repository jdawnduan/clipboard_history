use arboard::Clipboard;

pub fn get_clipboard() -> Result<String, Box<dyn std::error::Error>> {
    let mut clipboard = Clipboard::new()?;
    Ok(clipboard.get_text()?)
}

pub fn set_clipboard(content: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(content)?;
    Ok(())
}
