// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

#[derive(Debug, Clone)]
pub struct ScoreRoller {
    pub current: f32,
    pub start: f32,
    pub target: f32,
    pub elapsed: f32,
    pub duration: f32,
    pub delay: f32,
}

impl ScoreRoller {
    pub fn new(initial: u32) -> Self {
        Self {
            current: initial as f32,
            start: initial as f32,
            target: initial as f32,
            elapsed: 0.0,
            duration: 0.0,
            delay: 0.0,
        }
    }

    pub fn roll_to(&mut self, target: u32, duration: f32, delay: f32) {
        let tgt = target as f32;
        if duration <= 0.0 && delay <= 0.0 {
            self.current = tgt;
            self.start = tgt;
            self.target = tgt;
            self.elapsed = 0.0;
            self.duration = 0.0;
            self.delay = 0.0;
            return;
        }
        if (self.target - tgt).abs() < 0.5 && self.duration > 0.0 {
            return;
        }
        self.start = self.current;
        self.target = tgt;
        self.elapsed = 0.0;
        self.duration = duration;
        self.delay = delay;
    }

    pub fn update(&mut self, dt: f32) {
        if self.duration <= 0.0 {
            self.current = self.target;
            return;
        }
        if self.delay > 0.0 {
            self.delay = (self.delay - dt).max(0.0);
            return;
        }
        self.elapsed = (self.elapsed + dt).min(self.duration);
        let t = (self.elapsed / self.duration).clamp(0.0, 1.0);
        let eased = t * t * t * t * t;
        self.current = self.start + (self.target - self.start) * eased;
        if t >= 1.0 {
            self.current = self.target;
            self.duration = 0.0;
        }
    }

    pub fn value(&self) -> u32 {
        self.current.round().max(0.0) as u32
    }
}
