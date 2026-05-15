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
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use enigo::{Enigo, Settings, Key, Direction};

const MAX_HISTORY_SIZE: usize = 10;
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
            platform::acquire_single_instance()?;
            println!("Starting clipboard history daemon...");
            println!("Max entries: {}, Max entry size: {} KB", MAX_HISTORY_SIZE, MAX_ENTRY_BYTES / 1024);
            println!("Press Cmd+Option+v to show clipboard history popup");
            let result = run_daemon_with_hotkey();
            let _ = platform::release_single_instance();
            result?;
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

    // Shared flag to ignore clipboard changes triggered by the app itself
    let skip_next_monitor = Arc::new(AtomicBool::new(false));
    let skip_next_monitor_clone = skip_next_monitor.clone();

    // Register Cmd+Option+V
    let hotkey = HotKey::new(Some(Modifiers::META | Modifiers::ALT), Code::KeyV);
    let hotkey_id = hotkey.id();
    manager.register(hotkey)?;

    println!("Registered hotkey: Cmd+Option+v (id: {})", hotkey_id);

    // Create shared clipboard history (in-memory, no disk read on hotkey)
    let history = ClipboardHistory::new_shared();
    let monitor_history = history.clone();

    // Channel for async disk writes — monitor sends save signal, saver thread writes
    let (save_tx, save_rx) = std::sync::mpsc::channel::<()>();
    let saver_history = history.clone();
    std::thread::spawn(move || {
        // Background saver: receives save signals, debounces, writes to disk
        while save_rx.recv().is_ok() {
            // Drain any queued signals so rapid changes coalesce into one save
            while save_rx.try_recv().is_ok() {}
            // Brief debounce to avoid thrashing on rapid clipboard changes
            std::thread::sleep(Duration::from_millis(50));
            if let Ok(history) = saver_history.lock() {
                if let Err(e) = history.save() {
                    eprintln!("Failed to save history: {}", e);
                }
            }
        }
    });

    // Spawn clipboard monitoring thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = monitor_clipboard(skip_next_monitor_clone, monitor_history, save_tx).await {
                eprintln!("Clipboard monitor error: {}", e);
            }
        });
    });

    // Event-driven hotkey listener — blocking recv() instead of polling every 100ms
    let hotkey_triggered = Arc::new(AtomicBool::new(false));
    let hotkey_triggered_clone = hotkey_triggered.clone();
    let ctx_holder: Arc<Mutex<Option<egui::Context>>> = Arc::new(Mutex::new(None));
    let ctx_holder_clone = ctx_holder.clone();

    std::thread::spawn(move || {
        loop {
            if let Ok(event) = GlobalHotKeyEvent::receiver().recv() {
                if event.id == hotkey_id {
                    hotkey_triggered_clone.store(true, Ordering::SeqCst);
                    // Wake the eframe event loop immediately
                    if let Ok(ctx_opt) = ctx_holder_clone.lock() {
                        if let Some(ctx) = ctx_opt.as_ref() {
                            ctx.request_repaint();
                        }
                    }
                }
            }
        }
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
            Ok(Box::new(DaemonApp::new(skip_next_monitor, history, hotkey_triggered, ctx_holder)))
        }),
    ).map_err(|e| format!("Failed to run daemon: {}", e))?;

    Ok(())
}

struct DaemonApp {
    popup_open: bool,
    entries: Vec<String>,
    enigo: Enigo,
    skip_next_monitor: Arc<AtomicBool>,
    history: history::SharedClipboardHistory,
    hotkey_triggered: Arc<AtomicBool>,
    ctx_holder: Arc<Mutex<Option<egui::Context>>>,
}

impl DaemonApp {
    fn new(skip_next_monitor: Arc<AtomicBool>, history: history::SharedClipboardHistory, hotkey_triggered: Arc<AtomicBool>, ctx_holder: Arc<Mutex<Option<egui::Context>>>) -> Self {
        Self {
            popup_open: false,
            entries: Vec::new(),
            enigo: Enigo::new(&Settings::default()).unwrap(),
            skip_next_monitor,
            history,
            hotkey_triggered,
            ctx_holder,
        }
    }

    fn paste_entry(&mut self, index: usize, ctx: &egui::Context) {
        if let Some(content) = self.entries.get(index) {
            // Signal monitor to ignore this change
            self.skip_next_monitor.store(true, Ordering::SeqCst);
            
            if let Err(e) = clipboard::set_clipboard(content) {
                eprintln!("Failed to set clipboard: {}", e);
                self.skip_next_monitor.store(false, Ordering::SeqCst);
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
                    // Reduced delay to 6ms
                    std::thread::sleep(Duration::from_millis(6));
                    
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
                    std::thread::sleep(Duration::from_millis(20));
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
        // Set up CJK font fallbacks on first frame
        // Loads PingFang.ttc from macOS system fonts so CJK characters render correctly.
        // Must insert font data before referencing the name in families, or egui panics.
        {
            static FONTS_SET: std::sync::Once = std::sync::Once::new();
            FONTS_SET.call_once(|| {
                let mut fonts = egui::FontDefinitions::default();

                #[cfg(target_os = "macos")]
                {
                    // Try loading CJK fonts from macOS system paths
                    // On macOS 24+, PingFang.ttc moved to a PrivateFramework path (SIP-protected);
                    // epaint only scans standard directories so we must use fonts accessible there.
                    let cjk_candidates = [
                        "/System/Library/Fonts/STHeiti Medium.ttc",
                        "/System/Library/Fonts/Supplemental/Songti.ttc",
                    ];

                    for path in &cjk_candidates {
                        if let Ok(data) = std::fs::read(path) {
                            let label = path.rsplit('/').next().unwrap_or("CJK");
                            fonts.font_data
                                .insert(label.to_owned(), egui::FontData::from_owned(data));
                            if let Some(family) =
                                fonts.families.get_mut(&egui::FontFamily::Proportional)
                            {
                                family.insert(0, label.to_owned());
                            }
                            break;
                        }
                    }
                }

                ctx.set_fonts(fonts);
            });
        }

        // Continuously hide the native window while the popup is closed.
        // eframe's with_visible(false) doesn't always suppress the window on macOS,
        // and the first frame might fire before the window exists in [NSApp windows].
        // By hiding every frame until the popup opens, we catch it whenever it appears.
        if !self.popup_open {
            #[cfg(target_os = "macos")]
            platform::hide_main_window();
        }

        // Store egui context so the hotkey listener thread can wake us immediately
        {
            let mut ctx_opt = self.ctx_holder.lock().unwrap();
            if ctx_opt.is_none() {
                *ctx_opt = Some(ctx.clone());
            }
        }

        // Check for hotkey events (set by dedicated listener thread)
        if self.hotkey_triggered.swap(false, Ordering::SeqCst) && !self.popup_open {
            println!("Hotkey pressed! Showing clipboard history...");

            // Load history and show popup (in-memory, no disk I/O)
            if let Ok(history) = self.history.lock() {
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

                    // macOS-specific: unhide the app (in case it was hidden),
                    // then elevate window above full-screen, center on cursor
                    #[cfg(target_os = "macos")]
                    {
                        platform::unhide_app();
                        crate::platform::macos::setup_popup_window();
                    }

                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
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
                    ui.label(
                        egui::RichText::new("📋 Clipboard History")
                            .size(22.0)
                            .text_style(egui::TextStyle::Body),
                    );
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

        // Only request repaints while popup is open (hotkey thread wakes us otherwise)
        if self.popup_open {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }
}

async fn monitor_clipboard(skip_next: Arc<AtomicBool>, shared_history: history::SharedClipboardHistory, save_tx: std::sync::mpsc::Sender<()>) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_content: Option<String> = None;

    loop {
        if let Ok(content) = clipboard::get_clipboard() {
            let is_new = last_content.as_ref() != Some(&content);

            if is_new {
                if skip_next.swap(false, Ordering::SeqCst) {
                    last_content = Some(content);
                } else {
                    let is_valid_size = content.len() <= MAX_ENTRY_BYTES;

                    if is_valid_size && !content.is_empty() {
                        println!(
                            "New clipboard entry: {} bytes - {}",
                            content.len(),
                            truncate_preview(&content, 40)
                        );

                        // Update in-memory history (fast, no I/O)
                        if let Ok(mut history) = shared_history.lock() {
                            history.add(content.clone(), MAX_HISTORY_SIZE);
                        }
                        // Signal background saver to write to disk (non-blocking)
                        let _ = save_tx.send(());
                        last_content = Some(content);
                    } else if !is_valid_size {
                        println!(
                            "Skipped entry: {} bytes exceeds {} KB limit",
                            content.len(),
                            MAX_ENTRY_BYTES / 1024
                        );
                        last_content = Some(content);
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

fn truncate_preview(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', "⏎").replace('\r', "");
    if s.chars().count() > max_len {
        format!("{}...", s.chars().take(max_len).collect::<String>())
    } else {
        s
    }
}