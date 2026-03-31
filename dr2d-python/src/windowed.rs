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
use winit::keyboard::KeyCode;

/// Application state for the interactive windowed renderer.
pub(crate) struct App {
    title: String,
    width: u32,
    height: u32,
    sdf_instances: Vec<SdfInstance>,
    scene_points: Vec<[f32; 2]>,
    /// Start index and count for each layer in sdf_instances.
    layer_ranges: Vec<(usize, usize)>,
    /// Per-layer visibility flag. True = visible.
    layer_visible: Vec<bool>,
    renderer: Option<Renderer>,
    viewport: Viewport,
    input_queue: InputQueue,
    interaction: Option<InteractionProcessor>,
    last_cursor_pos: (f32, f32),
    window: Option<Arc<Window>>,
    scene: Scene,
    error: Option<String>,
    needs_redraw: bool,
    should_exit: bool,
}

impl App {
    /// Creates a new App with the given SDF instances and window parameters.
    ///
    /// `layer_sizes`: if empty, all instances are treated as a single layer.
    /// Otherwise each element is the instance count for one layer; cumulative
    /// offsets are computed to produce `(start, count)` ranges.
    pub(crate) fn new(
        title: String,
        width: u32,
        height: u32,
        sdf_instances: Vec<SdfInstance>,
        scene_points: Vec<[f32; 2]>,
        layer_sizes: Vec<usize>,
    ) -> Self {
        let (layer_ranges, layer_visible) = if layer_sizes.is_empty() {
            (vec![(0, sdf_instances.len())], vec![true])
        } else {
            let mut ranges = Vec::with_capacity(layer_sizes.len());
            let mut start = 0usize;
            for &count in &layer_sizes {
                ranges.push((start, count));
                start += count;
            }
            let visible = vec![true; layer_sizes.len()];
            (ranges, visible)
        };

        Self {
            title,
            width,
            height,
            sdf_instances,
            scene_points,
            layer_ranges,
            layer_visible,
            renderer: None,
            viewport: Viewport::new(),
            input_queue: InputQueue::new(),
            interaction: None,
            last_cursor_pos: (0.0, 0.0),
            window: None,
            scene: Scene::new(),
            error: None,
            needs_redraw: true,
            should_exit: false,
        }
    }

    /// Takes any stored error, leaving None in its place.
    pub(crate) fn take_error(&mut self) -> Result<(), String> {
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
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

    /// Returns instances grouped by shape_type, filtered to only visible layers.
    fn visible_instances(&self) -> Vec<(SdfShape, Vec<SdfInstance>)> {
        let mut groups: HashMap<u32, Vec<SdfInstance>> = HashMap::new();
        for ((start, count), visible) in self.layer_ranges.iter().zip(self.layer_visible.iter()) {
            if !visible {
                continue;
            }
            for inst in &self.sdf_instances[*start..*start + *count] {
                groups.entry(inst.shape_type).or_default().push(*inst);
            }
        }
        let mut result = Vec::new();
        for (shape_type, instances) in groups {
            let sdf_shape = match shape_type {
                0 => SdfShape::Circle,
                1 => SdfShape::RoundedRect,
                2 => SdfShape::Ring,
                3 => SdfShape::Diamond,
                4 => SdfShape::LineCap,
                _ => SdfShape::Circle,
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
                self.should_exit = true;
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

                // Pre-compute visible instances (filtered by layer visibility)
                let groups = self.visible_instances();

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

                // (removed — redraws are now triggered by about_to_wait)
                self.needs_redraw = false;
            }
            other => {
                // Intercept digit key presses (1-9) for layer toggle
                if let WindowEvent::KeyboardInput { event, .. } = &other {
                    if event.state == winit::event::ElementState::Pressed {
                        if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                            let layer_index = match key_code {
                                KeyCode::Digit1 => Some(0),
                                KeyCode::Digit2 => Some(1),
                                KeyCode::Digit3 => Some(2),
                                KeyCode::Digit4 => Some(3),
                                KeyCode::Digit5 => Some(4),
                                KeyCode::Digit6 => Some(5),
                                KeyCode::Digit7 => Some(6),
                                KeyCode::Digit8 => Some(7),
                                KeyCode::Digit9 => Some(8),
                                _ => None,
                            };
                            if let Some(idx) = layer_index {
                                if idx < self.layer_visible.len() {
                                    self.layer_visible[idx] = !self.layer_visible[idx];
                                    self.needs_redraw = true;
                                }
                                // Don't forward digit keys to InteractionProcessor
                                return;
                            }
                        }
                    }
                }

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
                        self.needs_redraw = true;
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.needs_redraw {
            if let Some(w) = self.window.as_ref() {
                w.request_redraw();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Storyboard types and application
// ---------------------------------------------------------------------------

/// Zoom mode for a storyboard slide.
pub(crate) enum ZoomMode {
    /// Auto-fit visible layers' bounding box.
    Fit,
    /// Use explicit pan_x, pan_y, zoom values.
    Explicit,
}

/// State for a single storyboard slide.
pub(crate) struct SlideState {
    pub zoom_mode: ZoomMode,
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    /// 0-based layer indices to show; empty = all visible.
    pub visible_layers: Vec<usize>,
    pub title: String,
}

/// Application state for the storyboard windowed renderer.
pub(crate) struct StoryboardApp {
    width: u32,
    height: u32,
    sdf_instances: Vec<SdfInstance>,
    scene_points: Vec<[f32; 2]>,
    layer_ranges: Vec<(usize, usize)>,
    layer_visible: Vec<bool>,
    slides: Vec<SlideState>,
    current_slide: usize,
    renderer: Option<Renderer>,
    viewport: Viewport,
    input_queue: InputQueue,
    interaction: Option<InteractionProcessor>,
    last_cursor_pos: (f32, f32),
    window: Option<Arc<Window>>,
    scene: Scene,
    error: Option<String>,
    press_pos: Option<(f32, f32)>,
    needs_redraw: bool,
    should_exit: bool,
}

impl StoryboardApp {
    pub(crate) fn new(
        width: u32,
        height: u32,
        sdf_instances: Vec<SdfInstance>,
        scene_points: Vec<[f32; 2]>,
        layer_sizes: Vec<usize>,
        slides: Vec<SlideState>,
    ) -> Self {
        let (layer_ranges, layer_visible) = if layer_sizes.is_empty() {
            (vec![(0, sdf_instances.len())], vec![true])
        } else {
            let mut ranges = Vec::with_capacity(layer_sizes.len());
            let mut start = 0usize;
            for &count in &layer_sizes {
                ranges.push((start, count));
                start += count;
            }
            let visible = vec![true; layer_sizes.len()];
            (ranges, visible)
        };

        Self {
            width,
            height,
            sdf_instances,
            scene_points,
            layer_ranges,
            layer_visible,
            slides,
            current_slide: 0,
            renderer: None,
            viewport: Viewport::new(),
            input_queue: InputQueue::new(),
            interaction: None,
            last_cursor_pos: (0.0, 0.0),
            window: None,
            scene: Scene::new(),
            error: None,
            press_pos: None,
            needs_redraw: true,
            should_exit: false,
        }
    }

    pub(crate) fn take_error(&mut self) -> Result<(), String> {
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// Returns instances grouped by shape_type, filtered to only visible layers.
    fn visible_instances(&self) -> Vec<(SdfShape, Vec<SdfInstance>)> {
        let mut groups: HashMap<u32, Vec<SdfInstance>> = HashMap::new();
        for ((start, count), visible) in self.layer_ranges.iter().zip(self.layer_visible.iter()) {
            if !visible {
                continue;
            }
            for inst in &self.sdf_instances[*start..*start + *count] {
                groups.entry(inst.shape_type).or_default().push(*inst);
            }
        }
        let mut result = Vec::new();
        for (shape_type, instances) in groups {
            let sdf_shape = match shape_type {
                0 => SdfShape::Circle,
                1 => SdfShape::RoundedRect,
                2 => SdfShape::Ring,
                3 => SdfShape::Diamond,
                4 => SdfShape::LineCap,
                _ => SdfShape::Circle,
            };
            result.push((sdf_shape, instances));
        }
        result
    }

    /// Collect scene_points for currently visible layers.
    fn visible_scene_points(&self) -> Vec<[f32; 2]> {
        let mut pts = Vec::new();
        for ((start, count), visible) in self.layer_ranges.iter().zip(self.layer_visible.iter()) {
            if *visible {
                pts.extend_from_slice(&self.scene_points[*start..*start + *count]);
            }
        }
        pts
    }

    /// Apply a slide: update layer visibility, viewport, window title, and request redraw.
    fn apply_slide(&mut self, slide_index: usize) {
        if slide_index >= self.slides.len() {
            return;
        }
        self.current_slide = slide_index;
        let slide = &self.slides[slide_index];

        // Update layer visibility
        if slide.visible_layers.is_empty() {
            // All layers visible
            for v in self.layer_visible.iter_mut() {
                *v = true;
            }
        } else {
            for v in self.layer_visible.iter_mut() {
                *v = false;
            }
            for &idx in &slide.visible_layers {
                if idx < self.layer_visible.len() {
                    self.layer_visible[idx] = true;
                }
            }
        }

        // Update viewport
        match slide.zoom_mode {
            ZoomMode::Fit => {
                let visible_pts = self.visible_scene_points();
                let w = self.width as f32;
                let h = self.height as f32;
                if let Some(bbox) = bounding_box(&HashMap::new(), &visible_pts) {
                    let (pan_x, pan_y, zoom) = fit_viewport(bbox, w, h, 0.1);
                    self.viewport.set_pan(pan_x, pan_y);
                    let _ = self.viewport.set_zoom(zoom);
                }
            }
            ZoomMode::Explicit => {
                self.viewport.set_pan(slide.pan_x, slide.pan_y);
                let _ = self.viewport.set_zoom(slide.zoom);
            }
        }

        // Update window title
        let total = self.slides.len();
        let title = if slide.title.is_empty() {
            format!("Slide {}/{}", slide_index + 1, total)
        } else {
            format!("Slide {}/{} — {}", slide_index + 1, total, slide.title)
        };
        if let Some(w) = self.window.as_ref() {
            w.set_title(&title);
            w.request_redraw();
        }
    }
}

impl ApplicationHandler for StoryboardApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("Storyboard")
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

        self.renderer = Some(renderer);
        self.window = Some(window);

        // Compute initial viewport (fit all) for InteractionProcessor stored viewport
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

        // Apply the first slide
        self.apply_slide(0);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.should_exit = true;
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

                let groups = self.visible_instances();

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

                if let Some(_w) = self.window.as_ref() {
                    self.needs_redraw = false;
                }
            }
            other => {
                // --- Keyboard navigation (intercept before forwarding) ---
                if let WindowEvent::KeyboardInput { ref event, .. } = other {
                    if event.state == winit::event::ElementState::Pressed {
                        if let winit::keyboard::PhysicalKey::Code(key_code) = event.physical_key {
                            let total = self.slides.len();
                            match key_code {
                                // Slide navigation: Right / N → next
                                KeyCode::ArrowRight | KeyCode::KeyN => {
                                    let next = (self.current_slide + 1).min(total - 1);
                                    self.apply_slide(next);
                                    return;
                                }
                                // Slide navigation: Left / P → previous
                                KeyCode::ArrowLeft | KeyCode::KeyP => {
                                    let prev = self.current_slide.saturating_sub(1);
                                    self.apply_slide(prev);
                                    return;
                                }
                                // Digit 1-9: jump to slide
                                KeyCode::Digit1 => { if total >= 1 { self.apply_slide(0); } return; }
                                KeyCode::Digit2 => { if total >= 2 { self.apply_slide(1); } return; }
                                KeyCode::Digit3 => { if total >= 3 { self.apply_slide(2); } return; }
                                KeyCode::Digit4 => { if total >= 4 { self.apply_slide(3); } return; }
                                KeyCode::Digit5 => { if total >= 5 { self.apply_slide(4); } return; }
                                KeyCode::Digit6 => { if total >= 6 { self.apply_slide(5); } return; }
                                KeyCode::Digit7 => { if total >= 7 { self.apply_slide(6); } return; }
                                KeyCode::Digit8 => { if total >= 8 { self.apply_slide(7); } return; }
                                KeyCode::Digit9 => { if total >= 9 { self.apply_slide(8); } return; }
                                // Digit 0: reset viewport to fit-all
                                KeyCode::Digit0 => {
                                    let all_pts = &self.scene_points;
                                    let w = self.width as f32;
                                    let h = self.height as f32;
                                    if let Some(bbox) = bounding_box(&HashMap::new(), all_pts) {
                                        let (pan_x, pan_y, zoom) = fit_viewport(bbox, w, h, 0.1);
                                        self.viewport.set_pan(pan_x, pan_y);
                                        let _ = self.viewport.set_zoom(zoom);
                                    }
                                    if let Some(w) = self.window.as_ref() {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                                // F: fit viewport to visible layers' bounding box
                                KeyCode::KeyF => {
                                    let visible_pts = self.visible_scene_points();
                                    let w = self.width as f32;
                                    let h = self.height as f32;
                                    if let Some(bbox) = bounding_box(&HashMap::new(), &visible_pts) {
                                        let (pan_x, pan_y, zoom) = fit_viewport(bbox, w, h, 0.1);
                                        self.viewport.set_pan(pan_x, pan_y);
                                        let _ = self.viewport.set_zoom(zoom);
                                    }
                                    if let Some(w) = self.window.as_ref() {
                                        w.request_redraw();
                                    }
                                    return;
                                }
                                // F11, +/-, scroll: forward to InteractionProcessor (fall through)
                                KeyCode::F11
                                | KeyCode::Equal | KeyCode::NumpadAdd
                                | KeyCode::Minus | KeyCode::NumpadSubtract => {
                                    // Fall through to forward to InteractionProcessor
                                }
                                // Alt+Enter: forward to InteractionProcessor
                                KeyCode::Enter => {
                                    // Fall through — InteractionProcessor handles Alt+Enter
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // --- Click-to-advance detection ---
                if let WindowEvent::MouseInput { state, button, .. } = &other {
                    if *button == winit::event::MouseButton::Left {
                        match state {
                            winit::event::ElementState::Pressed => {
                                self.press_pos = Some(self.last_cursor_pos);
                            }
                            winit::event::ElementState::Released => {
                                if let Some((px, py)) = self.press_pos.take() {
                                    let (rx, ry) = self.last_cursor_pos;
                                    let dx = rx - px;
                                    let dy = ry - py;
                                    let dist = (dx * dx + dy * dy).sqrt();
                                    if dist < 5.0 {
                                        // Click → advance slide
                                        let total = self.slides.len();
                                        let next = (self.current_slide + 1).min(total - 1);
                                        self.apply_slide(next);
                                        return;
                                    }
                                    // Otherwise it was a drag — fall through to InteractionProcessor
                                }
                            }
                        }
                    }
                }

                // Forward remaining events to InteractionProcessor
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
                        self.needs_redraw = true;
                    }
                }
            }
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if self.needs_redraw {
            if let Some(w) = self.window.as_ref() {
                w.request_redraw();
            }
        }
    }
}
