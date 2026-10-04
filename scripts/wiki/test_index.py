#!/usr/bin/env python3
"""统一 wiki 入口生成器的标准库单元测试。"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from index import (  # noqa: E402
    parse_javadoc_search_index,
    parse_rust_search_index,
    parse_typedoc_search_index,
    render_index,
    write_site,
)


class WikiIndexTest(unittest.TestCase):
    """覆盖三种原生索引和端点缺失时的入口行为。"""

    def test_rust_search_index_sample(self) -> None:
        entries = parse_rust_search_index(
            'var searchIndex = [{"n":"QiTransfer","q":"qi_physics::QiTransfer",'
            '"t":"struct","p":"struct.QiTransfer.html"}];'
        )
        self.assertEqual(entries[0]["name"], "QiTransfer")
        self.assertEqual(entries[0]["url"], "struct.QiTransfer.html")

    def test_javadoc_search_index_sample(self) -> None:
        entries = parse_javadoc_search_index(
            'typeSearchIndex = [{"p":"com.bong.Store","l":"AlchemyStore",'
            '"url":"com/bong/Store.html"}];'
        )
        self.assertEqual(entries[0]["qualified"], "com.bong.Store.AlchemyStore")
        self.assertEqual(entries[0]["name"], "AlchemyStore")
        self.assertEqual(entries[0]["url"], "com/bong/Store.html")

    def test_javadoc_member_link_uses_owner_page_and_anchor(self) -> None:
        entries = parse_javadoc_search_index(
            'memberSearchIndex = [{"p":"com.bong.Store","c":"AlchemyStore",'
            '"l":"refresh(java.lang.String)","u":"refresh(java.lang.String)"}];'
        )
        self.assertEqual(
            entries[0]["qualified"], "com.bong.Store.AlchemyStore.refresh(java.lang.String)"
        )
        self.assertEqual(entries[0]["name"], "refresh(java.lang.String)")
        self.assertEqual(
            entries[0]["url"],
            "com/bong/Store/AlchemyStore.html#refresh(java.lang.String)",
        )

    def test_typedoc_search_index_sample(self) -> None:
        entries = parse_typedoc_search_index(
            'var searchData = [{"name":"Command","kind":"interface",'
            '"url":"interfaces/Command.html"}];'
        )
        self.assertEqual(entries[0]["name"], "Command")
        self.assertEqual(entries[0]["kind"], "interface")

    def test_missing_endpoint_is_marked_without_blocking_site(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            server = output / "server"
            server.mkdir()
            (server / "struct.QiTransfer.html").write_text(
                "<html><head><title>QiTransfer in bong_server::qi_physics::ledger - Rust</title></head>"
                "<body><h1>QiTransfer</h1></body></html>",
                encoding="utf-8",
            )
            manifest = write_site(output)
            endpoints = {item["id"]: item for item in manifest["endpoints"]}
            self.assertTrue(endpoints["server"]["available"])
            self.assertEqual(
                endpoints["server"]["symbols"][0]["qualified"],
                "bong_server::qi_physics::ledger::QiTransfer",
            )
            self.assertFalse(endpoints["client"]["available"])
            self.assertFalse(endpoints["agent"]["available"])
            self.assertIn("生成物缺失", (output / "index.html").read_text(encoding="utf-8"))
            json.loads((output / "search-index.json").read_text(encoding="utf-8"))

    def test_rust_title_kind_prefix_is_removed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            server = output / "server"
            server.mkdir()
            (server / "struct.QiTransfer.html").write_text(
                "<html><head><title>Struct QiTransfer in bong_server::qi_physics - Rust</title>"
                "</head><body><h1>QiTransfer</h1></body></html>",
                encoding="utf-8",
            )
            manifest = write_site(output)
            self.assertEqual(
                manifest["endpoints"][0]["symbols"][0]["qualified"],
                "bong_server::qi_physics::QiTransfer",
            )

    def test_index_payload_uses_json_safe_script_escapes(self) -> None:
        manifest = {
            "version": 1,
            "endpoints": [
                {
                    "id": "server",
                    "label": "Server",
                    "available": True,
                    "path": "server/index.html",
                    "symbolCount": 1,
                    "symbols": [
                        {
                            "endpoint": "server",
                            "name": "A<B",
                            "qualified": "x&y",
                            "kind": "type",
                            "url": "server/a.html",
                        }
                    ],
                }
            ],
            "symbolCount": 1,
        }
        rendered = render_index(manifest)
        self.assertIn(r"\u003c", rendered)
        self.assertIn(r"\u0026", rendered)
        self.assertNotIn("&lt;", rendered)
        payload = rendered.split('<script id="wiki-data" type="application/json">', 1)[1]
        payload = payload.split("</script>", 1)[0]
        self.assertEqual(json.loads(payload)["endpoints"][0]["symbols"][0]["name"], "A<B")


if __name__ == "__main__":
    unittest.main()
