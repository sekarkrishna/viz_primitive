// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! Scatter plot example — renders ~100 SDF circles at deterministic positions.
//!
//! Run with: `cargo run --example scatter_sdf -p dr2d`

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use dr2d::{Renderer, SdfInstance, SdfShape, Viewport};

/// Generate 100 deterministic 2D points in a spiral pattern.
fn generate_points() -> Vec<SdfInstance> {
    let count = 100;
    let mut instances = Vec::with_capacity(count);
    for i in 0..count {
        let t = i as f32 / count as f32;
        let angle = t * 6.0 * std::f32::consts::PI;
        let radius = t * 200.0;
        let x = angle.cos() * radius;
        let y = angle.sin() * radius;

        // Vary color by position in the spiral
        let r = 0.2 + 0.8 * t;
        let g = 0.3 + 0.5 * (1.0 - t);
        let b = 0.6;

        instances.push(SdfInstance {
            position: [x, y],
            size: [4.0 + t * 6.0, 4.0 + t * 6.0],
            color: [r, g, b, 1.0],
            shape_type: SdfShape::Circle as u32,
            param: 0.0,
            _pad: [0.0; 2],
        });
    }
    instances
}

struct App {
    renderer: Option<Renderer>,
    viewport: Viewport,
    instances: Vec<SdfInstance>,
    window: Option<Arc<Window>>,
}

impl App {
    fn new() -> Self {
        Self {
            renderer: None,
            viewport: Viewport::new(),
            instances: generate_points(),
            window: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default().with_title("dr2d — scatter SDF");
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
                            frame.draw_sdf(SdfShape::Circle, &self.instances);
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
