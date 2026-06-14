// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
#[xtask_wasm::run_example(assets_dir = "assets")]
fn run() {
    use eframe::egui;
    use egui_minesweeper::{
        CellState, GameStatus, InteractionMode, MinesweeperGame, MinesweeperWidget,
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
        scene_rect: Option<egui::Rect>,
        prev_status: GameStatus,
        show_menu_drawer: bool,
        menu_suppress_close: bool,
    }

    impl Default for MinesweeperApp {
        fn default() -> Self {
            Self {
                game: MinesweeperGame::new(9, 9, 10),
                selected_preset: Preset::Beginner,
                question_marks: true,
                selected_cell: None,
                scene_rect: None,
                prev_status: GameStatus::Playing,
                show_menu_drawer: false,
                menu_suppress_close: false,
            }
        }
    }

    impl eframe::App for MinesweeperApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let bg = ui.max_rect();
            ui.painter()
                .rect_filled(bg, egui::CornerRadius::ZERO, ui.visuals().panel_fill);

            let is_mobile = Self::is_mobile(ui);
            self.show_top_bar(ui, is_mobile);

            if is_mobile {
                self.mobile_ui(ui);
            } else {
                self.desktop_ui(ui);
            }

            self.prev_status = self.game.status;
        }
    }

    impl MinesweeperApp {
        const MOBILE_CELL_SIZE: f32 = 34.0;

        fn is_mobile(ui: &egui::Ui) -> bool {
            let content = ui.ctx().content_rect();
            let width_small = content.width() < 900.0;
            let touch_device = web_sys::window()
                .and_then(|w| w.match_media("(pointer: coarse)").ok())
                .flatten()
                .map_or(false, |mql| mql.matches());
            width_small || touch_device
        }

        fn show_action_bar(&mut self, ui: &mut egui::Ui) {
            egui::Panel::bottom("action_bar")
                .resizable(false)
                .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 4)))
                .show_inside(ui, |ui| {
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

                    let center = egui::Layout::top_down(egui::Align::Center)
                        .with_cross_align(egui::Align::Center);

                    ui.columns(4, |columns| {
                        columns[0].with_layout(center, |ui| {
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("☰").size(36.0))
                                        .min_size(egui::vec2(64.0, 64.0)),
                                )
                                .clicked()
                            {
                                let was_open = self.show_menu_drawer;
                                self.show_menu_drawer = !self.show_menu_drawer;
                                self.menu_suppress_close = !was_open;
                            }
                        });

                        columns[1].with_layout(center, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection && on_hidden,
                                    egui::Button::new(egui::RichText::new("👁").size(36.0)).min_size(egui::vec2(64.0, 64.0)),
                                )
                                .clicked()
                            {
                                if let Some((x, y)) = self.selected_cell.take() {
                                    self.game.reveal(x, y);
                                }
                            }
                        });

                        columns[2].with_layout(center, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection,
                                    egui::Button::new(egui::RichText::new("🚩").size(36.0)).min_size(egui::vec2(64.0, 64.0)),
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

                        columns[3].with_layout(center, |ui| {
                            if self.question_marks {
                                if ui
                                    .add_enabled(
                                        playing && has_selection,
                                        egui::Button::new(egui::RichText::new("❓").size(36.0)).min_size(egui::vec2(64.0, 64.0)),
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

        fn show_difficulty_select(&mut self, ui: &mut egui::Ui, close_on_select: bool) {
            for &preset in Preset::ALL {
                if ui
                    .selectable_label(self.selected_preset == preset, preset.label())
                    .clicked()
                {
                    self.selected_preset = preset;
                    let (w, h, m) = preset.dims();
                    self.game = MinesweeperGame::new(w, h, m);
                    self.selected_cell = None;
                    self.scene_rect = None;
                    if close_on_select {
                        ui.close();
                    }
                }
            }
        }

        fn show_result_label(&mut self, ui: &mut egui::Ui) {
            match self.game.status {
                GameStatus::Won => {
                    if self.prev_status != GameStatus::Won {
                        self.selected_cell = None;
                    }
                    ui.colored_label(egui::Color32::GREEN, "You won!");
                }
                GameStatus::Lost => {
                    if self.prev_status != GameStatus::Lost {
                        self.selected_cell = None;
                    }
                    ui.colored_label(egui::Color32::RED, "Boom!");
                }
                GameStatus::Playing => {}
            }
        }

        fn show_top_bar(&mut self, ui: &mut egui::Ui, is_mobile: bool) {
            if is_mobile {
                egui::Panel::top("mobile_topbar")
                    .resizable(false)
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(4, 2)))
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.show_result_label(ui);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let flags = self.game.flags_placed();
                                    let remaining = (self.game.mines as isize) - (flags as isize);
                                    ui.label(format!("🚩 {flags}  💣 {remaining}"));
                                },
                            );
                        });
                    });
            } else {
                egui::Panel::top("top_bar")
                    .frame(egui::Frame::new().inner_margin(4.0))
                    .show_inside(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.visuals_mut().button_frame = false;
                            ui.add_space(8.0);
                            egui::widgets::global_theme_preference_switch(ui);
                            ui.toggle_value(&mut self.question_marks, "❓");
                            ui.separator();
                            self.show_difficulty_select(ui, false);
                            ui.separator();
                            let flags = self.game.flags_placed();
                            let remaining = (self.game.mines as isize) - (flags as isize);
                            ui.label(format!("🚩 {flags}  💣 {remaining}"));
                            self.show_result_label(ui);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("New Game").clicked() {
                                        self.game.reset();
                                        self.selected_cell = None;
                                        self.scene_rect = None;
                                    }
                                },
                            );
                        });
                    });
            }
        }

        fn desktop_ui(&mut self, ui: &mut egui::Ui) {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add(MinesweeperWidget::new(&mut self.game).question_marks(self.question_marks));
            });
        }

        fn mobile_ui(&mut self, ui: &mut egui::Ui) {
            ui.spacing_mut().interact_size.y = 64.0;

            // Animated menu width (0 → 250 when open, 250 → 0 when closed)
            let anim = ui
                .ctx()
                .animate_bool_responsive(egui::Id::new("menu_drawer_anim"), self.show_menu_drawer);
            let panel_width = anim * 250.0;

            // Side menu drawer (always rendered for smooth animation)
            if panel_width > 0.0 {
                egui::Panel::left("menu_drawer")
                    .exact_size(panel_width)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.spacing_mut().interact_size.y = 36.0;
                        if ui.button(egui::RichText::new("🔄 New Game").size(24.0)).clicked() {
                            self.game.reset();
                            self.selected_cell = None;
                            self.scene_rect = None;
                            self.show_menu_drawer = false;
                        }
                        ui.separator();
                        ui.label("Difficulty");
                        for &preset in Preset::ALL {
                            if ui.selectable_label(self.selected_preset == preset, preset.label()).clicked() {
                                self.selected_preset = preset;
                                let (w, h, m) = preset.dims();
                                self.game = MinesweeperGame::new(w, h, m);
                                self.selected_cell = None;
                                self.scene_rect = None;
                                self.show_menu_drawer = false;
                            }
                        }
                        ui.separator();
                        ui.label("Theme");
                        let mut tp = ui.options(|o| o.theme_preference);
                        ui.selectable_value(&mut tp, egui::ThemePreference::System, egui::RichText::new("💻 System").size(24.0));
                        ui.selectable_value(&mut tp, egui::ThemePreference::Light, egui::RichText::new("☀ Light").size(24.0));
                        ui.selectable_value(&mut tp, egui::ThemePreference::Dark, egui::RichText::new("🌙 Dark").size(24.0));
                        ui.ctx().set_theme(tp);
                        ui.separator();
                        ui.toggle_value(&mut self.question_marks, egui::RichText::new("❓ Question marks").size(24.0));
                    });
            }

            self.show_action_bar(ui);

            let board_size = egui::vec2(
                self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                self.game.height as f32 * Self::MOBILE_CELL_SIZE,
            );
            let mut scene_rect = self
                .scene_rect
                .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, board_size));

            egui::containers::Scene::new()
                .zoom_range(0.25..=4.0)
                .max_inner_size(board_size)
                .show(ui, &mut scene_rect, |ui| {
                    ui.add(
                        MinesweeperWidget::new(&mut self.game)
                            .cell_size(Self::MOBILE_CELL_SIZE)
                            .interaction_mode(InteractionMode::SelectOnly)
                            .selected_cell(&mut self.selected_cell)
                            .question_marks(self.question_marks),
                    );
                });

            self.scene_rect = Some(scene_rect);

            // Dimmed overlay that follows the animated panel width
            if anim > 0.0 {
                let content_rect = ui.ctx().content_rect();
                let overlay_rect = egui::Rect::from_min_size(
                    egui::Pos2::new(panel_width, content_rect.top()),
                    egui::vec2(
                        (content_rect.width() - panel_width).max(0.0),
                        content_rect.height(),
                    ),
                );
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new("menu_overlay"),
                ));
                painter.rect_filled(overlay_rect, 0.0, egui::Color32::from_black_alpha(100));
            }

            // Close menu when tapping outside the animated drawer
            if self.show_menu_drawer && !self.menu_suppress_close {
                if ui.input(|i| i.pointer.any_click()) {
                    if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                        if pos.x > panel_width + 10.0 {
                            self.show_menu_drawer = false;
                        }
                    }
                }
            }
            self.menu_suppress_close = false;
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
