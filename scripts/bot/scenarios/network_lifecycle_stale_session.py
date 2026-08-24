"""连接生命周期：陈旧 session 跨多次重连的幂等恢复。

流程：开一份 active craft session（材料已预扣）→ 异常断线 → 重连验证 session
恢复且**冻结**（不推进、不重建、不重复）→ 不取消再断线 → 再次重连，验证仍是
同一份陈旧 session（elapsed 不倒退也不凭空增长、completed 不推进、recipe/qty
一致）→ 取消 → 恰好一次退款（材料 70% 向下取整：quantity=2 每种退
floor(2×0.7)=1 份，material_returned==2 是两种材料合计 1+1，不是每种 2 份）。

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
# 第二次重连 join 帧 elapsed 的上界容差（相对「断连前最新可见 push」）：craft_emit
# 每 SESSION_STATE_PUSH_INTERVAL_TICKS=20 tick 才推一次进度，断连前捕获的最新 push
# 最多陈旧 19 tick；加上断连检测/落盘容差（首次重连的 +5 同量级）与 1 tick 余量。
# 断线期间 session 不推进（tick_craft_sessions 只对 With<Client> 推进），join 帧
# 反映的只是断连瞬间的持久化值，故上界即「20 tick push 间隔 + 落盘容差」——
# 恢复实现每次重连凭空加 elapsed（如 +100）的坏路径在此必红
# （central review 31444073731 #2）。
RESTORED_SESSION_ELAPSED_BOUND = 25


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


def _latest_session_state(bot) -> dict:
    """取当前事件历史中**最新**的 active craft_session_state（断连前最后可见进度）。

    与 _wait_active_session 相对：那一个取 join 帧（持久化冻结值，最早的 active
    payload），本函数取断连前的最后推进值——两次重连断言必须锚定「断连前最新可见
    状态」，否则连接期间的合法推进（join 帧后 session 正常恢复运行）会被误判成
    跨重连的推进（central review 31444073731 #1）。
    """
    latest = None
    for e in bot.events_of("server_data"):
        if (
            e.data["payload_type"] == "craft_session_state"
            and e.data["payload"].get("active") is True
        ):
            latest = e.data["payload"]
    assert latest is not None, "连接期间应有 active craft_session_state（join 帧必推）"
    return latest


def _assert_session_identity(restored: dict, context: str) -> None:
    assert restored.get("recipe_id") == RECIPE_ID, (
        f"{context}：重连应恢复 recipe={RECIPE_ID}，实际 {restored!r}"
    )
    assert restored.get("total_count") == 2, (
        f"{context}：重连应恢复 quantity=2，实际 {restored!r}"
    )


def _assert_inactive_session_state(state: dict, context: str) -> None:
    """最终重连必须从 durable store 观察到已清空的 craft session。"""
    assert state.get("active") is False, (
        f"{context} 必须收到 active=false craft_session_state，实际 {state!r}"
    )
    assert state.get("recipe_id") in (None, ""), (
        f"{context} 的 inactive craft_session_state 必须清除 recipe_id，实际 {state!r}"
    )
    for field in ("elapsed_ticks", "total_ticks", "completed_count", "total_count"):
        assert state.get(field) == 0, (
            f"{context} 的 inactive craft_session_state 必须清除 {field}，"
            f"实际 {state!r}"
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
    bot, elapsed_floor: int, completed_before: int, context: str
) -> dict:
    """第二次重连：会话不得被复制/重置/回退，断线期间不得推进。

    两个边界都锚定「第二次断连前最新可见状态」（elapsed_floor/completed_before 由
    调用方在断连前捕获）：
    - elapsed 不得回退（>= elapsed_floor）也不得在断线期间凭空增长
      （<= elapsed_floor + RESTORED_SESSION_ELAPSED_BOUND，见该常量注释）——
      旧实现只有下界，恢复路径每次重连凭空 +100 elapsed 的坏实现全过
      （central review 31444073731 #2）；
    - completed_count 与断连前捕获一致：连接期间 session 会正常推进（join 帧后
      恢复运行，跨过完成边界时 completed_count 合法增长），所以不能拿首次断连的
      冻结值来比（central review 31444073731 #1）；断线期间无 Client 不推进，
      join 帧反映的正是断连瞬间的持久化值，故与断连前捕获相等是契约保证的。
    session 若被复制或重置，取消时退款计数会露馅（见后段 refund 恰好一次断言）。
    """
    restored = _wait_active_session(bot)
    _assert_session_identity(restored, context)
    elapsed = restored.get("elapsed_ticks", -1)
    assert elapsed_floor <= elapsed <= elapsed_floor + RESTORED_SESSION_ELAPSED_BOUND, (
        f"{context}：陈旧 session 断线期间不得回退也不得推进（冻结契约），"
        f"断连前 elapsed={elapsed_floor}（上界 +{RESTORED_SESSION_ELAPSED_BOUND} "
        f"覆盖 push 间隔陈旧），恢复={restored!r}"
    )
    assert restored.get("completed_count") == completed_before, (
        f"{context}：陈旧 session 断线期间 completed_count 不得推进或回退；"
        f"断连前={completed_before}，恢复={restored!r}"
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
    # 进度推送按 20 tick 一拍（craft_emit::SESSION_STATE_PUSH_INTERVAL_TICKS），
    # 首个 elapsed>=20 的 push 最坏要等 39 tick；宿主负载高 TPS 掉到 ~3 时
    # 这是 ~13s 的墙钟时间，10s 超时不够。20s 只放宽环境容忍度，断言不变。
    session = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "craft_session_state"
        and e.t > anchor
        and e.data["payload"].get("active") is True
        and e.data["payload"].get("elapsed_ticks", 0) >= 20,
        timeout=20.0,
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
        # 断连前捕获最新可见进度：恢复的 session 在 join 帧后会正常推进（elapsed
        # 越过 join 帧，跨过完成边界时 completed_count 合法增长），第三次连接的
        # 幂等断言必须锚定「断连前最新状态」——对 join 帧比较会把连接期间的合法
        # 推进误判成跨重连推进（central review 31444073731 #1）。
        pre_disconnect = _latest_session_state(bot)

    time.sleep(1.5)

    # ---- 第三段连接：验证陈旧 session 第二次重连后仍幂等（不复制/不重置/不回退）----
    # 下界与上界都锚定「第二次断连前最新可见状态」：第二次重连不得回退到更早进度
    # （>= pre_disconnect），也不得在断线期间凭空推进（<= pre_disconnect +
    # RESTORED_SESSION_ELAPSED_BOUND）——「每次重连 +100 elapsed」的伪持久化路径
    # 在此必红（central review 31444073731 #2）。
    with env.new_bot("Stale") as bot:
        restored_inventory = wait_join_and_inventory(bot)
        _expect_restored_session_idempotent(
            bot,
            pre_disconnect.get("elapsed_ticks"),
            pre_disconnect.get("completed_count"),
            "第二次重连",
        )

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
        # material_returned 是 refund_manifest 的**跨材料合计**（session.rs cancel_craft：
        # 每材料 floor(reserved × CANCEL_REFUND_RATIO=0.7)，quantity=2 每材料 1 份，
        # 合计 1+1=2）——不是「每种 2 份」。
        assert outcome.get("material_returned") == 2, (
            f"取消 material_returned 应=2（每种 70% 向下取整=1 份，合计 1+1；"
            f"陈旧 session 只退一次），实际 {outcome!r}"
        )
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")

    time.sleep(1.5)

    # ---- 第四段连接：验证退款恰好一次（再次重连不重复退）----
    with env.new_bot("Stale") as bot:
        final_inventory = wait_join_and_inventory(bot)
        final_session = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "craft_session_state",
            timeout=10.0,
            description="取消退款后的最终 join-time craft_session_state",
        ).data["payload"]
        _assert_inactive_session_state(
            final_session,
            "取消退款后的最终重连",
        )
        assert find_item(final_inventory, "stone_chunk") is not None, (
            f"最终应持有 stone_chunk，实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "wood_handle") is not None, (
            f"最终应持有 wood_handle，实际 {final_inventory!r}"
        )
        # 取消退款契约 = 材料 70% 向下取整（session.rs CANCEL_REFUND_RATIO=0.7）：
        # quantity=2 每种材料退 floor(2×0.7)=1 份。stack_count==1 既钉住退款量（每种
        # 恰好 1 份，多退成 2 / 少退成 0 均红），又是「恰好一次」的检测面——退款若跨
        # 重连被重复执行，第二份会叠加成 stack_count=2。
        assert find_item(final_inventory, "stone_chunk")["item"]["stack_count"] == 1, (
            f"stone_chunk 退款必须恰好一次（quantity=2 材料 70% 向下取整退 1 份，"
            f"stack_count=1；重复退款会叠加成 2），实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "wood_handle")["item"]["stack_count"] == 1, (
            f"wood_handle 退款必须恰好一次（quantity=2 材料 70% 向下取整退 1 份，"
            f"stack_count=1；重复退款会叠加成 2），实际 {final_inventory!r}"
        )
        bot.assert_alive("陈旧 session 退款后再次重连")
