"""损坏玩家 slice 的只读降级验收。

该场景只在显式 persistence-guard harness 中运行：先让 server 正常落盘一行库存，
再由测试夹具把 JSON 损坏，随后重连并触发一次库存变化。若载入守护失效，默认库存
或后续变化会覆盖损坏行；正确实现会让玩家继续可连接，但保留损坏行等待人工恢复。
"""

from __future__ import annotations

import os
import sqlite3
import time

from bot.bot import BotAssertionError
from bot.scenarios._inventory_helpers import find_item, wait_inventory_contains, wait_join_and_inventory

DESCRIPTION = "损坏 inventory slice 进入只读降级，默认值与后续变化不覆盖原始行"
MODULES = ["network", "persistence", "inventory"]
DEFAULT_ENABLED = False
RUN_IN_ALL_WHEN_ENV = "BONG_RUN_PERSISTENCE_GUARDS"

TARGET_ITEM = "stone_chunk"
GIVE_COUNT = 2
CORRUPTED_JSON = "{broken-json"
DISCONNECT_PERSIST_GRACE = 1.5


def _database_path() -> str:
    path = os.environ.get("BONG_SERVER_DB")
    if not path:
        raise BotAssertionError("load_failure_guard 需要 BONG_SERVER_DB 夹具路径")
    return path


def _corrupt_inventory_row(username: str) -> None:
    with sqlite3.connect(_database_path(), timeout=10.0) as connection:
        connection.execute(
            "UPDATE inventories SET inventory_json = ?1 WHERE username = ?2",
            (CORRUPTED_JSON, username),
        )
        if connection.total_changes != 1:
            raise BotAssertionError(f"未找到 {username} 的 inventories 行，无法注入损坏 slice")
        connection.commit()


def _read_inventory_row(username: str) -> str:
    with sqlite3.connect(_database_path(), timeout=10.0) as connection:
        row = connection.execute(
            "SELECT inventory_json FROM inventories WHERE username = ?1", (username,)
        ).fetchone()
    if row is None:
        raise BotAssertionError(f"{username} 的 inventories 行在守护测试后消失")
    return row[0]


def run(env) -> None:
    username = f"B{env.run_tag}Guard"
    with env.new_bot("Guard") as bot:
        wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd(f"give {TARGET_ITEM} {GIVE_COUNT}")
        bot.expect_chat(f"[dev] gave {TARGET_ITEM} x{GIVE_COUNT}", timeout=10.0)
        snapshot = wait_inventory_contains(bot, TARGET_ITEM)
        assert find_item(snapshot, TARGET_ITEM) is not None
        bot.assert_alive("健康 inventory slice 落盘前连接保持")

    time.sleep(DISCONNECT_PERSIST_GRACE)
    _corrupt_inventory_row(username)

    with env.new_bot("Guard") as bot:
        snapshot = wait_join_and_inventory(bot)
        assert find_item(snapshot, TARGET_ITEM) is None, (
            "损坏 inventory slice 只能以默认运行时库存降级，不能把无法验证的旧物品当作已加载"
        )
        bot.cmd(f"give {TARGET_ITEM} 1")
        bot.expect_chat("[dev] gave", timeout=10.0)
        bot.assert_alive("损坏 slice 的只读降级会话保持连接")

    time.sleep(DISCONNECT_PERSIST_GRACE)
    persisted = _read_inventory_row(username)
    assert persisted == CORRUPTED_JSON, (
        "inventory load failure 必须从 WriteSet omit；默认库存和后续变化不得覆盖损坏行，"
        f"实际 inventories_json={persisted!r}"
    )
