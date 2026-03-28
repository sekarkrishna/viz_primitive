# viz_primitive

GPU-accelerated 2D data rendering ecosystem.

## Packages

| Package | Language | Description | Registry |
|---------|----------|-------------|----------|
| [dr2d](dr2d-rust/) | Rust | 2D rendering primitive on wgpu | [![crates.io](https://img.shields.io/crates/v/dr2d)](https://crates.io/crates/dr2d) |
| [dr2d-python](dr2d-python/) | Python | Python binding for dr2d | PyPI (Phase 1) |
| [justviz](justviz/) | Python | Charts, scenes, storyboards | [![PyPI](https://img.shields.io/pypi/v/justviz)](https://pypi.org/project/justviz) |

## Architecture

```
dr2d  (Rust, Apache 2.0)     GPU rendering primitive — vertices, viewport, parquet, SDF shapes
  └── dr2d-python  (MIT)     Python binding via PyO3
        └── justviz  (MIT)   Charts, scenes, animations, streaming, Jupyter
```

dr2d renders triangles. It does not interpret them. Meaning belongs to the layer above.

## Documentation

[git.sekrad.org/viz_primitive](https://git.sekrad.org/viz_primitive)

## Roadmap

See [ROADMAP.md](ROADMAP.md).

## License

- [`dr2d-rust/`](dr2d-rust/LICENSE) — Apache 2.0
- [`dr2d-python/`](dr2d-python/) — MIT (Phase 1)
- [`justviz/`](justviz/LICENSE) — MIT
