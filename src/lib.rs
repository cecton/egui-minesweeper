#![doc = include_str!("../README.md")]

use egui::{
    Align2, Color32, CornerRadius, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Vec2, Widget,
};

// ─── Game types ────────────────────────────────────────────────────────────────

/// The visibility state of a single cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellState {
    /// The cell has not been revealed or flagged yet.
    Hidden,
    /// The cell has been revealed by the player.
    Revealed,
    /// The player has placed a flag on this cell.
    Flagged,
    /// A blue marker used only as a visual indicator (not counted as a flag).
    Marked,
}

/// A single cell on the Minesweeper board.
#[derive(Clone, Debug)]
pub struct Cell {
    /// Whether this cell contains a mine.
    pub is_mine: bool,
    /// Current visibility state of the cell.
    pub state: CellState,
    /// Number of mines in the 8 neighboring cells (0 if this cell is a mine).
    pub adjacent_mines: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            is_mine: false,
            state: CellState::Hidden,
            adjacent_mines: 0,
        }
    }
}

/// The current status of the game.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameStatus {
    /// Waiting for the first click, or game is in progress.
    Playing,
    /// All non-mine cells have been revealed — the player won.
    Won,
    /// The player revealed a mine — the game is over.
    Lost,
}

// ─── Game logic ────────────────────────────────────────────────────────────────

/// The Minesweeper game state.
///
/// Holds the board dimensions, the cell grid, and the current game status.
/// Mines are not placed until the first [`reveal`](Self::reveal) call, ensuring
/// the player can never lose on their first click.
pub struct MinesweeperGame {
    /// Board width in cells.
    pub width: usize,
    /// Board height in cells.
    pub height: usize,
    /// Total number of mines on the board.
    pub mines: usize,
    /// Flat row-major grid of cells (`cells[y * width + x]`).
    pub cells: Vec<Cell>,
    /// Current game status.
    pub status: GameStatus,
    initialized: bool,
}

impl MinesweeperGame {
    /// Create a new game. Mines are placed on the first [`reveal`] call so
    /// the player can never lose on the very first click.
    pub fn new(width: usize, height: usize, mines: usize) -> Self {
        assert!(width > 0 && height > 0);
        let mines = mines.min(width * height - 1);
        Self {
            width,
            height,
            mines,
            cells: vec![Cell::default(); width * height],
            status: GameStatus::Playing,
            initialized: false,
        }
    }

    /// Reset to a fresh game with the same parameters.
    pub fn reset(&mut self) {
        *self = Self::new(self.width, self.height, self.mines);
    }

    /// Number of flags the player has placed.
    pub fn flags_placed(&self) -> usize {
        self.cells
            .iter()
            .filter(|c| c.state == CellState::Flagged)
            .count()
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    #[inline]
    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut out = Vec::with_capacity(8);
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32 {
                    out.push((nx as usize, ny as usize));
                }
            }
        }
        out
    }

    fn initialize(&mut self, safe_x: usize, safe_y: usize) {
        let safe = self.idx(safe_x, safe_y);

        // Collect candidate positions and shuffle them with fastrand.
        let mut positions: Vec<usize> = (0..self.width * self.height)
            .filter(|&i| i != safe)
            .collect();

        // Partial Fisher-Yates – only shuffle the first `mines` elements.
        let mines = self.mines.min(positions.len());
        for i in 0..mines {
            let j = i + fastrand::usize(0..(positions.len() - i));
            positions.swap(i, j);
        }

        for &pos in &positions[..mines] {
            self.cells[pos].is_mine = true;
        }

        // Pre-compute adjacent mine counts.
        for y in 0..self.height {
            for x in 0..self.width {
                if !self.cells[self.idx(x, y)].is_mine {
                    let count = self
                        .neighbors(x, y)
                        .iter()
                        .filter(|&&(nx, ny)| self.cells[self.idx(nx, ny)].is_mine)
                        .count();
                    let idx = self.idx(x, y);
                    self.cells[idx].adjacent_mines = count as u8;
                }
            }
        }

        self.initialized = true;
    }

    fn check_win(&mut self) {
        let all_safe_revealed = self
            .cells
            .iter()
            .all(|c| c.is_mine || c.state == CellState::Revealed);
        if all_safe_revealed {
            self.status = GameStatus::Won;
        }
    }

    // ── Public actions ────────────────────────────────────────────────────────

    /// Reveal a cell. Recursively reveals empty neighbours (flood-fill).
    pub fn reveal(&mut self, x: usize, y: usize) {
        if self.status != GameStatus::Playing {
            return;
        }
        if !self.initialized {
            self.initialize(x, y);
        }

        // Iterative flood-fill to avoid stack overflows on large boards.
        let mut stack = vec![(x, y)];
        while let Some((cx, cy)) = stack.pop() {
            let idx = self.idx(cx, cy);
            if !matches!(self.cells[idx].state, CellState::Hidden | CellState::Marked) {
                continue;
            }
            self.cells[idx].state = CellState::Revealed;

            if self.cells[idx].is_mine {
                self.status = GameStatus::Lost;
                // Reveal all mines on loss.
                for cell in &mut self.cells {
                    if cell.is_mine {
                        cell.state = CellState::Revealed;
                    }
                }
                return;
            }

            if self.cells[idx].adjacent_mines == 0 {
                for neighbor in self.neighbors(cx, cy) {
                    stack.push(neighbor);
                }
            }
        }

        self.check_win();
    }

    /// Cycle marker state on an unrevealed cell: hidden -> red flag -> blue marker -> hidden.
    pub fn cycle_flag(&mut self, x: usize, y: usize) {
        if self.status != GameStatus::Playing {
            return;
        }
        let idx = self.idx(x, y);
        match self.cells[idx].state {
            CellState::Hidden => self.cells[idx].state = CellState::Flagged,
            CellState::Flagged => self.cells[idx].state = CellState::Marked,
            CellState::Marked => self.cells[idx].state = CellState::Hidden,
            CellState::Revealed => {}
        }
    }
}

// ─── egui widget ───────────────────────────────────────────────────────────────

/// An egui widget that renders the minesweeper grid.
///
/// Left-click reveals a cell;
/// right-click cycles between hidden -> red flag -> blue marker -> hidden.
///
/// ```no_run
/// ui.add(egui_minesweeper::MinesweeperWidget::new(&mut game));
/// ```
pub struct MinesweeperWidget<'a> {
    game: &'a mut MinesweeperGame,
    cell_size: Option<f32>,
}

impl<'a> MinesweeperWidget<'a> {
    pub fn new(game: &'a mut MinesweeperGame) -> Self {
        Self {
            game,
            cell_size: None,
        }
    }

    /// Override the size (in logical pixels) of each cell.
    /// When not set, the cell size is computed automatically to fill the
    /// available space of the parent container.
    pub fn cell_size(mut self, size: f32) -> Self {
        self.cell_size = Some(size);
        self
    }
}

fn number_color(n: u8) -> Color32 {
    match n {
        1 => Color32::from_rgb(0, 0, 255),
        2 => Color32::from_rgb(0, 128, 0),
        3 => Color32::from_rgb(200, 0, 0),
        4 => Color32::from_rgb(0, 0, 128),
        5 => Color32::from_rgb(128, 0, 0),
        6 => Color32::from_rgb(0, 128, 128),
        7 => Color32::BLACK,
        _ => Color32::DARK_GRAY,
    }
}

fn draw_cell(painter: &egui::Painter, rect: Rect, cell: &Cell, cell_size: f32) {
    let inner = rect.shrink(1.0);
    let rounding = CornerRadius::same(2);

    match cell.state {
        CellState::Hidden => draw_hidden_base(painter, inner, rounding),
        CellState::Flagged | CellState::Marked => {
            draw_hidden_base(painter, inner, rounding);
            let color = if cell.state == CellState::Flagged {
                Color32::RED
            } else {
                Color32::BLUE
            };
            draw_flag(painter, inner, cell_size, color);
        }
        CellState::Revealed => {
            if cell.is_mine {
                painter.rect_filled(inner, CornerRadius::ZERO, Color32::from_rgb(255, 80, 80));
                // Draw a simple mine: filled circle with spikes.
                let c = rect.center();
                let r = cell_size * 0.22;
                painter.circle_filled(c, r, Color32::BLACK);
                // 8 spikes
                for i in 0..8u32 {
                    let angle = i as f32 * std::f32::consts::TAU / 8.0;
                    let inner_pt = c + Vec2::new(angle.cos(), angle.sin()) * r;
                    let outer_pt = c + Vec2::new(angle.cos(), angle.sin()) * (r * 1.7);
                    painter.line_segment([inner_pt, outer_pt], Stroke::new(2.0, Color32::BLACK));
                }
                // Shine dot
                painter.circle_filled(c + Vec2::new(-r * 0.3, -r * 0.3), r * 0.25, Color32::WHITE);
            } else {
                painter.rect_filled(inner, CornerRadius::ZERO, Color32::from_rgb(210, 210, 210));
                painter.rect_stroke(
                    inner,
                    CornerRadius::ZERO,
                    Stroke::new(0.5, Color32::GRAY),
                    StrokeKind::Inside,
                );
                if cell.adjacent_mines > 0 {
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        cell.adjacent_mines.to_string(),
                        FontId::monospace(cell_size * 0.58),
                        number_color(cell.adjacent_mines),
                    );
                }
            }
        }
    }
}

impl Widget for MinesweeperWidget<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let cell_size = self.cell_size.unwrap_or_else(|| {
            let available = ui.available_size();
            let by_width = available.x / self.game.width as f32;
            let by_height = available.y / self.game.height as f32;
            by_width.min(by_height).max(1.0)
        });

        let total = Vec2::new(self.game.width as f32, self.game.height as f32) * cell_size;

        let (response, painter) = ui.allocate_painter(total, Sense::click());
        let origin = response.rect.min;

        // ── Input handling ────────────────────────────────────────────────────
        if (response.clicked() || response.secondary_clicked())
            && self.game.status == GameStatus::Playing
        {
            if let Some(pos) = response.interact_pointer_pos() {
                let local = pos - origin;
                let cx = (local.x / cell_size).floor() as usize;
                let cy = (local.y / cell_size).floor() as usize;
                if cx < self.game.width && cy < self.game.height {
                    if response.clicked() {
                        self.game.reveal(cx, cy);
                    } else {
                        self.game.cycle_flag(cx, cy);
                    }
                }
            }
        }

        // ── Painting ──────────────────────────────────────────────────────────
        for y in 0..self.game.height {
            for x in 0..self.game.width {
                let cell_rect = Rect::from_min_size(
                    origin + Vec2::new(x as f32, y as f32) * cell_size,
                    Vec2::splat(cell_size),
                );
                let cell = &self.game.cells[y * self.game.width + x];
                draw_cell(&painter, cell_rect, cell, cell_size);
            }
        }

        response
    }
}

fn draw_hidden_base(painter: &egui::Painter, inner: Rect, rounding: CornerRadius) {
    // Raised 3-D look (classic Minesweeper style).
    painter.rect_filled(inner, rounding, Color32::from_rgb(192, 192, 192));
    // Highlight edges (top-left bright, bottom-right dark).
    let tl = inner.left_top();
    let tr = inner.right_top();
    let bl = inner.left_bottom();
    let br = inner.right_bottom();
    let highlight = Color32::WHITE;
    let shadow = Color32::from_rgb(100, 100, 100);
    let w = 2.0;
    painter.line_segment([tl, tr], Stroke::new(w, highlight));
    painter.line_segment([tl, bl], Stroke::new(w, highlight));
    painter.line_segment([tr, br], Stroke::new(w, shadow));
    painter.line_segment([bl, br], Stroke::new(w, shadow));
}

fn draw_flag(painter: &egui::Painter, inner: Rect, cell_size: f32, color: Color32) {
    // Draw a simple flag: a filled triangle for the flag and a pole.
    let cx = inner.center().x;
    let top = inner.min.y + cell_size * 0.15;
    let mid = inner.min.y + cell_size * 0.55;
    let bot = inner.max.y - cell_size * 0.15;
    // Pole
    painter.line_segment(
        [Pos2::new(cx, top), Pos2::new(cx, bot)],
        Stroke::new(2.0, Color32::BLACK),
    );
    // Flag triangle
    let flag_pts = vec![
        Pos2::new(cx, top),
        Pos2::new(cx + cell_size * 0.35, (top + mid) / 2.0),
        Pos2::new(cx, mid),
    ];
    painter.add(egui::Shape::convex_polygon(flag_pts, color, Stroke::NONE));
}
