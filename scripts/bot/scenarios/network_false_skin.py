"""伪皮肤组：forge_false_skin / equip_false_skin。

黑盒契约面（server/src/combat/tuike.rs + client_request_handler.rs）：
- `forge_false_skin` → FalseSkinForgeRequest → handle_false_skin_forge_requests：
  成功 → 扣材料（ash_spider_silk×1）+ 扣真元（qi_cost=5.0，走 zone ledger）+
  产出 tuike_false_skin_silk 入包 + revision bump（Changed<PlayerInventory> 推快照）；
  失败（RealmTooLow 先于 NotEnoughQi 先于 MissingMaterial）→ 仅 warn log，
  库存/灵气均不动、revision 不变（负断言锚在 intent 后的有限窗口）。
  SpiderSilk min_realm=Induce（Awaken → RealmTooLow）。
- `equip_false_skin` → 忽略请求里的 slot（plan-layered-equip-v1 P0.1 伪皮归
  Chest/Worn 层），恒走 handle_inventory_move 到 Equip{Chest, Worn}：
  伪皮物品进胸槽受境界门控（can_equip_false_skin）；境界不足 →
  `inventory_move_rejected`{reason=RealmTooLow, required_realm=Induce} + 因果权威回推
  （revision 不变、物品保留）；境界够 → 物品移入 equipped.chest_worn + revision bump；
  instance 不存在 → warn + revision 不变（负断言同上）。
"""

import re
import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    assert_no_inventory_change,
    drain_inventory_snapshots,
    equip_location,
    find_instance_by_id,
    inventory_signature,
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_revision_after,
)

DESCRIPTION = (
    "伪皮肤：forge 境界/灵气/材料拒绝、forge 产出、equip 正路径/境界拒绝/静默"
)
MODULES = ["inventory", "cultivation", "combat"]

SILK = "ash_spider_silk"
FALSE_SKIN = "tuike_false_skin_silk"
NEGATIVE_WINDOW = 2.0
# forge 正路径的 qi 契约（server/src/combat/tuike.rs qi_cost()）：SpiderSilk
# 扣 qi_cost=5.0，经 release_qi_amount_to_zone → zone ledger（zone.spirit_qi
# 归一化存储，QI_ZONE_UNIT_CAPACITY=50.0），目的地 zone 增加 5.0/50.0 = 0.10。
FORGE_QI_COST = 5.0
QI_ZONE_UNIT_CAPACITY = 50.0
# 目的地 zone 信用断言容差（normalized spirit_qi 单位，期望增量 0.10 的 5%）。
# zone qi 可能被 NPC regen drain / skill cast 在两次探针读之间轻微扰动；但「直接扣
# qi_current 却完全不入账」的绕过（delta=0）与错额入账都远超此容差，仍被抓红。
ZONE_QI_DELTA_TOLERANCE = 0.01
# 结构化装备拒绝后，服务端应立即发 inventory_snapshot；只接受拒绝回执
# 之后的实际快照，避免把 intent 前的历史快照当成因果回推。
ROLLBACK_WINDOW = 1.0


def _give_and_wait(bot, item_id: str) -> dict:
    """锚定 give 之后的快照，且物品必须在随身层（非 equipped）。

    equipped 层也搜得到同模板物品（如 step 4 已穿上的伪皮），不排除的话
    give-wait 会命中旧快照里的已装备实例，真正的 give 快照落入后续负断言窗口。"""
    anchor = last_event_time(bot)
    bot.cmd(f"give {item_id} 1")
    time.sleep(0.5)  # give 走 chat→command 通道，先落一拍再等快照（冷启动实测坑）
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
            and (item := find_item(e.data["payload"], item_id)) is not None
            and item["location"]["kind"] != "equip"
        ),
        timeout=10.0,
        description=f"give 后（时间锚之后）随身层含 {item_id} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _expect_no_inventory_change(bot, anchor_t: float, baseline: dict) -> None:
    assert_no_inventory_change(bot, anchor_t, baseline, window=NEGATIVE_WINDOW)

def _assert_qi_unchanged_after_rejection(bot, expected_qi: float, realm_id: str) -> None:
    """拒绝路径 qi 守恒：用 qi_max 变更触发可靠的 player_state 重推。

    同值 realm 写不保证触发 Bevy 的 Changed<Cultivation>，因此不能作为观察屏障。
    `qi max 20` 只改变上限，不改当前真元；重推中的 spirit_qi 仍是拒绝后的权威值，
    随后恢复默认上限，避免探针影响后续步骤。
    """
    probe_anchor = last_event_time(bot)
    bot.cmd("qi max 20")
    bot.expect_chat("[dev] qi max", timeout=10.0)
    state = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "player_state"
            and e.t > probe_anchor
            and expected_qi - 1e-6
            <= float(e.data["payload"].get("spirit_qi", -1.0))
            <= expected_qi + 0.25
        ),
        timeout=10.0,
        description=f"拒绝后（qi-max 重发探针，realm={realm_id}）spirit_qi 保持 {expected_qi}",
    )
    observed_qi = float(state.data["payload"].get("spirit_qi", -1.0))
    if observed_qi < expected_qi - 1e-6:
        raise BotAssertionError(
            f"拒绝路径不得扣减真元：期望至少 {expected_qi}，实际 {observed_qi}"
        )
    bot.cmd("qi max 10")
    bot.expect_chat("[dev] qi max", timeout=10.0)


def _wait_move_rejected(bot, anchor_t: float, timeout: float = 10.0) -> tuple[dict, float]:
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_move_rejected"
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"inventory_move_rejected（t>{anchor_t:.2f}）",
    )
    return event.data["payload"], event.t


def _read_current_zone_qi(bot) -> float:
    """`zone_qi get` 只读探针：回显执行者当前所在 zone 的权威 spirit_qi。

    server/src/cmd/dev/zone_qi.rs：GetCurrent 用执行者 Position + CurrentDimension
    解析所在 zone，回显 `[dev] zone_qi <name> spirit_qi=... zone_total=...`。
    场景在 forge intent 前后各读一次，断言目的地 zone 增量 == qi_cost /
    QI_ZONE_UNIT_CAPACITY——这是「扣真元走 zone ledger」契约在 wire 上可观察的
    目的地侧证据（source 侧由 player_state.spirit_qi 断言，两侧合起来才是守恒对）。
    锚定 e.t > anchor 排除历史回显；读不到值直接抛错（探针必须成功，不能静默跳过）。"""
    anchor = last_event_time(bot)
    bot.cmd("zone_qi get")
    event = bot.wait_for(
        lambda e: (
            e.kind == "chat"
            and e.t > anchor
            and "spirit_qi=" in e.data["text"]
        ),
        timeout=10.0,
        description="zone_qi get 回显（[dev] zone_qi <name> spirit_qi=...）",
    )
    match = re.search(r"spirit_qi=([-0-9.]+)", event.data["text"])
    if not match:
        raise BotAssertionError(
            f"[{bot.username}] zone_qi get 回显无法解析 spirit_qi：{event.data['text']!r}"
        )
    return float(match.group(1))


def run(env) -> None:
    with env.new_bot("FalseSkin") as bot:
        wait_for_ready(bot)
        # join 初始 inventory_snapshot 与 game_join/pos_look 异步到达（不同推送），
        # 先消费掉再锚，否则它会落进负断言窗口（实测坑）。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
            ),
            timeout=10.0,
            description="join 初始 inventory_snapshot",
        )
        # /reset 归零 realm/qi/背包——保证「fresh Awaken + qi=0」前提成立，
        # 否则 persisted bong.db 会恢复上轮 run 的 realm=Induce（实测坑）。
        bot.cmd("reset")
        bot.expect_chat("[dev] reset self", timeout=10.0)
        time.sleep(1.0)  # 命令通道冷却：reset 后 0.6s 内 give 仍会被静默丢弃（实测坑，1.0s 稳）

        # ── 1. forge 拒绝-境界：fresh Awaken → RealmTooLow，无回推 ──
        # central-review 2012 #3：qi=0 时扣 qi 不可观测（0−5 被 clamp 回 0），先抬到
        # 1 让「拒绝时误扣真元」的错误实现可被探针读到差异。
        bot.cmd("qi set 1")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        baseline = latest_inventory_snapshot(bot)
        anchor = time.monotonic() - bot.t0
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # reset 归零 realm=Awaken（见上方 reset 注释）；qi-max 探针不改变境界。
        _assert_qi_unchanged_after_rejection(bot, 1.0, "awaken")

        # ── 2. forge 拒绝-灵气：境界够但 qi=1（< qi_cost 5）→ NotEnoughQi，无回推 ──
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        # qi 仍是 1（上步 qi-max 探针未改 qi_current）；拒绝路径同样断言 qi 守恒。
        baseline = latest_inventory_snapshot(bot)
        anchor = time.monotonic() - bot.t0
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # 当前境界为 Induce（本步开头 realm set induce）；qi-max 探针不改变境界。
        _assert_qi_unchanged_after_rejection(bot, 1.0, "induce")

        # ── 2b. forge 拒绝-材料：境界够 + qi 够但缺 ash_spider_silk → MissingMaterial ──
        # review finding [1]（round 9）：拒收步 Awaken qi=1 / Induce qi=1 都停在更靠前
        # 的 realm/qi 检查；唯一 qi 够（5）的请求先 give silk 后成功——没有任何请求
        # 到达 realm=Induce、qi>=5、无 ash_spider_silk 的状态，服务端 forge 顺序
        # RealmTooLow→NotEnoughQi→MissingMaterial（tuike.rs forge_false_skin）的
        # 最后分支从未被触达。错误实现若在 MissingMaterial 分支创建伪皮、先扣 qi
        # 再返回 MissingMaterial、选错结果或变异库存，会全过六步。必须把请求推到
        # 该分支并验证文档化契约：qi 不变 + 库存占用不变 + revision 不变（无回推）。
        bot.cmd("qi set 5")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        baseline = latest_inventory_snapshot(bot)
        anchor = time.monotonic() - bot.t0
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        # 当前境界为 Induce（step 2 开头 realm set induce）；qi 已抬到 5——qi>=qi_cost
        # 才能越过 NotEnoughQi 到达材料检查。MissingMaterial 是 warn-only 拒绝，
        # 探针（qi-max 只刷新状态，不碰 qi_current）读回 spirit_qi 必须仍是 5.0；错误实现
        # 若在材料检查前扣真元，探针读回 != 5.0，wait_for 超时即红。
        _assert_qi_unchanged_after_rejection(bot, 5.0, "induce")

        # ── 3. forge 正路径：补灵气后产出 + 扣材料 + revision bump ──
        bot.cmd("qi set 5")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        # 将当前 zone (spawn) 重置到未饱和基线 (0.80)，确保有足够空间容纳 forge 的 0.10 增量
        # 即使在 --all 全量运行（cultivation_breakthrough 曾将 spawn 置为 1.00 饱和态）下也能确定性断言
        bot.cmd("zone_qi set spawn 0.80")
        bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)
        # 稍微 sleep 确保 server 完成 zone_qi 状态处理
        time.sleep(0.5)
        silk_snapshot = _give_and_wait(bot, SILK)
        forge_revision = int(silk_snapshot["revision"])
        # central-review 2012 #2：forge 正路径必须同时验证「扣真元走 zone ledger」的
        # 目的地侧——只断言 player_state.spirit_qi==0 会让「直接扣 qi_current、绕过
        # ledger、不进 zone」的错误实现也通过。intent 前用 zone_qi get 探针读一次
        # 当前 zone 的权威 spirit_qi 作基线；forge 成功后读回、断言增量 == qi_cost
        # / QI_ZONE_UNIT_CAPACITY（0.10），与 source 侧扣减合起来才是守恒对。
        zone_baseline = _read_current_zone_qi(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        forged = wait_inventory_revision_after(bot, forge_revision, timeout=10.0)
        assert int(forged["revision"]) > forge_revision, (
            f"forge 成功后 revision 应 bump（>{forge_revision}），实际 {forged['revision']}"
        )
        require_item(forged, FALSE_SKIN)
        assert find_item(forged, SILK) is None, (
            f"forge 后材料 {SILK} 应被消耗，实际仍在快照中"
        )
        # 成功 forge 会改变 Cultivation，但当前 emitter 不保证同 tick 自动重推
        # player_state。用 qi-max 只读式探针触发一次可靠重推；forge 成本为 5，
        # 因而当前真元应为 0，随后只允许极小的吐纳回升，不允许残留大额真元。
        qi_probe_anchor = last_event_time(bot)
        bot.cmd("qi max 20")
        bot.expect_chat("[dev] qi max", timeout=10.0)
        state_after_forge = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "player_state"
                and e.t > qi_probe_anchor
                and 0.0 <= float(e.data["payload"].get("spirit_qi", 9.9)) <= 0.25
            ),
            timeout=10.0,
            description="forge 扣真元后（qi-max 重发探针）spirit_qi 在 0..0.25 内",
        ).data["payload"]
        observed_qi = float(state_after_forge.get("spirit_qi", -1.0))
        if observed_qi > 0.25:
            raise BotAssertionError(
                f"forge 后真元应已扣除 5.0，允许吐纳回升至 0.25，实际 {observed_qi}"
            )
        bot.cmd("qi max 10")
        bot.expect_chat("[dev] qi max", timeout=10.0)
        # central-review 2012 #2 回归：destination zone 信用断言。源（player -5.0，
        # 上面 player_state）与目的地（zone +0.10）两侧都在，才证明 qi 确实走了
        # zone ledger 而不是直接蒸发。预期增量 = FORGE_QI_COST / QI_ZONE_UNIT_CAPACITY。
        zone_after = _read_current_zone_qi(bot)
        expected_zone_delta = FORGE_QI_COST / QI_ZONE_UNIT_CAPACITY
        zone_delta = zone_after - zone_baseline
        if abs(zone_delta - expected_zone_delta) > ZONE_QI_DELTA_TOLERANCE:
            raise BotAssertionError(
                f"[{bot.username}] forge 成功后当前 zone spirit_qi 应增加 "
                f"{expected_zone_delta}（qi_cost {FORGE_QI_COST} / capacity "
                f"{QI_ZONE_UNIT_CAPACITY}），实际 {zone_baseline} -> {zone_after}"
                f"（delta={zone_delta}）；绕过 zone ledger 的扣真元实现会在此抓红"
            )

        # ── 4. equip 正路径：伪皮进 equipped.chest_worn + revision bump ──
        false_skin = require_item(forged, FALSE_SKIN)
        false_skin_instance = int(false_skin["item"]["instance_id"])
        equip_revision = int(forged["revision"])
        # central-review 2012 #2：请求 slot 用非规范值 "legs"——若服务端错误地
        # honor 请求字段，伪皮会落 legs 而非 chest，下方 location 断言即红。旧场景
        # 两个 equip 请求都发规范槽 "chest"，无法区分「恒落 Chest/Worn」与「按请求
        # slot 转换」两种实现。
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "legs",
                "item_instance_id": false_skin_instance,
            }
        )
        equipped = wait_inventory_revision_after(bot, equip_revision, timeout=10.0)
        worn = find_item(equipped, FALSE_SKIN)
        # central-review 2012 #6：equip_false_skin 必须忽略请求里的 slot、恒落
        # Equip{Chest, Worn}——只断言 kind=="equip" 会让错误实现把伪皮塞进手/饰品等
        # 别的装备层也通过。精确 pin 槽位 chest + 状态 worn（请求发的是 legs）。
        assert worn is not None and worn["location"] == equip_location("chest", "worn"), (
            f"equip（请求 slot=legs）后 {FALSE_SKIN} 应落 equipped.chest_worn"
            f"（Chest/Worn），实际 {worn!r}"
        )

        # ── 5. equip 拒绝-境界：降回 Awaken 再穿 → RealmTooLow + 回推 revision 不变 ──
        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        _give_and_wait(bot, FALSE_SKIN)
        # `give` 与 realm 变更都可能在不同 tick 补发 inventory_snapshot；只取
        # 第一张含物品的快照会把在途的旧 revision 当成拒绝请求基线。等快照静默
        # 后再取最新权威快照，确保 rejection resync 与真正请求前状态对拍。
        drain_inventory_snapshots(bot)
        spare_snapshot = latest_inventory_snapshot(bot)
        spare = require_item(spare_snapshot, FALSE_SKIN)
        spare_instance = int(spare["item"]["instance_id"])
        # 请求发出时刻是拒绝链的起点；回推快照随后严格锚到 rejection 事件。
        reject_anchor = time.monotonic() - bot.t0
        reject_revision = int(spare_snapshot["revision"])
        # central-review 2012 #2：请求 slot 同样用非规范值 "off_hand"——境界拒绝
        # 与 slot 无关（realm 门控先于槽位解析），但错误实现若 honor 请求字段并落
        # off_hand，realm 门控下同样拒绝——此处验证拒绝仍按 Chest/Worn 契约发生
        # （reason=realm_too_low + 被拒实例原位不动）。
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "off_hand",
                "item_instance_id": spare_instance,
            }
        )
        rejected, rejected_t = _wait_move_rejected(bot, reject_anchor)
        assert rejected.get("reason") == "realm_too_low", (
            f"inventory_move_rejected.reason 应为 realm_too_low（serde snake_case），"
            f"实际 {rejected.get('reason')!r}"
        )
        assert rejected.get("required_realm") == "Induce", (
            f"inventory_move_rejected.required_realm 应为 Induce，实际 "
            f"{rejected.get('required_realm')!r}"
        )
        # 服务端 emit_inventory_move_rejected 先于 resync_snapshot；只有 rejection
        # 之后实际收到的快照才可作为这次拒绝的因果回推。
        kept = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > rejected_t
                and e.t < rejected_t + ROLLBACK_WINDOW
            ),
            timeout=10.0,
            description=(
                f"拒绝回执之后 {ROLLBACK_WINDOW}s 内（t∈({rejected_t:.2f}, "
                f"{rejected_t + ROLLBACK_WINDOW:.2f})）的回推 inventory_snapshot"
            ),
        ).data["payload"]
        assert int(kept["revision"]) == reject_revision, (
            f"境界拒绝不得移动物品：revision 应保持 {reject_revision}，实际 {kept['revision']}"
        )
        assert inventory_signature(kept) == inventory_signature(spare_snapshot), (
            "境界拒绝后的完整 inventory_snapshot 必须逐字段保持请求前内容，"
            "包括 durability/freshness/forge/alchemy 等物品元数据"
        )
        # central-review 2012 #2：第 4 步已装备一件同模板伪皮，模板级 require_item
        # 会被旧装备件满足——错误的拒绝实现「丢失新给 spare_instance 却不动旧件」
        # 也能过。必须 pin 到被拒实例本身 + 非 equipped 原位。
        kept_spare = find_instance_by_id(kept, spare_instance)
        assert kept_spare is not None, (
            f"境界拒绝后被拒伪皮实例 {spare_instance} 应仍在背包"
            f"（原位 {spare['location']!r}），实际快照中未找到该实例"
        )
        assert kept_spare["location"] == spare["location"], (
            f"境界拒绝不得移动被拒实例：位置应保持 {spare['location']!r}，"
            f"实际 {kept_spare['location']!r}"
        )

        # ── 6. equip 静默：instance 不存在 → warn + revision 无变更 ──
        anchor = time.monotonic() - bot.t0
        baseline = latest_inventory_snapshot(bot)
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "chest",
                "item_instance_id": 999999,
            }
        )
        _expect_no_inventory_change(bot, anchor, baseline)

        bot.assert_alive("伪皮肤 7 步正负路径后")
