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
        camera: BoardCamera,
        dark_mode: bool,
        theme_initialized: bool,
        mobile_refit_pending: bool,
        mobile_gestures: MobileGestureState,
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct MobileGestureState {
        press_cell: Option<(usize, usize)>,
        press_start_time: Option<f64>,
        press_start_pos: Option<egui::Pos2>,
        selected_before_press: Option<(usize, usize)>,
        long_press_fired: bool,
        tap_canceled: bool,
    }

    impl MobileGestureState {
        fn reset_press_state(&mut self, clear_long_press: bool) {
            self.press_cell = None;
            self.press_start_time = None;
            self.press_start_pos = None;
            self.selected_before_press = None;
            self.tap_canceled = false;
            if clear_long_press {
                self.long_press_fired = false;
            }
        }
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
                    zoom: 1.6,
                },
                dark_mode: false,
                theme_initialized: false,
                mobile_refit_pending: true,
                mobile_gestures: MobileGestureState::default(),
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

            fn reset_mobile_view(
                camera: &mut BoardCamera,
                selected: &mut Option<(usize, usize)>,
                mobile_refit_pending: &mut bool,
            ) {
                *selected = None;
                camera.offset = egui::Vec2::ZERO;
                camera.zoom = 1.0;
                *mobile_refit_pending = true;
            }

            fn fit_mobile_camera(app: &mut MinesweeperApp, board_view_size: egui::Vec2) {
                let board_view_size = board_view_size.max(egui::Vec2::splat(1.0));
                let cell_size = 34.0;
                let board_size = egui::vec2(
                    app.game.width as f32 * cell_size,
                    app.game.height as f32 * cell_size,
                );
                let fit_x = board_view_size.x / board_size.x.max(1.0);
                let fit_y = board_view_size.y / board_size.y.max(1.0);
                let zoom = (fit_x.min(fit_y) * 0.98).clamp(0.5, 4.0);

                let view_in_board = board_view_size / zoom;
                let max_x = (board_size.x - view_in_board.x).max(0.0);
                let max_y = (board_size.y - view_in_board.y).max(0.0);

                app.camera.zoom = zoom;
                app.camera.offset = egui::vec2(max_x * 0.5, max_y * 0.5);
                app.mobile_refit_pending = false;
            }

            fn mobile_cell_at_pointer(
                app: &MinesweeperApp,
                board_rect: egui::Rect,
                pointer_pos: egui::Pos2,
                cell_size: f32,
            ) -> Option<(usize, usize)> {
                if !board_rect.contains(pointer_pos) {
                    return None;
                }
                let board_size = egui::vec2(
                    app.game.width as f32 * cell_size,
                    app.game.height as f32 * cell_size,
                );
                let viewport_size = board_rect.size().max(egui::Vec2::splat(1.0));
                let board_pixel_size = board_size * app.camera.zoom;
                let view_shift = egui::vec2(
                    ((viewport_size.x - board_pixel_size.x) * 0.5).max(0.0),
                    ((viewport_size.y - board_pixel_size.y) * 0.5).max(0.0),
                );
                let local = pointer_pos - board_rect.min;
                let board_pos = ((local - view_shift) / app.camera.zoom) + app.camera.offset;
                if board_pos.x < 0.0
                    || board_pos.y < 0.0
                    || board_pos.x >= board_size.x
                    || board_pos.y >= board_size.y
                {
                    return None;
                }
                let cx = (board_pos.x / cell_size).floor() as usize;
                let cy = (board_pos.y / cell_size).floor() as usize;
                if cx < app.game.width && cy < app.game.height {
                    Some((cx, cy))
                } else {
                    None
                }
            }

            let mobile = is_mobile(ui);

            if !self.theme_initialized {
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

            if !mobile {
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
                                    .selectable_label(
                                        self.selected_preset == preset,
                                        preset.label(),
                                    )
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("New Game").clicked() {
                                        self.game.reset();
                                    }
                                },
                            );
                        });
                    });

                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add(
                        MinesweeperWidget::new(&mut self.game).question_marks(self.question_marks),
                    );
                });
                return;
            }

            // Mobile UI
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.heading("Minesweeper");
                ui.add_space(4.0);
            });

            ui.spacing_mut().interact_size.y = 44.0;

            egui::Panel::top("mobile_topbar")
                .resizable(false)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("mobile_preset")
                            .selected_text(self.selected_preset.label())
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
                                        reset_mobile_view(
                                            &mut self.camera,
                                            &mut self.selected_cell,
                                            &mut self.mobile_refit_pending,
                                        );
                                        self.mobile_gestures = MobileGestureState::default();
                                    }
                                }
                            });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("New Game").clicked() {
                                self.game.reset();
                                reset_mobile_view(
                                    &mut self.camera,
                                    &mut self.selected_cell,
                                    &mut self.mobile_refit_pending,
                                );
                                self.mobile_gestures = MobileGestureState::default();
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        let flags = self.game.flags_placed();
                        let remaining = (self.game.mines as isize) - (flags as isize);
                        ui.label(format!("Flags: {flags} | Mines: {remaining}"));
                        ui.toggle_value(&mut self.question_marks, "❓");
                        ui.checkbox(&mut self.dark_mode, "Dark");
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
                        }
                    });
                });

            let board_rect = ui.available_rect_before_wrap();
            if self.mobile_refit_pending {
                fit_mobile_camera(self, board_rect.size());
            }

            // Mobile gestures (kept in example layer): pinch, tap-select,
            // second tap cycle marker, long-press reveal.
            if self.game.status == GameStatus::Playing {
                let cell_size = 34.0;
                let board_size = egui::vec2(
                    self.game.width as f32 * cell_size,
                    self.game.height as f32 * cell_size,
                );
                let viewport_size = board_rect.size().max(egui::Vec2::splat(1.0));

                let pointer_pos = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos().or_else(|| i.pointer.hover_pos()));
                let primary_pressed = ui.ctx().input(|i| i.pointer.primary_pressed());
                let primary_released = ui.ctx().input(|i| i.pointer.primary_released());
                let now = ui.ctx().input(|i| i.time);

                if let Some(pos) = pointer_pos {
                    if primary_pressed && board_rect.contains(pos) {
                        if let Some((cx, cy)) =
                            mobile_cell_at_pointer(self, board_rect, pos, cell_size)
                        {
                            self.mobile_gestures.press_cell = Some((cx, cy));
                            self.mobile_gestures.press_start_time = Some(now);
                            self.mobile_gestures.press_start_pos = Some(pos);
                            self.mobile_gestures.selected_before_press = self.selected_cell;
                            self.mobile_gestures.long_press_fired = false;
                            self.mobile_gestures.tap_canceled = false;
                        } else {
                            self.mobile_gestures.reset_press_state(true);
                        }
                    } else if primary_pressed {
                        self.mobile_gestures.reset_press_state(true);
                    }

                    if let (Some(start), Some(start_pos), Some((x, y))) = (
                        self.mobile_gestures.press_start_time,
                        self.mobile_gestures.press_start_pos,
                        self.mobile_gestures.press_cell,
                    ) {
                        let moved = pos.distance(start_pos);
                        if moved > 10.0 {
                            self.mobile_gestures.tap_canceled = true;
                            self.mobile_gestures.press_cell = None;
                            self.mobile_gestures.press_start_time = None;
                            self.mobile_gestures.press_start_pos = None;
                            self.mobile_gestures.long_press_fired = false;
                        } else if !self.mobile_gestures.long_press_fired && (now - start) >= 0.32 {
                            self.game.reveal(x, y);
                            self.selected_cell = Some((x, y));
                            self.mobile_gestures.long_press_fired = true;
                        }
                    }
                }

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
                    self.mobile_gestures.reset_press_state(true);
                }

                if primary_released {
                    if !self.mobile_gestures.long_press_fired && !self.mobile_gestures.tap_canceled
                    {
                        let released_cell = pointer_pos
                            .and_then(|pos| {
                                mobile_cell_at_pointer(self, board_rect, pos, cell_size)
                            })
                            .or(self.mobile_gestures.press_cell);
                        if let Some((cx, cy)) = released_cell {
                            let same_selected =
                                self.mobile_gestures.selected_before_press == Some((cx, cy));
                            self.selected_cell = Some((cx, cy));
                            if same_selected {
                                self.game.cycle_flag(cx, cy);
                            }
                        }
                    }
                    self.mobile_gestures.reset_press_state(true);
                }
            }

            let board_widget = MinesweeperWidget::new(&mut self.game)
                .cell_size(34.0)
                .interaction_mode(InteractionMode::SelectOnly)
                .selected_cell(&mut self.selected_cell)
                .camera(&mut self.camera)
                .center_small_board_in_viewport(true)
                .question_marks(self.question_marks);
            ui.put(board_rect, board_widget);
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
