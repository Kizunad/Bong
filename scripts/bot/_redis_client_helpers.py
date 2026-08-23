"""Bot 场景用的最小 Redis RESP2 客户端（纯 stdlib，不依赖第三方 redis 库）。

仓库惯例是手写协议客户端（mc_protocol.py / proto_min.py 同款思路）：场景只需要
PUBLISH（注入 bong:agent_ui_cmd 等）与 SUBSCRIBE（观察 bong:agent_ui_response 等）
两个命令，不值得引入 redis-py。

与 scripts/bot/_redis_helpers.py（anqi 订阅者，后台线程泵帧）刻意分文件：
本模块是同步请求/响应客户端（调用线程内阻塞 recv），类名 RedisClient
与对方的 RedisPubSub 在文件与符号两级都不同名，两个分支可无冲突共存。

设计：
- ``RespFrames`` —— 纯编解码器（feed bytes → 拉帧），与 socket 解耦，可单测。
- ``RedisClient`` —— PUBLISH 走短连接（发完即关），SUBSCRIBE 走常驻连接，
  只有 ``message`` 帧会暴露给调用方，subscribe ack / pong 等被内部吞掉。

只实现 RESP2 子集：+simple / -error / :integer / $bulk（含 $-1 nil）/ *array。
"""

from __future__ import annotations

import json
import os
import socket
import time
from urllib.parse import urlparse


class _IncompleteFrame:
    """数据不足哨兵：``next_frame`` 以它表示「还需更多字节」。

    RESP nil（``$-1``）解析值为 ``None``；若数据不足也返回 ``None``，调用方就
    无法区分「收全了一条 nil 帧」与「字节还没喂够」，会误再等一次 socket read。
    """

    __slots__ = ()

    def __repr__(self) -> str:
        return "<incomplete RESP frame>"


_INCOMPLETE_FRAME = _IncompleteFrame()


class RespFrames:
    """RESP2 帧解析器：``feed()`` 喂字节，``next_frame()`` 取帧；数据不足返回 ``_INCOMPLETE_FRAME``。

    防滥用：缓冲用 ``bytearray.extend``（摊还 O(1)/块，避免 ``bytes += `` 每次把整个
    已积压缓冲再拷一遍的二次拷贝），并设协议级上限——单条 bulk 帧声明长度超
    ``max_bulk_size``、或缓冲累计超 ``max_buffer_size`` 直接抛 ValueError。RESP2 的
    bulk/array 长度只允许 ``-1``（nil）或非负值，长度 < -1 的非法负数同样拒绝。
    客户端跑在 agent_ui 命令/响应通道上，payload 是外部发布的 Redis 消息，不能让它用
    一条超大或永不收尾的帧把进程内存撑爆。
    """

    def __init__(
        self,
        max_bulk_size: int = 16 * 1024 * 1024,
        max_buffer_size: int = 32 * 1024 * 1024,
    ) -> None:
        self._buf = bytearray()
        self._max_bulk_size = max_bulk_size
        self._max_buffer_size = max_buffer_size

    def feed(self, data: bytes) -> None:
        if not data:
            return
        if len(self._buf) + len(data) > self._max_buffer_size:
            raise ValueError(
                f"RESP 缓冲累计超过上限 {self._max_buffer_size} bytes"
            )
        self._buf.extend(data)

    def next_frame(self):
        frame, consumed = self._parse_frame(0)
        if consumed == 0:
            return _INCOMPLETE_FRAME
        del self._buf[:consumed]
        return frame

    def _line(self, pos: int) -> tuple[bytearray | None, int]:
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
            if length < -1:
                raise ValueError(
                    f"RESP bulk 帧声明长度 {length} 非法（RESP2 仅 -1 表示 nil，其余必须 ≥ 0）"
                )
            if length > self._max_bulk_size:
                raise ValueError(
                    f"RESP bulk 帧声明长度 {length} 超过上限 {self._max_bulk_size}"
                )
            body_start = idx + 2
            end = body_start + length
            if len(self._buf) < end + 2:
                return None, 0
            if self._buf[end : end + 2] != b"\r\n":
                raise ValueError(
                    f"RESP bulk 帧 body 后缺 \\r\\n 分隔符，实际 {self._buf[end:end+2]!r}"
                )
            return bytes(self._buf[body_start:end]), end + 2
        if marker == b"*":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            count = int(line)
            if count == -1:
                return None, idx + 2
            if count < -1:
                raise ValueError(
                    f"RESP array 帧声明长度 {count} 非法（RESP2 仅 -1 表示 nil，其余必须 ≥ 0）"
                )
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


class RedisClient:
    """最小 Redis 客户端：PUBLISH（短连接）+ SUBSCRIBE（常驻连接）+ 消息等待。

    场景用法：先 ``subscribe("bong:agent_ui_response")`` 再触发 server 行为，
    ``wait_message`` 按 request_id 过滤（同通道其它消息被忽略）。
    """

    def __init__(
        self,
        host: str | None = None,
        port: int | None = None,
        connect_timeout: float = 5.0,
        io_timeout: float = 10.0,
    ) -> None:
        # bot-e2e.sh 在 ambient fixture 模式下把 server 起在私有 Redis 上（随机端口）
        # 并 export REDIS_URL；场景必须跟着这个端点走，硬编码 6379 会把 cmd 发到
        # 无人订阅的共享 Redis（本地非 ambient 模式 REDIS_URL 未设，回落默认值）。
        if host is None or port is None:
            parsed = urlparse(
                os.environ.get("REDIS_URL", "redis://127.0.0.1:6379")
            )
            if host is None:
                host = parsed.hostname or "127.0.0.1"
            if port is None:
                port = parsed.port or 6379
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
                if reply is not _INCOMPLETE_FRAME:
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

    def _next_frame(self, deadline: float | None = None):
        """取下一帧；``deadline``（``time.monotonic()`` 绝对时刻）给定时，跨多次
        recv 的帧重组也受该绝对时限约束：每次 recv 前按剩余时间缩 socket 超时，
        剩余耗尽即抛 TimeoutError——绝不因分片持续到达把单帧重组拖过调用方 deadline
        （socket 超时在这里永远触发不了，靠外层循环 deadline 检查兜底没用）。无
        deadline（subscribe ack 等非等待路径）保持 io_timeout 行为。
        """
        while True:
            frame = self._frames.next_frame()
            if frame is not _INCOMPLETE_FRAME:
                return frame
            if deadline is not None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("RESP 帧重组超过调用方 deadline，未收完整帧")
                self._sub_sock.settimeout(min(self.io_timeout, remaining))
            else:
                self._sub_sock.settimeout(self.io_timeout)
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
            # deadline 是端到端绝对时限：不仅 socket 超时受剩余时间约束，帧重组期间
            # 多次成功的 recv（部分帧分片持续到达、socket 超时永不触发）也逐次按剩余
            # 时间缩超时、耗尽即抛 TimeoutError。负向断言（expect=False）的窗口可能
            # 短于 io_timeout，固定 10s 阻塞会直接抛 TimeoutError 而非按调用方
            # deadline 返回 None。
            try:
                frame = self._next_frame(deadline)
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
                if not isinstance(payload, dict):
                    # 对象形状守卫：bong:agent_ui_response 契约是 JSON 对象。合法的
                    # 非对象 JSON（null/数组/字符串/数字）是无关发布者消息，跳过
                    # （与频道不符、无效 JSON 同款处理）；直接放行会让 predicate
                    # 调 .get() 抛 AttributeError 崩掉整个场景。
                    continue
                if predicate(payload):
                    return payload
