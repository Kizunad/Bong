"""Bot 场景用的最小 Redis RESP2 pubsub 客户端（纯 stdlib）。

仓库惯例：场景协议客户端手写（见 mc_protocol.py / proto_min.py）。暗器链
（anqi）的 service 事件走 Redis outbound 发布（bong:combat/carrier_charged、
bong:combat/projectile_despawned、bong:anqi/container_swap），bot 场景经
SUBSCRIBE 观察——与 GAP2 的 duo_she redis 证据同构。

设计：
- RespFrames —— 纯编解码器（feed 字节 → 逐帧），与 socket 解耦，可单测。
- RedisPubSub —— SUBSCRIBE 常驻连接 + 后台线程泵帧；只暴露 message 帧的
  JSON 解码结果，subscribe ack / pong 内部吞掉。stop() 后线程退出。

场景侧用法：
    pubsub = RedisPubSub.from_env()
    try:
        pubsub.subscribe("bong:combat/carrier_charged")
        ...  # 发 intent
        evt = pubsub.wait_event("bong:combat/carrier_charged", lambda e: e.get("full_charge") is False, timeout=20.0)
    finally:
        pubsub.stop()
"""

from __future__ import annotations

import json
import os
import socket
import threading
import time
from typing import Any, Callable, Optional
from urllib.parse import unquote, urlsplit

DEFAULT_REDIS_URL = "redis://127.0.0.1:6379"


def _frame_command(args: list[str]) -> bytes:
    """把命令参数编码为 RESP2 数组 + 批量字符串（redis 客户端标准参数框架）。

    每个参数都以字节长度前缀包裹，空格 / CRLF / 引号 / 二进制字节都不参与
    inline 解析，天然免疫命令注入与参数分裂——绝不手拼 `AUTH user pass`
    这样的命令文本（review finding [major]: AUTH 参数框架）。
    """
    out = bytearray(f"*{len(args)}\r\n".encode())
    for arg in args:
        raw = arg.encode("utf-8")
        out += b"$" + str(len(raw)).encode() + b"\r\n" + raw + b"\r\n"
    return bytes(out)


class RespFrames:
    """RESP2 帧解析器：feed() 喂字节，next_frame() 取一帧；数据不足返回 None。"""

    def __init__(self) -> None:
        self._buf = b""

    def feed(self, data: bytes) -> None:
        if data:
            self._buf += data

    def next_frame(self) -> Any:
        frame, consumed = self._parse_frame(0)
        if consumed == 0:
            return None
        self._buf = self._buf[consumed:]
        return frame

    def _line(self, pos: int) -> tuple[Optional[bytes], int]:
        idx = self._buf.find(b"\r\n", pos)
        if idx == -1:
            return None, 0
        return self._buf[pos:idx], idx

    def _parse_frame(self, pos: int) -> tuple[Any, int]:
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
            end = idx + 2 + length + 2
            if len(self._buf) < end:
                return None, 0
            return self._buf[idx + 2 : idx + 2 + length], end
        if marker == b"*":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            count = int(line)
            if count == -1:
                return None, idx + 2
            items: list[Any] = []
            cursor = idx + 2
            for _ in range(count):
                item, consumed = self._parse_frame(cursor)
                if consumed == 0:
                    return None, 0
                items.append(item)
                cursor = consumed
            return items, cursor
        raise ValueError(f"unsupported RESP2 frame marker {marker!r}")


def _parse_redis_url(url: str) -> tuple[str, int, Optional[str], Optional[str]]:
    """解析 redis:// URL 为 (host, port, username, password)。

    authority 边界（/ ? # 取最早分隔符）、bracketed IPv6 主机、userinfo 分离
    全部交给 urlsplit 的 RFC 3986 语义；userinfo 的百分号编码再由 unquote 解码。
    SUBSCRIBE 不需要选库，db 路径 / query / fragment 被 urlsplit 天然剥离，
    不再影响 host/port（review finding [major]: URL 解析）。
    """
    if not url:
        return "127.0.0.1", 6379, None, None
    parts = urlsplit(url)
    if parts.scheme != "redis":
        raise ValueError(f"unsupported redis url: {url!r}")
    username = unquote(parts.username) if parts.username is not None else None
    password = unquote(parts.password) if parts.password is not None else None
    return parts.hostname or "127.0.0.1", parts.port or 6379, username, password


class RedisPubSub:
    """SUBSCRIBE 常驻连接 + 后台线程泵帧，收 message 帧的 JSON 内容。"""

    def __init__(
        self,
        host: str = "127.0.0.1",
        port: int = 6379,
        timeout: float = 5.0,
        max_events: int = 5000,
        username: Optional[str] = None,
        password: Optional[str] = None,
    ):
        self._sock = socket.create_connection((host, port), timeout=timeout)
        self._frames = RespFrames()
        if password is not None:
            try:
                self._auth(username, password)
            except BaseException:
                try:
                    self._sock.close()
                except OSError:
                    pass
                raise
        # (seq, channel, payload)：seq 单调递增，供 wait_event 只扫本次等待期间
        # 新增的事件；max_events 兜底裁剪最旧条目，防止共享频道累积撑爆内存。
        self._events: list[tuple[int, str, dict[str, Any]]] = []
        self._event_seq = 0
        self._max_events = max_events
        self._lock = threading.Lock()
        self._closed = False
        self._thread: Optional[threading.Thread] = None

    @classmethod
    def from_env(cls) -> "RedisPubSub":
        host, port, username, password = _parse_redis_url(
            os.environ.get("REDIS_URL") or DEFAULT_REDIS_URL
        )
        return cls(host, port, username=username, password=password)

    def _auth(self, username: Optional[str], password: str) -> None:
        """连接后先 AUTH 再 SUBSCRIBE，凭据不得静默丢弃（review finding [3]）。

        Redis 6+ 支持 ``AUTH <user> <pass>``；无用户名退化为 ``AUTH <pass>``
        （默认用户）。参数走 _frame_command 的 RESP 批量字符串框架发送，密码含
        空格 / CRLF 也不会被拆成多参数或注入新命令（review finding [major]）。
        +OK 即通过；-NOAUTH/-WRONGPASS 等错误帧立即抛错，让配错凭据在连接
        阶段暴露，而不是场景跑起来才莫名失败。
        """
        args = ["AUTH"] + ([username] if username else []) + [password]
        self._sock.sendall(_frame_command(args))
        deadline = time.monotonic() + 5.0
        while time.monotonic() < deadline:
            frame = self._frames.next_frame()
            if frame is not None:
                if frame == "OK":
                    return
                raise RuntimeError(f"redis AUTH 失败: {frame!r}")
            self._sock.settimeout(0.5)
            try:
                self._frames.feed(self._sock.recv(4096))
            except socket.timeout:
                continue
            finally:
                self._sock.settimeout(5.0)
        raise RuntimeError("redis AUTH 超时")

    def subscribe(self, *channels: str) -> None:
        for channel in channels:
            self._sock.sendall(b"SUBSCRIBE " + channel.encode() + b"\r\n")
        deadline = time.monotonic() + 5.0
        remaining = set(channels)
        while remaining and time.monotonic() < deadline:
            frame = self._frames.next_frame()
            if frame is None:
                self._sock.settimeout(0.5)
                try:
                    self._frames.feed(self._sock.recv(4096))
                except socket.timeout:
                    continue
                finally:
                    self._sock.settimeout(5.0)
                continue
            if isinstance(frame, list) and len(frame) == 3 and frame[0] == b"subscribe":
                remaining.discard(frame[1].decode("utf-8", "replace"))
        if remaining:
            raise RuntimeError(f"redis SUBSCRIBE 未确认: {sorted(remaining)}")
        # ack 循环里临时装的 0.5s 超时只用于确认读取；确认完毕必须恢复阻塞，
        # 否则 5.0s 空闲超时会触发 _pump 的 socket.timeout 路径。
        self._sock.settimeout(None)
        self._thread = threading.Thread(target=self._pump, daemon=True)
        self._thread.start()

    def _pump(self) -> None:
        while not self._closed:
            try:
                data = self._sock.recv(4096)
            except socket.timeout:
                # 空闲超时不是连接终止：继续等待下一条订阅消息。
                continue
            except OSError:
                break
            if not data:
                break
            self._frames.feed(data)
            while True:
                frame = self._frames.next_frame()
                if frame is None:
                    break
                if not isinstance(frame, list) or len(frame) != 3 or frame[0] != b"message":
                    continue
                channel = frame[1].decode("utf-8", "replace")
                payload = frame[2]
                try:
                    decoded = json.loads(payload)
                except (ValueError, TypeError):
                    continue
                with self._lock:
                    self._events.append((self._event_seq, channel, decoded))
                    self._event_seq += 1
                    if len(self._events) > self._max_events:
                        del self._events[: len(self._events) - self._max_events]

    def events_for(self, channel: str) -> list[dict[str, Any]]:
        with self._lock:
            return [e for _, c, e in self._events if c == channel]

    def anchor(self) -> int:
        """捕获当前事件序列锚点，供「先锚定、再触发、后等待」使用。

        配合 wait_event(after=anchor)：调用方在发送触发 intent **之前**调用本
        方法，服务端响应即使先于 wait_event 被泵线程入队，序列号也 >= 锚点，
        不会被排除（review finding [1]/[4] 的竞态窗口）。锚点与等待之间入队的
        事件会被窗口包含——场景谓词需按 carrier/实体等过滤出本 bot 的响应。
        """
        with self._lock:
            return self._event_seq

    def wait_event(
        self,
        channel: str,
        predicate: Callable[[dict[str, Any]], bool],
        timeout: float = 20.0,
        description: str = "redis 事件",
        after: Optional[int] = None,
    ) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        # 记录等待起点的 event_seq：只扫本等待期间新增的事件，不重复扫全频道历史
        # （全局频道的历史在长跑场景里会无限累积，反复全扫越来越贵）。after= 允许
        # 调用方把窗口锚定在发送触发 intent 之前（见 anchor()），服务端响应即使
        # 抢先于 wait_event 被泵线程入队也不会落出窗口（review finding [1]/[4]）。
        with self._lock:
            start_seq = self._event_seq if after is None else after
        while True:
            with self._lock:
                fresh = [
                    e for seq, c, e in self._events if seq >= start_seq and c == channel
                ]
            for evt in fresh:
                if predicate(evt):
                    return evt
            if time.monotonic() >= deadline:
                raise AssertionError(
                    f"等待 {description} 超时（{timeout:.0f}s）: channel={channel}, "
                    f"已收 {len(self.events_for(channel))} 条"
                )
            time.sleep(0.2)

    def stop(self) -> None:
        if self._closed:
            return
        self._closed = True
        try:
            self._sock.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        try:
            self._sock.close()
        except OSError:
            pass
        if self._thread is not None:
            self._thread.join(timeout=2.0)
