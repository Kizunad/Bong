"""`bong:client_request` 通道喂坏数据 —— 最大化宽容红线（AGENTS.md §15.1）。

server/src/network/client_request_handler.rs 对非 UTF-8 / 坏 JSON / 未知 type /
未知版本的契约 = warn log + 忽略，连接必须保持。任何"坏 payload 踢人/panic/
断流"的回归在这里撞红。

顺带覆盖：向完全未注册的 bong:* 通道发数据也必须无害。
"""

DESCRIPTION = "向 bong:client_request 发非UTF8/坏JSON/未知type/未知版本不踢不panic"
MODULES = ["network"]

GARBAGE_PAYLOADS = [
    ("bong:client_request", b"\xff\xfe\x80\x81 not utf8 at all \x00"),
    ("bong:client_request", b"{oops, this is not json"),
    ("bong:client_request", b'{"v": 1, "type": "bot_e2e_no_such_type"}'),
    ("bong:client_request", b'{"v": 999999, "type": "breakthrough"}'),
    ("bong:client_request", b""),
    ("bong:bot_e2e_unregistered_channel", b"\x01\x02\x03"),
]


def run(env) -> None:
    with env.new_bot("Req") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        for channel, payload in GARBAGE_PAYLOADS:
            bot.send_payload(channel, payload)

        # 坏数据发完后 server 必须还在和我们心跳——用「新一轮 keepalive 到达」证明
        # 连接没被踢也没被单方面遗忘，而不是只看 socket 没断。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.wait_for(
            lambda e: e.kind == "keepalive" and e.t > sent_at,
            timeout=30.0,
            description="坏 payload 之后仍有 KeepAlive 往返（server 容忍坏输入不踢连接）",
        )
        bot.assert_alive("喂完 6 组坏 payload 后")
