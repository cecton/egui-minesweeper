# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](keep_a_changelog) and this project adheres to [Semantic
Versioning](semver).

## [Unreleased]

### Added

- Added `CellState::Marked` as a blue visual marker state.
- Right-click interaction now supports a 3-state cycle:
  `Hidden -> Flagged -> Marked -> Hidden`.

### Changed

- **BREAKING** Renamed `MinesweeperGame::toggle_flag` to `MinesweeperGame::cycle_flag`.
- Revealing now allows cells in `Marked` state (same as hidden cells).
- Refactored widget rendering by extracting helper functions:
  `draw_hidden_base` and `draw_flag`.

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
[Unreleased]: https://github.com/cecton/egui-minesweeper/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/cecton/egui-minesweeper/releases/tag/v0.1.0
