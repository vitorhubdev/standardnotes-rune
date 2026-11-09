#!/usr/bin/env python3
"""Check the actual release executable's scripted Rune snapshot."""
import json
from pathlib import Path
import sys


snapshot = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
elements = snapshot["elements"]
visible_text = {
    element.get("text")
    for element in elements
    if element.get("opacity", 1) > 0.99
}
expected = {
    "Teste de notas Rune",
    "Texto com acentuação: ação, coração e informação.",
    "Desafixar",
    "Escuro",
}
missing = expected - visible_text
if missing:
    raise SystemExit(f"Release smoke lost text or an action: {sorted(missing)}")
app = next(element for element in elements if element.get("key") == "app")
if app["fill"] != "#f0f1f4ff" or snapshot["animating"]:
    raise SystemExit("Release smoke ended before the light theme settled")
print("Release smoke passed: Unicode editing, pinning, light theme and settled rendering")
