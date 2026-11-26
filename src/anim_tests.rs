// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, Matrix2D, RgbaImage, SpriteItem};
use crate::game::types::{LevelDef, TileDef};
use crate::game::{Difficulty, GameColor, LevelSim, PieceShape, PieceType, SimEvent};
use crate::render::{Canvas, LOGICAL_H, LOGICAL_W, TexBank, draw_sprite_title};
use crate::tests::test_gpu;
use std::collections::HashMap;

fn tile_def(
    x: usize,
    y: usize,
    shape: PieceShape,
    piece_type: PieceType,
    direction: u8,
    secondary_color: Option<GameColor>,
) -> TileDef {
    TileDef {
        x,
        y,
        shape,
        piece_type,
        direction,
        is_moveable: piece_type == PieceType::Portal,
        can_rotate: false,
        is_gate: false,
        is_freeze: false,
        is_magic: false,
        secondary_color,
    }
}

fn base_level(
    cols: usize,
    rows: usize,
    start_delay: f32,
    liquid_speed: f32,
    tiles: Vec<TileDef>,
) -> LevelDef {
    LevelDef {
        index: 0,
        title: "AnimAudit".into(),
        description: String::new(),
        background_id: "DaySky".into(),
        liquid_color: GameColor::Blue,
        start_delay,
        liquid_speed,
        rotater_color: GameColor::Green,
        mover_color: GameColor::Orange,
        stationary_color: GameColor::Grey,
        gate_color: GameColor::White,
        freeze_color: GameColor::Blue,
        magic_color: GameColor::Violet,
        par_moves: 1,
        par_pipes: 4,
        par_time_ms: 10_000,
        cols,
        rows,
        tiles,
        tutorials: Vec::new(),
    }
}

#[test]
fn drain_slime_runs_15fps_and_ignores_fast_forward() {
    let def = base_level(
        3,
        1,
        0.0,
        24.0,
        vec![
            tile_def(0, 0, PieceShape::Spigot, PieceType::Source, 3, None),
            tile_def(1, 0, PieceShape::Straight, PieceType::Pipe, 1, None),
            tile_def(2, 0, PieceShape::Drain, PieceType::Drain, 1, None),
        ],
    );
    let mut sim = LevelSim::new(def, Difficulty::Hard);
    sim.start_level();

    let mut blubs = 0usize;
    let dt = 1.0 / 60.0;
    for _ in 0..600 {
        for ev in sim.update(dt) {
            if matches!(ev, SimEvent::PlaySfx("BlubThreeSound")) {
                blubs += 1;
            }
        }
        if sim.tiles[0][2].slime_anim.is_some() {
            break;
        }
    }
    assert_eq!(blubs, 1);
    assert!(sim.tiles[0][2].slime_playing);
    assert!(!sim.tiles[0][2].slime_done);

    // 1.0s at 15fps reaches frame 15 (< 25) and remains unfinished even after fast_forward().
    let _ = sim.fast_forward();
    for _ in 0..60 {
        sim.update(dt);
    }
    assert!(
        !sim.tiles[0][2].slime_done,
        "Drain slime must not finish within 1.0s"
    );
    assert!(!sim.finished);

    // Total > 25/15s (1.667s) finishes the 26-frame animation and wins the level.
    let mut cheered = false;
    for _ in 0..60 {
        for ev in sim.update(dt) {
            if matches!(ev, SimEvent::PlaySfx("CheeringSound")) {
                cheered = true;
            }
        }
        if sim.finished {
            break;
        }
    }
    assert!(sim.tiles[0][2].slime_done);
    assert!(!sim.tiles[0][2].slime_playing);
    assert_eq!(sim.won, Some(true));
    assert!(cheered);
}

#[test]
fn portal_slime_reset_bonus_and_5s_timer_with_no_swish() {
    let def = base_level(
        4,
        1,
        0.0,
        24.0,
        vec![
            tile_def(0, 0, PieceShape::Spigot, PieceType::Source, 3, None),
            tile_def(
                1,
                0,
                PieceShape::Spigot,
                PieceType::Portal,
                1,
                Some(GameColor::Violet),
            ),
            tile_def(
                2,
                0,
                PieceShape::Spigot,
                PieceType::Portal,
                3,
                Some(GameColor::Violet),
            ),
            tile_def(3, 0, PieceShape::Drain, PieceType::Drain, 1, None),
        ],
    );
    let mut sim = LevelSim::new(def, Difficulty::Hard);
    sim.start_level();
    sim.selected = Some((2, 0));

    let dt = 1.0 / 60.0;
    let mut saw_swish = false;
    let mut bonus_sfx_count = 0usize;
    let mut blurbs = Vec::new();
    let mut alarm_count = 0usize;

    for _ in 0..360 {
        for ev in sim.update(dt) {
            match ev {
                SimEvent::PlaySfx("SwishSound") => saw_swish = true,
                SimEvent::PlaySfx("AlarmBeepSound") => alarm_count += 1,
                SimEvent::PlayRandomBonusSfx => bonus_sfx_count += 1,
                SimEvent::ScoreBlurb(bx, by, pts) => blurbs.push((bx, by, pts)),
                _ => {}
            }
        }
        if sim.tiles[0][2].timer.is_some() {
            break;
        }
    }

    assert!(!saw_swish, "Portal transfer must not play SwishSound");
    assert_eq!(
        bonus_sfx_count, 2,
        "Expected bonus SFX on entering Portal A and teleporting to Portal B"
    );
    assert!(
        blurbs
            .iter()
            .any(|&(bx, by, pts)| pts == 100 && (bx - 2.5).abs() < 1e-4 && (by - 0.5).abs() < 1e-4),
        "Portal transfer must spawn +100 ScoreBlurb on destination, got {:?}",
        blurbs
    );
    assert_eq!(sim.score, 200);
    assert_eq!(sim.selected, None, "Destination portal must be deselected");
    assert!(sim.tiles[0][1].slime_done);
    assert!(!sim.tiles[0][1].slime_playing);
    assert_eq!(sim.tiles[0][1].slime_anim, Some(0.0));

    let timer = sim.tiles[0][2]
        .timer
        .expect("Portal B should start 5s timer");
    assert!((timer.remaining - 5.0).abs() < 0.05);

    // Tick down Portal B's 5s timer -> AlarmBeep at < 3s, fades 1.0s while exiting Center.
    for _ in 0..310 {
        for ev in sim.update(dt) {
            if matches!(ev, SimEvent::PlaySfx("AlarmBeepSound")) {
                alarm_count += 1;
            }
            if matches!(ev, SimEvent::PlaySfx("SwishSound")) {
                saw_swish = true;
            }
        }
        if !sim.tiles[0][2].fills.is_empty() {
            break;
        }
    }
    assert!(!saw_swish);
    assert_eq!(alarm_count, 3);
    assert!(sim.tiles[0][2].timer.is_some_and(|t| t.fading));
    for _ in 0..70 {
        sim.update(dt);
    }
    assert!(sim.tiles[0][2].timer.is_none());
}

#[test]
fn twist_sound_plays_once_and_nozzle_swells_then_snaps() {
    let def = base_level(
        3,
        1,
        1.0,
        6.0,
        vec![
            tile_def(0, 0, PieceShape::Spigot, PieceType::Source, 3, None),
            tile_def(1, 0, PieceShape::Straight, PieceType::Pipe, 1, None),
            tile_def(2, 0, PieceShape::Drain, PieceType::Drain, 1, None),
        ],
    );

    // Case 1: natural timer completion emits TwistSound once and starts 1s fade + nozzle swell.
    let mut natural = LevelSim::new(def.clone(), Difficulty::Hard);
    natural.start_level();
    let mut twist_count = 0usize;
    for _ in 0..80 {
        for ev in natural.update(1.0 / 60.0) {
            if matches!(ev, SimEvent::PlaySfx("TwistSound")) {
                twist_count += 1;
            }
        }
    }
    assert_eq!(twist_count, 1);
    assert!(natural.tiles[0][0].timer.is_some_and(|t| t.fading));
    let elapsed_mid = natural.tiles[0][0].fills[0].elapsed;
    assert!(elapsed_mid < 0.99);

    // Case 2: fast_forward while countdown active emits TwistSound once, repeats emit 0, snaps nozzle.
    let mut ff = LevelSim::new(def, Difficulty::Hard);
    ff.start_level();
    let mut ff_twists = 0usize;
    for ev in ff.fast_forward() {
        if matches!(ev, SimEvent::PlaySfx("TwistSound")) {
            ff_twists += 1;
        }
    }
    for ev in ff.fast_forward() {
        if matches!(ev, SimEvent::PlaySfx("TwistSound")) {
            ff_twists += 1;
        }
    }
    assert_eq!(ff_twists, 1);
    assert!(ff.tiles[0][0].timer.is_some_and(|t| t.fading));
    assert!((ff.tiles[0][0].fills[0].elapsed - 1.0).abs() < 1e-4);
}

#[test]
fn title_logo_rises_black_and_fades_from_black() {
    let solid_cyan = |w: u32, h: u32| -> RgbaImage {
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            data.extend_from_slice(&[0, 200, 255, 255]);
        }
        RgbaImage {
            width: w,
            height: h,
            data,
        }
    };

    let mut cid_images = HashMap::new();
    cid_images.insert(1, solid_cyan(40, 40));
    cid_images.insert(2, solid_cyan(20, 20));

    let mut sprites = HashMap::new();
    sprites.insert(
        "SpriteTitle".to_string(),
        vec![
            SpriteItem {
                bitmap_cid: 1,
                layer_name: Some("trueTitle".to_string()),
                depth: 1,
                matrix: Matrix2D::identity(),
            },
            SpriteItem {
                bitmap_cid: 2,
                layer_name: None,
                depth: 2,
                matrix: Matrix2D {
                    a: 1.0,
                    b: 0.0,
                    c: 0.0,
                    d: 1.0,
                    tx: -60.0,
                    ty: 0.0,
                },
            },
        ],
    );

    let bundle = AssetBundle {
        source_label: "synthetic".to_string(),
        images: HashMap::new(),
        cid_images,
        sprites,
        fonts: HashMap::new(),
        sounds: HashMap::new(),
        raw_xmls: HashMap::new(),
        campaign_levels: Vec::new(),
        lotd_levels: Vec::new(),
    };
    let gpu = test_gpu();
    let bank = TexBank::new(gpu.clone(), &bundle);
    let mut canvas = Canvas::new();

    let render_px = |c: &mut Canvas, cx: f32, age: f32| -> Vec<u8> {
        c.begin_frame(1.0);
        c.clear((40, 80, 120));
        draw_sprite_title(c, &bundle, &bank, cx, 200.0, age);
        gpu.render_to_rgba(c, &bank, LOGICAL_W as u32, LOGICAL_H as u32)
    };
    let px_at = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
        let i = (y * LOGICAL_W + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };

    // Rising piece at 3.0s must be a pure black silhouette (no cyan pixels leaked).
    let buf3 = render_px(&mut canvas, 280.0, 3.0);
    assert_eq!(
        px_at(&buf3, 230, 242),
        [0, 0, 0, 255],
        "rising piece should be solid black at 3.0s"
    );
    assert!(
        buf3.as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[0] <= 40 && p[1] <= 80 && p[2] <= 120),
        "all rising logo pixels before 3.5s must be black silhouette or shadow"
    );

    // At 3.5s handoff, trueTitle activates at pure black (0.0 shade).
    let buf35 = render_px(&mut canvas, 280.0, 3.5);
    assert_eq!(px_at(&buf35, 300, 220), [0, 0, 0, 255]);

    // At 4.6s (u = 0.5), quad_out(0.5) = 0.75 fades blue channel near 191 (above linear 128).
    let buf46 = render_px(&mut canvas, 280.0, 4.6);
    let mid_px = px_at(&buf46, 300, 220);
    assert!(
        mid_px[2] > 180 && mid_px[2] < 205,
        "expected quad_out mid-fade ~191, got {:?}",
        mid_px
    );

    // At 6.0s (settled), full cyan + soft drop shadow appear and shift cleanly with cx.
    let buf6 = render_px(&mut canvas, 280.0, 6.0);
    assert_eq!(px_at(&buf6, 300, 220), [0, 200, 255, 255]);
    let sh_px = px_at(&buf6, 324, 220);
    assert!(
        sh_px[2] < 115,
        "expected darkened +X drop shadow pixel over blue bg, got {:?}",
        sh_px
    );

    let buf6_shift = render_px(&mut canvas, 360.0, 6.0);
    assert_eq!(px_at(&buf6_shift, 300, 220), [40, 80, 120, 255]);
    assert_eq!(px_at(&buf6_shift, 380, 220), [0, 200, 255, 255]);
}
