//! wgpu adapter for `tpt-canvas-webgpu` draw lists.
//!
//! Each fill is a stencil pass followed by a cover pass, as in the headless design:
//!
//! - The stencil pass adds one to the stencil for front-facing fan triangles and
//!   subtracts one for back-facing ones. The resulting value is the winding number.
//!   Nonzero paths are inside where it is not zero. Even-odd paths are inside where
//!   its low bit is set.
//! - The cover pass draws the path bounds with paint, reads the clip layer, and
//!   blends premultiplied source-over. A clear pass then resets the stencil.
//! - A clip is an R8 layer. It is cleared, then written with the parent layer's value
//!   wherever the clip path winds.
//! - A blended fill (a non-normal [`BlendMode`]) copies the canvas first. Its cover pass reads
//!   that copy and writes the blended result, so the fixed-function blend is not used.
//!
//! The adapter renders into an RGBA8 target. [`GpuRenderer::render`] reads it back.
//! Colour is quantised to 8 bits, so it is only expected to match the CPU reference
//! to within one level.
#![warn(missing_docs)]

use std::sync::mpsc;

use tpt_canvas::color::srgb_to_linear;
use tpt_canvas::{
    Affine2, BlendMode, Color, ColorSpace, FillRule, ImageId, Paint, Rect, SpreadMode, Vec2,
};
use tpt_canvas_webgpu::{Command, DrawList, PaintEntry};
use wgpu::util::DeviceExt;

/// Maximum gradient stops the shader can hold.
pub const MAX_STOPS: usize = 16;
/// Byte size of the `Paint` uniform in `shader.wgsl`: 80 bytes of header and geometry, then 16 stops
/// of offset and colour, then 32 bytes for a pattern's matrix and image size, then 16 bytes for
/// the source rectangle of a drawn image.
const PAINT_BYTES: u64 = 640;

#[derive(Debug)]
/// Errors from GPU setup and rendering.
pub enum GpuError {
    /// No suitable adapter (no GPU, or a driver that wgpu cannot use).
    NoAdapter,
    /// Device creation or readback failed.
    Device(String),
    /// A gradient has more stops than the shader supports.
    TooManyStops(usize),
}

impl std::fmt::Display for GpuError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GpuError::NoAdapter => write!(f, "no GPU adapter available"),
            GpuError::Device(m) => write!(f, "device error: {m}"),
            GpuError::TooManyStops(n) => {
                write!(f, "gradient has {n} stops; the shader supports {MAX_STOPS}")
            }
        }
    }
}

impl std::error::Error for GpuError {}

/// A rendered frame: straight 8-bit RGBA is not produced; pixels are premultiplied RGBA8.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Premultiplied RGBA8 pixels, row by row.
    pub pixels: Vec<u8>,
}

struct Pipelines {
    stencil_rgba: wgpu::RenderPipeline,
    stencil_r8: wgpu::RenderPipeline,
    cover_nonzero: wgpu::RenderPipeline,
    cover_evenodd: wgpu::RenderPipeline,
    clip_cover: wgpu::RenderPipeline,
    /// Blended fills: the cover pass reads the copy of the canvas under the fill.
    blend_nonzero: wgpu::RenderPipeline,
    blend_evenodd: wgpu::RenderPipeline,
    /// Puts a finished group onto what is below it, scaled by its alpha.
    group_composite: wgpu::RenderPipeline,
    clear_rgba: wgpu::RenderPipeline,
    clear_r8: wgpu::RenderPipeline,
}

/// Runs draw lists on a GPU device.
pub struct GpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    fill_bgl: wgpu::BindGroupLayout,
    blend_bgl: wgpu::BindGroupLayout,
    composite_bgl: wgpu::BindGroupLayout,
    clip_bgl: wgpu::BindGroupLayout,
    pipelines: Pipelines,
    white: wgpu::TextureView,
}

impl GpuRenderer {
    /// Requests an adapter and device. Returns [`GpuError::NoAdapter`] when none is available.
    pub fn new() -> Result<Self, GpuError> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|_| GpuError::NoAdapter)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| GpuError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tpt-canvas shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let fill_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fill"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
            ],
        });
        let blend_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blend"),
            entries: &[
                uniform_entry(0),
                texture_entry(1),
                texture_entry(2),
                uniform_entry(3),
                texture_entry(5),
            ],
        });
        let clip_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("clip"),
            entries: &[texture_entry(1)],
        });
        let empty_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("empty"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let fill_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fill"),
            bind_group_layouts: &[Some(&fill_bgl)],
            immediate_size: 0,
        });
        let composite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("composite"),
            entries: &[texture_entry(6), uniform_entry(7)],
        });
        let composite_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite"),
            bind_group_layouts: &[Some(&composite_bgl)],
            immediate_size: 0,
        });
        let blend_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blend"),
            bind_group_layouts: &[Some(&blend_bgl)],
            immediate_size: 0,
        });
        let clip_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("clip"),
            bind_group_layouts: &[Some(&clip_bgl)],
            immediate_size: 0,
        });

        let pipelines = Pipelines {
            stencil_rgba: make_pipeline(
                &device,
                &shader,
                &empty_pl,
                "fs_empty",
                RGBA,
                Stencil::Wind,
                Blend::None,
                None,
            ),
            stencil_r8: make_pipeline(
                &device,
                &shader,
                &empty_pl,
                "fs_empty",
                R8,
                Stencil::Wind,
                Blend::None,
                None,
            ),
            cover_nonzero: make_pipeline(
                &device,
                &shader,
                &fill_pl,
                "fs_fill",
                RGBA,
                Stencil::Inside { even_odd: false },
                Blend::Premultiplied,
                None,
            ),
            cover_evenodd: make_pipeline(
                &device,
                &shader,
                &fill_pl,
                "fs_fill",
                RGBA,
                Stencil::Inside { even_odd: true },
                Blend::Premultiplied,
                None,
            ),
            blend_nonzero: make_pipeline(
                &device,
                &shader,
                &blend_pl,
                "fs_fill_blend",
                RGBA,
                Stencil::Inside { even_odd: false },
                Blend::None,
                None,
            ),
            // The stencil is zero after every fill, so the clear state tests nothing here.
            group_composite: make_pipeline(
                &device,
                &shader,
                &composite_pl,
                "fs_composite",
                RGBA,
                Stencil::Clear,
                Blend::Premultiplied,
                Some(wgpu::ColorWrites::ALL),
            ),
            blend_evenodd: make_pipeline(
                &device,
                &shader,
                &blend_pl,
                "fs_fill_blend",
                RGBA,
                Stencil::Inside { even_odd: true },
                Blend::None,
                None,
            ),
            clip_cover: make_pipeline(
                &device,
                &shader,
                &clip_pl,
                "fs_clip",
                R8,
                Stencil::Inside { even_odd: false },
                Blend::None,
                Some(wgpu::ColorWrites::ALL),
            ),
            clear_rgba: make_pipeline(
                &device,
                &shader,
                &empty_pl,
                "fs_empty",
                RGBA,
                Stencil::Clear,
                Blend::None,
                None,
            ),
            clear_r8: make_pipeline(
                &device,
                &shader,
                &empty_pl,
                "fs_empty",
                R8,
                Stencil::Clear,
                Blend::None,
                None,
            ),
        };

        let white_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("white"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &white_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255u8],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let white = white_tex.create_view(&wgpu::TextureViewDescriptor::default());

        Ok(GpuRenderer {
            device,
            queue,
            fill_bgl,
            blend_bgl,
            composite_bgl,
            clip_bgl,
            pipelines,
            white,
        })
    }

    /// Executes `list` on the GPU and returns the premultiplied RGBA8 frame.
    pub fn render(&self, list: &DrawList) -> Result<Frame, GpuError> {
        let (w, h) = (list.width, list.height);
        for paint in &list.paints {
            if let Paint::LinearGradient { stops, .. } | Paint::RadialGradient { stops, .. } =
                &paint.paint
            {
                if stops.len() > MAX_STOPS {
                    return Err(GpuError::TooManyStops(stops.len()));
                }
            }
        }

        // Vertex and index data: device triangles first, then one quad per command.
        let to_ndc = |p: Vec2| [p.x / w as f32 * 2.0 - 1.0, 1.0 - p.y / h as f32 * 2.0];
        let mut verts: Vec<[f32; 2]> = list.positions.iter().map(|&p| to_ndc(p)).collect();
        let mut indices: Vec<u32> = list.indices.clone();
        let mut quad_first: Vec<u32> = Vec::with_capacity(list.commands.len());
        for cmd in &list.commands {
            let b = match *cmd {
                Command::ClipMask { bounds, .. } | Command::Fill { bounds, .. } => bounds,
                Command::BeginGroup | Command::EndGroup { .. } => Rect {
                    x: 0.0,
                    y: 0.0,
                    w: list.width as f32,
                    h: list.height as f32,
                },
            };
            let base = verts.len() as u32;
            for p in [
                Vec2::new(b.x, b.y),
                Vec2::new(b.x + b.w, b.y),
                Vec2::new(b.x + b.w, b.y + b.h),
                Vec2::new(b.x, b.y + b.h),
            ] {
                verts.push(to_ndc(p));
            }
            quad_first.push(indices.len() as u32);
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let target = self.texture(
            "target",
            RGBA,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            w,
            h,
        );
        let target_view = target.create_view(&Default::default());
        // Each registered image becomes a float texture, so pattern samples match the reference.
        let mut images: Vec<(ImageId, u32, u32, wgpu::TextureView)> = Vec::new();
        for image in &list.images {
            let tex = self.texture(
                "image",
                wgpu::TextureFormat::Rgba32Float,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                image.width,
                image.height,
            );
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|p| p.iter().flat_map(|c| c.to_le_bytes()))
                .collect();
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(image.width * 16),
                    rows_per_image: Some(image.height),
                },
                wgpu::Extent3d {
                    width: image.width,
                    height: image.height,
                    depth_or_array_layers: 1,
                },
            );
            images.push((
                image.id,
                image.width,
                image.height,
                tex.create_view(&Default::default()),
            ));
        }
        let dummy = self
            .texture(
                "no pattern",
                wgpu::TextureFormat::Rgba32Float,
                wgpu::TextureUsages::TEXTURE_BINDING,
                1,
                1,
            )
            .create_view(&Default::default());
        // Blended fills read the canvas through this copy, because a render pass cannot
        // read the texture it writes.
        let dst_copy = self.texture(
            "dst copy",
            RGBA,
            wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            w,
            h,
        );
        let dst_view = dst_copy.create_view(&Default::default());
        let stencil = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("stencil"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Stencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let stencil_view = stencil.create_view(&Default::default());
        let layers: Vec<wgpu::Texture> = (0..list.clip_layers)
            .map(|i| {
                self.texture(
                    &format!("clip {i}"),
                    R8,
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    w,
                    h,
                )
            })
            .collect();
        let layer_views: Vec<wgpu::TextureView> = layers
            .iter()
            .map(|t| t.create_view(&Default::default()))
            .collect();

        let vb = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertices"),
                contents: &bytes_of_f32(verts.iter().flatten().copied()),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let ib = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("indices"),
                contents: &bytes_of_u32(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut enc = self.device.create_command_encoder(&Default::default());
        // Initial pass: transparent target, stencil zero.
        {
            let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(stencil_pass(&stencil_view, wgpu::LoadOp::Clear(0))),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        // Open groups, innermost last. Each one is a texture, which the commands draw into.
        let mut groups: Vec<(wgpu::Texture, wgpu::TextureView)> = Vec::new();
        for (cmd, &q) in list.commands.iter().zip(&quad_first) {
            // The texture this command draws into: the innermost open group, or the canvas.
            let (cur_tex, cur_view) = current(&groups, (&target, &target_view));
            match *cmd {
                Command::BeginGroup => {
                    let tex = self.texture(
                        "group",
                        RGBA,
                        // COPY_SRC: a blended fill inside the group copies it.
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::COPY_SRC,
                        w,
                        h,
                    );
                    let view = tex.create_view(&Default::default());
                    // A pass with no draws clears the new layer to transparent.
                    let _ = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("group clear"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    groups.push((tex, view));
                }
                Command::EndGroup { alpha } => {
                    let (_tex, group_view) = groups.pop().expect("EndGroup follows BeginGroup");
                    let (_, parent_view) = current(&groups, (&target, &target_view));
                    let alpha_buf =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("group alpha"),
                                contents: &bytes_of_f32([alpha, 0.0, 0.0, 0.0].into_iter()),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("group composite"),
                        layout: &self.composite_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 6,
                                resource: wgpu::BindingResource::TextureView(&group_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 7,
                                resource: alpha_buf.as_entire_binding(),
                            },
                        ],
                    });
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("group composite"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: parent_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(stencil_pass(
                            &stencil_view,
                            wgpu::LoadOp::Load,
                        )),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_pipeline(&self.pipelines.group_composite);
                    pass.set_bind_group(0, &bg, &[]);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                }
                Command::ClipMask {
                    layer,
                    parent,
                    tris,
                    ..
                } => {
                    let parent_view = parent.map_or(&self.white, |p| &layer_views[p as usize]);
                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("clip parent"),
                        layout: &self.clip_bgl,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(parent_view),
                        }],
                    });
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("clip mask"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &layer_views[layer as usize],
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(stencil_pass(
                            &stencil_view,
                            wgpu::LoadOp::Load,
                        )),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_pipeline(&self.pipelines.stencil_r8);
                    pass.draw_indexed(tris.first..tris.first + tris.count, 0, 0..1);
                    pass.set_pipeline(&self.pipelines.clip_cover);
                    pass.set_bind_group(0, &bg, &[]);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                    pass.set_pipeline(&self.pipelines.clear_r8);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                }
                Command::Fill {
                    tris,
                    rule,
                    paint,
                    clip,
                    blend,
                    ..
                } if blend != BlendMode::Normal => {
                    let pattern = pattern_view(&list.paints[paint as usize].paint, &images);
                    let pattern_size = pattern.map(|(_, w, h)| (w, h));
                    let pat_view = pattern.map_or(&dummy, |(v, _, _)| v);
                    let uniform = self.paint_buffer(&list.paints[paint as usize], pattern_size)?;
                    let clip_view = clip.map_or(&self.white, |c| &layer_views[c as usize]);
                    let mode = self
                        .device
                        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("blend mode"),
                            contents: &bytes_of_u32(&[blend_index(blend), 0, 0, 0]),
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                    // The stencil pass goes first, alone, so the copy sees the canvas as it was.
                    {
                        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("blend stencil"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: cur_view,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(stencil_pass(
                                &stencil_view,
                                wgpu::LoadOp::Load,
                            )),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_vertex_buffer(0, vb.slice(..));
                        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                        pass.set_pipeline(&self.pipelines.stencil_rgba);
                        pass.draw_indexed(tris.first..tris.first + tris.count, 0, 0..1);
                    }
                    enc.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: cur_tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &dst_copy,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("blend"),
                        layout: &self.blend_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: uniform.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(clip_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(pat_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 5,
                                resource: wgpu::BindingResource::TextureView(&dst_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 3,
                                resource: mode.as_entire_binding(),
                            },
                        ],
                    });
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("blend cover"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: cur_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(stencil_pass(
                            &stencil_view,
                            wgpu::LoadOp::Load,
                        )),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    match rule {
                        FillRule::NonZero => {
                            pass.set_pipeline(&self.pipelines.blend_nonzero);
                            pass.set_stencil_reference(0);
                        }
                        FillRule::EvenOdd => {
                            pass.set_pipeline(&self.pipelines.blend_evenodd);
                            pass.set_stencil_reference(1);
                        }
                    }
                    pass.set_bind_group(0, &bg, &[]);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                    pass.set_pipeline(&self.pipelines.clear_rgba);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                }
                Command::Fill {
                    tris,
                    rule,
                    paint,
                    clip,
                    ..
                } => {
                    let pattern = pattern_view(&list.paints[paint as usize].paint, &images);
                    let pattern_size = pattern.map(|(_, w, h)| (w, h));
                    let pat_view = pattern.map_or(&dummy, |(v, _, _)| v);
                    let uniform = self.paint_buffer(&list.paints[paint as usize], pattern_size)?;
                    let clip_view = clip.map_or(&self.white, |c| &layer_views[c as usize]);
                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("fill"),
                        layout: &self.fill_bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: uniform.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(clip_view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::TextureView(pat_view),
                            },
                        ],
                    });
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("fill"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: cur_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(stencil_pass(
                            &stencil_view,
                            wgpu::LoadOp::Load,
                        )),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_vertex_buffer(0, vb.slice(..));
                    pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    pass.set_pipeline(&self.pipelines.stencil_rgba);
                    pass.draw_indexed(tris.first..tris.first + tris.count, 0, 0..1);
                    match rule {
                        FillRule::NonZero => {
                            pass.set_pipeline(&self.pipelines.cover_nonzero);
                            pass.set_stencil_reference(0);
                        }
                        FillRule::EvenOdd => {
                            pass.set_pipeline(&self.pipelines.cover_evenodd);
                            pass.set_stencil_reference(1);
                        }
                    }
                    pass.set_bind_group(0, &bg, &[]);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                    pass.set_pipeline(&self.pipelines.clear_rgba);
                    pass.draw_indexed(q..q + 6, 0, 0..1);
                }
            }
        }

        // Read back with rows padded to the 256-byte copy alignment.
        let row = w * 4;
        let padded = row.div_ceil(256) * 256;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded as u64) * (h as u64),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| GpuError::Device(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Device(e.to_string()))?
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let mapped = slice
            .get_mapped_range()
            .map_err(|e| GpuError::Device(e.to_string()))?;
        let mut pixels = Vec::with_capacity((row * h) as usize);
        for y in 0..h as usize {
            let start = y * padded as usize;
            pixels.extend_from_slice(&mapped[start..start + row as usize]);
        }
        drop(mapped);
        readback.unmap();
        Ok(Frame {
            width: w,
            height: h,
            pixels,
        })
    }

    fn texture(
        &self,
        label: &str,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        w: u32,
        h: u32,
    ) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    /// Uploads the `Paint` uniform for one fill. Layout must match `shader.wgsl`.
    fn paint_buffer(
        &self,
        entry: &PaintEntry,
        pattern: Option<(u32, u32)>,
    ) -> Result<wgpu::Buffer, GpuError> {
        let mut b: Vec<u8> = Vec::with_capacity(PAINT_BYTES as usize);
        let push_f = |b: &mut Vec<u8>, v: f32| b.extend_from_slice(&v.to_le_bytes());
        let push_u = |b: &mut Vec<u8>, v: u32| b.extend_from_slice(&v.to_le_bytes());
        let m: Affine2 = entry.device_to_user;
        // A pattern maps user space into its image: the inverse of its transform, and the size.
        let (pat_inv, pat_size) = match &entry.paint {
            Paint::Pattern { transform, .. } => {
                let (w, h) = pattern
                    .ok_or_else(|| GpuError::Device("pattern image is not registered".into()))?;
                (
                    transform.invert().unwrap_or(Affine2::IDENTITY),
                    [w as f32, h as f32],
                )
            }
            _ => (Affine2::IDENTITY, [0.0, 0.0]),
        };
        let (kind, space, spread, count, color, geom, stops) = match &entry.paint {
            Paint::Solid(c) => (0u32, 0u32, 0u32, 0u32, *c, [0.0; 4], &[][..]),
            Paint::LinearGradient {
                start,
                end,
                stops,
                spread,
                space,
            } => (
                1,
                space_tag(*space),
                spread_tag(*spread),
                stops.len() as u32,
                Color::TRANSPARENT,
                [start.x, start.y, end.x, end.y],
                &stops[..],
            ),
            Paint::RadialGradient {
                center,
                radius,
                stops,
                spread,
                space,
            } => (
                2,
                space_tag(*space),
                spread_tag(*spread),
                stops.len() as u32,
                Color::TRANSPARENT,
                [center.x, center.y, *radius, 0.0],
                &stops[..],
            ),
            Paint::Pattern { .. } => (
                3u32,
                0u32,
                0u32,
                0u32,
                Color::TRANSPARENT,
                [0.0; 4],
                &[][..],
            ),
        };
        push_u(&mut b, kind);
        push_u(&mut b, space);
        push_u(&mut b, spread);
        push_u(&mut b, count);
        for v in [color.r, color.g, color.b, color.a] {
            push_f(&mut b, v);
        }
        for v in geom {
            push_f(&mut b, v);
        }
        for v in [m.a, m.b, m.c, m.d] {
            push_f(&mut b, v);
        }
        for v in [m.e, m.f, 0.0, 0.0] {
            push_f(&mut b, v);
        }
        // Stop offsets, then colours (decoded to linear when interpolating in linear space).
        let linear = space_tag(match &entry.paint {
            Paint::LinearGradient { space, .. } | Paint::RadialGradient { space, .. } => *space,
            _ => ColorSpace::Srgb,
        }) == 1;
        let mut offsets = [0.0f32; MAX_STOPS * 4];
        let mut colors = [0.0f32; MAX_STOPS * 4];
        for (i, s) in stops.iter().enumerate() {
            offsets[i * 4] = s.offset;
            let c = if linear {
                Color::rgba(
                    srgb_to_linear(s.color.r),
                    srgb_to_linear(s.color.g),
                    srgb_to_linear(s.color.b),
                    s.color.a,
                )
            } else {
                s.color
            };
            colors[i * 4..i * 4 + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
        }
        for v in offsets.iter().chain(colors.iter()) {
            push_f(&mut b, *v);
        }
        for v in [pat_inv.a, pat_inv.b, pat_inv.c, pat_inv.d] {
            push_f(&mut b, v);
        }
        for v in [pat_inv.e, pat_inv.f, pat_size[0], pat_size[1]] {
            push_f(&mut b, v);
        }
        // A drawn image's source rectangle, in image pixels. Zero width means no crop.
        let crop = entry.clip_image.map_or([0.0; 4], |r| [r.x, r.y, r.w, r.h]);
        for v in crop {
            push_f(&mut b, v);
        }
        debug_assert_eq!(b.len() as u64, PAINT_BYTES);
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("paint"),
            size: PAINT_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&buf, 0, &b);
        Ok(buf)
    }
}

const RGBA: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const R8: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

#[derive(Clone, Copy)]
enum Stencil {
    /// Front faces increment, back faces decrement.
    Wind,
    /// Test the winding result. `even_odd` tests the low bit, otherwise non-zero.
    Inside { even_odd: bool },
    /// Reset every covered pixel to zero.
    Clear,
}

#[derive(Clone, Copy)]
enum Blend {
    None,
    Premultiplied,
}

/// The view and size of the image a pattern paint samples, when `paint` is a pattern.
fn pattern_view<'a>(
    paint: &Paint,
    images: &'a [(ImageId, u32, u32, wgpu::TextureView)],
) -> Option<(&'a wgpu::TextureView, u32, u32)> {
    let Paint::Pattern { image, .. } = paint else {
        return None;
    };
    let (_, w, h, view) = images.iter().find(|(k, ..)| k == image)?;
    Some((view, *w, *h))
}

/// The texture the current command draws into: the innermost open group, or the canvas.
fn current<'a>(
    groups: &'a [(wgpu::Texture, wgpu::TextureView)],
    base: (&'a wgpu::Texture, &'a wgpu::TextureView),
) -> (&'a wgpu::Texture, &'a wgpu::TextureView) {
    match groups.last() {
        Some((tex, view)) => (tex, view),
        None => base,
    }
}

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// The index of a blend mode in `shader.wgsl`, which follows the order of `BlendMode`.
fn blend_index(mode: BlendMode) -> u32 {
    match mode {
        BlendMode::Normal => 0,
        BlendMode::Multiply => 1,
        BlendMode::Screen => 2,
        BlendMode::Overlay => 3,
        BlendMode::Darken => 4,
        BlendMode::Lighten => 5,
        BlendMode::Difference => 6,
        BlendMode::Exclusion => 7,
        BlendMode::Hue => 8,
        BlendMode::Saturation => 9,
        BlendMode::Color => 10,
        BlendMode::Luminosity => 11,
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn stencil_pass(
    view: &wgpu::TextureView,
    load: wgpu::LoadOp<u32>,
) -> wgpu::RenderPassDepthStencilAttachment<'_> {
    wgpu::RenderPassDepthStencilAttachment {
        view,
        depth_ops: None,
        stencil_ops: Some(wgpu::Operations {
            load,
            store: wgpu::StoreOp::Store,
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn make_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: &str,
    format: wgpu::TextureFormat,
    stencil: Stencil,
    blend: Blend,
    write_override: Option<wgpu::ColorWrites>,
) -> wgpu::RenderPipeline {
    let write_mask = match (write_override, stencil) {
        (Some(w), _) => w,
        // Stencil and clear passes draw no colour.
        (None, Stencil::Wind | Stencil::Clear) => wgpu::ColorWrites::empty(),
        (None, Stencil::Inside { .. }) => wgpu::ColorWrites::ALL,
    };
    let blend = match blend {
        Blend::None => None,
        Blend::Premultiplied => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }),
    };
    let keep = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };
    let (front, back, read_mask) = match stencil {
        Stencil::Wind => (
            wgpu::StencilFaceState {
                pass_op: wgpu::StencilOperation::IncrementWrap,
                ..keep
            },
            wgpu::StencilFaceState {
                pass_op: wgpu::StencilOperation::DecrementWrap,
                ..keep
            },
            0xff,
        ),
        Stencil::Inside { even_odd } => {
            let face = wgpu::StencilFaceState {
                compare: if even_odd {
                    wgpu::CompareFunction::Equal
                } else {
                    wgpu::CompareFunction::NotEqual
                },
                ..keep
            };
            (face, face, if even_odd { 0x01 } else { 0xff })
        }
        Stencil::Clear => {
            let face = wgpu::StencilFaceState {
                pass_op: wgpu::StencilOperation::Zero,
                ..keep
            };
            (face, face, 0xff)
        }
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(fragment),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 8,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Stencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front,
                back,
                read_mask,
                write_mask: 0xff,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fragment),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn space_tag(s: ColorSpace) -> u32 {
    match s {
        ColorSpace::Srgb => 0,
        ColorSpace::LinearSrgb => 1,
    }
}

fn spread_tag(s: SpreadMode) -> u32 {
    match s {
        SpreadMode::Pad => 0,
        SpreadMode::Repeat => 1,
        SpreadMode::Reflect => 2,
    }
}

fn bytes_of_f32(values: impl Iterator<Item = f32>) -> Vec<u8> {
    values.flat_map(|v| v.to_le_bytes()).collect()
}

fn bytes_of_u32(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
