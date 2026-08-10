"""GAP14 —— CoffinEnter 进延寿棺全链路（W14 覆盖率名册 Tier A）。

黑盒契约面 = `server/src/coffin/mod.rs` `handle_coffin_enter_requests`：
- 成功：`set_invisible(true)` + 瞬移到 `coffin_player_position`（lower+0.5, +0.05,
  +0.5）+ CoffinComponent + 推 `CoffinState{in_coffin:true, grade:"mundane",
  multiplier:0.9}`（mundane 0.9 见 `coffin_lifespan_multiplier` 单测）。
- 拒绝全静默（无回执）：无 registry 记录、occupied_by 已占、current_coffin 已在内、
  距离 >6m；维度门例外——回 `§c[棺]` chat 逐字。
- 维度门在 occupied_by（mod.rs:625）与 current_coffin（mod.rs:651）之间：要打到
  维度 chat，bot 必须先 leave 清 occupied_by，否则命中 occupied 静默拒绝。

断言面（state 读回而非响应 OK）：
1. 距离 >6m 拒绝——静默窗口无 CoffinState（移动走 8m 再 enter）。
2. 正向 enter——CoffinState 三字段 + **entity_metadata invisible 位（bit 5）置位**
   （valence Flags → TrackedData → EntityTrackerUpdateS2c 读回，字符状态实际切换）+
   **pos_look 瞬移到棺内坐标**（服务端权威位置读回）。
3. repeat enter 拒绝——静默窗口无二次 CoffinState/无新 metadata/位置不变（状态机幂等）。
4. occupied 拒绝——第二 bot（CoEnB）enter 同坐标静默，收不到 in_coffin:true。
5. leave 腾棺——invisible 位清除 + 位置回 `coffin_exit_position`（也即场景显式收尾）。
6. 异维拒绝——leave 后 tpdim tsy 再 enter 同坐标 → `§c[棺]` chat 逐字（此时
   occupied_by=None 才命中维度门）。
7. tpdim overworld 回主世界——场景不留脏状态。
"""

import math
import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    inventory_item_instances,
    require_item,
    wait_inventory_contains_new_instance,
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


def _assert_silent_window(bot, anchor, label, check_chat=False):
    """静默窗口：anchor 后 2s 内不得出现 coffin_state（**任何值**，含 in_coffin:false）、
    bot 自身实体带 flags 的 metadata 更新、或（check_chat）§c[棺] 前缀 chat 回执。

    拒绝路径的无回执契约是「什么都不该来」，不是「没收到成功回执」——只滤
    in_coffin:true 会把「拒发 in_coffin:false / 错误 chat / 状态更新」的坏实现放过去。"""
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
                or (check_chat and e.kind == "chat" and "§c[棺]" in e.data.get("text", ""))
            )
        ]
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 应静默拒绝（无 coffin_state/metadata/回执），"
            f"实际窗口内出现 {len(hits)} 条: {hits[:3]}"
        )


def _assert_no_entry_transition_after(bot, anchor, in_coffin_pos, label, window=2.0):
    """异维拒绝回执窗口：anchor 后 window 秒内不得出现构成「成功进棺」的三路状态转变。

    - CoffinState{in_coffin:true} 载荷；
    - bot 自身实体 invisible 位（bit 5）置位的 metadata；
    - 位置瞬移到棺内坐标（pos_look 落 in_coffin_pos）。

    review finding [2]：只等精确 chat 回执并不断言「之后无转变」，会让「维度门发了
    chat 却继续走进棺流程」的错误实现通过——它照发 in_coffin:true + invisible + 瞬移，
    场景随后所有 respawn 检查仍过，拒绝契约实际已破。本窗口把这三路转变全部钉死为
    负判据；anchor 取 intent 前，覆盖「chat 后 continue 漏掉」与「先转变后 chat」两种
    顺序（chat 事件本身 kind==chat 不落这三路，不误伤）。"""
    time.sleep(window)
    with bot._lock:
        hits = []
        for e in bot.events:
            if e.t <= anchor:
                continue
            if e.kind == "server_data" and e.data["payload_type"] == "coffin_state":
                if e.data["payload"].get("in_coffin") is True:
                    hits.append((f"coffin_state(in_coffin:true)@{e.t:.2f}",))
            elif (
                e.kind == "entity_metadata"
                and e.data["entity_id"] == bot.entity_id
                and e.data["flags"] is not None
                and e.data["flags"] & 0x20
            ):
                hits.append((f"metadata invisible 置位@{e.t:.2f}",))
            elif (
                e.kind == "pos_look"
                and _approx(e.data["x"], in_coffin_pos[0])
                and _approx(e.data["y"], in_coffin_pos[1])
                and _approx(e.data["z"], in_coffin_pos[2])
            ):
                hits.append((f"pos_look 瞬移到棺内@{e.t:.2f}",))
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 异维拒绝后 {window}s 内不得出现进棺状态转变，"
            f"实际 {len(hits)} 条: {hits[:3]}"
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


def _settle_previous_candidate(bot, candidate, settle_timeout=6.0, quiet=1.0):
    """结算上一候选的最终结果后，才允许发下一候选（review finding [1]）。

    上一候选的 3s wait 超时只说明「没等到消耗快照」，不代表它被拒——服务端对成功放置
    是**同 tick 同步发**消耗快照（coffin/mod.rs send_inventory_snapshot_to_client
    reason=coffin_place_consumed），慢处理下快照可迟到。若上一候选结果未定就发下一候选，
    上一候选的迟到快照会被下一候选的 wait 误认（它只判 e.t > anchor 且棺已不在包），
    返回的坐标在 registry 无记录，后续 enter/leave 全超时。本函数把上一候选**原子结算**：
    等新快照，任一快照显示棺已消耗 → 上一候选实际放置成功（返回其坐标，绝不把它的成功
    记到下一候选）；连续 quiet 秒无新快照且棺仍在 → 上一候选确定被拒（返回 None），
    此时才放行下一候选。返回放置坐标或 None。"""
    settle_anchor = last_event_time(bot)
    deadline = time.monotonic() + settle_timeout
    while time.monotonic() < deadline:
        try:
            event = bot.wait_for(
                lambda e, a=settle_anchor: e.kind == "server_data"
                and e.data["payload_type"] == "inventory_snapshot"
                and e.t > a,
                timeout=quiet,
                description="结算上一候选的 inventory_snapshot（t>上次锚）",
            )
        except BotAssertionError:
            return None  # quiet 秒无新快照 → 上一候选被静默拒绝，可放行下一候选
        if find_item(event.data["payload"], MUNDANE_COFFIN) is None:
            return candidate  # 上一候选实际放置成功：快照迟到但消耗真实
        settle_anchor = event.t  # 棺仍在，继续等更新的快照确认稳定性
    return None


def _place_coffin(bot, coffin, px, py, pz):
    """放棺到附近空位：服务器对非空气坐标静默拒绝（不消费物品），逐个候选位置重试直到
    mundane_coffin 从背包消耗的 inventory_snapshot 出现，返回实际放置坐标（后续 enter/
    leave 一律以实际坐标为准，保证 read-back 与 reject 几何一致）。

    成功判定必须绑定到本次请求：每次候选超时后先**原子结算**上一候选
    （_settle_previous_candidate）——若上一候选实际放置成功（消耗快照迟到于 3s 等待），
    返回上一候选坐标，绝不让上一候选的成功被误记到当前候选（否则 enter/leave 会指向
    registry 中不存在的坐标）。"""
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
    last_tried = None
    for x, y, z in candidates:
        if last_tried is not None:
            settled = _settle_previous_candidate(bot, last_tried)
            if settled is not None:
                return settled
        last_tried = (x, y, z)
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "coffin_place",
                "v": 1,
                "x": x,
                "y": y,
                "z": z,
                "item_instance_id": int(coffin["item"]["instance_id"]),
            }
        )
        try:
            bot.wait_for(
                lambda e, anchor=anchor: e.kind == "server_data"
                and e.data["payload_type"] == "inventory_snapshot"
                and e.t > anchor
                and find_item(e.data["payload"], MUNDANE_COFFIN) is None,
                timeout=3.0,
                description=f"候选放棺位 ({x},{y},{z}) 消耗 mundane_coffin",
            )
            return (x, y, z)
        except BotAssertionError:
            continue
    raise AssertionError(
        f"[{bot.username}] 附近 {len(candidates)} 个候选放棺位均被服务器静默拒绝"
        f"（position floor=({px},{py},{pz})）"
    )


def run(env) -> None:
    with env.new_bot("CoEn") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)
        pre_instances = inventory_item_instances(bot, MUNDANE_COFFIN)
        bot.cmd(f"give {MUNDANE_COFFIN} 1")
        snapshot = wait_inventory_contains_new_instance(bot, MUNDANE_COFFIN, pre_instances)
        coffin = require_item(snapshot, MUNDANE_COFFIN)

        _wait_settled(bot)
        assert bot.position is not None, "需要 pos_look 后的 bot.position 定放棺位"
        px, py, pz = (int(v) for v in bot.position)
        lower = _place_coffin(bot, coffin, px, py, pz)
        in_coffin_pos = (lower[0] + 0.5, lower[1] + 0.05, lower[2] + 0.5)
        exit_pos = (lower[0] - 0.5, lower[1] + 0.05, lower[2] + 0.5)

        # ── 距离 >6m 拒绝（静默窗口，未在棺内时先测） ──────────────────────
        bot.move_to(lower[0] + FAR_OFFSET, lower[1], lower[2], speed=5.5)
        time.sleep(1.5)  # 等 server 逐包消化移动上报
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        _assert_silent_window(bot, anchor, f"距离 {FAR_OFFSET}m 的 CoffinEnter")

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
        # 静默窗口滤到 bot 自身实体且 flags 非 None 的 metadata（与 _assert_silent_window
        # 同口径）：全局 entity_metadata 流里其他 tracked 实体的元数据包（flags=None）
        # 与棺状态无关，不得当作重复 enter 被拒的状态转变证据。
        _assert_silent_window(bot, anchor, "repeat CoffinEnter")
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
            # 距离必须对**实际放棺坐标**（_place_coffin 返回的 lower）判定：spawn 位可能
            # 落在 server 6m 交互半径之外，enter 会被**距离**门静默拒绝，occupied 测试就
            # 空过了（与距离路径同义）。先到原站位 (px,py,pz)——距棺中心 ~2-3m，保证命中
            # 的是 occupied_by 分支而非距离分支。move_to 后睡眠等 pos_look 落定。
            bpos = bystander.position
            coffin_center = (lower[0] + 0.5, lower[1] + 0.5, lower[2] + 0.5)
            if bpos is not None and math.dist(bpos, coffin_center) > ENTER_RANGE:
                bystander.move_to(px, py, pz, speed=5.5)
                time.sleep(1.5)
            before_pos = bystander.position
            b_anchor = last_event_time(bystander)
            bystander.intent(
                {"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]}
            )
            # occupied 拒绝契约是**全静默**（server occupied_by 分支直接 continue）：
            # 无 coffin_state（任何值）、无 metadata、无 §c[棺] 回执、位置不变——与距离
            # 路径同用综合静默窗口，而不是只滤 in_coffin:true。
            _assert_silent_window(
                bystander, b_anchor, "occupied CoffinEnter", check_chat=True
            )
            if before_pos is not None and not all(
                _approx(a, b) for a, b in zip(before_pos, bystander.position)
            ):
                raise AssertionError(
                    f"occupied 拒绝后 {bystander.username} 位置不应变化，"
                    f"期望 {before_pos}，实际 {bystander.position}"
                )

        # ── leave 腾棺：清 occupied_by + current_coffin，为维度门测试铺路 ──
        #    维度门在 occupied_by 检查之后：不 leave 则 bot 自己占着棺，enter 命中
        #    occupied 静默拒绝，维度 chat 永远打不到。leave 同时是场景显式收尾
        #    （invisible 清除 + 位置回 coffin_exit_position）。
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_leave", "v": 1})
        _coffin_state_after(
            bot,
            anchor,
            lambda p: p["in_coffin"] is False,
            timeout=10.0,
            description="异维测试前 leave 应推 in_coffin:false",
        )
        bot.wait_for(
            lambda e: e.kind == "entity_metadata"
            and e.t > anchor
            and e.data["entity_id"] == bot.entity_id
            and e.data["flags"] is not None
            and not e.data["flags"] & 0x20,
            timeout=10.0,
            description="leave 后 invisible 位应清除（维度测试前的显式收尾）",
        )
        _pos_look_after(
            bot,
            anchor,
            exit_pos,
            timeout=10.0,
            description=f"leave 后位置应回 coffin_exit_position {exit_pos}",
        )

        # ── 异维拒绝：leave 后 occupied_by=None，维度门才生效，chat 逐字 ──
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
        # review finding [2]：chat 回执必须与「无进棺状态转变」同时成立。维度门（coffin/
        # mod.rs:641）send_chat_message 后 continue，不该有任何 in_coffin:true / invisible
        # 置位 / 瞬移；只等 chat 会让「发了 chat 却继续走进棺流程」的错误实现通过。
        _assert_no_entry_transition_after(
            bot, anchor, in_coffin_pos, "异维 CoffinEnter（tsy）"
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
