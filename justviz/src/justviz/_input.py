"""Shared input resolution for polymorphic chart function arguments.

resolve_input(first_arg, second_arg, *, x, y) -> (x_array, y_array)

Dispatch order:
  1. str -> parquet path -> dr2d.load_parquet_columns(first_arg, x, y)
  2. has 'columns' attr -> DataFrame -> extract columns x, y as numpy float32
  3. otherwise -> array-like -> np.asarray passthrough
"""

from __future__ import annotations

import numpy as np


def resolve_input(
    first_arg,
    second_arg=None,
    *,
    x: str | None = None,
    y: str | None = None,
) -> tuple[np.ndarray, np.ndarray]:
    """Resolve polymorphic first argument into (x_array, y_array) float32 arrays."""

    # 1. String -> parquet path
    if isinstance(first_arg, str):
        if x is None or y is None:
            raise ValueError(
                "x and y column names are required when passing a file path. "
                "Example: scatter('data.parquet', x='col_a', y='col_b')"
            )
        import dr2d
        return dr2d.load_parquet_columns(first_arg, x, y)

    # 2. DataFrame (duck-typed via 'columns' attribute)
    if hasattr(first_arg, "columns"):
        if x is None or y is None:
            raise ValueError(
                "x and y column names are required when passing a DataFrame. "
                "Example: scatter(df, x='col_a', y='col_b')"
            )
        if x not in first_arg.columns:
            raise KeyError(f"Column '{x}' not found in DataFrame. Available: {list(first_arg.columns)}")
        if y not in first_arg.columns:
            raise KeyError(f"Column '{y}' not found in DataFrame. Available: {list(first_arg.columns)}")
        x_arr = np.asarray(first_arg[x], dtype=np.float32)
        y_arr = np.asarray(first_arg[y], dtype=np.float32)
        return x_arr, y_arr

    # 3. Array-like passthrough
    if first_arg is not None and second_arg is not None:
        return np.asarray(first_arg, dtype=np.float32), np.asarray(second_arg, dtype=np.float32)

    # 4. Unrecognized
    raise TypeError(
        f"First argument must be a file path (str), DataFrame, or array-like, "
        f"got {type(first_arg).__name__}"
    )
