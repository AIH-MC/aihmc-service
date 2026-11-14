"""Real Rust process and existing-instance database fixtures for Windows tests."""

import json
import os
import re
import shutil
import socket
import subprocess
import time
import uuid
from pathlib import Path

import httpx
import pymysql
import yaml

ROOT = Path(__file__).resolve().parents[1]
TOKEN = {"Authorization": "Bearer rust-integration-token"}


def encode_toml(config):
    lines = []
    for key, value in config.items():
        if not isinstance(value, dict) and not (
            isinstance(value, list) and value and isinstance(value[0], dict)
        ):
            lines.append(f"{key} = {json.dumps(value, ensure_ascii=False)}")
    for key, value in config.items():
        if isinstance(value, dict):
            lines.append(f"\n[{key}]")
            lines.extend(
                f"{name} = {json.dumps(item, ensure_ascii=False)}"
                for name, item in value.items()
            )
        elif isinstance(value, list) and value and isinstance(value[0], dict):
            for item in value:
                lines.append(f"\n[[{key}]]")
                lines.extend(
                    f"{name} = {json.dumps(field, ensure_ascii=False)}"
                    for name, field in item.items()
                )
    return "\n".join(lines) + "\n"


class IsolatedDatabase:
    def __init__(self):
        config_path = Path(os.environ.get("AIHMC_TEST_CONFIG", ROOT / "config.yml"))
        if config_path.suffix == ".toml":
            import tomllib

            config = tomllib.loads(config_path.read_text(encoding="utf-8"))
        else:
            config = yaml.safe_load(config_path.read_text(encoding="utf-8"))
        self.config = dict(config["database"])
        self.connection = pymysql.connect(
            host=self.config["host"],
            port=self.config["port"],
            user=self.config["user"],
            password=str(self.config["password"]),
            autocommit=True,
            charset="utf8mb4",
            cursorclass=pymysql.cursors.DictCursor,
        )
        self.name = "aihmc_migration_test_" + uuid.uuid4().hex
        self.config["db_name"] = self.name
        with self.connection.cursor() as cursor:
            cursor.execute(
                f"CREATE DATABASE `{self.name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
            )
            cursor.execute(f"USE `{self.name}`")
            for statement in re.findall(
                r"CREATE TABLE IF NOT EXISTS.*?;",
                (ROOT / "database.sql").read_text(encoding="utf-8"),
                re.DOTALL,
            ):
                cursor.execute(statement)

    def query(self, statement, args=()):
        with self.connection.cursor() as cursor:
            cursor.execute(statement, args)
            return cursor.fetchall()

    def reset(self):
        for table in [
            "users",
            "userlink",
            "namelink",
            "offline",
            "figura",
            "figura_server",
            "skins",
        ]:
            self.query(f"TRUNCATE TABLE `{table}`")

    def close(self):
        assert re.fullmatch(r"aihmc_migration_test_[0-9a-f]{32}", self.name)
        try:
            self.query(f"DROP DATABASE `{self.name}`")
        finally:
            self.connection.close()


class RustService:
    def __init__(self, database, sources=None, proxy=None, allow_offline=True):
        self.directory = ROOT / "test-output" / ("rust-http-" + uuid.uuid4().hex)
        self.directory.mkdir(parents=True)
        shutil.copytree(ROOT / "assets", self.directory / "assets")
        (self.directory / "textures").mkdir()
        (self.directory / "public.key").write_text(
            "baseline-public-key-not-for-production\n", encoding="utf-8"
        )
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            port = reserved.getsockname()[1]
        config = {
            "allow_offline": allow_offline,
            "figura_session": "2d",
            "access_token": "rust-integration-token",
            "extra_skin_domains": ["minecraft.net"],
            "servers": sources or [],
            "database": database.config,
            "server": {
                "host": "127.0.0.1",
                "port": port,
                "debug": False,
                "timeout": 2.0,
            },
            "rsakey": {"public_keyfile": "public.key", "private_keyfile": "unused.pem"},
        }
        config_path = self.directory / "config.toml"
        config_path.write_text(encode_toml(config), encoding="utf-8")
        environment = os.environ.copy()
        for name in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ]:
            environment.pop(name, None)
        if proxy:
            environment.update(
                HTTP_PROXY=proxy, http_proxy=proxy, NO_PROXY="", no_proxy=""
            )
        self.log = (self.directory / "server.log").open("w", encoding="utf-8")
        executable = Path(
            os.environ.get(
                "AIHMC_TEST_BINARY",
                ROOT / "target/x86_64-pc-windows-msvc/debug/aihmc-auth.exe",
            )
        )
        self.process = subprocess.Popen(
            [str(executable), "--config", str(config_path)],
            cwd=ROOT,
            env=environment,
            stdout=self.log,
            stderr=subprocess.STDOUT,
        )
        self.client = httpx.Client(
            base_url=f"http://127.0.0.1:{port}",
            headers=TOKEN,
            trust_env=False,
            timeout=15,
        )
        try:
            for _ in range(100):
                if self.process.poll() is not None:
                    raise RuntimeError(
                        f"Rust service exited; inspect {self.directory / 'server.log'}"
                    )
                try:
                    if self.client.get("/").status_code == 200:
                        return
                except httpx.TransportError:
                    time.sleep(0.1)
            raise RuntimeError("Rust service did not become ready")
        except BaseException:
            self.close()
            raise

    def close(self):
        self.client.close()
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=10)
        self.log.close()
        # Credentials remain only in the running fixture and are removed afterwards.
        (self.directory / "config.toml").unlink(missing_ok=True)
