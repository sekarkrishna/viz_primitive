"""Distance matrix heatmap — pairwise distance visualization via SDF rounded rects.

Public API:
    distances(data, *, columns, metric, color_ramp, width, height,
              padding, interactive) -> np.ndarray | None

Helpers (testable without GPU):
    _resolve_distance_input(...)   -> (N, N) float32
    _compute_distance_matrix(...)  -> (N, N) float32
    _build_heatmap_instances(...)  -> (N², 10) float32
"""

from __future__ import annotations

import numpy as np

_SUPPORTED_METRICS = {"euclidean", "cosine", "manhattan"}
_INSTANCE_BUDGET = 5_000_000
_MAX_N = 2236  # floor(sqrt(5_000_000))

_DEFAULT_COLOR_RAMP = ((0.1, 0.1, 0.4), (1.0, 0.9, 0.2))


# ── distance computation ────────────────────────────────────────────

def _compute_distance_matrix(
    embeddings: np.ndarray,
    metric: str,
) -> np.ndarray:
    """Compute N×N pairwise distance matrix using scipy.spatial.distance.cdist."""
    try:
        from scipy.spatial.distance import cdist
    except ImportError:
        raise ImportError(
            "scipy is required for distance computation. "
            "Install with: pip install scipy"
        )
    return cdist(embeddings, embeddings, metric=metric).astype(np.float32)


# ── input resolution ────────────────────────────────────────────────

def _resolve_distance_input(
    data,
    *,
    columns: list[str] | None = None,
    metric: str = "euclidean",
) -> np.ndarray:
    """Resolve input into an N×N float32 distance matrix.

    - Square 2D array (N×N) → treat as pre-computed
    - Non-square 2D array (N×D) → compute via cdist
    - DataFrame + columns → extract columns, compute via cdist
    """
    if metric not in _SUPPORTED_METRICS:
        raise ValueError(
            f"Unsupported metric '{metric}'. "
            f"Supported: {', '.join(sorted(_SUPPORTED_METRICS))}"
        )

    # DataFrame path
    if hasattr(data, "columns"):
        if columns is None:
            raise ValueError(
                "columns keyword argument is required when passing a DataFrame"
            )
        available = list(data.columns)
        for col in columns:
            if col not in available:
                raise KeyError(
                    f"Column '{col}' not found in DataFrame. Available: {available}"
                )
        embeddings = np.asarray(data[columns], dtype=np.float32)
        n = embeddings.shape[0]
        if n < 2:
            raise ValueError(f"At least 2 items are required, got {n}")
        if n > _MAX_N:
            raise ValueError(
                f"Distance matrix of {n}×{n} = {n*n} cells exceeds the "
                f"{_INSTANCE_BUDGET:,} instance limit. Reduce matrix size."
            )
        return _compute_distance_matrix(embeddings, metric)

    # Array path
    arr = np.asarray(data, dtype=np.float32)
    if arr.ndim != 2:
        raise ValueError(f"Input must be a 2D array, got {arr.ndim}D")

    n, d = arr.shape
    if n < 2:
        raise ValueError(f"At least 2 items are required, got {n}")

    # Square → pre-computed
    if n == d:
        return arr

    # Non-square → embeddings
    if n > _MAX_N:
        raise ValueError(
            f"Distance matrix of {n}×{n} = {n*n} cells exceeds the "
            f"{_INSTANCE_BUDGET:,} instance limit. Reduce matrix size."
        )
    return _compute_distance_matrix(arr, metric)


# ── heatmap instance builder ────────────────────────────────────────

def _build_heatmap_instances(
    dist_matrix: np.ndarray,
    color_ramp: tuple[tuple[float, float, float], tuple[float, float, float]],
    width: int,
    height: int,
    padding: float,
    gap_fraction: float = 0.05,
) -> np.ndarray:
    """Build N²×10 float32 instance array of RoundedRect cells.

    Each cell: shape_type=1, param=0.15 (corner radius).
    Color interpolated linearly between color_ramp[0] (min dist) and
    color_ramp[1] (max dist). Diagonal cells forced to color_ramp[0].
    """
    n = dist_matrix.shape[0]
    total = n * n

    # Grid layout
    pad = min(width, height) * padding
    usable_w = width - 2 * pad
    usable_h = height - 2 * pad
    cell_size = min(usable_w, usable_h) / n
    gap = cell_size * gap_fraction
    half = (cell_size - gap) / 2.0

    # Color ramp normalization
    lo = np.array(color_ramp[0], dtype=np.float32)
    hi = np.array(color_ramp[1], dtype=np.float32)
    d_min = float(dist_matrix.min())
    d_max = float(dist_matrix.max())
    d_range = d_max - d_min if d_max != d_min else 1.0

    # Vectorized grid positions via meshgrid
    cols, rows = np.meshgrid(np.arange(n), np.arange(n))
    cx = (pad + cols.ravel() * cell_size + cell_size / 2.0).astype(np.float32)
    cy = (pad + rows.ravel() * cell_size + cell_size / 2.0).astype(np.float32)

    # Vectorized color interpolation
    t = ((dist_matrix.ravel() - d_min) / d_range).astype(np.float32)
    rgb = lo[np.newaxis, :] * (1.0 - t[:, np.newaxis]) + hi[np.newaxis, :] * t[:, np.newaxis]

    # Force diagonal cells to lo color
    diag_mask = (rows.ravel() == cols.ravel())
    rgb[diag_mask] = lo

    instances = np.zeros((total, 10), dtype=np.float32)
    instances[:, 0] = cx
    instances[:, 1] = cy
    instances[:, 2] = half
    instances[:, 3] = half
    instances[:, 4] = rgb[:, 0]
    instances[:, 5] = rgb[:, 1]
    instances[:, 6] = rgb[:, 2]
    instances[:, 7] = 1.0
    instances[:, 8] = 1.0   # RoundedRect
    instances[:, 9] = 0.15  # corner radius

    return instances


# ── validation helpers ──────────────────────────────────────────────

def _validate_color_ramp(color_ramp) -> None:
    """Raise ValueError if color_ramp is not a tuple of two RGB tuples."""
    if (
        not isinstance(color_ramp, (tuple, list))
        or len(color_ramp) != 2
        or not all(
            isinstance(c, (tuple, list))
            and len(c) == 3
            and all(isinstance(v, (int, float)) for v in c)
            for c in color_ramp
        )
    ):
        raise ValueError("color_ramp must be a tuple of two RGB tuples")


# ── public API ──────────────────────────────────────────────────────

def distances(
    data,
    *,
    columns: list[str] | None = None,
    metric: str = "euclidean",
    color_ramp: tuple[tuple[float, float, float], tuple[float, float, float]] | None = None,
    width: int = 800,
    height: int = 800,
    padding: float = 0.02,
    interactive: bool = False,
) -> np.ndarray | None:
    """Pairwise distance matrix heatmap.

    Parameters
    ----------
    data : array-like or DataFrame
        Pre-computed N×N distance matrix, (N, D) embeddings, or a DataFrame.
    columns : list[str], optional
        Column names to extract from a DataFrame as feature vectors.
    metric : str
        Distance metric: "euclidean", "cosine", or "manhattan". Default "euclidean".
    color_ramp : tuple of two RGB tuples, optional
        (low_color, high_color). Default dark blue → bright yellow.
    width, height : int
        Output dimensions in pixels.
    padding : float
        Padding fraction around the grid.
    interactive : bool
        If True, open an interactive window instead of returning pixel data.

    Returns
    -------
    np.ndarray or None
        RGBA pixel data (height, width, 4) uint8, or None if interactive.
    """
    import time

    from justviz._renderer import get_renderer
    from justviz._window import launch_window

    if color_ramp is None:
        color_ramp = _DEFAULT_COLOR_RAMP
    else:
        _validate_color_ramp(color_ramp)

    dist_matrix = _resolve_distance_input(data, columns=columns, metric=metric)
    n = dist_matrix.shape[0]

    # Budget check (also done in _resolve_distance_input for embeddings path,
    # but needed here for pre-computed matrices too)
    total = n * n
    if total > _INSTANCE_BUDGET:
        raise ValueError(
            f"Distance matrix of {n}×{n} = {total} cells exceeds the "
            f"{_INSTANCE_BUDGET:,} instance limit. Reduce matrix size."
        )

    instances = _build_heatmap_instances(
        dist_matrix, color_ramp, width, height, padding,
    )

    if interactive:
        launch_window(instances, width, height, "justviz — distances")
        return None

    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(f"distances: {n}×{n} ({total:,} cells), {width}×{height}, render {dt*1000:.1f}ms")
    return result
