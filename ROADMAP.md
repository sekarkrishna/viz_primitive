# dr2d + justviz Roadmap

## Vision

Two projects. One core, one showcase.

- **dr2d** — A GPU-accelerated 2D data renderer built in Rust on wgpu. Pure rendering primitive. Knows about vertices, viewports, data, and GPU. Does not know what a "chart" or "scene" is. Apache 2.0.
- **justviz** — A Python library (with Rust via PyO3 where needed) that demonstrates what dr2d can do. Charts, scenes, storyboards, animations, interactive navigation. MIT.

```
dr2d (Apache 2.0)
├── Rust library (crates.io: dr2d)
├── Python binding (PyPI: dr2d) — PyO3/maturin, same name on both registries
│
├── Core capabilities
│   ├── GPU rendering (wgpu, WGSL shaders)
│   ├── SDF rendering (signed distance functions for perfect shapes)
│   ├── Data loading (Parquet → Arrow)
│   ├── Coordinate mapping (data range → scene coords)
│   ├── Viewport (pan, zoom, fit, transform matrix)
│   ├── Input (mouse, keyboard, scroll, hit testing)
│   ├── Instanced geometry (draw N instances from arrays)
│   ├── Shape primitives (rect, circle, triangle, polygon, line)
│   ├── Tessellation (lyon, shape → triangles, fallback for arbitrary shapes)
│   ├── Text rendering (SVG glyph atlas, English, tessellated)
│   ├── Headless rendering (render to PNG buffer)
│   └── Dirty-flag optimization (re-render only on change)
│
├── Future language bindings (Apache 2.0)
│   ├── dr2d-r (extendr → CRAN)
│   └── dr2d-wasm (wasm-bindgen → npm, needs WebGPU)
│
justviz (MIT, Python, PyPI)
│
├── Charts (scatter, bar, line, axes, legends, color themes)
├── Scenes (shapes, layers, composition, TOML declarative format)
├── Storyboard (sequenced views, transitions, presentation mode)
├── Navigation (clickable areas, next/prev, view selection)
├── Animation (viewport interpolation, opacity fades, data transitions)
├── Hot-reload (file watcher, live TOML editing)
├── Streaming data (append, sliding window, live updates)
└── Jupyter integration (inline rendering, widget)
```

---

## Phase 0: Refactor Current Code → dr2d Core ✅ COMPLETE

**Goal**: Extract the pure rendering primitive from the current viz-primitive codebase. Remove all chart-specific and scene-specific code from core. Upgrade rendering quality.

**Status**: All tasks complete. 14 tests pass, clippy clean, docs build, `cargo publish --dry-run` succeeds. Version `0.0.1-alpha.1`.

### 0.1 Restructure into Cargo workspace ✅
- Workspace root with `dr2d/` (library crate) and `dr2d-cli/` (binary crate)
- Core modules: scene, viewport, input, interaction, renderer (gpu, pipeline, vertex, tessellation), data (parquet_loader, coord_mapper)
- Removed from core: scatter.rs, axes.rs, serialization.rs (TOML), file_watcher.rs, app.rs

### 0.2 SDF rendering pipeline ✅
- SDF-based rendering for circle, rounded rect, ring, diamond, line cap
- Fragment shader with `smoothstep` + `fwidth` for anti-aliased, resolution-independent shapes
- Lyon tessellation retained as fallback for arbitrary polygons

### 0.3 Clean up the core API surface ✅
- `#![warn(missing_docs)]` with crate-level doc comments
- Public API: `draw_sdf`, `draw_instanced`, `draw_triangles`, `Viewport`, `InputQueue`, `Scene`

### 0.4 Add text rendering to core (feature flag) ✅
- `features = ["text"]` — GlyphAtlas with embedded ASCII glyph outlines, lyon tessellation

### 0.5 Add headless rendering to core (feature flag) ✅
- `features = ["headless"]` — HeadlessRenderer for offscreen rendering to RGBA pixel buffer

### 0.6 Prepare for crates.io publish ✅
- Cargo.toml metadata: authors, license, repository, readme, keywords, categories, rust-version
- LICENSE file (Apache 2.0), README.md, PHILOSOPHY.md
- `cargo clippy --all-features -- -D warnings` clean
- `cargo test --all-features` — 14 tests pass
- `cargo publish --dry-run` succeeds

### 0.7 Publish dr2d 0.0.1-alpha.1 to crates.io
- **Status**: Ready to publish. All prerequisites met.

---

## Phase 0.5: Dependency Optimization — Core vs Optional

**Goal**: Reduce the mandatory dependency footprint by feature-gating heavy deps that not every consumer needs. Ship as `0.0.1-alpha.2` or `0.0.2-alpha.1`.

### Dependency Analysis (current state)

Total: 213 transitive deps, ~36s full release build.

**Always core (non-negotiable):**

| Dep | Transitive deps | Size | Rationale |
|-----|-----------------|------|-----------|
| `wgpu` + `winit` | ~160 | ~36MB rlib | The GPU backend. No wgpu, no dr2d. |
| `bytemuck` | 1 | tiny | Zero-cost vertex/instance casting. Fundamental. |
| `thiserror` | 1 | tiny | Error types throughout the crate. |
| `log` | 0 | tiny | Logging facade. No allocations unless subscriber attached. |

**Should become optional (feature-gated):**

| Dep | Feature name | Transitive deps | Size | Effort | Rationale |
|-----|-------------|-----------------|------|--------|-----------|
| `arrow` + `parquet` | `data` | ~45 unique | ~30MB rlib | Low | Cleanly isolated in `data/` module. Many users bring their own data pipeline and feed `Vec<f32>` directly. Biggest win. |
| `serde` | `serde` | 2 | ~1MB | Trivial | Only `#[derive(Serialize, Deserialize)]` on a few structs. Gate with `#[cfg_attr]`. |
| `lyon` | `tessellation` | 7 | ~1MB | Moderate | Wired into `Renderer::render()` for polygon tessellation. Needs `#[cfg]` gates on scene rendering path. Pure SDF users don't need it. |

### Planned feature flag layout

```toml
[features]
default = ["data", "tessellation"]
data = ["dep:arrow", "dep:parquet"]
tessellation = ["dep:lyon"]
serde = ["dep:serde"]
text = []        # already exists
headless = []    # already exists
```

`default` includes `data` + `tessellation` so the out-of-box experience is full-featured. Power users opt out with `default-features = false`.

A minimal SDF-only build (no data, no tessellation, no serde) drops ~55 transitive deps.

### Implementation priority
1. `data` feature — biggest reduction, cleanest module boundary, lowest effort
2. `serde` feature — trivial `cfg_attr` gating
3. `tessellation` feature — moderate refactor, `#[cfg]` gates on `Renderer::render()` path

---

## Phase 1: dr2d Python Binding (PyPI: dr2d)

**Goal**: Make dr2d usable from Python via PyO3. Published to PyPI as `dr2d` (same name as the Rust crate on crates.io — different registries, no collision).

### 1.1 Create Python binding crate
- Add `dr2d-py/` to Cargo workspace (internal crate name, PyPI package name is `dr2d`)
- pyproject.toml with maturin build backend, `name = "dr2d"`
- Cargo.toml with `pyo3 = { features = ["extension-module"] }`
- `crate-type = ["cdylib"]`

### 1.2 Expose core API to Python
- `Renderer` class: create window, render frame, resize
- `Viewport` class: pan, zoom, fit, window_to_scene
- `draw_instanced(mesh_name, numpy_array, params)` — accepts numpy float32 arrays
- `draw_sdf(shape_type, numpy_array, params)` — SDF rendering from Python
- `draw_triangles(numpy_array)` — accepts numpy float32 vertex arrays
- `load_parquet(path)` — returns column data as numpy arrays
- Data exchange via numpy arrays (zero-copy where possible with PyO3)

### 1.3 Event callbacks
- `@renderer.on_click`, `@renderer.on_scroll`, `@renderer.on_key`
- Callbacks receive both window and scene coordinates
- Enable Python-side interactivity (tooltips, selection, navigation)

### 1.4 Headless rendering for Python
- `renderer.render_to_bytes(width, height)` → Python bytes (RGBA)
- `renderer.render_to_numpy(width, height)` → numpy array (H, W, 4)
- Enables Jupyter inline display and image export

### 1.5 Publish dr2d 0.1.0-alpha.1 to PyPI
- `pip install dr2d` — users get the GPU renderer directly
- `import dr2d` in Python

---

## Phase 2: justviz 0.1.0 — Charts

**Goal**: First usable release. Scatter, bar, line charts with axes and legends.

### 2.1 Project setup
- Pure Python package (or Rust+PyO3 where performance matters)
- Depends on `dr2d` (PyPI)
- MIT license

### 2.2 Chart types
- `justviz.scatter(df, x, y, color, size, opacity)` — scatter plot (uses dr2d SDF circles)
- `justviz.bar(df, x, y, color)` — vertical bar chart (uses dr2d SDF rounded rects)
- `justviz.line(df, x, y, color, width)` — line chart (uses dr2d SDF line caps)
- Each chart type generates instance arrays and passes to dr2d
- Multi-layer support: `chart.add_layer(...)` for overlays

### 2.3 Axes and grid
- Axis tick marks with "nice numbers" algorithm
- Grid lines (optional, toggleable)
- Axis labels using dr2d text rendering
- Auto-fit axis ranges from data, or manual override

### 2.4 Legends and titles
- Chart title, axis titles
- Color legend for multi-layer charts
- Rendered using dr2d text + shape primitives

### 2.5 Color themes
- Built-in palettes (categorical, sequential, diverging)
- Theme system for consistent styling
- Dark/light mode support

### 2.6 Publish justviz 0.1.0 to PyPI

---

## Phase 3: justviz 0.2.0 — Scenes and Composition

**Goal**: Declarative scene authoring, shape composition, interactive navigation.

### 3.1 TOML scene format
- Declarative scene files describing shapes, layers, viewpoints, data sources
- `justviz.load("scene.toml")` → renders the scene
- Hot-reload: file watcher detects changes, re-renders live

### 3.2 Shape composition
- Rectangles, circles, triangles, polygons, lines — all from dr2d primitives
- Layered composition with depth ordering
- Grouping and nesting

### 3.3 Interactive navigation
- Clickable areas (hit testing via dr2d input + coordinate conversion)
- Next/previous buttons rendered as shapes with glyph text
- View selection (click "View 1", "View 2" to jump to named viewpoints)
- Navigation bar component

### 3.4 Named viewpoints
- Register named viewpoints (pan, zoom, visible layers)
- Activate by name — viewport snaps to saved state
- Defined in TOML or programmatically

---

## Phase 4: justviz 0.3.0 — Storyboard and Animation

**Goal**: Animated data storytelling from declarative config.

### 4.1 Storyboard engine
- Ordered sequence of frames (each frame = viewpoint + visible layers + annotations)
- Defined in TOML: `[[storyboard.frame]]` sections
- Playback controls: next, previous, pause, auto-advance with duration

### 4.2 Viewport transitions
- Smooth interpolation between viewpoints (lerp on pan_x, pan_y, zoom)
- Easing functions (linear, ease-in-out, ease-out)
- Configurable transition duration

### 4.3 Layer transitions
- Opacity fade in/out for layers appearing/disappearing
- Data highlight regions (dim everything outside a range)

### 4.4 Keyboard and click controls
- Arrow keys: next/previous frame
- Space: pause/play auto-advance
- Click on navigation buttons
- Number keys: jump to frame N

---

## Phase 5: justviz 0.4.0 — Streaming and Live Data

**Goal**: Real-time data visualization.

### 5.1 Append-only streaming
- `chart.push_data(new_rows)` — append new data points
- Partial vertex buffer updates (no full rebuild)
- Auto-expanding or fixed axis ranges

### 5.2 Sliding window
- Fixed-size data window (e.g., last 60 seconds)
- Ring buffer for efficient memory use
- Scrolling axis mode

### 5.3 Data source adapters
- WebSocket adapter
- File polling adapter (watch a parquet file for appends)
- Python generator/iterator adapter

---

## Phase 6: justviz-jupyter — Notebook Integration

**Goal**: Inline chart rendering in Jupyter notebooks.

### 6.1 Jupyter widget
- ipywidgets-based widget
- Renders chart inline using headless rendering → PNG display
- Interactive: pan/zoom via mouse events forwarded to dr2d

### 6.2 Rich display
- `chart.show()` renders inline in notebook cell
- `chart.save("output.png")` exports to file
- HTML repr for static display

---

## Future (No Timeline)

### dr2d-r
- R language binding via extendr
- Same Rust core, R-native API
- CRAN package

### dr2d-wasm
- WebAssembly binding via wasm-bindgen
- Targets WebGPU in browser
- Requires WebGPU browser support to stabilize
- Different windowing model (canvas element)

### Advanced chart types
- Heatmap, histogram, box plot, area chart, contour
- Geographic/map rendering
- Network/graph visualization

### Scientific visualization
- Colormaps (viridis, plasma, etc.)
- Density rendering
- Image/texture rendering (texture quads)

### Additional SDF shapes
- Star, cross, arrow, custom parametric shapes
- SDF boolean operations (union, intersection, subtraction)
- Glow/shadow effects via SDF distance field manipulation

---

## Version Strategy

| Package | Registry | Language | License | Versioning |
|---------|----------|----------|---------|------------|
| dr2d | crates.io | Rust | Apache 2.0 | Independent, slow-moving, stability-focused |
| dr2d | PyPI | Rust+PyO3 | Apache 2.0 | Tracks crates.io dr2d (PyPI dr2d 0.1.x wraps crates.io dr2d 0.1.x) |
| justviz | PyPI | Python | MIT | Independent, fast-moving, feature-focused |
| justviz-jupyter | PyPI | Python | MIT | Tracks justviz |

Same name `dr2d` on both registries — Rust users `cargo add dr2d`, Python users `pip install dr2d`. No collision, different ecosystems.

All packages stay below 1.0 until APIs stabilize. Semver pre-1.0: minor version bumps can break APIs.

---

## Immediate Next Steps

### Step 1: Publish dr2d 0.0.1-alpha.1 to crates.io
- All prerequisites met (Phase 0 complete)
- `cargo publish` from `Viz Primitive/dr2d/dr2d/`

### Step 2: Dependency optimization (Phase 0.5)
- Feature-gate `arrow`+`parquet` behind `data`
- Feature-gate `serde` behind `serde`
- Feature-gate `lyon` behind `tessellation`
- Set `default = ["data", "tessellation"]`
- Publish as next alpha

### Step 3: Phase 1 — dr2d Python binding
- Create `dr2d-py/` crate in workspace with PyO3 + maturin
- Wrap core API: Renderer, Viewport, draw_sdf, draw_instanced
- Publish to PyPI as `dr2d`

### Step 4: Wire justviz → dr2d
- Update justviz to depend on `dr2d` (PyPI)
- Update scatter/bar/line to call `dr2d.draw_sdf(...)` with instance arrays

### Step 5: Publish justviz 0.1.0 to PyPI
