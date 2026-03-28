// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Named viewpoint (saved viewport state).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A saved viewport state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viewpoint {
    /// Pan X offset.
    pub pan_x: f32,
    /// Pan Y offset.
    pub pan_y: f32,
    /// Zoom level.
    pub zoom: f32,
}

/// Viewpoint lookup errors.
#[derive(Debug, Error)]
pub enum ViewpointError {
    /// Viewpoint not found by name.
    #[error("Viewpoint not found: '{0}'")]
    NotFound(String),
}
