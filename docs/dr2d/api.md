# API Reference

## Rendering Pipeline

```
begin_frame(viewport)
    │
    ├── draw_sdf(shape, instances)      ← SDF pipeline (resolution-independent)
    ├── draw_instanced(mesh, instances) ← shared mesh + per-instance transforms
    ├── draw_triangles(vertices)        ← flat triangle list
    │
    └── finish()                        ← submit + present
```

Two GPU pipelines:

- **SDF pipeline** — expands a unit quad per instance, evaluates signed distance functions
  in the fragment shader for anti-aliased, resolution-independent shapes
- **Tessellation pipeline** — renders pre-tessellated triangle meshes with optional instancing

Both share a single uniform bind group containing the viewport transform matrix.

---

## Renderer

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

---

## FrameEncoder

Returned by `begin_frame()`. Records draw calls into a GPU command encoder.

| Method | Description |
|--------|-------------|
| `draw_sdf(shape, &[SdfInstance])` | SDF shapes — resolution-independent, anti-aliased |
| `draw_instanced(&[Vertex], &[InstanceData])` | Shared mesh with per-instance transforms |
| `draw_triangles(&[Vertex])` | Flat triangle list |
| `finish()` | Submit commands and present the frame |

All draw calls skip silently when given empty slices.

---

## SDF Shapes

Built-in signed distance function shapes evaluated per-fragment on the GPU.

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
| `color` | `[f32; 4]` | RGBA |
| `shape_type` | `u32` | Cast from `SdfShape` enum |
| `param` | `f32` | Shape-specific parameter |

---

## Vertex / InstanceData

| Type | Fields | Description |
|------|--------|-------------|
| `Vertex` | `position: [f32; 2]`, `color: [f32; 4]` | GPU vertex for triangle rendering |
| `InstanceData` | `position: [f32; 2]`, `size: [f32; 2]`, `color: [f32; 4]` | Per-instance offset, scale, color |

---

## Viewport

2D translation (pan) and uniform scale (zoom). Transforms scene coordinates to NDC for GPU rendering.

```rust
let mut vp = Viewport::new();  // pan=(0,0), zoom=1.0

vp.set_pan(100.0, -50.0);
vp.set_zoom(2.0)?;  // returns Err if zoom <= 0

// GPU transform matrix (3×vec4)
let matrix: [f32; 12] = vp.transform_matrix(window_width, window_height);

// Coordinate conversion
let (scene_x, scene_y) = vp.window_to_scene(px_x, px_y, win_w, win_h);
```

Fields: `pan_x: f32`, `pan_y: f32`, `zoom: f32` (all public).

---

## InputQueue / InputEvent

Buffers input events between frames.

```rust
let mut queue = InputQueue::new();
queue.push(event);
let events: Vec<InputEvent> = queue.drain();
```

`InputEvent` variants — all carry both screen and scene coordinates:

| Variant | Key fields |
|---------|------------|
| `MouseButton` | `button`, `state`, `screen_x/y`, `scene_x/y` |
| `MouseMove` | `screen_x/y`, `scene_x/y` |
| `KeyboardKey` | `key: KeyCode`, `state: ElementState` |
| `Scroll` | `delta_x`, `delta_y` |
| `ModifiersChanged` | `alt`, `ctrl`, `shift`, `super_key` |

Use `convert_window_event()` to translate winit `WindowEvent` into `InputEvent`.

### Built-in Keyboard Shortcuts

| Key | Action |
|-----|--------|
| Arrow keys | Pan viewport |
| Scroll wheel | Zoom in/out |
| `+` / `-` | Zoom in / zoom out |
| Left click + drag | Pan viewport |
| `F` | Fit viewport to data |
| `Ctrl+Shift+H` | Fit viewport to data |
| `Home` | Reset to origin |
| `0` | Reset to initial viewport |
| `1`–`9` | Activate named viewpoints (sorted alphabetically) |
| `F11` / `Alt+Enter` | Toggle fullscreen |
| `Ctrl+S` | Save request (polled via `take_save_request()`) |

---

## Scene

Container for shapes and named viewpoints. Shapes are sorted by layer for draw ordering.

```rust
let mut scene = Scene::new();

let id = scene.add_shape(shape)?;
scene.update_shape(id, new_shape)?;
scene.remove_shape(id)?;
let shape_ref = scene.get_shape(id);
let sorted: &[ShapeId] = scene.shapes_sorted();
```

### Shape Geometry

| Type | Fields |
|------|--------|
| `Rectangle` | `x`, `y`, `width`, `height` |
| `Polygon` | `vertices: Vec<[f32; 2]>` (3–1024 vertices) |
| `Triangle` | `vertices: [[f32; 2]; 3]` |

Shape properties: `color: [f32; 3]`, `opacity: f32`, `layer: i32`,
`border_color: Option<[f32; 3]>`, `border_width: f32`.

### Viewpoints

```rust
scene.register_viewpoint("overview".into(), viewpoint);
let vp = scene.activate_viewpoint("overview")?;
scene.remove_viewpoint("overview");
```

---

## Data

### ParquetLoader

Reads Parquet files into Arrow RecordBatch or extracts typed column pairs.
Numeric columns (`f32`, `f64`, `i32`, `i64`) are cast to `f32` automatically.

```rust
// Stream columns directly from file
let pair = ParquetLoader::load_columns(path, "x", "y")?;

// Load full RecordBatch, then extract
let batch = ParquetLoader::load(path)?;
let pair = ParquetLoader::extract_columns(&batch, "x", "y")?;
```

`ColumnPair` contains `x: Vec<f32>` and `y: Vec<f32>`.

### CoordinateMapper

Linear mapping from data value ranges to scene coordinate ranges.
Default scene range: `0.0..1000.0` on both axes.

```rust
let mapper = CoordinateMapper::from_column_pairs(&[&pair1, &pair2]);

let (scene_x, scene_y) = mapper.map_point(data_x, data_y);
let points: Vec<[f32; 2]> = mapper.map_all(&pair);
```

---

## Text (feature: `text`)

Embedded glyph atlas with geometric outlines for ASCII characters.
Tessellated into triangle vertices via Lyon.

```rust
let atlas = GlyphAtlas::new();
let vertices: Vec<Vertex> = atlas.tessellate_string("Hello", x, y, font_size);
```

Unknown characters are skipped; the cursor still advances.

---

## Headless (feature: `headless`)

Render to an in-memory RGBA pixel buffer without a window.

```rust
let mut hr = HeadlessRenderer::new().await?;
let pixels: Vec<u8> = hr.render_to_image(800, 600).await?;
// pixels.len() == 800 * 600 * 4
```

Returns `HeadlessError::InvalidDimensions` if width or height is zero.
PNG encoding is left to downstream consumers.
