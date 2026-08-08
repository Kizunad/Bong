"""连接生命周期：陈旧 session 跨多次重连的幂等恢复。

流程：开一份 active craft session（材料已预扣）→ 异常断线 → 重连验证 session
恢复且**冻结**（不推进、不重建、不重复）→ 不取消再断线 → 再次重连，验证仍是
同一份陈旧 session（elapsed 不倒退、completed 不推进、recipe/qty 一致）→
取消 → 恰好一次退款。

与 production_craft_disconnect_resume 的区别：那一条只做一次重连；本条把陈旧
session 再断线再重连一次，锁住「多次重连不把 session 复制/重置/推进」的幂等性。
"""

import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "陈旧 craft session 跨两次重连幂等恢复：不复制/不重置/断线不推进"
MODULES = ["network", "craft", "inventory", "persistence"]

RECIPE_ID = "workbench.weapon.stone_knife"


def _wait_active_session(bot, timeout: float = 10.0) -> dict:
    """取重连后第一份 active craft_session_state。

    server 在 join 时对 restored session 推一帧 `session_state::join` payload（elapsed
    为持久化冻结值）；此后每 20 tick 的周期推送才开始推进。所以必须读**最早**那一份，
    否则会读到已推进若干 tick 的进度、把「冻结恢复」误判成「继续推进」。
    """
    existing = [
        e
        for e in bot.events_of("server_data")
        if e.data["payload_type"] == "craft_session_state"
        and e.data["payload"].get("active") is True
    ]
    if existing:
        return existing[0].data["payload"]
    event = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "craft_session_state"
        and e.data["payload"].get("active") is True,
        timeout=timeout,
        description="active craft session",
    )
    return event.data["payload"]


def _assert_session_identity(restored: dict, context: str) -> None:
    assert restored.get("recipe_id") == RECIPE_ID, (
        f"{context}：重连应恢复 recipe={RECIPE_ID}，实际 {restored!r}"
    )
    assert restored.get("total_count") == 2, (
        f"{context}：重连应恢复 quantity=2，实际 {restored!r}"
    )


def _expect_restored_session_frozen(
    bot, elapsed_before: int, completed_before: int, context: str
) -> dict:
    """第一次重连：elapsed 应冻结在断连前值（join 帧即恢复快照，尚未开始推进）。"""
    restored = _wait_active_session(bot)
    _assert_session_identity(restored, context)
    elapsed = restored.get("elapsed_ticks", -1)
    assert elapsed_before <= elapsed <= elapsed_before + 5, (
        f"{context}：陈旧 session 必须冻结在断连前进度（既不重置为 0 也不推进），"
        f"断线前 elapsed={elapsed_before}，恢复={restored!r}"
    )
    assert restored.get("completed_count") == completed_before, (
        f"{context}：陈旧 session 断线期间 completed_count 不得推进或回退；"
        f"断线前={completed_before}，恢复={restored!r}"
    )
    return restored


def _expect_restored_session_idempotent(
    bot, elapsed_before: int, completed_before: int, context: str
) -> dict:
    """第二次重连：会话不得被复制/重置/回退。

    注意：连接期间 active session 会正常推进 elapsed（20 tick 推一次进度），
    所以这里不断言严格冻结，只锁住「没有重建/回退」这一幂等不变量——elapsed 不会
    低于首次断连值、recipe/qty/completed 保持一致。session 若被复制或重置，取消时
    退款计数会露馅（见后段 refund 恰好一次断言）。
    """
    restored = _wait_active_session(bot)
    _assert_session_identity(restored, context)
    elapsed = restored.get("elapsed_ticks", -1)
    assert elapsed >= elapsed_before, (
        f"{context}：陈旧 session 不得被重置回退（多次重连仍应是同一份进度）；"
        f"首次断连 elapsed={elapsed_before}，恢复={restored!r}"
    )
    assert restored.get("completed_count") == completed_before, (
        f"{context}：陈旧 session 跨重连 completed_count 不得推进或回退；"
        f"首次断连={completed_before}，恢复={restored!r}"
    )
    return restored


def _start_craft(bot) -> tuple[int, int]:
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv", timeout=10.0)
    bot.cmd("give stone_chunk 2")
    bot.expect_chat("[dev] gave stone_chunk x2", timeout=10.0)
    bot.cmd("give wood_handle 2")
    bot.expect_chat("[dev] gave wood_handle x2", timeout=10.0)
    wait_inventory_contains(bot, "stone_chunk")
    wait_inventory_contains(bot, "wood_handle")
    time.sleep(1.0)

    anchor = last_event_time(bot)
    bot.intent(
        {
            "type": "craft_start",
            "v": 1,
            "recipe_id": RECIPE_ID,
            "quantity": 2,
        }
    )
    session = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "craft_session_state"
        and e.t > anchor
        and e.data["payload"].get("active") is True
        and e.data["payload"].get("elapsed_ticks", 0) >= 20,
        timeout=10.0,
        description="断线前已推进至少 20 tick 的 craft session",
    ).data["payload"]
    return session["elapsed_ticks"], session["completed_count"]


def run(env) -> None:
    # ---- 第一段连接：开出 session 并推进，然后异常断线 ----
    with env.new_bot("Stale") as bot:
        wait_join_and_inventory(bot)
        elapsed, completed = _start_craft(bot)

    time.sleep(1.5)  # 等 server 检测断连并落盘

    # ---- 第二段连接：验证 session 恢复且冻结，不取消再断线 ----
    with env.new_bot("Stale") as bot:
        wait_join_and_inventory(bot)
        _expect_restored_session_frozen(bot, elapsed, completed, "第一次重连")

    time.sleep(1.5)

    # ---- 第三段连接：验证陈旧 session 第二次重连后仍幂等（不复制/不重置/不回退）----
    with env.new_bot("Stale") as bot:
        restored_inventory = wait_join_and_inventory(bot)
        _expect_restored_session_idempotent(bot, elapsed, completed, "第二次重连")

        anchor = last_event_time(bot)
        bot.intent({"type": "craft_cancel", "v": 1})
        outcome = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "craft_outcome"
            and e.t > anchor
            and e.data["payload"].get("recipe_id") == RECIPE_ID,
            timeout=10.0,
            description="陈旧 session 取消 outcome",
        ).data["payload"]
        assert outcome.get("outcome") == "failed", f"取消应 failed，实际 {outcome!r}"
        assert outcome.get("reason") == 1, f"取消 reason 应=1，实际 {outcome!r}"
        assert outcome.get("material_returned") == 2, (
            f"取消应恰好返还两份材料（陈旧 session 只退一次），实际 {outcome!r}"
        )
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")

    time.sleep(1.5)

    # ---- 第四段连接：验证退款恰好一次（再次重连不重复退）----
    with env.new_bot("Stale") as bot:
        final_inventory = wait_join_and_inventory(bot)
        assert find_item(final_inventory, "stone_chunk") is not None, (
            f"最终应持有 stone_chunk，实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "wood_handle") is not None, (
            f"最终应持有 wood_handle，实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "stone_chunk")["item"]["stack_count"] == 1, (
            f"stone_chunk 退款必须恰好一次（stack_count=1），实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "wood_handle")["item"]["stack_count"] == 1, (
            f"wood_handle 退款必须恰好一次（stack_count=1），实际 {final_inventory!r}"
        )
        bot.assert_alive("陈旧 session 退款后再次重连")
