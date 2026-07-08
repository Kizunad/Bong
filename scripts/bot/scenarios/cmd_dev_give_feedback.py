"""dev 命令链路 —— `/give` 全链路（brigadier 解析 → 库存写入 → chat 反馈）。

黑盒断言面 = `server/src/cmd/dev/give.rs` 的 send_chat_message 反馈契约：
- 成功：`[dev] gave <template_id> x<count> revision=<n>`
- 未知模板：`[dev] give `<id>` failed: ...`

模板 `starter_talisman` 来自 server/assets/items/core.toml（改名会撞红本场景，
届时同步更新——这正是 pin 的意义）。
"""

DESCRIPTION = "/give 成功与未知模板两分支都有 chat 反馈（dev 命令链路 + 库存写入）"
MODULES = ["cmd", "inventory"]

KNOWN_TEMPLATE = "starter_talisman"
UNKNOWN_TEMPLATE = "bot_e2e_no_such_item"


def run(env) -> None:
    with env.new_bot("Give") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd(f"give {KNOWN_TEMPLATE} 1")
        bot.expect_chat(f"[dev] gave {KNOWN_TEMPLATE} x1", timeout=10.0)

        bot.cmd(f"give {UNKNOWN_TEMPLATE} 1")
        bot.expect_chat("failed", timeout=10.0)

        bot.assert_alive("give 双分支执行后")
