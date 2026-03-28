"""Scatter chart — builds instance arrays for dr2d SDF circles.

This is pure Python + numpy. The scatter logic is just array math:
for each data point, compute (x, y, color, size) and pack into a
float32 array that dr2d renders via instanced draw or SDF pipeline.
"""

import numpy as np


def scatter(x, y, *, color=None, size=4.0, opacity=1.0):
    """Create a scatter plot from x/y data.

    Args:
        x: array-like of x values
        y: array-like of y values
        color: RGB tuple (0-1) or array of per-point colors
        size: point size in pixels
        opacity: opacity (0-1)

    Returns:
        Instance array ready for dr2d rendering.
    """
    # TODO: Phase 2.2 — build instance array and pass to dr2d-py
    x = np.asarray(x, dtype=np.float32)
    y = np.asarray(y, dtype=np.float32)

    if color is None:
        color = [0.2, 0.6, 1.0]

    n = len(x)
    # Instance layout: [center_x, center_y, r, g, b, a] per point
    instances = np.zeros((n, 6), dtype=np.float32)
    instances[:, 0] = x
    instances[:, 1] = y
    instances[:, 2] = color[0]
    instances[:, 3] = color[1]
    instances[:, 4] = color[2]
    instances[:, 5] = opacity

    return instances
