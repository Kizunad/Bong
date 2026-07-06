"""哑客户端挂机宽容 —— AGENTS.md §15.1「最大化宽容」的常驻回归面。

Bot 故意不发 ClientSettings、不注册任何 plugin channel、不接资源包，
只回 KeepAlive 和 TeleportConfirm。server 必须容忍这种最小客户端：
不踢、不 panic、持续心跳。任何"缺 ClientSettings 就踢/卡死"的回归在这里撞红。

「持续心跳」用两次独立 KeepAlive 锁死：只测第一次的话，server 发完一次就
遗忘连接（不再心跳但也不断开）的回归会漏过。
"""

DESCRIPTION = "不发 ClientSettings/不注册通道的哑客户端可持续挂机，server 心跳不断"
MODULES = ["network"]

NEXT_KEEPALIVE_TIMEOUT = 25.0


def run(env) -> None:
    with env.new_bot("Tol") as bot:
        bot.expect_event("game_join", timeout=15.0)
        # server 必须主动维持心跳（valence 周期发 KeepAlive；断流 = server 侧忘了这条连接）
        first_keepalive = bot.expect_event("keepalive", timeout=30.0)
        bot.wait_for(
            lambda e: e.kind == "keepalive" and e.t > first_keepalive.t,
            timeout=NEXT_KEEPALIVE_TIMEOUT,
            description=(
                "第二次 KeepAlive（server 对哑客户端持续心跳，"
                "而非发一次就把连接遗忘）"
            ),
        )
        bot.assert_alive("两次心跳之后")
