"""Timeline explorer — multi-series temporal visualization via SDF circle chains.

Public API:
    timeline(data, values=None, *, x_col, y_cols, node_size, edge_size,
             edge_opacity, width, height, padding, independent_y,
             interactive, storyboard) -> np.ndarray | None

Internal helpers (testable without GPU):
    _resolve_single_input(dates, values)
    _resolve_dict_input(data)
    _resolve_dataframe_input(df, x_col, y_cols)
    _convert_dates(raw_dates)
    _normalize_series(series_list, independent_y)
    _build_timeline_layers(series_list, mapper, node_size, edge_size, edge_opacity)
    _compute_total_instances(series_list, mapper, edge_size)
"""

from __future__ import annotations

import math
import time
import numpy as np

from justviz.charts.scatter import (
    _build_scatter_instances,
    _padded_mapper,
)
from justviz.charts.clusters import CLUSTER_PALETTE
from justviz._renderer import get_renderer
from justviz._window import launch_window, launch_storyboard_window


# ── constants ───────────────────────────────────────────────────────

MAX_INSTANCES = 5_000_000


# ── input resolution: single series ────────────────────────────────

def _resolve_single_input(
    dates,
    values,
) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """Validate and wrap a single (dates, values) pair into the uniform series list.

    Returns
    -------
    List with one entry: [("Series", dates_arr, values_arr)]
    """
    dates_arr = np.asarray(dates)
    values_arr = np.asarray(values)

    if dates_arr.ndim == 0 or values_arr.ndim == 0:
        raise ValueError("dates and values must be array-like, not scalars")

    if len(dates_arr) != len(values_arr):
        raise ValueError(
            f"dates length {len(dates_arr)} does not match values length {len(values_arr)}"
        )

    if len(dates_arr) == 0:
        raise ValueError("Input must contain at least one data point")

    return [("Series", dates_arr, values_arr)]


# ── input resolution: dict ──────────────────────────────────────────

def _resolve_dict_input(
    data: dict,
) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """Validate and convert a {name: (dates, values)} dict into the uniform series list."""
    if not data:
        raise ValueError("Dictionary must contain at least one series")

    series_list: list[tuple[str, np.ndarray, np.ndarray]] = []
    for name, (dates, values) in data.items():
        dates_arr = np.asarray(dates)
        values_arr = np.asarray(values)

        if len(dates_arr) != len(values_arr):
            raise ValueError(
                f"Series '{name}': dates length {len(dates_arr)} does not match "
                f"values length {len(values_arr)}"
            )

        if len(dates_arr) == 0:
            raise ValueError(
                f"Series '{name}': must contain at least one data point"
            )

        series_list.append((name, dates_arr, values_arr))

    return series_list


# ── input resolution: DataFrame ─────────────────────────────────────

def _resolve_dataframe_input(
    df,
    x_col: str,
    y_cols: list[str],
) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """Extract series from a DataFrame using named columns."""
    available = list(df.columns)

    if x_col not in df.columns:
        raise KeyError(
            f"Column '{x_col}' not found in DataFrame. Available: {available}"
        )

    for col in y_cols:
        if col not in df.columns:
            raise KeyError(
                f"Column '{col}' not found in DataFrame. Available: {available}"
            )

    dates_arr = np.asarray(df[x_col])
    series_list: list[tuple[str, np.ndarray, np.ndarray]] = []
    for col in y_cols:
        values_arr = np.asarray(df[col])
        series_list.append((col, dates_arr, values_arr))

    return series_list


# ── date conversion ─────────────────────────────────────────────────

def _convert_dates(raw_dates: np.ndarray) -> np.ndarray:
    """Convert datetime-like arrays to float64 seconds since min date.

    Numeric arrays pass through unchanged.
    """
    if np.issubdtype(raw_dates.dtype, np.integer) or np.issubdtype(raw_dates.dtype, np.floating):
        return raw_dates.astype(np.float64)

    if np.issubdtype(raw_dates.dtype, np.datetime64):
        # Convert to float64 seconds since minimum
        min_date = raw_dates.min()
        deltas = (raw_dates - min_date).astype("timedelta64[ns]").astype(np.float64)
        return deltas / 1e9  # nanoseconds → seconds

    # Try object array with .timestamp() method (Python datetime objects)
    if raw_dates.dtype == object and len(raw_dates) > 0:
        try:
            timestamps = np.array([d.timestamp() for d in raw_dates], dtype=np.float64)
            min_ts = timestamps.min()
            return timestamps - min_ts
        except (AttributeError, TypeError):
            pass

    raise TypeError(
        "Unsupported date type. Supported formats: numeric (int/float), "
        "datetime64, datetime objects with .timestamp() method"
    )


# ── y-axis normalization ────────────────────────────────────────────

def _normalize_series(
    series_list: list[tuple[str, np.ndarray, np.ndarray]],
    independent_y: bool,
) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """When independent_y=True, normalize each series' y-values to [0, 1]."""
    if not independent_y:
        return series_list

    result: list[tuple[str, np.ndarray, np.ndarray]] = []
    for name, x, y in series_list:
        y_float = y.astype(np.float64)
        y_min = float(y_float.min())
        y_max = float(y_float.max())

        if y_min == y_max:
            # Constant series → map to 0.5
            y_norm = np.full_like(y_float, 0.5)
        else:
            y_norm = (y_float - y_min) / (y_max - y_min)

        result.append((name, x, y_norm))

    return result


# ── layer construction ──────────────────────────────────────────────

def _build_timeline_layers(
    series_list: list[tuple[str, np.ndarray, np.ndarray]],
    mapper,
    node_size: float,
    edge_size: float,
    edge_opacity: float,
) -> tuple[np.ndarray, list[int]]:
    """Build concatenated Nx10 instance array and layer_sizes list.

    For each series: build edge chain instances, then node instances.
    Returns (instances, layer_sizes) where layer_sizes has 2 entries per series
    (edges count, nodes count).
    """
    all_instances: list[np.ndarray] = []
    layer_sizes: list[int] = []

    for i, (name, x, y) in enumerate(series_list):
        color = CLUSTER_PALETTE[i % len(CLUSTER_PALETTE)]
        x_f32 = np.asarray(x, dtype=np.float32)
        y_f32 = np.asarray(y, dtype=np.float32)

        # Map to scene space
        scene_x, scene_y = mapper.map_points(x_f32, y_f32)

        # ── edge chains between consecutive points ──────────────
        edge_rows: list[np.ndarray] = []
        for j in range(len(scene_x) - 1):
            sx1, sy1 = float(scene_x[j]), float(scene_y[j])
            sx2, sy2 = float(scene_x[j + 1]), float(scene_y[j + 1])
            dist = math.sqrt((sx2 - sx1) ** 2 + (sy2 - sy1) ** 2)
            n = max(1, math.ceil(dist / edge_size))

            if n == 1:
                t = np.array([0.5], dtype=np.float32)
            else:
                t = np.linspace(0.0, 1.0, n, dtype=np.float32)

            px = sx1 + t * (sx2 - sx1)
            py = sy1 + t * (sy2 - sy1)

            instances = np.zeros((n, 10), dtype=np.float32)
            instances[:, 0] = px
            instances[:, 1] = py
            instances[:, 2] = edge_size
            instances[:, 3] = edge_size
            instances[:, 4] = color[0]
            instances[:, 5] = color[1]
            instances[:, 6] = color[2]
            instances[:, 7] = edge_opacity
            instances[:, 8] = 0.0  # Circle
            instances[:, 9] = 0.0
            edge_rows.append(instances)

        if edge_rows:
            edge_instances = np.concatenate(edge_rows, axis=0)
        else:
            edge_instances = np.zeros((0, 10), dtype=np.float32)

        all_instances.append(edge_instances)
        layer_sizes.append(len(edge_instances))

        # ── node circles ────────────────────────────────────────
        node_instances = _build_scatter_instances(
            x_f32, y_f32, color, node_size, 1.0, mapper,
        )
        all_instances.append(node_instances)
        layer_sizes.append(len(node_instances))

    if all_instances:
        combined = np.concatenate(all_instances, axis=0)
    else:
        combined = np.zeros((0, 10), dtype=np.float32)

    return combined, layer_sizes


def _compute_total_instances(
    series_list: list[tuple[str, np.ndarray, np.ndarray]],
    mapper,
    edge_size: float,
) -> int:
    """Pre-compute total instance count for budget check without allocating arrays."""
    total = 0
    for name, x, y in series_list:
        x_f32 = np.asarray(x, dtype=np.float32)
        y_f32 = np.asarray(y, dtype=np.float32)
        scene_x, scene_y = mapper.map_points(x_f32, y_f32)

        # Node count
        total += len(x)

        # Edge chain count
        for j in range(len(scene_x) - 1):
            sx1, sy1 = float(scene_x[j]), float(scene_y[j])
            sx2, sy2 = float(scene_x[j + 1]), float(scene_y[j + 1])
            dist = math.sqrt((sx2 - sx1) ** 2 + (sy2 - sy1) ** 2)
            total += max(1, math.ceil(dist / edge_size))

    return total


# ── public API ──────────────────────────────────────────────────────

def timeline(
    data,
    values=None,
    *,
    x_col: str | None = None,
    y_cols: list[str] | None = None,
    node_size: float = 4.0,
    edge_size: float = 1.0,
    edge_opacity: float = 0.6,
    width: int = 1024,
    height: int = 768,
    padding: float = 0.05,
    independent_y: bool = False,
    interactive: bool = False,
    storyboard: list[dict] | None = None,
) -> np.ndarray | None:
    """Render a multi-series timeline to an RGBA numpy array or interactive window.

    Parameters
    ----------
    data : array-like, dict, or DataFrame
        Single series dates (with *values*), a dict mapping names to
        ``(dates, values)`` tuples, or a DataFrame with *x_col* and *y_cols*.
    values : array-like, optional
        Value-axis data for single-series mode.
    x_col : str, optional
        Date column name for DataFrame input.
    y_cols : list[str], optional
        Value column names for DataFrame input.
    node_size : float
        Data-point circle radius in pixels (default 4.0).
    edge_size : float
        Edge chain circle radius in pixels (default 1.0).
    edge_opacity : float
        Edge chain alpha (default 0.6).
    width, height : int
        Output dimensions (default 1024×768).
    padding : float
        Bounding box padding fraction (default 0.05).
    independent_y : bool
        Normalize each series to [0, 1] independently (default False).
    interactive : bool
        Open interactive window instead of returning pixels (default False).
    storyboard : list[dict], optional
        Slide definitions for storyboard mode.

    Returns
    -------
    np.ndarray or None
        Shape (height, width, 4) uint8 RGBA when headless; None otherwise.
    """
    # ── input dispatch ──────────────────────────────────────────
    if values is not None:
        series_list = _resolve_single_input(data, values)
    elif isinstance(data, dict):
        series_list = _resolve_dict_input(data)
    elif hasattr(data, "columns"):
        if x_col is None or y_cols is None:
            raise ValueError(
                "x_col and y_cols are required when passing a DataFrame"
            )
        series_list = _resolve_dataframe_input(data, x_col, y_cols)
    else:
        raise TypeError(
            f"First argument must be array-like (with values=), dict, or DataFrame, "
            f"got {type(data).__name__}"
        )

    # ── date conversion ─────────────────────────────────────────
    converted: list[tuple[str, np.ndarray, np.ndarray]] = []
    for name, dates_arr, vals_arr in series_list:
        x_float = _convert_dates(dates_arr)
        converted.append((name, x_float, vals_arr.astype(np.float64)))
    series_list = converted

    # ── y-axis normalization ────────────────────────────────────
    series_list = _normalize_series(series_list, independent_y)

    # ── global bounding box + mapper ────────────────────────────
    all_x = np.concatenate([x for _, x, _ in series_list])
    all_y = np.concatenate([y for _, _, y in series_list])

    mapper = _padded_mapper(
        float(all_x.min()), float(all_x.max()),
        float(all_y.min()), float(all_y.max()),
        float(width), float(height), padding,
    )

    # ── budget guard ────────────────────────────────────────────
    total_count = _compute_total_instances(series_list, mapper, edge_size)
    if total_count > MAX_INSTANCES:
        raise ValueError(
            f"Total instance count {total_count:,} exceeds limit of "
            f"{MAX_INSTANCES:,}. Reduce data size or increase edge_size."
        )

    # ── build layers ────────────────────────────────────────────
    instances, layer_sizes = _build_timeline_layers(
        series_list, mapper, node_size, edge_size, edge_opacity,
    )

    # ── storyboard mode ─────────────────────────────────────────
    if storyboard is not None:
        launch_storyboard_window(
            instances, width, height, layer_sizes, storyboard,
        )
        return None

    # ── interactive mode ────────────────────────────────────────
    if interactive:
        launch_window(
            instances, width, height, "justviz — timeline",
            layer_sizes=layer_sizes,
        )
        return None

    # ── headless mode ───────────────────────────────────────────
    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    num_series = len(series_list)
    print(
        f"timeline: {num_series} series, {len(instances):,} instances, "
        f"{width}×{height}, render {dt*1000:.1f}ms"
    )
    return result
