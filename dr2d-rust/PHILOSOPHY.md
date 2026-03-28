# dr2d Philosophy

## 1. 2D is the ground truth

A scene on a monitor is fundamentally 2D, delivered frame by frame. Even in real
life, the retina receives flat images in succession; the brain constructs a sense
of 3D using memory, motion, and context.

dr2d takes inspiration from this view of the world. It keeps 2D as the truth and
expresses depth or motion through signals, not through full 3D calculation. Where
depth is needed, dr2d uses trigonometry, layering, perspective hints, and
geometric cues — the same techniques classic 2D engines used to suggest space
long before 3D pipelines existed.

This keeps the foundation simple, predictable, and aligned with how perception
actually works.

## 2. Constraints create clarity

More capability does not automatically produce better outcomes. Some of the most
memorable digital experiences were created under tight constraints. dr2d explores
this space deliberately: a constrained primitive supporting rich content and
storytelling.

The guiding constraints of dr2d are:

- 2D as the primitive
- Simple math — trigonometry for perceived depth rather than full linear-algebra 3D
- Explicit transforms
- No heavy scene graphs
- No deep hierarchies

The goal is not to avoid capability, but to use the right tool for the right job.
You don't cut a flower with an axe, and you don't need a full 3D engine to draw
a road that converges in the distance.

A 2D engine can achieve a large portion of what people expect from 3D, while
remaining simpler, more accessible, and more efficient.

## 3. The primitive is intentionally ignorant

This is part of the constraint: dr2d defines what it is by clearly defining what
it is not.

dr2d renders triangles. It does not interpret them.

A triangle might represent:

- a data point
- a tile
- a character
- a building
- a highlight
- a label

Meaning belongs to the layer above. This keeps the core stable, modular, and free
from opinionated design choices that would limit what can be built on top.

## 4. Data is first-class

dr2d treats data as structured, typed, columnar input. Parquet on disk, Arrow in
memory.

No guessing. No parsing. No heuristics.

The renderer expects arrays — clean, contiguous, numeric. This keeps the boundary
between "data world" and "GPU world" explicit, efficient, and predictable.

## 5. Native, offline, and local

Rendering happens on your machine, through native GPU APIs. No browser, no cloud,
no external dependencies.

The pipeline is deterministic, inspectable, and self-contained. Your data stays
with you.

## 6. Depth is a signal, not a coordinate

dr2d expresses depth the way the visual system does: as layering, shading,
perspective hints, motion, and trigonometric distortion applied to a flat image.

These cues are enough to construct a sense of space. Heavy computation is not
required. A world can be represented without adopting the full machinery of 3D
projection or unconstrained coordinate spaces.

This approach keeps the engine simple while still allowing expressive, spatially
rich visuals when needed.

## 7. Simplicity enables modularity

dr2d stays small so that anything can be built on top of it later. Features that
belong outside the core stay outside the core.

The engine remains:

- pure
- minimal
- predictable
- stable

Text rendering may come. Depth cues may come. But the core remains a 2D
rendering primitive, not a growing universe of abstractions.
