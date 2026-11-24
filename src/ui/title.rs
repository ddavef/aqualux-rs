// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::render::easing::{back_in, back_out};
use crate::render::{
    BackgroundRenderer, BgAnimState, Canvas, FontFamily, FontSet, Rect, TexBank, TextAlign,
    draw_sprite_title,
};
use crate::save::SaveData;
use crate::ui::title_data::{
    BUTTON_RIGHT, BUTTON_SPACING, BUTTON_Y, INFO_GUTTER, TICKER_TEXT, menu_entries,
};
use crate::ui::widgets::{UiAction, UiButton, draw_text_button, draw_text_button_blurred};

pub use crate::ui::title_data::TitleSubmenu;

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

pub fn build_title_buttons(
    submenu: TitleSubmenu,
    save: &SaveData,
    title_age: f32,
    hiding_age: Option<f32>,
) -> Vec<UiButton> {
    if hiding_age.is_some() || title_age < 4.5 {
        return Vec::new();
    }
    let entries = menu_entries(submenu, save);
    let (base_y, spacing, _) = title_menu_metrics(entries.len());
    let mut out: Vec<UiButton> = entries
        .into_iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let act = e.action?;
            Some(UiButton {
                id: e.id,
                rect: Rect::new(
                    BUTTON_RIGHT - 320.0,
                    base_y + i as f32 * spacing,
                    320.0,
                    spacing,
                ),
                action: act,
                enabled: true,
                hover_sfx: Some("PopTwoSound"),
            })
        })
        .collect();

    if save.has_profile() {
        out.push(UiButton {
            id: "btn_change_profile",
            rect: Rect::new(560.0, 44.0, 220.0, 26.0),
            action: UiAction::OpenProfileModal,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }
    out.push(UiButton {
        id: "btn_mmg",
        rect: Rect::new(575.0, 566.0, 220.0, 28.0),
        action: UiAction::OpenMmgNotice,
        enabled: true,
        hover_sfx: None,
    });
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
    submenu_age: f32,
    hiding_age: Option<f32>,
    mouse_pos: (f32, f32),
) {
    backgrounds.draw_night_hills_rear(canvas, assets, bank, bg_anim);

    let ticker_alpha = if let Some(h) = hiding_age {
        (1.0 - h / 1.4).clamp(0.0, 1.0)
    } else {
        ((title_age - 5.0) / 2.0).clamp(0.0, 1.0)
    };
    if ticker_alpha > 0.01 {
        let tw = fonts.measure_width(FontFamily::Neuropol, TICKER_TEXT, 18.0);
        let pos_x = 5.0 - ((title_age * 30.0) % (tw * 2.0 + 380.0));
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
                Some((0x05, 0x0D, 0x2B)),
                TextAlign::Left,
                TICKER_TEXT,
                ticker_alpha,
            );
        }
    }

    backgrounds.draw_night_hills_front(canvas, assets, bank, bg_anim);

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
    let is_settled = hiding_age.is_none() && title_age >= 4.5;
    for (i, entry) in entries.iter().enumerate() {
        if entry.label.is_empty() {
            continue;
        }
        let y = base_y + i as f32 * spacing;
        let hit = Rect::new(BUTTON_RIGHT - 320.0, y, 320.0, spacing);
        let hovered = is_settled && hit.contains(mouse_pos.0, mouse_pos.1);
        if hovered && !entry.info.is_empty() {
            hovered_info = Some(entry.info);
        }

        let (bx, b_alpha, blur) = if let Some(h) = hiding_age {
            let u = ((h - i as f32 * 0.1) / 1.3).clamp(0.0, 1.0);
            (
                BUTTON_RIGHT + (-260.0 - BUTTON_RIGHT) * back_in(u),
                1.0,
                (0.0, 0.0),
            )
        } else if submenu == TitleSubmenu::Main && title_age < 6.0 {
            let delay = 4.0 + i as f32 * 0.1;
            let u = ((title_age - delay) / 1.5).clamp(0.0, 1.0);
            if title_age < delay {
                (-260.0, 0.0, (0.0, 0.0))
            } else {
                (
                    -260.0 + (BUTTON_RIGHT + 260.0) * back_out(u),
                    1.0,
                    (0.0, 0.0),
                )
            }
        } else {
            let u = (submenu_age / 0.5).clamp(0.0, 1.0);
            let b = (1.0 - u) * 16.0;
            (BUTTON_RIGHT, u, (b, b))
        };

        draw_text_button_blurred(
            canvas,
            fonts,
            bx,
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
            Some((0x05, 0x0D, 0x2B)),
            TextAlign::Right,
            450.0,
            info,
            1.0,
        );
    }

    let mmg_alpha = if let Some(h) = hiding_age {
        (1.0 - h / 1.5).clamp(0.0, 1.0)
    } else {
        ((title_age - 5.2) / 2.0).clamp(0.0, 1.0)
    };
    let mmg_hov =
        is_settled && Rect::new(575.0, 566.0, 220.0, 28.0).contains(mouse_pos.0, mouse_pos.1);
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

    if let Some(prof) = save.active_profile() {
        let p_alpha = if let Some(h) = hiding_age {
            (1.0 - h / 1.5).clamp(0.0, 1.0)
        } else {
            ((title_age - 5.9) / 0.5).clamp(0.0, 1.0)
        };
        let greet = if prof.has_saved_game() {
            format!("Welcome back, {}!", prof.name)
        } else {
            format!("Welcome, {}!", prof.name)
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
            Some((0x05, 0x0D, 0x2B)),
            TextAlign::Right,
            &greet,
            p_alpha,
        );
        let ch_hov =
            is_settled && Rect::new(560.0, 44.0, 220.0, 26.0).contains(mouse_pos.0, mouse_pos.1);
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
            p_alpha,
        );
    }
}
