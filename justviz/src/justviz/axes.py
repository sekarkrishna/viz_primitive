"""Axis rendering — grid lines, tick marks, labels (placeholder).

Moved from dr2d core since axes are chart-specific, not a core primitive.
Uses dr2d text rendering for labels and dr2d quads for grid/tick lines.
"""

# TODO: Phase 2.3
# - nice_step() and compute_tick_positions() — nice numbers algorithm
# - AxisRenderer.generate() — grid lines + tick marks as dr2d vertex arrays
# - Axis labels using dr2d text rendering
# - Auto-fit axis ranges from data, or manual override
