// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::canvas::{Canvas, LOGICAL_H, LOGICAL_W};
use crate::render::easing::{back_in, back_out, quad_out, sine_out, strong_in};
use crate::render::particles::FireworkBurst;
use crate::render::tex_bank::TexBank;
use crate::render::text::{FontFamily, FontSet, TextAlign, TextOutline};

#[derive(Debug, Clone)]
pub struct ScoreBlurbFx {
    pub x: f32,
    pub y: f32,
    pub text: String,
    pub age: f32,
}

impl ScoreBlurbFx {
    pub fn is_done(&self) -> bool {
        self.age >= 0.9
    }
}

/// Renders the 6-stripe Bezier rainbow arch (`(0,550) -> (200,220) -> (800,140)` with control point `(0,95)`).
pub fn draw_rainbow(canvas: &mut Canvas, bank: &TexBank, progress: f32, alpha: f32) {
    let t_end = strong_in(progress).clamp(0.01, 1.0);
    canvas.draw_rainbow_tex(bank.rainbow_tex(), t_end, alpha);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SpanStyle {
    color: (u8, u8, u8),
    italic: bool,
}

#[derive(Debug, Clone)]
struct StyledSpan {
    text: String,
    style: SpanStyle,
}

fn parse_hex_color_attr(inner: &str) -> Option<(u8, u8, u8)> {
    let lower = inner.to_ascii_lowercase();
    let idx = lower.find("color=")?;
    let val = inner[idx + 6..].trim_start();
    let raw = if let Some(s) = val.strip_prefix('"') {
        s.split('"').next()?
    } else if let Some(s) = val.strip_prefix('\'') {
        s.split('\'').next()?
    } else {
        val.split(|c: char| c.is_ascii_whitespace() || c == '>')
            .next()?
    };
    let hex = raw.trim().strip_prefix('#').unwrap_or(raw.trim());
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

fn push_styled_fragment(line: &mut Vec<StyledSpan>, style: SpanStyle, frag: &str) {
    if frag.is_empty() {
        return;
    }
    if let Some(last) = line.last_mut()
        && last.style == style
    {
        last.text.push_str(frag);
        return;
    }
    line.push(StyledSpan {
        text: frag.to_string(),
        style,
    });
}

fn parse_tutorial_paragraphs(text: &str) -> Vec<Vec<StyledSpan>> {
    let normalized = text
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<BR>", "\n")
        .replace("<BR/>", "\n")
        .replace("<BR />", "\n")
        .replace("\r\n", "\n")
        .replace('\r', "\n");

    let mut paragraphs: Vec<Vec<StyledSpan>> = vec![Vec::new()];
    let mut color_stack: Vec<(u8, u8, u8)> = vec![(0, 0, 0)];
    let mut italic_depth: usize = 0;
    let mut buf = String::new();
    let mut chars = normalized.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\n' {
            if !buf.is_empty() {
                let st = SpanStyle {
                    color: *color_stack.last().unwrap_or(&(0, 0, 0)),
                    italic: italic_depth > 0,
                };
                push_styled_fragment(paragraphs.last_mut().unwrap(), st, &buf);
                buf.clear();
            }
            paragraphs.push(Vec::new());
            continue;
        }
        if ch == '<' {
            let mut tag = String::new();
            let mut closed = false;
            for tc in chars.by_ref() {
                if tc == '>' {
                    closed = true;
                    break;
                }
                tag.push(tc);
            }
            if !closed {
                buf.push('<');
                buf.push_str(&tag);
                continue;
            }
            if !buf.is_empty() {
                let st = SpanStyle {
                    color: *color_stack.last().unwrap_or(&(0, 0, 0)),
                    italic: italic_depth > 0,
                };
                push_styled_fragment(paragraphs.last_mut().unwrap(), st, &buf);
                buf.clear();
            }
            let inner = tag.trim();
            if inner.eq_ignore_ascii_case("i") {
                italic_depth += 1;
            } else if inner.eq_ignore_ascii_case("/i") {
                italic_depth = italic_depth.saturating_sub(1);
            } else if inner.eq_ignore_ascii_case("/font") {
                if color_stack.len() > 1 {
                    color_stack.pop();
                }
            } else if inner.len() >= 4
                && inner[..4].eq_ignore_ascii_case("font")
                && inner[4..].starts_with(|c: char| c.is_ascii_whitespace())
                && let Some(rgb) = parse_hex_color_attr(&inner[4..])
            {
                color_stack.push(rgb);
            }
            continue;
        }
        buf.push(ch);
    }

    if !buf.is_empty() {
        let st = SpanStyle {
            color: *color_stack.last().unwrap_or(&(0, 0, 0)),
            italic: italic_depth > 0,
        };
        push_styled_fragment(paragraphs.last_mut().unwrap(), st, &buf);
    }
    paragraphs
}

fn split_paragraph_words(spans: &[StyledSpan]) -> Vec<Vec<StyledSpan>> {
    let mut words: Vec<Vec<StyledSpan>> = Vec::new();
    let mut cur_word: Vec<StyledSpan> = Vec::new();
    for sp in spans {
        let mut frag = String::new();
        for ch in sp.text.chars() {
            if ch.is_whitespace() {
                if !frag.is_empty() {
                    push_styled_fragment(&mut cur_word, sp.style, &frag);
                    frag.clear();
                }
                if !cur_word.is_empty() {
                    words.push(std::mem::take(&mut cur_word));
                }
            } else {
                frag.push(ch);
            }
        }
        if !frag.is_empty() {
            push_styled_fragment(&mut cur_word, sp.style, &frag);
        }
    }
    if !cur_word.is_empty() {
        words.push(cur_word);
    }
    words
}

fn wrap_styled_lines(
    fonts: &FontSet,
    fam: FontFamily,
    paragraphs: &[Vec<StyledSpan>],
    size: f32,
    max_w: f32,
) -> Vec<Vec<StyledSpan>> {
    let space_w = fonts.measure_width(fam, " ", size);
    let mut lines: Vec<Vec<StyledSpan>> = Vec::new();
    for para in paragraphs {
        let words = split_paragraph_words(para);
        if words.is_empty() {
            lines.push(Vec::new());
            continue;
        }
        let mut cur_line: Vec<StyledSpan> = Vec::new();
        let mut cur_w = 0.0f32;
        for word in words {
            let word_w: f32 = word
                .iter()
                .map(|sp| fonts.measure_width(fam, &sp.text, size))
                .sum();
            if cur_line.is_empty() {
                cur_line = word;
                cur_w = word_w;
            } else if cur_w + space_w + word_w <= max_w {
                let space_style = cur_line.last().map(|sp| sp.style).unwrap_or(SpanStyle {
                    color: (0, 0, 0),
                    italic: false,
                });
                push_styled_fragment(&mut cur_line, space_style, " ");
                for sp in word {
                    push_styled_fragment(&mut cur_line, sp.style, &sp.text);
                }
                cur_w += space_w + word_w;
            } else {
                lines.push(cur_line);
                cur_line = word;
                cur_w = word_w;
            }
        }
        if !cur_line.is_empty() {
            lines.push(cur_line);
        }
    }
    if lines.is_empty() {
        lines.push(Vec::new());
    }
    lines
}

/// Renders tutorial bubble: rounded `0xEEEEEE` box, `4px` black stroke, inner bevel, narrow tail,
/// anchor-scaled `Back.easeOut` pop-in after `0.2s` delay, and styled `Helvetica` 18px text.
#[allow(clippy::too_many_arguments)]
pub fn draw_tutorial_bubble(
    canvas: &mut Canvas,
    fonts: &FontSet,
    target_cx: f32,
    target_cy: f32,
    text: &str,
    has_click_to_continue: bool,
    age: f32,
) -> (f32, f32, f32, f32) {
    let paragraphs = parse_tutorial_paragraphs(text);
    let mut plain = String::new();
    for (idx, para) in paragraphs.iter().enumerate() {
        if idx > 0 {
            plain.push('\n');
        }
        for sp in para {
            plain.push_str(&sp.text);
        }
    }
    let has_whirl = plain.contains("whirl");
    let narrow = has_whirl || plain.contains("FASTFORWARD") || plain.contains("click here");
    let width = if narrow { 180.0 } else { 250.0 };
    let text_w = if narrow { 160.0 } else { 230.0 };
    let lines = wrap_styled_lines(fonts, FontFamily::Helvetica, &paragraphs, 18.0, text_w);

    let line_h = 21.0;
    let body_h = (lines.len().max(1) as f32) * line_h;
    let text_h = body_h + if has_click_to_continue { 40.0 } else { 0.0 };
    let height = (text_h + 25.0).max(58.0);
    let tri_h = width / 4.0;
    let tail_hw = height / 20.0;

    let raw_cx = target_cx - if has_whirl { 26.0 } else { 0.0 };
    let raw_cy = target_cy - height / 1.3 - tri_h;
    let final_cx = raw_cx.clamp(width * 0.5 + 4.0, LOGICAL_W as f32 - width * 0.5 - 4.0);
    let final_cy = raw_cy.clamp(height * 0.5 + 4.0, LOGICAL_H as f32 - height * 0.5 - 30.0);
    let off_x = final_cx - target_cx;
    let off_y = final_cy - target_cy;

    if age < 0.2 {
        return (target_cx, target_cy, 0.0, 0.0);
    }
    let pop = back_out(((age - 0.2) / 0.6).clamp(0.0, 1.0)).max(0.0);
    if pop <= 0.01 {
        return (target_cx, target_cy, 0.0, 0.0);
    }

    let bw = width * pop;
    let bh = height * pop;
    let b_cx = target_cx + off_x * pop;
    let b_cy = target_cy + off_y * pop;
    let bx = b_cx - bw * 0.5;
    let by = b_cy - bh * 0.5;
    let r = (17.5 * pop).min(bw * 0.45).min(bh * 0.45);
    canvas.draw_tutorial_bubble(
        bx,
        by,
        bw,
        bh,
        r,
        4.0 * pop,
        b_cx,
        tail_hw * pop,
        tri_h * pop,
        pop,
    );

    let local_top = -text_h * 0.5;
    for (i, line) in lines.iter().enumerate() {
        let lw: f32 = line
            .iter()
            .map(|sp| fonts.measure_width(FontFamily::Helvetica, &sp.text, 18.0))
            .sum();
        let mut cur_x = b_cx - lw * 0.5 * pop;
        let line_y = b_cy + (local_top + i as f32 * line_h) * pop;
        for sp in line {
            fonts.draw_styled_span_fx(
                canvas,
                FontFamily::Helvetica,
                cur_x,
                line_y,
                18.0,
                pop,
                sp.style.color,
                sp.style.italic,
                &sp.text,
            );
            cur_x += fonts.measure_width(FontFamily::Helvetica, &sp.text, 18.0) * pop;
        }
    }
    if has_click_to_continue {
        let ctc = "Click to continue";
        let ctc_w = fonts.measure_width(FontFamily::Helvetica, ctc, 16.0);
        fonts.draw_styled_span_fx(
            canvas,
            FontFamily::Helvetica,
            b_cx - ctc_w * 0.5 * pop,
            b_cy + (local_top + body_h + line_h) * pop,
            16.0,
            pop,
            (0x40, 0x9F, 0xFF),
            false,
            ctc,
        );
    }
    (bx, by, bw, bh)
}

pub fn draw_fireworks(
    canvas: &mut Canvas,
    bank: &TexBank,
    fireworks: &[FireworkBurst],
    fw_alpha: f32,
) {
    for f in fireworks {
        f.draw(canvas, bank, fw_alpha);
    }
}

pub fn draw_score_blurbs(canvas: &mut Canvas, fonts: &FontSet, blurbs: &[ScoreBlurbFx]) {
    for b in blurbs {
        let c_mix = quad_out((b.age / 2.0).clamp(0.0, 1.0));
        let col = (
            (228.0 + (255.0 - 228.0) * c_mix) as u8,
            (233.0 + (255.0 - 233.0) * c_mix) as u8,
            (237.0 + (255.0 - 237.0) * c_mix) as u8,
        );
        let (y_off, scale, alpha, glow_blur, glow_a, blur) = if b.age < 0.6 {
            let u = (b.age / 0.6).clamp(0.0, 1.0);
            let e = sine_out(u);
            let a = 1.0 - 0.5 * e;
            (-60.0 * e, 1.0 + 0.3 * e, a, 32.0 * e, 0.55 * a, (0.0, 0.0))
        } else {
            let u = ((b.age - 0.6) / 0.3).clamp(0.0, 1.0);
            let e = back_in(u);
            let a = (0.5 * (1.0 - e)).clamp(0.0, 1.0);
            let b_px = (16.0 * e).max(0.0);
            (
                -60.0,
                (1.3 + (0.1 - 1.3) * e).max(0.05),
                a,
                32.0,
                0.55 * a,
                (b_px, b_px),
            )
        };
        let base_fsz = 40.0;
        let y_pos = b.y + y_off - base_fsz * scale * 0.5;
        let tint_rgb = [
            col.0 as f32 / 255.0,
            col.1 as f32 / 255.0,
            col.2 as f32 / 255.0,
        ];
        if glow_a > 0.01 {
            fonts.draw_glow_alpha(
                canvas,
                FontFamily::Pricedown,
                b.x,
                y_pos,
                base_fsz,
                scale,
                TextAlign::Center,
                &b.text,
                (255, 255, 255),
                Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
                (255, 255, 255),
                glow_blur,
                glow_a,
            );
        }
        fonts.draw_fx(
            canvas,
            FontFamily::Pricedown,
            b.x,
            y_pos,
            base_fsz,
            1.0,
            1.0,
            scale,
            scale,
            (255, 255, 255),
            Some(TextOutline::stroke((0x05, 0x0D, 0x2B))),
            TextAlign::Center,
            &b.text,
            1.0,
            alpha,
            tint_rgb,
            blur,
        );
    }
}
