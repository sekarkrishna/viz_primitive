// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Data loading and coordinate mapping.

pub mod coord_mapper;
pub mod parquet_loader;

use crate::data::parquet_loader::ColumnPair;

/// A loaded column pair ready for coordinate mapping.
pub struct LoadedData {
    /// The raw column pair from parquet.
    pub column_pair: ColumnPair,
    /// Scene-space points after coordinate mapping.
    pub scene_points: Vec<[f32; 2]>,
}
