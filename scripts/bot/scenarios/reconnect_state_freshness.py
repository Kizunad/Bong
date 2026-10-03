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
from bot.scenarios._rejection_helpers import (
    AMBIENT_SERVER_DATA_TYPES,
    drain_event_stream,
)

DESCRIPTION = "断线重连后首批 server_data 快照集合完整，旧 session 不会替代新灌入"
MODULES = ["network", "persistence"]

# inventory_snapshot 是每个正常玩家 join 都必须收到的通用快照，不依赖可选的炼丹
# mock 或玩法事件。techniques_snapshot 取决于该身份 hydration 后实际可观察到的功法
# 状态；Fresh 身份可能没有可展示功法，不能把它提升为所有身份都必须收到的契约。
# 若某个身份的首包实际包含 techniques_snapshot，下面的首次/重连集合差异断言会把
# 它保留为该身份自己的重连契约。
REQUIRED_JOIN_PAYLOAD_TYPES = frozenset(
    {"inventory_snapshot"}
)
# narration 是欢迎流程的动态文案，不是 client Store 的状态快照；它可能只在首次
# 建档时出现，不能把它纳入两次连接的集合相等性断言。周期同步也不能进入集合：
# heartbeat、vortex_state 等由连接/世界调度器独立发出，首次和重连的收集窗可能各自
# 刚好跨过不同的 tick，观察到的集合天然不稳定。这里复用拒绝路径共用的显式 ambient
# 注册表，新增周期 payload 时只需在一处登记，避免把事件流误当成 join snapshot。
NON_SNAPSHOT_PAYLOAD_TYPES = frozenset({"narration"}) | AMBIENT_SERVER_DATA_TYPES
# join 的 deferred attach 可能在 inventory_snapshot 之后继续排出状态；用一个有界
# 收集窗覆盖这段尾流，而不是要求全连接进入静默。heartbeat / HUD 周期流不会阻塞
# 场景，且两次连接使用相同的收集窗来比较 join 快照集合。
JOIN_PAYLOAD_COLLECTION_SECONDS = 1.5
DISCONNECT_SETTLE_SECONDS = 0.75


def _server_data_types(bot) -> frozenset[str]:
    """返回本次连接已观察到的 server_data 类型集合。"""
    return frozenset(
        event.data["payload_type"]
        for event in bot.events_of("server_data")
        if event.data.get("payload_type")
    )


def _join_payload_types(bot, context: str) -> frozenset[str]:
    """等待 join hydration 完成，再在有界收集窗内取集合。"""
    wait_join_and_inventory(bot)
    drain_event_stream(
        bot,
        quiet_s=JOIN_PAYLOAD_COLLECTION_SECONDS,
        max_s=JOIN_PAYLOAD_COLLECTION_SECONDS,
    )
    payload_types = _server_data_types(bot) - NON_SNAPSHOT_PAYLOAD_TYPES
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
