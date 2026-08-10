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
# rejected_t 之后 1.0s 内，周期性 flush（无法恰好落入该窗口）即不能冒充权威回推。
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


def _drain_inventory_snapshots(bot, quiet: float = 1.2, max_wait: float = 8.0) -> None:
    """静置到窗口内无新 inventory_snapshot 再放行。

    清包等命令的 Changed<PlayerInventory> 回推是分 tick 滴入的（实测 clearinv
    三连 ~1.1s），若在残余回推到达前取锚，它们会落进负断言窗口造成假失败。"""
    deadline = time.monotonic() + max_wait
    while True:
        anchor = last_event_time(bot)
        time.sleep(quiet)
        stray = [
            e
            for e in bot.events_of("server_data")
            if e.data.get("payload_type") == "inventory_snapshot" and e.t > anchor
        ]
        if not stray:
            return
        if time.monotonic() > deadline:
            return


def _expect_no_inventory_change(bot, anchor_t: float, baseline_revision: int) -> None:
    """失败路径负断言：窗口内不得出现 revision 更高的 inventory_snapshot。

    服务端每 100 tick 会周期 flush 一次「revision 不变的当前状态快照」（实测
    ~5s 一条），负判据必须容忍它；拒绝路径（RealmTooLow/NotEnoughQi/instance
    不存在）从不 bump revision，所以「窗口内无更高 revision」等价于「无变更」。"""
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
        _drain_inventory_snapshots(bot)

        # ── 1. forge 拒绝-境界：fresh Awaken → RealmTooLow，无回推 ──
        anchor = last_event_time(bot)
        baseline_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline_revision)

        # ── 2. forge 拒绝-灵气：境界够但 qi=0 → NotEnoughQi，无回推 ──
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        anchor = last_event_time(bot)
        baseline_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent({"type": "forge_false_skin", "v": 1, "kind": "spider_silk"})
        _expect_no_inventory_change(bot, anchor, baseline_revision)

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
        # 应低于 forge 前的 5.0（e.t > forge_anchor 排除历史 qi=0 快照）。
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "player_state"
                and e.t > forge_anchor
                and float(e.data["payload"].get("spirit_qi", 9.9)) < 5.0
            ),
            timeout=10.0,
            description="forge 扣真元后（intent 之后）spirit_qi < 5.0",
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
        assert worn is not None and worn["location"]["kind"] == "equip", (
            f"equip 后 {FALSE_SKIN} 应在 equipped 层，实际 {worn!r}"
        )

        # ── 5. equip 拒绝-境界：降回 Awaken 再穿 → RealmTooLow + 回推 revision 不变 ──
        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        spare_snapshot = _give_and_wait(bot, FALSE_SKIN)
        spare = require_item(spare_snapshot, FALSE_SKIN)
        spare_instance = int(spare["item"]["instance_id"])
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
        require_item(kept, FALSE_SKIN)

        # ── 6. equip 静默：instance 不存在 → warn + revision 无变更 ──
        anchor = last_event_time(bot)
        baseline_revision = int(latest_inventory_snapshot(bot)["revision"])
        bot.intent(
            {
                "type": "equip_false_skin",
                "v": 1,
                "slot": "chest",
                "item_instance_id": 999999,
            }
        )
        _expect_no_inventory_change(bot, anchor, baseline_revision)

        bot.assert_alive("伪皮肤 6 步正负路径后")
