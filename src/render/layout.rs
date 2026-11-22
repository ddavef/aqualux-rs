// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BoardLayout {
    pub origin_x: f32,
    pub origin_y: f32,
    pub tile_px: f32,
    pub cols: usize,
    pub rows: usize,
}

impl BoardLayout {
    pub fn compute(cols: usize, rows: usize, area: Rect) -> Self {
        let c = cols.max(1) as f32;
        let r = rows.max(1) as f32;
        let tile_px = ((area.w / c).min(area.h / r)).floor().clamp(24.0, 128.0);
        let board_w = tile_px * c;
        let board_h = tile_px * r;
        let origin_x = (area.x + (area.w - board_w) * 0.5).floor();
        let origin_y = (area.y + (area.h - board_h) * 0.5).floor();
        Self {
            origin_x,
            origin_y,
            tile_px,
            cols,
            rows,
        }
    }

    pub fn tile_center(&self, x: usize, y: usize) -> (f32, f32) {
        (
            self.origin_x + (x as f32 + 0.5) * self.tile_px,
            self.origin_y + (y as f32 + 0.5) * self.tile_px,
        )
    }

    pub fn hit_test(&self, px: f32, py: f32) -> Option<(usize, usize)> {
        if px < self.origin_x || py < self.origin_y {
            return None;
        }
        let gx = ((px - self.origin_x) / self.tile_px).floor() as i32;
        let gy = ((py - self.origin_y) / self.tile_px).floor() as i32;
        if gx >= 0 && gy >= 0 && (gx as usize) < self.cols && (gy as usize) < self.rows {
            Some((gx as usize, gy as usize))
        } else {
            None
        }
    }
}
