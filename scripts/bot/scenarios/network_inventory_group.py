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
「revision 变化的成功快照」。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_inventory_snapshot_after,
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
    bot, anchor_t: float, revision: int, timeout: float = 10.0
) -> dict:
    """拒绝路径的回推快照：revision 必须与请求前一致（server 未采纳任何变更）。"""
    snapshot = wait_inventory_snapshot_after(bot, anchor_t, timeout=timeout)
    got = int(snapshot["revision"])
    if got != revision:
        raise BotAssertionError(
            f"[{bot.username}] 拒绝路径回推快照 revision 应保持 {revision}，实际 {got}"
        )
    return snapshot


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

        # ── 1. repair_weapon_intent 正路径：修武器 → durability=1.0 + revision bump ──
        sword_snapshot = _give_and_wait(bot, IRON_SWORD)
        sword = require_item(sword_snapshot, IRON_SWORD)
        sword_instance = int(sword["item"]["instance_id"])
        sword_revision = int(sword_snapshot["revision"])
        bot.intent(
            {
                "type": "repair_weapon_intent",
                "v": 1,
                "instance_id": sword_instance,
                "station_pos": STATION_POS,
            }
        )
        repaired = wait_inventory_revision_after(bot, sword_revision, timeout=10.0)
        repaired_sword = require_item(repaired, IRON_SWORD)
        assert float(repaired_sword["item"]["durability"]) == 1.0, (
            f"修复后 durability 应为 1.0，实际 {repaired_sword['item']['durability']!r}"
        )

        # ── 2. repair 拒绝路径：非武器模板 → 回推快照 revision 不变、物品保留 ──
        talisman_snapshot = _give_and_wait(bot, STARTER_TALISMAN)
        talisman = require_item(talisman_snapshot, STARTER_TALISMAN)
        talisman_instance = int(talisman["item"]["instance_id"])
        talisman_revision = int(talisman_snapshot["revision"])
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

        # ── 3. inventory_discard_item 正路径：物品离包 + revision bump ──
        discard_revision = int(latest_inventory_snapshot(bot)["revision"])
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

        # ── 4. discard 拒绝路径：不存在的实例 → 回推快照 revision 不变 ──
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

        # ── 8. drop_weapon_intent 正路径：武器离包 + revision bump ──
        drop_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "drop_weapon_intent",
                "v": 1,
                "instance_id": sword_instance,
                "from": sword["location"],
            }
        )
        after_drop = wait_inventory_revision_after(bot, drop_revision, timeout=10.0)
        assert find_item(after_drop, IRON_SWORD) is None, (
            f"drop 后 {IRON_SWORD} 应离包，实际仍在快照中"
        )

        # ── 9. drop_weapon_intent 拒绝路径：from 位置与实例不符 → 回推快照 revision 不变 ──
        anchor = last_event_time(bot)
        stale_revision = int(after_drop["revision"])
        bot.intent(
            {
                "type": "drop_weapon_intent",
                "v": 1,
                "instance_id": stone_instance,
                "from": sword["location"],
            }
        )
        _wait_snapshot_same_revision(bot, anchor, stale_revision)

        bot.assert_alive("背包组 9 步正负路径后")
