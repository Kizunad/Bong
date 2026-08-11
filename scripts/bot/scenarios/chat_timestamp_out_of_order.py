"""聊天客户端时间戳乱序/极端：server 必须按到达序向同 zone 玩家全量广播。

Tiandao 5 分钟「近期民意」窗口的输入新鲜度由 server 自己的观察钟决定——
`ChatMessageV1.ts` 取自 `ChatObservationClock`（server 观察秒），客户端 C2S
时间戳只保留给内部消费者（unit 层已锁 ts 推导，e2e-chat-signal-window.sh 锁真实
链路上 server 观察秒）。本场景锁协议层可观察行为：客户端填未来 +1 天、epoch(0)、
正常当前时间三种时间戳，且三者乱序（非单调）发送时，server 必须全部接受、
全部按到达序向同 zone 玩家广播（发送者 echo **并** 在场景内的第二名同 zone
玩家处同样可见）、不踢不断连。

要求第二名玩家存在（`OO2` 接收者）才能观测「同 zone 广播」这个契约的一半——
若实现只把消息回给发送者、对其他玩家全部丢弃，单 bot 场景仍会全绿。故发送
者 `OO1` 与接收者 `OO2` 均需按到达序收到同一段三连。

历史背景：服务器曾信任客户端伪造时间戳（2026-07 修复，见 chat_collector.rs 的
`ChatObservationClock`）。回归点在「广播本身」——即使 ts 推导正确，若 server 按
客户端时间戳排序/丢弃，玩家发言仍会错序或消失。
"""

import time

from bot.bot import BotAssertionError

DESCRIPTION = "乱序/极端客户端时间戳的聊天必须按到达序全量广播（同 zone 兼收，不丢不踢）"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 10.0
# 每条发送间隔：> 1 个 server tick（20tps=50ms），保证各自落在独立 tick，
# 不触发 3 条/玩家/tick 限流，也让「到达序」断言只受网络与 server 广播序约束。
SEND_GAP_SECONDS = 0.4
FUTURE_OFFSET_MILLIS = 86_400_000  # +1 天


def _expected_line(username: str, raw: str) -> str:
    return f"<{username}> {raw}"


def _count_three(bot, expected: list[str], t0: float) -> list[str]:
    """t0 之后、文本落在期望三连内的广播，按到达序。"""
    return [
        e.data["text"]
        for e in bot.events_of("chat")
        if e.t >= t0 and e.data["text"] in expected
    ]


def _expect_three_broadcast(bot, expected: list[str], t0: float) -> None:
    """断言 bot 在 t0 之后按到达序收到整段三连（发送者 echo 或同 zone 接收者兼收）。"""
    try:
        bot.wait_for(
            lambda _: _count_three(bot, expected, t0) == expected,
            timeout=ECHO_TIMEOUT,
            description=f"{bot.username} 按到达序收齐乱序时间戳三连",
        )
    except BotAssertionError:
        seen = _count_three(bot, expected, t0)
        raise BotAssertionError(
            f"[{bot.username}] 期望乱序/极端客户端时间戳（+1天、epoch、当前）全部被接受"
            f"并按到达序广播给同 zone 玩家（期望 {expected!r}，实际 {seen!r}）——"
            "server 不得按客户端时间戳排序/丢弃，也不得只回显发送者、丢弃其他玩家"
        )


def run(env) -> None:
    tag = env.run_tag
    future_raw = f"tso-{tag}-future"
    epoch_raw = f"tso-{tag}-epoch"
    now_raw = f"tso-{tag}-now"

    # OO2 先加入并留在同 zone：它是被广播契约的另一半——必须能观察「非发送者」的收信。
    with env.new_bot("OO2") as receiver:
        receiver.expect_event("game_join", timeout=15.0)
        receiver.expect_event("pos_look", timeout=15.0)

        with env.new_bot("OO1") as sender:
            sender.expect_event("game_join", timeout=15.0)
            sender.expect_event("pos_look", timeout=15.0)
            time.sleep(1.0)

            expected = [
                _expected_line(sender.username, future_raw),
                _expected_line(sender.username, epoch_raw),
                _expected_line(sender.username, now_raw),
            ]
            # 各取「发送起点」事件时间（time.monotonic() - bot.t0 即 bot 当前事件时标）。
            receiver_t0 = time.monotonic() - receiver.t0
            sender_t0 = time.monotonic() - sender.t0

            now_millis = time.time_ns() // 1_000_000
            sender.chat(future_raw, timestamp_millis=now_millis + FUTURE_OFFSET_MILLIS)
            time.sleep(SEND_GAP_SECONDS)
            sender.chat(epoch_raw, timestamp_millis=0)
            time.sleep(SEND_GAP_SECONDS)
            sender.chat(now_raw, timestamp_millis=None)

            # 发送者 echo 与同 zone 接收者兼收都要按到达序齐收。
            _expect_three_broadcast(sender, expected, sender_t0)
            _expect_three_broadcast(receiver, expected, receiver_t0)

            sender.assert_alive("乱序时间戳聊天（含同 zone 接收）后")
            receiver.assert_alive("乱序时间戳聊天（含同 zone 接收）后")