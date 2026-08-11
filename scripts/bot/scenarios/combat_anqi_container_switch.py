"""暗器链：战斗容器切换（anqi_container_switch）的黑盒契约。

流程：显式切到 quiver（事件 from=hand_slot/to=quiver，暴露窗口 tick+10）→
同目标重复切换静默（不发事件）→ to=None 轮换（quiver→pocket_pouch）→
切到 fenglinghe 静默拒收（不允许战斗切换）→ 切回 hand_slot（
pocket_pouch→hand_slot）→ 同目标再次静默。

锁定的契约（server/src/combat/anqi_v2.rs switch_container_slot）：
- Fenglinghe 不允许 combat swap（allows_combat_swap=false）→ 返回 None，
  不发事件。
- 目标与当前 active 相同 → 返回当前槽位但不发事件。
- 实际切换 → switching_until_tick = tick + 10（CONTAINER_SWITCH_EXPOSURE_TICKS），
  发布 bong:anqi/container_swap（from/to/switching_until_tick/tick）。
- to=None → cycle_container_slot：next_combat_container 轮换
  HandSlot→Quiver→PocketPouch→HandSlot。
"""

import os
import re
import time
from typing import Optional

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import wait_join_and_inventory
from bot._redis_helpers import RedisPubSub
from bot._server_log import ServerLogScanner

DESCRIPTION = "暗器战斗容器切换：显式切换/轮换/拒收 fenglinghe/同目标静默"
MODULES = ["anqi", "combat"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

CH_SWAP = "bong:anqi/container_swap"
# fenglinghe 拒收护栏的正向证据：switch_container_slot 拒收早退
# （!allows_combat_swap，仅 fenglinghe）发 `container_switch guard
# carrier=player:{uuid} reason=rejected` info 日志（client_request_handler.rs）。
# 场景据此区分「拒收分支被走」与「请求在 wire 边界被丢」——单靠无 container_swap
# 事件无法证明请求到达了 switch 系统（review finding [major]）。
_GUARD_RE = re.compile(
    r"container_switch guard carrier=(?P<carrier>\S+) .*reason=(?P<reason>\S+)"
)
_GUARD_REASON = "rejected"
GUARD_TIMEOUT = 8.0


def _switch(bot, to) -> None:
    intent: dict = {"type": "anqi_container_switch", "v": 1}
    if to is not None:
        intent["to"] = to
    bot.intent(intent)


def _self_carrier(bot) -> str:
    """从 server 周期下发的 CarrierState 取本 bot 的线缆 wire id。

    bong:anqi/container_swap 是全局频道，共享服务器上所有玩家的切换都会
    发布到这里；只靠 from/to/tick 匹配会让别的玩家恰好做的同型切换满足
    断言。server 每 tick 周期向每个客户端推送自身 CarrierState（field 49，
    carrier=`player:{uuid}`），用它把事件归属钉到本 bot，才能证明事件由
    本 bot 的意图驱动。
    """
    evt = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data.get("payload_type") == "carrier_state"
        and e.data.get("payload", {}).get("carrier", "").startswith("player:"),
        timeout=15.0,
        description="server 下发本 bot 的 CarrierState（carrier=player: 线缆 id）",
    )
    return evt.data["payload"]["carrier"]


def _wait_swap(
    pubsub,
    carrier: str,
    from_kind: str,
    to_kind: str,
    after_tick: int,
    context: str,
    after: Optional[int] = None,
) -> dict:
    evt = pubsub.wait_event(
        CH_SWAP,
        lambda e: e.get("carrier") == carrier
        and e.get("from") == from_kind
        and e.get("to") == to_kind
        and int(e.get("tick", 0)) > after_tick,
        timeout=20.0,
        description=f"{context}（carrier={carrier}）：{from_kind} -> {to_kind}",
        after=after,
    )
    assert int(evt.get("switching_until_tick", 0)) == int(evt["tick"]) + 10, (
        f"{context}：暴露窗口应为 tick+10，实际 {evt!r}"
    )
    return evt


def _expect_silent(pubsub, carrier: str, anchor: int, window_s: float, context: str) -> None:
    # 只统计本 bot（carrier 归属）的事件：CH_SWAP 是全局频道，别的玩家切换也会
    # 发布到这里，全量计数会让外来切换把静默窗口误判为"本 bot 发事件"。
    # 窗口化扫描（review finding [minor]）：sleep 只是窗口截止判定，投递完成由
    # settle 的 PING/PONG 屏障证明；max_ts 取屏障返回时刻而非窗口截止——后者
    # 会把窗口内发布、迟到入队的事件排除出扫描，负向断言照样 miss（保留原缺陷）。
    time.sleep(window_s)
    pubsub.settle(grace=2.0)
    scan_ts = time.monotonic()
    ours = [
        e
        for e in pubsub.window_events(CH_SWAP, after=anchor, max_ts=scan_ts)
        if e.get("carrier") == carrier
    ]
    assert not ours, (
        f"{context}：本 bot（carrier={carrier}）在锚点后不应发布 container_swap "
        f"事件，实际 {len(ours)} 条，首条 {ours[0]!r}"
    )


def run(env) -> None:
    pubsub = RedisPubSub.from_env()
    try:
        pubsub.subscribe(CH_SWAP)
        with env.new_bot("Switch") as bot:
            wait_join_and_inventory(bot)
            bot.assert_alive("连接后")
            carrier = _self_carrier(bot)

            # 1) 显式切 quiver：hand_slot -> quiver。先锚定序列再发 intent——
            # 服务端响应即使抢先于 wait_event 被泵线程入队也不得落出等待窗口
            # （review finding [1]：_switch 后才进 wait_event 会漏掉已入队事件）。
            anchor = pubsub.anchor()
            _switch(bot, "quiver")
            first = _wait_swap(
                pubsub, carrier, "hand_slot", "quiver", 0, "首次切换", after=anchor
            )
            first_tick = int(first["tick"])

            # 2) 同目标重复：静默（不发事件）。先锚定再发 intent——窗口从本
            #    intent 起算，把首次切换的事件排除在外（review finding [minor]）。
            anchor = pubsub.anchor()
            _switch(bot, "quiver")
            _expect_silent(pubsub, carrier, anchor, 3.0, "同目标重复切换")

            # 3) to=None 轮换：quiver -> pocket_pouch
            anchor = pubsub.anchor()
            _switch(bot, None)
            _wait_swap(
                pubsub,
                carrier,
                "quiver",
                "pocket_pouch",
                first_tick,
                "轮换切换",
                after=anchor,
            )

            # 4) fenglinghe：拒收，静默。负向（无 container_swap 事件）+ 正向
            #    （switch 消费者系统的拒收 guard 标记）双证据。后者证明请求完成
            #    反序列化→派发→switch_container_slot 拒收分支——若 fenglinghe 不在
            #    client-request schema、被误编码、反序列化失败或在派发前被丢弃，
            #    同样的"无事件"结果会出现，负向断言无法区分（review finding
            #    [major]：fenglinghe 静默在请求未达 switch 系统时照样通过）。
            log_path = os.environ.get("BONG_SERVER_LOG")
            assert log_path and os.path.isfile(log_path), (
                "fenglinghe 拒收正向证据需要 server 日志：export "
                f"BONG_SERVER_LOG=<server log>（bot-e2e.sh 已导出；实际 "
                f"log_path={log_path!r}）"
            )
            scanner = ServerLogScanner(log_path, _GUARD_RE)
            scanner.scan()
            before_guard = scanner.guard_markers(carrier)
            before_failed = scanner.deserialize_failed(bot.username)
            anchor = pubsub.anchor()
            _switch(bot, "fenglinghe")
            guard_deadline = time.monotonic() + GUARD_TIMEOUT
            while True:
                scanner.scan()
                after_guard = scanner.guard_markers(carrier)
                if len(after_guard) > len(before_guard):
                    break
                if time.monotonic() >= guard_deadline:
                    break
                time.sleep(0.2)
            new_guard = after_guard[len(before_guard):]
            assert new_guard, (
                f"fenglinghe 拒收正向证据缺失：发出切换后 {GUARD_TIMEOUT:.0f}s 内 "
                f"server 未为 carrier={carrier!r} 新增 container_switch guard 标记"
                f"，实际新增 {new_guard!r}——请求可能在 schema/序列化/派发环节被丢"
                f"，负向静默断言无法区分"
            )
            for reason in new_guard:
                assert reason == _GUARD_REASON, (
                    f"fenglinghe 拒收 guard reason 应为 {_GUARD_REASON!r}，实际 "
                    f"{new_guard!r}"
                )
            # 辅助诊断：guard 标记只在反序列化成功后才会出现，此计数在 guard 断言
            # 通过后提供 schema 漂移的精确定位（若 schema 失配，guard 断言先行
            # 失败，这里不会掩盖）。
            scanner.scan()
            new_failed = scanner.deserialize_failed(bot.username) - before_failed
            assert new_failed == 0, (
                f"fenglinghe 切换窗口内出现归属本 bot 的 client_request "
                f"deserialize failed（新增 {new_failed} 条）——请求在反序列化处"
                f"断裂，未达 switch 系统"
            )
            _expect_silent(pubsub, carrier, anchor, 3.0, "fenglinghe 拒收")

            # 5) 切回 hand_slot：pocket_pouch -> hand_slot
            anchor = pubsub.anchor()
            _switch(bot, "hand_slot")
            _wait_swap(
                pubsub,
                carrier,
                "pocket_pouch",
                "hand_slot",
                first_tick,
                "切回手部",
                after=anchor,
            )

            # 6) 同目标 hand_slot：静默
            anchor = pubsub.anchor()
            _switch(bot, "hand_slot")
            _expect_silent(pubsub, carrier, anchor, 3.0, "同目标 hand_slot")

            bot.assert_alive("容器切换全链后")
    finally:
        pubsub.stop()
