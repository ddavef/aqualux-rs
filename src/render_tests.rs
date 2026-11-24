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
