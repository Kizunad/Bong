"""remains_loot 无效 id 静默拒绝（遗骸不可得路径的契约锁）。

遗骸实体只在 `natural_end`（寿元自然耗尽）时生成（inventory/mod.rs
apply_termination_drop_on_terminate：`cause == Some("natural_end")` 才
spawn_player_remains_entity），而寿元自然耗尽 ≈120h（lifespan.rs LIFESPAN 常量），
fixture 运行不可达——happy path 无法黑盒构造，本场景锁拒绝面：

1. 空字符串 remains_id → client_request_handler 空 id warn gate 静默丢弃。
2. 随机 UUID 字符串 remains_id → handle_remains_loot_intents 查无实体 →
   debug log + continue（无 S2C、无聊天、无连接变化）。

断言：两路均窗口内无任何新 server_data（含 remains_sync）、无聊天、连接保持。
"""

import time

from bot.bot import BotAssertionError

from ._inventory_helpers import wait_join_and_inventory

DESCRIPTION = "remains_loot 空/未知 remains_id 静默拒绝：无 remains_sync、无聊天、连接保持"
MODULES = ["inventory", "network"]

LOOT_REQUEST = {"type": "remains_loot", "v": 1}
SILENT_WINDOW = 4.0


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
    end_at = sent_at + SILENT_WINDOW
    while True:
        now = bot.events[-1].t if bot.events else 0.0
        for e in bot.events_of("server_data"):
            if e.t > sent_at and e.data["payload_type"] == "remains_sync":
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际收到 remains_sync（t={e.t:.3f}）"
                )
        for e in bot.events_of("chat"):
            if e.t > sent_at:
                raise BotAssertionError(
                    f"[{bot.username}] {description}，实际出现聊天 {e.data['text']!r}"
                )
        if now >= end_at:
            return
        time.sleep(0.1)
