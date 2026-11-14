"""Real MariaDB characterization in a uniquely named disposable database."""

import re
import os
import tomllib
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch

import aiomysql
import yaml

from tests.baseline_support import ROOT, settings
from libs import auth, database, figura, offline_auth, skin, utils


class MySQLBaseline(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        config_path = Path(os.environ.get("AIHMC_TEST_CONFIG", ROOT / "config.toml"))
        if not config_path.exists():
            config_path = ROOT / "config.yml"
        if not config_path.exists():
            self.skipTest(
                "Existing MySQL connection not configured in ignored config.yml"
            )
        text = config_path.read_text(encoding="utf-8")
        data = (
            tomllib.loads(text)
            if config_path.suffix == ".toml"
            else yaml.safe_load(text)
        )
        config = data["database"]
        self.connection = await aiomysql.connect(
            host=config["host"],
            port=config["port"],
            user=config["user"],
            password=str(config["password"]),
            autocommit=True,
        )
        self.db_name = "aihmc_migration_test_" + uuid.uuid4().hex
        self.old_config = settings.db
        settings.db = {**config, "db_name": self.db_name}
        self.db = database.Database()
        self.patches = []
        try:
            async with self.connection.cursor() as cursor:
                await cursor.execute(
                    f"CREATE DATABASE `{self.db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
                )
                await cursor.execute(f"USE `{self.db_name}`")
                schema = (ROOT / "database.sql").read_text(encoding="utf-8")
                for statement in re.findall(
                    r"CREATE TABLE IF NOT EXISTS.*?;", schema, re.DOTALL
                ):
                    await cursor.execute(statement)
            for module in [auth, figura, offline_auth, skin, utils]:
                replacement = patch.object(module, "db", self.db)
                replacement.start()
                self.patches.append(replacement)
        except BaseException:
            await self.asyncTearDown()
            raise

    async def asyncTearDown(self):
        for replacement in reversed(self.patches):
            replacement.stop()
        if self.db.pool:
            self.db.pool.close()
            await self.db.pool.wait_closed()
        try:
            async with self.connection.cursor() as cursor:
                # Only the random database created by this test may be removed.
                self.assertRegex(self.db_name, r"^aihmc_migration_test_[0-9a-f]{32}$")
                await cursor.execute(f"DROP DATABASE IF EXISTS `{self.db_name}`")
        finally:
            self.connection.close()
            settings.db = self.old_config

    async def test_account_profile_and_skin_round_trip(self):
        result = await offline_auth.offline_reg("Baseline", "密码", "密码", "192.0.2.1")
        self.assertEqual(result["code"], 200)
        self.assertEqual(
            (await offline_auth.offline_reg("Baseline", "密码", "密码", None))["code"],
            400,
        )
        self.assertEqual(
            (await offline_auth.offline_login("Baseline", "密码"))["code"], 200
        )
        self.assertEqual(
            (await offline_auth.offline_login("Baseline", "bad"))["code"], 403
        )
        self.assertEqual((await offline_auth.offline_check("Missing"))["code"], 404)
        self.assertEqual(
            (await offline_auth.offline_chpsswd("Baseline", "密码", "new"))["code"], 200
        )
        self.assertEqual(
            (await offline_auth.offline_login("Baseline", "new"))["code"], 200
        )
        player_uuid = utils.generate_offline_uuid("Baseline")
        self.assertIsNone(await utils.link_server_profile(player_uuid, "b" * 32))
        links = await self.db.query(
            "SELECT new_uuid FROM userlink WHERE uuid = %s", player_uuid
        )
        self.assertEqual(links[0]["new_uuid"], "b" * 32)
        await skin.save_skin_data("Baseline", "cape", "skin", "slim")
        self.assertEqual(
            await skin.read_skin_data("Baseline"), ("Baseline", "cape", "skin", "slim")
        )
        await figura.figura_server_sync(
            "12345678-1234-1234-1234-123456789abc", "Synced"
        )
        profile = await figura.figura_auth("Synced", "s", None)
        self.assertEqual(profile["id"], "12345678123412341234123456789abc")
        self.assertEqual(
            (await figura.figura_auth("Unregistered", "s", None))["id"],
            utils.generate_offline_uuid("Unregistered"),
        )

    async def test_insert_ignore_conflict_characterization(self):
        await auth.save_other_data("a" * 32, "SameName")
        await auth.save_other_data("b" * 32, "SameName")
        rows = await self.db.query("SELECT * FROM namelink WHERE uuid = %s", "b" * 32)
        self.assertFalse(rows)
        self.assertTrue(
            await self.db.execute(
                "INSERT IGNORE namelink (uuid, username) VALUES (%s, %s)",
                "c" * 32,
                "SameName",
            )
        )


if __name__ == "__main__":
    unittest.main()
