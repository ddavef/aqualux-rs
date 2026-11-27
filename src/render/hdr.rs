// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use wgpu::{
    Adapter, DisplayHdrInfo, Surface, SurfaceCapabilities, SurfaceColorSpace, SurfaceColorSpaces,
    TextureFormat,
};

pub const HDR_PAPER_WHITE_MIN: f32 = 80.0;
pub const HDR_PAPER_WHITE_MAX: f32 = 500.0;
pub const HDR_PAPER_WHITE_DEFAULT: f32 = 203.0;
pub const HDR_PEAK_MIN: f32 = 300.0;
pub const HDR_PEAK_MAX: f32 = 1600.0;
pub const HDR_PEAK_DEFAULT: f32 = 800.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdrOutputMode {
    ScRgb,
    Hdr10Pq,
}

impl HdrOutputMode {
    pub const fn surface_format(self) -> TextureFormat {
        match self {
            Self::ScRgb => TextureFormat::Rgba16Float,
            Self::Hdr10Pq => TextureFormat::Rgb10a2Unorm,
        }
    }

    pub const fn color_space(self) -> SurfaceColorSpace {
        match self {
            Self::ScRgb => SurfaceColorSpace::ExtendedSrgbLinear,
            Self::Hdr10Pq => SurfaceColorSpace::Bt2100Pq,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrPresentParams {
    pub mode: HdrOutputMode,
    pub scrgb_white_scale: f32,
    pub sdr_white_nits: f32,
    pub peak_headroom: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HdrDisplayCaps {
    pub surface_mode: Option<HdrOutputMode>,
    pub available: bool,
    pub needs_manual_calibration: bool,
    pub sdr_white_nits: f32,
    pub peak_headroom: f32,
    pub scrgb_white_scale: f32,
}

impl HdrDisplayCaps {
    pub const fn unsupported() -> Self {
        Self {
            surface_mode: None,
            available: false,
            needs_manual_calibration: false,
            sdr_white_nits: HDR_PAPER_WHITE_DEFAULT,
            peak_headroom: 1.0,
            scrgb_white_scale: 1.0,
        }
    }

    pub fn present_params(self) -> Option<HdrPresentParams> {
        let mode = self.available.then_some(self.surface_mode).flatten()?;
        Some(HdrPresentParams {
            mode,
            scrgb_white_scale: self.scrgb_white_scale,
            sdr_white_nits: self.sdr_white_nits,
            peak_headroom: self.peak_headroom,
        })
    }

    pub fn present_params_with_calibration(
        self,
        paper_white_nits: f32,
        peak_nits: f32,
    ) -> Option<HdrPresentParams> {
        if !self.available {
            return None;
        }
        if !self.needs_manual_calibration {
            return self.present_params();
        }
        let mode = self.surface_mode?;
        let pw = paper_white_nits.clamp(HDR_PAPER_WHITE_MIN, HDR_PAPER_WHITE_MAX);
        let pk = peak_nits.clamp(HDR_PEAK_MIN, HDR_PEAK_MAX);
        Some(HdrPresentParams {
            mode,
            scrgb_white_scale: pw / HDR_PAPER_WHITE_DEFAULT,
            sdr_white_nits: pw,
            peak_headroom: (pk / pw).clamp(1.2, 12.0),
        })
    }
}

pub fn pick_hdr_surface_mode(caps: &SurfaceCapabilities) -> Option<HdrOutputMode> {
    if caps
        .color_spaces(TextureFormat::Rgba16Float)
        .contains(SurfaceColorSpaces::EXTENDED_SRGB_LINEAR)
    {
        Some(HdrOutputMode::ScRgb)
    } else if caps
        .color_spaces(TextureFormat::Rgb10a2Unorm)
        .contains(SurfaceColorSpaces::BT2100_PQ)
    {
        Some(HdrOutputMode::Hdr10Pq)
    } else {
        None
    }
}

pub fn evaluate_hdr_caps(
    surface_mode: Option<HdrOutputMode>,
    info: DisplayHdrInfo,
) -> HdrDisplayCaps {
    let Some(mode) = surface_mode else {
        return HdrDisplayCaps::unsupported();
    };

    if let Some(hr) = info.headroom {
        let current = hr
            .current
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(1.0);
        let potential = hr.potential.filter(|v| v.is_finite() && *v > 0.0);
        let available = potential.is_some_and(|p| p > 1.01) || current > 1.01;
        let peak_headroom = if available {
            current.clamp(1.0, 12.0)
        } else {
            1.0
        };
        return HdrDisplayCaps {
            surface_mode: Some(mode),
            available,
            needs_manual_calibration: false,
            sdr_white_nits: HDR_PAPER_WHITE_DEFAULT,
            peak_headroom,
            scrgb_white_scale: 1.0,
        };
    }

    if let Some(lum) = info.luminance {
        let available = info.coarse.and_then(|c| c.high_dynamic_range) != Some(false);
        let sdr_white_nits = lum
            .sdr_white_nits
            .filter(|v| v.is_finite() && *v >= 40.0)
            .unwrap_or(200.0)
            .clamp(40.0, 600.0);
        let peak_headroom = if available {
            info.tone_map_headroom()
                .or_else(|| {
                    lum.max_nits
                        .filter(|m| m.is_finite() && *m > 0.0)
                        .map(|m| m / sdr_white_nits)
                })
                .unwrap_or(4.0)
                .clamp(1.0, 12.0)
        } else {
            1.0
        };
        return HdrDisplayCaps {
            surface_mode: Some(mode),
            available,
            needs_manual_calibration: false,
            sdr_white_nits,
            peak_headroom,
            scrgb_white_scale: sdr_white_nits / 80.0,
        };
    }

    if info.coarse.and_then(|c| c.high_dynamic_range) == Some(false) {
        return HdrDisplayCaps {
            surface_mode: Some(mode),
            available: false,
            needs_manual_calibration: false,
            sdr_white_nits: HDR_PAPER_WHITE_DEFAULT,
            peak_headroom: 1.0,
            scrgb_white_scale: 1.0,
        };
    }

    HdrDisplayCaps {
        surface_mode: Some(mode),
        available: true,
        needs_manual_calibration: true,
        sdr_white_nits: HDR_PAPER_WHITE_DEFAULT,
        peak_headroom: HDR_PEAK_DEFAULT / HDR_PAPER_WHITE_DEFAULT,
        scrgb_white_scale: 1.0,
    }
}

pub fn query_surface_hdr_caps(adapter: &Adapter, surface: &Surface<'_>) -> HdrDisplayCaps {
    let caps = surface.get_capabilities(adapter);
    let info = surface.display_hdr_info(adapter);
    evaluate_hdr_caps(pick_hdr_surface_mode(&caps), info)
}
