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
