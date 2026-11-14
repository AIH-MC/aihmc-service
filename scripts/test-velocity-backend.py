"""Run Velocity API tests against a real Rust process and isolated MariaDB data."""

import argparse
import os
from pathlib import Path
import subprocess
import sys

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
    try:
        database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            ("a" * 32, "Taken", "Official"),
        )
        database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)", ("a" * 32, "Taken")
        )
        service = RustService(database)
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
        database.close()


if __name__ == "__main__":
    sys.exit(main())
