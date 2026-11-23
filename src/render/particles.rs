// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::game::GameColor;
use crate::render::canvas::Canvas;
use crate::render::easing::back_out;
use crate::render::emitter::Particle30;
use crate::render::tex_bank::TexBank;
use rand::RngExt;

/// Multi-ray celebratory firework burst with fading star trails.
#[derive(Debug, Clone)]
pub struct FireworkBurst {
    origin_x: f32,
    origin_y: f32,
    rgb: (u8, u8, u8),
    particles: Vec<Particle30>,
    age_ticks: u32,
    accum: f32,
    elapsed: f32,
    star_durs: Vec<f32>,
}

impl FireworkBurst {
    pub fn new(
        origin_x: f32,
        origin_y: f32,
        rgb: (u8, u8, u8),
        power: f32,
        rng: &mut impl RngExt,
    ) -> Self {
        let count = 15usize;
        let mut particles = Vec::with_capacity(count);
        let mut star_durs = Vec::with_capacity(count);
        for i in 0..count {
            let deg = (i as f32) * (360.0 / count as f32);
            let rad = deg.to_radians();
            let xf = rad.sin() * (30.0 * power * ((rng.random_range(0.0..1.0) + 0.8) * 0.31));
            let yf = rad.cos() * (15.0 * power * ((rng.random_range(0.0..1.0) + 0.8) * 0.31));
            particles.push(Particle30::new(xf, yf, 0.7, 0.7, 0.05, 1.05));
            star_durs.push(rng.random_range(2.0..4.0));
        }
        Self {
            origin_x,
            origin_y,
            rgb,
            particles,
            age_ticks: 0,
            accum: 0.0,
            elapsed: 0.0,
            star_durs,
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;
        self.accum += dt;
        let step = 1.0 / 30.0;
        while self.accum >= step && self.age_ticks < 35 {
            self.accum -= step;
            self.age_ticks += 1;
            for p in &mut self.particles {
                p.step();
            }
        }
    }

    pub fn is_alive(&self) -> bool {
        self.age_ticks < 35
    }

    pub fn draw(&self, canvas: &mut Canvas, fade: f32) {
        let fade = fade.clamp(0.0, 1.0);
        if fade <= 0.01 {
            return;
        }
        for (i, p) in self.particles.iter().enumerate() {
            let u = (self.elapsed / self.star_durs[i]).clamp(0.0, 1.0);
            let alpha = ((1.0 - back_out(u).max(0.0)).clamp(0.0, 1.0) * fade * 255.0) as u8;
            if alpha < 4 {
                continue;
            }
            let sx = self.origin_x + p.x;
            let sy = self.origin_y + p.y;
            canvas.fill_circle_alpha(sx, sy, 6.0, (self.rgb.0, self.rgb.1, self.rgb.2, alpha / 3));
            canvas.fill_rect_alpha(
                crate::render::Rect::new(sx - 1.5, sy - 1.5, 3.0, 3.0),
                255,
                255,
                255,
                alpha,
            );
        }
    }
}

struct FieldStar {
    x_pos: f32,
    y_pos: f32,
    z_depth: f32,
    rot: f32,
    rot_vel: f32,
    pipe: Option<(&'static str, GameColor)>,
}

const STAR_PIPE_COLORS: [GameColor; 8] = [
    GameColor::Green,
    GameColor::Grey,
    GameColor::Orange,
    GameColor::Red,
    GameColor::Violet,
    GameColor::White,
    GameColor::Yellow,
    GameColor::Blue,
];

/// Parallax 250-star background field for the Champion screen.
pub struct StarField {
    stars: Vec<FieldStar>,
}

impl StarField {
    pub fn new(rng: &mut impl RngExt) -> Self {
        let mut stars = Vec::with_capacity(250);
        for i in 0..250 {
            let (x_pos, y_pos) = if i < 125 {
                (
                    rng.random_range(-800.0..800.0),
                    rng.random_range(-600.0..600.0),
                )
            } else {
                (
                    rng.random_range(-400.0..400.0),
                    rng.random_range(-300.0..300.0),
                )
            };
            let pipe = match rng.random_range(0..70) {
                1 => Some("PipeSpigotBitmap"),
                2 => Some("ElbowClearBitmap"),
                3 => Some("StraightClearBitmap"),
                _ => None,
            }
            .map(|k| {
                (
                    k,
                    STAR_PIPE_COLORS[rng.random_range(0..STAR_PIPE_COLORS.len())],
                )
            });
            stars.push(FieldStar {
                x_pos,
                y_pos,
                z_depth: rng.random_range(-1600.0..400.0),
                rot: 0.0,
                rot_vel: rng.random_range(-12.0..-6.0),
                pipe,
            });
        }
        Self { stars }
    }

    pub fn update(&mut self, dt: f32) {
        let ticks = dt * 30.0;
        for s in &mut self.stars {
            s.rot += s.rot_vel * ticks;
            s.z_depth += 30.0 * ticks;
            if s.z_depth > 600.0 {
                s.z_depth -= 2000.0;
            }
        }
    }

    pub fn draw(&self, canvas: &mut Canvas, bank: &TexBank, mouse_pos: (f32, f32), alpha: f32) {
        if alpha <= 0.004 {
            return;
        }
        let x_prop = ((mouse_pos.0 - 400.0) / 400.0 * -80.0).round();
        let y_prop = ((mouse_pos.1 - 300.0) / 300.0 * -80.0).round();
        for s in &self.stars {
            let denom = 500.0 - s.z_depth;
            if denom <= 10.0 {
                continue;
            }
            let scale = 500.0 / denom;
            let sx = 400.0 + s.x_pos * scale + x_prop;
            let sy = 300.0 + s.y_pos * scale + y_prop;
            let star_a = (scale + 0.2).clamp(0.0, 1.0) * alpha;
            if star_a <= 0.01 {
                continue;
            }
            let rad = (5.0 * scale).max(0.8);
            if sx + rad + 2.0 >= -4.0
                && sx - rad - 2.0 <= 804.0
                && sy + rad + 2.0 >= -4.0
                && sy - rad - 2.0 <= 604.0
            {
                canvas.draw_star_orb(sx, sy, rad, star_a);
            }
            if let Some((key, col)) = s.pipe
                && let Some((tid, bw, bh)) = bank.named(key)
            {
                let dw = bw as f32 * scale;
                let dh = bh as f32 * scale;
                let half = dw.max(dh) * 0.72;
                if sx + half >= 0.0 && sx - half <= 800.0 && sy + half >= 0.0 && sy - half <= 600.0
                {
                    canvas.draw_tex_uv_rotated_blur(
                        tid,
                        [0.0, 0.0, 1.0, 1.0],
                        sx,
                        sy,
                        dw,
                        dh,
                        s.rot,
                        Some((col, false)),
                        1.0,
                        star_a,
                        false,
                        (0.0, 0.0),
                    );
                }
            }
        }
    }
}
