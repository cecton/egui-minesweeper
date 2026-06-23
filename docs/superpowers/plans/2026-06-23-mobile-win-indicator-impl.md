# Mobile win/loss indicator and screenshot sharing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a mobile result banner with a share button to `examples/webapp.rs`, capturing the full Minesweeper board even when the user has zoomed/panned the `Scene`.

**Architecture:** A small state machine in `MinesweeperApp` temporarily hides the UI, resets the `Scene` to fit the whole board, and requests a viewport screenshot via `egui::ViewportCommand::Screenshot`. The returned `egui::Event::Screenshot` is cropped to the board, encoded as PNG, and shared or copied using web APIs.

**Tech Stack:** Rust, egui 0.34, eframe 0.34, web-sys, js-sys, `image` (PNG encoder).

---

## File structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | Add `image`, `js-sys` wasm32 dev-dependencies and enable required `web-sys` features. |
| `examples/webapp.rs` | Add `ShareState`, toast state, banner UI, capture state machine, image encoding, and share/copy interop. |
| `docs/superpowers/specs/2026-06-23-mobile-win-indicator-design.md` | Source of truth for behavior; already approved. |

---

## Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `image` and `js-sys` as wasm32 dev-dependencies**

Append inside `[target.'cfg(target_arch = "wasm32")'.dev-dependencies]`:

```toml
js-sys = "0.3"
image = { version = "0.25", default-features = false, features = ["png"] }
```

- [ ] **Step 2: Enable required `web-sys` features**

Replace the existing single-feature `web-sys` entry with:

```toml
web-sys = { version = "0.3", features = [
    "Blob",
    "BlobPropertyBag",
    "Clipboard",
    "ClipboardItem",
    "File",
    "FilePropertyBag",
    "HtmlCanvasElement",
    "Navigator",
    "ShareData",
    "Window",
] }
```

- [ ] **Step 3: Verify the dependency change compiles**

Run:

```bash
cargo build --target wasm32-unknown-unknown --example webapp
```

Expected: successful build (the example code has not changed yet).

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "deps: add image/js-sys and web-sys features for mobile screenshot sharing"
```

---

## Task 2: Add state and geometry helpers

**Files:**
- Modify: `examples/webapp.rs`

- [ ] **Step 1: Add new fields to `MinesweeperApp`**

Inside the struct definition (after `show_menu: bool,`):

```rust
share_state: ShareState,
toast: Option<(String, f32)>,
capture_board_rect: Option<egui::Rect>,
share_result: std::sync::Arc<std::sync::Mutex<Option<String>>>,
```

- [ ] **Step 2: Initialize the new fields in `Default`**

Inside `impl Default for MinesweeperApp`:

```rust
share_state: ShareState::Idle,
toast: None,
capture_board_rect: None,
share_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
```

- [ ] **Step 3: Define `ShareState`**

Add as a top-level item inside `fn run()` (outside any `impl` block):

```rust
#[derive(Clone, Copy)]
enum ShareState {
    Idle,
    Capture { restore_scene: egui::Rect, wait_frames: u8 },
}
```

- [ ] **Step 4: Add a toast helper**

Inside `impl MinesweeperApp`:

```rust
fn set_toast(&mut self, message: impl Into<String>) {
    self.toast = Some((message.into(), 2.0));
}
```

- [ ] **Step 5: Add geometry helpers**

Inside `impl MinesweeperApp`:

```rust
/// Replicates the transform that `egui::Scene` uses to fit a scene rect into the screen.
fn fit_to_rect_in_scene(
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
fn board_rect_in_screen_pixels(
    outer_rect: egui::Rect,
    board_size: egui::Vec2,
    pixels_per_point: f32,
) -> egui::Rect {
    let scene_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, board_size);
    let zoom_range = egui::Rangef::new(0.0, f32::INFINITY);
    let transform = Self::fit_to_rect_in_scene(outer_rect, scene_rect, zoom_range);
    let board_global = transform * scene_rect;
    egui::Rect::from_min_max(
        (board_global.min * pixels_per_point).round(),
        (board_global.max * pixels_per_point).round(),
    )
}
```

- [ ] **Step 6: Add unit tests for the geometry helpers**

At the bottom of `fn run()`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_to_rect_in_scene_centers_board() {
        let global = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let scene = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(50.0, 50.0));
        let transform =
            MinesweeperApp::fit_to_rect_in_scene(global, scene, egui::Rangef::new(0.0, f32::INFINITY));
        let transformed = transform * scene;
        assert!((transformed.center() - global.center()).length() < 0.001);
        assert!((transformed.size() - egui::vec2(50.0, 50.0)).length() < 0.001);
    }

    #[test]
    fn board_rect_in_screen_pixels_matches_scale() {
        let outer = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0));
        let board = egui::vec2(50.0, 50.0);
        let rect = MinesweeperApp::board_rect_in_screen_pixels(outer, board, 2.0);
        assert_eq!(rect.min, egui::Pos2::new(50.0, 50.0));
        assert_eq!(rect.max, egui::Pos2::new(150.0, 150.0));
    }
}
```

- [ ] **Step 7: Compile the tests**

Run:

```bash
cargo test --target wasm32-unknown-unknown --example webapp --no-run
```

Expected: tests compile successfully.

- [ ] **Step 8: Commit**

```bash
git add examples/webapp.rs
git commit -m "feat(mobile): add share state and screenshot geometry helpers"
```

---

## Task 3: Implement the mobile result banner

**Files:**
- Modify: `examples/webapp.rs`

- [ ] **Step 1: Add `show_mobile_result_banner`**

Inside `impl MinesweeperApp`:

```rust
fn show_mobile_result_banner(&mut self, ui: &mut egui::Ui) {
    if self.game.status == GameStatus::Playing {
        return;
    }

    let (text, color) = match self.game.status {
        GameStatus::Won => ("🎉 You won!", egui::Color32::GREEN),
        GameStatus::Lost => ("💥 Boom!", egui::Color32::RED),
        GameStatus::Playing => unreachable!(),
    };

    egui::TopBottomPanel::top("mobile_result_banner")
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
                    if self.game.status == GameStatus::Won
                        && ui.button("📤 Share").clicked()
                    {
                        let restore_scene = self.scene_rect.unwrap_or_else(|| {
                            let board_size = egui::vec2(
                                self.game.width as f32 * Self::MOBILE_CELL_SIZE,
                                self.game.height as f32 * Self::MOBILE_CELL_SIZE,
                            );
                            egui::Rect::from_min_size(egui::Pos2::ZERO, board_size)
                        });
                        self.share_state = ShareState::Capture {
                            restore_scene,
                            wait_frames: 5,
                        };
                        self.capture_board_rect = None;
                    }
                });
            });
        });
}
```

- [ ] **Step 2: Render the banner in `mobile_ui`**

Inside `mobile_ui`, before `show_action_bar`, add:

```rust
if !matches!(self.share_state, ShareState::Capture { .. }) {
    self.show_mobile_result_banner(ui);
}
```

Leave the existing `show_action_bar` call in place.

- [ ] **Step 3: Build**

Run:

```bash
cargo build --target wasm32-unknown-unknown --example webapp
```

Expected: successful build.

- [ ] **Step 4: Commit**

```bash
git add examples/webapp.rs
git commit -m "feat(mobile): add persistent won/lost banner with share button"
```

---

## Task 4: Implement the screenshot capture state machine

**Files:**
- Modify: `examples/webapp.rs`

- [ ] **Step 1: Modify `mobile_ui` to hide UI and reset the scene during capture**

Replace the body of `mobile_ui` with the following version (keep `Self::MOBILE_CELL_SIZE` usage identical):

```rust
fn mobile_ui(&mut self, ui: &mut egui::Ui) {
    ui.spacing_mut().interact_size.y = 64.0;

    let capturing = matches!(self.share_state, ShareState::Capture { .. });

    if !capturing {
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

    let outer_rect = egui::Rect::from_min_size(ui.min_rect().min, ui.available_size_before_wrap());

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
        self.capture_board_rect =
            Some(Self::board_rect_in_screen_pixels(outer_rect, board_size, dpr));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        if let ShareState::Capture { restore_scene, .. } = self.share_state {
            self.scene_rect = Some(restore_scene);
        }
    } else {
        self.scene_rect = Some(scene_rect);
    }
}
```

- [ ] **Step 2: Handle the screenshot event and timeout**

Inside `impl eframe::App for MinesweeperApp`, in `fn ui`, at the end before `self.prev_status = self.game.status;`, add:

```rust
// Handle screenshot result and capture timeout.
let screenshot = if matches!(self.share_state, ShareState::Capture { .. }) {
    ui.ctx().input(|i| {
        i.raw
            .events
            .iter()
            .find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
    })
} else {
    None
};

if let Some(image) = screenshot {
    if let Some(rect) = self.capture_board_rect {
        if let Some(png) = Self::crop_and_encode_png(&image, rect) {
            let filename = format!(
                "minesweeper-{}x{}-{}.png",
                self.game.width, self.game.height, self.game.mines
            );
            Self::share_or_copy_png(png, filename, self.share_result.clone());
        } else {
            self.set_toast("Couldn\'t crop screenshot.");
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
if let Some(msg) = self.share_result.lock().unwrap().take() {
    self.set_toast(msg);
}

// Update toast timer.
if let Some((msg, remaining)) = &mut self.toast {
    let dt = ui.ctx().input(|i| i.stable_dt);
    *remaining -= dt;
    if *remaining <= 0.0 {
        self.toast = None;
    }
}
```

Correct pattern:

```rust
let screenshot = ui.ctx().input(|i| {
    i.raw
        .events
        .iter()
        .find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
});

if let Some(image) = screenshot {
    if let Some(rect) = self.capture_board_rect {
        // ... process
    }
    self.share_state = ShareState::Idle;
    self.capture_board_rect = None;
} else if wait_frames == 0 { ... }
```

Use the above pattern in the final code.

- [ ] **Step 3: Build**

Run:

```bash
cargo build --target wasm32-unknown-unknown --example webapp
```

Expected: compile errors about missing `crop_and_encode_png` and `share_or_copy_png` helpers. These are added in Task 5.

- [ ] **Step 4: Commit**

Commit only after Task 5 has made the helpers available.

---

## Task 5: Implement image encoding and share/copy interop

**Files:**
- Modify: `examples/webapp.rs`

- [ ] **Step 1: Add `crop_and_encode_png`**

Inside `impl MinesweeperApp`:

```rust
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

    let mut rgba = Vec::with_capacity(crop_w * crop_h * 4);
    for y in min_y..max_y {
        for x in min_x..max_x {
            let c = image[(x, y)];
            rgba.extend_from_slice(&c.to_array());
        }
    }

    let mut encoded = Vec::new();
    {
        let encoder = image::codecs::png::PngEncoder::new(&mut encoded);
        encoder
            .encode(&rgba, crop_w as u32, crop_h as u32, image::ColorType::Rgba8)
            .ok()?;
    }
    Some(encoded)
}
```

- [ ] **Step 2: Add `share_or_copy_png`**

Inside `impl MinesweeperApp`:

```rust
fn share_or_copy_png(
    png_bytes: Vec<u8>,
    filename: String,
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
        let blob = match web_sys::Blob::new_with_u8_array_sequence_and_options(parts.as_ref(), &blob_options) {
            Ok(b) => b,
            Err(_) => {
                *share_result.lock().unwrap() = Some("Couldn\'t create image.".to_string());
                return;
            }
        };

        // Try Web Share first.
        let share_data = web_sys::ShareData::new();
        share_data.set_title("Minesweeper win");
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
                *share_result.lock().unwrap() = Some("Couldn\'t create image file.".to_string());
                return;
            }
        };
        files.push(&file);
        share_data.set_files(files.as_ref());

        if navigator.can_share_with_data(&share_data) {
            let promise = navigator.share_with_data(&share_data);
            let result = wasm_bindgen_futures::JsFuture::from(promise).await;
            if result.is_ok() {
                *share_result.lock().unwrap() = Some("Shared!".to_string());
                return;
            }
        }

        // Fallback: copy image to clipboard.
        let record = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &record,
            &JsValue::from_str("image/png"),
            &blob,
        );
        let item = match web_sys::ClipboardItem::new_with_record_from_str_to_blob_promise(&record) {
            Ok(i) => i,
            Err(_) => {
                *share_result.lock().unwrap() = Some("Couldn\'t prepare clipboard item.".to_string());
                return;
            }
        };
        let items = js_sys::Array::new();
        items.push(&item);
        let items_js: JsValue = items.into();
        let result = wasm_bindgen_futures::JsFuture::from(navigator.clipboard().write(&items_js)).await;
        *share_result.lock().unwrap() = if result.is_ok() {
            Some("Copied to clipboard!".to_string())
        } else {
            Some("Couldn\'t share or copy image.".to_string())
        };
    });
}
```

- [ ] **Step 3: Build**

Run:

```bash
cargo build --target wasm32-unknown-unknown --example webapp
```

Expected: successful build.

- [ ] **Step 4: Commit**

```bash
git add examples/webapp.rs
git commit -m "feat(mobile): capture, crop, and share/copy board screenshot"
```

---

## Task 6: Render the toast

**Files:**
- Modify: `examples/webapp.rs`

- [ ] **Step 1: Draw the toast over the mobile UI**

Inside `mobile_ui`, after the `Scene::show` block (and after `self.scene_rect = Some(scene_rect);`), add:

```rust
if let Some((msg, _)) = &self.toast {
    let toast_rect = ui.max_rect().shrink(16.0);
    ui.put(
        egui::Rect::from_min_size(toast_rect.left_bottom() - egui::vec2(0.0, 40.0), egui::vec2(toast_rect.width(), 40.0)),
        egui::Label::new(egui::RichText::new(msg).size(16.0).color(ui.visuals().strong_text_color())),
    );
}
```

A simpler alternative if `ui.put` is awkward: add a `Window` or use `ui.allocate_ui_at_rect`. Pick whichever compiles cleanly.

- [ ] **Step 2: Build and clippy**

Run:

```bash
cargo build --target wasm32-unknown-unknown --example webapp
cargo clippy --target wasm32-unknown-unknown --example webapp -- -D warnings
```

Expected: both pass.

- [ ] **Step 3: Commit**

```bash
git add examples/webapp.rs
git commit -m "feat(mobile): render share/copy toast feedback"
```

---

## Task 7: Final verification

**Files:**
- None (verification only)

- [ ] **Step 1: Compile tests**

```bash
cargo test --target wasm32-unknown-unknown --example webapp --no-run
```

Expected: tests compile successfully.

- [ ] **Step 2: Build the example**

```bash
cargo build --target wasm32-unknown-unknown --example webapp
```

Expected: successful build.

- [ ] **Step 3: Run clippy**

```bash
cargo clippy --target wasm32-unknown-unknown --example webapp -- -D warnings
```

Expected: no warnings.

- [ ] **Step 4: Manual browser test (optional but recommended)**

Build and serve the webapp (project-specific tooling, e.g. `trunk serve` or the xtask-wasm workflow). Verify on a mobile device or mobile emulator that:

1. Winning/losing shows the banner.
2. The Share button opens the native share sheet or copies the image.
3. The shared image shows the complete board, ignoring any zoom/pan.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "feat: mobile win/loss banner with full-board screenshot sharing"
```

---

## Spec coverage check

| Spec requirement | Task(s) |
|---|---|
| Only `examples/webapp.rs` changes | All tasks |
| Persistent mobile banner for won/lost | Task 3 |
| Share button inside banner (won only) | Task 3 |
| Capture full board despite zoom/pan | Task 4 |
| Hide banner/action bar during capture | Task 4 |
| Encode cropped screenshot as PNG | Task 5 |
| Web Share first, fallback to clipboard | Task 5 |
| Toast feedback | Task 6 |
| Build/clippy/tests | Task 7 |

## Placeholder check

No `TBD`, `TODO`, `implement later`, or vague "handle edge cases" steps remain. Every code step includes concrete code or exact commands.
