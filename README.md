# egui-minesweeper

[![crates.io](https://img.shields.io/crates/v/egui-minesweeper.svg)](https://crates.io/crates/egui-minesweeper)
[![docs.rs](https://docs.rs/egui-minesweeper/badge.svg)](https://docs.rs/egui-minesweeper)
[![deps.rs](https://deps.rs/repo/github/cecton/egui-minesweeper/status.svg)](https://deps.rs/repo/github/cecton/egui-minesweeper)
[![CI](https://github.com/cecton/egui-minesweeper/actions/workflows/ci.yml/badge.svg)](https://github.com/cecton/egui-minesweeper/actions/workflows/ci.yml)
[![Rust version](https://img.shields.io/badge/rustc-1.80+-ab6000.svg)](https://blog.rust-lang.org/2024/07/25/Rust-1.80.0.html)
[![License](https://img.shields.io/crates/l/egui-minesweeper.svg)](https://github.com/cecton/egui-minesweeper#license)
[![Changelog](https://img.shields.io/badge/changelog-Keep%20a%20Changelog%20v1.1.0-%23E05735)](CHANGELOG.md)
[![Live demo](https://img.shields.io/badge/demo-live-brightgreen)](https://cecton.github.io/egui-minesweeper)

A self-contained Minesweeper game library for [egui](https://github.com/emilk/egui).

## Features

- Pure game logic struct (`MinesweeperGame`) with no egui dependency — usable headlessly or with any renderer
- Ready-to-use egui `Widget` (`MinesweeperWidget`) that renders a fully interactive board
- Safe first click: mines are placed on the first reveal, guaranteeing the player can never lose immediately
- Iterative flood-fill reveal (no recursion, safe on large boards)
- Classic Minesweeper cell styling: raised hidden cells, numbered revealed cells, flagging, mine reveal on loss

## Usage

Add the dependency:

```toml
[dependencies]
egui-minesweeper = "0.1"
```

Then use it in your egui app:

```rust
use egui_minesweeper::{MinesweeperGame, MinesweeperWidget};

// Store the game in your app state
let mut game = MinesweeperGame::new(16, 16, 40);

// Inside your egui update/UI closure:
ui.add(MinesweeperWidget::new(&mut game));

// Optionally set a fixed cell size (otherwise fills available space):
ui.add(MinesweeperWidget::new(&mut game).cell_size(32.0));
```

After each frame you can inspect `game.status` to check for a win or loss:

```rust
use egui_minesweeper::GameStatus;

match game.status {
    GameStatus::Playing => {}
    GameStatus::Won => println!("You won!"),
    GameStatus::Lost => println!("You lost!"),
}
```

To start a new game with the same settings:

```rust
game.reset();
```

## egui version compatibility

| egui-minesweeper | egui |
|------------------|------|
| 0.1              | 0.34 |

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
