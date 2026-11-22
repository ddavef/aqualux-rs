// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::awards::Trophy;
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, TileDef};
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricResult {
    pub trophy: Trophy,
    pub total_score: u32,
    pub pipe_bonus: u32,
    pub move_bonus: u32,
    pub time_bonus: u32,
}

/// Calculates per-level bonus scores and trophy awards from par metrics.
pub fn calculate_score(
    moves: u32,
    pipes: u32,
    time_ms: u32,
    par_moves: u32,
    _par_pipes: u32,
    par_time_ms: u32,
    is_hard: bool,
) -> MetricResult {
    let pm = par_moves.max(1) as f64;
    let pt = par_time_ms.max(1) as f64;

    let move_ratio = (1.0 - (moves as f64 - pm) / (3.0 * pm)).max(0.0);
    let mut move_bonus = (move_ratio * 200.0).floor() as u32;

    let time_ratio = (1.0 - (time_ms as f64 - pt) / (14.0 * pt)).max(0.0);
    let mut time_bonus = (time_ratio * 400.0).floor() as u32;

    let mut pipe_bonus = pipes * 100;

    if is_hard {
        pipe_bonus = ((pipe_bonus as f64) * 1.5).floor() as u32;
        time_bonus = ((time_bonus as f64) * 1.5).floor() as u32;
        move_bonus = ((move_bonus as f64) * 1.5).floor() as u32;
    }

    let perf = (time_ratio * 0.66666 + move_ratio * 0.33333).clamp(0.0, 1.0);
    let mut trophy = if perf >= 0.95 {
        Trophy::Platinum
    } else if perf >= 0.85 {
        Trophy::Gold
    } else if perf >= 0.60 {
        Trophy::Silver
    } else if perf >= 0.40 {
        Trophy::Bronze
    } else {
        Trophy::None
    };

    if !is_hard && trophy == Trophy::Platinum {
        trophy = Trophy::Gold;
    }

    MetricResult {
        trophy,
        total_score: pipe_bonus + time_bonus + move_bonus,
        pipe_bonus,
        move_bonus,
        time_bonus,
    }
}

/// Procedural Match-mode puzzle generator (0-indexed `level_idx`).
pub fn generate_match_level(level_idx: usize, rng: &mut impl Rng) -> LevelDef {
    let (cols, rows, bg) = if level_idx < 5 {
        (3usize, 3usize, "NightHills")
    } else if level_idx < 15 {
        (4, 4, "NightHills")
    } else if level_idx < 30 {
        (5, 5, "DaySky")
    } else if level_idx < 50 {
        (6, 6, "SnowyHills")
    } else {
        (7, 7, "OuterSpace")
    };

    let sx = rng.gen_range(0..cols);
    let sy = rng.gen_range(0..rows);
    let mut dx = rng.gen_range(0..cols);
    let mut dy = rng.gen_range(0..rows);
    while dx == sx && dy == sy {
        dx = rng.gen_range(0..cols);
        dy = rng.gen_range(0..rows);
    }

    let mut tiles = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            let dir = rng.gen_range(0..3) as u8;
            if x == sx && y == sy {
                tiles.push(TileDef {
                    x,
                    y,
                    shape: PieceShape::Spigot,
                    piece_type: PieceType::Source,
                    direction: dir,
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: false,
                    is_freeze: false,
                    is_magic: false,
                    secondary_color: None,
                });
            } else if x == dx && y == dy {
                tiles.push(TileDef {
                    x,
                    y,
                    shape: PieceShape::Spigot,
                    piece_type: PieceType::Drain,
                    direction: dir,
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: false,
                    is_freeze: false,
                    is_magic: false,
                    secondary_color: None,
                });
            } else {
                let shape = match rng.gen_range(0..3) {
                    0 => PieceShape::Cross,
                    1 => PieceShape::Elbow,
                    _ => PieceShape::Straight,
                };
                let can_rotate = level_idx > 5 && rng.gen_range(0..3) == 0;
                let is_moveable = !can_rotate;
                let is_magic = level_idx > 30 && rng.gen_range(0.0..10.0) < 2.0;

                tiles.push(TileDef {
                    x,
                    y,
                    shape,
                    piece_type: PieceType::Pipe,
                    direction: dir,
                    is_moveable,
                    can_rotate,
                    is_gate: false,
                    is_freeze: false,
                    is_magic,
                    secondary_color: None,
                });
            }
        }
    }

    LevelDef {
        index: level_idx,
        title: format!("Match Challenge #{}", level_idx + 1),
        description: String::new(),
        background_id: bg.to_string(),
        liquid_color: GameColor::Blue,
        start_delay: 0.0,
        liquid_speed: 0.1,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Green,
        freeze_color: GameColor::White,
        magic_color: GameColor::Violet,
        par_moves: 10,
        par_pipes: (cols * rows) as u32,
        par_time_ms: 30000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    }
}
