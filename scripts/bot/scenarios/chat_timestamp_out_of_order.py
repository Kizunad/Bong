"""聊天客户端时间戳乱序/极端：server 必须按到达序全量广播，不因时间戳丢消息。

Tiandao 5 分钟「近期民意」窗口的输入新鲜度由 server 自己的观察钟决定——
`ChatMessageV1.ts` 取自 `ChatObservationClock`（server 观察秒），客户端 C2S
时间戳只保留给内部消费者（unit 层已锁 ts 推导，e2e-chat-signal-window.sh 锁真实
链路上 server 观察秒）。本场景锁协议层可观察行为：客户端填未来 +1 天、epoch(0)、
正常当前时间三种时间戳，且三者乱序（非单调）发送时，server 必须全部接受、
全部按到达序向同 zone 广播（含发送者 echo）、不踢不断连。

历史背景：服务器曾信任客户端伪造时间戳（2026-07 修复，见 chat_collector.rs 的
`ChatObservationClock`）。回归点在「广播本身」——即使 ts 推导正确，若 server 按
客户端时间戳排序/丢弃，玩家发言仍会错序或消失。
"""

import time

from bot.bot import BotAssertionError

DESCRIPTION = "乱序/极端客户端时间戳的聊天必须按到达序全量广播（不丢不踢）"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 10.0
# 每条发送间隔：> 1 个 server tick（20tps=50ms），保证各自落在独立 tick，
# 不触发 3 条/玩家/tick 限流，也让「到达序」断言只受网络与 server 广播序约束。
SEND_GAP_SECONDS = 0.4
FUTURE_OFFSET_MILLIS = 86_400_000  # +1 天


def _expected_line(username: str, raw: str) -> str:
    return f"<{username}> {raw}"


def run(env) -> None:
    with env.new_bot("OO1") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        time.sleep(1.0)

        tag = env.run_tag
        future_raw = f"tso-{tag}-future"
        epoch_raw = f"tso-{tag}-epoch"
        now_raw = f"tso-{tag}-now"
        expected = [
            _expected_line(bot.username, future_raw),
            _expected_line(bot.username, epoch_raw),
            _expected_line(bot.username, now_raw),
        ]

        # 发送起点（事件时间轴），只统计这之后的聊天广播。
        t0 = time.monotonic() - bot.t0
        now_millis = time.time_ns() // 1_000_000
        bot.chat(future_raw, timestamp_millis=now_millis + FUTURE_OFFSET_MILLIS)
        time.sleep(SEND_GAP_SECONDS)
        bot.chat(epoch_raw, timestamp_millis=0)
        time.sleep(SEND_GAP_SECONDS)
        bot.chat(now_raw, timestamp_millis=None)

        def echo_subsequence() -> list[str]:
            return [
                e.data["text"]
                for e in bot.events_of("chat")
                if e.t >= t0 and e.data["text"] in expected
            ]

        try:
            bot.wait_for(
                lambda _: echo_subsequence() == expected,
                timeout=ECHO_TIMEOUT,
                description="乱序时间戳聊天按到达序全量回显",
            )
        except BotAssertionError:
            seen = echo_subsequence()
            raise BotAssertionError(
                "期望乱序/极端客户端时间戳（+1天、epoch、当前）全部被接受并按到达序"
                f"广播（期望 {expected!r}，实际 {seen!r}）——"
                "server 不得按客户端时间戳排序或丢弃"
            )
        bot.assert_alive("乱序时间戳聊天后")
