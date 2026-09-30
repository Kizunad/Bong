"""连接生命周期：重连后的首批状态必须重新灌满。

同一身份首次加入和异常断线后的再次加入都应收到相同的 join-time
``bong:server_data`` payload 集合。场景先记录首次加入的集合，再以第二次加入的
首批集合做包含断言，避免把旧连接的静态状态或偶然的一条周期推送当成新 session
的 hydration。
"""

from __future__ import annotations

import time

from bot.bot import BotAssertionError
from bot.scenarios._inventory_helpers import wait_join_and_inventory
from bot.scenarios._rejection_helpers import wait_for_server_data_quiet

DESCRIPTION = "断线重连后首批 server_data 快照集合完整，旧 session 不会替代新灌入"
MODULES = ["network", "persistence"]

# 这些快照由每个正常玩家的 join 组件生成，不依赖可选的炼丹 mock 或玩法事件。
REQUIRED_JOIN_PAYLOAD_TYPES = frozenset(
    {"inventory_snapshot", "skill_snapshot", "techniques_snapshot"}
)
SERVER_DATA_QUIET_SECONDS = 0.75
SERVER_DATA_QUIET_TIMEOUT_SECONDS = 8.0
DISCONNECT_SETTLE_SECONDS = 0.75


def _server_data_types(bot) -> frozenset[str]:
    """返回本次连接已观察到的 server_data 类型集合。"""
    return frozenset(
        event.data["payload_type"]
        for event in bot.events_of("server_data")
        if event.data.get("payload_type")
    )


def _join_payload_types(bot, context: str) -> frozenset[str]:
    """等待 join hydration 静默后取集合，避免把尚未排出的首包误判为缺失。"""
    wait_join_and_inventory(bot)
    wait_for_server_data_quiet(
        bot,
        quiet_s=SERVER_DATA_QUIET_SECONDS,
        max_s=SERVER_DATA_QUIET_TIMEOUT_SECONDS,
    )
    payload_types = _server_data_types(bot)
    missing = REQUIRED_JOIN_PAYLOAD_TYPES - payload_types
    if missing:
        raise BotAssertionError(
            f"{context} 缺少正常玩家首包快照 {sorted(missing)}；"
            f"实际收到 {sorted(payload_types)}"
        )
    return payload_types


def run(env) -> None:
    with env.new_bot("Fresh") as bot:
        first_join_types = _join_payload_types(bot, "首次加入")
        bot.assert_alive("首次加入完成后连接保持")

    # 让服务端完成异常断线清理，再建立同身份的新连接；不发送 logout，覆盖
    # disconnect handler 和持久化 hydration 的真实边界。
    time.sleep(DISCONNECT_SETTLE_SECONDS)

    with env.new_bot("Fresh") as bot:
        reconnect_types = _join_payload_types(bot, "重连")
        missing = first_join_types - reconnect_types
        assert not missing, (
            "重连首批 server_data 必须至少包含首次加入已观察到的完整快照集合；"
            f"缺少 {sorted(missing)}，首次={sorted(first_join_types)}，"
            f"重连={sorted(reconnect_types)}"
        )
        bot.assert_alive("重连首包灌入后连接保持")
