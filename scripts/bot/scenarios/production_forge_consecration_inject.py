"""GAP17 ForgeConsecrationInject：开光注真元（装备持久化状态）。

链路：放砧→学图谱→起炉→淬炼×15→铭文×2→进入 Consecration →
dev 设 realm/qi → 开光注真元。断言：

1. 钳制核心：qi_current=100 时注入 120 → qi_injected==100.0（绝不超真元）。
2. 状态读回：注入后 /qi set 50 再注入 60 → 钳到 50 → qi_injected 累积到 150.0
   （两次注入总量 ≤ 累计真元 150，守恒）。
3. 负例：fake session / qi_amount=-1 / NaN / 步骤不匹配 → 全部拒绝，
   随后唯一一次合法注入的 qi_injected 精确等于 100.0 即证明负例零副作用。

wire 契约（bot 用 proto_min 解 bong:server_data，非 JSON schema）：
ForgeSessionDataV1.step_state 的 Consecration 形如
{"kind":"consecration","qi_injected":X,"qi_required":Y}——没有
color_imprint/min_realm（init_state_for 置 None，resolve_consecration 才算 color）。
"""

from __future__ import annotations

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._cultivation_helpers import (
    _set_qi_and_wait,
    _set_qi_max_and_wait,
)
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = "放砧→学图谱→起炉→淬炼×15→铭文×2→开光注真元：clamp + 累积读回 + 三负例"
MODULES = ["forge", "inventory"]

# ling_feng_v0.json 正典（server/assets/forge/blueprints）：
# tempering 15 拍 pattern、window_ticks 8、miss_allowed 1；
# inscription slots 2、required_scroll_count 2（fail_chance 只在步末结算 roll，
# 且 deterministic_step_roll 由 session_id 种子——结构上必然推进到 Consecration）；
# consecration qi_cost 80.0、min_realm "Spirit"。
#
# 砧档位修正（CI run 31448705964 / 31451142220 两次实证）：ling_feng_v0 的
# required 材料 sui_tie 是 MineralRarity::Xi（mineral/types.rs:135-137）→
# forge_tier_min()==3（Metal → rarity().tier().min(3)，types.rs:188-192）→
# 灵铁砧（tier 2）炼不动：handle_start_forge_requests 的 bp.validate_with 走
# TierMismatch → 只发 MineralFeedbackEvent 聊天回执「炼不动」（events.rs，
# 无任何日志行）→ 静默拒绝。必须用玄铁砧（tier 3，station.tier>=station_tier_min
# 仍满足 ling_feng_v0 的 station_tier_min=2）。
ANVIL_ID = "xuan_iron_anvil"
ANVIL_TIER = 3
BLUEPRINT_SCROLL_ID = "blueprint_scroll_ling_feng"
BLUEPRINT_ID = "ling_feng_v0"
SUI_TIE_ID = "sui_tie"
SUI_TIE_ITEM_ID = "mineral_sui_tie"  # 矿物落包即 mineral_ 前缀（za_gang 先例）
INSCRIPTION_SCROLL_ID = "inscription_scroll_qi_amplify_v0"
INSCRIPTION_ID = "qi_amplify_v0"
TEMPERING_PATTERN = ["H", "L", "F", "H", "L", "F", "F", "H", "L", "F", "H", "H", "F", "L", "H"]
QI_COST = 80.0


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


def _forge_consecration_inject(bot, session_id, qi_amount):
    bot.intent(
        {
            "type": "forge_consecration_inject",
            "v": 1,
            "session_id": session_id,
            "qi_amount": qi_amount,
        }
    )


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


def run(env) -> None:
    with env.new_bot("FoCj") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)

        # ── 备料：玄铁砧×1 + sui_tie×3 + 铭文残卷×2 + 图谱残卷×1 ──────────
        bot.cmd(f"give {ANVIL_ID} 1")
        wait_inventory_contains(bot, ANVIL_ID)
        bot.cmd(f"give {SUI_TIE_ID} 3")
        wait_inventory_contains(bot, SUI_TIE_ITEM_ID)
        bot.cmd(f"give {INSCRIPTION_SCROLL_ID} 2")
        wait_inventory_contains(bot, INSCRIPTION_SCROLL_ID)
        bot.cmd(f"give {BLUEPRINT_SCROLL_ID} 1")
        wait_inventory_contains(bot, BLUEPRINT_SCROLL_ID)

        snapshot = latest_inventory_snapshot(bot)
        anvil = require_item(snapshot, ANVIL_ID)
        sui_tie = require_item(snapshot, SUI_TIE_ITEM_ID)
        assert int(sui_tie["item"]["stack_count"]) == 3, (
            f"sui_tie 应恰好备 3 块（图谱材料需求），实际 stack_count="
            f"{sui_tie['item']['stack_count']}"
        )

        assert bot.position is not None, (
            "需要 pos_look 后的 bot.position 来定砧位（wait_for_ready 应已保证）"
        )
        px, py, pz = (int(v) for v in bot.position)
        station_pos = (px - 2, py, pz)

        # ── 放砧（真实 instance_id + tier 3：sui_tie 是 Xi 稀有度，forge_tier_min==3，
        #    灵铁砧 tier 2 会被 validate_with 静默拒绝——见文件头砧档位修正） ──
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
            description="真实 instance_id 放砧后砧应从背包消耗",
        )

        # ── 学图谱 → 残卷消耗 ────────────────────────────────────────────
        anchor = last_event_time(bot)
        _forge_learn_blueprint(bot, BLUEPRINT_ID)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], BLUEPRINT_SCROLL_ID) is None,
            timeout=45.0,
            description=f"forge_learn_blueprint({BLUEPRINT_ID}) 后残卷应从背包消耗",
        )
        # 学后即起炉有同帧竞态：fresh bot 无 LearnedBlueprints 组件，handler 走
        # commands 延迟插入，同帧到达的 start_session 在权威系统读到的仍是「未学」
        # → debug 级静默拒绝 + 仅 chat 回执「尚未习得图谱」（release 日志不可见，
        # e2e run 31448705964 实证：dispatch 后权威零日志、零快照）。跨 1 帧即可，
        # 睡 0.15s（≈3 tick）留足余量；同文件淬炼连发已有 0.05s 先例。
        time.sleep(0.15)

        # ── 起炉受理：sui_tie×3 原子扣料 → billet ────────────────────────
        anchor = last_event_time(bot)
        _forge_start_session(bot, station_pos, BLUEPRINT_ID, [(SUI_TIE_ID, 3)])
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
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_blueprint_book",
            lambda p: any(entry["id"] == BLUEPRINT_ID for entry in p["learned"]),
            timeout=45.0,
            description=f"起炉受理应一并推 forge_blueprint_book（含已学 {BLUEPRINT_ID}）",
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], SUI_TIE_ITEM_ID) is None,
            timeout=45.0,
            description="起炉受理原子扣料后 sui_tie×3 应从背包彻底消失",
        )

        # ── Billet → Tempering ──────────────────────────────────────────
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

        # ── 淬炼×15（精确匹配图谱 pattern；brief 放宽：hits ≥ 14） ────────
        for beat in TEMPERING_PATTERN:
            _forge_tempering_hit(bot, session_id, beat, ticks_remaining=1)
            time.sleep(0.05)  # 避免 15 连发挤爆单 tick 的 C2S 处理顺序
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "tempering"
                and p["step_state"].get("hits") >= 14
            ),
            timeout=45.0,
            description=(
                f"喂满 {len(TEMPERING_PATTERN)} 拍应使 tempering step_state hits>=14"
                "（miss_allowed=1，放宽断言不要求 15/15）"
            ),
        )

        # ── Tempering → Inscription ─────────────────────────────────────
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["session_id"] == session_id and p["current_step"] == "inscription",
            timeout=45.0,
            description="step_advance 后应推 current_step=inscription 的 forge_session 快照",
        )

        # ── 负例 1：步骤不匹配——在 Inscription 注入开光真元被拒 ───────────
        # require_owned_active_step（expected=Consecration）双闸：当前步不是
        # Consecration 且 pending 不是 Consecration → 静默拒绝（无事件、无快照）。
        # 可观测性由「推进到 Consecration 后基线 qi_injected==0.0」承接。
        anchor = last_event_time(bot)
        _forge_consecration_inject(bot, session_id, 10.0)

        # ── 铭文×2：qi_amplify_v0 残卷填入 2/2 槽 ────────────────────────
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "inscription"
                and p["step_state"].get("filled_slots") == 1
            ),
            timeout=45.0,
            description=f"铭文第 1 卷后应推 filled_slots=1 的 inscription step_state",
        )
        anchor = last_event_time(bot)
        _forge_inscription_scroll(bot, session_id, INSCRIPTION_ID)
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "inscription"
                and p["step_state"].get("filled_slots") == 2
            ),
            timeout=45.0,
            description=(
                f"铭文第 2 卷后应推 filled_slots=2 的 inscription step_state"
                "（required_scroll_count=2，填满即达标）"
            ),
        )

        # ── Inscription → Consecration（状态读回：进入步骤的基线） ────────
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        baseline = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["current_step"] == "consecration"
                and p["step_state"].get("kind") == "consecration"
            ),
            timeout=45.0,
            description=(
                "step_advance 后应推 current_step=consecration + step_state.kind="
                "consecration 的 forge_session 快照"
            ),
        ).data["payload"]
        assert baseline["active"] is True, (
            f"进入 Consecration 后 session 应仍 active（未 done），实际 active={baseline['active']}"
        )
        assert baseline["step_state"]["qi_injected"] == 0.0, (
            f"进入 Consecration 基线 qi_injected 应为 0.0（含 Inscription 期负例注入被拒的"
            f"证明），实际={baseline['step_state']['qi_injected']}"
        )
        assert baseline["step_state"]["qi_required"] == QI_COST, (
            f"图谱正典 qi_cost 应为 {QI_COST}，实际={baseline['step_state']['qi_required']}"
        )

        # ── dev 铺垫：Spirit 境界 + qi_current=100（fresh bot qi_max=10，须先提 max） ──
        bot.cmd("realm set spirit")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        _set_qi_max_and_wait(bot, 100.0)
        _set_qi_and_wait(bot, 100.0)

        # ── 负例 2/3/4：fake session / qi_amount=-1 / NaN → 全部拒绝 ──────
        # handler（client_request_handler.rs handle_forge_consecration_inject）先查
        # `!qi_amount.is_finite() || qi_amount < 0.0` 再查 require_owned_active_step：
        # - fake session_id：forge_sessions 无此行 → 拒
        # - -1.0：amount 门 → 拒
        # - float("nan")：json.dumps 产出 NaN（非标准 JSON）→ 信封解析拒 → 无 inject
        # 三者都不产生 ConsecrationInject 事件 → 不推快照、不动真元。
        _forge_consecration_inject(bot, 999999999, 50.0)
        _forge_consecration_inject(bot, session_id, -1.0)
        _forge_consecration_inject(bot, session_id, float("nan"))

        # ── 钳制核心：注入 120（qi_current=100）→ qi_injected==100.0 ──────
        # 若上面任一负例漏注入（fake +50 / -1 / NaN / 步骤不匹配 +10），
        # 本值必然偏离 100.0——精确断言即负例零副作用的合并证明。
        anchor = last_event_time(bot)
        _forge_consecration_inject(bot, session_id, 120.0)
        clamped = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "consecration"
                and p["step_state"].get("qi_injected") == 100.0
            ),
            timeout=45.0,
            description=(
                "qi_current=100 时注入 120 应钳到 100.0（以 ECS 真元为准，"
                "绝不信任 client 上报量）"
            ),
        ).data["payload"]
        assert clamped["current_step"] == "consecration", (
            f"注入后 current_step 应仍为 consecration，实际={clamped['current_step']}"
        )
        assert clamped["active"] is True, (
            f"注入后 session 应仍 active，实际 active={clamped['active']}"
        )
        assert clamped["step_state"]["qi_required"] == QI_COST, (
            f"qi_required 应恒为图谱 qi_cost {QI_COST}，实际="
            f"{clamped['step_state']['qi_required']}"
        )

        # ── 状态读回：把上限收至 50 并补满，再注入 60 → 钳到 50 → 累积 150.0 ──
        # 守恒：两次注入总量 100+50=150 ≤ 累计真元 100+50=150；ledger 侧
        # zone 余额同步 150（玩家真元 → zone ledger 搬运，不凭空增减）。把 qi_max
        # 同步收至 50 可阻止命令确认与注入处理之间的自然回复产生 epsilon，使这里
        # 真正验证精确 cap，而不是用浮点容差掩盖非确定性。
        _set_qi_max_and_wait(bot, 50.0)
        _set_qi_and_wait(bot, 50.0)
        anchor = last_event_time(bot)
        _forge_consecration_inject(bot, session_id, 60.0)
        accumulated = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "consecration"
                and p["step_state"].get("qi_injected") == 150.0
            ),
            timeout=45.0,
            description=(
                "补真元 50 后注入 60 应再钳到 50，qi_injected 累积到 150.0"
                "（持久化状态读回 + 守恒）"
            ),
        ).data["payload"]
        assert accumulated["active"] is True, (
            f"二次注入后 session 应仍 active，实际 active={accumulated['active']}"
        )
