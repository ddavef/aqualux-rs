// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::collections::HashMap;
use std::io::Cursor;

pub fn music_for_background(bg_id: &str) -> &'static str {
    match bg_id.trim().to_ascii_lowercase().as_str() {
        "nighthills" => "GlideMusic",
        "daysky" | "bluesky" => "InGameLittleRayMusic",
        "snowyhills" | "snowhills" => "IstanbulMusic",
        "outerspace" | "spacesky" => "DriftingMusic",
        _ => "GlideMusic",
    }
}

struct DelayedAudio {
    remaining: f32,
    is_music: bool,
    name: String,
    vol_scale: f32,
    loop_forever: bool,
}

pub struct AudioEngine {
    device_sink: Option<MixerDeviceSink>,
    music_player: Option<Player>,
    current_music: Option<String>,
    sounds: HashMap<String, Vec<u8>>,
    delayed: Vec<DelayedAudio>,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl AudioEngine {
    pub fn new(sounds: HashMap<String, Vec<u8>>, music_vol: f32, sfx_vol: f32) -> Self {
        let device_sink = DeviceSinkBuilder::open_default_sink().ok().map(|mut s| {
            s.log_on_drop(false);
            s
        });
        Self {
            device_sink,
            music_player: None,
            current_music: None,
            sounds,
            delayed: Vec::new(),
            music_volume: music_vol.clamp(0.0, 1.0),
            sfx_volume: sfx_vol.clamp(0.0, 1.0),
        }
    }

    pub fn apply_volumes(&self) {
        if let Some(player) = &self.music_player {
            player.set_volume(self.music_volume);
        }
    }

    pub fn update(&mut self, dt: f32) {
        if self.delayed.is_empty() {
            return;
        }
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.delayed.len() {
            self.delayed[i].remaining -= dt;
            if self.delayed[i].remaining <= 0.0 {
                ready.push(self.delayed.swap_remove(i));
            } else {
                i += 1;
            }
        }
        for item in ready {
            if item.is_music {
                self.play_music(&item.name, item.loop_forever);
            } else {
                self.play_sfx_vol(&item.name, item.vol_scale);
            }
        }
    }

    pub fn clear_delayed(&mut self) {
        self.delayed.clear();
    }

    #[cfg(test)]
    pub fn delayed_sfx(&self) -> Vec<(&str, f32)> {
        self.delayed
            .iter()
            .filter(|item| !item.is_music)
            .map(|item| (item.name.as_str(), item.remaining))
            .collect()
    }

    pub fn play_music(&mut self, name: &str, loop_forever: bool) {
        if self.current_music.as_deref() == Some(name)
            && self.music_player.as_ref().is_none_or(|p| !p.empty())
        {
            return;
        }
        self.stop_music();
        self.current_music = Some(name.to_string());
        let Some(sink) = &self.device_sink else {
            return;
        };
        let Some(bytes) = self.sounds.get(name) else {
            return;
        };
        let Ok(decoder) = Decoder::new(Cursor::new(bytes.clone())) else {
            return;
        };
        let player = Player::connect_new(sink.mixer());
        player.set_volume(self.music_volume);
        if loop_forever {
            player.append(decoder.repeat_infinite());
        } else {
            player.append(decoder);
        }
        self.music_player = Some(player);
    }

    pub fn play_music_delayed(&mut self, name: &str, delay_secs: f32, loop_forever: bool) {
        if delay_secs <= 0.0 {
            self.play_music(name, loop_forever);
            return;
        }
        self.delayed.push(DelayedAudio {
            remaining: delay_secs,
            is_music: true,
            name: name.to_string(),
            vol_scale: 1.0,
            loop_forever,
        });
    }

    pub fn stop_music(&mut self) {
        if let Some(player) = self.music_player.take() {
            player.stop();
        }
        self.current_music = None;
    }

    pub fn play_sfx(&self, name: &str) {
        self.play_sfx_vol(name, 1.0);
    }

    pub fn play_sfx_vol(&self, name: &str, vol_scale: f32) {
        let vol = (self.sfx_volume * vol_scale).clamp(0.0, 1.0);
        if vol <= 0.001 {
            return;
        }
        let Some(sink) = &self.device_sink else {
            return;
        };
        let Some(bytes) = self.sounds.get(name) else {
            return;
        };
        let Ok(decoder) = Decoder::new(Cursor::new(bytes.clone())) else {
            return;
        };
        let player = Player::connect_new(sink.mixer());
        player.set_volume(vol);
        player.append(decoder);
        player.detach();
    }

    pub fn play_sfx_delayed(&mut self, name: &str, delay_secs: f32, vol_scale: f32) {
        if delay_secs <= 0.0 {
            self.play_sfx_vol(name, vol_scale);
            return;
        }
        self.delayed.push(DelayedAudio {
            remaining: delay_secs,
            is_music: false,
            name: name.to_string(),
            vol_scale,
            loop_forever: false,
        });
    }
}
