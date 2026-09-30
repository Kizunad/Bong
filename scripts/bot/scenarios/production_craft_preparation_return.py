"""制作前材料真实移出、断线保管和未开工关闭返还。"""

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._craft_helpers import stage_material
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)
from bot.scenarios.production_craft_disconnect_resume import _reconnectable_session

DESCRIPTION = "未开始制作的材料跨连接保留，关闭后原实例原位全额返还"
MODULES = ["craft", "inventory", "persistence"]
RECIPE_ID = "workbench.weapon.stone_knife"


def run(env) -> None:
    with _reconnectable_session(env) as bot:
        initial = wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        cleared = wait_inventory_revision_after(bot, initial["revision"])
        give_anchor = last_event_time(bot)
        bot.cmd("give stone_chunk 2")
        bot.expect_chat("[dev] gave stone_chunk x2", timeout=10.0)
        before = wait_inventory_contains(
            bot,
            "stone_chunk",
            after_t=give_anchor,
            after_revision=cleared["revision"],
        )
        original = find_item(before, "stone_chunk")
        staged = stage_material(bot, RECIPE_ID, "stone_chunk", snapshot=before)
        assert find_item(staged, "stone_chunk") is None, "暂存后不能再从背包使用同一实例"
        assert staged["material_preparation"]["materials"] == [original["item"]]

    with _reconnectable_session(env) as bot:
        restored = wait_join_and_inventory(bot)
        assert restored["material_preparation"]["materials"] == [original["item"]], (
            "断线期间未开工材料必须保持实例、数量和属性"
        )
        bot.intent({"type": "craft_cancel", "v": 1})
        returned = wait_inventory_revision_after_matching(
            bot,
            restored["revision"],
            lambda state: not state.get("material_preparation", {}).get("materials")
            and find_item(state, "stone_chunk") is not None,
            "关闭未开工制作后完整返还材料",
        )
        assert find_item(returned, "stone_chunk") == original, "原位置空闲时必须原位原样返还"
