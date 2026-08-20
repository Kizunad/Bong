"""GAP19 经脉锻造入口 ForgeRequest —— 黑盒 e2e（server/src/cultivation/forging.rs）。

覆盖面：
- dev 铺垫：`/meridian open lung`（只开肺经，留给 heart 当 MeridianClosed 负向）+ `/qi max 100`
  + `/qi set 100`（新鲜角色默认 qi_current=0 / qi_max=10，必须显式设满才能锻造）。
- client_request：`forge_request {v, meridian, axis}`。wire 形状钉在
  server/src/schema/client_request.rs `forge_request_roundtrip`：
  meridian=snake_case "lung"、axis=PascalCase "Rate"（别发 "rate"，会被 serde 拒收）。
- 观察面：Redis `bong:forge_event`（CH_FORGE_EVENT）的 `ForgeEventV1` JSON
  `{meridian, axis, from_tier, to_tier, success}`（schema/cultivation.rs:177）。
  ForgeEventV1 不推送 bot 的 Minecraft CustomPayload（server 只转 server_data/vfx 等
  少数通道），必须直接订阅 Redis；bot-e2e.sh 自起模式导出 REDIS_URL（私有 Compose
  Redis 动态端口），本场景从 env 解析。
- 断言：正向字段 + 状态读回（第二次 forge 的 from_tier==1 → tier 恰好 +1）+ 成本
  （`/qi set 0` 回读 96.0 / 64.0 → tier1 cost=4 / tier3 cost=36，tier_cost 二次曲线）
  + 负向四分支（UnknownChannel 窗口内无事件 / MeridianClosed / NotEnoughQi /
  AtMaxTier 各 success:false）+ 双发无跳档（连锻到 cap 后仍 AtMaxTier，无超 cap 溢出）。

> ⚠ 成本曲线以代码为准：`tier_cost(n) = 4.0 * n²`（forging.rs:59）→ 实际消耗
> **4 / 16 / 36**（0→1 / 1→2 / 2→3）。函数体上方注释写的 "1:4, 2:9, 3:16" 与公式不符
> （对应 (n+1)²，从未被公式实现过；服务端测试也只断言单调不减）。黑盒按可执行行为
> 钉 4/16/36，注释与公式的出入留给 server 侧单独核实。

> ⚠ 场景契约：dev 铺垫与 tier 读回假设**新鲜玩家**（qi=0/10、经脉全关、tier0）。
> 服务端 bong.db 持久化在 worktree，固定 run tag 的重跑会复用旧玩家——`meridian open`
> 对已开经脉回显的是「already open」（meridian.rs dev-cmd Open 分支），forge 也会直接
> 撞 AtMaxTier。bot-e2e.sh 自起模式的 run tag 是 `$$ % 100000`（每次调用唯一），
> fixture 单跑必须每次换新 tag。

负向分支的 payload 不携带 error kind（success:false 事件只有 meridian/axis/0/0），按请求
顺序逐条消费；UnknownChannel 例外——`ForgeEventV1::validate` 拒绝 snake_case 非 humanoid
meridian，事件根本不发布（redis_bridge.rs:804），只能断言「窗口内无事件」。

说明：
- 刻意不新建 `_forge_helpers.py`：GAP18（t00178）在飞行中，共享锻造 setup 模块路径
  留给 GAP17/18 独占，本场景内联 RESP2 订阅者，避免 `_redis_helpers.py` 式 add/add。
- bot tag `FoRq` 已加进 scripts/bot-e2e.sh 的 BOT_E2E_OPERATOR_TAGS（dev 命令门）。
"""

from __future__ import annotations

import json
import os
import socket
import time
import urllib.parse

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = "经脉锻造 forge_request：ForgeEventV1 字段 + tier 读回 + 成本 + 四负向分支"
MODULES = ["cmd", "cultivation", "network"]

_INCOMPLETE = object()


class _ForgeEventSubscriber:
    """最小 RESP2 SUBSCRIBE 客户端，订阅 `bong:forge_event` 观察 ForgeEventV1。

    只实现 SUBSCRIBE + message 帧（server 只 PUBLISH，场景无需 PUBLISH）。
    与 gap3 分支 `_redis_client_helpers.py::RedisClient` 刻意同思路不同文件：
    本场景自包含，不依赖未合入分支的模块。
    """

    def __init__(self) -> None:
        parsed = urllib.parse.urlparse(
            os.environ.get("REDIS_URL", "redis://127.0.0.1:6379")
        )
        self.host = parsed.hostname or "127.0.0.1"
        self.port = parsed.port or 6379
        self._sock: socket.socket | None = None
        self._buf = bytearray()

    def subscribe(self, channel: str) -> None:
        self._sock = socket.create_connection((self.host, self.port), timeout=5.0)
        self._sock.settimeout(10.0)
        self._send_cmd("SUBSCRIBE", channel)
        self._read_frame()  # subscribe ack（*3 [subscribe, channel, count]）

    def close(self) -> None:
        if self._sock is not None:
            try:
                self._sock.close()
            except OSError:
                pass
            self._sock = None

    def _send_cmd(self, *parts: str) -> None:
        out = f"*{len(parts)}\r\n".encode("ascii")
        for part in parts:
            raw = part.encode("utf-8")
            out += f"${len(raw)}\r\n".encode("ascii") + raw + b"\r\n"
        self._sock.sendall(out)

    def _next_frame(self):
        while True:
            frame, consumed = self._parse_frame(0)
            if consumed == 0:
                return _INCOMPLETE
            del self._buf[:consumed]
            return frame

    def _parse_frame(self, pos: int):
        """RESP2 帧解析：返回 (value, consumed)；数据不足返回 (None, 0)。"""
        if pos >= len(self._buf):
            return None, 0
        marker = self._buf[pos : pos + 1]
        if marker == b"+":
            idx = self._buf.find(b"\r\n", pos + 1)
            if idx == -1:
                return None, 0
            return self._buf[pos + 1 : idx].decode("utf-8", "replace"), idx + 2
        if marker == b"$":
            idx = self._buf.find(b"\r\n", pos + 1)
            if idx == -1:
                return None, 0
            length = int(self._buf[pos + 1 : idx])
            if length == -1:
                return None, idx + 2
            start = idx + 2
            end = start + length
            if len(self._buf) < end + 2:
                return None, 0
            return bytes(self._buf[start:end]), end + 2
        if marker == b":":
            # RESP2 integer（SUBSCRIBE ack 的订阅计数 `:1` 就是这种帧）
            idx = self._buf.find(b"\r\n", pos + 1)
            if idx == -1:
                return None, 0
            return int(self._buf[pos + 1 : idx]), idx + 2
        if marker == b"-":
            # RESP2 error（订阅态下误发非订阅命令会得到 -ERR；解析为字符串不崩溃）
            idx = self._buf.find(b"\r\n", pos + 1)
            if idx == -1:
                return None, 0
            return self._buf[pos + 1 : idx].decode("utf-8", "replace"), idx + 2
        if marker == b"*":
            idx = self._buf.find(b"\r\n", pos + 1)
            if idx == -1:
                return None, 0
            count = int(self._buf[pos + 1 : idx])
            items: list = []
            offset = idx + 2
            for _ in range(count):
                item, used = self._parse_frame(offset)
                if used == 0:
                    return None, 0
                items.append(item)
                offset = used
            return items, offset
        raise ValueError(f"unknown RESP marker {marker!r}")

    def _recv_until_frame(self, deadline: float):
        while True:
            frame = self._next_frame()
            if frame is not _INCOMPLETE:
                return frame
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("RESP 帧重组超过调用方 deadline")
            try:
                self._sock.settimeout(min(10.0, remaining))
                data = self._sock.recv(4096)
            except socket.timeout:
                raise TimeoutError("订阅 socket 空闲超时（无新帧）") from None
            if not data:
                raise OSError(
                    f"redis {self.host}:{self.port} 订阅连接被对端关闭"
                )
            self._buf.extend(data)

    def _read_frame(self):
        return self._recv_until_frame(time.monotonic() + 10.0)

    def wait_forge_event(self, predicate, timeout: float, expect: bool = True):
        """等第一个满足 predicate 的 `bong:forge_event` JSON 消息，返回 dict。

        expect=False 时超时返回 None（负向断言「窗口内无事件」）。
        """
        deadline = time.monotonic() + timeout
        while True:
            try:
                frame = self._recv_until_frame(deadline)
            except TimeoutError:
                if expect:
                    raise BotAssertionError(
                        f"期望 {timeout}s 内收到匹配的 bong:forge_event，实际超时未出现"
                    )
                return None
            if not (isinstance(frame, list) and len(frame) >= 3 and frame[0] == b"message"):
                continue
            try:
                payload = json.loads(frame[2].decode("utf-8"))
            except (UnicodeDecodeError, ValueError):
                continue
            if not isinstance(payload, dict):
                continue
            if predicate(payload):
                return payload


def _forge(bot, subscriber, meridian: str, axis: str = "Rate") -> dict:
    """发一发 forge_request 并等对应的 ForgeEventV1，返回事件 dict。

    事件按请求顺序逐条消费（同一 bot 的 channel 无并发），predicate 只按
    meridian+axis 匹配，不预先假定成功/失败——成功与失败形态由调用方断言。
    """
    bot.intent({"type": "forge_request", "v": 1, "meridian": meridian, "axis": axis})
    return subscriber.wait_forge_event(
        # 事件 meridian 是 humanoid PascalCase（"Lung"/"Heart"），请求是 snake_case
        # （"lung"/"heart"），casefold 归一后匹配。
        lambda p: p.get("meridian", "").casefold() == meridian.casefold()
        and p.get("axis") == axis,
        timeout=12.0,
    )


def _expect_no_forge_event(subscriber, window: float = 8.0) -> None:
    """负向断言：window 内 bong:forge_event 无任何新事件。"""
    got = subscriber.wait_forge_event(lambda p: True, timeout=window, expect=False)
    if got is not None:
        raise BotAssertionError(
            f"期望 {window}s 窗口内无 forge 事件，实际收到 {got!r}"
        )


def _assert_success_event(ev: dict, *, meridian: str, axis: str, from_tier: int, to_tier: int) -> None:
    if ev.get("success") is not True:
        raise BotAssertionError(
            f"期望 forge 成功事件 success=true，实际 {ev!r}"
        )
    if ev.get("meridian") != meridian or ev.get("axis") != axis:
        raise BotAssertionError(
            f"期望 {meridian}/{axis}，实际 {ev!r}"
        )
    if ev.get("from_tier") != from_tier or ev.get("to_tier") != to_tier:
        raise BotAssertionError(
            f"期望 tier {from_tier}→{to_tier}，实际 {ev!r}"
        )


def _assert_failure_event(ev: dict, *, meridian: str) -> None:
    if ev.get("success") is not False:
        raise BotAssertionError(f"期望 forge 拒绝事件 success=false，实际 {ev!r}")
    if ev.get("meridian") != meridian:
        raise BotAssertionError(f"期望拒绝事件 meridian={meridian}，实际 {ev!r}")
    if ev.get("from_tier") != 0 or ev.get("to_tier") != 0:
        raise BotAssertionError(
            f"拒绝事件 from/to_tier 应为 0/0，实际 {ev!r}"
        )


def _expect_chat_after(bot, substring: str, after: float, timeout: float = 10.0) -> None:
    """只接受发送锚点之后的回显，避免重复历史消息满足本次命令。"""
    bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > after
        and substring in event.data["text"],
        timeout,
        f"发送锚点 t>{after:.3f}s 后包含「{substring}」的聊天消息",
    )


def run(env) -> None:
    subscriber = _ForgeEventSubscriber()
    with env.new_bot("FoRq") as bot:
        try:
            subscriber.subscribe("bong:forge_event")
        except OSError as error:
            raise BotAssertionError(f"无法订阅 bong:forge_event：{error}") from error

        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        # dev 铺垫：只开肺经（heart 保持关闭 → 负向 MeridianClosed），qi 设到已知值。
        bot.cmd("meridian open lung")
        bot.expect_chat("[dev] opened meridian", timeout=10.0)
        bot.cmd("qi max 100")
        bot.expect_chat("[dev] qi max", timeout=10.0)
        bot.cmd("qi set 100")
        # echo 是 "[dev] qi set 0.0 -> 100.0"（before=0），"qi set 100.0" 不是它的子串。
        bot.expect_chat("[dev] qi set 0.0 -> 100.0", timeout=10.0)

        # 正向：forge lung.Rate 0→1，命名字段全断言。
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_success_event(ev, meridian="Lung", axis="Rate", from_tier=0, to_tier=1)

        # 成本读回：tier1 cost = tier_cost(1) = 4，qi 100→96（/qi set 0 的 before 回读）。
        bot.cmd("qi set 0")
        bot.expect_chat("[dev] qi set 96.0 -> 0.0", timeout=10.0)
        refill_anchor = last_event_time(bot)
        bot.cmd("qi set 100")
        # 该回显与 setup 完全相同，必须锚定本次命令；否则历史消息可能让 forge
        # 在补气尚未生效时发送，形成间歇性的 NotEnoughQi 假失败。
        _expect_chat_after(bot, "[dev] qi set 0.0 -> 100.0", refill_anchor)

        # 状态读回：第二次 forge from_tier==1 → tier 前进恰好 1，无跳档。
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_success_event(ev, meridian="Lung", axis="Rate", from_tier=1, to_tier=2)

        # 成本读回：tier2 cost = tier_cost(2) = 16，qi 100→84。
        # 读回必须紧随 forge：服务端 qi 缓慢回复（实测 ≈0.04/s），隔了负向分支的
        # ~8s 后 before 会漂到 84.3，精确钉 cost 的读回就废了。
        bot.cmd("qi set 3")
        bot.expect_chat("[dev] qi set 84.0 -> 3.0", timeout=10.0)
        bot.cmd("qi set 100")
        # before=3.0 → echo "[dev] qi set 3.0 -> 100.0"，全史唯一，真等待。
        bot.expect_chat("[dev] qi set 3.0 -> 100.0", timeout=10.0)

        # 负向 MeridianClosed：heart 从未 open → success:false（meridian 仍映射 Heart）。
        ev = _forge(bot, subscriber, "heart", "Rate")
        _assert_failure_event(ev, meridian="Heart")

        # 负向 NotEnoughQi：qi=3 < tier3 cost(36) → success:false。
        # 回显 "100.0 -> 3.0" 顺带钉 refill 顶满（回复封顶 qi_max=100，heart 拒锻不耗 qi）。
        bot.cmd("qi set 3")
        bot.expect_chat("[dev] qi set 100.0 -> 3.0", timeout=10.0)
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_failure_event(ev, meridian="Lung")
        bot.cmd("qi set 100")
        # 与上一条 refill 同串，wait_for 历史匹配即过——refill 是否生效由紧随的
        # 冲 cap forge 兜底（qi 不足会以 NotEnoughQi success:false 暴露）。
        bot.expect_chat("[dev] qi set 3.0 -> 100.0", timeout=10.0)

        # 负向 UnknownChannel：未知 channel → validate 拒绝，窗口内无事件。
        # 放在最后：8s 窗口内 qi 停在 100（回复封顶 qi_max），不影响后续断言。
        bot.intent(
            {"type": "forge_request", "v": 1, "meridian": "nonexistent_channel", "axis": "Rate"}
        )
        _expect_no_forge_event(subscriber)

        # 冲 cap：2→3（cost 36，qi 100→64），再读回 64.0 证明 tier3 cost。
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_success_event(ev, meridian="Lung", axis="Rate", from_tier=2, to_tier=3)
        bot.cmd("qi set 0")
        bot.expect_chat("[dev] qi set 64.0 -> 0.0", timeout=10.0)

        # 负向 AtMaxTier：P1 上限 tier3，连发两次均 success:false → 无超 cap 溢出（双发检查）。
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_failure_event(ev, meridian="Lung")
        ev = _forge(bot, subscriber, "lung", "Rate")
        _assert_failure_event(ev, meridian="Lung")

        bot.assert_alive("锻造入口全链路 + 负向四分支后")
