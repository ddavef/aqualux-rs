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
            0.0,
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
            0.0,
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

#[test]
fn ui_colors_and_text_outline_shadow_parity() {
    use crate::game::{Difficulty, LevelSim};
    use crate::render::{
        BoardLayout, FontFamily, ScoreBlurbFx, TextAlign, TextOutline, draw_score_blurbs,
    };
    use crate::ui::hud;
    use crate::ui::summary::{SummaryState, render_summary_screen};
    use crate::ui::widgets::{draw_plain_text_button, draw_slider, draw_text_button};

    assert_eq!(GameColor::Red.rgb(), (0x8A, 0x3E, 0x35));
    assert_eq!(GameColor::Orange.rgb(), (0xE6, 0x73, 0x00));
    assert_eq!(GameColor::Yellow.rgb(), (0xD2, 0xD2, 0x00));
    assert_eq!(GameColor::Green.rgb(), (0x3A, 0xF4, 0x04));
    assert_eq!(GameColor::Blue.rgb(), (0x66, 0xB3, 0xFF));
    assert_eq!(GameColor::Violet.rgb(), (0xA9, 0x5C, 0xAF));
    assert_eq!(GameColor::White.rgb(), (0xE6, 0xE6, 0xE6));
    assert_eq!(GameColor::Grey.rgb(), (0x3F, 0x3F, 0x3F));

    let gpu = test_gpu();
    let empty_bank = TexBank::new(gpu.clone(), &empty_bundle());
    let mut c_slider = Canvas::new();
    c_slider.clear((0, 0, 0));
    draw_slider(&mut c_slider, 100.0, 100.0, 0.0);
    let px_slider = gpu.render_to_rgba(&c_slider, &empty_bank, 800, 600);
    let idx = |x: usize, y: usize| (y * 800 + x) * 4;
    let track = &px_slider[idx(160, 110)..idx(160, 110) + 4];
    let nub = &px_slider[idx(100, 110)..idx(100, 110) + 4];
    assert!(
        track[0].abs_diff(0x42) <= 2
            && track[1].abs_diff(0x83) <= 2
            && track[2].abs_diff(0xB3) <= 2,
        "slider track should be #4283B3, got {:?}",
        track
    );
    assert!(
        nub[0].abs_diff(0x7E) <= 2 && nub[1].abs_diff(0xAD) <= 2 && nub[2].abs_diff(0xCF) <= 2,
        "slider nub should be #7EADCF, got {:?}",
        nub
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

    // 1. Opt-in drop shadow: stroke-only has no neutral-grey shadow pixels on white bg.
    let render_a_with = |outline: Option<TextOutline>| -> Vec<u8> {
        let mut c = Canvas::new();
        c.clear((255, 255, 255));
        renderer.fonts.draw_styled_alpha(
            &mut c,
            FontFamily::Neuropol,
            120.0,
            120.0,
            40.0,
            1.0,
            1.0,
            (255, 255, 255),
            outline,
            TextAlign::Left,
            "A",
            1.0,
        );
        gpu.render_to_rgba(&c, &renderer.bank, 800, 600)
    };
    let px_plain_stroke = render_a_with(Some(TextOutline::stroke((255, 0, 0))));
    let px_shadowed = render_a_with(Some(TextOutline::shadowed((255, 0, 0))));
    let has_neutral_shadow = |px: &[u8]| -> bool {
        (80usize..220).any(|y| {
            (80usize..220).any(|x| {
                let i = idx(x, y);
                let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
                r < 240 && r.abs_diff(g) <= 2 && g.abs_diff(b) <= 2
            })
        })
    };
    assert!(
        !has_neutral_shadow(&px_plain_stroke),
        "stroke-only text must not emit a drop shadow"
    );
    assert!(
        has_neutral_shadow(&px_shadowed),
        "shadowed text must emit neutral-grey drop shadow pixels"
    );

    // 2. Dual solid stroke on Champion text (#56A0A9 fill, #000000 inner, #F4F28A outer).
    let mut c_champ = Canvas::new();
    c_champ.clear((120, 0, 120));
    renderer.fonts.draw_styled_alpha(
        &mut c_champ,
        FontFamily::Neuropol,
        100.0,
        200.0,
        46.0,
        1.0,
        1.0,
        (0x56, 0xA0, 0xA9),
        Some(TextOutline::double((0, 0, 0), (0xF4, 0xF2, 0x8A))),
        TextAlign::Left,
        "GRAND",
        1.0,
    );
    let px_champ = gpu.render_to_rgba(&c_champ, &renderer.bank, 800, 600);
    let (mut saw_fill, mut saw_inner, mut saw_outer) = (false, false, false);
    for y in 170usize..310 {
        for x in 60usize..420 {
            let i = idx(x, y);
            let (r, g, b) = (px_champ[i], px_champ[i + 1], px_champ[i + 2]);
            if r.abs_diff(0x56) <= 6 && g.abs_diff(0xA0) <= 6 && b.abs_diff(0xA9) <= 6 {
                saw_fill = true;
            } else if r <= 10 && g <= 10 && b <= 10 {
                saw_inner = true;
            } else if r >= 220 && g >= 218 && b <= 155 {
                saw_outer = true;
            }
        }
    }
    assert!(
        saw_fill && saw_inner && saw_outer,
        "dual stroke text missing colors: fill={saw_fill} inner={saw_inner} outer={saw_outer}"
    );

    // 3. Hovered draw_text_button uses #C9D8E1.
    let mut c_hov = Canvas::new();
    c_hov.clear((0, 0, 0));
    draw_text_button(
        &mut c_hov,
        &renderer.fonts,
        400.0,
        300.0,
        31.0,
        TextAlign::Center,
        "PLAY",
        true,
        None,
        1.0,
    );
    let px_hov = gpu.render_to_rgba(&c_hov, &renderer.bank, 800, 600);
    let saw_silver_blue = (280usize..380).any(|y| {
        (300usize..500).any(|x| {
            let i = idx(x, y);
            px_hov[i].abs_diff(0xC9) <= 6
                && px_hov[i + 1].abs_diff(0xD8) <= 6
                && px_hov[i + 2].abs_diff(0xE1) <= 6
        })
    });
    assert!(saw_silver_blue, "hovered button must use #C9D8E1");

    // 4. Plain Options button has #4283B3 fill and zero navy stroke pixels on white bg.
    let mut c_plain_btn = Canvas::new();
    c_plain_btn.clear((255, 255, 255));
    draw_plain_text_button(
        &mut c_plain_btn,
        &renderer.fonts,
        400.0,
        300.0,
        16.0,
        TextAlign::Center,
        "hard",
        false,
        (0x42, 0x83, 0xB3),
    );
    let px_pbtn = gpu.render_to_rgba(&c_plain_btn, &renderer.bank, 800, 600);
    let mut saw_steel = false;
    for y in 285usize..345 {
        for x in 320usize..480 {
            let i = idx(x, y);
            let (r, g, b) = (px_pbtn[i], px_pbtn[i + 1], px_pbtn[i + 2]);
            if r.abs_diff(0x42) <= 8 && g.abs_diff(0x83) <= 8 && b.abs_diff(0xB3) <= 8 {
                saw_steel = true;
            }
            assert!(
                !(r < 30 && g < 30 && b < 60),
                "plain button must not draw dark stroke/shadow at ({x},{y}): {:?}",
                &px_pbtn[i..i + 4]
            );
        }
    }
    assert!(saw_steel, "plain Options button should draw #4283B3 text");

    // 5. ScoreBlurb starts at #E4E9ED (228, 233, 237).
    let mut c_blurb = Canvas::new();
    c_blurb.clear((0, 0, 0));
    draw_score_blurbs(
        &mut c_blurb,
        &renderer.fonts,
        &[ScoreBlurbFx {
            x: 400.0,
            y: 300.0,
            text: "100".into(),
            age: 0.0,
        }],
    );
    let px_blurb = gpu.render_to_rgba(&c_blurb, &renderer.bank, 800, 600);
    let saw_ice_white = (250usize..350).any(|y| {
        (330usize..470).any(|x| {
            let i = idx(x, y);
            px_blurb[i].abs_diff(228) <= 6
                && px_blurb[i + 1].abs_diff(233) <= 6
                && px_blurb[i + 2].abs_diff(237) <= 6
        })
    });
    assert!(saw_ice_white, "ScoreBlurb at t=0 must be ice-white #E4E9ED");

    // 6. SummaryScreen uses #40D1EF cyan.
    let mut c_sum = Canvas::new();
    c_sum.clear((0, 0, 0));
    let mut sum = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 1000, 2400);
    sum.age = 2.0;
    render_summary_screen(
        &mut c_sum,
        &renderer.bank,
        &renderer.fonts,
        &sum,
        (0.0, 0.0),
    );
    let px_sum = gpu.render_to_rgba(&c_sum, &renderer.bank, 800, 600);
    let saw_cyan = (90usize..200).any(|y| {
        (460usize..780).any(|x| {
            let i = idx(x, y);
            px_sum[i] <= 90 && px_sum[i + 1] >= 190 && px_sum[i + 2] >= 230
        })
    });
    assert!(saw_cyan, "Summary 'well done!' must use #40D1EF cyan");

    // 7. HUD par labels use #161616 dark grey stroke instead of navy (0, 0, 64).
    let mut c_hud = Canvas::new();
    c_hud.clear((0, 0, 0));
    let sim = LevelSim::new(bundle.campaign_levels[0].clone(), Difficulty::Hard);
    let layout = BoardLayout::compute(sim.level.cols, sim.level.rows, hud::campaign_board_area());
    hud::render_campaign_hud(
        &mut c_hud,
        &renderer.bank,
        &renderer.fonts,
        &sim,
        layout,
        1000,
        sim.moves,
        sim.elapsed_ms as u32,
        0.0,
        None,
        (0.0, 0.0),
        false,
        false,
        &hud::HudAnimState::fully_visible(),
        true,
    );
    let px_hud = gpu.render_to_rgba(&c_hud, &renderer.bank, 800, 600);
    let mut saw_par_grey_stroke = false;
    for y in 565usize..596 {
        for x in 548usize..610 {
            let i = idx(x, y);
            let (r, g, b) = (px_hud[i], px_hud[i + 1], px_hud[i + 2]);
            if r.abs_diff(0x16) <= 4 && g.abs_diff(0x16) <= 4 && b.abs_diff(0x16) <= 4 {
                saw_par_grey_stroke = true;
            }
            assert!(
                !(r <= 10 && g <= 10 && b >= 45),
                "HUD par label must use #161616 stroke rather than navy at ({x},{y}): {:?}",
                &px_hud[i..i + 4]
            );
        }
    }
    assert!(
        saw_par_grey_stroke,
        "HUD par label should contain #161616 stroke pixels"
    );
}

#[test]
fn bevel_lighting_round_rect_radii_and_tutorial_bubble_parity() {
    use crate::render::{FontFamily, LeakEmitter, draw_tutorial_bubble};

    let gpu = test_gpu();
    let empty_bank = TexBank::new(gpu.clone(), &empty_bundle());
    let idx = |x: usize, y: usize| (y * 800 + x) * 4;
    let luma = |px: &[u8], x: usize, y: usize| -> f32 {
        let i = idx(x, y);
        0.2126 * px[i] as f32 + 0.7152 * px[i + 1] as f32 + 0.0722 * px[i + 2] as f32
    };

    // 1. LeakEmitter bevel highlights top-left/upper edge and shadows bottom-right/lower edge.
    let mut emitter = LeakEmitter::new((0, 0), 200.0, Side::Right, GameColor::Blue);
    for _ in 0..8 {
        emitter.update(1.0 / 30.0);
    }
    let mut c_leak = Canvas::new();
    c_leak.clear((0, 0, 0));
    emitter.draw_at(&mut c_leak, 200.0, 200.0);
    let px_leak = gpu.render_to_rgba(&c_leak, &empty_bank, 800, 600);
    let top_peak = (182usize..198)
        .map(|y| luma(&px_leak, 218, y))
        .fold(0.0f32, f32::max);
    let bot_peak = (202usize..218)
        .map(|y| luma(&px_leak, 218, y))
        .fold(0.0f32, f32::max);
    assert!(
        top_peak > bot_peak + 8.0,
        "Leak upper edge should receive highlight ({top_peak:.1} vs lower {bot_peak:.1})"
    );

    // 2. Modal bubble has corner radius 17.5 (not 35.0) and top-left highlight / bottom-right shadow.
    let mut c_modal = Canvas::new();
    c_modal.clear((0, 0, 0));
    c_modal.draw_modal_bubble(Rect::new(100.0, 100.0, 200.0, 100.0));
    let px_modal = gpu.render_to_rgba(&c_modal, &empty_bank, 800, 600);
    // Corner arc center at r=17.5 is (117.5, 117.5); (108, 108) is inside fill (whereas r=35 leaves it outside).
    let corner_in = &px_modal[idx(108, 108)..idx(108, 108) + 4];
    assert!(
        corner_in[2] > 70,
        "(108, 108) must lie inside 17.5px-radius modal corner, got {:?}",
        corner_in
    );
    let modal_tl = luma(&px_modal, 120, 103);
    let modal_br = luma(&px_modal, 280, 197);
    assert!(
        modal_tl > modal_br + 10.0,
        "Modal top-left bevel ({modal_tl:.1}) must be brighter than bottom-right ({modal_br:.1})"
    );

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

    // 3. TutorialBubble: delayed start, anchor-scaled pop-in without cache thrash, bevel, narrow tail, HTML spans & italics.
    let sample_msg = "Uh-oh! <font color=\"#0080FF\">AQUA</font> test <font color=\"#494949\"><i>DRAG</i></font>";
    let mut c_pre = Canvas::new();
    c_pre.clear((0, 255, 128));
    let (_, _, pre_w, pre_h) = draw_tutorial_bubble(
        &mut c_pre,
        &renderer.fonts,
        400.0,
        420.0,
        sample_msg,
        true,
        0.10,
    );
    assert_eq!((pre_w, pre_h), (0.0, 0.0));
    let px_pre = gpu.render_to_rgba(&c_pre, &renderer.bank, 800, 600);
    assert!(
        px_pre
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [0, 255, 128, 255]),
        "TutorialBubble before 0.2s delay must draw nothing"
    );

    let mut c_full = Canvas::new();
    c_full.clear((0, 255, 128));
    let (fx, fy, fw, fh) = draw_tutorial_bubble(
        &mut c_full,
        &renderer.fonts,
        400.0,
        420.0,
        sample_msg,
        true,
        1.0,
    );
    let px_full = gpu.render_to_rgba(&c_full, &renderer.bank, 800, 600);

    // Inner bevel: top edge of bubble is lighter than bottom edge near shadow.
    let f_cx = (fx + fw * 0.5).round() as usize;
    let tut_top = luma(&px_full, f_cx + 30, (fy + 4.0).round() as usize);
    let tut_bot = luma(&px_full, f_cx + 30, (fy + fh - 4.0).round() as usize);
    assert!(
        tut_top > tut_bot + 10.0,
        "TutorialBubble top bevel ({tut_top:.1}) must be lighter than bottom bevel ({tut_bot:.1})"
    );

    // Narrow tail: height/20 half-width at bottom edge -> at bot+15, center is filled, dx=8 is empty green bg.
    let bot = fy + fh;
    let y_tail = (bot + 15.0).round() as usize;
    let p_tail_ctr = &px_full[idx(f_cx, y_tail)..idx(f_cx, y_tail) + 4];
    let p_tail_out = &px_full[idx(f_cx + 8, y_tail)..idx(f_cx + 8, y_tail) + 4];
    assert_ne!(
        p_tail_ctr,
        &[0, 255, 128, 255],
        "tail center must be filled at y={y_tail}"
    );
    assert_eq!(
        p_tail_out,
        &[0, 255, 128, 255],
        "narrow tail must exclude dx=8 at y={y_tail}, got {:?}",
        p_tail_out
    );

    // Styled spans: blue AQUA (#0080FF) and Click to continue (#409FFF).
    let (mut saw_aqua_blue, mut saw_ctc_blue) = (false, false);
    for y in (fy as usize)..(bot as usize) {
        for x in (fx as usize)..((fx + fw) as usize) {
            let i = idx(x, y);
            let (r, g, b) = (px_full[i], px_full[i + 1], px_full[i + 2]);
            if r <= 30 && (95..=165).contains(&g) && b >= 220 {
                saw_aqua_blue = true;
            }
            if r.abs_diff(0x40) <= 22 && g.abs_diff(0x9F) <= 22 && b >= 225 {
                saw_ctc_blue = true;
            }
        }
    }
    assert!(saw_aqua_blue, "AQUA span must render #0080FF blue");
    assert!(saw_ctc_blue, "Click to continue must render #409FFF blue");

    // Scaled pop-in at age=0.35 renders scaled text & stays closer to anchor without thrashing font cache.
    let mut c_mid = Canvas::new();
    c_mid.clear((0, 255, 128));
    let (mx, my, mw, mh) = draw_tutorial_bubble(
        &mut c_mid,
        &renderer.fonts,
        400.0,
        420.0,
        sample_msg,
        true,
        0.35,
    );
    assert!(mw < fw * 0.85 && mh < fh * 0.85);
    assert!(
        (my + mh * 0.5) > (fy + fh * 0.5) + 15.0,
        "partial bubble must scale toward bottom anchor"
    );
    let px_mid = gpu.render_to_rgba(&c_mid, &renderer.bank, 800, 600);
    let saw_scaled_dark_text = ((my + 6.0) as usize..((my + mh - 6.0) as usize)).any(|y| {
        ((mx + 6.0) as usize..((mx + mw - 6.0) as usize)).any(|x| {
            let i = idx(x, y);
            px_mid[i] < 60 && px_mid[i + 1] < 60 && px_mid[i + 2] < 60
        })
    });
    assert!(
        saw_scaled_dark_text,
        "scaled text should already be visible at age=0.35"
    );

    let count_after_first = renderer.fonts.cache_entry_count();
    for age in [0.40, 0.50, 0.65, 0.78] {
        let mut c_anim = Canvas::new();
        draw_tutorial_bubble(
            &mut c_anim,
            &renderer.fonts,
            400.0,
            420.0,
            sample_msg,
            true,
            age,
        );
    }
    assert_eq!(
        renderer.fonts.cache_entry_count(),
        count_after_first,
        "animated TutorialBubble pop-in must reuse cached font textures"
    );

    // Italic shear check: <i>DRAG</i> top ink centroid shifts right vs bottom relative to upright DRAG.
    let measure_shear = |italic: bool| -> f32 {
        let mut c = Canvas::new();
        c.clear((0, 0, 0));
        renderer.fonts.draw_styled_span_fx(
            &mut c,
            FontFamily::Helvetica,
            200.0,
            200.0,
            24.0,
            1.0,
            (255, 255, 255),
            italic,
            "DRAG",
        );
        let px = gpu.render_to_rgba(&c, &renderer.bank, 800, 600);
        let centroid = |ys: std::ops::Range<usize>| -> f32 {
            let (mut wx, mut w) = (0.0f32, 0.0f32);
            for y in ys {
                for x in 180usize..340 {
                    let v = px[idx(x, y)] as f32;
                    if v > 20.0 {
                        wx += x as f32 * v;
                        w += v;
                    }
                }
            }
            wx / w.max(1.0)
        };
        centroid(196..209) - centroid(210..226)
    };
    let delta = measure_shear(true) - measure_shear(false);
    assert!(
        delta > 1.2,
        "<i> span should lean right at top relative to bottom (delta={delta:.2})"
    );
}

#[test]
fn board_effects_blurs_shadow_and_piece_parity() {
    use crate::render::board::{BoardAnim, BoardRenderer};
    use crate::render::board_underlay::tile_xform;
    use crate::render::canvas::DrawCmd;
    use crate::render::gpu_types::color_mat_id;
    use crate::render::pipe_mask::draw_flow_timer;
    use crate::render::tex_bank::{CURSOR_PAD, GlowPreset};
    use crate::render::{BoardLayout, FontFamily, LeakEmitter, ScoreBlurbFx, draw_score_blurbs};

    // 1. CURSOR_PAD accommodates blur=16 + 4px@45deg drop shadow without clipping.
    assert_eq!(CURSOR_PAD, 26);

    // 2. PopIn uses bounce_out for alpha/blur; HideDrop uses strong_in for blur.
    let layout1 = BoardLayout::compute(1, 1, Rect::new(336.0, 236.0, 128.0, 128.0));
    let mut dummy = Tile::from_def(&TileDef::dummy_blocker(), &minimal_test_level());
    dummy.x = 0;
    dummy.y = 0;
    let (_, _, _, pop_a, pop_b) = tile_xform(
        &dummy,
        0,
        1,
        0.0,
        layout1,
        false,
        BoardAnim::PopIn {
            elapsed: 0.15,
            start_delay: 0.0,
        },
    );
    assert!((pop_a - 0.472656).abs() < 1e-3, "got {pop_a}");
    assert!((pop_b - (1.0 - pop_a) * 8.0).abs() < 1e-3, "got {pop_b}");

    let (_, _, _, _, hide_b) = tile_xform(
        &dummy,
        0,
        1,
        0.0,
        layout1,
        false,
        BoardAnim::HideDrop {
            elapsed: 0.40,
            delay: 0.0,
        },
    );
    assert!((hide_b - 0.5).abs() < 1e-3, "got {hide_b}");

    let gpu = test_gpu();
    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

    let (arrow_raw_id, arrow_w, arrow_h) = renderer.bank.named("ArrowCursorBitmap").unwrap();
    let _ = arrow_raw_id;
    let (_, cw, ch, cpad) = renderer.bank.cursor(CursorKind::Arrow).unwrap();
    assert_eq!(cpad, 26);
    assert_eq!((cw, ch), (arrow_w + 52, arrow_h + 52));

    // 3. ScoreBlurb Phase 2 back_in overshoots upward and retains its 32px white glow.
    let mut c_blurb = Canvas::new();
    draw_score_blurbs(
        &mut c_blurb,
        &renderer.fonts,
        &[ScoreBlurbFx {
            x: 400.0,
            y: 300.0,
            text: "100".into(),
            age: 0.68,
        }],
    );
    let glows: Vec<_> = c_blurb
        .cmds
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::SpriteDirect(_, s) if s.params[1] < -0.5 => Some(*s),
            _ => None,
        })
        .collect();
    let mains: Vec<_> = c_blurb
        .cmds
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::SpriteDirect(_, s) if s.params[1] > 0.5 => Some(*s),
            _ => None,
        })
        .collect();
    assert!(!glows.is_empty(), "Phase 2 ScoreBlurb must keep white glow");
    assert!((glows[0].params[3] - 32.0).abs() < 1e-3);
    assert!(
        mains[0].row0[0] > 1.35,
        "Phase 2 back_in should overshoot scale > 1.35, got {}",
        mains[0].row0[0]
    );

    // 4. FlowTimer red/black warning color, one-shot elastic 1.0 -> 1.2 -> 0.9, and 64px fade blur.
    let mut c_ft_red = Canvas::new();
    c_ft_red.clear((0, 0, 0));
    let mut ft_state = crate::game::FlowTimerState::new(5.0);
    ft_state.age = 1.0;
    ft_state.remaining = 0.0;
    ft_state.warning_red = true;
    ft_state.warn_elapsed = Some(1.9);
    ft_state.fading = true;
    ft_state.fade_elapsed = 0.0;
    draw_flow_timer(
        &mut c_ft_red,
        &renderer.bank,
        &renderer.fonts,
        120.0,
        120.0,
        &ft_state,
        1.0,
    );
    let px_red = gpu.render_to_rgba(&c_ft_red, &renderer.bank, 800, 600);
    let saw_red_digit = (100usize..145).any(|y| {
        (90usize..155).any(|x| {
            let i = (y * 800 + x) * 4;
            px_red[i] > 180 && px_red[i + 1] < 35 && px_red[i + 2] < 35
        })
    });
    assert!(
        saw_red_digit,
        "0:00 with warning_red=true must draw red digits"
    );

    let mut c_ft_fade = Canvas::new();
    ft_state.fade_elapsed = 0.92;
    draw_flow_timer(
        &mut c_ft_fade,
        &renderer.bank,
        &renderer.fonts,
        120.0,
        120.0,
        &ft_state,
        1.0,
    );
    let watch_tid = renderer.bank.named("FlowTimerBitmap").unwrap().0;
    let watch_inst = c_ft_fade
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            DrawCmd::SpriteTex(tid, s) if *tid == watch_tid => Some(*s),
            _ => None,
        })
        .unwrap();
    let (_, watch_w, _) = renderer.bank.named("FlowTimerBitmap").unwrap();
    assert!(
        (watch_inst.row0[3] / watch_w as f32 - 0.9).abs() < 0.02,
        "post-warn FlowTimer should hold at 0.9x scale"
    );
    assert!(
        (watch_inst.params[3] - 0.92f32.powi(5) * 64.0).abs() < 0.05,
        "FlowTimer fade should ramp blur toward 64px, got {}",
        watch_inst.params[3]
    );

    // 5. PortalSprite one-shot shrink 1.0 -> 0.9 over 2s (no cosine bounce).
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
            secondary_color: Some(GameColor::Violet),
        },
        &minimal_test_level(),
    );
    let portal_tid = renderer.bank.named("YellowPortalBitmap").unwrap().0;
    let portal_width_at = |t: f32| -> f32 {
        let mut c = Canvas::new();
        BoardRenderer::new().draw_board(
            &mut c,
            &renderer.bank,
            &renderer.fonts,
            &[vec![portal.clone()]],
            layout1,
            None,
            &[],
            None,
            0.0,
            &[],
            false,
            false,
            BoardAnim::Active,
            t,
        );
        c.cmds
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::SpriteTex(tid, s) if *tid == portal_tid => Some(s.row0[3]),
                _ => None,
            })
            .unwrap()
    };
    assert!((portal_width_at(0.0) - 128.0).abs() < 1e-2);
    let pw2 = portal_width_at(2.0);
    let pw4 = portal_width_at(4.0);
    let pw6 = portal_width_at(6.0);
    assert!((pw2 - 115.2).abs() < 1e-2 && (pw4 - pw2).abs() < 1e-2 && (pw6 - pw2).abs() < 1e-2);

    // 6. Blocker tile renders color-blind badge when enabled.
    let mut blocker = dummy.clone();
    blocker.x = 0;
    blocker.y = 0;
    let mut c_cb_off = Canvas::new();
    c_cb_off.clear((0, 0, 0));
    BoardRenderer::new().draw_board(
        &mut c_cb_off,
        &renderer.bank,
        &renderer.fonts,
        &[vec![blocker.clone()]],
        layout1,
        None,
        &[],
        None,
        0.0,
        &[],
        false,
        false,
        BoardAnim::Active,
        0.0,
    );
    let mut c_cb_on = Canvas::new();
    c_cb_on.clear((0, 0, 0));
    BoardRenderer::new().draw_board(
        &mut c_cb_on,
        &renderer.bank,
        &renderer.fonts,
        &[vec![blocker]],
        layout1,
        None,
        &[],
        None,
        0.0,
        &[],
        true,
        false,
        BoardAnim::Active,
        0.0,
    );
    assert_ne!(
        gpu.render_to_rgba(&c_cb_off, &renderer.bank, 800, 600),
        gpu.render_to_rgba(&c_cb_on, &renderer.bank, 800, 600),
        "color_blind=true must draw badge on Blocker tiles"
    );

    // 7. Board-level PipeDropShadow renders strictly behind all tile bodies + Selection ramps 0..0.8s once.
    let elbow0 = Tile::from_def(
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
    let mut elbow1 = elbow0.clone();
    elbow1.x = 1;
    let layout2 = BoardLayout::compute(2, 1, Rect::new(272.0, 236.0, 256.0, 128.0));
    let sh_tid = renderer
        .bank
        .glow_named("ElbowClearBitmap", GlowPreset::PipeDropShadow)
        .unwrap()
        .0;
    let body_tid = renderer.bank.named("ElbowClearBitmap").unwrap().0;
    let sel_cmat = color_mat_id(Some((elbow0.pipe_color, true))) as f32;

    let mut c_board = Canvas::new();
    BoardRenderer::new().draw_board(
        &mut c_board,
        &renderer.bank,
        &renderer.fonts,
        &[vec![elbow0.clone(), elbow1.clone()]],
        layout2,
        Some((0, 0)),
        &[],
        None,
        0.4,
        &[],
        false,
        false,
        BoardAnim::Active,
        0.0,
    );
    let last_sh = c_board
        .cmds
        .iter()
        .rposition(|cmd| matches!(cmd, DrawCmd::SpriteTex(tid, _) if *tid == sh_tid))
        .unwrap();
    let first_body = c_board
        .cmds
        .iter()
        .position(|cmd| matches!(cmd, DrawCmd::SpriteTex(tid, _) if *tid == body_tid))
        .unwrap();
    assert!(
        last_sh < first_body,
        "all board PipeDropShadows ({last_sh}) must emit before first tile body ({first_body})"
    );
    let sel_alpha_at = |sa: f32| -> Option<f32> {
        let mut c = Canvas::new();
        BoardRenderer::new().draw_board(
            &mut c,
            &renderer.bank,
            &renderer.fonts,
            &[vec![elbow0.clone()]],
            layout1,
            Some((0, 0)),
            &[],
            None,
            sa,
            &[],
            false,
            false,
            BoardAnim::Active,
            0.0,
        );
        c.cmds.iter().find_map(|cmd| match cmd {
            DrawCmd::SpriteTex(tid, s)
                if *tid == body_tid && (s.params[0] - sel_cmat).abs() < 0.1 =>
            {
                Some(s.params[2])
            }
            _ => None,
        })
    };
    assert!(sel_alpha_at(0.0).is_none());
    assert!((sel_alpha_at(0.4).unwrap() - 0.75).abs() < 1e-4);
    assert!((sel_alpha_at(0.8).unwrap() - 1.0).abs() < 1e-4);
    assert!((sel_alpha_at(1.6).unwrap() - 1.0).abs() < 1e-4);

    // 8. pipe_mask.wgsl and leak.wgsl render blurred alpha without clipping.
    let full_tid = renderer.bank.named("StraightFullBitmap").unwrap().0;
    let caps = [[64.0, 0.0, 64.0, 128.0], [0.0; 4], [0.0; 4], [0.0; 4]];
    let radii = [24.0, 0.0, 0.0, 0.0];
    let mut c_sharp = Canvas::new();
    c_sharp.clear((0, 0, 0));
    c_sharp.draw_pipe_masked(
        full_tid,
        400.0,
        300.0,
        128.0,
        128.0,
        0.0,
        false,
        caps,
        radii,
        1,
        GameColor::Blue,
        1.0,
        (0.0, 0.0),
    );
    let px_sharp = gpu.render_to_rgba(&c_sharp, &renderer.bank, 800, 600);

    let mut c_blur = Canvas::new();
    c_blur.clear((0, 0, 0));
    c_blur.draw_pipe_masked(
        full_tid,
        400.0,
        300.0,
        128.0,
        128.0,
        0.0,
        false,
        caps,
        radii,
        1,
        GameColor::Blue,
        1.0,
        (16.0, 16.0),
    );
    let px_blur = gpu.render_to_rgba(&c_blur, &renderer.bank, 800, 600);
    // Above the tile top boundary (y < 236), sharp has zero ink while expanded blurred quad spreads upward.
    let sharp_edge = (231usize..236).any(|y| px_sharp[(y * 800 + 400) * 4 + 2] > 0);
    let blur_edge = (231usize..236).any(|y| px_blur[(y * 800 + 400) * 4 + 2] > 5);
    assert!(
        !sharp_edge && blur_edge,
        "blurred pipe_mask must spread outside sharp bounds"
    );

    let mut emitter = LeakEmitter::new((0, 0), 200.0, Side::Right, GameColor::Blue);
    for _ in 0..8 {
        emitter.update(1.0 / 30.0);
    }
    let mut c_lk_blur = Canvas::new();
    c_lk_blur.clear((0, 0, 0));
    emitter.draw_at_fx(&mut c_lk_blur, 200.0, 200.0, 0.5, 16.0);
    let px_lk = gpu.render_to_rgba(&c_lk_blur, &renderer.bank, 800, 600);
    let lk_peak = (176usize..224)
        .map(|y| px_lk[(y * 800 + 218) * 4 + 2])
        .max()
        .unwrap();
    assert!(
        lk_peak > 20 && lk_peak < 180,
        "expected attenuated blurred leak peak, got {lk_peak}"
    );
    let _ = FontFamily::Neuropol;
}

#[test]
fn background_popup_lexicographic_order_heights_darken_and_crossfade_parity() {
    use crate::assets::swf_sprite::PopupMeta;
    use crate::assets::{Matrix2D, SpriteItem};
    use crate::render::backgrounds::{BackgroundRenderer, BgAnimState};
    use crate::render::bg_sprites::{as3_number_string, draw_popup_sprite};
    use crate::render::canvas::DrawCmd;
    use std::collections::HashMap;

    // 1. AS3 sortOn("x") lexicographic ordering + layer-local y=800 offset propagation.
    assert_eq!(as3_number_string(-10.5), "-10.5");
    assert_eq!(as3_number_string(100.0), "100");
    assert_eq!(as3_number_string(20.0), "20");
    assert_eq!(as3_number_string(5.0), "5");

    let gpu = test_gpu();
    let solid = |rgba: [u8; 4]| RgbaImage {
        width: 8,
        height: 8,
        data: rgba.repeat(64),
    };
    let mut b = empty_bundle();
    b.images
        .insert("DaySkyBitmap".into(), solid([0, 255, 0, 255]));
    b.images
        .insert("CombinedBgBitmap".into(), solid([0, 0, 255, 255]));
    let mut cids = HashMap::new();
    for cid in 1..=4u16 {
        cids.insert(cid, solid([255, 255, 255, 255]));
    }
    b.cid_images = cids;
    let bank = TexBank::new(gpu.clone(), &b);

    let items = vec![
        SpriteItem {
            bitmap_cid: 1,
            layer_name: Some("backHillsLayer".into()),
            depth: 1,
            matrix: Matrix2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 5.0,
                ty: 170.0,
            },
            popup: Some(PopupMeta {
                child_idx: 0,
                child_x: 5.0,
                child_y: 50.0,
                layer_c: 0.0,
                layer_d: 1.0,
            }),
        },
        SpriteItem {
            bitmap_cid: 2,
            layer_name: Some("backHillsLayer".into()),
            depth: 2,
            matrix: Matrix2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 20.0,
                ty: 170.0,
            },
            popup: Some(PopupMeta {
                child_idx: 1,
                child_x: 20.0,
                child_y: 50.0,
                layer_c: 0.0,
                layer_d: 1.0,
            }),
        },
        SpriteItem {
            bitmap_cid: 3,
            layer_name: Some("backHillsLayer".into()),
            depth: 3,
            matrix: Matrix2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 100.0,
                ty: 170.0,
            },
            popup: Some(PopupMeta {
                child_idx: 2,
                child_x: 100.0,
                child_y: 50.0,
                layer_c: 0.0,
                layer_d: 1.0,
            }),
        },
        SpriteItem {
            bitmap_cid: 4,
            layer_name: Some("backHillsLayer".into()),
            depth: 4,
            matrix: Matrix2D {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: -10.5,
                ty: 170.0,
            },
            popup: Some(PopupMeta {
                child_idx: 3,
                child_x: -10.5,
                child_y: 50.0,
                layer_c: 0.0,
                layer_d: 1.0,
            }),
        },
    ];
    let anim0 = BgAnimState::new("NightHills");
    let mut c_pop0 = Canvas::new();
    draw_popup_sprite(&mut c_pop0, &bank, &items, &anim0, false, 1.0, |_| true);
    let tys0: Vec<f32> = c_pop0
        .cmds
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::SpriteTex(_, s) => Some(s.row1[2]),
            _ => None,
        })
        .collect();
    assert_eq!(tys0, vec![920.0, 920.0, 920.0, 920.0]);

    // Lexicographic sort order: "-10.5" (rank 0, cid 4), "100" (rank 1, cid 3), "20" (rank 2, cid 2), "5" (rank 3, cid 1).
    let mut anim_mid = BgAnimState::new("NightHills");
    anim_mid.age = 1.20;
    let mut c_pop_mid = Canvas::new();
    draw_popup_sprite(&mut c_pop_mid, &bank, &items, &anim_mid, false, 1.0, |_| {
        true
    });
    let tys_mid: Vec<f32> = c_pop_mid
        .cmds
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::SpriteTex(_, s) => Some(s.row1[2]),
            _ => None,
        })
        .collect();
    assert!(tys_mid[0] == 920.0, "x=5 (rank 3) must wait: {:?}", tys_mid);
    assert!(
        tys_mid[1] == 920.0,
        "x=20 (rank 2) must wait: {:?}",
        tys_mid
    );
    assert!(
        tys_mid[2] < 920.0,
        "x=100 (rank 1) must rise before x=20: {:?}",
        tys_mid
    );
    assert!(
        tys_mid[3] < tys_mid[2],
        "x=-10.5 (rank 0) leads: {:?}",
        tys_mid
    );

    // 2. Quad.easeOut darken/lighten with 1.0s delay + load() preservation on same id.
    let mut bg = BgAnimState::new("NightHills");
    bg.age = 5.0;
    bg.darken(0.0);
    assert_eq!(bg.darken_delay, 1.0);
    bg.update(1.0);
    assert!((bg.darken - 0.0).abs() < 1e-5);
    bg.update(0.8);
    assert!(
        (bg.darken - 0.75).abs() < 1e-4,
        "expected Quad.easeOut 0.75, got {}",
        bg.darken
    );
    bg.update(0.8);
    assert!((bg.darken - 1.0).abs() < 1e-5);

    bg.lighten(0.0);
    bg.update(1.0);
    assert!((bg.darken - 1.0).abs() < 1e-5);
    bg.update(0.8);
    assert!(
        (bg.darken - 0.25).abs() < 1e-4,
        "expected Quad.easeOut 0.25, got {}",
        bg.darken
    );
    bg.update(0.8);
    assert!((bg.darken - 0.0).abs() < 1e-5);

    bg.lighten(2.9);
    bg.load("NightHills");
    assert!((bg.age - 10.2).abs() < 1e-4);
    assert_eq!(bg.darken, 1.0);
    assert!((bg.darken_delay - 3.9).abs() < 1e-5);

    // 3. 1.0s Quad.easeOut crossfade renders outgoing background ON TOP of incoming without clearing incoming.
    bg.age = 5.0;
    bg.darken = 0.0;
    bg.load("DaySky");
    assert_eq!(bg.bg_id, "DaySky");
    assert!(bg.outgoing.is_some());
    bg.update(0.5);
    let mut c_cf = Canvas::new();
    c_cf.clear((0, 0, 0));
    BackgroundRenderer::new().draw_background(&mut c_cf, &b, &bank, &bg);
    let px_cf = gpu.render_to_rgba(&c_cf, &bank, 800, 600);
    let center = &px_cf[(300 * 800 + 400) * 4..(300 * 800 + 400) * 4 + 4];
    assert!(
        center[1].abs_diff(191) <= 4 && center[2].abs_diff(64) <= 4,
        "expected 0.25 NightHills over DaySky green at crossfade midpoint, got {:?}",
        center
    );
    bg.update(0.5);
    assert!(bg.outgoing.is_none());

    // 4. Real SWF background sprites populate PopupMeta on every layer child.
    if let Some(path) = asset_file_if_present() {
        let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
        for key in ["TitleBgAssetsSprite", "SnowHillsBgAssetsSprite"] {
            let items = bundle.sprites.get(key).expect("background sprite missing");
            assert!(!items.is_empty());
            assert!(
                items.iter().all(|it| it.popup.is_some()),
                "{key} items must carry PopupMeta"
            );
        }
    }
}

#[test]
fn splash_crossfade_scissor_clip_and_text_ticker_parity() {
    use crate::render::{LOGICAL_H, LOGICAL_W};
    use crate::ui::splash::{render_splash, render_splash_overlay};
    use crate::ui::title::{TICKER_CLIP_RECT, ticker_local_x};

    let gpu = test_gpu();
    let b = empty_bundle();
    let bank = TexBank::new(gpu.clone(), &b);

    // 1. Canvas::set_clip enforces hardware wgpu scissor bounds at TICKER_CLIP_RECT (x = 5..505).
    let mut c_clip = Canvas::new();
    c_clip.clear((0, 0, 0));
    c_clip.set_clip(Some(TICKER_CLIP_RECT));
    c_clip.fill_rect_alpha(Rect::new(0.0, 570.0, 700.0, 25.0), 0, 255, 0, 255);
    c_clip.set_clip(None);
    c_clip.fill_rect_alpha(Rect::new(520.0, 570.0, 40.0, 25.0), 255, 0, 0, 255);
    let px_clip = gpu.render_to_rgba(&c_clip, &bank, LOGICAL_W as u32, LOGICAL_H as u32);
    let px_at = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
        let i = (y * LOGICAL_W + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    assert_eq!(
        px_at(&px_clip, 2, 580)[1],
        0,
        "x=2 left of clip must stay black"
    );
    assert_eq!(
        px_at(&px_clip, 200, 580)[1],
        255,
        "x=200 inside clip must be green"
    );
    assert_eq!(
        px_at(&px_clip, 504, 580)[1],
        255,
        "x=504 inside clip must be green"
    );
    assert_eq!(
        px_at(&px_clip, 510, 580)[1],
        0,
        "x=510 right of clip must stay black"
    );
    assert_eq!(
        px_at(&px_clip, 530, 580)[0],
        255,
        "after set_clip(None), x=530 must draw red"
    );

    // 2. TextTicker scroll equation: starts at x=0, scrolls -30 px/s, wraps to x=380 at -2*single_w.
    assert!((ticker_local_x(0.0, 1000.0) - 0.0).abs() < 1e-4);
    assert!((ticker_local_x(5.0, 1000.0) - -150.0).abs() < 1e-4);
    assert!((ticker_local_x(2000.0 / 30.0 - 0.01, 1000.0) - -1999.7).abs() < 1e-2);
    assert!((ticker_local_x(60.0, 900.0) - 380.0).abs() < 1e-4);
    assert!((ticker_local_x(2000.1 / 30.0, 1000.0) - 379.9).abs() < 1e-2);
    assert!((ticker_local_x(2380.0 / 30.0, 1000.0) - 0.0).abs() < 1e-2);
    assert!((ticker_local_x(4380.3 / 30.0, 1000.0) - 379.7).abs() < 1e-2);

    // 3. Splash white backdrop + Quad.easeOut fade-out overlay over blue Title stage.
    let mut c_sp0 = Canvas::new();
    render_splash(&mut c_sp0, &bank, 0.4);
    let px_sp0 = gpu.render_to_rgba(&c_sp0, &bank, LOGICAL_W as u32, LOGICAL_H as u32);
    let p0 = px_at(&px_sp0, 400, 300);
    assert!(p0[0] > 250 && p0[1] > 250 && p0[2] > 250);

    let mut c_sp1 = Canvas::new();
    c_sp1.clear((0, 0, 200));
    render_splash_overlay(&mut c_sp1, &bank, 4.0);
    let px_sp1 = gpu.render_to_rgba(&c_sp1, &bank, LOGICAL_W as u32, LOGICAL_H as u32);
    let p1 = px_at(&px_sp1, 400, 300);
    assert!(
        p1[0].abs_diff(64) <= 3 && p1[1].abs_diff(64) <= 3 && p1[2] >= 210,
        "expected 0.25 Quad.easeOut white splash overlay over (0,0,200), got {:?}",
        p1
    );
}

#[test]
fn rainbow_opacity_glow_pop_summary_layout_and_champion_brightness_parity() {
    use crate::render::canvas::DrawCmd;
    use crate::render::{LOGICAL_H, LOGICAL_W, draw_rainbow};
    use crate::ui::champion::{ChampionState, render_champion_screen};
    use crate::ui::summary::{SummaryState, render_summary_screen};

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let gpu = test_gpu();
    let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

    let px_at = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
        let i = (y * LOGICAL_W + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };

    // 1. Rainbow: 100% stripe opacity with zero glow during draw (0..0.9s), then #FFFF9D glow pop after 0.9s.
    let rb_core = renderer.bank.rainbow_tex();
    let rb_glow = renderer.bank.rainbow_glow_tex();

    let mut c_rb_mid = Canvas::new();
    c_rb_mid.clear((0, 0, 0));
    draw_rainbow(&mut c_rb_mid, &renderer.bank, 0.8, 1.0);
    assert!(
        c_rb_mid
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteTex(t, _) if *t == rb_core))
    );
    assert!(
        !c_rb_mid
            .cmds
            .iter()
            .any(|c| matches!(c, DrawCmd::SpriteTex(t, _) if *t == rb_glow))
    );
    let px_rb_mid = gpu.render_to_rgba(
        &c_rb_mid,
        &renderer.bank,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
    );
    let stripe0 = px_at(&px_rb_mid, 50, 352);
    assert!(
        stripe0[0] >= 245 && stripe0[1] <= 10 && stripe0[2] <= 10,
        "Rainbow red stripe must render at ~100% opacity, got {:?}",
        stripe0
    );

    let mut c_rb_done = Canvas::new();
    c_rb_done.clear((0, 0, 0));
    draw_rainbow(&mut c_rb_done, &renderer.bank, 1.2, 1.0);
    let glow_idx = c_rb_done
        .cmds
        .iter()
        .position(|c| matches!(c, DrawCmd::SpriteTex(t, _) if *t == rb_glow))
        .expect("Rainbow glow must render after 0.9s");
    let core_idx = c_rb_done
        .cmds
        .iter()
        .position(|c| matches!(c, DrawCmd::SpriteTex(t, _) if *t == rb_core))
        .expect("Rainbow core must render");
    assert!(glow_idx < core_idx);
    let px_rb_done = gpu.render_to_rgba(
        &c_rb_done,
        &renderer.bank,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
    );
    let halo = px_at(&px_rb_done, 50, 340);
    assert!(
        halo[0] > 80 && halo[1] > 80 && halo[2] + 20 < halo[0],
        "Rainbow halo must be warm #FFFF9D glow, got {:?}",
        halo
    );

    // 2. SummaryScreen: right-aligned ScoreCounters + static btnContinue glow after 0.9s.
    let mut sum10 = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 0, 10);
    sum10.result.pipe_bonus = 10;
    sum10.fill_roller.roll_to(10, 0.0, 0.0);
    sum10.fill_roller.update(0.01);
    sum10.age = 4.0;
    let mut c_s10 = Canvas::new();
    render_summary_screen(
        &mut c_s10,
        &renderer.bank,
        &renderer.fonts,
        &sum10,
        (0.0, 0.0),
    );

    let mut sum12340 = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 0, 12340);
    sum12340.result.pipe_bonus = 12340;
    sum12340.fill_roller.roll_to(12340, 0.0, 0.0);
    sum12340.fill_roller.update(0.01);
    sum12340.age = 4.0;
    let mut c_s12340 = Canvas::new();
    render_summary_screen(
        &mut c_s12340,
        &renderer.bank,
        &renderer.fonts,
        &sum12340,
        (0.0, 0.0),
    );

    let fill_digits = |c: &Canvas| -> Vec<(f32, f32)> {
        c.cmds
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::SpriteDirect(_, inst)
                    if inst.row1[2] > 240.0
                        && inst.row1[2] < 285.0
                        && inst.row0[2] > 350.0
                        && inst.params[1] > 0.0 =>
                {
                    Some((inst.row0[2], inst.row0[2] + inst.row0[3]))
                }
                _ => None,
            })
            .collect()
    };
    let d10 = fill_digits(&c_s10);
    let d12340 = fill_digits(&c_s12340);
    assert_eq!(d10.len(), 2);
    assert_eq!(d12340.len(), 5);
    assert!(
        (d10.last().unwrap().1 - d12340.last().unwrap().1).abs() < 0.05,
        "Right-aligned ScoreCounter rightmost glyph must align: {:?} vs {:?}",
        d10,
        d12340
    );
    assert!(d12340[0].0 < d10[0].0 - 20.0);

    let cont_glow = |age: f32| -> f32 {
        let mut s = SummaryState::new(calculate_score(4, 5, 6000, 4, 5, 8000, true), 0, 100);
        s.age = age;
        let mut c = Canvas::new();
        render_summary_screen(&mut c, &renderer.bank, &renderer.fonts, &s, (0.0, 0.0));
        c.cmds
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::SpriteDirect(_, inst)
                    if inst.params[1] < -0.5 && inst.row1[2] >= 440.0 && inst.row1[2] <= 515.0 =>
                {
                    Some(inst.params[3])
                }
                _ => None,
            })
            .expect("Summary btnContinue glow should render")
    };
    assert!((cont_glow(4.0) - 16.0).abs() < 1e-4);
    assert!((cont_glow(4.45) - 16.0).abs() < 1e-4);

    // 3. sprite.wgsl additive brightness (> 1.0) & static ChampionScreen glow/brightness after 0.8s.
    let mut b_blk = empty_bundle();
    let mut black_px = Vec::with_capacity(32 * 32 * 4);
    for _ in 0..(32 * 32) {
        black_px.extend_from_slice(&[0, 0, 0, 255]);
    }
    b_blk.images.insert(
        "black_sq".into(),
        RgbaImage {
            width: 32,
            height: 32,
            data: black_px,
        },
    );
    let bank_blk = TexBank::new(gpu.clone(), &b_blk);
    let blk_tid = bank_blk.named("black_sq").unwrap().0;
    let mut c_bright = Canvas::new();
    c_bright.clear((0, 0, 0));
    c_bright.draw_tex_rect_blur(
        blk_tid,
        Rect::new(100.0, 100.0, 32.0, 32.0),
        None,
        1.8,
        1.0,
        (0.0, 0.0),
    );
    let px_bright = gpu.render_to_rgba(&c_bright, &bank_blk, LOGICAL_W as u32, LOGICAL_H as u32);
    let center = px_at(&px_bright, 116, 116);
    assert!(
        center[0].abs_diff(80) <= 2 && center[1].abs_diff(80) <= 2 && center[2].abs_diff(80) <= 2,
        "Flash ColorMatrixFilter setBrightness(1.8) must add +80 to opaque black, got {:?}",
        center
    );

    let mut rng = StdRng::seed_from_u64(5);
    let mut champ = ChampionState::new(100_000, false, &mut rng);
    let green_tid = renderer.bank.named("GreenTitleBitmap").unwrap().0;
    let mut champ_glow_at = |age: f32| -> f32 {
        champ.age = age;
        let mut c = Canvas::new();
        render_champion_screen(
            &mut c,
            &renderer.bank,
            &renderer.fonts,
            &champ,
            (400.0, 300.0),
        );
        c.cmds
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::SpriteTex(tid, inst) if *tid == green_tid && inst.params[1] < -0.5 => {
                    Some(inst.params[3])
                }
                _ => None,
            })
            .expect("GreenTitleBitmap glow should render")
    };
    assert!((champ_glow_at(5.5) - 32.0).abs() < 1e-4);
    assert!((champ_glow_at(5.9) - 32.0).abs() < 1e-4);
}

#[test]
fn modal_pop_in_pause_scrim_enter_name_clip_and_trophy_tooltip_render_parity() {
    use crate::game::{AwardKind, Difficulty, Trophy};
    use crate::render::canvas::DrawCmd;
    use crate::render::{LOGICAL_H, LOGICAL_W};
    use crate::save::{LevelRecord, ProfileData};
    use crate::ui::modals::{
        build_modal_buttons, options_layout, render_modal_stack, render_pause_menu,
    };
    use crate::ui::widgets::QuestionId;

    let Some(path) = asset_file_if_present() else {
        return;
    };
    let bundle = AssetBundle::load_from_path(path).expect("real assets should load");
    let px_at = |buf: &[u8], x: usize, y: usize| -> [u8; 4] {
        let i = (y * LOGICAL_W + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let gpu = test_gpu();
    let mut app = GameApp::new(bundle, Some(60), gpu.clone()).expect("GameApp init");
    app.save.profiles = vec![ProfileData {
        name: "Tester".into(),
        records: [(
            0,
            LevelRecord {
                best_trophy: Trophy::Gold,
                best_score: 1000,
                best_moves: Some(4),
                best_time_ms: Some(5000),
            },
        )]
        .into_iter()
        .collect(),
        awards: [true, false, false, false],
        ..Default::default()
    }];
    app.save.current_profile = Some("Tester".into());

    // 1. Standard modal pops in from 0.1x scale over 0.52s Back.easeOut; TrophyRoom fades in at 1.0x scale.
    let q_modal = [ModalState::Question {
        id: QuestionId::SimpleNotice,
        message: "Test".into(),
        yes_label: "Ok",
        no_label: None,
    }];
    let render_stack_at = |modals: &[ModalState],
                           ages: &[f32],
                           hover_age: f32,
                           mpos: (f32, f32)|
     -> (Canvas, Vec<u8>) {
        let mut c = Canvas::new();
        c.clear((0, 0, 0));
        render_modal_stack(
            &mut c,
            &app.assets,
            &app.renderer.bank,
            &app.renderer.fonts,
            &app.save,
            modals,
            ages,
            hover_age,
            true,
            false,
            false,
            false,
            0.0,
            mpos,
        );
        let rgba = gpu.render_to_rgba(&c, &app.renderer.bank, LOGICAL_W as u32, LOGICAL_H as u32);
        (c, rgba)
    };

    let (c_q0, _) = render_stack_at(&q_modal, &[0.0], 0.0, (-999.0, -999.0));
    let q0_bubble = c_q0
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            DrawCmd::Shape(sh) if (sh.meta[0] - 2.0).abs() < 0.1 => Some(sh.p0),
            _ => None,
        })
        .expect("Question bubble should emit at t=0");
    assert!(
        (q0_bubble[2] - 40.0).abs() < 1.0,
        "Question modal must start at 0.1x width (~40px), got {:?}",
        q0_bubble
    );

    let (c_q1, _) = render_stack_at(&q_modal, &[0.52], 0.0, (-999.0, -999.0));
    let q1_bubble = c_q1
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            DrawCmd::Shape(sh) if (sh.meta[0] - 2.0).abs() < 0.1 => Some(sh.p0),
            _ => None,
        })
        .expect("Question bubble at 0.52s");
    assert!(
        (q1_bubble[2] - 400.0).abs() < 1.0,
        "Question modal must settle to 400px at 0.52s, got {:?}",
        q1_bubble
    );

    let tr_modal = [ModalState::TrophyRoom {
        page: 0,
        hover_idx: None,
    }];
    let (c_tr0, _) = render_stack_at(&tr_modal, &[0.0], 0.0, (-999.0, -999.0));
    assert!(
        !c_tr0
            .cmds
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::Shape(sh) if (sh.meta[0] - 2.0).abs() < 0.1)),
        "TrophyRoom at age=0 must be invisible (alpha=0)"
    );
    let (c_tr_mid, _) = render_stack_at(&tr_modal, &[0.12], 0.0, (-999.0, -999.0));
    let tr_mid_bubble = c_tr_mid
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            DrawCmd::Shape(sh) if (sh.meta[0] - 2.0).abs() < 0.1 => Some((sh.p0, sh.fill[3])),
            _ => None,
        })
        .expect("TrophyRoom bubble at 0.12s");
    assert!(
        (tr_mid_bubble.0[2] - 716.0).abs() < 0.5 && tr_mid_bubble.1 < 0.9,
        "TrophyRoom must stay at full 716px width while fading alpha, got {:?}",
        tr_mid_bubble
    );

    // 2. PauseMenu uses 0.1 (~26/255) scrim and 1.0s fade-in.
    let mut c_p0 = Canvas::new();
    c_p0.clear((255, 255, 255));
    render_pause_menu(&mut c_p0, &app.renderer.fonts, "Main Menu", (0.0, 0.0), 0.0);
    let px_p0 = gpu.render_to_rgba(
        &c_p0,
        &app.renderer.bank,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
    );
    let bg_p0 = px_at(&px_p0, 20, 20);
    assert!(
        bg_p0[0].abs_diff(229) <= 2,
        "PauseMenu 0.1 scrim over white must leave ~229 (26/255 dim), got {:?}",
        bg_p0
    );
    let pause_has_title = |elapsed: f32| -> bool {
        let mut c = Canvas::new();
        c.clear((0, 0, 0));
        render_pause_menu(
            &mut c,
            &app.renderer.fonts,
            "Main Menu",
            (0.0, 0.0),
            elapsed,
        );
        c.cmds
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::SpriteDirect(_, _)))
    };
    assert!(!pause_has_title(0.0));
    assert!(pause_has_title(0.5));
    assert!(pause_has_title(1.0));

    // 3. Modals are vertically centered and two-button pairs use 10px spacing (x=301 and x=405).
    let opts = options_layout(true);
    assert!(
        ((opts.base_y + opts.bh * 0.5) - 300.0).abs() < 0.5,
        "Options modal should be vertically centered at y=300"
    );
    for m in [
        ModalState::Difficulty {
            selected: Difficulty::Medium,
            ok_only: false,
            trophy_level_idx: None,
        },
        ModalState::InGameDifficulty { level_only: false },
        ModalState::Award {
            kind: AwardKind::LightWater,
            then_champion: false,
            then_ultralux: false,
            then_unlock_match_level: None,
        },
        ModalState::EndlessSetup(Default::default()),
    ] {
        let (c_m, _) = render_stack_at(std::slice::from_ref(&m), &[1.0], 0.0, (-999.0, -999.0));
        let b = c_m
            .cmds
            .iter()
            .find_map(|cmd| match cmd {
                DrawCmd::Shape(sh) if (sh.meta[0] - 2.0).abs() < 0.1 => Some(sh.p0),
                _ => None,
            })
            .unwrap();
        let center_y = b[1] + b[3] * 0.5;
        assert!(
            (center_y - 300.0).abs() <= 1.5,
            "Modal {:?} center_y={center_y} should sit near 300",
            m
        );
        let modal_btns: Vec<_> = build_modal_buttons(
            &m,
            &app.assets,
            &app.renderer.bank,
            &app.renderer.fonts,
            &app.save,
            false,
            false,
        )
        .into_iter()
        .filter(|bt| (bt.rect.w - 94.0).abs() < 0.5)
        .collect();
        if modal_btns.len() == 2 {
            let gap = modal_btns[1].rect.x - (modal_btns[0].rect.x + modal_btns[0].rect.w);
            assert!(
                (gap - 10.0).abs() < 0.1,
                "Modal two-button gap must be 10px, got {gap}"
            );
        }
    }

    // 4. EnterName clips text & caret horizontally inside the white input field.
    let long_name = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW".to_string();
    let en_modal = [ModalState::EnterName {
        prompt: "Enter your name:".into(),
        input: long_name,
        allow_cancel: true,
        creating_for_profile_list: true,
        pending_launch_after: false,
    }];
    let mut c_en = Canvas::new();
    c_en.clear((200, 200, 200));
    render_modal_stack(
        &mut c_en,
        &app.assets,
        &app.renderer.bank,
        &app.renderer.fonts,
        &app.save,
        &en_modal,
        &[1.0],
        0.0,
        true,
        false,
        false,
        false,
        0.0,
        (-999.0, -999.0),
    );
    let px_en = gpu.render_to_rgba(
        &c_en,
        &app.renderer.bank,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
    );
    let clip_rect = c_en
        .cmds
        .iter()
        .find_map(|cmd| match cmd {
            DrawCmd::SetClip(Some(r)) => Some(*r),
            _ => None,
        })
        .expect("EnterName must set scissor clip on the input field");
    let right_x = (clip_rect.x + clip_rect.w).ceil() as usize;
    let clip_mid_y = (clip_rect.y + clip_rect.h * 0.5).round() as usize;
    let leaked_black = ((right_x + 3)..589).chain(602..790).any(|x| {
        let rgb = px_at(&px_en, x, clip_mid_y);
        rgb[0] < 15 && rgb[1] < 15 && rgb[2] < 15
    });
    assert!(
        !leaked_black,
        "EnterName text/caret must clip cleanly at right edge of input box"
    );

    // 5. TrophyRoom tooltip delays 0.3s, fades over 0.5s, and uses 139x80 dimensions.
    let slot0_pos = (100.0, 130.0);
    let tr_hov_modal = [ModalState::TrophyRoom {
        page: 0,
        hover_idx: Some(0),
    }];
    let tooltip_shape = |h_age: f32| -> Option<(f32, f32)> {
        let (c, _) = render_stack_at(&tr_hov_modal, &[1.0], h_age, slot0_pos);
        c.cmds.iter().find_map(|cmd| match cmd {
            DrawCmd::Shape(sh)
                if (sh.meta[0] - 1.0).abs() < 0.1
                    && (sh.p0[3] - 80.0).abs() < 0.5
                    && (sh.p0[2] - 139.0).abs() < 0.5 =>
            {
                Some((sh.p0[2], sh.fill[3]))
            }
            _ => None,
        })
    };
    assert!(
        tooltip_shape(0.25).is_none(),
        "Tooltip must stay hidden before 0.3s delay"
    );
    let mid_tip = tooltip_shape(0.55).expect("Tooltip must fade in at 0.55s");
    assert!(mid_tip.1 > 0.2 && mid_tip.1 < 0.78);
    let full_tip = tooltip_shape(0.9).expect("Tooltip must reach full opacity at 0.8s+");
    assert!((full_tip.0 - 139.0).abs() < 0.1);
    assert!(full_tip.1 >= 0.79);
}

#[test]
fn hdr_pipeline_scrgb_pq_and_emissive_headroom() {
    use crate::render::hdr::HdrPresentParams;
    use crate::render::{
        FireworkBurst, FontFamily, HdrOutputMode, LOGICAL_H, LOGICAL_W, LeakEmitter, TextAlign,
        draw_rainbow,
    };
    use crate::ui::champion::ChampionState;
    use crate::ui::champion::render_champion_screen;

    let gpu = test_gpu();
    let empty = empty_bundle();
    let bank_empty = TexBank::new(gpu.clone(), &empty);
    let hdr_px_at = |buf: &[[f32; 4]], x: usize, y: usize| -> [f32; 4] { buf[y * LOGICAL_W + x] };
    let max_rgb_in = |buf: &[[f32; 4]], x0: usize, y0: usize, x1: usize, y1: usize| -> f32 {
        let mut best = 0.0f32;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = hdr_px_at(buf, x, y);
                best = best.max(p[0]).max(p[1]).max(p[2]);
            }
        }
        best
    };

    // 1. ScRGB calibration, paper-white SDR preservation, modal bubble clamping, and highlight roll-off.
    let mut c_base = Canvas::new();
    c_base.clear((0, 0, 0));
    c_base.fill_rect_alpha(Rect::new(50.0, 50.0, 40.0, 40.0), 255, 255, 255, 255);
    c_base.draw_modal_bubble(Rect::new(150.0, 150.0, 120.0, 80.0));
    c_base.draw_star_glow(400.0, 300.0, 22.0, 1.0);
    c_base.draw_star_orb(520.0, 300.0, 22.0, 1.0);

    let scrgb_4x = HdrPresentParams {
        mode: HdrOutputMode::ScRgb,
        scrgb_white_scale: 1.0,
        sdr_white_nits: 203.0,
        peak_headroom: 4.0,
    };
    let px_4x = gpu.render_to_hdr_f32(
        &c_base,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_4x,
    );
    let black = hdr_px_at(&px_4x, 10, 10);
    assert!(
        black[0].abs() < 1e-4 && black[1].abs() < 1e-4 && black[2].abs() < 1e-4,
        "HDR black must remain 0.0, got {:?}",
        black
    );
    let white = hdr_px_at(&px_4x, 70, 70);
    assert!(
        (white[0] - 1.0).abs() < 0.02
            && (white[1] - 1.0).abs() < 0.02
            && (white[2] - 1.0).abs() < 0.02,
        "ScRGB SDR white with white_scale=1.0 must equal 1.0, got {:?}",
        white
    );
    let modal_max = max_rgb_in(&px_4x, 160, 160, 260, 220);
    assert!(
        modal_max <= 1.02,
        "Modal bubble must remain at paper white <= 1.02, got {modal_max}"
    );
    let glow_peak = max_rgb_in(&px_4x, 385, 285, 415, 315);
    let orb_peak = max_rgb_in(&px_4x, 505, 285, 535, 315);
    assert!(
        glow_peak > 1.08 && orb_peak > 1.8,
        "Star glow ({glow_peak}) and star orb ({orb_peak}) must exceed SDR 1.0"
    );

    // Windows 240-nit SDR white scaling (240 / 80 = 3.0x scRGB).
    let scrgb_win = HdrPresentParams {
        mode: HdrOutputMode::ScRgb,
        scrgb_white_scale: 3.0,
        sdr_white_nits: 240.0,
        peak_headroom: 4.0,
    };
    let px_win = gpu.render_to_hdr_f32(
        &c_base,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_win,
    );
    let white_win = hdr_px_at(&px_win, 70, 70);
    assert!(
        (white_win[0] - 3.0).abs() < 0.05 && (white_win[1] - 3.0).abs() < 0.05,
        "ScRGB white with 3.0x scale must output ~3.0, got {:?}",
        white_win
    );

    // Highlight shoulder roll-off caps peaks cleanly while keeping SDR untouched.
    let px_2x = gpu.render_to_hdr_f32(
        &c_base,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        HdrPresentParams {
            peak_headroom: 2.0,
            ..scrgb_4x
        },
    );
    let px_6x = gpu.render_to_hdr_f32(
        &c_base,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        HdrPresentParams {
            peak_headroom: 6.0,
            ..scrgb_4x
        },
    );
    let orb_2x = max_rgb_in(&px_2x, 505, 285, 535, 315);
    let orb_6x = max_rgb_in(&px_6x, 505, 285, 535, 315);
    assert!(
        orb_2x <= 2.02 && orb_6x > orb_2x + 0.3,
        "Roll-off must cap 2.0x headroom <= 2.02 ({orb_2x}) below 6.0x ({orb_6x})"
    );
    assert!(
        (hdr_px_at(&px_2x, 70, 70)[0] - hdr_px_at(&px_6x, 70, 70)[0]).abs() < 1e-3,
        "Roll-off must not alter SDR pixels <= 1.0"
    );

    // 2. Fireworks + LeakEmitter exceed 1.0 linear in HDR.
    let mut rng = StdRng::seed_from_u64(42);
    let fw = FireworkBurst::new(200.0, 200.0, (255, 255, 255), 1.0, &mut rng);
    let mut c_fx = Canvas::new();
    c_fx.clear((0, 0, 0));
    fw.draw(&mut c_fx, &bank_empty, 1.0);
    let mut leak = LeakEmitter::new((0, 0), 250.0, Side::Down, GameColor::Blue);
    for _ in 0..8 {
        leak.update(1.0 / 30.0);
    }
    leak.draw_at_fx(&mut c_fx, 500.0, 250.0, 1.0, 0.0);
    let px_fx = gpu.render_to_hdr_f32(
        &c_fx,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_4x,
    );
    let fw_peak = max_rgb_in(&px_fx, 150, 150, 250, 250);
    let leak_peak = max_rgb_in(&px_fx, 450, 220, 550, 340);
    assert!(
        fw_peak > 1.6 && leak_peak > 1.2,
        "Fireworks ({fw_peak}) and LeakEmitter ({leak_peak}) must reach HDR headroom"
    );

    // 3. HDR10 ST.2084 PQ encoding produces exact black, paper white (~594/1023), and HDR highlights.
    let pq_params = HdrPresentParams {
        mode: HdrOutputMode::Hdr10Pq,
        scrgb_white_scale: 1.0,
        sdr_white_nits: 203.0,
        peak_headroom: 4.0,
    };
    let pq = gpu.render_to_hdr10_u10(
        &c_base,
        &bank_empty,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        pq_params,
    );
    let pq_at = |x: usize, y: usize| pq[y * LOGICAL_W + x];
    assert_eq!(pq_at(10, 10)[0], 0);
    let pq_white = pq_at(70, 70);
    assert!(
        (590..=598).contains(&pq_white[0])
            && (590..=598).contains(&pq_white[1])
            && (590..=598).contains(&pq_white[2]),
        "HDR10 PQ white at 203 nits should map to ~594/1023, got {:?}",
        pq_white
    );
    let pq_slice = pq.as_slice();
    let pq_orb_peak = (505..535)
        .flat_map(|x| (285..315).map(move |y| pq_slice[y * LOGICAL_W + x][0]))
        .max()
        .unwrap_or(0);
    assert!(
        pq_orb_peak > 640 && pq_orb_peak < 1023,
        "HDR10 PQ emissive star orb must exceed SDR 594 code, got {pq_orb_peak}"
    );

    // 4. Real-asset HDR coverage: Rainbow glow, active pipe liquid, Champion +80%, and UI text ceiling.
    if let Some(path) = asset_file_if_present() {
        let bundle = AssetBundle::load_from_path(path).expect("real assets load");
        let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

        let mut c_real = Canvas::new();
        c_real.clear((0, 0, 0));
        draw_rainbow(&mut c_real, &renderer.bank, 1.0, 1.0);
        let px_rb = gpu.render_to_hdr_f32(
            &c_real,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let rb_peak = max_rgb_in(&px_rb, 50, 120, 750, 560);
        assert!(
            rb_peak > 1.45,
            "Rainbow win glow must reach HDR, got {rb_peak}"
        );

        let mut tile = Tile::from_def(
            &TileDef {
                x: 0,
                y: 0,
                shape: PieceShape::Straight,
                piece_type: PieceType::Pipe,
                direction: 0,
                is_moveable: true,
                can_rotate: true,
                is_gate: false,
                is_freeze: false,
                is_magic: false,
                secondary_color: None,
            },
            &minimal_test_level(),
        );
        tile.liquid_color = GameColor::Blue;
        tile.fills.push(crate::game::FillPass {
            entry: Side::Up,
            exit: Side::Down,
            progress: 1.0,
            elapsed: 1.0,
            completed: true,
        });
        let mut c_pipe = Canvas::new();
        c_pipe.clear((0, 0, 0));
        let full_tid = renderer.bank.named("StraightFullBitmap").unwrap().0;
        let (caps, radii, count) =
            crate::render::pipe_mask::build_pipe_capsules(&tile.fills, tile.direction);
        c_pipe.draw_pipe_masked(
            full_tid,
            400.0,
            300.0,
            128.0,
            128.0,
            tile.visual_rot_deg,
            false,
            caps,
            radii,
            count,
            tile.liquid_color,
            1.0,
            (0.0, 0.0),
        );
        let px_pipe = gpu.render_to_hdr_f32(
            &c_pipe,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let pipe_peak = max_rgb_in(&px_pipe, 340, 240, 460, 360);
        assert!(
            pipe_peak > 1.15,
            "Pipe liquid must reach subtle HDR glow, got {pipe_peak}"
        );

        let mut c_text = Canvas::new();
        c_text.clear((0, 0, 0));
        renderer.fonts.draw(
            &mut c_text,
            FontFamily::Neuropol,
            400.0,
            280.0,
            32.0,
            (255, 255, 255),
            TextAlign::Center,
            "Options",
        );
        let px_txt = gpu.render_to_hdr_f32(
            &c_text,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let txt_peak = max_rgb_in(&px_txt, 250, 260, 550, 340);
        assert!(
            txt_peak <= 1.02,
            "UI text must stay strictly at paper white <= 1.02, got {txt_peak}"
        );

        let mut champ = ChampionState::new(100_000, false, &mut rng);
        champ.age = 5.0;
        let mut c_ch = Canvas::new();
        c_ch.clear((0, 0, 0));
        render_champion_screen(
            &mut c_ch,
            &renderer.bank,
            &renderer.fonts,
            &champ,
            (400.0, 300.0),
        );
        let px_ch = gpu.render_to_hdr_f32(
            &c_ch,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let ch_peak = max_rgb_in(&px_ch, 100, 80, 700, 360);
        assert!(
            ch_peak > 1.25,
            "Champion title/glow must reach HDR, got {ch_peak}"
        );
    }
}

#[test]
fn hdr_bloom_wcg_and_specular_sheen() {
    use crate::render::hdr::HdrPresentParams;
    use crate::render::{HdrOutputMode, LOGICAL_H, LOGICAL_W, draw_sprite_title};

    let gpu = test_gpu();
    let hdr_px_at = |buf: &[[f32; 4]], x: usize, y: usize| -> [f32; 4] { buf[y * LOGICAL_W + x] };
    let max_rgb_in = |buf: &[[f32; 4]], x0: usize, y0: usize, x1: usize, y1: usize| -> f32 {
        let mut best = 0.0f32;
        for y in y0..y1 {
            for x in x0..x1 {
                let p = hdr_px_at(buf, x, y);
                best = best.max(p[0]).max(p[1]).max(p[2]);
            }
        }
        best
    };

    let mut b_syn = empty_bundle();
    let mut green_px = Vec::with_capacity(32 * 32 * 4);
    let mut white_px = Vec::with_capacity(32 * 32 * 4);
    let mut twotone_px = Vec::with_capacity(64 * 32 * 4);
    for _y in 0..32 {
        for _x in 0..32 {
            green_px.extend_from_slice(&[0, 255, 0, 255]);
            white_px.extend_from_slice(&[255, 255, 255, 255]);
        }
        for x in 0..64 {
            if x < 32 {
                twotone_px.extend_from_slice(&[30, 30, 30, 255]);
            } else {
                twotone_px.extend_from_slice(&[255, 255, 255, 255]);
            }
        }
    }
    b_syn.images.insert(
        "green".into(),
        RgbaImage {
            width: 32,
            height: 32,
            data: green_px,
        },
    );
    b_syn.images.insert(
        "white".into(),
        RgbaImage {
            width: 32,
            height: 32,
            data: white_px,
        },
    );
    b_syn.images.insert(
        "twotone".into(),
        RgbaImage {
            width: 64,
            height: 32,
            data: twotone_px,
        },
    );
    let bank_syn = TexBank::new(gpu.clone(), &b_syn);
    let green_tid = bank_syn.named("green").unwrap().0;
    let white_tid = bank_syn.named("white").unwrap().0;
    let twotone_tid = bank_syn.named("twotone").unwrap().0;

    let scrgb_4x = HdrPresentParams {
        mode: HdrOutputMode::ScRgb,
        scrgb_white_scale: 1.0,
        sdr_white_nits: 203.0,
        peak_headroom: 4.0,
    };

    // 1. Half-res thresholded HDR bloom produces halo around emissive sprite, zero haze near SDR white.
    let mut c_bloom = Canvas::new();
    c_bloom.clear((0, 0, 0));
    c_bloom.fill_rect_alpha(Rect::new(50.0, 50.0, 40.0, 40.0), 255, 255, 255, 255);
    c_bloom.draw_tex_rect(
        white_tid,
        Rect::new(500.0, 290.0, 20.0, 20.0),
        None,
        1.0,
        1.0,
    );
    c_bloom.boost_last_sprite(1.65);
    let px_bloom = gpu.render_to_hdr_f32(
        &c_bloom,
        &bank_syn,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_4x,
    );
    let near_sdr = hdr_px_at(&px_bloom, 95, 70);
    assert!(
        near_sdr[0].abs() < 1e-4 && near_sdr[1].abs() < 1e-4 && near_sdr[2].abs() < 1e-4,
        "SDR rect must cast zero bloom haze, got {:?}",
        near_sdr
    );
    let emissive_halo = hdr_px_at(&px_bloom, 526, 300);
    assert!(
        emissive_halo[0] > 0.03,
        "Emissive sprite must cast smooth HDR bloom halo outside its edge, got {:?}",
        emissive_halo
    );

    // 2. Wide Color Gamut expands saturated emissive greens beyond BT.709 into Display P3 / BT.2020.
    let mut c_wcg = Canvas::new();
    c_wcg.clear((0, 0, 0));
    c_wcg.draw_tex_rect(
        green_tid,
        Rect::new(200.0, 200.0, 40.0, 40.0),
        None,
        1.0,
        1.0,
    );
    c_wcg.boost_last_sprite(1.65);
    c_wcg.fill_rect_alpha(Rect::new(50.0, 50.0, 40.0, 40.0), 255, 255, 255, 255);

    let px_wcg = gpu.render_to_hdr_f32(
        &c_wcg,
        &bank_syn,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_4x,
    );
    let green_scrgb = hdr_px_at(&px_wcg, 220, 220);
    assert!(
        green_scrgb[1] > 1.8 && green_scrgb[0] < -0.02,
        "Wide-gamut scRGB green must push past BT.709 (negative red primary), got {:?}",
        green_scrgb
    );
    let white_scrgb = hdr_px_at(&px_wcg, 70, 70);
    assert!(
        (white_scrgb[0] - white_scrgb[1]).abs() < 0.01 && (white_scrgb[1] - 1.0).abs() < 0.02,
        "Neutral SDR white must remain un-shifted at 1.0, got {:?}",
        white_scrgb
    );
    let pq_wcg = gpu.render_to_hdr10_u10(
        &c_wcg,
        &bank_syn,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        HdrPresentParams {
            mode: HdrOutputMode::Hdr10Pq,
            ..scrgb_4x
        },
    );
    let green_pq = pq_wcg[220 * LOGICAL_W + 220];
    assert!(
        green_pq[1] > 640 && green_pq[1] <= 1023,
        "HDR10 BT.2020 green PQ code must exceed SDR white, got {:?}",
        green_pq
    );

    // 3. Luminance-gated specular sheen keeps SDR bit-identical and dark tones grounded in HDR.
    let mut c_sheen = Canvas::new();
    c_sheen.clear((0, 0, 0));
    c_sheen.draw_tex_rect(
        twotone_tid,
        Rect::new(100.0, 100.0, 64.0, 32.0),
        None,
        1.0,
        1.0,
    );
    c_sheen.sheen_last_sprite(1.42);
    let sdr_sheen = gpu.render_to_rgba(&c_sheen, &bank_syn, LOGICAL_W as u32, LOGICAL_H as u32);
    let sdr_dark = &sdr_sheen[(116 * LOGICAL_W + 108) * 4..(116 * LOGICAL_W + 108) * 4 + 3];
    let sdr_hi = &sdr_sheen[(116 * LOGICAL_W + 148) * 4..(116 * LOGICAL_W + 148) * 4 + 3];
    assert!(
        sdr_dark[0].abs_diff(30) <= 1 && sdr_hi[0] == 255,
        "SDR mode must ignore sheen tag bit-identically, got dark={:?} hi={:?}",
        sdr_dark,
        sdr_hi
    );
    let hdr_sheen = gpu.render_to_hdr_f32(
        &c_sheen,
        &bank_syn,
        LOGICAL_W as u32,
        LOGICAL_H as u32,
        scrgb_4x,
    );
    let dark_lin = hdr_px_at(&hdr_sheen, 108, 116);
    let hi_lin = hdr_px_at(&hdr_sheen, 148, 116);
    assert!(
        dark_lin[0] <= 0.025 && hi_lin[0] > 1.35,
        "HDR sheen must keep dark ink grounded ({dark_lin:?}) while lifting specular highlights ({hi_lin:?})",
    );

    // 4. Real-asset Title logo and Platinum trophy sheen in HDR.
    if let Some(path) = asset_file_if_present() {
        let bundle = AssetBundle::load_from_path(path).expect("real assets load");
        let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");

        let mut c_title = Canvas::new();
        c_title.clear((0, 0, 0));
        draw_sprite_title(&mut c_title, &bundle, &renderer.bank, 400.0, 120.0, 6.0);
        let px_title = gpu.render_to_hdr_f32(
            &c_title,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let title_peak = max_rgb_in(&px_title, 100, 40, 700, 240);
        assert!(
            title_peak > 1.25,
            "Illuminated Title logo specular sheen must reach HDR, got {title_peak}"
        );

        let mut c_trop = Canvas::new();
        c_trop.clear((0, 0, 0));
        if let Some((tid, _, _)) = renderer.bank.named("PlatinumTrophyBitmap") {
            c_trop.draw_tex_rect(tid, Rect::new(350.0, 250.0, 100.0, 100.0), None, 1.0, 1.0);
            c_trop.sheen_last_sprite(1.45);
        }
        let px_trop = gpu.render_to_hdr_f32(
            &c_trop,
            &renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
            scrgb_4x,
        );
        let trop_peak = max_rgb_in(&px_trop, 350, 250, 450, 350);
        assert!(
            trop_peak > 1.25,
            "Platinum trophy specular sheen must reach HDR, got {trop_peak}"
        );
    }
}

#[test]
fn skipped_sprite_draws_do_not_boost_prior_snowy_hills_sprite_in_hdr() {
    use crate::render::canvas::DrawCmd;
    use crate::render::hdr::HdrPresentParams;
    use crate::render::{BgAnimState, HdrOutputMode, LOGICAL_H, LOGICAL_W};
    use crate::ui::summary::{SummaryHideAction, SummaryState, render_summary_screen};

    let gpu = test_gpu();
    let tid = TexBank::new(gpu.clone(), &empty_bundle()).rainbow_tex();

    // 1. Skipped draw_rainbow_tex must not leak boost_last_sprite onto the previous sprite.
    let mut c1 = Canvas::new();
    c1.draw_tex_rect(tid, Rect::new(0.0, 0.0, 100.0, 100.0), None, 1.0, 1.0);
    c1.draw_rainbow_tex(tid, 1.0, 0.005, (0.0, 0.0));
    c1.boost_last_sprite(1.50);
    c1.boost_last_sprite(1.28);
    assert_eq!(c1.cmds.len(), 1);
    match &c1.cmds[0] {
        DrawCmd::SpriteTex(_, inst) => assert_eq!(inst.tint, [1.0, 1.0, 1.0, 0.0]),
        _ => panic!("expected SpriteTex"),
    }

    // 2. Skipped draw_tex_rect_blur must not leak sheen_last_sprite onto the previous sprite.
    let mut c2 = Canvas::new();
    c2.draw_tex_rect(tid, Rect::new(0.0, 0.0, 100.0, 100.0), None, 1.0, 1.0);
    c2.draw_tex_rect_blur(
        tid,
        Rect::new(10.0, 10.0, 50.0, 50.0),
        None,
        1.0,
        0.002,
        (0.0, 0.0),
    );
    c2.sheen_last_sprite(1.45);
    assert_eq!(c2.cmds.len(), 1);
    match &c2.cmds[0] {
        DrawCmd::SpriteTex(_, inst) => assert_eq!(inst.tint, [1.0, 1.0, 1.0, 0.0]),
        _ => panic!("expected SpriteTex"),
    }

    // 3. Double boost on one pushed sprite applies only once.
    let mut c3 = Canvas::new();
    c3.draw_tex_rect(tid, Rect::new(0.0, 0.0, 100.0, 100.0), None, 1.0, 1.0);
    c3.boost_last_sprite(1.50);
    c3.boost_last_sprite(1.50);
    match &c3.cmds[0] {
        DrawCmd::SpriteTex(_, inst) => assert_eq!(inst.tint, [1.50, 1.50, 1.50, 0.0]),
        _ => panic!("expected SpriteTex"),
    }

    // 4. Real-asset SnowyHills + SummaryState late hide preserves SDR luminance on left hill.
    if let Some(path) = asset_file_if_present() {
        let bundle = AssetBundle::load_from_path(path).expect("real assets load");
        let renderer = Renderer::new(gpu.clone(), &bundle).expect("renderer");
        let mut bg = BgAnimState::new("SnowyHills");
        bg.age = 6.0;
        bg.darken = 0.2;

        let res = calculate_score(4, 10, 5000, 4, 10, 10000, false);
        let scrgb_4x = HdrPresentParams {
            mode: HdrOutputMode::ScRgb,
            scrgb_white_scale: 1.0,
            sdr_white_nits: 203.0,
            peak_headroom: 4.0,
        };

        for h in [1.88f32, 1.95f32] {
            let mut summary = SummaryState::new(res, 0, 1000);
            summary.age = 5.0;
            summary.hiding = Some((h, SummaryHideAction::NextLevel));

            let mut c = Canvas::new();
            renderer.draw_background(&mut c, &bundle, &bg);
            let bg_cmd_count = c.cmds.len();
            render_summary_screen(
                &mut c,
                &renderer.bank,
                &renderer.fonts,
                &summary,
                (-999.0, -999.0),
            );

            for cmd in &c.cmds[..bg_cmd_count] {
                if let DrawCmd::SpriteTex(_, inst) = cmd {
                    assert!(
                        inst.tint[0] >= 0.0 && inst.tint[0] <= 1.001,
                        "SnowyHills background sprite tint corrupted at h={h}: {:?}",
                        inst.tint
                    );
                }
            }

            let px = gpu.render_to_hdr_f32(
                &c,
                &renderer.bank,
                LOGICAL_W as u32,
                LOGICAL_H as u32,
                scrgb_4x,
            );
            let mut left_hill_peak = 0.0f32;
            for y in 420..580 {
                for x in 20..160 {
                    let p = px[y * LOGICAL_W + x];
                    left_hill_peak = left_hill_peak.max(p[0]).max(p[1]).max(p[2]);
                }
            }
            assert!(
                left_hill_peak <= 1.02,
                "Left SnowyHills hill must stay SDR-grounded before transition (h={h}), got {left_hill_peak}"
            );
        }
    }
}
