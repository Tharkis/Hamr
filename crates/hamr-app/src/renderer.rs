//! Draws the 3D viewport with wgpu.
//!
//! The scene is rendered into an offscreen multisampled texture with its own depth buffer, then
//! copied into the egui panel. That keeps 3D depth testing independent of egui's render pass.

use std::sync::Arc;

use eframe::egui_wgpu::{self, wgpu, CallbackResources, CallbackTrait, ScreenDescriptor};
use eframe::wgpu::util::DeviceExt;
use glam::{Mat4, Vec3};
use hamr_core::grid::LineVertex;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
/// Viewport background, in linear colour.
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.035,
    g: 0.04,
    b: 0.05,
    a: 1.0,
};

/// A vertex for solid geometry, with the colour baked in.
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// Everything the GPU needs to draw one frame's geometry. A new `revision` triggers a re-upload.
#[derive(Default)]
pub struct SceneGeometry {
    pub revision: u64,
    /// Depth-tested lines, such as the grid.
    pub lines: Vec<LineVertex>,
    pub mesh_vertices: Vec<MeshVertex>,
    /// Opaque triangles first, then translucent ones.
    pub mesh_indices: Vec<u32>,
    pub opaque_index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 4],
    grid_fade: [f32; 4],
}

/// The paint callback handed to egui each frame.
pub struct ViewportCallback {
    pub view_proj: Mat4,
    /// Grid lines fade out with distance from this point, reaching zero at `grid_fade_radius`.
    pub grid_fade_center: Vec3,
    pub grid_fade_radius: f32,
    /// Lines drawn on top of everything, such as the selection outline and the handles. They
    /// change every frame with the camera, so they are uploaded every frame.
    pub overlay: Vec<LineVertex>,
    pub size_px: [u32; 2],
    pub geometry: Arc<SceneGeometry>,
}

/// GPU objects that live for the whole session, stored in egui's callback resources.
pub struct ViewportRenderer {
    globals: wgpu::Buffer,
    globals_bind: wgpu::BindGroup,
    line_pipeline: wgpu::RenderPipeline,
    overlay_pipeline: wgpu::RenderPipeline,
    mesh_pipeline: wgpu::RenderPipeline,
    translucent_pipeline: wgpu::RenderPipeline,
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    targets: Option<Targets>,
    buffers: Option<Buffers>,
}

struct Targets {
    size: [u32; 2],
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
    blit_bind: wgpu::BindGroup,
}

struct Buffers {
    revision: u64,
    lines: Option<(wgpu::Buffer, u32)>,
    /// Vertices, indices, opaque index count and total index count.
    mesh: Option<(wgpu::Buffer, wgpu::Buffer, u32, u32)>,
}

impl ViewportRenderer {
    /// Creates the renderer and registers it with egui. Call once at start-up.
    pub fn install(render_state: &egui_wgpu::RenderState) {
        let renderer = Self::new(&render_state.device, render_state.target_format);
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(renderer);
    }

    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let scene_shader = device.create_shader_module(wgpu::include_wgsl!("scene.wgsl"));
        let blit_shader = device.create_shader_module(wgpu::include_wgsl!("blit.wgsl"));

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hamr globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hamr globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hamr globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hamr scene"),
            bind_group_layouts: &[Some(&globals_layout)],
            ..Default::default()
        });

        let depth = |write: bool, compare: wgpu::CompareFunction| wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(write),
            depth_compare: Some(compare),
            stencil: Default::default(),
            bias: Default::default(),
        };
        let scene_target = [Some(wgpu::ColorTargetState {
            format: COLOR_FORMAT,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let msaa = wgpu::MultisampleState {
            count: SAMPLES,
            ..Default::default()
        };

        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];
        let line_pipeline_with = |label, depth_stencil| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene_shader,
                    entry_point: Some("vs_line"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<LineVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &line_attrs,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &scene_shader,
                    entry_point: Some("fs_line"),
                    compilation_options: Default::default(),
                    targets: &scene_target,
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::LineList,
                    ..Default::default()
                },
                depth_stencil: Some(depth_stencil),
                multisample: msaa,
                multiview_mask: None,
                cache: None,
            })
        };
        let line_pipeline =
            line_pipeline_with("hamr lines", depth(false, wgpu::CompareFunction::LessEqual));
        let overlay_pipeline =
            line_pipeline_with("hamr overlay", depth(false, wgpu::CompareFunction::Always));

        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let mesh_pipeline_with = |label, depth_stencil| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&scene_layout),
                vertex: wgpu::VertexState {
                    module: &scene_shader,
                    entry_point: Some("vs_mesh"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<MeshVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &mesh_attrs,
                    })],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &scene_shader,
                    entry_point: Some("fs_mesh"),
                    compilation_options: Default::default(),
                    targets: &scene_target,
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(depth_stencil),
                multisample: msaa,
                multiview_mask: None,
                cache: None,
            })
        };
        let mesh_pipeline =
            mesh_pipeline_with("hamr meshes", depth(true, wgpu::CompareFunction::LessEqual));
        // Translucent (negative) prims are depth tested but don't hide what is behind them.
        let translucent_pipeline = mesh_pipeline_with(
            "hamr translucent",
            depth(false, wgpu::CompareFunction::LessEqual),
        );

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hamr blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hamr blit"),
            bind_group_layouts: &[Some(&blit_layout)],
            ..Default::default()
        });
        let blit_entry = if target_format.is_srgb() {
            "fs_linear"
        } else {
            "fs_encode_srgb"
        };
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hamr blit"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some(blit_entry),
                compilation_options: Default::default(),
                targets: &[Some(target_format.into())],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hamr blit"),
            ..Default::default()
        });

        Self {
            globals,
            globals_bind,
            line_pipeline,
            overlay_pipeline,
            mesh_pipeline,
            translucent_pipeline,
            blit_pipeline,
            blit_layout,
            sampler,
            targets: None,
            buffers: None,
        }
    }

    fn ensure_targets(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        let texture = |label, format, samples, usage| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: extent,
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let attach = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let msaa = texture("hamr msaa", COLOR_FORMAT, SAMPLES, attach);
        let resolve = texture(
            "hamr resolve",
            COLOR_FORMAT,
            1,
            attach | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = texture("hamr depth", DEPTH_FORMAT, SAMPLES, attach);
        let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hamr blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&resolve),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.targets = Some(Targets {
            size,
            msaa,
            resolve,
            depth,
            blit_bind,
        });
    }

    fn ensure_buffers(&mut self, device: &wgpu::Device, geometry: &SceneGeometry) {
        if self
            .buffers
            .as_ref()
            .is_some_and(|b| b.revision == geometry.revision)
        {
            return;
        }
        let vertex_buffer = |label, contents: &[u8], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents,
                usage,
            })
        };
        let line_buffer = |label, lines: &[LineVertex]| {
            (!lines.is_empty()).then(|| {
                let buf = vertex_buffer(
                    label,
                    bytemuck::cast_slice(lines),
                    wgpu::BufferUsages::VERTEX,
                );
                (buf, lines.len() as u32)
            })
        };
        let lines = line_buffer("hamr lines", &geometry.lines);
        let mesh = (!geometry.mesh_indices.is_empty()).then(|| {
            let vertices = vertex_buffer(
                "hamr mesh vertices",
                bytemuck::cast_slice(&geometry.mesh_vertices),
                wgpu::BufferUsages::VERTEX,
            );
            let indices = vertex_buffer(
                "hamr mesh indices",
                bytemuck::cast_slice(&geometry.mesh_indices),
                wgpu::BufferUsages::INDEX,
            );
            let total = geometry.mesh_indices.len() as u32;
            (
                vertices,
                indices,
                geometry.opaque_index_count.min(total),
                total,
            )
        });
        self.buffers = Some(Buffers {
            revision: geometry.revision,
            lines,
            mesh,
        });
    }
}

impl CallbackTrait for ViewportCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer) = resources.get_mut::<ViewportRenderer>() else {
            return Vec::new();
        };
        if self.size_px[0] == 0 || self.size_px[1] == 0 {
            return Vec::new();
        }
        renderer.ensure_targets(device, self.size_px);
        renderer.ensure_buffers(device, &self.geometry);

        let light = Vec3::new(-0.4, -0.6, -1.0).normalize();
        let globals = Globals {
            view_proj: self.view_proj.to_cols_array_2d(),
            light_dir: light.extend(0.0).into(),
            grid_fade: self.grid_fade_center.extend(self.grid_fade_radius).into(),
        };
        queue.write_buffer(&renderer.globals, 0, bytemuck::bytes_of(&globals));

        let overlay = (!self.overlay.is_empty()).then(|| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hamr overlay"),
                contents: bytemuck::cast_slice(&self.overlay),
                usage: wgpu::BufferUsages::VERTEX,
            });
            (buffer, self.overlay.len() as u32)
        });

        let (Some(targets), Some(buffers)) = (&renderer.targets, &renderer.buffers) else {
            return Vec::new();
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("hamr scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &targets.msaa,
                depth_slice: None,
                resolve_target: Some(&targets.resolve),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
                    store: wgpu::StoreOp::Discard,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &targets.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &renderer.globals_bind, &[]);
        // Opaque solids first so they write depth, then lines and translucent solids test against
        // it, and the overlay goes on top of everything.
        if let Some((vertices, indices, opaque, _)) = &buffers.mesh {
            pass.set_pipeline(&renderer.mesh_pipeline);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..*opaque, 0, 0..1);
        }
        if let Some((vertices, count)) = &buffers.lines {
            pass.set_pipeline(&renderer.line_pipeline);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..*count, 0..1);
        }
        if let Some((vertices, indices, opaque, total)) = &buffers.mesh {
            if opaque < total {
                pass.set_pipeline(&renderer.translucent_pipeline);
                pass.set_vertex_buffer(0, vertices.slice(..));
                pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(*opaque..*total, 0, 0..1);
            }
        }
        if let Some((vertices, count)) = &overlay {
            pass.set_pipeline(&renderer.overlay_pipeline);
            pass.set_vertex_buffer(0, vertices.slice(..));
            pass.draw(0..*count, 0..1);
        }
        Vec::new()
    }

    fn paint(
        &self,
        _info: eframe::egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &CallbackResources,
    ) {
        let Some(renderer) = resources.get::<ViewportRenderer>() else {
            return;
        };
        let Some(targets) = &renderer.targets else {
            return;
        };
        pass.set_pipeline(&renderer.blit_pipeline);
        pass.set_bind_group(0, &targets.blit_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}
