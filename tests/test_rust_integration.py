import json
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

import bcrypt

from tests.rust_support import IsolatedDatabase, RustService


class RustDatabaseHTTP(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.database = IsolatedDatabase()
        try:
            cls.service = RustService(cls.database)
        except BaseException:
            cls.database.close()
            raise

    @classmethod
    def tearDownClass(cls):
        cls.service.close()
        cls.database.close()

    def setUp(self):
        self.database.reset()
        self.client = self.service.client

    def test_offline_account_and_cross_language_hashes(self):
        body = {
            "username": "Player",
            "passwd": "密码",
            "repasswd": "密码",
            "ip": "192.0.2.1",
        }
        self.assertEqual(
            self.client.post("/offline/reg", json=body).json(), {"message": "注册成功"}
        )
        self.assertEqual(self.client.post("/offline/reg", json=body).status_code, 400)
        row = self.database.query("SELECT * FROM offline")[0]
        self.assertTrue(bcrypt.checkpw("密码".encode(), row["password"].encode()))
        self.assertEqual(row["ip"], "192.0.2.1")
        self.assertEqual(
            self.client.post(
                "/offline/login", json={"username": "Player", "passwd": "wrong"}
            ).status_code,
            403,
        )
        self.assertEqual(
            self.client.post(
                "/offline/login", json={"username": "Missing", "passwd": "p"}
            ).status_code,
            404,
        )
        self.assertEqual(self.client.get("/offline/check/Player").status_code, 200)
        self.assertEqual(self.client.get("/offline/check/Missing").status_code, 404)
        self.assertEqual(
            self.client.post(
                "/offline/chpasswd",
                json={"username": "Player", "passwd": "密码", "newpasswd": "new"},
            ).status_code,
            200,
        )
        self.assertEqual(
            self.client.post(
                "/offline/login", json={"username": "Player", "passwd": "new"}
            ).status_code,
            200,
        )
        reference_hash = bcrypt.hashpw("参考密码".encode(), bcrypt.gensalt()).decode()
        self.database.query(
            "UPDATE offline SET password=%s WHERE username=%s",
            (reference_hash, "Player"),
        )
        self.assertEqual(
            self.client.post(
                "/offline/login", json={"username": "Player", "passwd": "参考密码"}
            ).status_code,
            200,
        )

    def test_figura_sessions_sync_and_fallback(self):
        query = {"username": "Player", "serverId": "s", "ip": "192.0.2.1"}
        self.assertEqual(
            self.client.get("/figura/hasJoined", params=query).status_code, 200
        )
        body = {"username": "Player", "passwd": "p", "repasswd": "p"}
        self.assertEqual(
            self.client.post(
                "/figura/register",
                json=body,
                headers={"X-Forwarded-For": "192.0.2.1, 192.0.2.2"},
            ).json(),
            {"message": "操作成功"},
        )
        self.assertEqual(
            self.client.get("/figura/hasJoined", params=query).status_code, 200
        )
        self.assertEqual(
            self.client.get(
                "/figura/hasJoined", params={**query, "ip": "192.0.2.2"}
            ).status_code,
            204,
        )
        self.database.query(
            "UPDATE figura SET last_time=%s", (int(time.time()) - 172801,)
        )
        self.assertEqual(
            self.client.get("/figura/hasJoined", params=query).status_code, 204
        )
        self.assertEqual(
            self.client.post(
                "/figura/refresh", json={"username": "Player", "passwd": "bad"}
            ).status_code,
            403,
        )
        self.assertEqual(
            self.client.post(
                "/figura/refresh",
                json={"username": "Player", "passwd": "p"},
                headers={"X-Forwarded-For": "192.0.2.1"},
            ).status_code,
            200,
        )
        self.assertEqual(
            self.client.get("/figura/hasJoined", params=query).status_code, 200
        )
        sync = {"uuid": "12345678-1234-1234-1234-123456789abc", "username": "Player"}
        self.assertEqual(
            self.client.post("/figura/server", json=sync).json(),
            {"message": "写入成功"},
        )
        self.assertEqual(
            self.client.post("/figura/server", json=sync).json(),
            {"message": "更新成功"},
        )
        self.assertEqual(
            self.client.get(
                "/figura/hasJoined", params={**query, "ip": "other"}
            ).json()["id"],
            sync["uuid"].replace("-", ""),
        )

    def test_profile_mutations_and_empty_responses(self):
        self.client.post(
            "/offline/reg", json={"username": "Player", "passwd": "p", "repasswd": "p"}
        )
        uuid = self.database.query("SELECT uuid FROM offline")[0]["uuid"]
        self.assertEqual(
            self.client.post(
                "/profile/linkprofile", json={"uuid": uuid, "new_uuid": "b" * 32}
            ).status_code,
            400,
        )
        self.assertEqual(
            self.database.query("SELECT new_uuid FROM userlink")[0]["new_uuid"],
            "b" * 32,
        )
        self.assertEqual(
            self.client.post(
                "/profile/rename", json={"uuid": uuid, "username": "Renamed"}
            ).status_code,
            200,
        )
        self.assertEqual(
            self.database.query("SELECT username FROM namelink")[0]["username"],
            "Renamed",
        )
        self.database.query(
            "INSERT INTO users (uuid,username) VALUES (%s,%s)", (uuid, "Original")
        )
        self.assertEqual(self.client.get(f"/profile/rstname/{uuid}").status_code, 200)
        self.assertEqual(
            self.database.query("SELECT username FROM namelink")[0]["username"],
            "Player",
        )
        for path in [
            "/auth/sessionserver/session/minecraft/hasJoined?username=P&serverId=s",
            f"/auth/sessionserver/session/minecraft/profile/{uuid}",
            "/auth/api/users/profiles/minecraft/Missing",
        ]:
            response = self.client.get(path)
            self.assertEqual(response.status_code, 204)
            self.assertEqual(response.content, b"")

    def test_http_input_and_auth(self):
        self.assertEqual(
            self.client.get(
                "/check_token", headers={"Authorization": "Bearer wrong"}
            ).status_code,
            401,
        )
        response = self.client.post("/offline/login", json={"username": "Player"})
        self.assertEqual(response.status_code, 422)
        self.assertEqual(response.json()["detail"][0]["loc"], ["body", "passwd"])
        self.assertEqual(
            self.client.post(
                "/offline/login", json={"username": 42, "passwd": None}
            ).status_code,
            422,
        )
        self.assertEqual(
            self.client.get("/figura/hasJoined?username=Player").status_code, 422
        )
        self.assertEqual(
            self.client.get("/auth").json()["signaturePublickey"].strip(),
            "baseline-public-key-not-for-production",
        )
        response = self.client.options(
            "/offline/login",
            headers={
                "Origin": "https://example.com",
                "Access-Control-Request-Method": "POST",
            },
        )
        self.assertEqual(response.headers["access-control-allow-origin"], "*")

    def test_disabled_offline_configuration(self):
        service = RustService(self.database, allow_offline=False)
        try:
            self.assertEqual(
                service.client.post(
                    "/offline/reg",
                    json={"username": "Player", "passwd": "p", "repasswd": "p"},
                ).status_code,
                400,
            )
            self.assertEqual(
                service.client.post(
                    "/offline/login", json={"username": "Player", "passwd": "p"}
                ).status_code,
                400,
            )
            self.assertEqual(
                service.client.get("/offline/check/Player").status_code, 400
            )
        finally:
            service.close()


class RustUpstreamHTTP(unittest.TestCase):
    def test_real_proxy_upstream_persistence_and_source_order(self):
        requests = []
        profile = {
            "id": "a" * 32,
            "name": "Player",
            "properties": [
                {
                    "name": "textures",
                    "value": "opaque-texture-value",
                    "signature": "opaque-signature",
                }
            ],
        }

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                url = urlparse(self.path)
                status = 204 if "first.test" in url.netloc else 200
                payload = profile
                if "/users/profiles/minecraft/" in url.path:
                    payload = {"id": profile["id"], "name": "Player"}
                data = json.dumps(payload).encode()
                self.send_response(status)
                self.send_header(
                    "Content-Length", str(len(data)) if status == 200 else "0"
                )
                self.end_headers()
                if status == 200:
                    self.wfile.write(data)

            def log_message(self, *args):
                pass

        proxy = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=proxy.serve_forever, daemon=True)
        thread.start()
        database = IsolatedDatabase()
        service = None
        try:
            sources = [
                {
                    "name": "Disabled",
                    "api_type": "mojang",
                    "root_url": "http://disabled.test",
                    "enabled": False,
                },
                {
                    "name": "First",
                    "api_type": "mojang",
                    "root_url": "http://first.test",
                    "enabled": True,
                },
                {
                    "name": "Second",
                    "api_type": "aihmc",
                    "root_url": "http://example.com",
                    "enabled": True,
                },
            ]
            service = RustService(
                database, sources, f"http://127.0.0.1:{proxy.server_port}"
            )
            client = service.client
            response = client.get(
                "/auth/sessionserver/session/minecraft/hasJoined",
                params={"username": "Player", "serverId": "s", "ip": "192.0.2.1"},
            )
            self.assertEqual(response.status_code, 200, response.text)
            self.assertEqual(response.json(), profile)
            self.assertEqual(len(requests), 2)
            self.assertIn("sessionserver.first.test", requests[0])
            self.assertIn("api.example.com/mcauth", requests[1])
            self.assertIn("serverId=s?ip=192.0.2.1", requests[1])
            row = database.query("SELECT * FROM users")[0]
            self.assertEqual(row["last_source"], "Second")
            self.assertEqual(row["textures_signature"], "opaque-signature")
            self.assertEqual(
                client.get("/auth/api/users/profiles/minecraft/Player").json()[
                    "source"
                ],
                "Second",
            )
            self.assertEqual(
                client.get(
                    "/auth/sessionserver/session/minecraft/profile/" + "a" * 32
                ).json(),
                profile,
            )
            database.query("UPDATE userlink SET new_uuid=%s", ("b" * 32,))
            database.query(
                "INSERT INTO namelink (uuid,username) VALUES (%s,%s)",
                ("b" * 32, "Linked"),
            )
            linked = client.get(
                "/auth/sessionserver/session/minecraft/profile/" + "a" * 32
            ).json()
            self.assertEqual((linked["id"], linked["name"]), ("b" * 32, "Linked"))
            self.assertEqual(linked["properties"][0]["signature"], "opaque-signature")
            requests.clear()
            self.assertEqual(
                client.get("/figura/hasJoined?username=Player&serverId=s&ip=ip").json(),
                profile,
            )
            self.assertNotIn("?ip=", requests[-1])
        finally:
            if service:
                service.close()
            database.close()
            proxy.shutdown()
            proxy.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
