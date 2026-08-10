"""GAP14 —— CoffinEnter 进延寿棺全链路（W14 覆盖率名册 Tier A）。

黑盒契约面 = `server/src/coffin/mod.rs` `handle_coffin_enter_requests`：
- 成功：`set_invisible(true)` + 瞬移到 `coffin_player_position`（lower+0.5, +0.05,
  +0.5）+ CoffinComponent + 推 `CoffinState{in_coffin:true, grade:"mundane",
  multiplier:0.9}`（mundane 0.9 见 `coffin_lifespan_multiplier` 单测）。
- 拒绝全静默（无回执）：无 registry 记录、occupied_by 已占、current_coffin 已在内、
  距离 >6m；维度门例外——回 `§c[棺]` chat 逐字。

断言面（state 读回而非响应 OK）：
1. 距离 >6m 拒绝——静默窗口无 CoffinState（移动走 8m 再 enter）。
2. 正向 enter——CoffinState 三字段 + **entity_metadata invisible 位（bit 5）置位**
   （valence Flags → TrackedData → EntityTrackerUpdateS2c entity_id=0 读回，字符状态
   实际切换）+ **pos_look 瞬移到棺内坐标**（服务端权威位置读回）。
3. repeat enter 拒绝——静默窗口无二次 CoffinState/无新 metadata/位置不变（状态机幂等）。
4. occupied 拒绝——第二 bot（CoEnB）enter 同坐标静默，收不到 in_coffin:true。
5. 异维拒绝——tpdim tsy 后 enter 同坐标 → `§c[棺]` chat 逐字（维度门在
   current_coffin 之前）。
6. leave 收尾——invisible 位清除 + 位置回 `coffin_exit_position`（场景不留脏状态；
   断连会走 persist 清理,但 leave 才是显式还原）。
"""

import math
import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = (
    "CoffinEnter 进延寿棺：正向往返全断言（CoffinState 字段 + invisible 位 + 位置瞬移），"
    "距离/重复/occupied/异维四路拒绝"
)
MODULES = ["coffin"]

MUNDANE_COFFIN = "mundane_coffin"
MUNDANE_MULTIPLIER = 0.9
DIMENSION_REJECTION = "§c[棺] 你不在主世界，无法操作延寿棺。"
# server: COFFIN_INTERACT_MAX_DISTANCE_SQ = 36.0（6m）
PLACE_OFFSET = 2
FAR_OFFSET = 8


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


def _invisible_after(bot, anchor, timeout, description):
    return bot.wait_for(
        lambda e: e.kind == "entity_metadata"
        and e.t > anchor
        and e.data["entity_id"] == bot.entity_id
        and e.data["flags"] is not None
        and e.data["flags"] & 0x20,
        timeout=timeout,
        description=description,
    )


def _pos_look_after(bot, anchor, xyz, timeout, description):
    return bot.wait_for(
        lambda e: e.kind == "pos_look"
        and e.t > anchor
        and _approx(e.data["x"], xyz[0])
        and _approx(e.data["y"], xyz[1])
        and _approx(e.data["z"], xyz[2]),
        timeout=timeout,
        description=description,
    )


def _assert_silent_window(bot, anchor, label, what="coffin_state"):
    """静默窗口：anchor 后 2s 内不得出现 new 的 what 事件（拒绝路径无回执契约）。"""
    time.sleep(2.0)
    with bot._lock:
        hits = [
            e
            for e in bot.events
            if e.t > anchor
            and (e.kind == "server_data" and e.data["payload_type"] == what)
        ]
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 应静默拒绝（无 {what} 回执），"
            f"实际窗口内出现 {len(hits)} 条: {hits[:3]}"
        )


def run(env) -> None:
    with env.new_bot("CoEn") as bot:
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

        # ── setup：CoffinPlace 消耗背包实例并注册 ──────────────────────────
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

        # ── 距离 >6m 拒绝（静默窗口，未在棺内时先测） ──────────────────────
        bot.move_to(px + FAR_OFFSET, py, pz, speed=5.5)
        time.sleep(1.5)  # 等 server 逐包消化移动上报
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        _assert_silent_window(bot, anchor, "距离 10m 的 CoffinEnter")

        # ── 正向 enter：三路状态读回 ───────────────────────────────────────
        bot.move_to(px, py, pz, speed=5.5)
        time.sleep(1.5)
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        _coffin_state_after(
            bot,
            anchor,
            lambda p: p["in_coffin"] is True
            and p["coffin_grade"] == "mundane"
            and math.isclose(p["lifespan_rate_multiplier"], MUNDANE_MULTIPLIER, abs_tol=1e-9),
            timeout=10.0,
            description=(
                f"进棺应推 CoffinState{{in_coffin:true, grade:mundane, "
                f"multiplier:{MUNDANE_MULTIPLIER}}}"
            ),
        )
        _invisible_after(
            bot,
            anchor,
            timeout=10.0,
            description="进棺后玩家 flags invisible 位（bit 5）应置位——字符状态实际切换",
        )
        _pos_look_after(
            bot,
            anchor,
            in_coffin_pos,
            timeout=10.0,
            description=f"进棺后应瞬移到棺内 coffin_player_position {in_coffin_pos}",
        )

        # ── repeat enter 拒绝：无二次状态突变、位置不动 ────────────────────
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        time.sleep(2.0)
        with bot._lock:
            extra_states = [
                e
                for e in bot.events
                if e.kind == "server_data"
                and e.data["payload_type"] == "coffin_state"
                and e.t > anchor
            ]
            extra_metadata = [
                e
                for e in bot.events
                if e.kind == "entity_metadata" and e.t > anchor
            ]
        if extra_states or extra_metadata:
            raise AssertionError(
                f"repeat enter 应被 current_coffin 静默拒绝，实际窗口内出现 "
                f"{len(extra_states)} 条 CoffinState / {len(extra_metadata)} 条 metadata"
            )
        if not _approx(bot.position[0], in_coffin_pos[0]) or not _approx(
            bot.position[1], in_coffin_pos[1]
        ) or not _approx(bot.position[2], in_coffin_pos[2]):
            raise AssertionError(
                f"repeat enter 被拒后位置不应变化，期望仍在棺内 {in_coffin_pos}，"
                f"实际 {bot.position}"
            )

        # ── occupied 拒绝：第二 bot 进同一棺静默 ───────────────────────────
        with env.new_bot("CoEnB") as bystander:
            wait_for_ready(bystander)
            b_anchor = last_event_time(bystander)
            bystander.intent(
                {"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]}
            )
            time.sleep(2.0)
            with bystander._lock:
                b_states = [
                    e
                    for e in bystander.events
                    if e.kind == "server_data"
                    and e.data["payload_type"] == "coffin_state"
                    and e.t > b_anchor
                    and e.data["payload"]["in_coffin"] is True
                ]
            if b_states:
                raise AssertionError(
                    f"occupied 拒绝契约：{bystander.username} 不得收到 in_coffin:true，"
                    f"实际 {len(b_states)} 条"
                )

        # ── 异维拒绝：维度门在 current_coffin 之前，chat 逐字 ─────────────
        anchor = last_event_time(bot)
        bot.cmd("tpdim tsy")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > anchor
            and "Queued /tpdim tsy within current XYZ gate." in e.data["text"],
            timeout=10.0,
            description="/tpdim tsy server 权威 transfer 排队反馈",
        )
        respawn = bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > anchor,
            timeout=10.0,
            description="/tpdim tsy 触发真实跨维 Respawn",
        )
        assert respawn.data["dimension_name"] == "bong:tsy", (
            f"tpdim tsy 应落在 bong:tsy，实际 {respawn.data['dimension_name']}"
        )
        bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t >= respawn.t,
            timeout=10.0,
            description="跨维后坐标确认脉冲",
        )
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        bot.wait_for(
            lambda e: e.kind == "chat" and e.t > anchor and DIMENSION_REJECTION in e.data["text"],
            timeout=10.0,
            description=f"异维进棺应逐字回执「{DIMENSION_REJECTION}」",
        )

        anchor = last_event_time(bot)
        bot.cmd("tpdim overworld")
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > anchor
            and "Queued /tpdim overworld within current XYZ gate." in e.data["text"],
            timeout=10.0,
            description="/tpdim overworld server 权威 transfer 排队反馈",
        )
        respawn = bot.wait_for(
            lambda e: e.kind == "respawn" and e.t > anchor,
            timeout=10.0,
            description="tpdim overworld 触发真实跨维 Respawn",
        )
        assert respawn.data["dimension_name"] == "minecraft:overworld", (
            f"tpdim overworld 应落回 minecraft:overworld，实际 {respawn.data['dimension_name']}"
        )
        bot.wait_for(
            lambda e: e.kind == "pos_look" and e.t >= respawn.t,
            timeout=10.0,
            description="回主世界后的坐标确认脉冲",
        )

        # ── leave 收尾：还原 invisible 与位置（GAP15 详断 leave 全链路） ───
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_leave", "v": 1})
        _coffin_state_after(
            bot,
            anchor,
            lambda p: p["in_coffin"] is False,
            timeout=10.0,
            description="leave 收尾应推 in_coffin:false 的 CoffinState",
        )
        bot.wait_for(
            lambda e: e.kind == "entity_metadata"
            and e.t > anchor
            and e.data["entity_id"] == bot.entity_id
            and e.data["flags"] is not None
            and not e.data["flags"] & 0x20,
            timeout=10.0,
            description="leave 收尾后 invisible 位应清除",
        )
        _pos_look_after(
            bot,
            anchor,
            exit_pos,
            timeout=10.0,
            description=f"leave 收尾后位置应回 coffin_exit_position {exit_pos}",
        )
