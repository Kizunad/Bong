"""武器锻造全链路：放砧→学图谱→起炉（拒因+受理）→淬炼×10→结算→产物入包。

plan-forge-session-entry-wiring-v1 P0/P1/P2 全部接线完成后的验收场景（AGENTS.md
§15 本 plan 验收门）。黑盒契约面：

- `forge_station_place{x,y,z,item_instance_id,station_tier}` 必须用真实
  instance_id——station.rs `forge_station_item_by_instance` 按 id 校验。放砧成功
  推专属 `forge_station` S2C 回执（P2 新接线，对齐 alchemy open_furnace 模式；此前
  只能从 inventory 扣除间接推断）。
- `forge_start_session` 用 `station_pos:(x,y,z)` 寻址（§4.1#3 决议，非旧版
  `station_id` 整数）；受理后推 `forge_station`+`forge_session`+`forge_blueprint_book`
  三件套（P2 新接线，`send_forge_snapshots_to_player` 真实调用点）。
- 起炉输入料在引擎侧原子校验+扣除（§4.1#4 CRUX）：声明的 materials 与图谱要求一致
  但背包实际持有不足时，整体拒绝、背包分文不动——本场景专门断言这条（材料不足→
  revision 不变、fan_tie 数量不变）。
- `za_gang` 是纯世界矿物（plan-mineral-v1 §2.2 无 ItemRegistry TOML 模板），
  `/give` 新增 dev-only fallback（复用生产 `MineralDropEvent` 链路）才能在 bot 场景
  里配出这份材料；落地后 item_id 是 `mineral_za_gang`（`mineral_id=za_gang` 才是
  forge 扣料的匹配键）。
- 淬炼击键 `forge_tempering_hit{session_id,beat,ticks_remaining}` 按图谱 pattern
  精确喂 10 拍（L,L,H,L,F,H,L,F,H,H）拿 Perfect 结局；每拍后端推 `forge_session`
  快照（P2 新接线，session-only echo）。
- `forge_step_advance{session_id}` 两次：Billet→Tempering、Tempering→Done。
  Done 后推 `forge_outcome`（P2 新接线，`send_forge_outcome_to_player` 真实调用
  点）+ 产物自动入包（`inventory_bridge::forge_outcome_to_inventory` +
  `emit_changed_inventory_snapshots` 自动回推，不需要手动 resync）。

超时口径：stage 等待统一 45s（非 10s）——CI e2e 串行连跑 ~20 场景到本场景时
server TPS 已显著退化（长跑攒实体的已知类，参 craft 场景 180s 先例），10s 窗口
在 fresh server 绿、CI 上边缘假红（两轮实测分别卡在相邻 stage）。断言本身不放宽。
"""

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
)

DESCRIPTION = "放砧→学图谱→起炉拒因/受理→淬炼×10→结算→qing_feng_sword 入包+输入料扣光"
MODULES = ["forge", "inventory"]

# qing_feng_v0 需要 za_gang（MineralId::ZaGang.forge_tier_min()==2，见
# mineral/types.rs），凡铁砧（tier 1）炼不动——必须用灵铁砧（tier 2）。
# station_tier_min（图谱级门槛）和逐材料 forge_tier_min（validate_with 校验）
# 是两层独立校验：qing_feng_v0.station_tier_min=1 只挡得住"砧太差连起炉都不配"，
# 材料级门槛才是实际卡 za_gang 的那道（bp.validate_with 静默 continue + chat
# 回执"炼不动"，不是"材料不足"——实测踩过这个坑）。
ANVIL_ID = "ling_iron_anvil"
ANVIL_TIER = 2
SCROLL_ID = "blueprint_scroll_qing_feng"
BLUEPRINT_ID = "qing_feng_v0"
WEAPON_ID = "qing_feng_sword"
# qing_feng_v0.json 图谱正典 tempering pattern（server/assets/forge/blueprints）。
TEMPERING_PATTERN = ["L", "L", "H", "L", "F", "H", "L", "F", "H", "H"]


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
    with env.new_bot("Forge") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=30.0)

        # ── 放砧：真实 instance_id + 专属 forge_station 回执 ──────────────
        bot.cmd(f"give {ANVIL_ID} 1")
        wait_inventory_contains(bot, ANVIL_ID)
        snapshot = latest_inventory_snapshot(bot)
        anvil = require_item(snapshot, ANVIL_ID)

        assert bot.position is not None, (
            "需要 pos_look 后的 bot.position 来定砧位（wait_for_ready 应已保证）"
        )
        px, py, pz = (int(v) for v in bot.position)
        station_pos = (px - 2, py, pz)

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
                "has_session=false）——P2 新接线，对齐 alchemy open_furnace 回执模式"
            ),
        )
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], ANVIL_ID) is None,
            timeout=45.0,
            description="真实 instance_id 放砧后砧应从背包消耗（forge_station_place_consumed）",
        )

        # ── 学图谱：给残卷 → forge_learn_blueprint → 残卷消耗 ─────────────
        bot.cmd(f"give {SCROLL_ID} 1")
        wait_inventory_contains(bot, SCROLL_ID)
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

        # ── 拒因分支：声明材料与图谱要求一致，但背包持有不足 → 整体拒绝 ──
        # （§4.1#4 CRUX anti-cheat：不核实持有会让这条变成合法造物漏洞）
        bot.cmd("give fan_tie 2")
        pre_reject_snapshot = wait_inventory_contains(bot, "fan_tie")
        pre_reject_revision = pre_reject_snapshot["revision"]

        anchor = last_event_time(bot)
        _forge_start_session(
            bot, station_pos, BLUEPRINT_ID, [("fan_tie", 4), ("za_gang", 1)]
        )
        bot.wait_for(
            lambda e: e.kind == "chat" and "材料不足" in e.data["text"] and e.t > anchor,
            timeout=45.0,
            description=(
                "声明 materials 与图谱一致但背包只有 fan_tie x2（< 需求 4，za_gang 0 "
                "< 需求 1）时应整体拒绝并回执「材料不足」，不得凭空建会话"
            ),
        )

        # 用一次无关的 /give（顺带备齐 za_gang）验证拒绝路径分文未扣：
        # revision 只应因这次 give 前进 1 步，fan_tie 数量应仍是拒绝前的 2。
        anchor = last_event_time(bot)
        bot.cmd("give za_gang 1")
        post_reject_snapshot = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], "mineral_za_gang") is not None,
            timeout=45.0,
            description="给 za_gang 后应出现含 mineral_za_gang 的 inventory_snapshot",
        ).data["payload"]
        assert post_reject_snapshot["revision"] == pre_reject_revision + 1, (
            f"起炉拒因路径必须分文不动：拒绝前 revision={pre_reject_revision}，"
            f"给 za_gang 后应恰好 +1（只有这次 give 生效），实际="
            f"{post_reject_snapshot['revision']}——若 >+1 说明拒绝路径偷偷扣过料"
        )
        fan_tie_after_reject = require_item(post_reject_snapshot, "fan_tie")
        assert fan_tie_after_reject["item"]["stack_count"] == 2, (
            f"拒因路径不应触碰 fan_tie 持有量，期望仍为 2，实际="
            f"{fan_tie_after_reject['item']['stack_count']}"
        )

        # ── 受理分支：补齐 fan_tie 到 4 → 起炉受理 ────────────────────────
        # 必须带时间锚：fan_tie 在拒因分支已出现过一次（stack_count=2），
        # 无锚扫描会命中那份旧快照而非补齐后 stack_count=4 的新快照。
        anchor = last_event_time(bot)
        bot.cmd("give fan_tie 2")
        snapshot = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and (find_item(e.data["payload"], "fan_tie") or {}).get("item", {}).get(
                "stack_count"
            )
            == 4,
            timeout=45.0,
            description="补齐 fan_tie 到 4（2+2）后应出现 stack_count=4 的 inventory_snapshot",
        ).data["payload"]
        fan_tie_before_start = require_item(snapshot, "fan_tie")
        assert fan_tie_before_start["item"]["stack_count"] == 4, (
            f"起炉前应恰好持有 fan_tie x4，实际={fan_tie_before_start['item']['stack_count']}"
        )

        anchor = last_event_time(bot)
        _forge_start_session(
            bot, station_pos, BLUEPRINT_ID, [("fan_tie", 4), ("za_gang", 1)]
        )
        session_payload = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: p["blueprint_id"] == BLUEPRINT_ID and p["current_step"] == "billet",
            timeout=45.0,
            description=(
                f"起炉受理应推 forge_session（blueprint_id={BLUEPRINT_ID}, "
                "current_step=billet）——send_forge_snapshots_to_player 真实调用点"
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
            and find_item(e.data["payload"], "fan_tie") is None
            and find_item(e.data["payload"], "mineral_za_gang") is None,
            timeout=45.0,
            description=(
                "起炉受理原子扣料后 fan_tie/mineral_za_gang 应从背包彻底消失"
                "（4 fan_tie + 1 za_gang 全额扣光）"
            ),
        )

        # ── Billet → Tempering ───────────────────────────────────────────
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

        # ── 淬炼×10（精确匹配图谱 pattern 拿 Perfect） ───────────────────
        for beat in TEMPERING_PATTERN:
            _forge_tempering_hit(bot, session_id, beat, ticks_remaining=1)
            time.sleep(0.05)  # 避免 10 连发挤爆单 tick 的 C2S 处理顺序
        _wait_forge_payload_after(
            bot,
            anchor,
            "forge_session",
            lambda p: (
                p["session_id"] == session_id
                and p["step_state"].get("kind") == "tempering"
                and p["step_state"].get("hits") == len(TEMPERING_PATTERN)
                and p["step_state"].get("misses") == 0
            ),
            timeout=45.0,
            description=(
                f"精确喂满 {len(TEMPERING_PATTERN)} 拍应使 tempering step_state "
                f"hits={len(TEMPERING_PATTERN)} misses=0（Perfect 前置条件）"
            ),
        )

        # ── Tempering → Done → outcome + 产物入包 ────────────────────────
        anchor = last_event_time(bot)
        _forge_step_advance(bot, session_id)
        outcome = _wait_forge_payload_after(
            bot,
            anchor,
            "forge_outcome",
            lambda p: p["session_id"] == session_id,
            timeout=45.0,
            description=(
                "第二次 step_advance 应结算收尾并推 forge_outcome——"
                "send_forge_outcome_to_player 真实调用点"
            ),
        ).data["payload"]
        assert outcome["bucket"] == "perfect", (
            f"10 拍精确命中、billet 材料精确匹配应判定 Perfect，实际 bucket={outcome['bucket']}"
        )
        assert outcome["weapon_item"] == WEAPON_ID, (
            f"期望产出 {WEAPON_ID}，实际={outcome['weapon_item']}"
        )
        assert outcome["achieved_tier"] == 2, (
            f"billet 精确成 + tempering Perfect 应达 tier=2（法器），实际={outcome['achieved_tier']}"
        )
        assert not outcome["flawed_path"], "Perfect 结局不应走 flawed_fallback 路径"

        final_snapshot = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > anchor
            and find_item(e.data["payload"], WEAPON_ID) is not None,
            timeout=45.0,
            description=(
                f"结算后应自动回推含 {WEAPON_ID} 的 inventory_snapshot"
                "（forge_outcome_to_inventory + emit_changed_inventory_snapshots 自动回推）"
            ),
        ).data["payload"]
        weapon = require_item(final_snapshot, WEAPON_ID)
        assert abs(float(weapon["item"]["stack_count"]) - 1) < 1e-9, (
            f"产物应恰好入包 +1，实际 stack_count={weapon['item']['stack_count']}"
        )
        assert find_item(final_snapshot, "fan_tie") is None, (
            "结算后 fan_tie 输入料应彻底从背包消失（起炉时已原子扣光，不应复现）"
        )
        assert find_item(final_snapshot, "mineral_za_gang") is None, (
            "结算后 za_gang 输入料应彻底从背包消失（起炉时已原子扣光，不应复现）"
        )
