"""重生决策：Fortune 死亡屏 → combat_reincarnate → 复活收屏。

黑盒契约面（全走真实协议）：
- `/kill self`（标准 DeathEvent）→ 首次死亡必出 Fortune 决策（无近期死亡保底）：
  death_screen visible=true，stage=FORTUNE(1)，can_reincarnate=true、
  can_terminate=false（Fortune 决策不可主动终结，见 server/combat/components.rs）。
- 时序：死亡后立即进入复活裁决，没有濒死等待阶段。
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

        anchor = last_event_time(bot)
        kill_at = time.monotonic() - bot.t0
        kill_self(bot)
        screen_t, screen = wait_death_screen_event(bot, after=anchor)
        if not 0.0 <= screen_t - kill_at < 15.0:
            raise AssertionError(
                f"死亡后应立即下发复活裁决，实际等待 {screen_t - kill_at:.1f}s"
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
