"""Shared assertions for inventory bot scenarios."""

from __future__ import annotations

import re
from typing import Any

from bot.bot import BotAssertionError


def wait_join_and_inventory(bot, timeout: float = 15.0) -> dict[str, Any]:
    bot.expect_event("game_join", timeout=timeout)
    bot.expect_event("pos_look", timeout=timeout)
    return latest_inventory_snapshot(bot, timeout=timeout)


def latest_inventory_snapshot(bot, timeout: float = 10.0) -> dict[str, Any]:
    events = _inventory_snapshot_events(bot)
    if events:
        return events[-1].data["payload"]
    event = bot.expect_server_data("inventory_snapshot", timeout=timeout)
    return event.data["payload"]


def wait_inventory_snapshot_after(bot, after_t: float, timeout: float = 10.0) -> dict[str, Any]:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t,
        timeout=timeout,
        description=f"t > {after_t:.3f}s 的 inventory_snapshot",
    )
    return event.data["payload"]


def wait_inventory_revision_after(bot, previous_revision: int, timeout: float = 10.0) -> dict[str, Any]:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.data["payload"]["revision"] > previous_revision,
        timeout=timeout,
        description=f"revision > {previous_revision} 的 inventory_snapshot",
    )
    return event.data["payload"]


def wait_inventory_revision_after_matching(
    bot,
    previous_revision: int,
    predicate,
    description: str,
    timeout: float = 10.0,
) -> dict[str, Any]:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.data["payload"]["revision"] > previous_revision
        and predicate(e.data["payload"]),
        timeout=timeout,
        description=f"revision > {previous_revision} 且 {description} 的 inventory_snapshot",
    )
    return event.data["payload"]


def give_inventory_revision_barrier(
    bot,
    item_id: str,
    timeout: float = 20.0,
) -> dict[str, Any]:
    """用 dev give 的回执 revision 建立 FIFO inventory barrier。

    仅等待命令后的任意 inventory_snapshot 会误收命令前请求的迟到快照。这里先从精确
    give 回执取得本次 mutation revision，再等待 revision 不低于该值的快照；调用方因而
    能读取命令前请求已经结算后的权威库存。barrier item 必须与待观察模板不同，避免可
    堆叠物品沿用同一 instance_id 而污染判定。
    """
    anchor = max((event.t for event in getattr(bot, "events", [])), default=0.0)
    bot.cmd(f"give {item_id} 1")
    pattern = re.compile(rf"^\[dev\] gave {re.escape(item_id)} x1 revision=(\d+)$")
    matched_revision: dict[str, int] = {}

    def is_barrier_receipt(event) -> bool:
        if event.kind != "chat" or event.t <= anchor:
            return False
        match = pattern.fullmatch(event.data["text"])
        if match is None:
            return False
        matched_revision["value"] = int(match.group(1))
        return True

    bot.wait_for(
        is_barrier_receipt,
        timeout=timeout,
        description=f"give-barrier {item_id} 的精确 revision 回执",
    )
    revision = matched_revision["value"]
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > anchor
        and e.data["payload"]["revision"] >= revision,
        timeout=timeout,
        description=f"give-barrier {item_id} revision >= {revision} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _inventory_snapshot_events(bot) -> list[Any]:
    if hasattr(bot, "events_of"):
        events = bot.events_of("server_data")
    else:
        events = [event for event in getattr(bot, "events", []) if event.kind == "server_data"]
    return [
        event
        for event in events
        if event.data.get("payload_type") == "inventory_snapshot"
    ]


def wait_inventory_contains(bot, item_id: str, timeout: float = 10.0) -> dict[str, Any]:
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and find_item(e.data["payload"], item_id) is not None,
        timeout=timeout,
        description=f"包含 item_id={item_id} 的 inventory_snapshot",
    )
    return event.data["payload"]


def find_items(snapshot: dict[str, Any], item_id: str) -> list[dict[str, Any]]:
    """snapshot 中 item_id 的**全部**匹配（placed/equipped/hotbar 全遍历）。

    find_item 只返回首个匹配；同一模板可同时存在多实例（恢复的旧存档实例 + 新 give 的
    同 id 实例），单查会漏掉其余实例，导致计数/排除失真。"""
    matches: list[dict[str, Any]] = []
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["item_id"] == item_id:
            matches.append(
                {
                    "location": {
                        "kind": "container",
                        "container_id": placed["container_id"],
                        "row": placed["row"],
                        "col": placed["col"],
                    },
                    "item": placed["item"],
                }
            )
    for slot, values in snapshot.get("equipped", {}).items():
        if slot.endswith("_worn"):
            equip_slot = slot[: -len("_worn")]
            for item in values:
                if item["item_id"] == item_id:
                    matches.append(
                        {
                            "location": {
                                "kind": "equip",
                                "slot": equip_slot,
                                "state": "worn",
                            },
                            "item": item,
                        }
                    )
        elif slot.endswith("_held"):
            item = values
            if item and item["item_id"] == item_id:
                matches.append(
                    {
                        "location": {
                            "kind": "equip",
                            "slot": slot[: -len("_held")],
                            "state": "held",
                        },
                        "item": item,
                    }
                )
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["item_id"] == item_id:
            matches.append({"location": {"kind": "hotbar", "index": index}, "item": item})
    return matches


def find_item(snapshot: dict[str, Any], item_id: str) -> dict[str, Any] | None:
    found = find_items(snapshot, item_id)
    return found[0] if found else None


def find_instance(
    snapshot: dict[str, Any], item_id: str, instance_id: int
) -> dict[str, Any] | None:
    """按实例 id 精确查找 item（find_item 首匹配无法区分同模板多实例——恢复的旧存档实例
    与新 give 实例共存时可能选中已消费的旧实例）。用于「具体实例是否仍在包内」的消耗/结算
    判定（review finding [major] round 5：同模板旧实例残留会掩盖新实例已被消费）。"""
    for found in find_items(snapshot, item_id):
        if found["item"]["instance_id"] == instance_id:
            return found
    return None


def require_item(snapshot: dict[str, Any], item_id: str) -> dict[str, Any]:
    found = find_item(snapshot, item_id)
    if found is None:
        raise BotAssertionError(
            f"期望 inventory_snapshot 中存在 {item_id}，实际 snapshot 未找到；"
            f"containers={snapshot.get('containers')}"
        )
    return found


def require_container(snapshot: dict[str, Any], container_id: str) -> dict[str, Any]:
    for container in snapshot.get("containers", []):
        if container["id"] == container_id:
            return container
    raise BotAssertionError(
        f"期望 inventory_snapshot 中存在 container={container_id}，实际 {snapshot.get('containers')}"
    )


def inventory_item_instances(bot, item_id: str) -> set[int]:
    """当前已知快照流中 item_id 的**全部**实例 id（含恢复/存档物品）。

    give 前调用：恢复的旧存档物品（如上次运行遗留）会在 clearinv 处理前仍出现在快照
    流里，其 instance_id 对 place/consume 已失效，必须从「新实例」候选中排除。同一
    快照可能同时含多个同 id 实例，必须逐个收集而非取首个匹配。
    """
    instances: set[int] = set()
    for event in _inventory_snapshot_events(bot):
        for found in find_items(event.data["payload"], item_id):
            instances.add(found["item"]["instance_id"])
    return instances


def wait_inventory_contains_new_instance(
    bot,
    item_id: str,
    exclude_instances: set[int],
    timeout: float = 10.0,
) -> dict[str, Any]:
    """等一个含 item_id 且其实例 id ∉ exclude_instances 的 inventory_snapshot，返回匹配的
    **item 实例**（find_items 风格 {"location","item"}），而非整个快照。

    wait_inventory_contains 无时间锚点，give 后可能命中恢复/存档的陈旧快照而返回旧实例
    id；本函数保证拿到的实例是新 give 产生的（实例 id 全局不复用）。**返回具体实例而非
    快照**（review finding [major] round 5）：调用方若用 require_item 首匹配，快照同时
    含恢复的旧实例与新 give 实例时会选中旧实例——其 instance_id 已 stale，放置被拒；
    消耗判定按「包内无 mundane_coffin」首匹配也会被旧同模板实例残留掩盖。调用方必须用
    本函数返回的实例 id 发送放置、并按该 id 判定消耗。
    """
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and any(
            found["item"]["instance_id"] not in exclude_instances
            for found in find_items(e.data["payload"], item_id)
        ),
        timeout=timeout,
        description=(
            f"含 item_id={item_id} 新实例（∉ {sorted(exclude_instances)}）的 inventory_snapshot"
        ),
    )
    for found in find_items(event.data["payload"], item_id):
        if found["item"]["instance_id"] not in exclude_instances:
            return found
    raise BotAssertionError(
        f"wait_for 命中后仍找不到 item_id={item_id} 的新实例（∉ {sorted(exclude_instances)}）"
    )


def require_pack_container(snapshot: dict[str, Any], owner_instance_id: int) -> dict[str, Any]:
    expected_id = f"pack_{owner_instance_id}"
    for container in snapshot.get("containers", []):
        if container["id"] != expected_id:
            continue
        if container.get("owner_instance_id") != owner_instance_id:
            raise BotAssertionError(
                f"穿戴背包容器 id={expected_id} 的 owner_instance_id 必须精确匹配；"
                f"expected={owner_instance_id} actual={container.get('owner_instance_id')}"
            )
        return container
    raise BotAssertionError(
        f"期望找到 id={expected_id} 且 owner_instance_id={owner_instance_id} 的穿戴背包容器，"
        f"实际 containers={snapshot.get('containers')}"
    )


def first_free_cell(
    snapshot: dict[str, Any],
    container_id: str,
    item_width: int,
    item_height: int,
) -> tuple[int, int]:
    container = require_container(snapshot, container_id)
    rows, cols = container["rows"], container["cols"]
    occupied = set()
    for placed in snapshot.get("placed_items", []):
        if placed["container_id"] != container_id:
            continue
        item = placed["item"]
        for row in range(placed["row"], placed["row"] + item["grid_height"]):
            for col in range(placed["col"], placed["col"] + item["grid_width"]):
                occupied.add((row, col))

    for row in range(0, rows - item_height + 1):
        for col in range(0, cols - item_width + 1):
            cells = [
                (r, c)
                for r in range(row, row + item_height)
                for c in range(col, col + item_width)
            ]
            if all(cell not in occupied for cell in cells):
                return row, col
    raise BotAssertionError(
        f"期望 container={container_id} 有 {item_width}x{item_height} 空位，实际已占 {sorted(occupied)}"
    )


def container_location(container_id: str, row: int, col: int) -> dict[str, Any]:
    return {"kind": "container", "container_id": container_id, "row": row, "col": col}


def equip_location(slot: str, state: str = "worn") -> dict[str, Any]:
    return {"kind": "equip", "slot": slot, "state": state}


def send_move(bot, instance_id: int, from_location: dict[str, Any], to_location: dict[str, Any]) -> None:
    bot.intent(
        {
            "type": "inventory_move_intent",
            "v": 1,
            "instance_id": instance_id,
            "from": from_location,
            "to": to_location,
            "rotated": False,
        }
    )
