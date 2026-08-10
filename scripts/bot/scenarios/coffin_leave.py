"""GAP15 —— CoffinLeave 出延寿棺全链路（W14 覆盖率名册 Tier A）。

黑盒契约面 = `server/src/coffin/mod.rs` `handle_coffin_leave_requests`：
- 成功：`set_invisible(false)` + 位置回 `coffin_exit_position`（lower-0.5, +0.05,
  +0.5）+ 移除 CoffinComponent + `occupied_by` 清 + 推
  `CoffinState{in_coffin:false, grade:缺席, multiplier:1.0}`。
- 拒绝全静默（无回执）：从未进棺（无 CoffinComponent）直接 continue。

断言面（state 读回而非响应 OK）：
1. 未进过 leave——静默窗口无任何 CoffinState 变化（CoLvB 只收过 join 初始快照）。
2. 正向 leave——CoffinState 三字段（grade 缺席、multiplier 回 1.0）+
   **entity_metadata invisible 位清除**（隐藏标记恢复）+ **pos_look 回
   coffin_exit_position**（服务端权威位置读回）。
3. 幂等 leave——第二次静默（组件已移除，无二次状态突变）。
4. **occupied_by 清的行为级读回**——CoLvB 随即对同一棺 CoffinEnter **成功**
   （in_coffin:true + invisible + 位置棺内）；这是比字段断言更强的状态证据：
   只有 `registry.clear_player` 真的清了 occupied_by，B 才能进得去。
5. CoLvB leave 收尾还原（场景不留脏状态）。
"""

import math

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = (
    "CoffinLeave 出延寿棺：CoffinState 三字段 + invisible 位清除 + 位置还原 + "
    "幂等/未进过双静默 + occupied_by 清后第二 bot 可进（行为级读回）"
)
MODULES = ["coffin"]

MUNDANE_COFFIN = "mundane_coffin"
MUNDANE_MULTIPLIER = 0.9
PLACE_OFFSET = 2
ENTER_RANGE = 5.0


def _approx(a: float, b: float, tol: float = 0.01) -> bool:
    return math.isclose(a, b, abs_tol=tol)


def _coffin_state_after(bot, anchor, predicate, timeout, description):
    return bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "coffin_state"
        and e.t > anchor
        and predicate(e.data["payload"]),
        timeout=timeout,
        description=description,
    )


def _assert_silent_window(bot, anchor, label):
    """anchor 后 2s 内不得出现新 CoffinState 或实体 metadata 更新。"""
    import time

    time.sleep(2.0)
    with bot._lock:
        hits = [
            e
            for e in bot.events
            if e.t > anchor
            and (
                (e.kind == "server_data" and e.data["payload_type"] == "coffin_state")
                or e.kind == "entity_metadata"
            )
        ]
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 应静默（无状态变化），实际窗口内 {len(hits)} 条: {hits[:3]}"
        )


def _enter_coffin(bot, lower, anchor=None, expect_success=True):
    anchor = anchor if anchor is not None else last_event_time(bot)
    bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
    if expect_success:
        _coffin_state_after(
            bot,
            anchor,
            lambda p: p["in_coffin"] is True
            and p["coffin_grade"] == "mundane"
            and math.isclose(p["lifespan_rate_multiplier"], MUNDANE_MULTIPLIER, abs_tol=1e-9),
            timeout=10.0,
            description="进棺应推 CoffinState{in_coffin:true, grade:mundane, multiplier:0.9}",
        )


def run(env) -> None:
    with env.new_bot("CoLv") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)
        bot.cmd(f"give {MUNDANE_COFFIN} 1")
        snapshot = wait_inventory_contains(bot, MUNDANE_COFFIN)
        coffin = require_item(snapshot, MUNDANE_COFFIN)

        assert bot.position is not None, "需要 pos_look 后的 bot.position 定放棺位"
        px, py, pz = (int(v) for v in bot.position)
        lower = (px - PLACE_OFFSET, py, pz)
        in_coffin_pos = (lower[0] + 0.5, lower[1] + 0.05, lower[2] + 0.5)
        exit_pos = (lower[0] - 0.5, lower[1] + 0.05, lower[2] + 0.5)

        # ── setup：放棺 + 进棺，建立 in-coffin 状态 ────────────────────────
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_place",
                "v": 1,
                "x": lower[0],
                "y": lower[1],
                "z": lower[2],
                "item_instance_id": int(coffin["item"]["instance_id"]),
            }
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], MUNDANE_COFFIN) is None,
            timeout=30.0,
            description="真实 instance_id 放棺后 mundane_coffin 应从背包消耗",
        )
        _enter_coffin(bot, lower)

        with env.new_bot("CoLvB") as bystander:
            wait_for_ready(bystander)

            # ── 未进过棺的 leave 静默（负向） ──────────────────────────────
            b_anchor = last_event_time(bystander)
            bystander.intent({"type": "coffin_leave", "v": 1})
            _assert_silent_window(bystander, b_anchor, "从未进棺的 CoffinLeave")

            # ── 正向 leave：三路状态读回 ───────────────────────────────────
            anchor = last_event_time(bot)
            bot.intent({"type": "coffin_leave", "v": 1})
            _coffin_state_after(
                bot,
                anchor,
                lambda p: p["in_coffin"] is False
                and p["coffin_grade"] is None
                and math.isclose(p["lifespan_rate_multiplier"], 1.0, abs_tol=1e-9),
                timeout=10.0,
                description=(
                    "出棺应推 CoffinState{in_coffin:false, grade:缺席, multiplier:1.0}"
                ),
            )
            bot.wait_for(
                lambda e: e.kind == "entity_metadata"
                and e.t > anchor
                and e.data["entity_id"] == bot.entity_id
                and e.data["flags"] is not None
                and not e.data["flags"] & 0x20,
                timeout=10.0,
                description="出棺后 invisible 位应清除——隐藏标记恢复",
            )
            bot.wait_for(
                lambda e: e.kind == "pos_look"
                and e.t > anchor
                and _approx(e.data["x"], exit_pos[0])
                and _approx(e.data["y"], exit_pos[1])
                and _approx(e.data["z"], exit_pos[2]),
                timeout=10.0,
                description=f"出棺后位置应回 coffin_exit_position {exit_pos}",
            )

            # ── 幂等 leave：第二次静默（组件已移除） ───────────────────────
            anchor = last_event_time(bot)
            bot.intent({"type": "coffin_leave", "v": 1})
            _assert_silent_window(bot, anchor, "二次 CoffinLeave（幂等）")

            # ── occupied_by 清的行为级读回：B 随即进同一棺必须成功 ─────────
            bx, by, bz = bystander.position
            distance = math.dist((bx, by, bz), (px, py, pz))
            if distance > ENTER_RANGE:
                bystander.move_to(px, py, pz, speed=5.5)
                import time

                time.sleep(1.5)
            b_anchor = last_event_time(bystander)
            bystander.intent(
                {"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]}
            )
            _coffin_state_after(
                bystander,
                b_anchor,
                lambda p: p["in_coffin"] is True,
                timeout=10.0,
                description=(
                    "leave 后 occupied_by 应已清——第二 bot 对同一棺 CoffinEnter 必须成功"
                ),
            )
            bystander.wait_for(
                lambda e: e.kind == "entity_metadata"
                and e.t > b_anchor
                and e.data["entity_id"] == bystander.entity_id
                and e.data["flags"] is not None
                and e.data["flags"] & 0x20,
                timeout=10.0,
                description="B 进棺后 invisible 位应置位（行为级读回的实体侧证据）",
            )
            bystander.wait_for(
                lambda e: e.kind == "pos_look"
                and e.t > b_anchor
                and _approx(e.data["x"], in_coffin_pos[0])
                and _approx(e.data["y"], in_coffin_pos[1])
                and _approx(e.data["z"], in_coffin_pos[2]),
                timeout=10.0,
                description="B 进棺后位置应瞬移到棺内",
            )

            # ── B 收尾：leave 还原 ─────────────────────────────────────────
            b_anchor = last_event_time(bystander)
            bystander.intent({"type": "coffin_leave", "v": 1})
            _coffin_state_after(
                bystander,
                b_anchor,
                lambda p: p["in_coffin"] is False,
                timeout=10.0,
                description="B leave 收尾应推 in_coffin:false",
            )
