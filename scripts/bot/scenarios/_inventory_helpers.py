"""Shared assertions for inventory bot scenarios."""

from __future__ import annotations

import json
import time
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


def find_instance_by_id(snapshot: dict[str, Any], instance_id: int) -> dict[str, Any] | None:
    """按 instance_id 定位物品，返回与 `find_item` 同构的 {location, item}。

    模板级 `find_item` 在「同一模板同时有装备位 + 随身实例」时会命中先扫到的那个，
    无法区分具体实例；需要 pin 到被拒绝/被丢弃的特定实例时用本函数。"""
    for placed in snapshot.get("placed_items", []):
        if int(placed["item"]["instance_id"]) == instance_id:
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
            for item in values:
                if int(item["instance_id"]) == instance_id:
                    return {
                        "location": {
                            "kind": "equip",
                            "slot": slot[: -len("_worn")],
                            "state": "worn",
                        },
                        "item": item,
                    }
        elif slot.endswith("_held"):
            if values and int(values["instance_id"]) == instance_id:
                return {
                    "location": {
                        "kind": "equip",
                        "slot": slot[: -len("_held")],
                        "state": "held",
                    },
                    "item": values,
                }
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and int(item["instance_id"]) == instance_id:
            return {"location": {"kind": "hotbar", "index": index}, "item": item}
    return None


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


def inventory_signature(snapshot: dict[str, Any]) -> str:
    """Canonicalize the complete serialized inventory content for rejection checks.

    The projection deliberately retains every item field (including durability and NBT-like
    metadata), container coordinates/metadata, equipped and hotbar structure, and bone coins.
    Derived player fields such as qi/realm/weight are outside the inventory contract and are
    checked by their respective scenarios.
    """
    content = {
        "containers": snapshot.get("containers", []),
        "placed_items": snapshot.get("placed_items", []),
        "equipped": snapshot.get("equipped", {}),
        "hotbar": snapshot.get("hotbar", []),
        "bone_coins": snapshot.get("bone_coins"),
    }
    return json.dumps(content, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def assert_no_inventory_change(
    bot, anchor_t: float, baseline: dict[str, Any], window: float = 2.0
) -> None:
    """在请求后有限窗口内拒绝任何库存 mutation 或结构化拒绝回执。

    warn-only handler 可以发一张内容完全相同的权威快照（例如显式 resync），但不能
    改 revision、改物品内容，也不能伪造 ``inventory_move_rejected``。因此这里不依赖
    不存在的周期快照或网格相位：只检查 ``anchor_t < event.t <= anchor_t + window``
    的真实事件。调用方应在发送 intent 前用 ``time.monotonic() - bot.t0`` 取 anchor。
    """
    time.sleep(window)
    window_end = anchor_t + window
    baseline_revision = int(baseline["revision"])
    baseline_content = inventory_signature(baseline)
    changed: list[Any] = []
    rejected: list[Any] = []
    for event in bot.events_of("server_data"):
        if not anchor_t < event.t <= window_end:
            continue
        payload_type = event.data.get("payload_type")
        if payload_type == "inventory_move_rejected":
            rejected.append(event)
            continue
        if payload_type != "inventory_snapshot":
            continue
        payload = event.data["payload"]
        if (
            int(payload.get("revision", -1)) != baseline_revision
            or inventory_signature(payload) != baseline_content
        ):
            changed.append(event)

    if changed:
        raise BotAssertionError(
            f"[{bot.username}] 请求后 {window:.1f}s 内背包 revision/content 必须保持不变，"
            f"实际发现 {len(changed)} 条变更快照"
        )
    if rejected:
        raise BotAssertionError(
            f"[{bot.username}] warn-only 拒绝在 {window:.1f}s 内不得发送 "
            f"inventory_move_rejected，实际收到 {len(rejected)} 条"
        )

    # 某些错误实现会在内存中直接改 PlayerInventory，却既不 bump revision 也不
    # 发快照；上面的事件窗口无法观察到这种静默 mutation。用一个 schema-valid、
    # 必然不存在的 discard 请求强制服务端走已知的 rejection→resync 读回路径。
    # 该探针在原请求窗口扫描之后才发出，因此它自己的 corrective snapshot 不会
    # 被误归因给原始 warn-only 请求。
    probe_anchor = time.monotonic() - bot.t0
    bot.intent(
        {
            "type": "inventory_discard_item",
            "v": 1,
            "instance_id": 9_007_199_254_740_991,
            "from": {"kind": "hotbar", "index": 0},
        }
    )
    probe_event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and probe_anchor < e.t <= probe_anchor + 1.5
        ),
        timeout=10.0,
        description="无效 discard 探针后的权威 inventory_snapshot",
    )
    probe_snapshot = probe_event.data["payload"]
    if int(probe_snapshot.get("revision", -1)) != baseline_revision:
        raise BotAssertionError(
            f"[{bot.username}] warn-only 拒绝后的探针 resync revision 应保持 "
            f"{baseline_revision}，实际 {probe_snapshot.get('revision')}"
        )
    if inventory_signature(probe_snapshot) != baseline_content:
        raise BotAssertionError(
            f"[{bot.username}] warn-only 拒绝后的探针 resync 必须保持完整 inventory 内容，"
            "包括 durability 与物品元数据"
        )
