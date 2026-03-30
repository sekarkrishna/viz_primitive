"""
justviz + dr2d test script for Windows laptop.

Install first:
    pip install dr2d==0.1.0a1 justviz==0.1.0a2 numpy Pillow

Run:
    python test_laptop.py
"""

import time
import numpy as np

# ── 1. Test dr2d import ─────────────────────────────────────────────
print("=" * 60)
print("1. Testing dr2d import...")
import dr2d
print(f"   dr2d loaded: {dir(dr2d)}")
print(f"   HeadlessRenderer: {hasattr(dr2d, 'HeadlessRenderer')}")
print(f"   CoordinateMapper: {hasattr(dr2d, 'CoordinateMapper')}")
print(f"   load_parquet_columns: {hasattr(dr2d, 'load_parquet_columns')}")
print(f"   show_sdf_window: {hasattr(dr2d, 'show_sdf_window')}")
print("   OK")

# ── 2. Test justviz import ──────────────────────────────────────────
print("\n2. Testing justviz import...")
from justviz import scatter, bar
print("   scatter and bar imported OK")

# ── 3. Basic scatter (headless) ─────────────────────────────────────
print("\n3. Basic scatter (1K points, headless)...")
np.random.seed(42)
x = np.random.uniform(0, 100, 1000).astype(np.float32)
y = np.random.uniform(0, 100, 1000).astype(np.float32)

t0 = time.perf_counter()
img = scatter(x, y, color=(1.0, 0.4, 0.6), size=4.0, width=800, height=600)
dt = time.perf_counter() - t0
print(f"   Shape: {img.shape}, dtype: {img.dtype}")
print(f"   Total time (incl GPU init): {dt*1000:.0f}ms")

# Save to file
from PIL import Image
Image.fromarray(img, mode="RGBA").save("test_scatter_1k.png")
print("   Saved: test_scatter_1k.png")

# ── 4. Basic bar chart (headless) ──────────────────────────────────
print("\n4. Basic bar chart (8 bars, headless)...")
categories = np.arange(8, dtype=np.float32)
values = np.array([23, 45, 12, 67, 34, 89, 56, 41], dtype=np.float32)

img = bar(categories, values, color=(0.4, 0.6, 1.0), bar_width=0.7, width=800, height=600)
print(f"   Shape: {img.shape}")
Image.fromarray(img, mode="RGBA").save("test_bar_8.png")
print("   Saved: test_bar_8.png")

# ── 5. Large scatter (1M points, 2 layers) ──────────────────────────
print("\n5. Multi-layer scatter (1M points, headless)...")
np.random.seed(123)
x1 = np.random.normal(30, 15, 500_000).astype(np.float32)
y1 = np.random.normal(30, 15, 500_000).astype(np.float32)
x2 = np.random.normal(70, 15, 500_000).astype(np.float32)
y2 = np.random.normal(70, 15, 500_000).astype(np.float32)

t0 = time.perf_counter()
img = scatter(
    None, None,
    layers=[
        {"x": x1, "y": y1, "color": (1.0, 0.3, 0.3), "size": 1.5, "opacity": 0.5},
        {"x": x2, "y": y2, "color": (0.3, 0.3, 1.0), "size": 1.5, "opacity": 0.5},
    ],
    width=1024, height=768,
)
dt = time.perf_counter() - t0
print(f"   Shape: {img.shape}")
print(f"   Total time: {dt*1000:.0f}ms")
Image.fromarray(img, mode="RGBA").save("test_scatter_1M.png")
print("   Saved: test_scatter_1M.png")

# ── 6. Interactive window test ──────────────────────────────────────
print("\n6. Interactive scatter window (close window to continue)...")
print("   Controls: drag=pan, scroll=zoom, F=fit, Home=reset, F11=fullscreen")
x_small = np.random.uniform(0, 100, 5000).astype(np.float32)
y_small = np.random.uniform(0, 100, 5000).astype(np.float32)
scatter(x_small, y_small, color=(0.2, 0.8, 0.4), size=3.0, interactive=True)
print("   Window closed OK")

# ── Done ────────────────────────────────────────────────────────────
print("\n" + "=" * 60)
print("All tests passed! Check the .png files in the current directory.")
