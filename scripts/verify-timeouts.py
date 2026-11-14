"""Compare Go/Rust HTTP header, whole-request read and write deadlines."""

import argparse
import http.client
import http.server
import importlib.util
import json
import pathlib
import re
import socket
import tempfile
import threading
import time
import tomllib
import urllib.parse

spec = importlib.util.spec_from_file_location(
    "contract", pathlib.Path(__file__).with_name("verify-contract.py")
)
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


def receive_closed(connection):
    connection.settimeout(2)
    data = bytearray()
    while True:
        try:
            chunk = connection.recv(4096)
        except ConnectionResetError:
            break
        if not chunk:
            break
        data.extend(chunk)
    return bytes(data)


def check(fixture):
    url = urllib.parse.urlsplit(fixture.url)
    address = (url.hostname, url.port)
    with socket.create_connection(address, timeout=2) as connection:
        connection.sendall(b"GET / HTTP/1.1\r\nHost: fixture\r\n")
        time.sleep(0.25)
        assert (
            receive_closed(connection) == b""
        ), "partial header exceeded header deadline"
    body = json.dumps(contract.login_body()).encode()
    header = (
        "POST /mcauth/authserver/authenticate HTTP/1.1\r\n"
        f"Host: fixture\r\nContent-Length: {len(body)}\r\n\r\n"
    ).encode()
    with socket.create_connection(address, timeout=2) as connection:
        connection.sendall(header[:-2])
        time.sleep(0.15)
        connection.sendall(header[-2:] + body[:1])
        time.sleep(0.20)
        data = receive_closed(connection)
        assert not data or data.startswith(
            b"HTTP/1.1 400"
        ), "whole-request read deadline was reset after headers"
    client = http.client.HTTPConnection(*address, timeout=2)
    try:
        client.request("GET", "/")
        response = client.getresponse()
        assert response.status == 200
        response.read()
        time.sleep(0.25)
        client.request("GET", "/")
        response = client.getresponse()
        assert response.status == 200, "keep-alive idle wait used header deadline"
        response.read()
    finally:
        client.close()
    return {
        "slow_header": "PASS",
        "whole_request_read_budget": "PASS",
        "keep_alive_idle_budget": "PASS",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["config", "go", "rust", "mysql", "output"]:
        parser.add_argument("--" + name, required=True, type=pathlib.Path)
    args = parser.parse_args()
    path = args.config.resolve()
    text = path.read_text(encoding="utf-8-sig")
    config = tomllib.loads(text)
    base = path.parent
    with tempfile.TemporaryDirectory(prefix="r3-timeouts-", dir=base) as temporary:
        directory = pathlib.Path(temporary)
        database = contract.Database(args.mysql.resolve(), config, base, directory)
        parent = contract.upstream_handler(config, base)

        class SlowUpstream(parent):
            def do_POST(self):
                time.sleep(0.04)
                super().do_POST()

        class Upstream(http.server.ThreadingHTTPServer):
            request_queue_size = 64

        upstream = Upstream(("127.0.0.1", 0), SlowUpstream)
        thread = threading.Thread(target=upstream.serve_forever, daemon=True)
        thread.start()
        fixture_config = base / (directory.name + ".toml")
        text = re.sub(r"(?m)^listen\s*=.*$", 'listen = "127.0.0.1:0"', text)
        text = re.sub(
            r"(?m)^endpoint\s*=.*$",
            f'endpoint = "http://127.0.0.1:{upstream.server_port}"',
            text,
        )
        for name, value in {
            "read_header_timeout": "200ms",
            "read_timeout": "300ms",
            "write_timeout": "3s",
        }.items():
            text = re.sub(rf"(?m)^{name}\s*=.*$", f'{name} = "{value}"', text)
        fixture_config.write_text(text, encoding="utf-8")
        results = {}
        try:
            for name, command in [
                ("go", [str(args.go.resolve()), "--config", str(fixture_config)]),
                ("rust", [str(args.rust.resolve()), str(fixture_config)]),
            ]:
                fixture = contract.Fixture(command)
                try:
                    results[name] = check(fixture)
                    print(
                        f"{name}: header/read/keep-alive deadlines passed", flush=True
                    )
                finally:
                    fixture.close()
            # All four UCenter requests fit its 10s timeout, but exceed write_timeout.
            short = re.sub(r"(?m)^write_timeout\s*=.*$", 'write_timeout = "50ms"', text)
            fixture_config.write_text(short, encoding="utf-8")
            for name, command in [
                ("go", [str(args.go.resolve()), "--config", str(fixture_config)]),
                ("rust", [str(args.rust.resolve()), str(fixture_config)]),
            ]:
                database.clean()
                fixture = contract.Fixture(command)
                try:
                    url = urllib.parse.urlsplit(fixture.url)
                    with socket.create_connection(
                        (url.hostname, url.port), timeout=2
                    ) as connection:
                        body = json.dumps(contract.login_body()).encode()
                        connection.sendall(
                            (
                                "POST /mcauth/authserver/authenticate HTTP/1.1\r\n"
                                f"Host: fixture\r\nContent-Length: {len(body)}\r\n\r\n"
                            ).encode()
                            + body
                        )
                        assert (
                            receive_closed(connection) == b""
                        ), "response was written after write deadline"
                    for table in [
                        "mcauth_profiles",
                        "mcauth_tokens",
                        "mcauth_textures",
                    ]:
                        assert (
                            database.query(
                                f"SELECT COUNT(*) FROM {table} WHERE uid=970001"
                            )
                            == "1"
                        ), "write deadline cancelled the transaction before response write"
                    results[name]["write_deadline"] = "PASS"
                    results[name]["write_deadline_preserves_committed_state"] = "PASS"
                finally:
                    fixture.close()
            assert results["go"] == results["rust"]
            args.output.write_text(json.dumps(results, indent=2), encoding="utf-8")
            print(json.dumps(results))
        finally:
            database.clean()
            fixture_config.unlink(missing_ok=True)
            upstream.shutdown()
            upstream.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    main()
