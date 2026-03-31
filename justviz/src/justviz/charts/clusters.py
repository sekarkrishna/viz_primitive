"""Cluster explorer — 2D embedding visualization colored by cluster labels.

Public API:
    clusters(data_or_path, labels=None, *, x_col, y_col, label_col, names,
             size, width, height, padding, interactive) -> np.ndarray | None

Internal helpers (testable without GPU):
    _resolve_cluster_input(data_or_path, labels, *, x_col, y_col, label_col)
    _build_cluster_layers(x, y, labels, size)
    _build_cluster_slides(x, y, labels, sorted_labels, has_noise, names, padding)
"""

from __future__ import annotations

import time
import numpy as np

from justviz.charts.scatter import (
    _build_scatter_instances,
    _padded_mapper,
)
from justviz._renderer import get_renderer
from justviz.storyboard.story import story


# ── constants ───────────────────────────────────────────────────────

CLUSTER_PALETTE: list[tuple[float, float, float]] = [
    (0.12, 0.47, 0.71),   # blue
    (1.00, 0.50, 0.05),   # orange
    (0.17, 0.63, 0.17),   # green
    (0.84, 0.15, 0.16),   # red
    (0.58, 0.40, 0.74),   # purple
    (0.55, 0.34, 0.29),   # brown
    (0.89, 0.47, 0.76),   # pink
    (0.50, 0.50, 0.50),   # gray (distinct from noise gray)
    (0.74, 0.74, 0.13),   # olive
    (0.09, 0.75, 0.81),   # cyan
]

NOISE_COLOR = (0.5, 0.5, 0.5)
NOISE_OPACITY = 0.3
NOISE_LABEL = -1


# ── input resolution ───────────────────────────────────────────────

def _resolve_cluster_input(
    data_or_path,
    labels=None,
    *,
    x_col: str | None = None,
    y_col: str | None = None,
    label_col: str | None = None,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Resolve polymorphic input into (x, y, labels) arrays.

    Dispatch order:
      1. str → parquet path → load with pandas, extract columns
      2. has 'columns' attr → DataFrame → extract named columns
      3. (N,2) ndarray + labels → split columns 0/1, use labels

    Returns
    -------
    (x, y, labels) as (float32, float32, int32) numpy arrays.
    """
    # 1. String → parquet path
    if isinstance(data_or_path, str):
        if x_col is None or y_col is None or label_col is None:
            raise ValueError(
                "x_col, y_col, and label_col are required when passing a file path"
            )
        import pandas as pd

        df = pd.read_parquet(data_or_path, columns=[x_col, y_col, label_col])
        x = np.asarray(df[x_col], dtype=np.float32)
        y = np.asarray(df[y_col], dtype=np.float32)
        lab = np.asarray(df[label_col], dtype=np.int32)
        if len(x) == 0:
            raise ValueError("Input must contain at least one point")
        return x, y, lab

    # 2. DataFrame (duck-typed via 'columns' attribute)
    if hasattr(data_or_path, "columns"):
        if x_col is None or y_col is None or label_col is None:
            raise ValueError(
                "x_col, y_col, and label_col are required when passing a DataFrame"
            )
        cols = list(data_or_path.columns)
        if x_col not in data_or_path.columns:
            raise KeyError(
                f"Column '{x_col}' not found in DataFrame. Available: {cols}"
            )
        if y_col not in data_or_path.columns:
            raise KeyError(
                f"Column '{y_col}' not found in DataFrame. Available: {cols}"
            )
        if label_col not in data_or_path.columns:
            raise KeyError(
                f"Column '{label_col}' not found in DataFrame. Available: {cols}"
            )
        x = np.asarray(data_or_path[x_col], dtype=np.float32)
        y = np.asarray(data_or_path[y_col], dtype=np.float32)
        lab = np.asarray(data_or_path[label_col], dtype=np.int32)
        if len(x) == 0:
            raise ValueError("Input must contain at least one point")
        return x, y, lab

    # 3. Array-like (N,2) + labels
    arr = np.asarray(data_or_path)
    if arr.ndim == 2 and arr.shape[1] == 2:
        if labels is None:
            raise ValueError(
                "labels array is required when passing a numpy array"
            )
        labels_arr = np.asarray(labels, dtype=np.int32)
        if len(arr) != len(labels_arr):
            raise ValueError(
                f"Embedding length {len(arr)} does not match labels length {len(labels_arr)}"
            )
        if len(arr) == 0:
            raise ValueError("Input must contain at least one point")
        x = np.asarray(arr[:, 0], dtype=np.float32)
        y = np.asarray(arr[:, 1], dtype=np.float32)
        return x, y, labels_arr

    raise TypeError(
        f"First argument must be an (N,2) array, DataFrame, or file path, "
        f"got {type(data_or_path).__name__}"
    )


# ── layer construction ──────────────────────────────────────────────

def _build_cluster_layers(
    x: np.ndarray,
    y: np.ndarray,
    labels: np.ndarray,
    size: float,
) -> tuple[list[dict], list[int]]:
    """Build one layer dict per cluster label.

    Returns (layer_dicts, sorted_non_noise_labels) where noise layer
    (if present) is first, followed by non-noise layers in ascending order.
    """
    unique_labels = np.unique(labels)
    has_noise = NOISE_LABEL in unique_labels
    non_noise = sorted(int(l) for l in unique_labels if l != NOISE_LABEL)

    layer_dicts: list[dict] = []

    # Noise layer first (bottom-most)
    if has_noise:
        mask = labels == NOISE_LABEL
        layer_dicts.append({
            "x": x[mask],
            "y": y[mask],
            "color": NOISE_COLOR,
            "size": size,
            "opacity": NOISE_OPACITY,
        })

    # Non-noise layers in ascending label order
    for i, label in enumerate(non_noise):
        mask = labels == label
        layer_dicts.append({
            "x": x[mask],
            "y": y[mask],
            "color": CLUSTER_PALETTE[i % len(CLUSTER_PALETTE)],
            "size": size,
            "opacity": 1.0,
        })

    return layer_dicts, non_noise


# ── slide generation ────────────────────────────────────────────────

def _build_cluster_slides(
    x: np.ndarray,
    y: np.ndarray,
    labels: np.ndarray,
    sorted_labels: list[int],
    has_noise: bool,
    names: list[str] | None,
    padding: float,
) -> list[dict]:
    """Build slide dicts for the storyboard.

    Slide 0: "All Clusters" — fit, all layers.
    Slides 1..K: one per non-noise cluster — fit to cluster bbox, show
    cluster layer + noise layer.
    """
    slides: list[dict] = []

    # Slide 0: overview
    slides.append({
        "title": "All Clusters",
        "zoom": "fit",
        "layers": "all",
    })

    # Per-cluster slides
    # Layer indexing (1-based for story()):
    #   if noise present: noise=1, cluster_0=2, cluster_1=3, ...
    #   if no noise:      cluster_0=1, cluster_1=2, ...
    noise_layer_idx = 1 if has_noise else None
    cluster_start_idx = 2 if has_noise else 1

    for i, label in enumerate(sorted_labels):
        # Title
        if names is not None:
            title = names[i]
        else:
            title = f"Cluster {label}"

        # Visible layers (1-based)
        cluster_layer_idx = cluster_start_idx + i
        visible = [cluster_layer_idx]
        if noise_layer_idx is not None:
            visible.append(noise_layer_idx)

        # Use zoom="fit" — the storyboard fit mode auto-fits to visible
        # layers' bounding box
        slides.append({
            "title": title,
            "zoom": "fit",
            "layers": visible,
        })

    return slides


# ── public API ──────────────────────────────────────────────────────

def clusters(
    data_or_path,
    labels=None,
    *,
    x_col: str | None = None,
    y_col: str | None = None,
    label_col: str | None = None,
    names: list[str] | None = None,
    size: float = 4.0,
    width: int = 1024,
    height: int = 768,
    padding: float = 0.05,
    interactive: bool = False,
) -> np.ndarray | None:
    """Visualize 2D embeddings colored by cluster labels.

    Parameters
    ----------
    data_or_path : array-like, DataFrame, or str
        (N,2) numpy array, pandas/polars DataFrame, or parquet file path.
    labels : array-like, optional
        1D integer cluster assignments. Required for array input.
    x_col, y_col, label_col : str, optional
        Column names for DataFrame/parquet input.
    names : list[str], optional
        Human-readable cluster names (length must equal non-noise cluster count).
    size : float
        Circle radius in pixels (default 4.0).
    width, height : int
        Output dimensions in pixels (default 1024×768).
    padding : float
        Bounding box padding fraction (default 0.05).
    interactive : bool
        If True, open storyboard window. If False, return RGBA array.

    Returns
    -------
    np.ndarray or None
        Shape (height, width, 4) uint8 RGBA when headless; None when interactive.
    """
    # ── resolve input ───────────────────────────────────────────
    x, y, lab = _resolve_cluster_input(
        data_or_path, labels,
        x_col=x_col, y_col=y_col, label_col=label_col,
    )

    # ── build layers ────────────────────────────────────────────
    layer_dicts, sorted_labels = _build_cluster_layers(x, y, lab, size)
    has_noise = NOISE_LABEL in lab

    # ── validate names ──────────────────────────────────────────
    num_non_noise = len(sorted_labels)
    if names is not None and len(names) != num_non_noise:
        raise ValueError(
            f"names has {len(names)} entries but there are {num_non_noise} non-noise clusters"
        )

    # ── build slides ────────────────────────────────────────────
    slide_dicts = _build_cluster_slides(
        x, y, lab, sorted_labels, has_noise, names, padding,
    )

    # ── render ──────────────────────────────────────────────────
    if interactive:
        story(
            layers=layer_dicts,
            slides=slide_dicts,
            width=width,
            height=height,
            padding=padding,
        )
        return None

    # Headless: build instances with global mapper, render all layers
    concat_x = np.concatenate([ld["x"] for ld in layer_dicts])
    concat_y = np.concatenate([ld["y"] for ld in layer_dicts])

    mapper = _padded_mapper(
        float(concat_x.min()), float(concat_x.max()),
        float(concat_y.min()), float(concat_y.max()),
        float(width), float(height), padding,
    )

    instance_arrays = []
    for ld in layer_dicts:
        arr = _build_scatter_instances(
            ld["x"], ld["y"], ld["color"], ld["size"], ld["opacity"], mapper,
        )
        instance_arrays.append(arr)

    instances = np.concatenate(instance_arrays, axis=0)

    total_points = len(instances)
    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(
        f"clusters: {total_points:,} points ({len(layer_dicts)} layers), "
        f"{width}×{height}, render {dt*1000:.1f}ms"
    )
    return result
