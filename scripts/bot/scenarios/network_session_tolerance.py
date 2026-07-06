"""哑客户端挂机宽容 —— AGENTS.md §15.1「最大化宽容」的常驻回归面。

Bot 故意不发 ClientSettings、不注册任何 plugin channel、不接资源包，
只回 KeepAlive 和 TeleportConfirm。server 必须容忍这种最小客户端：
不踢、不 panic、持续心跳。任何"缺 ClientSettings 就踢/卡死"的回归在这里撞红。
"""

import time

DESCRIPTION = "不发 ClientSettings/不注册通道的哑客户端可持续挂机，server 不踢不断流"
MODULES = ["network"]

IDLE_SECONDS = 8.0


def run(env) -> None:
    with env.new_bot("Tol") as bot:
        bot.expect_event("game_join", timeout=15.0)
        # server 必须主动维持心跳（valence 周期发 KeepAlive；断流 = server 侧忘了这条连接）
        first_keepalive = bot.expect_event("keepalive", timeout=30.0)
        idle_until = first_keepalive.t + IDLE_SECONDS
        while time.monotonic() - bot.t0 < idle_until:
            bot.assert_alive(f"哑客户端挂机（首个 keepalive 后再挂 {IDLE_SECONDS}s）")
            time.sleep(0.5)
        bot.assert_alive("哑客户端挂机结束时")
