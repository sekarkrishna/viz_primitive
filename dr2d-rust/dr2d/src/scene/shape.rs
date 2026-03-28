// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Shape types and geometry definitions.

use thiserror::Error;

/// Geometry-specific data for each shape type.
#[derive(Clone, Debug)]
pub enum ShapeGeometry {
    /// Axis-aligned rectangle.
    Rectangle {
        /// X position of the top-left corner.
        x: f32,
        /// Y position of the top-left corner.
        y: f32,
        /// Width in scene units.
        width: f32,
        /// Height in scene units.
        height: f32,
    },
    /// Arbitrary polygon (3+ vertices).
    Polygon {
        /// Ordered list of polygon vertices.
        vertices: Vec<[f32; 2]>,
    },
    /// Triangle (exactly 3 vertices).
    Triangle {
        /// Three vertices of the triangle.
        vertices: [[f32; 2]; 3],
    },
}

/// A visual element in the scene.
#[derive(Clone, Debug)]
pub struct Shape {
    /// The geometry of this shape.
    pub geometry: ShapeGeometry,
    /// RGB fill color.
    pub color: [f32; 3],
    /// Opacity (0.0 to 1.0).
    pub opacity: f32,
    /// Draw order layer.
    pub layer: i32,
    /// Optional border color.
    pub border_color: Option<[f32; 3]>,
    /// Border width in scene units.
    pub border_width: f32,
}

/// Unique identifier for a shape in a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ShapeId(pub u64);

/// Shape validation errors.
#[derive(Debug, Error)]
pub enum ShapeError {
    /// Rectangle dimensions must be positive.
    #[error("Invalid dimensions: width ({width}) and height ({height}) must be > 0.0")]
    InvalidDimensions {
        /// The invalid width value.
        width: f32,
        /// The invalid height value.
        height: f32,
    },
    /// Border width must be non-negative.
    #[error("Invalid border width: {0} must be >= 0.0")]
    InvalidBorderWidth(f32),
    /// Shape not found by ID.
    #[error("Shape not found: {0:?}")]
    NotFound(ShapeId),
    /// Polygon needs at least 3 vertices.
    #[error("Too few vertices: {count} (minimum 3 required)")]
    TooFewVertices {
        /// The actual vertex count provided.
        count: usize,
    },
    /// Polygon has too many vertices.
    #[error("Too many vertices: {count} (maximum 1024 allowed)")]
    TooManyVertices {
        /// The actual vertex count provided.
        count: usize,
    },
    /// Triangle must have exactly 3 vertices.
    #[error("Invalid triangle vertex count: {count} (exactly 3 required)")]
    InvalidTriangleVertexCount {
        /// The actual vertex count provided.
        count: usize,
    },
}

impl Shape {
    /// Validates shape geometry and properties.
    pub fn validate(&mut self) -> Result<(), ShapeError> {
        match &self.geometry {
            ShapeGeometry::Rectangle { width, height, .. } => {
                if *width <= 0.0 || *height <= 0.0 {
                    return Err(ShapeError::InvalidDimensions { width: *width, height: *height });
                }
            }
            ShapeGeometry::Polygon { vertices } => {
                let count = vertices.len();
                if count < 3 { return Err(ShapeError::TooFewVertices { count }); }
                if count > 1024 { return Err(ShapeError::TooManyVertices { count }); }
            }
            ShapeGeometry::Triangle { .. } => {}
        }
        if self.border_width < 0.0 {
            return Err(ShapeError::InvalidBorderWidth(self.border_width));
        }
        self.opacity = self.opacity.clamp(0.0, 1.0);
        Ok(())
    }
}
