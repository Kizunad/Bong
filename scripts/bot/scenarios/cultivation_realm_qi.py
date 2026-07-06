"""修炼 dev 命令反馈 —— `/realm set` 与 `/qi set` 的黑盒 chat 契约。

覆盖面：
- `server/src/cmd/dev/realm.rs`：成功应回 `[dev] realm set <prev> -> <next>`
- `server/src/cmd/dev/qi.rs`：成功应回 `[dev] qi set <before> -> <after>`
- 非法值必须给玩家可见反馈；否则 bot 只能看到命令静默丢失，排查从
  `RealmArg::parse_arg` / `QiCmd::Set` 的 send_chat_message 分支开始。
"""

DESCRIPTION = "/realm set 与 /qi set 成功/非法值都有玩家可见 chat 反馈"
MODULES = ["cmd", "cultivation"]


def _expect_realm_invalid_feedback(bot) -> None:
    sent_at = bot.events[-1].t if bot.events else 0.0
    bot.cmd("realm set bot_e2e_no_such_realm")
    bot.wait_for(
        lambda e: e.t > sent_at
        and e.kind == "chat"
        and (
            "bot_e2e_no_such_realm" in e.data["text"]
            or "awaken|induce|condense|solidify|spirit|void" in e.data["text"]
            or "expected" in e.data["text"]
            or "Expected" in e.data["text"]
            or "Invalid" in e.data["text"]
            or "invalid" in e.data["text"]
            or "无效" in e.data["text"]
        ),
        timeout=10.0,
        description=(
            "realm 非法 id 的聊天反馈；若超时，检查 "
            "server/src/cmd/dev/realm.rs RealmArg::parse_arg 的错误是否被命令框架回传给玩家"
        ),
    )


def run(env) -> None:
    with env.new_bot("Cult") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.expect_chat("Induce", timeout=10.0)

        _expect_realm_invalid_feedback(bot)

        bot.cmd("qi set 7.5")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        bot.cmd("qi set -1")
        bot.expect_chat(
            "[dev] qi set rejected: value must be finite >= 0",
            timeout=10.0,
        )

        bot.assert_alive("realm/qi dev 命令成功与非法值反馈后")
