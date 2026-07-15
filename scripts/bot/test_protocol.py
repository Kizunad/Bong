#!/usr/bin/env python3
"""Bot 框架编解码底座 + runner 纯逻辑的单元测试（无需 server，纯 stdlib）。

跑法：python3 scripts/bot/test_protocol.py
bot-e2e.sh 在起 server 之前先跑本文件——编解码坏了没必要浪费一次 server 启动。
"""

from __future__ import annotations

import ast
import json
import os
import pathlib
import re
import socket
import struct
import sys
import tempfile
import threading
import time
import tomllib
import types
import unittest
import zlib
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bot import mc_protocol as mc  # noqa: E402
from bot import make_novice_raster_fixture  # noqa: E402
from bot import proto_min  # noqa: E402
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
from bot.scenarios.terrain_poi_novice_startup import (  # noqa: E402
    _selection_strategy,
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

    def test_player_action_response_decodes_sequence(self):
        bot = _bare_bot()
        bot._dispatch(
            mc.write_varint(mc.S2C_PLAYER_ACTION_RESPONSE) + mc.write_varint(17)
        )

        event = bot.events[-1]
        self.assertEqual(event.kind, "player_action_response")
        self.assertEqual(event.data, {"sequence": 17})


class NoviceRasterFixtureTest(unittest.TestCase):
    def test_fixture_exposes_deterministic_spiritwood_seed_without_changing_poi_tile(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = make_novice_raster_fixture.generate(root)
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
            "cmd_dev_give_feedback",
            "cultivation_realm_qi",
            "network_client_request_tolerance",
            "network_session_tolerance",
            "terrain_join_chunk_delivery",
        }
        self.assertTrue(
            expected <= names,
            f"已提交场景应全部被发现（模块更新必配场景的 CI 抓手），实际 {names}",
        )

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
        freshness = (
            _pb_varint(1, 123)
            + _pb_fixed32(2, 98.5)
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
                "initial_qi": 98.5,
                "track": "Decay",
                "profile": "ling_mu_gun_v1",
                "frozen_accumulated": 17,
                "frozen_since_tick": 140,
            },
            "dropped_loot_sync 必须保留完整 freshness，拾取后才能对拍同一实例 NBT",
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
        furnace = _pb_varint_field(4, 1)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(11, furnace))
        self.assertEqual(
            decoded["type"], "alchemy_furnace", "envelope tag 11 应分发到 alchemy_furnace"
        )
        self.assertEqual(decoded["tier"], 1, "AlchemyFurnace.tier 是 field 4")

        session = _pb_len_field(1, b"ling_xi_wan_v1") + _pb_varint_field(2, 1)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(12, session))
        self.assertEqual(
            decoded["type"], "alchemy_session", "envelope tag 12 应分发到 alchemy_session"
        )
        self.assertEqual(
            decoded["recipe_id"], "ling_xi_wan_v1", "AlchemySession.recipe_id 是 field 1"
        )
        self.assertTrue(decoded["active"], "AlchemySession.active 是 field 2")

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


if __name__ == "__main__":
    unittest.main(verbosity=1)
