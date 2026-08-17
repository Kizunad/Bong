"""连接生命周期：同一身份双开登录。

同一 username 两条并发连接（等价玩家双开/重复登录）——server 是 offline mode、
无「踢旧登新」逻辑（valence 每连接各建独立 Client entity，username 只用于装载
持久化状态）。本场景锁住容错面：

- 第二条同身份连接能完成 join + pos_look，不被踢、不 panic
- 第一条连接保持存活（双开互不干扰）
- 任一方断线后，另一方连接不受影响（清理路径不误伤同身份的另一连接）
- 全部分散后，同身份重连仍能干净 join（server 状态未被污染）
"""

import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "同一身份双开登录：两连接共存不互踢，分散后重连干净"
MODULES = ["network", "multibot"]

KEEPALIVE_TIMEOUT = 25.0


def _wait_keepalive_after(bot, after: float, description: str):
    """等待严格晚于 phase marker 的 KeepAlive，排除 join 阶段的历史事件。"""
    return bot.wait_for(
        lambda event: event.kind == "keepalive" and event.t > after,
        timeout=KEEPALIVE_TIMEOUT,
        description=(
            f"{description}（必须是 t>{after:.3f}s 的新 KeepAlive，排除历史事件）"
        ),
    )


def _assert_surviving_connection(bot, context: str) -> None:
    """把 socket 存活与 reader 侧 connection_lost 观察一起钉住。"""
    bot.assert_alive(context)
    lost = bot.events_of("connection_lost")
    assert not lost, (
        f"{context} 后 surviving connection 不得出现 connection_lost，"
        f"实际 loss_events={lost!r}"
    )


def run(env) -> None:
    # 第一连接
    with env.new_bot("Dup") as first:
        wait_join_and_inventory(first)
        # marker 紧邻这个等待动作：只接受 join 完成后的新心跳，不能拿 join 期间的
        # KeepAlive 证明第一连接在 join 后仍由 server 持续维护。
        first_join_marker = last_event_time(first)
        _wait_keepalive_after(first, first_join_marker, "第一连接 join 后仍持续心跳")
        first.assert_alive("第一连接 join 后收到新 KeepAlive")

        # 第二连接：同 username
        with env.new_bot("Dup") as second:
            wait_join_and_inventory(second)
            # marker 紧邻第二连接的 continuity probe，排除第二连接 join 阶段已经
            # 收到的历史 KeepAlive。
            second_join_marker = last_event_time(second)
            _wait_keepalive_after(second, second_join_marker, "第二连接 join 后仍持续心跳")
            second.assert_alive("第二连接（同身份双开）收到新 KeepAlive 后未被踢")

            # 第二连接 join 后，第一连接仍是 surviving connection；marker 必须在
            # 这个等待动作前立即取，避免第一连接更早的心跳冒充本阶段证据。
            first_survives_join_marker = last_event_time(first)
            _wait_keepalive_after(
                first,
                first_survives_join_marker,
                "第二连接 join 后第一连接仍持续心跳",
            )
            # 关键的交叉观察：第一连接这一等待不能掩盖第二连接在它自己的
            # KeepAlive 之后稍晚才被 server 清理的情况。
            _assert_surviving_connection(
                second,
                "第二连接 join 后第一连接收到新 KeepAlive",
            )
            _assert_surviving_connection(
                first,
                "第二连接 join 后第一连接仍存活（双开互不干扰）",
            )

        # 第二连接退出（with 块结束即断线）；给 server 清理路径一个短暂的
        # 观察窗口，再用新的 marker 只接受清理完成后的第一连接心跳。
        time.sleep(1.5)
        first_cleanup_marker = last_event_time(first)
        _wait_keepalive_after(
            first,
            first_cleanup_marker,
            "第二连接清理后第一连接仍持续心跳",
        )
        _assert_surviving_connection(
            first,
            "第二连接断线后收到新 KeepAlive，第一连接仍存活（清理不误伤同身份另一连接）",
        )

    # 第一连接也退出
    time.sleep(1.5)

    # 同身份重连：server 不应被双开的散乱状态污染
    with env.new_bot("Dup") as third:
        wait_join_and_inventory(third)
        third_join_marker = last_event_time(third)
        _wait_keepalive_after(third, third_join_marker, "双开全分散后第三连接 join 后仍持续心跳")
        _assert_surviving_connection(
            third,
            "双开全分散后第三连接收到新 KeepAlive 并保持连接",
        )
