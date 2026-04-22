// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example]
fn run() {
    use eframe::egui;
    use egui_minesweeper::{GameStatus, MinesweeperGame, MinesweeperWidget};
    use xtask_wasm::wasm_bindgen::JsCast as _;

    struct MinesweeperApp {
        game: MinesweeperGame,
        presets: &'static [(&'static str, usize, usize, usize)],
        selected_preset: usize,
        dark_mode: bool,
        theme_initialized: bool,
    }

    impl Default for MinesweeperApp {
        fn default() -> Self {
            Self {
                game: MinesweeperGame::new(9, 9, 10),
                presets: &[
                    ("Beginner (9×9, 10 mines)", 9, 9, 10),
                    ("Intermediate (16×16, 40 mines)", 16, 16, 40),
                    ("Expert (30×16, 99 mines)", 30, 16, 99),
                ],
                selected_preset: 0,
                dark_mode: false,
                theme_initialized: false,
            }
        }
    }

    impl eframe::App for MinesweeperApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            if !self.theme_initialized {
                // Use system/browser preference when available; fallback to dark.
                self.dark_mode = {
                    web_sys::window()
                        .and_then(|w| w.match_media("(prefers-color-scheme: dark)").ok().flatten())
                        .map(|m| m.matches())
                        .unwrap_or(true)
                };
                self.theme_initialized = true;
            }
            ui.ctx().set_visuals(if self.dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });

            let bg = ui.max_rect();
            ui.painter()
                .rect_filled(bg, egui::CornerRadius::ZERO, ui.visuals().panel_fill);

            ui.vertical_centered(|ui| {
                ui.heading("Minesweeper");
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    for (i, (label, w, h, m)) in self.presets.iter().enumerate() {
                        if ui
                            .selectable_label(self.selected_preset == i, *label)
                            .clicked()
                        {
                            self.selected_preset = i;
                            self.game = MinesweeperGame::new(*w, *h, *m);
                        }
                    }
                });

                ui.add_space(6.0);

                let flags = self.game.flags_placed();
                let remaining = (self.game.mines as isize) - (flags as isize);
                ui.horizontal(|ui| {
                    ui.label(format!("Flags: {flags}  |  Mines remaining: {remaining}"));
                    ui.checkbox(&mut self.dark_mode, "Dark mode");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("New Game").clicked() {
                            self.game.reset();
                        }
                    });
                });

                ui.add_space(4.0);

                match self.game.status {
                    GameStatus::Won => {
                        ui.colored_label(
                            egui::Color32::GREEN,
                            "You won! Click 'New Game' to play again.",
                        );
                    }
                    GameStatus::Lost => {
                        ui.colored_label(
                            egui::Color32::RED,
                            "Boom! Click 'New Game' to try again.",
                        );
                    }
                    GameStatus::Playing => {}
                }

                ui.add_space(4.0);

                egui::ScrollArea::both().show(ui, |ui| {
                    ui.add(MinesweeperWidget::new(&mut self.game).cell_size(34.0));
                });
            });
        }
    }

    // Create a full-screen canvas and attach it to the page body.
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let canvas = document
        .create_element("canvas")
        .expect("failed to create canvas")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("not a HtmlCanvasElement");

    let style = canvas.style();
    style.set_property("position", "fixed").unwrap();
    style.set_property("top", "0").unwrap();
    style.set_property("left", "0").unwrap();
    style.set_property("width", "100%").unwrap();
    style.set_property("height", "100%").unwrap();

    let body = document.body().expect("no body");
    body.style().set_property("margin", "0").unwrap();
    body.append_child(&canvas).expect("failed to append canvas");

    // Start the eframe web runner on that canvas element.
    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|_cc| Ok(Box::new(MinesweeperApp::default()))),
            )
            .await
            .expect("failed to start eframe");
    });
}
