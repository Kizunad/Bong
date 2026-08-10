"""remains_loot 无效 id 静默拒绝（遗骸不可得路径的契约锁）。

遗骸实体只在 `natural_end`（寿元自然耗尽）时生成（inventory/mod.rs
apply_termination_drop_on_terminate：`cause == Some("natural_end")` 才
spawn_player_remains_entity），而寿元自然耗尽 ≈120h（lifespan.rs LIFESPAN 常量），
fixture 运行不可达——happy path 无法黑盒构造，本场景锁拒绝面：

1. 空字符串 remains_id → client_request_handler 空 id warn gate 静默丢弃。
2. 随机 UUID 字符串 remains_id → handle_remains_loot_intents 查无实体 →
   debug log + continue（无 S2C、无聊天、无连接变化）。

断言：两路均窗口内无任何新 server_data（含 remains_sync）、无聊天、连接保持。
fresh bot 无 cultivation/meridian 等系统，周期环境 payload 仅 `carrier_state`
（每 1s 无条件推给所有 client，carrier_state_emit.rs）——断言以它为唯一白名单，
其余任何 server_data 一律判失败；否则「拒收却发 event_alert / 库存更新」的坏
实现会漏网（review finding 4）。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "remains_loot 空/未知 remains_id 静默拒绝：无 remains_sync、无聊天、连接保持"
MODULES = ["inventory", "network"]

LOOT_REQUEST = {"type": "remains_loot", "v": 1}
SILENT_WINDOW = 4.0
# 与请求无关的周期环境 payload：carrier_state 每 1s 无条件推给所有 client。
# cultivation_detail 需 MeridianSystem+Cultivation，本场景未开经脉，不应出现——
# 若出现即判红（这正是契约要求的「无任何新 server_data」）。
AMBIENT_PERIODIC_PAYLOAD_TYPES = frozenset({"carrier_state"})


def run(env) -> None:
    with env.new_bot("RmH") as bot:
        wait_join_and_inventory(bot)

        for label, remains_id in (
            ("空 remains_id", ""),
            ("随机 UUID remains_id", "8f9c9d31-aaaa-4b3e-9c11-000000000000"),
        ):
            sent_at = bot.events[-1].t if bot.events else 0.0
            bot.intent({**LOOT_REQUEST, "remains_id": remains_id})
            _assert_quiet_rejection(
                bot,
                sent_at,
                f"{label} 应被静默拒绝（无 remains_sync、无聊天、连接保持）",
            )
        bot.assert_alive("remains_loot 拒绝路径全程")


def _assert_quiet_rejection(bot, sent_at: float, description: str) -> None:
    # 截止时刻用单调钟（time.monotonic），不用事件时间戳 bot.events[-1].t：
    # 静默断言正是"之后无事件到达"，事件时间不会推进，以事件时间做 deadline 会
    # 永远等不到 now >= end_at 而死循环（review finding 1/5）。
    deadline = time.monotonic() + SILENT_WINDOW
    while True:
        for e in bot.events_of("server_data"):
            # 模块契约是「无任何新 server_data」：白名单外的 payload 一律判红。
            # 只盯 remains_sync 会放走拒收却发 event_alert / 库存更新的坏实现。
            if e.t > sent_at and e.data["payload_type"] not in AMBIENT_PERIODIC_PAYLOAD_TYPES:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，"
                    f"实际窗口内收到 server_data/{e.data['payload_type']}（t={e.t:.3f}）"
                )
        for e in bot.events_of("chat"):
            if e.t > sent_at:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
                )
        if time.monotonic() >= deadline:
            return
        bot.assert_alive(f"{description} 窗口内连接保持")
        time.sleep(0.1)
