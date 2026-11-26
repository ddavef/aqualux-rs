// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::{Canvas, FontFamily, FontSet, LOGICAL_H, LOGICAL_W, Rect, TexBank, TextAlign};
use crate::ui::widgets::{UiAction, UiButton, draw_modal_bubble, draw_modal_button};

const PM_W: f32 = 436.0;
const PM_H: f32 = 345.0;
const PM_X: f32 = (LOGICAL_W as f32 - PM_W) * 0.5;
const PM_Y: f32 = (LOGICAL_H as f32 - PM_H) * 0.5;

pub fn build_profile_modal_buttons(item_count: usize) -> Vec<UiButton> {
    let mut out = Vec::new();
    let lx = PM_X + 23.0;
    let ly = PM_Y + 50.0;
    for i in 0..item_count.min(9) {
        out.push(UiButton {
            id: "prof_item",
            rect: Rect::new(lx + 2.0, ly + 2.0 + i as f32 * 21.0, 386.0, 21.0),
            action: UiAction::ProfileSelectIdx(i),
            enabled: true,
            hover_sfx: None,
        });
    }
    let btn_y = ly + 200.0 + 30.0;
    let btn_x0 = PM_X + 15.0;
    let acts = [
        ("pm_accept", UiAction::ProfileAccept),
        ("pm_new", UiAction::ProfileNew),
        ("pm_del", UiAction::ProfileDelete),
        ("pm_cancel", UiAction::CloseTopModal),
    ];
    for (i, (id, act)) in acts.into_iter().enumerate() {
        out.push(UiButton {
            id,
            rect: Rect::new(btn_x0 + i as f32 * 104.0, btn_y, 94.0, 53.0),
            action: act,
            enabled: true,
            hover_sfx: None,
        });
    }
    out
}

pub fn render_profile_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    items: &[String],
    selected: usize,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(PM_X, PM_Y, PM_W, PM_H));
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        400.0,
        PM_Y + 15.0,
        24.0,
        (255, 255, 255),
        TextAlign::Center,
        "-Choose Profile-",
    );

    let lx = PM_X + 23.0;
    let ly = PM_Y + 50.0;
    canvas.draw_rounded_rect(
        Rect::new(lx, ly, 390.0, 200.0),
        2.0,
        (0x05, 0x0D, 0x2B, 204),
        Some((0x0E, 0x15, 0x32, 255, 3.0)),
    );

    for (i, label) in items.iter().take(9).enumerate() {
        let ry = ly + 2.0 + i as f32 * 21.0;
        let row_r = Rect::new(lx + 2.0, ry, 386.0, 21.0);
        let active = i == selected;
        if active {
            canvas.fill_rect_alpha(row_r, 0x59, 0x95, 0xC1, 230);
        } else if row_r.contains(mouse_pos.0, mouse_pos.1) {
            canvas.fill_rect_alpha(row_r, 0x59, 0x95, 0xC1, 90);
        }
        fonts.draw(
            canvas,
            FontFamily::Neuropol,
            lx + 8.0,
            ry + 1.0,
            18.0,
            (255, 255, 255),
            TextAlign::Left,
            label,
        );
    }

    let btn_y = ly + 200.0 + 30.0;
    let btn_x0 = PM_X + 15.0;
    for (i, lbl) in ["accept", "New", "Delete", "Cancel"]
        .into_iter()
        .enumerate()
    {
        let r = Rect::new(btn_x0 + i as f32 * 104.0, btn_y, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r,
            lbl,
            r.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    }
}

pub fn enter_name_layout(fonts: &FontSet, prompt: &str) -> (f32, f32, f32, f32) {
    let lines = fonts.wrap_lines(FontFamily::Neuropol, prompt, 22.0, 360.0);
    let prompt_h = (lines.len().max(1) as f32) * 26.0;
    let content_h = prompt_h + 22.0 + 24.0;
    let total_w = 390.0;
    let total_h = 15.0 + content_h + 30.0 + 53.0 + 15.0;
    let bx = (LOGICAL_W as f32 - total_w) * 0.5;
    let by = (LOGICAL_H as f32 - total_h) * 0.5;
    (bx, by, total_w, prompt_h)
}

pub fn build_enter_name_buttons(
    fonts: &FontSet,
    prompt: &str,
    allow_cancel: bool,
) -> Vec<UiButton> {
    let (_bx, by, _tw, prompt_h) = enter_name_layout(fonts, prompt);
    let btn_y = by + 15.0 + prompt_h + 22.0 + 24.0 + 30.0;
    if allow_cancel {
        vec![
            UiButton {
                id: "en_save",
                rect: Rect::new(400.0 - 99.0, btn_y, 94.0, 53.0),
                action: UiAction::SubmitName,
                enabled: true,
                hover_sfx: None,
            },
            UiButton {
                id: "en_cancel",
                rect: Rect::new(405.0, btn_y, 94.0, 53.0),
                action: UiAction::CloseTopModal,
                enabled: true,
                hover_sfx: None,
            },
        ]
    } else {
        vec![UiButton {
            id: "en_save",
            rect: Rect::new(400.0 - 47.0, btn_y, 94.0, 53.0),
            action: UiAction::SubmitName,
            enabled: true,
            hover_sfx: None,
        }]
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_enter_name_modal(
    canvas: &mut Canvas,
    bank: &TexBank,
    fonts: &FontSet,
    prompt: &str,
    input: &str,
    allow_cancel: bool,
    time: f32,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    let (bx, by, total_w, prompt_h) = enter_name_layout(fonts, prompt);
    let total_h = 15.0 + prompt_h + 22.0 + 24.0 + 30.0 + 53.0 + 15.0;
    draw_modal_bubble(canvas, Rect::new(bx, by, total_w, total_h));
    fonts.draw_multiline(
        canvas,
        FontFamily::Neuropol,
        bx + 15.0,
        by + 15.0,
        22.0,
        26.0,
        (255, 255, 255),
        None,
        TextAlign::Left,
        total_w - 30.0,
        prompt,
        1.0,
    );

    let field_y = by + 15.0 + prompt_h + 22.0;
    let field_r = Rect::new(bx + 15.0, field_y, total_w - 30.0, 24.0);
    canvas.draw_rounded_rect(
        field_r,
        2.0,
        (255, 255, 255, 255),
        Some((0, 0, 0, 255, 1.5)),
    );
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        field_r.x + 6.0,
        field_r.y + 3.0,
        15.0,
        (0, 0, 0),
        TextAlign::Left,
        input,
    );
    if ((time * 2.0) as i32) % 2 == 0 {
        let tw = fonts.measure_width(FontFamily::Neuropol, input, 15.0);
        canvas.fill_rect_alpha(
            Rect::new(field_r.x + 8.0 + tw, field_r.y + 4.0, 2.0, 16.0),
            0,
            0,
            0,
            255,
        );
    }

    let btn_y = field_y + 24.0 + 30.0;
    if allow_cancel {
        let r1 = Rect::new(400.0 - 99.0, btn_y, 94.0, 53.0);
        let r2 = Rect::new(405.0, btn_y, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r1,
            "Save",
            r1.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r2,
            "Cancel",
            r2.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    } else {
        let r1 = Rect::new(400.0 - 47.0, btn_y, 94.0, 53.0);
        draw_modal_button(
            canvas,
            bank,
            fonts,
            r1,
            "Save",
            r1.contains(mouse_pos.0, mouse_pos.1),
            true,
        );
    }
}
