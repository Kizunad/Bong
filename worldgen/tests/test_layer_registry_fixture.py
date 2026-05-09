from __future__ import annotations

import json
import unittest
from pathlib import Path

from scripts.terrain_gen.dump_layer_registry import (
    dump_layer_registry_json,
    layer_registry_entries,
)


REPO_ROOT = Path(__file__).resolve().parents[2]
SERVER_FIXTURE = REPO_ROOT / "server/src/world/terrain/layer_registry_fixture.json"


class LayerRegistryFixtureTest(unittest.TestCase):
    def test_dump_matches_server_fixture_entries(self) -> None:
        expected = json.loads(SERVER_FIXTURE.read_text(encoding="utf-8"))

        self.assertEqual(layer_registry_entries(), expected)

    def test_dump_json_matches_server_fixture_format(self) -> None:
        expected = SERVER_FIXTURE.read_text(encoding="utf-8")

        self.assertEqual(dump_layer_registry_json(), expected)


if __name__ == "__main__":
    unittest.main()
