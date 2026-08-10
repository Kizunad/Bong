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

# RESP bulk string 长度上限：订阅消息的 JSON 载荷通常只有几 KB，超限即视为恶意
# 或异常发布者。resp2 解码器若信任对端自报的长度，会为任意大 bulk 无限累积
# `_buf` 直到 `length + 2` 字节到齐，单个大载荷就能耗尽 bot runner 内存
# （review finding [major]：unbounded RESP buffering）。
MAX_BULK_LEN = 1 << 20  # 1 MiB
# 接收缓冲区总量上限：整帧到齐前所有分片都会留在 _buf；即使单 bulk 未超
# MAX_BULK_LEN，海量小帧同样能撑爆进程，这里做第二道兜底。
MAX_BUF_LEN = 16 << 20  # 16 MiB
# 保留事件列表的累积载荷字节预算：max_events 只按条数裁剪，而单事件可接近
# MAX_BULK_LEN（1 MiB），5000 条 × 近 1 MiB = 数 GiB 保留会耗尽 bot runner
# 内存（review finding [major]：event-count cap 不约束字节）。字节预算与条数
# 上限双保险，超预算裁剪最旧条目。
MAX_EVENT_BYTES = 8 << 20  # 8 MiB
# RESP 数组结构性上限：订阅消息帧是长度 3 的浅数组，正常载荷远达不到这些界。
# 对端自报的 count/嵌套不可信——深嵌套（*1 链）会触发 Python 递归爆栈、海量
# 小元素会在 _buf 封顶内膨胀出大得多的 Python list、不完整数组每次 recv 还会
# 重解析全部前驱元素（二次方 CPU）。深度/元素总数硬上限把这三条路全部封死
# （review finding [major]：unbounded RESP array complexity）。
MAX_RESP_DEPTH = 8
MAX_RESP_ARRAY_ITEMS = 4096


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
        # bytearray：feed 原地追加，不复制整段缓冲。不可变 bytes 每次 `+=`
        # 都会重建整个累计缓冲——分片到达时呈二次方拷贝（4096 字节一帧、16MiB
        # 上限内约 4096 次逐步变大的复制 ≈ 几十 GiB 内存搬运），恶意对端在
        # 缓冲封顶前就能把 bot runner 拖垮（review finding [minor]：分片 RESP
        # 在尺寸上限拒绝前先制造二次方缓冲拷贝）。
        self._buf = bytearray()
        # 当前 next_frame 单帧解析已解码的数组元素总数；每次取帧重置。只在
        # 泵线程/握手循环串行调用 next_frame，无并发（见 _parse_frame 的元素
        # 计数预算，review finding [major]：unbounded RESP array complexity）。
        self._parse_items = 0

    def feed(self, data: bytes) -> None:
        if not data:
            return
        if len(self._buf) + len(data) > MAX_BUF_LEN:
            raise ValueError(
                f"RESP 接收缓冲区超限: {len(self._buf) + len(data)} bytes > "
                f"MAX_BUF_LEN={MAX_BUF_LEN}"
            )
        # bytearray 的 `+=` 走 __iadd__ 原地 extend，摊销 O(1)；bytes 会整段重建。
        self._buf += data

    def next_frame(self) -> Any:
        self._parse_items = 0
        frame, consumed = self._parse_frame(0)
        if consumed == 0:
            return None
        self._buf = self._buf[consumed:]
        return frame

    def _line(self, pos: int) -> tuple[Optional[bytes], int]:
        idx = self._buf.find(b"\r\n", pos)
        if idx == -1:
            return None, 0
        # 返回 bytes（调用方 decode/int/比较都按 bytes 契约）；bytearray 切片
        # 会返回 bytearray，与 bytes 不相等。
        return bytes(self._buf[pos:idx]), idx

    def _parse_frame(self, pos: int, depth: int = 0) -> tuple[Any, int]:
        if pos >= len(self._buf):
            return None, 0
        marker = bytes(self._buf[pos : pos + 1])
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
            if length < -1:
                # RESP2 只允许 -1 表示 null bulk，-2 及更小的负长度是畸形帧，
                # 不能当空串/空数组放行——否则结束偏移倒退、后续流错位
                # （review finding [minor]：negative RESP lengths decoded）。
                raise ValueError(f"RESP bulk string 负长度越界: {length}")
            if length == -1:
                return None, idx + 2
            if length > MAX_BULK_LEN:
                raise ValueError(
                    f"RESP bulk string 超限: {length} bytes > MAX_BULK_LEN={MAX_BULK_LEN}"
                )
            end = idx + 2 + length + 2
            if len(self._buf) < end:
                return None, 0
            if bytes(self._buf[idx + 2 + length : end]) != b"\r\n":
                # bulk 载荷末尾必须跟 CRLF；只按声明长度跳两字节会吞掉畸形
                # 流并从错误边界继续解析（review finding [minor]：CRLF 未校验）。
                raise ValueError(
                    f"RESP bulk string 缺少 CRLF 终止符: {bytes(self._buf[idx + 2 : end])!r}"
                )
            return bytes(self._buf[idx + 2 : idx + 2 + length]), end
        if marker == b"*":
            line, idx = self._line(pos + 1)
            if line is None:
                return None, 0
            count = int(line)
            if count < -1:
                raise ValueError(f"RESP array 负计数越界: {count}")
            if count == -1:
                return None, idx + 2
            if depth + 1 > MAX_RESP_DEPTH:
                raise ValueError(
                    f"RESP array 嵌套过深: depth={depth + 1} > MAX_RESP_DEPTH={MAX_RESP_DEPTH}"
                )
            items: list[Any] = []
            cursor = idx + 2
            for _ in range(count):
                self._parse_items += 1
                if self._parse_items > MAX_RESP_ARRAY_ITEMS:
                    raise ValueError(
                        f"RESP array 元素总数超限: > MAX_RESP_ARRAY_ITEMS={MAX_RESP_ARRAY_ITEMS}"
                    )
                item, consumed = self._parse_frame(cursor, depth + 1)
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
        # 只报 scheme，绝不回填完整 URL：REDIS_URL 可能带 userinfo 凭据，
        # 把原始 url 内插进异常会让密码出现在 CI/运维日志（review finding）。
        raise ValueError(f"unsupported redis url scheme: {parts.scheme!r}")
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
        max_event_bytes: int = MAX_EVENT_BYTES,
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
        # (seq, channel, payload, enqueued_at, wire_bytes)：seq 单调递增，供
        # wait_event 只扫本次等待期间新增的事件；enqueued_at（time.monotonic）
        # 让 wait_event 能区分「截止前到达」与「截止后入队」（review finding
        # [minor]）；wire_bytes 是原始 bulk 长度，配合 _max_event_bytes 按累积
        # 载荷字节裁剪最旧条目——max_events 只按条数兜底，单事件可接近 1 MiB，
        # 数千条 × 1 MiB 的保留会耗尽进程内存（review finding [major]）。
        self._events: list[tuple[int, str, dict[str, Any], float, int]] = []
        self._event_seq = 0
        self._max_events = max_events
        self._max_event_bytes = max_event_bytes
        self._event_bytes = 0
        self._lock = threading.Lock()
        # _frames（_buf/_parse_items）由 pump 线程与 settle() 的投递屏障共同
        # 消费：next_frame/feed/_drain_frames 必须互斥，否则两线程并发切片 _buf
        # 会破坏帧流（review finding [major]：最终扫描缺投递屏障时用 settle 补）。
        self._frame_lock = threading.Lock()
        self._closed = False
        self._fatal_error: Optional[BaseException] = None
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
                data = self._sock.recv(4096)
            except socket.timeout:
                continue
            finally:
                self._sock.settimeout(5.0)
            if not data:
                # 空读 = 对端永久关闭；继续循环只会立刻再拿到 EOF 空转烧 CPU
                # 直到超时（review finding [minor]：握手 EOF 空转）。
                raise RuntimeError("redis 连接被对端关闭（AUTH 阶段 EOF）")
            self._frames.feed(data)
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
                    data = self._sock.recv(4096)
                except socket.timeout:
                    continue
                finally:
                    self._sock.settimeout(5.0)
                if not data:
                    raise RuntimeError("redis 连接被对端关闭（SUBSCRIBE 阶段 EOF）")
                self._frames.feed(data)
                continue
            if isinstance(frame, list) and len(frame) == 3:
                if frame[0] == b"subscribe":
                    remaining.discard(frame[1].decode("utf-8", "replace"))
                elif frame[0] == b"message":
                    # 多频道依次 SUBSCRIBE 时，发布者可在首个 ack 之后、末个 ack
                    # 之前发布消息。ack 循环若只认 subscribe 帧、把穿插的 message
                    # 帧当噪声丢弃，wait_event 会在这条真实消息上超时（review
                    # finding [minor]）。此时泵线程尚未启动，直接在调用线程消费
                    # 入队，subscribe 返回后事件已可见。
                    self._handle_frame(frame)
        if remaining:
            raise RuntimeError(f"redis SUBSCRIBE 未确认: {sorted(remaining)}")
        # ack 循环里临时装的 0.5s 超时只用于确认读取；确认完毕必须恢复阻塞，
        # 否则 5.0s 空闲超时会触发 _pump 的 socket.timeout 路径。
        self._sock.settimeout(None)
        self._thread = threading.Thread(target=self._pump, daemon=True)
        self._thread.start()

    def _set_fatal(self, exc: BaseException) -> None:
        with self._lock:
            self._fatal_error = exc
        try:
            self._sock.close()
        except OSError:
            pass

    def _pump(self) -> None:
        while not self._closed:
            try:
                with self._frame_lock:
                    self._drain_frames()
                data = self._sock.recv(4096)
            except ValueError as exc:
                # 恶意/异常发布者的超大或畸形 RESP 帧：内存上限已封顶，但连接
                # 已经不可信，终止泵线程并记录致命错误，由 wait_event/events_for
                # 上报给场景（review finding [major]：unbounded RESP buffering）。
                self._set_fatal(exc)
                break
            except socket.timeout:
                # 空闲超时不是连接终止：继续等待下一条订阅消息。
                continue
            except OSError as exc:
                if not self._closed:
                    self._set_fatal(
                        RuntimeError(f"redis 连接读失败（socket 错误）: {exc!r}")
                    )
                break
            except Exception as exc:
                # 泵线程兜底失败边界：_drain_frames/_handle_frame 内的任何意外
                # 异常（如超深 JSON 载荷让 json.loads 抛 RecursionError，不在
                # _handle_frame 的 ValueError/TypeError 捕获之列）若逃逸，守护
                # 线程会静默退出且 _fatal_error 永不置位——负向断言在死订阅上
                # 照常通过、wait_event 报误导性超时（review finding [major]：
                # 未捕获的载荷异常静默终止观察线程）。BaseException
                # （KeyboardInterrupt/SystemExit）故意不吞。
                self._set_fatal(exc)
                break
            if not data:
                # 空读 = 对端关闭订阅连接。若不标记致命，events_for/wait_event 会
                # 继续返回冻结事件列表，负向断言（无事件）在死订阅上照常通过——
                # 必须让「已无订阅者在观察」升级为失败（review finding [major]：
                # 断连静默满足负向断言）。
                if not self._closed:
                    self._set_fatal(RuntimeError("redis 连接被对端关闭（EOF）"))
                break
            try:
                with self._frame_lock:
                    self._frames.feed(data)
            except ValueError as exc:
                self._set_fatal(exc)
                break
            except Exception as exc:
                # 与主 try 的兜底同语义：feed 解析异常也不得静默终止泵线程。
                self._set_fatal(exc)
                break

    def _drain_frames(self) -> None:
        """把已缓冲的 RESP 帧全部消费掉，再阻塞等下一次 recv。

        SUBSCRIBE 的 ack 循环可能在一次 recv(4096) 里同时读到「最终订阅确认 +
        首条已发布消息」，但它只解析确认就退出（remaining 空）。若 _pump 先阻塞
        recv 再解析缓冲，这条已入缓冲的消息要等下一次网络流量才会被处理——唯一
        一次发布时 wait_event 会一直超时（review finding：ack+message 合包搁置）。
        所以每轮循环先 drain 缓冲里的帧，再收新数据。
        """
        while True:
            frame = self._frames.next_frame()
            if frame is None:
                return
            self._handle_frame(frame)

    def _handle_frame(self, frame: Any) -> None:
        if not isinstance(frame, list) or len(frame) != 3 or frame[0] != b"message":
            return
        channel = frame[1].decode("utf-8", "replace")
        payload = frame[2]
        try:
            decoded = json.loads(payload)
        except (ValueError, TypeError):
            return
        # 订阅的频道是全局共享的，任何发布者都可能投递合法 JSON 的非对象值
        # （`null` / `[]` / `"x"` / 数字）。事件契约要求对象：非 dict 一律忽略，
        # 否则 wait_event 谓词与 events_for 推导里的 `e.get(...)` 会对标量/列表
        # 抛 AttributeError，让场景在共享频道收到垃圾时崩溃而非跳过
        # （review finding [minor]：valid non-object JSON crashes predicates）。
        if not isinstance(decoded, dict):
            return
        wire_bytes = len(payload)
        with self._lock:
            # 记录入队时刻（time.monotonic，与 wait_event 的 deadline 同钟）：
            # wait_event 的最终扫描按 ts <= deadline 排除截止后入队的事件。
            self._events.append(
                (self._event_seq, channel, decoded, time.monotonic(), wire_bytes)
            )
            self._event_seq += 1
            self._event_bytes += wire_bytes
            # 条数 + 累积字节双上限裁剪最旧条目：max_events 只按条数兜底，单事件
            # 可接近 MAX_BULK_LEN（1 MiB），数千条 × 1 MiB = 数 GiB 保留会耗尽
            # 进程内存（review finding [major]）。单事件本身超过字节预算时会被
            # 立即弹出（既最新又最旧），保内存优先于保事件。
            while (
                len(self._events) > self._max_events
                or self._event_bytes > self._max_event_bytes
            ):
                if not self._events:
                    self._event_bytes = 0
                    break
                _, _, _, _, b = self._events.pop(0)
                self._event_bytes -= b

    def events_for(self, channel: str) -> list[dict[str, Any]]:
        with self._lock:
            if self._fatal_error is not None:
                raise AssertionError(
                    f"redis 连接已因协议异常终止，事件流不可信: {self._fatal_error}"
                )
            return [e for _, c, e, _, _ in self._events if c == channel]

    def window_events(
        self,
        channel: str,
        after: Optional[int] = None,
        max_ts: Optional[float] = None,
    ) -> list[dict[str, Any]]:
        """加锁扫描 [after, +∞) 且（可选）入队时刻 <= max_ts 的事件快照。

        after=None 时从当前最新事件序号起扫（仅本次调用后新增的事件）；传
        anchor() 返回值可把窗口起点前移到触发动作之前。max_ts 把「截止后入队」
        的事件排除出窗口——序列号边界无法区分「截止前到达」与「截止后入队」，
        必须靠入队时间戳（review finding [minor]）。wait_event 的窗口扫描与
        场景负向断言的最终窗口化扫描共用本方法（review finding [major]：最终
        despawn 检查缺投递屏障/窗口化扫描）。
        """
        with self._lock:
            if self._fatal_error is not None:
                raise AssertionError(
                    f"redis 连接已因协议异常终止: {self._fatal_error}"
                )
            start = self._event_seq if after is None else after
            return [
                e
                for seq, c, e, ts, _ in self._events
                if seq >= start
                and c == channel
                and (max_ts is None or ts <= max_ts)
            ]

    def settle(self, grace: float = 0.5) -> None:
        """投递屏障：观察窗截止后，等待 pump 把已发布事件全部消费入队。

        pump 线程的 recv/入队与调用方（场景）的截止判定异步：server 在截止前
        发布的 despawn 可能仍滞留在 socket 接收缓冲或 _frames 缓冲里，截止判定
        通过后 pump 才把它入队——若在截止后直接做最终扫描，负向断言会 miss 窗口
        内事件（review finding [major]）。宽限期内反复消费已缓冲帧并让 pump 完成
        收尾 recv；屏障返回后调用方做最终窗口化扫描（window_events + max_ts），
        截止前已发布的事件已全部可见。本方法不直接读 socket——与 pump 并发 recv
        同一流会把先后到达的块错序拼进 _buf（帧损坏），故只靠 pump 收尾 + 自身
        加锁 drain。
        """
        end = time.monotonic() + grace
        while time.monotonic() < end:
            with self._frame_lock:
                self._drain_frames()
            time.sleep(0.05)
        with self._frame_lock:
            self._drain_frames()

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
            if self._fatal_error is not None:
                raise AssertionError(
                    f"redis 连接已因协议异常终止: {self._fatal_error}"
                )
            start_seq = self._event_seq if after is None else after
        while True:
            # 两次扫描都按 max_ts=deadline 过滤：序列号边界无法区分「截止前
            # 到达」与「截止后入队」，必须用入队时间戳把后者排除（review
            # finding [minor]：wait_event 接受截止后事件）。
            fresh = self.window_events(channel, after=start_seq, max_ts=deadline)
            for evt in fresh:
                if predicate(evt):
                    return evt
            if time.monotonic() >= deadline:
                # 快照与截止判定之间可能插入 pump 的入队：匹配事件在快照之后、
                # 截止之前到达时，上面的循环已不再扫它。抛超时前必须做一次加锁
                # 的最终扫描，把窗口内最后入队的事件捞回（review finding
                # [minor]：wait_event 在匹配事件已到达的情况下仍可能超时）。
                fresh = self.window_events(channel, after=start_seq, max_ts=deadline)
                for evt in fresh:
                    if predicate(evt):
                        return evt
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
