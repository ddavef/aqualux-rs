// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::tile::Tile;
use crate::game::types::{GameColor, LevelDef, PieceType, Side};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathSegment {
    pub x: usize,
    pub y: usize,
    pub entry: Side,
    pub exit: Side,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TilePathHighlight {
    pub seq: u32,
    pub segs: Vec<(Side, Side)>,
    pub elapsed: f32,
    pub hold: bool,
}

impl TilePathHighlight {
    pub fn add_segment(&mut self, local_entry: Side, local_exit: Side, seq: u32, hold: bool) {
        if self.seq == seq {
            self.segs.push((local_entry, local_exit));
        } else {
            self.seq = seq;
            self.segs = vec![(local_entry, local_exit)];
        }
        self.elapsed = 0.0;
        self.hold = hold;
    }

    pub fn alpha(&self) -> f32 {
        if self.elapsed < 0.2 {
            (self.elapsed / 0.2).clamp(0.0, 1.0)
        } else if self.hold || self.elapsed < 0.8 {
            1.0
        } else {
            (1.0 - (self.elapsed - 0.8) / 0.7).clamp(0.0, 1.0)
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        self.elapsed += dt;
        !self.hold && self.elapsed >= 1.5
    }
}

pub fn find_matching_portal(
    tiles: &[Vec<Tile>],
    rows: usize,
    cols: usize,
    px: usize,
    py: usize,
    sec_color: Option<GameColor>,
) -> Option<(usize, usize)> {
    for (y, row) in tiles.iter().enumerate().take(rows) {
        for (x, t) in row.iter().enumerate().take(cols) {
            if (x, y) != (px, py)
                && t.piece_type == PieceType::Portal
                && t.secondary_color == sec_color
            {
                return Some((x, y));
            }
        }
    }
    None
}

pub fn trace_highlighted_segments(tiles: &[Vec<Tile>], level: &LevelDef) -> Vec<PathSegment> {
    let mut segments = Vec::new();
    let start = (0..level.rows).find_map(|y| {
        (0..level.cols)
            .find(|&x| tiles[y][x].piece_type == PieceType::Source)
            .map(|x| (x, y))
    });
    let Some((mut cur_x, mut cur_y)) = start else {
        return segments;
    };

    let mut visited: HashSet<(usize, usize, u8)> = HashSet::new();
    let mut entry = Side::Center;

    for _ in 0..256 {
        if !visited.insert((cur_x, cur_y, entry as u8)) {
            break;
        }
        let tile = &tiles[cur_y][cur_x];
        if entry != Side::Center && (entry.index() >= 4 || !tile.ports[entry.index()]) {
            break;
        }
        let Some(exit) = tile.get_exit_side(entry) else {
            break;
        };
        segments.push(PathSegment {
            x: cur_x,
            y: cur_y,
            entry,
            exit,
        });
        if tile.piece_type == PieceType::Drain {
            break;
        }
        if tile.piece_type == PieceType::Portal && exit == Side::Center {
            if let Some((mx, my)) = find_matching_portal(
                tiles,
                level.rows,
                level.cols,
                cur_x,
                cur_y,
                tile.secondary_color,
            ) {
                cur_x = mx;
                cur_y = my;
                entry = Side::Center;
                continue;
            }
            break;
        }
        let (dx, dy) = exit.delta();
        let nx = cur_x as i32 + dx;
        let ny = cur_y as i32 + dy;
        if nx < 0 || ny < 0 || nx >= level.cols as i32 || ny >= level.rows as i32 {
            break;
        }
        cur_x = nx as usize;
        cur_y = ny as usize;
        entry = exit.opposite();
    }
    segments
}
