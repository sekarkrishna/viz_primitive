// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Scene struct, shape CRUD operations.

pub mod shape;
pub mod viewpoint;

use std::collections::HashMap;

use crate::scene::shape::{Shape, ShapeError, ShapeId};
use crate::scene::viewpoint::{Viewpoint, ViewpointError};

/// A scene containing shapes and named viewpoints.
pub struct Scene {
    shapes: HashMap<ShapeId, Shape>,
    sorted_ids: Vec<ShapeId>,
    viewpoints: HashMap<String, Viewpoint>,
    dirty: bool,
    next_id: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Creates a new empty scene.
    pub fn new() -> Self {
        Self {
            shapes: HashMap::new(),
            sorted_ids: Vec::new(),
            viewpoints: HashMap::new(),
            dirty: false,
            next_id: 0,
        }
    }

    /// Adds a shape to the scene, returning its ID.
    pub fn add_shape(&mut self, mut shape: Shape) -> Result<ShapeId, ShapeError> {
        shape.validate()?;
        let id = ShapeId(self.next_id);
        self.next_id += 1;
        self.shapes.insert(id, shape);
        self.dirty = true;
        Ok(id)
    }

    /// Updates an existing shape.
    pub fn update_shape(&mut self, id: ShapeId, mut shape: Shape) -> Result<(), ShapeError> {
        shape.validate()?;
        if !self.shapes.contains_key(&id) {
            return Err(ShapeError::NotFound(id));
        }
        self.shapes.insert(id, shape);
        self.dirty = true;
        Ok(())
    }

    /// Removes a shape by ID.
    pub fn remove_shape(&mut self, id: ShapeId) -> Result<(), ShapeError> {
        if self.shapes.remove(&id).is_none() {
            return Err(ShapeError::NotFound(id));
        }
        self.dirty = true;
        Ok(())
    }

    /// Gets a shape by ID.
    pub fn get_shape(&self, id: ShapeId) -> Option<&Shape> {
        self.shapes.get(&id)
    }

    /// Returns shape IDs sorted by layer.
    pub fn shapes_sorted(&mut self) -> &[ShapeId] {
        if self.dirty {
            self.sorted_ids = self.shapes.keys().copied().collect();
            self.sorted_ids.sort_by_key(|id| {
                self.shapes.get(id).map(|s| s.layer).unwrap_or(0)
            });
            self.dirty = false;
        }
        &self.sorted_ids
    }

    /// Returns the number of shapes.
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }

    /// Registers a named viewpoint.
    pub fn register_viewpoint(&mut self, name: String, viewpoint: Viewpoint) {
        self.viewpoints.insert(name, viewpoint);
    }

    /// Activates a named viewpoint.
    pub fn activate_viewpoint(&self, name: &str) -> Result<Viewpoint, ViewpointError> {
        self.viewpoints
            .get(name)
            .cloned()
            .ok_or_else(|| ViewpointError::NotFound(name.to_string()))
    }

    /// Removes a named viewpoint.
    pub fn remove_viewpoint(&mut self, name: &str) {
        self.viewpoints.remove(name);
    }

    /// Returns a reference to the shapes map.
    pub fn shapes(&self) -> &HashMap<ShapeId, Shape> {
        &self.shapes
    }

    /// Returns a reference to the viewpoints map.
    pub fn viewpoints(&self) -> &HashMap<String, Viewpoint> {
        &self.viewpoints
    }
}
