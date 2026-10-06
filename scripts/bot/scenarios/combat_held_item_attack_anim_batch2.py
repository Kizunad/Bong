"""手持物攻击动画（第 2 批）：青锋剑 / 灵剑 / 飞旋剑各自广播自己的动画 id。

服务端 `emit_attack_animation_triggers` 对普通近战（AttackSource::Melee）按攻击者手持物
选动画（plan-item-use-anim-v1 第 1、2 批）。三件剑各配一套独立动画，本场景逐件：
清场 → 给剑 → 装到主手 → 挥一刀 → 断言 typed VFX play_anim 精确为该件的动画 id。

清场用 `clearinv naked` + `clearinv all`：`clearinv all` 不清装备槽，出生 loadout 的铁剑
会一直占着主手，不先卸下来，新剑的 equip 会被 HandOccupied 拒绝（combat_weapon_equip_damage
的实测坑）。

依赖：在线服务端 + bot 客户端（`BOT_E2E_*` 环境）。本地无环境时不能直接跑。
"""

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

DESCRIPTION = "第 2 批手持物攻击动画：青锋剑 / 灵剑 / 飞旋剑普通近战各自广播精确的动画 id"
MODULES = ["combat", "inventory", "vfx"]

# (物品 template_id, 应广播的动画 id)。与 vfx_animation_trigger.rs 的 held_attack_anim 表一一对应。
ATTACK_ANIMS = (
    ("qing_feng_sword", "bong:qing_feng_sword_use"),
    ("spirit_sword", "bong:spirit_sword_use"),
    ("flying_sword_feixuan", "bong:flying_sword_feixuan_use"),
)


def _clear_inventory(bot) -> None:
    bot.cmd("clearinv naked")
    bot.expect_chat("[dev] clearinv", timeout=10.0)
    bot.cmd("clearinv all")
    time.sleep(0.5)


def _swing_with(bot, weapon_id: str, expected_anim: str) -> None:
    _clear_inventory(bot)

    queue_npc_scenario(bot, "clear")
    spawn = queue_passive_target(bot)
    target_id = spawn.data["entity_id"]
    move_to_melee_target(bot, target_id, spawn)

    give_anchor = last_event_time(bot)
    bot.cmd(f"give {weapon_id} 1")
    snapshot = wait_inventory_contains(bot, weapon_id, timeout=10.0, after_t=give_anchor)
    weapon = find_item(snapshot, weapon_id)
    assert weapon is not None, f"give {weapon_id} 后快照里应有该剑"
    send_move(
        bot,
        int(weapon["item"]["instance_id"]),
        weapon["location"],
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
        and event.data.get("anim_id") == expected_anim,
        timeout=10.0,
        description=f"持 {weapon_id} 普通近战后 typed VFX play_anim 精确为 {expected_anim}",
    )


def run(env) -> None:
    with env.new_bot("HeldAnim") as bot:
        wait_for_ready(bot)
        for weapon_id, expected_anim in ATTACK_ANIMS:
            _swing_with(bot, weapon_id, expected_anim)
        bot.assert_alive("第 2 批手持物攻击动画之后")
