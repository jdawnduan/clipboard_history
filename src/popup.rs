use eframe::egui;

pub fn show_popup(entries: Vec<String>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([500.0, 400.0])
            .with_always_on_top()
            .with_decorations(true)
            .with_title("Clipboard History"),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Clipboard History",
        options,
        Box::new(|_cc| Ok(Box::new(ClipboardPopup::new(entries)))),
    );
}

struct ClipboardPopup {
    entries: Vec<String>,
    selected: Option<usize>,
}

impl ClipboardPopup {
    fn new(entries: Vec<String>) -> Self {
        Self {
            entries,
            selected: None,
        }
    }

    fn paste_entry(&self, index: usize) {
        if let Some(content) = self.entries.get(index) {
            if let Err(e) = crate::clipboard::set_clipboard(content) {
                eprintln!("Failed to set clipboard: {}", e);
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
        // Handle keyboard input for number selection
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
                    self.selected = Some(index);
                }
            }

            // Escape to close
            if i.key_pressed(egui::Key::Escape) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }

            // Enter to confirm selection
            if i.key_pressed(egui::Key::Enter) {
                if let Some(idx) = self.selected {
                    self.paste_entry(idx);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("📋 Clipboard History");
            ui.add_space(5.0);
            ui.label("Press 1-9 (0 for 10) to select, Enter to paste, Esc to close");
            ui.add_space(10.0);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, entry) in self.entries.iter().enumerate() {
                    let display_num = if i == 9 { 0 } else { i + 1 };
                    let preview = Self::truncate_display(entry, 70);
                    let is_selected = self.selected == Some(i);

                    let label = format!("[{}] {}", display_num, preview);

                    let response = ui.selectable_label(is_selected, &label);

                    if response.clicked() {
                        self.selected = Some(i);
                    }

                    if response.double_clicked() {
                        self.paste_entry(i);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            });
        });

        // If a number key was pressed, paste and close
        if let Some(idx) = self.selected.take() {
            self.paste_entry(idx);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}