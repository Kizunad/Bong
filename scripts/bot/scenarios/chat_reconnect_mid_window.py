"""天道窗口中途断线重连：同一玩家的聊天跨重连持续可用，两段同窗。

场景：speaker 加入 → 发第一条（listener 见证）→ 断线 → 同用户名重连 →
再发第二条（listener 见证）。两条消息都落在同一天道 5 分钟窗口内，server 必须：
1) 两条都向同 zone 玩家广播（含发送者 echo，`collect_player_chat` 的 zone 广播格局）；
2) 重连不残留吞掉后续聊天的脏状态（历史教训：会话中途掉线后玩家发言如喊进虚空）；
3) listener 不得收到重复广播（断线重连不重放旧消息），且两段顺序先一后二。
"""

import time

from bot.bot import BotAssertionError

DESCRIPTION = "断线重连跨同一天道窗口：两条聊天均广播、无重复、无脏状态"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 10.0
# 断线与重连之间的间隔：给 server 完成旧连接清理，避免与残留会话竞态。
REJOIN_GRACE_SECONDS = 2.0


def run(env) -> None:
    tag = env.run_tag
    first_raw = f"rc-{tag}-first"
    second_raw = f"rc-{tag}-second"

    with env.new_bot("RW2") as listener:
        listener.expect_event("game_join", timeout=15.0)
        listener.expect_event("pos_look", timeout=15.0)

        # ── 第一段：加入即发第一条 ──
        with env.new_bot("RW1") as speaker:
            speaker.expect_event("game_join", timeout=15.0)
            speaker.expect_event("pos_look", timeout=15.0)
            time.sleep(1.0)
            listener_t0_first = time.monotonic() - listener.t0
            speaker.chat(first_raw)
            speaker.expect_chat(first_raw, timeout=ECHO_TIMEOUT)
            listener.expect_chat(first_raw, timeout=ECHO_TIMEOUT)
            speaker.assert_alive("第一段聊天广播后")

        # speaker 连接已随 with 退出关闭。
        time.sleep(REJOIN_GRACE_SECONDS)

        # ── 第二段：同用户名重连后再发第二条 ──
        with env.new_bot("RW1") as speaker:
            speaker.expect_event("game_join", timeout=15.0)
            speaker.expect_event("pos_look", timeout=15.0)
            time.sleep(1.0)
            listener_t0_second = time.monotonic() - listener.t0
            speaker.chat(second_raw)
            speaker.expect_chat(second_raw, timeout=ECHO_TIMEOUT)
            listener.expect_chat(second_raw, timeout=ECHO_TIMEOUT)
            speaker.assert_alive("第二段聊天广播后")

        # ── 无重复 + 顺序断言：listener 恰各见一次，且第二条晚于第一条 ──
        first_events = [
            e
            for e in listener.events_of("chat")
            if first_raw in e.data["text"] and e.t >= listener_t0_first
        ]
        second_events = [
            e
            for e in listener.events_of("chat")
            if second_raw in e.data["text"] and e.t >= listener_t0_second
        ]
        if len(first_events) != 1:
            raise BotAssertionError(
                f"期望 listener 恰好收到第一条一次（断线重连不得重放旧广播），"
                f"实际 {len(first_events)} 次"
            )
        if len(second_events) != 1:
            raise BotAssertionError(
                f"期望 listener 恰好收到第二条一次，实际 {len(second_events)} 次"
            )
        if not (first_events[0].t < second_events[0].t):
            raise BotAssertionError(
                "期望第二条广播晚于第一条（两段同窗、先一后二），"
                f"实际 first_t={first_events[0].t:.3f} second_t={second_events[0].t:.3f}"
            )
        listener.assert_alive("重连见证全程")
