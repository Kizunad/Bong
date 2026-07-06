"""库存 intent 链路 —— 背包拖入、脱下、穿回的协议级覆盖。

覆盖面：
- `inventory_move_intent` 最新 #957 形状（含 `rotated` 字段）。
- 非背包物品拖入 `pack_<id>` 后至少有 `inventory_pack_stow` VFX 或 moved event。
- 穿戴背包件 worn ↔ body_pocket 时分别有 `inventory_pack_unequip/equip` 反馈，
  且 pack move 触发全量 `inventory_snapshot` resync。
"""

from ._inventory_helpers import (
    container_location,
    equip_location,
    first_free_cell,
    latest_inventory_snapshot,
    require_item,
    require_pack_container,
    send_move,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "背包移动 intent 覆盖 pack stow/unequip/equip 反馈与 inventory resync"
MODULES = ["inventory"]


def run(env) -> None:
    with env.new_bot("Inv") as bot:
        snapshot = wait_join_and_inventory(bot)

        # 清出稳定起点：容器和 hotbar 空，装备保留，避免起手物品占格影响背包脱下。
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

        pack = require_item(snapshot, "worn_grass_pouch")
        pack_id = pack["item"]["instance_id"]
        pack_container = require_pack_container(snapshot, pack_id)

        bot.cmd("give starter_talisman 1")
        bot.expect_chat("[dev] gave starter_talisman x1", timeout=10.0)
        snapshot = wait_inventory_contains(bot, "starter_talisman", timeout=10.0)

        talisman = require_item(snapshot, "starter_talisman")
        row, col = first_free_cell(
            snapshot,
            pack_container["id"],
            talisman["item"]["grid_width"],
            talisman["item"]["grid_height"],
        )
        send_move(
            bot,
            talisman["item"]["instance_id"],
            talisman["location"],
            container_location(pack_container["id"], row, col),
        )
        _expect_pack_feedback(
            bot,
            "bong:inventory_pack_stow",
            talisman["item"]["instance_id"],
            "拖入穿戴背包容器应触发 stow VFX 或 inventory_event::moved",
        )

        send_move(
            bot,
            pack_id,
            equip_location("chest", "worn"),
            container_location("body_pocket", 0, 0),
        )
        bot.expect_vfx_event("bong:inventory_pack_unequip", timeout=10.0)
        snapshot = latest_inventory_snapshot(bot, timeout=10.0)
        unequipped = require_item(snapshot, "worn_grass_pouch")

        send_move(
            bot,
            pack_id,
            unequipped["location"],
            equip_location("chest", "worn"),
        )
        bot.expect_vfx_event("bong:inventory_pack_equip", timeout=10.0)
        latest_inventory_snapshot(bot, timeout=10.0)

        bot.assert_alive("背包拖入/脱下/穿回 intent 后")


def _expect_pack_feedback(bot, event_id: str, instance_id: int, description: str) -> None:
    bot.wait_for(
        lambda e: (
            e.kind == "vfx_event"
            and e.data.get("event_id") == event_id
        )
        or (
            e.kind == "server_data"
            and e.data["payload_type"] == "inventory_event"
            and e.data["payload"].get("kind") == "moved"
            and e.data["payload"].get("instance_id") == instance_id
        ),
        timeout=10.0,
        description=description,
    )
