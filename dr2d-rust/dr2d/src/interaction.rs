// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Interaction processor: translates input events into viewport mutations.

use std::sync::Arc;
use std::collections::HashMap;

use log::warn;
use winit::window::Window;

use crate::input::{InputEvent, InputQueue, ElementState, MouseButton as WinitMouseButton, KeyCode};
use crate::scene::Scene;
use crate::scene::shape::{Shape, ShapeGeometry, ShapeId};
use crate::viewport::Viewport;

/// Modifier key state.
#[derive(Default, Clone, Debug)]
pub struct ModifierState {
    /// Alt key pressed.
    pub alt: bool,
    /// Ctrl key pressed.
    pub ctrl: bool,
    /// Shift key pressed.
    pub shift: bool,
    /// Super/Meta key pressed.
    pub super_key: bool,
}

/// Computes the axis-aligned bounding box of all shapes and scatter points.
pub fn bounding_box(shapes: &HashMap<ShapeId, Shape>, scene_points: &[[f32; 2]]) -> Option<(f32, f32, f32, f32)> {
    let has_shapes = !shapes.is_empty();
    let has_points = !scene_points.is_empty();

    if !has_shapes && !has_points {
        return None;
    }

    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for shape in shapes.values() {
        match &shape.geometry {
            ShapeGeometry::Rectangle { x, y, width, height } => {
                min_x = min_x.min(*x);
                min_y = min_y.min(*y);
                max_x = max_x.max(*x + *width);
                max_y = max_y.max(*y + *height);
            }
            ShapeGeometry::Polygon { vertices } => {
                for v in vertices {
                    min_x = min_x.min(v[0]);
                    min_y = min_y.min(v[1]);
                    max_x = max_x.max(v[0]);
                    max_y = max_y.max(v[1]);
                }
            }
            ShapeGeometry::Triangle { vertices } => {
                for v in vertices {
                    min_x = min_x.min(v[0]);
                    min_y = min_y.min(v[1]);
                    max_x = max_x.max(v[0]);
                    max_y = max_y.max(v[1]);
                }
            }
        }
    }

    for pt in scene_points {
        min_x = min_x.min(pt[0]);
        min_y = min_y.min(pt[1]);
        max_x = max_x.max(pt[0]);
        max_y = max_y.max(pt[1]);
    }

    Some((min_x, min_y, max_x, max_y))
}

/// Computes viewport pan/zoom to fit a bounding box with padding.
pub fn fit_viewport(
    bbox: (f32, f32, f32, f32),
    window_width: f32,
    window_height: f32,
    padding_fraction: f32,
) -> (f32, f32, f32) {
    let (min_x, min_y, max_x, max_y) = bbox;
    let bbox_width = max_x - min_x;
    let bbox_height = max_y - min_y;
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    let padding_mult = 1.0 + 2.0 * padding_fraction;

    let zoom = if bbox_width <= 0.0 && bbox_height <= 0.0 {
        1.0
    } else if bbox_width <= 0.0 {
        window_height / (bbox_height * padding_mult)
    } else if bbox_height <= 0.0 {
        window_width / (bbox_width * padding_mult)
    } else {
        let zoom_x = window_width / (bbox_width * padding_mult);
        let zoom_y = window_height / (bbox_height * padding_mult);
        zoom_x.min(zoom_y)
    };

    (-center_x, -center_y, zoom)
}

/// Configuration for interaction behavior.
pub struct InteractionConfig {
    /// Pan speed in scene units per key press.
    pub pan_speed: f32,
    /// Zoom multiplier per scroll notch.
    pub zoom_factor: f32,
    /// Minimum zoom level.
    pub zoom_min: f32,
    /// Maximum zoom level.
    pub zoom_max: f32,
}

impl Default for InteractionConfig {
    fn default() -> Self {
        Self { pan_speed: 10.0, zoom_factor: 1.1, zoom_min: 0.01, zoom_max: 100.0 }
    }
}

/// Drag state for click-and-drag panning.
pub enum DragState {
    /// Not dragging.
    Idle,
    /// Currently dragging from a position.
    Dragging {
        /// Last known screen X position during drag.
        last_screen_x: f32,
        /// Last known screen Y position during drag.
        last_screen_y: f32,
    },
}

/// Stored viewport state for reset.
#[derive(Clone, Debug)]
pub struct StoredViewport {
    /// Pan X.
    pub pan_x: f32,
    /// Pan Y.
    pub pan_y: f32,
    /// Zoom.
    pub zoom: f32,
}

/// Processes input events and applies viewport mutations.
pub struct InteractionProcessor {
    config: InteractionConfig,
    drag_state: DragState,
    initial_viewport: StoredViewport,
    is_fullscreen: bool,
    pre_fullscreen_size: Option<(u32, u32)>,
    modifiers: ModifierState,
    save_requested: bool,
}

impl InteractionProcessor {
    /// Creates a new interaction processor.
    pub fn new(config: InteractionConfig, initial_viewport: StoredViewport) -> Self {
        Self {
            config, drag_state: DragState::Idle, initial_viewport,
            is_fullscreen: false, pre_fullscreen_size: None,
            modifiers: ModifierState::default(), save_requested: false,
        }
    }

    /// Updates the stored initial viewport.
    pub fn update_initial_viewport(&mut self, vp: StoredViewport) {
        self.initial_viewport = vp;
    }

    /// Returns true if Ctrl+S was pressed since the last call.
    pub fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_requested)
    }

    /// Drains all events from the queue and applies them to the viewport.
    #[allow(clippy::too_many_arguments)]
    pub fn process_events(
        &mut self,
        queue: &mut InputQueue,
        viewport: &mut Viewport,
        scene: &Scene,
        window: &Arc<Window>,
        window_width: f32,
        window_height: f32,
        scene_points: &[[f32; 2]],
    ) {
        let events = queue.drain();
        let mut pan_x = viewport.pan_x;
        let mut pan_y = viewport.pan_y;
        let mut zoom = viewport.zoom;

        for event in events {
            match event {
                InputEvent::KeyboardKey { key: KeyCode::ArrowLeft, state: ElementState::Pressed } => {
                    pan_x -= self.config.pan_speed / zoom;
                }
                InputEvent::KeyboardKey { key: KeyCode::ArrowRight, state: ElementState::Pressed } => {
                    pan_x += self.config.pan_speed / zoom;
                }
                InputEvent::KeyboardKey { key: KeyCode::ArrowUp, state: ElementState::Pressed } => {
                    pan_y += self.config.pan_speed / zoom;
                }
                InputEvent::KeyboardKey { key: KeyCode::ArrowDown, state: ElementState::Pressed } => {
                    pan_y -= self.config.pan_speed / zoom;
                }
                InputEvent::Scroll { delta_y, .. } => {
                    let factor = self.config.zoom_factor.powf(delta_y);
                    zoom = (zoom * factor).clamp(self.config.zoom_min, self.config.zoom_max);
                }
                InputEvent::MouseButton {
                    button: WinitMouseButton::Left, state: ElementState::Pressed,
                    screen_x, screen_y, ..
                } => {
                    self.drag_state = DragState::Dragging { last_screen_x: screen_x, last_screen_y: screen_y };
                }
                InputEvent::MouseButton {
                    button: WinitMouseButton::Left, state: ElementState::Released, ..
                } => {
                    self.drag_state = DragState::Idle;
                }
                InputEvent::MouseMove { screen_x, screen_y, .. } => {
                    if let DragState::Dragging { ref mut last_screen_x, ref mut last_screen_y } = self.drag_state {
                        let dx = screen_x - *last_screen_x;
                        let dy = screen_y - *last_screen_y;
                        pan_x += dx / zoom;
                        pan_y += -dy / zoom;
                        *last_screen_x = screen_x;
                        *last_screen_y = screen_y;
                    }
                }
                InputEvent::KeyboardKey { key, state: ElementState::Pressed }
                    if matches!(key, KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 |
                        KeyCode::Digit4 | KeyCode::Digit5 | KeyCode::Digit6 |
                        KeyCode::Digit7 | KeyCode::Digit8 | KeyCode::Digit9) =>
                {
                    let index = match key {
                        KeyCode::Digit1 => 0, KeyCode::Digit2 => 1, KeyCode::Digit3 => 2,
                        KeyCode::Digit4 => 3, KeyCode::Digit5 => 4, KeyCode::Digit6 => 5,
                        KeyCode::Digit7 => 6, KeyCode::Digit8 => 7, KeyCode::Digit9 => 8,
                        _ => unreachable!(),
                    };
                    let viewpoints = scene.viewpoints();
                    let mut names: Vec<&String> = viewpoints.keys().collect();
                    names.sort();
                    if let Some(name) = names.get(index) {
                        if let Some(vp) = viewpoints.get(*name) {
                            pan_x = vp.pan_x;
                            pan_y = vp.pan_y;
                            zoom = vp.zoom;
                        }
                    }
                }
                InputEvent::KeyboardKey { key: KeyCode::Home, state: ElementState::Pressed } => {
                    self.drag_state = DragState::Idle;
                    pan_x = 0.0; pan_y = 0.0; zoom = 1.0;
                }
                InputEvent::KeyboardKey { key: KeyCode::Digit0, state: ElementState::Pressed } => {
                    self.drag_state = DragState::Idle;
                    pan_x = self.initial_viewport.pan_x;
                    pan_y = self.initial_viewport.pan_y;
                    zoom = self.initial_viewport.zoom;
                }
                InputEvent::ModifiersChanged { alt, ctrl, shift, super_key } => {
                    self.modifiers = ModifierState { alt, ctrl, shift, super_key };
                }
                InputEvent::KeyboardKey { key: KeyCode::F11, state: ElementState::Pressed } => {
                    self.toggle_fullscreen(window);
                }
                InputEvent::KeyboardKey { key: KeyCode::Enter, state: ElementState::Pressed }
                    if self.modifiers.alt => { self.toggle_fullscreen(window); }
                InputEvent::KeyboardKey { key: KeyCode::KeyF, state: ElementState::Pressed }
                    if self.modifiers.super_key && self.modifiers.ctrl => { self.toggle_fullscreen(window); }
                InputEvent::KeyboardKey { key: KeyCode::Escape, state: ElementState::Pressed } => {
                    if self.is_fullscreen {
                        window.set_fullscreen(None);
                        if let Some((w, h)) = self.pre_fullscreen_size {
                            let scale = window.scale_factor();
                            let _ = window.request_inner_size(winit::dpi::LogicalSize::new(w as f64 / scale, h as f64 / scale));
                        }
                        self.is_fullscreen = false;
                        self.pre_fullscreen_size = None;
                    }
                }
                InputEvent::KeyboardKey { key: KeyCode::KeyF, state: ElementState::Pressed }
                    if !self.modifiers.super_key || !self.modifiers.ctrl =>
                {
                    if let Some(bbox) = bounding_box(scene.shapes(), scene_points) {
                        let (fit_pan_x, fit_pan_y, fit_zoom) = fit_viewport(bbox, window_width, window_height, 0.1);
                        pan_x = fit_pan_x;
                        pan_y = fit_pan_y;
                        zoom = fit_zoom.clamp(self.config.zoom_min, self.config.zoom_max);
                    } else {
                        pan_x = 0.0; pan_y = 0.0; zoom = 1.0;
                    }
                }
                InputEvent::KeyboardKey { key: KeyCode::KeyH, state: ElementState::Pressed }
                    if self.modifiers.ctrl && self.modifiers.shift =>
                {
                    if let Some(bbox) = bounding_box(scene.shapes(), scene_points) {
                        let (fit_pan_x, fit_pan_y, fit_zoom) = fit_viewport(bbox, window_width, window_height, 0.1);
                        pan_x = fit_pan_x; pan_y = fit_pan_y;
                        zoom = fit_zoom.clamp(self.config.zoom_min, self.config.zoom_max);
                    } else { pan_x = 0.0; pan_y = 0.0; zoom = 1.0; }
                }
                InputEvent::KeyboardKey { key: KeyCode::Equal | KeyCode::NumpadAdd, state: ElementState::Pressed } => {
                    zoom = (zoom * self.config.zoom_factor).clamp(self.config.zoom_min, self.config.zoom_max);
                }
                InputEvent::KeyboardKey { key: KeyCode::Minus | KeyCode::NumpadSubtract, state: ElementState::Pressed } => {
                    zoom = (zoom / self.config.zoom_factor).clamp(self.config.zoom_min, self.config.zoom_max);
                }
                InputEvent::KeyboardKey { key: KeyCode::KeyS, state: ElementState::Pressed }
                    if self.modifiers.ctrl => { self.save_requested = true; }
                _ => {}
            }
        }

        viewport.set_pan(pan_x, pan_y);
        if let Err(e) = viewport.set_zoom(zoom) {
            warn!("Failed to set final zoom: {e}");
        }
    }

    fn toggle_fullscreen(&mut self, window: &Arc<Window>) {
        if self.is_fullscreen {
            window.set_fullscreen(None);
            if let Some((w, h)) = self.pre_fullscreen_size {
                let scale = window.scale_factor();
                let _ = window.request_inner_size(winit::dpi::LogicalSize::new(w as f64 / scale, h as f64 / scale));
            }
            self.is_fullscreen = false;
            self.pre_fullscreen_size = None;
        } else {
            let size = window.inner_size();
            self.pre_fullscreen_size = Some((size.width, size.height));
            window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            self.is_fullscreen = true;
        }
    }
}
