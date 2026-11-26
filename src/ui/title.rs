// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::render::easing::{back_in, back_out, quad_out};
use crate::render::{
    BackgroundRenderer, BgAnimState, Canvas, FontFamily, FontSet, Rect, TexBank, TextAlign,
    TextOutline, draw_sprite_title,
};
use crate::save::SaveData;
use crate::ui::title_data::{
    BUTTON_RIGHT, BUTTON_SPACING, BUTTON_Y, INFO_GUTTER, TICKER_TEXT, menu_entries,
};
use crate::ui::widgets::{UiAction, UiButton, draw_text_button, draw_text_button_blurred};

pub use crate::ui::title_data::TitleSubmenu;

/// Horizontal 500px `TextTicker.scrollRect` placed at `x = 5`, padded vertically for stroke/shadow pixels around `y = 572`.
pub const TICKER_CLIP_RECT: Rect = Rect::new(5.0, 558.0, 500.0, 59.0);
pub const TITLE_BUTTON_INFO_MIN_CENTER_X: f32 = 295.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TitleMenuAnim {
    InitialSlide,
    FadeIn { elapsed: f32 },
    Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TitleProfileAnim {
    pub is_new_greeting: bool,
    pub elapsed: f32,
    pub welcome_delay: f32,
    pub button_delay: f32,
}

impl TitleProfileAnim {
    pub const fn on_title_enter() -> Self {
        Self {
            is_new_greeting: false,
            elapsed: 0.0,
            welcome_delay: 5.9,
            button_delay: 6.1,
        }
    }

    pub const fn on_first_profile_created() -> Self {
        Self {
            is_new_greeting: true,
            elapsed: 0.0,
            welcome_delay: 0.1,
            button_delay: 0.1,
        }
    }

    pub const fn immediate(is_new_greeting: bool) -> Self {
        Self {
            is_new_greeting,
            elapsed: 1.0,
            welcome_delay: 0.0,
            button_delay: 0.0,
        }
    }
}

/// `TextTicker` scroll X relative to its `x = 5` container at 30 px/s, wrapping to `380` after `width` scrolls past `0`.
pub fn ticker_local_x(title_age: f32, single_w: f32) -> f32 {
    let total_w = (single_w * 2.0).max(1.0);
    let scrolled = title_age.max(0.0) * 30.0;
    if scrolled < total_w {
        -scrolled
    } else {
        let period = total_w + 380.0;
        380.0 - ((scrolled - total_w) % period)
    }
}

pub fn title_menu_metrics(entry_count: usize) -> (f32, f32, f32) {
    if entry_count <= 6 {
        (BUTTON_Y, BUTTON_SPACING, 31.0)
    } else if entry_count == 7 {
        (266.0, 28.0, 30.0)
    } else if entry_count == 8 {
        (256.0, 27.0, 28.0)
    } else {
        (248.0, 25.0, 26.0)
    }
}

fn effective_profile_anim(
    save: &SaveData,
    profile_anim: Option<TitleProfileAnim>,
    wait_profile_latched: bool,
    title_age: f32,
) -> Option<TitleProfileAnim> {
    if wait_profile_latched || !save.has_profile() {
        return None;
    }
    Some(profile_anim.unwrap_or(TitleProfileAnim {
        is_new_greeting: false,
        elapsed: title_age,
        welcome_delay: 5.9,
        button_delay: 6.1,
    }))
}

fn quad_fade_in_out(
    elapsed: f32,
    delay: f32,
    in_dur: f32,
    hiding_age: Option<f32>,
    out_dur: f32,
) -> f32 {
    let show = |t: f32| quad_out(((t - delay) / in_dur).clamp(0.0, 1.0));
    if let Some(h) = hiding_age {
        show((elapsed - h).max(0.0)) * (1.0 - quad_out((h / out_dur).clamp(0.0, 1.0)))
    } else {
        show(elapsed)
    }
}

fn menu_button_pose(
    width: f32,
    entry_idx: usize,
    non_empty_idx: usize,
    title_age: f32,
    menu_anim: TitleMenuAnim,
    hiding_age: Option<f32>,
) -> (f32, f32, (f32, f32), bool) {
    let target_left = (BUTTON_RIGHT - width).floor();
    let start_left = if width < 265.0 { -260.0 } else { -width - 3.0 };
    if let Some(h) = hiding_age {
        let delay = non_empty_idx as f32 * 0.1;
        let u = ((h - delay) / 1.3).clamp(0.0, 1.0);
        let cur_left = target_left + (-260.0 - target_left) * back_in(u);
        return (cur_left, 1.0, (0.0, 0.0), true);
    }
    match menu_anim {
        TitleMenuAnim::InitialSlide => {
            let delay = 4.0 + entry_idx as f32 * 0.1;
            let cur_left = if title_age < delay {
                start_left
            } else {
                let u = ((title_age - delay) / 1.5).clamp(0.0, 1.0);
                start_left + (target_left - start_left) * back_out(u)
            };
            (cur_left, 1.0, (0.0, 0.0), title_age < delay + 1.5)
        }
        TitleMenuAnim::FadeIn { elapsed } => {
            let u = quad_out((elapsed / 0.5).clamp(0.0, 1.0));
            let b = (1.0 - u) * 16.0;
            (target_left, u, (b, b), elapsed < 0.5)
        }
        TitleMenuAnim::Instant => (target_left, 1.0, (0.0, 0.0), false),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn build_title_buttons(
    submenu: TitleSubmenu,
    save: &SaveData,
    fonts: &FontSet,
    title_age: f32,
    menu_anim: TitleMenuAnim,
    profile_anim: Option<TitleProfileAnim>,
    wait_profile_latched: bool,
    hiding_age: Option<f32>,
) -> Vec<UiButton> {
    if hiding_age.is_some() {
        return Vec::new();
    }
    let entries = menu_entries(submenu, save);
    let (base_y, spacing, font_size) = title_menu_metrics(entries.len());
    let mut out = Vec::with_capacity(entries.len() + 2);
    let mut non_empty_idx = 0usize;
    for (i, e) in entries.into_iter().enumerate() {
        let Some(act) = e.action else {
            continue;
        };
        let width = fonts.measure_width(FontFamily::Neuropol, e.label, font_size);
        let (cur_left, _, _, is_tweening) =
            menu_button_pose(width, i, non_empty_idx, title_age, menu_anim, hiding_age);
        non_empty_idx += 1;
        if !is_tweening && !wait_profile_latched {
            out.push(UiButton {
                id: e.id,
                rect: Rect::new(
                    cur_left,
                    base_y + i as f32 * spacing,
                    width.max(1.0),
                    spacing,
                ),
                action: act,
                enabled: true,
                hover_sfx: Some("PopTwoSound"),
            });
        }
    }

    if let Some(pa) = effective_profile_anim(save, profile_anim, wait_profile_latched, title_age)
        && pa.elapsed >= pa.button_delay + 0.5
    {
        let w = fonts.measure_width(FontFamily::Neuropol, "[Change Profile]", 18.0);
        out.push(UiButton {
            id: "btn_change_profile",
            rect: Rect::new(775.0 - w, 45.0, w.max(1.0), 24.0),
            action: UiAction::OpenProfileModal,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }
    if title_age >= 7.2 && !wait_profile_latched {
        let w = fonts.measure_width(FontFamily::Neuropol, "milkmangames.com", 18.0);
        out.push(UiButton {
            id: "btn_mmg",
            rect: Rect::new(790.0 - w, 570.0, w.max(1.0), 24.0),
            action: UiAction::OpenMmgNotice,
            enabled: true,
            hover_sfx: None,
        });
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn render_title_screen(
    canvas: &mut Canvas,
    assets: &AssetBundle,
    bank: &TexBank,
    backgrounds: &BackgroundRenderer,
    fonts: &FontSet,
    bg_anim: &BgAnimState,
    save: &SaveData,
    submenu: TitleSubmenu,
    title_age: f32,
    menu_anim: TitleMenuAnim,
    profile_anim: Option<TitleProfileAnim>,
    wait_profile_latched: bool,
    hiding_age: Option<f32>,
    modal_open: bool,
    mouse_pos: (f32, f32),
) {
    backgrounds.draw_night_hills_rear(canvas, assets, bank, bg_anim);

    let ticker_alpha = quad_fade_in_out(title_age, 5.0, 2.0, hiding_age, 2.0);
    if ticker_alpha > 0.004 {
        let tw = fonts.measure_width(FontFamily::Neuropol, TICKER_TEXT, 18.0);
        let pos_x = 5.0 + ticker_local_x(title_age, tw);
        canvas.set_clip(Some(TICKER_CLIP_RECT));
        for tx in [pos_x, pos_x + tw] {
            fonts.draw_styled_alpha(
                canvas,
                FontFamily::Neuropol,
                tx,
                572.0,
                18.0,
                1.0,
                1.0,
                (255, 255, 255),
                Some(TextOutline::shadowed((0x05, 0x0D, 0x2B))),
                TextAlign::Left,
                TICKER_TEXT,
                ticker_alpha,
            );
        }
        canvas.set_clip(None);
    }

    backgrounds.draw_night_hills_front(canvas, assets, bank, bg_anim);
    backgrounds.draw_outgoing_overlay(canvas, assets, bank, bg_anim);

    let title_x = if let Some(h) = hiding_age {
        let u = (h / 1.5).clamp(0.0, 1.0);
        280.0 + (1500.0 - 280.0) * back_in(u)
    } else {
        280.0
    };
    draw_sprite_title(canvas, assets, bank, title_x, 200.0, title_age);

    let entries = menu_entries(submenu, save);
    let (base_y, spacing, font_size) = title_menu_metrics(entries.len());
    let mut hovered_info: Option<&str> = None;
    let mut non_empty_idx = 0usize;
    for (i, entry) in entries.iter().enumerate() {
        if entry.label.is_empty() {
            continue;
        }
        let y = base_y + i as f32 * spacing;
        let width = fonts.measure_width(FontFamily::Neuropol, entry.label, font_size);
        let (cur_left, b_alpha, blur, _) =
            menu_button_pose(width, i, non_empty_idx, title_age, menu_anim, hiding_age);
        non_empty_idx += 1;

        let hit = Rect::new(cur_left, y, width.max(1.0), spacing);
        let hovered = !modal_open
            && hiding_age.is_none()
            && b_alpha > 0.01
            && hit.contains(mouse_pos.0, mouse_pos.1);
        if hovered
            && cur_left + width * 0.5 >= TITLE_BUTTON_INFO_MIN_CENTER_X
            && !entry.info.is_empty()
        {
            hovered_info = Some(entry.info);
        }

        draw_text_button_blurred(
            canvas,
            fonts,
            cur_left + width,
            y,
            font_size,
            TextAlign::Right,
            entry.label,
            hovered,
            None,
            b_alpha,
            blur,
        );
    }

    if let Some(info) = hovered_info {
        let info_y = (base_y + entries.len() as f32 * spacing + INFO_GUTTER).min(484.0);
        fonts.draw_multiline(
            canvas,
            FontFamily::Neuropol,
            BUTTON_RIGHT,
            info_y,
            18.0,
            21.0,
            (255, 255, 255),
            Some(TextOutline::shadowed((0x05, 0x0D, 0x2B))),
            TextAlign::Right,
            450.0,
            info,
            1.0,
        );
    }

    let mmg_alpha = quad_fade_in_out(title_age, 5.2, 2.0, hiding_age, 2.0);
    let mmg_w = fonts.measure_width(FontFamily::Neuropol, "milkmangames.com", 18.0);
    let mmg_hov = !modal_open
        && hiding_age.is_none()
        && mmg_alpha > 0.01
        && Rect::new(790.0 - mmg_w, 570.0, mmg_w.max(1.0), 24.0).contains(mouse_pos.0, mouse_pos.1);
    draw_text_button(
        canvas,
        fonts,
        790.0,
        570.0,
        18.0,
        TextAlign::Right,
        "milkmangames.com",
        mmg_hov,
        Some((0xFF, 0xFF, 0x08)),
        mmg_alpha,
    );

    if let Some(prof) = save.active_profile()
        && let Some(pa) =
            effective_profile_anim(save, profile_anim, wait_profile_latched, title_age)
    {
        let welcome_alpha = quad_fade_in_out(pa.elapsed, pa.welcome_delay, 0.5, hiding_age, 2.0);
        let btn_alpha = quad_fade_in_out(pa.elapsed, pa.button_delay, 0.5, hiding_age, 2.0);
        let greet = if pa.is_new_greeting {
            format!("Welcome, {}!", prof.name)
        } else {
            format!("Welcome back, {}!", prof.name)
        };
        fonts.draw_styled_alpha(
            canvas,
            FontFamily::Neuropol,
            775.0,
            15.0,
            26.0,
            1.0,
            1.0,
            (255, 255, 255),
            Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
            TextAlign::Right,
            &greet,
            welcome_alpha,
        );
        let prof_w = fonts.measure_width(FontFamily::Neuropol, "[Change Profile]", 18.0);
        let ch_hov = !modal_open
            && hiding_age.is_none()
            && btn_alpha > 0.01
            && Rect::new(775.0 - prof_w, 45.0, prof_w.max(1.0), 24.0)
                .contains(mouse_pos.0, mouse_pos.1);
        draw_text_button(
            canvas,
            fonts,
            775.0,
            45.0,
            18.0,
            TextAlign::Right,
            "[Change Profile]",
            ch_hov,
            None,
            btn_alpha,
        );
    }
}
