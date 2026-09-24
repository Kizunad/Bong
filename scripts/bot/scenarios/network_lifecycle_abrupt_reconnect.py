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
import re
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
# Keep the restore window strictly below the movement acceptance window's lower
# bound: a 4m partial move followed by a reset to the join origin must not satisfy
# both assertions and masquerade as a successfully persisted position.
POSITION_TOLERANCE = 3.0


def _horizontal_distance(a: tuple, b: tuple) -> float:
    return math.dist((a[0], a[2]), (b[0], b[2]))


def _assert_restored_position(
    pre_disconnect_position: tuple, restored_position: tuple
) -> None:
    distance = _horizontal_distance(pre_disconnect_position, restored_position)
    # The restore window is intentionally narrower than the movement probe's
    # accepted lower bound. A partial 4m move followed by a reset to the join
    # origin must therefore fail instead of satisfying both lifecycle checks.
    assert distance <= POSITION_TOLERANCE, (
        "重连位置应从持久化恢复（贴近断连前最后坐标），而不是回出生点或丢失；"
        f"断连前 {pre_disconnect_position}，重连后 {restored_position}，"
        f"水平偏差 {distance:.1f}m > 容忍 {POSITION_TOLERANCE}m"
    )


# 掐断 socket 后给 server 1~2 tick 检测断连并落盘，避免重连抢在清理前读到旧切片
DISCONNECT_PERSIST_GRACE = 1.5


def _position_from_authoritative_event(event, context: str) -> tuple[float, float, float]:
    if event.kind != "pos_look":
        raise BotAssertionError(
            f"{context} 必须来自 server PositionLook，实际 kind={event.kind!r}"
        )
    relative_xyz_flags = int(event.data.get("flags", 0)) & 0x07
    if relative_xyz_flags != 0:
        raise BotAssertionError(
            f"{context} 必须携带绝对 XYZ；实际 flags={event.data.get('flags')!r}"
        )
    return tuple(float(event.data[axis]) for axis in ("x", "y", "z"))


def _join_authoritative_position(bot, context: str) -> tuple[float, float, float]:
    events = bot.events_of("pos_look")
    if not events:
        raise BotAssertionError(f"{context} 前未收到 server-authoritative PositionLook")
    # wait_join_and_inventory 已经等待过首个 pos_look；固定取首帧，避免任何
    # 后续同步帧改变「join position」这个位移基线。
    return _position_from_authoritative_event(events[0], context)


def _wait_authoritative_position_after(
    bot, after: float, context: str, timeout: float = 10.0
) -> tuple[float, float, float]:
    event = bot.wait_for(
        lambda candidate: candidate.kind == "pos_look" and candidate.t > after,
        timeout=timeout,
        description=f"{context}（t>{after:.3f}s 的新 server PositionLook）",
    )
    return _position_from_authoritative_event(event, context)


def _move_and_record(bot):
    # `Bot.move_to` 是客户端动作模拟：每个 C2S movement 包都会立即覆盖
    # `bot.position`，因此不能把该镜像当作位移已被 server 接受的证据。
    start = _join_authoritative_position(bot, "移动前 join 坐标")
    bot.position = start
    x, y, z = start
    bot.move_to(x + MOVE_DISTANCE, y, z, speed=4.0)
    time.sleep(0.3)

    # `/top` 只改 server 侧当前 Position 的 Y，X/Z 仍来自 server 已接受的移动；
    # 它会产生协议可见的绝对 PositionLook，既避免读取本地镜像，也不引入新的
    # gameplay API。若 server 只接受了部分移动，下面的 5m 前置断言必须失败。
    bot.cmd("top")
    top_feedback = bot.expect_chat("Teleported to top", timeout=10.0)
    top_y_match = re.search(r"Y=(-?\d+(?:\.\d+)?)", top_feedback.data["text"])
    if top_y_match is None:
        raise BotAssertionError(
            f"/top feedback 必须携带 server 计算的目标 Y，实际 {top_feedback.data['text']!r}"
        )
    expected_top_y = float(top_y_match.group(1))
    top_position_event = bot.wait_for(
        lambda candidate: candidate.kind == "pos_look"
        and candidate.t > top_feedback.t
        and math.isclose(
            float(candidate.data["y"]), expected_top_y, rel_tol=0.0, abs_tol=1.0e-6
        ),
        timeout=10.0,
        description=(
            f"移动后 server /top PositionLook（t>{top_feedback.t:.3f}s，"
            f"y={expected_top_y:g}）"
        ),
    )
    moved = _position_from_authoritative_event(top_position_event, "移动后权威坐标")
    moved_distance = _horizontal_distance(start, moved)
    # 动作前置：必须从 join 时的 server 坐标实际位移约 5m。若 move_to 被忽略或
    # server 只接受了 1m，而本地镜像走到 5m，不能把伪造的本地目标当作断连锚点。
    # 只比水平位移：权威 Y 抬升是正交漂移，不能让正确的 X/Z 移动误红。
    assert abs(moved_distance - MOVE_DISTANCE) <= MOVE_VERIFY_TOLERANCE, (
        f"server 权威移动应产生 {MOVE_DISTANCE}m 位移以确立动作前置，"
        f"实际位移 {moved_distance:.2f}m（join={start}，authoritative={moved}）"
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
        # 只比水平位移：断连间隙 server 照常 tick，权威 Y 抬升（~8s 一步 +10）会让
        # 3D 偏差凭空 +10 而误红正确实现；回出生点则在 XZ 平面上有巨大偏移，水平
        # 距离照常能抓住。
        _assert_restored_position(pre_disconnect_position, restored_position)
        bot.assert_alive("同身份重连并恢复状态后连接保持")
