# viz_primitive

GPU-accelerated 2D data visualization ecosystem.

Three packages. One renders triangles, one wraps it for Python, one makes charts.

```
dr2d  (Rust, Apache 2.0)     GPU rendering primitive — SDF shapes, viewport, parquet
  └── dr2d-python  (MIT)     Python binding via PyO3
        └── justviz  (MIT)   Charts, storyboards, interactive exploration
```

## Install

```bash
pip install dr2d justviz
```

## Quick Start

```python
import justviz as jv
import numpy as np

# Scatter — 1M points, GPU-rendered
x = np.random.uniform(0, 100, 1_000_000).astype(np.float32)
y = np.random.uniform(0, 100, 1_000_000).astype(np.float32)
img = jv.scatter(x, y, size=1.5)

# From a DataFrame
jv.scatter(df, x_col="price", y_col="volume")

# From a parquet file (Rust reader, fast)
jv.scatter("data.parquet", x_col="price", y_col="volume")

# Interactive window — pan, zoom, fit
jv.scatter(x, y, interactive=True)
```

## Multi-Layer Scatter with Layer Toggle

```python
jv.scatter(
    None, None,
    layers=[
        {"x": x1, "y": y1, "color": (1.0, 0.3, 0.3), "size": 1.5},
        {"x": x2, "y": y2, "color": (0.3, 0.3, 1.0), "size": 1.5},
    ],
    interactive=True,  # press 1/2 to toggle layers
)
```

## Bar Chart

```python
jv.bar(df, x_col="category", y_col="count", color=(0.4, 0.6, 1.0))
```

## Performance

| Points | Render Time |
|--------|------------|
| 10K | 5ms |
| 100K | 28ms |
| 1M | 213ms |
| 5M | 1.4s |

Every point is a real SDF shape — anti-aliased, resolution-independent, zoomable.

## Interactive Window Controls

- Mouse drag: pan
- Scroll wheel: zoom
- F: fit all data
- +/-: zoom in/out
- Home: reset to origin
- 0: reset to initial view
- F11: fullscreen
- 1-9: toggle layers (or jump to slides in storyboard mode)

## Packages

| Package | Language | Description | Registry |
|---------|----------|-------------|----------|
| [dr2d](dr2d-rust/) | Rust | GPU 2D rendering primitive on wgpu | [crates.io](https://crates.io/crates/dr2d) |
| [dr2d-python](dr2d-python/) | Rust/Python | Python binding via PyO3 | [PyPI](https://pypi.org/project/dr2d/) |
| [justviz](justviz/) | Python | Charts, storyboards, exploration | [PyPI](https://pypi.org/project/justviz/) |

## Roadmap

See [justviz/ROADMAP.md](justviz/ROADMAP.md) for the full plan including storyboard presentations, cluster exploration, graph visualization, and more.

## License

- dr2d-rust: Apache 2.0
- dr2d-python: MIT
- justviz: MIT
