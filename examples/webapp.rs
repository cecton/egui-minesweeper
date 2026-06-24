// The #[run_example] macro generates:
//   - wasm32: a #[wasm_bindgen(start)] that calls this function body
//   - native: a main with `dist` / `start` sub-commands that build the wasm
//             bundle and serve it via a local dev server
mod geometry {
    /// Replicates the transform that `egui::Scene` uses to fit a scene rect into the screen.
    pub fn fit_to_rect_in_scene(
        rect_in_global: egui::Rect,
        rect_in_scene: egui::Rect,
        zoom_range: egui::Rangef,
    ) -> egui::emath::TSTransform {
        let scale = rect_in_global.size() / rect_in_scene.size();
        let scale = scale.min_elem();
        let scale = zoom_range.clamp(scale);
        let center_in_global = rect_in_global.center().to_vec2();
        let center_scene = rect_in_scene.center().to_vec2();
        egui::emath::TSTransform::from_translation(center_in_global - scale * center_scene)
            * egui::emath::TSTransform::from_scaling(scale)
    }

    /// Computes the board's on-screen pixel rectangle when the scene is reset to show the full board.
    pub fn board_rect_in_screen_pixels(
        outer_rect: egui::Rect,
        board_size: egui::Vec2,
        pixels_per_point: f32,
    ) -> egui::Rect {
        let scene_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, board_size);
        let zoom_range = egui::Rangef::new(0.0, f32::INFINITY);
        let transform = fit_to_rect_in_scene(outer_rect, scene_rect, zoom_range);
        let board_global = transform * scene_rect;
        egui::Rect::from_min_max(
            (board_global.min * pixels_per_point).round(),
            (board_global.max * pixels_per_point).round(),
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn fit_to_rect_in_scene_centers_board() {
            let global = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
            let scene = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(50.0, 50.0));
            let transform =
                fit_to_rect_in_scene(global, scene, egui::Rangef::new(0.0, f32::INFINITY));
            let transformed = transform * scene;
            assert!((transformed.center() - global.center()).length() < 0.001);
            assert!((transformed.size() - egui::vec2(50.0, 50.0)).length() < 0.001);
        }

        #[test]
        fn board_rect_in_screen_pixels_matches_scale() {
            let outer = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
            let board = egui::vec2(50.0, 50.0);
            let rect = board_rect_in_screen_pixels(outer, board, 2.0);
            assert_eq!(rect.min, egui::Pos2::new(50.0, 50.0));
            assert_eq!(rect.max, egui::Pos2::new(150.0, 150.0));
        }
    }
}

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
                Self::Beginner => "\u{FE82E} Beginner (9×9, 10 mines)",
                Self::Intermediate => "\u{FE82F} Intermediate (16×16, 40 mines)",
                Self::Expert => "\u{FE830} Expert (30×16, 99 mines)",
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

    #[derive(Clone, Copy)]
    enum ShareState {
        Idle,
        Capture {
            restore_scene: egui::Rect,
            wait_frames: u8,
        },
    }

    struct MinesweeperApp {
        game: MinesweeperGame,
        selected_preset: Preset,
        question_marks: bool,
        show_labels: bool,
        selected_cell: Option<(usize, usize)>,
        scene_rect: Option<egui::Rect>,
        prev_status: GameStatus,
        show_menu: bool,
        share_state: ShareState,
        toast: Option<(String, f32)>,
        capture_board_rect: Option<egui::Rect>,
        share_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    }

    impl Default for MinesweeperApp {
        fn default() -> Self {
            Self {
                game: MinesweeperGame::new(9, 9, 10),
                selected_preset: Preset::Beginner,
                question_marks: true,
                show_labels: false,
                selected_cell: None,
                scene_rect: None,
                prev_status: GameStatus::Playing,
                show_menu: false,
                share_state: ShareState::Idle,
                toast: None,
                capture_board_rect: None,
                share_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
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

            self.show_menu_modal(ui.ctx());

            // Handle screenshot result and capture timeout.
            let mut screenshot = None;
            if matches!(self.share_state, ShareState::Capture { .. }) {
                ui.input(|i| {
                    for event in &i.raw.events {
                        if let egui::Event::Screenshot { image, .. } = event {
                            screenshot = Some(image.clone());
                        }
                    }
                });
            }

            if let Some(image) = screenshot {
                if let Some(rect) = self.capture_board_rect {
                    if let Some(png) = Self::crop_and_encode_png(&image, rect) {
                        let filename = format!(
                            "minesweeper-{}x{}-{}.png",
                            self.game.width, self.game.height, self.game.mines
                        );
                        let title = match self.game.status {
                            GameStatus::Won => "Minesweeper win".to_string(),
                            GameStatus::Lost => "Minesweeper loss".to_string(),
                            GameStatus::Playing => "Minesweeper".to_string(),
                        };
                        Self::share_or_copy_png(png, filename, title, self.share_result.clone());
                    } else {
                        self.set_toast("Couldn't crop screenshot.");
                    }
                }
                self.share_state = ShareState::Idle;
                self.capture_board_rect = None;
            } else if let ShareState::Capture {
                restore_scene,
                wait_frames,
            } = self.share_state
            {
                if wait_frames == 0 {
                    self.share_state = ShareState::Idle;
                    self.capture_board_rect = None;
                    self.set_toast("Screenshot failed.");
                } else {
                    self.share_state = ShareState::Capture {
                        restore_scene,
                        wait_frames: wait_frames - 1,
                    };
                }
            }

            // Apply async share/copy result.
            let share_msg = self.share_result.lock().unwrap().take();
            if let Some(msg) = share_msg {
                self.set_toast(msg);
            }

            // Update toast timer.
            if let Some((_msg, remaining)) = &mut self.toast {
                let dt = ui.ctx().input(|i| i.stable_dt);
                *remaining -= dt;
                if *remaining <= 0.0 {
                    self.toast = None;
                }
            }

            self.prev_status = self.game.status;
        }
    }

    impl MinesweeperApp {
        const MOBILE_CELL_SIZE: f32 = 34.0;
        const MENU_FONT_SIZE: f32 = 24.0;
        const SCREENSHOT_TIMEOUT_FRAMES: u8 = 5;

        fn set_toast(&mut self, message: impl Into<String>) {
            self.toast = Some((message.into(), 2.0));
        }

        fn crop_and_encode_png(image: &egui::ColorImage, rect: egui::Rect) -> Option<Vec<u8>> {
            let [img_w, img_h] = image.size;
            let min_x = rect.min.x.max(0.0) as usize;
            let min_y = rect.min.y.max(0.0) as usize;
            let max_x = (rect.max.x as usize).min(img_w);
            let max_y = (rect.max.y as usize).min(img_h);
            let crop_w = max_x.saturating_sub(min_x);
            let crop_h = max_y.saturating_sub(min_y);
            if crop_w == 0 || crop_h == 0 {
                return None;
            }

            let cropped = image.region_by_pixels([min_x, min_y], [crop_w, crop_h]);
            let rgba: Vec<u8> = cropped.pixels.iter().flat_map(|c| c.to_array()).collect();

            let img = image::RgbaImage::from_raw(crop_w as u32, crop_h as u32, rgba)?;
            let mut encoded = Vec::new();
            img.write_to(
                &mut std::io::Cursor::new(&mut encoded),
                image::ImageFormat::Png,
            )
            .ok()?;
            Some(encoded)
        }

        fn share_or_copy_png(
            png_bytes: Vec<u8>,
            filename: String,
            title: String,
            share_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
        ) {
            use wasm_bindgen::JsValue;
            use wasm_bindgen_futures::spawn_local;
            use web_sys::{BlobPropertyBag, FilePropertyBag};

            spawn_local(async move {
                let window = match web_sys::window() {
                    Some(w) => w,
                    None => return,
                };
                let navigator = window.navigator();

                let array = js_sys::Uint8Array::from(&png_bytes[..]);
                let parts = js_sys::Array::new();
                parts.push(&array);

                let blob_options = BlobPropertyBag::new();
                blob_options.set_type("image/png");
                let blob = match web_sys::Blob::new_with_u8_array_sequence_and_options(
                    parts.as_ref(),
                    &blob_options,
                ) {
                    Ok(b) => b,
                    Err(_) => {
                        *share_result.lock().unwrap() = Some("Couldn't create image.".to_string());
                        return;
                    }
                };

                // Try Web Share first.
                let share_data = web_sys::ShareData::new();
                share_data.set_title(&title);
                let files = js_sys::Array::new();
                let file_options = FilePropertyBag::new();
                file_options.set_type("image/png");
                let file = match web_sys::File::new_with_u8_array_sequence_and_options(
                    parts.as_ref(),
                    &filename,
                    &file_options,
                ) {
                    Ok(f) => f,
                    Err(_) => {
                        *share_result.lock().unwrap() =
                            Some("Couldn't create image file.".to_string());
                        return;
                    }
                };
                files.push(&file);
                share_data.set_files(files.as_ref());

                let has = |target: &JsValue, name: &str| {
                    js_sys::Reflect::has(target, &JsValue::from_str(name)).unwrap_or(false)
                };

                let share_supported = has(&navigator, "share");
                let can_share_supported = has(&navigator, "canShare");
                let should_share = share_supported
                    && (!can_share_supported || navigator.can_share_with_data(&share_data));

                if should_share {
                    let promise = navigator.share_with_data(&share_data);
                    let result = wasm_bindgen_futures::JsFuture::from(promise).await;
                    if result.is_ok() {
                        *share_result.lock().unwrap() = Some("Shared!".to_string());
                        return;
                    }
                }

                // Fallback: copy image to clipboard.
                if !has(&navigator, "clipboard") {
                    *share_result.lock().unwrap() =
                        Some("Couldn't share or copy image.".to_string());
                    return;
                }
                let clipboard = navigator.clipboard();
                if !has(&clipboard, "write") {
                    *share_result.lock().unwrap() =
                        Some("Couldn't share or copy image.".to_string());
                    return;
                }

                let record = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&record, &JsValue::from_str("image/png"), &blob);
                let item =
                    match web_sys::ClipboardItem::new_with_record_from_str_to_blob_promise(&record)
                    {
                        Ok(i) => i,
                        Err(_) => {
                            *share_result.lock().unwrap() =
                                Some("Couldn't prepare clipboard item.".to_string());
                            return;
                        }
                    };
                let items = js_sys::Array::new();
                items.push(&item);
                let items_js: JsValue = items.into();
                let result = wasm_bindgen_futures::JsFuture::from(clipboard.write(&items_js)).await;
                *share_result.lock().unwrap() = if result.is_ok() {
                    Some("Copied to clipboard!".to_string())
                } else {
                    Some("Couldn't share or copy image.".to_string())
                };
            });
        }

        fn is_mobile(ui: &egui::Ui) -> bool {
            let content = ui.ctx().content_rect();
            let width_small = content.width() < 900.0;
            let touch_device = web_sys::window()
                .and_then(|w| w.match_media("(pointer: coarse)").ok())
                .flatten()
                .is_some_and(|mql| mql.matches());
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

                    ui.columns(5, |columns| {
                        columns[0].with_layout(center, |ui| {
                            self.show_hamburger_menu(ui);
                        });

                        columns[1].with_layout(center, |ui| {
                            let flags = self.game.flags_placed();
                            let remaining = (self.game.mines as isize) - (flags as isize);
                            let space = (ui.available_height() - 48.0).max(0.0) / 2.0;
                            ui.add_space(space);
                            ui.label(egui::RichText::new(format!("🚩 {flags}")).size(20.0));
                            ui.label(egui::RichText::new(format!("💣 {remaining}")).size(20.0));
                        });

                        columns[2].with_layout(center, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection && on_hidden,
                                    egui::Button::new(egui::RichText::new("👁").size(36.0))
                                        .min_size(egui::vec2(64.0, 64.0)),
                                )
                                .clicked()
                            {
                                if let Some((x, y)) = self.selected_cell.take() {
                                    self.game.reveal(x, y);
                                }
                            }
                        });

                        columns[3].with_layout(center, |ui| {
                            if ui
                                .add_enabled(
                                    playing && has_selection,
                                    egui::Button::new(egui::RichText::new("🚩").size(36.0))
                                        .min_size(egui::vec2(64.0, 64.0)),
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

                        columns[4].with_layout(center, |ui| {
                            if self.question_marks
                                && ui
                                    .add_enabled(
                                        playing && has_selection,
                                        egui::Button::new(egui::RichText::new("❓").size(36.0))
                                            .min_size(egui::vec2(64.0, 64.0)),
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
                        });
                    });
                });
        }

        fn show_mobile_result_banner(&mut self, ui: &mut egui::Ui) {
            if self.game.status == GameStatus::Playing {
                return;
            }

            let (text, color) = match self.game.status {
                GameStatus::Won => ("🎉 You won!", egui::Color32::GREEN),
                GameStatus::Lost => ("💥 Boom!", egui::Color32::RED),
                GameStatus::Playing => unreachable!(),
            };

            egui::Panel::top("mobile_result_banner")
                .resizable(false)
                .frame(
                    egui::Frame::NONE
                        .fill(ui.visuals().panel_fill)
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 8)),
                )
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(color, egui::RichText::new(text).size(20.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("📤 Share").clicked() {
                                let restore_scene = self.scene_rect.unwrap_or_else(|| {
                                    let board_size = egui::vec2(
                                        self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                                        self.game.height as f32 * Self::MOBILE_CELL_SIZE,
                                    );
                                    egui::Rect::from_min_size(egui::Pos2::ZERO, board_size)
                                });
                                self.share_state = ShareState::Capture {
                                    restore_scene,
                                    wait_frames: Self::SCREENSHOT_TIMEOUT_FRAMES,
                                };
                                self.capture_board_rect = None;
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
                    self.share_state = ShareState::Idle;
                    self.capture_board_rect = None;
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

        fn show_hamburger_menu(&mut self, ui: &mut egui::Ui) {
            if ui
                .add(
                    egui::Button::new(egui::RichText::new("☰").size(36.0))
                        .min_size(egui::vec2(64.0, 64.0)),
                )
                .clicked()
            {
                self.show_menu = true;
            }
        }

        fn show_menu_modal(&mut self, ctx: &egui::Context) {
            if !self.show_menu {
                return;
            }

            let vp_width = ctx.viewport_rect().width();
            let area = egui::Modal::default_area(egui::Id::new("menu_modal"))
                .anchor(egui::Align2::CENTER_BOTTOM, egui::Vec2::ZERO)
                .default_width(vp_width);

            let menu_font_size = Self::MENU_FONT_SIZE;
            let response = egui::Modal::new(egui::Id::new("menu_modal"))
                .area(area)
                .frame(
                    egui::Frame::popup(&ctx.global_style())
                        .inner_margin(egui::Margin::symmetric(16, 16)),
                )
                .backdrop_color(egui::Color32::from_black_alpha(128))
                .show(ctx, |ui| {
                    ui.set_min_width(vp_width - 32.0);
                    ui.spacing_mut().interact_size.y = 36.0;
                    {
                        let prev = ui.visuals().button_frame;
                        ui.visuals_mut().button_frame = false;
                        if ui
                            .button(egui::RichText::new("🔄 New Game").size(menu_font_size))
                            .clicked()
                        {
                            self.game.reset();
                            self.selected_cell = None;
                            self.scene_rect = None;
                            self.share_state = ShareState::Idle;
                            self.capture_board_rect = None;
                            self.show_menu = false;
                        }
                        ui.visuals_mut().button_frame = prev;
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Difficulty").size(menu_font_size));
                    for &preset in Preset::ALL {
                        if ui
                            .selectable_label(
                                self.selected_preset == preset,
                                egui::RichText::new(preset.label()).size(menu_font_size),
                            )
                            .clicked()
                        {
                            self.selected_preset = preset;
                            let (w, h, m) = preset.dims();
                            self.game = MinesweeperGame::new(w, h, m);
                            self.selected_cell = None;
                            self.scene_rect = None;
                            self.share_state = ShareState::Idle;
                            self.capture_board_rect = None;
                            self.show_menu = false;
                        }
                    }
                    ui.separator();
                    ui.label(egui::RichText::new("Theme").size(menu_font_size));
                    let mut tp = ui.options(|o| o.theme_preference);
                    ui.selectable_value(
                        &mut tp,
                        egui::ThemePreference::System,
                        egui::RichText::new("💻 System").size(menu_font_size),
                    );
                    ui.selectable_value(
                        &mut tp,
                        egui::ThemePreference::Light,
                        egui::RichText::new("☀ Light").size(menu_font_size),
                    );
                    ui.selectable_value(
                        &mut tp,
                        egui::ThemePreference::Dark,
                        egui::RichText::new("🌙 Dark").size(menu_font_size),
                    );
                    ui.ctx().set_theme(tp);
                });

            if response.should_close() {
                self.show_menu = false;
            }
        }

        fn show_top_bar(&mut self, ui: &mut egui::Ui, is_mobile: bool) {
            if is_mobile {
                // Top bar hidden on mobile; counters shown in action bar instead
            } else {
                egui::Panel::top("top_bar")
                    .frame(egui::Frame::new().inner_margin(4.0))
                    .show_inside(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.visuals_mut().button_frame = false;
                            ui.add_space(8.0);
                            egui::widgets::global_theme_preference_switch(ui);
                            ui.toggle_value(&mut self.question_marks, "❓");
                            ui.toggle_value(&mut self.show_labels, "123");
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
                                        self.share_state = ShareState::Idle;
                                        self.capture_board_rect = None;
                                    }
                                },
                            );
                        });
                    });
            }
        }

        fn desktop_ui(&mut self, ui: &mut egui::Ui) {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add(
                    MinesweeperWidget::new(&mut self.game)
                        .question_marks(self.question_marks)
                        .show_labels(self.show_labels),
                );
            });
        }

        fn mobile_ui(&mut self, ui: &mut egui::Ui) {
            ui.spacing_mut().interact_size.y = 64.0;

            let capturing = matches!(self.share_state, ShareState::Capture { .. });

            if !capturing {
                self.show_mobile_result_banner(ui);
                self.show_action_bar(ui);
            }

            let board_size = egui::vec2(
                self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                self.game.height as f32 * Self::MOBILE_CELL_SIZE,
            );

            if capturing {
                let full_board_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, board_size);
                self.scene_rect = Some(full_board_rect);
            }

            let mut scene_rect = self
                .scene_rect
                .unwrap_or_else(|| egui::Rect::from_min_size(egui::Pos2::ZERO, board_size));

            let outer_rect =
                egui::Rect::from_min_size(ui.min_rect().min, ui.available_size_before_wrap());

            egui::containers::Scene::new()
                .zoom_range(if capturing {
                    0.0..=f32::INFINITY
                } else {
                    0.25..=4.0
                })
                .max_inner_size(board_size)
                .show(ui, &mut scene_rect, |ui| {
                    ui.add(
                        MinesweeperWidget::new(&mut self.game)
                            .cell_size(Self::MOBILE_CELL_SIZE)
                            .interaction_mode(InteractionMode::SelectOnly)
                            .selected_cell(&mut self.selected_cell)
                            .question_marks(self.question_marks)
                            .show_labels(self.show_labels),
                    );
                });

            if capturing {
                let dpr = ui
                    .ctx()
                    .input(|i| i.viewport().native_pixels_per_point)
                    .unwrap_or(1.0);
                self.capture_board_rect = Some(geometry::board_rect_in_screen_pixels(
                    outer_rect, board_size, dpr,
                ));
                ui.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                if let ShareState::Capture { restore_scene, .. } = self.share_state {
                    self.scene_rect = Some(restore_scene);
                }
            } else {
                self.scene_rect = Some(scene_rect);
            }

            if let Some((msg, _)) = &self.toast {
                let toast_area = ui.max_rect().shrink(16.0);
                let toast_rect = egui::Rect::from_min_size(
                    toast_area.left_bottom() - egui::vec2(0.0, 40.0),
                    egui::vec2(toast_area.width(), 40.0),
                );
                ui.painter().rect_filled(
                    toast_rect,
                    egui::CornerRadius::same(6),
                    ui.visuals().panel_fill.gamma_multiply(0.85),
                );
                ui.put(
                    toast_rect,
                    egui::Label::new(
                        egui::RichText::new(msg)
                            .size(16.0)
                            .color(ui.visuals().strong_text_color()),
                    ),
                );
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
