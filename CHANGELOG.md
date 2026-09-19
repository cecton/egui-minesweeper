# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](keep_a_changelog) and this project adheres to [Semantic
Versioning](semver).

## [Unreleased]

## [0.3.0] - 2026-09-19

### Added

- Refresh icon on the new game button.

### Fixed

- The mobile hamburger menu no longer stays open if you resize/rotate from mobile to a desktop-width viewport while it's open — it's now only shown while in mobile mode, and closed automatically when switching back to desktop.
- Clamped the hamburger menu's minimum width to zero so it can't go negative on a very narrow viewport.
- Adapted the mobile cell size to the board size to avoid a blurry `Scene` downscale.
- The hamburger menu modal is now gated on mobile mode.

### Changed

- The mobile/touch-device detection now queries `matchMedia("(pointer: coarse)")` once at startup and caches the result, instead of re-querying it (a wasm/JS FFI call) on every frame.
- Updated egui to 0.36.

## [0.1.6] - 2026-07-04

### Added

- Game status emoji (🎉 green / 💥 red) and share button inline in the mobile action bar when game ends.
- File download fallback for screenshot sharing via blob URL and hidden `<a>` click.
- `download_blob` helper for downloading screenshots when Web Share is unavailable.
- Persist theme preference and difficulty preset across sessions via eframe storage.
  - Theme preference persists automatically through eframe/egui's built-in memory
    persistence (`persist_egui_memory` left at its default).
  - Implements `eframe::App::save()` to persist `Preset` via `eframe::set_value`, loaded
    back in `MinesweeperApp::new()`.
  - Enables eframe `persistence` feature and adds `serde` dependency.

### Fixed

- Board mine placement is now properly randomized on the web (wasm32) build — `fastrand` was
  missing the `js` feature needed to seed from `crypto.getRandomValues()`, so games were
  previously seeded deterministically.

## [0.1.5] - 2026-06-26

### Fixed

- Pixel-snap hidden tile bevel for clean rendering under zoom.

## [0.1.4] - 2026-06-16

### Added

- Optional row and column labels for easier cell reference on the board.
  - `MinesweeperWidget::show_labels()` builder method to enable labels.
  - Spreadsheet-style column letters (A, B, ..., Z, AA, AB, ...) along the top edge.
  - Numeric row labels (1, 2, 3, ...) along the left edge.
  - The widget auto-sizes to accommodate the labels when enabled.
  - Labels render in a monospace font at half cell size, respecting the active egui theme.
  - Feature is opt-in and defaults to disabled for backward compatibility.

## [0.1.3] - 2026-06-14

### Added

- PWA support: service worker with cache management, web manifest, and icons.
- Mobile-responsive layout with `InteractionMode::SelectOnly` for touch interaction.
- `MinesweeperGame::flag()`, `mark()`, `clear_marker()` methods for direct cell state control.
- Bottom action bar on mobile with flag, reveal, and mark buttons.
- Hamburger menu on mobile for restart, new game, and dark mode toggle.
- Selection highlight on hovered/selected cells.

### Changed

- **BREAKING** `MinesweeperWidget` now accepts `interaction_mode` and `selected_cell` builders for controlling interaction style.
- Web UI: redesigned top bar with difficulty presets, menu refinement, and consistent icon sizing.

## [0.1.2] - 2026-06-11

### Changed

- First click now reveals the clicked cell and all 8 neighbors, guaranteeing an
  open area instead of a single cell.

## [0.1.1] - 2026-05-10

### Added

- Added `CellState::Marked` as a blue visual marker state.
- Right-click interaction now supports a 3-state cycle:
  `Hidden -> Flagged -> Marked -> Hidden`.
- Added optional question mark mode (3-state cycle skips `Marked` by default).
- Web example now includes a `Dark mode` toggle to switch between light and dark visuals.
- Web example redesigned with egui-demo-style top bar.

### Changed

- **BREAKING** Renamed `MinesweeperGame::toggle_flag` to `MinesweeperGame::cycle_flag`.
- Refactored widget rendering by extracting helper functions:
  `draw_hidden_base` and `draw_flag`.
- `MinesweeperWidget` now adapts its visuals to the active egui theme (light/dark), including cell,
  mine, flag, and number colors.

## [0.1.0] - 2026-04-19

### Added

- Initial release of `egui-minesweeper`
- `MinesweeperGame` core game logic API (renderer-agnostic).
- `MinesweeperWidget` egui widget for interactive board rendering.
- Safe first click behavior (mines placed on first reveal).
- Iterative flood-fill reveal for empty cells.
- Classic Minesweeper visual style (hidden/revealed/flagged cells, mine reveal on loss).
- Web example and GitHub Pages deployment workflow.

[keep_a_changelog]: https://keepachangelog.com/en/1.1.0
[semver]: https://semver.org/spec/v2.0.0.html
[Unreleased]: https://github.com/cecton/egui-minesweeper/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.6
[0.1.5]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.5
[0.1.4]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.4
[0.1.3]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.3
[0.1.2]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.2
[0.1.1]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.1
[0.1.0]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.0
