"""战斗技能链路 —— dev 铺垫 → skill_bar_bind/cast intent → 凝针专属反馈。"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import (
    last_event_time,
    payload_text,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_payload_after,
    wait_for_ready,
    wait_for_server_data_after,
)

DESCRIPTION = "/technique give 后用 skill_bar_bind/cast 施放 dugu.shoot_needle，并断言专属 VFX/战斗反馈"
MODULES = ["combat", "skill", "network", "cmd"]

SKILL_ID = "dugu.shoot_needle"
SLOT = 0


def run(env) -> None:
    with env.new_bot("Cast") as bot:
        wait_for_ready(bot)

        # 独孤凝针要求引气境和至少 1 真元；这些 dev 命令只做 bot e2e 铺垫。
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.cmd("qi max 20")
        bot.expect_chat("[dev] qi max", timeout=10.0)
        bot.cmd("qi set 10")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        bot.cmd(f"technique give {SKILL_ID}")
        bot.expect_chat(f"[dev] technique give `{SKILL_ID}`", timeout=10.0)

        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)

        bot.intent(
            {
                "type": "skill_bar_bind",
                "v": 1,
                "slot": SLOT,
                "binding": {"kind": "skill", "skill_id": SKILL_ID},
            }
        )
        time.sleep(0.2)

        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_bar_cast",
                "v": 1,
                "slot": SLOT,
                "target": f"entity:{spawn.data['entity_id']}",
            }
        )

        wait_for_payload_after(
            bot,
            anchor,
            predicate=lambda e: e.data.get("channel") == "bong:vfx_event"
            and "bong:dugu_needle_bolt" in payload_text(e),
            timeout=10.0,
            description="凝针 skill cast 后 bong:vfx_event 携带 bong:dugu_needle_bolt",
        )
        wait_for_server_data_after(
            bot,
            anchor,
            expected_json_types={"cast_sync", "combat_event"},
            timeout=10.0,
            description="凝针 skill cast 后新的 bong:server_data cast_sync 或 combat_event payload",
        )
        bot.assert_alive("技能栏施放凝针并收到专属反馈后")
