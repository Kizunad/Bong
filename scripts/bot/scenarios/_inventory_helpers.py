"""Shared assertions for inventory bot scenarios."""

from __future__ import annotations

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


def find_item(snapshot: dict[str, Any], item_id: str) -> dict[str, Any] | None:
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["item_id"] == item_id:
            return {
                "location": {
                    "kind": "container",
                    "container_id": placed["container_id"],
                    "row": placed["row"],
                    "col": placed["col"],
                },
                "item": placed["item"],
            }
    for slot, values in snapshot.get("equipped", {}).items():
        if slot.endswith("_worn"):
            equip_slot = slot[: -len("_worn")]
            for item in values:
                if item["item_id"] == item_id:
                    return {
                        "location": {"kind": "equip", "slot": equip_slot, "state": "worn"},
                        "item": item,
                    }
        elif slot.endswith("_held"):
            item = values
            if item and item["item_id"] == item_id:
                return {
                    "location": {
                        "kind": "equip",
                        "slot": slot[: -len("_held")],
                        "state": "held",
                    },
                    "item": item,
                }
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["item_id"] == item_id:
            return {"location": {"kind": "hotbar", "index": index}, "item": item}
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


def require_pack_container(snapshot: dict[str, Any], owner_instance_id: int) -> dict[str, Any]:
    expected_id = f"pack_{owner_instance_id}"
    for container in snapshot.get("containers", []):
        if container.get("owner_instance_id") == owner_instance_id or container["id"] == expected_id:
            return container
    raise BotAssertionError(
        f"期望找到 owner_instance_id={owner_instance_id} 的穿戴背包容器，"
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
