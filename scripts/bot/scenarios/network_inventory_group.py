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

任一变体都要求拒绝路径不踢线不 panic；负断言用「revision 相同的回推快照」区分于
「revision 变化的成功快照」。周期 flush（~5s 一条，revision 不变）与拒绝回推观测
不可分，每条拒绝路径锚定前先 drain_inventory_quiet 把下一次周期 flush 推离到
quiet 秒之后（锚定最近一次快照、按 5s 周期算剩余时间），并把回推快照的接受限定在
请求后 window 秒内（window < quiet）——周期 flush 无法落进窗口冒充，「省略回推」的
错误实现窗口内无快照确定性红。
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
    drain_inventory_quiet,
    equip_location,
    find_instance,
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


def _wait_snapshot_same_revision(
    bot, anchor_t: float, revision: int, window: float = 1.5, timeout: float = 10.0
) -> dict:
    """拒绝路径的权威回推：revision 不变 + 请求后 window 秒内到达。

    调用方必须先 drain_inventory_quiet 把下一次周期 flush 推离到 quiet 秒之后
    （锚定最近一次快照、按 5s 周期算剩余时间，review finding [1]：只静默 quiet
    秒不锚快照会放跑 0.5s 后的 flush），再发意图——排干后下一次周期 flush 至少
    quiet 秒后才可能到，而拒绝回推与请求同 tick 同步下发（毫秒级）。把接受限定在
    (anchor_t, anchor_t+window]，周期 flush（实测 ~5s 一条）无法落进窗口冒充；
    「省略回推」的错误实现窗口内无快照，确定性红（review finding [2]：旧实现只查
    revision 不变，任何周期 flush 都能冒充）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=(
            f"拒绝回推 inventory_snapshot（t∈({anchor_t:.2f}, "
            f"{anchor_t + window:.2f}]）"
        ),
    )
    if event.t > anchor_t + window:
        raise BotAssertionError(
            f"[{bot.username}] 拒绝回推快照应在请求后 {window}s 内到达，"
            f"实际 {event.t - anchor_t:.2f}s 后才到（疑似周期 flush 而非回推）"
        )
    got = int(event.data["payload"]["revision"])
    if got != revision:
        raise BotAssertionError(
            f"[{bot.username}] 拒绝路径回推快照 revision 应保持 {revision}，实际 {got}"
        )
    return event.data["payload"]


def _assert_no_snapshot_change(
    bot, anchor_t: float, revision: int, window: float = 2.0
) -> None:
    """请求被静默忽略（无回推）时的负断言：窗口内不得出现新 inventory_snapshot。"""
    time.sleep(window)
    stray = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "inventory_snapshot" and e.t > anchor_t
    ]
    if stray:
        raise BotAssertionError(
            f"[{bot.username}] 期望 {window}s 内无 inventory_snapshot，实际收到 {len(stray)} 条"
        )


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
            found = find_instance(e.data["payload"], sword_instance)
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
        damaged_sword = find_instance(damaged, sword_instance)
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
        repaired_sword = find_instance(repaired, sword_instance)
        assert repaired_sword is not None and abs(
            float(repaired_sword["item"]["durability"]) - 1.0
        ) < 1e-6, (
            f"修复后 durability 应为 1.0，实际 {repaired_sword['item']['durability'] if repaired_sword else None!r}"
        )

        # ── 2. repair 拒绝路径：非武器模板 → 回推快照 revision 不变、物品保留 ──
        talisman_snapshot = _give_and_wait(bot, STARTER_TALISMAN)
        talisman = require_item(talisman_snapshot, STARTER_TALISMAN)
        talisman_instance = int(talisman["item"]["instance_id"])
        talisman_revision = int(talisman_snapshot["revision"])
        # 拒绝路径锚定前必须排干：周期 flush（revision 不变）与拒绝回推观测不可分，
        # 排干到 quiet 秒静默后下一次 flush 至少 quiet 秒才可能到，窗口内不可能冒充。
        drain_inventory_quiet(bot)
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "repair_weapon_intent",
                "v": 1,
                "instance_id": talisman_instance,
                "station_pos": STATION_POS,
            }
        )
        rejected = _wait_snapshot_same_revision(bot, anchor, talisman_revision)
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

        # ── 4. discard 拒绝路径：不存在的实例 → 回推快照 revision 不变 ──
        drain_inventory_quiet(bot)
        anchor = last_event_time(bot)
        before_reject = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "inventory_discard_item",
                "v": 1,
                "instance_id": 999999,
                "from": talisman["location"],
            }
        )
        _wait_snapshot_same_revision(bot, anchor, before_reject)

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

        # ── 7. treasure_activate 拒绝路径：非 Treasure 类物品 → 回推快照 revision 不变 ──
        drain_inventory_quiet(bot)
        anchor = last_event_time(bot)
        reject_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "treasure_activate",
                "v": 1,
                "instance_id": sword_instance,
                "activate": True,
            }
        )
        rejected = _wait_snapshot_same_revision(bot, anchor, reject_revision)
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

        # ── 9. drop_weapon_intent 拒绝路径：from 位置与实例不符 → 回推快照 revision 不变 ──
        drain_inventory_quiet(bot)
        anchor = last_event_time(bot)
        stale_revision = int(after_drop["revision"])
        # stone 在第 7 步后仍在背包容器（非装备位），用 main_hand_held 作 from 恒不匹配。
        # review finding [4]：拒绝契约必须连**内容**一起守恒——只查 revision 不变会让
        # 「删掉 stone 实例却留下 revision 不 bump」的错误实现通过（回推快照携带删除
        # 后状态）。先取该实例请求前的权威位置，回推后 pin 实例仍在**原位**。
        stone_spot = find_instance(after_drop, stone_instance)
        assert stone_spot is not None, (
            f"前置：drop 拒绝前 stone 实例 {stone_instance} 应在包中，实际未找到"
        )
        bot.intent(
            {
                "type": "drop_weapon_intent",
                "v": 1,
                "instance_id": stone_instance,
                "from": equip_location("main_hand", "held"),
            }
        )
        rejected = _wait_snapshot_same_revision(bot, anchor, stale_revision)
        kept_stone = find_instance(rejected, stone_instance)
        assert kept_stone is not None, (
            f"from 不匹配拒绝不得移除 stone 实例 {stone_instance}，实际快照中未找到"
        )
        assert kept_stone["location"] == stone_spot["location"], (
            f"from 不匹配拒绝不得移动 stone 实例：位置应保持 {stone_spot['location']!r}，"
            f"实际 {kept_stone['location']!r}"
        )

        bot.assert_alive("背包组 9 步正负路径后")
