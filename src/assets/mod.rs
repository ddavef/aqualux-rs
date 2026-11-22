// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

pub mod dir_loader;
pub mod neko;
pub mod swf;
pub mod swf_decode;
pub mod swf_font;
pub mod swf_geom;
pub mod swf_sprite;

pub use dir_loader::extract_assets_to_dir;
pub use neko::extract_swf_from_neko;
pub use swf_font::{GlyphOp, SwfFont};
pub use swf_geom::Matrix2D;
pub use swf_sprite::SpriteItem;

use crate::game::LevelDef;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    /// Straight (non-premultiplied) RGBA bytes, 4 bytes per pixel.
    pub data: Vec<u8>,
}

pub struct AssetBundle {
    pub source_label: String,
    pub images: HashMap<String, RgbaImage>,
    pub cid_images: HashMap<u16, RgbaImage>,
    pub sprites: HashMap<String, Vec<SpriteItem>>,
    pub fonts: HashMap<String, SwfFont>,
    pub sounds: HashMap<String, Vec<u8>>,
    pub raw_xmls: HashMap<String, String>,
    pub campaign_levels: Vec<LevelDef>,
    pub lotd_levels: Vec<LevelDef>,
}

const REQUIRED_BITMAPS: &[&str] = &[
    "MmgSplashBitmap",
    "CombinedBgBitmap",
    "DaySkyBitmap",
    "SnowySkyBitmap",
    "SpaceSkyBitmap",
    "GreenTitleBitmap",
    "ArrowCursorBitmap",
    "MoveCursorBitmap",
    "RotateCursorBitmap",
    "StopCursorBitmap",
    "WaitCursorBitmap",
    "ElbowClearBitmap",
    "ElbowFullBitmap",
    "ElbowFrostBitmap",
    "CrossClearBitmap",
    "CrossFullBitmap",
    "CrossFrostBitmap",
    "StraightClearBitmap",
    "StraightFullBitmap",
    "StraightFrostBitmap",
    "SpigotClearBitmap",
    "SpigotFullBitmap",
    "DrainPipeClearBitmap",
    "DrainPipeFullBitmap",
    "DrainBaseBitmap",
    "DrainRingBitmap",
    "DrainingSlimeSheet",
    "DrainingPortalSlimeSheet",
    "PipeSpigotBitmap",
    "FlowTimerBitmap",
    "StartSignBitmap",
    "GoalSignBitmap",
    "YellowPortalBitmap",
    "CementBlockerBitmap",
    "FastForwardButtonBitmap",
    "PauseButtonBitmap",
    "MuteButtonBitmap",
    "UnmuteButtonBitmap",
    "SkipTutorialButtonBitmap",
    "ModalButtonBitmap",
    "NoTrophyBitmap",
    "BronzeTrophyBitmap",
    "SilverTrophyBitmap",
    "GoldTrophyBitmap",
    "PlatinumTrophyBitmap",
    "MysteryTrophyBitmap",
    "LightTrophyBitmap",
    "ChampTrophyBitmap",
    "UltraluxTrophyBitmap",
    "MirrorTrophyBitmap",
    "LockIcon",
];

const REQUIRED_SPRITES: &[&str] = &[
    "TitleBgAssetsSprite",
    "SnowHillsBgAssetsSprite",
    "SpriteTitle",
];

const REQUIRED_FONTS: &[&str] = &["neuropol", "pricedown", "helvetica"];

const REQUIRED_SOUNDS: &[&str] = &[
    "LittleRayMusic",
    "InGameLittleRayMusic",
    "GlideMusic",
    "GlideSuccess",
    "IstanbulMusic",
    "DriftingMusic",
    "ClickOneSound",
    "ClickTwoSound",
    "TwistPieceSound",
    "TwistSound",
    "SwapOneSound",
    "SwapTwoSound",
    "SwishSound",
    "BigWhooshSound",
    "PopTwoSound",
    "BlubOneSound",
    "BlubTwoSound",
    "BlubThreeSound",
    "BlubFourSound",
    "BonusOneSound",
    "BonusTwoSound",
    "CheeringSound",
    "AlarmBeepSound",
    "PourSound",
    "FireworkOneSound",
    "FireworkTwoSound",
];

impl AssetBundle {
    pub fn clone_for_verify(&self) -> Self {
        Self {
            source_label: self.source_label.clone(),
            images: self.images.clone(),
            cid_images: self.cid_images.clone(),
            sprites: self.sprites.clone(),
            fonts: self.fonts.clone(),
            sounds: self.sounds.clone(),
            raw_xmls: self.raw_xmls.clone(),
            campaign_levels: self.campaign_levels.clone(),
            lotd_levels: self.lotd_levels.clone(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for name in REQUIRED_BITMAPS {
            if !self.images.contains_key(*name) {
                return Err(format!(
                    "Missing required bitmap '{}' in '{}'",
                    name, self.source_label
                ));
            }
        }
        for sname in REQUIRED_SPRITES {
            let items = self.sprites.get(*sname).ok_or_else(|| {
                format!(
                    "Missing required sprite '{}' in '{}'",
                    sname, self.source_label
                )
            })?;
            if items.is_empty() {
                return Err(format!(
                    "Sprite '{}' is empty in '{}'",
                    sname, self.source_label
                ));
            }
            for item in items {
                if !self.cid_images.contains_key(&item.bitmap_cid) {
                    return Err(format!(
                        "Sprite '{}' references missing bitmap cid {} in '{}'",
                        sname, item.bitmap_cid, self.source_label
                    ));
                }
            }
        }
        for fname in REQUIRED_FONTS {
            if !self.fonts.keys().any(|k| k.contains(fname)) {
                return Err(format!(
                    "Missing required SWF font '{}' in '{}'",
                    fname, self.source_label
                ));
            }
        }
        for snd in REQUIRED_SOUNDS {
            if !self.sounds.contains_key(*snd) {
                return Err(format!(
                    "Missing required sound '{}' in '{}'",
                    snd, self.source_label
                ));
            }
        }
        if self.campaign_levels.is_empty() {
            return Err(format!(
                "Missing campaign levels XML in '{}'",
                self.source_label
            ));
        }
        if self.lotd_levels.is_empty() {
            return Err(format!(
                "Missing fallback LOTD levels XML in '{}'",
                self.source_label
            ));
        }
        Ok(())
    }

    pub fn discover(cli_path: Option<&Path>) -> Result<Self, String> {
        if let Some(p) = cli_path {
            return Self::load_from_path(p);
        }
        if let Ok(env_val) = std::env::var("AQUALUX_ASSETS") {
            let p = PathBuf::from(&env_val);
            if !p.exists() {
                return Err(format!(
                    "AQUALUX_ASSETS points to missing path '{}'",
                    env_val
                ));
            }
            return Self::load_from_path(&p);
        }

        let mut candidates: Vec<PathBuf> = vec![
            PathBuf::from("resource.n"),
            PathBuf::from("aqualux.swf"),
            PathBuf::from("../resource.n"),
            PathBuf::from("../aqualux.swf"),
        ];
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            candidates.push(dir.join("resource.n"));
            candidates.push(dir.join("aqualux.swf"));
        }

        let mut last_err: Option<String> = None;
        for cand in candidates {
            if cand.exists() {
                match Self::load_from_path(&cand) {
                    Ok(bundle) => return Ok(bundle),
                    Err(e) => last_err = Some(e),
                }
            }
        }

        if let Some(err) = last_err {
            return Err(err);
        }
        Err(
            "Required Aqualux resources ('resource.n' or 'aqualux.swf') were not found. Place 'resource.n' or 'aqualux.swf' beside the executable or current directory, set AQUALUX_ASSETS, or pass --assets <PATH>."
                .to_string(),
        )
    }

    pub fn load_from_path(path: &Path) -> Result<Self, String> {
        if path.is_dir() {
            return Err(format!(
                "Unsupported asset path '{}': expected original 'resource.n' or 'aqualux.swf' file (directories are no longer supported)",
                path.display()
            ));
        }
        let bytes =
            fs::read(path).map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
        let label = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("custom")
            .to_string();

        if bytes.starts_with(b"NEKO") {
            let swf_bytes = extract_swf_from_neko(&bytes)?;
            return swf::load_bundle_from_swf(&swf_bytes, &label);
        }
        if bytes.starts_with(b"CWS") || bytes.starts_with(b"FWS") {
            return swf::load_bundle_from_swf(&bytes, &label);
        }
        Err(format!(
            "Unsupported asset file '{}': expected Neko resource.n or SWF archive",
            path.display()
        ))
    }
}

pub(crate) fn short_asset_name(full: &str) -> String {
    if let Some(rest) = full.strip_prefix("aqualux.display.AssetLibrary_") {
        rest.to_string()
    } else if let Some((_, tail)) = full.rsplit_once('.') {
        tail.to_string()
    } else {
        full.to_string()
    }
}
