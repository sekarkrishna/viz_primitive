// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Krishnamoorthy Sankaran <krishnamoorthy.sankaran@sekrad.org>

//! wgpu Device/Queue/Surface initialization.

use std::sync::Arc;
use crate::renderer::RendererError;

/// GPU context holding wgpu device, queue, and surface.
pub(crate) struct GpuContext {
    /// The wgpu surface.
    pub surface: wgpu::Surface<'static>,
    /// The wgpu device.
    pub device: wgpu::Device,
    /// The wgpu command queue.
    pub queue: wgpu::Queue,
    /// Surface configuration.
    pub config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    /// Creates a new GPU context for the given window.
    pub(crate) async fn new(window: Arc<winit::window::Window>) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())
            .map_err(|e| RendererError::Gpu(format!("Failed to create surface: {e}")))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| RendererError::Gpu("No suitable GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("dr2d device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            }, None)
            .await
            .map_err(|e| RendererError::Gpu(format!("Failed to create device: {e}")))?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb()).copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self { surface, device, queue, config })
    }

    /// Resizes the surface.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Reconfigures the surface (e.g. after a lost surface).
    pub(crate) fn reconfigure_surface(&self) {
        self.surface.configure(&self.device, &self.config);
    }
}
