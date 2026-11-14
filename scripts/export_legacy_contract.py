"""Freeze the original OpenAPI and documentation responses without private config."""

import sys
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from tests.baseline_support import main
from fastapi.testclient import TestClient

with TestClient(main.app) as client:
    for route, name in [
        ("/openapi.json", "openapi.json"),
        ("/docs", "docs.html.json"),
        ("/redoc", "redoc.html.json"),
        ("/docs/oauth2-redirect", "oauth2-redirect.html.json"),
    ]:
        response = client.get(route)
        response.raise_for_status()
        if name.endswith(".html.json"):
            # Encode the original HTML verbatim so its meaningful string contents
            # survive Git newline handling without source trailing whitespace.
            (ROOT / "assets" / name).write_text(
                json.dumps(response.text, ensure_ascii=False) + "\n", encoding="utf-8"
            )
        else:
            (ROOT / "assets" / name).write_bytes(response.content)
        print(f"Exported {route} -> assets/{name}")
