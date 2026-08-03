"""炼丹全链路：真炉放置 → 点火 → 投料 → 丹成 → 丹入包（+投料数量不符负分支）。

黑盒契约面（对齐 client_request_handler.rs handle_alchemy_*）：
- `alchemy_furnace_place{x,y,z,item_instance_id}` 必须用**真实 instance_id**
  （旧场景传 0 其实没放成炉——本场景从 inventory_snapshot 取真 id）。
- 点火在投料之前：`alchemy_ignite{furnace_pos,recipe_id}` 建会话；
  `alchemy_feed_slot` 在无会话时回 chat「尚未起炉」。
- 丹方 `ling_xi_wan_v1`（assets/alchemy/recipes/）：stage0 spirit_grass×3，
  fire 80 tick，qi_cost 5；zone 灵气门 MIN_ZONE_QI_TO_ALCHEMY=0.3
  （dev `/zone_qi set` 抬高保证过闸）。
- 数量必须精确：count≠required 回 chat「投料数量不符：需要 3，收到 2」。
- 投料后必须**火候干预**：`alchemy_intervention{kind:adjust_temp/inject_qi}`
  ——不调温不注真元 resolver 判 Waste 出渣（负→正对照的物理）。
- **结算触发 = `alchemy_take_back`（收丹）**：handler 快进剩余 fire tick、
  end_session 并 resolve → `alchemy_outcome_resolved` 只发给施术者，随后丹经
  alchemy_outcome_grant 入包（干预到位时 bucket=Perfect）。
"""

import math

from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = "炼丹全链路：真炉放置→点火→投料→丹成入包 + 投料数量不符负分支"
MODULES = ["alchemy", "inventory"]

RECIPE_ID = "ling_xi_wan_v1"
PILL_ID = "ling_xi_wan"


def _assert_recipe_guidance(
    payload: dict,
    *,
    active: bool,
    status_label: str,
    stage_completed: bool,
) -> None:
    assert payload["recipe_id"] == RECIPE_ID, (
        f"alchemy_session recipe_id 应为 {RECIPE_ID}，实际 {payload}"
    )
    assert payload["active"] is active, (
        f"alchemy_session active 应为 {active}，实际 {payload}"
    )
    assert payload["target_ticks"] == 80, (
        f"真实丹方 target_ticks=80 不得在 proto wire 上归零，实际 {payload}"
    )
    assert math.isclose(payload["temp_target"], 0.30, abs_tol=1e-9), (
        f"真实丹方 temp_target=0.30 不得归零，实际 {payload}"
    )
    assert math.isclose(payload["temp_band"], 0.15, abs_tol=1e-9), (
        f"真实丹方 temp_band=0.15 不得归零，实际 {payload}"
    )
    assert math.isclose(payload["qi_target"], 5.0, abs_tol=1e-9), (
        f"真实丹方 qi_target=5.0 不得归零，实际 {payload}"
    )
    assert payload["status_label"] == status_label, (
        f"alchemy_session status_label 应为 {status_label!r}，实际 {payload}"
    )
    assert len(payload["stages"]) == 1, (
        f"ling_xi_wan_v1 应保留唯一投料 stage，实际 {payload}"
    )
    stage = payload["stages"][0]
    assert stage == {
        "at_tick": 0,
        "window": 0,
        "summary": "spirit_grass×3",
        "completed": stage_completed,
        "missed": False,
    }, f"投料 stage 必须完整且状态准确，实际 {stage}"


def _assert_outcome(payload: dict) -> None:
    expected = {
        "v": 1,
        "type": "alchemy_outcome_resolved",
        "bucket": "perfect",
        "recipe_id": RECIPE_ID,
        "pill": PILL_ID,
        "quality": 1.0,
        "toxin_amount": 0.15,
        "toxin_color": "gentle",
        "qi_gain": 0.0,
        "side_effect_tag": None,
        "flawed_path": False,
        "damage": None,
        "meridian_crack": None,
    }
    assert payload == expected, (
        "真实炼丹结算必须在 field 14 保留 Perfect 丹丸的全部 optional/enum 字段；"
        f"expected={expected}, actual={payload}"
    )


def run(env) -> None:
    with env.new_bot("Brew") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)

        # 场地与资源铺垫
        bot.cmd("qi max 50")
        bot.cmd("qi set 50")
        bot.cmd("give furnace_fantie 1")
        bot.cmd("give spirit_grass 3")
        wait_inventory_contains(bot, "furnace_fantie")
        wait_inventory_contains(bot, "spirit_grass")

        snapshot = latest_inventory_snapshot(bot)
        furnace = require_item(snapshot, "furnace_fantie")
        furnace_iid = int(furnace["item"]["instance_id"])

        assert bot.position is not None, "需要 pos_look 后的 bot.position 来定炉位"
        px, py, pz = (int(v) for v in bot.position)
        fpos = (px + 2, py, pz + 2)

        # zone 灵气门：所在 zone 抬到 1.0（fallback/raster 世界通用）
        bot.cmd("zone_qi set spawn 1.0")

        # 放真炉
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_furnace_place",
                "v": 1,
                "x": fpos[0],
                "y": fpos[1],
                "z": fpos[2],
                "item_instance_id": furnace_iid,
            }
        )
        # 放炉的可观察面 = 炉物品被消耗（放置本身无专属回执，同 forge 砧——
        # 观察面弱点记录）；炉快照在 open_furnace 时推送
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and find_item(e.data["payload"], "furnace_fantie") is None
            and e.t > anchor,
            timeout=10.0,
            description=(
                "真实 instance_id 放炉后炉应从背包消耗（alchemy_furnace_place_"
                "consumed）——不消耗说明 instance 校验没走通（旧场景传 0 即假放置）"
            ),
        )

        # 打开炉 → 炉快照
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_open_furnace",
                "v": 1,
                "furnace_pos": list(fpos),
            }
        )
        open_furnace = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_furnace"
            and e.t > anchor,
            timeout=10.0,
            description="open_furnace 后应收到 alchemy_furnace 快照（炉阶/完整度可观察）",
        )
        assert open_furnace.data["payload"]["has_session"] is False, (
            f"首次 open 时炉内尚无 session，实际 {open_furnace.data['payload']}"
        )
        open_session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_session"
            and e.t > anchor,
            timeout=10.0,
            description="open_furnace 还必须推送 inactive 空 session 以清除旧 HUD",
        )
        assert open_session.data["payload"] == {
            "v": 1,
            "type": "alchemy_session",
            "recipe_id": "",
            "active": False,
            "elapsed_ticks": 0,
            "target_ticks": 0,
            "temp_current": 0.0,
            "temp_target": 0.0,
            "temp_band": 0.0,
            "qi_injected": 0.0,
            "qi_target": 0.0,
            "status_label": "未起炉",
            "stages": [],
            "interventions_recent": [],
        }, f"首次 open 的空炉快照必须清空完整 HUD 契约，实际 {open_session.data['payload']}"

        # 负分支：无会话投料 → chat「尚未起炉」
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_feed_slot",
                "v": 1,
                "furnace_pos": list(fpos),
                "slot_idx": 0,
                "material": "spirit_grass",
                "count": 3,
            }
        )
        bot.wait_for(
            lambda e: e.kind == "chat" and e.t > anchor and "尚未起炉" in e.data["text"],
            timeout=10.0,
            description=(
                "无丹火会话时投料应回 chat「尚未起炉」——会话前置校验丢失"
                "会让投料静默吞材料"
            ),
        )

        # 点火（先于投料）
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_ignite",
                "v": 1,
                "furnace_pos": list(fpos),
                "recipe_id": RECIPE_ID,
            }
        )
        ignite_session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_session"
            and e.t > anchor,
            timeout=10.0,
            description="ignite 后应收到 alchemy_session（丹火会话开启）",
        )
        _assert_recipe_guidance(
            ignite_session.data["payload"],
            active=True,
            status_label="炼制中",
            stage_completed=False,
        )
        assert ignite_session.data["payload"]["elapsed_ticks"] == 0
        assert math.isclose(ignite_session.data["payload"]["temp_current"], 0.0)
        assert math.isclose(ignite_session.data["payload"]["qi_injected"], 0.0)

        # 负分支：数量不符（需要 3 投 2）
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_feed_slot",
                "v": 1,
                "furnace_pos": list(fpos),
                "slot_idx": 0,
                "material": "spirit_grass",
                "count": 2,
            }
        )
        bot.wait_for(
            lambda e: e.kind == "chat" and e.t > anchor and "投料数量不符" in e.data["text"],
            timeout=10.0,
            description=(
                "count=2≠required 3 应回 chat「投料数量不符」——数量校验分支丢失"
                "会让配比玩法退化"
            ),
        )

        # 正确投料
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_feed_slot",
                "v": 1,
                "furnace_pos": list(fpos),
                "slot_idx": 0,
                "material": "spirit_grass",
                "count": 3,
            }
        )
        feed_session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_session"
            and e.t > anchor,
            timeout=10.0,
            description="正确投料后应收到 alchemy_session 更新（投料已受理）",
        )
        _assert_recipe_guidance(
            feed_session.data["payload"],
            active=True,
            status_label="炼制中",
            stage_completed=True,
        )

        # 火候干预：调温到目标带 + 注真元付 qi_cost——不干预 = 温度 0/无真元，
        # resolver 判 Waste 出渣（实测 bucket=Waste）
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "alchemy_intervention",
                "v": 1,
                "furnace_pos": list(fpos),
                "intervention": {"kind": "adjust_temp", "temp": 0.30},
            }
        )
        bot.intent(
            {
                "type": "alchemy_intervention",
                "v": 1,
                "furnace_pos": list(fpos),
                "intervention": {"kind": "inject_qi", "qi": 8.0},
            }
        )
        intervention_session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_session"
            and math.isclose(e.data["payload"]["temp_current"], 0.30, abs_tol=1e-9)
            and e.data["payload"]["qi_injected"] >= 8.0
            and e.t > anchor,
            timeout=10.0,
            description="干预后应收到 alchemy_session 更新（temp/qi 已写入）",
        )
        _assert_recipe_guidance(
            intervention_session.data["payload"],
            active=True,
            status_label="炼制中",
            stage_completed=True,
        )
        assert math.isclose(
            intervention_session.data["payload"]["temp_current"], 0.30, abs_tol=1e-9
        )
        assert math.isclose(
            intervention_session.data["payload"]["qi_injected"], 8.0, abs_tol=1e-9
        )
        assert len(intervention_session.data["payload"]["interventions_recent"]) == 2, (
            "调温与注气两次干预必须一起进入 session 快照"
        )

        with env.new_bot("Bystander") as bystander:
            wait_join_and_inventory(bystander)
            bystander_anchor = last_event_time(bystander)

            # 收丹 = 结算触发：handle_alchemy_take_back 快进剩余 fire tick、
            # end_session 并 resolve（丹成不靠墙钟等待）
            anchor = last_event_time(bot)
            bot.intent(
                {
                    "type": "alchemy_take_back",
                    "v": 1,
                    "furnace_pos": list(fpos),
                    "slot_idx": 0,
                }
            )
            outcome = bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "alchemy_outcome_resolved"
                and e.t > anchor,
                timeout=15.0,
                description=(
                    "take_back 结算必须向施术者发送深解码 field 14 "
                    "alchemy_outcome_resolved"
                ),
            )
            _assert_outcome(outcome.data["payload"])

            barrier_anchor = last_event_time(bystander)
            bystander.cmd("supply_coffin barrier")
            barrier = bystander.wait_for(
                lambda event: event.kind == "chat"
                and event.t > barrier_anchor
                and "[dev] supply_coffin barrier passed" in event.data["text"],
                timeout=10.0,
                description="结算后的 server-authoritative tick barrier",
            )
            leaked = [
                event
                for event in bystander.events_of("server_data")
                if bystander_anchor < event.t <= barrier.t
                and event.data["payload_type"] == "alchemy_outcome_resolved"
            ]
            assert leaked == [], (
                "炼丹结算是施术者私有回执，旁观 Bot 不得收到 field 14；"
                f"actual={leaked}"
            )

            finished_furnace = bot.wait_for(
                lambda e: e.kind == "server_data"
                and e.data["payload_type"] == "alchemy_furnace"
                and e.t > anchor
                and e.data["payload"]["has_session"] is False,
                timeout=15.0,
                description="take_back 后炉快照必须显示 session 已移除",
            )
        assert finished_furnace.data["payload"]["tier"] == 1
        finished_session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "alchemy_session"
            and e.t > anchor
            and e.data["payload"]["active"] is False,
            timeout=15.0,
            description="take_back 后必须发送保留 guidance 的 inactive finished session",
        )
        _assert_recipe_guidance(
            finished_session.data["payload"],
            active=False,
            status_label="已结束",
            stage_completed=True,
        )
        assert finished_session.data["payload"]["elapsed_ticks"] == 80
        assert math.isclose(finished_session.data["payload"]["temp_current"], 0.30)
        assert math.isclose(finished_session.data["payload"]["qi_injected"], 8.0)
        wait_inventory_contains(bot, PILL_ID, timeout=15.0)

        bot.assert_alive("炼丹全链路之后")
