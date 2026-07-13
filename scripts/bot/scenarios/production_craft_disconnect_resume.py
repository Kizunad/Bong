"""制作断线恢复：预扣后的 inventory 与 CraftSession 跨连接同值恢复。"""

import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "制作断线恢复：session 暂停重连后可取消，退款仅提交一次"
MODULES = ["craft", "inventory", "persistence"]

RECIPE_ID = "workbench.weapon.stone_knife"


def _wait_session(bot, active: bool, timeout: float = 10.0) -> dict:
    existing = [
        event.data["payload"]
        for event in bot.events_of("server_data")
        if event.data.get("payload_type") == "craft_session_state"
        and event.data["payload"].get("active") is active
    ]
    if existing:
        return existing[-1]
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.data["payload_type"] == "craft_session_state"
        and event.data["payload"].get("active") is active,
        timeout=timeout,
        description=f"craft_session_state active={active}",
    ).data["payload"]


def run(env) -> None:
    with env.new_bot("CraftResume") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("give stone_chunk 2")
        bot.expect_chat("[dev] gave stone_chunk x2", timeout=10.0)
        bot.cmd("give wood_handle 2")
        bot.expect_chat("[dev] gave wood_handle x2", timeout=10.0)
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")
        time.sleep(1.0)

        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "craft_start",
                "v": 1,
                "recipe_id": RECIPE_ID,
                "quantity": 2,
            }
        )
        session = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "craft_session_state"
            and event.t > anchor
            and event.data["payload"].get("active") is True,
            timeout=10.0,
            description="断线前 active craft session",
        ).data["payload"]
        assert session.get("total_count") == 2
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.t > anchor
            and find_item(event.data["payload"], "stone_chunk") is None
            and find_item(event.data["payload"], "wood_handle") is None,
            timeout=10.0,
            description="断线前两种材料已预扣的 inventory_snapshot",
        )
        session = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "craft_session_state"
            and event.data["payload"].get("active") is True
            and event.data["payload"].get("elapsed_ticks", 0) >= 20,
            timeout=10.0,
            description="断线前已推进至少 20 tick 的 craft session",
        ).data["payload"]
        elapsed_before_disconnect = session["elapsed_ticks"]
        completed_before_disconnect = session["completed_count"]

    with env.new_bot("CraftResume") as bot:
        restored_inventory = wait_join_and_inventory(bot)
        restored = _wait_session(bot, True)
        assert restored.get("recipe_id") == RECIPE_ID, (
            f"重连应恢复 recipe={RECIPE_ID}，实际 {restored!r}"
        )
        assert restored.get("total_count") == 2, (
            f"重连应恢复 quantity=2，实际 {restored!r}"
        )
        assert elapsed_before_disconnect <= restored.get("elapsed_ticks", -1) <= (
            elapsed_before_disconnect + 5
        ), (
            "断线后 session 必须暂停且重连不能重新创建；"
            f"断线前 elapsed={elapsed_before_disconnect}，恢复={restored!r}"
        )
        assert restored.get("completed_count") == completed_before_disconnect, (
            "断线期间 completed_count 不得推进或回退；"
            f"断线前={completed_before_disconnect}，恢复={restored!r}"
        )
        assert find_item(restored_inventory, "stone_chunk") is None
        assert find_item(restored_inventory, "wood_handle") is None

        anchor = last_event_time(bot)
        bot.intent({"type": "craft_cancel", "v": 1})
        outcome = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "craft_outcome"
            and event.t > anchor
            and event.data["payload"].get("recipe_id") == RECIPE_ID,
            timeout=10.0,
            description="重连恢复 session 的取消 outcome",
        ).data["payload"]
        assert outcome.get("outcome") == "failed"
        assert outcome.get("reason") == 1
        assert outcome.get("material_returned") == 2, (
            f"重连取消应返还两份材料，实际 {outcome!r}"
        )
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")

    with env.new_bot("CraftResume") as bot:
        final_inventory = wait_join_and_inventory(bot)
        _wait_session(bot, False)
        assert find_item(final_inventory, "stone_chunk") is not None
        assert find_item(final_inventory, "wood_handle") is not None
        assert find_item(final_inventory, "stone_chunk")["item"]["stack_count"] == 1, (
            f"stone_chunk 退款必须恰好一次，实际 {final_inventory!r}"
        )
        assert find_item(final_inventory, "wood_handle")["item"]["stack_count"] == 1, (
            f"wood_handle 退款必须恰好一次，实际 {final_inventory!r}"
        )
        bot.assert_alive("制作断线恢复并取消后再次重连")
