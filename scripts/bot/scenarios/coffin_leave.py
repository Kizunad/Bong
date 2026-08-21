"""GAP15 —— CoffinLeave 出延寿棺全链路（W14 覆盖率名册 Tier A）。

黑盒契约面 = `server/src/coffin/mod.rs` `handle_coffin_leave_requests`：
- 成功：`set_invisible(false)` + 位置回 `coffin_exit_position`（lower-0.5, +0.05,
  +0.5）+ 移除 CoffinComponent + `occupied_by` 清 + 推
  `CoffinState{in_coffin:false, grade:缺席, multiplier:1.0}`。
- 拒绝全静默（无回执、不瞬移）：从未进棺（无 CoffinComponent）直接 continue。

断言面（state 读回而非响应 OK）：
1. 未进过 leave——静默窗口无任何 CoffinState 变化、无 chat 回执、无瞬移（pos_look/
   窗末位置漂移双路，review finding [2]）（CoLvB 只收过 join 初始快照）。before_pos
   在 intent **之前**捕获（R6 finding 1：intent 后采样会被错误实现的瞬移污染）。
2. 正向 leave——CoffinState 三字段（grade 缺席、multiplier 回 1.0）+
   **entity_metadata invisible 位清除**（隐藏标记恢复）+ **pos_look 回
   coffin_exit_position**（服务端权威位置读回）。
3. 幂等 leave——第二次静默（组件已移除，无二次状态突变、无回执、无瞬移），
   before_pos 同样 intent 前捕获（R6 finding 1）。
4. **occupied_by 清的行为级读回**——CoLvB 随即对同一棺 CoffinEnter **成功**
   （in_coffin:true + invisible + 位置棺内）；这是比字段断言更强的状态证据：
   只有 `registry.clear_player` 真的清了 occupied_by，B 才能进得去。
5. CoLvB leave 收尾还原（场景不留脏状态）。
"""

import math
import time

from bot.bot import BotAssertionError
from bot.scenarios._coffin_helpers import teardown_coffin
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_instance,
    give_inventory_revision_barrier,
    inventory_item_instances,
    wait_inventory_contains_new_instance,
)

DESCRIPTION = (
    "CoffinLeave 出延寿棺：CoffinState 三字段 + invisible 位清除 + 位置还原 + "
    "幂等/未进过双静默 + occupied_by 清后第二 bot 可进（行为级读回）"
)
MODULES = ["coffin"]

MUNDANE_COFFIN = "mundane_coffin"
BARRIER_ITEM = "grass_fiber"
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


def _assert_silent_window(bot, anchor, label, before_pos, check_chat=True, check_pos=True):
    """anchor 后 2s 内不得出现新 CoffinState / bot 自身实体带 flags 的 metadata 更新 /
    chat 回执 /（check_pos）把 bot 移出 before_pos 的 pos_look 瞬移，且窗末 final
    position 不得漂移。

    **before_pos 是必传参数**：必须在 `bot.intent(...)` 之前捕获基线位置。若在 helper
    内采样，等请求发出后 bot.position 已被错误实现的瞬移覆盖，pos_look 落「新位置」会
    被当作 before_pos 同值放过（review finding 1, run 31434608946）。所有调用方必须在
    intent 前捕获 before_pos 传入。

    review finding [2]：leave 的 no-op（未进过 / 幂等二次）契约是**完全静默**——旧
    helper 只滤 coffin_state + metadata，把「发错误回执（chat）或瞬移玩家而不发
    coffin_state/metadata」的错误 leave 实现放过去。任何 chat 回执、任何落新坐标的
    pos_look、以及窗末位置漂移都是契约违反（两者共拒：chat 是回执通道，pos_look/位置
    是权威状态通道）。
    entity_id=0 是周期性世界实体心跳（flags=None），与棺状态无关，不得计入命中。"""
    time.sleep(2.0)
    with bot._lock:
        hits = [
            e
            for e in bot.events
            if e.t > anchor
            and (
                (e.kind == "server_data" and e.data["payload_type"] == "coffin_state")
                or (
                    e.kind == "entity_metadata"
                    and e.data["entity_id"] == bot.entity_id
                    and e.data["flags"] is not None
                )
                or (check_chat and e.kind == "chat")
                or (
                    check_pos
                    and before_pos is not None
                    and e.kind == "pos_look"
                    and not (
                        _approx(e.data["x"], before_pos[0])
                        and _approx(e.data["y"], before_pos[1])
                        and _approx(e.data["z"], before_pos[2])
                    )
                )
            )
        ]
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 应静默（无状态变化/回执/瞬移），"
            f"实际窗口内 {len(hits)} 条: {hits[:3]}"
        )
    after_pos = bot.position
    if check_pos and before_pos is not None and after_pos is not None and not all(
        _approx(a, b) for a, b in zip(before_pos, after_pos)
    ):
        raise AssertionError(
            f"[{bot.username}] {label} 应静默（位置不得变化），期望 {before_pos}，实际 {after_pos}"
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


def _wait_settled(bot, timeout=20.0, tol=0.02):
    """等 pos_look 连续两次采样位置不变（传送/坠落结束），避免在瞬移过程中采样 py，
    导致放棺位落在实心方块（服务器 `not empty` 静默拒绝，物品不消费）。"""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        prev = bot.position
        time.sleep(0.6)
        cur = bot.position
        if prev is not None and cur is not None and all(
            _approx(a, b, tol) for a, b in zip(prev, cur)
        ):
            return cur
    raise BotAssertionError(
        f"[{bot.username}] 位置在 {timeout:.0f}s 内未稳定，最后 position={bot.position}"
    )


def _settle_by_barrier(bot, instance_id, coords, barrier_timeout=20.0):
    """give-barrier 权威结算单个候选（review finding [3], run 31442491424）。

    本地观察死线不能区分「被拒」与「已接受但快照迟到」——静默拒绝本身就是协议，死线
    过期后带着未决请求发下一个放置（新实例 id、新坐标）会让两个非幂等请求先后都生效：
    双扣料、双棺、场景只记一个坐标，留持久脏状态。give 命令与放置请求共享同一条 TCP
    流，服务端按流序处理（FIFO）：从无关物品 give 的精确 revision 回执对拍到对应
    inventory_snapshot，即证明其前的放置请求已处理完毕，快照内容是权威裁决——

    - 该实例已不在快照 → 放置实际成功（消耗快照迟到而已）→ 返回坐标；
    - 该实例仍在快照 → 该候选被权威拒绝（未消费）→ 返回 None，放行下一候选。

    barrier 必须给与棺材不同的模板：棺材可堆叠，同模板 give 会增加被测实例的
    stack_count，既不会产生 fresh instance_id，也会让消费一次后实例仍留在快照。"""
    snapshot = give_inventory_revision_barrier(bot, BARRIER_ITEM, timeout=barrier_timeout)
    if find_instance(snapshot, MUNDANE_COFFIN, instance_id) is None:
        return coords
    return None


def _place_coffin(bot, px, py, pz):
    """放棺到附近空位：服务器对非空气坐标静默拒绝（不消费物品），逐个候选位置重试直到
    mundane_coffin 从背包消耗的 inventory_snapshot 出现，返回实际放置坐标（后续 enter/
    leave 一律以实际坐标为准，保证 read-back 与 reject 几何一致）。

    成功判定绑定到本次领取的具体实例 id（find_instance）。候选 3s 等待超时后走
    **give-barrier 权威结算**（_settle_by_barrier）——若其实际放置成功（消耗快照迟到
    于 3s 等待），返回其坐标，绝不让成功被误记到后续候选（否则 enter/leave 会指向
    registry 中不存在的坐标）；且**先结算再放行下一候选**，绝不带着未决请求发第二个
    放置（review finding [3]）。

    同一实例只在 barrier 明确证明上一候选已拒绝、物品仍在后复用；此时没有未决请求，
    不存在跨候选误记。不能逐候选 give fresh 棺材：可堆叠模板会沿用原 instance_id，
    第二次 give 后等待“新 id”必然超时。

    review finding [3]（R5）：消耗判定按**具体实例 id**（find_instance），不用 find_item
    首匹配——同模板旧实例（恢复的旧存档残留）会让首匹配到已消费的旧实例，掩盖新实例
    已被消费，场景把「已放置」误判为「候选全拒」。"""
    candidates = [
        (px - PLACE_OFFSET, py, pz),
        (px + PLACE_OFFSET, py, pz),
        (px, py, pz - PLACE_OFFSET),
        (px, py, pz + PLACE_OFFSET),
        (px - PLACE_OFFSET, py + 1, pz),
        (px + PLACE_OFFSET, py + 1, pz),
        (px, py + 1, pz - PLACE_OFFSET),
        (px, py + 1, pz + PLACE_OFFSET),
    ]
    pre = inventory_item_instances(bot, MUNDANE_COFFIN)
    bot.cmd(f"give {MUNDANE_COFFIN} 1")
    coffin = wait_inventory_contains_new_instance(bot, MUNDANE_COFFIN, pre)
    instance_id = int(coffin["item"]["instance_id"])
    for x, y, z in candidates:
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_place",
                "v": 1,
                "x": x,
                "y": y,
                "z": z,
                "item_instance_id": instance_id,
            }
        )
        try:
            bot.wait_for(
                lambda e, a=anchor, iid=instance_id: e.kind == "server_data"
                and e.data["payload_type"] == "inventory_snapshot"
                and e.t > a
                and find_instance(e.data["payload"], MUNDANE_COFFIN, iid) is None,
                timeout=3.0,
                description=f"候选放棺位 ({x},{y},{z}) 消耗 mundane_coffin(#{instance_id})",
            )
            return (x, y, z)
        except BotAssertionError:
            # 3s 等待超时不代表该候选被拒——服务端对成功放置同 tick 同步发消耗快照，慢
            # 处理下快照可迟到。**绝不带着未决请求重试下一候选**（review finding [3],
            # run 31442491424：两个不同实例 id 的放置请求可先后都生效 → 双棺双扣料、
            # 只记一个坐标）。give-barrier 权威结算本候选：give 快照到达即证明其前的
            # 放置已被处理，实例消失 = 放置成功（返回坐标）；仍在 = 权威拒绝，放行
            # 下一候选。
            settled = _settle_by_barrier(bot, instance_id, (x, y, z))
            if settled is not None:
                return settled
            continue
    raise AssertionError(
        f"[{bot.username}] 附近 {len(candidates)} 个候选放棺位均被服务器静默拒绝"
        f"（position floor=({px},{py},{pz})）"
    )


def run(env) -> None:
    with env.new_bot("CoLv") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)

        _wait_settled(bot)
        assert bot.position is not None, "需要 pos_look 后的 bot.position 定放棺位"
        px, py, pz = (int(v) for v in bot.position)
        # _place_coffin 领取一个实例并在每次权威拒绝后复用，返回实际放置坐标。
        lower = _place_coffin(bot, px, py, pz)
        in_coffin_pos = (lower[0] + 0.5, lower[1] + 0.05, lower[2] + 0.5)
        exit_pos = (lower[0] - 0.5, lower[1] + 0.05, lower[2] + 0.5)

        # ── setup：放棺 + 进棺，建立 in-coffin 状态 ────────────────────────
        _enter_coffin(bot, lower)

        with env.new_bot("CoLvB") as bystander:
            wait_for_ready(bystander)

            # ── 未进过棺的 leave 静默（负向） ──────────────────────────────
            # review finding 1 (run 31434608946)：before_pos 必须在 intent **之前**捕获
            # ——若在 helper 内采样，请求发出后 bot.position 已被错误实现的瞬移覆盖，
            # 迟到 pos_look 落「新位置」会被当作基线同值放过。先 settle 避免 join 的
            # 收尾 pos_look 在窗内落异坐标误报。
            _wait_settled(bystander)
            b_before = bystander.position
            b_anchor = last_event_time(bystander)
            bystander.intent({"type": "coffin_leave", "v": 1})
            _assert_silent_window(bystander, b_anchor, "从未进棺的 CoffinLeave", before_pos=b_before)

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
            # review finding 1 (run 31434608946)：before_pos 在 intent 前捕获（bot 刚
            # 完成首次 leave，位置已在 exit_pos 落定），窗内任何瞬移/窗末漂移都暴露。
            before_pos = bot.position
            anchor = last_event_time(bot)
            bot.intent({"type": "coffin_leave", "v": 1})
            _assert_silent_window(bot, anchor, "二次 CoffinLeave（幂等）", before_pos=before_pos)

            # ── occupied_by 清的行为级读回：B 随即进同一棺必须成功 ─────────
            # 距离必须对**实际放棺坐标**（_place_coffin 返回的 lower）判定，而不是对
            # bot 的原站位 (px,py,pz)：放棺位可偏移 PLACE_OFFSET 格，站在 (px,py,pz)
            # 对面 5m 的旁观者可能在服务器 6m 交互半径之外，被静默拒绝后场景超时。
            # server 按 coffin 中心（lower+0.5）算 3D 平方距离 ≤ 36.0。
            bx, by, bz = bystander.position
            coffin_center = (lower[0] + 0.5, lower[1] + 0.5, lower[2] + 0.5)
            if math.dist((bx, by, bz), coffin_center) > ENTER_RANGE:
                bystander.move_to(px, py, pz, speed=5.5)
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

        # leave 只释放占用，不销毁棺材。显式破坏并等待 marker despawn，避免污染同一
        # --all server 中后续 production_coffin_place_destroy 的世界实体不变量。
        teardown_coffin(bot, lower)
