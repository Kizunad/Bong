"""背包/武器/珍宝组：inventory_discard_item / drop_weapon_intent / repair_weapon_intent / treasure_activate。

黑盒契约面（server/src/network/client_request_handler.rs）：
- `inventory_discard_item` 与 `drop_weapon_intent` 汇入 handle_inventory_discard：
  成功 → 物品离包 + revision bump + dropped loot sync 广播；拒绝 → 权威快照回推
  （revision 不变，物品保留）。
- `repair_weapon_intent` → handle_repair_weapon → fully_repair_weapon_instance：
  任何武器实例都 set durability=1.0 并 bump revision；非武器模板拒绝（revision 不变）。
- `treasure_activate` → handle_treasure_activate → apply_treasure_activate：
  activate=true 物品移入 triggered_treasures（**不进 inventory_snapshot**，快照只可见
  「物品消失 + revision bump」）；activate=false 落回背包（物品重现 + bump）；
  非 Treasure 类物品拒绝（物品保留 + revision 不变）。

任一变体都要求拒绝路径不踢线不 panic；warn-only 路径只在 intent 后有限窗口内
检查 revision/content/rejection 不变，结构化拒绝则先等真实 rejection payload，再只接受
rejection 之后实际发出的因果 inventory_snapshot。所有断言都以 intent 时间为起点，
不依赖不存在的周期快照生产者。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
)
from bot.scenarios._inventory_helpers import (
    equip_location,
    find_instance_by_id,
    inventory_signature,
    find_item,
    latest_inventory_snapshot,
    require_item,
    send_move,
    wait_inventory_revision_after,
)

DESCRIPTION = (
    "背包组：discard/drop 成功离包+bump、repair 修武器 bump、treasure 激活/卸下/拒绝"
)
MODULES = ["inventory"]

IRON_SWORD = "iron_sword"
TREASURE_STONE = "spirit_niche_stone"
STARTER_TALISMAN = "starter_talisman"
STATION_POS = [0, 0, 0]


def _give_and_wait(bot, item_id: str, count: int = 1) -> dict:
    anchor = last_event_time(bot)
    bot.cmd(f"give {item_id} {count}")
    time.sleep(0.5)  # give 走 chat→command 通道，先落一拍再等快照（冷启动实测坑）
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], item_id) is not None
        ),
        timeout=10.0,
        description=f"give 后（时间锚之后）含 {item_id} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _wait_required_inventory_resync(
    bot, anchor_t: float, baseline: dict, window: float = 1.5, timeout: float = 10.0
) -> dict:
    """Require one causal rejection resync and verify its full inventory content."""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and anchor_t < e.t <= anchor_t + window
        ),
        timeout=timeout,
        description=(
            f"请求后因果 inventory_snapshot（t∈({anchor_t:.2f}, "
            f"{anchor_t + window:.2f}]）"
        ),
    )
    snapshot = event.data["payload"]
    expected_revision = int(baseline["revision"])
    actual_revision = int(snapshot.get("revision", -1))
    if actual_revision != expected_revision:
        raise BotAssertionError(
            f"[{bot.username}] 拒绝 resync revision 应保持 {expected_revision}，实际 {actual_revision}"
        )
    if inventory_signature(snapshot) != inventory_signature(baseline):
        raise BotAssertionError(
            f"[{bot.username}] 拒绝 resync 必须逐字段保持完整 inventory 内容，"
            "包括 durability 与物品元数据"
        )
    return snapshot


def _move_to_melee_range_live(bot, target_pos, distance: float = 1.0) -> None:
    """每刀前重新追到 NPC 当前坐标旁——NPC 会走位（wander/接战），拿 spawn 坐标当
    靶在 CI 时序下必 whiff（与 combat_weapon_equip_damage 同一套路）。"""
    tx, ty, tz = target_pos
    bx, by, bz = bot.position
    dx, dz = bx - tx, bz - tz
    length = max((dx * dx + dz * dz) ** 0.5, 0.001)
    if length <= distance + 0.3:
        return
    bot.move_to(tx + dx / length * distance, ty, tz + dz / length * distance, speed=5.5)


def _swing_until_durability_below(
    bot, target_id: int, sword_instance: int, timeout: float = 30.0
) -> dict:
    """反复追击出刀，直到快照里 sword 实例 durability < 1.0（打坏前置）。

    武器耐久经 combat resolve 每命中扣减（weapon.tick_durability）并写回
    ItemInstance（set_item_instance_durability → revision bump → 快照推送）。
    返回第一条含 damaged 状态的 inventory_snapshot；超时抛 BotAssertionError。"""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pos = bot.entity_pos(target_id)
        if pos is None:
            break  # 目标已 despawn
        _move_to_melee_range_live(bot, pos)
        swing_anchor = last_event_time(bot)
        bot.attack_entity(target_id)
        time.sleep(1.2)
        for e in bot.events_of("server_data"):
            if (
                e.data.get("payload_type") != "inventory_snapshot"
                or e.t <= swing_anchor
            ):
                continue
            found = find_instance_by_id(e.data["payload"], sword_instance)
            if found is not None and float(found["item"]["durability"]) < 1.0:
                return e.data["payload"]
    raise BotAssertionError(
        f"[{bot.username}] 打刀 {timeout:.0f}s 内未在 inventory_snapshot 观测到"
        f" sword 实例 {sword_instance} durability < 1.0（打坏前置失败）"
    )


def _wait_dropped_loot_for(
    bot, anchor_t: float, instance_id: int, item_id: str, timeout: float = 10.0
) -> dict:
    """discard/drop 成功后的跨系统后果：等待含该实例的 dropped_loot_sync 广播。

    契约面（dropped_loot_sync_emit + discard_inventory_item_to_dropped_loot）：
    discard/drop 把物品移入 DroppedLootRegistry（保留原 instance_id）并在内容
    变化当 tick 广播 dropped_loot_sync。只断言「物品离包 + revision bump」会让
    「删包但不落世界」的实现（永久物品丢失）也通过——必须看到世界层广播出现
    该实例才算成功。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "dropped_loot_sync"
            and e.t > anchor_t
            and any(
                d.get("instance_id") == instance_id
                and (d.get("item") or {}).get("item_id") == item_id
                for d in e.data["payload"].get("drops", [])
            )
        ),
        timeout=timeout,
        description=f"含实例 {instance_id}（{item_id}）的 dropped_loot_sync 广播",
    )
    return event.data["payload"]


def run(env) -> None:
    with env.new_bot("InvGroup") as bot:
        wait_for_ready(bot)
        # naked + all 双清：naked 卸装备槽（出生 main_hand 剑进包），all 清包+hotbar。
        # 只做 all 会留一把出生 worn 剑，污染 drop 断言（见 combat_weapon_equip_damage 注释）。
        bot.cmd("clearinv naked")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        time.sleep(1.0)  # 命令通道冷却：清包后 0.6s 内 give 仍会被静默丢弃（实测坑，1.0s 稳）

        # ── 1. repair_weapon_intent 正路径：先打坏武器（durability<1.0 前置）→
        #    修复 → durability=1.0 + revision bump ──
        sword_snapshot = _give_and_wait(bot, IRON_SWORD)
        sword = require_item(sword_snapshot, IRON_SWORD)
        sword_instance = int(sword["item"]["instance_id"])
        assert abs(float(sword["item"]["durability"]) - 1.0) < 1e-6, (
            f"give 出的新剑耐久应为 1.0，实际 {sword['item']['durability']}"
        )
        # central-review 2012 #4：give 出的新剑满耐久，直接 repair 会让「bump revision
        # 但不动耐久」的错误实现也通过（修后 1.0 == 修前 1.0）。必须建立打坏前置：
        # 装备 main_hand → 攻击战斗 NPC 直至快照可见 durability < 1.0。
        equip_anchor = last_event_time(bot)
        send_move(
            bot, sword_instance, sword["location"], equip_location("main_hand", "held")
        )
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > equip_anchor
                and (e.data["payload"].get("equipped", {}).get("main_hand_held") or {}).get(
                    "instance_id"
                )
                == sword_instance
            ),
            timeout=10.0,
            description=f"sword 实例 {sword_instance} 已装备到 main_hand_held",
        )
        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn, 1.2)
        bot.cmd("health set 100")
        damaged = _swing_until_durability_below(bot, target_id, sword_instance)
        damaged_sword = find_instance_by_id(damaged, sword_instance)
        assert damaged_sword is not None and float(damaged_sword["item"]["durability"]) < 1.0, (
            f"打坏前置失败：repair 前 durability 必须 < 1.0，实际 {damaged_sword!r}"
        )
        queue_npc_scenario(bot, "clear")  # 收掉战斗 NPC，避免干扰后续 discard/drop
        sword_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "repair_weapon_intent",
                "v": 1,
                "instance_id": sword_instance,
                "station_pos": STATION_POS,
            }
        )
        repaired = wait_inventory_revision_after(bot, sword_revision, timeout=10.0)
        repaired_sword = find_instance_by_id(repaired, sword_instance)
        assert repaired_sword is not None and abs(
            float(repaired_sword["item"]["durability"]) - 1.0
        ) < 1e-6, (
            f"修复后 durability 应为 1.0，实际 {repaired_sword['item']['durability'] if repaired_sword else None!r}"
        )

        # ── 2. repair 拒绝路径：非武器模板 → 回推快照 revision 不变、物品保留 ──
        talisman_snapshot = _give_and_wait(bot, STARTER_TALISMAN)
        talisman = require_item(talisman_snapshot, STARTER_TALISMAN)
        talisman_instance = int(talisman["item"]["instance_id"])
        # 请求时刻锚定 intent；warn-only 拒绝允许同内容 resync，但不得 mutation。
        anchor = time.monotonic() - bot.t0
        bot.intent(
            {
                "type": "repair_weapon_intent",
                "v": 1,
                "instance_id": talisman_instance,
                "station_pos": STATION_POS,
            }
        )
        rejected = _wait_required_inventory_resync(bot, anchor, talisman_snapshot)
        require_item(rejected, STARTER_TALISMAN)

        # ── 3. inventory_discard_item 正路径：物品离包 + revision bump + 落地广播 ──
        discard_revision = int(latest_inventory_snapshot(bot)["revision"])
        discard_anchor = last_event_time(bot)  # dropped-loot 因果锚：必须在 intent 前
        bot.intent(
            {
                "type": "inventory_discard_item",
                "v": 1,
                "instance_id": talisman_instance,
                "from": talisman["location"],
            }
        )
        after_discard = wait_inventory_revision_after(
            bot, discard_revision, timeout=10.0
        )
        assert find_item(after_discard, STARTER_TALISMAN) is None, (
            f"discard 后 {STARTER_TALISMAN} 应离包，实际仍在快照中"
        )
        _wait_dropped_loot_for(bot, discard_anchor, talisman_instance, STARTER_TALISMAN)

        # ── 4. discard 拒绝路径：不存在的实例 → revision/content 不变 ──
        anchor = time.monotonic() - bot.t0
        before_reject = latest_inventory_snapshot(bot)
        bot.intent(
            {
                "type": "inventory_discard_item",
                "v": 1,
                "instance_id": 999999,
                "from": talisman["location"],
            }
        )
        _wait_required_inventory_resync(bot, anchor, before_reject)

        # ── 5. treasure_activate 正路径：激活 → 物品离快照 + revision bump ──
        stone_snapshot = _give_and_wait(bot, TREASURE_STONE)
        stone = require_item(stone_snapshot, TREASURE_STONE)
        stone_instance = int(stone["item"]["instance_id"])
        stone_revision = int(stone_snapshot["revision"])
        bot.intent(
            {
                "type": "treasure_activate",
                "v": 1,
                "instance_id": stone_instance,
                "activate": True,
            }
        )
        activated = wait_inventory_revision_after(bot, stone_revision, timeout=10.0)
        assert find_item(activated, TREASURE_STONE) is None, (
            f"激活后 {TREASURE_STONE} 应移入触发位（不进快照），实际仍在快照中"
        )

        # ── 6. treasure_activate 卸下：物品落回背包 + revision bump ──
        deactivate_revision = int(activated["revision"])
        bot.intent(
            {
                "type": "treasure_activate",
                "v": 1,
                "instance_id": stone_instance,
                "activate": False,
            }
        )
        deactivated = wait_inventory_revision_after(
            bot, deactivate_revision, timeout=10.0
        )
        require_item(deactivated, TREASURE_STONE)

        # ── 7. treasure_activate 拒绝路径：非 Treasure 类物品 → revision/content 不变 ──
        anchor = time.monotonic() - bot.t0
        reject_snapshot = latest_inventory_snapshot(bot)
        bot.intent(
            {
                "type": "treasure_activate",
                "v": 1,
                "instance_id": sword_instance,
                "activate": True,
            }
        )
        rejected = _wait_required_inventory_resync(bot, anchor, reject_snapshot)
        require_item(rejected, IRON_SWORD)

        # ── 8. drop_weapon_intent 正路径：武器离包 + revision bump + 落地广播 ──
        #    第 1 步后剑仍装备在 main_hand_held，drop 的 from 必须是当前实际位置。
        drop_revision = int(latest_inventory_snapshot(bot)["revision"])
        drop_anchor = last_event_time(bot)  # dropped-loot 因果锚：必须在 intent 前
        bot.intent(
            {
                "type": "drop_weapon_intent",
                "v": 1,
                "instance_id": sword_instance,
                "from": equip_location("main_hand", "held"),
            }
        )
        after_drop = wait_inventory_revision_after(bot, drop_revision, timeout=10.0)
        assert find_item(after_drop, IRON_SWORD) is None, (
            f"drop 后 {IRON_SWORD} 应离包，实际仍在快照中"
        )
        _wait_dropped_loot_for(bot, drop_anchor, sword_instance, IRON_SWORD)

        # ── 9. drop_weapon_intent 拒绝路径：from 位置与实例不符 → revision/content 不变 ──
        anchor = time.monotonic() - bot.t0
        # stone 在第 7 步后仍在背包容器（非装备位），用 main_hand_held 作 from 恒不匹配。
        # review finding [4] + central-review 31442475206 finding [8]：拒绝契约必须连
        # **内容**一起守恒——只查 revision 不变 + 实例还在会让「删掉 stone 实例却
        # 留下 revision 不 bump」或「篡改 item_id/count/durability 等字段而 identity
        # 不变」的错误实现通过（回推快照携带删除/篡改后状态）。先取该实例请求前的
        # 权威完整内容（请求前最新快照即基线），回推后 pin 实例仍在**原位**且
        # item 逐字段相同。
        pre_reject = latest_inventory_snapshot(bot)
        stone_spot = find_instance_by_id(pre_reject, stone_instance)
        assert stone_spot is not None, (
            f"前置：drop 拒绝前 stone 实例 {stone_instance} 应在包中，实际未找到"
        )
        stone_item = stone_spot["item"]
        bot.intent(
            {
                "type": "drop_weapon_intent",
                "v": 1,
                "instance_id": stone_instance,
                "from": equip_location("main_hand", "held"),
            }
        )
        rejected = _wait_required_inventory_resync(bot, anchor, pre_reject)
        kept_stone = find_instance_by_id(rejected, stone_instance)
        assert kept_stone is not None, (
            f"from 不匹配拒绝不得移除 stone 实例 {stone_instance}，实际快照中未找到"
        )
        assert kept_stone["item"] == stone_item, (
            f"from 不匹配拒绝必须逐字段保留 stone 实例内容：请求前 {stone_item!r}，"
            f"回推后 {kept_stone['item']!r}"
        )
        assert kept_stone["location"] == stone_spot["location"], (
            f"from 不匹配拒绝不得移动 stone 实例：位置应保持 {stone_spot['location']!r}，"
            f"实际 {kept_stone['location']!r}"
        )

        bot.assert_alive("背包组 9 步正负路径后")
