// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Vertex layout, buffer building.

use crate::renderer::tessellation::TessellationCache;
use crate::scene::shape::ShapeGeometry;
use crate::scene::Scene;

/// GPU vertex with position and RGBA color.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Position in scene coordinates.
    pub position: [f32; 2],
    /// RGBA color.
    pub color: [f32; 4],
}

/// Per-instance data for instanced mesh rendering.
///
/// Each instance offsets and scales a shared mesh, with an RGBA color override.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceData {
    /// Offset position in scene coordinates.
    pub position: [f32; 2],
    /// Scale factors (width, height).
    pub size: [f32; 2],
    /// RGBA color override.
    pub color: [f32; 4],
}

impl InstanceData {
    /// Returns the wgpu vertex buffer layout for instanced mesh rendering.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceData>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position: [f32; 2] at location 2
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size: [f32; 2] at location 3
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color: [f32; 4] at location 4
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 2]>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

impl Vertex {
    /// Returns the wgpu vertex buffer layout.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1, format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Generates a quad (2 triangles, 6 vertices).
pub(crate) fn push_quad(vertices: &mut Vec<Vertex>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
    let tl = [x, y]; let tr = [x + w, y]; let bl = [x, y + h]; let br = [x + w, y + h];
    vertices.push(Vertex { position: tl, color });
    vertices.push(Vertex { position: tr, color });
    vertices.push(Vertex { position: bl, color });
    vertices.push(Vertex { position: bl, color });
    vertices.push(Vertex { position: tr, color });
    vertices.push(Vertex { position: br, color });
}

/// Emits exactly 3 vertices (1 triangle).
pub(crate) fn push_triangle(vertices: &mut Vec<Vertex>, tri: &[[f32; 2]; 3], color: [f32; 4]) {
    for &pos in tri {
        vertices.push(Vertex { position: pos, color });
    }
}

/// Builds a flat vertex buffer from all scene shapes, sorted by draw order.
pub(crate) fn build_vertex_buffer(scene: &mut Scene, tess_cache: &mut TessellationCache) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let sorted_ids = scene.shapes_sorted().to_vec();

    for id in &sorted_ids {
        let shape = match scene.get_shape(*id) {
            Some(s) => s,
            None => continue,
        };
        let fill_color = [shape.color[0], shape.color[1], shape.color[2], shape.opacity];

        match &shape.geometry {
            ShapeGeometry::Rectangle { x, y, width, height } => {
                push_quad(&mut vertices, *x, *y, *width, *height, fill_color);
                if shape.border_width > 0.0 {
                    if let Some(bc) = shape.border_color {
                        let bw = shape.border_width;
                        let border_color = [bc[0], bc[1], bc[2], shape.opacity];
                        push_quad(&mut vertices, *x, *y, *width, bw, border_color);
                        push_quad(&mut vertices, *x, *y + *height - bw, *width, bw, border_color);
                        push_quad(&mut vertices, *x, *y + bw, bw, *height - 2.0 * bw, border_color);
                        push_quad(&mut vertices, *x + *width - bw, *y + bw, bw, *height - 2.0 * bw, border_color);
                    }
                }
            }
            ShapeGeometry::Triangle { vertices: tri_verts } => {
                push_triangle(&mut vertices, tri_verts, fill_color);
                if shape.border_width > 0.0 {
                    if let Some(bc) = shape.border_color {
                        let border_color = [bc[0], bc[1], bc[2], shape.opacity];
                        let stroke_verts = tess_cache.get_or_tessellate_stroke(
                            *id, tri_verts.as_slice(), true, border_color, shape.border_width,
                        );
                        vertices.extend_from_slice(stroke_verts);
                    }
                }
            }
            ShapeGeometry::Polygon { vertices: poly_verts } => {
                let fill_verts = tess_cache.get_or_tessellate_fill(*id, poly_verts, fill_color);
                vertices.extend_from_slice(fill_verts);
                if shape.border_width > 0.0 {
                    if let Some(bc) = shape.border_color {
                        let border_color = [bc[0], bc[1], bc[2], shape.opacity];
                        let stroke_verts = tess_cache.get_or_tessellate_stroke(
                            *id, poly_verts, true, border_color, shape.border_width,
                        );
                        vertices.extend_from_slice(stroke_verts);
                    }
                }
            }
        }
    }
    vertices
}
