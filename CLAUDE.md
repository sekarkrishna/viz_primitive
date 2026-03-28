# viz_primitive — Project Context for Claude

This is a monorepo for the **viz_primitive** ecosystem: a GPU-accelerated 2D data
rendering stack. All packages here are part of the same project and evolve together.
Read this file to understand the whole system before touching any individual package.

## Dependency Chain

```
dr2d-rust  (Apache 2.0)   — Rust core, GPU rendering primitive on wgpu
     ↓  PyO3 binding (Phase 1, not yet started)
dr2d-python  (MIT)        — Python binding, exposes dr2d API to Python
     ↓  depends on dr2d-python
justviz  (MIT)            — High-level Python chart + scene library
```

## Package Summary

| Path | Language | Published as | Registry | License | Status |
|------|----------|-------------|----------|---------|--------|
| `dr2d-rust/` | Rust | `dr2d` | crates.io | Apache 2.0 | v0.0.1-alpha.1 |
| `dr2d-python/` | Rust+Python (PyO3) | `dr2d` | PyPI | MIT | Phase 1, not started |
| `justviz/` | Python | `justviz` | PyPI | MIT | v0.1.0a1, WIP |

## What dr2d Does

GPU-accelerated 2D renderer built on wgpu. Backends: Vulkan, Metal, DX12, WebGPU.

Core modules:
- **Renderer / FrameEncoder** — GPU context, two pipelines (SDF + tessellation), frame lifecycle
- **Viewport** — pan/zoom, window↔scene coordinate transforms, GPU matrix
- **Scene** — shape container (Rectangle, Polygon, Triangle), named viewpoints, layer sorting
- **InputQueue** — mouse, keyboard, scroll events with scene coordinate mapping
- **Data** — `ParquetLoader` (Arrow/Parquet → f32 columns), `CoordinateMapper` (data→scene range)
- **Text** (feature: `text`) — GlyphAtlas, ASCII, Lyon tessellation
- **Headless** (feature: `headless`) — offscreen RGBA pixel buffer, no window

It is intentionally ignorant of meaning. It renders triangles. Layers above interpret them.

## What dr2d-python Does (Phase 1)

PyO3 + maturin binding. Exposes `Renderer`, `Viewport`, `draw_sdf`, `draw_instanced`
to Python. Publishes to PyPI as `dr2d`. Uses `maturin publish` (not `uv publish`).

## What justviz Does

Pure Python chart library. Builds numpy arrays of instances, passes to dr2d for GPU rendering.
- Currently implemented: `scatter()` (WIP)
- Planned: bar, line + axes, scenes (TOML), storyboard, animation, streaming, Jupyter widget

## Workspace Layout

```
viz_primitive/
├── dr2d-rust/              Rust workspace (dr2d crate + dr2d-cli binary)
│   ├── Cargo.toml          workspace root
│   ├── dr2d/               library crate (publishes to crates.io)
│   └── dr2d-cli/           binary crate (not published, local scene loader + hot-reload)
├── dr2d-python/            empty placeholder — Phase 1
├── justviz/                Python package (publishes to PyPI)
│   ├── pyproject.toml
│   └── src/justviz/
├── docs/                   MkDocs source
├── .github/workflows/      CI/CD
├── mkdocs.yml
├── pyproject.toml          root — docs tooling only (not published)
└── ROADMAP.md
```

## Phase Status

- **Phase 0** ✅ COMPLETE — core refactoring, workspace, SDF rendering, text, headless
- **Phase 0.5** — feature-gate `arrow`/`parquet` behind `data`, `lyon` behind `tessellation`
- **Phase 1** — dr2d-python PyO3 binding (maturin)
- **Phase 2** — justviz charts (scatter, bar, line, axes, legends)
- **Phase 3** — justviz scenes / navigation / TOML hot-reload
- **Phase 4** — justviz storyboard + animation
- **Phase 5** — justviz streaming (append-only, WebSocket)
- **Phase 6** — justviz-jupyter widget

See `ROADMAP.md` for full detail.

## Publishing

| What | Trigger tag | Command | Secret |
|------|-------------|---------|--------|
| dr2d to crates.io | `dr2d-v*` | `cargo publish -p dr2d` | `CRATES_IO_TOKEN` |
| justviz to PyPI | `justviz-v*` | `uv publish` | `PYPI_TOKEN` |
| dr2d to PyPI | `dr2d-py-v*` | `maturin publish` | `PYPI_TOKEN` |

## Docs

Material for MkDocs. Auto-deployed on push to `main` via `docs.yml`.
Live at: `https://git.sekrad.org/viz_primitive`

## GitHub

- Main repo: `sekarkrishna/viz_primitive` (all development)
- Legacy: `sekarkrishna/dr2d` (Apache 2.0 license only — development moved here)

## Licensing

`dr2d-rust/` is Apache 2.0 (patent protection for a GPU rendering primitive).
All other packages are MIT (maximum adoption for wrappers and chart libs).
Each package carries its own `LICENSE` file. No root-level `LICENSE`.

## Author

Krishnamoorthy Sankaran `<krishnamoorthy.sankaran@sekrad.org>`
