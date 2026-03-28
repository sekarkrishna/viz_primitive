// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! dr2d-cli — temporary binary for testing the dr2d core library.
//! This will be replaced by justviz for end-user usage.

mod scene_loader;

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use clap::Parser;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

use dr2d::{InputQueue, Renderer, Viewport};

use crate::scene_loader::{apply_viewpoint, build_sdf_instances, load_scene, SceneConfig};

#[derive(Parser)]
#[command(name = "dr2d-cli", about = "dr2d test runner")]
struct Cli {
    /// Path to a scene.toml file
    scene_file: PathBuf,

    /// Watch the scene file for changes and hot-reload
    #[arg(long)]
    watch: bool,
}

/// Application state for the winit event loop.
struct App {
    cli: Cli,
    renderer: Option<Renderer>,
    viewport: Viewport,
    input_queue: InputQueue,
    scene_config: Option<SceneConfig>,
    last_cursor_pos: (f32, f32),
    file_rx: Option<mpsc::Receiver<()>>,
    _watcher: Option<notify::RecommendedWatcher>,
    window: Option<std::sync::Arc<Window>>,
}

impl App {
    fn new(cli: Cli) -> Self {
        let mut viewport = Viewport::new();

        let scene_config = match load_scene(&cli.scene_file) {
            Ok(cfg) => {
                log::info!(
                    "Loaded scene: {} shapes, {} data_sources, {} viewpoints",
                    cfg.shapes.len(),
                    cfg.data_sources.len(),
                    cfg.viewpoints.len()
                );
                apply_viewpoint(&cfg, &mut viewport);
                Some(cfg)
            }
            Err(e) => {
                log::error!("Failed to load scene: {e}");
                None
            }
        };

        let (file_rx, _watcher) = if cli.watch {
            setup_watcher(&cli.scene_file)
        } else {
            (None, None)
        };

        Self {
            cli,
            renderer: None,
            viewport,
            input_queue: InputQueue::new(),
            scene_config,
            last_cursor_pos: (0.0, 0.0),
            file_rx,
            _watcher,
            window: None,
        }
    }

    fn reload_scene(&mut self) {
        match load_scene(&self.cli.scene_file) {
            Ok(cfg) => {
                log::info!("Reloaded scene: {} shapes", cfg.shapes.len());
                apply_viewpoint(&cfg, &mut self.viewport);
                self.scene_config = Some(cfg);
            }
            Err(e) => {
                log::error!("Failed to reload scene: {e}");
            }
        }
    }

    fn render(&mut self) {
        let renderer = match self.renderer.as_mut() {
            Some(r) => r,
            None => return,
        };
        let config = match self.scene_config.as_ref() {
            Some(c) => c,
            None => return,
        };
        let groups = match build_sdf_instances(config) {
            Ok(g) => g,
            Err(e) => {
                log::error!("Failed to build instances: {e}");
                return;
            }
        };
        match renderer.begin_frame(&self.viewport) {
            Ok(mut frame) => {
                for (shape, instances) in &groups {
                    frame.draw_sdf(*shape, instances);
                }
                frame.finish();
            }
            Err(e) => {
                log::error!("Render error: {e}");
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = WindowAttributes::default().with_title("dr2d-cli");
        let window = std::sync::Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Failed to create window"),
        );
        let renderer = pollster::block_on(Renderer::new(window.clone()))
            .expect("Failed to create renderer");
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
                if let Some(rx) = &self.file_rx {
                    if rx.try_recv().is_ok() {
                        self.reload_scene();
                    }
                }
                self.render();
                if self.cli.watch {
                    if let Some(w) = self.window.as_ref() {
                        w.request_redraw();
                    }
                }
            }
            other => {
                if let Some(renderer) = self.renderer.as_ref() {
                    let (w, h) = renderer.window_size();
                    if let Some(input_event) = dr2d::input::convert_window_event(
                        &other,
                        &self.viewport,
                        w as f32,
                        h as f32,
                        &mut self.last_cursor_pos,
                    ) {
                        self.input_queue.push(input_event);
                    }
                }
            }
        }
    }
}

fn setup_watcher(
    path: &PathBuf,
) -> (Option<mpsc::Receiver<()>>, Option<notify::RecommendedWatcher>) {
    use notify::{Event, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel();

    let mut watcher = match notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() {
                let _ = tx.send(());
            }
        }
    }) {
        Ok(w) => w,
        Err(e) => {
            log::error!("Failed to create file watcher: {e}");
            return (None, None);
        }
    };

    let watch_dir = path.parent().unwrap_or(Path::new("."));
    if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
        log::error!("Failed to watch directory: {e}");
        return (None, None);
    }

    log::info!("Watching {:?} for changes", path);
    (Some(rx), Some(watcher))
}

fn main() {
    env_logger::init();
    let cli = Cli::parse();

    if !cli.scene_file.exists() {
        eprintln!("Error: scene file not found: {:?}", cli.scene_file);
        std::process::exit(1);
    }

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    let mut app = App::new(cli);
    event_loop.run_app(&mut app).expect("Event loop failed");
}
