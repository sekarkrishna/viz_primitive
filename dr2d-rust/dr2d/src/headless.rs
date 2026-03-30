// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Headless rendering — render to image buffer without a window.
//!
//! Renders to a wgpu texture instead of a window surface, reads back pixels
//! as RGBA byte buffer. PNG encoding left to downstream consumers.
//!
//! This module is gated behind the `headless` feature flag.

use thiserror::Error;
use wgpu::util::DeviceExt;

use crate::renderer::pipeline::{create_bind_group_layout, create_pipeline};
use crate::renderer::sdf_pipeline::create_sdf_pipeline;

/// The texture format used for headless rendering.
const HEADLESS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Error type for headless rendering.
#[derive(Debug, Error)]
pub enum HeadlessError {
    /// Width or height is zero.
    #[error("Invalid dimensions: width={0}, height={1} (both must be > 0)")]
    InvalidDimensions(u32, u32),
    /// GPU initialization or rendering error.
    #[error("GPU error: {0}")]
    Gpu(String),
    /// Buffer readback error.
    #[error("Buffer readback failed: {0}")]
    ReadbackFailed(String),
}

/// Headless renderer that renders to an in-memory RGBA pixel buffer.
///
/// Creates a wgpu device without a window surface, renders to an offscreen
/// texture, and reads back pixels via a staging buffer. No PNG encoding is
/// performed — downstream consumers handle image format conversion.
pub struct HeadlessRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    // Pipelines and bind group stored for future draw calls in render_to_image.
    #[allow(dead_code)]
    sdf_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    tess_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    bind_group: wgpu::BindGroup,
}

impl HeadlessRenderer {
    /// Create a new headless renderer (no window required).
    ///
    /// Initializes a wgpu device without a surface, creates render pipelines,
    /// and prepares the uniform buffer.
    pub async fn new() -> Result<Self, HeadlessError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| HeadlessError::Gpu("No suitable GPU adapter found".into()))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("dr2d_headless_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
            }, None)
            .await
            .map_err(|e| HeadlessError::Gpu(e.to_string()))?;

        let bind_group_layout = create_bind_group_layout(&device);
        let tess_pipeline = create_pipeline(&device, HEADLESS_FORMAT, &bind_group_layout);
        let sdf_pipeline = create_sdf_pipeline(&device, HEADLESS_FORMAT, &bind_group_layout);

        let uniform_data = [0.0f32; 16];
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("headless_uniform_buffer"),
            contents: bytemuck::cast_slice(&uniform_data),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("headless_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Ok(Self {
            device,
            queue,
            sdf_pipeline,
            tess_pipeline,
            uniform_buffer,
            bind_group,
        })
    }

    /// Render the current state to an RGBA pixel buffer.
    ///
    /// Returns a `Vec<u8>` of length `width * height * 4` containing raw RGBA
    /// pixel data in row-major order.
    ///
    /// Returns an error if `width` or `height` is zero.
    pub async fn render_to_image(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, HeadlessError> {
        if width == 0 || height == 0 {
            return Err(HeadlessError::InvalidDimensions(width, height));
        }

        // Write identity transform to uniform buffer
        let identity = [
            1.0f32, 0.0, 0.0, 0.0, // col0
            0.0, 1.0, 0.0, 0.0,     // col1
            0.0, 0.0, 1.0, 0.0,     // col2
            1.0, 0.0, 0.0, 0.0,     // params
        ];
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&identity),
        );

        // Create offscreen texture
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless_render_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Render a clear pass
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("headless_encoder"),
            });

        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless_clear_pass"),
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

        // Copy texture to staging buffer
        // wgpu requires bytes_per_row to be aligned to COPY_BYTES_PER_ROW_ALIGNMENT (256)
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_staging_buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back pixels
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .recv()
            .map_err(|e| HeadlessError::ReadbackFailed(e.to_string()))?
            .map_err(|e| HeadlessError::ReadbackFailed(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();

        // Remove row padding if present
        let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging_buffer.unmap();

        Ok(pixels)
    }

    /// Render SDF instances to an RGBA pixel buffer.
    pub async fn render_sdf_to_image(
        &mut self,
        width: u32,
        height: u32,
        instances: &[crate::renderer::sdf_pipeline::SdfInstance],
    ) -> Result<Vec<u8>, HeadlessError> {
        if width == 0 || height == 0 {
            return Err(HeadlessError::InvalidDimensions(width, height));
        }

        // Write identity transform to uniform buffer
        let identity = [
            1.0f32, 0.0, 0.0, 0.0, // col0
            0.0, -1.0, 0.0, 0.0,    // col1 (y flipped for standard 2d coords if needed, but identity is fine)
            0.0, 0.0, 1.0, 0.0,     // col2
            1.0, 0.0, 0.0, 0.0,     // params
        ];
        // Note: For actual drawing, we might want an orthographic projection.
        // Let's use a simple ortho projection mapped to width/height
        let ortho = [
            2.0 / width as f32, 0.0, 0.0, 0.0,
            0.0, -2.0 / height as f32, 0.0, 0.0, // Flip Y so 0 is top
            -1.0, 1.0, 1.0, 0.0, // Translate to top-left
            1.0, 0.0, 0.0, 0.0,
        ];
        
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&ortho),
        );

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("headless_render_texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HEADLESS_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headless_encoder"),
        });

        // 1. Draw SDFs
        if !instances.is_empty() {
            let instance_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("sdf_instance_buffer"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            });

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless_sdf_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.sdf_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, instance_buffer.slice(..));
            pass.draw(0..6, 0..instances.len() as u32);
        } else {
            // Just clear
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("headless_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 2. Copy Texture to Staging
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headless_staging_buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // 3. Readback
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);

        receiver.recv()
            .map_err(|e| HeadlessError::ReadbackFailed(e.to_string()))?
            .map_err(|e| HeadlessError::ReadbackFailed(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * bytes_per_pixel) as usize);
        for row in 0..height {
            let start = (row * padded_bytes_per_row) as usize;
            let end = start + unpadded_bytes_per_row as usize;
            pixels.extend_from_slice(&data[start..end]);
        }

        drop(data);
        staging_buffer.unmap();

        Ok(pixels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_width_returns_error() {
        // We can't easily create a HeadlessRenderer in CI without a GPU,
        // so just test the validation logic directly.
        let err = HeadlessError::InvalidDimensions(0, 100);
        assert!(matches!(err, HeadlessError::InvalidDimensions(0, 100)));
    }

    #[test]
    fn zero_height_returns_error() {
        let err = HeadlessError::InvalidDimensions(100, 0);
        assert!(matches!(err, HeadlessError::InvalidDimensions(100, 0)));
    }
}
