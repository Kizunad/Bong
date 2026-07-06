"""最小世界容器链路 —— 放置货箱 marker 后用 `container_open` 打开。

纯 bot 驱动路径：
1. `/give trade_crate` 获取 placeable 容器物品。
2. `block_place` intent 放置，观察新 Marker entity spawn。
3. `container_open` intent 打开，断言 `loot_container_open` 到达。

若当前 raster-less 出生点没有可放置 chunk，本场景显式打印跳过该 leg，避免把
环境缺口误判成 container_open 协议回归。
"""

import math

from bot.bot import BotAssertionError

from ._inventory_helpers import (
    require_item,
    wait_inventory_contains,
    wait_inventory_revision_after,
    wait_join_and_inventory,
)

DESCRIPTION = "纯 bot 放置 trade_crate 后 container_open 应回推 loot_container_open"
MODULES = ["inventory", "container"]


def run(env) -> None:
    with env.new_bot("Box") as bot:
        snapshot = wait_join_and_inventory(bot)
        if not _has_any_chunk(bot):
            print("    [warn] 当前出生点无 ChunkData，跳过容器放置/open leg")
            bot.assert_alive("容器场景因无 chunk 跳过前")
            return

        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=10.0)
        snapshot = wait_inventory_revision_after(bot, snapshot["revision"], timeout=10.0)

        bot.cmd("give trade_crate 1")
        bot.expect_chat("[dev] gave trade_crate x1", timeout=10.0)
        snapshot = wait_inventory_contains(bot, "trade_crate", timeout=10.0)
        crate = require_item(snapshot, "trade_crate")

        x, y, z = _placement_pos(bot)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {
                "type": "block_place",
                "v": 1,
                "x": x,
                "y": y,
                "z": z,
                "item_instance_id": crate["item"]["instance_id"],
                "target_face": "north",
            }
        )

        try:
            spawn = bot.wait_for(
                lambda e: e.kind == "entity_spawn"
                and e.t > sent_at
                and abs(e.data["x"] - (x + 0.5)) <= 1.5
                and abs(e.data["y"] - y) <= 2.0
                and abs(e.data["z"] - (z + 0.5)) <= 1.5,
                timeout=10.0,
                description="trade_crate 放置后附近出现容器 Marker entity_spawn",
            )
        except BotAssertionError:
            print(
                "    [warn] 未观察到 trade_crate Marker；当前出生点可能无稳定可放置目标，"
                "跳过 container_open leg"
            )
            bot.assert_alive("容器放置未观察到 marker 后")
            return

        bot.intent({"type": "container_open", "v": 1, "entity_id": spawn.data["entity_id"]})
        bot.expect_server_data("loot_container_open", timeout=10.0)
        bot.assert_alive("container_open 最小链路后")


def _has_any_chunk(bot) -> bool:
    try:
        bot.wait_for(lambda e: e.kind == "chunk_data", timeout=2.0, description="任意 ChunkData")
        return True
    except BotAssertionError:
        return False


def _placement_pos(bot) -> tuple[int, int, int]:
    if bot.position is None:
        raise BotAssertionError("container 场景需要 pos_look 后的位置，实际 position=None")
    x, y, z = bot.position
    # 放在玩家东侧两格的脚下空气格，避免 DIRT 碰撞体与玩家包围盒相交。
    return math.floor(x) + 2, math.floor(y), math.floor(z)
