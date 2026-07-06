"""战斗近战链路 —— NPC spawn → Bot 左键攻击 → combat server_data 回推。"""

from __future__ import annotations

from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
    wait_for_server_data_after,
)

DESCRIPTION = "观察近场 NPC entity_spawn，移动到攻击距离并断言左键攻击产生 combat_event payload"
MODULES = ["combat", "npc", "network"]


def run(env) -> None:
    with env.new_bot("Atk") as bot:
        wait_for_ready(bot)

        # 清掉上一轮同服复用遗留的 scenario NPC，避免攻击到旧实体导致断言漂移。
        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        move_to_melee_range(bot, spawn)

        anchor = last_event_time(bot)
        bot.attack_entity(spawn.data["entity_id"])
        wait_for_server_data_after(
            bot,
            anchor,
            expected_json_types={"combat_event"},
            timeout=10.0,
            description="近战命中后新的 bong:server_data combat_event payload",
        )
        bot.assert_alive("近战攻击 NPC 并收到 combat payload 后")
