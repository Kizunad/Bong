"""P5 多 bot 并发：两个协议 Bot 同服基础连接。

不联跑 agent，不断言天道 narration；chat → narration 需要 agent/Redis 编排，
应由后续 coverage/bug plan 单独承接。当前协议 Bot 下 chat 广播和 entity_spawn
互见均不稳定，本场景只锁住同 server 多连接不会互相踢下线。
"""

DESCRIPTION = "两个 Bot 同 server：均完成 join/pos_look，连接保持"
MODULES = ["network", "multibot"]


def run(env) -> None:
    with env.new_bot("MCA") as alice:
        alice.expect_event("game_join", timeout=15.0)
        alice.expect_event("pos_look", timeout=15.0)

        with env.new_bot("MCB") as bob:
            bob.expect_event("game_join", timeout=15.0)
            bob.expect_event("pos_look", timeout=15.0)

            alice.assert_alive("多 bot 同服连接检查后")
            bob.assert_alive("多 bot 同服连接检查后")
