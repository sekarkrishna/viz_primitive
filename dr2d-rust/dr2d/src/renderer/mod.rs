// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! GPU renderer, frame submission.

pub(crate) mod gpu;
pub(crate) mod pipeline;
pub mod sdf_pipeline;
pub(crate) mod tessellation;
pub mod vertex;

use std::sync::Arc;
use wgpu::util::DeviceExt;
use thiserror::Error;

use crate::scene::Scene;
use crate::viewport::Viewport;

use self::gpu::GpuContext;
use self::pipeline::{create_bind_group_layout, create_pipeline};
use self::sdf_pipeline::{create_sdf_pipeline, SdfInstance, SdfShape};
use self::tessellation::TessellationCache;
use self::vertex::{build_vertex_buffer, InstanceData, Vertex};

/// Renderer errors.
#[derive(Debug, Error)]
pub enum RendererError {
    /// Surface texture lost.
    #[error("Surface texture acquisition failed after reconfigure")]
    SurfaceLost,
    /// GPU error.
    #[error("GPU error: {0}")]
    Gpu(String),
}

/// GPU renderer for 2D scenes.
pub struct Renderer {
    gpu: GpuContext,
    pipeline: wgpu::RenderPipeline,
    sdf_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_count: u32,
    tess_cache: TessellationCache,
}

/// Holds GPU state for a single frame between [`Renderer::begin_frame`] and
/// [`FrameEncoder::finish`].
///
/// Draw calls (`draw_sdf`, `draw_instanced`, `draw_triangles`) are recorded
/// into the command encoder. The frame is submitted and presented when
/// [`Renderer::end_frame`] consumes this struct.
pub struct FrameEncoder<'a> {
    /// The wgpu command encoder for this frame.
    encoder: wgpu::CommandEncoder,
    /// The texture view for the current surface frame.
    view: wgpu::TextureView,
    /// The acquired surface texture (presented on end_frame).
    surface_texture: wgpu::SurfaceTexture,
    /// Reference to the tessellation render pipeline.
    tess_pipeline: &'a wgpu::RenderPipeline,
    /// Reference to the SDF render pipeline.
    sdf_pipeline: &'a wgpu::RenderPipeline,
    /// Reference to the uniform bind group.
    bind_group: &'a wgpu::BindGroup,
    /// Reference to the GPU device (for creating buffers).
    device: &'a wgpu::Device,
    /// Reference to the GPU queue (for submitting commands).
    queue: &'a wgpu::Queue,
}

impl<'a> FrameEncoder<'a> {
    /// Draw SDF shapes of the given type.
    ///
    /// Skips the draw call if `instances` is empty.
    /// Each instance is rendered as a screen-space quad (6 vertices) with
    /// per-fragment SDF evaluation.
    pub fn draw_sdf(&mut self, _shape: SdfShape, instances: &[SdfInstance]) {
        if instances.is_empty() {
            return;
        }

        let instance_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("sdf_instance_buffer"),
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sdf_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(self.sdf_pipeline);
        render_pass.set_bind_group(0, self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
        render_pass.draw(0..6, 0..instances.len() as u32);
    }

    /// Draw instanced geometry with a shared mesh and per-instance transforms.
    ///
    /// Skips the draw call if `mesh` or `instances` is empty.
    pub fn draw_instanced(&mut self, mesh: &[Vertex], instances: &[InstanceData]) {
        if instances.is_empty() || mesh.is_empty() {
            return;
        }

        let vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("instanced_vertex_buffer"),
                    contents: bytemuck::cast_slice(mesh),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let instance_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("instanced_instance_buffer"),
                    contents: bytemuck::cast_slice(instances),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("instanced_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(self.tess_pipeline);
        render_pass.set_bind_group(0, self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));
        render_pass.draw(0..mesh.len() as u32, 0..instances.len() as u32);
    }

    /// Draw a flat triangle list.
    ///
    /// Skips the draw call if `vertices` is empty.
    pub fn draw_triangles(&mut self, vertices: &[Vertex]) {
        if vertices.is_empty() {
            return;
        }

        let vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("triangles_vertex_buffer"),
                    contents: bytemuck::cast_slice(vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("triangles_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(self.tess_pipeline);
        render_pass.set_bind_group(0, self.bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertices.len() as u32, 0..1);
    }

    /// Submit recorded commands and present the surface texture.
    ///
    /// Consumes the `FrameEncoder`. This is the final step of the frame lifecycle.
    pub fn finish(self) {
        self.queue.submit(std::iter::once(self.encoder.finish()));
        self.surface_texture.present();
    }
}

impl Renderer {
    /// Creates a new renderer attached to a window.
    pub async fn new(window: Arc<winit::window::Window>) -> Result<Self, RendererError> {
        let gpu = GpuContext::new(window).await?;
        let bind_group_layout = create_bind_group_layout(&gpu.device);
        let pipeline = create_pipeline(&gpu.device, gpu.config.format, &bind_group_layout);
        let sdf_pipeline = create_sdf_pipeline(&gpu.device, gpu.config.format, &bind_group_layout);

        let uniform_data = [0.0f32; 16];
        let uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniform_buffer"),
            contents: bytemuck::cast_slice(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            gpu,
            pipeline,
            sdf_pipeline,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer: None,
            vertex_count: 0,
            tess_cache: TessellationCache::new(),
        })
    }

    /// Resizes the rendering surface.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.gpu.resize(width, height);
    }

    /// Begin a new frame.
    ///
    /// Acquires the surface texture, writes the uniform buffer with the
    /// viewport transform, and returns a [`FrameEncoder`] for recording
    /// draw calls. Call [`FrameEncoder::finish`] (or [`Renderer::end_frame`])
    /// when done.
    pub fn begin_frame(&mut self, viewport: &Viewport) -> Result<FrameEncoder<'_>, RendererError> {
        // Write uniform buffer with viewport transform
        let transform = viewport.transform_matrix(
            self.gpu.config.width as f32,
            self.gpu.config.height as f32,
        );
        let mut uniform_data = [0.0f32; 16];
        uniform_data[..12].copy_from_slice(&transform);
        uniform_data[13] = viewport.zoom;
        self.gpu
            .queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&uniform_data));

        // Acquire surface texture
        let output = match self.gpu.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.gpu.reconfigure_surface();
                self.gpu
                    .surface
                    .get_current_texture()
                    .map_err(|_| RendererError::SurfaceLost)?
            }
            Err(_) => return Err(RendererError::SurfaceLost),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Issue a clear pass so subsequent draw calls can use LoadOp::Load
        {
            let mut clear_encoder = self
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("clear_encoder"),
                });
            {
                let _clear_pass = clear_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1,
                                g: 0.1,
                                b: 0.1,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }
            self.gpu
                .queue
                .submit(std::iter::once(clear_encoder.finish()));
        }

        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame_encoder"),
            });

        Ok(FrameEncoder {
            encoder,
            view,
            surface_texture: output,
            tess_pipeline: &self.pipeline,
            sdf_pipeline: &self.sdf_pipeline,
            bind_group: &self.uniform_bind_group,
            device: &self.gpu.device,
            queue: &self.gpu.queue,
        })
    }

    /// End the frame: submit recorded commands and present the surface texture.
    ///
    /// This is equivalent to calling [`FrameEncoder::finish`] directly.
    pub fn end_frame(&mut self, encoder: FrameEncoder<'_>) -> Result<(), RendererError> {
        encoder.finish();
        Ok(())
    }

    /// Renders a frame with shapes from the scene.
    ///
    /// This is a convenience wrapper around [`begin_frame`](Self::begin_frame),
    /// [`FrameEncoder::draw_triangles`], and [`end_frame`](Self::end_frame).
    pub fn render(&mut self, scene: &mut Scene, viewport: &Viewport) -> Result<(), RendererError> {
        let vertices = build_vertex_buffer(scene, &mut self.tess_cache);

        // Update cached vertex buffer for backward compat
        self.vertex_count = vertices.len() as u32;
        if !vertices.is_empty() {
            self.vertex_buffer = Some(
                self.gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("vertex_buffer"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            );
        } else {
            self.vertex_buffer = None;
        }

        let mut frame = self.begin_frame(viewport)?;
        frame.draw_triangles(&vertices);
        frame.finish();
        Ok(())
    }

    /// Returns the current window size.
    pub fn window_size(&self) -> (u32, u32) {
        (self.gpu.config.width, self.gpu.config.height)
    }

    /// Clears the tessellation cache.
    pub fn clear_tess_cache(&mut self) {
        self.tess_cache.clear();
    }
}
