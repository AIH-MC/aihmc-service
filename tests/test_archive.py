import io
import os
import shutil
import socket
import subprocess
import sys
import time
import unittest
import uuid

import httpx
import yaml

from tests.rust_support import IsolatedDatabase, ROOT


class LegacyStandalone(unittest.TestCase):
    def test_archived_application_runs_independently(self):
        directory = ROOT / "test-output" / ("legacy-standalone-" + uuid.uuid4().hex)
        shutil.copytree(
            ROOT / "legacy/python",
            directory,
            ignore=shutil.ignore_patterns(
                "__pycache__", ".venv", "config.yml", "private.pem", "public.key"
            ),
        )
        database = IsolatedDatabase()
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            port = reserved.getsockname()[1]
        config = yaml.safe_load(
            (directory / "config.example.yml").read_text(encoding="utf-8")
        )
        config.update(
            database=database.config, servers=[], access_token="archive-test-token"
        )
        config["server"] = {"host": "127.0.0.1", "port": port, "debug": False}
        config_path = directory / "config.yml"
        config_path.write_text(yaml.safe_dump(config), encoding="utf-8")
        (directory / "keys/public.key").write_text(
            "archive-test-public-key", encoding="utf-8"
        )
        process = None
        environment = os.environ.copy()
        environment["PYTHONUTF8"] = "1"
        try:
            with (directory / "server.log").open("w", encoding="utf-8") as output:
                process = subprocess.Popen(
                    [sys.executable, "main.py"],
                    cwd=directory,
                    env=environment,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
                with httpx.Client(
                    base_url=f"http://127.0.0.1:{port}", trust_env=False
                ) as client:
                    for _ in range(100):
                        self.assertIsNone(process.poll(), str(directory / "server.log"))
                        try:
                            response = client.get("/", timeout=1)
                            break
                        except httpx.TransportError:
                            time.sleep(0.1)
                    else:
                        self.fail("Archived Python did not become ready")
                    self.assertEqual(response.json(), {"message": "Hello World"})
                    self.assertEqual(
                        client.get("/auth").json()["signaturePublickey"],
                        "archive-test-public-key",
                    )
                    response = client.post(
                        "/offline/reg",
                        headers={"Authorization": "Bearer archive-test-token"},
                        json={"username": "Archived", "passwd": "p", "repasswd": "p"},
                    )
                    self.assertEqual(response.status_code, 200, response.text)
                    self.assertEqual(
                        database.query("SELECT username FROM offline")[0]["username"],
                        "Archived",
                    )
        finally:
            if process is not None:
                process.terminate()
                process.wait(timeout=10)
            database.close()
            config_path.unlink(missing_ok=True)


if __name__ == "__main__":
    unittest.main()
