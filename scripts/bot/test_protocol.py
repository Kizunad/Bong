#!/usr/bin/env python3
"""Bot 框架编解码底座 + runner 纯逻辑的单元测试（无需 server，纯 stdlib）。

跑法：python3 scripts/bot/test_protocol.py
bot-e2e.sh 在起 server 之前先跑本文件——编解码坏了没必要浪费一次 server 启动。
"""

from __future__ import annotations

import json
import os
import socket
import struct
import sys
import threading
import types
import unittest
import zlib

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bot import mc_protocol as mc  # noqa: E402
from bot import proto_min  # noqa: E402
from bot.bot import Bot, _signed_12, _signed_26  # noqa: E402
from bot.server_data import decode_server_data_payload  # noqa: E402
from bot.scenarios._inventory_helpers import (  # noqa: E402
    latest_inventory_snapshot,
    wait_inventory_revision_after,
    wait_inventory_revision_after_matching,
    wait_inventory_snapshot_after,
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
    bot.position = None
    bot.health = None
    bot.entity_id = None
    bot.disconnect_reason = None
    bot.chunk_count = 0
    return bot


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


class ProdConsumeDecodeTest(unittest.TestCase):
    """三产三用 payload 解码 pin：envelope oneof tag 与字段号对齐 proto/bong/envelope.proto.

    这些解码器是 production_*/combat_*/cultivation_pill 场景的观察面地基——
    tag 或字段号漂移会让场景从「锁契约」退化成「永远超时」。
    """

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
        item = _pb_varint(1, 77) + _pb_string(2, "fan_tie") + _pb_varint(9, 1)
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
