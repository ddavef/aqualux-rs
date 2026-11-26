// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::{AssetBundle, RgbaImage};
use crate::game::scoring::calculate_score;
use crate::game::types::TileDef;
use crate::game::{GameColor, PieceShape, PieceType, Side, Tile};
use crate::render::{
    Canvas, CursorKind, Rect, Renderer, TexBank,
    gpu_bake::{bake_blur_texture, bake_path_highlight_texture, upload_premul_texture},
};
use crate::tests::{asset_file_if_present, empty_bundle, minimal_test_level, test_gpu};
use crate::ui::widgets::ModalState;
use crate::ui::{GameApp, Screen};
use rand::SeedableRng;
use rand::rngs::StdRng;

#[test]
fn real_assets_end_to_end_and_all_screens_render() {
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle =
        AssetBundle::load_from_path(path).expect("real resource.n / aqualux.swf should load");
    assert_eq!(bundle.campaign_levels.len(), 100);
    assert!(!bundle.lotd_levels.is_empty());
    let gpu = test_gpu();
    assert!(Renderer::new(gpu.clone(), &bundle).is_ok());

    let mut app = GameApp::new(bundle, Some(144), gpu.clone()).expect("GameApp init");
    assert_eq!(app.target_fps(), 144);
    let mut canvas = Canvas::new();

    // Splash -> Title -> Modals
    canvas.begin_frame(1.8);
    app.render(&mut canvas);
    app.screen = Screen::Title;
    app.title_age = 6.5;
    canvas.begin_frame(1.8);
    app.render(&mut canvas);

    for m in [
        ModalState::TrophyRoom {
            page: 0,
            hover_idx: None,
        },
        ModalState::Options {
            allow_difficulty: true,
        },
        ModalState::Profile {
            items: vec!["Player 1".into()],
            added: vec![],
            selected: 0,
            hover_slot: None,
        },
        ModalState::EnterName {
            prompt: "Enter your name:".into(),
            input: "Alex".into(),
            allow_cancel: true,
            creating_for_profile_list: false,
            pending_launch_after: false,
        },
    ] {
        app.modals = vec![m];
        canvas.begin_frame(1.8);
        app.render(&mut canvas);
    }
    app.modals.clear();

    // Match & Mirror Match
    for mirror in [false, true] {
        app.start_match_level(1, mirror, mirror);
        app.update(0.1);
        canvas.begin_frame(1.8);
        app.render(&mut canvas);
    }

    // Campaign + Cursor states + Pause + Summary + Champion
    app.start_campaign_level(0, false);
    canvas.begin_frame(1.8);
    app.render(&mut canvas);
    if let Some(sim) = &mut app.campaign_sim {
        let _ = sim.skip_tutorials();
        sim.is_ready = true;
        sim.is_started = true;
        sim.tiles[0][0].is_moveable = false;
        sim.tiles[0][0].can_rotate = true;
    }
    app.board_pop_elapsed = None;
    let layout = crate::render::BoardLayout::compute(
        app.campaign_sim.as_ref().unwrap().level.cols,
        app.campaign_sim.as_ref().unwrap().level.rows,
        crate::ui::hud::campaign_board_area(),
    );
    let (tx, ty) = layout.tile_center(0, 0);
    app.mouse_pos = (tx, ty);
    assert_eq!(app.current_cursor(), CursorKind::Rotate);

    app.screen = Screen::Paused;
    canvas.begin_frame(1.8);
    app.render(&mut canvas);
    let mut rng = StdRng::seed_from_u64(99);
    app.summary_state = Some(crate::ui::summary::SummaryState::new(
        calculate_score(4, 5, 6000, 4, 5, 8000, true),
        1000,
        2400,
    ));
    app.screen = Screen::Summary;
    canvas.begin_frame(1.8);
    app.render(&mut canvas);
    app.champion_state = Some(crate::ui::champion::ChampionState::new(
        125_000, true, &mut rng,
    ));
    app.screen = Screen::Champion;
    canvas.begin_frame(1.8);
    app.render(&mut canvas);
    let px = gpu.render_to_rgba(&canvas, &app.renderer.bank, 800, 600);
    assert!(
        px.as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[0] > 40 || p[1] > 40 || p[2] > 40)
    );
}

#[test]
fn gpu_sprite_tint_and_premul_blend_parity() {
    let gpu = test_gpu();
    let mut b = empty_bundle();
    b.images.insert(
        "pipe_base".into(),
        RgbaImage {
            width: 4,
            height: 4,
            data: [180u8, 140, 20, 255].repeat(16),
        },
    );
    b.images.insert(
        "fg".into(),
        RgbaImage {
            width: 4,
            height: 4,
            data: [200u8, 100, 50, 200].repeat(16),
        },
    );
    let bank = TexBank::new(gpu.clone(), &b);
    let mut c = Canvas::new();
    c.clear((0, 0, 0));
    let gid = bank.named("pipe_base").unwrap().0;
    c.draw_tex_rect(
        gid,
        Rect::new(0.0, 0.0, 200.0, 600.0),
        Some((GameColor::Blue, false)),
        1.0,
        1.0,
    );
    c.draw_tex_rect(
        gid,
        Rect::new(200.0, 0.0, 200.0, 600.0),
        Some((GameColor::Blue, true)),
        1.0,
        1.0,
    );
    c.fill_rect_alpha(Rect::new(400.0, 0.0, 400.0, 600.0), 100, 150, 220, 255);
    c.draw_tex_rect(
        bank.named("fg").unwrap().0,
        Rect::new(400.0, 0.0, 400.0, 600.0),
        None,
        1.0,
        0.5,
    );
    let out = gpu.render_to_rgba(&c, &bank, 800, 600);
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 800 + x) * 4;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    };
    let blue_dim = px(100, 300);
    let blue_bri = px(300, 300);
    assert!(blue_dim[2] > blue_dim[0] && blue_bri[2] > blue_dim[2]);
    let blend = px(600, 300);
    assert!(
        blend[0].abs_diff(139) <= 2 && blend[1].abs_diff(130) <= 2 && blend[2].abs_diff(153) <= 2,
        "got {:?}",
        blend
    );
}

#[test]
fn pipe_filters_and_sign_parity() {
    let gpu = test_gpu();
    let src_img = RgbaImage {
        width: 32,
        height: 32,
        data: [200u8, 100, 50, 255].repeat(32 * 32),
    };
    let src_tex = upload_premul_texture(&gpu, &src_img);
    let glow = bake_blur_texture(&gpu, &src_tex, 0.0, 0.0, (255, 255, 0), 1.0, 8.0, 2.0, 8);
    assert_eq!((glow.width, glow.height), (48, 48));
    let sh = bake_blur_texture(
        &gpu,
        &src_tex,
        4.0,
        45.0,
        (0x17, 0x16, 0x29),
        1.0,
        16.0,
        1.0,
        12,
    );
    assert_eq!((sh.width, sh.height), (56, 56));
    let ph = bake_path_highlight_texture(&gpu, Side::Left, Side::Right, 16);
    assert_eq!((ph.width, ph.height), (160, 160));
    let bank = TexBank::new(gpu.clone(), &empty_bundle());
    let mut c = Canvas::new();
    c.draw_direct_sprite_fx(
        std::sync::Arc::new(ph.bind_group),
        [1.0, 0.0, 0.0, 800.0],
        [0.0, 1.0, 0.0, 600.0],
        1.0,
        1.0,
        [1.0; 3],
        (0.0, 0.0),
    );
    let out = gpu.render_to_rgba(&c, &bank, 160, 160);
    assert!(
        out.as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[1] > 200 && p[3] == 255)
    );

    let portal = Tile::from_def(
        &TileDef {
            x: 0,
            y: 0,
            shape: PieceShape::Spigot,
            piece_type: PieceType::Portal,
            direction: 0,
            is_moveable: false,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: Some(GameColor::Green),
        },
        &minimal_test_level(),
    );
    assert_eq!(
        crate::render::board_underlay::tile_bitmap_keys(&portal),
        ("DrainPipeClearBitmap", "DrainPipeFullBitmap")
    );
}

#[test]
fn night_hills_starlight_matches_original_spec() {
    use crate::render::BgAnimState;
    use crate::render::backgrounds::NightStar;

    let mut rng = StdRng::seed_from_u64(42);
    let stars = NightStar::generate_layer(&mut rng);
    assert_eq!(stars.len(), 18);
    for s in &stars {
        assert!((25.0..=774.0).contains(&s.x) && s.x.fract() == 0.0);
        assert!((25.0..=424.0).contains(&s.y) && s.y.fract() == 0.0);
        assert!((0.0..=15.0).contains(&s.radius) && s.radius.fract() == 0.0);
        let expected_alpha = ((1.0 - s.y / 400.0) * 0.4 + 0.25).clamp(0.0, 1.0);
        assert!((s.alpha - expected_alpha).abs() < 1e-6);
    }

    // Verify exact 75ms step curve & 12-tick (0.90s) cycle across all initial values 0.2..=1.9
    let sample = NightStar {
        x: 100.0,
        y: 100.0,
        radius: 10.0,
        alpha: 0.55,
        scales: NightStar::build_scales(1.9, 1.0),
    };
    let expected_first_13 = [
        1.0, 1.0, 1.0, 1.0, 1.0, 0.8, 0.5, 0.2, 0.5, 0.8, 1.0, 1.0, 1.0,
    ];
    for (i, &exp) in expected_first_13.iter().enumerate() {
        let t_mid = (i as f32 + 0.5) * 0.075;
        assert!(
            (sample.scale_at_age(t_mid) - exp).abs() < 1e-5,
            "tick {i}: got {}, expected {exp}",
            sample.scale_at_age(t_mid)
        );
        if i >= 6 {
            let t_next = (i as f32 + 12.5) * 0.075;
            assert!((sample.scale_at_age(t_next) - exp).abs() < 1e-5);
        }
    }
    assert_eq!(sample.scale_at_age(0.01), sample.scale_at_age(0.06));
    for k in 2..=19 {
        let scales = NightStar::build_scales(f64::from(k) / 10.0, 1.0);
        let star = NightStar {
            x: 100.0,
            y: 100.0,
            radius: 10.0,
            alpha: 0.55,
            scales,
        };
        for tick in 6..18 {
            let a = star.scale_at_age(tick as f32 * 0.075 + 0.01);
            let b = star.scale_at_age((tick + 12) as f32 * 0.075 + 0.01);
            assert!((a - b).abs() < 1e-5, "init={k} tick={tick}: {a} vs {b}");
        }
    }

    // Verify BgAnimState::load preserves stars when already on NightHills and regenerates on switch
    let mut anim = BgAnimState::new("NightHills");
    let initial = anim.stars.clone();
    anim.load("NightHills");
    assert_eq!(anim.stars, initial);
    anim.load("DaySky");
    assert_eq!(anim.bg_id, "DaySky");
    assert_eq!(anim.stars, initial);
    anim.load("NightHills");
    assert_eq!(anim.stars.len(), 18);
    assert_ne!(anim.stars, initial);

    // GPU verification: radial 5-stop gradient matches StarSprite (alpha 0.60 at center, 0 outside radius)
    let gpu = test_gpu();
    let bank = TexBank::new(gpu.clone(), &empty_bundle());
    let mut c = Canvas::new();
    c.clear((0, 0, 0));
    c.draw_star_glow(400.5, 300.5, 15.0, 0.6);
    let px = gpu.render_to_rgba(&c, &bank, 800, 600);
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 800 + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    let center = at(400, 300);
    assert_eq!(center[0], center[1]);
    assert_eq!(center[1], center[2]);
    assert!(
        (88..=96).contains(&center[0]),
        "expected ~92 at center, got {:?}",
        center
    );
    let mid = at(406, 300);
    assert!(mid[0] > 0 && mid[0] < center[0], "mid pixel {:?}", mid);
    assert_eq!(at(420, 300), [0, 0, 0, 255]);
}

#[test]
fn firework_effect_matches_original_spec() {
    use crate::render::FireworkBurst;
    use crate::render::canvas::DrawCmd;

    // 1. Single FireStar GlowFilter texture: 3x3 white core + pure-color soft blur(6, strength=2)
    let gpu = test_gpu();
    let bank = TexBank::new(gpu.clone(), &empty_bundle());
    let (red_tid, base_sz) = bank.fire_star((255, 0, 0));
    assert_eq!(base_sz, 21.0);
    let mut c = Canvas::new();
    c.clear((0, 0, 0));
    c.draw_tex_uv_rotated_blur(
        red_tid,
        [0.0, 0.0, 1.0, 1.0],
        400.5,
        300.5,
        base_sz,
        base_sz,
        0.0,
        None,
        1.0,
        1.0,
        false,
        (0.0, 0.0),
    );
    let px = gpu.render_to_rgba(&c, &bank, 800, 600);
    let at = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 800 + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    };
    assert_eq!(at(400, 300), [255, 255, 255, 255]);
    let p403 = at(403, 300);
    let p405 = at(405, 300);
    assert!(
        p403[0] > 40 && p403[1] <= 2 && p403[2] <= 2,
        "p403={p403:?}"
    );
    assert!(
        p405[0] > 5 && p405[0] < p403[0] && p405[1] <= 2 && p405[2] <= 2,
        "p405={p405:?} vs p403={p403:?}"
    );
    assert_eq!(at(420, 300), [0, 0, 0, 255]);

    // 2. FireworkBurst emits 15 rotated + scaled SpriteTex FireStars (no flat circles)
    let mut rng = StdRng::seed_from_u64(7);
    let mut burst = FireworkBurst::new(200.0, 120.0, (0, 255, 0), 1.1, &mut rng);
    burst.update(0.3);
    c.begin_frame(1.0);
    burst.draw(&mut c, &bank, 1.0);
    let green_tid = bank.fire_star((0, 255, 0)).0;
    let star_cmds: Vec<_> = c
        .cmds
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::SpriteTex(tid, inst) if *tid == green_tid => Some(inst),
            _ => None,
        })
        .collect();
    assert_eq!(star_cmds.len(), 15);
    assert!(!c.cmds.iter().any(|cmd| matches!(cmd, DrawCmd::Shape(_))));
    for inst in star_cmds {
        assert!(inst.row0[3] > base_sz, "expected Back.easeOut scale > 1.0");
        assert!(
            inst.row0[1].abs() > 0.01 || inst.row1[0].abs() > 0.01,
            "expected non-zero 720-deg tween rotation"
        );
    }

    // 3. Pure RGB colors & behind-Summary Z-order
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let mut app = GameApp::new(bundle, Some(120), gpu).expect("init");
    for _ in 0..24 {
        app.spawn_firework(400.0, 100.0);
    }
    for fw in &app.fireworks {
        assert!(
            matches!(fw.rgb(), (255, 0, 0) | (0, 255, 0) | (0, 0, 255)),
            "unexpected firework color {:?}",
            fw.rgb()
        );
    }
    app.summary_state = Some(crate::ui::summary::SummaryState::new(
        calculate_score(4, 5, 6000, 4, 5, 8000, true),
        1000,
        2400,
    ));
    if let Some(s) = &mut app.summary_state {
        s.age = 0.5;
    }
    app.screen = Screen::Summary;
    c.begin_frame(1.0);
    app.render(&mut c);

    let fw_tids = [
        app.renderer.bank.fire_star((255, 0, 0)).0,
        app.renderer.bank.fire_star((0, 255, 0)).0,
        app.renderer.bank.fire_star((0, 0, 255)).0,
    ];
    let rainbow_tid = app.renderer.bank.rainbow_tex();
    let first_fw = c
        .cmds
        .iter()
        .position(|cmd| matches!(cmd, DrawCmd::SpriteTex(t, _) if fw_tids.contains(t)))
        .expect("firework SpriteTex should render");
    let rainbow_pos = c
        .cmds
        .iter()
        .position(|cmd| matches!(cmd, DrawCmd::SpriteTex(t, _) if *t == rainbow_tid))
        .expect("rainbow SpriteTex should render");
    assert!(
        first_fw < rainbow_pos,
        "fireworks ({first_fw}) must render behind Summary rainbow ({rainbow_pos})"
    );
}

#[test]
fn mirror_match_orientation_sign_and_preview_parity() {
    use crate::render::BoardLayout;
    use crate::render::board::{BoardAnim, BoardRenderer};
    use crate::render::board_underlay::{draw_attachment_underlay, tile_xform};

    let gpu = test_gpu();
    let mut b = empty_bundle();

    // 4x4 asymmetric tile: UP-center is red, RIGHT-center is green.
    let mut pipe_px = vec![0u8; 4 * 4 * 4];
    for x in 1..=2 {
        let i = x * 4;
        pipe_px[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
    }
    for y in 1..=2 {
        let i = (y * 4 + 3) * 4;
        pipe_px[i..i + 4].copy_from_slice(&[0, 255, 0, 255]);
    }
    b.images.insert(
        "asym_pipe".into(),
        RgbaImage {
            width: 4,
            height: 4,
            data: pipe_px,
        },
    );

    // 32x16 asymmetric StartSignBitmap: left half blue, right half yellow.
    let mut sign_px = vec![0u8; 32 * 16 * 4];
    for y in 0..16 {
        for x in 0..32 {
            let i = (y * 32 + x) * 4;
            let rgba = if x < 16 {
                [0, 0, 255, 255]
            } else {
                [255, 255, 0, 255]
            };
            sign_px[i..i + 4].copy_from_slice(&rgba);
        }
    }
    b.images.insert(
        "StartSignBitmap".into(),
        RgbaImage {
            width: 32,
            height: 16,
            data: sign_px,
        },
    );

    let bank = TexBank::new(gpu.clone(), &b);
    let mut c = Canvas::new();
    c.clear((0, 0, 0));

    let tid = bank.named("asym_pipe").unwrap().0;
    let uv = [0.0, 0.0, 1.0, 1.0];
    // 90 deg CW then horizontal mirror: UP -> RIGHT -> LEFT; RIGHT -> DOWN -> DOWN.
    c.draw_tex_uv_rotated_blur(
        tid,
        uv,
        100.0,
        100.0,
        80.0,
        80.0,
        90.0,
        None,
        1.0,
        1.0,
        true,
        (0.0, 0.0),
    );
    // 270 deg CW then horizontal mirror: UP -> LEFT -> RIGHT; RIGHT -> UP -> UP.
    c.draw_tex_uv_rotated_blur(
        tid,
        uv,
        300.0,
        100.0,
        80.0,
        80.0,
        270.0,
        None,
        1.0,
        1.0,
        true,
        (0.0, 0.0),
    );

    let source_tile = Tile::from_def(
        &TileDef {
            x: 0,
            y: 0,
            shape: PieceShape::Spigot,
            piece_type: PieceType::Source,
            direction: 0,
            is_moveable: false,
            can_rotate: false,
            is_gate: false,
            is_freeze: false,
            is_magic: false,
            secondary_color: None,
        },
        &minimal_test_level(),
    );
    draw_attachment_underlay(
        &mut c,
        &bank,
        &source_tile,
        500.0,
        300.0,
        128.0,
        1.0,
        true,
        (0.0, 0.0),
    );

    let out = gpu.render_to_rgba(&c, &bank, 800, 600);
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * 800 + x) * 4;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    };

    assert!(
        px(65, 100)[0] > 200 && px(65, 100)[1] < 20,
        "90deg mirrored LEFT {:?}",
        px(65, 100)
    );
    assert!(
        px(100, 135)[1] > 200 && px(100, 135)[0] < 20,
        "90deg mirrored DOWN {:?}",
        px(100, 135)
    );
    assert_eq!(px(135, 100), [0, 0, 0, 255]);
    assert_eq!(px(100, 65), [0, 0, 0, 255]);

    assert!(
        px(335, 100)[0] > 200 && px(335, 100)[1] < 20,
        "270deg mirrored RIGHT {:?}",
        px(335, 100)
    );
    assert!(
        px(300, 65)[1] > 200 && px(300, 65)[0] < 20,
        "270deg mirrored UP {:?}",
        px(300, 65)
    );
    assert_eq!(px(265, 100), [0, 0, 0, 255]);
    assert_eq!(px(300, 135), [0, 0, 0, 255]);

    // Mirrored StartSignBitmap flips around tile center x=500 -> quad covers 473..505, y=217..233.
    let sign_l = px(478, 225);
    let sign_r = px(500, 225);
    assert!(
        sign_l[0] > 200 && sign_l[1] > 200 && sign_l[2] < 20,
        "mirrored sign left should be yellow: {:?}",
        sign_l
    );
    assert!(
        sign_r[2] > 200 && sign_r[0] < 20 && sign_r[1] < 20,
        "mirrored sign right should be blue: {:?}",
        sign_r
    );
    assert_eq!(
        px(520, 225),
        [0, 0, 0, 255],
        "unmirrored side must be empty"
    );

    // BoardAnim::Preview has zero blur and omits PipeDropShadow.
    let layout = BoardLayout::compute(1, 1, Rect::new(336.0, 236.0, 128.0, 128.0));
    let (_, _, _, prev_a, prev_blur) = tile_xform(
        &source_tile,
        0,
        1,
        0.0,
        layout,
        false,
        BoardAnim::Preview { alpha: 0.5 },
    );
    let (_, _, _, obsc_a, obsc_blur) = tile_xform(
        &source_tile,
        0,
        1,
        0.0,
        layout,
        false,
        BoardAnim::Obscured { alpha: 0.5 },
    );
    assert!((prev_a - 0.5).abs() < 1e-5 && prev_blur == 0.0);
    assert!((obsc_a - 0.5).abs() < 1e-5 && (obsc_blur - 16.0).abs() < 1e-4);

    if let Some(path) = asset_file_if_present() {
        let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
        let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");
        let elbow = Tile::from_def(
            &TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Elbow,
                piece_type: PieceType::Pipe,
                direction: 0,
                is_moveable: true,
                can_rotate: false,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            &minimal_test_level(),
        );
        let tiles = vec![vec![elbow]];
        let mut board_r = BoardRenderer::new();

        let mut c_act = Canvas::new();
        c_act.clear((255, 255, 255));
        board_r.draw_board(
            &mut c_act,
            &renderer.bank,
            &renderer.fonts,
            &tiles,
            layout,
            None,
            &[],
            None,
            &[],
            &[],
            false,
            false,
            BoardAnim::Active,
            0.0,
        );
        let px_act = gpu.render_to_rgba(&c_act, &renderer.bank, 800, 600);

        let mut c_prv = Canvas::new();
        c_prv.clear((255, 255, 255));
        board_r.draw_board(
            &mut c_prv,
            &renderer.bank,
            &renderer.fonts,
            &tiles,
            layout,
            None,
            &[],
            None,
            &[],
            &[],
            false,
            false,
            BoardAnim::Preview { alpha: 1.0 },
            0.0,
        );
        let px_prv = gpu.render_to_rgba(&c_prv, &renderer.bank, 800, 600);

        let mut act_shadow = false;
        let mut prv_ring_dirty = false;
        for y in 224usize..376 {
            for x in 324usize..476 {
                if (336..464).contains(&x) && (236..364).contains(&y) {
                    continue;
                }
                let i = (y * 800 + x) * 4;
                if px_act[i] < 245 {
                    act_shadow = true;
                }
                if px_prv[i] < 255 || px_prv[i + 1] < 255 || px_prv[i + 2] < 255 {
                    prv_ring_dirty = true;
                }
            }
        }
        assert!(
            act_shadow,
            "Active board should render PipeDropShadow in exterior ring"
        );
        assert!(!prv_ring_dirty, "Preview board must omit PipeDropShadow");
    }
}
