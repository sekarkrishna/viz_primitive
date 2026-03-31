"""Scatter chart — builds 10-column SDF instance arrays for dr2d circles.

Public API:
    scatter(x_or_data, y=None, *, x_col, y_col, color, size, opacity,
            width, height, layers, interactive) -> np.ndarray | None

Helper (testable without GPU):
    _build_scatter_instances(x, y, color, size, opacity, mapper) -> np.ndarray
"""

from __future__ import annotations

import time
import numpy as np
import dr2d

from justviz._renderer import get_renderer
from justviz._input import resolve_input
from justviz._window import launch_window


# ── validation helpers ──────────────────────────────────────────────

def _validate_color(color: tuple[float, float, float]) -> None:
    """Raise ValueError if *color* is not an RGB tuple of 3 floats in [0, 1]."""
    if (
        not isinstance(color, (tuple, list))
        or len(color) != 3
        or not all(isinstance(c, (int, float)) for c in color)
        or not all(0.0 <= c <= 1.0 for c in color)
    ):
        raise ValueError("color must be an RGB tuple of 3 floats in [0, 1]")


def _validate_opacity(opacity: float) -> None:
    """Raise ValueError if *opacity* is not in [0, 1]."""
    if not (0.0 <= opacity <= 1.0):
        raise ValueError("opacity must be in [0, 1]")


# ── instance builder (GPU-free) ────────────────────────────────────

def _build_scatter_instances(
    x: np.ndarray,
    y: np.ndarray,
    color: tuple[float, float, float],
    size: float,
    opacity: float,
    mapper: dr2d.CoordinateMapper,
) -> np.ndarray:
    """Build an Nx10 float32 instance array for scatter circles.

    Layout per row:
        [position_x, position_y, size_x, size_y, r, g, b, a, shape_type, param]

    * shape_type = 0 (Circle)
    * param = 0.0
    * size_x = size_y = *size*

    Parameters
    ----------
    x, y : float32 numpy arrays (same length, already validated).
    color : RGB tuple with values in [0, 1].
    size : circle radius in pixels.
    opacity : alpha value in [0, 1].
    mapper : a ``dr2d.CoordinateMapper`` used to project data → scene coords.

    Returns
    -------
    np.ndarray of shape (N, 10), dtype float32.
    """
    scene_x, scene_y = mapper.map_points(x, y)

    n = len(x)
    instances = np.zeros((n, 10), dtype=np.float32)
    instances[:, 0] = scene_x          # position_x
    instances[:, 1] = scene_y          # position_y
    instances[:, 2] = size             # size_x
    instances[:, 3] = size             # size_y
    instances[:, 4] = color[0]         # r
    instances[:, 5] = color[1]         # g
    instances[:, 6] = color[2]         # b
    instances[:, 7] = opacity          # a
    instances[:, 8] = 0.0              # shape_type = Circle
    instances[:, 9] = 0.0              # param (unused for circles)
    return instances


# ── public API ──────────────────────────────────────────────────────

def _padded_mapper(
    x_min: float, x_max: float,
    y_min: float, y_max: float,
    width: float, height: float,
    padding: float = 0.05,
) -> dr2d.CoordinateMapper:
    """Create a CoordinateMapper with padding so data doesn't touch the edges."""
    x_range = x_max - x_min if x_max != x_min else 1.0
    y_range = y_max - y_min if y_max != y_min else 1.0
    pad_x = x_range * padding
    pad_y = y_range * padding
    return dr2d.CoordinateMapper(
        x_min - pad_x, x_max + pad_x,
        y_min - pad_y, y_max + pad_y,
        width, height,
    )


def scatter(
    x_or_data,
    y=None,
    *,
    x_col: str | None = None,
    y_col: str | None = None,
    color: tuple[float, float, float] | None = None,
    size: float = 4.0,
    opacity: float = 1.0,
    width: int = 800,
    height: int = 600,
    padding: float = 0.05,
    layers: list[dict] | None = None,
    interactive: bool = False,
) -> np.ndarray | None:
    """Render a scatter chart to an RGBA numpy array.

    Parameters
    ----------
    x_or_data : array-like, DataFrame, or str
        Either x-coordinates (array-like), a pandas/polars DataFrame, or a
        parquet file path.  When a DataFrame or path, use *x_col* and *y_col*
        to name the columns.
    y : array-like, optional
        y-coordinates when *x_or_data* is array-like.  Ignored for DataFrame
        or parquet path inputs.
    x_col, y_col : str, optional
        Column names to extract when *x_or_data* is a DataFrame or parquet path.
    color : tuple of 3 floats in [0, 1], optional
        RGB colour applied to every point.  Default ``(1.0, 0.4, 0.6)``.
    size : float
        Circle radius in pixels.  Default ``4.0``.
    opacity : float
        Alpha value in [0, 1].  Default ``1.0``.
    width, height : int
        Output image dimensions in pixels.
    layers : list of dicts, optional
        Multi-layer mode.  Each dict must contain ``"x"`` and ``"y"`` keys
        and may contain ``"color"``, ``"size"``, ``"opacity"``.
    interactive : bool
        If True, open an interactive window instead of returning pixel data.

    Returns
    -------
    np.ndarray or None
        Shape ``(height, width, 4)``, dtype ``uint8`` — RGBA pixel data.
        Returns None when *interactive* is True.
    """
    if color is None:
        color = (1.0, 0.4, 0.6)

    # ── single-layer mode ───────────────────────────────────────────
    if layers is None:
        x, y = resolve_input(x_or_data, y, x=x_col, y=y_col)

        if len(x) != len(y):
            raise ValueError("x and y must have the same length")
        if len(x) == 0:
            raise ValueError("x and y must not be empty")

        _validate_color(color)
        _validate_opacity(opacity)

        mapper = _padded_mapper(
            float(x.min()), float(x.max()),
            float(y.min()), float(y.max()),
            float(width), float(height), padding,
        )

        instances = _build_scatter_instances(x, y, color, size, opacity, mapper)

        if interactive:
            launch_window(instances, width, height, "justviz — scatter")
            return None

        t0 = time.perf_counter()
        renderer = get_renderer()
        result = renderer.render_sdf_to_numpy(instances, width, height)
        dt = time.perf_counter() - t0
        print(f"scatter: {len(x):,} points, {width}×{height}, render {dt*1000:.1f}ms")
        return result

    # ── multi-layer mode ───────────────────────────────────────────
    # 1. Validate layers and collect all x/y to compute global min/max
    all_x = []
    all_y = []
    parsed_layers = []

    for i, layer in enumerate(layers):
        if "x" not in layer or "y" not in layer:
            raise ValueError(f"Layer {i} must contain 'x' and 'y' keys")

        lx = np.asarray(layer["x"], dtype=np.float32)
        ly = np.asarray(layer["y"], dtype=np.float32)

        if len(lx) != len(ly):
            raise ValueError(f"Layer {i}: x and y must have the same length")
        if len(lx) == 0:
            raise ValueError(f"Layer {i}: x and y must not be empty")

        lcolor = layer.get("color", color)
        lsize = layer.get("size", size)
        lopacity = layer.get("opacity", opacity)

        _validate_color(lcolor)
        _validate_opacity(lopacity)

        all_x.append(lx)
        all_y.append(ly)
        parsed_layers.append((lx, ly, lcolor, lsize, lopacity))

    # 2. Compute global min/max across all layers
    concat_x = np.concatenate(all_x)
    concat_y = np.concatenate(all_y)

    mapper = _padded_mapper(
        float(concat_x.min()), float(concat_x.max()),
        float(concat_y.min()), float(concat_y.max()),
        float(width), float(height), padding,
    )

    # 3. Build instance arrays per layer and concatenate
    instance_arrays = []
    for lx, ly, lcolor, lsize, lopacity in parsed_layers:
        instance_arrays.append(
            _build_scatter_instances(lx, ly, lcolor, lsize, lopacity, mapper)
        )

    instances = np.concatenate(instance_arrays, axis=0)

    # 4. Render in one pass
    if interactive:
        layer_sizes = [len(lx) for lx, *_ in parsed_layers]
        launch_window(instances, width, height, "justviz — scatter", layer_sizes=layer_sizes)
        return None

    total_points = sum(len(lx) for lx, *_ in parsed_layers)
    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(f"scatter: {total_points:,} points ({len(parsed_layers)} layers), {width}×{height}, render {dt*1000:.1f}ms")
    return result
