"""Compare real Python and Rust HTTP services on separate real databases."""

import io
import unittest

from PIL import Image

from tests.rust_support import IsolatedDatabase, ROOT, RustService


class DifferentialHTTP(unittest.TestCase):
    def setUp(self):
        self.databases = []
        self.services = []
        try:
            for reference in [True, False]:
                database = IsolatedDatabase()
                self.databases.append(database)
                self.services.append(RustService(database, reference=reference))
        except BaseException:
            self.tearDown()
            raise

    def tearDown(self):
        for service in reversed(self.services):
            service.close()
        for database in reversed(self.databases):
            database.close()

    def compare(self, method, path, **kwargs):
        responses = [
            service.client.request(method, path, **kwargs) for service in self.services
        ]
        expected, actual = responses
        self.assertEqual(
            actual.status_code,
            expected.status_code,
            (path, expected.text[:200], actual.text[:200]),
        )
        if method == "HEAD" or not expected.content:
            self.assertEqual(actual.content, expected.content, path)
        elif expected.headers.get("content-type", "").startswith("image/png"):
            pixels = [
                Image.open(io.BytesIO(response.content)).convert("RGBA")
                for response in responses
            ]
            self.assertEqual(pixels[0].size, pixels[1].size)
            self.assertEqual(pixels[0].tobytes(), pixels[1].tobytes())
        elif "application/json" in expected.headers.get("content-type", ""):
            self.assertEqual(actual.json(), expected.json(), path)
        else:
            self.assertEqual(actual.content, expected.content, path)
        return responses

    def test_all_business_routes_and_persisted_state(self):
        for method, path, body in [
            ("GET", "/", None),
            ("GET", "/auth", None),
            (
                "GET",
                "/auth/sessionserver/session/minecraft/hasJoined?username=Player&serverId=s",
                None,
            ),
            ("GET", "/auth/sessionserver/session/minecraft/profile/" + "a" * 32, None),
            ("GET", "/auth/api/users/profiles/minecraft/Player", None),
            ("GET", "/csl/Player.json", None),
            ("GET", "/textures/missing", None),
            ("GET", "/head/!/13", None),
            ("GET", "/check_token", None),
            ("GET", "/offline/check/Player", None),
            (
                "POST",
                "/offline/reg",
                {"username": "Player", "passwd": "p", "repasswd": "p"},
            ),
            ("GET", "/offline/check/Player", None),
            ("POST", "/offline/login", {"username": "Player", "passwd": "p"}),
            (
                "POST",
                "/offline/chpasswd",
                {"username": "Player", "passwd": "p", "newpasswd": "q"},
            ),
            ("GET", "/figura/hasJoined?username=Player&serverId=s&ip=192.0.2.1", None),
            (
                "POST",
                "/figura/register",
                {"username": "Player", "passwd": "p", "repasswd": "p"},
            ),
            ("POST", "/figura/refresh", {"username": "Player", "passwd": "p"}),
            ("POST", "/figura/server", {"username": "Player", "uuid": "a" * 32}),
        ]:
            with self.subTest(path=path):
                self.compare(method, path, json=body)
        player_uuid = self.databases[0].query("SELECT uuid FROM offline")[0]["uuid"]
        self.compare(
            "POST",
            "/profile/linkprofile",
            json={"uuid": player_uuid, "new_uuid": "b" * 32},
        )
        self.compare(
            "POST", "/profile/rename", json={"uuid": player_uuid, "username": "Renamed"}
        )
        for database in self.databases:
            database.query(
                "INSERT INTO users (uuid,username) VALUES (%s,%s)",
                (player_uuid, "Original"),
            )
        self.compare("GET", f"/profile/rstname/{player_uuid}")
        for service in self.services:
            (service.directory / "textures/fixture").write_bytes(
                (ROOT / "assets/alex.png").read_bytes()
            )
        self.compare("GET", "/textures/fixture")
        for path in ["/docs", "/redoc", "/docs/oauth2-redirect"]:
            self.compare("GET", path)
        expected_schema = self.services[0].client.get("/openapi.json").json()
        actual_schema = self.services[1].client.get("/openapi.json").json()
        nickname_route = actual_schema["paths"].pop("/offline/profile/{username}")
        self.assertEqual(nickname_route["get"]["security"], [{"HTTPBearer": []}])
        actual_schema["components"]["schemas"].pop("OfflineProfile")
        actual_schema["components"]["schemas"]["OfflineReg"]["properties"].pop(
            "nickname"
        )
        self.assertEqual(actual_schema, expected_schema)
        for table in ["userlink", "namelink", "users", "skins", "figura_server"]:
            self.assertEqual(
                self.databases[0].query(f"SELECT * FROM {table}"),
                self.databases[1].query(f"SELECT * FROM {table}"),
            )

    def test_validation_and_protocol_details(self):
        for method, path, kwargs in [
            ("GET", "/check_token", {"headers": {"Authorization": ""}}),
            ("GET", "/check_token", {"headers": {"Authorization": "Bearer wrong"}}),
            ("GET", "/figura/hasJoined?username=Player", {}),
            ("GET", "/head/!/invalid", {}),
            ("POST", "/offline/login", {"json": {}}),
            ("POST", "/offline/login", {"json": {"username": 1, "passwd": None}}),
            ("POST", "/offline/login", {"json": []}),
            ("GET", "/missing-route", {}),
            ("POST", "/check_token", {}),
            ("HEAD", "/check_token", {}),
            ("GET", "/check_token/", {}),
        ]:
            with self.subTest(path=path, kwargs=kwargs):
                self.compare(method, path, **kwargs)
        responses = self.compare(
            "OPTIONS",
            "/offline/login",
            headers={
                "Origin": "https://example.com",
                "Access-Control-Request-Method": "POST",
            },
        )
        for header in [
            "access-control-allow-origin",
            "access-control-allow-methods",
            "access-control-max-age",
        ]:
            self.assertEqual(
                responses[0].headers.get(header),
                responses[1].headers.get(header),
                header,
            )


if __name__ == "__main__":
    unittest.main()
