// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::level_io::{MAX_GRID_COLS, MAX_GRID_ROWS, MIN_GRID_DIM};
use crate::game::path::PathSegment;
use crate::game::solver_route::{RouteCtx, route_and_verify};
use crate::game::tile::{Tile, build_tile_grid};
use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, Side};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub pos: Option<(usize, usize)>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerMove {
    Rotate {
        x: usize,
        y: usize,
    },
    Swap {
        a: (usize, usize),
        b: (usize, usize),
    },
}

#[derive(Debug, Clone)]
pub struct SolveResult {
    pub path: Vec<PathSegment>,
    pub moves: Vec<PlayerMove>,
    pub pipes_filled: u32,
    pub suggested_par_moves: u32,
    pub suggested_par_pipes: u32,
    pub suggested_par_time_ms: u32,
    pub verified_by_sim: bool,
}

#[derive(Debug, Clone)]
pub enum SolveOutcome {
    Solvable(SolveResult),
    Unsolvable(String),
    BudgetExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MovableKind {
    pub shape: PieceShape,
    pub canonical_dir: u8,
    pub is_gate: bool,
    pub is_magic: bool,
    pub is_freeze: bool,
}

pub(crate) fn canonical_period(shape: PieceShape) -> u8 {
    match shape {
        PieceShape::Straight => 2,
        PieceShape::Cross | PieceShape::Block => 1,
        PieceShape::Elbow | PieceShape::Spigot | PieceShape::Drain => 4,
    }
}

fn kind_from_tile(t: &Tile) -> MovableKind {
    let period = canonical_period(t.shape);
    MovableKind {
        shape: t.shape,
        canonical_dir: if period <= 1 {
            0
        } else {
            (t.direction % 4) % period
        },
        is_gate: t.is_gate,
        is_magic: t.is_magic,
        is_freeze: t.is_freeze,
    }
}

fn ports_for(shape: PieceShape, dir: u8) -> [bool; 4] {
    let mut ports = shape.default_ports();
    ports.rotate_right((dir % 4) as usize);
    ports
}

fn exit_for_ports(ports: [bool; 4], entry: Side) -> Option<Side> {
    if entry == Side::Center {
        for (i, &open) in ports.iter().enumerate() {
            if open {
                return Side::from_u8(i as u8);
            }
        }
        return None;
    }
    let opp = (entry.index() + 2) % 4;
    if ports[opp] {
        return Side::from_u8(opp as u8);
    }
    for (i, &open) in ports.iter().enumerate() {
        if open && i != entry.index() {
            return Side::from_u8(i as u8);
        }
    }
    None
}

pub fn validate_level_structure(level: &LevelDef) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if !(MIN_GRID_DIM..=MAX_GRID_COLS).contains(&level.cols)
        || !(MIN_GRID_DIM..=MAX_GRID_ROWS).contains(&level.rows)
    {
        issues.push(ValidationIssue {
            pos: None,
            message: format!(
                "Grid size {}x{} out of bounds ({}-{} x {}-{})",
                level.cols, level.rows, MIN_GRID_DIM, MAX_GRID_COLS, MIN_GRID_DIM, MAX_GRID_ROWS
            ),
        });
        return issues;
    }

    let mut seen_coords = HashSet::new();
    let mut sources = Vec::new();
    let mut drains = Vec::new();
    let mut portal_colors: HashMap<Option<GameColor>, Vec<(usize, usize)>> = HashMap::new();

    for t in &level.tiles {
        if t.x >= level.cols || t.y >= level.rows {
            issues.push(ValidationIssue {
                pos: Some((t.x, t.y)),
                message: format!("Tile ({}, {}) is outside grid", t.x, t.y),
            });
            continue;
        }
        if !seen_coords.insert((t.x, t.y)) {
            issues.push(ValidationIssue {
                pos: Some((t.x, t.y)),
                message: format!("Duplicate tile at ({}, {})", t.x, t.y),
            });
        }
        match t.piece_type {
            PieceType::Source => sources.push((t.x, t.y, t.direction)),
            PieceType::Drain => drains.push((t.x, t.y, t.direction)),
            PieceType::Portal => {
                portal_colors
                    .entry(t.secondary_color)
                    .or_default()
                    .push((t.x, t.y));
                let ports = ports_for(t.shape, t.direction);
                if !spout_in_bounds(t.x, t.y, level.cols, level.rows, ports) {
                    issues.push(ValidationIssue {
                        pos: Some((t.x, t.y)),
                        message: format!("Portal at ({}, {}) points outside grid", t.x, t.y),
                    });
                }
            }
            _ => {}
        }
    }

    if sources.len() != 1 {
        if sources.is_empty() {
            issues.push(ValidationIssue {
                pos: None,
                message: "Expected exactly 1 Source, found 0".to_string(),
            });
        } else {
            for &(sx, sy, _) in &sources {
                issues.push(ValidationIssue {
                    pos: Some((sx, sy)),
                    message: format!("Expected exactly 1 Source, found {}", sources.len()),
                });
            }
        }
    }
    if drains.len() != 1 {
        if drains.is_empty() {
            issues.push(ValidationIssue {
                pos: None,
                message: "Expected exactly 1 Drain, found 0".to_string(),
            });
        } else {
            for &(dx, dy, _) in &drains {
                issues.push(ValidationIssue {
                    pos: Some((dx, dy)),
                    message: format!("Expected exactly 1 Drain, found {}", drains.len()),
                });
            }
        }
    }
    for (sx, sy, dir) in sources {
        if !spout_in_bounds(
            sx,
            sy,
            level.cols,
            level.rows,
            ports_for(PieceShape::Spigot, dir),
        ) {
            issues.push(ValidationIssue {
                pos: Some((sx, sy)),
                message: format!("Source at ({}, {}) points outside grid", sx, sy),
            });
        }
    }
    for (dx, dy, dir) in drains {
        if !spout_in_bounds(
            dx,
            dy,
            level.cols,
            level.rows,
            ports_for(PieceShape::Drain, dir),
        ) {
            issues.push(ValidationIssue {
                pos: Some((dx, dy)),
                message: format!("Drain at ({}, {}) points outside grid", dx, dy),
            });
        }
    }
    for (color, list) in portal_colors {
        if list.len() != 2 {
            let cname = color
                .map(|c| format!("{:?}", c))
                .unwrap_or_else(|| "None".into());
            for &(px, py) in &list {
                issues.push(ValidationIssue {
                    pos: Some((px, py)),
                    message: format!(
                        "Portal color {} needs 2 portals (found {})",
                        cname,
                        list.len()
                    ),
                });
            }
        }
    }
    issues
}

fn spout_in_bounds(x: usize, y: usize, cols: usize, rows: usize, ports: [bool; 4]) -> bool {
    let Some(side) = exit_for_ports(ports, Side::Center) else {
        return false;
    };
    let (dx, dy) = side.delta();
    let nx = x as i32 + dx;
    let ny = y as i32 + dy;
    nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows
}

struct SolverCtx<'a> {
    cols: usize,
    rows: usize,
    grid: Vec<Vec<Tile>>,
    shapes: Vec<PieceShape>,
    initial_rot_dirs: Vec<u8>,
    comp_id: Vec<Option<usize>>,
    comp_cells: Vec<Vec<usize>>,
    comp_inv: Vec<Vec<(MovableKind, usize)>>,
    initial_move_kinds: Vec<Option<MovableKind>>,
    portals: HashMap<usize, usize>,
    drains: Vec<(usize, usize)>,
    used_sides: Vec<u8>,
    filled: Vec<bool>,
    locked_rot: Vec<Option<u8>>,
    locked_move: Vec<Option<MovableKind>>,
    portal_entered: Vec<bool>,
    path: Vec<PathSegment>,
    start: Instant,
    budget: Duration,
    nodes: u64,
    timed_out: bool,
    solution: Option<SolveResult>,
    level: &'a LevelDef,
}

pub fn solve_level(level: &LevelDef, budget: Duration) -> SolveOutcome {
    let issues = validate_level_structure(level);
    if let Some(first) = issues.first() {
        return SolveOutcome::Unsolvable(first.message.clone());
    }

    let grid = build_tile_grid(level);
    let cols = level.cols;
    let rows = level.rows;
    let n = cols * rows;

    let mut shapes = vec![PieceShape::Block; n];
    let mut initial_rot_dirs = vec![0u8; n];
    let mut initial_move_kinds = vec![None; n];
    let mut comp_id = vec![None; n];
    let mut comp_cells = Vec::new();
    let mut comp_inv = Vec::new();
    let mut drains = Vec::new();
    let mut source_pos = None;
    let mut portal_groups: HashMap<Option<GameColor>, Vec<usize>> = HashMap::new();

    for (y, row) in grid.iter().enumerate() {
        for (x, t) in row.iter().enumerate() {
            let idx = y * cols + x;
            shapes[idx] = t.shape;
            initial_rot_dirs[idx] = t.direction % 4;
            if t.piece_type == PieceType::Source {
                source_pos = Some((x, y));
            } else if t.piece_type == PieceType::Drain {
                drains.push((x, y));
            } else if t.piece_type == PieceType::Portal {
                portal_groups
                    .entry(t.secondary_color)
                    .or_default()
                    .push(idx);
            }
        }
    }

    let mut portals = HashMap::new();
    for list in portal_groups.values() {
        if list.len() == 2 {
            portals.insert(list[0], list[1]);
            portals.insert(list[1], list[0]);
        }
    }

    for start_idx in 0..n {
        let sx = start_idx % cols;
        let sy = start_idx / cols;
        let st = &grid[sy][sx];
        let is_movable_pipe = matches!(st.piece_type, PieceType::Pipe | PieceType::FrozenPipe)
            && st.is_moveable
            && !st.can_rotate;
        if !is_movable_pipe || comp_id[start_idx].is_some() {
            continue;
        }
        let cid = comp_cells.len();
        let mut cells = Vec::new();
        let mut counts: HashMap<MovableKind, usize> = HashMap::new();
        let mut q = VecDeque::new();
        comp_id[start_idx] = Some(cid);
        q.push_back(start_idx);

        while let Some(u) = q.pop_front() {
            cells.push(u);
            let t = &grid[u / cols][u % cols];
            let mk = kind_from_tile(t);
            initial_move_kinds[u] = Some(mk);
            *counts.entry(mk).or_insert(0) += 1;

            let (ux, uy) = (u % cols, u / cols);
            for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                let nx = ux as i32 + dx;
                let ny = uy as i32 + dy;
                if nx >= 0 && ny >= 0 && (nx as usize) < cols && (ny as usize) < rows {
                    let v = ny as usize * cols + nx as usize;
                    let nt = &grid[ny as usize][nx as usize];
                    if comp_id[v].is_none()
                        && matches!(nt.piece_type, PieceType::Pipe | PieceType::FrozenPipe)
                        && nt.is_moveable
                        && !nt.can_rotate
                    {
                        comp_id[v] = Some(cid);
                        q.push_back(v);
                    }
                }
            }
        }
        let inv: Vec<(MovableKind, usize)> = counts.into_iter().collect();
        comp_cells.push(cells);
        comp_inv.push(inv);
    }

    let Some((sx, sy)) = source_pos else {
        return SolveOutcome::Unsolvable("Level has no Source".to_string());
    };

    let mut ctx = SolverCtx {
        cols,
        rows,
        grid,
        shapes,
        initial_rot_dirs,
        comp_id,
        comp_cells,
        comp_inv,
        initial_move_kinds,
        portals,
        drains,
        used_sides: vec![0; n],
        filled: vec![false; n],
        locked_rot: vec![None; n],
        locked_move: vec![None; n],
        portal_entered: vec![false; n],
        path: Vec::new(),
        start: Instant::now(),
        budget,
        nodes: 0,
        timed_out: false,
        solution: None,
        level,
    };

    ctx.dfs(sx, sy, Side::Center, 0);

    if let Some(res) = ctx.solution {
        SolveOutcome::Solvable(res)
    } else if ctx.timed_out {
        SolveOutcome::BudgetExceeded
    } else {
        SolveOutcome::Unsolvable("No valid pipe path from Source to Drain".to_string())
    }
}

impl<'a> SolverCtx<'a> {
    fn dfs(&mut self, x: usize, y: usize, entry: Side, gates_hit: u8) -> bool {
        if self.timed_out {
            return false;
        }
        self.nodes += 1;
        if self.nodes.is_multiple_of(256) && self.start.elapsed() >= self.budget {
            self.timed_out = true;
            return false;
        }

        let idx = y * self.cols + x;
        let ptype = self.grid[y][x].piece_type;
        if ptype == PieceType::Blocker {
            return false;
        }

        // Source (entry Center) & Drain
        if ptype == PieceType::Source {
            if entry != Side::Center || self.filled[idx] {
                return false;
            }
            let ports = ports_for(self.grid[y][x].shape, self.grid[y][x].direction);
            let Some(exit) = exit_for_ports(ports, Side::Center) else {
                return false;
            };
            self.filled[idx] = true;
            self.used_sides[idx] |= 1 << exit.index();
            self.path.push(PathSegment { x, y, entry, exit });
            let found = self.step_out(x, y, exit, gates_hit);
            self.path.pop();
            self.used_sides[idx] &= !(1 << exit.index());
            self.filled[idx] = false;
            return found;
        }

        if ptype == PieceType::Drain {
            if entry == Side::Center {
                return false;
            }
            let ports = ports_for(self.grid[y][x].shape, self.grid[y][x].direction);
            if !ports[entry.index()] || (self.used_sides[idx] & (1 << entry.index())) != 0 {
                return false;
            }
            self.path.push(PathSegment {
                x,
                y,
                entry,
                exit: Side::Center,
            });
            self.solution = route_and_verify(RouteCtx {
                level: self.level,
                path: self.path.clone(),
                locked_rot: &self.locked_rot,
                locked_move: &self.locked_move,
                comp_id: &self.comp_id,
                comp_cells: &self.comp_cells,
                initial_move_kinds: &self.initial_move_kinds,
                initial_rot_dirs: &self.initial_rot_dirs,
                shapes: &self.shapes,
            });
            if self.solution.is_some() {
                return true;
            }
            self.path.pop();
            return false;
        }

        if ptype == PieceType::Portal {
            if entry == Side::Center {
                if self.filled[idx] {
                    return false;
                }
                let ports = ports_for(self.grid[y][x].shape, self.grid[y][x].direction);
                let Some(exit) = exit_for_ports(ports, Side::Center) else {
                    return false;
                };
                self.filled[idx] = true;
                self.used_sides[idx] |= 1 << exit.index();
                self.path.push(PathSegment { x, y, entry, exit });
                let found = self.step_out(x, y, exit, gates_hit);
                self.path.pop();
                self.used_sides[idx] &= !(1 << exit.index());
                self.filled[idx] = false;
                return found;
            } else {
                if self.portal_entered[idx] {
                    return false;
                }
                let ports = ports_for(self.grid[y][x].shape, self.grid[y][x].direction);
                if !ports[entry.index()] || (self.used_sides[idx] & (1 << entry.index())) != 0 {
                    return false;
                }
                let Some(&mate) = self.portals.get(&idx) else {
                    return false;
                };
                if self.filled[mate] {
                    return false;
                }
                let prev_filled = self.filled[idx];
                self.filled[idx] = true;
                self.portal_entered[idx] = true;
                self.used_sides[idx] |= 1 << entry.index();
                let next_gates = if self.grid[y][x].is_gate {
                    (gates_hit + 1) % 4
                } else {
                    gates_hit
                };
                self.path.push(PathSegment {
                    x,
                    y,
                    entry,
                    exit: Side::Center,
                });
                let found = self.dfs(mate % self.cols, mate / self.cols, Side::Center, next_gates);
                self.path.pop();
                self.used_sides[idx] &= !(1 << entry.index());
                self.portal_entered[idx] = false;
                self.filled[idx] = prev_filled;
                return found;
            }
        }

        // Standard Pipe / FrozenPipe
        if entry == Side::Center || (self.used_sides[idx] & (1 << entry.index())) != 0 {
            return false;
        }
        let t_shape = self.grid[y][x].shape;
        let t_can_rotate = self.grid[y][x].can_rotate;
        let t_is_moveable = self.grid[y][x].is_moveable;
        let t_is_gate = self.grid[y][x].is_gate;
        let t_is_magic = self.grid[y][x].is_magic;
        let t_dir = self.grid[y][x].direction % 4;

        // Already filled once (only Cross can be re-entered on perpendicular axis).
        if self.filled[idx] {
            let cur_eff_dir = if self.comp_id[idx].is_some() {
                let Some(mk) = self.locked_move[idx] else {
                    return false;
                };
                if mk.shape != PieceShape::Cross {
                    return false;
                }
                mk.canonical_dir
            } else if t_shape != PieceShape::Cross {
                return false;
            } else {
                0
            };
            let ports = ports_for(PieceShape::Cross, cur_eff_dir);
            let Some(exit) = exit_for_ports(ports, entry) else {
                return false;
            };
            if (self.used_sides[idx] & (1 << exit.index())) != 0 {
                return false;
            }
            let gates_after = if self.comp_id[idx]
                .is_some_and(|_| self.locked_move[idx].is_some_and(|m| m.is_gate))
                || (self.comp_id[idx].is_none() && t_is_gate)
            {
                (gates_hit + 1) % 4
            } else {
                gates_hit
            };
            self.used_sides[idx] |= (1 << entry.index()) | (1 << exit.index());
            self.path.push(PathSegment { x, y, entry, exit });
            let found = self.step_out(x, y, exit, gates_after);
            self.path.pop();
            self.used_sides[idx] &= !((1 << entry.index()) | (1 << exit.index()));
            return found;
        }

        // Unfilled tile: Branch by fixed / rotatable / movable
        if t_can_rotate {
            let period = canonical_period(t_shape);
            let mut dirs: Vec<u8> = (0..period).collect();
            let magic_shift = if t_is_magic { gates_hit } else { 0 };
            let target_no_rot = (t_dir + period - (magic_shift % period)) % period;
            dirs.sort_by_key(|&d| {
                let eff = (d + magic_shift) % 4;
                let ports = ports_for(t_shape, eff);
                if !ports[entry.index()] {
                    return (2usize, 999usize);
                }
                let zero_cost = usize::from(d != target_no_rot);
                let dist = exit_for_ports(ports, entry)
                    .map(|ex| self.step_heuristic(x, y, ex))
                    .unwrap_or(999);
                (zero_cost, dist)
            });

            for init_d in dirs {
                let eff_dir = (init_d + magic_shift) % 4;
                let ports = ports_for(t_shape, eff_dir);
                if !ports[entry.index()] {
                    continue;
                }
                let Some(exit) = exit_for_ports(ports, entry) else {
                    continue;
                };
                if (self.used_sides[idx] & (1 << exit.index())) != 0 {
                    continue;
                }
                let gates_after = if t_is_gate {
                    (gates_hit + 1) % 4
                } else {
                    gates_hit
                };
                self.filled[idx] = true;
                self.locked_rot[idx] = Some(init_d);
                self.used_sides[idx] |= (1 << entry.index()) | (1 << exit.index());
                self.path.push(PathSegment { x, y, entry, exit });
                let found = self.step_out(x, y, exit, gates_after);
                self.path.pop();
                self.used_sides[idx] &= !((1 << entry.index()) | (1 << exit.index()));
                self.locked_rot[idx] = None;
                self.filled[idx] = false;
                if found {
                    return true;
                }
            }
            return false;
        }

        if let Some(cid) = self.comp_id[idx] {
            let orig_mk = self.initial_move_kinds[idx];
            let mut candidates: Vec<(usize, MovableKind, Side)> = Vec::new();
            for (slot, &(mk, count)) in self.comp_inv[cid].iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let eff_dir = (mk.canonical_dir + if mk.is_magic { gates_hit } else { 0 }) % 4;
                let ports = ports_for(mk.shape, eff_dir);
                if !ports[entry.index()] {
                    continue;
                }
                if let Some(exit) = exit_for_ports(ports, entry) {
                    candidates.push((slot, mk, exit));
                }
            }
            candidates.sort_by_key(|&(_, mk, exit)| {
                let prefer_orig = usize::from(orig_mk != Some(mk));
                let h = self.step_heuristic(x, y, exit);
                (h, prefer_orig)
            });

            for (slot, mk, exit) in candidates {
                let gates_after = if mk.is_gate {
                    (gates_hit + 1) % 4
                } else {
                    gates_hit
                };
                self.comp_inv[cid][slot].1 -= 1;
                self.filled[idx] = true;
                self.locked_move[idx] = Some(mk);
                self.used_sides[idx] |= (1 << entry.index()) | (1 << exit.index());
                self.path.push(PathSegment { x, y, entry, exit });
                let found = self.step_out(x, y, exit, gates_after);
                self.path.pop();
                self.used_sides[idx] &= !((1 << entry.index()) | (1 << exit.index()));
                self.locked_move[idx] = None;
                self.filled[idx] = false;
                self.comp_inv[cid][slot].1 += 1;
                if found {
                    return true;
                }
            }
            return false;
        }

        // Fixed Pipe / FrozenPipe (not movable, not rotatable)
        let _ = t_is_moveable;
        let eff_dir = (t_dir + if t_is_magic { gates_hit } else { 0 }) % 4;
        let ports = ports_for(t_shape, eff_dir);
        if !ports[entry.index()] {
            return false;
        }
        let Some(exit) = exit_for_ports(ports, entry) else {
            return false;
        };
        let gates_after = if t_is_gate {
            (gates_hit + 1) % 4
        } else {
            gates_hit
        };
        self.filled[idx] = true;
        self.used_sides[idx] |= (1 << entry.index()) | (1 << exit.index());
        self.path.push(PathSegment { x, y, entry, exit });
        let found = self.step_out(x, y, exit, gates_after);
        self.path.pop();
        self.used_sides[idx] &= !((1 << entry.index()) | (1 << exit.index()));
        self.filled[idx] = false;
        found
    }

    fn step_out(&mut self, x: usize, y: usize, exit: Side, gates_hit: u8) -> bool {
        let (dx, dy) = exit.delta();
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 || (nx as usize) >= self.cols || (ny as usize) >= self.rows {
            return false;
        }
        let (ux, uy) = (nx as usize, ny as usize);
        if !self.can_reach_drain(ux, uy, exit.opposite()) {
            return false;
        }
        self.dfs(ux, uy, exit.opposite(), gates_hit)
    }

    fn step_heuristic(&self, x: usize, y: usize, exit: Side) -> usize {
        let (dx, dy) = exit.delta();
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx < 0 || ny < 0 || (nx as usize) >= self.cols || (ny as usize) >= self.rows {
            return 999;
        }
        let (ux, uy) = (nx as usize, ny as usize);
        self.drains
            .iter()
            .map(|&(dx, dy)| ux.abs_diff(dx) + uy.abs_diff(dy))
            .min()
            .unwrap_or(999)
    }

    fn can_reach_drain(&self, sx: usize, sy: usize, entry: Side) -> bool {
        let n = self.cols * self.rows;
        let mut seen = vec![false; n];
        let mut q = VecDeque::new();
        let start_idx = sy * self.cols + sx;
        if (self.used_sides[start_idx] & (1 << entry.index())) != 0 {
            return false;
        }
        if !self.cell_accepts_any_traversal(sx, sy) {
            return false;
        }
        seen[start_idx] = true;
        q.push_back(start_idx);

        while let Some(u) = q.pop_front() {
            let ux = u % self.cols;
            let uy = u / self.cols;
            let ptype = self.grid[uy][ux].piece_type;
            if ptype == PieceType::Drain {
                return true;
            }
            if ptype == PieceType::Portal
                && let Some(&mate) = self.portals.get(&u)
                && !self.filled[mate]
                && !seen[mate]
            {
                seen[mate] = true;
                q.push_back(mate);
            }
            for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                let nx = ux as i32 + dx;
                let ny = uy as i32 + dy;
                if nx < 0 || ny < 0 || (nx as usize) >= self.cols || (ny as usize) >= self.rows {
                    continue;
                }
                let v = ny as usize * self.cols + nx as usize;
                if !seen[v] && self.cell_accepts_any_traversal(nx as usize, ny as usize) {
                    seen[v] = true;
                    q.push_back(v);
                }
            }
        }
        false
    }

    fn cell_accepts_any_traversal(&self, x: usize, y: usize) -> bool {
        let idx = y * self.cols + x;
        let t = &self.grid[y][x];
        match t.piece_type {
            PieceType::Blocker | PieceType::Source => false,
            PieceType::Drain => true,
            PieceType::Portal => !self.portal_entered[idx],
            PieceType::Pipe | PieceType::FrozenPipe => {
                if !self.filled[idx] {
                    true
                } else if let Some(mk) = self.locked_move[idx] {
                    mk.shape == PieceShape::Cross && self.used_sides[idx] != 0b1111
                } else {
                    t.shape == PieceShape::Cross && self.used_sides[idx] != 0b1111
                }
            }
        }
    }
}
