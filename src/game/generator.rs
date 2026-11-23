// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

// Phase 1 procedural level generator; UI wiring in Phase 2/3 will consume this.
#![allow(dead_code)]

use crate::game::level_io::default_starter_level;
use crate::game::solver::{SolveOutcome, solve_level};
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, Side, TileDef};
use rand::RngExt;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenDifficulty {
    Easy,
    Medium,
    Hard,
}

pub fn generate_custom_level(diff: GenDifficulty, rng: &mut impl RngExt) -> LevelDef {
    for attempt in 0..16 {
        if let Some(mut lvl) = try_build_candidate(diff, rng, attempt)
            && let SolveOutcome::Solvable(res) = solve_level(&lvl, Duration::from_millis(250))
            && res.verified_by_sim
            && !res.moves.is_empty()
        {
            lvl.par_moves = res.suggested_par_moves;
            lvl.par_pipes = res.suggested_par_pipes;
            lvl.par_time_ms = res.suggested_par_time_ms;
            return lvl;
        }
    }

    let mut fallback = default_starter_level(0);
    fallback.title = match diff {
        GenDifficulty::Easy => "Generated Easy".to_string(),
        GenDifficulty::Medium => "Generated Medium".to_string(),
        GenDifficulty::Hard => "Generated Hard".to_string(),
    };
    if let SolveOutcome::Solvable(res) = solve_level(&fallback, Duration::from_millis(250)) {
        fallback.par_moves = res.suggested_par_moves;
        fallback.par_pipes = res.suggested_par_pipes;
        fallback.par_time_ms = res.suggested_par_time_ms;
    }
    fallback
}

fn try_build_candidate(
    diff: GenDifficulty,
    rng: &mut impl RngExt,
    attempt: usize,
) -> Option<LevelDef> {
    let (cols, rows, min_len, bg, delay, speed) = match diff {
        GenDifficulty::Easy => (
            5usize,
            if rng.random_bool(0.5) { 4usize } else { 5usize },
            6usize,
            if rng.random_bool(0.5) {
                "NightHills"
            } else {
                "DaySky"
            },
            14.0f32,
            1.3f32,
        ),
        GenDifficulty::Medium => (
            6,
            if rng.random_bool(0.5) { 5 } else { 6 },
            8,
            if rng.random_bool(0.5) {
                "DaySky"
            } else {
                "SnowyHills"
            },
            10.0,
            1.6,
        ),
        GenDifficulty::Hard => (
            if rng.random_bool(0.5) { 7 } else { 8 },
            6,
            11,
            if rng.random_bool(0.5) {
                "SnowyHills"
            } else {
                "OuterSpace"
            },
            8.0,
            2.0,
        ),
    };

    let sx = 0usize;
    let sy = rng.random_range(0..rows);
    let dx = cols - 1;
    let dy = rng.random_range(0..rows);

    let s_dir = 3u8; // Right -> (1, sy)
    let d_dir = 1u8; // Left <- (cols - 2, dy)
    let start_pipe = (sx + 1, sy);
    let end_pipe = (dx - 1, dy);
    if start_pipe == end_pipe {
        return None;
    }

    let mut blocked = vec![false; cols * rows];
    blocked[sy * cols + sx] = true;
    blocked[dy * cols + dx] = true;

    let path = random_self_avoiding_path(cols, rows, start_pipe, end_pipe, &blocked, min_len, rng)?;

    let mut tiles_map: HashMap<(usize, usize), TileDef> = HashMap::new();
    tiles_map.insert(
        (sx, sy),
        TileDef {
            x: sx,
            y: sy,
            shape: PieceShape::Spigot,
            piece_type: PieceType::Source,
            direction: s_dir,
            is_moveable: false,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
    );
    tiles_map.insert(
        (dx, dy),
        TileDef {
            x: dx,
            y: dy,
            shape: PieceShape::Drain,
            piece_type: PieceType::Drain,
            direction: d_dir,
            is_moveable: false,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
    );

    // Build path tiles (optionally split across a portal pair on Medium/Hard)
    let use_portal = diff != GenDifficulty::Easy && path.len() >= 7 && rng.random_bool(0.55);
    let portal_split = if use_portal {
        Some(path.len() / 2)
    } else {
        None
    };
    let gate_pos = if diff == GenDifficulty::Hard && !use_portal && path.len() >= 8 {
        Some(2usize)
    } else {
        None
    };

    for i in 0..path.len() {
        let (x, y) = path[i];
        let prev = if i == 0 { (sx, sy) } else { path[i - 1] };
        let next = if i + 1 < path.len() {
            path[i + 1]
        } else {
            (dx, dy)
        };
        let entry_side = side_between((x, y), prev)?;
        let exit_side = side_between((x, y), next)?;

        if portal_split == Some(i) {
            tiles_map.insert(
                (x, y),
                TileDef {
                    x,
                    y,
                    shape: PieceShape::Spigot,
                    piece_type: PieceType::Portal,
                    direction: spigot_dir_toward(entry_side),
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: false,
                    is_freeze: false,
                    is_magic: false,
                    secondary_color: Some(GameColor::Violet),
                },
            );
            continue;
        }
        if portal_split.is_some_and(|sp| sp + 1 == i) {
            tiles_map.insert(
                (x, y),
                TileDef {
                    x,
                    y,
                    shape: PieceShape::Spigot,
                    piece_type: PieceType::Portal,
                    direction: spigot_dir_toward(exit_side),
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: false,
                    is_freeze: false,
                    is_magic: false,
                    secondary_color: Some(GameColor::Violet),
                },
            );
            continue;
        }

        let (shape, solved_dir) = pipe_for_sides(entry_side, exit_side)?;
        let is_gate = gate_pos == Some(i);
        let is_magic = gate_pos.is_some_and(|gp| i > gp && i <= gp + 2);
        let is_freeze =
            diff != GenDifficulty::Easy && !is_gate && !is_magic && rng.random_bool(0.15);

        let (is_moveable, can_rotate) = if is_gate || is_magic {
            (false, false)
        } else if rng.random_bool(0.5) {
            (true, false)
        } else {
            (false, true)
        };

        let dir = if is_magic {
            (solved_dir + 3) % 4
        } else {
            solved_dir
        };

        tiles_map.insert(
            (x, y),
            TileDef {
                x,
                y,
                shape,
                piece_type: if is_freeze {
                    PieceType::FrozenPipe
                } else {
                    PieceType::Pipe
                },
                direction: dir,
                is_moveable,
                can_rotate,
                is_gate,
                is_freeze,
                is_magic,
                secondary_color: None,
            },
        );
    }

    // Fill off-path cells with movable/rotatable distractors and occasional blockers
    for y in 0..rows {
        for x in 0..cols {
            if tiles_map.contains_key(&(x, y)) {
                continue;
            }
            if rng.random_bool(0.18) {
                tiles_map.insert(
                    (x, y),
                    TileDef {
                        x,
                        y,
                        ..TileDef::dummy_blocker()
                    },
                );
            } else {
                let shape = match rng.random_range(0..3) {
                    0 => PieceShape::Elbow,
                    1 => PieceShape::Straight,
                    _ => PieceShape::Cross,
                };
                let can_rotate = rng.random_bool(0.35);
                tiles_map.insert(
                    (x, y),
                    TileDef {
                        x,
                        y,
                        shape,
                        piece_type: PieceType::Pipe,
                        direction: rng.random_range(0..4) as u8,
                        is_moveable: !can_rotate,
                        can_rotate,
                        is_gate: false,
                        is_freeze: false,
                        is_magic: false,
                        secondary_color: None,
                    },
                );
            }
        }
    }

    // Scramble rotatables and shuffle movable connected components
    let mut rotated_any = false;
    for t in tiles_map.values_mut() {
        if t.can_rotate && t.shape != PieceShape::Cross {
            let offset = rng.random_range(1..4) as u8;
            t.direction = (t.direction + offset) % 4;
            rotated_any = true;
        }
    }

    let movable_coords: Vec<(usize, usize)> = tiles_map
        .iter()
        .filter(|&(_, t)| {
            matches!(t.piece_type, PieceType::Pipe | PieceType::FrozenPipe)
                && t.is_moveable
                && !t.can_rotate
        })
        .map(|(&pos, _)| pos)
        .collect();
    let mut visited = vec![false; cols * rows];
    for start in movable_coords {
        let sidx = start.1 * cols + start.0;
        if visited[sidx] {
            continue;
        }
        let mut comp = Vec::new();
        let mut q = VecDeque::new();
        visited[sidx] = true;
        q.push_back(start);
        while let Some((cx, cy)) = q.pop_front() {
            comp.push((cx, cy));
            for (dx_i, dy_i) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                let nx = cx as i32 + dx_i;
                let ny = cy as i32 + dy_i;
                if nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
                    let np = (nx as usize, ny as usize);
                    let nidx = np.1 * cols + np.0;
                    if !visited[nidx]
                        && tiles_map.get(&np).is_some_and(|t| {
                            matches!(t.piece_type, PieceType::Pipe | PieceType::FrozenPipe)
                                && t.is_moveable
                                && !t.can_rotate
                        })
                    {
                        visited[nidx] = true;
                        q.push_back(np);
                    }
                }
            }
        }
        if comp.len() >= 2 {
            let mut snapshots: Vec<TileDef> = comp.iter().map(|p| tiles_map[p].clone()).collect();
            for i in (1..snapshots.len()).rev() {
                let j = rng.random_range(0..=i);
                snapshots.swap(i, j);
            }
            for (&(tx, ty), mut td) in comp.iter().zip(snapshots) {
                td.x = tx;
                td.y = ty;
                tiles_map.insert((tx, ty), td);
            }
        }
    }

    if !rotated_any && tiles_map.values().filter(|t| t.is_moveable).count() < 2 {
        return None;
    }

    let mut tiles = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            tiles.push(tiles_map.remove(&(x, y))?);
        }
    }

    Some(LevelDef {
        index: 0,
        title: format!(
            "{} Puzzle {}",
            match diff {
                GenDifficulty::Easy => "Relaxed",
                GenDifficulty::Medium => "Tactical",
                GenDifficulty::Hard => "Master",
            },
            attempt + 1
        ),
        description: "Generated puzzle.".to_string(),
        background_id: bg.to_string(),
        liquid_color: GameColor::Blue,
        start_delay: delay,
        liquid_speed: speed,
        rotater_color: GameColor::Blue,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Green,
        freeze_color: GameColor::White,
        magic_color: GameColor::Violet,
        par_moves: 10,
        par_pipes: path.len() as u32,
        par_time_ms: 20000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    })
}

fn random_self_avoiding_path(
    cols: usize,
    rows: usize,
    start: (usize, usize),
    end: (usize, usize),
    blocked: &[bool],
    min_len: usize,
    rng: &mut impl RngExt,
) -> Option<Vec<(usize, usize)>> {
    for _ in 0..24 {
        let mut seen = blocked.to_vec();
        let mut path = vec![start];
        seen[start.1 * cols + start.0] = true;
        if dfs_walk(
            cols, rows, start, end, min_len, &mut seen, &mut path, rng, &mut 0,
        ) {
            return Some(path);
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn dfs_walk(
    cols: usize,
    rows: usize,
    cur: (usize, usize),
    end: (usize, usize),
    min_len: usize,
    seen: &mut [bool],
    path: &mut Vec<(usize, usize)>,
    rng: &mut impl RngExt,
    steps: &mut usize,
) -> bool {
    *steps += 1;
    if *steps > 600 {
        return false;
    }
    if cur == end {
        return path.len() >= min_len;
    }

    let mut dirs = [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)];
    for i in (1..dirs.len()).rev() {
        let j = rng.random_range(0..=i);
        dirs.swap(i, j);
    }

    let mut nexts = Vec::with_capacity(4);
    for (dx, dy) in dirs {
        let nx = cur.0 as i32 + dx;
        let ny = cur.1 as i32 + dy;
        if nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
            let np = (nx as usize, ny as usize);
            let idx = np.1 * cols + np.0;
            if !seen[idx] {
                nexts.push(np);
            }
        }
    }

    for np in nexts {
        let idx = np.1 * cols + np.0;
        seen[idx] = true;
        path.push(np);
        if dfs_walk(cols, rows, np, end, min_len, seen, path, rng, steps) {
            return true;
        }
        path.pop();
        seen[idx] = false;
    }
    false
}

fn side_between(from: (usize, usize), to: (usize, usize)) -> Option<Side> {
    match (to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32) {
        (0, -1) => Some(Side::Up),
        (1, 0) => Some(Side::Right),
        (0, 1) => Some(Side::Down),
        (-1, 0) => Some(Side::Left),
        _ => None,
    }
}

fn spigot_dir_toward(side: Side) -> u8 {
    // Spigot default port is Down (dir 0) -> Left (1), Up (2), Right (3)
    match side {
        Side::Down | Side::Center => 0,
        Side::Left => 1,
        Side::Up => 2,
        Side::Right => 3,
    }
}

fn pipe_for_sides(a: Side, b: Side) -> Option<(PieceShape, u8)> {
    if a.opposite() == b {
        let dir = match a {
            Side::Up | Side::Down => 0,
            Side::Left | Side::Right => 1,
            Side::Center => return None,
        };
        return Some((PieceShape::Straight, dir));
    }
    let dir = match (a, b) {
        (Side::Up, Side::Right) | (Side::Right, Side::Up) => 0,
        (Side::Right, Side::Down) | (Side::Down, Side::Right) => 1,
        (Side::Down, Side::Left) | (Side::Left, Side::Down) => 2,
        (Side::Left, Side::Up) | (Side::Up, Side::Left) => 3,
        _ => return None,
    };
    Some((PieceShape::Elbow, dir))
}
