// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 ddavef aqualux-rs

use crate::render::gpu_types::{GlyphVertex, PipeMaskInstance, ShapeInstance, SpriteInstance};
use wgpu::{
    BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState,
    BufferBindingType, ColorTargetState, ColorWrites, CompareFunction, DepthBiasState,
    DepthStencilState, Device, FragmentState, MultisampleState, PipelineCompilationOptions,
    PipelineLayout, PipelineLayoutDescriptor, PrimitiveState, RenderPipeline,
    RenderPipelineDescriptor, SamplerBindingType, ShaderModule, ShaderModuleDescriptor,
    ShaderSource, ShaderStages, StencilFaceState, StencilOperation, StencilState, TextureFormat,
    TextureSampleType, TextureViewDimension, VertexAttribute, VertexBufferLayout, VertexFormat,
    VertexState, VertexStepMode,
};

pub struct GpuPipelines {
    pub stage_cmat_bgl: BindGroupLayout,
    pub stage_only_bgl: BindGroupLayout,
    pub tex_bgl: BindGroupLayout,
    pub leak_bgl: BindGroupLayout,
    pub blur_bgl: BindGroupLayout,
    pub glyph_bgl: BindGroupLayout,
    pub present_bgl: BindGroupLayout,
    pub sprite_target: RenderPipeline,
    pub sprite_unorm: RenderPipeline,
    pub sprite_hdr: RenderPipeline,
    pub pipe_mask: RenderPipeline,
    pub pipe_mask_hdr: RenderPipeline,
    pub shape_target: RenderPipeline,
    pub shape_unorm: RenderPipeline,
    pub shape_msaa: RenderPipeline,
    pub shape_hdr: RenderPipeline,
    pub leak: RenderPipeline,
    pub leak_hdr: RenderPipeline,
    pub blur_unorm: RenderPipeline,
    pub glyph_stencil: RenderPipeline,
    pub glyph_cover: RenderPipeline,
    pub present_scrgb: RenderPipeline,
    pub present_pq: RenderPipeline,
}

fn uniform_entry(binding: u32, visibility: ShaderStages) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn tex_entry(binding: u32, filterable: bool) -> BindGroupLayoutEntry {
    BindGroupLayoutEntry {
        binding,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Texture {
            sample_type: TextureSampleType::Float { filterable },
            view_dimension: TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

const SPRITE_ATTRS: [VertexAttribute; 5] = [
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 64,
        shader_location: 4,
    },
];

const PIPE_MASK_ATTRS: [VertexAttribute; 8] = [
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 64,
        shader_location: 4,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 80,
        shader_location: 5,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 96,
        shader_location: 6,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 112,
        shader_location: 7,
    },
];

const SHAPE_ATTRS: [VertexAttribute; 6] = [
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 0,
        shader_location: 0,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 16,
        shader_location: 1,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 32,
        shader_location: 2,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 48,
        shader_location: 3,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 64,
        shader_location: 4,
    },
    VertexAttribute {
        format: VertexFormat::Float32x4,
        offset: 80,
        shader_location: 5,
    },
];

const GLYPH_ATTRS: [VertexAttribute; 1] = [VertexAttribute {
    format: VertexFormat::Float32x2,
    offset: 0,
    shader_location: 0,
}];

fn build_pipe(
    device: &Device,
    label: &str,
    bgls: &[&BindGroupLayout],
    shader: &ShaderModule,
    buffers: &[Option<VertexBufferLayout<'_>>],
    target: ColorTargetState,
    msaa_ds: (u32, Option<DepthStencilState>),
) -> RenderPipeline {
    let opt_bgls: Vec<Option<&BindGroupLayout>> = bgls.iter().copied().map(Some).collect();
    let layout: PipelineLayout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &opt_bgls,
        immediate_size: 0,
    });
    let targets = [Some(target)];
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            buffers,
        },
        primitive: PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: msaa_ds.1,
        multisample: MultisampleState {
            count: msaa_ds.0,
            ..Default::default()
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            compilation_options: PipelineCompilationOptions::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn premul(format: TextureFormat) -> ColorTargetState {
    ColorTargetState {
        format,
        blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
        write_mask: ColorWrites::ALL,
    }
}

impl GpuPipelines {
    pub fn new(device: &Device, target_format: TextureFormat) -> Self {
        let stage_cmat_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("stage_cmat_bgl"),
            entries: &[
                uniform_entry(0, ShaderStages::VERTEX | ShaderStages::FRAGMENT),
                uniform_entry(1, ShaderStages::FRAGMENT),
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let stage_only_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("stage_only_bgl"),
            entries: &[uniform_entry(
                0,
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            )],
        });
        let tex_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("tex_bgl"),
            entries: &[tex_entry(0, true)],
        });
        let leak_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("leak_bgl"),
            entries: &[uniform_entry(
                0,
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            )],
        });
        let blur_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("blur_bgl"),
            entries: &[
                uniform_entry(0, ShaderStages::FRAGMENT),
                tex_entry(1, false),
            ],
        });
        let glyph_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("glyph_bgl"),
            entries: &[uniform_entry(
                0,
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
            )],
        });
        let present_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("present_bgl"),
            entries: &[
                uniform_entry(0, ShaderStages::FRAGMENT),
                tex_entry(1, false),
            ],
        });

        let load = |label, wgsl| {
            device.create_shader_module(ShaderModuleDescriptor {
                label: Some(label),
                source: ShaderSource::Wgsl(wgsl),
            })
        };
        let sh_sprite = load("sprite.wgsl", include_str!("shaders/sprite.wgsl").into());
        let sh_pm = load(
            "pipe_mask.wgsl",
            include_str!("shaders/pipe_mask.wgsl").into(),
        );
        let sh_shape = load("shape.wgsl", include_str!("shaders/shape.wgsl").into());
        let sh_leak = load("leak.wgsl", include_str!("shaders/leak.wgsl").into());
        let sh_blur = load("blur.wgsl", include_str!("shaders/blur.wgsl").into());
        let sh_glyph = load("glyph.wgsl", include_str!("shaders/glyph.wgsl").into());
        let sh_present = load("present.wgsl", include_str!("shaders/present.wgsl").into());

        let sp_buf = [Some(VertexBufferLayout {
            array_stride: size_of::<SpriteInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &SPRITE_ATTRS,
        })];
        let sh_buf = [Some(VertexBufferLayout {
            array_stride: size_of::<ShapeInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &SHAPE_ATTRS,
        })];
        let pm_buf = [Some(VertexBufferLayout {
            array_stride: size_of::<PipeMaskInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: &PIPE_MASK_ATTRS,
        })];
        let gl_buf = [Some(VertexBufferLayout {
            array_stride: size_of::<GlyphVertex>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: &GLYPH_ATTRS,
        })];

        let sprite_target = build_pipe(
            device,
            "sprite_t",
            &[&stage_cmat_bgl, &tex_bgl],
            &sh_sprite,
            &sp_buf,
            premul(target_format),
            (1, None),
        );
        let sprite_unorm = build_pipe(
            device,
            "sprite_u",
            &[&stage_cmat_bgl, &tex_bgl],
            &sh_sprite,
            &sp_buf,
            premul(TextureFormat::Rgba8Unorm),
            (1, None),
        );
        let sprite_hdr = build_pipe(
            device,
            "sprite_hdr",
            &[&stage_cmat_bgl, &tex_bgl],
            &sh_sprite,
            &sp_buf,
            premul(TextureFormat::Rgba16Float),
            (1, None),
        );
        let pipe_mask = build_pipe(
            device,
            "pipe_mask",
            &[&stage_cmat_bgl, &tex_bgl],
            &sh_pm,
            &pm_buf,
            premul(target_format),
            (1, None),
        );
        let pipe_mask_hdr = build_pipe(
            device,
            "pipe_mask_hdr",
            &[&stage_cmat_bgl, &tex_bgl],
            &sh_pm,
            &pm_buf,
            premul(TextureFormat::Rgba16Float),
            (1, None),
        );
        let shape_target = build_pipe(
            device,
            "shape_t",
            &[&stage_only_bgl],
            &sh_shape,
            &sh_buf,
            premul(target_format),
            (1, None),
        );
        let shape_unorm = build_pipe(
            device,
            "shape_u",
            &[&stage_only_bgl],
            &sh_shape,
            &sh_buf,
            premul(TextureFormat::Rgba8Unorm),
            (1, None),
        );
        let shape_hdr = build_pipe(
            device,
            "shape_hdr",
            &[&stage_only_bgl],
            &sh_shape,
            &sh_buf,
            premul(TextureFormat::Rgba16Float),
            (1, None),
        );
        let msaa_ds = DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::Always),
            stencil: StencilState::default(),
            bias: DepthBiasState::default(),
        };
        let shape_msaa = build_pipe(
            device,
            "shape_msaa",
            &[&stage_only_bgl],
            &sh_shape,
            &sh_buf,
            premul(TextureFormat::Rgba8Unorm),
            (4, Some(msaa_ds)),
        );
        let leak = build_pipe(
            device,
            "leak",
            &[&stage_only_bgl, &leak_bgl],
            &sh_leak,
            &[],
            premul(target_format),
            (1, None),
        );
        let leak_hdr = build_pipe(
            device,
            "leak_hdr",
            &[&stage_only_bgl, &leak_bgl],
            &sh_leak,
            &[],
            premul(TextureFormat::Rgba16Float),
            (1, None),
        );
        let present_scrgb = build_pipe(
            device,
            "present_scrgb",
            &[&present_bgl],
            &sh_present,
            &[],
            ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
            (1, None),
        );
        let present_pq = build_pipe(
            device,
            "present_pq",
            &[&present_bgl],
            &sh_present,
            &[],
            ColorTargetState {
                format: TextureFormat::Rgb10a2Unorm,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
            (1, None),
        );
        let blur_unorm = build_pipe(
            device,
            "blur",
            &[&blur_bgl],
            &sh_blur,
            &[],
            ColorTargetState {
                format: TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
            (1, None),
        );

        let inv_face = StencilFaceState {
            compare: CompareFunction::Always,
            fail_op: StencilOperation::Invert,
            depth_fail_op: StencilOperation::Invert,
            pass_op: StencilOperation::Invert,
        };
        let cov_face = StencilFaceState {
            compare: CompareFunction::NotEqual,
            fail_op: StencilOperation::Zero,
            depth_fail_op: StencilOperation::Zero,
            pass_op: StencilOperation::Zero,
        };
        let ds_for = |face, rm, wm| DepthStencilState {
            format: TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(CompareFunction::Always),
            stencil: StencilState {
                front: face,
                back: face,
                read_mask: rm,
                write_mask: wm,
            },
            bias: DepthBiasState::default(),
        };
        let glyph_stencil = build_pipe(
            device,
            "gl_st",
            &[&glyph_bgl],
            &sh_glyph,
            &gl_buf,
            ColorTargetState {
                format: TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: ColorWrites::empty(),
            },
            (4, Some(ds_for(inv_face, 0xFF, 0x01))),
        );
        let glyph_cover = build_pipe(
            device,
            "gl_cov",
            &[&glyph_bgl],
            &sh_glyph,
            &gl_buf,
            premul(TextureFormat::Rgba8Unorm),
            (4, Some(ds_for(cov_face, 0x01, 0x01))),
        );

        Self {
            stage_cmat_bgl,
            stage_only_bgl,
            tex_bgl,
            leak_bgl,
            blur_bgl,
            glyph_bgl,
            present_bgl,
            sprite_target,
            sprite_unorm,
            sprite_hdr,
            pipe_mask,
            pipe_mask_hdr,
            shape_target,
            shape_unorm,
            shape_msaa,
            shape_hdr,
            leak,
            leak_hdr,
            blur_unorm,
            glyph_stencil,
            glyph_cover,
            present_scrgb,
            present_pq,
        }
    }
}
