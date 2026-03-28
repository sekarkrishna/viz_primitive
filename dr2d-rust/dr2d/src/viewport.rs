// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Viewport state (pan, zoom, transform matrix).

use thiserror::Error;

/// Viewport error types.
#[derive(Debug, Error)]
pub enum ViewportError {
    /// Zoom factor must be positive.
    #[error("Invalid zoom factor: {0} must be > 0.0")]
    InvalidZoom(f32),
}

/// Viewport state: 2D translation (pan) and uniform scale (zoom).
/// Transforms scene coordinates to NDC for GPU rendering.
#[derive(Clone, Debug)]
pub struct Viewport {
    /// Horizontal pan offset in scene units.
    pub pan_x: f32,
    /// Vertical pan offset in scene units.
    pub pan_y: f32,
    /// Zoom factor (must be > 0).
    pub zoom: f32,
}

impl Viewport {
    /// Creates a new Viewport with default state: no pan, zoom = 1.0.
    pub fn new() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }

    /// Sets the pan offset.
    pub fn set_pan(&mut self, x: f32, y: f32) {
        self.pan_x = x;
        self.pan_y = y;
    }

    /// Sets the zoom factor. Returns an error if zoom <= 0.0.
    pub fn set_zoom(&mut self, zoom: f32) -> Result<(), ViewportError> {
        if zoom <= 0.0 {
            return Err(ViewportError::InvalidZoom(zoom));
        }
        self.zoom = zoom;
        Ok(())
    }

    /// Builds a 3x3 transform matrix padded to 3×vec4 (12 floats) for GPU uniform alignment.
    pub fn transform_matrix(&self, window_width: f32, window_height: f32) -> [f32; 12] {
        let sx = self.zoom * 2.0 / window_width;
        let sy = self.zoom * 2.0 / window_height;
        [
            sx, 0.0, 0.0, 0.0,
            0.0, sy, 0.0, 0.0,
            self.pan_x * sx, self.pan_y * sy, 1.0, 0.0,
        ]
    }

    /// Converts window pixel coordinates to scene coordinates.
    pub fn window_to_scene(
        &self,
        window_x: f32,
        window_y: f32,
        window_width: f32,
        window_height: f32,
    ) -> (f32, f32) {
        let ndc_x = (window_x / window_width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (window_y / window_height) * 2.0;
        let scene_x = ndc_x * window_width / (2.0 * self.zoom) - self.pan_x;
        let scene_y = ndc_y * window_height / (2.0 * self.zoom) - self.pan_y;
        (scene_x, scene_y)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}
