"""玩家聊天 zone 广播回归 —— 锁 fix-playtest-batch 的 chat echo 修复。

历史 bug（2026-07-06 bot playtest）：普通聊天只进 Redis `bong:player_chat`
喂天道，发送者和同 zone 玩家全都看不见——发言如喊进虚空。修复后
`collect_player_chat` 的 PlayerChat 分支向同 zone client（含发送者）广播
`<username> message`，广播格局与 SpiritTreasureDialogue 一致。

断言（CodeRabbit #1068 补全边界）：
1. 发送者自己收到回显（echo）
2. 同 zone 的另一个玩家也收到广播（修复的主目的）
3. 跨 zone 的玩家收不到（zone 边界生效）——raster-less fallback 世界只有
   一个 zone，无法把 bot 挪去别的 zone，此 leg 显式打印跳过（不静默）。
"""

import time

from bot.bot import BotAssertionError

DESCRIPTION = "普通聊天必须广播给同 zone 玩家（含发送者 echo），跨 zone 隔离"
MODULES = ["network", "social"]

ECHO_TIMEOUT = 6.0
# 跨 zone 隔离的观察窗：比 ECHO_TIMEOUT 短——正例已证明投递路径 <1s，
# 3s 内没到就可判定没发（负断言窗口过长只会拖慢全套场景）。
ISOLATION_WINDOW = 3.0


def run(env) -> None:
    with env.new_bot("CE1") as sender, env.new_bot("CE2") as listener:
        for bot in (sender, listener):
            bot.expect_event("game_join", timeout=15.0)
            bot.expect_event("pos_look", timeout=15.0)
        time.sleep(1.0)

        # ── 1+2：发送者 echo + 同 zone 广播（两 bot 均出生于 spawn zone）──
        marker = f"echo-{env.run_tag}"
        sender.chat(marker)
        own = sender.expect_chat(marker, timeout=ECHO_TIMEOUT)
        if sender.username not in own.data["text"]:
            raise BotAssertionError(
                f"期望发送者回显带自己名字（<{sender.username}> {marker} 广播格式），"
                f"实际 {own.data['text']!r}"
            )
        heard = listener.expect_chat(marker, timeout=ECHO_TIMEOUT)
        if sender.username not in heard.data["text"]:
            raise BotAssertionError(
                f"期望同 zone 玩家收到含 <{sender.username}> 的广播，"
                f"实际 {heard.data['text']!r}"
            )

        # ── 3：跨 zone 隔离——把 listener 挪去另一个 zone 再发一条 ──
        listener.cmd("tpzone qingyun_peaks")
        time.sleep(3.0)
        moved = listener.events_of("chat") and any(
            "Teleported" in e.data["text"] for e in listener.events_of("chat")
        )
        if not moved:
            # raster-less fallback 世界只有 spawn 一个 zone，tp 不出去。
            print(
                "    [warn] 无法把 listener 传送出 spawn zone"
                "（raster-less 世界单 zone）——跨 zone 隔离 leg 跳过"
            )
            sender.assert_alive("chat echo 全程")
            return

        marker2 = f"iso-{env.run_tag}"
        t0 = time.monotonic() - listener.t0
        sender.chat(marker2)
        # 正例确认投递路径通畅后再做负断言。
        sender.expect_chat(marker2, timeout=ECHO_TIMEOUT)
        time.sleep(ISOLATION_WINDOW)
        leaked = [
            e.data["text"]
            for e in listener.events_of("chat")
            if e.t >= t0 and marker2 in e.data["text"]
        ]
        if leaked:
            raise BotAssertionError(
                f"期望跨 zone 玩家收不到 spawn zone 聊天（zone 边界隔离），"
                f"实际泄漏 {leaked!r}——广播回归成了全服"
            )
        sender.assert_alive("chat echo 全程")
        listener.assert_alive("跨 zone 隔离全程")
