// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::types::{GameColor, LevelDef, PieceShape, PieceType, Side, TileDef};

#[derive(Debug, Clone)]
pub struct Tile {
    pub x: usize,
    pub y: usize,
    pub piece_type: PieceType,
    pub shape: PieceShape,
    pub direction: u8,
    pub ports: [bool; 4],
    pub is_moveable: bool,
    pub can_rotate: bool,
    pub is_gate: bool,
    pub is_freeze: bool,
    pub is_magic: bool,
    pub pipe_color: GameColor,
    pub liquid_color: GameColor,
    pub secondary_color: Option<GameColor>,
    pub fills: Vec<crate::game::FillPass>,
    pub has_filled: bool,
    pub used_sides: [bool; 4],
    pub visual_rot_deg: f32,
    pub target_rot_deg: f32,
    pub rot_from_deg: f32,
    pub rot_elapsed: f32,
    pub rot_dur: f32,
    pub is_rotating: bool,
    pub swap_offset: (f32, f32),
    pub swap_from: (f32, f32),
    pub swap_elapsed: f32,
    pub swap_dur: f32,
    pub is_swapping: bool,
    pub timer: Option<crate::game::FlowTimerState>,
    /// Elapsed slime animation seconds for Drain / Portal (15fps, 26 frames).
    pub slime_anim: Option<f32>,
    pub slime_playing: bool,
    pub slime_done: bool,
    pub path_hl: Option<crate::game::path::TilePathHighlight>,
}

impl Tile {
    pub fn from_def(def: &TileDef, level: &LevelDef) -> Self {
        let pipe_color = if def.can_rotate {
            level.rotater_color
        } else if def.is_moveable {
            level.mover_color
        } else if def.is_gate {
            level.gate_color
        } else if def.is_freeze || def.piece_type == PieceType::FrozenPipe {
            level.freeze_color
        } else if def.is_magic {
            level.magic_color
        } else {
            level.stationary_color
        };

        let mut tile = Self {
            x: def.x,
            y: def.y,
            piece_type: def.piece_type,
            shape: def.shape,
            direction: 0,
            ports: def.shape.default_ports(),
            is_moveable: def.is_moveable,
            can_rotate: def.can_rotate,
            is_gate: def.is_gate,
            is_freeze: def.is_freeze || def.piece_type == PieceType::FrozenPipe,
            is_magic: def.is_magic,
            pipe_color,
            liquid_color: level.liquid_color,
            secondary_color: def.secondary_color,
            fills: Vec::new(),
            has_filled: false,
            used_sides: [false; 4],
            visual_rot_deg: 0.0,
            target_rot_deg: 0.0,
            rot_from_deg: 0.0,
            rot_elapsed: 0.0,
            rot_dur: 0.0,
            is_rotating: false,
            swap_offset: (0.0, 0.0),
            swap_from: (0.0, 0.0),
            swap_elapsed: 0.0,
            swap_dur: 0.0,
            is_swapping: false,
            timer: None,
            slime_anim: None,
            slime_playing: false,
            slime_done: false,
            path_hl: None,
        };
        let steps = def.direction % 4;
        if steps > 0 {
            tile.rotate_cw_instant(steps);
        }
        tile
    }

    pub fn rotate_cw_instant(&mut self, steps: u8) {
        let s = steps % 4;
        if s == 0 {
            return;
        }
        self.direction = (self.direction + s) % 4;
        self.ports.rotate_right(s as usize);
        self.target_rot_deg += (s as f32) * 90.0;
        self.visual_rot_deg = self.target_rot_deg;
        self.is_rotating = false;
    }

    pub fn rotate_cw_timed(&mut self, steps: u8, duration: f32) {
        let s = steps % 4;
        if s == 0 {
            return;
        }
        self.direction = (self.direction + s) % 4;
        self.ports.rotate_right(s as usize);
        self.rot_from_deg = self.visual_rot_deg;
        self.target_rot_deg += (s as f32) * 90.0;
        self.rot_elapsed = 0.0;
        self.rot_dur = duration.max(0.001);
        self.is_rotating = true;
    }

    pub fn is_acting(&self) -> bool {
        self.is_rotating || self.is_swapping
    }

    pub fn can_fill_from_side(&self, entry: Side) -> bool {
        if self.piece_type == PieceType::Blocker {
            return false;
        }
        if entry == Side::Center {
            return true;
        }
        let idx = entry.index();
        idx < 4 && self.ports[idx] && !self.used_sides[idx]
    }

    pub fn get_exit_side(&self, entry: Side) -> Option<Side> {
        if self.piece_type == PieceType::Drain
            || (self.piece_type == PieceType::Portal && entry != Side::Center)
        {
            return Some(Side::Center);
        }
        if entry == Side::Center {
            for (i, &open) in self.ports.iter().enumerate() {
                if open {
                    return Side::from_u8(i as u8);
                }
            }
            return None;
        }
        let opp = (entry.index() + 2) % 4;
        if self.ports[opp] {
            return Side::from_u8(opp as u8);
        }
        for (i, &open) in self.ports.iter().enumerate() {
            if open && i != entry.index() {
                return Side::from_u8(i as u8);
            }
        }
        None
    }

    pub fn start_filling(&mut self, entry: Side, fast_nozzle: bool) -> Option<Side> {
        if !self.can_fill_from_side(entry) {
            return None;
        }
        let exit = self.get_exit_side(entry)?;
        self.has_filled = true;
        self.is_moveable = false;
        self.can_rotate = false;
        if entry.index() < 4 {
            self.used_sides[entry.index()] = true;
        }
        if exit.index() < 4 {
            self.used_sides[exit.index()] = true;
        }
        let initial_progress = if entry == Side::Center { 0.5 } else { 0.0 };
        self.fills.push(crate::game::FillPass {
            entry,
            exit,
            progress: initial_progress,
            elapsed: if fast_nozzle { 1.0 } else { 0.0 },
            completed: false,
        });
        Some(exit)
    }

    pub fn is_equal_to(&self, other: &Tile) -> bool {
        if self.shape != other.shape
            || self.piece_type != other.piece_type
            || self.can_rotate != other.can_rotate
            || self.is_moveable != other.is_moveable
        {
            return false;
        }
        if matches!(
            self.piece_type,
            PieceType::Drain | PieceType::Source | PieceType::Blocker
        ) || matches!(self.shape, PieceShape::Cross | PieceShape::Block)
        {
            return true;
        }
        let d1 = self.direction % 4;
        let d2 = other.direction % 4;
        if self.shape == PieceShape::Straight {
            (d1 % 2) == (d2 % 2)
        } else {
            d1 == d2
        }
    }

    pub fn is_locked_or_active(&self) -> bool {
        self.has_filled || !self.fills.is_empty() || self.timer.is_some()
    }
}

pub fn build_tile_grid(level: &LevelDef) -> Vec<Vec<Tile>> {
    let rows = level.rows.max(1);
    let cols = level.cols.max(1);
    let dummy = TileDef::dummy_blocker();
    let mut grid = vec![vec![Tile::from_def(&dummy, level); cols]; rows];
    for tdef in &level.tiles {
        if tdef.y < rows && tdef.x < cols {
            grid[tdef.y][tdef.x] = Tile::from_def(tdef, level);
        }
    }
    grid
}

pub fn is_adjacent(a: (usize, usize), b: (usize, usize)) -> bool {
    (a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs() == 1
}

pub fn swap_grid_tiles_raw(grid: &mut [Vec<Tile>], a: (usize, usize), b: (usize, usize)) {
    if a == b {
        return;
    }
    if a.1 == b.1 {
        grid[a.1].swap(a.0, b.0);
    } else {
        let (r_lo, r_hi, x_lo, x_hi) = if a.1 < b.1 {
            (a.1, b.1, a.0, b.0)
        } else {
            (b.1, a.1, b.0, a.0)
        };
        let (top, bot) = grid.split_at_mut(r_hi);
        std::mem::swap(&mut top[r_lo][x_lo], &mut bot[0][x_hi]);
    }
    grid[a.1][a.0].x = a.0;
    grid[a.1][a.0].y = a.1;
    grid[b.1][b.0].x = b.0;
    grid[b.1][b.0].y = b.1;
}

pub fn swap_grid_tiles_timed(
    grid: &mut [Vec<Tile>],
    a: (usize, usize),
    b: (usize, usize),
    duration: f32,
) {
    if a == b {
        return;
    }
    let dx = b.0 as f32 - a.0 as f32;
    let dy = b.1 as f32 - a.1 as f32;
    swap_grid_tiles_raw(grid, a, b);
    grid[a.1][a.0].swap_from = (dx, dy);
    grid[a.1][a.0].swap_offset = (dx, dy);
    grid[a.1][a.0].swap_elapsed = 0.0;
    grid[a.1][a.0].swap_dur = duration.max(0.001);
    grid[a.1][a.0].is_swapping = true;

    grid[b.1][b.0].swap_from = (-dx, -dy);
    grid[b.1][b.0].swap_offset = (-dx, -dy);
    grid[b.1][b.0].swap_elapsed = 0.0;
    grid[b.1][b.0].swap_dur = duration.max(0.001);
    grid[b.1][b.0].is_swapping = true;
}

/// Advances timed swap and rotation tweens; returns `true` when player action completes.
pub fn animate_grid_tiles(grid: &mut [Vec<Tile>], dt: f32, tracked: &[(usize, usize)]) -> bool {
    let mut completed_tracked = false;
    for row in grid {
        for tile in row {
            let was_tracked = tracked.contains(&(tile.x, tile.y));
            if tile.is_rotating {
                tile.rot_elapsed += dt;
                let t = (tile.rot_elapsed / tile.rot_dur).clamp(0.0, 1.0);
                // Smooth quadratic ease-out
                let eased = t * (2.0 - t);
                tile.visual_rot_deg =
                    tile.rot_from_deg + (tile.target_rot_deg - tile.rot_from_deg) * eased;
                if t >= 1.0 {
                    tile.visual_rot_deg = tile.target_rot_deg;
                    tile.is_rotating = false;
                    if was_tracked {
                        completed_tracked = true;
                    }
                }
            }
            if tile.is_swapping {
                tile.swap_elapsed += dt;
                let t = (tile.swap_elapsed / tile.swap_dur).clamp(0.0, 1.0);
                let eased = t * (2.0 - t);
                tile.swap_offset.0 = tile.swap_from.0 * (1.0 - eased);
                tile.swap_offset.1 = tile.swap_from.1 * (1.0 - eased);
                if t >= 1.0 {
                    tile.swap_offset = (0.0, 0.0);
                    tile.is_swapping = false;
                    if was_tracked {
                        completed_tracked = true;
                    }
                }
            }
        }
    }
    completed_tracked
}

pub fn collect_swap_eligible(
    grid: &[Vec<Tile>],
    cols: usize,
    rows: usize,
    pos: (usize, usize),
    check_locked: bool,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let (x, y) = pos;
    if y >= rows || x >= cols || !grid[y][x].is_moveable {
        return out;
    }
    for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && ny >= 0 && nx < cols as i32 && ny < rows as i32 {
            let (ux, uy) = (nx as usize, ny as usize);
            let t = &grid[uy][ux];
            if t.is_moveable && (!check_locked || !t.is_locked_or_active()) {
                out.push((ux, uy));
            }
        }
    }
    out
}
