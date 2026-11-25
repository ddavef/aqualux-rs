// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::assets::AssetBundle;
use crate::game::Side;
use crate::render::cursor::CursorKind;
use crate::render::gpu_bake::{
    GpuTexture, bake_blur_texture, bake_cursor_texture, bake_fire_star_texture,
    bake_path_highlight_texture, bake_rainbow_texture, upload_premul_texture,
};
use crate::render::gpu_core::GpuCore;
use crate::render::gpu_types::TexId;
use rustc_hash::FxHashMap;
use std::cell::{Ref, RefCell};
use std::sync::Arc;

pub const CURSOR_PAD: u32 = 16;
pub const PATH_HL_PAD: u32 = 16;
pub const TITLE_SHADOW_PAD: u32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlowPreset {
    ButtonWhite,
    FlowTimerYellow,
    TrophyYellow,
    TrophyShadow,
    PipeDropShadow,
    PipeHover,
    PipeSelected,
    PipeAccent,
    PipeTutorial,
}

impl GlowPreset {
    pub fn pad(self) -> u32 {
        match self {
            Self::PipeAccent => 6,
            Self::ButtonWhite | Self::TrophyYellow | Self::TrophyShadow | Self::PipeHover => 10,
            Self::FlowTimerYellow
            | Self::PipeDropShadow
            | Self::PipeSelected
            | Self::PipeTutorial => 16,
        }
    }
}

type GlowCache = RefCell<FxHashMap<(String, GlowPreset), (TexId, u32, u32, u32)>>;
type PathHlCache = RefCell<FxHashMap<(Side, Side), (TexId, u32, u32)>>;
type FireStarCache = RefCell<FxHashMap<(u8, u8, u8), (TexId, f32)>>;

pub struct TexBank {
    gpu: Arc<GpuCore>,
    textures: RefCell<Vec<Option<GpuTexture>>>,
    named: FxHashMap<String, (TexId, u32, u32)>,
    cid: FxHashMap<u16, (TexId, u32, u32)>,
    cid_shadows: RefCell<FxHashMap<u16, (TexId, u32, u32, u32)>>,
    glows: GlowCache,
    path_hl: PathHlCache,
    rainbow: RefCell<Option<TexId>>,
    fire_stars: FireStarCache,
    cursors: FxHashMap<CursorKind, (TexId, u32, u32, u32)>,
}

impl TexBank {
    pub fn new(gpu: Arc<GpuCore>, assets: &AssetBundle) -> Self {
        let mut textures: Vec<Option<GpuTexture>> = Vec::new();
        fn push_tex(textures: &mut Vec<Option<GpuTexture>>, tex: GpuTexture) -> (TexId, u32, u32) {
            let id = TexId(textures.len() as u32);
            let dims = (tex.width, tex.height);
            textures.push(Some(tex));
            (id, dims.0, dims.1)
        }

        let mut named = FxHashMap::default();
        for (k, img) in &assets.images {
            let t = upload_premul_texture(&gpu, img);
            named.insert(k.clone(), push_tex(&mut textures, t));
        }

        let mut cid = FxHashMap::default();
        for (&id, img) in &assets.cid_images {
            let t = upload_premul_texture(&gpu, img);
            cid.insert(id, push_tex(&mut textures, t));
        }

        let mut cursors = FxHashMap::default();
        for kind in [
            CursorKind::Arrow,
            CursorKind::Move,
            CursorKind::Rotate,
            CursorKind::Stop,
            CursorKind::Wait,
        ] {
            if let Some(&(src_id, _, _)) = named.get(kind.bitmap_name())
                && let Some(baked) = textures
                    .get(src_id.0 as usize)
                    .and_then(|o| o.as_ref())
                    .map(|src_tex| bake_cursor_texture(&gpu, src_tex, CURSOR_PAD))
            {
                let (bw, bh) = (baked.width, baked.height);
                let (tid, _, _) = push_tex(&mut textures, baked);
                cursors.insert(kind, (tid, bw, bh, CURSOR_PAD));
            }
        }

        Self {
            gpu,
            textures: RefCell::new(textures),
            named,
            cid,
            cid_shadows: RefCell::new(FxHashMap::default()),
            glows: RefCell::new(FxHashMap::default()),
            path_hl: RefCell::new(FxHashMap::default()),
            rainbow: RefCell::new(None),
            fire_stars: RefCell::new(FxHashMap::default()),
            cursors,
        }
    }

    fn alloc_tex(&self, tex: GpuTexture) -> (TexId, u32, u32) {
        let (w, h) = (tex.width, tex.height);
        let mut list = self.textures.borrow_mut();
        let tid = TexId(list.len() as u32);
        list.push(Some(tex));
        (tid, w, h)
    }

    pub fn textures_ref(&self) -> Ref<'_, Vec<Option<GpuTexture>>> {
        self.textures.borrow()
    }

    pub fn named(&self, key: &str) -> Option<(TexId, u32, u32)> {
        self.named.get(key).copied()
    }

    pub fn cid(&self, cid: u16) -> Option<(TexId, u32, u32)> {
        self.cid.get(&cid).copied()
    }

    pub fn cursor(&self, kind: CursorKind) -> Option<(TexId, u32, u32, u32)> {
        self.cursors.get(&kind).copied()
    }

    pub fn cid_shadow(&self, cid: u16) -> Option<(TexId, u32, u32, u32)> {
        if let Some(&hit) = self.cid_shadows.borrow().get(&cid) {
            return Some(hit);
        }
        let (src_id, _, _) = self.cid.get(&cid).copied()?;
        let out = {
            let texs = self.textures.borrow();
            let src = texs.get(src_id.0 as usize)?.as_ref()?;
            bake_blur_texture(
                &self.gpu,
                src,
                0.0,
                0.0,
                (0, 0, 0),
                0.8,
                32.0,
                1.0,
                TITLE_SHADOW_PAD,
            )
        };
        let (tid, w, h) = self.alloc_tex(out);
        let entry = (tid, w, h, TITLE_SHADOW_PAD);
        self.cid_shadows.borrow_mut().insert(cid, entry);
        Some(entry)
    }

    pub fn glow_named(&self, key: &str, preset: GlowPreset) -> Option<(TexId, u32, u32, u32)> {
        let ck = (key.to_string(), preset);
        if let Some(&hit) = self.glows.borrow().get(&ck) {
            return Some(hit);
        }
        let (src_id, _, _) = self.named.get(key).copied()?;
        let pad = preset.pad();
        let out = {
            let texs = self.textures.borrow();
            let src = texs.get(src_id.0 as usize)?.as_ref()?;
            match preset {
                GlowPreset::ButtonWhite => bake_blur_texture(
                    &self.gpu,
                    src,
                    0.0,
                    0.0,
                    (255, 255, 255),
                    0.9,
                    8.0,
                    2.0,
                    pad,
                ),
                GlowPreset::FlowTimerYellow => bake_blur_texture(
                    &self.gpu,
                    src,
                    0.0,
                    0.0,
                    (255, 255, 144),
                    0.9,
                    12.0,
                    1.2,
                    pad,
                ),
                GlowPreset::TrophyYellow => {
                    bake_blur_texture(&self.gpu, src, 0.0, 0.0, (255, 255, 0), 1.0, 6.0, 2.0, pad)
                }
                GlowPreset::TrophyShadow => {
                    bake_blur_texture(&self.gpu, src, 4.0, 45.0, (0, 0, 0), 0.8, 4.0, 1.0, pad)
                }
                GlowPreset::PipeDropShadow => {
                    bake_blur_texture(&self.gpu, src, 0.0, 0.0, (0, 0, 0), 0.5, 16.0, 3.0, pad)
                }
                GlowPreset::PipeHover => {
                    bake_blur_texture(&self.gpu, src, 0.0, 0.0, (255, 255, 0), 0.9, 8.0, 1.5, pad)
                }
                GlowPreset::PipeSelected => {
                    bake_blur_texture(&self.gpu, src, 0.0, 0.0, (255, 255, 0), 1.0, 15.0, 2.0, pad)
                }
                GlowPreset::PipeAccent => {
                    bake_blur_texture(&self.gpu, src, 0.0, 0.0, (0, 255, 64), 1.0, 4.0, 1.0, pad)
                }
                GlowPreset::PipeTutorial => bake_blur_texture(
                    &self.gpu,
                    src,
                    0.0,
                    0.0,
                    (0x2F, 0x97, 0xFF),
                    1.0,
                    15.0,
                    3.0,
                    pad,
                ),
            }
        };
        let (tid, w, h) = self.alloc_tex(out);
        let entry = (tid, w, h, pad);
        self.glows.borrow_mut().insert(ck, entry);
        Some(entry)
    }

    pub fn path_highlight(&self, entry: Side, exit: Side) -> Option<(TexId, u32, u32)> {
        let ck = (entry, exit);
        if let Some(&hit) = self.path_hl.borrow().get(&ck) {
            return Some(hit);
        }
        let out = bake_path_highlight_texture(&self.gpu, entry, exit, PATH_HL_PAD);
        let entry_val = self.alloc_tex(out);
        self.path_hl.borrow_mut().insert(ck, entry_val);
        Some(entry_val)
    }

    pub fn rainbow_tex(&self) -> TexId {
        if let Some(tid) = *self.rainbow.borrow() {
            return tid;
        }
        let tex = bake_rainbow_texture(&self.gpu);
        let (tid, _, _) = self.alloc_tex(tex);
        *self.rainbow.borrow_mut() = Some(tid);
        tid
    }

    pub fn fire_star(&self, rgb: (u8, u8, u8)) -> (TexId, f32) {
        if let Some(&hit) = self.fire_stars.borrow().get(&rgb) {
            return hit;
        }
        let tex = bake_fire_star_texture(&self.gpu, rgb);
        let (tid, w, _) = self.alloc_tex(tex);
        let entry = (tid, w as f32 / 4.0);
        self.fire_stars.borrow_mut().insert(rgb, entry);
        entry
    }
}
