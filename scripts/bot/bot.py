"""协议级黑盒玩家 Bot —— Bong e2e 场景的动作 + 观察 + 断言 API。

设计原则（AGENTS.md §15）：
- 动作只走真实协议：dev 命令(0x04) / chat(0x05) / `bong:client_request` JSON intent(0x0D) / 原版交互包
- 观察只走真实协议：vanilla 包 + `bong:*` payload 通道 + chat 反馈；不读 server 内部状态
- 断言消息带修复线索："期望 X 因为 Y，实际 Z"

Bot 故意**不**发送 ClientSettings / 不注册 plugin channel / 不接资源包 ——
这是"Bot 客户端最大化宽容"红线的常驻回归面；需要时用 send_client_settings() 显式补。
"""

from __future__ import annotations

import json
import struct
import threading
import time

from . import mc_protocol as mc
from . import proto_min
from .mc_protocol import Connection, Reader, login, mc_string, write_varint
from .server_data import decode_server_data_payload


class BotAssertionError(AssertionError):
    """场景断言失败（带修复线索的消息由调用方拼好）。"""


class Event:
    """一条已解码的 S2C 观察事件。kind 是稳定字符串契约，场景据此断言。"""

    __slots__ = ("t", "kind", "data")

    def __init__(self, t: float, kind: str, data: dict):
        self.t = t
        self.kind = kind
        self.data = data

    def __repr__(self) -> str:
        return f"Event({self.t:.3f}s {self.kind} {self.data})"


class Bot:
    """一条独立连接 = 一个玩家。用完必须 close()（或用 with 语句）。"""

    def __init__(
        self,
        username: str,
        host: str = "127.0.0.1",
        port: int = 25565,
        connect_timeout: float = 10.0,
    ):
        if len(username) > 16:
            raise ValueError(f"MC 用户名上限 16 字符：{username!r} 有 {len(username)} 字符")
        self.username = username
        self.conn = Connection(host, port, timeout=connect_timeout)
        login(self.conn, username)
        self.conn.sock.settimeout(0.5)

        self.t0 = time.monotonic()
        self.events: list[Event] = []
        # 实体位置表（entity_spawn 建、rel-move/teleport 更、destroy 删）——
        # 近战场景要追活体 NPC，不能拿 spawn 坐标当靶（NPC 会走）。
        self.entities: dict[int, tuple[float, float, float]] = {}
        # RLock：wait_for 的 predicate 在持锁状态下执行，场景里 predicate 常会
        # 回调 events_of()/chunk_count 等同样要锁的方法——非重入锁在这里会死锁。
        self._lock = threading.RLock()
        self._new_event = threading.Condition(self._lock)
        self._send_lock = threading.Lock()
        self._closing = False

        # 便捷状态镜像（都能从 events 重建，仅为场景少写样板）
        self.position: tuple[float, float, float] | None = None
        self.health: float | None = None
        self.entity_id: int | None = None
        self.disconnect_reason: str | None = None
        self.chunk_count = 0

        self._reader_thread = threading.Thread(
            target=self._reader_loop, name=f"bot-reader-{username}", daemon=True
        )
        self._reader_thread.start()

    # ------------------------------------------------------------------ 生命周期

    def __enter__(self) -> "Bot":
        return self

    def __exit__(self, *_exc) -> None:
        self.close()

    def close(self) -> None:
        self._closing = True
        self.conn.close()
        self._reader_thread.join(timeout=2.0)

    @property
    def alive(self) -> bool:
        """连接仍活着（reader 未因断连/踢出退出）。"""
        return self._reader_thread.is_alive() and self.disconnect_reason is None

    # ------------------------------------------------------------------ 观察面

    def _emit(self, kind: str, data: dict) -> None:
        event = Event(time.monotonic() - self.t0, kind, data)
        with self._new_event:
            self.events.append(event)
            self._new_event.notify_all()

    def _reader_loop(self) -> None:
        while not self._closing:
            try:
                body = self.conn.read_frame()
            except TimeoutError:
                continue
            except (ConnectionError, OSError):
                if not self._closing:
                    self._emit("connection_lost", {})
                return
            try:
                self._dispatch(body)
            except Exception as error:  # 解码坏一个包不拖垮整个观察流
                self._emit("decode_error", {"error": repr(error), "size": len(body)})

    def _dispatch(self, body: bytes) -> None:
        reader = Reader(body)
        packet_id = reader.varint()

        if packet_id == mc.S2C_KEEP_ALIVE:
            ka_id = reader.i64()
            self._send(mc.C2S_KEEP_ALIVE, struct.pack(">q", ka_id))
            self._emit("keepalive", {"id": ka_id})
        elif packet_id == mc.S2C_POS_LOOK:
            x, y, z = reader.f64(), reader.f64(), reader.f64()
            yaw, pitch = reader.f32(), reader.f32()
            flags = reader.u8()
            teleport_id = reader.varint()
            self._send(mc.C2S_TELEPORT_CONFIRM, write_varint(teleport_id))
            self.position = (x, y, z)
            self._emit(
                "pos_look",
                {"x": x, "y": y, "z": z, "yaw": yaw, "pitch": pitch, "flags": flags},
            )
        elif packet_id == mc.S2C_GAME_JOIN:
            self.entity_id = reader.i32()
            self._emit("game_join", {"entity_id": self.entity_id})
        elif packet_id == mc.S2C_CHUNK_DATA:
            x, z = reader.i32(), reader.i32()
            self.chunk_count += 1
            self._emit("chunk_data", {"x": x, "z": z})
        elif packet_id == mc.S2C_UNLOAD_CHUNK:
            self._emit("unload_chunk", {"x": reader.i32(), "z": reader.i32()})
        elif packet_id == mc.S2C_CHUNK_CENTER:
            self._emit("chunk_center", {"x": reader.varint(), "z": reader.varint()})
        elif packet_id == mc.S2C_CHUNK_LOAD_DISTANCE:
            self._emit("chunk_load_distance", {"distance": reader.varint()})
        elif packet_id == mc.S2C_CUSTOM_PAYLOAD:
            channel = reader.string()
            data = reader.rest()
            self._emit("payload", {"channel": channel, "data": data})
            if channel == "bong:server_data":
                try:
                    payload = decode_server_data_payload(data)
                except Exception as error:
                    self._emit(
                        "server_data_decode_error",
                        {"error": repr(error), "size": len(data)},
                    )
                else:
                    if payload is not None:
                        self._emit(
                            "server_data",
                            {"payload_type": payload.get("type"), "payload": payload},
                        )
            elif channel == "bong:vfx_event":
                try:
                    payload = json.loads(data.decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError) as error:
                    self._emit("vfx_decode_error", {"error": repr(error), "size": len(data)})
                else:
                    if isinstance(payload, dict):
                        self._emit("vfx_event", payload)
        elif packet_id == mc.S2C_GAME_MESSAGE:
            raw = reader.string()
            overlay = reader.boolean()
            self._emit(
                "chat",
                {"text": mc.chat_text_to_plain(raw), "raw": raw, "overlay": overlay},
            )
        elif packet_id == mc.S2C_HEALTH_UPDATE:
            self.health = reader.f32()
            food = reader.varint()
            self._emit("health", {"health": self.health, "food": food})
        elif packet_id == mc.S2C_ENTITY_SPAWN:
            entity_id = reader.varint()
            reader.pos += 16  # uuid
            entity_type = reader.varint()
            x, y, z = reader.f64(), reader.f64(), reader.f64()
            self.entities[entity_id] = (x, y, z)
            self._emit(
                "entity_spawn",
                {"entity_id": entity_id, "type": entity_type, "x": x, "y": y, "z": z},
            )
        elif packet_id in (mc.S2C_ENTITY_POSITION, mc.S2C_ENTITY_POSITION_ROTATION):
            entity_id = reader.varint()
            dx = reader.i16() / 4096.0
            dy = reader.i16() / 4096.0
            dz = reader.i16() / 4096.0
            prev = self.entities.get(entity_id)
            if prev is not None:
                self.entities[entity_id] = (prev[0] + dx, prev[1] + dy, prev[2] + dz)
        elif packet_id == mc.S2C_ENTITY_TELEPORT:
            entity_id = reader.varint()
            x, y, z = reader.f64(), reader.f64(), reader.f64()
            self.entities[entity_id] = (x, y, z)
        elif packet_id == mc.S2C_ENTITIES_DESTROY:
            count = reader.varint()
            ids = [reader.varint() for _ in range(count)]
            for eid in ids:
                self.entities.pop(eid, None)
            self._emit("entities_destroy", {"entity_ids": ids})
        elif packet_id == mc.S2C_BLOCK_UPDATE:
            packed = struct.unpack_from(">Q", reader.data, reader.pos)[0]
            reader.pos += 8
            x = _signed_26(packed >> 38)
            z = _signed_26((packed >> 12) & 0x3FFFFFF)
            y = _signed_12(packed & 0xFFF)
            self._emit("block_update", {"x": x, "y": y, "z": z, "state": reader.varint()})
        elif packet_id == mc.S2C_DEATH_MESSAGE:
            self._emit("death_message", {"size": len(body)})
        elif packet_id == mc.S2C_RESPAWN:
            self._emit("respawn", {"size": len(body)})
        elif packet_id == mc.S2C_DISCONNECT:
            reason = reader.string()
            self.disconnect_reason = mc.chat_text_to_plain(reason)
            self._emit("disconnect", {"reason": self.disconnect_reason})
        else:
            self._emit("other", {"packet_id": packet_id, "size": len(body)})

    # ------------------------------------------------------------------ 动作面

    def _send(self, packet_id: int, body: bytes = b"") -> None:
        with self._send_lock:
            self.conn.send_packet(packet_id, body)

    def cmd(self, command: str) -> None:
        """执行服务器命令（不带前导 `/`），如 cmd("give starter_talisman 1")。

        763 命令包带聊天签名字段；offline server 不校验，全部填零。
        """
        command = command.lstrip("/")
        body = (
            mc_string(command)
            + struct.pack(">qq", 0, 0)  # timestamp + salt
            + write_varint(0)  # argument signatures: 0 条
            + write_varint(0)  # message_count
            + b"\x00\x00\x00"  # acknowledged 固定 20-bit BitSet
        )
        self._send(mc.C2S_COMMAND_EXECUTION, body)

    def chat(self, message: str) -> None:
        body = (
            mc_string(message)
            + struct.pack(">qq", 0, 0)
            + b"\x00"  # has_signature=false
            + write_varint(0)  # message_count
            + b"\x00\x00\x00"  # acknowledged BitSet
        )
        self._send(mc.C2S_CHAT_MESSAGE, body)

    def send_payload(self, channel: str, data: bytes) -> None:
        """向任意 plugin channel 发原始字节（宽容度测试直接喂坏数据用这个）。"""
        self._send(mc.C2S_CUSTOM_PAYLOAD, mc_string(channel) + data)

    def intent(self, request: dict) -> None:
        """向 `bong:client_request` 发 ClientRequestV1 JSON —— 和 Fabric 客户端 UI 同构的玩法驱动入口。

        字段形状以 server/src/schema/client_request.rs 为准（tag="type"，snake_case，
        deny_unknown_fields：字段写错会被 server 拒收并留 warn log）。
        """
        self.send_payload("bong:client_request", json.dumps(request).encode("utf-8"))

    def set_position(self, x: float, y: float, z: float, on_ground: bool = True) -> None:
        self._send(
            mc.C2S_MOVE_POSITION,
            struct.pack(">ddd", x, y, z) + (b"\x01" if on_ground else b"\x00"),
        )
        self.position = (x, y, z)

    def move_to(
        self, x: float, y: float, z: float, speed: float = 4.0, tick_hz: float = 20.0
    ) -> None:
        """以近似步行速度直线走向目标（阻塞）。别瞬移：server 有 anticheat 观察面。"""
        if self.position is None:
            raise BotAssertionError(
                "move_to 需要已知当前位置（等 pos_look 事件先到），实际 position=None"
            )
        step = speed / tick_hz
        cx, cy, cz = self.position
        while True:
            dx, dy, dz = x - cx, y - cy, z - cz
            dist = (dx * dx + dy * dy + dz * dz) ** 0.5
            if dist <= step:
                self.set_position(x, y, z)
                return
            cx += dx / dist * step
            cy += dy / dist * step
            cz += dz / dist * step
            self.set_position(cx, cy, cz)
            time.sleep(1.0 / tick_hz)

    def entity_pos(self, entity_id: int) -> tuple[float, float, float] | None:
        """实体最近已知坐标（spawn+rel-move+teleport 累积）；已 destroy 返回 None。"""
        with self._lock:
            return self.entities.get(entity_id)

    def swing(self, hand: int = 0) -> None:
        self._send(mc.C2S_HAND_SWING, write_varint(hand))

    def attack_entity(self, entity_id: int, sneaking: bool = False) -> None:
        body = (
            write_varint(entity_id)
            + write_varint(1)  # type=attack
            + (b"\x01" if sneaking else b"\x00")
        )
        self._send(mc.C2S_INTERACT_ENTITY, body)
        self.swing()

    def use_item(self, hand: int = 0, sequence: int = 0) -> None:
        self._send(mc.C2S_INTERACT_ITEM, write_varint(hand) + write_varint(sequence))

    def select_slot(self, slot: int) -> None:
        self._send(mc.C2S_SELECT_SLOT, struct.pack(">h", slot))

    def respawn(self) -> None:
        self._send(mc.C2S_CLIENT_STATUS, write_varint(0))

    def send_client_settings(self, view_distance: int = 10) -> None:
        """显式补发 ClientSettings（默认不发是宽容度回归面的一部分）。"""
        body = (
            mc_string("zh_cn")
            + bytes([view_distance])
            + write_varint(0)  # chat mode: enabled
            + b"\x01"  # chat colors
            + bytes([0x7F])  # skin parts
            + write_varint(1)  # main hand: right
            + b"\x00"  # text filtering
            + b"\x01"  # allow server listings
        )
        self._send(mc.C2S_CLIENT_SETTINGS, body)

    # ------------------------------------------------------------------ 断言面

    def wait_for(self, predicate, timeout: float, description: str) -> Event:
        """等第一个满足 predicate 的事件（含历史事件）。超时抛 BotAssertionError。"""
        deadline = time.monotonic() + timeout
        cursor = 0
        with self._new_event:
            while True:
                while cursor < len(self.events):
                    event = self.events[cursor]
                    cursor += 1
                    if predicate(event):
                        return event
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise BotAssertionError(
                        f"[{self.username}] 期望 {timeout}s 内出现「{description}」，"
                        f"实际超时未出现；已收 {len(self.events)} 事件，"
                        f"最近 5 条: {self.events[-5:]}"
                    )
                self._new_event.wait(timeout=min(remaining, 0.5))

    def expect_event(self, kind: str, timeout: float = 5.0) -> Event:
        return self.wait_for(lambda e: e.kind == kind, timeout, f"kind={kind} 事件")

    def expect_payload(self, channel: str, timeout: float = 5.0) -> Event:
        return self.wait_for(
            lambda e: e.kind == "payload" and e.data["channel"] == channel,
            timeout,
            f"channel={channel} 的 payload（server 应 emit 该通道，检查 emit system 是否接线）",
        )

    def expect_server_data(self, payload_type: str, timeout: float = 5.0) -> Event:
        return self.wait_for(
            lambda e: e.kind == "server_data" and e.data["payload_type"] == payload_type,
            timeout,
            f"bong:server_data/{payload_type} payload（server 应推送该玩法数据）",
        )

    def expect_vfx_event(self, event_id: str, timeout: float = 5.0) -> Event:
        return self.wait_for(
            lambda e: e.kind == "vfx_event" and e.data.get("event_id") == event_id,
            timeout,
            f"bong:vfx_event event_id={event_id}（玩法反馈 VFX 不应失联）",
        )

    def expect_payload_after(self, channel: str, after: float, timeout: float = 5.0) -> Event:
        return self.wait_for(
            lambda e: e.kind == "payload" and e.data["channel"] == channel and e.t > after,
            timeout,
            f"t>{after:.3f}s 后 channel={channel} 的 payload",
        )

    def expect_server_data_payload(
        self,
        payload_name: str | None = None,
        timeout: float = 5.0,
        after: float = 0.0,
    ) -> Event:
        def matches(event: Event) -> bool:
            if (
                event.kind != "payload"
                or event.data["channel"] != "bong:server_data"
                or event.t <= after
            ):
                return False
            if payload_name is None:
                return True
            return proto_min.server_data_payload_name(event.data["data"]) == payload_name

        detail = "任意 bong:server_data payload"
        if payload_name is not None:
            detail = f"payload_type={payload_name} 的 bong:server_data payload"
        return self.wait_for(matches, timeout, f"t>{after:.3f}s 后 {detail}")

    def expect_inventory_item(
        self,
        item_id: str,
        timeout: float = 5.0,
        after: float = 0.0,
    ) -> proto_min.InventoryItemRef:
        found: dict[str, proto_min.InventoryItemRef] = {}

        def matches(event: Event) -> bool:
            if (
                event.kind != "payload"
                or event.data["channel"] != "bong:server_data"
                or event.t <= after
            ):
                return False
            for item in proto_min.inventory_item_refs(event.data["data"]):
                if item.item_id == item_id:
                    found["item"] = item
                    return True
            return False

        self.wait_for(
            matches,
            timeout,
            (
                f"inventory_snapshot 内出现 item_id={item_id}"
                "（bot 只做 protobuf 浅扫描以取得 instance_id，P6 再决定深断言方案）"
            ),
        )
        return found["item"]

    def expect_chat(self, substring: str, timeout: float = 5.0) -> Event:
        return self.wait_for(
            lambda e: e.kind == "chat" and substring in e.data["text"],
            timeout,
            f"包含「{substring}」的聊天消息",
        )

    def assert_alive(self, context: str) -> None:
        if self.disconnect_reason is not None:
            raise BotAssertionError(
                f"[{self.username}] 期望连接保持（{context}），"
                f"实际被服务器断开：{self.disconnect_reason!r}"
            )
        if not self._reader_thread.is_alive():
            raise BotAssertionError(
                f"[{self.username}] 期望连接保持（{context}），实际底层 socket 已断"
            )

    def events_of(self, kind: str) -> list[Event]:
        with self._lock:
            return [e for e in self.events if e.kind == kind]


def _signed_26(value: int) -> int:
    return value - (1 << 26) if value >= (1 << 25) else value


def _signed_12(value: int) -> int:
    return value - (1 << 12) if value >= (1 << 11) else value
