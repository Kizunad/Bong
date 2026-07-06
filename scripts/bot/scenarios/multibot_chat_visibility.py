"""P5 多 bot 并发：两个协议 Bot 同服互见聊天广播。

不联跑 agent，不断言天道 narration；chat → narration 需要 agent/Redis 编排，
应由后续 coverage/bug plan 单独承接。本场景只锁住同 server 多连接下的基础
可见性和广播回流。
"""

DESCRIPTION = "两个 Bot 同 server：A 发 chat，B 必须收到同一条广播文本"
MODULES = ["network", "multibot", "chat"]


def run(env) -> None:
    with env.new_bot("MCA") as alice, env.new_bot("MCB") as bob:
        alice.expect_event("game_join", timeout=15.0)
        bob.expect_event("game_join", timeout=15.0)
        alice.expect_event("pos_look", timeout=15.0)
        bob.expect_event("pos_look", timeout=15.0)

        marker = f"bot-e2e-chat-{env.run_tag}"
        alice.chat(marker)
        bob.expect_chat(marker, timeout=10.0)

        alice.assert_alive("多 bot chat 可见性检查后")
        bob.assert_alive("多 bot chat 可见性检查后")
