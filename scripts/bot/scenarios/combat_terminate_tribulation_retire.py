"""主动归隐：Tribulation 决策 → combat_terminate → 终结屏 + 死亡屏收屏。

黑盒契约面：
- 死亡次数推进决策阶梯：Fortune 决策（can_terminate=false）逐次 kill→reincarnate
  推进 death_count，越过保底线后出 Tribulation 决策（can_terminate=true）——
  循环-直到-观察到，不依赖运势/业力初值。
- Tribulation 决策下 `combat_terminate` → terminate_lifecycle("voluntary_retire")：
  terminate_screen visible=true（final_words/epilogue 非空）+ death_screen
  visible=false 收屏，连接保持（Terminated 后仍可继续操作）。
"""

from __future__ import annotations

from bot.scenarios._death_screen_helpers import (
    DEATH_SCREEN_STAGE_TRIBULATION,
    escalate_to_tribulation_death,
    last_event_time,
    wait_death_screen_hidden,
    wait_terminate_screen,
)

DESCRIPTION = "Tribulation 决策 → combat_terminate → 终结屏显屏 + 死亡屏收屏"
MODULES = ["combat", "network"]


def run(env) -> None:
    with env.new_bot("Term") as bot:
        bot.expect_event("game_join", timeout=20.0)
        bot.expect_event("pos_look", timeout=20.0)

        tribulation = escalate_to_tribulation_death(bot)
        if tribulation.get("stage") != DEATH_SCREEN_STAGE_TRIBULATION:
            raise AssertionError(
                f"期望 Tribulation 决策 stage=TRIBULATION({DEATH_SCREEN_STAGE_TRIBULATION})，"
                f"实际 stage={tribulation.get('stage')}，payload={tribulation}"
            )

        # ── 主动归隐：combat_terminate → 终结屏 + 收屏 ────────────────
        anchor = last_event_time(bot)
        bot.intent({"type": "combat_terminate", "v": 1})
        terminal = wait_terminate_screen(bot, visible=True, after=anchor)
        if not terminal.get("final_words") or not terminal.get("epilogue"):
            raise AssertionError(
                f"终结屏应带 final_words/epilogue，实际 payload={terminal}"
            )
        hidden = wait_death_screen_hidden(bot, anchor)
        if hidden.get("visible") is not False:
            raise AssertionError(f"期望死亡屏收屏 visible=false，实际 {hidden}")
        bot.assert_alive("combat_terminate 终结后连接保持")
