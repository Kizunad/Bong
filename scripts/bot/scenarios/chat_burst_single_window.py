"""单一天道窗口内的聊天突发：密集交替广播必须全量送达且两玩家顺序一致。

天道「近期民意」以最近 5 分钟为窗口聚合玩家聊天信号（chat-processor.ts
`CHAT_CONTEXT_WINDOW_SECONDS`）。协议层可观察的性质是：一次落在同一窗口内的
密集突发必须全部送达 zone 内所有玩家（含发送者 echo），顺序稳定且与发送一致。

突发编排刻意保持在限流边界内（3 条/玩家/tick，server chat_collector.rs
`MAX_CHAT_MESSAGES_PER_PLAYER_PER_TICK`）：2 玩家交替共 6 条，每条间隔 >1 tick，
锁的是「分布在不同 tick 的突发不允许丢/错序/崩服」这个正向契约；同 tick 超预算
丢弃是另一条已由 unit 锁住的行为，不做协议层 flaky 断言。
"""

import time

from bot.bot import BotAssertionError

DESCRIPTION = "同一窗口内的聊天突发：多玩家交替全量广播、两玩家顺序一致、不丢不崩"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 10.0
SEND_GAP_SECONDS = 0.4
BURST_ROUNDS = 3


def run(env) -> None:
    tag = env.run_tag
    with env.new_bot("BR1") as alice, env.new_bot("BR2") as bob:
        for bot in (alice, bob):
            bot.expect_event("game_join", timeout=15.0)
            bot.expect_event("pos_look", timeout=15.0)
        time.sleep(1.0)

        send_plan: list[tuple] = []
        for round_index in range(1, BURST_ROUNDS + 1):
            send_plan.append((alice, f"br-{tag}-a{round_index}"))
            send_plan.append((bob, f"br-{tag}-b{round_index}"))
        expected_lines = [f"<{sender.username}> {raw}" for sender, raw in send_plan]

        alice_t0 = time.monotonic() - alice.t0
        bob_t0 = time.monotonic() - bob.t0

        for sender, raw in send_plan:
            sender.chat(raw)
            time.sleep(SEND_GAP_SECONDS)

        def require_full_burst(bot, t0, label: str) -> None:
            def subsequence() -> list[str]:
                return [
                    e.data["text"]
                    for e in bot.events_of("chat")
                    if e.t >= t0 and e.data["text"] in expected_lines
                ]

            try:
                bot.wait_for(
                    lambda _: subsequence() == expected_lines,
                    timeout=ECHO_TIMEOUT,
                    description=f"{label} 收到突发全部 {len(expected_lines)} 条且顺序与发送一致",
                )
            except BotAssertionError:
                seen = subsequence()
                raise BotAssertionError(
                    f"{label} 期望突发 {len(expected_lines)} 条聊天全部送达且顺序一致"
                    f"（期望 {expected_lines!r}，实际 {seen!r}）——突发不得丢/错序"
                )
            bot.assert_alive(f"{label} 突发广播后")

        require_full_burst(alice, alice_t0, "发送者A")
        require_full_burst(bob, bob_t0, "发送者B")
