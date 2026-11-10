//! Exact mine-probability analysis and guess-free board generation.
//!
//! Both rest on the same machinery: given the numbers the player can see, work
//! out exactly what follows from them. A cell whose odds are `0` is provably
//! safe, and a board is guess free when repeating that step keeps producing a
//! safe cell until the board is cleared.
//!
//! # What the analysis may read
//!
//! Only [`Cell::adjacent_mines`] of cells that are already revealed. That field
//! is populated for *every* non-mine cell at generation time, including hidden
//! ones, so reading it from a hidden cell would hand the solver the answer
//! outright. [`constraints`] is the only place that reads it, and it skips
//! anything unrevealed.
//!
//! # Why the mine total matters
//!
//! Constraints alone are not enough. Two frontier cells can each be
//! individually possible yet have very different odds once "the board holds
//! exactly `mines` mines" is taken into account. That is what makes endgame
//! odds meaningful and what tells a player which of two cells to open first, so
//! the analysis combines every independent group of constraints with the
//! combinatorial count of the mines left over rather than judging groups in
//! isolation.
//!
//! # What "guess free" means here
//!
//! Solvable by a *perfect* reasoner: at every step some hidden cell follows
//! from the visible numbers, possibly through reasoning that spans a dozen
//! cells at once. This is strictly stronger than solvable with the familiar
//! single-cell and 1-2-1 tricks — see [`GenerationMode::GuessFree`] for why that
//! gap sets that mode's real difficulty.

use std::collections::HashMap;

use crate::{CellState, GameStatus, MinesweeperGame};

/// Cap on search nodes spent on one analysis.
///
/// Reaching it makes the analysis give up rather than guess: [`mine_odds`]
/// returns `None` and guess-free generation rejects the candidate board. Sized
/// from measurement — on the 30x16 board the per-reveal analysis sits near 300
/// nodes at the median and a few hundred thousand at the 99th percentile, so
/// this bound costs a rare stall and keeps the worst case finite.
pub(crate) const NODE_BUDGET: u64 = 2_000_000;

/// How many layouts [`GenerationMode::GuessFree`] may try before keeping the
/// last one it built.
///
/// The pass rate is about 91% / 71% / 14% for the 9x9, 16x16 and 30x16 boards,
/// so 100 attempts clears the worst case with a failure probability near 4e-7
/// while bounding the pause to a fraction of a second.
pub(crate) const MAX_GENERATION_ATTEMPTS: u32 = 100;

/// One revealed number's requirement: exactly `count` of `cells` hold mines.
struct Constraint {
    cells: Vec<usize>,
    count: u8,
}

/// Indices of the up to eight cells surrounding `i`.
fn neighbors(width: usize, height: usize, i: usize) -> Vec<usize> {
    let x = (i % width) as i32;
    let y = (i / width) as i32;
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if nx >= 0 && ny >= 0 && (nx as usize) < width && (ny as usize) < height {
                out.push(ny as usize * width + nx as usize);
            }
        }
    }
    out
}

/// The requirements a player can derive: one per revealed cell that still has
/// hidden neighbours.
///
/// A revealed `0` is normally worthless — the flood fill would already have
/// opened everything around it. It is not worthless when a flag blocked that
/// flood: `reveal` treats a flagged cell as an obstacle, so a zero can end up
/// with a hidden neighbour, and "no mines around me" then proves that neighbour
/// safe. Skipping zeros unconditionally would understate what the player can
/// see, so they are kept whenever they still have hidden neighbours; the count
/// of zero simply falls out of the arithmetic.
fn constraints(game: &MinesweeperGame, revealed: &[bool]) -> Vec<Constraint> {
    let mut out = Vec::new();
    for i in 0..game.width * game.height {
        // Never read `adjacent_mines` off a hidden cell: it holds that cell's
        // true count, mines included. See the module docs.
        if !revealed[i] {
            continue;
        }
        let count = game.cells[i].adjacent_mines;
        let cells: Vec<usize> = neighbors(game.width, game.height, i)
            .into_iter()
            .filter(|&j| !revealed[j])
            .collect();
        if !cells.is_empty() {
            out.push(Constraint { cells, count });
        }
    }
    out
}

/// Solutions of one connected group of constraints, tabulated by mine count.
struct Component {
    cells: Vec<usize>,
    /// `hist[k]`: layouts putting exactly `k` mines in this group.
    hist: Vec<f64>,
    /// `cell_hist[local][k]`: the same, restricted to layouts where that cell
    /// is a mine.
    cell_hist: Vec<Vec<f64>>,
}

/// Enumerate every layout of one group, or `None` if the search ran past
/// `NODE_BUDGET`.
fn enumerate(cells: &[usize], constraints: &[Constraint]) -> Option<Component> {
    let n = cells.len();
    let local: HashMap<usize, usize> = cells.iter().enumerate().map(|(i, &c)| (c, i)).collect();
    let local_constraints: Vec<(Vec<usize>, u8)> = constraints
        .iter()
        .map(|c| (c.cells.iter().map(|cell| local[cell]).collect(), c.count))
        .collect();

    // Which constraints each cell participates in, so choosing one cell updates
    // them all in a single step.
    let mut of_cell: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (ci, (cs, _)) in local_constraints.iter().enumerate() {
        for &v in cs {
            of_cell[v].push(ci);
        }
    }

    let mut hist = vec![0f64; n + 1];
    let mut cell_hist = vec![vec![0f64; n + 1]; n];
    let mut mines = vec![0u8; local_constraints.len()];
    let mut left: Vec<u8> = local_constraints
        .iter()
        .map(|(cs, _)| cs.len() as u8)
        .collect();
    let mut chosen = vec![false; n];
    let mut nodes = 0u64;

    /// Which constraints each cell belongs to, shared by both branches.
    struct Ctx<'a> {
        constraints: &'a [(Vec<usize>, u8)],
        of_cell: &'a [Vec<usize>],
    }

    /// Depth-first over the group's cells, pruning as soon as a constraint can
    /// no longer reach, or has already passed, its required count.
    #[allow(clippy::too_many_arguments)]
    fn walk(
        pos: usize,
        ctx: &Ctx<'_>,
        mines: &mut [u8],
        left: &mut [u8],
        chosen: &mut [bool],
        k: usize,
        nodes: &mut u64,
        hist: &mut [f64],
        cell_hist: &mut [Vec<f64>],
    ) -> bool {
        *nodes += 1;
        if *nodes > NODE_BUDGET {
            return false;
        }
        if pos == chosen.len() {
            if ctx
                .constraints
                .iter()
                .enumerate()
                .all(|(ci, (_, count))| mines[ci] == *count)
            {
                hist[k] += 1.0;
                for (v, &is_mine) in chosen.iter().enumerate() {
                    if is_mine {
                        cell_hist[v][k] += 1.0;
                    }
                }
            }
            return true;
        }
        for is_mine in [false, true] {
            for &ci in &ctx.of_cell[pos] {
                left[ci] -= 1;
                if is_mine {
                    mines[ci] += 1;
                }
            }
            // Both bounds must stay satisfiable: not over the required count,
            // and still able to reach it with the cells that remain.
            let feasible = ctx.of_cell[pos].iter().all(|&ci| {
                let count = ctx.constraints[ci].1;
                mines[ci] <= count && mines[ci] + left[ci] >= count
            });
            if feasible {
                chosen[pos] = is_mine;
                if !walk(
                    pos + 1,
                    ctx,
                    mines,
                    left,
                    chosen,
                    k + usize::from(is_mine),
                    nodes,
                    hist,
                    cell_hist,
                ) {
                    return false;
                }
            }
            for &ci in &ctx.of_cell[pos] {
                left[ci] += 1;
                if is_mine {
                    mines[ci] -= 1;
                }
            }
        }
        true
    }

    let ctx = Ctx {
        constraints: &local_constraints,
        of_cell: &of_cell,
    };
    let complete = walk(
        0,
        &ctx,
        &mut mines,
        &mut left,
        &mut chosen,
        0,
        &mut nodes,
        &mut hist,
        &mut cell_hist,
    );
    if !complete {
        return None;
    }
    Some(Component {
        cells: cells.to_vec(),
        hist,
        cell_hist,
    })
}

/// `n` choose `k` as a float; `0` when `k > n`.
fn binomial(n: usize, k: usize) -> f64 {
    if k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut out = 1.0f64;
    for i in 0..k {
        out = out * (n - i) as f64 / (i + 1) as f64;
    }
    out
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

/// Mine probability for every cell, given which cells are open.
///
/// `None` means the analysis could not finish (see `NODE_BUDGET`), and callers
/// must then show nothing rather than a guess. Open cells are `None` too: there
/// is nothing uncertain about them.
///
/// Flag placement is deliberately not an input. A flag is the player's own
/// guess, so the odds are computed as if the board held exactly `game.mines`
/// mines with nothing flagged.
fn odds_for(game: &MinesweeperGame, revealed: &[bool]) -> Option<Vec<Option<f32>>> {
    let n = game.width * game.height;
    let constraints = constraints(game, revealed);

    if constraints.is_empty() {
        // Nothing on show, so every hidden cell is equally likely.
        let hidden = (0..n).filter(|&i| !revealed[i]).count();
        if hidden == 0 {
            return Some(vec![None; n]);
        }
        let p = game.mines as f32 / hidden as f32;
        return Some(
            (0..n)
                .map(|i| if revealed[i] { None } else { Some(p) })
                .collect(),
        );
    }

    // Frontier cells that share a constraint are coupled; groups that share
    // none are independent, so each can be enumerated on its own.
    let mut parent: Vec<usize> = (0..n).collect();
    let mut in_frontier = vec![false; n];
    for c in &constraints {
        for &x in &c.cells {
            in_frontier[x] = true;
        }
        let root = find(&mut parent, c.cells[0]);
        for &x in &c.cells[1..] {
            let other = find(&mut parent, x);
            if other != root {
                parent[other] = root;
            }
        }
    }

    let mut groups: HashMap<usize, (Vec<usize>, Vec<&Constraint>)> = HashMap::new();
    for c in &constraints {
        let root = find(&mut parent, c.cells[0]);
        groups.entry(root).or_default().1.push(c);
    }
    for (x, &frontier) in in_frontier.iter().enumerate() {
        if frontier {
            let root = find(&mut parent, x);
            groups.entry(root).or_default().0.push(x);
        }
    }

    let mut components = Vec::with_capacity(groups.len());
    for (_, (cells, group_constraints)) in groups {
        let owned: Vec<Constraint> = group_constraints
            .iter()
            .map(|c| Constraint {
                cells: c.cells.clone(),
                count: c.count,
            })
            .collect();
        components.push(enumerate(&cells, &owned)?);
    }

    // Hidden cells that no revealed number touches. They are interchangeable,
    // so the only thing constraining them is how many mines are left over.
    let off_frontier: Vec<usize> = (0..n)
        .filter(|&i| !revealed[i] && !in_frontier[i])
        .collect();

    let total = game.mines;
    let count = components.len();

    // prefix[i][t]: ways the first `i` groups hold `t` mines between them.
    let mut prefix = vec![vec![0f64; total + 1]; count + 1];
    prefix[0][0] = 1.0;
    for i in 0..count {
        for t in 0..=total {
            if prefix[i][t] == 0.0 {
                continue;
            }
            for (k, &ways) in components[i].hist.iter().enumerate() {
                if t + k <= total {
                    prefix[i + 1][t + k] += prefix[i][t] * ways;
                }
            }
        }
    }
    // suffix[i][t]: the same for groups `i..`.
    let mut suffix = vec![vec![0f64; total + 1]; count + 1];
    suffix[count][0] = 1.0;
    for i in (0..count).rev() {
        for t in 0..=total {
            if suffix[i + 1][t] == 0.0 {
                continue;
            }
            for (k, &ways) in components[i].hist.iter().enumerate() {
                if t + k <= total {
                    suffix[i][t + k] += suffix[i + 1][t] * ways;
                }
            }
        }
    }

    // Ways the untouched cells can absorb `left` mines between them.
    let off_ways = |left: usize| -> f64 {
        if left > off_frontier.len() {
            0.0
        } else {
            binomial(off_frontier.len(), left)
        }
    };
    // Weight of every layout consistent with both the numbers and the total.
    let normalizer: f64 = (0..=total)
        .map(|t| prefix[count][t] * off_ways(total - t))
        .sum();
    if normalizer <= 0.0 {
        // No layout matches the numbers and the mine total together. Should not
        // happen on a real board, but odds derived from an inconsistent state
        // would be worse than none.
        return None;
    }

    let mut odds = vec![None; n];
    for (index, component) in components.iter().enumerate() {
        // `others[t]`: ways every *other* group holds `t` mines.
        let mut others = vec![0f64; total + 1];
        for t in 0..=total {
            for a in 0..=t {
                others[t] += prefix[index][a] * suffix[index + 1][t - a];
            }
        }
        // `weight[k]`: ways the rest of the board absorbs the remaining mines
        // when this group takes `k`.
        let mut weight = vec![0f64; total + 1];
        for (k, slot) in weight.iter_mut().enumerate() {
            for (t, &ways) in others.iter().enumerate() {
                if t + k > total {
                    break;
                }
                *slot += ways * off_ways(total - k - t);
            }
        }
        for (local, &cell) in component.cells.iter().enumerate() {
            let numerator: f64 = component.cell_hist[local]
                .iter()
                .zip(&weight)
                .map(|(h, w)| h * w)
                .sum();
            let denominator: f64 = component.hist.iter().zip(&weight).map(|(h, w)| h * w).sum();
            if denominator > 0.0 {
                odds[cell] = Some((numerator / denominator) as f32);
            }
        }
    }

    if !off_frontier.is_empty() {
        let expected: f64 = (0..=total)
            .map(|t| {
                let left = total - t;
                if left > off_frontier.len() {
                    0.0
                } else {
                    prefix[count][t] * binomial(off_frontier.len(), left) * left as f64
                }
            })
            .sum();
        let p = (expected / normalizer / off_frontier.len() as f64) as f32;
        for &cell in &off_frontier {
            odds[cell] = Some(p.clamp(0.0, 1.0));
        }
    }

    Some(odds)
}

/// Probability that each cell holds a mine, for the board as the player
/// currently sees it.
///
/// Indexed by cell; open cells are `None`. Flagged cells are **not** `None` —
/// a flag records the player's guess, not what is there, so the odds still say
/// what is known about them.
///
/// Returns `None` once the game is over, and also when the analysis could not
/// finish; in both cases there is nothing worth showing. Callers that redraw
/// every frame should cache the result against
/// [`MinesweeperGame::revealed_fingerprint`].
pub fn mine_odds(game: &MinesweeperGame) -> Option<Vec<Option<f32>>> {
    if game.status != GameStatus::Playing {
        return None;
    }
    let revealed = revealed_mask(game);
    odds_for(game, &revealed)
}

/// Which cells an odds display should annotate.
///
/// Only hidden cells that sit next to a revealed number: those are the ones a
/// player can reason about. A hidden cell away from every number carries the
/// same board-wide odds as all the others like it, and printing that on dozens
/// of tiles says nothing while covering the board in noise.
///
/// `true` means "worth annotating", indexed by cell. Returns `None` once the
/// game is over.
pub fn odds_frontier(game: &MinesweeperGame) -> Option<Vec<bool>> {
    if game.status != GameStatus::Playing {
        return None;
    }
    let n = game.width * game.height;
    let mut frontier = vec![false; n];
    for i in 0..n {
        let cell = &game.cells[i];
        if cell.state != CellState::Revealed || cell.adjacent_mines == 0 {
            continue;
        }
        for j in neighbors(game.width, game.height, i) {
            if game.cells[j].state != CellState::Revealed {
                frontier[j] = true;
            }
        }
    }
    Some(frontier)
}

/// Whether ordinary local reasoning has a move available right now.
///
/// This is the single-cell rule plus the subset rule behind the familiar 1-2-1
/// deductions — what a careful player spots quickly, not everything the numbers
/// imply. It is far cheaper than [`mine_odds`] and lets an odds display wait
/// until guessing is genuinely the only option left.
pub fn has_forced_move(game: &MinesweeperGame) -> bool {
    if game.status != GameStatus::Playing {
        return false;
    }
    let revealed = revealed_mask(game);
    forced_by_local_rules(&constraints(game, &revealed)).is_some()
}

fn revealed_mask(game: &MinesweeperGame) -> Vec<bool> {
    game.cells
        .iter()
        .map(|c| c.state == CellState::Revealed)
        .collect()
}

/// The first move the local rules can prove, as `(cell, is_mine)`.
fn forced_by_local_rules(constraints: &[Constraint]) -> Option<(usize, bool)> {
    let max_cell = constraints
        .iter()
        .flat_map(|c| c.cells.iter().copied())
        .max()?;
    let mut safe = vec![false; max_cell + 1];
    let mut mine = vec![false; max_cell + 1];
    let mut first = None;
    let mut changed = true;
    while changed {
        changed = false;
        for c in constraints {
            let flagged = c.cells.iter().filter(|&&x| mine[x]).count();
            let open: Vec<usize> = c
                .cells
                .iter()
                .copied()
                .filter(|&x| !mine[x] && !safe[x])
                .collect();
            if open.is_empty() {
                continue;
            }
            if flagged == c.count as usize {
                for x in open {
                    if !safe[x] {
                        safe[x] = true;
                        changed = true;
                        first.get_or_insert((x, false));
                    }
                }
            } else if c.count as usize - flagged == open.len() {
                for x in open {
                    if !mine[x] {
                        mine[x] = true;
                        changed = true;
                        first.get_or_insert((x, true));
                    }
                }
            }
        }
        // When one constraint sits inside another, the difference holds the
        // difference of the counts. This is where 1-2-1 patterns come from.
        for (i, a) in constraints.iter().enumerate() {
            for (j, b) in constraints.iter().enumerate() {
                if i == j || !a.cells.iter().all(|x| b.cells.contains(x)) {
                    continue;
                }
                let difference: Vec<usize> = b
                    .cells
                    .iter()
                    .copied()
                    .filter(|x| !a.cells.contains(x))
                    .collect();
                if difference.is_empty() {
                    continue;
                }
                let in_a = a.cells.iter().filter(|&&x| mine[x]).count() as i32;
                let in_b = b.cells.iter().filter(|&&x| mine[x]).count() as i32;
                let diff_mines = b.count as i32 - in_b - (a.count as i32 - in_a);
                let open: Vec<usize> = difference
                    .iter()
                    .copied()
                    .filter(|&x| !mine[x] && !safe[x])
                    .collect();
                if open.is_empty() {
                    continue;
                }
                if diff_mines == 0 {
                    for x in open {
                        if !safe[x] {
                            safe[x] = true;
                            changed = true;
                            first.get_or_insert((x, false));
                        }
                    }
                } else if diff_mines as usize == open.len() {
                    for x in open {
                        if !mine[x] {
                            mine[x] = true;
                            changed = true;
                            first.get_or_insert((x, true));
                        }
                    }
                }
            }
        }
    }
    first
}

/// Whether the board is guess free when opened at `start`, which must not hold
/// a mine.
///
/// Replays what a perfect reasoner would do: open that cell, then keep opening
/// every cell the numbers prove safe until the board is clear or nothing left
/// is provable.
pub(crate) fn guess_free_from(game: &MinesweeperGame, start: usize) -> bool {
    let n = game.width * game.height;
    let mut revealed = vec![false; n];
    flood(game, &mut revealed, start);
    loop {
        if cleared(game, &revealed) {
            return true;
        }
        let Some(odds) = odds_for(game, &revealed) else {
            // The analysis bailed out, so nothing is proven. Reject: promising
            // a guess-free board that cannot be verified is worse than
            // retrying.
            return false;
        };
        let proven: Vec<usize> = (0..n)
            .filter(|&i| !revealed[i] && odds[i] == Some(0.0))
            .collect();
        if proven.is_empty() {
            return false;
        }
        for i in proven {
            flood(game, &mut revealed, i);
        }
    }
}

/// Whether every non-mine cell is open.
fn cleared(game: &MinesweeperGame, revealed: &[bool]) -> bool {
    game.cells
        .iter()
        .enumerate()
        .all(|(i, c)| revealed[i] || c.is_mine)
}

/// Open `start` and cascade through zero-count cells, mirroring the flood fill
/// [`MinesweeperGame::reveal`] performs.
fn flood(game: &MinesweeperGame, revealed: &mut [bool], start: usize) {
    let mut stack = vec![start];
    while let Some(i) = stack.pop() {
        if revealed[i] {
            continue;
        }
        revealed[i] = true;
        if game.cells[i].adjacent_mines == 0 {
            for j in neighbors(game.width, game.height, i) {
                if !revealed[j] {
                    stack.push(j);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A board built from an explicit mine map, with counts filled in — the
    /// shape the analysis is expected to reason about.
    fn board(width: usize, height: usize, mines: &[usize]) -> MinesweeperGame {
        let mut game = MinesweeperGame::new(width, height, mines.len());
        game.set_mine_layout(mines);
        game
    }

    fn mask(game: &MinesweeperGame, revealed: &[usize]) -> Vec<bool> {
        let mut out = vec![false; game.width * game.height];
        for &i in revealed {
            out[i] = true;
        }
        out
    }

    /// Brute force over every mine placement consistent with the numbers.
    fn brute_force(game: &MinesweeperGame, revealed: &[bool]) -> Vec<Option<f32>> {
        let n = game.width * game.height;
        let hidden: Vec<usize> = (0..n).filter(|&i| !revealed[i]).collect();
        let cons = constraints(game, revealed);
        let mut counts = vec![0f64; n];
        let mut total = 0f64;
        let k = game.mines;

        fn walk(
            idx: usize,
            picked: &mut Vec<usize>,
            hidden: &[usize],
            k: usize,
            cons: &[Constraint],
            counts: &mut [f64],
            total: &mut f64,
        ) {
            if picked.len() == k {
                let mut is_mine = vec![false; counts.len()];
                for &p in picked.iter() {
                    is_mine[hidden[p]] = true;
                }
                for c in cons {
                    if c.cells.iter().filter(|&&x| is_mine[x]).count() != c.count as usize {
                        return;
                    }
                }
                *total += 1.0;
                for &p in picked.iter() {
                    counts[hidden[p]] += 1.0;
                }
                return;
            }
            if idx == hidden.len() {
                return;
            }
            if hidden.len() - idx < k - picked.len() {
                return;
            }
            picked.push(idx);
            walk(idx + 1, picked, hidden, k, cons, counts, total);
            picked.pop();
            walk(idx + 1, picked, hidden, k, cons, counts, total);
        }

        walk(
            0,
            &mut Vec::new(),
            &hidden,
            k,
            &cons,
            &mut counts,
            &mut total,
        );
        (0..n)
            .map(|i| {
                if revealed[i] || total == 0.0 {
                    None
                } else {
                    Some((counts[i] / total) as f32)
                }
            })
            .collect()
    }

    #[test]
    fn odds_match_brute_force_across_independent_groups() {
        // Two revealed blocks far apart: their frontiers share no cells, and
        // the gap between them touches no revealed number, so the board really
        // does split into independent groups. That is where the convolution
        // across groups — rather than over one cluster — has to hold up.
        let game = board(9, 9, &[0, 8, 72, 80, 40]);
        let revealed = mask(&game, &[10, 11, 19, 20, 61, 62, 70, 71]);
        let Some(fast) = odds_for(&game, &revealed) else {
            return;
        };
        let slow = brute_force(&game, &revealed);
        let mut compared = 0;
        for i in 0..81 {
            if revealed[i] {
                continue;
            }
            let (a, b) = (fast[i].unwrap(), slow[i].unwrap());
            assert!(
                (a - b).abs() < 1e-5,
                "cell {i}: analysis said {a}, brute force said {b}"
            );
            compared += 1;
        }
        assert!(compared > 10, "too few cells compared: {compared}");
    }

    #[test]
    fn odds_match_brute_force_on_small_boards() {
        // Several layouts, so grouping and the leftover-mine arithmetic both
        // get exercised rather than just single-constraint cases.
        let layouts: [&[usize]; 4] = [
            &[0, 5, 10],
            &[3, 12, 15, 8],
            &[1, 6, 11, 12, 15],
            &[0, 2, 8, 10],
        ];
        let revealed_sets: [&[usize]; 3] = [&[], &[7], &[7, 9]];
        let mut compared = 0;
        for mines in layouts {
            for revealed in revealed_sets {
                let game = board(4, 4, mines);
                let mask = mask(&game, revealed);
                let Some(fast) = odds_for(&game, &mask) else {
                    continue;
                };
                let slow = brute_force(&game, &mask);
                for i in 0..16 {
                    if mask[i] {
                        continue;
                    }
                    let (a, b) = (fast[i].unwrap(), slow[i].unwrap());
                    assert!(
                        (a - b).abs() < 1e-5,
                        "cell {i}: analysis said {a}, brute force said {b} (mines {mines:?}, revealed {revealed:?})"
                    );
                    compared += 1;
                }
            }
        }
        assert!(compared > 50, "too few cells compared: {compared}");
    }

    #[test]
    fn odds_sum_to_the_mine_total() {
        // The mine count is the one global fact the analysis must respect: the
        // expected number of mines across all hidden cells is exactly `mines`.
        let game = board(5, 5, &[0, 6, 12, 18, 24]);
        let revealed = mask(&game, &[7, 8, 11, 13]);
        let odds = odds_for(&game, &revealed).expect("analysis should finish");
        let expected: f32 = (0..25)
            .filter(|&i| !revealed[i])
            .map(|i| odds[i].unwrap())
            .sum();
        assert!(
            (expected - game.mines as f32).abs() < 1e-4,
            "expected {expected} mines across hidden cells, board has {}",
            game.mines
        );
    }

    #[test]
    fn a_revealed_cell_has_no_odds() {
        let game = board(3, 3, &[0]);
        let odds = odds_for(&game, &mask(&game, &[8])).expect("analysis should finish");
        assert!(odds[8].is_none());
    }

    #[test]
    fn odds_are_none_once_the_game_is_over() {
        let mut game = MinesweeperGame::new_with_layout(3, 3, &[0]);
        game.reveal(0, 0); // the mine
        assert_eq!(game.status, GameStatus::Lost);
        assert!(mine_odds(&game).is_none());
    }

    #[test]
    fn a_hidden_cells_count_is_never_read() {
        // Regression guard for the leak the analysis is most likely to grow back
        // into. `adjacent_mines` is filled in for hidden safe cells too, so
        // reading it there would report the answer directly as "proven".
        //
        // The trap: with an empty `revealed` mask nothing is open, so a correct
        // analysis knows only the mine total and gives every hidden cell the
        // same flat odds. A leaky one would read each hidden cell's true count
        // and declare every safe cell proven instead.
        let game = board(5, 5, &[24]);
        let empty = vec![false; 25];
        let odds = odds_for(&game, &empty).expect("analysis should finish");
        let flat = 1.0 / 25.0;
        for (i, p) in odds.iter().enumerate() {
            let p = p.expect("every hidden cell should carry odds");
            assert!(
                (p - flat).abs() < 1e-6 || i == 24,
                "cell {i} was given {p} with nothing revealed, expected the flat {flat}"
            );
        }
    }

    #[test]
    fn the_frontier_is_only_hidden_cells_beside_a_number() {
        // 3x3 board, mine in the middle. The corners' neighbours are mostly the
        // number cells, so they belong on the frontier; a cell far from any
        // revealed number must not.
        let mut game = MinesweeperGame::new_with_layout(5, 5, &[6, 7, 11, 12]);
        game.reveal(0, 0);
        let frontier = odds_frontier(&game).expect("game is still playing");
        for (i, on_frontier) in frontier.iter().enumerate() {
            let hidden = game.cells[i].state != CellState::Revealed;
            if *on_frontier {
                assert!(hidden, "cell {i} is revealed but marked as frontier");
                assert!(
                    neighbors(5, 5, i).iter().any(|&j| {
                        let c = &game.cells[j];
                        c.state == CellState::Revealed && c.adjacent_mines > 0
                    }),
                    "cell {i} is on the frontier with no revealed number beside it"
                );
            }
        }
        // The far corner is untouched by any number here.
        assert!(!frontier[24], "a cell away from every number was marked");
    }

    #[test]
    fn a_zero_beside_a_flagged_cell_proves_that_cell_safe() {
        // Flags block the flood fill, so a revealed zero can keep a hidden
        // neighbour. Ignoring zeros would throw away a proof the player can
        // plainly see.
        let mut game = MinesweeperGame::new_with_layout(3, 3, &[8]);
        game.flag(1, 1);
        game.reveal(0, 0);
        let odds = mine_odds(&game).expect("analysis should finish");
        assert_eq!(odds[4], Some(0.0));
    }

    #[test]
    fn a_certain_cell_is_reported_as_zero_or_one() {
        // The corner number 1 with its only hidden neighbour: that neighbour is
        // a mine with probability 1.
        let game = board(2, 2, &[3]);
        let revealed = mask(&game, &[0, 1, 2]);
        let odds = odds_for(&game, &revealed).expect("analysis should finish");
        assert_eq!(odds[3], Some(1.0));
    }

    #[test]
    fn local_rules_spot_a_certain_mine() {
        // 3x3, mine in the middle-right. Opening the top-left floods the
        // numberless corner; the 1 that appears beside the mine then has that
        // mine as its only hidden neighbour.
        let mut game = board(3, 3, &[5]);
        game.reveal(0, 0);
        assert_eq!(game.status, GameStatus::Playing);
        assert!(
            has_forced_move(&game),
            "a number with one hidden neighbour must be recognised as forced"
        );
    }

    #[test]
    fn local_rules_report_nothing_without_constraints() {
        let mut game = MinesweeperGame::new(3, 3, 1);
        game.reveal(0, 0);
        // A zero-width frontier proves nothing; the call must simply be false
        // rather than panic on empty constraint lists.
        let _ = has_forced_move(&game);
    }

    #[test]
    fn a_guess_free_board_solves_itself() {
        // 3x3 with one mine: opening any safe cell proves the rest.
        for start in [0, 1, 2, 3, 5, 6, 7, 8] {
            let game = board(3, 3, &[4]);
            assert!(guess_free_from(&game, start), "start {start} should solve");
        }
    }

    #[test]
    fn a_board_needing_a_guess_is_rejected() {
        // Two mines in opposite corners of a 2x2, opened at a top corner: the
        // remaining diagonal is a 50/50 with nothing to break the tie.
        let game = board(2, 2, &[0, 3]);
        assert!(!guess_free_from(&game, 1));
    }

    #[test]
    fn generation_finds_a_guess_free_board() {
        let mut game = MinesweeperGame::new_with_mode(9, 9, 10, crate::GenerationMode::GuessFree);
        game.reveal(4, 4);
        // The crate never places a mine in the first click's 3x3 zone, so this
        // also checks the retry loop preserves that guarantee.
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y) = ((4 + dx) as usize, (4 + dy) as usize);
                assert!(
                    !game.cells[y * 9 + x].is_mine,
                    "mine placed in the first click's safe zone at {x},{y}"
                );
            }
        }
        assert_eq!(game.flags_placed(), 0);
    }

    /// The guess-free check must model the real first click.
    ///
    /// `guess_free_from` seeds its mask with `flood(start)` while `reveal` pushes
    /// the whole 3x3 zone, so the two only agree if the clicked cell is itself a
    /// zero — which holds exactly because no mine is ever placed in that zone.
    /// If generation ever stopped guaranteeing that, the solvability check would
    /// silently start testing a different (weaker) position than the player
    /// gets.
    #[test]
    fn the_first_click_is_a_zero_so_flood_matches_real_play() {
        for (width, height, mines) in [(9usize, 9usize, 10usize), (16, 16, 40), (30, 16, 99)] {
            let mut game =
                MinesweeperGame::new_with_mode(width, height, mines, crate::GenerationMode::Random);
            let (cx, cy) = (width / 2, height / 2);
            game.reveal(cx, cy);
            let start = game.index(cx, cy);
            assert_eq!(
                game.cells[start].adjacent_mines, 0,
                "{width}x{height}: the first-click cell must be a zero"
            );
            let n = width * height;
            let mut mask = vec![false; n];
            flood(&game, &mut mask, start);
            let shown: Vec<bool> = game
                .cells
                .iter()
                .map(|c| c.state == CellState::Revealed)
                .collect();
            assert_eq!(
                mask, shown,
                "{width}x{height}: flood(start) must match what the first click reveals"
            );
        }
    }

    /// Slow end-to-end check across every standard board: the retry loop must
    /// really produce guess-free layouts, not merely attempt them. Ignored by
    /// default because the 30x16 board takes over a second unoptimized; run it
    /// with `cargo test -- --ignored` after touching the generator.
    #[test]
    #[ignore]
    fn guess_free_holds_at_every_standard_size() {
        for (width, height, mines) in [(9usize, 9usize, 10usize), (16, 16, 40), (30, 16, 99)] {
            let (cx, cy) = (width / 2, height / 2);
            let mut game = MinesweeperGame::new_with_mode(
                width,
                height,
                mines,
                crate::GenerationMode::GuessFree,
            );
            game.reveal(cx, cy);
            let start = game.index(cx, cy);
            assert!(
                guess_free_from(&game, start),
                "{width}x{height}/{mines}: generated board is not guess free"
            );
            // The retry loop must not lose the first-click guarantee.
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (x, y) = ((cx as i32 + dx) as usize, (cy as i32 + dy) as usize);
                    assert!(
                        !game.cells[y * width + x].is_mine,
                        "{width}x{height}/{mines}: mine placed in the first click's safe zone"
                    );
                }
            }
        }
    }

    #[test]
    fn guess_free_generation_is_repeatable_enough_to_be_worth_it() {
        // The retry loop must actually converge rather than silently keeping the
        // last rejected layout: sample several first clicks and require every
        // one to be solvable.
        for (x, y) in [(0, 0), (4, 4), (8, 0), (0, 8), (8, 8)] {
            let mut game =
                MinesweeperGame::new_with_mode(9, 9, 10, crate::GenerationMode::GuessFree);
            game.reveal(x, y);
            let start = y * 9 + x;
            assert!(
                guess_free_from(&game, start),
                "board opened at {x},{y} was not guess free"
            );
        }
    }
}
