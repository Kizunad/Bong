"""Shared assertions for inventory bot scenarios."""

from __future__ import annotations

import re
import time
from typing import Any

from bot.bot import BotAssertionError


def drain_inventory_snapshots(bot, quiet: float = 0.8, max_wait: float = 5.0) -> None:
    """排空在途 inventory_snapshot：发送请求前保证没有未解码的快照留在 socket。

    event.t 是**客户端解码时刻**（bot.py ``_emit`` 用 ``time.monotonic() - t0``）：
    一张在请求发送前就已入队、之后才解码的快照会以 ``t > 锚点`` 冒充「拒绝 resync」——
    请求尚未处理，快照已满足断言（central-review 1993 #2）。inventory_snapshot 无
    周期发射（只随 join / Changed / resync），等一个「无新快照」的短窗口即可排干在途
    快照；用快照级静默而非全事件流静默，避免被 carrier_state 每 1s 周期流量拖到上限。
    到上限仍持续到达则报错（fail-closed，不留「无法排空仍继续」的静默路径）。
    """
    deadline = time.monotonic() + max_wait
    while True:
        anchor = bot.events[-1].t if bot.events else 0.0
        time.sleep(quiet)
        fresh = [
            e
            for e in bot.events
            if e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
        ]
        if not fresh:
            return
        if time.monotonic() > deadline:
            raise BotAssertionError(
                "inventory_snapshot 在排空窗口内持续到达，无法建立请求前快照静默"
            )


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


def wait_inventory_snapshot_after(
    bot,
    after_t: float,
    timeout: float = 10.0,
    until_t: float | None = None,
    after_event_t: float | None = None,
    revision_equals: int | None = None,
) -> dict[str, Any]:
    """等一张 ``after_t < e.t <= until_t`` 且满足附加谓词的 inventory_snapshot。

    ``until_t`` / ``after_event_t`` 给调用方上下界：把快照断言收窄到「与某个请求
    处理有因果关系」的区间（同步点确认时刻 / 同一 handler 前置响应事件时刻）。
    ``revision_equals`` 要求快照 revision 精确等于给定值 —— 任何背包 mutation 都
    bump revision（inventory/mod.rs add_item_to_player_inventory_inner 恰好一次），
    故排除全部 Changed 驱动的更高 revision 重发，把候选取收敛到「零 mutation 快照」
    （central-review 1993 #1：拒绝 resync 是零 mutation，周期/Changed 重发若含
    mutation 则 revision 必然更高）。不传这些约束时行为同旧版（只要求 t > after_t）。
    """
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "inventory_snapshot"
        and e.t > after_t
        and (until_t is None or e.t <= until_t)
        and (after_event_t is None or e.t > after_event_t)
        and (
            revision_equals is None
            or e.data["payload"].get("revision") == revision_equals
        ),
        timeout=timeout,
        description=(
            f"{after_t:.3f}s < t <= {until_t if until_t is not None else '∞'}，"
            f"revision={revision_equals if revision_equals is not None else '任意'} 的 "
            f"inventory_snapshot"
        ),
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


def wait_inventory_contains(
    bot,
    item_id: str,
    timeout: float = 10.0,
    *,
    after_t: float | None = None,
    after_revision: int | None = None,
) -> dict[str, Any]:
    """Wait for a matching snapshot after an explicit request watermark.

    A fresh call must not silently reuse the pre-request snapshot already in
    the bot history.  Rejection/resync callers pass the send timestamp and the
    previous revision; requiring both when supplied makes the post-response
    inventory observation explicit.
    """
    def matches(event) -> bool:
        if event.kind != "server_data" or event.data["payload_type"] != "inventory_snapshot":
            return False
        if after_t is not None and event.t <= after_t:
            return False
        payload = event.data["payload"]
        if after_revision is not None and int(payload["revision"]) <= after_revision:
            return False
        return find_item(payload, item_id) is not None

    event = bot.wait_for(
        matches,
        timeout=timeout,
        description=(
            f"{item_id} 的 inventory_snapshot"
            + (f"（t>{after_t:.3f}）" if after_t is not None else "")
            + (f"（revision>{after_revision}）" if after_revision is not None else "")
        ),
    )
    return event.data["payload"]


def inventory_instance_map(snapshot: dict[str, Any]) -> dict[int, dict[str, Any]]:
    """Return every authoritative inventory item keyed by globally unique instance_id."""
    items: dict[int, dict[str, Any]] = {}

    def add(item: dict[str, Any]) -> None:
        instance_id = int(item["instance_id"])
        if instance_id in items:
            raise BotAssertionError(
                f"inventory_snapshot 内 instance_id={instance_id} 重复出现，无法证明唯一所有权"
            )
        items[instance_id] = item

    for placed in snapshot.get("placed_items", []):
        add(placed["item"])
    for slot, values in snapshot.get("equipped", {}).items():
        if slot.endswith("_worn"):
            for item in values:
                add(item)
        elif slot.endswith("_held") and values:
            add(values)
    for item in snapshot.get("hotbar", []):
        if item:
            add(item)
    return items


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
