"""物资棺跨维 session 门禁的真实 C2S → server → S2C 黑盒回归。"""

from __future__ import annotations

import json
import time
from pathlib import Path

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    first_free_cell,
    wait_inventory_snapshot_after,
    wait_join_and_inventory,
)

DESCRIPTION = "物资棺 open 后跨维应 close；旧 session move/open 均拒绝并回推权威状态"
MODULES = ["inventory", "supply_coffin", "dimension"]

TSY_FAMILY = "tsy_daneng_01"


def run(env) -> None:
    with env.new_bot("ScDim") as bot:
        initial_snapshot = wait_join_and_inventory(bot)

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.data["payload"]["revision"] > initial_snapshot["revision"],
            timeout=10.0,
            description="clearinv 后 revision 递增的 inventory_snapshot",
        ).data["payload"]

        bot.cmd("supply_coffin reset")
        bot.expect_chat("[dev] reset cleared", timeout=10.0)
        before_spawn = _event_watermark(bot)
        bot.cmd("supply_coffin spawn common")
        bot.expect_chat("[dev] spawned common", timeout=10.0)
        coffin_spawn = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > before_spawn
            and _near_player(bot, event.data),
            timeout=10.0,
            description="/supply_coffin spawn common 后玩家脚下的物资棺 Marker",
        )
        coffin_entity_id = coffin_spawn.data["entity_id"]
        coffin_spawn_signature = {
            key: coffin_spawn.data[key] for key in ("type", "x", "y", "z")
        }

        open_sent_at = _event_watermark(bot)
        bot.intent({"type": "supply_coffin_open", "v": 1, "entity_id": coffin_entity_id})
        opened = _server_data_after(bot, "loot_container_open", open_sent_at, timeout=10.0)
        open_payload = opened.data["payload"]
        session_id = open_payload["session_id"]
        placed_items = open_payload["placed_items"]
        if len(placed_items) < 2:
            raise BotAssertionError(
                "common 物资棺按 loot 契约应至少有 2 种物品；"
                f"expected_count>=2 actual_count={len(placed_items)}"
            )
        moved_source = placed_items[0]
        moved_instance_id = moved_source["item"]["instance_id"]
        instance_id = placed_items[1]["item"]["instance_id"]
        open_snapshot = wait_inventory_snapshot_after(bot, open_sent_at, timeout=10.0)
        destination_container_id, destination_row, destination_col = _first_free_destination(
            open_snapshot,
            moved_source["item"]["grid_width"],
            moved_source["item"]["grid_height"],
        )

        move_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id,
                "instance_id": moved_instance_id,
                "from": {
                    "kind": "container",
                    "container_id": f"ext_{session_id}",
                    "row": moved_source["row"],
                    "col": moved_source["col"],
                },
                "to": {
                    "kind": "container",
                    "container_id": destination_container_id,
                    "row": destination_row,
                    "col": destination_col,
                },
            }
        )
        moved_update = _server_data_after(
            bot, "loot_container_update", move_sent_at, timeout=10.0
        ).data["payload"]
        if moved_update["session_id"] != session_id:
            raise BotAssertionError(
                "合法 move 必须回推当前物资棺 session；"
                f"expected={session_id} actual={moved_update['session_id']}"
            )
        remaining_locations = [
            f"{placed['container_id']}@{placed['row']},{placed['col']}"
            for placed in moved_update["placed_items"]
            if placed["item"]["instance_id"] == moved_instance_id
        ]
        if remaining_locations:
            raise BotAssertionError(
                f"合法 move 后实例 {moved_instance_id} 不得残留在权威 container update；"
                f"expected=[] actual={remaining_locations}"
            )
        authoritative_source = _require_placed_item(
            moved_update["placed_items"],
            instance_id,
            context="合法 move 后供陈旧请求攻击的第二个棺内实例",
        )
        moved_snapshot = wait_inventory_snapshot_after(bot, move_sent_at, timeout=10.0)
        _assert_instance_present_in_inventory(moved_snapshot, moved_instance_id)
        stale_destination_container_id, stale_destination_row, stale_destination_col = (
            _first_free_destination(
                moved_snapshot,
                authoritative_source["item"]["grid_width"],
                authoritative_source["item"]["grid_height"],
            )
        )

        transition_sent_at = _event_watermark(bot)
        premature_closes = _server_data_payloads_between(
            bot,
            "loot_container_close",
            session_id,
            after=opened.t,
            through=transition_sent_at,
        )
        if premature_closes:
            raise BotAssertionError(
                "物资棺 session 在跨维命令前不得提前关闭；"
                f"expected=[] actual={premature_closes}"
            )
        bot.cmd(f"tsy_spawn {TSY_FAMILY}")
        bot.expect_chat(f"Queued /tsy_spawn {TSY_FAMILY}", timeout=10.0)
        entered = bot.wait_for(
            lambda event: event.kind == "respawn" and event.t > transition_sent_at,
            timeout=15.0,
            description=f"通过 {TSY_FAMILY} 入口发生真实跨维 Respawn",
        )

        closed = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "loot_container_close"
            and event.data["payload"]["session_id"] == session_id
            and event.t > transition_sent_at,
            timeout=10.0,
            description=(
                f"/tsy_spawn watermark {transition_sent_at:.3f}s 后 session={session_id} "
                "的 loot_container_close"
            ),
        )
        close_payload = closed.data["payload"]
        if close_payload["session_id"] != session_id or close_payload["reason"] != "distance":
            raise BotAssertionError(
                "跨维必须以 distance 关闭原物资棺 session；"
                f"expected session={session_id}, actual={close_payload}"
            )
        close_snapshot = wait_inventory_snapshot_after(bot, closed.t, timeout=10.0)
        _assert_instance_absent_from_inventory(close_snapshot, instance_id)
        _assert_no_event_after(
            bot,
            "respawn",
            entered.t,
            timeout=0.75,
            context="TSY 入场落点必须位于出口触发圈外，旧实现会在下一 tick 自动回主世界",
        )

        stale_move_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id,
                "instance_id": instance_id,
                "from": {
                    "kind": "container",
                    "container_id": authoritative_source["container_id"],
                    "row": authoritative_source["row"],
                    "col": authoritative_source["col"],
                },
                "to": {
                    "kind": "container",
                    "container_id": stale_destination_container_id,
                    "row": stale_destination_row,
                    "col": stale_destination_col,
                },
            }
        )
        stale_snapshot = wait_inventory_snapshot_after(
            bot, stale_move_sent_at, timeout=10.0
        )
        _assert_instance_absent_from_inventory(stale_snapshot, instance_id)
        if stale_snapshot["revision"] != close_snapshot["revision"]:
            raise BotAssertionError(
                "旧 session move 被拒时 inventory revision 必须不变；"
                f"before={close_snapshot['revision']} actual={stale_snapshot['revision']}"
            )

        reopen_sent_at = _event_watermark(bot)
        bot.intent({"type": "supply_coffin_open", "v": 1, "entity_id": coffin_entity_id})
        _assert_no_server_data_after(
            bot,
            "loot_container_open",
            reopen_sent_at,
            timeout=2.0,
            context="TSY 玩家用旧 Overworld entity_id 重发 supply_coffin_open",
        )

        exit_sent_at = _event_watermark(bot)
        returned = _move_until_respawn(
            bot,
            _tsy_shallow_center(TSY_FAMILY),
            after=exit_sent_at,
            timeout=15.0,
            context=f"踏入 {TSY_FAMILY} Exit 后返回 Overworld",
        )
        bot.wait_for(
            lambda event: event.kind == "pos_look"
            and event.t > exit_sent_at
            and _within_distance(event.data, coffin_spawn_signature, 4.5),
            timeout=5.0,
            description="踏入 Exit 后原物资棺 4.5 格内的 Overworld 权威 PositionLook",
        )
        _assert_no_event_after(
            bot,
            "respawn",
            returned.t,
            timeout=0.75,
            context="返回点必须位于 Entry 触发圈外，避免立刻重新进入 TSY",
        )
        reloaded_coffin = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > exit_sent_at
            and _same_spawn(event.data, coffin_spawn_signature),
            timeout=10.0,
            description=(
                "返回 Overworld 后按原物资棺类型与坐标重新观察 Marker entity_spawn"
            ),
        )
        reloaded_coffin_entity_id = reloaded_coffin.data["entity_id"]

        verify_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "supply_coffin_open",
                "v": 1,
                "entity_id": reloaded_coffin_entity_id,
            }
        )
        reopened = _server_data_after(
            bot, "loot_container_open", verify_sent_at, timeout=10.0
        ).data["payload"]
        if reopened["session_id"] != session_id:
            raise BotAssertionError(
                "返回主世界重开必须恢复同一物资棺 session；"
                f"expected={session_id} actual={reopened['session_id']}"
            )
        reopened_source = _require_placed_item(
            reopened["placed_items"],
            instance_id,
            context="陈旧 move 被拒后返回主世界重开的权威棺内容",
        )
        if reopened_source != authoritative_source:
            raise BotAssertionError(
                "陈旧 move 被拒后棺内源实例必须保持完整不变；"
                f"expected={authoritative_source} actual={reopened_source}"
            )
        bot.assert_alive("物资棺跨维 session 门禁场景完成后")


def _event_watermark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _server_data_payloads_between(
    bot, payload_type: str, session_id: int, after: float, through: float
) -> list[dict]:
    """返回时间窗内属于指定 session 的权威 server_data payload。"""
    return [
        event.data["payload"]
        for event in bot.events
        if event.kind == "server_data"
        and event.data["payload_type"] == payload_type
        and event.data["payload"].get("session_id") == session_id
        and after < event.t <= through
    ]


def _require_placed_item(
    placed_items: list[dict], instance_id: int, context: str
) -> dict:
    """从权威容器 payload 中取得目标实例，否则报告实际实例集合。"""
    for placed in placed_items:
        if placed["item"]["instance_id"] == instance_id:
            return placed
    actual_ids = [placed["item"]["instance_id"] for placed in placed_items]
    raise BotAssertionError(
        f"{context} 必须包含 instance_id={instance_id}；"
        f"expected={instance_id} actual_ids={actual_ids}"
    )


def _tsy_shallow_center(family_id: str) -> tuple[float, float, float]:
    """读取权威 TSY blueprint，返回该 family 的 Exit portal 中心。"""
    zones_path = Path(__file__).resolve().parents[3] / "server" / "zones.tsy.json"
    try:
        zones = json.loads(zones_path.read_text(encoding="utf-8"))["zones"]
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise BotAssertionError(
            f"读取 TSY Exit 权威坐标失败；path={zones_path} error={error}"
        ) from error

    expected_name = f"{family_id}_shallow"
    for zone in zones:
        if zone.get("name") != expected_name:
            continue
        minimum = zone["aabb"]["min"]
        maximum = zone["aabb"]["max"]
        return tuple(
            (float(low) + float(high)) / 2.0
            for low, high in zip(minimum, maximum)
        )
    actual_names = [zone.get("name") for zone in zones]
    raise BotAssertionError(
        f"TSY family 必须存在 shallow zone；expected={expected_name} actual={actual_names}"
    )


def _move_until_respawn(
    bot,
    target: tuple[float, float, float],
    after: float,
    timeout: float,
    context: str,
    speed: float = 4.0,
    tick_hz: float = 20.0,
):
    """逐步走向传送点，并在 Respawn 到达时立即停止发送旧位面坐标。"""
    if bot.position is None:
        raise BotAssertionError(f"{context} 需要已知玩家坐标；actual=None")

    deadline = time.monotonic() + timeout
    step = speed / tick_hz
    while time.monotonic() < deadline:
        current = bot.position
        if current is None:
            raise BotAssertionError(f"{context} 移动中丢失玩家坐标；actual=None")
        delta = tuple(destination - origin for origin, destination in zip(current, target))
        distance = sum(component * component for component in delta) ** 0.5
        if distance > 0.0:
            scale = min(step, distance) / distance
            bot.set_position(
                *(origin + component * scale for origin, component in zip(current, delta))
            )

        remaining = deadline - time.monotonic()
        try:
            return bot.wait_for(
                lambda event: event.kind == "respawn" and event.t > after,
                timeout=min(1.0 / tick_hz, max(remaining, 0.001)),
                description=context,
            )
        except BotAssertionError:
            continue

    raise BotAssertionError(f"{context}；expected=Respawn within {timeout}s actual=timeout")


def _first_free_destination(
    snapshot: dict, item_width: int, item_height: int
) -> tuple[str, int, int]:
    containers = sorted(
        snapshot.get("containers", []),
        key=lambda container: not container.get("quick_access", False),
    )
    for container in containers:
        try:
            row, col = first_free_cell(
                snapshot, container["id"], item_width, item_height
            )
        except BotAssertionError:
            continue
        return container["id"], row, col
    raise BotAssertionError(
        f"合法 move 需要至少一个 {item_width}x{item_height} 玩家容器空位，"
        f"实际 containers={containers}"
    )


def _near_player(bot, data: dict) -> bool:
    if bot.position is None:
        return False
    x, y, z = bot.position
    return (
        abs(data["x"] - x) <= 1.5
        and abs(data["y"] - y) <= 1.5
        and abs(data["z"] - z) <= 1.5
    )


def _same_spawn(actual: dict, expected: dict) -> bool:
    return actual["type"] == expected["type"] and all(
        abs(actual[axis] - expected[axis]) <= 1.0e-6 for axis in ("x", "y", "z")
    )


def _within_distance(actual: dict, expected: dict, maximum: float) -> bool:
    return (
        sum(
            (actual[axis] - expected[axis]) ** 2
            for axis in ("x", "y", "z")
        )
        <= maximum * maximum
    )


def _server_data_after(bot, payload_type: str, after: float, timeout: float):
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.data["payload_type"] == payload_type
        and event.t > after,
        timeout=timeout,
        description=f"t>{after:.3f}s 后 server_data/{payload_type}",
    )


def _assert_no_server_data_after(
    bot, payload_type: str, after: float, timeout: float, context: str
) -> None:
    try:
        event = _server_data_after(bot, payload_type, after, timeout)
    except BotAssertionError:
        return
    raise BotAssertionError(
        f"{context} 必须被拒绝，实际收到 {payload_type}: {event.data['payload']}"
    )


def _assert_no_event_after(
    bot, kind: str, after: float, timeout: float, context: str
) -> None:
    try:
        event = bot.wait_for(
            lambda candidate: candidate.kind == kind and candidate.t > after,
            timeout=timeout,
            description=f"t>{after:.3f}s 后 kind={kind} 事件",
        )
    except BotAssertionError:
        return
    raise BotAssertionError(f"{context}，实际收到额外事件: {event}")


def _assert_instance_absent_from_inventory(snapshot: dict, instance_id: int) -> None:
    locations = _inventory_locations(snapshot, instance_id)
    if locations:
        raise BotAssertionError(
            f"跨维旧 session 不得把棺内实例 {instance_id} 搬入玩家背包；"
            f"expected=[] actual={locations}"
        )


def _assert_instance_present_in_inventory(snapshot: dict, instance_id: int) -> None:
    locations = _inventory_locations(snapshot, instance_id)
    if not locations:
        raise BotAssertionError(
            f"合法 move 后实例 {instance_id} 必须进入玩家背包；"
            "expected=至少一个权威背包位置 actual=[]"
        )


def _inventory_locations(snapshot: dict, instance_id: int) -> list[str]:
    locations: list[str] = []
    for placed in snapshot.get("placed_items", []):
        if placed["item"]["instance_id"] == instance_id:
            locations.append(
                f"{placed['container_id']}@{placed['row']},{placed['col']}"
            )
    for index, item in enumerate(snapshot.get("hotbar", [])):
        if item and item["instance_id"] == instance_id:
            locations.append(f"hotbar@{index}")
    for slot, value in snapshot.get("equipped", {}).items():
        items = value if isinstance(value, list) else [value]
        if any(item and item["instance_id"] == instance_id for item in items):
            locations.append(f"equip@{slot}")
    return locations
