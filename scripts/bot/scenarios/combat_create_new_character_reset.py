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
    DEATH_SCREEN_STAGE_FORTUNE,
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
        # 收屏锚点紧贴 intent 现场取（reincarnate 缺省 after），不复用负向观察之前的
        # 陈旧锚点（review finding 2）。
        reincarnate(bot)
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

        # ── 重置后验：死亡推进必须随新角色归零 ─────────────────────────
        # 收屏 + 连接保持不足以证明"字符状态已重置"——只隐藏终结屏、保留死亡推进的
        # 实现也能过那两条断言。可观察差异在死亡决策阶梯：reset_for_new_character
        # 清零 death_count（combat::lifecycle::reset_for_new_character →
        # cultivation::luck_pool::reset_for_new_life）。若未重置，Terminated 前的
        # 最后一次死亡已是 Tribulation（can_terminate=true），再次死亡会直接回到
        # Tribulation；归零后首次死亡必出 Fortune（stage=1, can_terminate=false）。
        # 若角色仍停在 Terminated，kill self 不会触发任何死亡决策，wait 超时即红。
        reset_anchor = last_event_time(bot)
        kill_self(bot)
        reset_screen = wait_death_screen(bot, after=reset_anchor)
        if reset_screen.get("stage") != DEATH_SCREEN_STAGE_FORTUNE:
            raise AssertionError(
                f"新建角色后死亡应回 Fortune 决策（死亡推进已归零），"
                f"实际 stage={reset_screen.get('stage')}，payload={reset_screen}"
            )
        if reset_screen.get("can_terminate") is not False:
            raise AssertionError(
                f"新建角色后 Fortune 决策 can_terminate 应为 false，"
                f"实际 {reset_screen.get('can_terminate')}，payload={reset_screen}"
            )
        bot.assert_alive("combat_create_new_character 重置后连接保持")
