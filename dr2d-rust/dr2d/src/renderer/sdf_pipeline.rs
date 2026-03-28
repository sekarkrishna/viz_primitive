// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! SDF (Signed Distance Function) pipeline types and pure Rust SDF evaluation functions.

/// Built-in SDF shape types evaluated in the fragment shader.
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SdfShape {
    /// Unit circle: `length(uv) - 1.0`.
    Circle = 0,
    /// Rounded rectangle with configurable corner radius.
    RoundedRect = 1,
    /// Ring (donut) with configurable thickness.
    Ring = 2,
    /// Diamond (rotated square).
    Diamond = 3,
    /// Line cap (capsule) for line segments with round ends.
    LineCap = 4,
}

/// Per-instance data for SDF rendering.
///
/// Each instance describes a single shape to be rendered as a screen-space quad
/// with per-fragment SDF evaluation.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfInstance {
    /// Center position in scene coordinates.
    pub position: [f32; 2],
    /// Half-extents (width/2, height/2) of the bounding quad.
    pub size: [f32; 2],
    /// RGBA color.
    pub color: [f32; 4],
    /// Shape type (cast from [`SdfShape`] enum).
    pub shape_type: u32,
    /// Extra parameter: corner_radius for RoundedRect, thickness for Ring, etc.
    pub param: f32,
    /// Padding for 16-byte alignment.
    pub _pad: [f32; 2],
}

impl SdfInstance {
    /// Returns the wgpu vertex buffer layout for instanced SDF rendering.
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SdfInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // position: [f32; 2] at location 0
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // size: [f32; 2] at location 1
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // color: [f32; 4] at location 2
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // shape_type: u32 at location 3
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 4]>() + std::mem::size_of::<[f32; 4]>())
                        as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
                // param: f32 at location 4
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<[f32; 4]>()
                        + std::mem::size_of::<u32>()) as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Pure Rust SDF evaluation functions (mirror the WGSL shader for testing)
// ---------------------------------------------------------------------------

/// Circle SDF: returns `sqrt(u² + v²) - 1.0`.
///
/// Negative inside the unit circle, zero on the boundary, positive outside.
pub fn sdf_circle(uv: [f32; 2]) -> f32 {
    let [u, v] = uv;
    (u * u + v * v).sqrt() - 1.0
}

/// Rounded rectangle SDF with the given corner radius.
///
/// The box spans `[-1, 1]²` before rounding.
pub fn sdf_rounded_rect(uv: [f32; 2], corner_radius: f32) -> f32 {
    let dx = uv[0].abs() - (1.0 - corner_radius);
    let dy = uv[1].abs() - (1.0 - corner_radius);
    let dx_pos = dx.max(0.0);
    let dy_pos = dy.max(0.0);
    (dx_pos * dx_pos + dy_pos * dy_pos).sqrt() + dx.max(dy).min(0.0) - corner_radius
}

/// Ring (donut) SDF: `abs(length(uv) - (1.0 - thickness)) - thickness`.
pub fn sdf_ring(uv: [f32; 2], thickness: f32) -> f32 {
    let [u, v] = uv;
    let len = (u * u + v * v).sqrt();
    (len - (1.0 - thickness)).abs() - thickness
}

/// Diamond SDF: `(abs(u) + abs(v) - 1.0) / sqrt(2.0)`.
pub fn sdf_diamond(uv: [f32; 2]) -> f32 {
    let [u, v] = uv;
    (u.abs() + v.abs() - 1.0) / 2.0_f32.sqrt()
}

/// Line cap (capsule) SDF: capsule along the x-axis from `(-1, 0)` to `(1, 0)`.
pub fn sdf_line_cap(uv: [f32; 2]) -> f32 {
    let [u, v] = uv;
    let px = u.abs() - 1.0;
    let px_pos = px.max(0.0);
    let py_pos = v.max(0.0);
    (px_pos * px_pos + py_pos * py_pos).sqrt() + px.max(v).min(0.0)
}

/// WGSL shader source for SDF rendering.
///
/// The vertex shader expands a unit quad `[-1,1]²` per instance using `vertex_index`.
/// The fragment shader evaluates the SDF based on `shape_type`, uses `smoothstep` +
/// `fwidth` for anti-aliasing, and discards transparent fragments.
const SDF_SHADER_SOURCE: &str = r#"
struct Uniforms {
    col0: vec4<f32>,
    col1: vec4<f32>,
    col2: vec4<f32>,
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct SdfVaryings {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shape_type: u32,
    @location(3) param: f32,
};

@vertex
fn vs_sdf(
    @builtin(vertex_index) vi: u32,
    @location(0) inst_position: vec2<f32>,
    @location(1) inst_size: vec2<f32>,
    @location(2) inst_color: vec4<f32>,
    @location(3) inst_shape_type: u32,
    @location(4) inst_param: f32,
) -> SdfVaryings {
    // 6 vertices for 2 triangles forming a quad
    var quad_pos = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),  vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let uv = quad_pos[vi];
    let world_pos = inst_position + uv * inst_size;

    let m = mat3x3<f32>(
        uniforms.col0.xyz,
        uniforms.col1.xyz,
        uniforms.col2.xyz,
    );
    let transformed = m * vec3<f32>(world_pos, 1.0);

    var out: SdfVaryings;
    out.clip_position = vec4<f32>(transformed.xy, 0.0, 1.0);
    out.uv = uv;
    out.color = inst_color;
    out.shape_type = inst_shape_type;
    out.param = inst_param;
    return out;
}

// --- SDF evaluation functions ---

fn sdf_circle(uv: vec2<f32>) -> f32 {
    return length(uv) - 1.0;
}

fn sdf_rounded_rect(uv: vec2<f32>, r: f32) -> f32 {
    let d = abs(uv) - vec2<f32>(1.0 - r);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - r;
}

fn sdf_ring(uv: vec2<f32>, thickness: f32) -> f32 {
    return abs(length(uv) - (1.0 - thickness)) - thickness;
}

fn sdf_diamond(uv: vec2<f32>) -> f32 {
    let p = abs(uv);
    return (p.x + p.y - 1.0) / sqrt(2.0);
}

fn sdf_line_cap(uv: vec2<f32>) -> f32 {
    let p = vec2<f32>(abs(uv.x) - 1.0, uv.y);
    return length(max(p, vec2<f32>(0.0))) + min(max(p.x, p.y), 0.0);
}

fn sdf_evaluate(uv: vec2<f32>, shape_type: u32, param: f32) -> f32 {
    switch shape_type {
        case 0u: { return sdf_circle(uv); }
        case 1u: { return sdf_rounded_rect(uv, param); }
        case 2u: { return sdf_ring(uv, param); }
        case 3u: { return sdf_diamond(uv); }
        case 4u: { return sdf_line_cap(uv); }
        default: { return sdf_circle(uv); }
    }
}

@fragment
fn fs_sdf(in: SdfVaryings) -> @location(0) vec4<f32> {
    let d = sdf_evaluate(in.uv, in.shape_type, in.param);
    let aa = fwidth(d);
    let alpha = 1.0 - smoothstep(-aa, aa, d);
    if (alpha < 0.001) {
        discard;
    }
    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
"#;

/// Creates the SDF render pipeline.
///
/// The pipeline uses instanced rendering with no vertex buffer — quad positions
/// are generated from `vertex_index`. Instance data comes from an [`SdfInstance`]
/// buffer. Alpha blending is enabled. The bind group layout is shared with the
/// existing tessellation pipeline.
pub(crate) fn create_sdf_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("dr2d_sdf_shader"),
        source: wgpu::ShaderSource::Wgsl(SDF_SHADER_SOURCE.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sdf_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sdf_render_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_sdf"),
            buffers: &[SdfInstance::desc()],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_sdf"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_at_origin_is_negative() {
        assert!(sdf_circle([0.0, 0.0]) < 0.0);
    }

    #[test]
    fn circle_outside_is_positive() {
        assert!(sdf_circle([2.0, 0.0]) > 0.0);
    }

    #[test]
    fn circle_on_boundary_is_zero() {
        let d = sdf_circle([1.0, 0.0]);
        assert!(d.abs() < 1e-6);
    }

    #[test]
    fn sdf_instance_size_is_48_bytes() {
        assert_eq!(std::mem::size_of::<SdfInstance>(), 48);
    }
}
