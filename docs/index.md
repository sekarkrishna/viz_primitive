# viz_primitive

GPU-accelerated 2D data rendering ecosystem.

## What is this?

**viz_primitive** is a layered stack of packages for rendering data natively on the GPU.
It is designed to be modular, constrained, and composable — each layer doing exactly one thing.

```
dr2d  (Rust)          Rendering primitive — vertices, viewport, parquet, GPU
  └── dr2d-python     Python binding via PyO3
        └── justviz   Charts, scenes, animations, streaming, Jupyter
```

The core principle: **dr2d renders triangles. It does not interpret them.**
Meaning belongs to the layer above.

## Packages

=== "dr2d (Rust)"

    GPU-accelerated 2D renderer built on [wgpu](https://wgpu.rs).
    Handles SDF shapes, instanced rendering, viewport transforms,
    Parquet/Arrow data loading, and interactive input.

    - **License**: Apache 2.0
    - **Registry**: [crates.io/crates/dr2d](https://crates.io/crates/dr2d)
    - **Status**: v0.0.1-alpha.1

    ```toml
    [dependencies]
    dr2d = "0.0.1-alpha.1"
    ```

=== "justviz (Python)"

    High-level Python chart library. Builds numpy instance arrays,
    passes them to dr2d for rendering. Charts, scenes, storyboards,
    animations, streaming, Jupyter.

    - **License**: MIT
    - **Registry**: [pypi.org/project/justviz](https://pypi.org/project/justviz)
    - **Status**: v0.1.0a1 (WIP)

    ```bash
    pip install justviz
    ```

=== "dr2d-python (Phase 1)"

    Python binding for dr2d via PyO3 + maturin. Exposes the Rust
    rendering API directly to Python. Not yet released.

    - **License**: MIT
    - **Registry**: PyPI as `dr2d` (Phase 1)

## Design Goals

- **Native, offline, local** — GPU rendering on your machine. No browser, no cloud.
- **Data-first** — Parquet/Arrow as the input format. No parsing, no heuristics.
- **Constrained** — 2D as the truth. Depth is a signal, not a coordinate.
- **Modular** — Core stays small. Everything else is above.

See [dr2d Philosophy](dr2d/philosophy.md) for the full design rationale.
