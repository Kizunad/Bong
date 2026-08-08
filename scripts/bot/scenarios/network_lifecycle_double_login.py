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

from bot.scenarios._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "同一身份双开登录：两连接共存不互踢，分散后重连干净"
MODULES = ["network", "multibot"]


def run(env) -> None:
    # 第一连接
    with env.new_bot("Dup") as first:
        wait_join_and_inventory(first)
        first.assert_alive("第一连接就绪后")

        # 第二连接：同 username
        with env.new_bot("Dup") as second:
            wait_join_and_inventory(second)
            second.assert_alive("第二连接（同身份双开）join 后未被踢")
            first.assert_alive("第二连接 join 后，第一连接仍存活（双开互不干扰）")

        # 第二连接退出（with 块结束即断线）
        time.sleep(1.5)
        first.assert_alive("第二连接断线后，第一连接仍存活（清理不误伤同身份另一连接）")

    # 第一连接也退出
    time.sleep(1.5)

    # 同身份重连：server 不应被双开的散乱状态污染
    with env.new_bot("Dup") as third:
        wait_join_and_inventory(third)
        third.assert_alive("双开全分散后同身份重连，干净 join")
