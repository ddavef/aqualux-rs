// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::level_sim::{ACTION_TWEEN_SECS, SimEvent};
use crate::game::scoring::generate_match_level;
use crate::game::tile::{
    Tile, animate_grid_tiles, build_tile_grid, collect_swap_eligible, is_adjacent,
    swap_grid_tiles_raw, swap_grid_tiles_timed,
};
use crate::game::types::{LevelDef, PieceType};
use rand::RngExt;

#[derive(Debug, Clone)]
pub struct MatchSim {
    pub level_index: usize,
    pub mirror_mode: bool,
    pub level: LevelDef,
    pub goal_tiles: Vec<Vec<Tile>>,
    pub play_tiles: Vec<Vec<Tile>>,
    pub selected: Option<(usize, usize)>,
    pub swapping_tiles: Vec<(usize, usize)>,
    pub moves: u32,
    pub is_ready: bool,
    pub is_started: bool,
    pub is_paused: bool,
    pub finished: bool,
}

impl MatchSim {
    pub fn new(level_index: usize, mirror_mode: bool, rng: &mut impl RngExt) -> Self {
        let level = generate_match_level(level_index, rng);
        let play_tiles = build_tile_grid(&level);
        let mut goal_tiles = build_tile_grid(&level);
        Self::scramble_grid(&mut goal_tiles, level.rows, level.cols, rng);
        if Self::grids_equal(&play_tiles, &goal_tiles, level.rows, level.cols) {
            Self::scramble_grid(&mut goal_tiles, level.rows, level.cols, rng);
        }
        Self {
            level_index,
            mirror_mode,
            level,
            goal_tiles,
            play_tiles,
            selected: None,
            swapping_tiles: Vec::new(),
            moves: 0,
            is_ready: false,
            is_started: false,
            is_paused: false,
            finished: false,
        }
    }

    pub fn start_level(&mut self) {
        self.is_ready = true;
        self.is_started = true;
    }

    pub fn is_acting(&self) -> bool {
        !self.swapping_tiles.is_empty()
    }

    /// Scrambles the Match board away from its solved configuration.
    fn scramble_grid(grid: &mut [Vec<Tile>], rows: usize, cols: usize, rng: &mut impl RngExt) {
        for y in 0..rows {
            for x in 0..cols {
                let elig = collect_swap_eligible(grid, cols, rows, (x, y), false);
                if elig.is_empty() {
                    continue;
                }
                let partner = elig[rng.random_range(0..elig.len())];
                if grid[y][x].can_rotate {
                    let turns = rng.random_range(1..=2) as u8;
                    grid[y][x].rotate_cw_instant(turns);
                } else if grid[partner.1][partner.0].can_rotate {
                    let turns = rng.random_range(1..=2) as u8;
                    grid[partner.1][partner.0].rotate_cw_instant(turns);
                } else {
                    let a_t = grid[y][x].piece_type;
                    let b_t = grid[partner.1][partner.0].piece_type;
                    if a_t != PieceType::Source
                        && a_t != PieceType::Drain
                        && b_t != PieceType::Source
                        && b_t != PieceType::Drain
                    {
                        swap_grid_tiles_raw(grid, (x, y), partner);
                    }
                }
            }
        }
    }

    fn grids_equal(a: &[Vec<Tile>], b: &[Vec<Tile>], rows: usize, cols: usize) -> bool {
        for y in 0..rows {
            for x in 0..cols {
                if !a[y][x].is_equal_to(&b[y][x]) {
                    return false;
                }
            }
        }
        true
    }

    pub fn is_solved(&self) -> bool {
        Self::grids_equal(
            &self.play_tiles,
            &self.goal_tiles,
            self.level.rows,
            self.level.cols,
        )
    }

    pub fn click_tile(&mut self, tx: usize, ty: usize) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if !self.is_ready
            || !self.is_started
            || self.is_paused
            || self.finished
            || self.is_acting()
            || ty >= self.level.rows
            || tx >= self.level.cols
        {
            return events;
        }
        let tile = &self.play_tiles[ty][tx];
        if !(tile.is_moveable || tile.can_rotate) {
            return events;
        }
        if tile.can_rotate {
            self.play_tiles[ty][tx].rotate_cw_timed(1, ACTION_TWEEN_SECS);
            self.selected = None;
            self.swapping_tiles = vec![(tx, ty)];
            events.push(SimEvent::PlaySfx("TwistPieceSound"));
            return events;
        }
        if let Some((sx, sy)) = self.selected {
            if (sx, sy) == (tx, ty) {
                self.selected = None;
            } else if is_adjacent((sx, sy), (tx, ty)) && self.play_tiles[sy][sx].is_moveable {
                self.selected = None;
                swap_grid_tiles_timed(&mut self.play_tiles, (sx, sy), (tx, ty), ACTION_TWEEN_SECS);
                self.swapping_tiles = vec![(sx, sy), (tx, ty)];
                events.push(SimEvent::PlaySfx("SwapTwoSound"));
            } else {
                self.selected = Some((tx, ty));
            }
        } else {
            self.selected = Some((tx, ty));
            events.push(SimEvent::PlaySfx("ClickTwoSound"));
        }
        events
    }

    pub fn try_swap_drag(&mut self, from: (usize, usize), to: (usize, usize)) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if !self.is_ready
            || !self.is_started
            || self.is_paused
            || self.finished
            || self.is_acting()
            || !is_adjacent(from, to)
            || from.1 >= self.level.rows
            || from.0 >= self.level.cols
            || to.1 >= self.level.rows
            || to.0 >= self.level.cols
        {
            return events;
        }
        if self.play_tiles[from.1][from.0].is_moveable && self.play_tiles[to.1][to.0].is_moveable {
            self.selected = None;
            swap_grid_tiles_timed(&mut self.play_tiles, from, to, ACTION_TWEEN_SECS);
            self.swapping_tiles = vec![from, to];
            events.push(SimEvent::PlaySfx("SwapTwoSound"));
        }
        events
    }

    pub fn update(&mut self, dt: f32) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if self.is_paused {
            return events;
        }
        if animate_grid_tiles(&mut self.play_tiles, dt, &self.swapping_tiles) {
            self.swapping_tiles.clear();
            self.moves += 1;
            if !self.finished && self.is_solved() {
                self.finished = true;
                events.push(SimEvent::PlaySfx("CheeringSound"));
                events.push(SimEvent::PlaySfx("BonusTwoSound"));
                events.push(SimEvent::MatchWon);
            }
        }
        events
    }

    pub fn get_swap_eligible(&self, pos: (usize, usize)) -> Vec<(usize, usize)> {
        collect_swap_eligible(
            &self.play_tiles,
            self.level.cols,
            self.level.rows,
            pos,
            false,
        )
    }
}
