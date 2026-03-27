use eframe::egui;

pub fn show_popup(entries: Vec<String>) {
    println!("Opening popup with {} entries...", entries.len());

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_always_on_top()
            .with_decorations(true)
            .with_title("Clipboard History")
            .with_active(true),
        centered: true,
        ..Default::default()
    };

    match eframe::run_native(
        "Clipboard History",
        options,
        Box::new(|_cc| Ok(Box::new(ClipboardPopup::new(entries)))),
    ) {
        Ok(_) => println!("Popup closed normally"),
        Err(e) => eprintln!("Popup error: {}", e),
    }
}

struct ClipboardPopup {
    entries: Vec<String>,
    should_close: bool,
}

impl ClipboardPopup {
    fn new(entries: Vec<String>) -> Self {
        Self {
            entries,
            should_close: false,
        }
    }

    fn paste_entry(&mut self, index: usize) {
        if let Some(content) = self.entries.get(index) {
            if let Err(e) = crate::clipboard::set_clipboard(content) {
                eprintln!("Failed to set clipboard: {}", e);
            } else {
                println!("Pasted entry {}", index);
                self.should_close = true;
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

impl eframe::App for ClipboardPopup {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle close request
        if self.should_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

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
                self.should_close = true;
            }
        });

        // Process selection after input handling
        if let Some(idx) = selected_index {
            self.paste_entry(idx);
        }

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

        // Handle click selection
        if let Some(idx) = selected_index {
            self.paste_entry(idx);
        }
    }
}