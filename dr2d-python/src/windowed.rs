use std::sync::Arc;
use std::collections::HashMap;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowAttributes, WindowId};

use dr2d::renderer::sdf_pipeline::{SdfInstance, SdfShape};
use dr2d::{
    InputQueue, InteractionConfig, InteractionProcessor, Renderer, Scene, StoredViewport, Viewport,
    bounding_box, fit_viewport,
};
use dr2d::input::convert_window_event;

/// Application state for the interactive windowed renderer.
pub(crate) struct App {
    title: String,
    width: u32,
    height: u32,
    sdf_instances: Vec<SdfInstance>,
    scene_points: Vec<[f32; 2]>,
    renderer: Option<Renderer>,
    viewport: Viewport,
    input_queue: InputQueue,
    interaction: Option<InteractionProcessor>,
    last_cursor_pos: (f32, f32),
    window: Option<Arc<Window>>,
    scene: Scene,
    error: Option<String>,
}

impl App {
    /// Creates a new App with the given SDF instances and window parameters.
    pub(crate) fn new(
        title: String,
        width: u32,
        height: u32,
        sdf_instances: Vec<SdfInstance>,
        scene_points: Vec<[f32; 2]>,
    ) -> Self {
        Self {
            title,
            width,
            height,
            sdf_instances,
            scene_points,
            renderer: None,
            viewport: Viewport::new(),
            input_queue: InputQueue::new(),
            interaction: None,
            last_cursor_pos: (0.0, 0.0),
            window: None,
            scene: Scene::new(),
            error: None,
        }
    }

    /// Takes any stored error, leaving None in its place.
    pub(crate) fn take_error(&mut self) -> Result<(), String> {
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Groups SDF instances by shape_type and returns (SdfShape, Vec<SdfInstance>) pairs.
    fn grouped_instances(&self) -> Vec<(SdfShape, Vec<SdfInstance>)> {
        let mut groups: HashMap<u32, Vec<SdfInstance>> = HashMap::new();
        for inst in &self.sdf_instances {
            groups.entry(inst.shape_type).or_default().push(*inst);
        }
        let mut result = Vec::new();
        for (shape_type, instances) in groups {
            let sdf_shape = match shape_type {
                0 => SdfShape::Circle,
                1 => SdfShape::RoundedRect,
                2 => SdfShape::Ring,
                3 => SdfShape::Diamond,
                4 => SdfShape::LineCap,
                _ => SdfShape::Circle, // fallback
            };
            result.push((sdf_shape, instances));
        }
        result
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title(&self.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height));

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.error = Some(format!("Failed to create window: {e}"));
                event_loop.exit();
                return;
            }
        };

        let renderer = match pollster::block_on(Renderer::new(window.clone())) {
            Ok(r) => r,
            Err(e) => {
                self.error = Some(format!("Failed to create renderer: {e}"));
                event_loop.exit();
                return;
            }
        };

        // Compute bounding box and initial viewport
        let w = self.width as f32;
        let h = self.height as f32;
        if let Some(bbox) = bounding_box(self.scene.shapes(), &self.scene_points) {
            let (pan_x, pan_y, zoom) = fit_viewport(bbox, w, h, 0.1);
            self.viewport.set_pan(pan_x, pan_y);
            let _ = self.viewport.set_zoom(zoom);
        }

        let stored = StoredViewport {
            pan_x: self.viewport.pan_x,
            pan_y: self.viewport.pan_y,
            zoom: self.viewport.zoom,
        };
        self.interaction = Some(InteractionProcessor::new(
            InteractionConfig::default(),
            stored,
        ));

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
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let (win_w, win_h) = match self.renderer.as_ref() {
                    Some(r) => r.window_size(),
                    None => return,
                };
                let w = win_w as f32;
                let h = win_h as f32;

                // Process interaction events
                if let Some(interaction) = self.interaction.as_mut() {
                    if let Some(window) = self.window.as_ref() {
                        interaction.process_events(
                            &mut self.input_queue,
                            &mut self.viewport,
                            &self.scene,
                            window,
                            w,
                            h,
                            &self.scene_points,
                        );
                    }
                }

                // Pre-compute grouped instances before borrowing renderer
                let groups = self.grouped_instances();

                // Render frame
                let renderer = self.renderer.as_mut().unwrap();
                match renderer.begin_frame(&self.viewport) {
                    Ok(mut frame) => {
                        for (shape, instances) in &groups {
                            frame.draw_sdf(*shape, instances);
                        }
                        frame.finish();
                    }
                    Err(e) => {
                        self.error = Some(format!("Render error: {e}"));
                    }
                }

                // Request continuous redraw for smooth interaction
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            other => {
                if let Some(renderer) = self.renderer.as_ref() {
                    let (win_w, win_h) = renderer.window_size();
                    if let Some(input_event) = convert_window_event(
                        &other,
                        &self.viewport,
                        win_w as f32,
                        win_h as f32,
                        &mut self.last_cursor_pos,
                    ) {
                        self.input_queue.push(input_event);
                    }
                }
            }
        }
    }
}
