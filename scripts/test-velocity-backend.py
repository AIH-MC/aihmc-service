"""Run Velocity API tests against a real Rust process and isolated MariaDB data."""

import argparse
import json
import os
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

import bcrypt

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from tests.rust_support import IsolatedDatabase, RustService


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plugin", type=Path, default=ROOT / "test-output/velocity-plugin"
    )
    arguments = parser.parse_args()
    database = IsolatedDatabase()
    service = None

    class UpstreamHandler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = urlparse(self.path).path
            query = parse_qs(urlparse(self.path).query)
            name = None
            if "/users/profiles/minecraft/" in path:
                requested = path.rsplit("/", 1)[-1]
                if requested in ("Taken", "OnlineOriginal", "ConflictAccount"):
                    name = requested
            elif path.endswith("/hasJoined"):
                if query.get("username") == ["OnlineOriginal"]:
                    name = "OnlineOriginal"
            elif path.endswith("/" + "b" * 32):
                name = "OtherAccount"
            elif path.endswith("/" + "c" * 32):
                name = "OnlineOriginal"
            elif path.endswith("/" + "a" * 32):
                name = "Taken"
            ids = {
                "Taken": "a",
                "ConflictAccount": "a",
                "OtherAccount": "b",
                "OnlineOriginal": "c",
            }
            body = json.dumps(
                {
                    "id": ids.get(name, "c") * 32,
                    "name": name,
                    "properties": [
                        {
                            "name": "textures",
                            "value": "eyJ0ZXh0dXJlcyI6e319",
                            "signature": "sig",
                        }
                    ],
                }
            ).encode()
            self.send_response(200 if name else 204)
            self.send_header("Content-Length", str(len(body)) if name else "0")
            self.end_headers()
            if name:
                self.wfile.write(body)

        def log_message(self, *args):
            pass

    upstream = ThreadingHTTPServer(("127.0.0.1", 0), UpstreamHandler)
    thread = threading.Thread(target=upstream.serve_forever, daemon=True)
    thread.start()
    try:
        database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            ("a" * 32, "Taken", "Official"),
        )
        database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)", ("a" * 32, "Taken")
        )
        database.query(
            "INSERT INTO offline (username,uuid,password) VALUES (%s,%s,%s)",
            (
                "ConflictAccount",
                "b" * 32,
                bcrypt.hashpw(b"password", bcrypt.gensalt()).decode(),
            ),
        )
        database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            ("b" * 32, "OfflineDisplay"),
        )
        service = RustService(
            database,
            sources=[
                {
                    "name": "Official",
                    "api_type": "mojang",
                    "root_url": "http://example.com",
                    "enabled": True,
                }
            ],
            proxy=f"http://127.0.0.1:{upstream.server_port}",
        )
        environment = os.environ.copy()
        environment["AIHMC_INTEGRATION_API"] = str(service.client.base_url).rstrip("/")
        environment["AIHMC_INTEGRATION_TOKEN"] = "rust-integration-token"
        # This wrapper is itself executed in the foreground PowerShell Tee-Object
        # pipeline; Gradle inherits its live stdout/stderr without suppression.
        result = subprocess.run(
            [
                str(arguments.plugin / "gradlew.bat"),
                "--no-daemon",
                "test",
                "build",
                "--rerun-tasks",
            ],
            cwd=arguments.plugin,
            env=environment,
        )
        if result.returncode:
            return result.returncode
        owner = database.query(
            "SELECT uuid FROM namelink WHERE username=%s", ("Taken",)
        )[0]["uuid"]
        if owner != "a" * 32:
            raise AssertionError("Official account identity was changed")
        print(
            "PASS: actual Rust/MariaDB + Velocity API event integration; no live proxy/client started"
        )
        return 0
    finally:
        if service:
            service.close()
        upstream.shutdown()
        upstream.server_close()
        thread.join()
        database.close()


if __name__ == "__main__":
    sys.exit(main())
