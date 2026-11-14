"""Name reset regressions through real HTTP, Rust and isolated MariaDB."""

import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

from tests.rust_support import IsolatedDatabase, RustService


class NameResetHTTP(unittest.TestCase):
    def setUp(self):
        self.database = IsolatedDatabase()
        self.services = []
        self.available = True
        self.source_id = "a" * 32
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                path = urlparse(self.path).path
                found = owner.available and (
                    path.endswith("/" + owner.source_id) or path.endswith("/hasJoined")
                )
                body = json.dumps(
                    {
                        "id": owner.source_id,
                        "name": "Original",
                        "properties": [
                            {"name": "textures", "value": "opaque", "signature": "sig"}
                        ],
                    }
                ).encode()
                self.send_response(200 if found else 204)
                self.send_header("Content-Length", str(len(body)) if found else "0")
                self.end_headers()
                if found:
                    self.wfile.write(body)

            def log_message(self, *args):
                pass

        self.upstream = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.upstream.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        for service in self.services:
            service.close()
        self.upstream.shutdown()
        self.upstream.server_close()
        self.thread.join()
        self.database.close()

    def client(self, kind="mojang"):
        service = RustService(
            self.database,
            sources=[
                {
                    "name": "Source",
                    "api_type": kind,
                    "root_url": "http://example.com",
                    "enabled": True,
                }
            ],
            proxy=f"http://127.0.0.1:{self.upstream.server_port}",
        )
        self.services.append(service)
        return service.client

    def test_reset_after_rename_and_reauthentication_uses_upstream_original(self):
        for kind in ("mojang", "blessingskin"):
            with self.subTest(kind=kind):
                self.database.reset()
                client = self.client(kind)
                join = "/auth/sessionserver/session/minecraft/hasJoined?username=Original&serverId=test"
                self.database.query(
                    "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
                    (self.source_id, "Original", "Source"),
                )
                self.database.query(
                    "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
                    (self.source_id, "Original"),
                )
                self.assertEqual(
                    client.post(
                        "/profile/rename",
                        json={"uuid": self.source_id, "username": "Display"},
                    ).status_code,
                    200,
                )
                # Reproduce save_player during a real hasJoined request.
                self.assertEqual(client.get(join).status_code, 200)
                self.assertEqual(
                    self.database.query("SELECT username FROM users")[0]["username"],
                    "Display",
                )
                self.assertEqual(
                    client.get(f"/profile/rstname/{self.source_id}").status_code, 200
                )
                profile = client.get(
                    f"/auth/sessionserver/session/minecraft/profile/{self.source_id}"
                ).json()
                self.assertEqual(
                    (profile["id"], profile["name"]), (self.source_id, "Original")
                )

    def test_mapped_uuid_reset_creates_missing_name_without_changing_uuid_link(self):
        target = "b" * 32
        # A premium account can be mapped to the existing offline data UUID.
        self.database.query(
            "INSERT INTO offline (username,uuid,password) VALUES (%s,%s,%s)",
            ("OldOffline", target, "unused"),
        )
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            (self.source_id, "Stale", "Source"),
        )
        self.database.query(
            "INSERT INTO userlink (uuid,new_uuid) VALUES (%s,%s)",
            (self.source_id, target),
        )
        client = self.client()
        self.assertEqual(client.get(f"/profile/rstname/{target}").status_code, 200)
        self.assertEqual(
            self.database.query(
                "SELECT username FROM namelink WHERE uuid=%s", (target,)
            )[0]["username"],
            "Original",
        )
        self.assertEqual(
            self.database.query(
                "SELECT new_uuid FROM userlink WHERE uuid=%s", (self.source_id,)
            )[0]["new_uuid"],
            target,
        )

    def test_unavailable_source_does_not_report_success_or_overwrite_nickname(self):
        self.available = False
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            (self.source_id, "Stale", "Source"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            (self.source_id, "Display"),
        )
        self.assertNotEqual(
            self.client().get(f"/profile/rstname/{self.source_id}").status_code, 200
        )
        self.assertEqual(
            self.database.query("SELECT username FROM namelink")[0]["username"],
            "Display",
        )

    def test_offline_reset_uses_account_name_and_keeps_uuid(self):
        client = self.client()
        self.assertEqual(
            client.post(
                "/offline/reg",
                json={
                    "username": "Account",
                    "passwd": "p",
                    "repasswd": "p",
                    "nickname": "Display",
                },
            ).status_code,
            200,
        )
        before = self.database.query("SELECT * FROM offline")[0]
        self.assertEqual(
            client.get(f"/profile/rstname/{before['uuid']}").status_code, 200
        )
        profile = client.get("/offline/profile/Account").json()
        self.assertEqual((profile["id"], profile["name"]), (before["uuid"], "Account"))
        self.assertEqual(self.database.query("SELECT * FROM offline")[0], before)

    def test_online_reset_displaces_offline_owner_without_changing_either_uuid(self):
        offline_uuid = "b" * 32
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            (self.source_id, "Display", "Source"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            (self.source_id, "Display"),
        )
        self.database.query(
            "INSERT INTO offline (username,uuid,password) VALUES (%s,%s,%s)",
            ("Offline", offline_uuid, "unused"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            (offline_uuid, "Original"),
        )
        self.assertEqual(
            self.client().get(f"/profile/rstname/{self.source_id}").status_code, 200
        )
        names = {
            row["uuid"]: row["username"]
            for row in self.database.query("SELECT * FROM namelink")
        }
        self.assertEqual(names[self.source_id], "Original")
        self.assertRegex(names[offline_uuid], r"^AIHMC_[a-z0-9]{8}$")
        self.assertEqual(
            self.database.query("SELECT uuid FROM offline")[0]["uuid"], offline_uuid
        )

    def test_online_reset_conflict_is_explicit_and_does_not_overwrite_owner(self):
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            (self.source_id, "Display", "Source"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            (self.source_id, "Display"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            ("b" * 32, "Original"),
        )
        self.assertEqual(
            self.client().get(f"/profile/rstname/{self.source_id}").status_code, 409
        )
        self.assertEqual(
            self.database.query(
                "SELECT username FROM namelink WHERE uuid=%s", (self.source_id,)
            )[0]["username"],
            "Display",
        )
