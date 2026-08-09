"""Bot 场景用的最小 Redis RESP2 客户端（纯 stdlib，不依赖第三方 redis 库）。

仓库惯例是手写协议客户端（mc_protocol.py / proto_min.py 同款思路）：场景只需要
PUBLISH（注入 bong:agent_ui_cmd 等）与 SUBSCRIBE（观察 bong:agent_ui_response 等）
两个命令，不值得引入 redis-py。

设计：
- ``RespFrames`` —— 纯编解码器（feed bytes → 拉帧），与 socket 解耦，可单测。
- ``RedisPubSub`` —— PUBLISH 走短连接（发完即关），SUBSCRIBE 走常驻连接，
  只有 ``message`` 帧会暴露给调用方，subscribe ack / pong 等被内部吞掉。

只实现 RESP2 子集：+simple / -error / :integer / $bulk（含 $-1 nil）/ *array。
"""

from __future__ import annotations

import json
import socket
import time


class RespFrames:
    """RESP2 帧解析器：``feed()`` 喂字节，``next_frame()`` 取帧；数据不足返回 None。"""

    def __init__(self) -> None:
        self._buf = b""

    def feed(self, data: bytes) -> None:
        if data:
            self._buf += data

    def next_frame(self):
        frame, consumed = self._parse_frame(0)
        if consumed == 0:
            return None
        self._buf = self._buf[consumed:]
        return frame

    def _line(self, pos: int) -> tuple[bytes | None, int]:
        idx = self._buf.find(b"\r\n", pos)
        if idx == -1:
            return None, 0
        return self._buf[pos:idx], idx

    def _parse_frame(self, pos: int):
        """返回 (value, consumed)；数据不足返回 (None, 0)。"""
        if pos >= len(self._buf):
            return None, 0
        marker = self._buf[pos : pos + 1]
        if marker in (b"+", b"-"):
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            return line.decode("utf-8", "replace"), idx + 2
        if marker == b":":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            return int(line), idx + 2
        if marker == b"$":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            length = int(line)
            if length == -1:
                return None, idx + 2
            body_start = idx + 2
            if len(self._buf) < body_start + length + 2:
                return None, 0
            return self._buf[body_start : body_start + length], body_start + length + 2
        if marker == b"*":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            count = int(line)
            if count == -1:
                return None, idx + 2
            items: list = []
            offset = idx + 2
            for _ in range(count):
                item, consumed = self._parse_frame(offset)
                if consumed == 0:
                    return None, 0
                items.append(item)
                offset = consumed
            return items, offset
        raise ValueError(f"unknown RESP marker {marker!r}")


def _encode_command(*parts: str) -> bytes:
    out = f"*{len(parts)}\r\n".encode("ascii")
    for part in parts:
        raw = part.encode("utf-8")
        out += f"${len(raw)}\r\n".encode("ascii") + raw + b"\r\n"
    return out


class RedisPubSub:
    """最小 Redis 客户端：PUBLISH（短连接）+ SUBSCRIBE（常驻连接）+ 消息等待。

    场景用法：先 ``subscribe("bong:agent_ui_response")`` 再触发 server 行为，
    ``wait_message`` 按 request_id 过滤（同通道其它消息被忽略）。
    """

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 6379,
        connect_timeout: float = 5.0,
        io_timeout: float = 10.0,
    ) -> None:
        self.host = host
        self.port = port
        self.connect_timeout = connect_timeout
        self.io_timeout = io_timeout
        self._sub_sock: socket.socket | None = None
        self._frames = RespFrames()

    def close(self) -> None:
        if self._sub_sock is not None:
            try:
                self._sub_sock.close()
            except OSError:
                pass
            self._sub_sock = None

    def publish(self, channel: str, payload: str) -> int:
        """PUBLISH 并返回订阅者数量；短连接，发完即关。"""
        with socket.create_connection(
            (self.host, self.port), timeout=self.connect_timeout
        ) as sock:
            sock.settimeout(self.io_timeout)
            sock.sendall(_encode_command("PUBLISH", channel, payload))
            parser = RespFrames()
            while True:
                data = sock.recv(4096)
                if not data:
                    raise OSError(
                        f"redis {self.host}:{self.port} PUBLISH 连接被对端关闭"
                    )
                parser.feed(data)
                reply = parser.next_frame()
                if reply is not None:
                    if not isinstance(reply, int):
                        raise OSError(f"redis PUBLISH 意外回复 {reply!r}")
                    return reply

    def subscribe(self, channel: str) -> None:
        if self._sub_sock is None:
            self._sub_sock = socket.create_connection(
                (self.host, self.port), timeout=self.connect_timeout
            )
            self._sub_sock.settimeout(self.io_timeout)
        self._sub_sock.sendall(_encode_command("SUBSCRIBE", channel))
        # 吞掉 subscribe ack（*3 [subscribe, channel, count]）
        self._next_frame()

    def _next_frame(self):
        while True:
            frame = self._frames.next_frame()
            if frame is not None:
                return frame
            data = self._sub_sock.recv(4096)
            if not data:
                raise OSError(f"redis {self.host}:{self.port} 订阅连接被对端关闭")
            self._frames.feed(data)

    def wait_message(
        self,
        channel: str,
        predicate,
        timeout: float = 15.0,
        expect: bool = True,
    ):
        """等第一个满足 predicate 的 channel 消息；返回解析后的 JSON dict。

        ``expect=False`` 时超时返回 None（用于「不应出现」的负向断言）；
        ``expect=True`` 时超时抛 AssertionError。
        """
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                if expect:
                    raise AssertionError(
                        f"期望 {timeout}s 内收到 channel={channel} 的匹配消息，实际超时"
                    )
                return None
            # recv 窗口不能超过剩余 deadline：负向断言（expect=False）的窗口
            # 可能短于 io_timeout，固定 10s 阻塞会直接抛 TimeoutError 而非按
            # 调用方 deadline 返回 None。
            self._sub_sock.settimeout(min(self.io_timeout, remaining))
            try:
                frame = self._next_frame()
            except TimeoutError:
                continue
            if isinstance(frame, list) and len(frame) >= 3 and frame[0] == b"message":
                got_channel = frame[1].decode("utf-8", "replace")
                if got_channel != channel:
                    continue
                try:
                    payload = json.loads(frame[2].decode("utf-8"))
                except (UnicodeDecodeError, json.JSONDecodeError):
                    continue
                if predicate(payload):
                    return payload
