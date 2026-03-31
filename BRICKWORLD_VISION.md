# Brickworld — A New Visual Language for Data

## The Idea

Everything is a brick. No bars, no lines, no pie charts. Every data point is a physical object — a brick — arranged in space. Statistical concepts become spatial relationships you can see and touch, not abstract summaries you have to remember.

This is not a chart library. It is a complete reimagining of how data is visualized.

## Why

Traditional visualization compresses data into abstract shapes. A bar chart takes 1000 values and shows you one rectangle. A histogram bins your data and throws away the individuals. A box plot reduces an entire distribution to 5 numbers. You have to learn what these abstractions mean before you can read them.

Brickworld doesn't compress. Every value is visible. Statistical properties emerge from the physical arrangement of bricks, not from learned conventions. You don't need to know what "median" means — you can see it's the brick in the middle of the pile.

## The Primitive

A brick is a small rounded rectangle (SDF RoundedRect in dr2d). A few supporting shapes exist:
- Solid brick: a data point
- Hollow/ghost brick: a computed statistic (mean, median, predicted value)
- Rail: a movable vertical line you can drag to slice the data
- Connector: a thin line linking related bricks across views
- Ground: a baseline surface bricks sit on

## What Brickworld Replaces

### Distribution (replaces histogram, box plot, violin)
- Each value is a brick stacked at its position on the number line
- Value 3 appears twice? Two bricks stacked at x=3
- Mean: a ghost brick floating at the mean position
- Median: the middle brick highlighted
- Mode: the tallest stack glows
- Quartiles: the ground changes color at Q1, Q2, Q3
- Drag the rail to see "what fraction of data is below this point"

### Correlation (replaces scatter plot)
- Two number lines, one horizontal, one vertical
- Each data point is a brick placed at its (x, y) position
- Strong correlation: bricks form a diagonal wall
- No correlation: bricks scattered randomly
- Regression line: a row of ghost bricks showing the predicted path

### Comparison (replaces grouped bar chart)
- Two grounds side by side, each with its own brick stacks
- Same scale, same brick size — differences are immediately spatial
- The taller pile is taller. No axis labels needed.

### Time series (replaces line chart)
- Bricks laid out left to right, one per time step
- Height of each brick = value at that time
- Trend: the brick tops form a slope you can see
- Seasonality: repeating patterns in brick heights
- Anomaly: one brick sticking way above or below its neighbors

### Proportion (replaces pie chart)
- A row of 100 bricks. Each category gets a color.
- 30% = 30 colored bricks. You can count them.
- No angles to estimate. No "is this 23% or 27%?"

### Graph / Network (replaces node-edge diagram)
- Nodes are brick towers — height encodes importance/degree
- Edges are brick bridges connecting towers
- Communities: towers on the same ground platform
- Hub: the tallest tower with the most bridges

## Interactivity

Brickworld is inherently interactive. The physical metaphor demands it.

- Pan/zoom: navigate the brick landscape
- Rail: drag to slice data at any point
- Hover: highlight a brick, see its value
- Click: select a brick, see it highlighted across all views
- Storyboard: guided tour through the brick world ("here's the raw data → here's where the mean sits → here's the outlier")
- Gravity: toggle physics simulation — bricks fall and stack naturally

## Architecture

```
dr2d (Rust)                    GPU rendering primitive (shared)
  └── dr2d-python (PyO3)      Python binding (shared)
        ├── justviz            Traditional charts (scatter, bar, graph, storyboard)
        └── brickworld         The new visual language (everything is a brick)
```

Brickworld depends on dr2d-python for rendering. It does NOT depend on justviz. They are siblings, not parent-child.

Brickworld reuses from dr2d:
- SDF RoundedRect for bricks
- SDF Circle for markers
- HeadlessRenderer for static images
- show_sdf_window / subprocess launcher for interactive windows
- Viewport, pan/zoom, interaction
- Storyboard infrastructure

Brickworld adds its own:
- Brick layout engine (stacking, spacing, ground computation)
- Statistical brick placement (mean/median/mode as ghost bricks)
- Rail interaction (drag to slice)
- Physics simulation (optional gravity/stacking)
- Multi-view linking (click a brick in one view, highlight in another)

## API Sketch

```python
import brickworld as bw

# Distribution
bw.distribution(data, show=["mean", "median", "mode"], rail=True)

# Correlation
bw.correlation(df, x="price", y="volume")

# Comparison
bw.compare({"Group A": data_a, "Group B": data_b})

# Time series
bw.timeline(dates, values)

# Proportion
bw.proportion({"Cat": 30, "Dog": 45, "Bird": 25})

# Graph
bw.network(G)

# Storyboard through a dataset
bw.explore(df)  # auto-generates brick views for each column + relationships
```

## Target Audience

- Students learning statistics for the first time
- Teachers who want to show, not tell, what mean/median/mode are
- Data scientists who want to see every data point, not just summaries
- Presenters who want audiences to understand data without statistical training

## Language Support

- Python (via dr2d-python) — first
- R (via dr2d-r, future) — second
- Web (via dr2d-wasm, future) — for educational platforms

## Status

Vision document. Not yet started. justviz is the proving ground for the rendering stack. Once justviz is stable, brickworld development begins.

## Inspiration

The question: "Why do we make people learn what a bar chart means before they can understand their data? What if the visualization was so intuitive that a child could read it?"

The answer: make every data point a physical thing. Let statistical properties emerge from spatial arrangement. Don't abstract — show.
