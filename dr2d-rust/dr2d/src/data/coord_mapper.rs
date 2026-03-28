// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! CoordinateMapper — linear mapping from data value ranges to scene coordinate ranges.

use super::parquet_loader::ColumnPair;

/// Min/max range for a single data axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DataRange {
    /// Minimum value.
    pub min: f32,
    /// Maximum value.
    pub max: f32,
}

impl DataRange {
    fn span(&self) -> f32 {
        let s = self.max - self.min;
        if s.abs() < f32::EPSILON { 1.0 } else { s }
    }

    fn effective_min(&self) -> f32 {
        if (self.max - self.min).abs() < f32::EPSILON { self.min - 0.5 } else { self.min }
    }
}

/// Maps data coordinates to scene coordinates via linear interpolation.
#[derive(Debug, Clone, PartialEq)]
pub struct CoordinateMapper {
    /// X axis data range.
    pub x_data_range: DataRange,
    /// Y axis data range.
    pub y_data_range: DataRange,
    /// X axis scene range.
    pub x_scene_range: (f32, f32),
    /// Y axis scene range.
    pub y_scene_range: (f32, f32),
}

impl CoordinateMapper {
    const DEFAULT_SCENE_MIN: f32 = 0.0;
    const DEFAULT_SCENE_MAX: f32 = 1000.0;

    /// Compute unified data ranges from multiple column pairs.
    pub fn from_column_pairs(pairs: &[&ColumnPair]) -> Self {
        let mut x_min = f32::INFINITY;
        let mut x_max = f32::NEG_INFINITY;
        let mut y_min = f32::INFINITY;
        let mut y_max = f32::NEG_INFINITY;

        for pair in pairs {
            for &v in &pair.x { x_min = x_min.min(v); x_max = x_max.max(v); }
            for &v in &pair.y { y_min = y_min.min(v); y_max = y_max.max(v); }
        }

        if x_min > x_max { x_min = 0.0; x_max = 1.0; }
        if y_min > y_max { y_min = 0.0; y_max = 1.0; }

        Self {
            x_data_range: DataRange { min: x_min, max: x_max },
            y_data_range: DataRange { min: y_min, max: y_max },
            x_scene_range: (Self::DEFAULT_SCENE_MIN, Self::DEFAULT_SCENE_MAX),
            y_scene_range: (Self::DEFAULT_SCENE_MIN, Self::DEFAULT_SCENE_MAX),
        }
    }

    /// Map a single data point to scene coordinates.
    pub fn map_point(&self, data_x: f32, data_y: f32) -> (f32, f32) {
        let sx = self.interpolate(data_x, &self.x_data_range, self.x_scene_range);
        let sy = self.interpolate(data_y, &self.y_data_range, self.y_scene_range);
        (sx, sy)
    }

    /// Map all points in a ColumnPair to scene coordinates.
    pub fn map_all(&self, pair: &ColumnPair) -> Vec<[f32; 2]> {
        if pair.x.is_empty() { return Vec::new(); }
        pair.x.iter().zip(pair.y.iter())
            .map(|(&dx, &dy)| { let (sx, sy) = self.map_point(dx, dy); [sx, sy] })
            .collect()
    }

    fn interpolate(&self, value: f32, data_range: &DataRange, scene_range: (f32, f32)) -> f32 {
        let (scene_min, scene_max) = scene_range;
        scene_min + (value - data_range.effective_min()) / data_range.span() * (scene_max - scene_min)
    }
}
