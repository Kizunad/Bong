"""物资棺跨维 session 门禁的真实 C2S → server → S2C 黑盒回归。"""

from __future__ import annotations

import math

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    first_free_cell,
    wait_inventory_snapshot_after,
    wait_join_and_inventory,
)

DESCRIPTION = "物资棺有效 session 跨维 move/open 均拒绝，resume 后再权威 close"
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
        open_probe_spawn_signature = {
            key: open_probe_spawn.data[key] for key in ("type", "x", "y", "z")
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

        pause_sent_at = _event_watermark(bot)
        bot.cmd("supply_coffin lifecycle pause")
        bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > pause_sent_at
            and "[dev] supply_coffin lifecycle paused for executor"
            in event.data["text"],
            timeout=10.0,
            description="物资棺 lifecycle 玩家级暂停确认",
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
        entered, tsy_position = _transfer_dimension_same_xyz(
            bot,
            target="tsy",
            after=transition_sent_at,
            coffin_spawn=coffin_spawn_signature,
            context="保持物资棺裸 XYZ 不变进入 TSY",
        )
        _assert_position_within_gate(
            tsy_position,
            open_probe_spawn_signature,
            max_distance=4.5,
            context="TSY forged open 的实际第二具目标棺",
        )

        forged_open_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "supply_coffin_open",
                "v": 1,
                "entity_id": open_probe_entity_id,
            }
        )
        forged_barrier = _supply_coffin_barrier(
            bot,
            after=forged_open_sent_at,
            context="forged open 处理完成",
        )
        _assert_no_server_data_between(
            bot,
            "loot_container_open",
            after=forged_open_sent_at,
            through=forged_barrier.t,
            context="TSY 玩家在 4.5 格内对第二具未占用 Overworld 棺 forged open",
        )

        # lifecycle 仍被玩家级 marker 暂停，因此 mapping、opened_by 与 active source
        # 都保持有效。此时通过真实 bot socket 发 move；收到 external update + inventory
        # snapshot 才能证明请求命中了 owner session 的 authority 拒绝，而非 unknown session。
        active_move_sent_at = _event_watermark(bot)
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
        rejected_update_event = _server_data_after(
            bot, "loot_container_update", active_move_sent_at, timeout=10.0
        )
        rejected_update = rejected_update_event.data["payload"]
        if rejected_update["session_id"] != session_id:
            raise BotAssertionError(
                "有效 session 跨维 move 拒绝必须回推原 external container；"
                f"expected={session_id} actual={rejected_update['session_id']}"
            )
        rejected_source = _require_placed_item(
            rejected_update["placed_items"],
            instance_id,
            context="有效 session 跨维 move 被拒后的权威棺内实例",
        )
        if rejected_source != authoritative_source:
            raise BotAssertionError(
                "有效 session 跨维 move 被拒后源实例位置与内容必须完全不变；"
                f"expected={authoritative_source} actual={rejected_source}"
            )
        rejected_snapshot = wait_inventory_snapshot_after(
            bot, rejected_update_event.t, timeout=10.0
        )
        _assert_instance_absent_from_inventory(rejected_snapshot, instance_id)
        if rejected_snapshot["revision"] != moved_snapshot["revision"]:
            raise BotAssertionError(
                "有效 session 跨维 move 被拒时 inventory revision 必须不变；"
                f"before={moved_snapshot['revision']} "
                f"actual={rejected_snapshot['revision']}"
            )

        post_move_barrier = _supply_coffin_barrier(
            bot,
            after=_event_watermark(bot),
            context="有效 session move 与 forged open 后置水位",
        )
        _assert_no_server_data_between(
            bot,
            "loot_container_open",
            after=forged_open_sent_at,
            through=post_move_barrier.t,
            context="forged open 到有效 session move 完成后的完整处理区间",
        )
        premature_closes = _server_data_payloads_between(
            bot,
            "loot_container_close",
            session_id,
            after=transition_sent_at,
            through=post_move_barrier.t,
        )
        if premature_closes:
            raise BotAssertionError(
                "lifecycle pause 期间原 session 必须保持有效供跨维 move 探测；"
                f"expected=[] actual={premature_closes}"
            )

        resume_sent_at = _event_watermark(bot)
        bot.cmd("supply_coffin lifecycle resume")
        resumed = bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > resume_sent_at
            and "[dev] supply_coffin lifecycle resumed for executor"
            in event.data["text"],
            timeout=10.0,
            description="物资棺 lifecycle 玩家级恢复确认",
        )
        closed = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "loot_container_close"
            and event.data["payload"]["session_id"] == session_id
            and event.t > resumed.t,
            timeout=10.0,
            description=(
                f"lifecycle resume watermark {resumed.t:.3f}s 后 session={session_id} "
                "的权威 loot_container_close"
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

        close_barrier = _supply_coffin_barrier(
            bot,
            after=_event_watermark(bot),
            context="lifecycle resume 后的 session cleanup 水位",
        )
        close_payloads = [
            event.data["payload"]
            for event in bot.events
            if event.kind == "server_data"
            and event.data["payload_type"] == "loot_container_close"
            and forged_open_sent_at < event.t <= close_barrier.t
        ]
        if close_payloads != [close_payload]:
            raise BotAssertionError(
                "forged open 不得创建会在 lifecycle resume 时泄露的隐藏 session；"
                f"expected={[close_payload]} actual={close_payloads}"
            )
        _assert_no_event_after(
            bot,
            "respawn",
            entered.t,
            timeout=0.75,
            context="同 XYZ dev transfer 后不得发生未请求的第二次跨维",
        )

        # 上一段已经通过玩家级 lifecycle pause 锁定有效 session 的真实网络 move。
        # close 后再发一次，只独立验证 session cleanup / unknown-session 幂等拒绝。
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
        _assert_no_server_data_between(
            bot,
            "loot_container_open",
            after=forged_open_sent_at,
            through=return_sent_at,
            context="forged open 请求到离开 TSY 前的完整事件区间",
        )
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
        reloaded_open_probe = bot.wait_for(
            lambda event: event.kind == "entity_spawn"
            and event.t > return_sent_at
            and event.data["entity_id"] == open_probe_entity_id
            and _same_spawn(event.data, open_probe_spawn_signature),
            timeout=10.0,
            description=(
                "返回 Overworld 后按原 entity_id、类型与坐标重新观察第二具物资棺 Marker"
            ),
        )
        reloaded_open_probe_entity_id = reloaded_open_probe.data["entity_id"]

        return_barrier = _supply_coffin_barrier(
            bot,
            after=_event_watermark(bot),
            context="返回 Overworld 后 forged open 最终水位",
        )
        _assert_no_server_data_between(
            bot,
            "loot_container_open",
            after=forged_open_sent_at,
            through=return_barrier.t,
            context="forged open 到返回主世界处理水位的完整事件区间",
        )

        # 第二具棺在 forged open 前没有 ExternalContainer。resume 水位只有原 session
        # 的 close，且返回后能首次创建紧邻 next session；两者共同证明攻击请求没有
        # 留下 opened_by/session/loot 副作用。
        probe_verify_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "supply_coffin_open",
                "v": 1,
                "entity_id": reloaded_open_probe_entity_id,
            }
        )
        probe_opened = _server_data_after(
            bot, "loot_container_open", probe_verify_sent_at, timeout=10.0
        ).data["payload"]
        if probe_opened["session_id"] != session_id + 1:
            raise BotAssertionError(
                "第二具棺首次合法 open 必须取得紧邻的新 session，证明 forged open 未分配会话；"
                f"expected={session_id + 1} actual={probe_opened['session_id']}"
            )
        if len(probe_opened["placed_items"]) < 2:
            raise BotAssertionError(
                "第二具 common 物资棺首次合法 open 必须生成完整 loot；"
                f"expected_count>=2 actual_count={len(probe_opened['placed_items'])}"
            )

        probe_close_sent_at = _event_watermark(bot)
        bot.intent(
            {
                "type": "external_container_close",
                "v": 1,
                "session_id": probe_opened["session_id"],
            }
        )
        probe_closed = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "loot_container_close"
            and event.data["payload"]["session_id"] == probe_opened["session_id"]
            and event.t > probe_close_sent_at,
            timeout=10.0,
            description="第二具棺首次合法 session 的 player_closed 回执",
        )
        if probe_closed.data["payload"]["reason"] != "player_closed":
            raise BotAssertionError(
                "第二具棺清理必须使用 player_closed；"
                f"actual={probe_closed.data['payload']}"
            )
        wait_inventory_snapshot_after(bot, probe_closed.t, timeout=10.0)

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
        and f"Queued /tpdim {target} within current XYZ gate." in event.data["text"],
        timeout=10.0,
        description=f"/tpdim {target} server 权威 transfer 排队反馈",
    )
    respawn = bot.wait_for(
        lambda event: event.kind == "respawn" and event.t > after,
        timeout=10.0,
        description=f"/tpdim {target} 触发真实跨维 Respawn",
    )
    expected_dimension = {
        "tsy": "bong:tsy",
        "overworld": "minecraft:overworld",
    }[target]
    actual_dimensions = {
        key: respawn.data.get(key)
        for key in ("dimension_type_name", "dimension_name")
    }
    if any(value != expected_dimension for value in actual_dimensions.values()):
        raise BotAssertionError(
            f"/tpdim {target} 的 Respawn 必须携带权威目标维度；"
            f"expected={expected_dimension} actual={actual_dimensions}"
        )
    pulse_position = bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t >= respawn.t,
        timeout=10.0,
        description=f"/tpdim {target} 对应 Respawn 之后的坐标确认脉冲",
    )
    position = bot.wait_for(
        lambda event: event.kind == "pos_look" and event.t > pulse_position.t,
        timeout=10.0,
        description=f"/tpdim {target} 坐标确认脉冲后的最终权威 PositionLook",
    )
    # PlayerPositionLook 的低三位分别控制 X/Y/Z 相对坐标；Valence 会合法地把
    # yaw/pitch 标成相对（0x18）。这里只要求用于门限证明的 XYZ 均为绝对值。
    relative_xyz_flags = position.data["flags"] & 0x07
    if relative_xyz_flags != 0:
        raise BotAssertionError(
            f"{context} 必须返回绝对 XYZ 的 PositionLook；"
            f"expected_xyz_flags=0 actual_xyz_flags={relative_xyz_flags} "
            f"raw_flags={position.data['flags']}"
        )

    expected_xyz = (
        coffin_spawn["x"] + (0.25 if target == "tsy" else 0.0),
        coffin_spawn["y"],
        coffin_spawn["z"],
    )
    actual_xyz = tuple(position.data[axis] for axis in ("x", "y", "z"))
    if math.dist(actual_xyz, expected_xyz) > 1.0e-6:
        raise BotAssertionError(
            f"/tpdim {target} 的最终 PositionLook 必须恢复精确 transfer target；"
            f"expected={expected_xyz} actual={actual_xyz}"
        )

    _assert_position_within_gate(
        position,
        coffin_spawn,
        max_distance=4.5,
        context=context,
    )
    return respawn, position


def _assert_position_within_gate(
    position, coffin_spawn: dict, max_distance: float, context: str
) -> None:
    actual_xyz = tuple(position.data[axis] for axis in ("x", "y", "z"))
    coffin_xyz = tuple(coffin_spawn[axis] for axis in ("x", "y", "z"))
    if not all(math.isfinite(value) for value in actual_xyz):
        raise BotAssertionError(
            f"{context} 的权威坐标必须有限；expected=finite actual={actual_xyz}"
        )
    actual_distance = math.dist(actual_xyz, coffin_xyz)
    if actual_distance > max_distance:
        raise BotAssertionError(
            f"{context} 必须保留旧 XYZ-only open 阈值内前提；"
            f"expected_distance<={max_distance} actual_distance={actual_distance:.6f} "
            f"player={actual_xyz} coffin={coffin_xyz}"
        )


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


def _supply_coffin_barrier(bot, after: float, context: str):
    bot.cmd("supply_coffin barrier")
    return bot.wait_for(
        lambda event: event.kind == "chat"
        and event.t > after
        and "[dev] supply_coffin barrier passed" in event.data["text"],
        timeout=10.0,
        description=f"{context}的 server-authoritative supply_coffin barrier",
    )


def _assert_no_server_data_between(
    bot, payload_type: str, after: float, through: float, context: str
) -> None:
    unexpected = [
        event
        for event in bot.events
        if event.kind == "server_data"
        and event.data["payload_type"] == payload_type
        and after < event.t <= through
    ]
    if unexpected:
        raise BotAssertionError(
            f"{context} 不得产生 {payload_type}；"
            f"window=({after:.3f},{through:.3f}] actual={unexpected}"
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
