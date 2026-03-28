# justviz

Charts, scenes, storyboards, and animations powered by [dr2d](../dr2d/index.md).

justviz is a pure Python library. It builds instance arrays using numpy and passes
them to dr2d for GPU rendering. The chart library is what you interact with;
dr2d handles the pixels.

## Install

```bash
pip install justviz
```

## Quick Start

```python
import justviz as jv
import numpy as np

x = np.random.randn(10_000)
y = np.random.randn(10_000)

jv.scatter(x, y)
```

## Planned Features

=== "Charts"
    - Scatter plot
    - Bar chart
    - Line chart with axes, ticks, labels
    - Color themes
    - Multi-layer support
    - Auto-fit data ranges

=== "Scenes"
    - Declarative TOML scene format
    - Shape composition and layering
    - Hot-reload on file change
    - Clickable navigation areas

=== "Storyboard & Animation"
    - Sequenced views with transitions
    - Viewport interpolation
    - Opacity fades and data transitions
    - Presentation mode

=== "Streaming"
    - Append-only data
    - Sliding window
    - WebSocket and polling adapters
    - Live data updates

=== "Jupyter"
    - Inline rendering
    - Interactive widget
    - PNG export

## Roadmap

See the [Roadmap](../roadmap.md) for the full phase-by-phase plan.

## License

MIT — Copyright 2026 Krishnamoorthy Sankaran
