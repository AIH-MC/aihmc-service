"""Isolated configuration for Python contract tests; never read production config."""

import sys
import types
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
settings = SimpleNamespace(
    db={},
    servers=[],
    extra_skin_domains=["minecraft.net"],
    skin_apis=[],
    server={"host": "127.0.0.1", "port": 0, "debug": False},
    keys={"public_keyfile": str(ROOT / "tests/fixtures/public.key")},
    allow_offline=True,
    figura_session="2d",
    access_token="baseline-test-token",
)
config_module = types.ModuleType("libs.config_loader")
config_module.settings = settings
sys.modules["libs.config_loader"] = config_module

# Import only after the isolated settings module is installed.
import main  # noqa: E402

TOKEN = {"Authorization": "Bearer baseline-test-token"}
