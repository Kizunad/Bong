"""库存 intent 链路 —— 背包拖入、脱下、穿回的协议级覆盖。

覆盖面：
- `inventory_move_intent` 最新 #957 形状（含 `rotated` 字段）。
- 非背包物品拖入 `pack_<id>` 后触发全量 `inventory_snapshot` resync。
- 空穿戴背包件 worn ↔ body_pocket 用 revision 水位确认脱下/穿回落位。
"""

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    container_location,
    equip_location,
    find_item,
    first_free_cell,
    require_item,
    require_pack_container,
    send_move,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "背包移动 intent 覆盖 pack stow/unequip/equip 状态与 inventory resync"
MODULES = ["inventory"]


def run(env) -> None:
    _run_stow_into_equipped_pack(env)
    _run_empty_pack_unequip_equip(env)


def _run_stow_into_equipped_pack(env) -> None:
    with env.new_bot("Stw") as bot:
        snapshot = _clear_pack_only(bot)
        pack = require_item(snapshot, "worn_grass_pouch")
        pack_id = pack["item"]["instance_id"]
        pack_container = require_pack_container(snapshot, pack_id)
        item = _first_body_pocket_item(snapshot)
        row, col = first_free_cell(
            snapshot,
            pack_container["id"],
            item["item"]["grid_width"],
            item["item"]["grid_height"],
        )
        send_move(
            bot,
            item["item"]["instance_id"],
            item["location"],
            container_location(pack_container["id"], row, col),
        )
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda s: _instance_location(s, item["item"]["instance_id"]) == container_location(
                pack_container["id"], row, col
            ),
            f"instance {item['item']['instance_id']} 已进入 {pack_container['id']}",
            timeout=10.0,
        )
        pack = require_item(snapshot, "worn_grass_pouch")
        moved_location = _instance_location(snapshot, item["item"]["instance_id"])
        if pack["location"] != equip_location("chest", "worn"):
            raise BotAssertionError(
                f"stow 后背包件应仍穿在 chest/worn，实际 location={pack['location']}"
            )
        if moved_location != container_location(pack_container["id"], row, col):
            raise BotAssertionError(
                "stow 后物品应位于穿戴背包容器 "
                f"{pack_container['id']}，实际 location={moved_location}"
            )


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
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda s: _item_location(s, "worn_grass_pouch") == unequip_target,
            f"worn_grass_pouch 已脱到 {unequip_target}",
            timeout=10.0,
        )
        unequipped = require_item(snapshot, "worn_grass_pouch")

        send_move(
            bot,
            pack_id,
            unequipped["location"],
            equip_location("chest", "worn"),
        )
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda s: _item_location(s, "worn_grass_pouch") == equip_location("chest", "worn"),
            "worn_grass_pouch 已穿回 chest/worn",
            timeout=10.0,
        )
        equipped = require_item(snapshot, "worn_grass_pouch")
        if equipped["location"] != equip_location("chest", "worn"):
            raise BotAssertionError(
                f"穿回后背包件应回到 chest/worn，实际 location={equipped['location']}"
            )

        bot.assert_alive("空背包脱下/穿回 intent 后")


def _clear_pack_and_hotbar(bot) -> dict:
    snapshot = wait_join_and_inventory(bot)
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
    return wait_inventory_revision_after_matching(
        bot,
        snapshot["revision"],
        _carried_containers_empty,
        "clearinv all 后 carried containers/hotbar 为空且保留 worn_grass_pouch",
        timeout=10.0,
    )


def _clear_pack_only(bot) -> dict:
    snapshot = wait_join_and_inventory(bot)
    bot.cmd("clearinv pack")
    bot.expect_chat("[dev] clearinv PackOnly revision=", timeout=10.0)
    return wait_inventory_revision_after_matching(
        bot,
        snapshot["revision"],
        _pack_empty_with_body_item,
        "clearinv pack 后 pack 为空且 body_pocket 保留可移动物品",
        timeout=10.0,
    )


def _carried_containers_empty(snapshot: dict) -> bool:
    if snapshot.get("placed_items"):
        return False
    if any(item is not None for item in snapshot.get("hotbar", [])):
        return False
    return find_item(snapshot, "worn_grass_pouch") is not None


def _pack_empty_with_body_item(snapshot: dict) -> bool:
    pack = find_item(snapshot, "worn_grass_pouch")
    if pack is None:
        return False
    pack_container_id = f"pack_{pack['item']['instance_id']}"
    for placed in snapshot.get("placed_items", []):
        if placed["container_id"] == pack_container_id:
            return False
    return _first_body_pocket_item(snapshot) is not None


def _first_body_pocket_item(snapshot: dict) -> dict | None:
    for placed in snapshot.get("placed_items", []):
        if placed["container_id"] == "body_pocket":
            return {
                "location": container_location("body_pocket", placed["row"], placed["col"]),
                "item": placed["item"],
            }
    return None


def _instance_location(snapshot: dict, instance_id: int) -> dict | None:
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["instance_id"] == instance_id:
            return container_location(placed["container_id"], placed["row"], placed["col"])
    for slot, values in snapshot.get("equipped", {}).items():
        if slot.endswith("_worn"):
            equip_slot = slot[: -len("_worn")]
            for item in values:
                if item["instance_id"] == instance_id:
                    return equip_location(equip_slot, "worn")
        elif slot.endswith("_held"):
            if values and values["instance_id"] == instance_id:
                return equip_location(slot[: -len("_held")], "held")
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["instance_id"] == instance_id:
            return {"kind": "hotbar", "index": index}
    return None


def _item_location(snapshot: dict, item_id: str, container_id: str | None = None) -> dict | None:
    found = find_item(snapshot, item_id)
    if found is None:
        return None
    location = found["location"]
    if container_id is not None and location.get("container_id") != container_id:
        return None
    return location
