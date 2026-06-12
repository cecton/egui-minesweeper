// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example(assets_dir = "assets")]
fn run() {
    use eframe::egui;
    use egui_minesweeper::{
        BoardCamera, CellState, GameStatus, InteractionMode, MinesweeperGame, MinesweeperWidget,
    };
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

        fn short_label(self) -> &'static str {
            match self {
                Self::Beginner => "Beginner",
                Self::Intermediate => "Intermediate",
                Self::Expert => "Expert",
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
        selected_cell: Option<(usize, usize)>,
        camera: BoardCamera,
        mobile_refit_pending: bool,
    }

    impl Default for MinesweeperApp {
        fn default() -> Self {
            Self {
                game: MinesweeperGame::new(9, 9, 10),
                selected_preset: Preset::Beginner,
                question_marks: true,
                selected_cell: None,
                camera: BoardCamera {
                    offset: egui::Vec2::ZERO,
                    zoom: 1.0,
                },
                mobile_refit_pending: true,
            }
        }
    }

    impl eframe::App for MinesweeperApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let bg = ui.max_rect();
            ui.painter()
                .rect_filled(bg, egui::CornerRadius::ZERO, ui.visuals().panel_fill);

            if Self::is_mobile(ui) {
                self.mobile_ui(ui);
            } else {
                self.desktop_ui(ui);
            }
        }
    }

    impl MinesweeperApp {
        const MOBILE_CELL_SIZE: f32 = 34.0;

        fn is_mobile(ui: &egui::Ui) -> bool {
            let screen = ui.ctx().content_rect();
            let width_small = screen.width() < 900.0;
            let touch = ui.ctx().input(|i| i.multi_touch().is_some());
            width_small || touch
        }

        fn reset_mobile_view(&mut self) {
            self.selected_cell = None;
            self.camera.offset = egui::Vec2::ZERO;
            self.camera.zoom = 1.0;
            self.mobile_refit_pending = true;
        }

        fn fit_mobile_camera(&mut self, board_view_size: egui::Vec2) {
            let board_view_size = board_view_size.max(egui::Vec2::splat(1.0));
            let board_size = egui::vec2(
                self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                self.game.height as f32 * Self::MOBILE_CELL_SIZE,
            );
            let fit_x = board_view_size.x / board_size.x.max(1.0);
            let fit_y = board_view_size.y / board_size.y.max(1.0);
            let zoom = (fit_x.min(fit_y) * 0.98).clamp(0.5, 4.0);

            let view_in_board = board_view_size / zoom;
            let max_x = (board_size.x - view_in_board.x).max(0.0);
            let max_y = (board_size.y - view_in_board.y).max(0.0);

            self.camera.zoom = zoom;
            self.camera.offset = egui::vec2(max_x * 0.5, max_y * 0.5);
            self.mobile_refit_pending = false;
        }

        fn show_action_bar(&mut self, ui: &mut egui::Ui) {
            egui::Panel::bottom("action_bar")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.spacing_mut().interact_size.y = 48.0;

                    let playing = self.game.status == GameStatus::Playing;
                    let has_selection = self.selected_cell.is_some();

                    let (on_hidden, on_flagged, on_marked) = match self.selected_cell {
                        Some((x, y)) => {
                            let cell = &self.game.cells[y * self.game.width + x];
                            (
                                matches!(cell.state, CellState::Hidden),
                                matches!(cell.state, CellState::Flagged),
                                matches!(cell.state, CellState::Marked),
                            )
                        }
                        None => (false, false, false),
                    };

                    let centered = egui::Layout::top_down(egui::Align::Center)
                        .with_cross_align(egui::Align::Center);

                    ui.columns(3, |columns| {
                        columns[0].with_layout(centered, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection && on_hidden,
                                    egui::Button::new("👁 Reveal").min_size(egui::vec2(70.0, 0.0)),
                                )
                                .clicked()
                            {
                                if let Some((x, y)) = self.selected_cell.take() {
                                    self.game.reveal(x, y);
                                }
                            }
                        });

                        columns[1].with_layout(centered, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection,
                                    egui::Button::new(if on_flagged {
                                        "🚩Unflag"
                                    } else {
                                        "🚩 Flag"
                                    })
                                    .min_size(egui::vec2(70.0, 0.0)),
                                )
                                .clicked()
                            {
                                if let Some((x, y)) = self.selected_cell {
                                    if on_flagged {
                                        self.game.clear_marker(x, y);
                                    } else {
                                        self.game.flag(x, y);
                                    }
                                }
                            }
                        });

                        columns[2].with_layout(centered, |ui| {
                            if self.question_marks {
                                if ui
                                    .add_enabled(
                                        playing && has_selection,
                                        egui::Button::new(if on_marked {
                                            "❓Unmark"
                                        } else {
                                            "❓ Mark"
                                        })
                                        .min_size(egui::vec2(70.0, 0.0)),
                                    )
                                    .clicked()
                                {
                                    if let Some((x, y)) = self.selected_cell {
                                        if on_marked {
                                            self.game.clear_marker(x, y);
                                        } else {
                                            self.game.mark(x, y);
                                        }
                                    }
                                }
                            }
                        });
                    });
                });
        }

        fn desktop_ui(&mut self, ui: &mut egui::Ui) {
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

        fn mobile_ui(&mut self, ui: &mut egui::Ui) {
            ui.spacing_mut().interact_size.y = 44.0;

            egui::Panel::top("mobile_topbar")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("mobile_preset")
                            .selected_text(self.selected_preset.short_label())
                            .show_ui(ui, |ui| {
                                for &preset in Preset::ALL {
                                    if ui
                                        .selectable_label(
                                            self.selected_preset == preset,
                                            preset.label(),
                                        )
                                        .clicked()
                                    {
                                        self.selected_preset = preset;
                                        let (w, h, m) = preset.dims();
                                        self.game = MinesweeperGame::new(w, h, m);
                                        self.reset_mobile_view();
                                    }
                                }
                            });

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
                                self.reset_mobile_view();
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        ui.visuals_mut().button_frame = false;
                        egui::widgets::global_theme_preference_switch(ui);
                        ui.toggle_value(&mut self.question_marks, "❓");

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let flags = self.game.flags_placed();
                            let remaining = (self.game.mines as isize) - (flags as isize);
                            ui.label(format!("🚩 {flags}  💣 {remaining}"));
                        });
                    });
                });

            self.show_action_bar(ui);
            let board_rect = ui.available_rect_before_wrap();
            if self.mobile_refit_pending {
                self.fit_mobile_camera(board_rect.size());
            }

            let board_widget = MinesweeperWidget::new(&mut self.game)
                .cell_size(Self::MOBILE_CELL_SIZE)
                .interaction_mode(InteractionMode::SelectOnly)
                .selected_cell(&mut self.selected_cell)
                .camera(&mut self.camera)
                .center_small_board_in_viewport(true)
                .question_marks(self.question_marks);
            ui.put(board_rect, board_widget);

            if self.game.status == GameStatus::Playing {
                let board_size = egui::vec2(
                    self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                    self.game.height as f32 * Self::MOBILE_CELL_SIZE,
                );
                let viewport_size = board_rect.size().max(egui::Vec2::splat(1.0));

                let pointer_pos = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
                let zoom_delta = ui.ctx().input(|i| {
                    if i.multi_touch().is_some() {
                        i.zoom_delta()
                    } else {
                        1.0
                    }
                });
                if board_rect.contains(pointer_pos.unwrap_or(board_rect.center()))
                    && (zoom_delta - 1.0).abs() > f32::EPSILON
                {
                    let old_zoom = self.camera.zoom;
                    self.camera.zoom = (self.camera.zoom * zoom_delta).clamp(0.5, 4.0);
                    if let Some(pos) = pointer_pos {
                        let local = pos - board_rect.min;
                        let board_pixel_size = board_size * old_zoom;
                        let old_shift = egui::vec2(
                            ((viewport_size.x - board_pixel_size.x) * 0.5).max(0.0),
                            ((viewport_size.y - board_pixel_size.y) * 0.5).max(0.0),
                        );
                        let local_board = local - old_shift;
                        let board_before = (local_board / old_zoom) + self.camera.offset;
                        let new_board_pixel_size = board_size * self.camera.zoom;
                        let new_shift = egui::vec2(
                            ((viewport_size.x - new_board_pixel_size.x) * 0.5).max(0.0),
                            ((viewport_size.y - new_board_pixel_size.y) * 0.5).max(0.0),
                        );
                        let local_board_new = local - new_shift;
                        self.camera.offset = board_before - (local_board_new / self.camera.zoom);
                    }
                    let new_view_in_board = viewport_size / self.camera.zoom;
                    let max_x = (board_size.x - new_view_in_board.x).max(0.0);
                    let max_y = (board_size.y - new_view_in_board.y).max(0.0);
                    self.camera.offset.x = self.camera.offset.x.clamp(0.0, max_x);
                    self.camera.offset.y = self.camera.offset.y.clamp(0.0, max_y);
                }
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
    canvas.style().set_property("touch-action", "none").unwrap();

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
