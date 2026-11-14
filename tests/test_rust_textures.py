import base64
import io
import json
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse

from PIL import Image

from tests.rust_support import IsolatedDatabase, ROOT, RustService


def expected_head(source_bytes, size):
    image = Image.open(io.BytesIO(source_bytes)).convert("RGBA")
    result = image.crop((8, 8, 16, 16))
    if image.width >= 64:
        overlay = image.crop((40, 8, 48, 16))
        result.paste(overlay, (0, 0), overlay)
    return result.resize((size, size), Image.Resampling.NEAREST)


class RustTexturesHTTP(unittest.TestCase):
    def setUp(self):
        self.database = IsolatedDatabase()
        self.requests = []
        self.fail_download = False
        image = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
        for y in range(8):
            for x in range(8):
                image.putpixel((x + 8, y + 8), (x * 31, y * 31, 73, 255))
                image.putpixel((x + 40, y + 8), (17, 201, 29, (x * 37 + y * 53) % 256))
        buffer = io.BytesIO()
        image.save(buffer, format="PNG")
        self.png = buffer.getvalue()
        fixture = self

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                fixture.requests.append(self.path)
                path = urlparse(self.path).path
                status = 200
                content_type = "application/json"
                if path.startswith(("/textures/", "/texture/", "/image/", "/storage/")):
                    status = 404 if fixture.fail_download else 200
                    payload = fixture.png
                    content_type = "image/png"
                elif path.startswith("/csl/"):
                    if "Missing" in path:
                        status = 204
                    payload = {
                        "username": "Player",
                        "skins": {"slim": "skin"},
                        "cape": "cape",
                    }
                    if "CapeOnly" in path:
                        payload["skins"] = {}
                else:
                    value = base64.b64encode(
                        json.dumps(
                            {
                                "textures": {
                                    "SKIN": {
                                        "url": "http://textures.example.com/image/skin",
                                        "metadata": {"model": "slim"},
                                    },
                                    "CAPE": {
                                        "url": "http://textures.example.com/image/cape"
                                    },
                                }
                            }
                        ).encode()
                    ).decode()
                    payload = {
                        "id": "a" * 32,
                        "name": "Player",
                        "properties": [
                            {
                                "name": "textures",
                                "value": value,
                                "signature": "signature",
                            }
                        ],
                    }
                body = (
                    payload
                    if isinstance(payload, bytes)
                    else json.dumps(payload).encode()
                )
                self.send_response(status)
                self.send_header("Content-Type", content_type)
                self.send_header(
                    "Content-Length", str(len(body)) if status != 204 else "0"
                )
                self.end_headers()
                if status != 204:
                    self.wfile.write(body)

            def log_message(self, *args):
                pass

        self.proxy = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.thread = threading.Thread(target=self.proxy.serve_forever, daemon=True)
        self.thread.start()
        self.services = []

    def tearDown(self):
        for service in self.services:
            service.close()
        self.database.close()
        self.proxy.shutdown()
        self.proxy.server_close()
        self.thread.join()

    def service(self, kind):
        service = RustService(
            self.database,
            [
                {
                    "name": kind,
                    "api_type": kind,
                    "root_url": "http://example.com",
                    "enabled": True,
                }
            ],
            f"http://127.0.0.1:{self.proxy.server_port}",
        )
        self.services.append(service)
        return service

    def test_csl_cache_and_material_fetch(self):
        for kind in ["aihmc", "blessingskin"]:
            with self.subTest(kind=kind):
                service = self.service(kind)
                client = service.client
                self.assertEqual(
                    client.get("/csl/Player.json").json(),
                    {"username": "Player", "skins": {"slim": "skin"}, "cape": "cape"},
                )
                for texture in ["skin", "cape"]:
                    response = client.get(f"/textures/{texture}")
                    self.assertEqual(response.status_code, 200)
                    self.assertEqual(response.headers["content-type"], "image/png")
                    self.assertEqual(response.content, self.png)
                downloads = len(
                    [path for path in self.requests if "/textures/" in path]
                )
                self.assertEqual(client.get("/csl/Player").status_code, 200)
                self.assertEqual(
                    len([path for path in self.requests if "/textures/" in path]),
                    downloads,
                )
                self.assertEqual(client.get("/textures/missing").status_code, 404)
                self.assertEqual(client.get("/csl/Missing").status_code, 404)
                self.assertEqual(
                    client.get("/csl/CapeOnly").json()["skins"], {"default": None}
                )

    def test_mojang_legacy_download_behavior_and_elyby(self):
        service = self.service("mojang")
        response = service.client.get("/csl/Player")
        self.assertEqual(response.status_code, 200)
        self.assertEqual(response.json()["skins"], {"slim": "skin"})
        self.assertEqual(service.client.get("/textures/skin").status_code, 404)
        self.requests.clear()
        service = self.service("elyby")
        self.assertEqual(service.client.get("/csl/Player").status_code, 200)
        self.assertEqual(service.client.get("/textures/skin").content, self.png)
        self.assertTrue(
            any("textures.minecraft.net/texture/skin" in path for path in self.requests)
        )

    def test_remote_avatar_pixels_identifiers_and_sizes(self):
        service = self.service("mojang")
        for account, size in [
            ("Player", 13),
            ("a" * 32, 8),
            ("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", 64),
        ]:
            response = service.client.get(f"/head/{account}/{size}")
            self.assertEqual(
                response.status_code,
                200,
                response.text[:80] if response.status_code != 200 else "",
            )
            image = Image.open(io.BytesIO(response.content)).convert("RGBA")
            self.assertEqual(image.size, (size, size))
            self.assertEqual(image.tobytes(), expected_head(self.png, size).tobytes())
        self.assertEqual(service.client.get("/head/Player/invalid").status_code, 422)
        self.assertEqual(service.client.get("/head/Player/0").status_code, 404)

    def test_default_avatar_and_remote_download_failure(self):
        service = self.service("mojang")
        default_bytes = (ROOT / "assets/alex.png").read_bytes()
        for account in ["!", "Player"]:
            self.fail_download = True
            response = service.client.get(f"/head/{account}/16")
            self.assertEqual(response.status_code, 200)
            decoded = Image.open(io.BytesIO(response.content)).convert("RGBA")
            self.assertEqual(
                decoded.tobytes(), expected_head(default_bytes, 16).tobytes()
            )
        (service.directory / "assets/alex.png").unlink()
        self.assertEqual(service.client.get("/head/!/16").status_code, 404)


if __name__ == "__main__":
    unittest.main()
