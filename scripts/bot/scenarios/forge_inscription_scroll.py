"""刻铭（GAP18）：残卷入炉 filled_slots 0→1→2 + 负例不消耗 + 双发不重复 grant。

`ForgeInscriptionScroll{session_id, inscription_id}` 的黑盒契约（对照
DONE-W9-BOTSCEN-GAP12-19-BRIEFS.md §GAP18）：

- 正例：灵锋（ling_feng_v0）走到 Inscription 步，残卷 `inscription_scroll_sharp_v0`
  （inscription_id=sharp_v0）入炉一枚 → `forge_session` read-back 的
  `step_state` 变 `{kind:inscription, filled_slots:1, max_slots:2, failed:false}`，
  残卷从背包消耗（`forge_inscription_scroll_consumed` 是服务端日志 reason，
  wire 上的 InventorySnapshotV1 没有 reason 字段——断言可观察字段：残卷 1→0 +
  filled_slots 读回，不锁日志）。
- 再入炉第二枚 → filled_slots 1→2（max_slots=2 封顶）。
- 负例一（不存在 id）：`inscription_id:"nonexistent"` → 无实例可扣，服务端
  resync 一份 inventory_snapshot 且残卷仍在（`forge_inscription_scroll_missing`
  路径，无消费）。
- 负例二（步骤错配）：推进到 Consecration 后再提交 → `require_owned_active_step`
  静默拒绝（无任何事件、不消费探针残卷）。
- 双发保护：同一 inscription_id 在消费后立即重发 → 无第二实例可扣，filled_slots
  不得前进到 2（若 +2 即双发 grant bug）。
- 收尾：全部负例之后 Consecration → Done → `forge_outcome` 正常结算（测试 bot
  境界 < Spirit，确定性 Flawed 结局），证明会话未因拒绝而损坏。

与服务端代码的对应：
- client_request_handler.rs handle_forge_inscription_scroll：空 id return；
  require_owned_active_step 门；find_inscription_scroll_instance_id → None →
  resync_snapshot(missing)。
- forge/mod.rs handle_scroll_submits：session 在 Inscription + caster 匹配 +
  instance_matches → consume_item_instance_once → apply_scroll(filled_slots+=1) →
  InscriptionScrollApplied（forge_snapshot_emit 据此回推 forge_session）+
  send_inventory_snapshot_to_client(consumed)。

正例命名断言字段（brief §GAP18 点名）：filled_slots、max_slots、failed、current_step
与背包残卷数量。超时口径对齐 production_forge_station_real_place.py：stage 等待统一
45s（CI e2e 长跑后 TPS 退化），否定断言用 4s settle 窗口，不放大任何断言。
"""

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    require_item,
)

DESCRIPTION = "刻铭：sharp_v0 残卷入炉 filled_slots 0→1→2 + 负例（不存在id/步骤错配）不消耗 + 双发不重复 grant + 会话照常结算"
MODULES = ["forge", "inventory"]

# sui_tie（Xi 稀铁）forge_tier_min=3 → 必须玄铁砧（tier 3）；灵铁砧 tier 2 会被
# bp.validate_with 静默 TierMismatch（仅 feedback chat，无 server log）。
ANVIL_ID = "xuan_iron_anvil"
ANVIL_TIER = 3
SCROLL_ID = "blueprint_scroll_ling_feng"
BLUEPRINT_ID = "ling_feng_v0"
INSCRIPTION_SCROLL_ID = "inscription_scroll_sharp_v0"
INSCRIPTION_ID = "sharp_v0"
# sui_tie 无 ItemRegistry TOML 模板 → `/give sui_tie` 走 dev-only mineral fallback，
# 落地 item_id 是 mineral_sui_tie（mineral_id=sui_tie 才是 forge 扣料的匹配键）。
MATERIAL = "sui_tie"
MATERIAL_ITEM_ID = "mineral_sui_tie"
WEAPON_FLAWED_ID = "ling_feng_sword_flawed"
# ling_feng_v0.json 图谱正典 tempering pattern（15 拍，window_ticks=8，miss_allowed=1）。
TEMPERING_PATTERN = ["H", "L", "F", "H", "L", "F", "F", "H", "L", "F", "H", "H", "F", "L", "H"]
# 否定断言 settle 窗口：短于 stage 超时，用来确认"不该来的没来"。
NEGATIVE_SETTLE = 4.0


def _forge_start_session(bot, station_pos, blueprint_id, materials):
    bot.intent(
        {
            "type": "forge_start_session",
            "v": 1,
            "station_pos": list(station_pos),
            "blueprint_id": blueprint_id,
            "materials": [[material, count] for material, count in materials],
        }
    )


def _forge_learn_blueprint(bot, blueprint_id):
    bot.intent({"type": "forge_learn_blueprint", "v": 1, "blueprint_id": blueprint_id})


def _forge_tempering_hit(bot, session_id, beat, ticks_remaining=1):
    bot.intent(
        {
            "type": "forge_tempering_hit",
            "v": 1,
            "session_id": session_id,
            "beat": beat,
            "ticks_remaining": ticks_remaining,
        }
    )


def _forge_step_advance(bot, session_id):
    bot.intent({"type": "forge_step_advance", "v": 1, "session_id": session_id})


def _forge_inscription_scroll(bot, session_id, inscription_id):
    bot.intent(
        {
            "type": "forge_inscription_scroll",
            "v": 1,
            "session_id": session_id,
            "inscription_id": inscription_id,
        }
    )


def _wait_forge_payload_after(bot, anchor, payload_type, predicate, timeout, description):
    return bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == payload_type
        and e.t > anchor
        and predicate(e.data["payload"]),
        timeout=timeout,
        description=description,
    )


def _inscription_step_state(payload):
    """Inscription 步的 step_state（proto_min 已解码为 kind=filled_slots/max_slots/failed）。"""
    step_state = payload.get("step_state") or {}
    if step_state.get("kind") != "inscription":
        return None
    return step_state


def _give_and_wait(
    bot,
    command_item_id,
    expected_item_id=None,
    count=1,
    expected_stack_count=None,
    timeout=45.0,
):
    """Wait for the inventory snapshot caused by this give, not a historical snapshot."""
    expected_item_id = expected_item_id or command_item_id
    anchor = last_event_time(bot)
    bot.cmd(f"give {command_item_id} {count}")

    def matches(event):
        if (
            event.kind != "server_data"
            or event.data["payload_type"] != "inventory_snapshot"
            or event.t <= anchor
        ):
            return False
        found = find_item(event.data["payload"], expected_item_id)
        if found is None:
            return False
        return (
            expected_stack_count is None
            or int(found["item"]["stack_count"]) == expected_stack_count
        )

    return bot.wait_for(
        matches,
        timeout=timeout,
        description=(
            f"give {command_item_id} {count} 后应收到新 inventory_snapshot，"
            f"其中包含 {expected_item_id}"
        ),
    ).data["payload"]


def run(env) -> None:
    with env.new_bot("FoSc") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)

        # ── 放砧：真实 instance_id + 专属 forge_station 回执（镜像 production 链） ──
        snapshot = _give_and_wait(bot, ANVIL_ID)
        anvil = require_item(snapshot, ANVIL_ID)

        assert bot.position is not None, (
            "需要 pos_look 后的 bot.position 来定砧位（wait_for_ready 应已保证）"
        )
        px, py, pz = (int(v) for v in bot.position)
        # 与 production_forge_station_real_place 的 (px-2,py,pz) 错开，避免同套
        # e2e 里站台位置撞车（station.rs 只拦 position 唯一，无 air/距离门）。
        station_pos = (px, py, pz - 3)

        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "forge_station_place",
                "v": 1,
                "x": station_pos[0],
                "y": station_pos[1],
                "z": station_pos[2],
                "item_instance_id": int(anvil["item"]["instance_id"]),
                "station_tier": ANVIL_TIER,
            }
        )
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_station",
            lambda p: tuple(p["pos"]) == station_pos
            and p["tier"] == ANVIL_TIER
            and not p["has_session"],
            timeout=45.0,
            description=(
                f"放砧成功应专属回推 forge_station（pos={station_pos}, tier={ANVIL_TIER}, "
                "has_session=false）"
            ),
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], ANVIL_ID) is None,
            timeout=45.0,
            description="放砧后砧应从背包消耗",
        )

        # ── 学图谱：给残卷 → forge_learn_blueprint → 残卷消耗 ────────────────
        _give_and_wait(bot, SCROLL_ID)
        anchor = last_event_time(bot)
        _forge_learn_blueprint(bot, BLUEPRINT_ID)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], SCROLL_ID) is None,
            timeout=45.0,
            description=f"forge_learn_blueprint({BLUEPRINT_ID}) 后残卷应从背包消耗",
        )

        # ── 备料 sui_tie×3 → 起炉受理（ling_feng_v0 要 3 块 sui_tie） ─────────
        material_snapshot = _give_and_wait(
            bot,
            MATERIAL,
            expected_item_id=MATERIAL_ITEM_ID,
            count=3,
            expected_stack_count=3,
        )

        anchor = last_event_time(bot)
        _forge_start_session(bot, station_pos, BLUEPRINT_ID, [(MATERIAL, 3)])
        session_payload = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["blueprint_id"] == BLUEPRINT_ID and p["current_step"] == "billet",
            timeout=45.0,
            description=(
                f"起炉受理应推 forge_session（blueprint_id={BLUEPRINT_ID}, "
                "current_step=billet）"
            ),
        ).data["payload"]
        session_id = session_payload["session_id"]
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], MATERIAL_ITEM_ID) is None,
            timeout=45.0,
            description=f"起炉受理原子扣料后 {MATERIAL_ITEM_ID} 应从背包消失",
        )

        # ── Billet → Tempering → Inscription ────────────────────────────────
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["session_id"] == session_id and p["current_step"] == "tempering",
            timeout=45.0,
            description="step_advance 后应推 current_step=tempering 的 forge_session 快照",
        )

        for beat in TEMPERING_PATTERN:
            _forge_tempering_hit(bot, session_id, beat, ticks_remaining=1)
            time.sleep(0.05)  # 避免连发挤爆单 tick 的 C2S 处理顺序
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "tempering"
                and p["step_state"].get("hits") >= len(TEMPERING_PATTERN) - 1
            ),
            timeout=45.0,
            description=(
                f"喂满 {len(TEMPERING_PATTERN)} 拍（窗口内正确节拍）后 tempering "
                f"step_state hits 应 >= {len(TEMPERING_PATTERN) - 1}"
            ),
        )

        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["session_id"] == session_id and p["current_step"] == "inscription",
            timeout=45.0,
            description="第二次 step_advance 后应推 current_step=inscription 的 forge_session 快照",
        )

        # ── 备第一枚刻铭残卷（sharp_v0） ─────────────────────────────────────
        snapshot = _give_and_wait(bot, INSCRIPTION_SCROLL_ID)
        first_scroll = require_item(snapshot, INSCRIPTION_SCROLL_ID)
        assert int(first_scroll["item"]["stack_count"]) == 1, (
            f"备料应恰好 1 枚 {INSCRIPTION_SCROLL_ID}，实际 stack_count="
            f"{first_scroll['item']['stack_count']}"
        )

        # ── 负例一：不存在的 inscription_id → 无实例可扣，resync 且不消耗 ─────
        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, "nonexistent")
        resync = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], INSCRIPTION_SCROLL_ID) is not None,
            timeout=45.0,
            description=(
                f'inscription_id="nonexistent" 应触发 forge_inscription_scroll_missing 的 '
                f"resync inventory_snapshot，且 {INSCRIPTION_SCROLL_ID} 仍在背包（未消耗）"
            ),
        ).data["payload"]
        still_scroll = require_item(resync, INSCRIPTION_SCROLL_ID)
        assert int(still_scroll["item"]["stack_count"]) == 1, (
            "不存在 inscription_id 的提交不得消耗残卷（resync 后仍应 stack_count=1），"
            f"实际={still_scroll['item']['stack_count']}"
        )

        # ── 正例一：sharp_v0 入炉第 1 枚 → filled_slots 0→1，残卷消耗 ────────
        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["current_step"] == "inscription"
                and p["step_state"].get("kind") == "inscription"
                and p["step_state"].get("filled_slots") == 1
                and p["step_state"].get("max_slots") == 2
                and p["step_state"].get("failed") is False
            ),
            timeout=45.0,
            description=(
                "第 1 枚残卷入炉后 forge_session read-back 应为 "
                "{kind:inscription, filled_slots:1, max_slots:2, failed:false}"
            ),
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], INSCRIPTION_SCROLL_ID) is None,
            timeout=45.0,
            description="第 1 枚残卷入炉后应从背包消耗（forge_inscription_scroll_consumed）",
        )

        # ── 双发保护：消费后立即重发同一 inscription_id → 无第二实例，不再 +1 ──
        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], INSCRIPTION_SCROLL_ID) is None,
            timeout=45.0,
            description=(
                "重复提交应命中 forge_inscription_scroll_missing 的 resync "
                f"（{INSCRIPTION_SCROLL_ID} 仍不在背包，无第二实例可扣）"
            ),
        )
        double_granted = False
        try:
            bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "forge_session"
                and e.data["payload"].get("session_id") == session_id
                and (_inscription_step_state(e.data["payload"]) or {}).get("filled_slots") == 2,
                timeout=NEGATIVE_SETTLE,
                description="重复提交不得让 filled_slots 前进到 2（双发 grant bug 探测）",
            )
            double_granted = True
        except AssertionError:
            pass  # settle 窗口内没有 filled_slots=2 → 期望的否定结果
        assert not double_granted, (
            "同一 inscription_id 在消费后重发竟把 filled_slots 推到了 2——双发 grant bug"
        )

        # ── 正例二：第 2 枚残卷入炉 → filled_slots 1→2（max_slots=2 封顶） ────
        _give_and_wait(bot, INSCRIPTION_SCROLL_ID)
        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and (_inscription_step_state(p) or {}).get("filled_slots") == 2
                and (_inscription_step_state(p) or {}).get("max_slots") == 2
            ),
            timeout=45.0,
            description="第 2 枚残卷入炉后 filled_slots 应=2（max_slots=2 封顶）",
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], INSCRIPTION_SCROLL_ID) is None,
            timeout=45.0,
            description="第 2 枚残卷入炉后应从背包消耗",
        )

        # ── 负例二：步骤错配——探针残卷在包，推进到 Consecration 后提交 → 静默拒绝 ──
        probe = _give_and_wait(bot, INSCRIPTION_SCROLL_ID)
        assert int(require_item(probe, INSCRIPTION_SCROLL_ID)["item"]["stack_count"]) == 1, (
            "步骤错配负例的探针残卷应恰好 1 枚"
        )
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["session_id"] == session_id and p["current_step"] == "consecration",
            timeout=45.0,
            description="Inscription 填满后 step_advance 应推进到 consecration",
        )

        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        consumed_at_wrong_step = False
        try:
            bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "inventory_snapshot"
                and e.t > anchor
                and find_item(e.data["payload"], INSCRIPTION_SCROLL_ID) is None,
                timeout=NEGATIVE_SETTLE,
                description="Consecration 阶段提交不得消耗探针残卷（settle 窗口）",
            )
            consumed_at_wrong_step = True
        except AssertionError:
            pass  # settle 窗口内没有消费 snapshot → require_owned_active_step 静默拒绝
        assert not consumed_at_wrong_step, (
            f"Consecration 阶段提交 {INSCRIPTION_SCROLL_ID} 竟被消耗——步骤守卫失效"
        )
        # 正面读回：用无关 give 触发一次 inventory 回推，探针残卷应仍在。
        give_anchor = last_event_time(bot)
        bot.cmd("give fan_tie 1")
        readback = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > give_anchor
            and find_item(e.data["payload"], "fan_tie") is not None,
            timeout=45.0,
            description="用无关 give 触发一次 snapshot 读回背包状态",
        ).data["payload"]
        probe_after = require_item(readback, INSCRIPTION_SCROLL_ID)
        assert int(probe_after["item"]["stack_count"]) == 1, (
            f"步骤错配负例后探针残卷应原封不动（stack_count=1），实际="
            f"{probe_after['item']['stack_count']}"
        )

        # ── 收尾：Consecration → Done → outcome（证明会话在全部负例后仍能结算） ──
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        outcome = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_outcome",
            lambda p: p["session_id"] == session_id,
            timeout=45.0,
            description="最后一次 step_advance 应结算并推 forge_outcome",
        ).data["payload"]
        assert outcome["weapon_item"] == WEAPON_FLAWED_ID, (
            "测试 bot 境界 < Spirit，Consecration 确定性 Failed → Flawed 结局，"
            f"期望产物 {WEAPON_FLAWED_ID}，实际={outcome['weapon_item']}"
        )
