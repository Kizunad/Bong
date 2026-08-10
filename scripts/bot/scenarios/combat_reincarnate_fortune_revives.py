"""重生决策：Fortune 死亡屏 → combat_reincarnate → 复活收屏。

黑盒契约面（全走真实协议）：
- `/kill self`（标准 DeathEvent）→ 首次死亡必出 Fortune 决策（无近期死亡保底）：
  death_screen visible=true，stage=FORTUNE(1)，can_reincarnate=true、
  can_terminate=false（Fortune 决策不可主动终结，见 server/combat/components.rs）。
- 时序：决策死亡屏在死亡后 NEAR_DEATH_WINDOW_TICKS（30s 濒死宽限窗）走完才出现
  （`_death_screen_helpers.wait_death_screen` 的 timeout 已覆盖此窗）。
- Fortune 决策下 `combat_terminate` 必须被静默拒绝（决策窗口保持、无终结屏、
  无收屏）——负向门禁。
- `combat_reincarnate` → server 复活：death_screen visible=false 收屏，连接保持。
"""

from __future__ import annotations

import time

from bot.scenarios._death_screen_helpers import (
    DEATH_SCREEN_STAGE_FORTUNE,
    assert_no_screen_events,
    kill_self,
    last_event_time,
    reincarnate,
    wait_death_screen_event,
)

DESCRIPTION = "Fortune 死亡屏 → combat_terminate 负向 noop → combat_reincarnate 复活收屏"
MODULES = ["combat", "network"]


def run(env) -> None:
    with env.new_bot("Rein") as bot:
        bot.expect_event("game_join", timeout=20.0)
        bot.expect_event("pos_look", timeout=20.0)

        # ── 死亡：首次死亡必 Fortune，且死亡屏受濒死宽限窗延迟 ──────
        anchor = last_event_time(bot)
        kill_self(bot)
        # 濒死宽限窗（NEAR_DEATH_WINDOW_TICKS=600 combat ticks）的**权威**钉死，双管齐下
        # （review finding 1：原实现只做 3s 负窗 smoke，把"被延迟到窗末"的回归放过去）：
        # 1) 死亡屏距 kill 的 wall 间隙 ≥ 28s：600 ticks ÷ 20 TPS 上限 = 30s 下界（低
        #    TPS 只会更久），28s 地板留 2s 给 kill 聊天确认在负载下的投递延迟。把宽限窗
        #    缩到 80 ticks（~4s）或固定 4s 延迟下发的回归实现，此断言立即红。
        # 2) 决策窗口（REVIVAL_CONFIRM_WINDOW_TICKS=1200 ticks × MILLIS_PER_TICK=50）由
        #    协议暴露的 countdown_until_ms 钉死：server 在决策下发时把它算成
        #    current_unix_millis() + 60000（lifecycle.rs decision_deadline_ms）。收到后
        #    与本地 unix 时钟对表，余量应落在 [48000, 60500] ms（名义 60000；低边界给
        #    接收延迟留 12s，高边界只容同机时钟抖动）。缩短决策窗口的回归实现
        #    （如 200 ticks → +10000ms）立即红。
        kill_at = time.monotonic() - bot.t0
        # 负向观察：死亡屏必须在濒死宽限窗走完后才下发（kill 后 3s 内绝无屏）。
        # 宽限窗以 tick 计量，任何现实 TPS 下都 ≥ 数十秒 wall：设计 20 TPS≈30s，
        # 本盒低负载下实测更久。3s 负窗在正确实现下必空；kill 后立即下发死亡屏的
        # 回归实现会命中并红。
        assert_no_screen_events(
            bot,
            anchor,
            window_secs=3.0,
            label="濒死宽限窗内（kill 后 3s）death_screen 不应下发",
        )
        screen_t, screen = wait_death_screen_event(bot, after=anchor)
        if screen_t - kill_at < 28.0:
            raise AssertionError(
                "死亡屏应在濒死宽限窗（NEAR_DEATH_WINDOW_TICKS=600 combat ticks ≥ 30s "
                "@ ≤20 TPS）走完后才下发，实际 kill 后 "
                f"{screen_t - kill_at:.1f}s 就出现——宽限窗被缩短或改成了固定延迟"
            )
        if screen.get("stage") != DEATH_SCREEN_STAGE_FORTUNE:
            raise AssertionError(
                f"期望首次死亡决策 stage=FORTUNE({DEATH_SCREEN_STAGE_FORTUNE})，"
                f"实际 stage={screen.get('stage')}，payload={screen}"
            )
        if screen.get("can_terminate") is not False:
            raise AssertionError(
                f"期望 Fortune 决策 can_terminate=false（不可主动终结），"
                f"实际 can_terminate={screen.get('can_terminate')}，payload={screen}"
            )
        if screen.get("can_reincarnate") is not True:
            raise AssertionError(
                f"期望决策窗口开启 can_reincarnate=true，实际={screen.get('can_reincarnate')}"
            )
        countdown_until_ms = screen.get("countdown_until_ms")
        if countdown_until_ms is None:
            raise AssertionError(
                "death_screen payload 必须携带 countdown_until_ms（决策窗口剩余毫秒，"
                f"proto field 5），实际缺失，payload={screen!r}"
            )
        received_unix_ms = int(time.time() * 1000)
        remaining_ms = countdown_until_ms - received_unix_ms
        if not (48_000 <= remaining_ms <= 60_500):
            raise AssertionError(
                "决策窗口 countdown_until_ms 应钉在收到时刻 + REVIVAL_CONFIRM_WINDOW_TICKS"
                "(1200) × MILLIS_PER_TICK(50) = +60000ms，实际 countdown_until_ms="
                f"{countdown_until_ms}、收到时刻 unix_ms={received_unix_ms}、"
                f"余量 {remaining_ms}ms 不在 [48000, 60500]——决策窗口被缩短"
            )

        # ── 负向：Fortune 决策下 combat_terminate 必须 noop ──────────
        anchor = last_event_time(bot)
        bot.intent({"type": "combat_terminate", "v": 1})
        assert_no_screen_events(
            bot,
            anchor,
            window_secs=2.5,
            label="Fortune 决策下 combat_terminate",
        )

        # ── 正向：combat_reincarnate → 复活收屏 ─────────────────────
        # 收屏锚点在 reincarnate 内部紧贴 intent 现场取（缺省 after），不复用负向观察
        # 之前的陈旧锚点（review finding 2）。
        hidden = reincarnate(bot)
        if hidden.get("visible") is not False:
            raise AssertionError(f"期望收屏 visible=false，实际 {hidden}")
        bot.assert_alive("combat_reincarnate 复活后连接保持")
