// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Lyon tessellation helpers and per-polygon tessellation cache.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use lyon::math::point;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

use crate::renderer::vertex::Vertex;
use crate::scene::shape::ShapeId;

/// Tessellate a closed polygon into filled triangles.
pub(crate) fn tessellate_polygon_fill(vertices: &[[f32; 2]], color: [f32; 4]) -> Vec<Vertex> {
    if vertices.len() < 3 { return Vec::new(); }
    let mut builder = Path::builder();
    builder.begin(point(vertices[0][0], vertices[0][1]));
    for v in &vertices[1..] { builder.line_to(point(v[0], v[1])); }
    builder.close();
    let path = builder.build();

    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let result = fill_tess.tessellate_path(
        &path, &FillOptions::default(),
        &mut BuffersBuilder::new(&mut geometry, |vertex: FillVertex| Vertex {
            position: vertex.position().to_array(), color,
        }),
    );
    match result {
        Ok(_) => indexed_to_flat(&geometry),
        Err(e) => { log::warn!("Fill tessellation failed: {e}"); Vec::new() }
    }
}

/// Tessellate a polyline or closed polygon outline into stroke triangles.
pub(crate) fn tessellate_stroke(vertices: &[[f32; 2]], closed: bool, color: [f32; 4], width: f32) -> Vec<Vertex> {
    if vertices.len() < 2 { return Vec::new(); }
    let mut builder = Path::builder();
    builder.begin(point(vertices[0][0], vertices[0][1]));
    for v in &vertices[1..] { builder.line_to(point(v[0], v[1])); }
    if closed { builder.close(); } else { builder.end(false); }
    let path = builder.build();

    let mut geometry: VertexBuffers<Vertex, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();
    let options = StrokeOptions::default().with_line_width(width).with_line_join(LineJoin::Miter);
    let result = stroke_tess.tessellate_path(
        &path, &options,
        &mut BuffersBuilder::new(&mut geometry, |vertex: StrokeVertex| Vertex {
            position: vertex.position().to_array(), color,
        }),
    );
    match result {
        Ok(_) => indexed_to_flat(&geometry),
        Err(e) => { log::warn!("Stroke tessellation failed: {e}"); Vec::new() }
    }
}

fn indexed_to_flat(geometry: &VertexBuffers<Vertex, u32>) -> Vec<Vertex> {
    geometry.indices.iter().map(|&i| geometry.vertices[i as usize]).collect()
}

struct CachedTessellation {
    fill_hash: u64,
    fill_vertices: Vec<Vertex>,
    stroke_hash: u64,
    stroke_vertices: Vec<Vertex>,
}

/// Per-shape tessellation cache keyed by ShapeId.
pub(crate) struct TessellationCache {
    cache: HashMap<ShapeId, CachedTessellation>,
}

impl TessellationCache {
    /// Creates a new empty cache.
    pub fn new() -> Self { Self { cache: HashMap::new() } }

    /// Return cached fill vertices or tessellate and cache.
    pub fn get_or_tessellate_fill(&mut self, id: ShapeId, vertices: &[[f32; 2]], color: [f32; 4]) -> &[Vertex] {
        let hash = hash_fill_inputs(vertices, color);
        let entry = self.cache.entry(id).or_insert_with(|| CachedTessellation {
            fill_hash: 0, fill_vertices: Vec::new(), stroke_hash: 0, stroke_vertices: Vec::new(),
        });
        if entry.fill_hash != hash || entry.fill_vertices.is_empty() {
            entry.fill_hash = hash;
            entry.fill_vertices = tessellate_polygon_fill(vertices, color);
        }
        &entry.fill_vertices
    }

    /// Return cached stroke vertices or tessellate and cache.
    pub fn get_or_tessellate_stroke(&mut self, id: ShapeId, vertices: &[[f32; 2]], closed: bool, color: [f32; 4], width: f32) -> &[Vertex] {
        let hash = hash_stroke_inputs(vertices, color, width);
        let entry = self.cache.entry(id).or_insert_with(|| CachedTessellation {
            fill_hash: 0, fill_vertices: Vec::new(), stroke_hash: 0, stroke_vertices: Vec::new(),
        });
        if entry.stroke_hash != hash || entry.stroke_vertices.is_empty() {
            entry.stroke_hash = hash;
            entry.stroke_vertices = tessellate_stroke(vertices, closed, color, width);
        }
        &entry.stroke_vertices
    }

    /// Invalidate a specific shape.
    #[allow(dead_code)]
    pub fn invalidate(&mut self, id: ShapeId) { self.cache.remove(&id); }

    /// Clear all cached entries.
    pub fn clear(&mut self) { self.cache.clear(); }
}

fn hash_fill_inputs(vertices: &[[f32; 2]], color: [f32; 4]) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    let vertex_bytes: &[u8] = bytemuck::cast_slice(vertices);
    vertex_bytes.hash(&mut hasher);
    bytemuck::bytes_of(&color).hash(&mut hasher);
    hasher.finish()
}

fn hash_stroke_inputs(vertices: &[[f32; 2]], color: [f32; 4], width: f32) -> u64 {
    let mut hasher = std::hash::DefaultHasher::new();
    let vertex_bytes: &[u8] = bytemuck::cast_slice(vertices);
    vertex_bytes.hash(&mut hasher);
    bytemuck::bytes_of(&color).hash(&mut hasher);
    width.to_bits().hash(&mut hasher);
    hasher.finish()
}
