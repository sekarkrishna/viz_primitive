"""Graph explorer — network/knowledge graph visualization via SDF circle chains.

Public API:
    graph(data, edges_df=None, *, layout, x_col, y_col, source_col, target_col,
          color_by, size_by, edge_layers, node_size, edge_size, edge_color,
          edge_opacity, width, height, padding, interactive) -> np.ndarray | None

Internal helpers (testable without GPU):
    _resolve_networkx_input(G, layout, color_by, size_by, edge_layers)
    _resolve_dataframe_input(node_df, edge_df, x_col, y_col, source_col, target_col,
                             color_by, size_by, edge_layers)
    _build_edge_chain_instances(edge_list, scene_positions, edge_size, edge_color,
                                edge_opacity, edge_type_values)
    _compute_edge_chain_count(edge_list, scene_positions, edge_size)
    _build_node_instances(x, y, node_size, color_by_values, size_by_values, mapper)
"""

from __future__ import annotations

import math
import time
import numpy as np
import dr2d

from justviz.charts.scatter import (
    _build_scatter_instances,
    _padded_mapper,
)
from justviz.charts.clusters import CLUSTER_PALETTE
from justviz._renderer import get_renderer


# ── constants ───────────────────────────────────────────────────────

SUPPORTED_LAYOUTS = {"spring", "kamada_kawai", "spectral", "circular", "shell", "random"}
MAX_INSTANCES = 5_000_000


# ── input resolution: NetworkX ──────────────────────────────────────

def _resolve_networkx_input(
    G,
    layout: str,
    color_by: str | None,
    size_by: str | None,
    edge_layers: str | None,
) -> tuple[np.ndarray, np.ndarray, list, list[tuple], list | None, list | None, list | None]:
    """Extract nodes, edges, positions, and attributes from a NetworkX graph.

    Parameters
    ----------
    G : networkx.Graph
        The graph object.
    layout : str
        Layout algorithm name (must be in SUPPORTED_LAYOUTS).
    color_by : str or None
        Node attribute name for categorical coloring.
    size_by : str or None
        Node attribute name for numeric sizing.
    edge_layers : str or None
        Edge attribute name for layer grouping.

    Returns
    -------
    (x, y, node_ids, edge_list, color_values, size_values, edge_type_values)
    """
    try:
        import networkx as nx
    except ImportError:
        raise ImportError(
            "NetworkX is required for graph input. Install with: pip install networkx"
        )

    if layout not in SUPPORTED_LAYOUTS:
        raise ValueError(
            f"Unsupported layout '{layout}'. "
            f"Supported: {', '.join(sorted(SUPPORTED_LAYOUTS))}"
        )

    # Compute layout positions
    layout_func = getattr(nx, f"{layout}_layout")
    positions = layout_func(G)

    # Extract arrays
    node_ids = list(positions.keys())
    x = np.array([positions[n][0] for n in node_ids], dtype=np.float32)
    y = np.array([positions[n][1] for n in node_ids], dtype=np.float32)

    # Edge list
    edge_list = [(src, tgt) for src, tgt in G.edges()]

    # Optional node attributes
    color_values = None
    if color_by is not None:
        available = set()
        for n in node_ids:
            available.update(G.nodes[n].keys())
        for n in node_ids:
            if color_by not in G.nodes[n]:
                raise KeyError(
                    f"Node attribute '{color_by}' not found. "
                    f"Available: {sorted(available)}"
                )
        color_values = [G.nodes[n][color_by] for n in node_ids]

    size_values = None
    if size_by is not None:
        available = set()
        for n in node_ids:
            available.update(G.nodes[n].keys())
        for n in node_ids:
            if size_by not in G.nodes[n]:
                raise KeyError(
                    f"Node attribute '{size_by}' not found. "
                    f"Available: {sorted(available)}"
                )
        size_values = [G.nodes[n][size_by] for n in node_ids]

    # Optional edge attribute
    edge_type_values = None
    if edge_layers is not None:
        available = set()
        for u, v, d in G.edges(data=True):
            available.update(d.keys())
        for u, v, d in G.edges(data=True):
            if edge_layers not in d:
                raise KeyError(
                    f"Edge attribute '{edge_layers}' not found. "
                    f"Available: {sorted(available)}"
                )
        edge_type_values = [G.edges[u, v][edge_layers] for u, v in G.edges()]

    return (x, y, node_ids, edge_list, color_values, size_values, edge_type_values)


# ── input resolution: DataFrame ─────────────────────────────────────

def _resolve_dataframe_input(
    node_df,
    edge_df,
    x_col: str,
    y_col: str,
    source_col: str,
    target_col: str,
    color_by: str | None,
    size_by: str | None,
    edge_layers: str | None,
) -> tuple[np.ndarray, np.ndarray, list, list[tuple], list | None, list | None, list | None]:
    """Extract nodes, edges, positions, and attributes from DataFrames.

    Returns
    -------
    (x, y, node_ids, edge_list, color_values, size_values, edge_type_values)
    """
    # Validate node_df columns
    node_cols = list(node_df.columns)
    if x_col not in node_df.columns:
        raise KeyError(
            f"Column '{x_col}' not found in node DataFrame. Available: {node_cols}"
        )
    if y_col not in node_df.columns:
        raise KeyError(
            f"Column '{y_col}' not found in node DataFrame. Available: {node_cols}"
        )

    # Validate non-empty
    if len(node_df) == 0:
        raise ValueError("Node DataFrame must contain at least one row")

    # Validate edge_df columns
    edge_cols = list(edge_df.columns)
    if source_col not in edge_df.columns:
        raise KeyError(
            f"Column '{source_col}' not found in edge DataFrame. Available: {edge_cols}"
        )
    if target_col not in edge_df.columns:
        raise KeyError(
            f"Column '{target_col}' not found in edge DataFrame. Available: {edge_cols}"
        )

    # Extract positions
    x = np.asarray(node_df[x_col], dtype=np.float32)
    y = np.asarray(node_df[y_col], dtype=np.float32)

    # Node IDs from index
    node_ids = list(node_df.index)
    node_id_set = set(node_ids)

    # Edge list + validate endpoints
    sources = list(edge_df[source_col])
    targets = list(edge_df[target_col])
    edge_list = []
    for src, tgt in zip(sources, targets):
        if src not in node_id_set:
            raise ValueError(
                f"Edge references node '{src}' which is not in the node DataFrame"
            )
        if tgt not in node_id_set:
            raise ValueError(
                f"Edge references node '{tgt}' which is not in the node DataFrame"
            )
        edge_list.append((src, tgt))

    # Optional node attributes
    color_values = None
    if color_by is not None:
        if color_by not in node_df.columns:
            raise KeyError(
                f"Column '{color_by}' not found in node DataFrame. Available: {node_cols}"
            )
        color_values = list(node_df[color_by])

    size_values = None
    if size_by is not None:
        if size_by not in node_df.columns:
            raise KeyError(
                f"Column '{size_by}' not found in node DataFrame. Available: {node_cols}"
            )
        size_values = list(node_df[size_by])

    # Optional edge attribute
    edge_type_values = None
    if edge_layers is not None:
        if edge_layers not in edge_df.columns:
            raise KeyError(
                f"Column '{edge_layers}' not found in edge DataFrame. Available: {edge_cols}"
            )
        edge_type_values = list(edge_df[edge_layers])

    return (x, y, node_ids, edge_list, color_values, size_values, edge_type_values)


# ── edge chain builder ──────────────────────────────────────────────

def _compute_edge_chain_count(
    edge_list: list[tuple],
    scene_positions: dict,
    edge_size: float,
) -> int:
    """Count total edge chain circles without allocating arrays."""
    total = 0
    for src, tgt in edge_list:
        sx1, sy1 = scene_positions[src]
        sx2, sy2 = scene_positions[tgt]
        dist = math.sqrt((sx2 - sx1) ** 2 + (sy2 - sy1) ** 2)
        n = max(1, math.ceil(dist / edge_size))
        total += n
    return total


def _build_edge_chain_instances(
    edge_list: list[tuple],
    scene_positions: dict,
    edge_size: float,
    edge_color: tuple[float, float, float],
    edge_opacity: float,
    edge_type_values: list | None = None,
) -> tuple[np.ndarray, list[int]]:
    """Build SDF circle chain instances for all edges.

    Returns
    -------
    (instances, layer_sizes) where instances is Nx10 float32 and
    layer_sizes[i] is the instance count per edge type layer.
    When edge_type_values is None, layer_sizes has one entry.
    """
    if edge_type_values is not None:
        # Group edges by type
        groups: dict[str, list[int]] = {}
        for i, val in enumerate(edge_type_values):
            groups.setdefault(val, []).append(i)

        all_instances = []
        layer_sizes = []
        for _type_key in sorted(groups.keys()):
            edge_indices = groups[_type_key]
            type_rows = []
            for idx in edge_indices:
                src, tgt = edge_list[idx]
                rows = _chain_for_edge(src, tgt, scene_positions, edge_size, edge_color, edge_opacity)
                type_rows.append(rows)
            if type_rows:
                layer_arr = np.concatenate(type_rows, axis=0)
            else:
                layer_arr = np.zeros((0, 10), dtype=np.float32)
            all_instances.append(layer_arr)
            layer_sizes.append(len(layer_arr))

        if all_instances:
            instances = np.concatenate(all_instances, axis=0)
        else:
            instances = np.zeros((0, 10), dtype=np.float32)
        return instances, layer_sizes
    else:
        # Single layer for all edges
        rows_list = []
        for src, tgt in edge_list:
            rows = _chain_for_edge(src, tgt, scene_positions, edge_size, edge_color, edge_opacity)
            rows_list.append(rows)
        if rows_list:
            instances = np.concatenate(rows_list, axis=0)
        else:
            instances = np.zeros((0, 10), dtype=np.float32)
        return instances, [len(instances)]


def _chain_for_edge(
    src,
    tgt,
    scene_positions: dict,
    edge_size: float,
    edge_color: tuple[float, float, float],
    edge_opacity: float,
) -> np.ndarray:
    """Build Nx10 instance rows for a single edge's circle chain."""
    sx1, sy1 = scene_positions[src]
    sx2, sy2 = scene_positions[tgt]
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
    instances[:, 4] = edge_color[0]
    instances[:, 5] = edge_color[1]
    instances[:, 6] = edge_color[2]
    instances[:, 7] = edge_opacity
    instances[:, 8] = 0.0  # shape_type = Circle
    instances[:, 9] = 0.0  # param
    return instances


# ── node instance builder ───────────────────────────────────────────

def _build_node_instances(
    x: np.ndarray,
    y: np.ndarray,
    node_size: float,
    color_by_values: list | None,
    size_by_values: list | None,
    mapper: dr2d.CoordinateMapper,
) -> np.ndarray:
    """Build SDF circle instances for graph nodes.

    When color_by and size_by are both None, delegates to _build_scatter_instances.
    Otherwise builds per-node instances with individual colors/sizes.
    """
    if color_by_values is None and size_by_values is None:
        return _build_scatter_instances(
            x, y, (0.3, 0.6, 1.0), node_size, 1.0, mapper,
        )

    # Map to scene space
    scene_x, scene_y = mapper.map_points(x, y)
    n = len(x)

    # Resolve per-node colors
    if color_by_values is not None:
        unique_vals = sorted(set(color_by_values))
        val_to_idx = {v: i for i, v in enumerate(unique_vals)}
        colors = np.zeros((n, 3), dtype=np.float32)
        for i, val in enumerate(color_by_values):
            cidx = val_to_idx[val] % len(CLUSTER_PALETTE)
            colors[i] = CLUSTER_PALETTE[cidx]
    else:
        colors = np.full((n, 3), (0.3, 0.6, 1.0), dtype=np.float32)

    # Resolve per-node sizes
    if size_by_values is not None:
        vals = np.array(size_by_values, dtype=np.float32)
        vmin, vmax = float(vals.min()), float(vals.max())
        if vmin == vmax:
            sizes = np.full(n, 7.0, dtype=np.float32)
        else:
            sizes = 2.0 + (vals - vmin) / (vmax - vmin) * 10.0
    else:
        sizes = np.full(n, node_size, dtype=np.float32)

    instances = np.zeros((n, 10), dtype=np.float32)
    instances[:, 0] = scene_x
    instances[:, 1] = scene_y
    instances[:, 2] = sizes
    instances[:, 3] = sizes
    instances[:, 4] = colors[:, 0]
    instances[:, 5] = colors[:, 1]
    instances[:, 6] = colors[:, 2]
    instances[:, 7] = 1.0   # opacity
    instances[:, 8] = 0.0   # shape_type = Circle
    instances[:, 9] = 0.0   # param
    return instances


# ── public API ──────────────────────────────────────────────────────

def graph(
    data,
    edges_df=None,
    *,
    layout: str = "spring",
    x_col: str = "x",
    y_col: str = "y",
    source_col: str = "source",
    target_col: str = "target",
    color_by: str | None = None,
    size_by: str | None = None,
    edge_layers: str | None = None,
    node_size: float = 5.0,
    edge_size: float = 1.0,
    edge_color: tuple[float, float, float] = (0.5, 0.5, 0.5),
    edge_opacity: float = 0.3,
    width: int = 1024,
    height: int = 768,
    padding: float = 0.05,
    interactive: bool = False,
) -> np.ndarray | None:
    """Render a network graph to an RGBA numpy array or interactive window.

    Parameters
    ----------
    data : NetworkX graph or DataFrame
        A NetworkX graph object, or a node DataFrame with position columns.
    edges_df : DataFrame, optional
        Edge DataFrame (required when *data* is a DataFrame).
    layout : str
        NetworkX layout algorithm (default "spring").
    x_col, y_col : str
        Node position column names for DataFrame input.
    source_col, target_col : str
        Edge endpoint column names for DataFrame input.
    color_by : str, optional
        Node attribute for categorical coloring.
    size_by : str, optional
        Node attribute for numeric sizing.
    edge_layers : str, optional
        Edge attribute for layer grouping.
    node_size : float
        Default node circle radius in pixels (default 5.0).
    edge_size : float
        Edge chain circle radius in pixels (default 1.0).
    edge_color : tuple
        RGB edge color (default medium gray).
    edge_opacity : float
        Edge alpha (default 0.3).
    width, height : int
        Output dimensions (default 1024×768).
    padding : float
        Bounding box padding fraction (default 0.05).
    interactive : bool
        If True, open interactive window. If False, return RGBA array.

    Returns
    -------
    np.ndarray or None
        Shape (height, width, 4) uint8 RGBA when headless; None when interactive.
    """
    # ── input dispatch ──────────────────────────────────────────
    if hasattr(data, 'nodes') and hasattr(data, 'edges'):
        # NetworkX graph
        x, y, node_ids, edge_list, color_values, size_values, edge_type_values = (
            _resolve_networkx_input(data, layout, color_by, size_by, edge_layers)
        )
    elif hasattr(data, 'columns') and edges_df is not None:
        # DataFrame pair
        x, y, node_ids, edge_list, color_values, size_values, edge_type_values = (
            _resolve_dataframe_input(
                data, edges_df, x_col, y_col, source_col, target_col,
                color_by, size_by, edge_layers,
            )
        )
    else:
        raise TypeError(
            f"First argument must be a NetworkX graph or DataFrame, "
            f"got {type(data).__name__}"
        )

    num_nodes = len(x)
    num_edges = len(edge_list)

    # ── coordinate mapper ───────────────────────────────────────
    mapper = _padded_mapper(
        float(x.min()), float(x.max()),
        float(y.min()), float(y.max()),
        float(width), float(height), padding,
    )

    # ── map node positions to scene space ───────────────────────
    scene_x, scene_y = mapper.map_points(x, y)
    node_id_to_idx = {nid: i for i, nid in enumerate(node_ids)}
    scene_positions = {
        nid: (float(scene_x[node_id_to_idx[nid]]), float(scene_y[node_id_to_idx[nid]]))
        for nid in node_ids
    }

    # ── budget check ────────────────────────────────────────────
    edge_chain_count = _compute_edge_chain_count(edge_list, scene_positions, edge_size)
    total_instances = edge_chain_count + num_nodes
    if total_instances > MAX_INSTANCES:
        raise ValueError(
            f"Total instance count {total_instances:,} exceeds limit of "
            f"{MAX_INSTANCES:,}. Reduce graph size or increase edge circle spacing."
        )

    # ── build edge instances ────────────────────────────────────
    edge_instances, edge_layer_sizes = _build_edge_chain_instances(
        edge_list, scene_positions, edge_size, edge_color, edge_opacity,
        edge_type_values,
    )

    # ── build node instances ────────────────────────────────────
    node_instances = _build_node_instances(
        x, y, node_size, color_values, size_values, mapper,
    )

    # ── concatenate: edges first, nodes last ────────────────────
    if len(edge_instances) > 0 and len(node_instances) > 0:
        instances = np.concatenate([edge_instances, node_instances], axis=0)
    elif len(edge_instances) > 0:
        instances = edge_instances
    else:
        instances = node_instances

    # ── layer sizes: edge layers + node layer ───────────────────
    layer_sizes = edge_layer_sizes + [len(node_instances)]

    # ── render ──────────────────────────────────────────────────
    if interactive:
        dr2d.show_sdf_window(
            instances, width, height, "justviz — graph",
            layer_sizes=layer_sizes,
        )
        return None

    t0 = time.perf_counter()
    renderer = get_renderer()
    result = renderer.render_sdf_to_numpy(instances, width, height)
    dt = time.perf_counter() - t0
    print(
        f"graph: {num_nodes} nodes, {num_edges} edges, "
        f"{len(instances):,} instances, render {dt*1000:.1f}ms"
    )
    return result
