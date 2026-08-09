"""`bong:client_request` 超长 payload —— valence 协议级 32767 字节上限干净拒绝。

valence `CustomPayloadC2s.data` 是 `Bounded<RawBytes, MAX_PAYLOAD_SIZE=32767>`
（`packets/play/custom_payload_c2s.rs`）：任何超过 32767 字节的 plugin channel
payload 在**解码层整包丢弃**（warn log），根本不会进 `handle_client_request_payloads`
（`client_request_handler.rs` 入口）。32767 字节恰好放行、32768 即拒，边界在
协议层，不在 serde。

本场景锁的是：超长包被**干净**拒绝 —— server 不崩、不踢、连接状态完好；
探针窗口内无任何玩法副作用（server_data / chat / vfx 均未出现，且探针后背包
快照指纹不变）—— 超长包在解码层被丢弃，未产生任何玩法副作用；之后一个合法
请求仍被正常处理（拒绝没有毒化连接）。
"""

import json

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 超长 payload(>32767B) 协议层干净丢弃：不崩不踢且连接可用"
MODULES = ["network"]

# valence CustomPayloadC2s MAX_PAYLOAD_SIZE（Bounded<RawBytes, 32767>）。
MAX_PAYLOAD_SIZE = 32767

PROBES = [
    ("恰好超 1 字节（32768B）", MAX_PAYLOAD_SIZE + 1),
    ("远超上限（200_000B）", 200_000),
]


def run(env) -> None:
    from ._inventory_helpers import latest_inventory_snapshot, wait_join_and_inventory
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Big") as bot:
        snapshot = wait_join_and_inventory(bot)
        pre_fingerprint = inventory_fingerprint(snapshot)

        def make_raw_send(size: int):
            def send() -> None:
                # 帧层 varint 长度前缀能承载任意大小；故意给足字节让解码层撞 Bounded 上限。
                bot.send_payload("bong:client_request", b"A" * size)

            return send

        probes = [(label, make_raw_send(size)) for label, size in PROBES]
        probes.append(
            (
                "超长但结构合法的 JSON（meridian 巨型字符串，同样 >32767B）",
                lambda: bot.send_payload(
                    "bong:client_request",
                    json.dumps(
                        {
                            "v": 1,
                            "type": "set_meridian_target",
                            "meridian": "x" * 50_000,
                        }
                    ).encode("utf-8"),
                ),
            )
        )
        fire_probes_and_keep_connection(bot, "超长 payload", probes)

        post = latest_inventory_snapshot(bot)
        post_fingerprint = inventory_fingerprint(post)
        if post_fingerprint != pre_fingerprint:
            raise BotAssertionError(
                "超长 payload 探针后背包快照指纹变化：某个超长包被部分处理了，"
                f"探针前={pre_fingerprint} 探针后={post_fingerprint}"
            )

        assert_valid_request_still_works(bot)
