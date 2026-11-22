// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::awards::Difficulty;
use crate::game::scoring::MetricResult;
use crate::game::tile::{
    Tile, animate_grid_tiles, build_tile_grid, is_adjacent, swap_grid_tiles_timed,
};
use crate::game::types::{FlowTimerState, GameColor, LevelDef, PieceType, Side, TutorialMsg};

pub const ACTION_TWEEN_SECS: f32 = 0.4;

#[derive(Debug, Clone)]
pub enum SimEvent {
    PlaySfx(&'static str),
    PlayRandomBonusSfx,
    ScoreBlurb(f32, f32, u32),
    LeakErupt(usize, usize, Side, GameColor),
    LevelWon(MetricResult),
    LevelLost,
    MatchWon,
}

#[derive(Debug, Clone)]
pub struct LevelSim {
    pub level: LevelDef,
    pub difficulty: Difficulty,
    pub tiles: Vec<Vec<Tile>>,
    pub is_ready: bool,
    pub is_started: bool,
    pub is_paused: bool,
    pub is_accelerated: bool,
    pub finished: bool,
    pub won: Option<bool>,
    pub elapsed_ms: f32,
    pub moves: u32,
    pub pipes_filled: u32,
    pub score: u32,
    pub current_speed: f32,
    pub selected: Option<(usize, usize)>,
    pub swapping_tiles: Vec<(usize, usize)>,
    pub(crate) pending_exits: Vec<(usize, usize, Side)>,
    pub path_complete: bool,
    pub hilight_seq: u32,
    pub tutorial_idx: usize,
    pub tutorials_skipped: bool,
    pub last_result: Option<MetricResult>,
}

impl LevelSim {
    pub fn new(level: LevelDef, difficulty: Difficulty) -> Self {
        let tiles = build_tile_grid(&level);
        // Pipe fill speed uses 0.5x level liquid_speed across all difficulties;
        // difficulty scales start delay, par display, score multiplier, and Platinum eligibility.
        let current_speed = level.liquid_speed * 0.5;

        Self {
            level,
            difficulty,
            tiles,
            is_ready: false,
            is_started: false,
            is_paused: false,
            is_accelerated: false,
            finished: false,
            won: None,
            elapsed_ms: 0.0,
            moves: 0,
            pipes_filled: 0,
            score: 0,
            current_speed,
            selected: None,
            swapping_tiles: Vec::new(),
            pending_exits: Vec::new(),
            path_complete: false,
            hilight_seq: 0,
            tutorial_idx: 0,
            tutorials_skipped: false,
            last_result: None,
        }
    }

    pub fn is_acting(&self) -> bool {
        !self.swapping_tiles.is_empty()
    }

    /// Called after the opening board pop-in animation finishes.
    pub fn start_level(&mut self) -> Vec<SimEvent> {
        self.is_ready = true;
        if self.level.tutorials.is_empty() || self.tutorials_skipped {
            self.start_level_play()
        } else {
            vec![SimEvent::PlaySfx("PopTwoSound")]
        }
    }

    pub fn start_level_play(&mut self) -> Vec<SimEvent> {
        let mut events = Vec::new();
        self.is_ready = true;
        self.is_started = true;
        self.moves = 0;
        self.pipes_filled = 0;
        self.hilight_seq = 0;
        if self.difficulty != Difficulty::Easy {
            let delay = match self.difficulty {
                Difficulty::Medium if self.level.start_delay > 0.0 => {
                    Some(self.level.start_delay * 3.0)
                }
                Difficulty::Hard if self.level.start_delay > 0.0 => Some(self.level.start_delay),
                _ => None,
            };
            if let Some(secs) = delay {
                for row in &mut self.tiles {
                    for tile in row {
                        if tile.piece_type == PieceType::Source {
                            tile.timer = Some(FlowTimerState::new(secs));
                        }
                    }
                }
            } else {
                self.start_source_fill(&mut events);
            }
        }
        self.recompute_path(true, &mut events);
        events
    }

    pub fn active_tutorial(&self) -> Option<&TutorialMsg> {
        if self.tutorials_skipped || !self.is_ready {
            return None;
        }
        self.level.tutorials.get(self.tutorial_idx)
    }

    pub fn next_tutorial(&mut self) -> Vec<SimEvent> {
        let clear_held = self.tutorial_idx >= 1
            && self
                .level
                .tutorials
                .get(self.tutorial_idx - 1)
                .is_some_and(|t| t.hold_hilight);
        if self.tutorial_idx < self.level.tutorials.len() {
            self.tutorial_idx += 1;
        }
        if clear_held || self.tutorial_idx >= self.level.tutorials.len() {
            self.clear_all_path_highlights();
        }
        if self.tutorial_idx >= self.level.tutorials.len() {
            self.start_level_play()
        } else {
            vec![SimEvent::PlaySfx("PopTwoSound")]
        }
    }

    pub fn skip_tutorials(&mut self) -> Vec<SimEvent> {
        self.tutorials_skipped = true;
        self.clear_all_path_highlights();
        self.start_level_play()
    }

    pub(crate) fn start_source_fill(&mut self, events: &mut Vec<SimEvent>) {
        let fast_nozzle = self.is_accelerated || self.current_speed >= 15.0;
        let mut started = false;
        for row in &mut self.tiles {
            for tile in row {
                if tile.piece_type == PieceType::Source && !tile.has_filled {
                    if let Some(t) = &mut tile.timer {
                        t.trigger_fade();
                    }
                    if tile.start_filling(Side::Center, fast_nozzle).is_some() {
                        started = true;
                    }
                }
            }
        }
        if started {
            events.push(SimEvent::PlaySfx("TwistSound"));
        }
    }

    pub fn fast_forward(&mut self) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if self.is_paused || self.finished || !self.is_started {
            return events;
        }
        self.is_accelerated = true;
        self.current_speed = 120.0;

        self.start_source_fill(&mut events);

        for row in &mut self.tiles {
            for tile in row {
                for pass in &mut tile.fills {
                    if !pass.completed {
                        pass.elapsed = 1.0;
                    }
                }
                if tile.piece_type == PieceType::Portal
                    && let Some(t) = &mut tile.timer
                    && !t.fading
                {
                    t.remaining = 0.0;
                    t.trigger_fade();
                    tile.start_filling(Side::Center, true);
                }
            }
        }
        events
    }

    pub fn click_tile(&mut self, tx: usize, ty: usize) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if !self.is_ready
            || (!self.is_started && self.active_tutorial().is_none())
            || self.is_paused
            || self.finished
            || self.is_acting()
            || ty >= self.level.rows
            || tx >= self.level.cols
        {
            return events;
        }
        if !self.tutorial_allows_click((tx, ty)) {
            return events;
        }
        let t = &self.tiles[ty][tx];
        if t.is_locked_or_active() || (!t.is_moveable && !t.can_rotate) {
            return events;
        }
        if t.can_rotate {
            self.tiles[ty][tx].rotate_cw_timed(1, ACTION_TWEEN_SECS);
            self.selected = None;
            self.swapping_tiles = vec![(tx, ty)];
            events.push(SimEvent::PlaySfx("TwistPieceSound"));
            return events;
        }
        if let Some((sx, sy)) = self.selected {
            if (sx, sy) == (tx, ty) {
                self.selected = None;
            } else if is_adjacent((sx, sy), (tx, ty))
                && !self.tiles[sy][sx].is_locked_or_active()
                && self.tiles[sy][sx].is_moveable
            {
                self.selected = None;
                swap_grid_tiles_timed(&mut self.tiles, (sx, sy), (tx, ty), ACTION_TWEEN_SECS);
                self.swapping_tiles = vec![(sx, sy), (tx, ty)];
                events.push(SimEvent::PlaySfx("SwapTwoSound"));
            } else {
                self.selected = Some((tx, ty));
            }
        } else {
            self.selected = Some((tx, ty));
            events.push(SimEvent::PlaySfx("ClickTwoSound"));
            if self
                .active_tutorial()
                .is_some_and(|tut| tut.wait_for_select == Some((tx, ty)))
            {
                events.extend(self.next_tutorial());
            }
        }
        events
    }

    pub fn try_swap_drag(&mut self, from: (usize, usize), to: (usize, usize)) -> Vec<SimEvent> {
        if !self.is_ready
            || (!self.is_started && self.active_tutorial().is_none())
            || self.is_paused
            || self.finished
            || self.is_acting()
            || !is_adjacent(from, to)
            || from.1 >= self.level.rows
            || from.0 >= self.level.cols
            || to.1 >= self.level.rows
            || to.0 >= self.level.cols
        {
            return Vec::new();
        }
        if !self.tutorial_allows_drag(from, to) {
            self.selected = None;
            return Vec::new();
        }
        let a = &self.tiles[from.1][from.0];
        let b = &self.tiles[to.1][to.0];
        if a.is_moveable && b.is_moveable && !a.is_locked_or_active() && !b.is_locked_or_active() {
            self.selected = None;
            swap_grid_tiles_timed(&mut self.tiles, from, to, ACTION_TWEEN_SECS);
            self.swapping_tiles = vec![from, to];
            return vec![SimEvent::PlaySfx("SwapTwoSound")];
        }
        Vec::new()
    }

    pub fn update(&mut self, dt: f32) -> Vec<SimEvent> {
        let mut events = Vec::new();
        if self.is_paused {
            return events;
        }
        for row in &mut self.tiles {
            for tile in row {
                if let Some(ref mut hl) = tile.path_hl
                    && hl.tick(dt)
                {
                    tile.path_hl = None;
                }
            }
        }
        if animate_grid_tiles(&mut self.tiles, dt, &self.swapping_tiles) {
            self.on_swap_complete(&mut events);
        }
        self.advance_slimes(dt);

        if self.finished || !self.is_started {
            return events;
        }
        if !self.is_accelerated {
            self.elapsed_ms += dt * 1000.0;
        }
        self.tick_timers(dt, &mut events);
        let completed_exits = self.tick_fill_passes(dt, &mut events);
        self.handle_completed_exits(completed_exits, &mut events);
        events
    }
}
