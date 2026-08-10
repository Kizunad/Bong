"""死亡屏（AwaitingRevival 决策窗口）流程的协议级观察助手。

只消费 `bong:server_data` 的 death_screen（field 72）/ terminate_screen（field 73）
解码 payload，不读 server 内部状态。死亡/复活/终结/新建角色全走真实协议：
`/kill self` 触发标准 DeathEvent，`combat_*` intent 走 `bong:client_request`。
"""

from __future__ import annotations

import time

from bot.bot import Bot, BotAssertionError

DEATH_SCREEN_STAGE_FORTUNE = 1
DEATH_SCREEN_STAGE_TRIBULATION = 2

MAX_ESCALATION_DEATHS = 6


def last_event_time(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def kill_self(bot: Bot, timeout: float = 15.0) -> None:
    bot.cmd("kill self")
    bot.expect_chat("[dev] kill self", timeout=timeout)


def wait_death_screen_event(
    bot: Bot, after: float = 0.0, timeout: float = 240.0
) -> tuple[float, dict]:
    """等待 visible=true 的死亡屏，返回 ``(event.t, payload)``。

    ``event.t`` 与场景时钟（``time.monotonic() - bot.t0``）同一刻度，供调用方把屏事件
    钉到具体时刻（如"距 kill 超过濒死宽限窗"的时序断言，见
    ``combat_reincarnate_fortune_revives``）。

    timeout 必须覆盖生产侧的濒死宽限窗：death_arbiter 只把角色推入 NearDeath，
    决策（并附死亡屏）要等 near_death_deadline_tick（NEAR_DEATH_WINDOW_TICKS=600 ticks）
    走完才由 near_death_tick 给出——过早断言会稳定超时（实测 15s 全灭）。宽限窗以
    combat tick 计量，wall 时长随服务端实际 TPS 伸缩：20 TPS 下约 30s，而本盒在
    高负载（NPC sim 超预算）下实测坍缩到 3.4-5.5 TPS（600 ticks ≈ 110-176s，观测于
    L10/L12 两次运行）——因此默认 240s（覆盖 2.5 TPS 留余量），健康路径在决策到达
    即提前返回，不受上限拖慢。
    """
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "death_screen"
        and e.data["payload"].get("visible") is True,
        timeout,
        "death_screen(visible=true) payload（濒死决策已出、决策窗口开启）",
    )
    return event.t, event.data["payload"]


def wait_death_screen(bot: Bot, after: float = 0.0, timeout: float = 240.0) -> dict:
    """等待 visible=true 的死亡屏 payload（濒死决策已出，AwaitingRevival 决策窗口开启）。"""
    return wait_death_screen_event(bot, after, timeout)[1]


def wait_death_screen_hidden(bot: Bot, after: float, timeout: float = 15.0) -> dict:
    """等待 death_screen visible=false（复活/终结/新建后的收屏）。"""
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "death_screen"
        and e.data["payload"].get("visible") is False,
        timeout,
        "death_screen(visible=false) payload（收屏）",
    )
    return event.data["payload"]


def wait_terminate_screen(
    bot: Bot, *, visible: bool, after: float = 0.0, timeout: float = 15.0
) -> dict:
    """等待终结屏，按 visible 区分显屏/收屏。"""
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "terminate_screen"
        and e.data["payload"].get("visible") is visible,
        timeout,
        f"terminate_screen(visible={visible}) payload",
    )
    return event.data["payload"]


def reincarnate(bot: Bot, after: float | None = None, timeout: float = 15.0) -> dict:
    """发送 combat_reincarnate 并等待死亡屏收屏（Fortune 决策必复活）。

    收屏锚点在**发送 intent 的同一时刻**（``last_event_time`` 紧贴 ``bot.intent``，
    均在负向观察之后）取得：调用方若传更早的锚点（如负向门禁观察**之前**捕获的），
    负向窗口内到达的屏事件会被算进收屏等待、提前放行或漏掉断言（review finding 2）。
    缺省 ``after`` 时现场取新锚点，杜绝陈旧锚点脚枪；显式 ``after`` 仍接受（escalate
    循环用它逐轮推进，调用方自己负责在紧贴动作处取锚）。
    """
    if after is None:
        after = last_event_time(bot)
    bot.intent({"type": "combat_reincarnate", "v": 1})
    return wait_death_screen_hidden(bot, after, timeout)


def assert_no_screen_events(
    bot: Bot, after: float, window_secs: float, label: str
) -> None:
    """断言窗口期内没有新死亡屏/终结屏事件（负向门禁：intent 应为 noop）。

    观测必须是原子的：``bot.events`` 由接收线程异步 append，两次独立加锁读取会得到
    两个先后快照，夹缝中新到达的屏事件会被漏过负向断言（review finding 2 原实现
    ``death_screens_after`` / ``terminate_screens_after`` 各读一份快照再并集）。这里
    只在一次 ``bot._lock`` 内复制事件集合，再从同一份拷贝同时分类两种屏 payload，
    覆盖完整窗口。
    """
    time.sleep(window_secs)
    with bot._lock:
        events = list(bot.events)
    new_screens = [
        e.data["payload"]
        for e in events
        if e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] in ("death_screen", "terminate_screen")
    ]
    if new_screens:
        raise BotAssertionError(
            f"{label} 应无任何屏事件（noop），实际收到 {len(new_screens)} 条：{new_screens}"
        )


def escalate_to_tribulation_death(bot: Bot) -> dict:
    """反复 kill→combat_reincarnate，直到某次死亡的决策 can_terminate=true。

    死亡次数驱动决策：前几次死亡必为 Fortune（can_terminate=false），death_count
    越过保底线后进入 Tribulation（can_terminate=true）。循环-直到-观察到目标决策，
    对运势/业力初值不敏感，因此是确定性的。

    关键：`wait_for` 扫历史事件不消费，`wait_death_screen` 必须以 `after` 锚定，否则会
    命中更早的旧决策屏（visible=true）提前放行——下一轮 combat_reincarnate 在 Alive 态
    被服务端静默拒绝，收屏永远不来（实测三连超时）。初始锚点用进入时的最新事件时间：
    调用方可能已有前置死亡（如负向门禁），锚到 0.0 会重匹配那屏。
    """
    after = last_event_time(bot)
    for cycle in range(1, MAX_ESCALATION_DEATHS + 1):
        kill_self(bot)
        screen = wait_death_screen(bot, after=after)
        if screen.get("can_terminate"):
            return screen
        after = last_event_time(bot)
        reincarnate(bot, after)
    raise BotAssertionError(
        f"循环 {MAX_ESCALATION_DEATHS} 次 kill→reincarnate 仍未观察到 can_terminate=true "
        "的死亡屏——决策阶梯没有按死亡次数推进，死亡屏链路可能断了"
    )
