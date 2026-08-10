"""伪皮肤组：forge_false_skin / equip_false_skin。

黑盒契约面（server/src/combat/tuike.rs + client_request_handler.rs）：
- `forge_false_skin` → FalseSkinForgeRequest → handle_false_skin_forge_requests：
  成功 → 扣材料（ash_spider_silk×1）+ 扣真元（qi_cost=5.0，走 zone ledger）+
  产出 tuike_false_skin_silk 入包 + revision bump（Changed<PlayerInventory> 推快照）；
  失败（RealmTooLow 先于 NotEnoughQi 先于 MissingMaterial）→ 仅 warn log，
  库存/灵气均不动、revision 不变（负断言用「窗口内无更高 revision 的快照」——
  服务端每 100 tick 周期 flush 会推 revision 不变的当前状态快照，必须容忍）。
  SpiderSilk min_realm=Induce（Awaken → RealmTooLow）。
- `equip_false_skin` → 忽略请求里的 slot（plan-layered-equip-v1 P0.1 伪皮归
  Chest/Worn 层），恒走 handle_inventory_move 到 Equip{Chest, Worn}：
  伪皮物品进胸槽受境界门控（can_equip_false_skin）；境界不足 →
  `inventory_move_rejected`{reason=RealmTooLow, required_realm=Induce} + 权威回推
  （revision 不变、物品保留）；境界够 → 物品移入 equipped.chest_worn + revision bump；
  instance 不存在 → warn + revision 不变（负断言同上）。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    drain_inventory_quiet,
    equip_location,
    find_instance,
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_revision_after,
)

DESCRIPTION = (
    "伪皮肤：forge 境界/灵气拒绝、forge 产出、equip 正路径/境界拒绝/静默"
)
MODULES = ["inventory", "cultivation", "combat"]

SILK = "ash_spider_silk"
FALSE_SKIN = "tuike_false_skin_silk"
NEGATIVE_WINDOW = 2.0
# 拒绝回执之后的因果窗口：拒绝处理在**同一 tick** 同步 emit rejection + resync
# 回推（毫秒级到达），而 revision 不变的周期 flush 约 5s 一条。把回推快照限定在
# rejected_t 之后 1.0s 内。单靠 5s 周期假设不能排除 flush 恰好落在窗口里
# （review finding [2]），所以拒绝意图锚定前必须排干到 quiet=2.0s 静默
# （window 1.0 < quiet 2.0）——排干后下一次 flush 至少 2.0s 才可能到，窗口内不可能。
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


def _inventory_signature(snapshot: dict) -> tuple:
    """背包占用字段的规范投影：placed/equipped/hotbar/resource 等会随 forge 消耗
    而变的字段。排除推导字段（weight/body_level/qi_*/realm/revision）——周期 flush
    与拒绝回推共享同一份当前状态，推导字段可能被无关系统漂移，占用字段则不会
    在没有 mutation 的情况下变化。"""
    placed = sorted(
        (
            p["container_id"],
            p["row"],
            p["col"],
            p["item"]["instance_id"],
            p["item"].get("count", 1),
            p["item"]["item_id"],
        )
        for p in snapshot.get("placed_items", [])
    )
    equipped = {
        slot: [
            (i["instance_id"], i.get("count", 1), i["item_id"])
            for i in (items if isinstance(items, list) else ([items] if items else []))
        ]
        for slot, items in snapshot.get("equipped", {}).items()
    }
    hotbar = [
        None
        if slot is None
        else (slot["instance_id"], slot.get("count", 1), slot["item_id"])
        for slot in snapshot.get("hotbar", [])
    ]
    return (placed, equipped, hotbar, snapshot.get("bone_coins"))


def _expect_no_inventory_change(bot, anchor_t: float, baseline: dict) -> None:
    """失败路径负断言：窗口内不得出现 inventory 变更（revision bump **或**占用字段漂移）。

    服务端每 100 tick 会周期 flush 一次「revision 不变的当前状态快照」（实测 ~5s
    一条），负判据必须容忍它；拒绝路径（RealmTooLow/NotEnoughQi/instance 不存在）
    从不 bump revision，所以「窗口内无更高 revision」通常等价于「无变更」。
    但 review finding [8]：revision 不动不代表内容不动——错误实现可直接改容器/装备
    字段而不走 bump_revision API（绕过 revision 感知的写路径），仍产生一条 revision
    不变的快照。故在 revision 检查之外再对**最近一次权威快照**做占用字段签名比对：
    基线捕获在 intent 前、期间无任何 inventory 命令，签名任何漂移都是拒绝路径违规。"""
    baseline_revision = int(baseline["revision"])
    time.sleep(NEGATIVE_WINDOW)
    stray = [
        e
        for e in bot.events_of("server_data")
        if e.data.get("payload_type") == "inventory_snapshot"
        and e.t > anchor_t
        and int(e.data["payload"].get("revision", -1)) > baseline_revision
    ]
    if stray:
        raise BotAssertionError(
            f"[{bot.username}] 期望 {NEGATIVE_WINDOW}s 内无 revision 变更"
            f"（>{baseline_revision}），实际收到 {len(stray)} 条"
        )
    latest = latest_inventory_snapshot(bot)
    if _inventory_signature(latest) != _inventory_signature(baseline):
        raise BotAssertionError(
            f"[{bot.username}] 拒绝后背包占用字段应保持不变（基线 revision="
            f"{baseline_revision}），实际内容漂移"
        )


def _assert_qi_unchanged_after_rejection(bot, intent_anchor: float, expected_qi: float) -> None:
    """拒绝路径 qi 守恒：warn-only 拒绝不改 Cultivation，player_state 不会自动重发。

    `qi set <expected_qi>` 做只读探针强制重发：同值写仍触发 Changed<Cultivation> 的
    player_state 发射（qi_current = min(value, qi_max) 不变），从而读回权威
    spirit_qi。请求锚定：e.t > intent_anchor 排除探针前一切历史 player_state
    （包括 intent 前 qi set 的旧态）。错误实现若在拒绝时扣了 qi，探针读回的值
    与 expected_qi 不符，wait_for 超时即红。"""
    bot.cmd(f"qi set {expected_qi}")
    bot.expect_chat("[dev] qi set", timeout=10.0)
    bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "player_state"
            and e.t > intent_anchor
            and abs(float(e.data["payload"].get("spirit_qi", -1.0)) - expected_qi) < 1e-6
        ),
        timeout=10.0,
        description=f"拒绝后（intent 之后）spirit_qi 保持 {expected_qi}",
    )


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
        drain_inventory_quiet(bot, quiet=1.2, max_wait=8.0)

        # ── 1. forge 拒绝-境界：fresh Awaken → RealmTooLow，无回推 ──
        # central-review 2012 #3：qi=0 时扣 qi 不可观测（0−5 被 clamp 回 0），先抬到
        # 1 让「拒绝时误扣真元」的错误实现可被探针读到差异。
        bot.cmd("qi set 1")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        _assert_qi_unchanged_after_rejection(bot, anchor, 1.0)

        # ── 2. forge 拒绝-灵气：境界够但 qi=1（< qi_cost 5）→ NotEnoughQi，无回推 ──
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        # qi 仍是 1（上步探针为同值写，未改 qi_current）；拒绝路径同样断言 qi 守恒。
        anchor = last_event_time(bot)
        baseline = latest_inventory_snapshot(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline)
        _assert_qi_unchanged_after_rejection(bot, anchor, 1.0)

        # ── 3. forge 正路径：补灵气后产出 + 扣材料 + revision bump ──
        bot.cmd("qi set 5")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        silk_snapshot = _give_and_wait(bot, SILK)
        forge_revision = int(silk_snapshot["revision"])
        # central-review 2012 #3 回归：qi 断言必须锚在 forge intent 之后——否则
        # 「qi set 5 之前」spirit_qi=0 的旧 player_state 会满足 <5.0，扣真元缺失的
        # forge 也能通过。watermark 取 intent 前，排除一切历史快照。
        forge_anchor = last_event_time(bot)
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        forged = wait_inventory_revision_after(bot, forge_revision, timeout=10.0)
        assert int(forged["revision"]) > forge_revision, (
            f"forge 成功后 revision 应 bump（>{forge_revision}），实际 {forged['revision']}"
        )
        require_item(forged, FALSE_SKIN)
        assert find_item(forged, SILK) is None, (
            f"forge 后材料 {SILK} 应被消耗，实际仍在快照中"
        )
        # 真元走 zone ledger 扣除：forge intent 之后下发的 player_state.spirit_qi
        # 必须恰好等于 5.0 − qi_cost(5.0) = 0.0（守恒敏感契约，central-review 2012
        # #10）——只断言「< 5.0」会让扣 0.1、双扣成负等错误实现也通过。e.t >
        # forge_anchor 排除历史 qi=0 快照；浮点容差 1e-6 吸收序列化抖动。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "player_state"
                and e.t > forge_anchor
                and abs(float(e.data["payload"].get("spirit_qi", 9.9)) - 0.0) < 1e-6
            ),
            timeout=10.0,
            description="forge 扣真元后（intent 之后）spirit_qi == 0.0（5.0 − 5.0）",
        )

        # ── 4. equip 正路径：伪皮进 equipped.chest_worn + revision bump ──
        false_skin = require_item(forged, FALSE_SKIN)
        false_skin_instance = int(false_skin["item"]["instance_id"])
        equip_revision = int(forged["revision"])
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "chest",
                "item_instance_id": false_skin_instance,
            }
        )
        equipped = wait_inventory_revision_after(bot, equip_revision, timeout=10.0)
        worn = find_item(equipped, FALSE_SKIN)
        # central-review 2012 #6：equip_false_skin 必须忽略请求里的 slot、恒落
        # Equip{Chest, Worn}——只断言 kind=="equip" 会让错误实现把伪皮塞进手/饰品等
        # 别的装备层也通过。精确 pin 槽位 chest + 状态 worn。
        assert worn is not None and worn["location"] == equip_location("chest", "worn"), (
            f"equip 后 {FALSE_SKIN} 应落 equipped.chest_worn（Chest/Worn），"
            f"实际 {worn!r}"
        )

        # ── 5. equip 拒绝-境界：降回 Awaken 再穿 → RealmTooLow + 回推 revision 不变 ──
        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        spare_snapshot = _give_and_wait(bot, FALSE_SKIN)
        spare = require_item(spare_snapshot, FALSE_SKIN)
        spare_instance = int(spare["item"]["instance_id"])
        # review finding [2]：周期 flush（~5s 一条，revision 不变）可落进
        # ROLLBACK_WINDOW 冒充拒绝回推。锚定前排干到 quiet=2.0s——下一次
        # flush 至少 2.0s 后才可能到，而回推窗口只有 1.0s（window < quiet）。
        drain_inventory_quiet(bot, quiet=2.0)
        reject_anchor = last_event_time(bot)
        reject_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "chest",
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
        # central-review 2012 #3 回归：回推快照必须因果锚在拒绝回执**之后**
        # （rejected_t），不是 intent 前水位——否则请求与拒绝之间任意周期性 flush
        # 的「revision 不变」快照都能冒充权威回推，拒绝停发回推时断言仍过。
        # 服务端 emit_inventory_move_rejected 先于 resync_snapshot 下发，rejected_t
        # 之后必有真实回推快照。再加 ROLLBACK_WINDOW 上限：拒绝处理在同一 tick 同步
        # 回推（毫秒级到达），周期 flush ~5s 一条，窗口把「回执之后较晚的无关 flush」
        # 也排除——回推必须是对拒绝回执的即时后果。
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
        # central-review 2012 #2：第 4 步已装备一件同模板伪皮，模板级 require_item
        # 会被旧装备件满足——错误的拒绝实现「丢失新给 spare_instance 却不动旧件」
        # 也能过。必须 pin 到被拒实例本身 + 非 equipped 原位。
        kept_spare = find_instance(kept, spare_instance)
        assert kept_spare is not None, (
            f"境界拒绝后被拒伪皮实例 {spare_instance} 应仍在背包"
            f"（原位 {spare['location']!r}），实际快照中未找到该实例"
        )
        assert kept_spare["location"] == spare["location"], (
            f"境界拒绝不得移动被拒实例：位置应保持 {spare['location']!r}，"
            f"实际 {kept_spare['location']!r}"
        )

        # ── 6. equip 静默：instance 不存在 → warn + revision 无变更 ──
        anchor = last_event_time(bot)
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

        bot.assert_alive("伪皮肤 6 步正负路径后")
