"""连接生命周期：动作中途断连后同身份重连，状态从持久化恢复。

Bot 完成 join + 库存就绪 → `/give stone_chunk 2`（真实玩法状态变更 = "动作"）→
按步行速度移动一小段 → 不打招呼直接掐断 socket（无 logout 包，等价异常断线）→
同身份重连。

server 必须：
- 断连时不崩不踢，把当前库存+位置落盘（despawn_disconnected_clients 持久化路径）
- 重连时从持久化恢复（库存仍在、位置贴近断连前最后坐标），而不是回出生点/空包

「中途」的强调点：断连发生在一个真实动作**之后**（库存已写入、位置已移动），
不是 join 完就断。位置断言用宽容偏差吸收运动确认的微小差异。
"""

import math
import time

from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "动作中途断连重连：库存与位置从持久化恢复，server 不崩不踢"
MODULES = ["network", "persistence", "inventory"]

TARGET_ITEM = "stone_chunk"
GIVE_COUNT = 2
# move_to 走 5 格后重连位置应贴近断连前最后坐标；4m 偏差容纳 server 运动确认差异
POSITION_TOLERANCE = 4.0
# 掐断 socket 后给 server 1~2 tick 检测断连并落盘，避免重连抢在清理前读到旧切片
DISCONNECT_PERSIST_GRACE = 1.5


def _move_and_record(bot):
    if bot.position is None:
        raise BotAssertionError("move_to 前需要已知 bot.position，实际 None")
    x, y, z = bot.position
    bot.move_to(x + 5.0, y, z, speed=4.0)
    time.sleep(0.3)
    if bot.position is None:
        raise BotAssertionError("move_to 后 bot.position 仍为 None")
    return tuple(bot.position)


def run(env) -> None:
    # ---- 第一段连接：做出真实动作（发库存 + 移动），然后异常断线 ----
    with env.new_bot("Abrupt") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd(f"give {TARGET_ITEM} {GIVE_COUNT}")
        bot.expect_chat(f"[dev] gave {TARGET_ITEM} x{GIVE_COUNT}", timeout=10.0)
        wait_inventory_contains(bot, TARGET_ITEM)
        pre_disconnect_position = _move_and_record(bot)
        bot.assert_alive("动作执行后、掐断前")

    # 退出 with 块 = 直接关 socket，无 logout 包
    time.sleep(DISCONNECT_PERSIST_GRACE)

    # ---- 第二段连接：同身份重连，状态应从持久化恢复 ----
    with env.new_bot("Abrupt") as bot:
        restored_inventory = wait_join_and_inventory(bot)
        restored_item = find_item(restored_inventory, TARGET_ITEM)
        assert restored_item is not None, (
            f"重连后应恢复 {TARGET_ITEM}（动作中途断连应把库存落盘再恢复），"
            f"实际 snapshot 未找到；containers={restored_inventory.get('containers')}"
        )
        assert restored_item["item"]["stack_count"] == GIVE_COUNT, (
            f"重连后 {TARGET_ITEM} 数量应仍为 {GIVE_COUNT}，"
            f"实际 {restored_item['item']['stack_count']}"
        )

        restored_position = bot.position
        assert restored_position is not None, "重连后应收到 pos_look 恢复坐标"
        distance = math.dist(pre_disconnect_position, restored_position)
        assert distance <= POSITION_TOLERANCE, (
            "重连位置应从持久化恢复（贴近断连前最后坐标），而不是回出生点或丢失；"
            f"断连前 {pre_disconnect_position}，重连后 {restored_position}，"
            f"偏差 {distance:.1f}m > 容忍 {POSITION_TOLERANCE}m"
        )
        bot.assert_alive("同身份重连并恢复状态后连接保持")
