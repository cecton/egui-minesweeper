// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example]
fn run() {
    use eframe::egui;
    use egui_minesweeper::{GameStatus, MinesweeperGame, MinesweeperWidget};
    use xtask_wasm::wasm_bindgen::JsCast as _;

    #[derive(Clone, Copy, PartialEq)]
    enum Preset {
        Beginner,
        Intermediate,
        Expert,
    }

    impl Preset {
        const ALL: &'static [Preset] = &[Self::Beginner, Self::Intermediate, Self::Expert];

        fn label(self) -> &'static str {
            match self {
                Self::Beginner => "Beginner (9×9, 10 mines)",
                Self::Intermediate => "Intermediate (16×16, 40 mines)",
                Self::Expert => "Expert (30×16, 99 mines)",
            }
        }

        fn dims(self) -> (usize, usize, usize) {
            match self {
                Self::Beginner => (9, 9, 10),
                Self::Intermediate => (16, 16, 40),
                Self::Expert => (30, 16, 99),
            }
        }
    }

    struct MinesweeperApp {
        game: MinesweeperGame,
        selected_preset: Preset,
        question_marks: bool,
    }

    impl Default for MinesweeperApp {
        fn default() -> Self {
            Self {
                game: MinesweeperGame::new(9, 9, 10),
                selected_preset: Preset::Beginner,
                question_marks: true,
            }
        }
    }

    impl eframe::App for MinesweeperApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let bg = ui.max_rect();
            ui.painter()
                .rect_filled(bg, egui::CornerRadius::ZERO, ui.visuals().panel_fill);

            let flags = self.game.flags_placed();
            let remaining = (self.game.mines as isize) - (flags as isize);

            egui::Panel::top("top_bar")
                .frame(egui::Frame::new().inner_margin(4.0))
                .show_inside(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.visuals_mut().button_frame = false;
                        ui.add_space(8.0);
                        egui::widgets::global_theme_preference_switch(ui);
                        ui.toggle_value(&mut self.question_marks, "❓");
                        ui.separator();
                        for &preset in Preset::ALL {
                            if ui
                                .selectable_label(self.selected_preset == preset, preset.label())
                                .clicked()
                            {
                                self.selected_preset = preset;
                                let (w, h, m) = preset.dims();
                                self.game = MinesweeperGame::new(w, h, m);
                            }
                        }
                        ui.separator();
                        ui.label(format!("🚩 {flags}  💣 {remaining}"));
                        match self.game.status {
                            GameStatus::Won => {
                                ui.colored_label(egui::Color32::GREEN, "You won!");
                            }
                            GameStatus::Lost => {
                                ui.colored_label(egui::Color32::RED, "Boom!");
                            }
                            GameStatus::Playing => {}
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("New Game").clicked() {
                                self.game.reset();
                            }
                        });
                    });
                });

            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add(MinesweeperWidget::new(&mut self.game).question_marks(self.question_marks));
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
