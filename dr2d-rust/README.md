# dr2d - 2d Data Renderer

GPU-accelerated 2D data renderer built in Rust on wgpu.

dr2d is a rendering primitive. It handles vertices, viewports, data, and GPU.
Meaning belongs to the layer above — dr2d renders triangles, it does not
interpret them.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                      dr2d                           │
│                                                     │
│  ┌───────────┐   ┌───────────┐   ┌──────────────┐  │
│  │  Renderer  │   │  Viewport  │   │  InputQueue  │  │
│  │  ├ SDF     │   │  pan/zoom  │   │  events      │  │
│  │  ├ Tess    │   │  transform │   │  drain()     │  │
│  │  └ Frame   │   └───────────┘   └──────────────┘  │
│  └───────────┘                                      │
│                                                     │
│  ┌───────────┐   ┌───────────────────────────────┐  │
│  │   Scene    │   │           Data                │  │
│  │  shapes    │   │  ParquetLoader  CoordMapper   │  │
│  │  viewpoints│   └───────────────────────────────┘  │
│  └───────────┘                                      │
│                                                     │
│  ┌─────────────────┐   ┌──────────────────────┐    │
│  │  Text (feature)  │   │  Headless (feature)  │    │
│  │  GlyphAtlas      │   │  HeadlessRenderer    │    │
│  └─────────────────┘   └──────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

### Rendering Pipeline

```
begin_frame(viewport)
    │
    ├── draw_sdf(shape, instances)      ← SDF pipeline (resolution-independent)
    ├── draw_instanced(mesh, instances) ← shared mesh + per-instance transforms
    ├── draw_triangles(vertices)        ← flat triangle list
    │
    └── finish()                        ← submit + present
```

The renderer maintains two GPU pipelines:

- SDF pipeline — expands a unit quad per instance, evaluates signed distance
  functions in the fragment shader for anti-aliased, resolution-independent shapes
- Tessellation pipeline — renders pre-tessellated triangle meshes with optional
  instancing, used for polygons, rectangles, and text glyphs

Both pipelines share a single uniform bind group containing the viewport
transform matrix.

## Features

- SDF rendering — resolution-independent shapes (circle, rounded rect, ring, diamond, line cap) with anti-aliased edges
- Instanced rendering — `draw_sdf`, `draw_instanced`, `draw_triangles`
- GPU rendering via wgpu (Vulkan, Metal, DX12, WebGPU)
- Parquet/Arrow data loading with streaming row-group reads
- Viewport with pan, zoom, fit-to-data, window-to-scene coordinate conversion
- Mouse and keyboard input with scene coordinate mapping
- Lyon-based polygon tessellation with caching
- Text rendering (feature: `text`) — embedded glyph atlas, ASCII
- Headless rendering (feature: `headless`) — render to RGBA pixel buffer

## Quick Start

```rust
use dr2d::{Renderer, Viewport, SdfShape, SdfInstance};

let renderer = Renderer::new(window).await?;
let viewport = Viewport::new();

let mut frame = renderer.begin_frame(&viewport)?;
frame.draw_sdf(SdfShape::Circle, &[SdfInstance {
    position: [100.0, 100.0],
    size: [20.0, 20.0],
    color: [1.0, 0.0, 0.0, 1.0],
    shape_type: SdfShape::Circle as u32,
    param: 0.0,
    _pad: [0.0; 2],
}]);
frame.finish();
```

## API Reference

### Renderer

Creates and manages the GPU context, pipelines, and frame lifecycle.

```rust
// Create renderer attached to a winit window
let renderer = Renderer::new(window).await?;

// Resize the rendering surface
renderer.resize(width, height);

// Frame lifecycle
let mut frame = renderer.begin_frame(&viewport)?;
// ... draw calls ...
frame.finish();
// or: renderer.end_frame(frame)?;

// Convenience: render all scene shapes in one call
renderer.render(&mut scene, &viewport)?;

// Query window dimensions
let (w, h) = renderer.window_size();
```

### FrameEncoder

Returned by `begin_frame()`. Records draw calls into a GPU command encoder.

| Method | Description |
|--------|-------------|
| `draw_sdf(shape, &[SdfInstance])` | Draw SDF shapes (resolution-independent, anti-aliased) |
| `draw_instanced(&[Vertex], &[InstanceData])` | Draw shared mesh with per-instance transforms |
| `draw_triangles(&[Vertex])` | Draw a flat triangle list |
| `finish()` | Submit commands and present the frame |

All draw calls skip silently when given empty slices.

### SDF Shapes

Built-in signed distance function shapes evaluated per-fragment on the GPU:

| Variant | Description | `param` usage |
|---------|-------------|---------------|
| `Circle` | Unit circle | unused |
| `RoundedRect` | Rounded rectangle | corner radius |
| `Ring` | Donut / annulus | ring thickness |
| `Diamond` | Rotated square | unused |
| `LineCap` | Capsule / line segment with round ends | unused |

### SdfInstance

Per-instance data for SDF rendering (48 bytes, GPU-aligned):

| Field | Type | Description |
|-------|------|-------------|
| `position` | `[f32; 2]` | Center in scene coordinates |
| `size` | `[f32; 2]` | Half-extents of bounding quad |
| `color` | `[f32; 4]` | RGBA color |
| `shape_type` | `u32` | Cast from `SdfShape` enum |
| `param` | `f32` | Shape-specific parameter |

### Vertex / InstanceData

| Type | Fields | Description |
|------|--------|-------------|
| `Vertex` | `position: [f32; 2]`, `color: [f32; 4]` | GPU vertex for triangle rendering |
| `InstanceData` | `position: [f32; 2]`, `size: [f32; 2]`, `color: [f32; 4]` | Per-instance offset, scale, color |

### Viewport

2D translation (pan) and uniform scale (zoom). Transforms scene coordinates to
NDC for GPU rendering.

```rust
let mut vp = Viewport::new(); // pan=(0,0), zoom=1.0

vp.set_pan(100.0, -50.0);
vp.set_zoom(2.0)?; // returns Err if zoom <= 0

// Build 3×vec4 transform matrix for GPU uniform buffer
let matrix: [f32; 12] = vp.transform_matrix(window_width, window_height);

// Convert window pixel coordinates to scene coordinates
let (scene_x, scene_y) = vp.window_to_scene(px_x, px_y, win_w, win_h);
```

Fields: `pan_x: f32`, `pan_y: f32`, `zoom: f32` (all public).

### InputQueue / InputEvent

Buffers input events between frames. Events carry both screen and scene
coordinates.

```rust
let mut queue = InputQueue::new();
queue.push(event);
let events: Vec<InputEvent> = queue.drain();
```

`InputEvent` variants:

| Variant | Key fields |
|---------|------------|
| `MouseButton` | `button`, `state`, `screen_x/y`, `scene_x/y` |
| `MouseMove` | `screen_x/y`, `scene_x/y` |
| `KeyboardKey` | `key: KeyCode`, `state: ElementState` |
| `Scroll` | `delta_x`, `delta_y` |
| `ModifiersChanged` | `alt`, `ctrl`, `shift`, `super_key` |

Use `convert_window_event()` to translate winit `WindowEvent` into `InputEvent`.

### Scene

Container for shapes and named viewpoints. Shapes are sorted by layer for
draw ordering.

```rust
let mut scene = Scene::new();

let id = scene.add_shape(shape)?;
scene.update_shape(id, new_shape)?;
scene.remove_shape(id)?;
let shape_ref = scene.get_shape(id);
let sorted: &[ShapeId] = scene.shapes_sorted();
```

Shape geometry types:

| Geometry | Fields |
|----------|--------|
| `Rectangle` | `x`, `y`, `width`, `height` |
| `Polygon` | `vertices: Vec<[f32; 2]>` (3–1024 vertices) |
| `Triangle` | `vertices: [[f32; 2]; 3]` |

Shape properties: `color: [f32; 3]`, `opacity: f32`, `layer: i32`,
`border_color: Option<[f32; 3]>`, `border_width: f32`.

Viewpoints:

```rust
scene.register_viewpoint("overview".into(), viewpoint);
let vp = scene.activate_viewpoint("overview")?;
scene.remove_viewpoint("overview");
```

### Data Module

#### ParquetLoader

Reads Parquet files into Arrow RecordBatch or extracts typed column pairs.
Supports streaming row-group reads. Numeric columns (f32, f64, i32, i64) are
cast to f32 automatically.

```rust
// Stream columns directly from file
let pair = ParquetLoader::load_columns(path, "x", "y")?;

// Load full RecordBatch, then extract
let batch = ParquetLoader::load(path)?;
let pair = ParquetLoader::extract_columns(&batch, "x", "y")?;
```

`ColumnPair` contains `x: Vec<f32>` and `y: Vec<f32>`.

#### CoordinateMapper

Linear mapping from data value ranges to scene coordinate ranges.

```rust
let mapper = CoordinateMapper::from_column_pairs(&[&pair1, &pair2]);

let (scene_x, scene_y) = mapper.map_point(data_x, data_y);
let points: Vec<[f32; 2]> = mapper.map_all(&pair);
```

Default scene range: `0.0..1000.0` on both axes.

### Text (feature: `text`)

Embedded glyph atlas with geometric outlines for ASCII characters (A-Z, a-z,
0-9, common punctuation). Tessellated into triangle vertices via lyon.

```rust
let atlas = GlyphAtlas::new();
let vertices: Vec<Vertex> = atlas.tessellate_string("Hello", x, y, font_size);
```

Unknown characters are skipped but the cursor still advances.

### Headless (feature: `headless`)

Render to an in-memory RGBA pixel buffer without a window. Creates a wgpu
device without a surface.

```rust
let mut hr = HeadlessRenderer::new().await?;
let pixels: Vec<u8> = hr.render_to_image(800, 600).await?;
// pixels.len() == 800 * 600 * 4
```

Returns `HeadlessError::InvalidDimensions` if width or height is zero.
PNG encoding is left to downstream consumers.

## Keyboard Shortcuts

Built-in interaction processor translates input events into viewport mutations.

| Key | Action |
|-----|--------|
| Arrow keys | Pan viewport |
| Scroll wheel | Zoom in/out |
| `+` / `-` | Zoom in / zoom out |
| Left click + drag | Pan viewport |
| `F` | Fit viewport to data |
| `Ctrl+Shift+H` | Fit viewport to data |
| `Home` | Reset to origin (pan=0, zoom=1.0) |
| `0` | Reset to initial viewport state |
| `1`–`9` | Activate named viewpoints (sorted alphabetically) |
| `F11` | Toggle fullscreen |
| `Alt+Enter` | Toggle fullscreen |
| `Ctrl+Super+F` | Toggle fullscreen |
| `Escape` | Exit fullscreen |
| `Ctrl+S` | Save request (polled via `take_save_request()`) |

## Using dr2d Downstream

Add dr2d as a dependency in your `Cargo.toml`:

```toml
[dependencies]
dr2d = "0.0.1-alpha.1"

# Optional features
# dr2d = { version = "0.0.1-alpha.1", features = ["text", "headless"] }
```

Minimal windowed application:

```rust
use std::sync::Arc;
use dr2d::{Renderer, Viewport, Scene, SdfShape, SdfInstance};

async fn run(window: Arc<winit::window::Window>) {
    let mut renderer = Renderer::new(window).await.unwrap();
    let viewport = Viewport::new();

    // Draw a red circle
    let mut frame = renderer.begin_frame(&viewport).unwrap();
    frame.draw_sdf(SdfShape::Circle, &[SdfInstance {
        position: [0.0, 0.0],
        size: [50.0, 50.0],
        color: [1.0, 0.0, 0.0, 1.0],
        shape_type: SdfShape::Circle as u32,
        param: 0.0,
        _pad: [0.0; 2],
    }]);
    frame.finish();
}
```

For scene-based rendering with shapes and tessellation:

```rust
use dr2d::{Renderer, Viewport, Scene};
use dr2d::scene::shape::{Shape, ShapeGeometry};

let mut scene = Scene::new();
scene.add_shape(Shape {
    geometry: ShapeGeometry::Rectangle { x: 10.0, y: 10.0, width: 200.0, height: 100.0 },
    color: [0.2, 0.4, 0.8],
    opacity: 1.0,
    layer: 0,
    border_color: None,
    border_width: 0.0,
}).unwrap();

renderer.render(&mut scene, &viewport).unwrap();
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `text` | no | GlyphAtlas with embedded ASCII glyph outlines and lyon tessellation |
| `headless` | no | HeadlessRenderer for offscreen rendering to RGBA pixel buffers |

## Examples

```bash
cargo run --example scatter_sdf -p dr2d
cargo run --example custom_shapes -p dr2d
cargo run --example headless_export -p dr2d --features headless
```

## CLI

The `dr2d-cli` crate loads TOML scene files and renders them in a window.

```bash
cargo run -p dr2d-cli -- scene.toml
cargo run -p dr2d-cli -- scene.toml --watch   # hot-reload on file change
```

## Philosophy

See [PHILOSOPHY.md](PHILOSOPHY.md).

## License

Apache 2.0 — Copyright 2026 Krishnamoorthy Sankaran. See [LICENSE](LICENSE).

All source files carry the following header convention:

```
// SPDX-License-Identifier: Apache-2.0
```
