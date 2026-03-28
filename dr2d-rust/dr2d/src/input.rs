// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Input event types, coordinate conversion, event queue.

use crate::viewport::Viewport;

pub use winit::event::{ElementState, MouseButton};
pub use winit::keyboard::KeyCode;

/// Input events in both screen and scene coordinates.
#[derive(Clone, Debug)]
pub enum InputEvent {
    /// Mouse button press/release with coordinates.
    MouseButton {
        /// Which button.
        button: MouseButton,
        /// Pressed or released.
        state: ElementState,
        /// Screen X in pixels.
        screen_x: f32,
        /// Screen Y in pixels.
        screen_y: f32,
        /// Scene X coordinate.
        scene_x: f32,
        /// Scene Y coordinate.
        scene_y: f32,
    },
    /// Mouse movement with coordinates.
    MouseMove {
        /// Screen X in pixels.
        screen_x: f32,
        /// Screen Y in pixels.
        screen_y: f32,
        /// Scene X coordinate.
        scene_x: f32,
        /// Scene Y coordinate.
        scene_y: f32,
    },
    /// Keyboard key press/release.
    KeyboardKey {
        /// Which key.
        key: KeyCode,
        /// Pressed or released.
        state: ElementState,
    },
    /// Scroll wheel input.
    Scroll {
        /// Horizontal scroll delta.
        delta_x: f32,
        /// Vertical scroll delta.
        delta_y: f32,
    },
    /// Modifier key state change.
    ModifiersChanged {
        /// Alt key state.
        alt: bool,
        /// Ctrl key state.
        ctrl: bool,
        /// Shift key state.
        shift: bool,
        /// Super/Meta key state.
        super_key: bool,
    },
}

/// Queue for buffering input events between frames.
pub struct InputQueue {
    events: Vec<InputEvent>,
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl InputQueue {
    /// Creates a new empty input queue.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    /// Pushes an event onto the queue.
    pub fn push(&mut self, event: InputEvent) {
        self.events.push(event);
    }

    /// Drains all events from the queue.
    pub fn drain(&mut self) -> Vec<InputEvent> {
        std::mem::take(&mut self.events)
    }
}

/// Converts a winit WindowEvent into an InputEvent, if applicable.
pub fn convert_window_event(
    event: &winit::event::WindowEvent,
    viewport: &Viewport,
    window_width: f32,
    window_height: f32,
    last_cursor_pos: &mut (f32, f32),
) -> Option<InputEvent> {
    match event {
        winit::event::WindowEvent::MouseInput { state, button, .. } => {
            let (scene_x, scene_y) = viewport.window_to_scene(
                last_cursor_pos.0, last_cursor_pos.1, window_width, window_height,
            );
            Some(InputEvent::MouseButton {
                button: *button, state: *state,
                screen_x: last_cursor_pos.0, screen_y: last_cursor_pos.1,
                scene_x, scene_y,
            })
        }
        winit::event::WindowEvent::CursorMoved { position, .. } => {
            last_cursor_pos.0 = position.x as f32;
            last_cursor_pos.1 = position.y as f32;
            let (scene_x, scene_y) = viewport.window_to_scene(
                position.x as f32, position.y as f32, window_width, window_height,
            );
            Some(InputEvent::MouseMove {
                screen_x: position.x as f32, screen_y: position.y as f32,
                scene_x, scene_y,
            })
        }
        winit::event::WindowEvent::KeyboardInput { event, .. } => {
            if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                Some(InputEvent::KeyboardKey { key: key_code, state: event.state })
            } else {
                None
            }
        }
        winit::event::WindowEvent::MouseWheel { delta, .. } => {
            let (dx, dy) = match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    const PIXELS_PER_LINE: f32 = 20.0;
                    (pos.x as f32 / PIXELS_PER_LINE, pos.y as f32 / PIXELS_PER_LINE)
                }
            };
            Some(InputEvent::Scroll { delta_x: dx, delta_y: dy })
        }
        winit::event::WindowEvent::ModifiersChanged(modifiers) => {
            let state = modifiers.state();
            Some(InputEvent::ModifiersChanged {
                alt: state.alt_key(), ctrl: state.control_key(),
                shift: state.shift_key(), super_key: state.super_key(),
            })
        }
        _ => None,
    }
}
