import json
import threading
import unittest
from concurrent.futures import ThreadPoolExecutor
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

from tests.rust_support import IsolatedDatabase, RustService


class OfflineNicknameHTTP(unittest.TestCase):
    def setUp(self):
        self.database = IsolatedDatabase()
        self.services = []

    def tearDown(self):
        for service in self.services:
            service.close()
        self.database.close()

    def service(self, **options):
        service = RustService(self.database, **options)
        self.services.append(service)
        return service.client

    def test_nickname_changes_name_without_changing_account_or_uuid(self):
        client = self.service()
        body = {
            "username": "Account",
            "passwd": "p",
            "repasswd": "p",
            "nickname": "Display",
        }
        self.assertEqual(client.post("/offline/reg", json=body).status_code, 200)
        account = self.database.query("SELECT * FROM offline")[0]
        profile = client.get("/offline/profile/Account").json()
        self.assertEqual(
            (profile["id"], profile["name"], profile["username"]),
            (account["uuid"], "Display", "Account"),
        )
        self.assertTrue(profile["registered"])
        dashed = "-".join(
            [
                account["uuid"][:8],
                account["uuid"][8:12],
                account["uuid"][12:16],
                account["uuid"][16:20],
                account["uuid"][20:],
            ]
        )
        self.assertEqual(
            client.post(
                "/profile/rename", json={"uuid": dashed, "username": "NewDisplay"}
            ).status_code,
            200,
        )
        profile = client.get("/offline/profile/account").json()
        self.assertEqual(
            (profile["id"], profile["name"]), (account["uuid"], "NewDisplay")
        )
        self.assertEqual(self.database.query("SELECT * FROM offline")[0], account)
        self.assertEqual(
            client.post(
                "/offline/login", json={"username": "Account", "passwd": "p"}
            ).status_code,
            200,
        )
        self.assertEqual(
            client.post(
                "/offline/login", json={"username": "NewDisplay", "passwd": "p"}
            ).status_code,
            404,
        )
        self.assertEqual(
            self.database.query("SELECT new_uuid FROM userlink")[0]["new_uuid"],
            account["uuid"],
        )

    def test_existing_account_preserves_stored_uuid_and_player_data(self):
        stored_uuid = "b" * 32
        self.database.query(
            "INSERT INTO offline (username,uuid,password) VALUES (%s,%s,%s)",
            ("OldAccount", stored_uuid, "unused"),
        )
        self.database.query(
            "INSERT INTO userlink (uuid,new_uuid) VALUES (%s,%s)",
            (stored_uuid, stored_uuid),
        )
        self.database.query(
            "INSERT INTO users (uuid,username,textures_value) VALUES (%s,%s,%s)",
            (stored_uuid, "OldAccount", "player-data-sentinel"),
        )
        client = self.service()
        self.assertEqual(
            client.post(
                "/profile/rename", json={"uuid": stored_uuid, "username": "OldNickname"}
            ).status_code,
            200,
        )
        self.assertEqual(
            client.get("/offline/profile/OldAccount").json()["id"], stored_uuid
        )
        self.assertEqual(
            self.database.query(
                "SELECT textures_value FROM users WHERE uuid=%s", (stored_uuid,)
            )[0]["textures_value"],
            "player-data-sentinel",
        )

    def test_existing_online_name_randomizes_only_the_offline_account(self):
        online_uuid = "a" * 32
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            (online_uuid, "Taken", "Official"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
            (online_uuid, "Taken"),
        )
        client = self.service()
        self.assertEqual(
            client.post(
                "/offline/reg",
                json={
                    "username": "Offline",
                    "passwd": "p",
                    "repasswd": "p",
                    "nickname": "taken",
                },
            ).status_code,
            200,
        )
        first = client.get("/offline/profile/Offline").json()
        second = client.get("/offline/profile/Offline").json()
        self.assertRegex(first["name"], r"^AIHMC_[a-z0-9]+$")
        self.assertEqual(first, second)
        self.assertEqual(
            self.database.query(
                "SELECT username FROM namelink WHERE uuid=%s", (online_uuid,)
            )[0]["username"],
            "Taken",
        )
        self.assertEqual(
            first["id"], self.database.query("SELECT uuid FROM offline")[0]["uuid"]
        )

    def test_new_online_login_displaces_offline_nickname_not_uuid(self):
        fixture = {"online": False}
        profile = {
            "id": "a" * 32,
            "name": "Shared",
            "properties": [
                {"name": "textures", "value": "opaque", "signature": "signature"}
            ],
        }

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                path = urlparse(self.path).path
                name_lookup = "/users/profiles/minecraft/" in path
                found = (
                    not name_lookup or fixture["online"] and path.endswith("/Shared")
                )
                body = json.dumps(profile).encode()
                self.send_response(200 if found else 204)
                self.send_header("Content-Length", str(len(body)) if found else "0")
                self.end_headers()
                if found:
                    self.wfile.write(body)

            def log_message(self, *args):
                pass

        proxy = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=proxy.serve_forever, daemon=True)
        thread.start()
        try:
            client = self.service(
                sources=[
                    {
                        "name": "Official",
                        "api_type": "mojang",
                        "root_url": "http://example.com",
                        "enabled": True,
                    }
                ],
                proxy=f"http://127.0.0.1:{proxy.server_port}",
            )
            self.assertEqual(
                client.post(
                    "/offline/reg",
                    json={
                        "username": "Offline",
                        "passwd": "p",
                        "repasswd": "p",
                        "nickname": "Shared",
                    },
                ).status_code,
                200,
            )
            before = client.get("/offline/profile/Offline").json()
            self.assertEqual(before["name"], "Shared")
            fixture["online"] = True
            joined = client.get(
                "/auth/sessionserver/session/minecraft/hasJoined?username=Shared&serverId=s"
            )
            self.assertEqual(joined.status_code, 200, joined.text)
            self.assertEqual(
                (joined.json()["id"], joined.json()["name"]), (profile["id"], "Shared")
            )
            after = client.get("/offline/profile/Offline").json()
            self.assertRegex(after["name"], r"^AIHMC_[a-z0-9]+$")
            self.assertEqual(after["id"], before["id"])
            self.assertEqual(client.get("/offline/profile/Offline").json(), after)
        finally:
            proxy.shutdown()
            proxy.server_close()
            thread.join()

    def test_guest_profile_auth_and_invalid_nickname(self):
        client = self.service()
        response = client.get("/offline/profile/Guest")
        self.assertEqual(response.status_code, 200)
        self.assertFalse(response.json()["registered"])
        self.assertEqual(response.json()["name"], "Guest")
        self.assertEqual(
            client.get(
                "/offline/profile/Guest", headers={"Authorization": ""}
            ).status_code,
            401,
        )
        self.assertEqual(
            client.post(
                "/offline/reg",
                json={
                    "username": "Account",
                    "passwd": "p",
                    "repasswd": "p",
                    "nickname": "bad name",
                },
            ).status_code,
            400,
        )
        self.assertFalse(self.database.query("SELECT * FROM offline"))

    def test_concurrent_accounts_receive_distinct_nicknames(self):
        client = self.service()

        def register(index):
            return client.post(
                "/offline/reg",
                json={
                    "username": f"Account{index}",
                    "passwd": "p",
                    "repasswd": "p",
                    "nickname": "SameNickname",
                },
            )

        with ThreadPoolExecutor(max_workers=4) as executor:
            responses = list(executor.map(register, range(4)))
        self.assertEqual([response.status_code for response in responses], [200] * 4)
        rows = self.database.query("SELECT * FROM namelink")
        self.assertEqual(len(rows), 4)
        self.assertEqual(len({row["username"].lower() for row in rows}), 4)

    def test_conflicting_guest_registers_under_original_account_and_uuid(self):
        self.database.query(
            "INSERT INTO users (uuid,username,last_source) VALUES (%s,%s,%s)",
            ("a" * 32, "Taken", "Official"),
        )
        self.database.query(
            "INSERT INTO namelink (uuid,username) VALUES (%s,%s)", ("a" * 32, "Taken")
        )
        client = self.service()
        guest = client.get("/offline/profile/Taken").json()
        self.assertFalse(guest["registered"])
        self.assertRegex(guest["name"], r"^AIHMC_[a-z0-9]+$")
        response = client.post(
            "/offline/reg",
            json={
                "username": "Taken",
                "passwd": "p",
                "repasswd": "p",
                "nickname": guest["name"],
            },
        )
        self.assertEqual(response.status_code, 200)
        registered = client.get("/offline/profile/Taken").json()
        self.assertEqual(
            (registered["id"], registered["name"]), (guest["id"], guest["name"])
        )
        self.assertTrue(registered["registered"])

    def test_auth_source_name_check_switch_keeps_local_collisions_and_uuid(self):
        requests = []

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                path = urlparse(self.path).path
                requests.append(path)
                found = path.endswith("/RemoteName")
                body = json.dumps({"id": "a" * 32, "name": "RemoteName"}).encode()
                self.send_response(200 if found else 204)
                self.send_header("Content-Length", str(len(body)) if found else "0")
                self.end_headers()
                if found:
                    self.wfile.write(body)

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            for setting in (None, True, False):
                with self.subTest(check_auth_source_names=setting):
                    self.database.reset()
                    requests.clear()
                    client = self.service(
                        sources=[
                            {
                                "name": "Official",
                                "api_type": "mojang",
                                "root_url": "http://example.com",
                                "enabled": True,
                            }
                        ],
                        proxy=f"http://127.0.0.1:{server.server_port}",
                        check_auth_source_names=setting,
                    )
                    self.assertEqual(
                        client.post(
                            "/offline/reg",
                            json={
                                "username": "Account",
                                "passwd": "p",
                                "repasswd": "p",
                                "nickname": "RemoteName",
                            },
                        ).status_code,
                        200,
                    )
                    profile = client.get("/offline/profile/Account").json()
                    uuid = profile["id"]
                    if setting is False:
                        self.assertEqual(profile["name"], "RemoteName")
                        self.assertEqual(requests, [])
                    else:
                        self.assertRegex(profile["name"], r"^AIHMC_[a-z0-9]{8}$")
                        self.assertTrue(requests)
                    # Both local tables must remain authoritative when external checks are off.
                    for table, owner, taken in (
                        ("namelink", "b" * 32, "LocalLink"),
                        ("users", "c" * 32, "LocalUser"),
                    ):
                        self.database.query(
                            f"INSERT INTO {table} (uuid,username) VALUES (%s,%s)",
                            (owner, taken),
                        )
                        self.assertEqual(
                            client.post(
                                "/profile/rename",
                                json={
                                    "uuid": uuid,
                                    "username": taken.lower(),
                                },
                            ).status_code,
                            200,
                        )
                        renamed = client.get("/offline/profile/Account").json()
                        self.assertRegex(renamed["name"], r"^AIHMC_[a-z0-9]{8}$")
                        self.assertEqual(renamed["id"], uuid)
                        self.assertEqual(
                            renamed, client.get("/offline/profile/Account").json()
                        )
                        self.assertEqual(
                            self.database.query(
                                f"SELECT username FROM {table} WHERE uuid=%s", (owner,)
                            )[0]["username"],
                            taken,
                        )
                    # Online manual rename must also skip the external name availability query.
                    response = client.post(
                        "/profile/rename",
                        json={
                            "uuid": "c" * 32,
                            "username": "RemoteName",
                        },
                    )
                    self.assertEqual(
                        response.status_code, 200 if setting is False else 400
                    )
                    if setting is False:
                        self.assertEqual(requests, [])
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
