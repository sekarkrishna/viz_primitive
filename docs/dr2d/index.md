# dr2d

GPU-accelerated 2D data renderer built in Rust on [wgpu](https://wgpu.rs).

dr2d is a rendering primitive. It handles vertices, viewports, data, and GPU.
Meaning belongs to the layer above — dr2d renders triangles, it does not interpret them.

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

## Add to Your Project

```toml
[dependencies]
dr2d = "0.0.1-alpha.1"

# Optional features
# dr2d = { version = "0.0.1-alpha.1", features = ["text", "headless"] }
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `text` | off | GlyphAtlas with embedded ASCII glyph outlines, tessellated via Lyon |
| `headless` | off | HeadlessRenderer for offscreen rendering to RGBA pixel buffers |

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

## License

Apache 2.0 — Copyright 2026 Krishnamoorthy Sankaran
