"""GAP14 —— CoffinEnter 进延寿棺全链路（W14 覆盖率名册 Tier A）。

黑盒契约面 = `server/src/coffin/mod.rs` `handle_coffin_enter_requests`：
- 成功：`set_invisible(true)` + 瞬移到 `coffin_player_position`（lower+0.5, +0.05,
  +0.5）+ CoffinComponent + 推 `CoffinState{in_coffin:true, grade:"mundane",
  multiplier:0.9}`（mundane 0.9 见 `coffin_lifespan_multiplier` 单测）。
- 拒绝全静默（无回执、不瞬移）：无 registry 记录、occupied_by 已占、current_coffin
  已在内、距离 >6m；维度门例外——回 `§c[棺]` chat **逐字**（`==` 全串相等，子串包含
  会把追加前后缀的错误实现放过去）。
- 维度门在 occupied_by（mod.rs:625）与 current_coffin（mod.rs:651）之间：要打到
  维度 chat，bot 必须先 leave 清 occupied_by，否则命中 occupied 静默拒绝。

断言面（state 读回而非响应 OK）：
1. 距离 >6m 拒绝——静默窗口无 CoffinState、无 §c[棺] 回执、无瞬移（pos_look/窗末
  位置漂移双路，review finding [4]）（移动走 8m 再 enter）。
2. 正向 enter——CoffinState 三字段 + **entity_metadata invisible 位（bit 5）置位**
   （valence Flags → TrackedData → EntityTrackerUpdateS2c 读回，字符状态实际切换）+
   **pos_look 瞬移到棺内坐标**（服务端权威位置读回）。
3. repeat enter 拒绝——静默窗口无二次 CoffinState/无新 metadata/无 §c[棺] 回执/位置不变
   （状态机幂等，review finding [4]：旧场景未开 check_chat，发错误回执的实现会过）。
4. occupied 拒绝——第二 bot（CoEnB）enter 同坐标静默，收不到 in_coffin:true。
4b. **无注册棺拒绝**（R6 finding 4）——对 bot 自身站位 (px,py,pz)（距棺 PLACE_OFFSET
   格、registry 必然无记录、距 bot 0m 在交互半径内）发 enter → 全静默 + 不瞬移；
   旧场景所有请求都用已注册坐标，「registry.lookup None → 静默 continue」分支从未
   被打中。
5. leave 腾棺——invisible 位清除 + 位置回 `coffin_exit_position`（也即场景显式收尾）。
6. 异维拒绝——leave 后 tpdim tsy 再 enter 同坐标 → `§c[棺]` chat 逐字（此时
   occupied_by=None 才命中维度门）＋ **无任何进棺状态转变、无任何移出 before_pos 的
   pos_look、窗末 final 位置不变**（R6 finding 2：只滤落棺内坐标会让 teleport 到
   fallback/spawn/exit 等任意错误坐标的实现通过）。
7. tpdim overworld 回主世界——场景不留脏状态。
"""

import math
import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_instance,
    inventory_item_instances,
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


def _assert_silent_window(bot, anchor, label, check_chat=False, before_pos=None, check_pos=False):
    """静默窗口：anchor 后 2s 内不得出现 coffin_state（**任何值**，含 in_coffin:false）、
    bot 自身实体带 flags 的 metadata 更新、（check_chat）§c[棺] 前缀 chat 回执、
    （check_pos 且给定 before_pos）任何把 bot 移出 before_pos 的 pos_look 瞬移。

    拒绝路径的无回执契约是「什么都不该来」，不是「没收到成功回执」——只滤
    in_coffin:true 会把「拒发 in_coffin:false / 错误 chat / 状态更新」的坏实现放过去。
    check_pos 同时做窗末 final position 比对：窗口内 pos_look 落新坐标即红，迟到的
    teleport 落位也被窗末位置漂移暴露（review finding [4]：旧距离测试只滤前两类，
    「拒建状态却发回执 / 把远距玩家瞬移进棺」的实现全过）。"""
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
            f"[{bot.username}] {label} 应静默拒绝（无 coffin_state/metadata/回执/瞬移），"
            f"实际窗口内出现 {len(hits)} 条: {hits[:3]}"
        )
    if check_pos and before_pos is not None:
        after_pos = bot.position
        if after_pos is not None and not all(
            _approx(a, b) for a, b in zip(before_pos, after_pos)
        ):
            raise AssertionError(
                f"[{bot.username}] {label} 应静默（位置不得变化），"
                f"期望 {before_pos}，实际 {after_pos}"
            )


def _assert_no_entry_transition_after(bot, anchor, before_pos, label, window=2.0):
    """异维拒绝回执窗口：anchor 后 window 秒内不得出现构成「成功进棺」的三路状态转变，
    且位置不得有任何变化。

    - CoffinState{in_coffin:true} 载荷；
    - bot 自身实体 invisible 位（bit 5）置位的 metadata；
    - **任何**把 bot 移出 before_pos 的 pos_look（不只滤落棺内坐标——review finding 2,
      run 31434608946：teleport 到 fallback/spawn/exit 等任意错误坐标同样违反契约，
      旧版只滤 pos_look 恰落 in_coffin_pos，其余瞬移全过）；
    - 窗末 final position 比对：bot.position 必须仍等于 before_pos（迟到 teleport 的
      落位也被窗末漂移暴露）。

    review finding [2]（R5）：只等精确 chat 回执并不断言「之后无转变」，会让「维度门发
    了 chat 却继续走进棺流程」的错误实现通过——它照发 in_coffin:true + invisible +
    瞬移，场景随后所有 respawn 检查仍过，拒绝契约实际已破。本窗口把三路转变 + 任意位置
    变化全部钉死为负判据；anchor 取 intent 前、before_pos 取 intent 前（调用方保证），
    覆盖「chat 后 continue 漏掉」与「先转变后 chat」两种顺序（chat 事件本身 kind==chat
    不落这三路，不误伤）。"""
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
            elif e.kind == "pos_look" and not all(
                _approx(e.data[c], b) for c, b in zip(("x", "y", "z"), before_pos)
            ):
                hits.append(
                    (
                        f"pos_look 瞬移到 "
                        f"({e.data['x']:.2f},{e.data['y']:.2f},{e.data['z']:.2f})@{e.t:.2f}",
                    )
                )
    after_pos = bot.position
    if after_pos is not None and before_pos is not None and not all(
        _approx(a, b) for a, b in zip(after_pos, before_pos)
    ):
        hits.append((f"窗末位置漂移到 {after_pos}",))
    if hits:
        raise AssertionError(
            f"[{bot.username}] {label} 异维拒绝后 {window}s 内不得出现进棺状态转变/位置变化，"
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


def _settle_instance(bot, instance_id, anchor, coords, settle_timeout=6.0, quiet=1.0):
    """结算单个候选的最终结果：等新快照，任一快照显示**该实例**已消耗 → 候选实际放置
    成功（返回坐标）；连续 quiet 秒无新快照且该实例仍在 → 候选确定被拒（返回 None）。

    候选的 3s wait 超时只说明「没等到消耗快照」，不代表它被拒——服务端对成功放置是
    **同 tick 同步发**消耗快照（coffin/mod.rs send_inventory_snapshot_to_client
    reason=coffin_place_consumed），慢处理下快照可迟到。本函数把该候选**原子结算**。

    review finding 3 (run 31434608946)：**实例 id 即候选标识**——快照不带请求标识，
    但 `_place_coffin` 给每个候选独立 fresh 实例，结算只按**本候选自己的实例 id**
    （find_instance）判定消耗，同一实例永远只对应一个候选请求。旧实现多候选复用同一
    实例，settle 窗口超时后上一候选的迟到成功快照会被下一候选的 wait 误认，返回的坐标
    在 registry 无记录，后续 enter/leave 全超时。

    review finding [1]（R5）：下界初值必须是候选自身的原始锚（candidate_anchor，即该
    候选 intent 前取的 last_event_time），不是函数入口现取的 last_event_time——intent
    与结算之间没有任何事件屏障，若成功消耗快照在原 wait_for 最后一次扫描后、函数入口
    取锚前落到事件流，入口现取锚会把 ≤ 锚的快照永久排除。保留原锚覆盖 intent→结算整个
    无覆盖区间；锚只在对某快照**做过消耗判定后**才推进（先检查再前进）。review finding
    [3]（R5）：消耗判定按**具体实例 id**（find_instance），同模板旧实例残留不会让首匹配
    掩盖新实例已被消费。"""
    settle_anchor = anchor
    deadline = time.monotonic() + settle_timeout
    while time.monotonic() < deadline:
        try:
            event = bot.wait_for(
                lambda e, a=settle_anchor: e.kind == "server_data"
                and e.data["payload_type"] == "inventory_snapshot"
                and e.t > a,
                timeout=quiet,
                description=f"结算候选坐标 {coords} 的 inventory_snapshot（t>{settle_anchor:.3f}）",
            )
        except BotAssertionError:
            # quiet 秒内无新快照只说明「这 1s 没等到」，不证明该候选被拒——旧实现在这里
            # 立即 return None，settle_timeout 死线从未生效，延迟快照（>1s 才到）被当成
            # 拒收。wait_for 每次全量重扫事件列表，迟到快照在下一次 wait 立即命中，
            # continue 保持完整结算窗口。
            continue
        if find_instance(event.data["payload"], MUNDANE_COFFIN, instance_id) is None:
            return coords  # 候选实际放置成功：快照迟到但消耗真实
        settle_anchor = event.t  # 该实例仍在，推进观察点等更新的快照确认稳定性
    return None


def _place_coffin(bot, px, py, pz):
    """放棺到附近空位：服务器对非空气坐标静默拒绝（不消费物品），逐个候选位置重试直到
    mundane_coffin 从背包消耗的 inventory_snapshot 出现，返回实际放置坐标（后续 enter/
    leave 一律以实际坐标为准，保证 read-back 与 reject 几何一致）。

    成功判定必须绑定到本次请求：**每个候选 give 一个独立 fresh mundane_coffin 实例**，
    消耗判定与结算全部按各自实例 id（find_instance）。候选 3s 等待超时后先**原子结算**
    该候选（_settle_instance）——若其实际放置成功（消耗快照迟到于 3s 等待），返回其坐标，
    绝不让成功被误记到后续候选（否则 enter/leave 会指向 registry 中不存在的坐标）。

    review finding 3 (run 31434608946)：实例 id 即候选标识——旧实现多候选复用同一实例，
    快照不带请求标识，settle 窗口超时后上一候选的迟到成功快照会被下一候选的 wait 误认，
    返回的坐标在 registry 无记录，后续 enter/leave 全超时。fresh 实例使快照按实例 id
    精确归属，上一候选的迟到快照只证明「它自己的实例」被消费，下一候选的 wait 检查
    新实例 id，绝无跨候选误记。

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
    for x, y, z in candidates:
        pre = inventory_item_instances(bot, MUNDANE_COFFIN)
        bot.cmd(f"give {MUNDANE_COFFIN} 1")
        # wait_inventory_contains_new_instance 返回新 give 的**实例**（instance_id
        # ∉ pre），排除同模板旧实例（恢复的旧存档残留）——消耗判定必须按该实例 id。
        coffin = wait_inventory_contains_new_instance(bot, MUNDANE_COFFIN, pre)
        instance_id = int(coffin["item"]["instance_id"])
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
            # 处理下快照可迟到。结算该**实例**：迟到快照显示该实例被消费 → 此候选实际
            # 放置成功（返回其坐标）；该实例仍在 → 此候选确定被拒，放行下一候选。
            settled = _settle_instance(bot, instance_id, anchor, (x, y, z))
            if settled is not None:
                return settled
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

        _wait_settled(bot)
        assert bot.position is not None, "需要 pos_look 后的 bot.position 定放棺位"
        px, py, pz = (int(v) for v in bot.position)
        # _place_coffin 内部按候选 give fresh mundane_coffin 实例，返回实际放置坐标。
        lower = _place_coffin(bot, px, py, pz)
        in_coffin_pos = (lower[0] + 0.5, lower[1] + 0.05, lower[2] + 0.5)
        exit_pos = (lower[0] - 0.5, lower[1] + 0.05, lower[2] + 0.5)

        # ── 距离 >6m 拒绝（静默窗口，未在棺内时先测） ──────────────────────
        bot.move_to(lower[0] + FAR_OFFSET, lower[1], lower[2], speed=5.5)
        time.sleep(1.5)  # 等 server 逐包消化移动上报
        _wait_settled(bot)  # 位置稳定后再取 before_pos：移动收尾的迟到 pos_look 若落
        # 在静默窗口内，会被 check_pos 当成瞬移红（review finding [4] 误报）
        before_pos = bot.position
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        # review finding [4]：距离拒绝契约是**全静默 + 不瞬移**——check_chat 钉死错误
        # chat 回执（server 距离分支 line 651 是 silent continue，不该有 §c[棺]），
        # check_pos 钉死「把远距玩家瞬移进棺」的 teleport（pos_look 落新坐标 / 窗末
        # 位置漂移双路）。旧调用默认 check_chat=False 且无任何位置断言，只滤 coffin_state
        # + metadata 会让「拒建状态却发回执/瞬移」的实现通过。
        _assert_silent_window(
            bot,
            anchor,
            f"距离 {FAR_OFFSET}m 的 CoffinEnter",
            check_chat=True,
            before_pos=before_pos,
            check_pos=True,
        )

        # ── 无注册棺拒绝（负向）：坐标在交互半径内但 registry 无记录 ──────
        # review finding 4 (run 31434608946)：旧场景所有 coffin_enter 都用 _place_coffin
        # 返回的已注册坐标，handle_coffin_enter_requests 的「registry.lookup 为 None →
        # 静默 continue」分支从未被请求真正打中——一个「registry.get 返回 None 时接受任意
        # 在范围内坐标」的错误实现（不发 CoffinState 的常规进棺）会全过。对 bot 自身站位
        # (px,py,pz) 发 enter：距棺 PLACE_OFFSET 格（必然无 registry 记录）、距 bot 0m
        # （必在 6m 交互半径内，排除距离分支干扰），断言全静默 + 不瞬移。
        bot.move_to(px, py, pz, speed=5.5)
        time.sleep(1.5)
        _wait_settled(bot)
        before_pos = bot.position
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": px, "y": py, "z": pz})
        _assert_silent_window(
            bot,
            anchor,
            "无注册棺的 CoffinEnter",
            check_chat=True,
            before_pos=before_pos,
            check_pos=True,
        )

        # ── 正向 enter：三路状态读回 ───────────────────────────────────────
        _wait_settled(bot)
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

        # ── repeat enter 拒绝：无二次状态突变、无回执、位置不动 ────────────
        before_pos = bot.position
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        # 静默窗口滤到 bot 自身实体且 flags 非 None 的 metadata（与 _assert_silent_window
        # 同口径）：全局 entity_metadata 流里其他 tracked 实体的元数据包（flags=None）
        # 与棺状态无关，不得当作重复 enter 被拒的状态转变证据。
        #
        # review finding [4]：repeat-enter 契约是**全静默**（in_coffin 已 true 时 server
        # 直接 silent return），不只滤状态突变——check_chat 钉死「拒了却发错误回执」的
        # 实现，check_pos 钉死「重复 enter 把玩家瞬移/移位」的实现。旧调用默认
        # check_chat=False 且无位置断言，一个发错误 chat 的坏实现全过。
        _assert_silent_window(
            bot,
            anchor,
            "repeat CoffinEnter",
            check_chat=True,
            before_pos=before_pos,
            check_pos=True,
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
        # review finding 2 (run 31434608946)：before_pos 必须在 intent **之前**捕获并传入
        # _assert_no_entry_transition_after——该 helper 现在拒绝**任何**移出 before_pos 的
        # pos_look 并比对窗末 final position，before_pos 若在请求后采样则被错误实现的
        # 瞬移污染。先 settle 跨维落点，避免收尾 pos_look 在窗内落异坐标误报。
        _wait_settled(bot)
        before_pos = bot.position
        anchor = last_event_time(bot)
        bot.intent({"type": "coffin_enter", "v": 1, "x": lower[0], "y": lower[1], "z": lower[2]})
        # review finding [1]（R5）：维度门契约是**逐字** chat（server send_chat_message
        # 原样发出 COFFIN_DIMENSION_REJECTION_MESSAGE）——子串包含判定会把「前缀/后缀
        # 追加额外文本」的错误实现放过去，必须全串相等。chat 事件本身不含 §c[棺] 前缀
        # 之外的拼接，整串相等才满足「用户可见回执逐字一致」的契约。
        bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > anchor
            and e.data["text"] == DIMENSION_REJECTION,
            timeout=10.0,
            description=f"异维进棺应逐字回执「{DIMENSION_REJECTION}」",
        )
        # review finding [2]：chat 回执必须与「无进棺状态转变」同时成立。维度门（coffin/
        # mod.rs:641）send_chat_message 后 continue，不该有任何 in_coffin:true / invisible
        # 置位 / 瞬移；只等 chat 会让「发了 chat 却继续走进棺流程」的错误实现通过。
        _assert_no_entry_transition_after(bot, anchor, before_pos, "异维 CoffinEnter（tsy）")

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
