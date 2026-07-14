"""物资棺跨维 session 门禁的真实 C2S → server → S2C 黑盒回归。"""

from __future__ import annotations

import math

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    first_free_cell,
    wait_inventory_snapshot_after,
    wait_join_and_inventory,
)

DESCRIPTION = "物资棺 open 后跨维应 close；旧 session move/open 均拒绝并回推权威状态"
MODULES = ["inventory", "supply_coffin", "dimension"]

SETUP_ZONE = "spawn"


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

        # 该回归只验证跨维 session 权威链路。用 server-side dev teleport 把
        # 场景放到稳定空域，避免邻列地形的穿地恢复把返回点额外抬高。
        setup_sent_at = _event_watermark(bot)
        bot.cmd(f"tpzone {SETUP_ZONE}")
        bot.expect_chat(f"Teleported to zone `{SETUP_ZONE}`.", timeout=10.0)
        bot.wait_for(
            lambda event: event.kind == "pos_look" and event.t > setup_sent_at,
            timeout=10.0,
            description=f"/tpzone {SETUP_ZONE} 后 server 权威 PositionLook",
        )

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

        # 第二具棺保持 active 但不创建 session，用于在跨维同裸坐标下独立验证
        # forged open 会被 dimension gate 拒绝，而不是被“已有 opened_by”旁路拒绝。
        open_probe_spawn_sent_at = _event_watermark(bot)
        bot.cmd("supply_coffin spawn common")
        bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > open_probe_spawn_sent_at
            and "[dev] spawned common" in event.data["text"],
            timeout=10.0,
            description="第二具 common 物资棺 spawn 命令反馈",
        )
        open_probe_spawn = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > open_probe_spawn_sent_at
            and event.data["entity_id"] != coffin_entity_id
            and _near_player(bot, event.data),
            timeout=10.0,
            description="供跨维 forged open 使用的第二具物资棺 Marker",
        )
        open_probe_entity_id = open_probe_spawn.data["entity_id"]

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
        entered, _ = _transfer_dimension_same_xyz(
            bot,
            target="tsy",
            after=transition_sent_at,
            coffin_spawn=coffin_spawn_signature,
            context="保持物资棺裸 XYZ 不变进入 TSY",
        )

        forged_open_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "supply_coffin_open",
                "v": 1,
                "entity_id": open_probe_entity_id,
            }
        )
        _assert_no_server_data_after(
            bot,
            "loot_container_open",
            forged_open_sent_at,
            timeout=2.0,
            context=(
                "TSY 玩家在 4.5 格内用未占用 Overworld 棺 entity_id 发起 forged open"
            ),
        )

        closed = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "loot_container_close"
            and event.data["payload"]["session_id"] == session_id
            and event.t > transition_sent_at,
            timeout=10.0,
            description=(
                f"/tpdim tsy watermark {transition_sent_at:.3f}s 后 session={session_id} "
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
            context="同 XYZ dev transfer 后不得发生未请求的第二次跨维",
        )

        # close 后请求只验证 session cleanup / unknown-session 拒绝。映射仍有效且
        # opened_by 仍属于玩家时的跨维 move 续权，由真实 C2S 入口 Rust 测试
        # `supply_coffin_external_move_real_c2s_rejects_cross_dimension_same_xyz_while_session_is_valid_and_resyncs`
        # 锁定，避免依赖 lifecycle 与网络请求之间的非确定竞速。
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

        return_sent_at = _event_watermark(bot)
        returned, _ = _transfer_dimension_same_xyz(
            bot,
            target="overworld",
            after=return_sent_at,
            coffin_spawn=coffin_spawn_signature,
            context="保持物资棺裸 XYZ 不变返回 Overworld",
        )
        _assert_no_event_after(
            bot,
            "respawn",
            returned.t,
            timeout=0.75,
            context="返回 Overworld 后不得发生未请求的第二次跨维",
        )
        reloaded_coffin = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > return_sent_at
            and event.data["entity_id"] == coffin_entity_id
            and _same_spawn(event.data, coffin_spawn_signature),
            timeout=10.0,
            description=(
                "返回 Overworld 后按原 entity_id、类型与坐标重新观察物资棺 Marker"
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


def _transfer_dimension_same_xyz(
    bot, target: str, after: float, coffin_spawn: dict, context: str
):
    """走正式 transfer consumer，并锁定跨维前后与棺材的裸 XYZ 仍在 4.5 格内。"""
    bot.cmd(f"tpdim {target}")
    bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > after
        and f"Queued /tpdim {target} at current XYZ." in event.data["text"],
        timeout=10.0,
        description=f"/tpdim {target} server 权威 transfer 排队反馈",
    )
    respawn = bot.wait_for(
        lambda event: event.kind == "respawn" and event.t > after,
        timeout=10.0,
        description=f"/tpdim {target} 触发真实跨维 Respawn",
    )
    position = bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t > after,
        timeout=10.0,
        description=f"/tpdim {target} 后 server 权威 PositionLook",
    )
    if position.data["flags"] != 0:
        raise BotAssertionError(
            f"{context} 必须返回绝对 PositionLook；"
            f"expected_flags=0 actual_flags={position.data['flags']}"
        )

    actual_xyz = tuple(position.data[axis] for axis in ("x", "y", "z"))
    coffin_xyz = tuple(coffin_spawn[axis] for axis in ("x", "y", "z"))
    if not all(math.isfinite(value) for value in actual_xyz):
        raise BotAssertionError(
            f"{context} 的权威坐标必须有限；expected=finite actual={actual_xyz}"
        )
    actual_distance = math.dist(actual_xyz, coffin_xyz)
    if actual_distance > 4.5:
        raise BotAssertionError(
            f"{context} 必须保留旧 XYZ-only open 阈值内前提；"
            f"expected_distance<=4.5 actual_distance={actual_distance:.6f} "
            f"player={actual_xyz} coffin={coffin_xyz}"
        )
    return respawn, position


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
