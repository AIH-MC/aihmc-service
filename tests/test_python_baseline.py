"""Characterization of existing behavior, not approval of known defects."""

import asyncio
import base64
import io
import json
import socket
import subprocess
import sys
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from unittest.mock import AsyncMock, patch

import httpx
import tldextract
from fastapi.testclient import TestClient
from PIL import Image

from tests.baseline_support import ROOT, TOKEN, main, settings
from libs import auth, figura, head, offline_auth, skin, utils

PROFILE = {"id": "0" * 32, "name": "Player", "properties": []}


class RouteContracts(unittest.TestCase):
    def setUp(self):
        self.client = TestClient(main.app, raise_server_exceptions=False)

    def test_public_routes(self):
        self.assertEqual(self.client.get("/").json(), {"message": "Hello World"})
        meta = self.client.get("/auth").json()
        self.assertEqual(
            meta["signaturePublickey"].strip(), "baseline-public-key-not-for-production"
        )
        self.assertEqual(meta["meta"]["implementationVersion"], "1.0.0")
        for path in ["/openapi.json", "/docs", "/redoc"]:
            self.assertEqual(self.client.get(path).status_code, 200)

    def test_profile_and_join_responses(self):
        cases = [
            (
                "ygg_auth",
                "/auth/sessionserver/session/minecraft/hasJoined?username=Player&serverId=s",
            ),
            (
                "ygg_seesion",
                "/auth/sessionserver/session/minecraft/profile/" + "0" * 32,
            ),
            ("check_player_name", "/auth/api/users/profiles/minecraft/Player"),
            ("figura_auth", "/figura/hasJoined?username=Player&serverId=s"),
        ]
        for symbol, path in cases:
            for value, status in [(PROFILE, 200), (False, 204)]:
                with self.subTest(symbol=symbol, status=status):
                    with patch.object(main, symbol, AsyncMock(return_value=value)):
                        response = self.client.get(path)
                    self.assertEqual(response.status_code, status)
                    if status == 204:
                        self.assertEqual(response.content, b"")
                    else:
                        self.assertEqual(response.json(), PROFILE)

    def test_protected_business_adapters(self):
        cases = [
            (
                "post",
                "/figura/register",
                "figura_reg",
                {"username": "Player", "passwd": "p", "repasswd": "p"},
                "操作成功",
            ),
            (
                "post",
                "/figura/refresh",
                "figura_refresh",
                {"username": "Player", "passwd": "p"},
                "操作成功",
            ),
            (
                "post",
                "/figura/server",
                "figura_server_sync",
                {"uuid": "0" * 32, "username": "Player"},
                "样本消息",
            ),
            ("get", "/offline/check/Player", "offline_check", None, "样本消息"),
            (
                "post",
                "/offline/reg",
                "offline_reg",
                {"username": "Player", "passwd": "p", "repasswd": "p"},
                "样本消息",
            ),
            (
                "post",
                "/offline/login",
                "offline_login",
                {"username": "Player", "passwd": "p"},
                "样本消息",
            ),
            (
                "post",
                "/offline/chpasswd",
                "offline_chpsswd",
                {"username": "Player", "passwd": "p", "newpasswd": "q"},
                "样本消息",
            ),
        ]
        for method, path, symbol, body, message in cases:
            for code in [200, 400, 403, 404]:
                with self.subTest(path=path, code=code):
                    with patch.object(
                        main,
                        symbol,
                        AsyncMock(return_value={"code": code, "msg": "样本消息"}),
                    ):
                        response = self.client.request(
                            method, path, json=body, headers=TOKEN
                        )
                    expected_code = (
                        400 if symbol == "figura_server_sync" and code != 200 else code
                    )
                    self.assertEqual(response.status_code, expected_code)
                    expected = (
                        {"message": message}
                        if code == 200
                        else {"detail": {"message": "样本消息"}}
                    )
                    self.assertEqual(response.json(), expected)

    def test_profile_mutation_adapters(self):
        for path, symbol, body in [
            (
                "/profile/linkprofile",
                "link_server_profile",
                {"uuid": "a", "new_uuid": "b"},
            ),
            (
                "/profile/rename",
                "server_player_rename",
                {"uuid": "a", "username": "Player"},
            ),
        ]:
            for result, status in [(True, 200), (False, 400)]:
                with patch.object(main, symbol, AsyncMock(return_value=result)):
                    self.assertEqual(
                        self.client.post(path, json=body, headers=TOKEN).status_code,
                        status,
                    )
        with patch.object(main, "server_player_rstname", AsyncMock()):
            self.assertEqual(
                self.client.get("/profile/rstname/a", headers=TOKEN).json(),
                {"message": "操作成功"},
            )

    def test_auth_validation_and_cors(self):
        for headers in [{}, {"Authorization": "Basic token"}]:
            response = self.client.get("/check_token", headers=headers)
            self.assertEqual(response.status_code, 401)
            self.assertEqual(response.json(), {"detail": "Not authenticated"})
        response = self.client.get(
            "/check_token", headers={"Authorization": "Bearer wrong"}
        )
        self.assertEqual(response.status_code, 401)
        self.assertEqual(response.headers["www+authenticate"], "Bearer")
        self.assertEqual(
            self.client.get("/check_token", headers=TOKEN).json(),
            {"message": "验证通过"},
        )
        response = self.client.post(
            "/offline/login", headers=TOKEN, json={"username": "Player"}
        )
        self.assertEqual(response.status_code, 422)
        self.assertEqual(response.json()["detail"][0]["loc"], ["body", "passwd"])
        self.assertEqual(
            self.client.get("/figura/hasJoined?username=Player").status_code, 422
        )
        response = self.client.options(
            "/offline/login",
            headers={
                "Origin": "https://example.com",
                "Access-Control-Request-Method": "POST",
            },
        )
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.headers["access-control-allow-origin"], "*")

    def test_skin_texture_and_head_routes(self):
        with patch.object(
            main,
            "create_csl_data",
            AsyncMock(return_value={"username": "Player", "skins": {"slim": "hash"}}),
        ) as call:
            self.assertEqual(
                self.client.get("/csl/Player.json").json()["skins"], {"slim": "hash"}
            )
            call.assert_awaited_once_with("Player")
        with patch.object(main, "create_csl_data", AsyncMock(return_value=False)):
            self.assertEqual(self.client.get("/csl/Player").status_code, 404)
        self.assertEqual(
            self.client.get("/textures/missing-baseline-texture").status_code, 404
        )
        texture = ROOT / "textures/baseline-test-image"
        texture.write_bytes((ROOT / "assets/alex.png").read_bytes())
        try:
            response = self.client.get("/textures/baseline-test-image")
            self.assertEqual(response.status_code, 200)
            self.assertEqual(response.headers["content-type"], "image/png")
        finally:
            texture.unlink()
        response = self.client.get("/head/!/16")
        self.assertEqual(response.status_code, 200)
        self.assertEqual(Image.open(io.BytesIO(response.content)).size, (16, 16))
        self.assertEqual(self.client.get("/head/!/invalid").status_code, 422)
        with patch.object(main, "get_player_head", AsyncMock(return_value=None)):
            self.assertEqual(self.client.get("/head/!/16").status_code, 404)

    def test_ip_and_optional_body_fields(self):
        with patch.object(
            main, "figura_reg", AsyncMock(return_value={"code": 200})
        ) as call:
            self.client.post(
                "/figura/register",
                headers={**TOKEN, "X-Forwarded-For": "192.0.2.1, 192.0.2.2"},
                json={
                    "username": "Player",
                    "passwd": "p",
                    "repasswd": "p",
                    "extra": True,
                },
            )
            self.assertEqual(call.await_args.args[-1], "192.0.2.1")
        with patch.object(
            main, "offline_reg", AsyncMock(return_value={"code": 200, "msg": "ok"})
        ) as call:
            self.client.post(
                "/offline/reg",
                headers=TOKEN,
                json={"username": "Player", "passwd": "p", "repasswd": "p"},
            )
            self.assertIsNone(call.await_args.args[-1])


class Algorithms(unittest.IsolatedAsyncioTestCase):
    async def test_source_urls_order_and_domains(self):
        extractor = tldextract.TLDExtract(suffix_list_urls=(), cache_dir=None)
        self.assertEqual(
            extractor("https://api.example.co.uk").registered_domain, "example.co.uk"
        )
        expected = {
            "mojang": "https://sessionserver.example.com/session/minecraft/hasJoined",
            "aihmc": "https://api.example.com/mcauth/sessionserver/session/minecraft/hasJoined",
            "blessingskin": "https://example.com/api/yggdrasil/sessionserver/session/minecraft/hasJoined",
            "elyby": "https://account.example.com/api/minecraft/session/hasJoined",
        }
        for source, url in expected.items():
            servers = [
                {
                    "name": "disabled",
                    "enabled": False,
                    "root_url": "https://example.com",
                    "api_type": source,
                },
                {"name": source, "root_url": "https://example.com", "api_type": source},
            ]
            with patch.object(settings, "servers", servers), patch.object(
                auth.tldextract, "extract", extractor
            ), patch.object(
                auth, "get_ygg_data", AsyncMock(return_value=PROFILE)
            ) as request, patch.object(
                auth, "save_player_data", AsyncMock()
            ), patch.object(
                auth, "final_profile", AsyncMock(return_value=PROFILE)
            ):
                self.assertEqual(await auth.ygg_auth("Player", "s", None), PROFILE)
                request.assert_awaited_once_with(url, "Player", "s", None)

    async def test_offline_disabled_and_figura_mismatch(self):
        with patch.object(settings, "allow_offline", False):
            for result in [
                await offline_auth.offline_reg("Player", "p", "p", None),
                await offline_auth.offline_login("Player", "p"),
                await offline_auth.offline_check("Player"),
                await offline_auth.offline_chpsswd("Player", "p", "q"),
            ]:
                self.assertEqual(result["code"], 400)
        self.assertEqual(
            (await figura.figura_reg("Player", "p", "q", None))["code"], 400
        )

    async def test_uuid_duration_and_identifiers(self):
        self.assertEqual(
            utils.generate_offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f"
        )
        self.assertNotEqual(
            utils.generate_offline_uuid("Player"), utils.generate_offline_uuid("player")
        )
        self.assertEqual(utils.parse_duration("7d 10h 50m 20s"), 643820)
        self.assertEqual(utils.parse_duration("bad"), 0)
        for value, kind in [("玩家", "USERNAME"), ("a" * 32, "UUID"), ("!", "UNKNOWN")]:
            self.assertEqual(utils.identify_identifier(value), kind)

    async def test_password_compatibility(self):
        hashed = utils.hash_password("密码-baseline")
        self.assertTrue(utils.check_password("密码-baseline", hashed))
        self.assertTrue(utils.check_password("密码-baseline", hashed.decode()))
        self.assertFalse(utils.check_password("wrong", hashed))
        long_hash = utils.hash_password("a" * 72 + "x")
        self.assertTrue(utils.check_password("a" * 72 + "y", long_hash))

    async def test_texture_parsing_and_pixels(self):
        payload = {
            "textures": {
                "SKIN": {
                    "url": "https://example.com/skin.png",
                    "metadata": {"model": "slim"},
                },
                "CAPE": {"url": "https://example.com/cape"},
            }
        }
        profile = {
            "properties": [
                {
                    "name": "textures",
                    "value": base64.b64encode(json.dumps(payload).encode()).decode(),
                }
            ]
        }
        self.assertEqual(skin.parse_mojang_textures(profile), ("skin", "cape", "slim"))
        self.assertEqual(
            skin.parse_csl_json({"skins": {"default": "skin"}, "cape": "cape"}),
            ("skin", "cape", "default"),
        )
        image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
        image.paste((200, 0, 0, 255), (8, 8, 16, 16))
        image.paste((0, 100, 0, 128), (40, 8, 48, 16))
        source = io.BytesIO()
        image.save(source, format="PNG")
        result = await head.process_skin_to_head(source.getvalue(), 16)
        decoded = Image.open(io.BytesIO(result)).convert("RGBA")
        self.assertEqual(decoded.getpixel((0, 0)), (100, 50, 0, 191))
        self.assertEqual(decoded.getpixel((15, 15)), (100, 50, 0, 191))

    async def test_figura_session_boundaries(self):
        for rows, ip, expected in [
            ([], "ip", "NOT_REG"),
            ([{"ip": "ip", "last_time": 100}], "other", "IP_INVALID"),
            ([{"ip": "ip", "last_time": 100}], "ip", "SUCCESS"),
            ([{"ip": "ip", "last_time": 99}], "ip", "SESSION_TIMEDOUT"),
        ]:
            with patch.object(
                figura.db, "query", AsyncMock(return_value=rows)
            ), patch.object(figura.time, "time", return_value=172900):
                self.assertEqual(
                    await figura.check_figura_session("Player", ip), expected
                )

    async def test_characterize_link_and_empty_skin(self):
        with patch.object(
            utils.db, "query", AsyncMock(return_value=[{"uuid": "a"}])
        ), patch.object(utils.db, "execute", AsyncMock(return_value=True)) as execute:
            self.assertIsNone(await utils.link_server_profile("a", "b"))
            execute.assert_awaited_once()
        with patch.object(skin.db, "query", AsyncMock(return_value=[])):
            with self.assertRaises(IndexError):
                await skin.read_skin_data("Player")


class LoopbackHTTP(unittest.TestCase):
    def test_python_application_starts_on_windows(self):
        with socket.socket() as reserved:
            reserved.bind(("127.0.0.1", 0))
            port = reserved.getsockname()[1]
        output = ROOT / "test-output"
        output.mkdir(exist_ok=True)
        with (output / "python-server.log").open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                [sys.executable, "-m", "tests.baseline_server", str(port)],
                cwd=ROOT,
                stdout=log,
                stderr=subprocess.STDOUT,
            )
            try:
                for _ in range(100):
                    if process.poll() is not None:
                        self.fail(
                            "Python server exited; inspect test-output/python-server.log"
                        )
                    try:
                        response = httpx.get(f"http://127.0.0.1:{port}/", timeout=1)
                        break
                    except httpx.TransportError:
                        time.sleep(0.1)
                else:
                    self.fail("Python server did not become ready")
                self.assertEqual(response.json(), {"message": "Hello World"})
                self.assertEqual(
                    httpx.get(
                        f"http://127.0.0.1:{port}/check_token", headers=TOKEN
                    ).status_code,
                    200,
                )
            finally:
                process.terminate()
                process.wait(timeout=10)

    def test_real_upstream_request_and_existing_ip_query(self):
        requests = []

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                requests.append(self.path)
                body = json.dumps(PROFILE).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            url = f"http://127.0.0.1:{server.server_port}/hasJoined"
            self.assertEqual(
                asyncio.run(auth.get_ygg_data(url, "Player", "s", None)), PROFILE
            )
            asyncio.run(auth.get_ygg_data(url, "Player", "s", "192.0.2.1"))
            self.assertEqual(
                requests,
                [
                    "/hasJoined?username=Player&serverId=s",
                    "/hasJoined?username=Player&serverId=s?ip=192.0.2.1",
                ],
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
