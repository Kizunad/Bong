"""制作满包取消：退款落地、重复取消幂等、掉落可拾回。"""

import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "制作满包取消：双 cancel 仅退款一次，落地材料可完整拾回"
MODULES = ["craft", "inventory"]

RECIPE_ID = "workbench.weapon.stone_knife"
REFUND_ITEMS = {"stone_chunk", "wood_handle"}


def _latest_drops(bot, timeout: float = 10.0) -> list[dict]:
    observed = [
        event.data["payload"]
        for event in bot.events_of("server_data")
        if event.data.get("payload_type") == "dropped_loot_sync"
    ]
    if observed:
        return observed[-1]["drops"]
    return bot.expect_server_data("dropped_loot_sync", timeout=timeout).data["payload"][
        "drops"
    ]


def _wait_drop_ids(
    bot, expected_ids: set[int], tracked_ids: set[int], after_t: float, timeout: float = 10.0
):
    event = bot.wait_for(
        lambda candidate: candidate.kind == "server_data"
        and candidate.data["payload_type"] == "dropped_loot_sync"
        and candidate.t > after_t
        and {
            drop["instance_id"] for drop in candidate.data["payload"]["drops"]
        }.intersection(tracked_ids)
        == expected_ids,
        timeout=timeout,
        description=f"包含退款掉落 instance_id={sorted(expected_ids)} 的 dropped_loot_sync",
    )
    return event.data["payload"]["drops"]


def _inventory_count(snapshot: dict, item_id: str) -> int:
    return sum(
        placed["item"]["stack_count"]
        for placed in snapshot.get("placed_items", [])
        if placed["item"]["item_id"] == item_id
    )


def run(env) -> None:
    refund_ids: set[int]
    with env.new_bot("CraftRefund") as bot:
        wait_join_and_inventory(bot)
        baseline_ids = {drop["instance_id"] for drop in _latest_drops(bot)}

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("give stone_chunk 2")
        bot.expect_chat("[dev] gave stone_chunk x2", timeout=10.0)
        bot.cmd("give wood_handle 2")
        bot.expect_chat("[dev] gave wood_handle x2", timeout=10.0)
        wait_inventory_contains(bot, "stone_chunk")
        wait_inventory_contains(bot, "wood_handle")
        time.sleep(1.0)

        start_anchor = last_event_time(bot)
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
            and event.t > start_anchor
            and event.data["payload"].get("active") is True,
            timeout=10.0,
            description="quantity=2 craft_start 后的 active session",
        ).data["payload"]
        assert session.get("recipe_id") == RECIPE_ID, (
            f"session recipe 应为 {RECIPE_ID}，实际 {session.get('recipe_id')!r}"
        )
        assert session.get("total_count") == 2, (
            f"session total_count 应为 2，实际 {session.get('total_count')!r}"
        )

        consumed = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "inventory_snapshot"
            and event.t > start_anchor
            and find_item(event.data["payload"], "stone_chunk") is None
            and find_item(event.data["payload"], "wood_handle") is None,
            timeout=10.0,
            description="craft_start 后 stone_chunk/wood_handle 均已预扣",
        ).data["payload"]
        assert _inventory_count(consumed, "stone_chunk") == 0
        assert _inventory_count(consumed, "wood_handle") == 0

        # Misc 每栈 16；默认背包 3x3、贴身口袋 2x3。分两次填满两个容器。
        bot.cmd("give grass_fiber 144")
        bot.expect_chat("[dev] gave grass_fiber x144", timeout=10.0)
        bot.cmd("give grass_fiber 96")
        bot.expect_chat("[dev] gave grass_fiber x96", timeout=10.0)
        wait_inventory_contains(bot, "grass_fiber")

        cancel_anchor = last_event_time(bot)
        bot.intent({"type": "craft_cancel", "v": 1})
        bot.intent({"type": "craft_cancel", "v": 1})
        outcome = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "craft_outcome"
            and event.t > cancel_anchor
            and event.data["payload"].get("recipe_id") == RECIPE_ID,
            timeout=10.0,
            description="满包双 cancel 后唯一 craft_outcome",
        ).data["payload"]
        assert outcome.get("outcome") == "failed", (
            f"取消应走 failed outcome，实际 {outcome!r}"
        )
        assert outcome.get("reason") == 1, (
            f"取消 reason 应为 PlayerCancelled(1)，实际 {outcome.get('reason')!r}"
        )
        assert outcome.get("material_returned") == 2, (
            "quantity=2 时两种原料各 floor(2*0.7)=1，"
            f"material_returned 应为 2，实际 {outcome.get('material_returned')!r}"
        )
        time.sleep(1.0)
        matching_outcomes = [
            event
            for event in bot.events_of("server_data")
            if event.data.get("payload_type") == "craft_outcome"
            and event.t > cancel_anchor
            and event.data["payload"].get("recipe_id") == RECIPE_ID
        ]
        assert len(matching_outcomes) == 1, (
            f"同帧双 cancel 只能产生一个 outcome，实际 {len(matching_outcomes)}"
        )

        drops = bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.data["payload_type"] == "dropped_loot_sync"
            and event.t > cancel_anchor
            and len(
                [
                    drop
                    for drop in event.data["payload"]["drops"]
                    if drop["instance_id"] not in baseline_ids
                    and drop["item"]["item_id"] in REFUND_ITEMS
                ]
            )
            == 2,
            timeout=10.0,
            description="满包取消后包含两份新退款的 dropped_loot_sync",
        ).data["payload"]["drops"]
        new_refunds = [
            drop
            for drop in drops
            if drop["instance_id"] not in baseline_ids
            and drop["item"]["item_id"] in REFUND_ITEMS
        ]
        assert len(new_refunds) == 2, (
            f"应新增两条退款掉落，实际 {new_refunds!r}"
        )
        assert {drop["item"]["item_id"] for drop in new_refunds} == REFUND_ITEMS
        assert all(drop["item"]["stack_count"] == 1 for drop in new_refunds)
        refund_ids = {drop["instance_id"] for drop in new_refunds}
        assert len(refund_ids) == 2 and 0 not in refund_ids, (
            f"退款掉落 instance_id 必须非零且互异，实际 {refund_ids}"
        )

    with env.new_bot("CraftRefund") as bot:
        wait_join_and_inventory(bot)
        reconnect_drops = _latest_drops(bot)
        assert refund_ids.issubset(
            {drop["instance_id"] for drop in reconnect_drops}
        ), "重连后必须仍能观察并回收取消时的地面退款"

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        remaining = set(refund_ids)
        for instance_id in sorted(refund_ids):
            anchor = last_event_time(bot)
            bot.intent(
                {
                    "type": "pickup_dropped_item",
                    "v": 1,
                    "instance_id": instance_id,
                }
            )
            remaining.remove(instance_id)
            if remaining:
                _wait_drop_ids(bot, remaining, refund_ids, anchor)
            else:
                bot.wait_for(
                    lambda event: event.kind == "server_data"
                    and event.data["payload_type"] == "dropped_loot_sync"
                    and event.t > anchor
                    and refund_ids.isdisjoint(
                        {
                            drop["instance_id"]
                            for drop in event.data["payload"]["drops"]
                        }
                    ),
                    timeout=10.0,
                    description="两份退款拾取后 dropped_loot_sync 不再包含退款 ID",
                )

        stone = wait_inventory_contains(bot, "stone_chunk")
        wood = wait_inventory_contains(bot, "wood_handle")
        assert _inventory_count(stone, "stone_chunk") == 1
        assert _inventory_count(wood, "wood_handle") == 1
        bot.assert_alive("制作退款落地并拾回之后")
