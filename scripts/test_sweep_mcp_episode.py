#!/usr/bin/env python3
"""Exercise the actual sweep track through the shared reset/provenance canaries."""
import json
from pathlib import Path
import runpy
import sys

if __name__ == "__main__":
    inputs = Path(sys.argv[sys.argv.index("--inputs") + 1])
    manifest = json.loads((inputs / "manifest.json").read_text())
    assert manifest["track"] == "serialized-owned-sweep-synthetic-diagnostic-v1"
    runpy.run_path(str(Path(__file__).with_name("test_reset_mcp_episode.py")), run_name="__main__")
