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


def death_screens_after(bot: Bot, after: float) -> list[dict]:
    with bot._lock:
        events = list(bot.events)
    return [
        e.data["payload"]
        for e in events
        if e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "death_screen"
    ]


def terminate_screens_after(bot: Bot, after: float) -> list[dict]:
    with bot._lock:
        events = list(bot.events)
    return [
        e.data["payload"]
        for e in events
        if e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "terminate_screen"
    ]


def wait_death_screen(bot: Bot, after: float = 0.0, timeout: float = 45.0) -> dict:
    """等待 visible=true 的死亡屏（濒死决策已出，AwaitingRevival 决策窗口开启）。

    timeout 必须覆盖生产侧的濒死宽限窗：death_arbiter 只把角色推入 NearDeath，
    决策（并附死亡屏）要等 near_death_deadline_tick（NEAR_DEATH_WINDOW_TICKS=30s）
    走完才由 near_death_tick 给出——过早断言会稳定超时（实测 15s 全灭）。
    """
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > after
        and e.data["payload_type"] == "death_screen"
        and e.data["payload"].get("visible") is True,
        timeout,
        "death_screen(visible=true) payload（濒死决策已出、决策窗口开启）",
    )
    return event.data["payload"]


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


def reincarnate(bot: Bot, after: float, timeout: float = 15.0) -> dict:
    """发送 combat_reincarnate 并等待死亡屏收屏（Fortune 决策必复活）。"""
    bot.intent({"type": "combat_reincarnate", "v": 1})
    return wait_death_screen_hidden(bot, after, timeout)


def assert_no_screen_events(
    bot: Bot, after: float, window_secs: float, label: str
) -> None:
    """断言窗口期内没有新死亡屏/终结屏事件（负向门禁：intent 应为 noop）。"""
    time.sleep(window_secs)
    new_screens = death_screens_after(bot, after) + terminate_screens_after(bot, after)
    if new_screens:
        raise BotAssertionError(
            f"{label} 应无任何屏事件（noop），实际收到 {len(new_screens)} 条：{new_screens}"
        )


def escalate_to_tribulation_death(bot: Bot) -> dict:
    """反复 kill→combat_reincarnate，直到某次死亡的决策 can_terminate=true。

    死亡次数驱动决策：前几次死亡必为 Fortune（can_terminate=false），death_count
    越过保底线后进入 Tribulation（can_terminate=true）。循环-直到-观察到目标决策，
    对运势/业力初值不敏感，因此是确定性的。
    """
    for cycle in range(1, MAX_ESCALATION_DEATHS + 1):
        kill_self(bot)
        screen = wait_death_screen(bot)
        if screen.get("can_terminate"):
            return screen
        anchor = last_event_time(bot)
        reincarnate(bot, anchor)
    raise BotAssertionError(
        f"循环 {MAX_ESCALATION_DEATHS} 次 kill→reincarnate 仍未观察到 can_terminate=true "
        "的死亡屏——决策阶梯没有按死亡次数推进，死亡屏链路可能断了"
    )
