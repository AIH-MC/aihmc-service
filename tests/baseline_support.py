"""Isolated configuration for Python contract tests; never read production config."""

import sys
import types
import os
import tomllib
from pathlib import Path
from types import SimpleNamespace

ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = ROOT / "legacy/python"
sys.path.insert(0, str(PYTHON_ROOT))
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
if reference_path := os.environ.get("AIHMC_REFERENCE_CONFIG"):
    reference_path = Path(reference_path)
    config = tomllib.loads(reference_path.read_text(encoding="utf-8"))
    settings.db = config["database"]
    settings.servers = config.get("servers", [])
    settings.extra_skin_domains = config.get("extra_skin_domains", [])
    settings.server = config["server"]
    settings.keys = config["rsakey"]
    settings.keys["public_keyfile"] = str(
        reference_path.parent / settings.keys["public_keyfile"]
    )
    settings.allow_offline = config["allow_offline"]
    settings.figura_session = config["figura_session"]
    settings.access_token = config["access_token"]
config_module = types.ModuleType("libs.config_loader")
config_module.settings = settings
sys.modules["libs.config_loader"] = config_module

# Import only after the isolated settings module is installed.
import main  # noqa: E402

TOKEN = {"Authorization": "Bearer baseline-test-token"}
