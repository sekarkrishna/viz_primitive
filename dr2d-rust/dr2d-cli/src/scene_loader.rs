// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! TOML scene file parsing and conversion to dr2d types.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::Deserialize;

use dr2d::{SdfInstance, SdfShape, Viewport};

/// Errors from scene loading.
#[derive(Debug)]
pub enum SceneLoadError {
    /// File I/O error.
    Io(std::io::Error),
    /// TOML parse error.
    Parse(toml::de::Error),
    /// Unknown shape type in TOML.
    UnknownShape(String),
}

impl fmt::Display for SceneLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "Failed to read scene file: {e}"),
            Self::Parse(e) => write!(f, "Failed to parse TOML: {e}"),
            Self::UnknownShape(s) => write!(f, "Unknown shape type: '{s}'"),
        }
    }
}

impl From<std::io::Error> for SceneLoadError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for SceneLoadError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

// ── TOML-deserializable structs ──────────────────────────────────────

/// Top-level TOML scene file.
#[derive(Debug, Deserialize)]
pub struct SceneConfig {
    /// Inline shapes declared in the TOML.
    #[serde(default)]
    pub shapes: Vec<TomlShape>,
    /// Data source references (Parquet files).
    #[serde(default)]
    pub data_sources: Vec<TomlDataSource>,
    /// Named viewpoints.
    #[serde(default)]
    pub viewpoints: HashMap<String, TomlViewpoint>,
}

/// A shape entry in the TOML file.
#[derive(Debug, Deserialize)]
pub struct TomlShape {
    /// Shape type name: "circle", "rounded_rect", "ring", "diamond", "line_cap".
    #[serde(rename = "type")]
    pub shape_type: String,
    /// X position in scene coordinates.
    pub x: f32,
    /// Y position in scene coordinates.
    pub y: f32,
    /// Size (half-extent).
    #[serde(default = "default_size")]
    pub size: f32,
    /// RGBA color array.
    #[serde(default = "default_color")]
    pub color: [f32; 4],
    /// Extra parameter (corner radius, ring thickness, etc.).
    #[serde(default)]
    pub param: f32,
}

/// A data source entry in the TOML file.
#[derive(Debug, Deserialize)]
#[allow(dead_code)] // Fields deserialized from TOML, used in future data source rendering
pub struct TomlDataSource {
    /// Path to the Parquet file.
    pub path: String,
    /// Column name for X values.
    pub x_column: String,
    /// Column name for Y values.
    pub y_column: String,
    /// Shape type for data points.
    #[serde(default = "default_shape_str")]
    pub shape: String,
    /// Size for data point shapes.
    #[serde(default = "default_size")]
    pub size: f32,
}

/// A viewpoint entry in the TOML file.
#[derive(Debug, Deserialize)]
pub struct TomlViewpoint {
    /// Pan X offset.
    #[serde(default)]
    pub pan_x: f32,
    /// Pan Y offset.
    #[serde(default)]
    pub pan_y: f32,
    /// Zoom level.
    #[serde(default = "default_zoom")]
    pub zoom: f32,
}

fn default_size() -> f32 {
    5.0
}
fn default_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}
fn default_shape_str() -> String {
    "circle".to_string()
}
fn default_zoom() -> f32 {
    1.0
}

// ── Parsing ──────────────────────────────────────────────────────────

/// Load and parse a TOML scene file.
pub fn load_scene(path: &Path) -> Result<SceneConfig, SceneLoadError> {
    let contents = std::fs::read_to_string(path)?;
    let config: SceneConfig = toml::from_str(&contents)?;
    Ok(config)
}

// ── Conversion helpers ───────────────────────────────────────────────

/// Map a shape type string to an `SdfShape` enum variant.
pub fn parse_sdf_shape(name: &str) -> Result<SdfShape, SceneLoadError> {
    match name {
        "circle" => Ok(SdfShape::Circle),
        "rounded_rect" => Ok(SdfShape::RoundedRect),
        "ring" => Ok(SdfShape::Ring),
        "diamond" => Ok(SdfShape::Diamond),
        "line_cap" => Ok(SdfShape::LineCap),
        other => Err(SceneLoadError::UnknownShape(other.to_string())),
    }
}

/// Convert a `TomlShape` into an `SdfInstance`.
pub fn toml_shape_to_instance(shape: &TomlShape) -> Result<SdfInstance, SceneLoadError> {
    let sdf_shape = parse_sdf_shape(&shape.shape_type)?;
    Ok(SdfInstance {
        position: [shape.x, shape.y],
        size: [shape.size, shape.size],
        color: shape.color,
        shape_type: sdf_shape as u32,
        param: shape.param,
        _pad: [0.0; 2],
    })
}

/// Convert all TOML shapes into SDF instances, grouped by shape type.
/// Returns a Vec of (SdfShape, Vec<SdfInstance>) pairs.
pub fn build_sdf_instances(
    config: &SceneConfig,
) -> Result<Vec<(SdfShape, Vec<SdfInstance>)>, SceneLoadError> {
    // Group instances by shape type using a simple vec scan
    let mut groups: Vec<(SdfShape, Vec<SdfInstance>)> = Vec::new();
    for shape in &config.shapes {
        let sdf_shape = parse_sdf_shape(&shape.shape_type)?;
        let instance = toml_shape_to_instance(shape)?;
        if let Some(group) = groups.iter_mut().find(|(s, _)| *s as u32 == sdf_shape as u32) {
            group.1.push(instance);
        } else {
            groups.push((sdf_shape, vec![instance]));
        }
    }
    Ok(groups)
}

/// Apply the first viewpoint (or "default") to a Viewport.
pub fn apply_viewpoint(config: &SceneConfig, viewport: &mut Viewport) {
    let vp = config
        .viewpoints
        .get("default")
        .or_else(|| config.viewpoints.values().next());
    if let Some(vp) = vp {
        viewport.set_pan(vp.pan_x, vp.pan_y);
        let _ = viewport.set_zoom(vp.zoom.max(0.001));
    }
}
