"""Subprocess-based interactive window launcher.

Spawns the interactive window in a separate process so it doesn't
block or interfere with the notebook kernel. Data is passed via a
temporary numpy file.
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import numpy as np


def launch_window(
    instances: np.ndarray,
    width: int,
    height: int,
    title: str = "dr2d",
    layer_sizes: list[int] | None = None,
) -> None:
    """Launch an interactive SDF window in a subprocess."""
    with tempfile.NamedTemporaryFile(suffix=".npz", delete=False) as f:
        tmp_path = f.name
        np.savez(
            f,
            instances=instances,
            width=np.array([width]),
            height=np.array([height]),
            layer_sizes=np.array(layer_sizes or []),
        )

    # Spawn subprocess with a small script that loads data and opens window
    script = f"""
import numpy as np
import dr2d

data = np.load({tmp_path!r}, allow_pickle=False)
instances = data["instances"]
width = int(data["width"][0])
height = int(data["height"][0])
layer_sizes = data["layer_sizes"].tolist()
layer_sizes = layer_sizes if len(layer_sizes) > 0 else None

dr2d.show_sdf_window(instances, width, height, {title!r}, layer_sizes=layer_sizes)

import os
os.unlink({tmp_path!r})
"""
    subprocess.Popen(
        [sys.executable, "-c", script],
        start_new_session=True,
    )


def launch_storyboard_window(
    instances: np.ndarray,
    width: int,
    height: int,
    layer_sizes: list[int],
    slides: list[dict],
) -> None:
    """Launch a storyboard window in a subprocess."""
    import json

    with tempfile.NamedTemporaryFile(suffix=".npz", delete=False) as f:
        tmp_path = f.name
        np.savez(
            f,
            instances=instances,
            width=np.array([width]),
            height=np.array([height]),
            layer_sizes=np.array(layer_sizes),
        )

    slides_json = json.dumps(slides)

    script = f"""
import numpy as np
import json
import dr2d

data = np.load({tmp_path!r}, allow_pickle=False)
instances = data["instances"]
width = int(data["width"][0])
height = int(data["height"][0])
layer_sizes = data["layer_sizes"].tolist()
slides = json.loads({slides_json!r})

dr2d.show_storyboard_window(instances, width, height, layer_sizes, slides)

import os
os.unlink({tmp_path!r})
"""
    subprocess.Popen(
        [sys.executable, "-c", script],
        start_new_session=True,
    )
