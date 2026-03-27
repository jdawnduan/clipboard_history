mod clipboard;
mod history;
mod platform;
mod popup;

use clap::{Parser, Subcommand};
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use history::ClipboardHistory;
use std::time::Duration;

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
            println!("Press Cmd+Shift+V to show clipboard history popup");
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
    // Create hotkey manager
    let manager = GlobalHotKeyManager::new()?;

    // Register Cmd+Shift+V (on macOS, Meta = Cmd)
    let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyV);
    let hotkey_id = hotkey.id();
    manager.register(hotkey)?;

    println!("Registered hotkey: Cmd+Shift+V (id: {})", hotkey_id);

    // Spawn clipboard monitoring thread
    let _monitor_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            monitor_clipboard().await.unwrap();
        });
    });

    // Main event loop for hotkey
    let hotkey_receiver = GlobalHotKeyEvent::receiver();

    loop {
        // Check for hotkey events
        if let Ok(event) = hotkey_receiver.try_recv() {
            if event.id == hotkey_id {
                println!("Hotkey pressed! Showing clipboard history...");

                // Load history and show popup
                if let Ok(history) = ClipboardHistory::load() {
                    let entries: Vec<String> = history
                        .entries()
                        .iter()
                        .take(10)
                        .map(|e| e.content.clone())
                        .collect();

                    if entries.is_empty() {
                        println!("No clipboard history to show.");
                    } else {
                        // Show popup in a new thread to not block the event loop
                        let entries_clone = entries.clone();
                        std::thread::spawn(move || {
                            popup::show_popup(entries_clone);
                        });
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(50));
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