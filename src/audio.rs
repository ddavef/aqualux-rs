// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
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
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    music_sink: Option<Sink>,
    current_music: Option<String>,
    sounds: HashMap<String, Vec<u8>>,
    delayed: Vec<DelayedAudio>,
    pub music_volume: f32,
    pub sfx_volume: f32,
}

impl AudioEngine {
    pub fn new(sounds: HashMap<String, Vec<u8>>, music_vol: f32, sfx_vol: f32) -> Self {
        let (stream, handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(_) => (None, None),
        };
        Self {
            _stream: stream,
            handle,
            music_sink: None,
            current_music: None,
            sounds,
            delayed: Vec::new(),
            music_volume: music_vol.clamp(0.0, 1.0),
            sfx_volume: sfx_vol.clamp(0.0, 1.0),
        }
    }

    pub fn apply_volumes(&self) {
        if let Some(sink) = &self.music_sink {
            sink.set_volume(self.music_volume);
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

    pub fn play_music(&mut self, name: &str, loop_forever: bool) {
        if self.current_music.as_deref() == Some(name)
            && self.music_sink.as_ref().is_none_or(|s| !s.empty())
        {
            return;
        }
        self.stop_music();
        self.current_music = Some(name.to_string());
        let Some(handle) = &self.handle else {
            return;
        };
        let Some(bytes) = self.sounds.get(name) else {
            return;
        };
        let Ok(decoder) = Decoder::new(Cursor::new(bytes.clone())) else {
            return;
        };
        let Ok(sink) = Sink::try_new(handle) else {
            return;
        };
        sink.set_volume(self.music_volume);
        if loop_forever {
            sink.append(decoder.repeat_infinite());
        } else {
            sink.append(decoder);
        }
        self.music_sink = Some(sink);
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
        if let Some(sink) = self.music_sink.take() {
            sink.stop();
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
        let Some(handle) = &self.handle else {
            return;
        };
        let Some(bytes) = self.sounds.get(name) else {
            return;
        };
        let Ok(decoder) = Decoder::new(Cursor::new(bytes.clone())) else {
            return;
        };
        if let Ok(sink) = Sink::try_new(handle) {
            sink.set_volume(vol);
            sink.append(decoder);
            sink.detach();
        }
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
