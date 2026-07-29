"""MC 1.20.1 (protocol 763) offline-mode 协议层 —— Bot e2e 框架传输/编解码底座。

纯 stdlib，零第三方依赖（CI 免装包）。覆盖：
- VarInt 帧 + LoginCompression 后的 zlib 帧
- offline LoginStart 握手（无加密；server 必须 offline mode）
- 常用 C2S 动作包编码 / 常用 S2C 观察包解码

包 ID 权威来源：valence checkout `tools/packet_inspector/extracted/packets.json`
（rev 2b705351，与 server Cargo.toml pin 一致）。新增包 ID 时先查那份 JSON，
不要凭 wiki.vg 其他版本猜。
"""

from __future__ import annotations

import json
import socket
import struct
import uuid
import zlib

PROTOCOL_VERSION = 763

# ---- S2C play 包 ID（观察面）----
S2C_ENTITY_SPAWN = 0x01
S2C_PLAYER_SPAWN = 0x03
S2C_PLAYER_ACTION_RESPONSE = 0x06
S2C_BLOCK_UPDATE = 0x0A
S2C_INVENTORY = 0x12
S2C_SLOT_UPDATE = 0x14
S2C_CUSTOM_PAYLOAD = 0x17
S2C_DISCONNECT = 0x1A
S2C_UNLOAD_CHUNK = 0x1E
S2C_KEEP_ALIVE = 0x23
S2C_CHUNK_DATA = 0x24
S2C_GAME_JOIN = 0x28
S2C_ENTITY_POSITION = 0x2B
S2C_ENTITY_POSITION_ROTATION = 0x2C
S2C_PLAYER_CHAT = 0x35
S2C_DEATH_MESSAGE = 0x38
S2C_PLAYER_REMOVE = 0x39
S2C_PLAYER_LIST = 0x3A
S2C_POS_LOOK = 0x3C
S2C_ENTITIES_DESTROY = 0x3E
S2C_ENTITY_TELEPORT = 0x68
S2C_RESPAWN = 0x41
S2C_CHUNK_CENTER = 0x4E
S2C_CHUNK_LOAD_DISTANCE = 0x4F
S2C_SPAWN_POSITION = 0x50
S2C_HEALTH_UPDATE = 0x57
S2C_GAME_MESSAGE = 0x64

# ---- C2S play 包 ID（动作面）----
C2S_TELEPORT_CONFIRM = 0x00
C2S_COMMAND_EXECUTION = 0x04
C2S_CHAT_MESSAGE = 0x05
C2S_CLIENT_STATUS = 0x07
C2S_CLIENT_SETTINGS = 0x08
C2S_CUSTOM_PAYLOAD = 0x0D
C2S_INTERACT_ENTITY = 0x10
C2S_KEEP_ALIVE = 0x12
C2S_MOVE_POSITION = 0x14
C2S_MOVE_FULL = 0x15
C2S_PLAYER_ACTION = 0x1D
C2S_SELECT_SLOT = 0x28
C2S_HAND_SWING = 0x2F
C2S_INTERACT_ITEM = 0x32


def write_varint(value: int) -> bytes:
    out = b""
    value &= 0xFFFFFFFF
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out += bytes([byte | 0x80])
        else:
            out += bytes([byte])
            return out


def mc_string(s: str) -> bytes:
    raw = s.encode("utf-8")
    return write_varint(len(raw)) + raw


def block_position(x: int, y: int, z: int) -> bytes:
    """vanilla Position 编码：x/z 各 26 bit，y 12 bit，拼一个 u64 大端。"""
    packed = ((x & 0x3FFFFFF) << 38) | ((z & 0x3FFFFFF) << 12) | (y & 0xFFF)
    return struct.pack(">Q", packed)


class Reader:
    """从 bytes 顺序读取 MC 协议基本类型。"""

    def __init__(self, data: bytes):
        self.data = data
        self.pos = 0

    def varint(self) -> int:
        value = 0
        for shift in range(0, 35, 7):
            byte = self.data[self.pos]
            self.pos += 1
            value |= (byte & 0x7F) << shift
            if not byte & 0x80:
                break
        else:
            raise ValueError("varint 超过 5 字节 —— 流错位或包解析越界")
        if value >= 2**31:
            value -= 2**32
        return value

    def i16(self) -> int:
        value = struct.unpack_from(">h", self.data, self.pos)[0]
        self.pos += 2
        return value

    def i32(self) -> int:
        value = struct.unpack_from(">i", self.data, self.pos)[0]
        self.pos += 4
        return value

    def i64(self) -> int:
        value = struct.unpack_from(">q", self.data, self.pos)[0]
        self.pos += 8
        return value

    def f32(self) -> float:
        value = struct.unpack_from(">f", self.data, self.pos)[0]
        self.pos += 4
        return value

    def f64(self) -> float:
        value = struct.unpack_from(">d", self.data, self.pos)[0]
        self.pos += 8
        return value

    def u8(self) -> int:
        value = self.data[self.pos]
        self.pos += 1
        return value

    def boolean(self) -> bool:
        return self.u8() != 0

    def string(self) -> str:
        length = self.varint()
        raw = self.data[self.pos : self.pos + length]
        self.pos += length
        return raw.decode("utf-8", "replace")

    def uuid(self) -> str:
        raw = self.data[self.pos : self.pos + 16]
        if len(raw) != 16:
            raise ValueError("UUID 需要 16 字节，packet 已截断")
        self.pos += 16
        return str(uuid.UUID(bytes=raw))

    def rest(self) -> bytes:
        return self.data[self.pos :]


class Connection:
    """帧层：VarInt 长度前缀 + 可选 zlib 压缩。线程安全性：send 侧加锁由 Bot 负责。"""

    def __init__(self, host: str, port: int, timeout: float = 10.0):
        self.sock = socket.create_connection((host, port), timeout=timeout)
        self.host = host
        self.port = port
        self.buf = b""
        self.compression_threshold = -1

    def _try_parse_frame(self) -> bytes | None:
        """只在 buf 里已有**完整**一帧时才消费并返回，否则原样保留返回 None。

        这样 socket timeout 永远只落在两次 recv 之间，半帧不会被撕掉——
        长连接 + 短 timeout 轮询下的流错位就是这么防的。
        """
        length = 0
        offset = 0
        for shift in range(0, 35, 7):
            if offset >= len(self.buf):
                return None
            byte = self.buf[offset]
            offset += 1
            length |= (byte & 0x7F) << shift
            if not byte & 0x80:
                break
        else:
            raise ValueError("frame varint 超长 —— 流错位")
        if len(self.buf) < offset + length:
            return None
        payload = self.buf[offset : offset + length]
        self.buf = self.buf[offset + length :]
        if self.compression_threshold >= 0:
            reader = Reader(payload)
            data_len = reader.varint()
            body = payload[reader.pos :]
            if data_len > 0:
                body = zlib.decompress(body)
            return body
        return payload

    def read_frame(self) -> bytes:
        """读一帧，返回解压后的 packet body（含 packet id varint）。

        socket timeout 会原样抛出（TimeoutError），此时帧缓冲保持一致，可直接重试。
        """
        while True:
            frame = self._try_parse_frame()
            if frame is not None:
                return frame
            chunk = self.sock.recv(65536)
            if not chunk:
                raise ConnectionError("socket closed by server")
            self.buf += chunk

    def send_packet(self, packet_id: int, body: bytes = b"") -> None:
        data = write_varint(packet_id) + body
        if self.compression_threshold >= 0:
            if len(data) >= self.compression_threshold:
                frame = write_varint(len(data)) + zlib.compress(data)
            else:
                frame = write_varint(0) + data
            self.sock.sendall(write_varint(len(frame)) + frame)
        else:
            self.sock.sendall(write_varint(len(data)) + data)

    def close(self) -> None:
        try:
            self.sock.close()
        except OSError:
            pass


class LoginError(RuntimeError):
    pass


def login(conn: Connection, username: str) -> None:
    """握手 + offline LoginStart，直到 LoginSuccess 进入 play 阶段。"""
    conn.send_packet(
        0x00,
        write_varint(PROTOCOL_VERSION)
        + mc_string(conn.host)
        + struct.pack(">H", conn.port)
        + write_varint(2),
    )
    conn.send_packet(0x00, mc_string(username) + b"\x00")  # has_uuid=false
    while True:
        body = conn.read_frame()
        reader = Reader(body)
        packet_id = reader.varint()
        if packet_id == 0x03:  # LoginCompression
            conn.compression_threshold = reader.varint()
        elif packet_id == 0x02:  # LoginSuccess → play
            return
        elif packet_id == 0x00:  # LoginDisconnect
            raise LoginError(f"login disconnect: {body[reader.pos:][:200]!r}")
        elif packet_id == 0x01:
            raise LoginError("server 要求加密 —— 不是 offline mode，bot 无法登录")


def chat_text_to_plain(raw: str) -> str:
    """把 S2C 文本组件 JSON 拍平成纯文本（取 text + extra 递归拼接）。"""

    def walk(node) -> str:
        if isinstance(node, str):
            return node
        if isinstance(node, list):
            return "".join(walk(item) for item in node)
        if isinstance(node, dict):
            own = node.get("text", "")
            extra = "".join(walk(item) for item in node.get("extra", []))
            return own + extra
        return ""

    try:
        return walk(json.loads(raw))
    except (json.JSONDecodeError, TypeError):
        return raw
