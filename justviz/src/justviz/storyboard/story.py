"""Storyboard — GPU-rendered click-through data presentations.

Public API:
    story(*, data, layers, slides, width, height, padding) -> None
"""

from __future__ import annotations

import numpy as np
import dr2d

from justviz.charts.scatter import (
    _build_scatter_instances,
    _padded_mapper,
    _validate_color,
    _validate_opacity,
)


def _resolve_slides(slides: list[dict], num_layers: int) -> list[dict]:
    """Resolve user-facing slide dicts into normalised dicts for Rust.

    Each resolved dict has keys:
        zoom_mode  – "fit" or "explicit"
        pan_x, pan_y, zoom – floats (0.0/1.0 when fit)
        visible_layers – list[int] 0-based (empty = all)
        title – str
    """
    resolved: list[dict] = []

    for i, slide in enumerate(slides):
        # ── zoom ────────────────────────────────────────────────
        zoom_raw = slide.get("zoom", "fit")

        if zoom_raw == "fit":
            zoom_mode = "fit"
            pan_x, pan_y, zoom_val = 0.0, 0.0, 1.0
        elif (
            isinstance(zoom_raw, (tuple, list))
            and len(zoom_raw) == 3
            and all(isinstance(v, (int, float)) for v in zoom_raw)
        ):
            zoom_mode = "explicit"
            pan_x, pan_y, zoom_val = (
                float(zoom_raw[0]),
                float(zoom_raw[1]),
                float(zoom_raw[2]),
            )
        else:
            raise ValueError(
                f"Slide {i}: zoom must be 'fit' or a (pan_x, pan_y, zoom) tuple"
            )

        # ── layers ──────────────────────────────────────────────
        layers_raw = slide.get("layers", "all")

        if layers_raw == "all":
            visible_layers: list[int] = []
        elif isinstance(layers_raw, (list, tuple)):
            visible_layers = []
            for idx in layers_raw:
                if not isinstance(idx, int):
                    raise ValueError(
                        f"Slide {i}: layer indices must be integers, got {type(idx).__name__}"
                    )
                if idx < 1:
                    raise ValueError(
                        f"Slide {i}: layer indices are 1-based, got {idx}"
                    )
                if idx > num_layers:
                    raise ValueError(
                        f"Slide {i}: layer index {idx} exceeds number of layers ({num_layers})"
                    )
                visible_layers.append(idx - 1)  # convert to 0-based
        else:
            raise ValueError(
                f"Slide {i}: layers must be 'all' or a list of 1-based integers"
            )

        # ── title ───────────────────────────────────────────────
        title = slide.get("title", "")

        resolved.append(
            {
                "zoom_mode": zoom_mode,
                "pan_x": pan_x,
                "pan_y": pan_y,
                "zoom": zoom_val,
                "visible_layers": visible_layers,
                "title": str(title),
            }
        )

    return resolved


def story(
    *,
    data: np.ndarray | None = None,
    layers: list[dict] | None = None,
    slides: list[dict],
    width: int = 1024,
    height: int = 768,
    padding: float = 0.05,
) -> None:
    """Create a GPU-rendered click-through storyboard presentation.

    Parameters
    ----------
    data : np.ndarray, optional
        Pre-built Nx10 float32 instance array (single layer).
    layers : list[dict], optional
        Layer dicts with ``"x"``, ``"y"`` and optional ``"color"``,
        ``"size"``, ``"opacity"`` keys (same format as ``scatter()``
        multi-layer mode).
    slides : list[dict]
        Slide definitions. Each dict may contain:
        - ``"zoom"``: ``"fit"`` (default) or ``(pan_x, pan_y, zoom)`` tuple
        - ``"layers"``: ``"all"`` (default) or list of 1-based layer indices
        - ``"title"``: str (default ``""``)
    width, height : int
        Window dimensions in pixels.
    padding : float
        Padding fraction for fit-viewport computation.

    Returns
    -------
    None
        Blocks until the window is closed.
    """
    # ── mutual exclusivity ──────────────────────────────────────
    if data is not None and layers is not None:
        raise ValueError("Only one of `data` or `layers` may be specified")
    if data is None and layers is None:
        raise ValueError("Either `data` or `layers` is required")

    # ── slides must be non-empty ────────────────────────────────
    if not slides:
        raise ValueError("At least one slide is required")

    # ── build instance data ─────────────────────────────────────
    if layers is not None:
        instances, layer_sizes = _build_layers(layers, width, height, padding)
        num_layers = len(layers)
    else:
        # data path: validate Nx10 float32
        data = np.asarray(data, dtype=np.float32)
        if data.ndim != 2 or data.shape[1] != 10:
            raise ValueError("data must be an Nx10 float32 array")
        instances = data
        layer_sizes = [len(data)]
        num_layers = 1

    # ── resolve slides ──────────────────────────────────────────
    resolved_slides = _resolve_slides(slides, num_layers)

    # ── call Rust storyboard window ─────────────────────────────
    from justviz._window import launch_storyboard_window
    launch_storyboard_window(
        instances, width, height, layer_sizes, resolved_slides
    )


def _build_layers(
    layers: list[dict],
    width: int,
    height: int,
    padding: float,
) -> tuple[np.ndarray, list[int]]:
    """Build concatenated Nx10 instance array from layer dicts.

    Returns (instances, layer_sizes).
    """
    default_color = (1.0, 0.4, 0.6)
    default_size = 4.0
    default_opacity = 1.0

    # 1. Parse and validate all layers, collect x/y arrays
    all_x: list[np.ndarray] = []
    all_y: list[np.ndarray] = []
    parsed: list[tuple[np.ndarray, np.ndarray, tuple, float, float]] = []

    for i, layer in enumerate(layers):
        if "x" not in layer or "y" not in layer:
            raise ValueError(f"Layer {i} must contain 'x' and 'y' keys")

        lx = np.asarray(layer["x"], dtype=np.float32)
        ly = np.asarray(layer["y"], dtype=np.float32)

        if len(lx) != len(ly):
            raise ValueError(f"Layer {i}: x and y must have the same length")
        if len(lx) == 0:
            raise ValueError(f"Layer {i}: x and y must not be empty")

        lcolor = layer.get("color", default_color)
        lsize = layer.get("size", default_size)
        lopacity = layer.get("opacity", default_opacity)

        _validate_color(lcolor)
        _validate_opacity(lopacity)

        all_x.append(lx)
        all_y.append(ly)
        parsed.append((lx, ly, lcolor, lsize, lopacity))

    # 2. Global padded mapper from combined bounding box
    concat_x = np.concatenate(all_x)
    concat_y = np.concatenate(all_y)

    mapper = _padded_mapper(
        float(concat_x.min()),
        float(concat_x.max()),
        float(concat_y.min()),
        float(concat_y.max()),
        float(width),
        float(height),
        padding,
    )

    # 3. Build instances per layer
    instance_arrays: list[np.ndarray] = []
    layer_sizes: list[int] = []

    for lx, ly, lcolor, lsize, lopacity in parsed:
        arr = _build_scatter_instances(lx, ly, lcolor, lsize, lopacity, mapper)
        instance_arrays.append(arr)
        layer_sizes.append(len(arr))

    instances = np.concatenate(instance_arrays, axis=0)
    return instances, layer_sizes
