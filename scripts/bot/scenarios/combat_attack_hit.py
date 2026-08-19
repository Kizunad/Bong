"""战斗近战链路 —— NPC spawn → Bot 左键攻击 → combat server_data 回推。"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_range,
    queue_passive_target,
    queue_npc_scenario,
    wait_for_ready,
)

DESCRIPTION = "被动靶协议链：左键命中产生 outgoing combat_event，致死后 exact target entities_destroy"
MODULES = ["combat", "npc", "network"]


def run(env) -> None:
    with env.new_bot("Atk") as bot:
        wait_for_ready(bot)

        # 清掉上一轮同服复用遗留的 scenario NPC，避免攻击到旧实体导致断言漂移。
        queue_npc_scenario(bot, "clear")
        spawn = queue_passive_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn)

        anchor = last_event_time(bot)
        for _ in range(40):
            if bot.entity_pos(target_id) is None:
                break
            bot.attack_entity(target_id)
            time.sleep(0.25)

        outgoing_hit = any(
            event.kind == "server_data"
            and event.data.get("payload_type") == "combat_event"
            and any(
                entry.get("kind") == "hit"
                and entry.get("outgoing") is True
                and float(entry.get("amount", 0.0)) > 0.0
                for entry in event.data.get("payload", {}).get("events", [])
            )
            for event in bot.events
            if event.t > anchor
        )
        assert outgoing_hit, "被动靶左键命中必须产生 outgoing=true 且 amount>0 的 typed combat_event"
        destroyed = bot.wait_for(
            lambda event: event.kind == "entities_destroy"
            and target_id in event.data.get("entity_ids", [])
            and event.t > anchor,
            timeout=10.0,
            description="被动靶致死后的 exact target entities_destroy",
        )
        assert target_id in destroyed.data["entity_ids"]
        bot.assert_alive("近战攻击 NPC 并收到 combat payload 后")
