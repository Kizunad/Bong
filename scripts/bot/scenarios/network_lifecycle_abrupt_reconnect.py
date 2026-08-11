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

from bot.bot import BotAssertionError
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_contains,
    wait_join_and_inventory,
)

DESCRIPTION = "动作中途断连重连：库存与位置从持久化恢复，server 不崩不踢"
MODULES = ["network", "persistence", "inventory"]

TARGET_ITEM = "stone_chunk"
GIVE_COUNT = 2
# move_to 走 MOVE_DISTANCE 格后重连位置应贴近断连前最后坐标；POSITION_TOLERANCE 偏差
# 容纳 server 运动确认差异。MOVE_VERIFY_TOLERANCE 钉死「动作前置」：记录断连前坐标前
# 必须确认位移确实发生，否则原地不动也会被误记为断连前坐标、重连回出生点照样过 4m 断言。
# 两处距离断言都只用**水平（XZ 平面）**位移：权威 Position 有周期性纵向抬升
# （~8s 一步 +10，与 mineral_probe Y 带同根），垂直漂移与 move/持久化正确性正交，
# 算进 3D 距离会把正确实现误判成「动作没发生/位置没恢复」。
MOVE_DISTANCE = 5.0
MOVE_VERIFY_TOLERANCE = 1.0
POSITION_TOLERANCE = 4.0


def _horizontal_distance(a: tuple, b: tuple) -> float:
    return math.dist((a[0], a[2]), (b[0], b[2]))
# 掐断 socket 后给 server 1~2 tick 检测断连并落盘，避免重连抢在清理前读到旧切片
DISCONNECT_PERSIST_GRACE = 1.5


def _move_and_record(bot):
    if bot.position is None:
        raise BotAssertionError("move_to 前需要已知 bot.position，实际 None")
    start = tuple(bot.position)
    x, y, z = start
    bot.move_to(x + MOVE_DISTANCE, y, z, speed=4.0)
    time.sleep(0.3)
    if bot.position is None:
        raise BotAssertionError("move_to 后 bot.position 仍为 None")
    moved = tuple(bot.position)
    moved_distance = _horizontal_distance(start, moved)
    # 动作前置：位移必须实际发生（≈MOVE_DISTANCE）。若 move_to 被忽略/原地不动，
    # 记录下的「断连前坐标」实为出生点，重连回出生点也能通过 4m 断言，测不出位置持久化。
    # 只比水平位移：move_to 目标是 (x+MOVE_DISTANCE, y, z)，纯 X 轴向；权威 Y 抬升
    # （~8s 一步 +10）是正交漂移，算进 3D 距离会把正确移动误判成位移 10.3m≠5m。
    assert abs(moved_distance - MOVE_DISTANCE) <= MOVE_VERIFY_TOLERANCE, (
        f"move_to 应产生 {MOVE_DISTANCE}m 位移以确立动作前置，"
        f"实际位移 {moved_distance:.2f}m（start={start}，moved={moved}）"
    )
    return moved


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
        distance = _horizontal_distance(pre_disconnect_position, restored_position)
        # 只比水平位移：断连间隙 server 照常 tick，权威 Y 抬升（~8s 一步 +10）会让
        # 3D 偏差凭空 +10 而误红正确实现；回出生点则在 XZ 平面上有巨大偏移，水平
        # 距离照常能抓住。
        assert distance <= POSITION_TOLERANCE, (
            "重连位置应从持久化恢复（贴近断连前最后坐标），而不是回出生点或丢失；"
            f"断连前 {pre_disconnect_position}，重连后 {restored_position}，"
            f"水平偏差 {distance:.1f}m > 容忍 {POSITION_TOLERANCE}m"
        )
        bot.assert_alive("同身份重连并恢复状态后连接保持")
