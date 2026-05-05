// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example(assets_dir = "assets")]
fn run() {
    use eframe::egui;
    use egui_minesweeper::{
        BoardCamera, GameStatus, InteractionMode, MinesweeperGame, MinesweeperWidget,
    };
    use xtask_wasm::wasm_bindgen::JsCast as _;

    struct MinesweeperApp {
        game: MinesweeperGame,
        presets: &'static [(&'static str, usize, usize, usize)],
        selected_preset: usize,
        selected_cell: Option<(usize, usize)>,
        camera: BoardCamera,
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
                selected_cell: None,
                camera: BoardCamera {
                    offset: egui::Vec2::ZERO,
                    zoom: 1.6,
                },
                dark_mode: false,
                theme_initialized: false,
            }
        }
    }

    impl eframe::App for MinesweeperApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            fn is_mobile(ui: &egui::Ui) -> bool {
                let screen = ui.ctx().content_rect();
                let width_small = screen.width() < 900.0;
                let touch = ui.ctx().input(|i| i.multi_touch().is_some());
                width_small || touch
            }

            fn reset_mobile_view(camera: &mut BoardCamera, selected: &mut Option<(usize, usize)>) {
                *selected = None;
                camera.offset = egui::Vec2::ZERO;
                camera.zoom = 1.6;
            }

            let mobile = is_mobile(ui);
            let screen = ui.ctx().content_rect();
            let landscape = screen.width() >= screen.height();

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
            });

            if !mobile {
                ui.vertical_centered(|ui| {
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
                return;
            }

            // Mobile UI
            ui.spacing_mut().interact_size.y = 44.0;
            let board_ui = |ui: &mut egui::Ui, app: &mut MinesweeperApp| {
                let widget = MinesweeperWidget::new(&mut app.game)
                    .cell_size(34.0)
                    .interaction_mode(InteractionMode::SelectOnly)
                    .selected_cell(&mut app.selected_cell)
                    .camera(&mut app.camera);
                ui.add(widget);
            };

            let menu_ui = |ui: &mut egui::Ui, app: &mut MinesweeperApp| {
                let flags = app.game.flags_placed();
                let remaining = (app.game.mines as isize) - (flags as isize);
                ui.label(format!("Flags: {flags}"));
                ui.label(format!("Mines remaining: {remaining}"));
                ui.add_space(4.0);

                egui::ComboBox::from_label("Preset")
                    .selected_text(app.presets[app.selected_preset].0)
                    .show_ui(ui, |ui| {
                        for (i, (label, w, h, m)) in app.presets.iter().enumerate() {
                            if ui
                                .selectable_label(app.selected_preset == i, *label)
                                .clicked()
                            {
                                app.selected_preset = i;
                                app.game = MinesweeperGame::new(*w, *h, *m);
                                reset_mobile_view(&mut app.camera, &mut app.selected_cell);
                            }
                        }
                    });

                if ui.button("New Game").clicked() {
                    app.game.reset();
                    reset_mobile_view(&mut app.camera, &mut app.selected_cell);
                }

                ui.checkbox(&mut app.dark_mode, "Dark mode");
                ui.add_space(6.0);

                let can_reveal = app.selected_cell.is_some();
                if ui
                    .add_enabled(can_reveal, egui::Button::new("Reveal"))
                    .clicked()
                {
                    if let Some((x, y)) = app.selected_cell {
                        app.game.reveal(x, y);
                    }
                }

                if ui.button("Flag/mark").clicked() {
                    if let Some((x, y)) = app.selected_cell {
                        app.game.cycle_flag(x, y);
                    }
                }

                ui.horizontal(|ui| {
                    if ui.button("Zoom -").clicked() {
                        app.camera.zoom = (app.camera.zoom * 0.9).clamp(0.5, 4.0);
                    }
                    if ui.button("Zoom +").clicked() {
                        app.camera.zoom = (app.camera.zoom * 1.1).clamp(0.5, 4.0);
                    }
                    if ui.button("Reset view").clicked() {
                        app.camera.offset = egui::Vec2::ZERO;
                        app.camera.zoom = 1.6;
                    }
                });

                ui.add_space(6.0);
                match app.game.status {
                    GameStatus::Won => {
                        ui.colored_label(
                            egui::Color32::GREEN,
                            "You won! Tap New Game to play again.",
                        );
                    }
                    GameStatus::Lost => {
                        ui.colored_label(egui::Color32::RED, "Boom! Tap New Game to try again.");
                    }
                    GameStatus::Playing => {}
                }
            };

            if landscape {
                ui.columns(2, |cols| {
                    cols[0].vertical(|ui| {
                        ui.set_min_width(ui.available_width());
                        board_ui(ui, self);
                    });
                    cols[1].vertical(|ui| {
                        menu_ui(ui, self);
                    });
                });
            } else {
                let total_h = ui.available_height();
                let board_h = (total_h * 0.62).max(220.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), board_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| board_ui(ui, self),
                );
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| menu_ui(ui, self));
            }
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
    body.style().set_property("touch-action", "none").unwrap();

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
