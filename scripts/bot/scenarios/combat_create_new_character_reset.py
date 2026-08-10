"""新建角色：AwaitingRevival 负向 noop → Terminated 后 combat_create_new_character 重置。

黑盒契约面：
- 决策窗口（AwaitingRevival）中 `combat_create_new_character` 必须被静默拒绝
  （状态门禁：CreateNewCharacter 仅接受 Terminated）——负向门禁。
- 主动归隐（combat_terminate）到达 Terminated：terminate_screen visible=true。
- Terminated 后 `combat_create_new_character` → reset_for_new_character：
  terminate_screen visible=false 收屏，角色重置，连接保持。
"""

from __future__ import annotations

from bot.scenarios._death_screen_helpers import (
    assert_no_screen_events,
    escalate_to_tribulation_death,
    kill_self,
    last_event_time,
    reincarnate,
    wait_death_screen,
    wait_death_screen_hidden,
    wait_terminate_screen,
)

DESCRIPTION = "AwaitingRevival 下 combat_create_new_character noop → Terminated 后新建角色收屏"
MODULES = ["combat", "network"]


def run(env) -> None:
    with env.new_bot("NewCh") as bot:
        bot.expect_event("game_join", timeout=20.0)
        bot.expect_event("pos_look", timeout=20.0)

        # ── 负向：决策窗口内 combat_create_new_character 必须 noop ────
        kill_self(bot)
        wait_death_screen(bot)
        anchor = last_event_time(bot)
        bot.intent({"type": "combat_create_new_character", "v": 1})
        assert_no_screen_events(
            bot,
            anchor,
            window_secs=2.5,
            label="AwaitingRevival 决策窗口内 combat_create_new_character",
        )

        # ── 复活并推进决策阶梯到 Tribulation ──────────────────────────
        reincarnate(bot, anchor)
        tribulation = escalate_to_tribulation_death(bot)
        if tribulation.get("can_terminate") is not True:
            raise AssertionError(
                f"期望 escalate 到 can_terminate=true 的 Tribulation 决策，"
                f"实际 payload={tribulation}"
            )

        # ── 主动归隐到达 Terminated ───────────────────────────────────
        anchor = last_event_time(bot)
        bot.intent({"type": "combat_terminate", "v": 1})
        wait_terminate_screen(bot, visible=True, after=anchor)
        wait_death_screen_hidden(bot, anchor)

        # ── Terminated 后新建角色：terminate 屏收屏 ────────────────────
        anchor = last_event_time(bot)
        bot.intent({"type": "combat_create_new_character", "v": 1})
        hidden = wait_terminate_screen(bot, visible=False, after=anchor)
        if hidden.get("visible") is not False:
            raise AssertionError(f"期望终结屏收屏 visible=false，实际 {hidden}")
        bot.assert_alive("combat_create_new_character 重置后连接保持")
