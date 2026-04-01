"""Data explorer — general-purpose scatter with column-mapped visual channels.

Public API:
    explore(x_or_data, y=None, *, x, y, color, size, facet,
            opacity, width, height, padding, interactive) -> np.ndarray | None

Helpers (testable without GPU):
    _resolve_explore_input(...)  -> (x, y, source_data, available_columns)
    _resolve_color_column(...)   -> (N, 3) float32
    _resolve_size_column(...)    -> (N,) float32
    _build_explore_instances(...) -> (N, 10) float32
    _build_facet_layers(...)     -> (list[ndarray], list[str])
"""

from __future__ import annotations

import numpy as np

from justviz._input import resolve_input
from justviz.charts.clusters import CLUSTER_PALETTE


# ── input resolution ────────────────────────────────────────────────

_INSTANCE_BUDGET = 10_000_000


def _resolve_explore_input(
    x_or_data,
    y_pos=None,
    *,
    x: str | None = None,
    y: str | None = None,
) -> tuple[np.ndarray, np.ndarray, object, list[str]]:
    """Resolve polymorphic input into (x_arr, y_arr, source_data, available_columns).

    Delegates to resolve_input() for x/y extraction.
    Returns source_data (DataFrame or None) for column lookups by color/size/facet.
    Returns available_columns for error messages.
    """
    # Detect DataFrame (duck-typed via 'columns' attribute)
    if hasattr(x_or_data, "columns"):
        source_data = x_or_data
        available_columns = list(x_or_data.columns)
    else:
        source_data = None
        available_columns = []

    x_arr, y_arr = resolve_input(x_or_data, y_pos, x=x, y=y)

    if len(x_arr) != len(y_arr):
        raise ValueError(
            f"x and y must have the same length (got {len(x_arr)} and {len(y_arr)})"
        )
    if len(x_arr) == 0:
        raise ValueError("x and y must not be empty")

    return x_arr, y_arr, source_data, available_columns


# ── categorical color mapping ───────────────────────────────────────

def _resolve_color_column(
    source_data,
    color_arg: str | tuple | None,
    n_points: int,
    available_columns: list[str],
) -> np.ndarray:
    """Resolve color argument into an (N, 3) float32 RGB array.

    - str column name → categorical mapping via CLUSTER_PALETTE (cycling)
    - RGB tuple → broadcast to all points
    - None → default (0.3, 0.6, 1.0)
    """
    if color_arg is None:
        return np.full((n_points, 3), (0.3, 0.6, 1.0), dtype=np.float32)

    if isinstance(color_arg, str):
        if source_data is None:
            raise KeyError(
                f"Column '{color_arg}' cannot be resolved — input is not a DataFrame"
            )
        if color_arg not in available_columns:
            raise KeyError(
                f"Column '{color_arg}' not found in DataFrame. "
                f"Available: {available_columns}"
            )
        col = np.asarray(source_data[color_arg])
        unique_vals = sorted(set(col.tolist()))
        val_to_color = {
            v: CLUSTER_PALETTE[i % len(CLUSTER_PALETTE)]
            for i, v in enumerate(unique_vals)
        }
        colors = np.zeros((n_points, 3), dtype=np.float32)
        for i, val in enumerate(col):
            colors[i] = val_to_color[val]
        return colors

    # RGB tuple
    if (
        isinstance(color_arg, (tuple, list))
        and len(color_arg) == 3
        and all(isinstance(c, (int, float)) for c in color_arg)
        and all(0.0 <= c <= 1.0 for c in color_arg)
    ):
        return np.full((n_points, 3), color_arg, dtype=np.float32)

    raise ValueError("color must be a column name (str) or an RGB tuple of 3 floats in [0, 1]")


# ── numeric size mapping ────────────────────────────────────────────

def _resolve_size_column(
    source_data,
    size_arg: str | float | None,
    n_points: int,
    available_columns: list[str],
) -> np.ndarray:
    """Resolve size argument into an (N,) float32 array of radii.

    - str column name → linear interpolation [2.0, 12.0], uniform 7.0 if constant
    - float scalar → broadcast to all points
    - None → default 4.0
    """
    if size_arg is None:
        return np.full(n_points, 4.0, dtype=np.float32)

    if isinstance(size_arg, (int, float)) and not isinstance(size_arg, str):
        return np.full(n_points, float(size_arg), dtype=np.float32)

    if isinstance(size_arg, str):
        if source_data is None:
            raise KeyError(
                f"Column '{size_arg}' cannot be resolved — input is not a DataFrame"
            )
        if size_arg not in available_columns:
            raise KeyError(
                f"Column '{size_arg}' not found in DataFrame. "
                f"Available: {available_columns}"
            )
        col = np.asarray(source_data[size_arg], dtype=np.float32)
        vmin, vmax = float(col.min()), float(col.max())
        if vmin == vmax:
            return np.full(n_points, 7.0, dtype=np.float32)
        # Linear interpolation: [2.0, 12.0]
        return 2.0 + (col - vmin) / (vmax - vmin) * 10.0

    raise ValueError("size must be a column name (str) or a numeric scalar")


# ── instance builders ───────────────────────────────────────────────

def _build_explore_instances(
    x: np.ndarray,
    y: np.ndarray,
    colors: np.ndarray,
    sizes: np.ndarray,
    opacity: float,
    mapper,
) -> np.ndarray:
    """Build Nx10 float32 instance array with per-point color and size.

    Layout: [pos_x, pos_y, size_x, size_y, r, g, b, a, shape_type, param]
    shape_type=0 (Circle), param=0.0.
    """
    scene_x, scene_y = mapper.map_points(x, y)
    n = len(x)
    instances = np.zeros((n, 10), dtype=np.float32)
    instances[:, 0] = scene_x
    instances[:, 1] = scene_y
    instances[:, 2] = sizes
    instances[:, 3] = sizes
    instances[:, 4] = colors[:, 0]
    instances[:, 5] = colors[:, 1]
    instances[:, 6] = colors[:, 2]
    instances[:, 7] = opacity
    instances[:, 8] = 0.0   # Circle
    instances[:, 9] = 0.0
    return instances


def _build_facet_layers(
    x: np.ndarray,
    y: np.ndarray,
    colors: np.ndarray,
    sizes: np.ndarray,
    opacity: float,
    facet_values: np.ndarray,
    mapper,
) -> tuple[list[np.ndarray], list[str]]:
    """Build one Nx10 instance array per unique facet value.

    Returns (instance_arrays, facet_names) where facet_names[i] is the
    string label for layer i.
    """
    unique_vals = sorted(set(facet_values.tolist()))
    instance_arrays: list[np.ndarray] = []
    facet_names: list[str] = []

    for val in unique_vals:
        mask = facet_values == val
        arr = _build_explore_instances(
            x[mask], y[mask], colors[mask], sizes[mask], opacity, mapper,
        )
        instance_arrays.append(arr)
        facet_names.append(str(val))

    return instance_arrays, facet_names


# ── public API ──────────────────────────────────────────────────────

def explore(
    x_or_data,
    y_pos=None,
    *,
    x: str | None = None,
    y: str | None = None,
    color: str | tuple[float, float, float] | None = None,
    size: str | float | None = None,
    facet: str | None = None,
    opacity: float = 1.0,
    width: int = 1024,
    height: int = 768,
    padding: float = 0.05,
    interactive: bool = False,
) -> np.ndarray | None:
    """General-purpose data explorer.

    Parameters
    ----------
    x_or_data : array-like, DataFrame, or str
        Data source — array-like x-coordinates, a DataFrame, or a parquet path.
    y_pos : array-like, optional
        y-coordinates when *x_or_data* is array-like.
    x, y : str, optional
        Column names for DataFrame / parquet inputs.
    color : str or RGB tuple, optional
        Column name for categorical coloring, or an RGB tuple for uniform color.
        Default ``(0.3, 0.6, 1.0)``.
    size : str or float, optional
        Column name for numeric size mapping, or a scalar radius.
        Default ``4.0``.
    facet : str, optional
        Column name for faceted storyboard (one slide per unique value).
    opacity : float
        Alpha value in [0, 1]. Default ``1.0``.
    width, height : int
        Output dimensions in pixels.
    padding : float
        Padding fraction around the data bounding box.
    interactive : bool
        If True, open an interactive window instead of returning pixel data.

    Returns
    -------
    np.ndarray or None
        RGBA pixel data (height, width, 4) uint8, or None if interactive.
    """
    import time

    from justviz.charts.scatter import _padded_mapper
    from justviz._renderer import get_renderer
    from justviz._window import launch_window, launch_storyboard_window

    if not (0.0 <= opacity <= 1.0):
        raise ValueError("opacity must be in [0, 1]")

    # ── resolve input ───────────────────────────────────────────
    x_arr, y_arr, source_data, available_columns = _resolve_explore_input(
        x_or_data, y_pos, x=x, y=y,
    )
    n = len(x_arr)

    # ── budget check ────────────────────────────────────────────
    if n > _INSTANCE_BUDGET:
        raise ValueError(
            f"explore: {n:,} points exceeds the {_INSTANCE_BUDGET:,} instance "
            f"limit. Reduce the dataset size or sample before plotting."
        )

    # ── resolve color and size ──────────────────────────────────
    colors = _resolve_color_column(source_data, color, n, available_columns)
    sizes = _resolve_size_column(source_data, size, n, available_columns)

    # ── resolve facet column ────────────────────────────────────
    facet_values = None
    if facet is not None:
        if source_data is None:
            raise KeyError(
                f"Column '{facet}' cannot be resolved — input is not a DataFrame"
            )
        if facet not in available_columns:
            raise KeyError(
                f"Column '{facet}' not found in DataFrame. "
                f"Available: {available_columns}"
            )
        facet_values = np.asarray(source_data[facet])

    # ── global mapper ───────────────────────────────────────────
    mapper = _padded_mapper(
        float(x_arr.min()), float(x_arr.max()),
        float(y_arr.min()), float(y_arr.max()),
        float(width), float(height), padding,
    )

    # ── faceted mode ────────────────────────────────────────────
    if facet_values is not None:
        layer_arrays, facet_names = _build_facet_layers(
            x_arr, y_arr, colors, sizes, opacity, facet_values, mapper,
        )
        layer_sizes = [len(a) for a in layer_arrays]
        instances = np.concatenate(layer_arrays, axis=0)

        # Build storyboard slides: "All" + one per facet value
        slides = [{"title": "All", "zoom": "fit", "layers": "all"}]
        for i, name in enumerate(facet_names):
            slides.append({
                "title": name,
                "zoom": "fit",
                "layers": [i + 1],  # 1-based
            })

        if interactive:
            launch_storyboard_window(instances, width, height, layer_sizes, slides)
            return None

        # Headless: render all layers combined
        t0 = time.perf_counter()
        renderer = get_renderer()
        result = renderer.render_sdf_to_numpy(instances, width, height)
        dt = time.perf_counter() - t0
        print(f"explore: {n:,} points ({len(facet_names)} facets), {width}×{height}, render {dt*1000:.1f}ms")
        return result

    # ── single-layer mode ───────────────────────────────────────
    instances = _build_explore_instances(
        x_arr, y_arr, colors, sizes, opacity, mapper,
    )

    if interactive:
        launch_window(instances, width, height, "justviz — explore")
        return None

    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(f"explore: {n:,} points, {width}×{height}, render {dt*1000:.1f}ms")
    return result
