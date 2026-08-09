#!/usr/bin/env python3
"""Bot 框架编解码底座 + runner 纯逻辑的单元测试（无需 server，纯 stdlib）。

跑法：python3 scripts/bot/test_protocol.py
bot-e2e.sh 在起 server 之前先跑本文件——编解码坏了没必要浪费一次 server 启动。
"""

from __future__ import annotations

import ast
import io
import json
import os
import pathlib
import re
import shutil
import shlex
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import types
import unittest
import uuid
import zlib
from contextlib import redirect_stdout
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bot import mc_protocol as mc  # noqa: E402
from bot import make_novice_raster_fixture  # noqa: E402
from bot import proto_min  # noqa: E402
from bot import run_scenarios as scenario_runner  # noqa: E402
from bot.bot import Bot, BotAssertionError, _signed_12, _signed_26  # noqa: E402
from bot.server_data import decode_server_data_payload  # noqa: E402
from bot.scenarios._inventory_helpers import (  # noqa: E402
    latest_inventory_snapshot,
    wait_inventory_revision_after,
    wait_inventory_revision_after_matching,
    wait_inventory_snapshot_after,
)
from bot.scenarios.cultivation_pill_consume import (  # noqa: E402
    NON_CLAMP_EXPECTED_QI,
    PILL_ID,
    PILL_QI_RECOVERY,
    SERVER_TICK_OBSERVATION_TICKS,
    _assert_settled_consumption,
    _expected_qi_after_pill,
    _has_departed_baseline,
    _is_qi_max_confirmation,
    _is_qi_set_confirmation,
    _player_state_values,
    _server_tick_from_event,
    _set_qi_and_wait,
    _set_qi_max_and_wait,
    _snapshot_after_server_tick_fence,
)
from bot.scenarios.npc_ambient_surface_resolution import (  # noqa: E402
    FIXTURE_MANIFEST_ENV,
    FIXTURE_OWNED_ENV,
    FIXTURE_TOKEN_ENV,
    _assert_raster_fixture_contract,
)
from bot.scenarios.terrain_poi_novice_startup import (  # noqa: E402
    _selection_strategy,
)
from bot.scenarios.terrain_north_rift_scorch_zone_identity import (  # noqa: E402
    PROBES as NORTH_RIFT_PROBES,
    REQUIRED_ENV as NORTH_RIFT_REQUIRED_ENV,
    _assert_ambient as north_rift_assert_ambient,
    _position_matches as north_rift_position_matches,
)
from bot.run_scenarios import (  # noqa: E402
    ScenarioEnv,
    check_server_reachable,
    discover_scenarios,
    validate_scenario_module,
)


class VarIntTest(unittest.TestCase):
    def test_roundtrip_boundaries(self):
        for value in [0, 1, 127, 128, 255, 300, 25565, 2**21, 2**28, 2**31 - 1, -1, -(2**31)]:
            reader = mc.Reader(mc.write_varint(value))
            got = reader.varint()
            self.assertEqual(
                got, value, f"varint 往返应无损（编码后解码回原值），{value} 却变成 {got}"
            )

    def test_negative_one_canonical_bytes(self):
        # -1 的协议标准编码是 5 字节全 f + 0f —— 锁字节形状防止两端编码器漂移
        self.assertEqual(mc.write_varint(-1), b"\xff\xff\xff\xff\x0f")

    def test_overlong_varint_raises(self):
        with self.assertRaises(ValueError, msg="6 字节连续位 varint 应报流错位而非静默吞"):
            mc.Reader(b"\xff\xff\xff\xff\xff\xff").varint()


class StringTest(unittest.TestCase):
    def test_roundtrip(self):
        for text in ["", "abc", "骨币", "bong:client_request", "a" * 500]:
            reader = mc.Reader(mc.mc_string(text))
            self.assertEqual(reader.string(), text)


class BlockPositionTest(unittest.TestCase):
    def test_roundtrip_corners(self):
        cases = [
            (0, 0, 0),
            (18357644, 831, -20882616),  # wiki.vg 经典样例坐标
            (-1, -1, -1),
            (-33554432, -2048, -33554432),  # 各字段最小值
            (33554431, 2047, 33554431),  # 各字段最大值
        ]
        for x, y, z in cases:
            packed = struct.unpack(">Q", mc.block_position(x, y, z))[0]
            got = (
                _signed_26(packed >> 38),
                _signed_12(packed & 0xFFF),
                _signed_26((packed >> 12) & 0x3FFFFFF),
            )
            self.assertEqual(got, (x, y, z), f"Position 编码往返 {x, y, z} 变成 {got}")


class DiggingActionTest(unittest.TestCase):
    def test_start_digging_encodes_vanilla_player_action(self):
        bot = _bare_bot()
        sent = []
        bot._send = lambda packet_id, body=b"": sent.append((packet_id, body))

        bot.start_digging(1292, 73, 1519, face=1, sequence=7)

        self.assertEqual(len(sent), 1)
        packet_id, body = sent[0]
        self.assertEqual(packet_id, mc.C2S_PLAYER_ACTION)
        reader = mc.Reader(body)
        self.assertEqual(reader.varint(), 0, "action=0 才是 Start Destroy Block")
        packed = struct.unpack(">Q", reader.data[reader.pos : reader.pos + 8])[0]
        reader.pos += 8
        self.assertEqual(
            (
                _signed_26(packed >> 38),
                _signed_12(packed & 0xFFF),
                _signed_26((packed >> 12) & 0x3FFFFFF),
            ),
            (1292, 73, 1519),
        )
        self.assertEqual(reader.u8(), 1)
        self.assertEqual(reader.varint(), 7)
        self.assertEqual(reader.rest(), b"")

    def test_start_digging_rejects_invalid_face_and_sequence(self):
        bot = _bare_bot()
        bot._send = lambda *_args: self.fail("invalid digging request must not be sent")
        for face in (-1, 6):
            with self.assertRaises(ValueError):
                bot.start_digging(0, 0, 0, face=face)
        with self.assertRaises(ValueError):
            bot.start_digging(0, 0, 0, sequence=-1)
        with self.assertRaises(ValueError):
            bot.start_digging(0, 0, 0, sequence=0x80000000)

    def test_start_digging_accepts_maximum_non_negative_varint_sequence(self):
        bot = _bare_bot()
        sent = []
        bot._send = lambda packet_id, body=b"": sent.append((packet_id, body))

        bot.start_digging(0, 0, 0, sequence=0x7FFFFFFF)

        reader = mc.Reader(sent[0][1])
        self.assertEqual(reader.varint(), 0)
        reader.pos += 8
        self.assertEqual(reader.u8(), 1)
        self.assertEqual(reader.varint(), 0x7FFFFFFF)
        self.assertEqual(reader.rest(), b"")

    def test_player_action_response_decodes_sequence(self):
        bot = _bare_bot()
        bot._dispatch(
            mc.write_varint(mc.S2C_PLAYER_ACTION_RESPONSE) + mc.write_varint(17)
        )

        event = bot.events[-1]
        self.assertEqual(event.kind, "player_action_response")
        self.assertEqual(event.data, {"sequence": 17})


class NoviceRasterFixtureTest(unittest.TestCase):
    TOKEN = "unit-test-ambient-fixture-token"

    def _generate(self, root: pathlib.Path) -> pathlib.Path:
        return make_novice_raster_fixture.generate(root, self.TOKEN)

    def _contract_env(self, manifest_path: pathlib.Path, token: str | None = None):
        return mock.patch.dict(
            os.environ,
            {
                FIXTURE_OWNED_ENV: "1",
                FIXTURE_MANIFEST_ENV: str(manifest_path),
                FIXTURE_TOKEN_ENV: self.TOKEN if token is None else token,
            },
            clear=False,
        )

    def test_fixture_exposes_deterministic_spiritwood_seed_without_changing_poi_tile(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

            self.assertEqual(
                {(tile["tile_x"], tile["tile_z"]) for tile in manifest["tiles"]},
                {(0, 0), (4, 5), (5, 5), (4, 6), (5, 6)},
            )
            self.assertEqual(
                manifest["world_bounds"],
                {"min_x": 0, "max_x": 1535, "min_z": 0, "max_z": 1791},
            )
            palette = manifest["biome_palette"]
            self.assertEqual(palette[4], "minecraft:meadow")
            for tile in manifest["tiles"]:
                biome_ids = (
                    root / tile["dir"] / "biome_id.bin"
                ).read_bytes()
                self.assertEqual(len(biome_ids), make_novice_raster_fixture.TILE_SIZE**2)
                self.assertLess(max(biome_ids), len(palette))

            self.assertEqual(set((root / "tile_0_0" / "biome_id.bin").read_bytes()), {0})
            for tile_x, tile_z in ((4, 5), (5, 5), (4, 6), (5, 6)):
                self.assertEqual(
                    set((root / f"tile_{tile_x}_{tile_z}" / "biome_id.bin").read_bytes()),
                    {4},
                )
            spirit_biomes = (root / "tile_5_5" / "biome_id.bin").read_bytes()
            seed_index = (1519 - 5 * 256) * 256 + (1292 - 5 * 256)
            self.assertEqual(spirit_biomes[seed_index], 4)

    def test_fixture_pins_ambient_support_air_and_no_water_contract(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(
                manifest["bot_fixture"],
                {
                    "kind": "ambient-surface-v1",
                    "token": self.TOKEN,
                    "surface_y": 72,
                    "support": "grass_block",
                    "feet_y": 73,
                    "head_y": 74,
                },
            )
            with self._contract_env(manifest_path):
                _assert_raster_fixture_contract()

    def test_ambient_fixture_contract_rejects_stale_token_and_missing_ownership(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            with self._contract_env(manifest_path, token="wrong-token"):
                with self.assertRaises(BotAssertionError, msg="stale token must not pin a prior fixture"):
                    _assert_raster_fixture_contract()
            with mock.patch.dict(
                os.environ,
                {
                    FIXTURE_OWNED_ENV: "0",
                    FIXTURE_MANIFEST_ENV: str(manifest_path),
                    FIXTURE_TOKEN_ENV: self.TOKEN,
                },
                clear=False,
            ):
                with self.assertRaises(BotAssertionError, msg="reuse/no-ownership must fail closed"):
                    _assert_raster_fixture_contract()

    def test_fixture_generator_requires_explicit_token(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaises(TypeError):
                make_novice_raster_fixture.generate(pathlib.Path(temp_dir))

            with self.assertRaises(ValueError):
                make_novice_raster_fixture.generate(pathlib.Path(temp_dir), " ")


class ChatTextTest(unittest.TestCase):
    def test_variants(self):
        cases = [
            ('"plain"', "plain"),
            ('{"text":"a","extra":[{"text":"b"},"c"]}', "abc"),
            ('{"extra":[{"text":"x","extra":["y"]}]}', "xy"),
            ('["a",{"text":"b"}]', "ab"),
            ("not json at all", "not json at all"),
            ("null", ""),
        ]
        for raw, expected in cases:
            self.assertEqual(mc.chat_text_to_plain(raw), expected, f"raw={raw!r}")


class ServerDataDecodeTest(unittest.TestCase):
    def test_json_inventory_snapshot_payload_decodes(self):
        payload = b'{"v":1,"type":"inventory_snapshot","revision":7}'
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "inventory_snapshot")
        self.assertEqual(decoded["revision"], 7)

    def test_malformed_server_data_returns_none(self):
        self.assertIsNone(decode_server_data_payload(b"\xff\x00not protobuf"))

    def test_proto_narration_payload_decodes_routing_fields(self):
        decoded = decode_server_data_payload(_server_data_narration_bytes())

        self.assertEqual(decoded["type"], "narration")
        self.assertEqual(
            decoded["narrations"],
            [
                {
                    "text": "天道的注意力掠过，境界未至",
                    "scope": "player",
                    "style": "system_warning",
                    "target": "offline:Alice",
                    "kind": "realm_gate_rejected",
                },
                {
                    "text": "旧式全服旁白",
                    "scope": "broadcast",
                    "style": "narration",
                },
            ],
            "Bot 必须解出 narration text/scope/style/optional target/kind，"
            "否则双玩家黑盒无法区分目标提示与同轮无关旁白",
        )

    def test_bot_dispatch_emits_decoded_narration_event(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_CUSTOM_PAYLOAD)
            + mc.mc_string("bong:server_data")
            + _server_data_narration_bytes()
        )

        bot._dispatch(body)

        decoded_events = bot.events_of("server_data")
        self.assertEqual(len(decoded_events), 1)
        self.assertEqual(decoded_events[0].data["payload_type"], "narration")
        self.assertEqual(
            decoded_events[0].data["payload"]["narrations"][0]["target"],
            "offline:Alice",
            "真实 Bot reader 必须把 production protobuf narration 暴露为可断言事件",
        )

    def test_proto_zone_info_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_zone_info_bytes())

        self.assertEqual(
            decoded,
            {
                "v": 1,
                "type": "zone_info",
                "zone": "rift_mouth_north_002",
                "spirit_qi": 0.068602,
                "danger_level": 5,
                "status": "Normal",
                "active_events": ["rift_mouth_entry", "wind_warning"],
                "perception_text": "灵气骤薄",
            },
            "production protobuf zone_info 应完整解出标量、repeated 与 optional 字段",
        )

    def test_proto_inventory_snapshot_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_inventory_snapshot_bytes())

        self.assertEqual(decoded["type"], "inventory_snapshot")
        self.assertEqual(decoded["revision"], 12)
        self.assertEqual(decoded["containers"][0]["id"], "body_pocket")
        self.assertEqual(decoded["equipped"]["chest_worn"][0]["item_id"], "worn_grass_pouch")

    def test_proto_inventory_event_moved_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_inventory_event_moved_bytes())

        self.assertEqual(decoded["type"], "inventory_event")
        self.assertEqual(decoded["kind"], "moved")
        self.assertEqual(decoded["revision"], 13)
        self.assertEqual(decoded["instance_id"], 99)
        self.assertEqual(decoded["from"]["container_id"], "body_pocket")
        self.assertEqual(decoded["to"]["slot"], "chest")
        self.assertEqual(decoded["to"]["state"], "worn")

    def test_proto_loot_container_open_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_open_bytes())

        self.assertEqual(decoded["type"], "loot_container_open")
        self.assertEqual(decoded["session_id"], 7)
        self.assertEqual(decoded["rows"], 3)
        self.assertEqual(decoded["cols"], 4)

    def test_proto_loot_container_update_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_update_bytes())

        self.assertEqual(
            decoded["type"],
            "loot_container_update",
            "expected type=loot_container_update so the bot dispatches the authoritative "
            f"update payload, actual={decoded['type']}",
        )
        self.assertEqual(
            decoded["session_id"],
            7,
            "expected session_id=7 so the update remains bound to its opened session, "
            f"actual={decoded['session_id']}",
        )
        self.assertEqual(
            decoded["placed_items"][0]["container_id"],
            "ext_7",
            "expected container_id=ext_7 so the update targets the session container, "
            f"actual={decoded['placed_items'][0]['container_id']}",
        )
        self.assertEqual(
            decoded["placed_items"][0]["item"]["instance_id"],
            99,
            "expected instance_id=99 so the update preserves item identity, "
            f"actual={decoded['placed_items'][0]['item']['instance_id']}",
        )

    def test_proto_loot_container_close_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_close_bytes())

        self.assertEqual(
            decoded["type"],
            "loot_container_close",
            "expected type=loot_container_close so the bot dispatches the close payload, "
            f"actual={decoded['type']}",
        )
        self.assertEqual(
            decoded["session_id"],
            7,
            "expected session_id=7 so close invalidates the opened session, "
            f"actual={decoded['session_id']}",
        )
        self.assertEqual(
            decoded["reason"],
            "distance",
            "expected reason=distance so the bot observes the server rejection cause, "
            f"actual={decoded['reason']}",
        )

    def test_proto_morph_state_full_payload_decodes(self):
        # plan-race-system-v1 P4 — field 142，mode="full"，一条 active=true entry。
        decoded = decode_server_data_payload(
            _server_data_morph_state_bytes(
                mode="full",
                entity_id=42,
                model_kind=1,
                form_race_id="whale",
                form_body_plan_id="whale",
                active=True,
            )
        )

        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["mode"], "full")
        self.assertEqual(len(decoded["entries"]), 1)
        entry = decoded["entries"][0]
        self.assertEqual(entry["entity_id"], 42)
        self.assertEqual(entry["model_kind"], 1)
        self.assertEqual(entry["form_race_id"], "whale")
        self.assertEqual(entry["form_body_plan_id"], "whale")
        self.assertTrue(entry["active"])

    def test_proto_morph_state_delta_release_payload_decodes(self):
        # mode="delta" + active=false —— 客户端应据此从本地缓存删除该 entity_id。
        decoded = decode_server_data_payload(
            _server_data_morph_state_bytes(
                mode="delta",
                entity_id=42,
                model_kind=0,
                form_race_id="",
                form_body_plan_id="",
                active=False,
            )
        )

        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["mode"], "delta")
        entry = decoded["entries"][0]
        self.assertEqual(entry["entity_id"], 42)
        self.assertFalse(entry["active"])

    def test_proto_morph_state_empty_entries_decodes(self):
        # 未易形 / 无实体时的常态：entries 为空，不应报错或返回 None。
        payload = _pb_message(
            142,
            _pb_varint(1, 1) + _pb_string(2, "full"),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["entries"], [])

    def test_proto_tribulation_state_payload_name_decodes(self):
        # DONE-W6-HEADLESSAUDIT §5 P0-4 gap：渡虚劫状态 payload（envelope.proto:66）。
        # kind/phase 在 wire 上是 string 字段（envelope.proto:2374-2375），断言解出字面值。
        payload = _pb_message(
            66,
            _pb_varint(1, 1)
            + _pb_string(2, "offline:Alice")
            + _pb_string(3, "Alice")
            + _pb_string(4, "du_xu")
            + _pb_string(5, "omen")
            + _pb_varint(8, 2)
            + _pb_varint(9, 3),
        )
        self.assertEqual(proto_min.server_data_payload_name(payload), "tribulation_state")
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "tribulation_state")
        self.assertTrue(decoded["active"])
        self.assertEqual(decoded["char_id"], "offline:Alice")
        self.assertEqual(decoded["actor_name"], "Alice")
        self.assertEqual(decoded["kind"], "du_xu")
        self.assertEqual(decoded["phase"], "omen")
        self.assertEqual(decoded["wave_current"], 2)
        self.assertEqual(decoded["wave_total"], 3)

    def test_proto_insight_offer_payload_name_decodes(self):
        # DONE-W6-HEADLESSAUDIT §5 P0-4 gap：顿悟邀约 payload（envelope.proto:131）。
        # 断言 offer_id/trigger_id/character_id 字段号→输出键映射 + choices 计数。
        choice = _pb_string(2, "qi_regen_factor") + _pb_string(6, "converge")
        payload = _pb_message(
            131,
            _pb_string(1, "insight:5:first_breakthrough_to_Induce")
            + _pb_string(2, "first_breakthrough_to_Induce")
            + _pb_string(3, "offline:Alice")
            + _pb_message(4, choice)
            + _pb_message(4, choice)
            + _pb_message(4, choice),
        )
        self.assertEqual(proto_min.server_data_payload_name(payload), "insight_offer")
        self.assertIn(b"first_breakthrough_to_Induce", payload)
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "insight_offer")
        self.assertEqual(decoded["offer_id"], "insight:5:first_breakthrough_to_Induce")
        self.assertEqual(decoded["trigger_id"], "first_breakthrough_to_Induce")
        self.assertEqual(decoded["character_id"], "offline:Alice")
        self.assertEqual(len(decoded["choices"]), 3)

    def test_proto_death_screen_visible_payload_decodes(self):
        # field 72 DeathScreen：濒死决策已出（Fortune，stage=1），决策窗口开启。
        # 字段 3/4/5 一并钉死：luck_remaining(double)/final_words(repeated string)/
        # countdown_until_ms(uint64 varint)——与 server 侧 fixture（proto_gen.rs）对齐。
        payload = _pb_message(
            72,
            _pb_varint(1, 1)  # visible=true
            + _pb_string(2, "voluntary_retire")
            + _pb_fixed64(3, 0.3)  # luck_remaining=0.3
            + _pb_string(4, "你的修为到此为止")
            + _pb_string(4, "但愿来生...")  # final_words（repeated）
            + _pb_varint(5, 1700000030000)  # countdown_until_ms
            + _pb_varint(6, 1)  # can_reincarnate=true
            # can_terminate=false（Fortune 决策不可主动终结）
            + _pb_varint(8, 1)  # stage=FORTUNE
            + _pb_varint(9, 1)  # death_number=1
            + _pb_varint(10, 1),  # zone_kind=ORDINARY
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertTrue(decoded["visible"])
        self.assertEqual(decoded["cause"], "voluntary_retire")
        self.assertAlmostEqual(decoded["luck_remaining"], 0.3)
        self.assertEqual(
            decoded["final_words"], ["你的修为到此为止", "但愿来生..."],
            "repeated string 字段 4 应按序解出列表",
        )
        self.assertEqual(decoded["countdown_until_ms"], 1700000030000)
        self.assertTrue(decoded["can_reincarnate"])
        self.assertFalse(decoded["can_terminate"])
        self.assertEqual(decoded["stage"], 1)
        self.assertEqual(decoded["death_number"], 1)
        self.assertEqual(decoded["zone_kind"], 1)

    def test_proto_death_screen_tribulation_payload_decodes(self):
        # Tribulation（stage=2）决策：can_terminate=true。
        payload = _pb_message(
            72,
            _pb_varint(1, 1)
            + _pb_fixed64(3, 0.85)  # luck_remaining
            + _pb_string(4, "大限将至")  # final_words（repeated，单条）
            + _pb_varint(5, 1700000050000)  # countdown_until_ms
            + _pb_varint(6, 1)
            + _pb_varint(7, 1)
            + _pb_varint(8, 2)
            + _pb_varint(9, 4),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertTrue(decoded["visible"])
        self.assertAlmostEqual(decoded["luck_remaining"], 0.85)
        self.assertEqual(decoded["final_words"], ["大限将至"])
        self.assertEqual(decoded["countdown_until_ms"], 1700000050000)
        self.assertTrue(decoded["can_terminate"])
        self.assertEqual(decoded["stage"], 2)
        self.assertEqual(decoded["death_number"], 4)

    def test_proto_death_screen_hidden_payload_decodes(self):
        # 复活/终结后的收屏：visible=false，无 stage/death_number（optional 未填）。
        payload = _pb_message(72, _pb_varint(1, 0))
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertFalse(decoded["visible"])
        self.assertNotIn("stage", decoded)
        self.assertNotIn("death_number", decoded)

    def test_proto_terminate_screen_visible_payload_decodes(self):
        # field 73 TerminateScreen：主动归隐终结后的终结屏。
        payload = _pb_message(
            73,
            _pb_varint(1, 1)  # visible=true
            + _pb_string(2, "此身止于此。")
            + _pb_string(3, "你选择了归隐与终结。")
            + _pb_string(4, "凡人"),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "terminate_screen")
        self.assertTrue(decoded["visible"])
        self.assertEqual(decoded["final_words"], "此身止于此。")
        self.assertEqual(decoded["epilogue"], "你选择了归隐与终结。")
        self.assertEqual(decoded["archetype_suggestion"], "凡人")

    def test_proto_terminate_screen_hidden_payload_decodes(self):
        # 新建角色后的收屏：visible=false。
        payload = _pb_message(73, _pb_varint(1, 0))
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "terminate_screen")
        self.assertFalse(decoded["visible"])


class InventoryHelperTest(unittest.TestCase):
    def test_latest_inventory_snapshot_uses_newest_history(self):
        bot = _FakeBot(
            [
                _snapshot_event(1.0, 1, "old_pack"),
                _snapshot_event(2.0, 2, "new_pack"),
            ]
        )

        snapshot = latest_inventory_snapshot(bot)

        self.assertEqual(snapshot["revision"], 2)
        self.assertEqual(snapshot["marker"], "new_pack")

    def test_wait_inventory_snapshot_after_ignores_old_history(self):
        bot = _FakeBot(
            [
                _snapshot_event(1.0, 1, "old_pack"),
                _snapshot_event(3.0, 2, "after_unequip"),
            ]
        )

        snapshot = wait_inventory_snapshot_after(bot, after_t=2.0)

        self.assertEqual(snapshot["revision"], 2)
        self.assertEqual(snapshot["marker"], "after_unequip")

    def test_inventory_move_watermarks_skip_stow_snapshot_before_unequip(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 2, "after_give"),
                _snapshot_event(3.0, 3, "after_stow"),
                _snapshot_event(4.0, 4, "after_unequip"),
            ]
        )

        after_give_revision = 2
        stow_snapshot = wait_inventory_revision_after(bot, after_give_revision)
        unequip_snapshot = wait_inventory_revision_after(bot, stow_snapshot["revision"])

        self.assertEqual(stow_snapshot["revision"], 3)
        self.assertEqual(stow_snapshot["marker"], "after_stow")
        self.assertEqual(unequip_snapshot["revision"], 4)
        self.assertEqual(unequip_snapshot["marker"], "after_unequip")

    def test_wait_inventory_revision_after_matching_skips_intermediate_snapshot(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 2, "command_intermediate"),
                _snapshot_event(3.0, 3, "command_final"),
            ]
        )

        snapshot = wait_inventory_revision_after_matching(
            bot,
            1,
            lambda payload: payload["marker"] == "command_final",
            "command_final marker",
        )

        self.assertEqual(snapshot["revision"], 3)
        self.assertEqual(snapshot["marker"], "command_final")


class NovicePoiScenarioParsingTest(unittest.TestCase):
    def test_selection_strategy_requires_exact_token_not_known_prefix(self):
        relaxed = "relaxed_radius_2000"
        qi_margin = "relaxed_radius_2000_qi_margin_0_1"

        self.assertEqual(
            _selection_strategy(f"[dev] novice_poi mutant_nest pos=1,2,3 selection={relaxed}"),
            relaxed,
        )
        self.assertEqual(
            _selection_strategy(
                f"[dev] novice_poi spirit_herb_valley pos=1,2,3 selection={qi_margin}"
            ),
            qi_margin,
        )
        self.assertNotEqual(_selection_strategy(f"selection={qi_margin}"), relaxed)
        self.assertNotEqual(_selection_strategy(f"selection={relaxed}"), qi_margin)

    def test_selection_strategy_rejects_missing_or_empty_value(self):
        self.assertIsNone(_selection_strategy("[dev] novice_poi mutant_nest pos=1,2,3"))
        self.assertIsNone(_selection_strategy("[dev] novice_poi mutant_nest selection="))


class NorthRiftScenarioContractTest(unittest.TestCase):
    def test_ambient_contract_accepts_only_state_matched_wilderness_recipes(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, ambient_recipe_id in (
            ("AMBIENT", "ambient_wilderness"),
            ("CULTIVATION", "cultivation_meditate"),
        ):
            with self.subTest(music_state=music_state):
                north_rift_assert_ambient(
                    bot,
                    probe,
                    {
                        "pos": expected_pos,
                        "music_state": music_state,
                        "ambient_recipe_id": ambient_recipe_id,
                    },
                )

    def test_ambient_contract_rejects_crossed_state_recipe_pairs(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, wrong_recipe in (
            ("AMBIENT", "cultivation_meditate"),
            ("CULTIVATION", "ambient_wilderness"),
        ):
            with self.subTest(music_state=music_state, wrong_recipe=wrong_recipe):
                with self.assertRaisesRegex(BotAssertionError, "ambient_recipe_id"):
                    north_rift_assert_ambient(
                        bot,
                        probe,
                        {
                            "pos": expected_pos,
                            "music_state": music_state,
                            "ambient_recipe_id": wrong_recipe,
                        },
                    )

    def test_ambient_contract_rejects_unexpected_or_missing_music_state(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, production_recipe in (
            ("COMBAT", "combat_music"),
            ("TSY", "ambient_tsy"),
            ("TRIBULATION", "tribulation_atmosphere"),
            ("UNKNOWN", "ambient_wilderness"),
            (None, "ambient_wilderness"),
        ):
            with self.subTest(
                music_state=music_state, production_recipe=production_recipe
            ):
                payload = {
                    "pos": expected_pos,
                    "ambient_recipe_id": production_recipe,
                }
                if music_state is not None:
                    payload["music_state"] = music_state
                with self.assertRaisesRegex(BotAssertionError, "music_state"):
                    north_rift_assert_ambient(bot, probe, payload)

    def test_ambient_contract_still_rejects_wrong_authoritative_position(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")

        with self.assertRaisesRegex(BotAssertionError, "pos"):
            north_rift_assert_ambient(
                bot,
                probe,
                {
                    "pos": [int(probe.pos[0]), int(probe.pos[1]), int(probe.pos[2]) + 1],
                    "music_state": "CULTIVATION",
                    "ambient_recipe_id": "cultivation_meditate",
                },
            )

    def test_probes_pin_three_production_z_coordinates_and_zone_identity(self):
        actual = {probe.pos[2]: probe.zone for probe in NORTH_RIFT_PROBES}
        self.assertEqual(
            actual,
            {
                -7303.0: "rift_mouth_north_002",
                -7800.0: "north_waste_east_scorch",
                -7500.0: "north_waste_east_scorch",
            },
            "真实场景必须覆盖 portal 半径外的迁移后渊口、旧入口焦土点与 inclusive 边界",
        )
        rift_probe = next(
            probe for probe in NORTH_RIFT_PROBES if probe.zone == "rift_mouth_north_002"
        )
        self.assertGreater(
            abs(rift_probe.pos[2] - (-7300.0)),
            2.0,
            "真实 bot 的 rift identity 点必须避开 z=-7300 portal anchor 的 2 格传送半径",
        )

    def test_probe_order_forces_a_zone_transition_before_every_assertion(self):
        zones = [probe.zone for probe in NORTH_RIFT_PROBES]
        self.assertTrue(
            all(left != right for left, right in zip(zones, zones[1:])),
            f"相邻 probe 必须跨 zone 才会产生 after-watermark zone_info，实际 {zones}",
        )

    def test_probe_perception_text_matches_production_qi_transition_ratios(self):
        self.assertEqual(
            [probe.perception_text for probe in NORTH_RIFT_PROBES],
            [
                "灵气稀薄，引气如吸沙",
                "灵气几近断绝，此地有不祥预感",
                "此地灵气骤然浓郁，呼吸间元气盈满",
            ],
            "spawn→scorch→rift→scorch 的 production qi 比值必须钉住真实感知文本",
        )

    def test_authoritative_position_match_rejects_stale_and_wrong_packets(self):
        probe = NORTH_RIFT_PROBES[0]
        exact = types.SimpleNamespace(
            kind="pos_look",
            t=2.0,
            data={
                "x": probe.pos[0],
                "y": probe.pos[1],
                "z": probe.pos[2],
                "yaw": probe.yaw,
                "pitch": probe.pitch,
            },
        )
        self.assertTrue(north_rift_position_matches(exact, probe, watermark=1.0))
        self.assertFalse(
            north_rift_position_matches(exact, probe, watermark=2.0),
            "watermark 之前/同刻的历史 pos_look 不得冒充本次 authoritative 回包",
        )
        wrong = types.SimpleNamespace(
            kind="pos_look",
            t=3.0,
            data={**exact.data, "z": probe.pos[2] + 1.0},
        )
        self.assertFalse(
            north_rift_position_matches(wrong, probe, watermark=1.0),
            "坐标仅差一格也不得通过 production point 对拍",
        )


class BotE2eDevModeContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        root = pathlib.Path(__file__).parents[2]
        cls.source = (root / "scripts/bot-e2e.sh").read_text(encoding="utf-8")
        cls.compose_source = (root / "docker-compose.test.yml").read_text(
            encoding="utf-8"
        )
        cls.workflow_source = (root / ".github/workflows/e2e.yml").read_text(
            encoding="utf-8"
        )

    def test_mode_contract_distinguishes_generic_inputs_from_owned_fixture_inputs(self):
        guard_end = self.source.index('\nmkdir -p "$EVIDENCE_ROOT"')
        guard = self.source[:guard_end]
        self.assertIn('AMBIENT_FIXTURE_MODE="${BOT_E2E_AMBIENT_FIXTURE_MODE:-0}"', guard)
        self.assertIn('BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_REUSE=1 互斥', guard)
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', guard)
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', guard)
        self.assertNotIn('if [ "$REUSE" != "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', guard)
        self.assertNotIn('if [ "$REUSE" != "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', guard)

    def test_generic_mode_preserves_caller_inputs_and_skips_ambient_ownership(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        self.assertIn('if [ "$REUSE" != "1" ] && [ -z "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', fixture)
        self.assertIn('BOT_FIXTURE_TOKEN="$(python3 -c', fixture)
        self.assertIn('--fixture-token "$BOT_FIXTURE_TOKEN"', fixture)
        state_start = self.source.index('# Ambient-owned runs pin state')
        state_end = self.source.index('\nport_open() {', state_start)
        state = self.source[state_start:state_end]
        self.assertIn('elif [ "$REUSE" != "1" ] && [ -z "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', state)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_OWNED', fixture)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_MANIFEST', fixture)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_TOKEN', fixture)

        redis_start = self.source.index('# ---- redis ----')
        redis_end = self.source.index('\n# ---- server ----', redis_start)
        redis = self.source[redis_start:redis_end]
        redis_guard = 'if [ "$REUSE" != "1" ] && { [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then'
        self.assertIn(redis_guard, redis)
        self.assertNotIn('export REDIS_URL=', redis[:redis.index(redis_guard)])

    def test_owned_fixture_mode_keeps_private_runtime_and_exact_marker_gate(self):
        runtime_start = self.source.index('if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then', self.source.index('SERVER_LOG='))
        runtime_end = self.source.index('\n# Owned-fixture mode generates', runtime_start)
        runtime = self.source[runtime_start:runtime_end]
        self.assertIn('SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"', runtime)
        self.assertIn('ln -s "$ROOT/server/assets" "$SERVER_RUNTIME_DIR/server/assets"', runtime)

        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index('\n# ---- 场景 ----', readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        self.assertIn('grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"', readiness)
        self.assertIn('export BOT_E2E_AMBIENT_FIXTURE_OWNED=1', readiness)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', readiness)
        self.assertLess(readiness.index('grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"'), readiness.index('export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'))

    def test_generic_and_reuse_never_grant_ambient_ownership(self):
        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index('\n# ---- 场景 ----', readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        grant = 'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then', readiness)
        self.assertLess(readiness.index('if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then'), readiness.index(grant))
        self.assertIn('BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起', self.source)
        self.assertIn('$HOST:$PORT 已被占用。要对着现有 server 跑请设 BOT_E2E_REUSE=1', self.source)
        self.assertNotIn('pkill', self.source)

    def test_mode_contract_executes_early_rejections_before_harness_setup(self):
        root = pathlib.Path(__file__).parents[2]
        cases = (
            (
                {"BOT_E2E_AMBIENT_FIXTURE_MODE": "bogus"},
                "BOT_E2E_AMBIENT_FIXTURE_MODE 仅接受空值、0 或 1",
            ),
            (
                {"BOT_E2E_AMBIENT_FIXTURE_MODE": "1", "BOT_E2E_REUSE": "1"},
                "BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_REUSE=1 互斥",
            ),
            (
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BONG_TERRAIN_RASTER_PATH": "/caller/terrain.json",
                },
                "ambient fixture mode 不接受外部 BONG_TERRAIN_RASTER_PATH",
            ),
            (
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BONG_SPIRITWOOD_HARVESTED_PATH": "/caller/harvested.json",
                },
                "ambient fixture mode 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH",
            ),
        )
        isolated = (
            "BOT_E2E_AMBIENT_FIXTURE_MODE",
            "BOT_E2E_REUSE",
            "BOT_E2E_HOST",
            "BOT_E2E_PORT",
            "BONG_TERRAIN_RASTER_PATH",
            "BONG_SPIRITWOOD_HARVESTED_PATH",
            "REDIS_URL",
        )
        for overrides, expected in cases:
            with self.subTest(overrides=overrides):
                env = os.environ.copy()
                for name in isolated:
                    env.pop(name, None)
                env.update(overrides)
                result = subprocess.run(
                    ["bash", "scripts/bot-e2e.sh"],
                    cwd=root,
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn(expected, result.stderr)

    def test_generic_no_raster_generates_tokenized_fixture_before_tool_failure(self):
        root = pathlib.Path(__file__).parents[2]
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_bin = pathlib.Path(temp_dir) / "bin"
            fake_bin.mkdir()
            log_path = pathlib.Path(temp_dir) / "python.log"
            real_python = shutil.which("python3")
            (fake_bin / "python3").write_text(
                "#!/usr/bin/env bash\n"
                f"printf '%s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "if [[ \"$1\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"$1\" == *make_novice_raster_fixture.py ]]; then\n"
                "  test \"$3\" = \"--fixture-token\" || exit 41\n"
                "  test -n \"$4\" || exit 42\n"
                "  printf '%s/manifest.json\\n' \"$2\"\n"
                "  exit 0\n"
                "fi\n"
                f"exec {shlex.quote(real_python)} \"$@\"\n",
                encoding="utf-8",
            )
            (fake_bin / "python3").chmod(0o755)
            (fake_bin / "docker").write_text("#!/usr/bin/env bash\nexit 1\n", encoding="utf-8")
            (fake_bin / "docker").chmod(0o755)
            (fake_bin / "cargo").write_text("#!/usr/bin/env bash\nexit 46\n", encoding="utf-8")
            (fake_bin / "cargo").chmod(0o755)
            env = os.environ.copy()
            for name in (
                "BOT_E2E_AMBIENT_FIXTURE_MODE",
                "BOT_E2E_REUSE",
                "BOT_E2E_HOST",
                "BOT_E2E_PORT",
                "BONG_TERRAIN_RASTER_PATH",
                "BONG_SPIRITWOOD_HARVESTED_PATH",
                "REDIS_URL",
            ):
                env.pop(name, None)
            env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
            result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=False,
                timeout=20,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--fixture-token", log_path.read_text(encoding="utf-8"))
            self.assertNotIn("BOT_RASTER_FIXTURE_READY", result.stderr)

    def test_ci_selects_explicit_ambient_fixture_mode(self):
        bot_stage = self.workflow_source[self.workflow_source.index('Bot e2e stage'):]
        self.assertIn('BOT_E2E_AMBIENT_FIXTURE_MODE: "1"', bot_stage)

    def test_self_started_server_builds_unique_exact_fixture_identity(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        for required in (
            'mktemp -d "$EVIDENCE_DIR/novice-raster.XXXXXX"',
            '--fixture-token "$BOT_E2E_AMBIENT_FIXTURE_TOKEN"',
            'export BOT_E2E_AMBIENT_FIXTURE_TOKEN',
            'export BOT_E2E_AMBIENT_FIXTURE_MANIFEST="$BONG_TERRAIN_RASTER_PATH"',
            "pathlib.Path(sys.argv[1]).resolve(strict=True)",
            'BOT_RASTER_READY_PAYLOAD="[bong][world] BOT_RASTER_FIXTURE_READY '
            'manifest=$BONG_TERRAIN_RASTER_PATH token=$BOT_E2E_AMBIENT_FIXTURE_TOKEN"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, fixture)
        self.assertNotIn(
            'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1', fixture,
            "生成本地文件不能授予 ownership；必须等 server exact ready marker",
        )

    def test_fixture_ownership_is_granted_only_after_exact_server_marker_and_port_ownership(self):
        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index("\n# ---- 场景 ----", readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        marker_match = 'grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"'
        ownership = 'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'

        self.assertIn(marker_match, readiness)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', readiness)
        self.assertIn('port_open "$HOST" "$PORT"', readiness)
        self.assertIn(ownership, readiness)
        self.assertLess(
            readiness.index(marker_match),
            readiness.index(ownership),
            "必须先由 server 日志对拍 canonical manifest+token，再允许场景声明 ownership",
        )
        self.assertLess(
            self.source.index(ownership, readiness_start),
            self.source.index('python3 "$ROOT/scripts/bot/run_scenarios.py" --all'),
            "场景只可在本轮 server fixture identity 与端口 ownership 同时成立后运行",
        )

    def test_bot_e2e_pipeline_propagates_runner_then_tee_status(self) -> None:
        scenario_start = self.source.index("set +e\n", self.source.index("# ---- 场景 ----"))
        scenario_end = self.source.index("\nset -e", scenario_start) + len("\nset -e")
        pipeline = self.source[scenario_start:scenario_end]
        runner = '''BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" \\
  python3 "$ROOT/scripts/bot/run_scenarios.py" --all 2>&1'''
        sink = 'tee "$SCENARIOS_LOG"'
        self.assertEqual(
            pipeline.count(runner),
            1,
            "场景块必须仅含一个 canonical runner，避免测试替换到错误命令",
        )
        self.assertEqual(
            pipeline.count(sink),
            1,
            "场景块必须仅含一个 canonical sink，避免测试替换到错误管道",
        )

        for runner_code, tee_code, expected in (
            (7, 0, 7),
            (7, 9, 7),
            (0, 9, 9),
            (0, 0, 0),
        ):
            with self.subTest(runner_code=runner_code, tee_code=tee_code):
                executable = pipeline.replace(
                    runner,
                    f"bash -c 'exit {runner_code}'",
                    1,
                ).replace(
                    sink,
                    f"bash -c 'exit {tee_code}'",
                    1,
                )
                result = subprocess.run(
                    ["bash", "-c", f'{executable}\nexit "$EXIT_CODE"'],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    result.returncode,
                    expected,
                    "真实 bot-e2e status block 必须优先传播 runner 失败，并传播独立 sink 失败",
                )

    def test_fixture_runtime_ownership_is_rechecked_during_and_after_scenarios(self) -> None:
        helper_start = self.source.index("self_started_fixture_runtime_is_current() {")
        helper_end = self.source.index("\n}\n", helper_start)
        helper = self.source[helper_start:helper_end]
        for required in (
            'kill -0 "$SERVER_PID"',
            'grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"',
            'port_open "$HOST" "$PORT"',
            'port_owned_by_tree "$SERVER_PID" "$PORT"',
        ):
            with self.subTest(required=required):
                self.assertIn(
                    required,
                    helper,
                    "fixture runtime helper 必须重新绑定活 server、exact marker 与当前 listener",
                )

        scenario_start = self.source.index("# ---- 场景 ----")
        scenario = self.source[scenario_start:]
        runner = 'python3 "$ROOT/scripts/bot/run_scenarios.py" --all'
        self.assertIn('while true; do', scenario)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', scenario)
        self.assertIn('echo lost >"$RUNTIME_WATCH_LOG"', scenario)
        self.assertIn('echo complete >"$RUNTIME_WATCH_LOG"', scenario)
        self.assertIn('pipeline_status=("${PIPESTATUS[@]}")', scenario)
        self.assertIn('EXIT_CODE=${pipeline_status[0]}', scenario)
        self.assertIn('EXIT_CODE=${pipeline_status[1]}', scenario)
        self.assertIn('wait "$WATCH_PID"', scenario)
        self.assertGreaterEqual(
            scenario.count("self_started_fixture_runtime_is_current"),
            2,
            "场景前和场景后都必须重验 server/fixture ownership，不能把一次 readiness 当永久授权",
        )
        self.assertLess(
            scenario.index("self_started_fixture_runtime_is_current"),
            scenario.index(runner),
            "运行 Bot 前必须重新验证本轮 server 仍持有 listener",
        )
        self.assertGreater(
            scenario.rindex("self_started_fixture_runtime_is_current"),
            scenario.index(runner),
            "Bot 结束后必须再次验证本轮 server 仍是证据主体",
        )

    def test_reuse_mode_clears_fixture_ownership_instead_of_claiming_external_server(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        for variable in (
            "BOT_E2E_AMBIENT_FIXTURE_OWNED",
            "BOT_E2E_AMBIENT_FIXTURE_MANIFEST",
            "BOT_E2E_AMBIENT_FIXTURE_TOKEN",
        ):
            with self.subTest(variable=variable):
                self.assertIn(f"unset {variable}", fixture)

    def test_reuse_without_listener_fails_closed_before_private_self_start(self):
        server_start = self.source.index("# ---- server ----")
        server = self.source[server_start:]
        rejection = 'BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起'
        self.assertIn(rejection, server)
        self.assertLess(server.index(rejection), server.index('cd "$SERVER_RUNTIME_DIR/server"'))

    def test_owned_fixture_mode_uses_private_cwd_and_generic_uses_checkout_cwd(self):
        launch_start = self.source.index('  (\n    if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then')
        launch_end = self.source.index('  ) >"$SERVER_LOG"', launch_start)
        launch = self.source[launch_start:launch_end]
        for required in (
            'cd "$SERVER_RUNTIME_DIR/server"',
            'cd "$ROOT/server"',
            'export BONG_DORMANT_ROGUE_SEED_COUNT="${BONG_DORMANT_ROGUE_SEED_COUNT:-0}"',
            'export BONG_ASSETS_DIR="$ROOT/server"',
            'export BONG_DEV_MODE=1',
            'exec "$ROOT/scripts/build-token.sh" cargo run --locked --manifest-path "$ROOT/server/Cargo.toml" $PROFILE_FLAG',
        ):
            with self.subTest(required=required):
                self.assertIn(required, launch)

    def test_each_run_owns_private_evidence_and_persistent_logs(self):
        evidence_setup = self.source[:self.source.index('# Owned-fixture mode generates')]
        for required in (
            'EVIDENCE_ROOT="$ROOT/.sisyphus/evidence/bot-e2e"',
            'EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"',
            'SERVER_LOG="$EVIDENCE_DIR/server.log"',
            'SCENARIOS_LOG="$EVIDENCE_DIR/scenarios.log"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)
        self.assertIn('mkdir -p "$EVIDENCE_ROOT"', evidence_setup)
        self.assertNotIn('EVIDENCE_DIR="$ROOT/.sisyphus/evidence/bot-e2e"', self.source)

    def test_redis_adopts_default_listener_or_starts_owned_private_instance(self):
        redis_start = self.source.index("# ---- redis ----")
        redis_end = self.source.index("\n# ---- server ----", redis_start)
        redis = self.source[redis_start:redis_end]
        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\ntrap cleanup EXIT", cleanup_start)
        cleanup = self.source[cleanup_start:cleanup_end]

        adopt_guard = 'if [ "$REUSE" != "1" ] && [ "$AMBIENT_FIXTURE_MODE" != "1" ] && [ -z "${REDIS_URL:-}" ] && port_open 127.0.0.1 6379; then'
        private_guard = 'elif [ "$REUSE" != "1" ] && { [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then'
        self.assertIn(adopt_guard, redis)
        self.assertIn('沿用调用方默认 Redis 127.0.0.1:6379', redis)
        self.assertIn(private_guard, redis)
        self.assertLess(redis.index(adopt_guard), redis.index(private_guard))
        self.assertNotIn('export REDIS_URL=', redis[:redis.index(private_guard)])
        self.assertIn('if [ "$STARTED_REDIS" = "1" ] && [ -n "$REDIS_COMPOSE_PROJECT" ]; then', cleanup)

        root = pathlib.Path(__file__).parents[2]
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_bin = pathlib.Path(temp_dir) / "bin"
            fake_bin.mkdir()
            log_path = pathlib.Path(temp_dir) / "tools.log"
            real_python = shutil.which("python3")
            (fake_bin / "python3").write_text(
                "#!/usr/bin/env bash\n"
                f"printf 'python3 %s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "if [[ \"$1\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"$1\" == *make_novice_raster_fixture.py ]]; then printf '%s/manifest.json\\n' \"$2\"; exit 0; fi\n"
                "if [[ \"$1\" == '-' ]] && [[ \"$2\" == '127.0.0.1' ]] && [[ \"$3\" == '6379' ]]; then exit \"${FAKE_DEFAULT_REDIS_OPEN:-1}\"; fi\n"
                f"exec {shlex.quote(real_python)} \"$@\"\n",
                encoding="utf-8",
            )
            (fake_bin / "python3").chmod(0o755)
            (fake_bin / "docker").write_text(
                "#!/usr/bin/env bash\n"
                f"printf 'docker %s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "exit 47\n",
                encoding="utf-8",
            )
            (fake_bin / "docker").chmod(0o755)
            (fake_bin / "cargo").write_text("#!/usr/bin/env bash\nexit 46\n", encoding="utf-8")
            (fake_bin / "cargo").chmod(0o755)

            base_env = os.environ.copy()
            for name in (
                "BOT_E2E_AMBIENT_FIXTURE_MODE", "BOT_E2E_REUSE", "BOT_E2E_HOST",
                "BOT_E2E_PORT", "BONG_TERRAIN_RASTER_PATH",
                "BONG_SPIRITWOOD_HARVESTED_PATH", "REDIS_URL",
            ):
                base_env.pop(name, None)
            base_env["PATH"] = f"{fake_bin}{os.pathsep}{base_env['PATH']}"

            adopted = base_env | {"FAKE_DEFAULT_REDIS_OPEN": "0"}
            adopted_result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"], cwd=root, env=adopted,
                capture_output=True, text=True, check=False, timeout=20,
            )
            adopted_tools = log_path.read_text(encoding="utf-8")
            self.assertIn("沿用调用方默认 Redis 127.0.0.1:6379", adopted_result.stdout)
            self.assertNotIn("docker ", adopted_tools)
            self.assertNotIn("REDIS_URL=", adopted_tools)
            self.assertNotIn("down -v", adopted_tools)

            log_path.unlink()
            private = base_env | {"FAKE_DEFAULT_REDIS_OPEN": "1"}
            private_result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"], cwd=root, env=private,
                capture_output=True, text=True, check=False, timeout=20,
            )
            private_tools = log_path.read_text(encoding="utf-8")
            self.assertEqual(private_result.returncode, 47, private_result.stderr)
            self.assertIn("docker compose", private_tools)
            self.assertIn("up -d redis --wait", private_tools)

    def test_redis_is_private_only_when_generic_caller_did_not_supply_url(self):
        redis_start = self.source.index("# ---- redis ----")
        redis_end = self.source.index("\n# ---- server ----", redis_start)
        redis = self.source[redis_start:redis_end]
        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\ntrap cleanup EXIT", cleanup_start)
        cleanup = self.source[cleanup_start:cleanup_end]
        self.assertIn('if [ "$REUSE" != "1" ] && [ "$AMBIENT_FIXTURE_MODE" != "1" ] && [ -z "${REDIS_URL:-}" ] && port_open 127.0.0.1 6379; then', redis)
        self.assertIn('elif [ "$REUSE" != "1" ] && { [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then', redis)
        for required in (
            'REDIS_COMPOSE_PROJECT="bong-bot-e2e-${RUN_ID,,}"',
            'BONG_TEST_COMPOSE_PROJECT="$REDIS_COMPOSE_PROJECT" BONG_TEST_REDIS_PORT=0',
            'docker compose -f "$ROOT/docker-compose.test.yml" up -d redis --wait',
            'export REDIS_URL="redis://127.0.0.1:$redis_port"',
        ):
            self.assertIn(required, redis)
        self.assertIn('if [ "$STARTED_REDIS" = "1" ] && [ -n "$REDIS_COMPOSE_PROJECT" ]; then', cleanup)
        self.assertNotIn("pkill", self.source)
        self.assertNotIn("BOT_E2E_KILL_STALE", self.source)
        self.assertIn("name: ${BONG_TEST_COMPOSE_PROJECT:-bong-test}", self.compose_source)

    def _run_owned_fixture_runtime_case(
        self, runner_mode: str, *, runner_exit: int = 0, tee_exit: int | None = None
    ) -> tuple[subprocess.CompletedProcess[str], str, str, str]:
        """Run the real bot-e2e shell path against only test-owned fake processes."""
        root = pathlib.Path(__file__).parents[2]
        real_python = shutil.which("python3")
        self.assertIsNotNone(real_python)

        with tempfile.TemporaryDirectory() as temp_dir:
            temp = pathlib.Path(temp_dir)
            fake_bin = temp / "bin"
            fake_bin.mkdir()
            runner_log = temp / "runner.log"
            runner_result_file = temp / "runner-result"
            server_pid_file = temp / "server.pid"
            listener_pid_file = temp / "listener.pid"
            replacement_pid_file = temp / "replacement.pid"
            replacement_ready_file = temp / "replacement-ready"
            watcher_status_file = temp / "watcher-status"
            redis_port = 39999
            port = self._unused_local_port()
            fake_python = fake_bin / "python3"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                f"real_python={shlex.quote(real_python)}\n"
                "if [[ \"${1:-}\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"${1:-}\" == *make_novice_raster_fixture.py ]]; then\n"
                "  mkdir -p \"$2\"\n"
                "  : > \"$2/manifest.json\"\n"
                "  printf '%s/manifest.json\\n' \"$2\"\n"
                "  exit 0\n"
                "fi\n"
                "if [[ \"${1:-}\" == *run_scenarios.py ]]; then\n"
                f"  printf '%s\\n' \"$FAKE_RUNNER_MODE\" >> {shlex.quote(str(runner_log))}\n"
                "  wait_watcher_lost() {\n"
                "    local status\n"
                "    for _ in $(seq 1 200); do\n"
                "      status=$(find \"$FAKE_EVIDENCE_ROOT\" -path '*/runtime-watch.*/status' -type f -print -quit)\n"
                "      if test -n \"$status\" && test \"$(cat \"$status\" 2>/dev/null || true)\" = lost; then\n"
                "        printf 'watcher-lost-observed\\n' > \"$FAKE_RUNNER_RESULT_FILE\"\n"
                "        return 0\n"
                "      fi\n"
                "      sleep 0.01\n"
                "    done\n"
                "    printf 'watcher-lost-timeout\\n' > \"$FAKE_RUNNER_RESULT_FILE\"\n"
                "    return 1\n"
                "  }\n"
                "  port_owned_by_pid() {\n"
                "    kill -0 \"$1\" 2>/dev/null || return 1\n"
                "    \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY' || return 1\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "    lsof -nP -iTCP:\"$BOT_E2E_PORT\" -sTCP:LISTEN -Fp 2>/dev/null | grep -q \"p$1\"\n"
                "  }\n"
                "  case \"$FAKE_RUNNER_MODE\" in\n"
                "    success) printf 'runner-complete\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 0 ;;\n"
                "    runner-fail) printf 'runner-failed\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit \"$FAKE_RUNNER_EXIT\" ;;\n"
                "    kill-server)\n"
                "      kill -TERM \"$(cat \"$FAKE_SERVER_PID_FILE\")\"\n"
                "      for _ in $(seq 1 200); do\n"
                "        if ! kill -0 \"$(cat \"$FAKE_SERVER_PID_FILE\")\" 2>/dev/null \\\n"
                "          && ! \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "        then break; fi\n"
                "        sleep 0.01\n"
                "      done\n"
                "      if kill -0 \"$(cat \"$FAKE_SERVER_PID_FILE\")\" 2>/dev/null \\\n"
                "        || \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "      then printf 'fault-setup-timeout\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 79; fi\n"
                "      wait_watcher_lost || { printf 'watcher-lost-timeout\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 80; }\n"
                "      exit 0\n"
                "      ;;\n"
                "    replace-listener)\n"
                "      kill -TERM \"$(cat \"$FAKE_LISTENER_PID_FILE\")\"\n"
                "      for _ in $(seq 1 200); do\n"
                "        if ! \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "        then break; fi\n"
                "        sleep 0.01\n"
                "      done\n"
                "      \"$real_python\" -u -c 'import os, socket, sys, time\ns = socket.socket()\ns.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\nfor _ in range(100):\n    try:\n        s.bind((\"127.0.0.1\", int(sys.argv[1])))\n        break\n    except OSError:\n        time.sleep(0.01)\nelse:\n    raise SystemExit(77)\ns.listen()\nopen(sys.argv[2], \"w\").write(str(os.getpid()))\nwhile True:\n    conn, _ = s.accept()\n    conn.close()' \"$BOT_E2E_PORT\" \"$FAKE_REPLACEMENT_READY_FILE\" </dev/null >/dev/null 2>&1 &\n"
                f"      printf '%s\\n' \"$!\" > {shlex.quote(str(replacement_pid_file))}\n"
                "      for _ in $(seq 1 200); do\n"
                "        test -s \"$FAKE_REPLACEMENT_READY_FILE\" && port_owned_by_pid \"$(cat \"$FAKE_REPLACEMENT_READY_FILE\")\" && break\n"
                "        sleep 0.01\n"
                "      done\n"
                "      test -s \"$FAKE_REPLACEMENT_READY_FILE\" && port_owned_by_pid \"$(cat \"$FAKE_REPLACEMENT_READY_FILE\")\" || { printf 'replacement-owner-timeout\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 81; }\n"
                "      wait_watcher_lost || exit 80\n"
                "      exit 0\n"
                "      ;;\n"
                "    *) printf 'invalid-mode\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 78 ;;\n"
                "  esac\n"
                "fi\n"
                "if [[ \"${1:-}\" == \"-\" && \"${2:-}\" == \"127.0.0.1\" && \"${3:-}\" == \"39999\" ]]; then exit 0; fi\n"
                "exec \"$real_python\" \"$@\"\n",
                encoding="utf-8",
            )
            fake_python.chmod(0o755)
            (fake_bin / "docker").write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$*\" == *\" port redis 6379\" ]]; then printf '127.0.0.1:39999\\n'; fi\n"
                "exit 0\n",
                encoding="utf-8",
            )
            (fake_bin / "docker").chmod(0o755)
            (fake_bin / "cargo").write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                f"real_python={shlex.quote(real_python)}\n"
                f"printf '%s\\n' \"$$\" > {shlex.quote(str(server_pid_file))}\n"
                "\"$real_python\" -u -c 'import socket, sys\ns = socket.socket()\ns.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\ns.bind((\"127.0.0.1\", int(sys.argv[1])))\ns.listen()\nwhile True:\n    conn, _ = s.accept()\n    conn.close()' \"$BOT_E2E_PORT\" &\n"
                "listener=$!\n"
                f"printf '%s\\n' \"$listener\" > {shlex.quote(str(listener_pid_file))}\n"
                "cleanup_listener() { kill \"$listener\" 2>/dev/null || true; wait \"$listener\" 2>/dev/null || true; exit 0; }\n"
                "trap cleanup_listener TERM INT\n"
                "for _ in $(seq 1 100); do\n"
                "  \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY' && break\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.1):\n"
                "    pass\n"
                "PY\n"
                "  sleep 0.01\n"
                "done\n"
                "printf '%s\\n' \"[bong][world] BOT_RASTER_FIXTURE_READY manifest=$BONG_TERRAIN_RASTER_PATH token=$BOT_E2E_AMBIENT_FIXTURE_TOKEN\"\n"
                "while true; do sleep 1; done\n",
                encoding="utf-8",
            )
            (fake_bin / "cargo").chmod(0o755)
            if tee_exit is not None:
                (fake_bin / "tee").write_text(
                    f"#!/usr/bin/env bash\nexit {tee_exit}\n", encoding="utf-8"
                )
                (fake_bin / "tee").chmod(0o755)

            evidence_root = root / ".sisyphus/evidence/bot-e2e"
            env = os.environ.copy()
            for name in (
                "BOT_E2E_REUSE", "BOT_E2E_HOST", "BOT_E2E_PORT",
                "BONG_TERRAIN_RASTER_PATH", "BONG_SPIRITWOOD_HARVESTED_PATH",
                "REDIS_URL", "BOT_E2E_AMBIENT_FIXTURE_OWNED",
            ):
                env.pop(name, None)
            env.update(
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BOT_E2E_PORT": str(port),
                    "FAKE_RUNNER_MODE": runner_mode,
                    "FAKE_RUNNER_EXIT": str(runner_exit),
                    "FAKE_SERVER_PID_FILE": str(server_pid_file),
                    "FAKE_LISTENER_PID_FILE": str(listener_pid_file),
                    "FAKE_EVIDENCE_ROOT": str(evidence_root),
                    "FAKE_RUNNER_RESULT_FILE": str(runner_result_file),
                    "FAKE_REPLACEMENT_READY_FILE": str(replacement_ready_file),
                    "BOT_E2E_WATCH_STATUS_EVIDENCE_PATH": str(watcher_status_file),
                }
            )
            env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
            evidence_before = set(evidence_root.glob("run.*")) if evidence_root.exists() else set()
            runner_output = ""
            runner_result = ""
            watcher_status = ""
            try:
                result = subprocess.run(
                    ["bash", "scripts/bot-e2e.sh"],
                    cwd=root,
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                    timeout=30,
                )
                runner_output = (
                    runner_log.read_text(encoding="utf-8") if runner_log.exists() else ""
                )
                runner_result = (
                    runner_result_file.read_text(encoding="utf-8")
                    if runner_result_file.exists()
                    else ""
                )
                watcher_status = (
                    watcher_status_file.read_text(encoding="utf-8")
                    if watcher_status_file.exists()
                    else ""
                )
            finally:
                for pid_file in (server_pid_file, listener_pid_file, replacement_pid_file):
                    if pid_file.exists():
                        try:
                            os.kill(int(pid_file.read_text(encoding="utf-8").strip()), 15)
                        except ProcessLookupError:
                            pass
                if evidence_root.exists():
                    for evidence_dir in set(evidence_root.glob("run.*")) - evidence_before:
                        shutil.rmtree(evidence_dir, ignore_errors=True)
            return result, runner_output, runner_result, watcher_status

    @staticmethod
    def _unused_local_port() -> int:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("127.0.0.1", 0))
            return int(sock.getsockname()[1])

    def test_owned_fixture_runtime_watcher_accepts_successful_owned_runner(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("success")
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(runner_output.strip(), "success")
        self.assertEqual(runner_result.strip(), "runner-complete")
        self.assertEqual(watcher_status.strip(), "complete")

    def test_owned_fixture_runtime_watcher_rejects_server_exit_during_runner(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("kill-server")
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(runner_output.strip(), "kill-server")
        self.assertEqual(runner_result.strip(), "watcher-lost-observed")
        self.assertEqual(watcher_status.strip(), "lost")
        self.assertIn("失去端口 ownership", result.stderr)

    def test_owned_fixture_runtime_watcher_rejects_replacement_listener(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("replace-listener")
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(runner_output.strip(), "replace-listener")
        self.assertEqual(runner_result.strip(), "watcher-lost-observed")
        self.assertEqual(watcher_status.strip(), "lost")
        self.assertIn("失去端口 ownership", result.stderr)

    def test_owned_fixture_runtime_preserves_runner_then_tee_failure_priority(self):
        for runner_exit, tee_exit, expected in ((7, None, 7), (7, 9, 7), (0, 9, 9)):
            with self.subTest(runner_exit=runner_exit, tee_exit=tee_exit):
                mode = "runner-fail" if runner_exit else "success"
                result, runner_output, runner_result, watcher_status = (
                    self._run_owned_fixture_runtime_case(
                        mode, runner_exit=runner_exit, tee_exit=tee_exit
                    )
                )
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertTrue(runner_output.strip())
                self.assertEqual(
                    runner_result.strip(),
                    "runner-failed" if runner_exit else "runner-complete",
                )
                self.assertEqual(watcher_status.strip(), "complete")


class NorthRiftPreviewHarnessContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = (
            pathlib.Path(__file__).parents[2] / "scripts/e2e-redis.sh"
        ).read_text(encoding="utf-8")

    def test_manifest_tracks_dedicated_server_and_bot_evidence(self):
        start = self.source.index("write_manifest() {")
        end = self.source.index("\n}\n\nfinalize_failure()", start)
        manifest_body = self.source[start:end]

        for evidence_var in ("NORTH_RIFT_SERVER_LOG", "NORTH_RIFT_BOT_LOG"):
            with self.subTest(evidence=evidence_var):
                self.assertIn(
                    f'"${evidence_var}"',
                    manifest_body,
                    f"Finish evidence manifest 必须收录 {evidence_var}",
                )

    def test_preview_env_is_scoped_after_tps_gate_and_around_one_scenario(self):
        phase_start = self.source.index('CURRENT_STAGE="north-rift-preview"')
        phase_end = self.source.index('CURRENT_STAGE="summary"', phase_start)
        phase = self.source[phase_start:phase_end]

        self.assertEqual(
            self.source.count('start_server_process_group "$NORTH_RIFT_SERVER_LOG" 1'),
            1,
            "preview mode 只能传给 dedicated north-rift server launch",
        )
        first_stop = phase.index("if ! stop_server;")
        preview_env = phase.index(
            'start_server_process_group "$NORTH_RIFT_SERVER_LOG" 1'
        )
        scenario = phase.index("--scenario terrain_north_rift_scorch_zone_identity")
        second_stop = phase.index("if ! stop_server;", first_stop + 1)
        self.assertLess(first_stop, preview_env, "必须先停普通 100 NPC server 再开 preview")
        self.assertLess(preview_env, scenario, "专用 server 激活 preview 后才能运行真实 bot")
        self.assertLess(scenario, second_stop, "唯一场景结束后必须立即 stop_server")
        self.assertIn("BOT_E2E_NORTH_RIFT_PREVIEW=1", phase)
        self.assertIn(
            'BONG_ROGUE_SEED_COUNT="$([ "$preview_mode" -eq 1 ]',
            self.source,
            "dedicated preview launch 必须显式禁用 rogue seed",
        )

    def test_stop_server_delegates_shared_lifecycle_and_cleanup_fails_closed(self):
        start = self.source.index("stop_server() {")
        end = self.source.index("\n}\n\ncleanup()", start)
        stop_body = self.source[start:end]
        helper_branch = stop_body.index(
            "if bong_server_stop_owned_process_group_and_release_port"
        )
        empty_pid_port_check = stop_body.index(
            "bong_server_confirm_port_released 25565", helper_branch
        )
        clear_pid = stop_body.index('SERVER_PID=""', helper_branch)
        success_return = stop_body.index("return 0", clear_pid)
        capture_status = stop_body.index("stop_status=$?", success_return)
        failure_return = stop_body.index('return "$stop_status"', capture_status)
        branch_end = stop_body.index("\n  fi", failure_return)

        self.assertLess(
            helper_branch,
            clear_pid,
            "e2e stop_server 必须先由共享 helper 确认停树和端口释放，才能清空 PID",
        )
        self.assertLess(clear_pid, success_return)
        self.assertLess(success_return, capture_status)
        self.assertLess(capture_status, failure_return)
        self.assertLess(failure_return, branch_end)
        self.assertIn(
            'return "$stop_status"',
            stop_body,
            "共享 helper 的 forced/uncertain 状态必须原样传播，不能压平为普通失败",
        )
        self.assertIn(
            'if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then',
            stop_body,
            "只有 READY persistence cleanup 的空 PID 分支才必须获取新端口证据",
        )
        self.assertLess(
            stop_body.index('if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then'),
            empty_pid_port_check,
            "空 PID 的端口探测必须受 READY restore gate 约束",
        )
        self.assertEqual(
            stop_body.count('SERVER_PID=""'),
            1,
            "SERVER_PID 只能在共享 helper 成功分支清空一次",
        )
        self.assertEqual(
            stop_body.count('SERVER_PGID=""'),
            1,
            "SERVER_PGID 只能在整组确认退出后清空一次",
        )
        for authority_var, local_name in (
            ("SERVER_OWNER_STARTTIME", "owner_starttime"),
            ("SERVER_OWNER_EXECUTABLE_IDENTITY", "owner_executable_identity"),
        ):
            with self.subTest(authority=authority_var):
                self.assertIn(
                    f'local {local_name}="${authority_var}"',
                    stop_body,
                    "stop_server 必须把 pinned supervisor 身份传给共享 helper",
                )
                self.assertEqual(
                    stop_body.count(f'{authority_var}=""'),
                    1,
                    "supervisor authority 只能在整组确认退出后清空",
                )
        self.assertIn('if [ "$SERVER_AUTHORITY_UNCERTAIN" -ne 0 ]; then', stop_body)
        self.assertLess(
            stop_body.index('if [ "$SERVER_AUTHORITY_UNCERTAIN" -ne 0 ]; then'),
            helper_branch,
            "authority 尚未完整固定时必须先 fail closed，不能扫描裸 PGID",
        )
        self.assertNotIn("authority_path=", self.source)
        self.assertNotIn("setsid --fork", self.source)
        self.assertIn("bong-process-group-supervisor.py", self.source)
        for legacy_detail in (
            'kill_tree "$pid"',
            'wait "$pid"',
            "port_open 25565",
        ):
            with self.subTest(legacy_detail=legacy_detail):
                self.assertNotIn(
                    legacy_detail,
                    stop_body,
                    "stop_server 不得重新内联共享 lifecycle helper 的停树/等待/端口逻辑",
                )

        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\n\ntrap cleanup EXIT", cleanup_start)
        cleanup_body = self.source[cleanup_start:cleanup_end]
        unconfirmed = cleanup_body.index("STOP_SERVER_CONFIRMED=0")
        stop_if = cleanup_body.index("if stop_server; then", unconfirmed)
        confirmed = cleanup_body.index("STOP_SERVER_CONFIRMED=1", stop_if)
        stop_else = cleanup_body.index("\n  else", confirmed)
        stop_fi = cleanup_body.index("\n  fi", stop_else)
        finalize = cleanup_body.index(
            "bong_server_finalize_preview_persistence_after_stop", stop_fi
        )
        finalize_end = cleanup_body.index("; then", finalize)
        finalize_call = cleanup_body[finalize:finalize_end]

        self.assertLess(unconfirmed, stop_if)
        self.assertLess(
            stop_if,
            confirmed,
            "cleanup 只能在 stop_server 成功分支标记停服已确认",
        )
        self.assertLess(confirmed, stop_else)
        self.assertEqual(
            cleanup_body.count("STOP_SERVER_CONFIRMED=1"),
            1,
            "cleanup 只能从 stop_server 成功分支产生唯一确认状态",
        )
        self.assertNotIn(
            "stop_server || true",
            cleanup_body,
            "停服失败不能被吞掉，否则可能错误恢复仍被 preview server 占用的 SQLite",
        )
        self.assertIn(
            '"$STOP_SERVER_CONFIRMED"',
            finalize_call,
            "cleanup 必须把停服确认结果交给共享 finalize helper 决定 restore 或 durable abort",
        )

    def test_preview_persistence_interval_holds_shared_lifecycle_lock(self):
        phase_start = self.source.index('CURRENT_STAGE="north-rift-preview"')
        phase_end = self.source.index('CURRENT_STAGE="summary"', phase_start)
        phase = self.source[phase_start:phase_end]
        fn_start = phase.index("run_north_rift_preview() {")
        fn_end = phase.index("\n}\n\nif ! bong_server_with_preview_persistence_lock", fn_start)
        preview_body = phase[fn_start:fn_end]
        lock_call = phase.index(
            "bong_server_with_preview_persistence_lock run_north_rift_preview",
            fn_end,
        )

        for required_step in (
            "if ! stop_server; then",
            "bong_server_confirm_port_released 25565",
            'bong_server_persistence_transaction_begin "$ROOT/server/data"',
            'bong_server_stash_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"',
            '--scenario terrain_north_rift_scorch_zone_identity',
            'bong_server_restore_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"',
            "bong_server_persistence_transaction_complete",
        ):
            with self.subTest(step=required_step):
                self.assertIn(
                    required_step,
                    preview_body,
                    "preview lifecycle lock 的函数体必须覆盖完整 stash→run→restore 临界区",
                )
        self.assertGreater(
            lock_call,
            fn_end,
            "完整 preview transaction 必须经生产 start/reload 共用的 lifecycle lock 调用",
        )


class _FakeEvent:
    def __init__(self, t: float, kind: str, data: dict):
        self.t = t
        self.kind = kind
        self.data = data

    def __repr__(self) -> str:
        return f"_FakeEvent({self.t} {self.kind} {self.data})"


class _FakeBot:
    username = "Fake"

    def __init__(self, events: list[_FakeEvent]):
        self.events = events

    def events_of(self, kind: str) -> list[_FakeEvent]:
        return [event for event in self.events if event.kind == kind]

    def expect_server_data(self, payload_type: str, timeout: float = 5.0) -> _FakeEvent:
        return self.wait_for(
            lambda e: e.kind == "server_data" and e.data["payload_type"] == payload_type,
            timeout,
            f"server_data/{payload_type}",
        )

    def wait_for(self, predicate, timeout: float, description: str) -> _FakeEvent:
        for event in self.events:
            if predicate(event):
                return event
        raise AssertionError(f"未找到 {description}; events={self.events}")


class _CommandFakeBot(_FakeBot):
    def __init__(self, events: list[_FakeEvent], pending: list[_FakeEvent]):
        super().__init__(events)
        self._lock = threading.Lock()
        self.pending = list(pending)
        self.commands: list[str] = []

    def cmd(self, command: str) -> None:
        self.commands.append(command)

    def wait_for(self, predicate, timeout: float, description: str) -> _FakeEvent:
        while True:
            for event in self.events:
                if predicate(event):
                    return event
            if not self.pending:
                raise AssertionError(f"未找到 {description}; events={self.events}")
            self.events.append(self.pending.pop(0))


def _snapshot_event(t: float, revision: int, marker: str) -> _FakeEvent:
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "inventory_snapshot",
            "payload": {
                "type": "inventory_snapshot",
                "revision": revision,
                "marker": marker,
            },
        },
    )


def _pill_snapshot_event(t: float, revision: int, count: int, qi: float) -> _FakeEvent:
    placed_items = []
    if count > 0:
        placed_items.append(
            {
                "container_id": "body_pocket",
                "row": 0,
                "col": 0,
                "item": {
                    "instance_id": 7,
                    "item_id": PILL_ID,
                    "stack_count": count,
                },
            }
        )
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "inventory_snapshot",
            "payload": {
                "type": "inventory_snapshot",
                "revision": revision,
                "placed_items": placed_items,
                "equipped": {},
                "hotbar": [],
                "qi_current": qi,
            },
        },
    )


def _player_state_event(t: float, qi: float, qi_max: float = 100.0) -> _FakeEvent:
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "player_state",
            "payload": {
                "type": "player_state",
                "spirit_qi": qi,
                "spirit_qi_max": qi_max,
            },
        },
    )


class CultivationPillScenarioTest(unittest.TestCase):
    def test_qi_set_confirmation_is_anchored_to_exact_target(self):
        good = _FakeEvent(2.0, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})
        wrong_target = _FakeEvent(2.0, "chat", {"text": "[dev] qi set 5.0 -> 95.0"})
        misleading = _FakeEvent(2.0, "chat", {"text": "prefix [dev] qi set 5.0 -> 5.0"})
        at_anchor = _FakeEvent(1.0, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})
        non_chat = _FakeEvent(2.0, "server_data", {"text": "[dev] qi set 95.0 -> 5.0"})

        self.assertTrue(
            _is_qi_set_confirmation(good, 1.0, 5.0),
            "完整前后缀且目标为 5.0 的确认应被接受",
        )
        self.assertFalse(
            _is_qi_set_confirmation(wrong_target, 1.0, 5.0),
            "其他目标的历史确认不得满足本次 qi set 5",
        )
        self.assertFalse(
            _is_qi_set_confirmation(misleading, 1.0, 5.0),
            "仅在正文中包含 qi set 片段的聊天不得被误认成确认",
        )
        self.assertFalse(
            _is_qi_set_confirmation(at_anchor, 1.0, 5.0),
            "event.t == anchor 属于命令前水位，不得满足本次 qi set 确认",
        )
        self.assertFalse(
            _is_qi_set_confirmation(non_chat, 1.0, 5.0),
            "非 chat 事件即使正文相同也不得满足 qi set 确认",
        )

    def test_authoritative_qi_wait_uses_command_anchor_not_chat_order(self):
        authoritative = _player_state_event(1.1, 5.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史事件"})],
            [
                authoritative,
                _FakeEvent(1.2, "chat", {"text": "[dev] qi set 95.0 -> 5.0"}),
            ],
        )

        result = _set_qi_and_wait(bot, 5.0)

        self.assertIs(
            result,
            authoritative,
            "player_state 可能与 chat 同 tick 乱序，权威 qi 等待必须锚定发命令前水位线",
        )

    def test_authoritative_qi_wait_rejects_stale_event_before_anchor(self):
        bot = _CommandFakeBot(
            [
                _player_state_event(1.0, 5.0),
                _FakeEvent(2.0, "chat", {"text": "历史水位"}),
            ],
            [_FakeEvent(2.1, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state",
            msg="anchor 前的目标 qi 快照不得满足命令后的权威状态等待",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_set_wait_rejects_wrong_confirmation_target(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [_FakeEvent(1.1, "chat", {"text": "[dev] qi set 5.0 -> 95.0"})],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "精确目标",
            msg="错误目标的 chat 确认必须让 qi set 等待超时",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_set_wait_rejects_wrong_authoritative_value(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] qi set 95.0 -> 5.0"}),
                _player_state_event(1.2, 6.0),
            ],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state",
            msg="确认 chat 正确但权威 qi 错误时必须超时",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_max_wait_consumes_authoritative_player_state(self):
        authoritative = _player_state_event(1.1, 5.0, 80.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                authoritative,
                _FakeEvent(
                    1.2,
                    "chat",
                    {"text": "[dev] qi max 100.0 -> 80.0; current=5.0"},
                ),
            ],
        )

        result = _set_qi_max_and_wait(bot, 80.0)

        self.assertIs(result, authoritative)
        self.assertEqual(bot.commands, ["qi max 80.0"])
        self.assertEqual(_player_state_values(result), (5.0, 80.0))

    def test_qi_max_confirmation_is_anchored_to_exact_target(self):
        good = _FakeEvent(
            2.0,
            "chat",
            {"text": "[dev] qi max 100.0 -> 80.0; current=5.0"},
        )
        wrong = _FakeEvent(
            2.0,
            "chat",
            {"text": "[dev] qi max 100.0 -> 90.0; current=5.0"},
        )
        self.assertTrue(_is_qi_max_confirmation(good, 1.0, 80.0))
        self.assertFalse(_is_qi_max_confirmation(wrong, 1.0, 80.0))
        self.assertFalse(_is_qi_max_confirmation(good, 2.0, 80.0))

    def test_clamp_target_comes_from_authoritative_non_default_qi_max(self):
        self.assertEqual(_expected_qi_after_pill(5.0, 80.0), 65.0)
        self.assertEqual(
            _expected_qi_after_pill(70.0, 80.0),
            80.0,
            "clamp 目标必须由权威 qi_max=80 推导，不能硬编码 100",
        )

    def test_settled_consumption_carries_non_default_qi_max_through_state_machine(self):
        final = _assert_settled_consumption(
            [
                _pill_snapshot_event(1.05, 10, 3, 70.0),
                _player_state_event(1.1, 70.0, 80.0),
                _pill_snapshot_event(1.2, 11, 2, 80.0),
                _player_state_event(1.3, 80.0, 80.0),
                _player_state_event(1.4, 80.0, 80.0),
            ],
            before_revision=10,
            before_count=3,
            baseline_qi=70.0,
            expected_qi=_expected_qi_after_pill(70.0, 80.0),
            expected_qi_max=80.0,
        )

        self.assertEqual(final["revision"], 11)

    def test_non_finite_authoritative_qi_fails_before_later_target(self):
        for invalid in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(
                AssertionError,
                "spirit_qi 必须是有限数",
                msg="5 -> 非有限值 -> 65 不得静默跳过非法中间态",
            ):
                _assert_settled_consumption(
                    [
                        _pill_snapshot_event(1.1, 11, 2, 65.0),
                        _player_state_event(1.15, 5.0),
                        _player_state_event(1.2, invalid),
                        _player_state_event(1.3, 65.0),
                    ],
                    before_revision=10,
                    before_count=3,
                    baseline_qi=5.0,
                    expected_qi=65.0,
                    expected_qi_max=100.0,
                )

    def test_missing_or_non_finite_authoritative_qi_max_fails(self):
        invalid_values = (None, 0.0, float("nan"), float("inf"), float("-inf"))
        for invalid in invalid_values:
            event = _player_state_event(1.0, 5.0)
            if invalid is None:
                event.data["payload"].pop("spirit_qi_max")
                message = "必须是数值"
            else:
                event.data["payload"]["spirit_qi_max"] = invalid
                message = "必须是有限正数"
            with self.subTest(invalid=invalid), self.assertRaisesRegex(
                AssertionError, message
            ):
                _player_state_values(event)

    def test_server_tick_fence_crosses_requested_tick_target_before_snapshot(self):
        self.assertEqual(
            SERVER_TICK_OBSERVATION_TICKS,
            20,
            "生产服丹观察窗口必须继续覆盖至少 20 个权威 server tick",
        )
        duplicate = _pill_snapshot_event(1.55, 12, 1, 65.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 101"}),
                _FakeEvent(1.3, "chat", {"text": "[dev] time now: 102"}),
                _FakeEvent(1.4, "chat", {"text": "[dev] time now: 103"}),
                _FakeEvent(1.5, "chat", {"text": "[dev] time now: 103"}),
                duplicate,
                _FakeEvent(1.6, "chat", {"text": "[dev] time now: 104"}),
            ],
        )

        snapshot = _snapshot_after_server_tick_fence(
            bot, minimum_ticks=2, timeout=1.0
        )

        self.assertIn(
            duplicate,
            snapshot,
            "首个 drain chat 后、第二个 drain marker 前的重复帧必须进入快照",
        )
        self.assertEqual(
            bot.commands,
            ["time now"] * 6,
            "100 起点 +2 后先跨到 103，再做两次 round-trip 才覆盖 post-emit 帧",
        )

    def test_server_tick_fence_rejects_malformed_or_rollback_tick(self):
        malformed = _FakeEvent(2.0, "chat", {"text": "[dev] time now: 12x"})
        with self.assertRaisesRegex(AssertionError, "十进制 tick"):
            _server_tick_from_event(malformed, 1.0)

        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 99"}),
            ],
        )
        with self.assertRaisesRegex(AssertionError, "不得回退"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=1.0)

    def test_server_tick_fence_rejects_invalid_bounds_and_deadlines(self):
        bot = _CommandFakeBot([], [])
        with self.assertRaisesRegex(ValueError, "minimum_ticks"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=0, timeout=1.0)
        with self.assertRaisesRegex(ValueError, "timeout"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=0.0)

        main_timeout_bot = _CommandFakeBot(
            [],
            [_FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"})],
        )
        with mock.patch(
            "bot.scenarios.cultivation_pill_consume.time.monotonic",
            side_effect=[0.0, 2.0],
        ), self.assertRaisesRegex(BotAssertionError, "推进至少"):
            _snapshot_after_server_tick_fence(
                main_timeout_bot, minimum_ticks=1, timeout=1.0
            )

        drain_timeout_bot = _CommandFakeBot(
            [],
            [
                _FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 102"}),
            ],
        )
        with mock.patch(
            "bot.scenarios.cultivation_pill_consume.time.monotonic",
            side_effect=[0.0, 0.1, 2.0],
        ), self.assertRaisesRegex(BotAssertionError, "post-emit drain"):
            _snapshot_after_server_tick_fence(
                drain_timeout_bot, minimum_ticks=1, timeout=1.0
            )

    def test_server_tick_fence_rejects_rollback_during_post_emit_drain(self):
        bot = _CommandFakeBot(
            [],
            [
                _FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 102"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 101"}),
            ],
        )
        with self.assertRaisesRegex(AssertionError, "不得回退"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=1.0)

    def test_stale_same_tick_baseline_is_not_new_authoritative_value(self):
        self.assertFalse(
            _has_departed_baseline(5.0, 5.0, 65.0),
            "同 tick 残留的旧基线不得被当作服丹结果",
        )
        self.assertTrue(
            _has_departed_baseline(65.0, 5.0, 65.0),
            "真实恢复到 65 应离开旧基线",
        )
        self.assertTrue(
            _has_departed_baseline(20.0, 5.0, 65.0),
            "任何非基线权威值都必须进入校验，不能用中点过滤错误中间态",
        )
        self.assertFalse(
            _has_departed_baseline(65.0, 65.0, 5.0),
            "下降目标下旧基线 65 不得被当作新值",
        )
        self.assertTrue(
            _has_departed_baseline(5.0, 65.0, 5.0),
            "下降目标真实到达 5 时应越过变化屏障",
        )
        self.assertFalse(
            _has_departed_baseline(5.0, 5.0, 5.0),
            "目标等于基线时没有可观察状态转换，不得伪称已离开基线",
        )
        final = _assert_settled_consumption(
            [
                _pill_snapshot_event(1.05, 10, 3, 5.0),
                _player_state_event(1.1, 5.0),
                _pill_snapshot_event(1.2, 11, 2, 65.0),
                _player_state_event(1.3, 65.0),
                _player_state_event(1.4, 65.0),
            ],
            before_revision=10,
            before_count=3,
            baseline_qi=5.0,
            expected_qi=65.0,
            expected_qi_max=100.0,
        )
        self.assertEqual(
            final["revision"], 11,
            "正常一次消费应只把 inventory revision 从 10 推进到 11",
        )

    def test_huiyuan_asset_pins_expected_qi_recovery(self):
        asset = pathlib.Path(__file__).parents[2] / "server/assets/items/pills.toml"
        items = tomllib.loads(asset.read_text(encoding="utf-8"))["item"]
        huiyuan = next(item for item in items if item["id"] == PILL_ID)
        self.assertEqual(
            huiyuan["effect"]["kind"],
            "qi_recovery",
            f"{PILL_ID} 必须继续走 qi_recovery，实际 effect={huiyuan['effect']}",
        )
        self.assertEqual(
            float(huiyuan["effect"]["magnitude"]), PILL_QI_RECOVERY,
            f"场景药效推导常量应为 {PILL_QI_RECOVERY}，"
            f"实际 asset effect={huiyuan['effect']}",
        )
        self.assertEqual(
            _expected_qi_after_pill(5.0, 100.0),
            NON_CLAMP_EXPECTED_QI,
            f"qi=5、权威 qi_max=100 时应推导 {NON_CLAMP_EXPECTED_QI}",
        )

    def test_repeated_qi_effect_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state 应持续稳定",
            msg="第二次真元生效必须命中权威状态稳定性断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _player_state_event(1.3, 100.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_repeated_inventory_decrement_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="第二次扣存推进 revision 必须命中逐帧 revision 断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 12, 1, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_repeated_decrement_then_surface_rollback_still_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="11/2 -> 12/1 -> 11/2 的表面正确终态仍必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 12, 1, 65.0),
                    _pill_snapshot_event(1.4, 11, 2, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_consumed_snapshot_rollback_to_old_state_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="消费后回滚到旧 revision/count 必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 10, 3, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_same_revision_count_change_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版丹药数量必须保持",
            msg="revision 未变但丹药再次减少也必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 11, 1, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_inventory_qi_regression_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory qi 必须保持",
            msg="库存快照 qi 回滚必须命中逐帧 qi 断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 11, 2, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_authoritative_qi_regression_after_effect_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state 应持续稳定",
            msg="权威真元先恢复到 65 又回落到旧基线时必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 5.0),
                    _player_state_event(1.3, 65.0),
                    _player_state_event(1.4, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_wrong_authoritative_qi_before_target_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "错误中间态",
            msg="5 -> 20 -> 65 中的 20 是消费后的错误权威态，不得被中点过滤",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.15, 5.0),
                    _player_state_event(1.2, 20.0),
                    _player_state_event(1.3, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )


def _server_data_zone_info_bytes() -> bytes:
    zone_info = (
        _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "rift_mouth_north_002")
        + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.068602)
        + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 5)
        + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Normal")
        + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "rift_mouth_entry")
        + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "wind_warning")
        + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "灵气骤薄")
    )
    return _pb_message(proto_min.SERVER_DATA_ZONE_INFO_FIELD, zone_info)


def _server_data_inventory_snapshot_bytes() -> bytes:
    item = (
        _pb_varint(1, 9)
        + _pb_string(2, "worn_grass_pouch")
        + _pb_string(3, "破草包")
        + _pb_varint(4, 2)
        + _pb_varint(5, 2)
        + _pb_fixed64(6, 0.25)
        + _pb_string(7, "common")
        + _pb_string(8, "test")
        + _pb_varint(9, 1)
        + _pb_fixed64(10, 0.0)
        + _pb_fixed64(11, 0.3)
    )
    container = (
        _pb_string(1, "body_pocket")
        + _pb_string(2, "贴身口袋")
        + _pb_varint(3, 2)
        + _pb_varint(4, 3)
    )
    equipped = _pb_message(3, item)
    weight = _pb_fixed64(1, 1.0) + _pb_fixed64(2, 23.0)
    snapshot = (
        _pb_varint(1, 12)
        + _pb_message(2, container)
        + _pb_message(4, equipped)
        + _pb_message(7, weight)
    )
    return _pb_message(8, snapshot)


def _server_data_narration_bytes() -> bytes:
    target = (
        _pb_string(1, "天道的注意力掠过，境界未至")
        + _pb_string(2, "player")
        + _pb_string(3, "system_warning")
        + _pb_string(4, "offline:Alice")
        + _pb_string(5, "realm_gate_rejected")
    )
    broadcast = (
        _pb_string(1, "旧式全服旁白")
        + _pb_string(2, "broadcast")
        + _pb_string(3, "narration")
    )
    return _pb_message(3, _pb_message(1, target) + _pb_message(1, broadcast))


def _server_data_inventory_event_moved_bytes() -> bytes:
    from_location = _pb_message(
        1,
        _pb_string(1, "body_pocket") + _pb_varint(2, 0) + _pb_varint(3, 1),
    )
    to_location = _pb_message(2, _pb_varint(1, 2) + _pb_varint(2, 1))
    moved = (
        _pb_varint(1, 13)
        + _pb_varint(2, 99)
        + _pb_message(3, from_location)
        + _pb_message(4, to_location)
    )
    return _pb_message(80, _pb_message(1, moved))


def _server_data_loot_container_open_bytes() -> bytes:
    open_payload = (
        _pb_varint(1, 7)
        + _pb_string(2, '{"kind":"storage_crate","is_herb":false}')
        + _pb_varint(3, 3)
        + _pb_varint(4, 4)
    )
    return _pb_message(119, open_payload)


def _server_data_loot_container_update_bytes() -> bytes:
    item = (
        _pb_varint(1, 99)
        + _pb_string(2, "refined_iron")
        + _pb_string(3, "精铁")
        + _pb_varint(4, 1)
        + _pb_varint(5, 1)
        + _pb_fixed64(6, 0.1)
        + _pb_string(7, "common")
        + _pb_string(8, "test")
        + _pb_varint(9, 2)
        + _pb_fixed64(10, 0.0)
        + _pb_fixed64(11, 1.0)
    )
    placed = (
        _pb_string(1, "ext_7")
        + _pb_varint(2, 0)
        + _pb_varint(3, 1)
        + _pb_message(4, item)
    )
    return _pb_message(120, _pb_varint(1, 7) + _pb_message(2, placed))


def _server_data_loot_container_close_bytes() -> bytes:
    return _pb_message(121, _pb_varint(1, 7) + _pb_string(2, "distance"))


def _server_data_morph_state_bytes(
    *,
    mode: str,
    entity_id: int,
    model_kind: int,
    form_race_id: str,
    form_body_plan_id: str,
    active: bool,
) -> bytes:
    """plan-race-system-v1 P4 — field 142 `morph_state`（见 proto/bong/common.proto
    `MorphState`/`MorphStateEntry`）。"""
    entry = (
        _pb_varint(1, entity_id)
        + _pb_varint(2, model_kind)
        + _pb_string(3, form_race_id)
        + _pb_string(4, form_body_plan_id)
        + _pb_varint(5, 1 if active else 0)
    )
    state = _pb_varint(1, 1) + _pb_string(2, mode) + _pb_message(3, entry)
    return _pb_message(142, state)


def _pb_key(field: int, wire: int) -> bytes:
    return _pb_raw_varint((field << 3) | wire)


def _pb_raw_varint(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def _pb_varint(field: int, value: int) -> bytes:
    return _pb_key(field, 0) + _pb_raw_varint(value)


def _pb_fixed64(field: int, value: float) -> bytes:
    return _pb_key(field, 1) + struct.pack("<d", value)


def _pb_fixed32(field: int, value: float) -> bytes:
    return _pb_key(field, 5) + struct.pack("<f", value)


def _pb_bytes(field: int, value: bytes) -> bytes:
    return _pb_key(field, 2) + _pb_raw_varint(len(value)) + value


def _pb_string(field: int, value: str) -> bytes:
    return _pb_bytes(field, value.encode("utf-8"))


def _pb_message(field: int, value: bytes) -> bytes:
    return _pb_bytes(field, value)


def _pb_varint_field(number: int, value: int) -> bytes:
    return mc.write_varint(number << 3) + mc.write_varint(value)


def _pb_u64_varint_field(number: int, value: int) -> bytes:
    """protobuf uint64 字段（mc.write_varint 是 32 位 MC varint，装不下 u64）。"""
    body = bytearray()
    remaining = value
    while remaining >= 0x80:
        body.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    body.append(remaining)
    return mc.write_varint(number << 3) + bytes(body)


def _pb_len_field(number: int, value: bytes) -> bytes:
    return mc.write_varint((number << 3) | 2) + mc.write_varint(len(value)) + value


class ProtoMinTest(unittest.TestCase):
    def test_server_data_payload_name_reads_oneof_field(self):
        envelope = _pb_len_field(31, b"\x08\x01")
        self.assertEqual(proto_min.server_data_payload_name(envelope), "lingtian_session")

    def test_inventory_snapshot_extracts_placed_item_location(self):
        item = (
            _pb_varint_field(1, 4242)
            + _pb_len_field(2, b"furnace_fantie")
            + _pb_len_field(3, "凡铁炉".encode("utf-8"))
        )
        placed = (
            _pb_len_field(1, b"main_pack")
            + _pb_varint_field(2, 1)
            + _pb_varint_field(3, 2)
            + _pb_len_field(4, item)
        )
        inventory = _pb_len_field(3, placed)
        envelope = _pb_len_field(8, inventory)

        refs = proto_min.inventory_item_refs(envelope)
        self.assertEqual(len(refs), 1)
        self.assertEqual(refs[0].instance_id, 4242)
        self.assertEqual(refs[0].item_id, "furnace_fantie")
        self.assertEqual(
            refs[0].location,
            {"kind": "container", "container_id": "main_pack", "row": 1, "col": 2},
        )

    def test_inventory_snapshot_extracts_equipped_item_location(self):
        item = _pb_varint_field(1, 77) + _pb_len_field(2, b"hoe_iron")
        equipped = _pb_len_field(10, item)
        inventory = _pb_len_field(4, equipped)
        envelope = _pb_len_field(8, inventory)

        refs = proto_min.inventory_item_refs(envelope)
        self.assertEqual(refs[0].location, {"kind": "equip", "slot": "main_hand", "state": "held"})

    def test_trade_offer_decodes_offer_and_item_summaries(self):
        offered = (
            _pb_u64_varint_field(1, 1001)
            + _pb_len_field(2, b"starter_talisman")
            + _pb_len_field(3, b"talisman")
            + _pb_varint_field(4, 1)
        )
        requested = (
            _pb_u64_varint_field(1, 2002)
            + _pb_len_field(2, b"huiyuan_pill")
            + _pb_len_field(3, b"pill")
            + _pb_varint_field(4, 3)
        )
        trade_offer = (
            _pb_len_field(1, b"trade:00000000-0000-7000-8000-000000000000")
            + _pb_len_field(2, b"char:alice")
            + _pb_len_field(3, b"char:bob")
            + _pb_len_field(4, offered)
            + _pb_len_field(5, requested)
            + _pb_u64_varint_field(6, 9876543210)
        )
        envelope = _pb_len_field(65, trade_offer)

        self.assertEqual(proto_min.server_data_payload_name(envelope), "trade_offer")
        payload = proto_min.decode_server_data_envelope(envelope)
        self.assertEqual(payload["type"], "trade_offer")
        self.assertEqual(payload["offer_id"], "trade:00000000-0000-7000-8000-000000000000")
        self.assertEqual(payload["initiator"], "char:alice")
        self.assertEqual(payload["target"], "char:bob")
        self.assertEqual(
            payload["offered_item"],
            {"instance_id": 1001, "item_id": "starter_talisman", "display_name": "talisman", "stack_count": 1},
        )
        self.assertEqual(
            payload["requested_items"],
            [{"instance_id": 2002, "item_id": "huiyuan_pill", "display_name": "pill", "stack_count": 3}],
        )
        self.assertEqual(payload["expires_at_ms"], 9876543210)

    def test_sparring_invite_payload_name_registered(self):
        envelope = _pb_len_field(64, _pb_len_field(1, b"sparring:1"))
        self.assertEqual(proto_min.server_data_payload_name(envelope), "sparring_invite")


def _bare_connection(threshold: int = -1) -> mc.Connection:
    """不开 socket 的 Connection —— 只测帧解析状态机。"""
    conn = mc.Connection.__new__(mc.Connection)
    conn.buf = b""
    conn.compression_threshold = threshold
    return conn


def _frame(payload: bytes) -> bytes:
    return mc.write_varint(len(payload)) + payload


class FrameParseTest(unittest.TestCase):
    def test_partial_buffers_preserved(self):
        conn = _bare_connection()
        payload = b"\x28" + b"x" * 300  # 两字节 varint 长度前缀
        frame = _frame(payload)
        # 任意截断点：半帧必须返回 None 且 buf 原样保留（timeout 落点安全）
        for cut in [0, 1, 2, 10, len(frame) - 1]:
            conn.buf = frame[:cut]
            self.assertIsNone(conn._try_parse_frame(), f"cut={cut} 时不该解析出帧")
            self.assertEqual(len(conn.buf), cut, f"cut={cut} 时半帧字节不能被消费")
        conn.buf = frame
        self.assertEqual(conn._try_parse_frame(), payload)
        self.assertEqual(conn.buf, b"")

    def test_two_frames_sequential(self):
        conn = _bare_connection()
        conn.buf = _frame(b"\x01aa") + _frame(b"\x02bb")
        self.assertEqual(conn._try_parse_frame(), b"\x01aa")
        self.assertEqual(conn._try_parse_frame(), b"\x02bb")
        self.assertIsNone(conn._try_parse_frame())

    def test_compressed_frames(self):
        conn = _bare_connection(threshold=64)
        # 阈值下未压缩：DataLength=0 + 原文
        small = b"\x17small"
        conn.buf = _frame(mc.write_varint(0) + small)
        self.assertEqual(conn._try_parse_frame(), small)
        # 达阈值 zlib：DataLength=原长 + 压缩体
        big = b"\x24" + b"y" * 200
        conn.buf = _frame(mc.write_varint(len(big)) + zlib.compress(big))
        self.assertEqual(conn._try_parse_frame(), big)


class RunnerLogicTest(unittest.TestCase):
    def test_new_bot_rejects_long_username(self):
        env = ScenarioEnv("127.0.0.1", 1, run_tag="12345678901234")
        with self.assertRaises(ValueError, msg="用户名超 16 字符必须在连接前报错"):
            env.new_bot("LongTag")

    def test_bot_rejects_long_username_before_connect(self):
        with self.assertRaises(ValueError):
            Bot("A" * 17, host="127.0.0.1", port=1)

    def test_validate_scenario_module_contract(self):
        good = types.SimpleNamespace(DESCRIPTION="d", MODULES=["m"], run=lambda env: None)
        validate_scenario_module("good", good)  # 不应抛
        for missing in ("DESCRIPTION", "MODULES", "run"):
            attrs = {"DESCRIPTION": "d", "MODULES": ["m"], "run": lambda env: None}
            del attrs[missing]
            with self.assertRaises(RuntimeError, msg=f"缺 {missing} 应被契约校验拒绝"):
                validate_scenario_module("bad", types.SimpleNamespace(**attrs))

    def test_discover_scenarios_finds_committed_set(self):
        names = set(discover_scenarios())
        expected = {
            "agent_ui_realm_gate_private_narration",
            "cmd_dev_give_feedback",
            "cultivation_realm_qi",
            "network_client_request_tolerance",
            "network_session_tolerance",
            "terrain_join_chunk_delivery",
            "terrain_north_rift_scorch_zone_identity",
        }
        self.assertTrue(
            expected <= names,
            f"已提交场景应全部被发现（模块更新必配场景的 CI 抓手），实际 {names}",
        )

    def test_north_rift_scenario_is_explicitly_dedicated(self):
        scenario = discover_scenarios()["terrain_north_rift_scorch_zone_identity"]
        self.assertFalse(
            scenario.DEFAULT_ENABLED,
            "north-rift preview 场景不得进入常规 --all server，避免 ViewDistance(32) 扩散",
        )
        self.assertEqual(
            scenario.REQUIRED_ENV,
            NORTH_RIFT_REQUIRED_ENV,
            "runner SKIP 提示与场景执行门必须引用同一个显式环境变量",
        )

    def test_all_skips_north_rift_even_when_preview_env_is_present(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=NORTH_RIFT_REQUIRED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {NORTH_RIFT_REQUIRED_ENV: "1"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_not_called()
        self.assertIn("SKIP", output.getvalue())
        self.assertIn("skip=1", output.getvalue())

    def test_all_runs_ambient_only_when_fixture_ownership_is_declared(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="owned fixture",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=FIXTURE_OWNED_ENV,
            RUN_IN_ALL_WHEN_ENV=FIXTURE_OWNED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"npc_ambient_surface_resolution": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {FIXTURE_OWNED_ENV: "1"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_called_once()
        self.assertIn("PASS", output.getvalue())
        self.assertIn("pass=1", output.getvalue())

    def test_all_skips_ambient_without_fixture_ownership(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="owned fixture",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=FIXTURE_OWNED_ENV,
            RUN_IN_ALL_WHEN_ENV=FIXTURE_OWNED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"npc_ambient_surface_resolution": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {FIXTURE_OWNED_ENV: "0"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_not_called()
        self.assertIn("SKIP", output.getvalue())

    def test_explicit_scenario_without_required_env_fails_closed(self):
        run = mock.Mock()
        required_env = "BOT_E2E_TEST_DEDICATED"
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=required_env,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {required_env: "0"}, clear=False),
            mock.patch.object(
                sys,
                "argv",
                [
                    "run_scenarios.py",
                    "--scenario",
                    "terrain_north_rift_scorch_zone_identity",
                ],
            ),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 1, "显式场景缺 REQUIRED_ENV 必须以非零退出")
        run.assert_not_called()
        self.assertIn("ERROR", output.getvalue())
        self.assertIn(f"需 {required_env}=1", output.getvalue())

    def test_explicit_scenario_with_required_env_runs_normally(self):
        run = mock.Mock()
        required_env = "BOT_E2E_TEST_DEDICATED"
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=required_env,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {required_env: "1"}, clear=False),
            mock.patch.object(
                sys,
                "argv",
                [
                    "run_scenarios.py",
                    "--scenario",
                    "terrain_north_rift_scorch_zone_identity",
                ],
            ),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0, "满足 REQUIRED_ENV 的显式场景应保留正常 PASS 语义")
        run.assert_called_once()
        self.assertIn("PASS", output.getvalue())
        self.assertIn("pass=1", output.getvalue())

    def test_scenarios_do_not_reuse_literal_bot_tags(self):
        owners: dict[str, str] = {}
        scenarios_dir = pathlib.Path(__file__).parent / "scenarios"
        for path in scenarios_dir.glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if not (
                    isinstance(node, ast.Call)
                    and isinstance(node.func, ast.Attribute)
                    and node.func.attr == "new_bot"
                    and node.args
                    and isinstance(node.args[0], ast.Constant)
                    and isinstance(node.args[0].value, str)
                ):
                    continue
                tag = node.args[0].value
                previous = owners.setdefault(tag, path.name)
                self.assertEqual(
                    previous,
                    path.name,
                    f"bot tag {tag!r} 被 {previous} 与 {path.name} 跨场景复用；"
                    "持久玩家状态会污染后续场景",
                )

    def test_check_server_reachable(self):
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        port = listener.getsockname()[1]
        accepted = threading.Thread(target=lambda: listener.accept(), daemon=True)
        accepted.start()
        try:
            self.assertTrue(check_server_reachable("127.0.0.1", port, timeout=2.0))
        finally:
            listener.close()
        self.assertFalse(check_server_reachable("127.0.0.1", 1, timeout=0.5))

    def test_intent_payload_is_valid_json_utf8(self):
        # intent() 的 wire 形状：channel string + UTF-8 JSON —— 锁编码不锁语义
        body = mc.mc_string("bong:client_request") + json.dumps(
            {"v": 1, "type": "breakthrough"}
        ).encode("utf-8")
        reader = mc.Reader(body)
        self.assertEqual(reader.string(), "bong:client_request")
        self.assertEqual(json.loads(reader.rest()), {"v": 1, "type": "breakthrough"})


def _bare_bot() -> Bot:
    """不开 socket 的 Bot——只测 _dispatch 解码与实体位置表状态机。"""
    import threading as _threading

    bot = Bot.__new__(Bot)
    bot.t0 = 0.0
    bot.events = []
    bot.entities = {}
    bot.player_names = {}
    bot.player_entity_uuids = {}
    bot._lock = _threading.RLock()
    bot._new_event = _threading.Condition(bot._lock)
    bot._send_lock = _threading.Lock()
    bot._closing = False
    bot.position = None
    bot.health = None
    bot.entity_id = None
    bot.disconnect_reason = None
    bot.chunk_count = 0
    return bot


class BotChatPacketTest(unittest.TestCase):
    def test_chat_packet_uses_unix_milliseconds_at_protocol_boundaries(self):
        cases = [
            (0, 0),
            (1_000_000_000, 1_000),
            (1_999_000_000, 1_999),
            (1_712_345_700_999_000_000, 1_712_345_700_999),
        ]

        for time_ns, expected_millis in cases:
            with self.subTest(expected_millis=expected_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )

                with mock.patch("bot.bot.time.time_ns", return_value=time_ns):
                    bot.chat("窗口验真")

                self.assertEqual(
                    len(sent),
                    1,
                    f"一次 Bot.chat 必须只发一帧 C2S chat，实际 sent={sent!r}",
                )
                packet_id, body = sent[0]
                self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)

                reader = mc.Reader(body)
                self.assertEqual(reader.string(), "窗口验真")
                self.assertEqual(
                    reader.i64(),
                    expected_millis,
                    "Minecraft 1.20.1 ChatMessageC2s timestamp 必须是 Unix 毫秒；"
                    "0/1000/1999 与真实 13 位值均不得误写成 Unix 秒",
                )
                self.assertEqual(reader.i64(), 0, "offline bot 的 salt 保持 0")
                self.assertFalse(reader.boolean(), "offline bot 不携带聊天签名")
                self.assertEqual(reader.varint(), 0, "message_count 保持 0")
                self.assertEqual(
                    reader.rest(),
                    b"\x00\x00\x00",
                    "acknowledged 必须保留协议 763 的 20-bit 固定 BitSet",
                )

    def test_chat_packet_can_model_a_forged_future_client_timestamp(self):
        forged_future_millis = 1_712_432_100_999
        bot = _bare_bot()
        sent: list[tuple[int, bytes]] = []
        bot._send = lambda packet_id, body, _sent=sent: _sent.append((packet_id, body))

        with mock.patch(
            "bot.bot.time.time_ns",
            side_effect=AssertionError("explicit protocol timestamps must not read the local clock"),
        ):
            bot.chat("未来时间验真", timestamp_millis=forged_future_millis)

        self.assertEqual(len(sent), 1)
        packet_id, body = sent[0]
        self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)
        reader = mc.Reader(body)
        self.assertEqual(reader.string(), "未来时间验真")
        self.assertEqual(
            reader.i64(),
            forged_future_millis,
            "测试 Bot 必须能按原版 C2S 字节精确模拟客户端未来时间，而不是在 harness 内预先钳制",
        )

    def test_chat_packet_encodes_signed_i64_timestamp_boundaries(self):
        cases = [
            -(2**63),
            -1,
            2**63 - 1,
        ]
        for timestamp_millis in cases:
            with self.subTest(timestamp_millis=timestamp_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )
                bot.chat("边界时间验真", timestamp_millis=timestamp_millis)
                self.assertEqual(
                    len(sent),
                    1,
                    f"合法 signed i64 必须原样发出，timestamp={timestamp_millis}, sent={sent!r}",
                )
                packet_id, body = sent[0]
                self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)
                reader = mc.Reader(body)
                self.assertEqual(reader.string(), "边界时间验真")
                self.assertEqual(
                    reader.i64(),
                    timestamp_millis,
                    "signed i64 timestamp 边界必须可原样 encode/decode",
                )
                self.assertEqual(reader.i64(), 0, "offline bot 的 salt 保持 0")

    def test_chat_packet_rejects_signed_i64_timestamp_overflow(self):
        overflow_cases = [
            -(2**63) - 1,
            2**63,
        ]
        for timestamp_millis in overflow_cases:
            with self.subTest(timestamp_millis=timestamp_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )
                with self.assertRaises(
                    (OverflowError, struct.error, ValueError),
                    msg=(
                        "越界 timestamp 必须在编码阶段失败，"
                        f"timestamp={timestamp_millis}"
                    ),
                ):
                    bot.chat("越界时间验真", timestamp_millis=timestamp_millis)
                self.assertEqual(
                    sent,
                    [],
                    f"越界 timestamp 不得发送任何包，timestamp={timestamp_millis}, sent={sent!r}",
                )


class RespawnDecodeTest(unittest.TestCase):
    def test_respawn_exposes_authoritative_dimension_names(self):
        for dimension in ("minecraft:overworld", "bong:tsy"):
            with self.subTest(dimension=dimension):
                bot = _bare_bot()
                body = (
                    mc.write_varint(mc.S2C_RESPAWN)
                    + mc.mc_string(dimension)
                    + mc.mc_string(dimension)
                )

                bot._dispatch(body)

                self.assertEqual(len(bot.events), 1)
                event = bot.events[0]
                self.assertEqual(event.kind, "respawn")
                self.assertEqual(event.data["dimension_type_name"], dimension)
                self.assertEqual(event.data["dimension_name"], dimension)


class EntityTrackingTest(unittest.TestCase):
    """实体位置表 pin：spawn 建 / rel-move 累积(Δ=i16/4096) / teleport 覆写 / destroy 删。

    近战场景（combat_weapon_equip_damage 追击式采样）依赖 entity_pos 追活体
    NPC——拿 spawn 坐标当靶在 CI 时序下必 whiff。"""

    def _spawn(self, bot, eid=7, x=10.0, y=64.0, z=-3.0):
        body = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(eid)
            + b"\x00" * 16
            + mc.write_varint(1)
            + struct.pack(">ddd", x, y, z)
        )
        bot._dispatch(body)

    def test_spawn_registers_position(self):
        bot = _bare_bot()
        self._spawn(bot)
        self.assertEqual(
            bot.entity_pos(7), (10.0, 64.0, -3.0),
            "entity_spawn 应把实体坐标登记进位置表（追击采样的起点）",
        )

    def test_rel_move_accumulates_quarter_4096(self):
        bot = _bare_bot()
        self._spawn(bot)
        # Δ = 4096 → +1.0 block（wiki.vg：delta 编码为 (cur-prev)*4096 的 i16）
        body = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(7)
            + struct.pack(">hhh", 4096, -2048, 0)
            + b"\x01"
        )
        bot._dispatch(body)
        x, y, z = bot.entity_pos(7)
        self.assertAlmostEqual(x, 11.0, places=6, msg="dx=4096/4096 应 +1.0")
        self.assertAlmostEqual(y, 63.5, places=6, msg="dy=-2048/4096 应 -0.5")
        self.assertAlmostEqual(z, -3.0, places=6)

    def test_rel_move_unknown_entity_is_ignored(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(99)
            + struct.pack(">hhh", 4096, 0, 0)
            + b"\x01"
        )
        bot._dispatch(body)  # 未知实体的 rel-move 无锚点，不得 KeyError
        self.assertIsNone(bot.entity_pos(99), "未 spawn 实体的 rel-move 应被忽略")

    def test_teleport_overwrites_absolute(self):
        bot = _bare_bot()
        self._spawn(bot)
        body = (
            mc.write_varint(mc.S2C_ENTITY_TELEPORT)
            + mc.write_varint(7)
            + struct.pack(">ddd", -100.0, 70.0, 200.0)
            + b"\x00\x00\x01"
        )
        bot._dispatch(body)
        self.assertEqual(
            bot.entity_pos(7), (-100.0, 70.0, 200.0),
            "teleport 应绝对覆写位置（不叠加）",
        )

    def test_destroy_removes_entity(self):
        bot = _bare_bot()
        self._spawn(bot)
        body = mc.write_varint(mc.S2C_ENTITIES_DESTROY) + mc.write_varint(1) + mc.write_varint(7)
        bot._dispatch(body)
        self.assertIsNone(bot.entity_pos(7), "destroy 后实体应从位置表移除（追击应停止）")


def _pb_float32_field(number: int, value: float) -> bytes:
    import struct as _struct

    return mc.write_varint((number << 3) | 5) + _struct.pack("<f", value)


def _pb_int32_field(number: int, value: int) -> bytes:
    """protobuf `int32` 字段的正确 wire 编码——负值走 64-bit 补码变长编码（最长 10
    字节），不是 `mc.write_varint` 那种给实际 MC 协议 varint 用的 32-bit 掩码。
    forge station_pos_x/y/z 断言负坐标（末法残土常见）时必须用这个而非
    `_pb_varint_field`。"""
    return mc.write_varint(number << 3) + _pb_raw_varint(value & 0xFFFFFFFFFFFFFFFF)


def _proto_message_body(source: str, message_name: str) -> str:
    match = re.search(rf"\bmessage\s+{re.escape(message_name)}\s*\{{", source)
    if match is None:
        raise AssertionError(f"authoritative proto missing message {message_name}")
    start = match.end() - 1
    depth = 0
    for index in range(start, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start + 1 : index]
    raise AssertionError(f"authoritative proto message {message_name} has no closing brace")


def _proto_field_signature(message_body: str, field_name: str) -> tuple[str, int]:
    match = re.search(
        rf"^\s*(?:optional\s+|repeated\s+)?([A-Za-z_][\w.]*)\s+"
        rf"{re.escape(field_name)}\s*=\s*(\d+)\s*;",
        message_body,
        flags=re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"authoritative proto missing field {field_name}")
    return match.group(1), int(match.group(2))


class ZoneInfoProtoDecodeTest(unittest.TestCase):
    """zone_info field 4 的 wire 契约饱和 pin。"""

    def _decode(self, message: bytes) -> dict:
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_ZONE_INFO_FIELD, message)
        )
        self.assertIsNotNone(decoded, "envelope field 4 必须分发到 zone_info decoder")
        return decoded

    def test_decoder_constants_match_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        zone_info = _proto_message_body(source, "ZoneInfo")

        self.assertEqual(
            _proto_field_signature(envelope, "zone_info"),
            ("ZoneInfo", proto_min.SERVER_DATA_ZONE_INFO_FIELD),
            "Bot envelope field 4 常量必须与权威 ServerDataEnvelope.zone_info 对齐",
        )
        expected_fields = {
            "zone": ("string", proto_min.ZONE_INFO_ZONE_FIELD),
            "spirit_qi": ("double", proto_min.ZONE_INFO_SPIRIT_QI_FIELD),
            "danger_level": ("uint32", proto_min.ZONE_INFO_DANGER_LEVEL_FIELD),
            "status": ("string", proto_min.ZONE_INFO_STATUS_FIELD),
            "active_events": ("string", proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD),
            "perception_text": ("string", proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD),
        }
        for field_name, expected in expected_fields.items():
            with self.subTest(field=field_name):
                self.assertEqual(
                    _proto_field_signature(zone_info, field_name),
                    expected,
                    f"Bot ZoneInfo.{field_name} 常量必须与权威 proto 对齐",
                )

    def test_happy_path_decodes_negative_qi_repeated_events_and_perception(self):
        message = (
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "blood_valley")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, -0.42)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 3)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Collapsed")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "beast_tide")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "realm_collapse")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "灵气几近断绝")
        )

        self.assertEqual(
            self._decode(message),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "blood_valley",
                "spirit_qi": -0.42,
                "danger_level": 3,
                "status": "Collapsed",
                "active_events": ["beast_tide", "realm_collapse"],
                "perception_text": "灵气几近断绝",
            },
        )

    def test_empty_message_uses_protobuf_defaults(self):
        self.assertEqual(
            self._decode(b""),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "",
                "spirit_qi": 0.0,
                "danger_level": 0,
                "status": "",
                "active_events": [],
                "perception_text": None,
            },
            "空 ZoneInfo 是合法 protobuf；repeated 应为空表，optional 应保留 absent=None",
        )

    def test_optional_perception_distinguishes_absent_from_present_empty(self):
        absent = self._decode(_pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "spawn"))
        present_empty = self._decode(
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "spawn")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "")
        )

        self.assertIsNone(absent["perception_text"])
        self.assertEqual(
            present_empty["perception_text"],
            "",
            "proto3 optional string 的 present-empty 不得退化成 absent=None",
        )

    def test_duplicate_scalars_use_last_value_and_repeated_keeps_wire_order(self):
        message = (
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "old_zone")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "first")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.1)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 1)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Normal")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "old")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "")
            + _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "new_zone")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.9)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 7)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "FutureStatus")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "first")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "new")
        )
        decoded = self._decode(message)

        self.assertEqual(decoded["zone"], "new_zone")
        self.assertEqual(decoded["spirit_qi"], 0.9)
        self.assertEqual(decoded["danger_level"], 7)
        self.assertEqual(decoded["status"], "FutureStatus")
        self.assertEqual(decoded["active_events"], ["first", "", "first"])
        self.assertEqual(decoded["perception_text"], "new")

    def test_danger_uint32_boundaries_decode_without_signed_wrap(self):
        for value in (0, 7, 2**32 - 1):
            with self.subTest(value=value):
                decoded = self._decode(
                    _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, value)
                )
                self.assertEqual(
                    decoded["danger_level"],
                    value,
                    f"uint32 danger_level={value} 不得发生有符号回绕",
                )

    def test_wrong_wire_types_are_ignored_instead_of_impersonating_fields(self):
        message = (
            _pb_fixed64(proto_min.ZONE_INFO_ZONE_FIELD, 1.0)
            + _pb_varint(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 1)
            + _pb_string(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, "7")
            + _pb_varint(proto_min.ZONE_INFO_STATUS_FIELD, 1)
            + _pb_fixed64(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, 1.0)
            + _pb_varint(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, 1)
        )

        self.assertEqual(
            self._decode(message),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "",
                "spirit_qi": 0.0,
                "danger_level": 0,
                "status": "",
                "active_events": [],
                "perception_text": None,
            },
            "错误 wire type 必须按 protobuf unknown-field 语义忽略",
        )

    def test_unknown_fields_of_every_supported_wire_type_are_ignored(self):
        message = (
            _pb_varint(200, 99)
            + _pb_fixed64(201, 12.5)
            + _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "rift_mouth_north_002")
            + _pb_string(202, "future")
            + _pb_float32_field(203, 0.75)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 5)
        )
        decoded = self._decode(message)

        self.assertEqual(decoded["zone"], "rift_mouth_north_002")
        self.assertEqual(decoded["danger_level"], 5)
        self.assertEqual(decoded["active_events"], [])

    def test_unknown_envelope_fields_do_not_hide_later_zone_info(self):
        envelope = (
            _pb_varint(199, 1)
            + _pb_bytes(200, b"\xff\x00future")
            + _pb_message(
                proto_min.SERVER_DATA_ZONE_INFO_FIELD,
                _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "north_waste_east_scorch"),
            )
            + _pb_float32_field(201, 0.5)
        )
        decoded = proto_min.decode_server_data_envelope(envelope)

        self.assertIsNotNone(decoded)
        self.assertEqual(decoded["type"], "zone_info")
        self.assertEqual(decoded["zone"], "north_waste_east_scorch")

    def test_zone_info_envelope_with_wrong_wire_type_is_not_dispatched(self):
        self.assertIsNone(
            proto_min.decode_server_data_envelope(
                _pb_varint(proto_min.SERVER_DATA_ZONE_INFO_FIELD, 1)
            ),
            "oneof message field 4 的 varint 不得冒充 zone_info",
        )

    def test_truncated_nested_fixed64_is_rejected(self):
        truncated = (
            _pb_key(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 1) + b"\x00" * 7
        )
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated fixed64",
            msg="ZoneInfo.spirit_qi fixed64 少一字节必须报协议错误",
        ):
            self._decode(truncated)

    def test_truncated_nested_string_is_rejected(self):
        truncated = (
            _pb_key(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, 2)
            + _pb_raw_varint(3)
            + b"ab"
        )
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated length-delimited field",
            msg="active_events 声明 3 字节却只给 2 字节必须报协议错误",
        ):
            self._decode(truncated)

    def test_unsupported_nested_wire_type_is_rejected(self):
        malformed = _pb_key(7, 3)
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "unsupported wire type 3",
            msg="group wire type 不在 minimal decoder 支持集内，必须显式报错",
        ):
            self._decode(malformed)

    def test_public_payload_decoder_turns_malformed_zone_info_into_none(self):
        malformed = (
            _pb_key(proto_min.SERVER_DATA_ZONE_INFO_FIELD, 2)
            + _pb_raw_varint(5)
            + b"ab"
        )
        self.assertIsNone(
            decode_server_data_payload(malformed),
            "Bot 公共观察面遇到截断 zone_info 应返回 None，而不是拖垮 reader thread",
        )


class ProdConsumeDecodeTest(unittest.TestCase):
    """三产三用 payload 解码 pin：envelope oneof tag 与字段号对齐 proto/bong/envelope.proto.

    这些解码器是 production_*/combat_*/cultivation_pill 场景的观察面地基——
    tag 或字段号漂移会让场景从「锁契约」退化成「永远超时」。
    """

    def test_player_state_decoder_constants_match_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        player_state = _proto_message_body(source, "PlayerState")

        self.assertEqual(
            _proto_field_signature(envelope, "player_state"),
            ("PlayerState", proto_min.SERVER_DATA_PLAYER_STATE_FIELD),
            "Bot envelope 分发常量必须与权威 ServerDataEnvelope.player_state 对齐",
        )
        self.assertEqual(
            _proto_field_signature(player_state, "spirit_qi"),
            ("double", proto_min.PLAYER_STATE_SPIRIT_QI_FIELD),
            "Bot spirit_qi 常量及 fixed64 wire type 必须与权威 PlayerState 对齐",
        )
        self.assertEqual(
            _proto_field_signature(player_state, "spirit_qi_max"),
            ("double", proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD),
            "Bot spirit_qi_max 常量及 fixed64 wire type 必须与权威 PlayerState 对齐",
        )

    def test_player_state_tag5_decodes_authoritative_qi(self):
        msg = _pb_fixed64(
            proto_min.PLAYER_STATE_SPIRIT_QI_FIELD, 65.0
        ) + _pb_fixed64(proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD, 100.0)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertIsNotNone(
            decoded,
            "envelope tag 5 必须解码为 player_state，实际返回 None",
        )
        self.assertEqual(
            decoded["type"],
            "player_state",
            f"envelope tag 5 应分发到 player_state，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi"],
            65.0,
            f"PlayerState.spirit_qi 必须读取 fixed64 field 3，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi_max"],
            100.0,
            f"PlayerState.spirit_qi_max 必须读取 fixed64 field 11，实际 payload={decoded}",
        )

    def test_player_state_missing_qi_fields_use_protobuf_zero_defaults(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, b"")
        )
        self.assertIsNotNone(
            decoded,
            "空 player_state message 仍是合法 protobuf，实际返回 None",
        )
        self.assertEqual(
            decoded["spirit_qi"],
            0.0,
            f"缺失 fixed64 field 3 应使用 protobuf 默认 0，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi_max"],
            0.0,
            f"缺失 fixed64 field 11 应使用 protobuf 默认 0，实际 payload={decoded}",
        )

    def test_player_state_wrong_qi_wire_type_is_ignored(self):
        msg = _pb_varint_field(
            proto_min.PLAYER_STATE_SPIRIT_QI_FIELD, 65
        ) + _pb_varint_field(proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD, 100)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertIsNotNone(
            decoded,
            "wire type 错误的 player_state envelope 仍应被识别，实际返回 None",
        )
        self.assertEqual(
            (decoded["spirit_qi"], decoded["spirit_qi_max"]),
            (0.0, 0.0),
            f"field 3/11 的 varint 不得冒充 fixed64，实际 payload={decoded}",
        )

    def test_player_state_truncated_fixed64_is_rejected(self):
        truncated = mc.write_varint(
            (proto_min.PLAYER_STATE_SPIRIT_QI_FIELD << 3) | 1
        ) + b"\x00" * 7
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated fixed64",
            msg="PlayerState fixed64 field 3 截断时必须报协议错误",
        ):
            proto_min.decode_server_data_envelope(
                _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, truncated)
            )

    def test_craft_session_state_tag22(self):
        msg = (
            _pb_varint_field(3, 1)
            + _pb_len_field(4, b"workbench.weapon.stone_knife")
            + _pb_varint_field(5, 10)
            + _pb_varint_field(6, 400)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(22, msg))
        self.assertEqual(
            decoded["type"], "craft_session_state",
            "envelope tag 22 应分发到 craft_session_state（envelope.proto oneof）",
        )
        self.assertTrue(decoded["active"], "field3=1 应解为 active=True")
        self.assertEqual(
            decoded["recipe_id"], "workbench.weapon.stone_knife",
            "CraftSessionState.recipe_id 是 field 4（optional string）",
        )
        self.assertEqual(
            decoded["total_ticks"], 400, "CraftSessionState.total_ticks 是 field 6"
        )

    def test_craft_outcome_completed_tag23(self):
        completed = (
            _pb_len_field(3, b"workbench.weapon.stone_knife")
            + _pb_len_field(4, b"stone_knife")
            + _pb_varint_field(5, 1)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(23, _pb_len_field(1, completed))
        )
        self.assertEqual(
            decoded["type"], "craft_outcome", "envelope tag 23 应分发到 craft_outcome"
        )
        self.assertEqual(
            decoded["outcome"], "completed", "oneof field 1 = CraftOutcomeCompleted"
        )
        self.assertEqual(
            decoded["output_template"], "stone_knife",
            "CraftOutcomeCompleted.output_template 是 field 4",
        )
        self.assertEqual(decoded["output_count"], 1, "output_count 是 field 5")

    def test_craft_outcome_failed_branch(self):
        failed = (
            _pb_len_field(3, b"r")
            + _pb_varint_field(4, 2)
            + _pb_varint_field(5, 7)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(23, _pb_len_field(2, failed))
        )
        self.assertEqual(decoded["outcome"], "failed", "oneof field 2 = CraftOutcomeFailed")
        self.assertEqual(decoded["reason"], 2, "CraftOutcomeFailed.reason 是 field 4（enum）")
        self.assertEqual(
            decoded["material_returned"],
            7,
            "CraftOutcomeFailed.material_returned 是 field 5，Bot 必须核对真实退款数",
        )

    def test_dropped_loot_sync_tag81_decodes_pickup_identity(self):
        initial_qi_bits = 0x42C50001
        initial_qi = struct.unpack("<f", struct.pack("<I", initial_qi_bits))[0]
        freshness = (
            _pb_varint(1, 123)
            + _pb_fixed32(2, initial_qi)
            + _pb_string(3, "Decay")
            + _pb_string(4, "ling_mu_gun_v1")
            + _pb_varint(5, 17)
            + _pb_varint(6, 140)
        )
        item = (
            _pb_varint(1, 77)
            + _pb_string(2, "fan_tie")
            + _pb_varint(9, 1)
            + _pb_message(22, freshness)
        )
        entry = (
            _pb_varint(1, 77)
            + _pb_string(2, "overflow:fan_tie")
            + _pb_varint(3, 0)
            + _pb_varint(4, 0)
            + _pb_fixed64(5, 8.0)
            + _pb_fixed64(6, 65.0)
            + _pb_fixed64(7, -2.0)
            + _pb_message(8, item)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(81, _pb_message(1, entry))
        )
        self.assertEqual(
            decoded["type"],
            "dropped_loot_sync",
            "envelope tag 81 应分发到 dropped_loot_sync",
        )
        self.assertEqual(len(decoded["drops"]), 1, "sync field 1 应解出一条掉落")
        drop = decoded["drops"][0]
        self.assertEqual(drop["instance_id"], 77, "instance_id 用于 pickup intent")
        self.assertEqual(drop["item"]["item_id"], "fan_tie")
        self.assertEqual(drop["item"]["stack_count"], 1)
        self.assertEqual(drop["world_pos"], [8.0, 65.0, -2.0])
        self.assertEqual(
            drop["item"]["freshness"],
            {
                "created_at_tick": 123,
                "initial_qi": initial_qi,
                "track": "Decay",
                "profile": "ling_mu_gun_v1",
                "frozen_accumulated": 17,
                "frozen_since_tick": 140,
            },
            "dropped_loot_sync 必须保留完整 freshness，拾取后才能对拍同一实例 NBT",
        )
        self.assertEqual(
            struct.pack("<f", drop["item"]["freshness"]["initial_qi"]),
            struct.pack("<I", initial_qi_bits),
            "Bot 必须逐 bit 保留 Rust Freshness.initial_qi 的 f32 wire 值",
        )

    def test_inventory_item_without_freshness_decodes_none(self):
        item = _pb_varint(1, 77) + _pb_string(2, "fan_tie") + _pb_varint(9, 1)
        entry = _pb_varint(1, 77) + _pb_message(8, item)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(81, _pb_message(1, entry))
        )
        self.assertIsNone(decoded["drops"][0]["item"]["freshness"])

    def test_lumber_progress_tag29_decodes_terminal_contract(self):
        progress = (
            _pb_string(1, "offline:wood")
            + _pb_varint(2, (1 << 64) - 1292)
            + _pb_varint(3, 73)
            + _pb_varint(4, 1519)
            + _pb_fixed64(5, 1.0)
            + _pb_varint(6, 0)
            + _pb_varint(7, 1)
            + _pb_string(8, "背包已满，灵木原木已落地 ×3")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(29, progress))
        self.assertEqual(decoded["type"], "lumber_progress")
        self.assertEqual(decoded["session_id"], "offline:wood")
        self.assertEqual(decoded["log_pos"], [-1292, 73, 1519])
        self.assertEqual(decoded["progress"], 1.0)
        self.assertFalse(decoded["interrupted"])
        self.assertTrue(decoded["completed"])
        self.assertIn("背包已满", decoded["detail"])

    def test_craft_outcome_unknown_fallback(self):
        # 空 CraftOutcome（无 oneof 分支）→ 解码器兜底 unknown，不 crash
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(23, b""))
        self.assertEqual(
            decoded["outcome"], "unknown",
            "无 completed/failed 分支的 CraftOutcome 应兜底 outcome=unknown（防解码 crash）",
        )

    def test_alchemy_furnace_tag11_and_session_tag12(self):
        furnace = (
            _pb_varint_field(4, 1)
            + _pb_fixed64(5, 0.92)
            + _pb_fixed64(6, 1.0)
            + _pb_string(7, "Azure")
            + _pb_varint_field(8, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(11, furnace))
        self.assertEqual(
            decoded["type"], "alchemy_furnace", "envelope tag 11 应分发到 alchemy_furnace"
        )
        self.assertEqual(decoded["tier"], 1, "AlchemyFurnace.tier 是 field 4")
        self.assertAlmostEqual(decoded["integrity"], 0.92)
        self.assertAlmostEqual(decoded["integrity_max"], 1.0)
        self.assertEqual(decoded["owner_name"], "Azure")
        self.assertTrue(decoded["has_session"], "AlchemyFurnace.has_session 是 field 8")

        first_stage = (
            _pb_varint_field(1, 0)
            + _pb_varint_field(2, 0)
            + _pb_string(3, "spirit_grass×3")
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 0)
        )
        empty_stage = (
            _pb_varint_field(1, 80)
            + _pb_varint_field(2, 4)
            + _pb_string(3, "")
            + _pb_varint_field(4, 0)
            + _pb_varint_field(5, 1)
        )
        session = (
            _pb_len_field(1, b"ling_xi_wan_v1")
            + _pb_varint_field(2, 1)
            + _pb_varint_field(3, 44)
            + _pb_varint_field(4, 80)
            + _pb_fixed64(5, 0.30)
            + _pb_fixed64(6, 0.30)
            + _pb_fixed64(7, 0.15)
            + _pb_fixed64(8, 8.0)
            + _pb_fixed64(9, 5.0)
            + _pb_string(10, "炼制中")
            + _pb_message(11, first_stage)
            + _pb_message(11, empty_stage)
            + _pb_string(12, "§7InjectQi(8.0)")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(12, session))
        self.assertEqual(
            decoded["type"], "alchemy_session", "envelope tag 12 应分发到 alchemy_session"
        )
        self.assertEqual(
            decoded["recipe_id"], "ling_xi_wan_v1", "AlchemySession.recipe_id 是 field 1"
        )
        self.assertTrue(decoded["active"], "AlchemySession.active 是 field 2")
        self.assertEqual(decoded["elapsed_ticks"], 44)
        self.assertEqual(decoded["target_ticks"], 80)
        self.assertAlmostEqual(decoded["temp_current"], 0.30)
        self.assertAlmostEqual(decoded["temp_target"], 0.30)
        self.assertAlmostEqual(decoded["temp_band"], 0.15)
        self.assertAlmostEqual(decoded["qi_injected"], 8.0)
        self.assertAlmostEqual(decoded["qi_target"], 5.0)
        self.assertEqual(decoded["status_label"], "炼制中")
        self.assertEqual(len(decoded["stages"]), 2)
        self.assertEqual(decoded["stages"][0]["at_tick"], 0)
        self.assertEqual(decoded["stages"][0]["window"], 0)
        self.assertEqual(decoded["stages"][0]["summary"], "spirit_grass×3")
        self.assertTrue(decoded["stages"][0]["completed"])
        self.assertEqual(decoded["stages"][1]["at_tick"], 80)
        self.assertEqual(decoded["stages"][1]["window"], 4)
        self.assertEqual(decoded["stages"][1]["summary"], "")
        self.assertTrue(decoded["stages"][1]["missed"])
        self.assertEqual(decoded["interventions_recent"], ["§7InjectQi(8.0)"])

    def test_alchemy_outcome_resolved_tag14(self):
        outcome = _pb_varint_field(1, 1) + _pb_len_field(3, b"ling_xi_wan")
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(14, outcome))
        self.assertEqual(
            decoded["type"], "alchemy_outcome_resolved",
            "envelope tag 14 应分发到 alchemy_outcome_resolved",
        )
        self.assertEqual(
            decoded["pill"], "ling_xi_wan", "AlchemyOutcomeResolved.pill 是 field 3"
        )

    def test_cast_sync_tag34_outcome_names(self):
        # outcome=8 → meridian_gated（经脉门拒因，场景负分支断言依赖此命名）
        msg = _pb_varint_field(1, 3) + _pb_varint_field(2, 1) + _pb_varint_field(5, 8)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(34, msg))
        self.assertEqual(decoded["type"], "cast_sync", "envelope tag 34 应分发到 cast_sync")
        self.assertEqual(decoded["phase"], "complete", "CastPhase=3 → complete")
        self.assertEqual(decoded["slot"], 1, "CastSync.slot 是 field 2")
        self.assertEqual(
            decoded["outcome"], "meridian_gated",
            "CastOutcome=8 → meridian_gated（场景负分支断言依赖此命名）",
        )

    def test_inventory_move_rejected_tag137_race_mismatch_reason_no_extra_fields(self):
        # plan-race-system-v1 P3b —— field 137 此前未接入 decode_server_data_envelope
        # 白名单，任何 bot 场景断言 inventory_move_rejected（含新增的 race_mismatch）
        # 都会静默超时；本测试锁死该 payload_type 现已可解码。
        msg = _pb_string(1, "race_mismatch")
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(137, msg))
        self.assertEqual(
            decoded["type"], "inventory_move_rejected", "envelope tag 137 应分发到 inventory_move_rejected"
        )
        self.assertEqual(decoded["reason"], "race_mismatch")
        self.assertIsNone(decoded["required_realm"], "race_mismatch 不携带 required_realm")
        self.assertIsNone(decoded["slot"], "race_mismatch 不携带 slot")
        self.assertIsNone(decoded["cap"], "race_mismatch 不携带 cap")

    def test_inventory_move_rejected_tag137_worn_cap_full_carries_slot_and_cap(self):
        msg = _pb_string(1, "worn_cap_full") + _pb_string(3, "chest") + _pb_varint(4, 3)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(137, msg))
        self.assertEqual(decoded["reason"], "worn_cap_full")
        self.assertEqual(decoded["slot"], "chest")
        self.assertEqual(decoded["cap"], 3)
        self.assertIsNone(decoded["required_realm"])

    def test_combat_event_floater_tag51_amount_float32(self):
        entry = (
            _pb_len_field(1, b"damage")
            + _pb_float32_field(2, 12.5)
            + _pb_len_field(3, b"-12")
            + _pb_varint_field(7, 1)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(51, _pb_len_field(1, entry))
        )
        self.assertEqual(
            decoded["type"], "combat_event", "envelope tag 51 应分发到 combat_event"
        )
        self.assertEqual(len(decoded["events"]), 1, "repeated entries 应逐条解出")
        self.assertEqual(decoded["events"][0]["kind"], "damage", "entry.kind 是 field 1")
        self.assertAlmostEqual(
            decoded["events"][0]["amount"], 12.5, places=4,
            msg="entry.amount 是 field 2（float32 wire type 5，非 double）",
        )
        self.assertTrue(
            decoded["events"][0]["outgoing"],
            "entry.outgoing 是 field 7（bool）——方向标识，场景据此区分己方输出/承伤",
        )

    def test_combat_event_floater_outgoing_defaults_false(self):
        # 老 server 不发 field 7 → proto3 缺省 false（承伤视角），解码不得崩
        entry = _pb_len_field(1, b"hit") + _pb_float32_field(2, 3.0)
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(51, _pb_len_field(1, entry))
        )
        self.assertFalse(
            decoded["events"][0]["outgoing"],
            "缺 field 7 时 outgoing 应缺省 False（proto3 bool 缺省），不得 KeyError",
        )

    def test_forge_station_tag17_decodes_pos_including_negative(self):
        # plan-forge-session-entry-wiring-v1 P2：station_pos_x/y/z 是 int32，末法残土
        # 常见负坐标——必须走 64-bit 补码 varint，不能用 32-bit 掩码的 _pb_varint_field。
        station = (
            _pb_string(1, "forge_station_Azure")
            + _pb_varint_field(2, 1)
            + _pb_float32_field(3, 0.75)
            + _pb_string(4, "Azure")
            + _pb_varint_field(5, 1)
            + _pb_int32_field(6, -12)
            + _pb_varint_field(7, 64)
            + _pb_int32_field(8, -8)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(17, station))
        self.assertEqual(
            decoded["type"], "forge_station", "envelope tag 17 应分发到 forge_station"
        )
        self.assertEqual(
            decoded["station_id"], "forge_station_Azure",
            "WeaponForgeStationDataV1.station_id 是 field 1",
        )
        self.assertEqual(decoded["tier"], 1, "field 2 是 tier")
        self.assertAlmostEqual(
            decoded["integrity"], 0.75, places=4,
            msg="field 3 是 integrity（float32 wire type 5）",
        )
        self.assertTrue(decoded["has_session"], "field 5 是 has_session（bool）")
        self.assertEqual(
            decoded["pos"], [-12, 64, -8],
            "station_pos_x/y/z 是 field 6/7/8；负坐标须按 int32 补码正确还原（不是 4294967284）",
        )

    def test_forge_session_tag18_decodes_tempering_step_state(self):
        tempering = (
            _pb_varint_field(2, 3)
            + _pb_varint_field(3, 5)
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 2)
            + _pb_fixed64(6, 1.5)
        )
        step_state = _pb_message(2, tempering)  # ForgeStepState.tempering = field 2
        session = (
            _pb_varint_field(1, 7)
            + _pb_string(2, "qing_feng_v0")
            + _pb_string(3, "青锋剑（测试）")
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 2)  # ForgeStep.TEMPERING
            + _pb_varint_field(6, 1)
            + _pb_varint_field(7, 1)
            + _pb_message(8, step_state)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(18, session))
        self.assertEqual(
            decoded["type"], "forge_session", "envelope tag 18 应分发到 forge_session"
        )
        self.assertEqual(decoded["session_id"], 7, "field 1 是 session_id（uint64）")
        self.assertEqual(
            decoded["blueprint_id"], "qing_feng_v0", "field 2 是 blueprint_id"
        )
        self.assertTrue(decoded["active"], "field 4 是 active（bool）")
        self.assertEqual(
            decoded["current_step"], "tempering",
            "ForgeStep=2 应解为 tempering（枚举与 proto/bong/envelope.proto 对齐）",
        )
        self.assertEqual(decoded["step_index"], 1, "field 6 是 step_index")
        self.assertEqual(decoded["achieved_tier"], 1, "field 7 是 achieved_tier")
        self.assertEqual(
            decoded["step_state"],
            {
                "kind": "tempering",
                "beat_cursor": 3,
                "hits": 5,
                "misses": 1,
                "deviation": 2,
                "qi_spent": 1.5,
            },
            "oneof step_state 应分发到 tempering 分支（ForgeStepState.tempering=field 2）",
        )

    def test_forge_session_step_state_none_fallback(self):
        # Done session 的 step_state 是 ForgeStepState.none_state（field 5，bool）——
        # 解码器应兜底 kind=none，不得 crash。
        session = _pb_varint_field(5, 5)  # ForgeStep.DONE
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(18, session))
        self.assertEqual(decoded["current_step"], "done", "ForgeStep=5 应解为 done")
        self.assertEqual(
            decoded["step_state"], {"kind": "none"},
            "无 billet/tempering/inscription/consecration 分支时应兜底 kind=none",
        )

    def test_forge_outcome_tag19_perfect_bucket(self):
        outcome = (
            _pb_varint_field(1, 9)
            + _pb_string(2, "qing_feng_v0")
            + _pb_varint_field(3, 1)  # ForgeOutcomeBucket.PERFECT
            + _pb_string(4, "qing_feng_sword")
            + _pb_float32_field(5, 1.0)
            + _pb_string(7, "brittle_edge")
            + _pb_varint_field(8, 2)
            + _pb_varint_field(9, 0)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(19, outcome))
        self.assertEqual(
            decoded["type"], "forge_outcome", "envelope tag 19 应分发到 forge_outcome"
        )
        self.assertEqual(decoded["bucket"], "perfect", "ForgeOutcomeBucket=1 应解为 perfect")
        self.assertEqual(
            decoded["weapon_item"], "qing_feng_sword",
            "field 4 是 optional string weapon_item",
        )
        self.assertAlmostEqual(decoded["quality"], 1.0, places=4)
        self.assertEqual(
            decoded["side_effects"], ["brittle_edge"],
            "field 7 是 repeated string side_effects",
        )
        self.assertEqual(decoded["achieved_tier"], 2, "field 8 是 achieved_tier")
        self.assertFalse(decoded["flawed_path"], "field 9=0 → flawed_path=False")

    def test_forge_outcome_flawed_path_true_and_missing_weapon_item(self):
        # Waste bucket 无 weapon_item（proto optional 缺省不发 field 4）——不得 KeyError。
        outcome = (
            _pb_varint_field(1, 10)
            + _pb_varint_field(3, 4)  # ForgeOutcomeBucket.WASTE
            + _pb_varint_field(9, 1)  # flawed_path=True
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(19, outcome))
        self.assertEqual(decoded["bucket"], "waste", "ForgeOutcomeBucket=4 应解为 waste")
        self.assertIsNone(
            decoded["weapon_item"], "缺 field 4 时 weapon_item 应为 None，不得 KeyError"
        )
        self.assertTrue(decoded["flawed_path"], "field 9=1 → flawed_path=True")
        self.assertEqual(
            decoded["side_effects"], [], "无 repeated 条目时应兜底空列表，不得 crash"
        )

    def test_forge_blueprint_book_tag20(self):
        entry = (
            _pb_string(1, "qing_feng_v0")
            + _pb_string(2, "青锋剑（测试）")
            + _pb_varint_field(3, 2)
            + _pb_varint_field(4, 2)
        )
        book = _pb_message(1, entry) + _pb_varint_field(2, 0)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(20, book))
        self.assertEqual(
            decoded["type"], "forge_blueprint_book",
            "envelope tag 20 应分发到 forge_blueprint_book",
        )
        self.assertEqual(len(decoded["learned"]), 1, "repeated ForgeBlueprintEntry 应逐条解出")
        self.assertEqual(decoded["learned"][0]["id"], "qing_feng_v0")
        self.assertEqual(decoded["learned"][0]["step_count"], 2)
        self.assertEqual(decoded["current_index"], 0)


class NewServerDataDecoderContractTest(unittest.TestCase):
    """S1 拆分新增的深度解码器契约 pin（central-review finding 1 的补测）。

    S1 在 decode_server_data_envelope 注册了 botany_harvest_progress(25) /
    gathering_session(30) / lingtian_session(31) / skill_bar_config(36) /
    breakthrough_cinematic(71)，并改写了 player_state(5)（新增 realm）与
    alchemy_outcome_resolved(14)（bucket 转枚举名、可空字段、toxin_color）。
    这些解码器此前只在 test_protocol.py 初始化了 fixture，没有任何 field→值
    的 wire 契约断言——字段号/枚举映射/packed 解码错了场景照样静默观察错误数据。
    本类用与 authoritative proto 对齐的字段号逐项钉死解码输出。
    """

    def test_botany_harvest_progress_tag25_decodes_full_contract(self):
        msg = (
            _pb_string(1, "botany:session_7")
            + _pb_string(2, "spirit_grass_001")
            + _pb_string(3, "灵草")
            + _pb_string(4, "spirit_grass")
            + _pb_string(5, "manual")
            + _pb_fixed64(6, 0.42)
            + _pb_varint(7, 1)
            + _pb_varint(8, 1)
            + _pb_varint(9, 0)
            + _pb_varint(10, 0)
            + _pb_string(11, "正在采集")
            + _pb_string(12, "hint_a")
            + _pb_string(12, "hint_b")
            + _pb_fixed64(13, 10.0)
            + _pb_fixed64(14, 64.0)
            + _pb_fixed64(15, -3.0)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(25, msg))
        self.assertEqual(decoded["type"], "botany_harvest_progress")
        self.assertEqual(decoded["session_id"], "botany:session_7")
        self.assertEqual(decoded["target_id"], "spirit_grass_001")
        self.assertEqual(decoded["target_name"], "灵草")
        self.assertEqual(decoded["plant_kind"], "spirit_grass")
        self.assertEqual(decoded["mode"], "manual")
        self.assertEqual(decoded["progress"], 0.42)
        self.assertTrue(decoded["auto_selectable"])
        self.assertTrue(decoded["request_pending"])
        self.assertFalse(decoded["interrupted"])
        self.assertFalse(decoded["completed"])
        self.assertEqual(decoded["detail"], "正在采集")
        self.assertEqual(decoded["hazard_hints"], ["hint_a", "hint_b"])
        self.assertEqual(
            decoded["target_pos"],
            [10.0, 64.0, -3.0],
            "target_pos_x/y/z 是 field 13/14/15，三个 optional double 拆成平铺坐标",
        )

    def test_botany_harvest_progress_absent_target_pos_is_none_triple(self):
        msg = _pb_string(1, "botany:session_8") + _pb_fixed64(6, 0.5)
        decoded = proto_min.decode_server_data_envelope(_pb_message(25, msg))
        self.assertEqual(
            decoded["target_pos"],
            [None, None, None],
            "缺 field 13/14/15 时 target_pos 必须是 [None, None, None]，不得用 0 冒充",
        )
        self.assertEqual(decoded["hazard_hints"], [])
        self.assertEqual(decoded["progress"], 0.5)

    def test_gathering_session_tag30_decodes_enum_and_optional(self):
        msg = (
            _pb_string(1, "gather:42")
            + _pb_varint(2, 5)
            + _pb_varint(3, 20)
            + _pb_string(4, "精铁矿")
            + _pb_varint(5, 2)  # GATHERING_TARGET_TYPE_ORE
            + _pb_varint(6, 5)  # GATHERING_QUALITY_HINT_PERFECT
            + _pb_string(7, "iron_pickaxe")
            + _pb_varint(8, 0)
            + _pb_varint(9, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(30, msg))
        self.assertEqual(decoded["type"], "gathering_session")
        self.assertEqual(decoded["session_id"], "gather:42")
        self.assertEqual(decoded["progress_ticks"], 5)
        self.assertEqual(decoded["total_ticks"], 20)
        self.assertEqual(decoded["target_name"], "精铁矿")
        self.assertEqual(decoded["target_type"], "ore")
        self.assertEqual(decoded["quality_hint"], "perfect")
        self.assertEqual(decoded["tool_used"], "iron_pickaxe")
        self.assertFalse(decoded["interrupted"])
        self.assertTrue(decoded["completed"])

    def test_gathering_session_absent_tool_used_is_none(self):
        msg = _pb_string(1, "gather:43") + _pb_varint(5, 1)
        decoded = proto_min.decode_server_data_envelope(_pb_message(30, msg))
        self.assertIsNone(decoded["tool_used"], "缺 field 7 时 tool_used 应为 None")
        self.assertEqual(decoded["target_type"], "herb")

    def test_lingtian_session_tag31_decodes_kind_pos_and_optional(self):
        msg = (
            _pb_varint(1, 1)
            + _pb_varint(2, 3)  # LINGTIAN_SESSION_KIND_PLANTING
            + _pb_int32_field(3, 100)
            + _pb_int32_field(4, 72)
            + _pb_int32_field(5, -50)
            + _pb_varint(6, 12)
            + _pb_varint(7, 60)
            + _pb_string(8, "spirit_rice")
            + _pb_string(9, "player:Alice")
            + _pb_float32_field(10, 0.15)
            + _pb_varint(11, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(31, msg))
        self.assertEqual(decoded["type"], "lingtian_session")
        self.assertTrue(decoded["active"])
        self.assertEqual(decoded["kind"], "planting")
        self.assertEqual(
            decoded["pos"],
            [100, 72, -50],
            "pos_x/y/z 是 field 3/4/5（int32，含负坐标补码），平铺成 pos",
        )
        self.assertEqual(decoded["elapsed_ticks"], 12)
        self.assertEqual(decoded["target_ticks"], 60)
        self.assertEqual(decoded["plant_id"], "spirit_rice")
        self.assertEqual(decoded["source"], "player:Alice")
        self.assertAlmostEqual(decoded["dye_contamination"], 0.15, places=4)
        self.assertTrue(decoded["dye_contamination_warning"])

    def test_lingtian_session_empty_message_defaults(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(31, b""))
        self.assertFalse(decoded["active"])
        self.assertEqual(decoded["kind"], "unspecified")
        self.assertEqual(decoded["pos"], [0, 0, 0])
        self.assertIsNone(decoded["plant_id"])
        self.assertIsNone(decoded["source"])
        self.assertIsNone(decoded["dye_contamination"])
        self.assertFalse(decoded["dye_contamination_warning"])

    def test_skill_bar_config_tag36_decodes_item_skill_and_empty_slots(self):
        item = (
            _pb_string(1, "spirit_grass")
            + _pb_string(2, "灵草")
            + _pb_varint(3, 100)
            + _pb_varint(4, 200)
            + _pb_string(5, "bong:icons/spirit_grass")
        )
        skill = (
            _pb_string(1, "meditate")
            + _pb_string(2, "静心")
            + _pb_varint(3, 500)
            + _pb_varint(4, 3000)
            + _pb_string(5, "bong:icons/meditate")
        )
        slot_item = _pb_message(1, _pb_message(1, item))  # entry.item = field 1
        slot_skill = _pb_message(1, _pb_message(2, skill))  # entry.skill = field 2
        packed = _pb_bytes(2, _pb_raw_varint(123) + _pb_raw_varint(456))
        msg = (
            _pb_message(1, slot_item)
            + _pb_message(1, slot_skill)
            + _pb_message(1, b"")  # 空 OptionalSkillBarEntry → None
            + packed
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(36, msg))
        self.assertEqual(decoded["type"], "skillbar_config")
        self.assertEqual(
            decoded["slots"][0],
            {
                "kind": "item",
                "template_id": "spirit_grass",
                "display_name": "灵草",
                "cast_duration_ms": 100,
                "cooldown_ms": 200,
                "icon_texture": "bong:icons/spirit_grass",
            },
        )
        self.assertEqual(
            decoded["slots"][1],
            {
                "kind": "skill",
                "skill_id": "meditate",
                "display_name": "静心",
                "cast_duration_ms": 500,
                "cooldown_ms": 3000,
                "icon_texture": "bong:icons/meditate",
            },
        )
        self.assertIsNone(decoded["slots"][2])
        self.assertEqual(
            decoded["cooldown_until_ms"],
            [123, 456],
            "field 2 的 packed uint64（length-delimited 内连 varint）必须解包",
        )

    def test_skill_bar_config_cooldowns_accept_unpacked_wire(self):
        msg = _pb_varint(2, 1000) + _pb_varint(2, 2000)
        decoded = proto_min.decode_server_data_envelope(_pb_message(36, msg))
        self.assertEqual(decoded["type"], "skillbar_config")
        self.assertEqual(decoded["cooldown_until_ms"], [1000, 2000])

    def test_breakthrough_cinematic_tag71_decodes_full_contract(self):
        msg = (
            _pb_string(1, "player:Alice")
            + _pb_string(2, "lightning")
            + _pb_varint(3, 30)
            + _pb_varint(4, 120)
            + _pb_string(5, "spirit")
            + _pb_string(6, "void")
            + _pb_string(7, "success")
            + _pb_varint(8, 0)
            + _pb_fixed64(9, 100.0)
            + _pb_fixed64(10, 72.0)
            + _pb_fixed64(11, -200.0)
            + _pb_fixed64(12, 64.0)
            + _pb_varint(13, 1)
            + _pb_varint(14, 1)
            + _pb_float32_field(15, 0.8)
            + _pb_float32_field(16, 0.5)
            + _pb_string(17, "autumn")
            + _pb_string(18, "cinematic")
            + _pb_varint(19, 4242)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(71, msg))
        self.assertEqual(decoded["type"], "breakthrough_cinematic")
        self.assertEqual(decoded["actor_id"], "player:Alice")
        self.assertEqual(decoded["phase"], "lightning")
        self.assertEqual(decoded["phase_tick"], 30)
        self.assertEqual(decoded["phase_duration_ticks"], 120)
        self.assertEqual(decoded["realm_from"], "spirit")
        self.assertEqual(decoded["realm_to"], "void")
        self.assertEqual(decoded["result"], "success")
        self.assertFalse(decoded["interrupted"])
        self.assertEqual(
            decoded["world_pos"],
            [100.0, 72.0, -200.0],
            "world_pos_x/y/z 是 field 9/10/11，拆成平铺坐标",
        )
        self.assertEqual(decoded["visible_radius_blocks"], 64.0)
        self.assertTrue(decoded["global"])
        self.assertTrue(decoded["distant_billboard"])
        self.assertAlmostEqual(decoded["particle_density"], 0.8, places=4)
        self.assertAlmostEqual(decoded["intensity"], 0.5, places=4)
        self.assertEqual(decoded["season_overlay"], "autumn")
        self.assertEqual(decoded["style"], "cinematic")
        self.assertEqual(decoded["at_tick"], 4242)

    def test_player_state_tag5_decodes_realm_enum(self):
        msg = _pb_varint(2, 5) + _pb_fixed64(3, 65.0) + _pb_fixed64(11, 100.0)
        decoded = proto_min.decode_server_data_envelope(_pb_message(5, msg))
        self.assertEqual(decoded["type"], "player_state")
        self.assertEqual(decoded["realm"], "Spirit", "PlayerState.realm=5 应解为 Spirit")
        self.assertEqual(decoded["spirit_qi"], 65.0)

    def test_player_state_absent_realm_uses_unspecified(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(5, b""))
        self.assertEqual(decoded["realm"], "Unspecified", "缺 field 2 应解为 Unspecified")

    def test_alchemy_outcome_resolved_tag14_full_contract(self):
        msg = (
            _pb_varint(1, 1)  # ALCHEMY_OUTCOME_BUCKET_PERFECT
            + _pb_string(2, "ling_xi_wan_v1")
            + _pb_string(3, "灵犀丹")
            + _pb_fixed64(4, 0.95)
            + _pb_fixed64(5, 0.05)
            + _pb_varint(6, 10)  # COLOR_KIND_TURBID
            + _pb_fixed64(7, 2.0)
            + _pb_string(8, "meridian_sore")
            + _pb_varint(9, 1)
            + _pb_fixed64(10, 1.5)
            + _pb_fixed64(11, 0.2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(14, msg))
        self.assertEqual(decoded["type"], "alchemy_outcome_resolved")
        self.assertEqual(decoded["bucket"], "perfect")
        self.assertEqual(decoded["recipe_id"], "ling_xi_wan_v1")
        self.assertEqual(decoded["pill"], "灵犀丹")
        self.assertEqual(decoded["quality"], 0.95)
        self.assertEqual(decoded["toxin_amount"], 0.05)
        self.assertEqual(decoded["toxin_color"], "turbid")
        self.assertEqual(decoded["qi_gain"], 2.0)
        self.assertEqual(decoded["side_effect_tag"], "meridian_sore")
        self.assertTrue(decoded["flawed_path"])
        self.assertEqual(decoded["damage"], 1.5)
        self.assertEqual(decoded["meridian_crack"], 0.2)

    def test_alchemy_outcome_resolved_empty_message_defaults(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(14, b""))
        self.assertEqual(decoded["bucket"], "unspecified")
        self.assertIsNone(decoded["recipe_id"])
        self.assertIsNone(decoded["pill"])
        self.assertIsNone(decoded["quality"])
        self.assertIsNone(decoded["toxin_amount"])
        self.assertIsNone(decoded["toxin_color"])
        self.assertIsNone(decoded["qi_gain"])
        self.assertIsNone(decoded["side_effect_tag"])
        self.assertFalse(decoded["flawed_path"])
        self.assertIsNone(decoded["damage"])
        self.assertIsNone(decoded["meridian_crack"])

    def test_out_of_range_enum_values_fall_back_to_unknown(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(14, _pb_varint(1, 99))
        )
        self.assertEqual(decoded["bucket"], "unknown_99")
        gathering = proto_min.decode_server_data_envelope(
            _pb_message(30, _pb_varint(5, 7))
        )
        self.assertEqual(gathering["target_type"], "unknown_7")

    def test_new_registry_tags_dispatch_to_named_decoders(self):
        for field, expected_type in (
            (5, "player_state"),
            (14, "alchemy_outcome_resolved"),
            (25, "botany_harvest_progress"),
            (30, "gathering_session"),
            (31, "lingtian_session"),
            (36, "skillbar_config"),
            (71, "breakthrough_cinematic"),
        ):
            with self.subTest(field=field):
                decoded = proto_min.decode_server_data_envelope(
                    _pb_message(field, b"")
                )
                self.assertIsNotNone(
                    decoded,
                    f"envelope tag {field} 必须有深度解码器，实际返回 None",
                )
                self.assertEqual(decoded["type"], expected_type)
        self.assertIsNone(
            proto_min.decode_server_data_envelope(_pb_message(6, b"")),
            "无深度解码器的 known oneof tag（cultivation_detail）应返回 None，不得误分发",
        )


class PlayerPacketContractTest(unittest.TestCase):
    """S1 新增玩家身份包（Player List/Remove/Spawn）与两张身份表的契约 pin。

    central-review finding 2：bot.py 新增的 PlayerList/PlayerRemove/PlayerSpawn
    解析与 entity→player 身份映射没有任何 packet-level 断言——忽略 action mask、
    给 spawn 配错 UUID、destroy 不删身份映射，场景都会照样绿。本类逐项钉死
    这些正/负半面。
    """

    def _player_list(self, bot, actions: int, raw_entries: list[bytes]):
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + bytes([actions])
            + mc.write_varint(len(raw_entries))
            + b"".join(raw_entries)
        )
        bot._dispatch(body)

    def test_player_list_add_updates_names_and_emits_entries(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        self._player_list(
            bot,
            0x11,  # add_player | update_latency
            [
                alice.bytes + mc.mc_string("Alice") + mc.write_varint(0) + mc.write_varint(42),
                bob.bytes + mc.mc_string("Bob") + mc.write_varint(0) + mc.write_varint(7),
            ],
        )
        self.assertEqual(bot.player_names, {str(alice): "Alice", str(bob): "Bob"})
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["actions"], 0x11)
        self.assertEqual(len(event.data["entries"]), 2)
        self.assertEqual(event.data["entries"][0]["uuid"], str(alice))
        self.assertEqual(event.data["entries"][0]["username"], "Alice")
        self.assertEqual(event.data["entries"][1]["username"], "Bob")

    def test_player_list_non_add_entry_does_not_fabricate_username(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        self._player_list(bot, 0x10, [alice.bytes + mc.write_varint(99)])
        self.assertEqual(
            bot.player_names,
            {},
            "仅 update_latency 的 entry 不得登记用户名——实现若忽略 action mask 就会误读 99 当 username",
        )
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["entries"][0]["uuid"], str(alice))
        self.assertNotIn("username", event.data["entries"][0])

    def test_player_list_full_action_mask_parses_all_action_payloads(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        entry_alice = (
            alice.bytes
            + mc.mc_string("Alice") + mc.write_varint(1)  # add_player + 1 property
            + mc.mc_string("textures") + mc.mc_string("http://skin") + b"\x01" + mc.mc_string("sig")
            + b"\x01" + alice.bytes + struct.pack(">q", 1234)  # initialize_chat signed session
            + mc.write_varint(2) + b"\xab\xcd" + mc.write_varint(1) + b"\xef"  # chat key/signature
            + mc.write_varint(4)  # update_game_mode
            + b"\x01"  # update_listed
            + mc.write_varint(15)  # update_latency
            + b"\x01" + mc.mc_string("§aAlice")  # update_display_name
        )
        # 第二条 entry 用不同的值，任何一条变长 payload 错位都会让本条的
        # 字段对不上号，从而暴露第一条 entry 的解析越界。
        entry_bob = (
            bob.bytes
            + mc.mc_string("Bob") + mc.write_varint(0)  # add_player + 0 properties
            + b"\x00"  # initialize_chat: no chat session
            + mc.write_varint(3)  # update_game_mode
            + b"\x00"  # update_listed: false
            + mc.write_varint(7)  # update_latency
            + b"\x00"  # update_display_name: absent
        )
        self._player_list(bot, 0x3F, [entry_alice, entry_bob])
        self.assertEqual(
            bot.player_names,
            {str(alice): "Alice", str(bob): "Bob"},
            "两条 entry 都要登记 username",
        )
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["actions"], 0x3F)
        parsed = event.data["entries"][0]
        # 六个 action 的 payload 全部要解码出来并被断言，跳过任意一个都会让
        # 后面的字段读错（尤其变长 payload：properties / chat key/sig / display_name）
        self.assertEqual(parsed["uuid"], str(alice))
        self.assertEqual(
            parsed["username"],
            "Alice",
            "0x01 add_player 必须解出 username",
        )
        self.assertEqual(
            parsed["properties"],
            [{"name": "textures", "value": "http://skin", "signature": "sig"}],
            "0x01 add_player 的 property 三元组（name/value/可选 signature）必须逐项解出",
        )
        self.assertEqual(
            parsed["initialize_chat"],
            {
                "has_chat_session": True,
                "session_id": str(alice),
                "public_key_expiry": 1234,
                "public_key": b"\xab\xcd",
                "signature": b"\xef",
            },
            "0x02 initialize_chat 的 signed session（has/会话 id/过期时间/公钥/signature）必须逐项解出",
        )
        self.assertEqual(
            parsed["game_mode"],
            4,
            "0x04 update_game_mode 必须解出 varint 值",
        )
        self.assertEqual(
            parsed["listed"],
            True,
            "0x08 update_listed 必须解出 boolean 值",
        )
        self.assertEqual(
            parsed["latency"],
            15,
            "0x10 update_latency 必须解出 varint 值",
        )
        self.assertEqual(
            parsed["display_name"],
            "§aAlice",
            "0x20 update_display_name 必须解出变长 string",
        )
        bob_entry = event.data["entries"][1]
        self.assertEqual(
            bob_entry,
            {
                "uuid": str(bob),
                "username": "Bob",
                "properties": [],
                "initialize_chat": {"has_chat_session": False},
                "game_mode": 3,
                "listed": False,
                "latency": 7,
            },
            "第二条 entry 六字段齐全且各为独立值——第一条的任何变长 payload 错位都会在此暴露",
        )

    def test_player_remove_pops_names(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        self._player_list(
            bot,
            0x01,
            [
                alice.bytes + mc.mc_string("Alice") + mc.write_varint(0),
                bob.bytes + mc.mc_string("Bob") + mc.write_varint(0),
            ],
        )
        remove = (
            mc.write_varint(mc.S2C_PLAYER_REMOVE)
            + mc.write_varint(1)
            + alice.bytes
        )
        bot._dispatch(remove)
        self.assertEqual(bot.player_names, {str(bob): "Bob"})
        event = bot.events_of("player_remove")[-1]
        self.assertEqual(event.data["uuids"], [str(alice)])

    def test_player_spawn_associates_entity_with_uuid_and_username(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        self._player_list(bot, 0x01, [alice.bytes + mc.mc_string("Alice") + mc.write_varint(0)])
        spawn = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(900)
            + alice.bytes
            + struct.pack(">ddd", 12.0, 64.0, -5.0)
            + b"\x00\x00"
        )
        bot._dispatch(spawn)
        self.assertEqual(bot.entity_pos(900), (12.0, 64.0, -5.0))
        self.assertEqual(
            bot.player_entity_uuids[900],
            str(alice),
            "player_spawn 必须把 entity_id 关联到正确的玩家 UUID",
        )
        event = bot.events_of("player_spawn")[-1]
        self.assertEqual(event.data["entity_id"], 900)
        self.assertEqual(event.data["uuid"], str(alice))
        self.assertEqual(event.data["username"], "Alice")
        self.assertEqual(event.data["yaw"], 0)
        self.assertEqual(event.data["pitch"], 0)

    def test_destroy_removes_entity_to_player_mapping(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        spawn = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(900)
            + alice.bytes
            + struct.pack(">ddd", 1.0, 2.0, 3.0)
            + b"\x00\x00"
        )
        bot._dispatch(spawn)
        self.assertIn(900, bot.player_entity_uuids)
        destroy = (
            mc.write_varint(mc.S2C_ENTITIES_DESTROY)
            + mc.write_varint(1)
            + mc.write_varint(900)
        )
        bot._dispatch(destroy)
        self.assertIsNone(bot.entity_pos(900))
        self.assertNotIn(
            900,
            bot.player_entity_uuids,
            "destroy 必须同时删除 entity→player 身份映射",
        )

    def test_entity_spawn_exposes_uuid(self):
        bot = _bare_bot()
        uid = uuid.UUID(int=77)
        body = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(10)
            + uid.bytes
            + mc.write_varint(3)
            + struct.pack(">ddd", 5.0, 6.0, 7.0)
        )
        bot._dispatch(body)
        self.assertEqual(bot.entity_pos(10), (5.0, 6.0, 7.0))
        event = bot.events_of("entity_spawn")[-1]
        self.assertEqual(event.data["uuid"], str(uid))

    def test_rel_move_emits_entity_move_event(self):
        bot = _bare_bot()
        spawn = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(7) + b"\x00" * 16 + mc.write_varint(1)
            + struct.pack(">ddd", 10.0, 64.0, -3.0)
        )
        bot._dispatch(spawn)
        move = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(7)
            + struct.pack(">hhh", 4096, -2048, 0)
            + b"\x01"
        )
        bot._dispatch(move)
        event = bot.events_of("entity_move")[-1]
        self.assertEqual(event.data["entity_id"], 7)
        self.assertAlmostEqual(event.data["x"], 11.0)
        self.assertAlmostEqual(event.data["y"], 63.5)
        self.assertAlmostEqual(event.data["z"], -3.0)

    def test_teleport_emits_entity_move_event(self):
        bot = _bare_bot()
        spawn = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(7) + b"\x00" * 16 + mc.write_varint(1)
            + struct.pack(">ddd", 10.0, 64.0, -3.0)
        )
        bot._dispatch(spawn)
        teleport = (
            mc.write_varint(mc.S2C_ENTITY_TELEPORT)
            + mc.write_varint(7)
            + struct.pack(">ddd", -100.0, 70.0, 200.0)
            + b"\x00\x00\x01"
        )
        bot._dispatch(teleport)
        event = bot.events_of("entity_move")[-1]
        self.assertEqual(event.data, {"entity_id": 7, "x": -100.0, "y": 70.0, "z": 200.0})


if __name__ == "__main__":
    unittest.main(verbosity=1)
