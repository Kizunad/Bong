"""使用武器：装备走 Bong equip 链（非 MC held slot）→ 命中伤害高于空手。

黑盒契约面：
- Bong 的 `Weapon` component 从 `PlayerInventory.equipped["main_hand"].held`
  派生（combat/weapon.rs sync_weapon_component_from_equipped）——MC 原版
  C2S_SELECT_SLOT **不影响**武器判定。装备必须走
  `inventory_move_intent → {kind:"equip", slot:"main_hand", state:"held"}`。
- 命中反馈 = server_data `combat_event`（CombatEventFloaterEntryV1.amount 伤害浮字）。
- **发现记录（2026-07-07 真机实测）**：基础攻击伤害当前**不吃**
  `Weapon.damage_multiplier`——resolve.rs 攻方倍率 = attrs.attack_power ×
  臂伤系数 × 剑技 profile（仅技能路径），武器倍率只有剑技吃。空手/持剑
  普攻 amount 同为 ~10.2。是设计还是断链待拍板；若未来接上，此场景应
  升级为 bare-vs-armed 伤害对照断言。
"""

import time

from bot.scenarios._combat_helpers import (
    extract_floater_amounts,
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
)
from bot.scenarios._inventory_helpers import (
    equip_location,
    require_item,
    send_move,
    wait_inventory_contains,
)

DESCRIPTION = "铁剑经 inventory_move 装备进 main_hand_held + 持剑命中伤害浮字可观察"
MODULES = ["combat", "inventory"]

WEAPON_ID = "iron_sword"


def _hit_amount(bot, target_id: int, anchor: float, label: str) -> float:
    bot.attack_entity(target_id)
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "combat_event"
        and e.t > anchor,
        timeout=10.0,
        description=f"{label} 攻击后应收到 combat_event（伤害浮字可观察）",
    )
    payload = event.data["payload"]
    amounts = extract_floater_amounts(payload)
    assert amounts, (
        f"{label} 的 combat_event 应携带数值 amount（伤害浮字），实际 payload={payload!r}"
    )
    return max(amounts)


def run(env) -> None:
    with env.new_bot("Sword") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)

        # 给剑并装备到 main_hand held
        bot.cmd(f"give {WEAPON_ID} 1")
        snapshot = wait_inventory_contains(bot, WEAPON_ID)
        sword = require_item(snapshot, WEAPON_ID)
        anchor = last_event_time(bot)
        send_move(
            bot,
            int(sword["item"]["instance_id"]),
            sword["location"],
            equip_location("main_hand", "held"),
        )
        equipped = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and (e.data["payload"].get("equipped", {}).get("main_hand_held") or {}).get(
                "item_id"
            )
            == WEAPON_ID,
            timeout=10.0,
            description=(
                "inventory_move_intent 装备后 snapshot.equipped.main_hand_held "
                f"应为 {WEAPON_ID}——装备链断则 Weapon component 永远不生效"
            ),
        )
        assert equipped is not None
        time.sleep(0.5)  # sync_weapon_component 跑一个 tick

        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn, 1.8)

        # 持剑命中：combat_event 伤害浮字可观察且为正数
        anchor = last_event_time(bot)
        armed = _hit_amount(bot, target_id, anchor, "持剑")
        assert armed > 0.0, (
            f"持剑命中 combat_event 应携带正伤害浮字，实际 {armed}"
        )
        bot.assert_alive("武器装备+持剑命中之后")
