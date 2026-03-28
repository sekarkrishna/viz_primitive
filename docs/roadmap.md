# Roadmap

Full roadmap is maintained in [`ROADMAP.md`](https://github.com/sekarkrishna/viz_primitive/blob/main/ROADMAP.md) in the repository.

## Phase Summary

| Phase | Name | Status |
|-------|------|--------|
| 0 | Core refactoring | ✅ Complete |
| 0.5 | Dependency optimization | Planned |
| 1 | dr2d Python binding (PyO3) | Planned |
| 2 | justviz charts | In progress |
| 3 | justviz scenes + navigation | Planned |
| 4 | justviz storyboard + animation | Planned |
| 5 | justviz streaming | Planned |
| 6 | justviz-jupyter | Planned |

## Phase 0 — Complete

Core refactoring of the dr2d Rust crate:

- Workspace restructuring (`dr2d` lib + `dr2d-cli` binary)
- SDF rendering pipeline (Circle, RoundedRect, Ring, Diamond, LineCap)
- Tessellation pipeline (Lyon)
- Viewport: pan, zoom, window↔scene coordinate transforms
- Scene: shapes, layers, named viewpoints
- Data: ParquetLoader, CoordinateMapper
- Text rendering (`text` feature) — GlyphAtlas, ASCII
- Headless rendering (`headless` feature) — offscreen RGBA
- 14 tests passing, clippy clean, `cargo publish --dry-run` succeeds

## Phase 0.5 — Dependency Optimization

Feature-gate heavy dependencies to reduce the minimal build:

- `data` feature — gates `arrow` + `parquet` (~45 transitive deps)
- `tessellation` feature — gates `lyon` (~7 deps)
- Goal: reduce ~213 transitive deps to ~55 for minimal `default = []` builds

## Phase 1 — dr2d Python Binding

Create `dr2d-python/` crate using PyO3 + maturin:

- Expose `Renderer`, `Viewport`, `draw_sdf`, `draw_instanced` to Python
- Publish to PyPI as `dr2d` (Apache 2.0 → MIT wrapper)
- Tag `dr2d-py-v*` → CI builds wheels + `maturin publish`

## Phase 2 — justviz Charts

- Scatter, bar, line charts
- Axes: ticks, labels, grid lines
- Legends and color themes
- Multi-layer support
- Auto-fit data ranges

## Phase 3 — justviz Scenes

- Declarative TOML scene format
- Shape composition and layering
- Hot-reload on file change (notify)
- Clickable navigation areas, next/prev, view selection

## Phase 4 — justviz Storyboard + Animation

- Sequenced frames with transitions
- Viewport interpolation (pan/zoom easing)
- Opacity fades, data transitions
- Presentation mode

## Phase 5 — justviz Streaming

- Append-only data model
- Sliding window view
- WebSocket and polling adapters
- Live update loop

## Phase 6 — justviz-jupyter

- ipywidgets integration
- Inline rendering in Jupyter notebooks
- Interactive pan/zoom widget
- PNG export
