// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::game::{AwardKind, Trophy};
use crate::render::{
    Canvas, FontFamily, FontSet, GlowPreset, LOGICAL_H, LOGICAL_W, Rect, TexBank, TextAlign,
};
use crate::save::SaveData;
use crate::ui::widgets::{
    UiAction, UiButton, draw_modal_bubble, draw_modal_button, draw_text_button,
};

const PER_PAGE: usize = 26;
const MODAL_X: f32 = 42.0;
const MODAL_Y: f32 = 34.0;
const MODAL_W: f32 = 716.0;
const MODAL_H: f32 = 532.0;
const GRID_X: f32 = MODAL_X + 15.0;
const GRID_Y: f32 = MODAL_Y + 56.0;

fn slot_rect(i: usize, btn_w: f32, btn_h: f32) -> Rect {
    let cols = 7usize;
    let c = i % cols;
    let r = i / cols;
    Rect::new(
        GRID_X + c as f32 * (btn_w + 4.0),
        GRID_Y + r as f32 * (btn_h + 4.0),
        btn_w,
        btn_h,
    )
}

fn slot_dims(bank: &TexBank) -> (f32, f32) {
    if let Some((_, bw, bh)) = bank.named("NoTrophyBitmap") {
        (bw as f32 * 0.7, bh as f32 * 0.7)
    } else {
        (86.0, 86.0)
    }
}

pub fn build_trophy_room_buttons(
    assets: &AssetBundle,
    bank: &TexBank,
    save: &SaveData,
    page: usize,
) -> Vec<UiButton> {
    let mut out = Vec::new();
    let total = assets.campaign_levels.len();
    let total_slots = total + 4;
    let start = page * PER_PAGE;
    let (bw, bh) = slot_dims(bank);
    let prof = save.active_profile();

    for i in 0..PER_PAGE {
        let slot = start + i;
        if slot >= total_slots {
            break;
        }
        if slot < total {
            let unlocked = prof.is_some_and(|p| p.is_level_unlocked(slot));
            out.push(UiButton {
                id: "tr_level",
                rect: slot_rect(i, bw, bh),
                action: UiAction::TrophyPickLevel(slot),
                enabled: unlocked,
                hover_sfx: Some("ClickOneSound"),
            });
        }
    }

    let pages = total_slots.div_ceil(PER_PAGE);
    if pages > 1 {
        let mx = GRID_X + 7.0 * (bw + 4.0) - 185.0;
        let my = GRID_Y + 4.0 * (bh + 4.0) - 34.0;
        out.push(UiButton {
            id: "tr_more",
            rect: Rect::new(mx, my, 185.0, 32.0),
            action: UiAction::TrophyNextPage,
            enabled: true,
            hover_sfx: Some("PopTwoSound"),
        });
    }
    let done_x = MODAL_X + MODAL_W - 15.0 - 94.0;
    let done_y = MODAL_Y + MODAL_H - 15.0 - 53.0;
    out.push(UiButton {
        id: "tr_done",
        rect: Rect::new(done_x, done_y, 94.0, 53.0),
        action: UiAction::CloseTopModal,
        enabled: true,
        hover_sfx: None,
    });
    out
}

pub fn render_trophy_room(
    canvas: &mut Canvas,
    assets: &AssetBundle,
    bank: &TexBank,
    fonts: &FontSet,
    save: &SaveData,
    page: usize,
    mouse_pos: (f32, f32),
) {
    canvas.fill_rect_alpha(
        Rect::new(0.0, 0.0, LOGICAL_W as f32, LOGICAL_H as f32),
        0,
        0,
        0,
        105,
    );
    draw_modal_bubble(canvas, Rect::new(MODAL_X, MODAL_Y, MODAL_W, MODAL_H));
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        GRID_X + 66.0,
        MODAL_Y + 15.0,
        24.0,
        (255, 255, 255),
        TextAlign::Left,
        "-Pick a level to try for a new Trophy!-",
    );

    let total = assets.campaign_levels.len();
    let total_slots = total + 4;
    let start = page * PER_PAGE;
    let (bw, bh) = slot_dims(bank);
    let prof = save.active_profile();
    let has_save = prof.is_some_and(|p| p.has_saved_game());

    enum HoverCard {
        Level(usize, String, u32, Trophy),
        Locked(usize),
        Award(AwardKind),
    }
    let mut hovered: Option<HoverCard> = None;

    for i in 0..PER_PAGE {
        let slot = start + i;
        if slot >= total_slots {
            break;
        }
        let rect = slot_rect(i, bw, bh);
        let is_hov = rect.contains(mouse_pos.0, mouse_pos.1);

        if slot < total {
            let rec = prof.and_then(|p| p.records.get(&slot));
            let trophy = rec.map(|r| r.best_trophy).unwrap_or(Trophy::None);
            let unlocked = prof.is_some_and(|p| p.is_level_unlocked(slot));

            if is_hov {
                hovered = if let Some(r) = rec {
                    Some(HoverCard::Level(
                        slot,
                        assets.campaign_levels[slot].title.clone(),
                        r.best_score,
                        r.best_trophy,
                    ))
                } else if unlocked {
                    Some(HoverCard::Level(
                        slot,
                        assets.campaign_levels[slot].title.clone(),
                        0,
                        Trophy::None,
                    ))
                } else {
                    Some(HoverCard::Locked(slot))
                };
            }
            draw_trophy_slot(canvas, bank, rect, trophy.bitmap_name(), is_hov);
            fonts.draw_stroked_plain(
                canvas,
                FontFamily::Neuropol,
                rect.x + bw * 0.5 + 6.0,
                rect.y + bh - 22.0,
                18.0,
                (255, 255, 255),
                (5, 13, 43),
                TextAlign::Left,
                &format!("{}", slot + 1),
            );
            if !unlocked && let Some((lid, lw, lh)) = bank.named("LockIcon") {
                canvas.draw_tex_rect(
                    lid,
                    Rect::new(
                        rect.x + 10.0,
                        rect.y + bh - lh as f32 - 3.0,
                        lw as f32,
                        lh as f32,
                    ),
                    None,
                    1.0,
                    1.0,
                );
            }
        } else {
            let award_slot = slot - total;
            let earned = prof.is_some_and(|p| p.awards[award_slot]);
            let kind = AwardKind::from_slot(award_slot, earned);
            if is_hov {
                hovered = Some(HoverCard::Award(kind));
            }
            draw_trophy_slot(canvas, bank, rect, kind.bitmap_name(), is_hov);
        }
    }

    let pages = total_slots.div_ceil(PER_PAGE);
    if pages > 1 {
        let mx = GRID_X + 7.0 * (bw + 4.0) - 4.0;
        let my = GRID_Y + 4.0 * (bh + 4.0) - 32.0;
        let mr = Rect::new(mx - 185.0, my, 185.0, 32.0);
        draw_text_button(
            canvas,
            fonts,
            mx,
            my,
            24.0,
            TextAlign::Right,
            "More levels...",
            mr.contains(mouse_pos.0, mouse_pos.1),
            None,
            1.0,
        );
    }

    let total_pts = prof.map(|p| p.total_score()).unwrap_or(0);
    let done_x = MODAL_X + MODAL_W - 15.0 - 94.0;
    let done_y = MODAL_Y + MODAL_H - 15.0 - 53.0;
    fonts.draw(
        canvas,
        FontFamily::Neuropol,
        GRID_X,
        done_y + 18.0,
        22.0,
        (255, 255, 255),
        TextAlign::Left,
        &format!("Your Score: {}", total_pts),
    );

    let r_done = Rect::new(done_x, done_y, 94.0, 53.0);
    draw_modal_button(
        canvas,
        bank,
        fonts,
        r_done,
        "done",
        r_done.contains(mouse_pos.0, mouse_pos.1),
        true,
    );

    if let Some(card) = hovered {
        let bx = (mouse_pos.0 + 35.0).min(730.0 - 15.0 - 165.0);
        let by = (mouse_pos.1 - 38.0).max(20.0);
        let box_r = Rect::new(bx, by, 165.0, 80.0);
        canvas.draw_rounded_rect(box_r, 4.0, (0, 0, 0, 212), Some((180, 190, 200, 180, 1.5)));
        match card {
            HoverCard::Award(kind) => {
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 6.0,
                    16.0,
                    (255, 255, 0),
                    TextAlign::Center,
                    kind.id(),
                );
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 32.0,
                    13.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    kind.descrip1().trim(),
                );
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 50.0,
                    13.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    kind.descrip2().trim(),
                );
            }
            HoverCard::Level(idx, title, score, trophy) => {
                let played = prof.is_some_and(|p| p.records.contains_key(&idx));
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 6.0,
                    16.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    &format!("-{}-", title),
                );
                if played {
                    fonts.draw(
                        canvas,
                        FontFamily::Neuropol,
                        bx + 8.0,
                        by + 32.0,
                        14.0,
                        (255, 255, 255),
                        TextAlign::Left,
                        "trophy :",
                    );
                    fonts.draw(
                        canvas,
                        FontFamily::Neuropol,
                        bx + 78.0,
                        by + 32.0,
                        14.0,
                        trophy.status_color_rgb(),
                        TextAlign::Left,
                        trophy.status_label(),
                    );
                    fonts.draw(
                        canvas,
                        FontFamily::Neuropol,
                        bx + 8.0,
                        by + 52.0,
                        14.0,
                        (255, 255, 255),
                        TextAlign::Left,
                        &format!("score : {}", score),
                    );
                } else {
                    fonts.draw(
                        canvas,
                        FontFamily::Neuropol,
                        bx + 8.0,
                        by + 32.0,
                        14.0,
                        (255, 255, 255),
                        TextAlign::Left,
                        "trophy : ?",
                    );
                    fonts.draw(
                        canvas,
                        FontFamily::Neuropol,
                        bx + 8.0,
                        by + 52.0,
                        14.0,
                        (255, 255, 255),
                        TextAlign::Left,
                        "score : ?",
                    );
                }
            }
            HoverCard::Locked(idx) => {
                let line1 = if idx == 0 && !has_save {
                    "start a game to".to_string()
                } else {
                    format!("beat level {} to", idx)
                };
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 6.0,
                    17.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    "-LOCKED-",
                );
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 32.0,
                    14.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    &line1,
                );
                fonts.draw(
                    canvas,
                    FontFamily::Neuropol,
                    bx + 82.5,
                    by + 52.0,
                    14.0,
                    (255, 255, 255),
                    TextAlign::Center,
                    "unlock this level",
                );
            }
        }
    }
}

fn draw_trophy_slot(canvas: &mut Canvas, bank: &TexBank, rect: Rect, bmp_key: &str, hovered: bool) {
    let preset = if hovered {
        GlowPreset::TrophyYellow
    } else {
        GlowPreset::TrophyShadow
    };
    let Some((tid, bw, bh)) = bank.named(bmp_key) else {
        return;
    };
    if let Some((fx, gw, gh, pad)) = bank.glow_named(bmp_key, preset) {
        let sx = rect.w / bw.max(1) as f32;
        let sy = rect.h / bh.max(1) as f32;
        canvas.draw_tex_rect(
            fx,
            Rect::new(
                rect.x - pad as f32 * sx,
                rect.y - pad as f32 * sy,
                gw as f32 * sx,
                gh as f32 * sy,
            ),
            None,
            1.0,
            1.0,
        );
    }
    canvas.draw_tex_rect(tid, rect, None, 1.0, 1.0);
}
