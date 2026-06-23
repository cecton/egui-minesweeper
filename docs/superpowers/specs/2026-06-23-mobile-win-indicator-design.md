# Mobile win/loss indicator and screenshot sharing

## Goal
Add an unintrusive win/loss indicator to the mobile UI of the `egui-minesweeper` webapp example, and let the user share or copy a screenshot of the completed board.

## Scope
- Only `examples/webapp.rs` is modified.
- The `egui-minesweeper` library crate is left untouched.

## Decisions from brainstorming
| Question | Decision |
|---|---|
| Where does this live? | Webapp example only. |
| Won only or won + lost? | Show the indicator for both won and lost. |
| Indicator persistence | Persistent banner until a new game starts. |
| Screenshot content | The board only, captured in full even if the user has zoomed/panned the `Scene`. |
| Share button placement | Inside the result banner. |
| Share vs copy | One "Share" button that tries the Web Share API first, then falls back to copying the image to the clipboard. |

## UI design

### Mobile result banner
A thin top panel appears in the mobile layout when `game.status != Playing`.

- **Won**: green text “🎉 You won!”
- **Lost**: red text “💥 Boom!”
- **Right side**: a small “📤 Share” button (shown only when the game is won).
- The banner has a subtle rounded background using the current theme’s panel fill. It sits above the board and does not cover it.
- The banner is hidden during the one-frame screenshot capture so it is not part of the shared image.

## Architecture

### New state in `MinesweeperApp`

```rust
enum ShareState {
    Idle,
    Capture {
        restore_scene: egui::Rect,
        wait_frames: u8,
    },
}
```

- `restore_scene` stores the user’s current `scene_rect` before it is temporarily reset.
- `wait_frames` is a guard that abandons the capture if the backend never delivers a screenshot event.

A transient toast field is also added:

```rust
toast: Option<(String, f32)>, // message + remaining seconds
```

### Capture flow

1. **User taps Share.**
   - `share_state` becomes `Capture { restore_scene, wait_frames: 5 }`.

2. **The next frame renders the capture view.**
   - The banner and bottom action bar are not drawn.
   - `self.scene_rect` is set to a rectangle covering the full board:
     ```rust
     let board_size = vec2(width, height) * MOBILE_CELL_SIZE;
     self.scene_rect = Some(Rect::from_min_size(Pos2::ZERO, board_size));
     ```
   - The `Scene` is rendered with an unconstrained zoom range (e.g. `0.0..=f32::INFINITY`) so the whole board is guaranteed to fit.
   - The `MinesweeperWidget` is rendered inside the `Scene`.
   - The board’s exact screen rectangle is computed by replicating the scene-to-screen transform that `egui::Scene` uses (`fit_to_rect_in_scene`), then converted from logical points to physical pixels with `ui.ctx().input(|i| i.viewport().native_pixels_per_point)`.
   - `ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot)` is called.
   - `self.scene_rect` is restored immediately so normal pan/zoom returns on the following frame.

3. **Receiving the screenshot.**
   - Each frame, input events are scanned for `egui::Event::Screenshot { image, .. }`.
   - The image is cropped to the stored board rectangle, encoded to PNG, and shared/copied.
   - If no event arrives before `wait_frames` reaches zero, the capture is abandoned and an error toast is shown.

### Image encoding
- The cropped `egui::ColorImage` is encoded to PNG with the `image` crate.
- The `image` crate is added as a wasm32 dev-dependency with `default-features = false` and `features = ["png"]`.
- A JS `Blob` / `File` is built from the PNG bytes for Web Share and clipboard writes.

### Share / copy fallback chain
1. Try `navigator.share()` if `navigator.canShare()` reports the file is shareable.
2. Otherwise write the PNG `File` to `navigator.clipboard` through a `ClipboardItem`.
3. If both fail, show a toast: “Couldn’t share or copy image.”

## Error handling
- Crop rectangle is clamped to the screenshot image bounds.
- Missing browser APIs fall through the chain above.
- Screenshot event timeout after a few frames.
- Clipboard/share failures surface as a toast.

## Verification
- `cargo build --target wasm32-unknown-unknown --example webapp` must succeed.
- Clippy must pass on the example.
- Manual browser testing is required because Web Share and clipboard image APIs depend on a real browser context (secure context, user gesture, etc.).
