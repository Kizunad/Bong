"""技能卷轴组：learn_skill_scroll / technique_scroll_use。

黑盒契约面（server/src/network/client_request_handler.rs）：
- `learn_skill_scroll` → handle_learn_skill_scroll：首次习得 → 消耗卷轴 + 推
  `skill_xp_gain`（source=scroll）+ `skill_scroll_used`（was_duplicate=false,
  xp_granted=500）+ `skill_snapshot` + inventory_snapshot；重复习得 → was_duplicate=true，
  **不消耗**卷轴（回推快照 revision 不变）。
- **已知服务端行为**：xp 经 `SkillXpGain` 事件由 `consume_skill_xp_gain` 下一 tick 异步
  落地，而 handler 内同步发的 skill_snapshot 反映的是**落地前**状态（herbalism.xp 仍为
  0）；`Changed<SkillSet>` 只驱动 sqlite 持久化、不重发快照。故首次习得的 xp 证据断言
  放在 skill_xp_gain / skill_scroll_used 上；重复习得路径的 resync 快照才是落地后的
  xp（≥500）——本条即断言「快照最终追上 xp」。
- `technique_scroll_use` → handle_learn_technique_scroll：Learned → 消耗 + `techniques_snapshot`
  含新招式；RealmTooLow/RaceMismatch 等拒绝 → 先下发 `inventory_move_rejected`
  （reason=realm_too_low / race_mismatch，RealmTooLow 带 required_realm），再回推
  inventory_snapshot + techniques_snapshot，**不消耗**卷轴（revision 不变）。
  sword.cleave required_realm=Awaken（可学）；sword.infuse required_realm=Induce
  （fresh Awaken 玩家 → RealmTooLow，卷轴保留）。

卷轴模板：skill_scroll_herbalism_baicao_can（materials.toml）、
scroll_technique_sword_cleave / scroll_technique_sword_infuse（onboarding_scrolls.toml）。
"""

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import find_item, require_item

DESCRIPTION = "技能卷轴：skill 习得/重复/technique 习得/境界拒绝，消耗与保留各断言"
MODULES = ["skill", "inventory", "cultivation"]

HERBALISM_SCROLL = "skill_scroll_herbalism_baicao_can"
CLEAVE_SCROLL = "scroll_technique_sword_cleave"
INFUSE_SCROLL = "scroll_technique_sword_infuse"
SCROLL_XP = 500
# 拒绝回执/习得回执之后的因果窗口：处理在同一 tick 同步回推 resync 快照（毫秒级），
# 而 revision 不变的周期 flush ~5s 一条。把保留断言锚在回执事件**之后**并加窗口上限，
# 排除「回执前已到达的周期 flush」与「回执后较晚的无关 flush」冒充回推（review
# finding [10]）。
ROLLBACK_WINDOW = 1.0


def _give_and_wait(bot, item_id: str) -> dict:
    """锚定 give 之后的快照：不锚会匹配历史里第一条含该物品的快照——持久化残留
    （上轮 run 的 leftover 卷轴随 bong.db 恢复进 join 快照）或上一轮 give 的旧
    快照都会给出已失效的 instance_id，intent 被静默丢弃（实测 12:31/12:41/12:43 三次
    失败全因此）。"""
    anchor = last_event_time(bot)
    bot.cmd(f"give {item_id} 1")
    time.sleep(0.5)  # give 走 chat→command 通道，先落一拍再等快照（冷启动实测坑）
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], item_id) is not None
        ),
        timeout=10.0,
        description=f"give 后（时间锚之后）含 {item_id} 的 inventory_snapshot",
    )
    return event.data["payload"]


def _wait_scroll_used(
    bot,
    scroll_id: str,
    was_duplicate: bool,
    anchor_t: float,
    timeout: float = 10.0,
) -> tuple[dict, float]:
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "skill_scroll_used"
            and e.data["payload"].get("scroll_id") == scroll_id
            and e.t > anchor_t
        ),
        timeout=timeout,
        description=f"skill_scroll_used scroll_id={scroll_id}（t>{anchor_t:.2f}）",
    )
    payload = event.data["payload"]
    assert payload.get("was_duplicate") is was_duplicate, (
        f"skill_scroll_used.was_duplicate 应为 {was_duplicate}，实际 {payload.get('was_duplicate')!r}"
    )
    return payload, event.t


def _wait_skill_snapshot_skill(
    bot, skill_name: str, anchor_t: float, min_xp: int, timeout: float = 10.0
) -> dict:
    """锚定到 intent 之后、且该技能 xp>=min_xp 的 skill_snapshot。
    join 时会推一次全量 skill_snapshot（xp=0 基线），无锚会命中它。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "skill_snapshot"
            and e.t > anchor_t
            and any(
                entry.get("skill_name") == skill_name
                and int(entry.get("entry", {}).get("total_xp", 0)) >= min_xp
                for entry in e.data["payload"].get("skills", [])
            )
        ),
        timeout=timeout,
        description=f"skill_snapshot 含 {skill_name} 且 xp>={min_xp}（intent 之后）",
    )
    return event.data["payload"]


def _wait_technique_in_snapshot(
    bot, technique_id: str, anchor_t: float, timeout: float = 10.0
) -> dict:
    """锚定到 intent 之后的 techniques_snapshot 且含该招式。

    不锚会把 join 时恢复的持久化旧快照（上轮 run 已学 sword.cleave）当作本次
    intent 的习得证据——`reset` 只清账本、不清历史事件，无锚即静默误报
    （central-review 2012 #2）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "techniques_snapshot"
            and e.t > anchor_t
            and any(
                entry.get("id") == technique_id
                for entry in e.data["payload"].get("entries", [])
            )
        ),
        timeout=timeout,
        description=f"techniques_snapshot 含 {technique_id}（t>{anchor_t:.2f}）",
    )
    return event.data["payload"]


def _last_inventory_revision(bot) -> int:
    events = bot.events_of("server_data")
    for event in reversed(events):
        if event.data.get("payload_type") == "inventory_snapshot":
            return int(event.data["payload"]["revision"])
    raise BotAssertionError(f"[{bot.username}] 事件流中没有任何 inventory_snapshot")


def run(env) -> None:
    with env.new_bot("Scrolls") as bot:
        wait_for_ready(bot)
        # /reset 一次性归零 skill/technique/realm/qi/背包——本场景对「技能未习得、
        # 卷轴未消耗」有前提，persisted bong.db 恢复的旧状态（上轮 run 的
        # restored_skill=true）会让首个习得变成重复习得（实测坑）。
        bot.cmd("reset")
        bot.expect_chat("[dev] reset self", timeout=10.0)
        time.sleep(1.0)  # 命令通道冷却：reset 后 0.6s 内 give 仍会被静默丢弃（实测坑，1.0s 稳）

        # ── 1. learn_skill_scroll 正路径：消耗 + XP 三连 payload ──
        snapshot = _give_and_wait(bot, HERBALISM_SCROLL)
        scroll = require_item(snapshot, HERBALISM_SCROLL)
        scroll_instance = int(scroll["item"]["instance_id"])
        # 锚必须在 intent 前：skill_scroll_used/xp_gain/skill_snapshot 同 tick 齐发，
        # 事后锚会把自己要等的快照排除掉（实测坑）。
        skill_anchor = last_event_time(bot)
        bot.intent(
            {"type": "learn_skill_scroll", "v": 1, "instance_id": scroll_instance}
        )
        used, _used_t = _wait_scroll_used(
            bot, HERBALISM_SCROLL, was_duplicate=False, anchor_t=skill_anchor
        )
        assert used.get("skill") == "herbalism", (
            f"skill_scroll_used.skill 应为 herbalism，实际 {used.get('skill')!r}"
        )
        assert int(used.get("xp_granted", 0)) == SCROLL_XP, (
            f"skill_scroll_used.xp_granted 应为 {SCROLL_XP}，实际 {used.get('xp_granted')!r}"
        )
        gain = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "skill_xp_gain"
                and e.data["payload"].get("skill") == "herbalism"
                and e.t > skill_anchor
            ),
            timeout=10.0,
            description="skill_xp_gain skill=herbalism（intent 之后）",
        ).data["payload"]
        assert int(gain.get("amount", 0)) == SCROLL_XP, (
            f"skill_xp_gain.amount 应为 {SCROLL_XP}，实际 {gain.get('amount')!r}"
        )
        source = gain.get("source") or {}
        assert (
            source.get("kind") == "scroll"
            and source.get("scroll_id") == HERBALISM_SCROLL
        ), f"skill_xp_gain.source 应为 scroll/{HERBALISM_SCROLL}，实际 {source!r}"
        skill_snap = _wait_skill_snapshot_skill(
            bot, "herbalism", skill_anchor, 0
        )
        herb_entry = next(
            entry["entry"]
            for entry in skill_snap["skills"]
            if entry["skill_name"] == "herbalism"
        )
        # handler 同步发的快照是 xp 落地前的（异步 consume_skill_xp_gain 下一 tick
        # 才应用），此处只断言条目存在；xp 数额断言已在上面的 gain/used 上做过。
        assert isinstance(herb_entry, dict) and "xp" in herb_entry, (
            f"skill_snapshot herbalism entry 应携带 xp 字段，实际 {herb_entry!r}"
        )
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > skill_anchor
                and find_item(e.data["payload"], HERBALISM_SCROLL) is None
            ),
            timeout=10.0,
            description=f"卷轴 {HERBALISM_SCROLL} 消耗离包（intent 之后）",
        )

        # ── 2. 重复习得：was_duplicate=true + 卷轴保留（revision 不变）──
        snapshot = _give_and_wait(bot, HERBALISM_SCROLL)
        scroll2 = require_item(snapshot, HERBALISM_SCROLL)
        scroll2_instance = int(scroll2["item"]["instance_id"])
        before_revision = _last_inventory_revision(bot)
        anchor = last_event_time(bot)
        bot.intent(
            {"type": "learn_skill_scroll", "v": 1, "instance_id": scroll2_instance}
        )
        _, dup_t = _wait_scroll_used(
            bot, HERBALISM_SCROLL, was_duplicate=True, anchor_t=anchor
        )
        # review finding [10]：保留断言必须锚在 skill_scroll_used 回执**之后**（dup_t）
        # 并加窗口上限——旧实现锚 intent 前水位，回执前任意周期 flush 的「revision
        # 不变 + 卷轴仍在」快照都能冒充回推；错误实现「回了 was_duplicate 却仍消耗
        # 卷轴」也能过。回执同一 tick 同步回推 resync，dup_t 之后窗口内必有真实回推。
        kept = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > dup_t
                and e.t < dup_t + ROLLBACK_WINDOW
            ),
            timeout=10.0,
            description=(
                f"重复习得回执之后 {ROLLBACK_WINDOW}s 内（t∈({dup_t:.2f}, "
                f"{dup_t + ROLLBACK_WINDOW:.2f})）的回推 inventory_snapshot"
            ),
        ).data["payload"]
        assert int(kept["revision"]) == before_revision, (
            f"重复习得不消耗卷轴：revision 应保持 {before_revision}，实际 {kept['revision']}"
        )
        require_item(kept, HERBALISM_SCROLL)
        # 重复路径的 resync 快照反映的是 xp 异步落地后的状态：herbalism.total_xp 应 ≥500。
        # （首次习得的快照是落地前；注意 xp 是级内余量、total_xp 才是累计证据。）
        dup_snap = _wait_skill_snapshot_skill(bot, "herbalism", anchor, SCROLL_XP)
        dup_entry = next(
            entry["entry"]
            for entry in dup_snap["skills"]
            if entry["skill_name"] == "herbalism"
        )
        assert int(dup_entry.get("total_xp", 0)) >= SCROLL_XP, (
            f"重复习得 resync 快照 herbalism.total_xp 应 >= {SCROLL_XP}（xp 已落地），实际 {dup_entry!r}"
        )

        # ── 3. technique_scroll_use 正路径：消耗 + techniques_snapshot 含招式 ──
        snapshot = _give_and_wait(bot, CLEAVE_SCROLL)
        cleave = require_item(snapshot, CLEAVE_SCROLL)
        cleave_instance = int(cleave["item"]["instance_id"])
        cleave_anchor = last_event_time(bot)  # 锚必须在 intent 前（见 _wait_technique_in_snapshot）
        bot.intent(
            {"type": "technique_scroll_use", "v": 1, "instance_id": cleave_instance}
        )
        tech_snap = _wait_technique_in_snapshot(bot, "sword.cleave", cleave_anchor)
        cleave_entry = next(
            entry for entry in tech_snap["entries"] if entry["id"] == "sword.cleave"
        )
        assert cleave_entry.get("required_realm") == "Awaken", (
            f"sword.cleave required_realm 应为 Awaken，实际 {cleave_entry.get('required_realm')!r}"
        )
        bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > cleave_anchor
                and find_item(e.data["payload"], CLEAVE_SCROLL) is None
            ),
            timeout=10.0,
            description=f"卷轴 {CLEAVE_SCROLL} 消耗离包（intent 之后）",
        )

        # ── 4. technique_scroll_use 境界拒绝：RealmTooLow → 拒绝回执 + 卷轴保留 ──
        snapshot = _give_and_wait(bot, INFUSE_SCROLL)
        infuse = require_item(snapshot, INFUSE_SCROLL)
        infuse_instance = int(infuse["item"]["instance_id"])
        before_revision = _last_inventory_revision(bot)
        anchor = last_event_time(bot)
        bot.intent(
            {"type": "technique_scroll_use", "v": 1, "instance_id": infuse_instance}
        )
        # 拒绝原因必须在 wire 上可观察：只断 revision/保留无法区分「RealmTooLow 拒绝」
        # 与「静默忽略/错误原因拒绝」（central-review 2012 #3）。服务端在非习得拒绝时
        # 下发 inventory_move_rejected（reason=realm_too_low + required_realm）。
        rejected_event = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_move_rejected"
                and e.t > anchor
            ),
            timeout=10.0,
            description="technique_scroll_use 拒绝回执（inventory_move_rejected）",
        )
        rejected = rejected_event.data["payload"]
        rejected_t = rejected_event.t
        assert rejected.get("reason") == "realm_too_low", (
            f"拒绝 reason 应为 realm_too_low，实际 {rejected.get('reason')!r}"
        )
        assert rejected.get("required_realm") == "Induce", (
            f"拒绝 required_realm 应为 Induce，实际 {rejected.get('required_realm')!r}"
        )
        # review finding [10]：保留断言必须锚在拒绝回执**之后**（rejected_t）并加
        # 窗口上限——旧实现锚 intent 前水位，回执前任意周期 flush 的「revision 不变
        # + 卷轴仍在」快照都能冒充回推；错误实现「回 RealmTooLow 却仍消耗卷轴/漏发
        # 回推」也能过。服务端 emit 拒绝回执先于 resync 回推，rejected_t 之后窗口内
        # 必有真实回推。
        kept = bot.wait_for(
            lambda e: (
                e.kind == "server_data"
                and e.data.get("payload_type") == "inventory_snapshot"
                and e.t > rejected_t
                and e.t < rejected_t + ROLLBACK_WINDOW
            ),
            timeout=10.0,
            description=(
                f"境界拒绝回执之后 {ROLLBACK_WINDOW}s 内（t∈({rejected_t:.2f}, "
                f"{rejected_t + ROLLBACK_WINDOW:.2f})）的回推 inventory_snapshot"
            ),
        ).data["payload"]
        assert int(kept["revision"]) == before_revision, (
            f"RealmTooLow 拒绝不得消耗卷轴：revision 应保持 {before_revision}，实际 {kept['revision']}"
        )
        require_item(kept, INFUSE_SCROLL)

        bot.assert_alive("技能卷轴 4 步正负路径后")
