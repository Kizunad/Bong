"""手持物攻击动画链：持铁剑普通近战 → typed VFX play_anim 精确为 bong:iron_sword_v2_use。

服务端 `emit_attack_animation_triggers` 对普通近战（AttackSource::Melee）按攻击者手持物
选动画（plan-item-use-anim-v1 第 1 批）：有 `Weapon` 取其 template_id，没有取主手持有物。
铁剑在表内 → 应广播 `bong:iron_sword_v2_use`，而不是伤口类型的默认剑斩 `bong:sword_slash_down`。

依赖：在线服务端 + bot 客户端（`BOT_E2E_*` 环境）。本地无环境时不能直接跑。
"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_target,
    queue_npc_scenario,
    queue_passive_target,
    wait_for_ready,
)
from bot.scenarios._inventory_helpers import (
    equip_location,
    find_item,
    send_move,
    wait_inventory_contains,
)

DESCRIPTION = "手持物攻击动画：持铁剑普通近战 → typed vfx play_anim 精确为 bong:iron_sword_v2_use"
MODULES = ["combat", "inventory", "vfx"]

WEAPON_ID = "iron_sword"
SWORD_ANIM = "bong:iron_sword_v2_use"
DEFAULT_SLASH_ANIM = "bong:sword_slash_down"


def run(env) -> None:
    with env.new_bot("HeldAnim") as bot:
        wait_for_ready(bot)

        queue_npc_scenario(bot, "clear")
        spawn = queue_passive_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_target(bot, target_id, spawn)

        bot.cmd(f"give {WEAPON_ID} 1")
        snapshot = wait_inventory_contains(bot, WEAPON_ID, timeout=10.0)
        sword = find_item(snapshot, WEAPON_ID)
        send_move(
            bot,
            int(sword["item"]["instance_id"]),
            sword["location"],
            equip_location("main_hand", "held"),
        )
        # sync_weapon_component 跑一个 tick，之后 Weapon 组件才挂在玩家身上。
        time.sleep(0.5)

        anchor = last_event_time(bot)
        bot.attack_entity(target_id)
        bot.wait_for(
            lambda event: event.kind == "vfx_event"
            and event.t > anchor
            and event.data.get("type") == "play_anim"
            and event.data.get("anim_id") == SWORD_ANIM,
            timeout=10.0,
            description=f"持铁剑普通近战后 typed VFX play_anim 精确为 {SWORD_ANIM}（非 {DEFAULT_SLASH_ANIM}）",
        )
        bot.assert_alive("手持物攻击动画之后")
