// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::render::{Canvas, GpuCore, LOGICAL_H, LOGICAL_W};
use crate::save::SaveData;
use crate::ui::GameApp;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::{
    Instance, PresentMode, Surface, SurfaceConfiguration, SurfaceError, TextureUsages,
    TextureViewDescriptor,
};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{Fullscreen, Window, WindowId};

struct WindowState {
    window: Arc<Window>,
    surface: Surface<'static>,
    config: SurfaceConfiguration,
    gpu: Arc<GpuCore>,
    configured_fps: u32,
}

struct Runner {
    pending: Option<(AssetBundle, Option<u32>, bool)>,
    app: Option<GameApp>,
    canvas: Canvas,
    win_state: Option<WindowState>,
    last_frame: Instant,
    next_frame_at: Instant,
    redraw_pending: bool,
    modifiers: ModifiersState,
    cursor_phys: (f64, f64),
    win_size: (u32, u32),
}

fn monitor_refresh_hz(window: &Window) -> Option<u32> {
    window
        .current_monitor()
        .and_then(|m| m.refresh_rate_millihertz())
        .map(|mhz| (mhz + 500) / 1000)
}

fn choose_present_mode(
    modes: &[PresentMode],
    target_fps: u32,
    monitor_hz: Option<u32>,
) -> PresentMode {
    let pick = |prefs: &[PresentMode]| {
        prefs
            .iter()
            .copied()
            .find(|m| modes.contains(m))
            .unwrap_or(PresentMode::Fifo)
    };
    if target_fps == 0 {
        pick(&[
            PresentMode::Immediate,
            PresentMode::Mailbox,
            PresentMode::Fifo,
        ])
    } else if target_fps <= 60 || monitor_hz.is_some_and(|hz| hz >= target_fps) {
        PresentMode::Fifo
    } else {
        pick(&[
            PresentMode::Mailbox,
            PresentMode::Immediate,
            PresentMode::Fifo,
        ])
    }
}

impl Runner {
    fn map_cursor(&self, px: f64, py: f64) -> (f32, f32) {
        let (ww, wh) = (self.win_size.0.max(1) as f32, self.win_size.1.max(1) as f32);
        let scale = (ww / LOGICAL_W as f32).min(wh / LOGICAL_H as f32);
        let dw = LOGICAL_W as f32 * scale;
        let dh = LOGICAL_H as f32 * scale;
        let ox = (ww - dw) * 0.5;
        let oy = (wh - dh) * 0.5;
        (
            ((px as f32 - ox) / dw) * LOGICAL_W as f32,
            ((py as f32 - oy) / dh) * LOGICAL_H as f32,
        )
    }

    fn apply_fullscreen(&self, want_full: bool) {
        if let Some(ws) = &self.win_state
            && want_full != ws.window.fullscreen().is_some()
        {
            ws.window.set_fullscreen(if want_full {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            });
        }
    }

    fn present_frame(&mut self) {
        let (Some(ws), Some(app)) = (&mut self.win_state, &mut self.app) else {
            return;
        };
        if ws.config.width == 0 || ws.config.height == 0 {
            return;
        }
        let target_fps = app.target_fps();
        if target_fps != ws.configured_fps {
            let hz = monitor_refresh_hz(&ws.window);
            let desired_pm = choose_present_mode(&ws.gpu.present_modes, target_fps, hz);
            if desired_pm != ws.config.present_mode {
                ws.config.present_mode = desired_pm;
                ws.surface.configure(&ws.gpu.device, &ws.config);
            }
            ws.configured_fps = target_fps;
        }
        let scale = (ws.config.width as f32 / LOGICAL_W as f32)
            .min(ws.config.height as f32 / LOGICAL_H as f32);
        self.canvas.begin_frame(scale);
        app.render(&mut self.canvas);

        let frame = match ws.surface.get_current_texture() {
            Ok(f) => f,
            Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                ws.surface.configure(&ws.gpu.device, &ws.config);
                return;
            }
            Err(_) => return,
        };
        let view = frame.texture.create_view(&TextureViewDescriptor {
            format: Some(ws.gpu.target_format),
            ..Default::default()
        });
        ws.gpu.render_frame(
            &self.canvas,
            &app.renderer.bank,
            &view,
            ws.config.width,
            ws.config.height,
        );
        frame.present();
    }
}

impl ApplicationHandler for Runner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.win_state.is_some() {
            return;
        }
        let Some((assets, fps_override, init_full)) = self.pending.take() else {
            return;
        };
        let mut attrs = Window::default_attributes()
            .with_title("Aqualux")
            .with_inner_size(LogicalSize::new(960.0, 720.0))
            .with_min_inner_size(LogicalSize::new(400.0, 300.0));
        if init_full {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        }
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        window.set_cursor_visible(false);

        let instance = Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create wgpu surface");
        let gpu = Arc::new(
            GpuCore::new_for_surface(&instance, &surface)
                .expect("Failed to initialize GPU backend"),
        );
        let app = GameApp::new(assets, fps_override, gpu.clone())
            .expect("Failed to initialize game state");

        let sz = window.inner_size();
        self.win_size = (sz.width, sz.height);
        let init_fps = app.target_fps();
        let hz = monitor_refresh_hz(&window);
        let view_formats = if gpu.surface_format != gpu.target_format {
            vec![gpu.target_format]
        } else {
            Vec::new()
        };
        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: gpu.surface_format,
            width: sz.width.max(1),
            height: sz.height.max(1),
            present_mode: choose_present_mode(&gpu.present_modes, init_fps, hz),
            alpha_mode: gpu.alpha_mode,
            view_formats,
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&gpu.device, &config);

        self.app = Some(app);
        self.win_state = Some(WindowState {
            window,
            surface,
            config,
            gpu,
            configured_fps: init_fps,
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(app) = &mut self.app else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => {
                app.save.save();
                event_loop.exit();
            }
            WindowEvent::Resized(sz) => {
                self.win_size = (sz.width, sz.height);
                if let Some(ws) = &mut self.win_state {
                    if sz.width > 0 && sz.height > 0 {
                        ws.config.width = sz.width;
                        ws.config.height = sz.height;
                        ws.surface.configure(&ws.gpu.device, &ws.config);
                    }
                    self.redraw_pending = true;
                    ws.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                self.modifiers = mods.state();
            }
            WindowEvent::CursorEntered { .. } => {
                app.cursor_in_window = true;
                if let Some(ws) = &self.win_state {
                    ws.window.set_cursor_visible(false);
                }
            }
            WindowEvent::CursorLeft { .. } => {
                app.cursor_in_window = false;
                if let Some(ws) = &self.win_state {
                    ws.window.set_cursor_visible(true);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_phys = (position.x, position.y);
                let (lx, ly) = self.map_cursor(position.x, position.y);
                if let Some(a) = &mut self.app {
                    a.on_mouse_move(lx, ly);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                let (lx, ly) = self.map_cursor(self.cursor_phys.0, self.cursor_phys.1);
                if let Some(a) = &mut self.app {
                    match state {
                        ElementState::Pressed => a.on_mouse_down(lx, ly),
                        ElementState::Released => a.on_mouse_up(lx, ly),
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let toggle_full = matches!(event.logical_key, Key::Named(NamedKey::F11))
                    || (matches!(event.logical_key, Key::Named(NamedKey::Enter))
                        && self.modifiers.alt_key());
                if toggle_full {
                    app.save.config.fullscreen = !app.save.config.fullscreen;
                    let want = app.save.config.fullscreen;
                    app.save.save();
                    self.apply_fullscreen(want);
                } else {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => app.on_key_escape(),
                        Key::Named(NamedKey::Backspace) => app.on_key_backspace(),
                        Key::Named(NamedKey::Enter) => app.on_key_enter(),
                        Key::Named(NamedKey::Space) => app.on_text_char(' '),
                        Key::Character(s)
                            if !self.modifiers.control_key() && !self.modifiers.alt_key() =>
                        {
                            for ch in s.chars() {
                                app.on_text_char(ch);
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw_pending = false;
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;
                app.update(dt);
                let sync_req = app.sync_fullscreen_req;
                let want_full = app.save.config.fullscreen;
                let quit = app.should_quit;
                if sync_req {
                    app.sync_fullscreen_req = false;
                    self.apply_fullscreen(want_full);
                }
                if quit {
                    event_loop.exit();
                    return;
                }
                self.present_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(app) = &self.app else {
            return;
        };
        let fps = app.target_fps();
        if fps == 0 {
            event_loop.set_control_flow(ControlFlow::Poll);
            if !self.redraw_pending
                && let Some(ws) = &self.win_state
            {
                self.redraw_pending = true;
                ws.window.request_redraw();
            }
            return;
        }
        let frame_dur = Duration::from_secs_f64(1.0 / fps as f64);
        let now = Instant::now();
        if !self.redraw_pending && now >= self.next_frame_at {
            if let Some(ws) = &self.win_state {
                self.redraw_pending = true;
                ws.window.request_redraw();
            }
            self.next_frame_at = if now.duration_since(self.next_frame_at) > frame_dur * 2 {
                now + frame_dur
            } else {
                self.next_frame_at + frame_dur
            };
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_at));
    }
}

pub fn run_window(bundle: AssetBundle, fps_override: Option<u32>) -> Result<(), String> {
    let init_full = SaveData::load().config.fullscreen;
    let event_loop = EventLoop::new().map_err(|e| format!("Failed to create EventLoop: {}", e))?;
    let now = Instant::now();
    let mut runner = Runner {
        pending: Some((bundle, fps_override, init_full)),
        app: None,
        canvas: Canvas::new(),
        win_state: None,
        last_frame: now,
        next_frame_at: now,
        redraw_pending: false,
        modifiers: ModifiersState::empty(),
        cursor_phys: (400.0, 300.0),
        win_size: (960, 720),
    };
    event_loop
        .run_app(&mut runner)
        .map_err(|e| format!("EventLoop error: {}", e))
}
