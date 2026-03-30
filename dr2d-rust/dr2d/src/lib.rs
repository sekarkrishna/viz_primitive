// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! # dr2d
//!
//! GPU-accelerated 2D data renderer built on wgpu.
//!
//! dr2d is a pure rendering primitive. It knows about vertices, viewports,
//! data, and GPU. It does not know what a "chart" or "scene" is.

#![warn(missing_docs)]

pub mod data;
pub mod input;
#[allow(dead_code)]
pub mod interaction;
pub mod renderer;
pub mod scene;
pub mod viewport;

// Placeholder modules behind feature flags
#[cfg(feature = "text")]
pub mod text;

#[cfg(feature = "headless")]
pub mod headless;

// --- Public re-exports ---

// Viewport
pub use viewport::{Viewport, ViewportError};

// Input
pub use input::{InputQueue, InputEvent};

// Scene
pub use scene::shape::{Shape, ShapeGeometry, ShapeId, ShapeError};
pub use scene::Scene;

// Renderer
pub use renderer::{Renderer, RendererError, FrameEncoder};

// SDF pipeline
pub use renderer::sdf_pipeline::{SdfShape, SdfInstance};

// Vertex types
pub use renderer::vertex::{Vertex, InstanceData};

// Data loading
pub use data::parquet_loader::{ParquetLoader, ColumnPair, ParquetError};
pub use data::coord_mapper::{CoordinateMapper, DataRange};

// Interaction
pub use interaction::{
    InteractionProcessor, InteractionConfig, StoredViewport,
    DragState, ModifierState, bounding_box, fit_viewport,
};

// Feature-gated re-exports

#[cfg(feature = "text")]
pub use text::{GlyphAtlas, GlyphError};

#[cfg(feature = "headless")]
pub use headless::{HeadlessRenderer, HeadlessError};
