// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

#[cfg(test)]
mod anim_tests;
mod assets;
mod audio;
mod custom;
#[cfg(test)]
mod custom_tests;
mod game;
#[cfg(test)]
mod highlight_tests;
#[cfg(test)]
mod parity_tests;
mod render;
#[cfg(test)]
mod render_tests;
mod save;
#[cfg(test)]
mod tests;
mod ui;
mod window;

use crate::assets::{AssetBundle, extract_assets_to_dir};
use crate::game::{Difficulty, LevelSim, MetricResult, Trophy};
use crate::render::{Canvas, GpuCore, LOGICAL_H, LOGICAL_W};
use crate::ui::champion::ChampionState;
use crate::ui::summary::SummaryState;
use crate::ui::widgets::ModalState;
use crate::ui::{GameApp, Screen};
use rand::SeedableRng;
use std::path::PathBuf;
use std::sync::Arc;

struct CliArgs {
    assets_path: Option<PathBuf>,
    extract_dir: Option<PathBuf>,
    custom_levels_dir: Option<PathBuf>,
    verify_assets: bool,
    fps_override: Option<u32>,
}

fn parse_fps_str(s: &str) -> Result<u32, String> {
    let v: u32 = s.trim().parse().map_err(|_| {
        format!(
            "Invalid --fps value '{}': expected 0 (uncapped) or 15..=1000",
            s
        )
    })?;
    if v != 0 && !(15..=1000).contains(&v) {
        return Err(format!(
            "--fps value {} out of range: expected 0 (uncapped) or 15..=1000",
            v
        ));
    }
    Ok(v)
}

fn parse_cli() -> Result<CliArgs, String> {
    let mut args = std::env::args().skip(1);
    let mut fps_override = std::env::var("AQUALUX_FPS")
        .ok()
        .map(|v| parse_fps_str(&v))
        .transpose()?;
    let env_custom = std::env::var_os("AQUALUX_CUSTOM_LEVELS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let mut out = CliArgs {
        assets_path: None,
        extract_dir: None,
        custom_levels_dir: env_custom,
        verify_assets: false,
        fps_override: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--assets" => {
                let val = args.next().ok_or("--assets requires a path argument")?;
                out.assets_path = Some(PathBuf::from(val));
            }
            "--fps" => {
                let val = args.next().ok_or("--fps requires a numeric argument")?;
                fps_override = Some(parse_fps_str(&val)?);
            }
            "--extract-assets" => {
                let val = args
                    .next()
                    .ok_or("--extract-assets requires an output directory")?;
                out.extract_dir = Some(PathBuf::from(val));
            }
            "--custom-levels" => {
                let val = args
                    .next()
                    .ok_or("--custom-levels requires a directory path")?;
                out.custom_levels_dir = Some(PathBuf::from(val));
            }
            "--verify-assets" => out.verify_assets = true,
            "--help" | "-h" => {
                println!(
                    "aqualux-rs\n\nOptions:\n  --assets <PATH>          Load original resource.n or aqualux.swf (required)\n  --fps <N>                Target frame rate (15..=1000, or 0 for uncapped)\n  --extract-assets <DIR>   Extract PNGs/MP3s/XMLs to <DIR> and exit\n  --custom-levels <DIR>    Override custom level packs directory\n  --verify-assets          Verify assets, simulate all levels, render all screens, exit"
                );
                std::process::exit(0);
            }
            other => return Err(format!("Unknown argument: {}", other)),
        }
    }
    out.fps_override = fps_override;
    Ok(out)
}

fn verify_bundle(bundle: &AssetBundle, fps_override: Option<u32>) -> Result<(), String> {
    if bundle.campaign_levels.is_empty() {
        return Err("No campaign levels loaded".to_string());
    }
    for (idx, lvl) in bundle.campaign_levels.iter().enumerate() {
        let mut sim = LevelSim::new(lvl.clone(), Difficulty::Medium);
        sim.start_level();
        let _ = sim.update(0.016);
        if sim.tiles.is_empty() || sim.tiles[0].is_empty() {
            return Err(format!("Level {} failed to build tile grid", idx));
        }
    }

    let gpu = Arc::new(GpuCore::new_headless()?);
    let mut app = GameApp::new(bundle.clone_for_verify(), fps_override, gpu.clone())?;
    let mut canvas = Canvas::new();

    let check_screen = |app: &mut GameApp, canvas: &mut Canvas| {
        canvas.begin_frame(1.0);
        app.render(canvas);
        let rgba = gpu.render_to_rgba(
            canvas,
            &app.renderer.bank,
            LOGICAL_W as u32,
            LOGICAL_H as u32,
        );
        assert!(rgba.len() == LOGICAL_W * LOGICAL_H * 4);
    };

    // Splash -> Title -> TrophyRoom -> Options -> Campaign -> Match -> Mirror -> Pause -> Summary -> Champion
    check_screen(&mut app, &mut canvas);
    app.screen = Screen::Title;
    app.title_age = 6.0;
    check_screen(&mut app, &mut canvas);
    app.modals.push(ModalState::TrophyRoom {
        page: 0,
        hover_idx: None,
    });
    check_screen(&mut app, &mut canvas);
    app.modals.clear();
    app.modals.push(ModalState::Options {
        allow_difficulty: true,
    });
    check_screen(&mut app, &mut canvas);
    app.modals.clear();
    app.start_match_level(1, false, true);
    check_screen(&mut app, &mut canvas);
    app.start_match_level(2, true, false);
    check_screen(&mut app, &mut canvas);
    app.start_campaign_level(0, true);
    check_screen(&mut app, &mut canvas);
    app.screen = Screen::Paused;
    check_screen(&mut app, &mut canvas);
    app.summary_state = Some(SummaryState::new(
        MetricResult {
            trophy: Trophy::Gold,
            total_score: 1100,
            pipe_bonus: 500,
            move_bonus: 200,
            time_bonus: 400,
        },
        0,
        1100,
    ));
    app.screen = Screen::Summary;
    check_screen(&mut app, &mut canvas);
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    app.champion_state = Some(ChampionState::new(50000, false, &mut rng));
    app.screen = Screen::Champion;
    check_screen(&mut app, &mut canvas);

    let bench_gpu = |app: &mut GameApp, canvas: &mut Canvas, w: u32, h: u32| -> f64 {
        let dpr = (w as f32 / LOGICAL_W as f32).min(h as f32 / LOGICAL_H as f32);
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("verify_bench"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        canvas.begin_frame(dpr);
        app.render(canvas);
        gpu.render_frame(canvas, &app.renderer.bank, &view, w, h);
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let t0 = std::time::Instant::now();
        for _ in 0..100 {
            canvas.begin_frame(dpr);
            app.render(canvas);
            gpu.render_frame(canvas, &app.renderer.bank, &view, w, h);
        }
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        t0.elapsed().as_secs_f64() * 1000.0 / 100.0
    };

    app.modals.clear();
    app.screen = Screen::Title;
    app.bg_anim.age = 7.0;
    app.title_age = 7.0;
    let title_ms = bench_gpu(&mut app, &mut canvas, 800, 600);
    let title_hd_ms = bench_gpu(&mut app, &mut canvas, 1920, 1080);

    app.start_campaign_level(0, false);
    if let Some(sim) = &mut app.campaign_sim {
        let _ = sim.skip_tutorials();
    }
    app.board_pop_elapsed = None;
    app.bg_anim.age = 7.0;
    let board_ms = bench_gpu(&mut app, &mut canvas, 800, 600);
    let board_hd_ms = bench_gpu(&mut app, &mut canvas, 1920, 1080);

    app.screen = Screen::Title;
    app.modals.push(ModalState::TrophyRoom {
        page: 0,
        hover_idx: None,
    });
    let trophy_ms = bench_gpu(&mut app, &mut canvas, 800, 600);
    let trophy_hd_ms = bench_gpu(&mut app, &mut canvas, 1920, 1080);

    println!(
        "Verified source='{}': images={}, cid_images={}, sprites={}, fonts={}, sounds={}, campaign_levels={}, lotd_levels={}",
        bundle.source_label,
        bundle.images.len(),
        bundle.cid_images.len(),
        bundle.sprites.len(),
        bundle.fonts.len(),
        bundle.sounds.len(),
        bundle.campaign_levels.len(),
        bundle.lotd_levels.len()
    );
    println!(
        "GPU 800x600 avg: Title={:.3}ms ({:.0}fps), Campaign={:.3}ms ({:.0}fps), TrophyRoom={:.3}ms ({:.0}fps)",
        title_ms,
        1000.0 / title_ms.max(0.001),
        board_ms,
        1000.0 / board_ms.max(0.001),
        trophy_ms,
        1000.0 / trophy_ms.max(0.001),
    );
    println!(
        "GPU 1920x1080 avg: Title={:.3}ms ({:.0}fps), Campaign={:.3}ms ({:.0}fps), TrophyRoom={:.3}ms ({:.0}fps)",
        title_hd_ms,
        1000.0 / title_hd_ms.max(0.001),
        board_hd_ms,
        1000.0 / board_hd_ms.max(0.001),
        trophy_hd_ms,
        1000.0 / trophy_hd_ms.max(0.001),
    );
    Ok(())
}

fn main() {
    let cli = match parse_cli() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let bundle = match AssetBundle::discover(cli.assets_path.as_deref()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Failed to load required assets: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(out_dir) = cli.extract_dir {
        if let Err(e) = extract_assets_to_dir(&bundle, &out_dir) {
            eprintln!("Extraction failed: {}", e);
            std::process::exit(1);
        }
        println!(
            "Extracted {} images, {} sounds, {} XML files to '{}'",
            bundle.images.len(),
            bundle.sounds.len(),
            bundle.raw_xmls.len(),
            out_dir.display()
        );
        return;
    }

    if cli.verify_assets {
        if let Err(e) = verify_bundle(&bundle, cli.fps_override) {
            eprintln!("Verification failed: {}", e);
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = window::run_window(bundle, cli.fps_override, cli.custom_levels_dir) {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}
