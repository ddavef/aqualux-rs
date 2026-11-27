// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::level_io::{MAX_GRID_COLS, MAX_GRID_ROWS};
use crate::game::solver::{SolveOutcome, SolveResult, solve_level};
use crate::game::types::{
    CAMPAIGN_LIQUID_COLORS, GameColor, LevelDef, PieceShape, PieceType, Side, TileDef,
};
use crate::save::EndlessElements;
use rand::RngExt;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

pub const MIN_GEN_COLS: usize = 4;
pub const MIN_GEN_ROWS: usize = 3;

const PORTAL_PALETTE: [GameColor; 5] = [
    GameColor::Orange,
    GameColor::Violet,
    GameColor::Red,
    GameColor::Green,
    GameColor::Blue,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenDifficulty {
    Easy,
    Medium,
    Hard,
}

impl GenDifficulty {
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
        }
    }
}

#[derive(Clone, Copy)]
struct GenSpec {
    elements: EndlessElements,
    title_prefix: &'static str,
    bgs: &'static [&'static str],
    start_delay: f32,
    liquid_speed: f32,
    portal_chance: f64,
    gate_magic_chance: f64,
    immovable_path_chance: f64,
    immovable_offpath_chance: f64,
    frosted_path_chance: f64,
    frosted_offpath_chance: f64,
    rock_chance: f64,
}

#[derive(Clone, Copy)]
struct PathStep {
    pos: (usize, usize),
    entry: Side,
    exit: Side,
    force_rotatable: bool,
    seg_idx: usize,
}

#[derive(Clone, Copy)]
struct PortalPlacement {
    entry_pos: (usize, usize),
    entry_dir: u8,
    exit_pos: (usize, usize),
    exit_dir: u8,
    color: GameColor,
}

struct SolvedLayout {
    source: SpigotPos,
    drain: SpigotPos,
    path_steps: Vec<PathStep>,
    portals: Vec<PortalPlacement>,
    wall_tiles: HashMap<(usize, usize), TileDef>,
}

#[derive(Clone, Copy)]
enum WallKind {
    Vertical,
    Horizontal,
    Corner(u8),
}

#[derive(Clone, Copy)]
enum GateMagicArchetype {
    InterleavedChain,
    MagicFortress,
    GateGauntlet,
}

#[derive(Default)]
struct GateMagicPlan {
    on_path_gates: HashSet<usize>,
    on_path_magic_dirs: HashMap<usize, u8>,
    off_path_magic: Vec<((usize, usize), PieceShape, u8)>,
    off_path_gates: Vec<((usize, usize), PieceShape, u8)>,
}

pub fn generate_custom_level(diff: GenDifficulty, rng: &mut impl RngExt) -> LevelDef {
    generate_custom_level_with_max_size(diff, 8, 6, rng)
}

pub fn generate_custom_level_with_max_size(
    diff: GenDifficulty,
    max_cols: usize,
    max_rows: usize,
    rng: &mut impl RngExt,
) -> LevelDef {
    let mc = max_cols.clamp(MIN_GEN_COLS, MAX_GRID_COLS);
    let mr = max_rows.clamp(MIN_GEN_ROWS, MAX_GRID_ROWS);
    let spec = spec_for_difficulty(diff);

    generate_from_spec(spec, mc, mr, rng).unwrap_or_else(|| fallback_level_within(diff, mc, mr))
}

pub fn generate_custom_level_with_elements(
    max_cols: usize,
    max_rows: usize,
    elements: EndlessElements,
    rng: &mut impl RngExt,
) -> LevelDef {
    let mc = max_cols.clamp(MIN_GEN_COLS, MAX_GRID_COLS);
    let mr = max_rows.clamp(MIN_GEN_ROWS, MAX_GRID_ROWS);
    let elements = elements.sanitized();
    let spec = GenSpec {
        elements,
        title_prefix: "Custom",
        bgs: &["NightHills", "DaySky", "SnowyHills", "OuterSpace"],
        start_delay: 10.0,
        liquid_speed: 1.6,
        portal_chance: if elements.portals { 0.80 } else { 0.0 },
        gate_magic_chance: if elements.gate_magic { 0.85 } else { 0.0 },
        immovable_path_chance: 0.20,
        immovable_offpath_chance: 0.18,
        frosted_path_chance: 0.20,
        frosted_offpath_chance: 0.16,
        rock_chance: 0.16,
    };

    generate_from_spec(spec, mc, mr, rng)
        .unwrap_or_else(|| fallback_level_with_elements(mc, mr, elements))
}

fn spec_for_difficulty(diff: GenDifficulty) -> GenSpec {
    match diff {
        GenDifficulty::Easy => GenSpec {
            elements: EndlessElements {
                movable: true,
                rotatable: true,
                immovable: false,
                frosted: false,
                rocks: true,
                gate_magic: false,
                portals: false,
            },
            title_prefix: "Relaxed",
            bgs: &["NightHills", "DaySky"],
            start_delay: 14.0,
            liquid_speed: 1.3,
            portal_chance: 0.0,
            gate_magic_chance: 0.0,
            immovable_path_chance: 0.0,
            immovable_offpath_chance: 0.0,
            frosted_path_chance: 0.0,
            frosted_offpath_chance: 0.0,
            rock_chance: 0.18,
        },
        GenDifficulty::Medium => GenSpec {
            elements: EndlessElements {
                movable: true,
                rotatable: true,
                immovable: true,
                frosted: true,
                rocks: true,
                gate_magic: false,
                portals: true,
            },
            title_prefix: "Tactical",
            bgs: &["DaySky", "SnowyHills"],
            start_delay: 10.0,
            liquid_speed: 1.6,
            portal_chance: 0.60,
            gate_magic_chance: 0.0,
            immovable_path_chance: 0.14,
            immovable_offpath_chance: 0.14,
            frosted_path_chance: 0.16,
            frosted_offpath_chance: 0.10,
            rock_chance: 0.18,
        },
        GenDifficulty::Hard => GenSpec {
            elements: EndlessElements {
                movable: true,
                rotatable: true,
                immovable: true,
                frosted: true,
                rocks: true,
                gate_magic: true,
                portals: true,
            },
            title_prefix: "Master",
            bgs: &["SnowyHills", "OuterSpace"],
            start_delay: 8.0,
            liquid_speed: 2.0,
            portal_chance: 0.55,
            gate_magic_chance: 0.80,
            immovable_path_chance: 0.18,
            immovable_offpath_chance: 0.16,
            frosted_path_chance: 0.18,
            frosted_offpath_chance: 0.14,
            rock_chance: 0.16,
        },
    }
}

fn generate_from_spec(
    spec: GenSpec,
    mc: usize,
    mr: usize,
    rng: &mut impl RngExt,
) -> Option<LevelDef> {
    for attempt in 0..36 {
        let Some(mut lvl) = try_build_spec_candidate(spec, mc, mr, rng, attempt) else {
            continue;
        };
        let SolveOutcome::Solvable(res) = solve_level(&lvl, Duration::from_millis(250)) else {
            continue;
        };
        if !res.verified_by_sim || res.moves.is_empty() {
            continue;
        }
        if lvl.tiles.iter().any(|t| t.piece_type == PieceType::Portal)
            && !portal_solution_valid(&lvl, &res)
        {
            continue;
        }
        if !gate_magic_solution_valid(&lvl, &res) {
            continue;
        }
        lvl.par_moves = res.suggested_par_moves;
        lvl.par_pipes = res.suggested_par_pipes;
        lvl.par_time_ms = res.suggested_par_time_ms;
        return Some(lvl);
    }
    None
}

fn portal_solution_valid(lvl: &LevelDef, res: &SolveResult) -> bool {
    let portal_colors: Vec<GameColor> = {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for t in &lvl.tiles {
            if t.piece_type == PieceType::Portal
                && let Some(c) = t.secondary_color
                && seen.insert(c)
            {
                out.push(c);
            }
        }
        out
    };
    if portal_colors.is_empty() {
        return true;
    }

    let visited: HashSet<(usize, usize)> = res.path.iter().map(|seg| (seg.x, seg.y)).collect();
    for &color in &portal_colors {
        let visited_this_color = lvl.tiles.iter().any(|t| {
            t.piece_type == PieceType::Portal
                && t.secondary_color == Some(color)
                && visited.contains(&(t.x, t.y))
        });
        if !visited_this_color {
            return false;
        }

        let mut without_pair = lvl.clone();
        for t in &mut without_pair.tiles {
            if t.piece_type == PieceType::Portal && t.secondary_color == Some(color) {
                *t = TileDef {
                    x: t.x,
                    y: t.y,
                    ..TileDef::dummy_blocker()
                };
            }
        }
        if !matches!(
            solve_level(&without_pair, Duration::from_millis(120)),
            SolveOutcome::Unsolvable(_)
        ) {
            return false;
        }
    }
    true
}

fn gate_magic_solution_valid(lvl: &LevelDef, res: &SolveResult) -> bool {
    let has_gate = lvl.tiles.iter().any(|t| t.is_gate);
    let has_magic = lvl.tiles.iter().any(|t| t.is_magic);
    if !has_gate && !has_magic {
        return true;
    }
    if has_gate != has_magic {
        return false;
    }
    let visited: HashSet<(usize, usize)> = res.path.iter().map(|seg| (seg.x, seg.y)).collect();
    let used_gate = lvl
        .tiles
        .iter()
        .any(|t| t.is_gate && visited.contains(&(t.x, t.y)));
    let used_magic = lvl
        .tiles
        .iter()
        .any(|t| t.is_magic && visited.contains(&(t.x, t.y)));
    used_gate && used_magic
}

fn pipe_tile(
    x: usize,
    y: usize,
    shape: PieceShape,
    direction: u8,
    is_moveable: bool,
    can_rotate: bool,
) -> TileDef {
    TileDef {
        x,
        y,
        shape,
        piece_type: PieceType::Pipe,
        direction: direction % 4,
        is_moveable,
        can_rotate,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

fn source_tile(x: usize, y: usize, dir: u8) -> TileDef {
    TileDef {
        x,
        y,
        shape: PieceShape::Spigot,
        piece_type: PieceType::Source,
        direction: dir % 4,
        is_moveable: false,
        can_rotate: false,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

fn drain_tile(x: usize, y: usize, dir: u8) -> TileDef {
    TileDef {
        x,
        y,
        shape: PieceShape::Spigot,
        piece_type: PieceType::Drain,
        direction: dir % 4,
        is_moveable: false,
        can_rotate: false,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: None,
    }
}

fn portal_tile(x: usize, y: usize, dir: u8, color: GameColor) -> TileDef {
    TileDef {
        x,
        y,
        shape: PieceShape::Spigot,
        piece_type: PieceType::Portal,
        direction: dir % 4,
        is_moveable: false,
        can_rotate: false,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color: Some(color),
    }
}

fn blocker_tile(x: usize, y: usize) -> TileDef {
    TileDef {
        x,
        y,
        ..TileDef::dummy_blocker()
    }
}

fn make_wall_tile(x: usize, y: usize, kind: WallKind, use_imm_wall: bool) -> TileDef {
    if !use_imm_wall {
        return blocker_tile(x, y);
    }
    match kind {
        WallKind::Vertical => pipe_tile(x, y, PieceShape::Straight, 0, false, false),
        WallKind::Horizontal => pipe_tile(x, y, PieceShape::Straight, 1, false, false),
        WallKind::Corner(dir) => pipe_tile(x, y, PieceShape::Elbow, dir, false, false),
    }
}

fn fallback_level_with_elements(
    max_cols: usize,
    max_rows: usize,
    elements: EndlessElements,
) -> LevelDef {
    let elements = elements.sanitized();
    let cols = max_cols.clamp(MIN_GEN_COLS, 5);
    let rows = max_rows.clamp(MIN_GEN_ROWS, 5);
    let mid_y = rows / 2;
    let movable_only = !elements.rotatable;
    let mut tiles = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            let tile = if y == mid_y && x == 0 {
                source_tile(x, y, 3)
            } else if y == mid_y && x + 1 == cols {
                drain_tile(x, y, 1)
            } else if movable_only {
                if y == mid_y && x == 1 {
                    pipe_tile(x, y, PieceShape::Straight, 0, true, false)
                } else if (y == mid_y && x > 1) || (y + 1 == mid_y && x == 1) {
                    pipe_tile(x, y, PieceShape::Straight, 1, true, false)
                } else if elements.rocks {
                    blocker_tile(x, y)
                } else {
                    pipe_tile(x, y, PieceShape::Straight, 0, true, false)
                }
            } else if y == mid_y {
                pipe_tile(x, y, PieceShape::Straight, 0, false, true)
            } else if elements.rocks {
                blocker_tile(x, y)
            } else {
                pipe_tile(x, y, PieceShape::Straight, 0, false, true)
            };
            tiles.push(tile);
        }
    }
    let mut fallback = LevelDef {
        index: 0,
        title: "Generated Custom".to_string(),
        description: "Generated puzzle.".to_string(),
        background_id: "NightHills".to_string(),
        liquid_color: GameColor::Blue,
        start_delay: 10.0,
        liquid_speed: 1.6,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 1,
        par_pipes: cols as u32,
        par_time_ms: 15000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    };
    if let SolveOutcome::Solvable(res) = solve_level(&fallback, Duration::from_millis(250)) {
        fallback.par_moves = res.suggested_par_moves;
        fallback.par_pipes = res.suggested_par_pipes;
        fallback.par_time_ms = res.suggested_par_time_ms;
    }
    fallback
}

fn fallback_level_within(diff: GenDifficulty, max_cols: usize, max_rows: usize) -> LevelDef {
    let cols = max_cols.clamp(MIN_GEN_COLS, 5);
    let rows = max_rows.clamp(MIN_GEN_ROWS, 5);
    let mid_y = rows / 2;
    let mut tiles = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            let tile = if y == mid_y && x == 0 {
                source_tile(x, y, 3)
            } else if y == mid_y && x + 1 == cols {
                drain_tile(x, y, 1)
            } else if y == mid_y {
                pipe_tile(x, y, PieceShape::Straight, 0, false, true)
            } else {
                blocker_tile(x, y)
            };
            tiles.push(tile);
        }
    }
    let mut fallback = LevelDef {
        index: 0,
        title: match diff {
            GenDifficulty::Easy => "Generated Easy".to_string(),
            GenDifficulty::Medium => "Generated Medium".to_string(),
            GenDifficulty::Hard => "Generated Hard".to_string(),
        },
        description: "Generated puzzle.".to_string(),
        background_id: "NightHills".to_string(),
        liquid_color: GameColor::Blue,
        start_delay: 10.0,
        liquid_speed: 1.5,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: (cols.saturating_sub(2)).max(1) as u32,
        par_pipes: cols as u32,
        par_time_ms: 15000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    };
    if let SolveOutcome::Solvable(res) = solve_level(&fallback, Duration::from_millis(250)) {
        fallback.par_moves = res.suggested_par_moves;
        fallback.par_pipes = res.suggested_par_pipes;
        fallback.par_time_ms = res.suggested_par_time_ms;
    }
    fallback
}

fn pick_interactive_flags(
    elements: EndlessElements,
    movable_bias: f64,
    rng: &mut impl RngExt,
) -> (bool, bool) {
    match (elements.movable, elements.rotatable) {
        (true, true) => {
            if rng.random_bool(movable_bias) {
                (true, false)
            } else {
                (false, true)
            }
        }
        (true, false) => (true, false),
        (false, true) => (false, true),
        (false, false) => (true, false),
    }
}

fn pick_grid_and_min_len(
    max_cols: usize,
    max_rows: usize,
    richer_path: bool,
    rng: &mut impl RngExt,
) -> (usize, usize, usize) {
    let cols = rng.random_range(MIN_GEN_COLS..=max_cols);
    let rows = rng.random_range(MIN_GEN_ROWS..=max_rows);
    let avail = (cols * rows).saturating_sub(2);
    let max_feasible = (avail * 50 / 100).max(2);
    let target_min_len = if richer_path {
        ((cols * rows) * 28 / 100).clamp(5, 18)
    } else {
        ((cols * rows) * 20 / 100).clamp(4, 14)
    };
    let min_len = target_min_len.min(max_feasible);
    (cols, rows, min_len)
}

fn spigot_side(dir: u8) -> Side {
    match dir % 4 {
        0 => Side::Down,
        1 => Side::Left,
        2 => Side::Up,
        _ => Side::Right,
    }
}

fn spigot_dir_toward(side: Side) -> u8 {
    match side {
        Side::Down | Side::Center => 0,
        Side::Left => 1,
        Side::Up => 2,
        Side::Right => 3,
    }
}

fn step_in_side(
    pos: (usize, usize),
    side: Side,
    cols: usize,
    rows: usize,
) -> Option<(usize, usize)> {
    let (dx, dy) = side.delta();
    let nx = pos.0 as i32 + dx;
    let ny = pos.1 as i32 + dy;
    if nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
        Some((nx as usize, ny as usize))
    } else {
        None
    }
}

fn ports_for_shape(shape: PieceShape, dir: u8) -> [bool; 4] {
    let mut ports = shape.default_ports();
    ports.rotate_right((dir % 4) as usize);
    ports
}

fn shuffled_portal_colors(rng: &mut impl RngExt) -> [GameColor; 5] {
    let mut palette = PORTAL_PALETTE;
    for i in (1..palette.len()).rev() {
        let j = rng.random_range(0..=i);
        palette.swap(i, j);
    }
    palette
}

#[allow(clippy::too_many_arguments)]
fn build_segment_steps(
    cols: usize,
    rows: usize,
    from_pos: (usize, usize),
    from_side: Side,
    to_pos: (usize, usize),
    to_side: Side,
    blocked: &[bool],
    min_len: usize,
    seg_idx: usize,
    rng: &mut impl RngExt,
) -> Option<Vec<PathStep>> {
    let start_pipe = step_in_side(from_pos, from_side, cols, rows)?;
    if start_pipe == to_pos {
        return Some(Vec::new());
    }
    let end_pipe = step_in_side(to_pos, to_side, cols, rows)?;
    if blocked[start_pipe.1 * cols + start_pipe.0] || blocked[end_pipe.1 * cols + end_pipe.0] {
        return None;
    }

    let raw_path = if start_pipe == end_pipe {
        vec![start_pipe]
    } else {
        random_self_avoiding_path(cols, rows, start_pipe, end_pipe, blocked, min_len, rng).or_else(
            || {
                if min_len > 2 {
                    random_self_avoiding_path(cols, rows, start_pipe, end_pipe, blocked, 2, rng)
                } else {
                    None
                }
            },
        )?
    };

    let mut steps = Vec::with_capacity(raw_path.len());
    for i in 0..raw_path.len() {
        let pos = raw_path[i];
        let prev = if i == 0 { from_pos } else { raw_path[i - 1] };
        let next = if i + 1 < raw_path.len() {
            raw_path[i + 1]
        } else {
            to_pos
        };
        let entry = side_between(pos, prev)?;
        let exit = side_between(pos, next)?;
        if entry == exit {
            return None;
        }
        steps.push(PathStep {
            pos,
            entry,
            exit,
            force_rotatable: false,
            seg_idx,
        });
    }
    Some(steps)
}

fn build_plain_topology(
    cols: usize,
    rows: usize,
    min_len: usize,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let ((sx, sy, s_dir), (dx, dy, d_dir)) = if rows >= 4 && rng.random_bool(0.35) {
        let sx = rng.random_range(0..cols);
        let dx = rng.random_range(0..cols);
        if rng.random_bool(0.5) {
            ((sx, 0, 0u8), (dx, rows - 1, 2u8))
        } else {
            ((sx, rows - 1, 2u8), (dx, 0, 0u8))
        }
    } else {
        let sy = rng.random_range(0..rows);
        let dy = rng.random_range(0..rows);
        if rng.random_bool(0.35) {
            ((cols - 1, sy, 1u8), (0, dy, 3u8))
        } else {
            ((0, sy, 3u8), (cols - 1, dy, 1u8))
        }
    };

    let mut blocked = vec![false; cols * rows];
    blocked[sy * cols + sx] = true;
    blocked[dy * cols + dx] = true;

    let path_steps = build_segment_steps(
        cols,
        rows,
        (sx, sy),
        spigot_side(s_dir),
        (dx, dy),
        spigot_side(d_dir),
        &blocked,
        min_len,
        0,
        rng,
    )?;

    Some(SolvedLayout {
        source: (sx, sy, s_dir),
        drain: (dx, dy, d_dir),
        path_steps,
        portals: Vec::new(),
        wall_tiles: HashMap::new(),
    })
}

fn build_portal_topology(
    cols: usize,
    rows: usize,
    elements: EndlessElements,
    want_gate_magic: bool,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let can_wall = elements.rocks || elements.immovable;
    let use_imm_wall = elements.immovable && (!elements.rocks || rng.random_bool(0.45));
    let colors = shuffled_portal_colors(rng);

    let mut candidates: Vec<(u8, u32)> = Vec::new();
    if can_wall {
        if cols >= 8 || rows >= 8 {
            candidates.push((2, 25)); // P2: 2 portal pairs, 3 isolated regions
        }
        if cols >= 5 || rows >= 5 {
            candidates.push((1, 45)); // P1: 1 wall dividing 2 regions
        }
        candidates.push((3, 30)); // P3: Enclosed corner vault
    } else {
        candidates.push((4, 100)); // P4: Wall-less guaranteed portals
    }

    let total_weight: u32 = candidates.iter().map(|&(_, w)| w).sum();
    let pick = rng.random_range(0..total_weight);
    let mut acc = 0;
    let mut chosen = candidates[0].0;
    for &(kind, w) in &candidates {
        acc += w;
        if pick < acc {
            chosen = kind;
            break;
        }
    }

    match chosen {
        2 => build_portal_double_wall(cols, rows, use_imm_wall, colors, rng)
            .or_else(|| build_portal_single_wall(cols, rows, use_imm_wall, colors[0], rng))
            .or_else(|| build_portal_corner_vault(cols, rows, use_imm_wall, colors[0], rng)),
        1 => build_portal_single_wall(cols, rows, use_imm_wall, colors[0], rng)
            .or_else(|| build_portal_corner_vault(cols, rows, use_imm_wall, colors[0], rng)),
        3 => build_portal_corner_vault(cols, rows, use_imm_wall, colors[0], rng),
        _ => build_portal_wallless(cols, rows, elements, want_gate_magic, colors[0], rng),
    }
}

fn build_portal_double_wall(
    cols: usize,
    rows: usize,
    use_imm_wall: bool,
    colors: [GameColor; 5],
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let vertical = if cols >= 8 && rows >= 8 {
        rng.random_bool(0.5)
    } else {
        cols >= 8
    };
    let forward = rng.random_bool(0.5);
    let mut blocked = vec![false; cols * rows];
    let mut wall_tiles = HashMap::new();

    if vertical {
        if cols < 8 {
            return None;
        }
        let wx1 = rng.random_range(2..=(cols - 6));
        let wx2 = rng.random_range((wx1 + 3)..=(cols - 3));
        let (w_first, w_second) = if forward { (wx1, wx2) } else { (wx2, wx1) };
        let (sx, s_dir, p1_a_dir, p1_b_dir, dx, d_dir) = if forward {
            (0usize, 3u8, 1u8, 3u8, cols - 1, 1u8)
        } else {
            (cols - 1, 1u8, 3u8, 1u8, 0usize, 3u8)
        };
        let sy = rng.random_range(0..rows);
        let dy = rng.random_range(0..rows);

        let ry1a = rng.random_range(0..rows);
        let mut ry1b = rng.random_range(0..rows);
        if ry1b == ry1a {
            ry1b = (ry1a + 1 + rng.random_range(0..(rows - 1))) % rows;
        }
        let ry2a = rng.random_range(0..rows);
        let mut ry2b = rng.random_range(0..rows);
        if ry2b == ry2a {
            ry2b = (ry2a + 1 + rng.random_range(0..(rows - 1))) % rows;
        }

        let p1a = (w_first, ry1a);
        let p1b = (w_first, ry1b);
        let p2a = (w_second, ry2a);
        let p2b = (w_second, ry2b);

        for y in 0..rows {
            for wx in [wx1, wx2] {
                blocked[y * cols + wx] = true;
                if (wx, y) != p1a && (wx, y) != p1b && (wx, y) != p2a && (wx, y) != p2b {
                    wall_tiles.insert(
                        (wx, y),
                        make_wall_tile(wx, y, WallKind::Vertical, use_imm_wall),
                    );
                }
            }
        }
        blocked[sy * cols + sx] = true;
        blocked[dy * cols + dx] = true;

        let seg1_min = rows.clamp(2, 4);
        let seg2_min = ((w_first.abs_diff(w_second) - 1) * rows / 2).clamp(2, 5);
        let seg3_min = rows.clamp(2, 4);

        let mut path_steps = build_segment_steps(
            cols,
            rows,
            (sx, sy),
            spigot_side(s_dir),
            p1a,
            spigot_side(p1_a_dir),
            &blocked,
            seg1_min,
            0,
            rng,
        )?;
        let seg2 = build_segment_steps(
            cols,
            rows,
            p1b,
            spigot_side(p1_b_dir),
            p2a,
            spigot_side(p1_a_dir),
            &blocked,
            seg2_min,
            1,
            rng,
        )?;
        let seg3 = build_segment_steps(
            cols,
            rows,
            p2b,
            spigot_side(p1_b_dir),
            (dx, dy),
            spigot_side(d_dir),
            &blocked,
            seg3_min,
            2,
            rng,
        )?;
        path_steps.extend(seg2);
        path_steps.extend(seg3);

        Some(SolvedLayout {
            source: (sx, sy, s_dir),
            drain: (dx, dy, d_dir),
            path_steps,
            portals: vec![
                PortalPlacement {
                    entry_pos: p1a,
                    entry_dir: p1_a_dir,
                    exit_pos: p1b,
                    exit_dir: p1_b_dir,
                    color: colors[0],
                },
                PortalPlacement {
                    entry_pos: p2a,
                    entry_dir: p1_a_dir,
                    exit_pos: p2b,
                    exit_dir: p1_b_dir,
                    color: colors[1],
                },
            ],
            wall_tiles,
        })
    } else {
        if rows < 8 {
            return None;
        }
        let wy1 = rng.random_range(2..=(rows - 6));
        let wy2 = rng.random_range((wy1 + 3)..=(rows - 3));
        let (w_first, w_second) = if forward { (wy1, wy2) } else { (wy2, wy1) };
        let (sy, s_dir, p1_a_dir, p1_b_dir, dy, d_dir) = if forward {
            (0usize, 0u8, 2u8, 0u8, rows - 1, 2u8)
        } else {
            (rows - 1, 2u8, 0u8, 2u8, 0usize, 0u8)
        };
        let sx = rng.random_range(0..cols);
        let dx = rng.random_range(0..cols);

        let rx1a = rng.random_range(0..cols);
        let mut rx1b = rng.random_range(0..cols);
        if rx1b == rx1a {
            rx1b = (rx1a + 1 + rng.random_range(0..(cols - 1))) % cols;
        }
        let rx2a = rng.random_range(0..cols);
        let mut rx2b = rng.random_range(0..cols);
        if rx2b == rx2a {
            rx2b = (rx2a + 1 + rng.random_range(0..(cols - 1))) % cols;
        }

        let p1a = (rx1a, w_first);
        let p1b = (rx1b, w_first);
        let p2a = (rx2a, w_second);
        let p2b = (rx2b, w_second);

        for x in 0..cols {
            for wy in [wy1, wy2] {
                blocked[wy * cols + x] = true;
                if (x, wy) != p1a && (x, wy) != p1b && (x, wy) != p2a && (x, wy) != p2b {
                    wall_tiles.insert(
                        (x, wy),
                        make_wall_tile(x, wy, WallKind::Horizontal, use_imm_wall),
                    );
                }
            }
        }
        blocked[sy * cols + sx] = true;
        blocked[dy * cols + dx] = true;

        let seg1_min = cols.clamp(2, 4);
        let seg2_min = ((w_first.abs_diff(w_second) - 1) * cols / 2).clamp(2, 5);
        let seg3_min = cols.clamp(2, 4);

        let mut path_steps = build_segment_steps(
            cols,
            rows,
            (sx, sy),
            spigot_side(s_dir),
            p1a,
            spigot_side(p1_a_dir),
            &blocked,
            seg1_min,
            0,
            rng,
        )?;
        let seg2 = build_segment_steps(
            cols,
            rows,
            p1b,
            spigot_side(p1_b_dir),
            p2a,
            spigot_side(p1_a_dir),
            &blocked,
            seg2_min,
            1,
            rng,
        )?;
        let seg3 = build_segment_steps(
            cols,
            rows,
            p2b,
            spigot_side(p1_b_dir),
            (dx, dy),
            spigot_side(d_dir),
            &blocked,
            seg3_min,
            2,
            rng,
        )?;
        path_steps.extend(seg2);
        path_steps.extend(seg3);

        Some(SolvedLayout {
            source: (sx, sy, s_dir),
            drain: (dx, dy, d_dir),
            path_steps,
            portals: vec![
                PortalPlacement {
                    entry_pos: p1a,
                    entry_dir: p1_a_dir,
                    exit_pos: p1b,
                    exit_dir: p1_b_dir,
                    color: colors[0],
                },
                PortalPlacement {
                    entry_pos: p2a,
                    entry_dir: p1_a_dir,
                    exit_pos: p2b,
                    exit_dir: p1_b_dir,
                    color: colors[1],
                },
            ],
            wall_tiles,
        })
    }
}

fn build_portal_single_wall(
    cols: usize,
    rows: usize,
    use_imm_wall: bool,
    color: GameColor,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let vertical = if cols >= 5 && rows >= 5 {
        rng.random_bool(0.5)
    } else {
        cols >= 5
    };
    let forward = rng.random_bool(0.5);
    let wall_mounted = rng.random_bool(0.55);

    let mut blocked = vec![false; cols * rows];
    let mut wall_tiles = HashMap::new();

    if vertical {
        if cols < 5 {
            return None;
        }
        let wx = rng.random_range(2..=(cols - 3));
        for y in 0..rows {
            blocked[y * cols + wx] = true;
            wall_tiles.insert(
                (wx, y),
                make_wall_tile(wx, y, WallKind::Vertical, use_imm_wall),
            );
        }

        if wall_mounted {
            let ry1 = rng.random_range(0..rows);
            let mut ry2 = rng.random_range(0..rows);
            if ry2 == ry1 {
                ry2 = (ry1 + 1 + rng.random_range(0..(rows - 1))) % rows;
            }
            let (sx, s_dir, pa_dir, pb_dir, dx, d_dir) = if forward {
                (0usize, 3u8, 1u8, 3u8, cols - 1, 1u8)
            } else {
                (cols - 1, 1u8, 3u8, 1u8, 0usize, 3u8)
            };
            let sy = rng.random_range(0..rows);
            let dy = rng.random_range(0..rows);
            let pa = (wx, ry1);
            let pb = (wx, ry2);
            wall_tiles.remove(&pa);
            wall_tiles.remove(&pb);
            blocked[sy * cols + sx] = true;
            blocked[dy * cols + dx] = true;

            let min1 = (wx * rows / 2).clamp(2, 6);
            let min2 = ((cols - 1 - wx) * rows / 2).clamp(2, 6);
            let (r1_min, r2_min) = if forward { (min1, min2) } else { (min2, min1) };
            let mut path_steps = build_segment_steps(
                cols,
                rows,
                (sx, sy),
                spigot_side(s_dir),
                pa,
                spigot_side(pa_dir),
                &blocked,
                r1_min,
                0,
                rng,
            )?;
            let seg2 = build_segment_steps(
                cols,
                rows,
                pb,
                spigot_side(pb_dir),
                (dx, dy),
                spigot_side(d_dir),
                &blocked,
                r2_min,
                1,
                rng,
            )?;
            path_steps.extend(seg2);

            Some(SolvedLayout {
                source: (sx, sy, s_dir),
                drain: (dx, dy, d_dir),
                path_steps,
                portals: vec![PortalPlacement {
                    entry_pos: pa,
                    entry_dir: pa_dir,
                    exit_pos: pb,
                    exit_dir: pb_dir,
                    color,
                }],
                wall_tiles,
            })
        } else {
            let (r1_range, r2_range) = if forward {
                (0..wx, (wx + 1)..cols)
            } else {
                ((wx + 1)..cols, 0..wx)
            };
            let ((sx, sy, s_dir), (pax, pay, pa_dir)) =
                pick_two_spigots_in_region(r1_range.clone(), 0..rows, cols, rows, &blocked, rng)?;
            let ((pbx, pby, pb_dir), (dx, dy, d_dir)) =
                pick_two_spigots_in_region(r2_range.clone(), 0..rows, cols, rows, &blocked, rng)?;
            blocked[sy * cols + sx] = true;
            blocked[pay * cols + pax] = true;
            blocked[pby * cols + pbx] = true;
            blocked[dy * cols + dx] = true;

            let r1_area = r1_range.len() * rows;
            let r2_area = r2_range.len() * rows;
            let mut path_steps = build_segment_steps(
                cols,
                rows,
                (sx, sy),
                spigot_side(s_dir),
                (pax, pay),
                spigot_side(pa_dir),
                &blocked,
                (r1_area / 3).clamp(2, 6),
                0,
                rng,
            )?;
            let seg2 = build_segment_steps(
                cols,
                rows,
                (pbx, pby),
                spigot_side(pb_dir),
                (dx, dy),
                spigot_side(d_dir),
                &blocked,
                (r2_area / 3).clamp(2, 6),
                1,
                rng,
            )?;
            path_steps.extend(seg2);

            Some(SolvedLayout {
                source: (sx, sy, s_dir),
                drain: (dx, dy, d_dir),
                path_steps,
                portals: vec![PortalPlacement {
                    entry_pos: (pax, pay),
                    entry_dir: pa_dir,
                    exit_pos: (pbx, pby),
                    exit_dir: pb_dir,
                    color,
                }],
                wall_tiles,
            })
        }
    } else {
        if rows < 5 {
            return None;
        }
        let wy = rng.random_range(2..=(rows - 3));
        for x in 0..cols {
            blocked[wy * cols + x] = true;
            wall_tiles.insert(
                (x, wy),
                make_wall_tile(x, wy, WallKind::Horizontal, use_imm_wall),
            );
        }

        if wall_mounted {
            let rx1 = rng.random_range(0..cols);
            let mut rx2 = rng.random_range(0..cols);
            if rx2 == rx1 {
                rx2 = (rx1 + 1 + rng.random_range(0..(cols - 1))) % cols;
            }
            let (sy, s_dir, pa_dir, pb_dir, dy, d_dir) = if forward {
                (0usize, 0u8, 2u8, 0u8, rows - 1, 2u8)
            } else {
                (rows - 1, 2u8, 0u8, 2u8, 0usize, 0u8)
            };
            let sx = rng.random_range(0..cols);
            let dx = rng.random_range(0..cols);
            let pa = (rx1, wy);
            let pb = (rx2, wy);
            wall_tiles.remove(&pa);
            wall_tiles.remove(&pb);
            blocked[sy * cols + sx] = true;
            blocked[dy * cols + dx] = true;

            let min1 = (wy * cols / 2).clamp(2, 6);
            let min2 = ((rows - 1 - wy) * cols / 2).clamp(2, 6);
            let (r1_min, r2_min) = if forward { (min1, min2) } else { (min2, min1) };
            let mut path_steps = build_segment_steps(
                cols,
                rows,
                (sx, sy),
                spigot_side(s_dir),
                pa,
                spigot_side(pa_dir),
                &blocked,
                r1_min,
                0,
                rng,
            )?;
            let seg2 = build_segment_steps(
                cols,
                rows,
                pb,
                spigot_side(pb_dir),
                (dx, dy),
                spigot_side(d_dir),
                &blocked,
                r2_min,
                1,
                rng,
            )?;
            path_steps.extend(seg2);

            Some(SolvedLayout {
                source: (sx, sy, s_dir),
                drain: (dx, dy, d_dir),
                path_steps,
                portals: vec![PortalPlacement {
                    entry_pos: pa,
                    entry_dir: pa_dir,
                    exit_pos: pb,
                    exit_dir: pb_dir,
                    color,
                }],
                wall_tiles,
            })
        } else {
            let (r1_range, r2_range) = if forward {
                (0..wy, (wy + 1)..rows)
            } else {
                ((wy + 1)..rows, 0..wy)
            };
            let ((sx, sy, s_dir), (pax, pay, pa_dir)) =
                pick_two_spigots_in_region(0..cols, r1_range.clone(), cols, rows, &blocked, rng)?;
            let ((pbx, pby, pb_dir), (dx, dy, d_dir)) =
                pick_two_spigots_in_region(0..cols, r2_range.clone(), cols, rows, &blocked, rng)?;
            blocked[sy * cols + sx] = true;
            blocked[pay * cols + pax] = true;
            blocked[pby * cols + pbx] = true;
            blocked[dy * cols + dx] = true;

            let r1_area = cols * r1_range.len();
            let r2_area = cols * r2_range.len();
            let mut path_steps = build_segment_steps(
                cols,
                rows,
                (sx, sy),
                spigot_side(s_dir),
                (pax, pay),
                spigot_side(pa_dir),
                &blocked,
                (r1_area / 3).clamp(2, 6),
                0,
                rng,
            )?;
            let seg2 = build_segment_steps(
                cols,
                rows,
                (pbx, pby),
                spigot_side(pb_dir),
                (dx, dy),
                spigot_side(d_dir),
                &blocked,
                (r2_area / 3).clamp(2, 6),
                1,
                rng,
            )?;
            path_steps.extend(seg2);

            Some(SolvedLayout {
                source: (sx, sy, s_dir),
                drain: (dx, dy, d_dir),
                path_steps,
                portals: vec![PortalPlacement {
                    entry_pos: (pax, pay),
                    entry_dir: pa_dir,
                    exit_pos: (pbx, pby),
                    exit_dir: pb_dir,
                    color,
                }],
                wall_tiles,
            })
        }
    }
}

type SpigotPos = (usize, usize, u8);

fn pick_two_spigots_in_region(
    xr: std::ops::Range<usize>,
    yr: std::ops::Range<usize>,
    cols: usize,
    rows: usize,
    blocked: &[bool],
    rng: &mut impl RngExt,
) -> Option<(SpigotPos, SpigotPos)> {
    let mut candidates = Vec::new();
    for y in yr.clone() {
        for x in xr.clone() {
            if blocked[y * cols + x] {
                continue;
            }
            for dir in 0..4u8 {
                let side = spigot_side(dir);
                if let Some((nx, ny)) = step_in_side((x, y), side, cols, rows)
                    && xr.contains(&nx)
                    && yr.contains(&ny)
                    && !blocked[ny * cols + nx]
                {
                    candidates.push((x, y, dir, (nx, ny)));
                }
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    for _ in 0..24 {
        let a = candidates[rng.random_range(0..candidates.len())];
        let b = candidates[rng.random_range(0..candidates.len())];
        let a_pos = (a.0, a.1);
        let b_pos = (b.0, b.1);
        if a_pos != b_pos && a.3 != b_pos && b.3 != a_pos {
            return Some(((a.0, a.1, a.2), (b.0, b.1, b.2)));
        }
    }
    None
}

fn build_portal_corner_vault(
    cols: usize,
    rows: usize,
    use_imm_wall: bool,
    color: GameColor,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    if cols < 4 || rows < 3 {
        return None;
    }
    let (vw, vh) = if cols >= 5 && rows >= 4 {
        (
            rng.random_range(2..=(cols - 3).min(3)),
            rng.random_range(2..=(rows - 2).min(3)),
        )
    } else if rows >= 4 {
        (2, 2)
    } else {
        (2, 1)
    };

    let flip_x = rng.random_bool(0.5);
    let flip_y = rng.random_bool(0.5);
    let tx = |x: usize| if flip_x { cols - 1 - x } else { x };
    let ty = |y: usize| if flip_y { rows - 1 - y } else { y };
    let t_side = |s: Side| -> Side {
        match s {
            Side::Left if flip_x => Side::Right,
            Side::Right if flip_x => Side::Left,
            Side::Up if flip_y => Side::Down,
            Side::Down if flip_y => Side::Up,
            other => other,
        }
    };

    let mut blocked = vec![false; cols * rows];
    let mut wall_tiles = HashMap::new();

    for y in 0..vh {
        let wx = tx(vw);
        let wy = ty(y);
        blocked[wy * cols + wx] = true;
        wall_tiles.insert(
            (wx, wy),
            make_wall_tile(wx, wy, WallKind::Vertical, use_imm_wall),
        );
    }
    for x in 0..vw {
        let wx = tx(x);
        let wy = ty(vh);
        blocked[wy * cols + wx] = true;
        wall_tiles.insert(
            (wx, wy),
            make_wall_tile(wx, wy, WallKind::Horizontal, use_imm_wall),
        );
    }
    let cx = tx(vw);
    let cy = ty(vh);
    let (_, corner_dir) = pipe_for_sides(t_side(Side::Up), t_side(Side::Left))?;
    blocked[cy * cols + cx] = true;
    wall_tiles.insert(
        (cx, cy),
        make_wall_tile(cx, cy, WallKind::Corner(corner_dir), use_imm_wall),
    );

    // Pick vault endpoints (Drain or Source at canonical (0, 0), vault Portal at far vault corner).
    let vault_holds_drain = rng.random_bool(0.5);
    let (v_anchor_pos, v_anchor_side, v_portal_pos, v_portal_side) = if vh == 1 {
        (
            (tx(0), ty(0)),
            t_side(Side::Right),
            (tx(1), ty(0)),
            t_side(Side::Left),
        )
    } else {
        (
            (tx(0), ty(0)),
            t_side(if rng.random_bool(0.5) {
                Side::Right
            } else {
                Side::Down
            }),
            (tx(vw - 1), ty(vh - 1)),
            t_side(if rng.random_bool(0.5) {
                Side::Left
            } else {
                Side::Up
            }),
        )
    };
    let v_anchor_dir = spigot_dir_toward(v_anchor_side);
    let v_portal_dir = spigot_dir_toward(v_portal_side);

    // Outer region endpoints outside `(0..=vw, 0..=vh)` rectangle or anywhere unblocked.
    let mut outer_candidates = Vec::new();
    for y in 0..rows {
        for x in 0..cols {
            let ux = if flip_x { cols - 1 - x } else { x };
            let uy = if flip_y { rows - 1 - y } else { y };
            if ux <= vw && uy <= vh {
                continue;
            }
            for dir in 0..4u8 {
                let s = spigot_side(dir);
                if let Some((nx, ny)) = step_in_side((x, y), s, cols, rows)
                    && !blocked[ny * cols + nx]
                {
                    let nux = if flip_x { cols - 1 - nx } else { nx };
                    let nuy = if flip_y { rows - 1 - ny } else { ny };
                    if !(nux <= vw && nuy <= vh) {
                        outer_candidates.push((x, y, dir, (nx, ny)));
                    }
                }
            }
        }
    }
    if outer_candidates.len() < 2 {
        return None;
    }

    let mut chosen_outer = None;
    for _ in 0..24 {
        let a = outer_candidates[rng.random_range(0..outer_candidates.len())];
        let b = outer_candidates[rng.random_range(0..outer_candidates.len())];
        let ap = (a.0, a.1);
        let bp = (b.0, b.1);
        if ap != bp && a.3 != bp && b.3 != ap && ap.0.abs_diff(bp.0) + ap.1.abs_diff(bp.1) >= 2 {
            chosen_outer = Some((a, b));
            break;
        }
    }
    let ((ox1, oy1, odir1, _), (ox2, oy2, odir2, _)) = chosen_outer?;

    blocked[v_anchor_pos.1 * cols + v_anchor_pos.0] = true;
    blocked[v_portal_pos.1 * cols + v_portal_pos.0] = true;
    blocked[oy1 * cols + ox1] = true;
    blocked[oy2 * cols + ox2] = true;

    let outer_area = (cols * rows).saturating_sub((vw + 1) * (vh + 1));
    let outer_min = (outer_area / 3).clamp(2, 8);
    let vault_min = if vh == 1 {
        0
    } else {
        (vw * vh / 2).clamp(2, 4)
    };

    if vault_holds_drain {
        let mut path_steps = build_segment_steps(
            cols,
            rows,
            (ox1, oy1),
            spigot_side(odir1),
            (ox2, oy2),
            spigot_side(odir2),
            &blocked,
            outer_min,
            0,
            rng,
        )?;
        let seg_vault = build_segment_steps(
            cols,
            rows,
            v_portal_pos,
            spigot_side(v_portal_dir),
            v_anchor_pos,
            spigot_side(v_anchor_dir),
            &blocked,
            vault_min,
            1,
            rng,
        )?;
        path_steps.extend(seg_vault);

        Some(SolvedLayout {
            source: (ox1, oy1, odir1),
            drain: (v_anchor_pos.0, v_anchor_pos.1, v_anchor_dir),
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: (ox2, oy2),
                entry_dir: odir2,
                exit_pos: v_portal_pos,
                exit_dir: v_portal_dir,
                color,
            }],
            wall_tiles,
        })
    } else {
        let mut path_steps = build_segment_steps(
            cols,
            rows,
            v_anchor_pos,
            spigot_side(v_anchor_dir),
            v_portal_pos,
            spigot_side(v_portal_dir),
            &blocked,
            vault_min,
            0,
            rng,
        )?;
        let seg_outer = build_segment_steps(
            cols,
            rows,
            (ox1, oy1),
            spigot_side(odir1),
            (ox2, oy2),
            spigot_side(odir2),
            &blocked,
            outer_min,
            1,
            rng,
        )?;
        path_steps.extend(seg_outer);

        Some(SolvedLayout {
            source: (v_anchor_pos.0, v_anchor_pos.1, v_anchor_dir),
            drain: (ox2, oy2, odir2),
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: v_portal_pos,
                entry_dir: v_portal_dir,
                exit_pos: (ox1, oy1),
                exit_dir: odir1,
                color,
            }],
            wall_tiles,
        })
    }
}

fn build_portal_wallless(
    cols: usize,
    rows: usize,
    elements: EndlessElements,
    want_gate_magic: bool,
    color: GameColor,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    if elements.rotatable
        && cols >= 4
        && rows >= 3
        && rng.random_bool(0.55)
        && let Some(layout) = build_portal_corner_pocket(cols, rows, color, rng)
    {
        return Some(layout);
    }
    build_portal_chokepoint_anchor(cols, rows, want_gate_magic, color, rng)
}

fn build_portal_corner_pocket(
    cols: usize,
    rows: usize,
    color: GameColor,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let flip_x = rng.random_bool(0.5);
    let flip_y = rng.random_bool(0.5);
    let pocket_holds_drain = rng.random_bool(0.5);
    let tx = |x: usize| if flip_x { x } else { cols - 1 - x };
    let ty = |y: usize| if flip_y { rows - 1 - y } else { y };
    let t_side = |s: Side| -> Side {
        match s {
            Side::Left if !flip_x => Side::Right,
            Side::Right if !flip_x => Side::Left,
            Side::Up if flip_y => Side::Down,
            Side::Down if flip_y => Side::Up,
            other => other,
        }
    };

    // Canonical Top-Right corner pocket at offset x_from_edge:
    // anchor=(cols-1, 0), pocket_pipe=(cols-2, 0), p_inner=(cols-2, 1), p_outer=(cols-3, 0).
    let anchor = (tx(0), ty(0));
    let anchor_side = t_side(Side::Left);
    let anchor_dir = spigot_dir_toward(anchor_side);

    let pocket_pipe_pos = (tx(1), ty(0));
    let p_inner_pos = (tx(1), ty(1));
    let p_inner_side = t_side(Side::Up);
    let p_inner_dir = spigot_dir_toward(p_inner_side);

    let p_outer_pos = (tx(2), ty(0));
    let p_outer_side = t_side(if rng.random_bool(0.5) {
        Side::Left
    } else {
        Side::Down
    });
    let p_outer_dir = spigot_dir_toward(p_outer_side);

    // Far endpoint on opposite corner/edge of grid so entry neighbor never gets boxed in.
    let far_x = if flip_x { cols - 1 } else { 0 };
    let far_y = if flip_y {
        if rows >= 4 {
            rng.random_range(0..=(rows - 3))
        } else {
            0
        }
    } else if rows >= 4 {
        rng.random_range(2..rows)
    } else {
        rows - 1
    };
    let far_side = if flip_x { Side::Left } else { Side::Right };
    let far_dir = spigot_dir_toward(far_side);

    let mut blocked = vec![false; cols * rows];
    blocked[anchor.1 * cols + anchor.0] = true;
    blocked[pocket_pipe_pos.1 * cols + pocket_pipe_pos.0] = true;
    blocked[p_inner_pos.1 * cols + p_inner_pos.0] = true;
    blocked[p_outer_pos.1 * cols + p_outer_pos.0] = true;
    blocked[far_y * cols + far_x] = true;

    let min_len = ((cols * rows) / 4).clamp(3, 8);
    let (pocket_entry, pocket_exit) = if pocket_holds_drain {
        (t_side(Side::Down), t_side(Side::Right))
    } else {
        (t_side(Side::Right), t_side(Side::Down))
    };
    let pocket_step = PathStep {
        pos: pocket_pipe_pos,
        entry: pocket_entry,
        exit: pocket_exit,
        force_rotatable: true,
        seg_idx: if pocket_holds_drain { 1 } else { 0 },
    };

    if pocket_holds_drain {
        let mut path_steps = build_segment_steps(
            cols,
            rows,
            (far_x, far_y),
            far_side,
            p_outer_pos,
            p_outer_side,
            &blocked,
            min_len,
            0,
            rng,
        )?;
        path_steps.push(pocket_step);

        Some(SolvedLayout {
            source: (far_x, far_y, far_dir),
            drain: (anchor.0, anchor.1, anchor_dir),
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: p_outer_pos,
                entry_dir: p_outer_dir,
                exit_pos: p_inner_pos,
                exit_dir: p_inner_dir,
                color,
            }],
            wall_tiles: HashMap::new(),
        })
    } else {
        let mut path_steps = vec![pocket_step];
        let seg1 = build_segment_steps(
            cols,
            rows,
            p_outer_pos,
            p_outer_side,
            (far_x, far_y),
            far_side,
            &blocked,
            min_len,
            1,
            rng,
        )?;
        path_steps.extend(seg1);

        Some(SolvedLayout {
            source: (anchor.0, anchor.1, anchor_dir),
            drain: (far_x, far_y, far_dir),
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: p_inner_pos,
                entry_dir: p_inner_dir,
                exit_pos: p_outer_pos,
                exit_dir: p_outer_dir,
                color,
            }],
            wall_tiles: HashMap::new(),
        })
    }
}

fn build_portal_chokepoint_anchor(
    cols: usize,
    rows: usize,
    want_gate_magic: bool,
    color: GameColor,
    rng: &mut impl RngExt,
) -> Option<SolvedLayout> {
    let anchor_drain = if want_gate_magic && cols * rows <= 16 {
        false
    } else {
        rng.random_bool(0.5)
    };
    let anchor_on_right = rng.random_bool(0.5);
    let ay = rng.random_range(0..rows);

    let (anchor, p_anchor, anchor_dir, p_anchor_dir) = if anchor_on_right {
        ((cols - 1, ay), (cols - 2, ay), 1u8, 3u8)
    } else {
        ((0, ay), (1, ay), 3u8, 1u8)
    };

    let mut blocked = vec![false; cols * rows];
    blocked[anchor.1 * cols + anchor.0] = true;
    blocked[p_anchor.1 * cols + p_anchor.0] = true;

    let xr = if anchor_on_right {
        0..(cols - 1)
    } else {
        1..cols
    };
    let (ep1, ep2) = pick_two_spigots_in_region(xr, 0..rows, cols, rows, &blocked, rng)?;
    if ep1.0.abs_diff(p_anchor.0) + ep1.1.abs_diff(p_anchor.1) < 2
        && ep2.0.abs_diff(p_anchor.0) + ep2.1.abs_diff(p_anchor.1) < 2
    {
        return None;
    }

    let (far_end, p_far) = if ep2.0.abs_diff(p_anchor.0) + ep2.1.abs_diff(p_anchor.1)
        >= ep1.0.abs_diff(p_anchor.0) + ep1.1.abs_diff(p_anchor.1)
    {
        (ep1, ep2)
    } else {
        (ep2, ep1)
    };

    blocked[far_end.1 * cols + far_end.0] = true;
    blocked[p_far.1 * cols + p_far.0] = true;

    let min_len = ((cols * rows) / 4).clamp(3, 9);
    if anchor_drain {
        let path_steps = build_segment_steps(
            cols,
            rows,
            (far_end.0, far_end.1),
            spigot_side(far_end.2),
            (p_far.0, p_far.1),
            spigot_side(p_far.2),
            &blocked,
            min_len,
            0,
            rng,
        )?;
        Some(SolvedLayout {
            source: far_end,
            drain: (anchor.0, anchor.1, anchor_dir),
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: (p_far.0, p_far.1),
                entry_dir: p_far.2,
                exit_pos: p_anchor,
                exit_dir: p_anchor_dir,
                color,
            }],
            wall_tiles: HashMap::new(),
        })
    } else {
        let path_steps = build_segment_steps(
            cols,
            rows,
            (p_far.0, p_far.1),
            spigot_side(p_far.2),
            (far_end.0, far_end.1),
            spigot_side(far_end.2),
            &blocked,
            min_len,
            1,
            rng,
        )?;
        Some(SolvedLayout {
            source: (anchor.0, anchor.1, anchor_dir),
            drain: far_end,
            path_steps,
            portals: vec![PortalPlacement {
                entry_pos: p_anchor,
                entry_dir: p_anchor_dir,
                exit_pos: (p_far.0, p_far.1),
                exit_dir: p_far.2,
                color,
            }],
            wall_tiles: HashMap::new(),
        })
    }
}

fn plan_gate_magic(
    layout: &SolvedLayout,
    elements: EndlessElements,
    free_offpath: &[(usize, usize)],
    max_offpath_stationary: usize,
    rng: &mut impl RngExt,
) -> Option<GateMagicPlan> {
    let steps = &layout.path_steps;
    let l = steps.len();
    if l < 4 {
        return None;
    }

    let anchor_end = (0..l).rfind(|&i| !steps[i].force_rotatable)?;
    if anchor_end < 3 {
        return None;
    }

    let mut solved_pipes = Vec::with_capacity(l);
    for step in steps {
        solved_pipes.push(pipe_for_sides(step.entry, step.exit)?);
    }

    // Reserve >= 2 interactive steps so the player still rotates/swaps pipes.
    let mut interactive_reserved = HashSet::new();
    interactive_reserved.insert(0usize);
    if steps[l - 1].force_rotatable {
        interactive_reserved.insert(l - 1);
    } else if anchor_end >= 4 {
        let mid = rng.random_range(1..(anchor_end - 1));
        interactive_reserved.insert(mid);
    }

    let eligible_gates: Vec<usize> = (1..anchor_end)
        .filter(|i| !steps[*i].force_rotatable && !interactive_reserved.contains(i))
        .collect();
    if eligible_gates.is_empty() {
        return None;
    }

    let archetype = match rng.random_range(0..3) {
        0 => GateMagicArchetype::InterleavedChain,
        1 => GateMagicArchetype::MagicFortress,
        _ => GateMagicArchetype::GateGauntlet,
    };

    let target_gates = match archetype {
        GateMagicArchetype::InterleavedChain => {
            if anchor_end >= 10 {
                rng.random_range(2..=3)
            } else if anchor_end >= 6 {
                2
            } else {
                1
            }
        }
        GateMagicArchetype::MagicFortress => {
            if anchor_end >= 8 && rng.random_bool(0.55) {
                2
            } else {
                1
            }
        }
        GateMagicArchetype::GateGauntlet => {
            if anchor_end >= 9 {
                rng.random_range(2..=3)
            } else if anchor_end >= 6 {
                2
            } else {
                1
            }
        }
    }
    .min(eligible_gates.len())
    .min(3);

    let mut on_path_gates: HashSet<usize> = HashSet::new();
    if target_gates == 1 {
        let cap = (eligible_gates.len() / 2).max(1);
        let idx = eligible_gates[rng.random_range(0..cap)];
        on_path_gates.insert(idx);
    } else {
        // Spread gates along the path; bias Gate 1 into segment 0 when portals exist.
        if !layout.portals.is_empty()
            && let Some(&seg0_gate) = eligible_gates.iter().find(|&&i| steps[i].seg_idx == 0)
        {
            on_path_gates.insert(seg0_gate);
        }
        let max_idx_for_slots = if matches!(archetype, GateMagicArchetype::MagicFortress) {
            (eligible_gates.len() * 2 / 3).max(target_gates)
        } else {
            eligible_gates.len()
        };
        let mut cursor = 0usize;
        while on_path_gates.len() < target_gates && cursor < max_idx_for_slots {
            let slot_hi = (((on_path_gates.len() + 1) * max_idx_for_slots) / target_gates)
                .clamp(cursor + 1, max_idx_for_slots);
            let chosen = eligible_gates[rng.random_range(cursor..slot_hi)];
            on_path_gates.insert(chosen);
            cursor = eligible_gates
                .iter()
                .position(|&v| v == chosen)
                .map_or(max_idx_for_slots, |p| p + 1);
        }
    }

    let mut gates_vec: Vec<usize> = on_path_gates.iter().copied().collect();
    gates_vec.sort_unstable();

    // Ensure anchor_end can be a meaningful misaligned magic pipe.
    let anchor_shape = solved_pipes[anchor_end].0;
    let gates_at_anchor = (gates_vec.len() as u8) % 4;
    if anchor_shape == PieceShape::Straight && gates_at_anchor.is_multiple_of(2) {
        if gates_vec.len() >= 2 {
            let popped = gates_vec.pop().unwrap();
            on_path_gates.remove(&popped);
        } else {
            return None;
        }
    }

    let mut gates_before = vec![0u8; l];
    let mut g_count = 0u8;
    for (i, slot) in gates_before.iter_mut().enumerate().take(l) {
        *slot = g_count % 4;
        if on_path_gates.contains(&i) {
            g_count += 1;
        }
    }

    let mut on_path_magic_dirs: HashMap<usize, u8> = HashMap::new();
    let is_meaningful =
        |shape: PieceShape, solved_dir: u8, entry: Side, exit: Side, k: u8| -> bool {
            let k = k % 4;
            if k == 0 {
                return false;
            }
            if matches!(shape, PieceShape::Cross)
                || (shape == PieceShape::Straight && k.is_multiple_of(2))
            {
                return false;
            }
            let init_dir = (solved_dir + 4 - k) % 4;
            let ports = ports_for_shape(shape, init_dir);
            !(ports[entry.index()] && ports[exit.index()])
        };

    // Always anchor `anchor_end` if meaningful, else fallback cleanly.
    let (a_shape, a_solved_dir) = solved_pipes[anchor_end];
    if is_meaningful(
        a_shape,
        a_solved_dir,
        steps[anchor_end].entry,
        steps[anchor_end].exit,
        gates_before[anchor_end],
    ) {
        on_path_magic_dirs.insert(
            anchor_end,
            (a_solved_dir + 4 - gates_before[anchor_end]) % 4,
        );
    }

    let mut magic_candidates: Vec<usize> = (1..anchor_end)
        .filter(|&i| {
            !on_path_gates.contains(&i)
                && !interactive_reserved.contains(&i)
                && !steps[i].force_rotatable
                && is_meaningful(
                    solved_pipes[i].0,
                    solved_pipes[i].1,
                    steps[i].entry,
                    steps[i].exit,
                    gates_before[i],
                )
        })
        .collect();

    match archetype {
        GateMagicArchetype::MagicFortress => {
            // Take from the end backwards to form a long contiguous chain leading to Drain/Portal.
            let take = ((l * 2 / 3).max(3)).min(magic_candidates.len());
            for &i in magic_candidates.iter().rev().take(take) {
                let (_, solved_dir) = solved_pipes[i];
                on_path_magic_dirs.insert(i, (solved_dir + 4 - gates_before[i]) % 4);
            }
        }
        GateMagicArchetype::InterleavedChain => {
            let target = rng.random_range((l / 2).max(2)..=((l * 2 / 3).max(3)));
            for &i in magic_candidates.iter().rev() {
                if on_path_magic_dirs.len() >= target {
                    break;
                }
                // Prefer interleaving: ensure at least 1 magic tile after each gate when possible.
                let (_, solved_dir) = solved_pipes[i];
                on_path_magic_dirs.insert(i, (solved_dir + 4 - gates_before[i]) % 4);
            }
        }
        GateMagicArchetype::GateGauntlet => {
            for i in (1..magic_candidates.len()).rev() {
                let j = rng.random_range(0..=i);
                magic_candidates.swap(i, j);
            }
            let target = rng.random_range(2..=(l / 2).max(2));
            for &i in &magic_candidates {
                if on_path_magic_dirs.len() >= target {
                    break;
                }
                let (_, solved_dir) = solved_pipes[i];
                on_path_magic_dirs.insert(i, (solved_dir + 4 - gates_before[i]) % 4);
            }
        }
    }

    // Double check >= 2 interactive path steps remain if elements allow.
    let remaining_interactive = (0..l)
        .filter(|i| !on_path_gates.contains(i) && !on_path_magic_dirs.contains_key(i))
        .count();
    if remaining_interactive < 2 {
        return None;
    }
    if on_path_gates.is_empty() || on_path_magic_dirs.is_empty() {
        return None;
    }

    // Plan off-path magic pipes & decoy gates within stationary budget.
    let mut magic_anchors: Vec<(usize, usize)> =
        on_path_magic_dirs.keys().map(|&i| steps[i].pos).collect();
    magic_anchors.push((layout.drain.0, layout.drain.1));

    let mut ranked_off = free_offpath.to_vec();
    // Shuffle first to vary equal-distance picks, then sort by cluster proximity.
    for i in (1..ranked_off.len()).rev() {
        let j = rng.random_range(0..=i);
        ranked_off.swap(i, j);
    }
    ranked_off.sort_by_key(|&(x, y)| {
        magic_anchors
            .iter()
            .map(|&(ax, ay)| x.abs_diff(ax) + y.abs_diff(ay))
            .min()
            .unwrap_or(99)
    });

    let avail_budget = if elements.rocks || elements.immovable {
        (max_offpath_stationary * 2 / 3).max(max_offpath_stationary.min(4))
    } else {
        max_offpath_stationary
    };

    let (want_off_magic, want_decoy_gates) = match archetype {
        GateMagicArchetype::MagicFortress => {
            let m_lo = (free_offpath.len() / 4).clamp(2, 6);
            let m_hi = (free_offpath.len() * 2 / 5).clamp(m_lo, 14);
            let m = rng.random_range(m_lo..=m_hi);
            let g_hi = (free_offpath.len() / 8).clamp(1, 3);
            let g = if free_offpath.len() >= 10 {
                rng.random_range(0..=g_hi)
            } else {
                0
            };
            (m, g)
        }
        GateMagicArchetype::InterleavedChain => {
            let m_hi = (free_offpath.len() / 4).clamp(2, 8);
            let m = rng.random_range(2..=m_hi);
            let g = if free_offpath.len() >= 12 {
                rng.random_range(0..=2)
            } else {
                0
            };
            (m, g)
        }
        GateMagicArchetype::GateGauntlet => {
            let g_hi = (free_offpath.len() / 5).clamp(2, 7);
            let g = rng.random_range(2..=g_hi);
            let m_hi = (free_offpath.len() / 6).clamp(1, 5);
            let m = rng.random_range(1..=m_hi);
            (m, g)
        }
    };

    let off_magic_n = want_off_magic.min(avail_budget).min(ranked_off.len());
    let off_gate_n = want_decoy_gates
        .min(avail_budget.saturating_sub(off_magic_n))
        .min(ranked_off.len().saturating_sub(off_magic_n));

    let mut off_path_magic = Vec::with_capacity(off_magic_n);
    for &pos in ranked_off.iter().take(off_magic_n) {
        let shape = if rng.random_bool(0.6) {
            PieceShape::Elbow
        } else {
            PieceShape::Straight
        };
        let dir = rng.random_range(0..4) as u8;
        off_path_magic.push((pos, shape, dir));
    }

    // Spread decoy gates slightly farther across remaining free cells.
    let mut gate_pool = ranked_off[off_magic_n..].to_vec();
    for i in (1..gate_pool.len()).rev() {
        let j = rng.random_range(0..=i);
        gate_pool.swap(i, j);
    }
    let mut off_path_gates = Vec::with_capacity(off_gate_n);
    for &pos in gate_pool.iter().take(off_gate_n) {
        let shape = match rng.random_range(0..10) {
            0..=4 => PieceShape::Elbow,
            5..=8 => PieceShape::Straight,
            _ => PieceShape::Cross,
        };
        let dir = rng.random_range(0..4) as u8;
        off_path_gates.push((pos, shape, dir));
    }

    Some(GateMagicPlan {
        on_path_gates,
        on_path_magic_dirs,
        off_path_magic,
        off_path_gates,
    })
}

fn try_build_spec_candidate(
    spec: GenSpec,
    max_cols: usize,
    max_rows: usize,
    rng: &mut impl RngExt,
    attempt: usize,
) -> Option<LevelDef> {
    let elements = spec.elements;
    let want_portal = elements.portals && rng.random_bool(spec.portal_chance);
    let want_gate_magic = elements.gate_magic && rng.random_bool(spec.gate_magic_chance);

    let (cols, rows, min_len) = pick_grid_and_min_len(max_cols, max_rows, want_gate_magic, rng);

    let layout = if want_portal {
        build_portal_topology(cols, rows, elements, want_gate_magic, rng)
    } else {
        build_plain_topology(cols, rows, min_len, rng)
    }?;

    let mut tiles_map: HashMap<(usize, usize), TileDef> = layout.wall_tiles.clone();
    tiles_map.insert(
        (layout.source.0, layout.source.1),
        source_tile(layout.source.0, layout.source.1, layout.source.2),
    );
    tiles_map.insert(
        (layout.drain.0, layout.drain.1),
        drain_tile(layout.drain.0, layout.drain.1, layout.drain.2),
    );
    for p in &layout.portals {
        tiles_map.insert(
            p.entry_pos,
            portal_tile(p.entry_pos.0, p.entry_pos.1, p.entry_dir, p.color),
        );
        tiles_map.insert(
            p.exit_pos,
            portal_tile(p.exit_pos.0, p.exit_pos.1, p.exit_dir, p.color),
        );
    }

    // Mark path positions to compute free off-path coordinates for Gate/Magic planning.
    let mut occupied: HashSet<(usize, usize)> = tiles_map.keys().copied().collect();
    for step in &layout.path_steps {
        occupied.insert(step.pos);
    }
    let mut free_offpath = Vec::new();
    for y in 0..rows {
        for x in 0..cols {
            if !occupied.contains(&(x, y)) {
                free_offpath.push((x, y));
            }
        }
    }

    let max_offpath_stationary = if !elements.rotatable {
        (free_offpath.len() * 38) / 100
    } else {
        (free_offpath.len() * 45) / 100
    };

    let gm_plan = if want_gate_magic {
        plan_gate_magic(
            &layout,
            elements,
            &free_offpath,
            max_offpath_stationary,
            rng,
        )
    } else {
        None
    };

    // Place on-path pipes.
    for (i, step) in layout.path_steps.iter().enumerate() {
        let (x, y) = step.pos;
        let (shape, solved_dir) = pipe_for_sides(step.entry, step.exit)?;
        let is_gate = gm_plan
            .as_ref()
            .is_some_and(|p| p.on_path_gates.contains(&i));
        let magic_dir = gm_plan
            .as_ref()
            .and_then(|p| p.on_path_magic_dirs.get(&i).copied());
        let is_magic = magic_dir.is_some();

        let (is_moveable, can_rotate) = if step.force_rotatable {
            (false, true)
        } else if is_gate
            || is_magic
            || (elements.immovable && i > 0 && rng.random_bool(spec.immovable_path_chance))
        {
            (false, false)
        } else {
            pick_interactive_flags(elements, 0.5, rng)
        };

        let is_frozen = elements.frosted
            && !is_magic
            && !step.force_rotatable
            && rng.random_bool(spec.frosted_path_chance);
        let is_freeze = is_frozen && (is_moveable || can_rotate);
        let dir = magic_dir.unwrap_or(solved_dir);

        tiles_map.insert(
            (x, y),
            TileDef {
                x,
                y,
                shape,
                piece_type: if is_frozen {
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

    // Place off-path Gate/Magic tiles if planned.
    let mut offpath_stationary_used = 0usize;
    if let Some(plan) = &gm_plan {
        for &(pos, shape, dir) in &plan.off_path_magic {
            tiles_map.insert(
                pos,
                TileDef {
                    x: pos.0,
                    y: pos.1,
                    shape,
                    piece_type: PieceType::Pipe,
                    direction: dir,
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: false,
                    is_freeze: false,
                    is_magic: true,
                    secondary_color: None,
                },
            );
            offpath_stationary_used += 1;
        }
        for &(pos, shape, dir) in &plan.off_path_gates {
            tiles_map.insert(
                pos,
                TileDef {
                    x: pos.0,
                    y: pos.1,
                    shape,
                    piece_type: PieceType::Pipe,
                    direction: dir,
                    is_moveable: false,
                    can_rotate: false,
                    is_gate: true,
                    is_freeze: false,
                    is_magic: false,
                    secondary_color: None,
                },
            );
            offpath_stationary_used += 1;
        }
    }

    // Fill remaining off-path cells with rocks, immovables, and interactive distractors.
    for y in 0..rows {
        for x in 0..cols {
            if tiles_map.contains_key(&(x, y)) {
                continue;
            }
            let can_add_stationary = offpath_stationary_used < max_offpath_stationary;
            if elements.rocks && can_add_stationary && rng.random_bool(spec.rock_chance) {
                tiles_map.insert((x, y), blocker_tile(x, y));
                offpath_stationary_used += 1;
            } else {
                let shape = match rng.random_range(0..3) {
                    0 => PieceShape::Elbow,
                    1 => PieceShape::Straight,
                    _ => PieceShape::Cross,
                };
                let (is_moveable, can_rotate) = if elements.immovable
                    && can_add_stationary
                    && rng.random_bool(spec.immovable_offpath_chance)
                {
                    offpath_stationary_used += 1;
                    (false, false)
                } else {
                    pick_interactive_flags(elements, 0.65, rng)
                };
                let is_frozen = elements.frosted && rng.random_bool(spec.frosted_offpath_chance);
                let is_freeze = is_frozen && (is_moveable || can_rotate);
                tiles_map.insert(
                    (x, y),
                    TileDef {
                        x,
                        y,
                        shape,
                        piece_type: if is_frozen {
                            PieceType::FrozenPipe
                        } else {
                            PieceType::Pipe
                        },
                        direction: rng.random_range(0..4) as u8,
                        is_moveable,
                        can_rotate,
                        is_gate: false,
                        is_freeze,
                        is_magic: false,
                        secondary_color: None,
                    },
                );
            }
        }
    }

    // Scramble rotatables and shuffle movable connected components.
    let mut rotated_any = false;
    for t in tiles_map.values_mut() {
        if t.can_rotate && t.shape != PieceShape::Cross {
            let offset = rng.random_range(1..4) as u8;
            t.direction = (t.direction + offset) % 4;
            rotated_any = true;
        }
    }
    let moved_any = shuffle_movable_components(&mut tiles_map, cols, rows, rng);

    if !rotated_any && !moved_any {
        return None;
    }

    let mut tiles = Vec::with_capacity(cols * rows);
    for y in 0..rows {
        for x in 0..cols {
            tiles.push(tiles_map.remove(&(x, y))?);
        }
    }

    let bg = spec.bgs[rng.random_range(0..spec.bgs.len())];
    let liquid_color = CAMPAIGN_LIQUID_COLORS[rng.random_range(0..CAMPAIGN_LIQUID_COLORS.len())];

    Some(LevelDef {
        index: 0,
        title: format!("{} Puzzle {}", spec.title_prefix, attempt + 1),
        description: "Generated puzzle.".to_string(),
        background_id: bg.to_string(),
        liquid_color,
        start_delay: spec.start_delay,
        liquid_speed: spec.liquid_speed,
        rotater_color: GameColor::White,
        mover_color: GameColor::Yellow,
        stationary_color: GameColor::Orange,
        gate_color: GameColor::Violet,
        freeze_color: GameColor::Yellow,
        magic_color: GameColor::Green,
        par_moves: 10,
        par_pipes: layout.path_steps.len() as u32,
        par_time_ms: 20000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    })
}

fn shuffle_movable_components(
    tiles_map: &mut HashMap<(usize, usize), TileDef>,
    cols: usize,
    rows: usize,
    rng: &mut impl RngExt,
) -> bool {
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
    let mut moved_any = false;
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
            let orig: Vec<(PieceShape, u8)> = comp
                .iter()
                .map(|p| {
                    let t = &tiles_map[p];
                    let norm_dir = match t.shape {
                        PieceShape::Cross => 0,
                        PieceShape::Straight => t.direction % 2,
                        _ => t.direction % 4,
                    };
                    (t.shape, norm_dir)
                })
                .collect();
            let mut snapshots: Vec<TileDef> = comp.iter().map(|p| tiles_map[p].clone()).collect();
            for i in (1..snapshots.len()).rev() {
                let j = rng.random_range(0..=i);
                snapshots.swap(i, j);
            }
            let same_layout = |snaps: &[TileDef]| -> bool {
                orig.iter().zip(snaps).all(|(&(s, d), td)| {
                    let nd = match td.shape {
                        PieceShape::Cross => 0,
                        PieceShape::Straight => td.direction % 2,
                        _ => td.direction % 4,
                    };
                    td.shape == s && nd == d
                })
            };
            if same_layout(&snapshots) {
                for idx in 1..snapshots.len() {
                    let (s0, d0) = orig[0];
                    let td = &snapshots[idx];
                    let nd = match td.shape {
                        PieceShape::Cross => 0,
                        PieceShape::Straight => td.direction % 2,
                        _ => td.direction % 4,
                    };
                    if (td.shape, nd) != (s0, d0) {
                        snapshots.swap(0, idx);
                        break;
                    }
                }
            }
            if !same_layout(&snapshots) {
                moved_any = true;
            }
            for (&(tx, ty), mut td) in comp.iter().zip(snapshots) {
                td.x = tx;
                td.y = ty;
                tiles_map.insert((tx, ty), td);
            }
        }
    }
    moved_any
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
    if *steps > 1200 {
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
