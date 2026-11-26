// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::{GameColor, Side};
use crate::render::canvas::{Canvas, LOGICAL_H, LOGICAL_W};
use crate::render::gpu_types::LeakUniform;

#[derive(Debug, Clone)]
pub(crate) struct Particle30 {
    pub x: f32,
    pub y: f32,
    pub prev_x: f32,
    pub prev_y: f32,
    x_mov: f32,
    y_mov: f32,
    x_force: f32,
    y_force: f32,
    x_decay: f32,
    y_decay: f32,
    gravity: f32,
    gravity_decay: f32,
}

impl Particle30 {
    pub(crate) fn new(
        x_force: f32,
        y_force: f32,
        x_decay: f32,
        y_decay: f32,
        gravity: f32,
        gravity_decay: f32,
    ) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            prev_x: 0.0,
            prev_y: 0.0,
            x_mov: 0.0,
            y_mov: 0.0,
            x_force,
            y_force,
            x_decay,
            y_decay,
            gravity,
            gravity_decay,
        }
    }

    /// Advances a leak stream droplet by one 30Hz simulation tick.
    pub(crate) fn step(&mut self) {
        self.prev_x = self.x;
        self.prev_y = self.y;
        self.x_mov = self.x_mov.clamp(10.0, 100.0);
        self.y_mov = self.y_mov.min(90.0);
        self.x_mov = self.x_force * self.x_decay;
        self.gravity *= self.gravity_decay;
        self.y_mov += self.gravity;
        self.x = (self.x_mov * self.y_decay + self.x).floor();
        self.y = (self.y_mov + self.y_force + self.y).floor();
    }
}

/// Continuous beveled 38px round liquid leak stream running at fixed 30Hz ticks.
#[derive(Debug, Clone)]
pub struct LeakEmitter {
    pub tile_pos: (usize, usize),
    pub exit_side: Side,
    pub origin_y: f32,
    pub rgb: (u8, u8, u8),
    x_force: f32,
    y_force: f32,
    tick: u32,
    accum: f32,
    particles: Vec<Particle30>,
    pub paused: bool,
}

impl LeakEmitter {
    pub fn new(tile_pos: (usize, usize), origin_y: f32, exit_side: Side, color: GameColor) -> Self {
        let (x_force, y_force) = match exit_side {
            Side::Left => (-130.0, -10.0),
            Side::Right => (130.0, -10.0),
            Side::Up => (15.0, -20.0),
            Side::Down | Side::Center => (0.0, 1.0),
        };
        let rgb = if exit_side == Side::Center {
            (0, 255, 0)
        } else {
            color.rgb()
        };
        Self {
            tile_pos,
            exit_side,
            origin_y,
            rgb,
            x_force,
            y_force,
            tick: 0,
            accum: 1.0 / 30.0,
            particles: Vec::new(),
            paused: false,
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.paused {
            return;
        }
        self.accum += dt;
        let step_dt = 1.0 / 30.0;
        while self.accum >= step_dt {
            self.accum -= step_dt;
            self.step_30hz();
        }
    }

    fn step_30hz(&mut self) {
        self.tick += 1;
        if self.tick.is_multiple_of(2) {
            self.particles.push(Particle30::new(
                self.x_force,
                self.y_force,
                0.2,
                0.9,
                1.8,
                1.0,
            ));
            self.x_force *= 0.98;
            self.y_force *= 0.9;
            if self.particles.len() > 140 {
                self.particles.remove(0);
            }
        }
        let bottom_edge = (LOGICAL_H as f32 - self.origin_y) + 25.0;
        for p in &mut self.particles {
            if p.y <= bottom_edge {
                p.step();
            }
        }
    }

    #[cfg(test)]
    pub fn tick_count(&self) -> u32 {
        self.tick
    }

    #[cfg(test)]
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    #[cfg(test)]
    pub fn draw_at(&self, canvas: &mut Canvas, origin_x: f32, origin_y: f32) {
        self.draw_at_fx(canvas, origin_x, origin_y, 1.0, 0.0);
    }

    pub fn draw_at_fx(
        &self,
        canvas: &mut Canvas,
        origin_x: f32,
        origin_y: f32,
        alpha: f32,
        blur: f32,
    ) {
        let eff_alpha = alpha.clamp(0.0, 1.0);
        if self.particles.is_empty() || eff_alpha <= 0.004 {
            return;
        }
        let bottom_edge = (LOGICAL_H as f32 - origin_y) + 25.0;
        let sub_t = (self.accum / (1.0 / 30.0)).clamp(0.0, 1.0);
        let mut pts = Vec::with_capacity(self.particles.len() + 1);
        pts.push((origin_x, origin_y));
        let mut prev_idx = self.particles.len() - 1;
        for idx in (0..self.particles.len()).rev() {
            if self.particles[prev_idx].y <= bottom_edge {
                let p = &self.particles[idx];
                let px = p.prev_x + (p.x - p.prev_x) * sub_t;
                let py = p.prev_y + (p.y - p.prev_y) * sub_t;
                pts.push((origin_x + px, origin_y + py));
                prev_idx = idx;
            }
        }
        if pts.len() < 2 {
            return;
        }

        let blur_px = blur.max(0.0);
        let pad = 26.0 + blur_px * 0.65;
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for &(x, y) in &pts {
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
        let x0 = ((min_x - pad).floor()).max(0.0);
        let y0 = ((min_y - pad).floor()).max(0.0);
        let x1 = ((max_x + pad).ceil()).min(LOGICAL_W as f32);
        let y1 = ((max_y + pad).ceil()).min(LOGICAL_H as f32);
        let bw = (x1 - x0).max(0.0);
        let bh = (y1 - y0).max(0.0);
        if bw <= 0.0 || bh <= 0.0 {
            return;
        }
        let n = pts.len().min(144);
        let mut packed = [[0.0f32; 4]; 72];
        for (i, &(x, y)) in pts.iter().take(n).enumerate() {
            let slot = &mut packed[i >> 1];
            if (i & 1) == 0 {
                slot[0] = x;
                slot[1] = y;
            } else {
                slot[2] = x;
                slot[3] = y;
            }
        }
        canvas.draw_leak(LeakUniform {
            bbox: [x0, y0, bw, bh],
            rgb_alpha: [
                self.rgb.0 as f32 / 255.0,
                self.rgb.1 as f32 / 255.0,
                self.rgb.2 as f32 / 255.0,
                eff_alpha,
            ],
            count_pad: [n as u32, blur_px.to_bits(), 0, 0],
            pts: packed,
        });
    }
}
