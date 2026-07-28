"""库存权威链路 —— 动态背包拓扑、同实例穿脱、clearinv 三分支。"""

from __future__ import annotations

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    container_location,
    equip_location,
    find_item,
    first_free_cell,
    require_container,
    require_item,
    require_pack_container,
    send_move,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "权威 inventory_snapshot 锁定动态 pack owner、同实例穿脱、容量与 clearinv 三分支"
MODULES = ["inventory"]


def run(env) -> None:
    with env.new_bot("Eqp") as bot:
        snapshot = wait_join_and_inventory(bot)
        pack = require_item(snapshot, "worn_grass_pouch")
        pack_id = int(pack["item"]["instance_id"])
        pack_container_id = f"pack_{pack_id}"
        require_pack_container(snapshot, pack_id)
        _assert_pack_location(snapshot, pack_id, "equip")
        _assert_max_weight(snapshot, 23.0, "起手穿戴破草包")
        _assert_instance_count(snapshot, pack_id, 1, "起手快照")

        # pack 只清背包网格，不清 body_pocket/hotbar/equipment。起手动态 pack 非空，
        # 因而这条命令必须生成一个 revision +1 的 typed snapshot。
        body_ids_before = _container_instance_ids(snapshot, "body_pocket")
        hotbar_ids_before = _hotbar_instance_ids(snapshot)
        snapshot = _clearinv(
            bot,
            snapshot,
            "pack",
            "PackOnly",
            lambda candidate: _container_empty(candidate, pack_container_id),
            "动态 pack_<instance_id> 内容为空",
        )
        require_pack_container(snapshot, pack_id)
        if _container_instance_ids(snapshot, "body_pocket") != body_ids_before:
            raise BotAssertionError("clearinv pack 不得改动 body_pocket 精确实例集合")
        if _hotbar_instance_ids(snapshot) != hotbar_ids_before:
            raise BotAssertionError("clearinv pack 不得改动 hotbar 精确实例集合")
        _assert_pack_location(snapshot, pack_id, "equip")
        _assert_max_weight(snapshot, 23.0, "clearinv pack 后仍穿戴破草包")
        _assert_instance_count(snapshot, pack_id, 1, "clearinv pack 后")

        # 起手 body_pocket 可能已被教程物品占满。先通过真实 all 分支清空
        # carried surfaces，确认装备、动态 pack owner 与容量均不变，再做同实例穿脱。
        snapshot = _clearinv(
            bot,
            snapshot,
            "all",
            "PackAndHotbar",
            _carried_empty,
            "穿脱准备阶段所有 container/hotbar 为空",
        )
        require_pack_container(snapshot, pack_id)
        _assert_pack_location(snapshot, pack_id, "equip")
        _assert_max_weight(snapshot, 23.0, "穿脱准备 clearinv all 后")
        _assert_instance_count(snapshot, pack_id, 1, "穿脱准备 clearinv all 后")

        # 同一实例 worn -> body_pocket；动态 pack 容器仍属于该实例，但只有 worn 状态提供容量。
        pack = require_item(snapshot, "worn_grass_pouch")
        row, col = first_free_cell(
            snapshot,
            "body_pocket",
            pack["item"]["grid_width"],
            pack["item"]["grid_height"],
        )
        send_move(bot, pack_id, pack["location"], container_location("body_pocket", row, col))
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: _instance_at_container(
                candidate, pack_id, "body_pocket", row, col
            ),
            f"同一 pack instance={pack_id} 位于 body_pocket@{row},{col}",
            timeout=10.0,
        )
        pack_container = require_pack_container(snapshot, pack_id)
        if pack_container["id"] != pack_container_id:
            raise BotAssertionError(
                f"脱下后动态 pack id 必须保持 {pack_container_id}，实际 {pack_container}"
            )
        _assert_max_weight(snapshot, 15.0, "破草包脱到暗袋")
        _assert_instance_count(snapshot, pack_id, 1, "破草包脱下后")

        # 同实例 body_pocket -> chest worn，容量恢复；不得创建新背包实例或新 owner。
        pack = require_item(snapshot, "worn_grass_pouch")
        send_move(bot, pack_id, pack["location"], equip_location("chest"))
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: _instance_at_equip(candidate, pack_id, "chest_worn"),
            f"同一 pack instance={pack_id} 回到 chest worn",
            timeout=10.0,
        )
        require_pack_container(snapshot, pack_id)
        _assert_max_weight(snapshot, 23.0, "破草包重新穿回")
        _assert_instance_count(snapshot, pack_id, 1, "破草包穿回后")

        # all 清 carried containers + hotbar，但保留装备、动态 pack 拓扑和容量。
        snapshot = _clearinv(
            bot,
            snapshot,
            "all",
            "PackAndHotbar",
            _carried_empty,
            "所有 container/hotbar 为空",
        )
        require_pack_container(snapshot, pack_id)
        _assert_pack_location(snapshot, pack_id, "equip")
        _assert_max_weight(snapshot, 23.0, "clearinv all 后仍穿戴破草包")
        _assert_instance_count(snapshot, pack_id, 1, "clearinv all 后")

        # naked 清装备并 rebuild：动态 pack 消失、暗袋保留、容量回到裸体 BASE。
        snapshot = _clearinv(
            bot,
            snapshot,
            "naked",
            "All",
            lambda candidate: _carried_empty(candidate)
            and _equipment_empty(candidate)
            and not _has_container(candidate, pack_container_id),
            "装备为空、动态 pack 消失且 carried 为空",
        )
        require_container(snapshot, "body_pocket")
        if _has_container(snapshot, pack_container_id):
            raise BotAssertionError(
                f"clearinv naked 后不得残留 orphan {pack_container_id}"
            )
        _assert_max_weight(snapshot, 15.0, "clearinv naked")
        _assert_instance_count(snapshot, pack_id, 0, "clearinv naked 后")
        bot.assert_alive("动态背包拓扑与 clearinv 三分支完成后")


def _clearinv(bot, snapshot, scope, feedback_scope, predicate, description) -> dict:
    previous_revision = int(snapshot["revision"])
    anchor = bot.events[-1].t if bot.events else 0.0
    bot.cmd(f"clearinv {scope}")
    bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > anchor
        and event.data.get("text")
        == f"[dev] clearinv {feedback_scope} revision={previous_revision + 1}",
        timeout=10.0,
        description=f"clearinv {scope} 精确 revision={previous_revision + 1} 回执",
    )
    return wait_inventory_revision_after_matching(
        bot,
        previous_revision,
        predicate,
        description,
        timeout=10.0,
    )


def _container_empty(snapshot: dict, container_id: str) -> bool:
    return not any(
        placed["container_id"] == container_id
        for placed in snapshot.get("placed_items", [])
    )


def _carried_empty(snapshot: dict) -> bool:
    return not snapshot.get("placed_items") and not _hotbar_instance_ids(snapshot)


def _equipment_empty(snapshot: dict) -> bool:
    for value in snapshot.get("equipped", {}).values():
        if isinstance(value, list) and value:
            return False
        if isinstance(value, dict):
            return False
    return True


def _has_container(snapshot: dict, container_id: str) -> bool:
    return any(container["id"] == container_id for container in snapshot.get("containers", []))


def _container_instance_ids(snapshot: dict, container_id: str) -> list[int]:
    return sorted(
        int(placed["item"]["instance_id"])
        for placed in snapshot.get("placed_items", [])
        if placed["container_id"] == container_id
    )


def _hotbar_instance_ids(snapshot: dict) -> list[int]:
    return sorted(
        int(item["instance_id"])
        for item in snapshot.get("hotbar", [])
        if isinstance(item, dict)
    )


def _instance_at_container(
    snapshot: dict, instance_id: int, container_id: str, row: int, col: int
) -> bool:
    found = find_item(snapshot, "worn_grass_pouch")
    return bool(
        found
        and int(found["item"]["instance_id"]) == instance_id
        and found["location"]
        == container_location(container_id, row, col)
    )


def _instance_at_equip(snapshot: dict, instance_id: int, equipped_key: str) -> bool:
    return any(
        int(item["instance_id"]) == instance_id
        for item in snapshot.get("equipped", {}).get(equipped_key, [])
    )


def _assert_pack_location(snapshot: dict, instance_id: int, kind: str) -> None:
    pack = require_item(snapshot, "worn_grass_pouch")
    if int(pack["item"]["instance_id"]) != instance_id or pack["location"]["kind"] != kind:
        raise BotAssertionError(
            f"破草包位置/实例不符；expected_instance={instance_id} expected_kind={kind} "
            f"actual={pack}"
        )


def _assert_max_weight(snapshot: dict, expected: float, context: str) -> None:
    actual = float(snapshot.get("weight", {}).get("max", -1.0))
    if abs(actual - expected) > 1e-6:
        raise BotAssertionError(
            f"{context} max_weight 不符；expected={expected} actual={actual}"
        )


def _assert_instance_count(
    snapshot: dict, instance_id: int, expected: int, context: str
) -> None:
    count = 0
    for placed in snapshot.get("placed_items", []):
        count += int(placed["item"]["instance_id"]) == instance_id
    for value in snapshot.get("equipped", {}).values():
        if isinstance(value, list):
            count += sum(int(item["instance_id"]) == instance_id for item in value)
        elif isinstance(value, dict):
            count += int(value["instance_id"]) == instance_id
    count += sum(
        int(item["instance_id"]) == instance_id
        for item in snapshot.get("hotbar", [])
        if isinstance(item, dict)
    )
    if count != expected:
        raise BotAssertionError(
            f"{context} instance_id={instance_id} 全局计数不符；"
            f"expected={expected} actual={count}"
        )
