"""`bong:client_request` 过期 / 重放 session token —— session 权威门禁干净拒绝。

`external_container_move` / `external_container_close` 的 `session_id` 指向服务端
`ExternalContainerRegistry::sessions`（`client_request_handler.rs`
`handle_external_container_move`）。本场景覆盖两类坏 token：

1. **真实 session 的 closed token（重放）**：先用 container_open 打开一个真实外部
   容器 session（server 发放 session_id），close 成功（server 回推
   `loot_container_close` 证明该 token 曾有效），然后**重放这个已关闭的
   session_id** 发 move / close —— server 必须干净拒绝：move 回推背包 resync
   快照且**零 mutation**（快照指纹 revision+内容与请求前一致），且**没有**
   `loot_container_update`（成功路径响应不能出现）；close 干净 no-op（不再回推
   close 事件）。这锁定的是"曾有效、现已关闭"的 token 不再被接受。
2. **forged token（从未发放）**：高位全 1 的 session_id 不可能被分配过 —— 同样
   干净拒绝：move resync + 零 mutation，close no-op。

拒绝响应的可观测契约：resync 快照的指纹（revision + 内容）与请求前**完全一致**
（server 只在背包内容突变时 bump revision，单纯重发快照不改 revision；故指纹
相同 ⇒ 零 mutation）。注意 `reason=external_container_resync` 只是 server 日志
标注 —— InventorySnapshotV1 的 wire 上没有 reason 字段，零 mutation 必须靠
revision + 内容指纹来断言。

连接状态：整个场景连接保持，心跳连续，最后合法请求仍被正常处理。
"""

import math
import time

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 过期/重放 external-container session token 干净拒绝并 resync 背包"
MODULES = ["network"]

# 伪造 session token：u64 高位全 1，不可能是服务端分配的真实 session id。
FORGED_SESSION_ID = 0xFFFF_FFFF_FFFF_FF00
FORGED_INSTANCE_ID = 0xDEAD_BEEF_CAFE_0001


def _move_request(session_id: int) -> dict:
    return {
        "v": 1,
        "type": "external_container_move",
        "session_id": session_id,
        "instance_id": FORGED_INSTANCE_ID,
        "from": {
            "kind": "container",
            "container_id": f"ext_{session_id}",
            "row": 0,
            "col": 0,
        },
        "to": {"kind": "container", "container_id": "main_pack", "row": 0, "col": 0},
    }


def _close_request(session_id: int) -> dict:
    return {"v": 1, "type": "external_container_close", "session_id": session_id}


def _has_any_chunk(bot) -> bool:
    # 5s 留足 raster 首块流式送达余量：真实 session leg 依赖可放置 chunk。
    try:
        bot.wait_for(lambda e: e.kind == "chunk_data", timeout=5.0, description="任意 ChunkData")
        return True
    except BotAssertionError:
        return False


def _placement_pos(bot) -> tuple[int, int, int]:
    if bot.position is None:
        raise BotAssertionError("stale session 场景需要 pos_look 后的位置，实际 position=None")
    x, y, z = bot.position
    # 放在玩家东侧两格的脚下空气格，避免碰撞体与玩家包围盒相交。
    return math.floor(x) + 2, math.floor(y), math.floor(z)


def _open_real_session(bot) -> int | None:
    """创建并打开一个真实外部容器 session，返回其 session_id（供 replay leg 用）。

    路径与 `inventory_container_open_minimal` 一致：`/give trade_crate` →
    `block_place` → `container_open` → server 发放 `loot_container_open`。
    若当前环境没有可放置 chunk / 未观察到 Marker，显式打印跳过并返回 None
    （环境缺口，不误判成协议回归）—— 剩余 forged leg 仍完整覆盖"从未发放的
    token"。
    """
    from ._inventory_helpers import (
        latest_inventory_snapshot,
        require_item,
        wait_inventory_contains,
        wait_inventory_revision_after,
    )

    if not _has_any_chunk(bot):
        print("    [warn] 当前出生点无 ChunkData，跳过真实 session leg（closed-token 重放）")
        bot.assert_alive("无 chunk 跳过真实 session leg 前")
        return None

    snapshot = latest_inventory_snapshot(bot)
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
    snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

    bot.cmd("give trade_crate 1")
    bot.expect_chat("[dev] gave trade_crate x1", timeout=10.0)
    snapshot = wait_inventory_contains(bot, "trade_crate", timeout=10.0)
    crate = require_item(snapshot, "trade_crate")

    x, y, z = _placement_pos(bot)
    sent_at = bot.events[-1].t if bot.events else 0.0
    bot.intent(
        {
            "type": "block_place",
            "v": 1,
            "x": x,
            "y": y,
            "z": z,
            "item_instance_id": crate["item"]["instance_id"],
            "target_face": "north",
        }
    )

    try:
        spawn = bot.wait_for(
            lambda e: e.kind == "entity_spawn"
            and e.t > sent_at
            and abs(e.data["x"] - (x + 0.5)) <= 1.5
            and abs(e.data["y"] - y) <= 2.0
            and abs(e.data["z"] - (z + 0.5)) <= 1.5,
            timeout=10.0,
            description="trade_crate 放置后附近出现容器 Marker entity_spawn",
        )
    except BotAssertionError:
        print("    [warn] 未观察到 trade_crate Marker，跳过真实 session leg（closed-token 重放）")
        bot.assert_alive("容器放置未观察到 marker 后")
        return None

    bot.intent({"type": "container_open", "v": 1, "entity_id": spawn.data["entity_id"]})
    opened = bot.expect_server_data("loot_container_open", timeout=10.0)
    bot.assert_alive("container_open 打开真实 session 后")
    return opened.data["payload"]["session_id"]


def _assert_stale_move_rejected_zero_mutation(bot, session_id: int, label: str) -> None:
    """stale / 重放 external_container_move 必须被干净拒绝：resync 回推 + 零 mutation。

    pre 指纹与锚点在同一时刻捕获（发请求**之前**），resync 必须严格晚于锚点 →
    必然是本次请求触发的新快照；指纹（revision + 内容）与 pre 一致即证明请求
    没有造成任何背包 mutation。
    """
    from ._inventory_helpers import latest_inventory_snapshot, wait_inventory_snapshot_after
    from ._rejection_helpers import inventory_fingerprint

    before = bot.events[-1].t if bot.events else 0.0
    pre = latest_inventory_snapshot(bot)
    bot.intent(_move_request(session_id))
    resync = wait_inventory_snapshot_after(bot, before, timeout=10.0)
    if inventory_fingerprint(resync) != inventory_fingerprint(pre):
        raise BotAssertionError(
            f"{label}：期望 resync 回推背包与请求前完全一致（零 mutation），"
            f"实际 pre={inventory_fingerprint(pre)} resync={inventory_fingerprint(resync)}"
        )


def run(env) -> None:
    from ._inventory_helpers import latest_inventory_snapshot, wait_inventory_snapshot_after
    from ._rejection_helpers import (
        assert_no_server_data_payload_since,
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Stl") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        # 等 join 时的初始快照突发放完，避免把 join snapshot 误判成拒绝响应。
        time.sleep(1.0)

        # ---- 1. 真实 session 生命周期：open → close → replay（closed token 重放）
        session_id = _open_real_session(bot)
        if session_id is not None:
            # 证明 token 曾有效：close 被接受，server 回推 loot_container_close。
            # 同时断言 close 后的 resync 快照零 mutation（指纹与 close 前一致）。
            close_before = bot.events[-1].t if bot.events else 0.0
            pre_close = latest_inventory_snapshot(bot)
            bot.intent(_close_request(session_id))
            bot.expect_server_data("loot_container_close", timeout=10.0)
            resync = wait_inventory_snapshot_after(bot, close_before, timeout=10.0)
            if inventory_fingerprint(resync) != inventory_fingerprint(pre_close):
                raise BotAssertionError(
                    "真实 session close：期望 close 后 resync 背包零 mutation，"
                    f"实际 pre={inventory_fingerprint(pre_close)} "
                    f"resync={inventory_fingerprint(resync)}"
                )
            bot.assert_alive("真实 session close 后")

            # replay 已关闭 session 的 move —— 必须被干净拒绝：resync + 零 mutation，
            # 且**没有** loot_container_update（成功路径响应不能出现）。
            replay_before = bot.events[-1].t if bot.events else 0.0
            pre_replay = latest_inventory_snapshot(bot)
            bot.intent(_move_request(session_id))
            resync = wait_inventory_snapshot_after(bot, replay_before, timeout=10.0)
            if inventory_fingerprint(resync) != inventory_fingerprint(pre_replay):
                raise BotAssertionError(
                    "replay 已关闭 session 的 move：期望零 mutation 拒绝，"
                    f"实际 pre={inventory_fingerprint(pre_replay)} "
                    f"resync={inventory_fingerprint(resync)}"
                )
            assert_no_server_data_payload_since(
                bot, replay_before, "loot_container_update", "replay 已关闭 session 的 move"
            )
            bot.assert_alive("replay 已关闭 session 的 move 拒绝后")

            # replay 已关闭 session 的 close —— 干净 no-op：不再回推 close 事件。
            replay_close_before = bot.events[-1].t if bot.events else 0.0
            bot.intent(_close_request(session_id))
            time.sleep(1.0)
            assert_no_server_data_payload_since(
                bot,
                replay_close_before,
                "loot_container_close",
                "replay 已关闭 session 的 close",
            )
            bot.assert_alive("replay 已关闭 session 的 close 后")

        # ---- 2. forged（从未发放的 token）—— 权威门禁同样干净拒绝
        _assert_stale_move_rejected_zero_mutation(
            bot, FORGED_SESSION_ID, "stale move #1（forged token）"
        )
        bot.assert_alive("stale move #1 拒绝响应后")

        # stale move #2（重放同一 forged token）—— 同样干净拒绝，连接不坏。
        _assert_stale_move_rejected_zero_mutation(
            bot, FORGED_SESSION_ID, "stale move #2（重放 forged token）"
        )
        bot.assert_alive("stale move #2 重放拒绝后")

        # stale close（forged token）—— 未知 token 干净 no-op，不再回推 close 事件。
        stale_close_before = bot.events[-1].t if bot.events else 0.0
        bot.intent(_close_request(FORGED_SESSION_ID))
        time.sleep(1.0)
        assert_no_server_data_payload_since(
            bot, stale_close_before, "loot_container_close", "stale close（forged token）"
        )
        bot.assert_alive("stale close 后")

        fire_probes_and_keep_connection(
            bot,
            "stale session",
            [("重放 close", lambda: bot.intent(_close_request(FORGED_SESSION_ID)))],
        )
        assert_valid_request_still_works(bot)
