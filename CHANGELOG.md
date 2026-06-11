# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](keep_a_changelog) and this project adheres to [Semantic
Versioning](semver).

## [Unreleased]

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
[Unreleased]: https://github.com/cecton/egui-minesweeper/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.2
[0.1.1]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.1
[0.1.0]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.0
