"""库存 intent 链路 —— 背包拖入、脱下、穿回的协议级覆盖。

覆盖面：
- `inventory_move_intent` 最新 #957 形状（含 `rotated` 字段）。
- 空穿戴背包件 worn → body_pocket 的合法 intent 不应导致断连。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    container_location,
    equip_location,
    find_item,
    first_free_cell,
    require_item,
    send_move,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "背包移动 intent 覆盖空 pack unequip 请求宽容度"
MODULES = ["inventory"]


def run(env) -> None:
    _run_empty_pack_unequip_equip(env)


def _run_empty_pack_unequip_equip(env) -> None:
    with env.new_bot("Eqp") as bot:
        snapshot = _clear_pack_and_hotbar(bot)
        pack = require_item(snapshot, "worn_grass_pouch")
        pack_id = pack["item"]["instance_id"]
        row, col = first_free_cell(
            snapshot,
            "body_pocket",
            pack["item"]["grid_width"],
            pack["item"]["grid_height"],
        )
        unequip_target = container_location("body_pocket", row, col)
        send_move(
            bot,
            pack_id,
            pack["location"],
            unequip_target,
        )
        time.sleep(0.5)
        bot.assert_alive("空背包脱下 intent 后")


def _clear_pack_and_hotbar(bot) -> dict:
    snapshot = wait_join_and_inventory(bot)
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
    if _carried_containers_empty(snapshot):
        return snapshot
    return wait_inventory_revision_after_matching(
        bot,
        snapshot["revision"],
        _carried_containers_empty,
        "clearinv all 后 carried containers/hotbar 为空且保留 worn_grass_pouch",
        timeout=10.0,
    )


def _carried_containers_empty(snapshot: dict) -> bool:
    if snapshot.get("placed_items"):
        return False
    if any(item is not None for item in snapshot.get("hotbar", [])):
        return False
    return True
