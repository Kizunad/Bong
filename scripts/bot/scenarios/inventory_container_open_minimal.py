"""权威世界货箱链路：放置、双向 move、拒绝回滚与显式 close。"""

from __future__ import annotations

import math

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    container_location,
    find_item,
    first_free_cell,
    require_item,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "trade_crate 真实放置后以 typed 快照锁定双向 move、拒绝回滚、无丢失复制与 close"
MODULES = ["inventory", "container"]


def run(env) -> None:
    with env.new_bot("Box") as bot:
        snapshot = wait_join_and_inventory(bot)
        bot.send_client_settings(view_distance=10)

        rendezvous_at = _event_watermark(bot)
        bot.cmd("tpzone spawn")
        bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > rendezvous_at
            and event.data.get("text") == "Teleported to zone `spawn`.",
            timeout=10.0,
            description="/tpzone spawn 权威命令回执",
        )
        bot.wait_for(
            lambda event: event.kind == "pos_look" and event.t > rendezvous_at,
            timeout=10.0,
            description="/tpzone spawn 后 server 权威 PositionLook",
        )
        bot.wait_for(
            lambda event: event.kind == "chunk_data" and event.t > rendezvous_at,
            timeout=15.0,
            description="trade_crate 放置前 rendezvous 周围真实 ChunkData",
        )

        clear_at = _event_watermark(bot)
        bot.cmd("clearinv all")
        bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > clear_at
            and event.data.get("text")
            == f"[dev] clearinv PackAndHotbar revision={snapshot['revision'] + 1}",
            timeout=10.0,
            description="clearinv all 精确 revision 回执",
        )
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: not candidate.get("placed_items")
            and not _hotbar_instance_ids(candidate),
            "carried containers 与 hotbar 全空",
            timeout=10.0,
        )

        bot.cmd("give trade_crate 1")
        bot.expect_chat("[dev] gave trade_crate x1", timeout=10.0)
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: find_item(candidate, "trade_crate") is not None,
            "出现 trade_crate",
            timeout=10.0,
        )
        crate = require_item(snapshot, "trade_crate")
        crate_instance_id = int(crate["item"]["instance_id"])

        x, y, z = _placement_pos(bot)
        placed_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "block_place",
                "v": 1,
                "x": x,
                "y": y,
                "z": z,
                "item_instance_id": crate_instance_id,
                "target_face": "north",
            }
        )
        spawn = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > placed_at
            and abs(event.data["x"] - (x + 0.5)) <= 0.01
            and abs(event.data["y"] - y) <= 0.01
            and abs(event.data["z"] - (z + 0.5)) <= 0.01,
            timeout=10.0,
            description="trade_crate 放置后精确坐标的容器 Marker entity_spawn",
        )
        crate_snapshot_revision = snapshot["revision"]
        snapshot = _inventory_after(
            bot,
            placed_at,
            lambda candidate: _instance_count(candidate, crate_instance_id) == 0,
            f"放置消费 instance={crate_instance_id}",
        )
        if snapshot["revision"] != crate_snapshot_revision + 1:
            raise BotAssertionError(
                "成功放置 trade_crate 必须精确 bump revision +1；"
                f"before={crate_snapshot_revision} actual={snapshot['revision']}"
            )

        opened_at = _event_watermark(bot)
        bot.intent(
            {"type": "container_open", "v": 1, "entity_id": spawn.data["entity_id"]}
        )
        opened = _server_data_after(bot, "loot_container_open", opened_at)
        payload = opened.data["payload"]
        session_id = int(payload["session_id"])
        if payload["source_kind"] != '{"storage_crate":{"is_herb":false}}':
            raise BotAssertionError(
                "trade_crate open 必须声明非灵草 storage_crate source_kind；"
                f"actual={payload['source_kind']!r}"
            )
        if (payload["rows"], payload["cols"]) != (4, 4):
            raise BotAssertionError(
                "trade_crate 权威 grid 必须为 4x4；"
                f"actual={payload['rows']}x{payload['cols']}"
            )
        if payload["placed_items"]:
            raise BotAssertionError(
                "新放置 trade_crate 初始必须为空；"
                f"actual={payload['placed_items']!r}"
            )
        if payload["timeout_wall_secs"] != 0:
            raise BotAssertionError(
                "普通 storage_crate 不得携带物资棺 wall-clock timeout；"
                f"actual={payload['timeout_wall_secs']}"
            )
        ext_container_id = f"ext_{session_id}"
        opened_snapshot = _inventory_after(bot, opened_at, lambda _candidate: True, "open 后背包同步")
        if opened_snapshot["revision"] != snapshot["revision"]:
            raise BotAssertionError(
                "只打开空货箱不得修改 inventory revision；"
                f"before={snapshot['revision']} actual={opened_snapshot['revision']}"
            )

        bot.cmd("give grass_fiber 1")
        bot.expect_chat("[dev] gave grass_fiber x1", timeout=10.0)
        player_snapshot = wait_inventory_revision_after_matching(
            bot,
            opened_snapshot["revision"],
            lambda candidate: find_item(candidate, "grass_fiber") is not None,
            "货箱打开期间出现 grass_fiber",
            timeout=10.0,
        )
        fiber = require_item(player_snapshot, "grass_fiber")
        fiber_instance_id = int(fiber["item"]["instance_id"])
        fiber_source = fiber["location"]
        _assert_total_instance_count(
            player_snapshot,
            [],
            fiber_instance_id,
            1,
            "give 后玩家侧唯一实例",
        )

        into_ext_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id,
                "instance_id": fiber_instance_id,
                "from": fiber_source,
                "to": container_location(ext_container_id, 1, 2),
            }
        )
        into_update = _server_data_after(bot, "loot_container_update", into_ext_at).data[
            "payload"
        ]
        into_placed = _require_external_instance(
            into_update,
            session_id,
            fiber_instance_id,
            row=1,
            col=2,
            context="player→external 成功",
        )
        into_snapshot = _inventory_after(
            bot,
            into_ext_at,
            lambda candidate: _instance_count(candidate, fiber_instance_id) == 0,
            f"player→external 后玩家侧 instance={fiber_instance_id} 消失",
        )
        if into_snapshot["revision"] != player_snapshot["revision"] + 1:
            raise BotAssertionError(
                "player→external 成功必须精确 bump revision +1；"
                f"before={player_snapshot['revision']} actual={into_snapshot['revision']}"
            )
        _assert_total_instance_count(
            into_snapshot,
            into_update["placed_items"],
            fiber_instance_id,
            1,
            "player→external 后全局唯一实例",
        )

        # 错误坐标只用于攻击 source assertion：instance 在 ext@1,2，却伪报 ext@0,0。
        # server 必须拒绝并原样回推，不能仅凭 instance_id 接受。
        forged_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id,
                "instance_id": fiber_instance_id,
                "from": container_location(ext_container_id, 0, 0),
                "to": container_location("body_pocket", 0, 0),
            }
        )
        forged_update = _server_data_after(
            bot, "loot_container_update", forged_at
        ).data["payload"]
        _require_external_instance(
            forged_update,
            session_id,
            fiber_instance_id,
            row=1,
            col=2,
            context="伪造 source 坐标拒绝",
        )
        forged_snapshot = _inventory_after(
            bot,
            forged_at,
            lambda candidate: _instance_count(candidate, fiber_instance_id) == 0,
            "伪造 source 坐标拒绝后的背包同步",
        )
        if forged_snapshot["revision"] != into_snapshot["revision"]:
            raise BotAssertionError(
                "伪造 external source 坐标被拒时 revision 必须不变；"
                f"before={into_snapshot['revision']} actual={forged_snapshot['revision']}"
            )
        _assert_total_instance_count(
            forged_snapshot,
            forged_update["placed_items"],
            fiber_instance_id,
            1,
            "伪造 source 坐标拒绝后无丢失复制",
        )

        destination_row, destination_col = first_free_cell(
            forged_snapshot,
            "body_pocket",
            into_placed["item"]["grid_width"],
            into_placed["item"]["grid_height"],
        )
        back_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id,
                "instance_id": fiber_instance_id,
                "from": container_location(ext_container_id, 1, 2),
                "to": container_location(
                    "body_pocket", destination_row, destination_col
                ),
            }
        )
        back_update = _server_data_after(bot, "loot_container_update", back_at).data[
            "payload"
        ]
        if back_update["session_id"] != session_id:
            raise BotAssertionError(
                "external→player update 必须回推当前 session；"
                f"expected={session_id} actual={back_update['session_id']}"
            )
        if any(
            placed["item"]["instance_id"] == fiber_instance_id
            for placed in back_update["placed_items"]
        ):
            raise BotAssertionError(
                f"external→player 后 instance={fiber_instance_id} 不得残留在货箱"
            )
        back_snapshot = _inventory_after(
            bot,
            back_at,
            lambda candidate: _instance_at_container(
                candidate,
                fiber_instance_id,
                "body_pocket",
                destination_row,
                destination_col,
            ),
            "external→player 后精确回到 body_pocket 目标格",
        )
        if back_snapshot["revision"] != forged_snapshot["revision"] + 1:
            raise BotAssertionError(
                "external→player 成功必须精确 bump revision +1；"
                f"before={forged_snapshot['revision']} actual={back_snapshot['revision']}"
            )
        _assert_total_instance_count(
            back_snapshot,
            back_update["placed_items"],
            fiber_instance_id,
            1,
            "external→player 后全局唯一实例",
        )

        wrong_session_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_move",
                "v": 1,
                "session_id": session_id + 1_000_000,
                "instance_id": fiber_instance_id,
                "from": container_location(
                    "body_pocket", destination_row, destination_col
                ),
                "to": container_location(ext_container_id, 0, 0),
            }
        )
        wrong_session_snapshot = _inventory_after(
            bot,
            wrong_session_at,
            lambda candidate: _instance_at_container(
                candidate,
                fiber_instance_id,
                "body_pocket",
                destination_row,
                destination_col,
            ),
            "unknown session move 拒绝后的背包同步",
        )
        if wrong_session_snapshot["revision"] != back_snapshot["revision"]:
            raise BotAssertionError(
                "unknown session move 被拒时 revision 必须不变；"
                f"before={back_snapshot['revision']} "
                f"actual={wrong_session_snapshot['revision']}"
            )

        closed_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_close",
                "v": 1,
                "session_id": session_id,
            }
        )
        closed = _server_data_after(bot, "loot_container_close", closed_at).data[
            "payload"
        ]
        if closed != {
            "v": 1,
            "type": "loot_container_close",
            "session_id": session_id,
            "reason": "player_closed",
        }:
            raise BotAssertionError(
                "显式关闭普通货箱必须返回精确 player_closed；"
                f"actual={closed!r}"
            )
        close_snapshot = _inventory_after(
            bot,
            closed_at,
            lambda candidate: _instance_at_container(
                candidate,
                fiber_instance_id,
                "body_pocket",
                destination_row,
                destination_col,
            ),
            "close 后背包权威同步",
        )
        if close_snapshot["revision"] != back_snapshot["revision"]:
            raise BotAssertionError(
                "关闭货箱不得修改 inventory revision；"
                f"before={back_snapshot['revision']} actual={close_snapshot['revision']}"
            )
        bot.assert_alive("世界货箱双向权威链路完成后")


def _event_watermark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _server_data_after(bot, payload_type: str, after: float, timeout: float = 10.0):
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.data.get("payload_type") == payload_type
        and event.t > after,
        timeout=timeout,
        description=f"t>{after:.3f}s 后 typed {payload_type}",
    )


def _inventory_after(bot, after: float, predicate, description: str, timeout: float = 10.0):
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.data.get("payload_type") == "inventory_snapshot"
        and event.t > after
        and predicate(event.data["payload"]),
        timeout=timeout,
        description=f"t>{after:.3f}s 且 {description} 的 inventory_snapshot",
    ).data["payload"]


def _placement_pos(bot) -> tuple[int, int, int]:
    if bot.position is None:
        raise BotAssertionError("container 场景需要 pos_look 后的位置，实际 position=None")
    x, y, z = bot.position
    return math.floor(x) + 2, math.floor(y), math.floor(z)


def _hotbar_instance_ids(snapshot: dict) -> list[int]:
    return [
        int(item["instance_id"])
        for item in snapshot.get("hotbar", [])
        if isinstance(item, dict)
    ]


def _instance_count(snapshot: dict, instance_id: int) -> int:
    count = sum(
        int(placed["item"]["instance_id"]) == instance_id
        for placed in snapshot.get("placed_items", [])
    )
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
    return count


def _instance_at_container(
    snapshot: dict, instance_id: int, container_id: str, row: int, col: int
) -> bool:
    return any(
        int(placed["item"]["instance_id"]) == instance_id
        and placed["container_id"] == container_id
        and placed["row"] == row
        and placed["col"] == col
        for placed in snapshot.get("placed_items", [])
    )


def _require_external_instance(
    payload: dict,
    session_id: int,
    instance_id: int,
    *,
    row: int,
    col: int,
    context: str,
) -> dict:
    if payload["session_id"] != session_id:
        raise BotAssertionError(
            f"{context} 必须回推 session={session_id}；actual={payload['session_id']}"
        )
    matches = [
        placed
        for placed in payload["placed_items"]
        if int(placed["item"]["instance_id"]) == instance_id
    ]
    if len(matches) != 1:
        raise BotAssertionError(
            f"{context} 必须恰有一个 external instance={instance_id}；"
            f"actual={matches!r} all={payload['placed_items']!r}"
        )
    placed = matches[0]
    expected_location = (f"ext_{session_id}", row, col)
    actual_location = (placed["container_id"], placed["row"], placed["col"])
    if actual_location != expected_location:
        raise BotAssertionError(
            f"{context} external 位置不符；"
            f"expected={expected_location} actual={actual_location}"
        )
    return placed


def _assert_total_instance_count(
    snapshot: dict,
    external_items: list[dict],
    instance_id: int,
    expected: int,
    context: str,
) -> None:
    actual = _instance_count(snapshot, instance_id) + sum(
        int(placed["item"]["instance_id"]) == instance_id
        for placed in external_items
    )
    if actual != expected:
        raise BotAssertionError(
            f"{context} instance={instance_id} 全局计数不符；"
            f"expected={expected} actual={actual}"
        )
