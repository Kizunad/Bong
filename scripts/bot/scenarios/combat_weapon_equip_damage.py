"""使用武器：卸出生剑 → 空手基线 → 装满耐久铁剑 → 方向过滤后 armed > bare。

黑盒契约面（2026-07-07 全链路澄清后重写）：
- **出生 loadout 自带 main_hand 铁剑**（assets/inventory/loadouts/default.toml，
  durability=0.5 破损起手剑）；`clearinv all` 只清 pack+hotbar **不清装备槽**，
  必须 `clearinv naked` 才能得到真空手——旧版场景的装备断言被出生剑假满足、
  give 剑的 equip 实际被 HandOccupied 拒绝。
- Bong 的 `Weapon` component 从 `equipped["main_hand"].held` 派生
  （combat/weapon.rs sync），MC 原版 select_slot 不影响。
- 命中反馈 = `combat_event` 浮字（CombatEventFloaterEntryV1）；本场景依赖
  其 `outgoing` 方向字段（true=己方输出，false=承伤）过滤掉 NPC 反击——
  无方向时双方浮字同值不可区分（playtest 误判「武器不加伤」的根源）。
- 伤害契约（plan-weapon-v1 §6.1 + combat/resolve.rs physical 分支）：
  damage = weapon_base × body 部位系数 × attrs × weapon_multiplier ×
  wound × sword_profile。空手 weapon_base=1.0（触 1.0 伤害地板）；满耐久
  iron_sword = 12 × 1.2 → 己方输出必须远高于空手。
"""

import time

from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
)
from bot.scenarios._inventory_helpers import (
    equip_location,
    find_item,
    require_item,
    send_move,
)

DESCRIPTION = "卸出生剑→空手基线→装满耐久铁剑→outgoing 过滤后 armed>bare（武器伤害契约）"
MODULES = ["combat", "inventory"]

WEAPON_ID = "iron_sword"


def _outgoing_hits(bot, target_id: int, swings: int = 4) -> list[float]:
    amounts: list[float] = []
    for _ in range(swings):
        anchor = last_event_time(bot)
        bot.attack_entity(target_id)
        time.sleep(1.2)
        for e in bot.events:
            if (
                e.kind == "server_data"
                and e.data["payload_type"] == "combat_event"
                and e.t > anchor
            ):
                for entry in e.data["payload"].get("events", []):
                    if entry.get("kind") == "hit" and entry.get("outgoing"):
                        amounts.append(float(entry["amount"]))
    return amounts


def run(env) -> None:
    with env.new_bot("Sword") as bot:
        wait_for_ready(bot)

        # 清场两连：naked 卸装备槽（出生剑会被卸进背包而非删除！），
        # 再 all 清空背包+hotbar——只做 naked 会留一把 0.5 耐久旧剑在包里
        # 污染后续 require_item（实测坑）
        bot.cmd("clearinv naked")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("clearinv all")
        time.sleep(0.5)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and not e.data["payload"].get("equipped", {}).get("main_hand_held")
            and find_item(e.data["payload"], WEAPON_ID) is None,
            timeout=10.0,
            description=(
                "清场后全域不应再有任何 iron_sword（main_hand 空 + 包内无）——"
                "出生 loadout 剑必须彻底出场，否则基线和装备全被污染"
            ),
        )

        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn, 1.2)

        # 空手基线（outgoing 过滤：只统计己方输出）
        bare_hits = _outgoing_hits(bot, target_id)
        assert bare_hits, (
            "空手攻击应产生 outgoing=true 的 hit 浮字（拳距 2.0 格内、伤害地板 "
            "≥1.0）——收不到说明空手攻击链或方向标识断了"
        )
        bare = max(bare_hits)

        # 给满耐久铁剑并装备（手已空，equip 必须成功）。
        # 必须带时间锚：无锚扫描会命中清场前含出生剑(0.5 耐久)的旧快照（实测坑）
        give_anchor = last_event_time(bot)
        bot.cmd(f"give {WEAPON_ID} 1")
        given_event = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > give_anchor
            and find_item(e.data["payload"], WEAPON_ID) is not None,
            timeout=10.0,
            description="give 后（时间锚之后）应出现含新铁剑的 inventory_snapshot",
        )
        sword = require_item(given_event.data["payload"], WEAPON_ID)
        given_iid = int(sword["item"]["instance_id"])
        assert abs(float(sword["item"]["durability"]) - 1.0) < 1e-6, (
            f"/give 出的新剑耐久应为 1.0（runtime_instance_from_template），"
            f"实际 {sword['item']['durability']}"
        )
        anchor = last_event_time(bot)
        send_move(bot, given_iid, sword["location"], equip_location("main_hand", "held"))
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and (e.data["payload"].get("equipped", {}).get("main_hand_held") or {}).get(
                "instance_id"
            )
            == given_iid,
            timeout=10.0,
            description=(
                f"equip 后 main_hand_held.instance_id 应为刚给的 {given_iid}"
                f"（按 instance 锁死，防任何同名剑混淆）"
            ),
        )
        time.sleep(0.5)  # sync_weapon_component 跑一个 tick

        # 换新靶+回血+重新贴脸：bare 阶段挨了 NPC 数秒反击，血量/站位已不可控
        bot.cmd("health set 100")
        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn, 1.2)

        # 持剑输出（outgoing 过滤）
        armed_hits = _outgoing_hits(bot, target_id)
        assert armed_hits, "持剑攻击应产生 outgoing=true 的 hit 浮字"
        armed = max(armed_hits)

        assert armed > bare * 2.0, (
            f"满耐久铁剑（base 12 × multiplier 1.2）己方输出应远高于空手"
            f"（weapon_base=1.0 伤害地板）：期望 armed > 2×bare，实际 "
            f"bare={bare} armed={armed}——不成立说明 equipped→Weapon 派生或 "
            f"resolve 武器因子回归"
        )
        bot.assert_alive("武器装备+方向过滤对照之后")
