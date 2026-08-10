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

from bot.scenarios._death_screen_helpers import (
    DEATH_SCREEN_STAGE_FORTUNE,
    assert_no_screen_events,
    kill_self,
    last_event_time,
    reincarnate,
    wait_death_screen,
)

DESCRIPTION = "Fortune 死亡屏 → combat_terminate 负向 noop → combat_reincarnate 复活收屏"
MODULES = ["combat", "network"]


def run(env) -> None:
    with env.new_bot("Rein") as bot:
        bot.expect_event("game_join", timeout=20.0)
        bot.expect_event("pos_look", timeout=20.0)

        # ── 死亡：首次死亡必 Fortune ────────────────────────────────
        kill_self(bot)
        screen = wait_death_screen(bot)
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
        hidden = reincarnate(bot, anchor)
        if hidden.get("visible") is not False:
            raise AssertionError(f"期望收屏 visible=false，实际 {hidden}")
        bot.assert_alive("combat_reincarnate 复活后连接保持")
