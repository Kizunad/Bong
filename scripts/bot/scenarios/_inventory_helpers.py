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


PERIODIC_FLUSH_PERIOD = 5.0  # 服务端严格 100 tick @ 20tps
PERIODIC_FLUSH_GRID_TOL = 0.4


def _periodic_flush_events(bot) -> list[Any]:
    """只保留与服务器固定周期 flush 网格对齐的快照。

    服务端每 100 tick（严格 5.0s）发一条「revision 不变的当前状态快照」，固定
    相位网格不随任何事件平移；mutation（give/move/forge）bump revision 且不重置
    定时器，拒绝回推与请求同 tick 下发（revision 也不变）。payload 无 reason
    字段，单条快照无法区分周期 flush 与拒绝回推——但时间戳暴露了网格：真 flush
    严格落在 5.0s 算术网格上，拒绝回推落在请求时刻（任意相位）。本函数取覆盖
    同 revision 事件数最多的网格相位，且必须 ≥2 条事件确认（单条或两两歧义时
    无法判定，保守返回空——误锚回推会把「距下一 flush 的剩余时间」高估到 ≥
    quiet，central-review 31442475206 finding [1] 的精确攻击面）。"""
    snapshots = _inventory_snapshot_events(bot)
    same_rev: list[Any] = []
    previous_revision: int | None = None
    for event in snapshots:
        revision = int(event.data["payload"].get("revision", -1))
        if previous_revision is not None and revision == previous_revision:
            same_rev.append(event)
        previous_revision = revision
    if not same_rev:
        return []

    def phase_dist(t: float, offset: float) -> float:
        d = (t - offset) % PERIODIC_FLUSH_PERIOD
        return min(d, PERIODIC_FLUSH_PERIOD - d)

    best_offset: float | None = None
    best_count = 0
    for candidate in same_rev:
        offset = candidate.t % PERIODIC_FLUSH_PERIOD
        count = sum(
            1 for e in same_rev if phase_dist(e.t, offset) <= PERIODIC_FLUSH_GRID_TOL
        )
        if count > best_count:
            best_count = count
            best_offset = offset
    if best_offset is None or best_count < 2:
        return []
    return [
        e for e in same_rev if phase_dist(e.t, best_offset) <= PERIODIC_FLUSH_GRID_TOL
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


def drain_inventory_quiet(bot, quiet: float = 2.0, max_wait: float = 12.0) -> None:
    """请求前排干：把「距下一次周期 flush 的剩余时间」建为 ≥ quiet 秒再放行。

    服务端每 100 tick 会周期 flush 一次「revision 不变的当前状态快照」（实测 ~5s
    一条，100 tick @ 20tps 严格 5.0s），与拒绝回推（同 tick 同步下发、revision 也
    不变）观测上不可分（review finding [2]）。要建立的是**剩余时间**而非「观测到
    连续 quiet 秒无快照」——连续静默只能证明上一快照的年龄 ≥ quiet，无法排除下一
    flush 已在 quiet 秒内逼近（review finding [1]：旧实现从 helper 起跑时刻计静默，
    可在上一 flush 后 2.5s 起跑、静默 2s、4.5s 处返回，下一 flush 0.5s 后就到）。
    本实现锚在**最近一次网格确认的周期 flush**上（`_periodic_flush_events` 先按
    revision 与前一条快照相同排除 bump revision 的 mutation，再按 5.0s 固定网格
    相位排除拒绝回推——回推 revision 也不变、只落在请求时刻，central-review
    31438252846 finding [1] + 31442475206 finding [1] 根因；锚在 mutation 或回推
    上都会误判剩余时间），且返回点必须落在
        quiet ≤ (now − last_periodic_flush) ≤ PERIOD − quiet
    的窗口内：太早（距上一 flush < quiet）说明残余回推可能仍在滴入；太晚（距下一
    周期 flush 不足 quiet 秒）则 flush 可能落进接受窗口。窗口外则等到下一 flush
    重新锚定后再判。排干结束即保证下一次周期 flush 至少 quiet 秒后才可能到达；
    配合把拒绝回推的接受窗口设成 < quiet，周期 flush 便无法落进窗口冒充权威回推；
    「省略回推」的错误实现窗口内拿不到任何快照，确定性红。
    """
    period = PERIODIC_FLUSH_PERIOD
    deadline = time.monotonic() + max_wait
    while True:
        now = time.monotonic()
        periodic = _periodic_flush_events(bot)
        if periodic:
            age = now - (bot.t0 + periodic[-1].t)
            if quiet <= age <= period - quiet:
                return
        if now >= deadline:
            raise BotAssertionError(
                f"背包快照过于密集，{max_wait:.0f}s 内未排干到距下一周期 flush "
                f"≥ {quiet:.0f}s（mutation 快照不重置周期定时器，只能锚在真周期 "
                f"flush 上，周期 flush 无法满足测试前提）"
            )
        time.sleep(0.25)
