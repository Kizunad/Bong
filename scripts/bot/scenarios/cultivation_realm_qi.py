"""修炼 dev 命令反馈 —— `/realm set` 与 `/qi set` 的黑盒 chat 契约。

覆盖面：
- `server/src/cmd/dev/realm.rs`：成功应回 `[dev] realm set <prev> -> <next>`
- `server/src/cmd/dev/qi.rs`：成功应回 `[dev] qi set <before> -> <after>`
- `server/src/cmd/dev/qi.rs`：非法 qi 值应回 `[dev] qi set rejected: ...`

注意：`/realm set <非法 id>` 当前在 `RealmArg::parse_arg` / Brigadier 解析层
被拒，live bot 看不到玩家 chat 反馈。该产品缺口已记录到
`docs/plans-skeleton/plan-bughunt-bot-realm-invalid-feedback-v1.md`，本场景不再
把它当成现有可观测契约。
"""

DESCRIPTION = "/realm set 成功反馈，/qi set 成功与非法值都有玩家可见 chat 反馈"
MODULES = ["cmd", "cultivation"]


def run(env) -> None:
    with env.new_bot("Cult") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.expect_chat("Induce", timeout=10.0)

        bot.cmd("qi set 7.5")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        bot.cmd("qi set -1")
        bot.expect_chat(
            "[dev] qi set rejected: value must be finite >= 0",
            timeout=10.0,
        )

        bot.assert_alive("realm 成功与 qi 成功/非法值反馈后")
