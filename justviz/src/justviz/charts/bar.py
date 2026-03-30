"""Bar chart — builds 10-column SDF instance arrays for dr2d rounded rects.

Public API:
    bar(x, y, *, color, bar_width, opacity, width, height) -> np.ndarray

Helper (testable without GPU):
    _build_bar_instances(x, y, color, bar_width, opacity, mapper, scene_width) -> np.ndarray
"""

from __future__ import annotations

import time
import numpy as np
import dr2d


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


def _validate_bar_width(bar_width: float) -> None:
    """Raise ValueError if *bar_width* is not in (0, 1]."""
    if not (0.0 < bar_width <= 1.0):
        raise ValueError("bar_width must be in (0, 1]")


# ── instance builder (GPU-free) ────────────────────────────────────

def _build_bar_instances(
    x: np.ndarray,
    y: np.ndarray,
    color: tuple[float, float, float],
    bar_width: float,
    opacity: float,
    scene_width: float,
    scene_height: float,
    padding: float,
) -> np.ndarray:
    """Build an Nx10 float32 instance array for bar chart rounded rects.

    The coordinate system has Y=0 at top, Y=scene_height at bottom (GPU ortho).
    Bars grow upward from the bottom baseline.

    Layout per row:
        [position_x, position_y, size_x, size_y, r, g, b, a, shape_type, param]
    """
    n = len(x)

    # X: evenly space bars across the scene width with padding
    x_pad = scene_width * padding
    usable_width = scene_width - 2 * x_pad
    spacing = usable_width / n
    half_w = (spacing * bar_width) / 2.0

    # Bar center X positions: evenly spaced
    bar_centers_x = x_pad + spacing * (np.arange(n, dtype=np.float32) + 0.5)

    # Y: map bar heights proportionally. Baseline is at the bottom.
    y_max = float(y.max()) if y.max() > 0 else 1.0
    y_pad = scene_height * padding
    usable_height = scene_height - 2 * y_pad

    # Bar pixel heights (proportional to y values)
    bar_heights = (y / y_max) * usable_height
    half_h = bar_heights / 2.0

    # Bar center Y: bottom of usable area minus half the bar height
    # (Y=0 is top, so bottom = scene_height - y_pad)
    baseline_y = scene_height - y_pad
    bar_centers_y = baseline_y - half_h

    instances = np.zeros((n, 10), dtype=np.float32)
    instances[:, 0] = bar_centers_x
    instances[:, 1] = bar_centers_y
    instances[:, 2] = half_w
    instances[:, 3] = half_h
    instances[:, 4] = color[0]
    instances[:, 5] = color[1]
    instances[:, 6] = color[2]
    instances[:, 7] = opacity
    instances[:, 8] = 1.0              # RoundedRect
    instances[:, 9] = np.clip(0.15, 0.0, 0.5)  # corner radius in normalized UV space

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


def bar(
    x,
    y,
    *,
    color: tuple[float, float, float] | None = None,
    bar_width: float = 0.8,
    opacity: float = 1.0,
    width: int = 800,
    height: int = 600,
    padding: float = 0.05,
    interactive: bool = False,
) -> np.ndarray | None:
    """Render a bar chart to an RGBA numpy array.

    Parameters
    ----------
    x, y : array-like
        Data coordinates.  ``x`` gives category positions, ``y`` gives bar heights.
    color : tuple of 3 floats in [0, 1], optional
        RGB colour applied to every bar.  Default ``(0.4, 0.6, 1.0)``.
    bar_width : float
        Fraction of spacing between bars (0.0 to 1.0].  Default ``0.8``.
    opacity : float
        Alpha value in [0, 1].  Default ``1.0``.
    width, height : int
        Output image dimensions in pixels.

    Returns
    -------
    np.ndarray
        Shape ``(height, width, 4)``, dtype ``uint8`` — RGBA pixel data.
    """
    if color is None:
        color = (0.4, 0.6, 1.0)

    x = np.asarray(x, dtype=np.float32)
    y = np.asarray(y, dtype=np.float32)

    if len(x) != len(y):
        raise ValueError("x and y must have the same length")
    if len(x) == 0:
        raise ValueError("x and y must not be empty")

    _validate_color(color)
    _validate_bar_width(bar_width)
    _validate_opacity(opacity)

    mapper = _padded_mapper(
        float(x.min()), float(x.max()),
        0.0, float(y.max()),
        float(width), float(height), padding,
    )

    instances = _build_bar_instances(
        x, y, color, bar_width, opacity,
        float(width), float(height), padding,
    )

    if interactive:
        dr2d.show_sdf_window(instances, width, height, "justviz — bar")
        return None

    t0 = time.perf_counter()
    renderer = dr2d.HeadlessRenderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(f"bar: {len(x)} bars, {width}×{height}, render {dt*1000:.1f}ms")
    return result
