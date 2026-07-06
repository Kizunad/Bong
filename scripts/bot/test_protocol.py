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
from bot.bot import Bot, _signed_12, _signed_26  # noqa: E402
from bot.server_data import decode_server_data_payload  # noqa: E402
from bot.scenarios._inventory_helpers import (  # noqa: E402
    latest_inventory_snapshot,
    wait_inventory_revision_after,
    wait_inventory_revision_after_matching,
    wait_inventory_snapshot_after,
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


if __name__ == "__main__":
    unittest.main(verbosity=1)
