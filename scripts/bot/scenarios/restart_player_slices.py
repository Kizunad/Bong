"""玩家 slice 的重启等价验收。

标准 bot-e2e 由外层脚本拥有 server 进程，场景不能自行结束一个不属于自己的
PID。因此这里覆盖同一持久化 hydrate 边界：先写入库存和位置，异常断线后以同一
身份重连，断言聚合玩家 slice 仍然可读。CI 的 staged smoke 负责真正的进程重启；
两者共同覆盖「断线保存」与「重启载入」两条边界。
"""

from __future__ import annotations

import time

from bot.scenarios._inventory_helpers import find_item, wait_inventory_contains, wait_join_and_inventory
from bot.scenarios.network_lifecycle_abrupt_reconnect import _move_and_record

DESCRIPTION = "玩家 slice 断线后重连：库存与位置仍从 durable hydrate 恢复"
MODULES = ["network", "persistence", "inventory"]

TARGET_ITEM = "stone_chunk"
GIVE_COUNT = 2
DISCONNECT_PERSIST_GRACE = 1.5


def run(env) -> None:
    with env.new_bot("Slice") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd(f"give {TARGET_ITEM} {GIVE_COUNT}")
        bot.expect_chat(f"[dev] gave {TARGET_ITEM} x{GIVE_COUNT}", timeout=10.0)
        inventory = wait_inventory_contains(bot, TARGET_ITEM)
        pre_disconnect_position = _move_and_record(bot)
        item = find_item(inventory, TARGET_ITEM)
        assert item is not None, "写入前必须能在权威 inventory snapshot 中找到测试物品"
        bot.assert_alive("玩家 slice 写入后保持连接")

    time.sleep(DISCONNECT_PERSIST_GRACE)

    with env.new_bot("Slice") as bot:
        restored_inventory = wait_join_and_inventory(bot)
        restored_item = find_item(restored_inventory, TARGET_ITEM)
        assert restored_item is not None, (
            f"重连后应恢复 {TARGET_ITEM}；实际 containers={restored_inventory.get('containers')}"
        )
        assert restored_item["item"]["stack_count"] == GIVE_COUNT, (
            f"重连后 {TARGET_ITEM} 数量应为 {GIVE_COUNT}，"
            f"实际 {restored_item['item']['stack_count']}"
        )
        assert bot.position is not None, "重连后应收到权威 PositionLook"
        horizontal_distance = ((bot.position[0] - pre_disconnect_position[0]) ** 2 +
                               (bot.position[2] - pre_disconnect_position[2]) ** 2) ** 0.5
        assert horizontal_distance <= 3.0, (
            "重连后位置应贴近断线前 durable position，"
            f"水平偏差={horizontal_distance:.1f}m"
        )
        bot.assert_alive("玩家 slice hydrate 后保持连接")
