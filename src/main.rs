mod clipboard;
mod history;
mod platform;

use clap::{Parser, Subcommand};
use eframe::egui;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use history::ClipboardHistory;
use std::time::Duration;
use enigo::{Enigo, Settings, Key, Direction};

const MAX_HISTORY_SIZE: usize = 20;
const MAX_ENTRY_BYTES: usize = 128 * 1024; // 128 KB
const POLL_INTERVAL_MS: u64 = 500;

#[derive(Parser)]
#[command(name = "clipboard-history")]
#[command(about = "A clipboard history manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the clipboard monitoring daemon with hotkey support
    Daemon,
    /// List clipboard history
    List {
        /// Number of entries to show (default: all)
        #[arg(short, long)]
        count: Option<usize>,
    },
    /// Get a specific entry by index (0 = most recent)
    Get {
        /// Index of the entry
        index: usize,
    },
    /// Copy a history entry back to clipboard
    Paste {
        /// Index of the entry to paste
        index: usize,
    },
    /// Clear all history
    Clear,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize platform-specific features
    platform::init()?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => {
            println!("Starting clipboard history daemon...");
            println!("Max entries: {}, Max entry size: {} KB", MAX_HISTORY_SIZE, MAX_ENTRY_BYTES / 1024);
            println!("Press Cmd+Option+V to show clipboard history popup");
            run_daemon_with_hotkey()?;
        }
        Commands::List { count } => {
            let history = ClipboardHistory::load()?;
            let entries = history.entries();
            let count = count.unwrap_or(entries.len());

            if entries.is_empty() {
                println!("No clipboard history.");
            } else {
                for (i, entry) in entries.iter().take(count).enumerate() {
                    let preview = truncate_preview(&entry.content, 60);
                    let size = entry.content.len();
                    println!(
                        "[{}] {} ({} bytes) - {}",
                        i,
                        entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                        size,
                        preview
                    );
                }
            }
        }
        Commands::Get { index } => {
            let history = ClipboardHistory::load()?;
            match history.get(index) {
                Some(entry) => println!("{}", entry.content),
                None => eprintln!("No entry at index {}", index),
            }
        }
        Commands::Paste { index } => {
            let history = ClipboardHistory::load()?;
            match history.get(index) {
                Some(entry) => {
                    clipboard::set_clipboard(&entry.content)?;
                    println!("Copied entry {} to clipboard", index);
                }
                None => eprintln!("No entry at index {}", index),
            }
        }
        Commands::Clear => {
            let mut history = ClipboardHistory::load()?;
            history.clear();
            history.save()?;
            println!("Clipboard history cleared.");
        }
    }

    Ok(())
}


fn run_daemon_with_hotkey() -> Result<(), Box<dyn std::error::Error>> {
    // Create hotkey manager - must be created on main thread for macOS
    let manager = GlobalHotKeyManager::new()?;

    // Register Cmd+Option+V
    let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::ALT), Code::KeyV);
    let hotkey_id = hotkey.id();
    manager.register(hotkey)?;

    println!("Registered hotkey: Cmd+Option+V (id: {})", hotkey_id);

    // Spawn clipboard monitoring thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = monitor_clipboard().await {
                eprintln!("Clipboard monitor error: {}", e);
            }
        });
    });

    // Use eframe to create a hidden window that provides the event loop
    // This is required on macOS for global hotkeys to work
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_always_on_top()
            .with_decorations(true)
            .with_title("Clipboard History")
            .with_active(false)
            .with_visible(false),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Clipboard History Daemon",
        options,
        Box::new(move |_cc| {
            Ok(Box::new(DaemonApp::new(hotkey_id)))
        }),
    ).map_err(|e| format!("Failed to run daemon: {}", e))?;

    Ok(())
}

struct DaemonApp {
    hotkey_id: u32,
    popup_open: bool,
    entries: Vec<String>,
    enigo: Enigo,
}

impl DaemonApp {
    fn new(hotkey_id: u32) -> Self {
        Self {
            hotkey_id,
            popup_open: false,
            entries: Vec::new(),
            enigo: Enigo::new(&Settings::default()).unwrap(),
        }
    }

    fn paste_entry(&mut self, index: usize, ctx: &egui::Context) {
        if let Some(content) = self.entries.get(index) {
            if let Err(e) = clipboard::set_clipboard(content) {
                eprintln!("Failed to set clipboard: {}", e);
            } else {
                println!("Pasted entry {}", index);
                self.popup_open = false;
                // Hide window immediately before simulating paste
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus); // Might help trigger app deactivation
                
                // On macOS, Cmd+V
                #[cfg(target_os = "macos")]
                {
                    use enigo::Keyboard;
                    // Force the app to deactivate to help window switching
                    platform::deactivate_app();

                    // Small delay to ensure focus has returned to the previous window
                    // Reduced delay to 12ms
                    std::thread::sleep(Duration::from_millis(12));
                    
                    println!("Simulating Cmd+V using enigo Meta+V...");
                    // Release all modifiers first in case they are stuck
                    let _ = self.enigo.key(Key::Alt, Direction::Release);
                    let _ = self.enigo.key(Key::Meta, Direction::Release);
                    let _ = self.enigo.key(Key::Shift, Direction::Release);
                    let _ = self.enigo.key(Key::Control, Direction::Release);
                    std::thread::sleep(Duration::from_millis(1));

                    let _ = self.enigo.key(Key::Meta, Direction::Press);
                    std::thread::sleep(Duration::from_millis(2));
                    let _ = self.enigo.key(Key::Unicode('v'), Direction::Press);
                    std::thread::sleep(Duration::from_millis(2));
                    let _ = self.enigo.key(Key::Unicode('v'), Direction::Release);
                    std::thread::sleep(Duration::from_millis(2));
                    let _ = self.enigo.key(Key::Meta, Direction::Release);
                    println!("Simulated Cmd+V complete");
                }
                
                // On Linux/Windows, Ctrl+V
                #[cfg(not(target_os = "macos"))]
                {
                    std::thread::sleep(Duration::from_millis(37));
                    let _ = self.enigo.key(Key::Control, Direction::Press);
                    let _ = self.enigo.key(Key::Unicode('v'), Direction::Click);
                    let _ = self.enigo.key(Key::Control, Direction::Release);
                }
            }
        }
    }

    fn truncate_display(s: &str, max_len: usize) -> String {
        let s = s.replace('\n', "⏎").replace('\r', "").replace('\t', "→");
        if s.chars().count() > max_len {
            format!("{}...", s.chars().take(max_len).collect::<String>())
        } else {
            s
        }
    }
}

impl eframe::App for DaemonApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ensure window starts hidden on first frame
        #[cfg(target_os = "macos")]
        {
            static ONCE: std::sync::Once = std::sync::Once::new();
            ONCE.call_once(|| {
                if !self.popup_open {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
            });
        }

        // Check for hotkey events
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.hotkey_id && !self.popup_open {
                println!("Hotkey pressed! Showing clipboard history...");

                // Load history and show popup
                if let Ok(history) = ClipboardHistory::load() {
                    self.entries = history
                        .entries()
                        .iter()
                        .take(10)
                        .map(|e| e.content.clone())
                        .collect();

                    if self.entries.is_empty() {
                        println!("No clipboard history to show.");
                    } else {
                        self.popup_open = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    }
                }
            }
        }

        if self.popup_open {
            // Handle keyboard input for number selection
            let mut selected_index: Option<usize> = None;

            ctx.input(|i| {
                // Check for number keys 1-9 and 0
                for (key, index) in [
                    (egui::Key::Num1, 0),
                    (egui::Key::Num2, 1),
                    (egui::Key::Num3, 2),
                    (egui::Key::Num4, 3),
                    (egui::Key::Num5, 4),
                    (egui::Key::Num6, 5),
                    (egui::Key::Num7, 6),
                    (egui::Key::Num8, 7),
                    (egui::Key::Num9, 8),
                    (egui::Key::Num0, 9),
                ] {
                    if i.key_pressed(key) && index < self.entries.len() {
                        selected_index = Some(index);
                    }
                }

                // Escape to close
                if i.key_pressed(egui::Key::Escape) {
                    self.popup_open = false;
                }
            });

            if !self.popup_open {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("📋 Clipboard History");
                    ui.add_space(5.0);
                    ui.label("Press 1-9 (0 for 10) to select and paste, Esc to close");
                    ui.add_space(10.0);
                    ui.separator();

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (i, entry) in self.entries.iter().enumerate() {
                            let display_num = if i == 9 { 0 } else { i + 1 };
                            let preview = Self::truncate_display(entry, 70);

                            let label = format!("[{}] {}", display_num, preview);

                            let response = ui.selectable_label(false, &label);

                            if response.clicked() || response.double_clicked() {
                                selected_index = Some(i);
                            }
                        }
                    });
                });

                // Handle selection (from keyboard or click)
                if let Some(idx) = selected_index {
                    self.paste_entry(idx, ctx);
                }
            }
        }

        // Request continuous repainting to check for hotkey events
        ctx.request_repaint_after(Duration::from_millis(100));
    }
}

async fn monitor_clipboard() -> Result<(), Box<dyn std::error::Error>> {
    let mut history = ClipboardHistory::load()?;
    let mut last_content: Option<String> = None;

    loop {
        if let Ok(content) = clipboard::get_clipboard() {
            let is_new = last_content.as_ref() != Some(&content);
            let is_valid_size = content.len() <= MAX_ENTRY_BYTES;

            if is_new && is_valid_size && !content.is_empty() {
                println!(
                    "New clipboard entry: {} bytes - {}",
                    content.len(),
                    truncate_preview(&content, 40)
                );

                history.add(content.clone(), MAX_HISTORY_SIZE);
                history.save()?;
                last_content = Some(content);
            } else if is_new && !is_valid_size {
                println!(
                    "Skipped entry: {} bytes exceeds {} KB limit",
                    content.len(),
                    MAX_ENTRY_BYTES / 1024
                );
                last_content = Some(content);
            }
        }

        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

fn truncate_preview(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', "⏎").replace('\r', "");
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s
    }
}