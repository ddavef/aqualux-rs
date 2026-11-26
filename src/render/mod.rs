// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

pub mod backgrounds;
mod bg_sprites;
pub mod board;
pub(crate) mod board_underlay;
pub mod canvas;
pub mod color_matrix;
pub mod cursor;
pub mod easing;
pub mod effects;
pub mod emitter;
pub mod gpu_bake;
pub mod gpu_core;
pub mod gpu_pipelines;
pub mod gpu_text;
pub mod gpu_types;
pub mod layout;
pub mod particles;
pub mod pipe_mask;
pub mod tex_bank;
pub mod text;

pub use backgrounds::{BackgroundRenderer, BgAnimState};
pub use bg_sprites::draw_sprite_title;
pub use board::BoardAnim;
pub use canvas::{Canvas, LOGICAL_H, LOGICAL_W};
pub use cursor::{CursorKind, draw_cursor};
pub use effects::{
    ScoreBlurbFx, draw_fireworks, draw_rainbow, draw_score_blurbs, draw_tutorial_bubble,
};
pub use emitter::LeakEmitter;
pub use gpu_core::GpuCore;
pub use layout::{BoardLayout, Rect};
pub use particles::{FireworkBurst, StarField};
pub use tex_bank::{GlowPreset, TexBank};
pub use text::{FontFamily, FontSet, TextAlign, TextOutline};

use crate::assets::AssetBundle;
use crate::game::Tile;
use board::BoardRenderer;
use std::sync::Arc;

pub struct Renderer {
    pub bank: TexBank,
    pub fonts: FontSet,
    pub backgrounds: BackgroundRenderer,
    pub board: BoardRenderer,
}

impl Renderer {
    pub fn new(gpu: Arc<GpuCore>, assets: &AssetBundle) -> Result<Self, String> {
        Ok(Self {
            bank: TexBank::new(gpu.clone(), assets),
            fonts: FontSet::new(gpu, assets)?,
            backgrounds: BackgroundRenderer::new(),
            board: BoardRenderer::new(),
        })
    }

    pub fn draw_background(&self, canvas: &mut Canvas, assets: &AssetBundle, anim: &BgAnimState) {
        self.backgrounds
            .draw_background(canvas, assets, &self.bank, anim);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw_board(
        &mut self,
        canvas: &mut Canvas,
        tiles: &[Vec<Tile>],
        layout: BoardLayout,
        selected: Option<(usize, usize)>,
        swap_eligible: &[(usize, usize)],
        hover: Option<(usize, usize)>,
        tutorial_tiles: &[(usize, usize)],
        leaks: &[LeakEmitter],
        color_blind: bool,
        mirror_x: bool,
        anim: BoardAnim,
        time: f32,
    ) {
        self.board.draw_board(
            canvas,
            &self.bank,
            &self.fonts,
            tiles,
            layout,
            selected,
            swap_eligible,
            hover,
            tutorial_tiles,
            leaks,
            color_blind,
            mirror_x,
            anim,
            time,
        );
    }
}
