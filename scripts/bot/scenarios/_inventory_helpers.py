"""Shared assertions for inventory bot scenarios."""

from __future__ import annotations

import time
from typing import Any

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time


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


def find_instance(snapshot: dict[str, Any], instance_id: int) -> dict[str, Any] | None:
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


def drain_inventory_quiet(bot, quiet: float = 2.0, max_wait: float = 12.0) -> None:
    """请求前排干：把「距下一次周期 flush 的剩余时间」建为 ≥ quiet 秒再放行。

    服务端每 100 tick 会周期 flush 一次「revision 不变的当前状态快照」（实测 ~5s
    一条，100 tick @ 20tps 严格 5.0s），与拒绝回推（同 tick 同步下发、revision 也
    不变）观测上不可分（review finding [2]）。要建立的是**剩余时间**而非「观测到
    连续 quiet 秒无快照」——连续静默只能证明上一快照的年龄 ≥ quiet，无法排除下一
    flush 已在 quiet 秒内逼近（review finding [1]：旧实现从 helper 起跑时刻计静默，
    可在上一 flush 后 2.5s 起跑、静默 2s、4.5s 处返回，下一 flush 0.5s 后就到）。
    本实现锚在**最近一次已观测快照**上，且返回点必须落在
        quiet ≤ (now − last_snap) ≤ PERIOD − quiet
    的窗口内：太早（距上一快照 < quiet）说明残余回推可能仍在滴入；太晚（距下一
    周期 flush 不足 quiet 秒）则 flush 可能落进接受窗口。窗口外则等到下一 flush
    重新锚定后再判。排干结束即保证下一次周期 flush 至少 quiet 秒后才可能到达；
    配合把拒绝回推的接受窗口设成 < quiet，周期 flush 便无法落进窗口冒充权威回推；
    「省略回推」的错误实现窗口内拿不到任何快照，确定性红。
    """
    period = 5.0  # 周期 flush 严格 100 tick @ 20tps
    deadline = time.monotonic() + max_wait
    while True:
        now = time.monotonic()
        snapshots = _inventory_snapshot_events(bot)
        if snapshots:
            age = now - (bot.t0 + snapshots[-1].t)
            if quiet <= age <= period - quiet:
                return
        if now >= deadline:
            raise BotAssertionError(
                f"背包快照过于密集，{max_wait:.0f}s 内未排干到距下一周期 flush "
                f"≥ {quiet:.0f}s（周期 flush 无法满足测试前提）"
            )
        time.sleep(0.25)
