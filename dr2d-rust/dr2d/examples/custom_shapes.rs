// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Custom shapes example — renders one of each SDF shape type in a row.
//!
//! Run with: `cargo run --example custom_shapes -p dr2d`

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use dr2d::{Renderer, SdfInstance, SdfShape, Viewport};

/// Build one instance per SDF shape type, arranged horizontally.
fn build_shape_instances() -> Vec<(SdfShape, Vec<SdfInstance>)> {
    let shapes = [
        (SdfShape::Circle, [0.2, 0.6, 1.0, 1.0], 0.0),
        (SdfShape::RoundedRect, [1.0, 0.4, 0.2, 1.0], 0.3),
        (SdfShape::Ring, [0.3, 1.0, 0.5, 1.0], 0.15),
        (SdfShape::Diamond, [1.0, 0.8, 0.1, 1.0], 0.0),
        (SdfShape::LineCap, [0.7, 0.3, 0.9, 1.0], 0.0),
    ];

    let spacing = 80.0;
    let start_x = -((shapes.len() as f32 - 1.0) / 2.0) * spacing;

    shapes
        .iter()
        .enumerate()
        .map(|(i, &(shape, color, param))| {
            let x = start_x + i as f32 * spacing;
            let instance = SdfInstance {
                position: [x, 0.0],
                size: [25.0, 25.0],
                color,
                shape_type: shape as u32,
                param,
                _pad: [0.0; 2],
            };
            (shape, vec![instance])
        })
        .collect()
}

struct App {
    renderer: Option<Renderer>,
    viewport: Viewport,
    shape_groups: Vec<(SdfShape, Vec<SdfInstance>)>,
    window: Option<Arc<Window>>,
}

impl App {
    fn new() -> Self {
        Self {
            renderer: None,
            viewport: Viewport::new(),
            shape_groups: build_shape_instances(),
            window: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default().with_title("dr2d — custom shapes");
        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        let renderer =
            pollster::block_on(Renderer::new(window.clone())).expect("Failed to create renderer");
        self.renderer = Some(renderer);
        self.window = Some(window);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(r) = self.renderer.as_mut() {
                    match r.begin_frame(&self.viewport) {
                        Ok(mut frame) => {
                            for (shape, instances) in &self.shape_groups {
                                frame.draw_sdf(*shape, instances);
                            }
                            frame.finish();
                        }
                        Err(e) => eprintln!("Render error: {e}"),
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("Event loop failed");
}
