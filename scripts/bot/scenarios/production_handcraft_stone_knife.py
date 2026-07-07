"""手搓（handcraft）全链路：材料发现解锁 → craft_start → 会话 → 产物入包。

黑盒契约面：
- 手搓配方 station=None（`workbench.weapon.stone_knife`，craft/workbench_recipes.rs），
  不需要工作台实体——fallback 世界可全链路走通。
- 解锁走「材料发现」被动路径（craft_emit.rs apply_material_discovery_unlock）：
  /give 原料后下个 inventory tick 自动解锁，无需显式 learn。
- C2S `craft_start{v,recipe_id}`（client_request.rs::CraftStart）→
  S2C `craft_session_state{active:true,...}` → time_ticks 走完 →
  `craft_outcome`(Completed) + 产物进 inventory_snapshot。
- 会话按服务器 Update 帧计 tick（**非** CultivationClock，`/time advance`
  不加速）——400 tick 在 debug ~10 TPS 下约 40s，outcome 等待给足余量。
"""

import time

from bot.scenarios._inventory_helpers import wait_inventory_contains, wait_join_and_inventory
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = "手搓石刀全链路：give 原料自动解锁 → craft_start → craft_outcome → 石刀入包"
MODULES = ["craft", "inventory"]

RECIPE_ID = "workbench.weapon.stone_knife"
OUTPUT_ID = "stone_knife"


def run(env) -> None:
    with env.new_bot("Craft") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)

        bot.cmd("give stone_chunk 1")
        bot.cmd("give wood_handle 1")
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")
        time.sleep(1.0)  # 材料发现解锁跑一个 inventory tick

        anchor = last_event_time(bot)
        bot.intent({"type": "craft_start", "v": 1, "recipe_id": RECIPE_ID})

        session = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "craft_session_state"
            and e.t > anchor,
            timeout=10.0,
            description=(
                "craft_start 后应收到 craft_session_state（会话开启回执）——"
                "收不到说明手搓解锁/受理链路断了"
            ),
        )
        payload = session.data["payload"]
        assert payload.get("recipe_id") == RECIPE_ID, (
            f"craft_session_state 应回显 recipe_id={RECIPE_ID}（会话与请求一致），"
            f"实际 {payload.get('recipe_id')!r}"
        )

        outcome = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "craft_outcome"
            and e.t > anchor,
            timeout=100.0,
            description=(
                "craft 会话走完后应收到 craft_outcome——收不到说明会话 tick "
                "或完工结算断链"
            ),
        )
        outcome_blob = str(outcome.data["payload"])
        assert OUTPUT_ID in outcome_blob and "ailed" not in outcome_blob, (
            f"craft_outcome 应为 Completed 且产物={OUTPUT_ID}（材料齐全的手搓配方"
            f"不应失败），实际 payload={outcome_blob[:300]}"
        )

        wait_inventory_contains(bot, OUTPUT_ID, timeout=10.0)
        bot.assert_alive("手搓全链路之后")
