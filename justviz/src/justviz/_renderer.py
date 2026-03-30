"""Cached HeadlessRenderer — initialized once, reused across all chart calls."""

from __future__ import annotations

import dr2d

_renderer: dr2d.HeadlessRenderer | None = None


def get_renderer() -> dr2d.HeadlessRenderer:
    """Return the cached HeadlessRenderer, creating it on first call."""
    global _renderer
    if _renderer is None:
        _renderer = dr2d.HeadlessRenderer()
    return _renderer
