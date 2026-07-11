"""修炼 dev 命令反馈 —— `/realm set` 与 `/qi set` 的黑盒 chat 契约。

覆盖面：
- `server/src/cmd/dev/realm.rs`：成功应回 `[dev] realm set <prev> -> <next>`
- `server/src/cmd/dev/realm.rs`：非法 id 应回含原输入与允许值的拒绝 chat
- `server/src/cmd/dev/qi.rs`：成功应回 `[dev] qi set <before> -> <after>`
- `server/src/cmd/dev/qi.rs`：非法 qi 值应回 `[dev] qi set rejected: ...`
"""

DESCRIPTION = "/realm set 成功与非法 id、/qi set 成功与非法值都有玩家可见 chat 反馈"
MODULES = ["cmd", "cultivation"]


def run(env) -> None:
    with env.new_bot("Cult") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.expect_chat("Induce", timeout=10.0)

        bot.cmd("realm set bot_e2e_no_such_realm")
        bot.expect_chat(
            "[dev] realm set rejected: unknown realm \"bot_e2e_no_such_realm\"; "
            "allowed: awaken|induce|condense|solidify|spirit|void",
            timeout=10.0,
        )

        bot.cmd("qi set 7.5")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        bot.cmd("qi set -1")
        bot.expect_chat(
            "[dev] qi set rejected: value must be finite >= 0",
            timeout=10.0,
        )

        bot.assert_alive("realm 成功/非法 id 与 qi 成功/非法值反馈后")
