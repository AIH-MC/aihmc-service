"""Compare real Go/Rust HTTP handlers against one isolated database, serially.

The two fixture executables use a controlled material provider. Texture download,
image processing and production executable wiring are verified in R4/R5.
"""

import argparse
import base64
import concurrent.futures
import hashlib
import http.server
import json
import pathlib
import queue
import re
import subprocess
import tempfile
import threading
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request


def parse_arguments():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=pathlib.Path)
    parser.add_argument("--go", required=True, type=pathlib.Path)
    parser.add_argument("--rust", required=True, type=pathlib.Path)
    parser.add_argument("--bridge", required=True, type=pathlib.Path)
    parser.add_argument("--mysql", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--concurrency-only", action="store_true")
    return parser.parse_args()


class Database:
    def __init__(self, executable, config, base, directory):
        database = config["database"]
        assert database["name"] == "aihmc_migration_test", "refusing business database"
        password_file = base / database["password_file"]
        password = password_file.read_text(encoding="utf-8").rstrip("\r\n")
        self.options = directory / "mysql-client.ini"
        lines = ["[client]"]
        for key, value in {
            "host": database["host"],
            "port": database["port"],
            "user": database["user"],
            "password": password,
        }.items():
            escaped = str(value).replace("\\", "\\\\").replace('"', '\\"')
            escaped = escaped.replace("\n", "\\n").replace("\r", "\\r")
            lines.append(f'{key}="{escaped}"')
        self.options.write_text("\n".join(lines), encoding="utf-8")
        self.command = [
            str(executable),
            f"--defaults-extra-file={self.options}",
            "--default-character-set=utf8mb4",
            "--database=aihmc_migration_test",
            "--batch",
            "--raw",
            "--skip-column-names",
        ]
        assert self.query("SELECT DATABASE()") == "aihmc_migration_test"

    def query(self, sql):
        result = subprocess.run(
            self.command,
            input=sql,
            text=True,
            encoding="utf-8",
            capture_output=True,
            timeout=20,
        )
        if result.returncode:
            raise AssertionError("isolated SQL command failed")
        return result.stdout.rstrip("\r\n")

    def clean(self):
        self.query("""
            DELETE FROM mcauth_logins WHERE server_id LIKE 'migration-contract-%';
            DELETE FROM mcauth_tokens WHERE uid BETWEEN 970001 AND 970003;
            DELETE FROM mcauth_profiles WHERE uid BETWEEN 970001 AND 970003;
            DELETE FROM mcauth_textures WHERE uid BETWEEN 970001 AND 970003;
        """)

    def snapshot(self):
        statements = {
            "profiles": """SELECT JSON_OBJECT('uid',uid,'name',player_name,
                'profileId',profile_id,'uuid',player_uuid,'texture_data',texture_data)
                FROM mcauth_profiles WHERE uid BETWEEN 970001 AND 970003 ORDER BY uid""",
            "tokens": """SELECT JSON_OBJECT('uid',uid,'name',player_name,
                'accessToken',access_token,'clientToken',client_token,'create_time',create_time)
                FROM mcauth_tokens WHERE uid BETWEEN 970001 AND 970003 ORDER BY uid""",
            "textures": """SELECT JSON_OBJECT('uid',uid,'uuid',uuid,'name',player_name,
                'skin',skin_hash,'model',skin_type,'cape',cape_hash,'timestamps',timestamps)
                FROM mcauth_textures WHERE uid BETWEEN 970001 AND 970003 ORDER BY uid""",
            "joins": """SELECT JSON_OBJECT('server',server_id,'accessToken',access_token,
                'name',playername,'ip',ip,'timestamp',timestamp) FROM mcauth_logins
                WHERE server_id LIKE 'migration-contract-%' ORDER BY server_id""",
        }
        return {
            table: [json.loads(line) for line in self.query(sql).splitlines()]
            for table, sql in statements.items()
        }


def upstream_handler(config, base):
    secret = (
        (base / config["ucenter"]["secret_file"])
        .read_text(encoding="utf-8")
        .rstrip("\r\n")
    )

    class Upstream(http.server.BaseHTTPRequestHandler):
        def log_message(self, *_):
            pass

        def do_POST(self):
            timestamp = self.headers["t"]
            source = config["ucenter"]["nonce"] + timestamp + secret
            expected = base64.b64encode(
                hashlib.sha256(source.encode()).hexdigest().encode()
            ).decode()
            assert self.headers["sign"] == expected
            assert self.headers["appid"] == config["ucenter"]["app_id"]
            body = json.loads(self.rfile.read(int(self.headers["content-length"])))
            if self.path == "/token":
                response = {"token": "fixture-token"}
            elif self.path == "/user/login":
                response = {
                    "data": {
                        "status": 1 if body["password"] == "fixture-password" else -1,
                        "username": body["username"],
                    }
                }
            elif self.path == "/user/get_user":
                users = {
                    "MigrationHTTP": 970001,
                    "迁移用户": 970002,
                    "ConcurrentFirst": 970003,
                }
                response = {
                    "data": {
                        "uid": str(users[body["username"]]),
                        "username": body["username"],
                        "email": "fixture@example.com",
                    }
                }
            else:
                self.send_error(404)
                return
            encoded = json.dumps(response, ensure_ascii=False).encode()
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(encoded)))
            self.end_headers()
            self.wfile.write(encoded)

    return Upstream


class Fixture:
    def __init__(self, command):
        self.name = pathlib.Path(command[0]).name
        self.process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
        )
        lines = queue.Queue()
        threading.Thread(
            target=lambda: lines.put(self.process.stdout.readline()), daemon=True
        ).start()
        try:
            line = lines.get(timeout=15).strip()
            assert line.startswith("fixture listening "), "fixture did not start"
            self.url = "http://" + line.removeprefix("fixture listening ")
        except Exception:
            self.close()
            raise

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=10)
        self.process.stdout.close()
        diagnostic = self.process.stderr.read()
        self.process.stderr.close()
        if diagnostic:
            print(f"{self.name}: {diagnostic.strip()}", flush=True)


def request(fixture, method, path, body=None, expected=200, headers=None, raw=None):
    payload = (
        raw
        if raw is not None
        else (None if body is None else json.dumps(body, ensure_ascii=False).encode())
    )
    outgoing = urllib.request.Request(
        fixture.url + path, data=payload, method=method, headers=headers or {}
    )
    try:
        response = urllib.request.urlopen(outgoing, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        data = response.read()
        assert (
            response.status == expected
        ), f"{fixture.name} {method} {path}: expected {expected}, got {response.status}"
        assert response.headers["content-type"] == "application/json; charset=utf-8"
        if expected == 204:
            assert data == b"", "204 must have no body"
            parsed = None
        else:
            parsed = json.loads(data)
        return {"status": response.status, "body": parsed}


def login_body(name="MigrationHTTP", client="contract-client", request_user=False):
    return {
        "username": name,
        "password": "fixture-password",
        "clientToken": client,
        "requestUser": request_user,
        "agent": {"name": "minecraft"},
    }


def token_body(login):
    return {key: login[key] for key in ["accessToken", "clientToken"]}


class Sequence:
    def __init__(self, fixture, database, bridge, config):
        self.fixture = fixture
        self.database = database
        self.bridge = bridge
        self.config = config
        self.records = []
        self.replacements = {}
        self.started = int(time.time())

    def send(self, label, method, path, body=None, expected=200, **kwargs):
        response = request(self.fixture, method, path, body, expected, **kwargs)
        self.records.append({"case": label, **response})
        return response["body"]

    def capture_login(self, login):
        name = login["selectedProfile"]["name"]
        self.replacements[login["selectedProfile"]["id"]] = f"<uuid:{name}>"
        self.replacements.setdefault(
            login["accessToken"], f"<access:{len(self.replacements)}>"
        )
        if len(login["clientToken"]) == 32:
            self.replacements[login["clientToken"]] = "<generated-client>"

    def verify_signature(self, profile):
        property = profile["properties"][0]
        assert property["name"] == "textures" and property["signature"]
        check = subprocess.run(
            [str(self.bridge), "--config", str(self.config)],
            input=json.dumps(
                {"value": property["value"], "signature": property["signature"]}
            ),
            text=True,
            capture_output=True,
            timeout=10,
        )
        assert check.returncode == 0, "HTTP profile signature failed Go verification"
        assert json.loads(check.stdout)["verified"] is True

    def normalize(self, value, key=""):
        if (
            key in {"timestamp", "timestamps", "create_time"}
            and isinstance(value, int)
            and value >= self.started - 60
        ):
            return "<current-time>"
        if key in {"value", "texture_data"} and isinstance(value, str):
            return self.normalize(json.loads(base64.b64decode(value)))
        if key == "signature":
            return "<verified-signature>" if value else value
        if isinstance(value, dict):
            return {name: self.normalize(item, name) for name, item in value.items()}
        if isinstance(value, list):
            return [self.normalize(item) for item in value]
        if isinstance(value, str):
            return self.replacements.get(value, value)
        return value

    def run(self):
        self.send("root", "GET", "/")
        self.send("root-post", "POST", "/", {})
        for path in ["/mcauth", "/unknown", "/mcauth?x=1", "/mcauth/"]:
            metadata = self.send("metadata:" + path, "GET", path)
            assert "signaturePublickey" in metadata
        self.send("metadata-method", "POST", "/mcauth", {}, 405)
        auth = "/mcauth/authserver/"
        session = "/mcauth/sessionserver/session/minecraft/"
        self.send("invalid-json", "POST", auth + "authenticate", expected=400, raw=b"{")
        self.send(
            "trailing-json", "POST", auth + "authenticate", expected=400, raw=b"{} {}"
        )
        self.send("missing-account", "POST", auth + "authenticate", {}, 400)
        self.send(
            "wrong-agent",
            "POST",
            auth + "authenticate",
            {**login_body(), "agent": {"name": "other"}},
            403,
        )
        self.send(
            "unicode-agent",
            "POST",
            auth + "authenticate",
            {**login_body(), "agent": {"name": "MİNECRAFT"}},
            403,
        )
        self.send(
            "wrong-password",
            "POST",
            auth + "authenticate",
            {**login_body(), "password": "wrong"},
            403,
        )
        login = self.send("authenticate", "POST", auth + "authenticate", login_body())
        self.capture_login(login)
        assert "user" not in login and len(login["availableProfiles"]) == 1
        repeated = self.send(
            "repeated-login",
            "POST",
            auth + "authenticate",
            login_body(request_user=True),
        )
        assert repeated["accessToken"] == login["accessToken"]
        assert repeated["selectedProfile"] == login["selectedProfile"]
        assert repeated["user"]["properties"] == []
        self.send("validate", "POST", auth + "validate", token_body(login), 204)
        profile_path = session + "profile/" + login["selectedProfile"]["id"]
        for suffix in ["", "?unsigned=0", "?unsigned=false&unsigned=0"]:
            profile = self.send("profile:" + suffix, "GET", profile_path + suffix)
            self.verify_signature(profile)
        unsigned = self.send("unsigned-false", "GET", profile_path + "?unsigned=false")
        assert "signature" not in unsigned["properties"][0]
        for prefix in [
            "/mcauth/users/profiles/minecraft/",
            "/mcauth/api/users/profiles/minecraft/",
        ]:
            self.send("name:" + prefix, "GET", prefix + "MigrationHTTP")
        self.send(
            "missing-name",
            "GET",
            "/mcauth/users/profiles/minecraft/Unknown",
            expected=404,
        )
        self.send(
            "missing-profile", "GET", session + "profile/" + "a" * 32, expected=204
        )
        batch = self.send(
            "batch",
            "POST",
            "/mcauth/api/profiles/minecraft",
            ["MigrationHTTP", "Unknown", "migrationhttp"],
        )
        assert len(batch) == 2 and batch[1]["name"] == "migrationhttp"
        self.send("null-batch", "POST", "/mcauth/api/profiles/minecraft", raw=b"null")
        join = {
            "serverId": "migration-contract-main",
            "accessToken": login["accessToken"],
            "selectedProfile": login["selectedProfile"]["id"],
        }
        self.send(
            "wrong-profile-join",
            "POST",
            session + "join",
            {**join, "selectedProfile": "wrong"},
            403,
        )
        self.send(
            "join",
            "POST",
            session + "join",
            join,
            204,
            headers={"X-Real-IP": "198.51.100.9"},
        )
        self.send("duplicate-join", "POST", session + "join", join, 500)
        joined = self.send(
            "has-joined-other-ip",
            "GET",
            session
            + "hasJoined?username=MigrationHTTP&serverId=migration-contract-main&ip=203.0.113.99",
        )
        self.verify_signature(joined)
        self.send(
            "missing-join",
            "GET",
            session
            + "hasJoined?username=MigrationHTTP&serverId=migration-contract-missing",
            expected=204,
        )
        self.database.query(
            """INSERT INTO mcauth_logins VALUES
            ('migration-contract-expired','fixture-expired','MigrationHTTP','127.0.0.1',0)"""
        )
        self.send(
            "expired-first-read",
            "GET",
            session
            + "hasJoined?username=MigrationHTTP&serverId=migration-contract-expired",
        )
        self.send(
            "expired-cleared",
            "GET",
            session
            + "hasJoined?username=MigrationHTTP&serverId=migration-contract-expired",
            expected=204,
        )
        refreshed = self.send(
            "refresh",
            "POST",
            auth + "refresh",
            {**token_body(login), "requestUser": "false"},
        )
        self.capture_login(refreshed)
        assert "availableProfiles" not in refreshed and "user" in refreshed
        self.send("old-token", "POST", auth + "validate", token_body(login), 403)
        self.send("new-token", "POST", auth + "validate", token_body(refreshed), 204)
        self.send("invalidate", "POST", auth + "invalidate", token_body(refreshed), 204)
        self.send(
            "invalidate-repeat", "POST", auth + "invalidate", token_body(refreshed), 204
        )
        self.send(
            "invalidated-token", "POST", auth + "validate", token_body(refreshed), 403
        )
        new_login = self.send(
            "generated-client", "POST", auth + "authenticate", login_body(client="")
        )
        self.capture_login(new_login)
        assert len(new_login["clientToken"]) == 32
        assert new_login["selectedProfile"] == login["selectedProfile"]
        self.send(
            "signout-wrong",
            "POST",
            auth + "signout",
            {"username": "MigrationHTTP", "password": "wrong"},
            403,
        )
        self.send(
            "signout",
            "POST",
            auth + "signout",
            {"username": "MigrationHTTP", "password": "fixture-password"},
            204,
        )
        self.send(
            "signed-out-token", "POST", auth + "validate", token_body(new_login), 403
        )
        chinese = self.send(
            "chinese-login",
            "POST",
            auth + "authenticate",
            login_body("迁移用户", "chinese-client", True),
        )
        self.capture_login(chinese)
        queried = self.send(
            "chinese-name",
            "GET",
            "/mcauth/users/profiles/minecraft/" + urllib.parse.quote("迁移用户"),
        )
        assert queried["id"] == chinese["selectedProfile"]["id"]
        snapshots = self.normalize(self.database.snapshot())
        return {"responses": self.normalize(self.records), "database": snapshots}


def run_concurrency(fixture, database):
    auth = "/mcauth/authserver/"
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as executor:
        logins = list(
            executor.map(
                lambda _: request(
                    fixture,
                    "POST",
                    auth + "authenticate",
                    login_body("ConcurrentFirst", "concurrent-client"),
                )["body"],
                range(8),
            )
        )
    assert len({login["selectedProfile"]["id"] for login in logins}) == 1
    for table in ["mcauth_profiles", "mcauth_tokens", "mcauth_textures"]:
        assert database.query(f"SELECT COUNT(*) FROM {table} WHERE uid=970003") == "1"
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        results = list(
            executor.map(
                lambda _: concurrent_refresh(fixture, token_body(logins[0])), range(2)
            )
        )
    assert sorted(results) == [200, 403], results


def concurrent_refresh(fixture, token):
    outgoing = urllib.request.Request(
        fixture.url + "/mcauth/authserver/refresh",
        data=json.dumps(token).encode(),
        method="POST",
    )
    try:
        response = urllib.request.urlopen(outgoing, timeout=15)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        response.read()
        return response.status


def main():
    args = parse_arguments()
    config_path = args.config.resolve()
    text = config_path.read_text(encoding="utf-8-sig")
    config = tomllib.loads(text)
    base = config_path.parent
    with tempfile.TemporaryDirectory(prefix="r3-contract-", dir=base) as temporary:
        directory = pathlib.Path(temporary)
        database = Database(args.mysql.resolve(), config, base, directory)

        class FixtureUpstream(http.server.ThreadingHTTPServer):
            # Eight simultaneous logins each perform four separate HTTP requests.
            # The default Windows backlog of five rejects part of this test load.
            request_queue_size = 64

        upstream = FixtureUpstream(("127.0.0.1", 0), upstream_handler(config, base))
        thread = threading.Thread(target=upstream.serve_forever, daemon=True)
        thread.start()
        # Keep fixture config next to the original so all relative secret paths retain meaning.
        fixture_config = base / (directory.name + ".toml")
        fixture_text = re.sub(r"(?m)^listen\s*=.*$", 'listen = "127.0.0.1:0"', text)
        fixture_text = re.sub(
            r"(?m)^endpoint\s*=.*$",
            f'endpoint = "http://127.0.0.1:{upstream.server_port}"',
            fixture_text,
        )
        fixture_config.write_text(fixture_text, encoding="utf-8")
        fixtures = []
        try:
            go = Fixture([str(args.go.resolve()), "--config", str(fixture_config)])
            fixtures.append(go)
            rust = Fixture([str(args.rust.resolve()), str(fixture_config)])
            fixtures.append(rust)
            if args.concurrency_only:
                for fixture in fixtures:
                    database.clean()
                    run_concurrency(fixture, database)
                    print(f"{fixture.name}: isolated concurrency passed", flush=True)
                return
            results = []
            for fixture in [go, rust]:
                database.clean()
                results.append(
                    Sequence(
                        fixture, database, args.bridge.resolve(), fixture_config
                    ).run()
                )
                print(
                    f"{fixture.name}: {len(results[-1]['responses'])} sequential HTTP cases passed",
                    flush=True,
                )
                run_concurrency(fixture, database)
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(
                json.dumps(
                    {"go": results[0], "rust": results[1]}, ensure_ascii=False, indent=2
                ),
                encoding="utf-8",
            )
            assert (
                results[0] == results[1]
            ), "Go/Rust HTTP or database contract differs; inspect comparison output"
            # Do not reset state here: verify each runtime reads the other's live token writes.
            from_go = request(
                go, "POST", "/mcauth/authserver/authenticate", login_body()
            )["body"]
            request(
                rust, "POST", "/mcauth/authserver/validate", token_body(from_go), 204
            )
            from_rust = request(
                rust, "POST", "/mcauth/authserver/refresh", token_body(from_go)
            )["body"]
            request(
                go, "POST", "/mcauth/authserver/validate", token_body(from_rust), 204
            )
            # A second refresh in the same second can reproduce the legacy refreshed token.
            time.sleep(1.05)
            back_to_go = request(
                go, "POST", "/mcauth/authserver/refresh", token_body(from_rust)
            )["body"]
            request(
                rust, "POST", "/mcauth/authserver/validate", token_body(back_to_go), 204
            )
            rust.close()
            fixtures.remove(rust)
            rust = Fixture([str(args.rust.resolve()), str(fixture_config)])
            fixtures.append(rust)
            request(
                rust, "POST", "/mcauth/authserver/validate", token_body(back_to_go), 204
            )
            chinese_path = "/mcauth/users/profiles/minecraft/" + urllib.parse.quote(
                "迁移用户"
            )
            assert request(go, "GET", chinese_path) == request(
                rust, "GET", chinese_path
            )
            result = {
                "stage": "R3",
                "platform": "windows-x64",
                "result": "PASS",
                "http_cases_per_runtime": len(results[0]["responses"]),
                "normalized_http_and_database_equal": True,
                "eight_concurrent_first_logins_each_runtime": "PASS",
                "concurrent_refresh_each_runtime": "PASS",
                "go_rust_live_token_interoperability": "PASS",
                "restarted_rust_process_reads_existing_tokens_and_identity": "PASS",
                "scope": "real HTTP handlers + real isolated DB + controlled UCenter and materials",
                "production_material_and_binary_wiring": "R4 pending",
                "comparison": results[1],
            }
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(
                json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8"
            )
            print(
                json.dumps(
                    {
                        key: value
                        for key, value in result.items()
                        if key != "comparison"
                    },
                    ensure_ascii=False,
                )
            )
        finally:
            for fixture in fixtures:
                fixture.close()
            database.clean()
            fixture_config.unlink(missing_ok=True)
            upstream.shutdown()
            upstream.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    main()
