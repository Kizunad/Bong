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

**超长探针是「结构合法的请求 + 越限尺寸」**（review finding：坏探针只用不可解析的
字节串/非法 meridian 值，会假通过）。32768/200000 字节的探针都填成合法
`set_meridian_target`（meridian="lung"，JSON 空白填充到越限尺寸）—— 若协议层
`Bounded<RawBytes, 32767>` 上限被移除/放宽，这些请求会进入 JSON/serde 层并被
**接受**，handler 回推「已收到经脉目标」聊天 → 探针窗口标记副作用；若协议层拒绝
仍在，则和坏字节串一样在解码层整包丢弃、零副作用。由此协议层拒绝与下游 serde 拒绝
可被区分。

反向锁定**正向边界**：恰好 `MAX_PAYLOAD_SIZE`（32767）字节的结构合法请求必须被
**接受** —— 用 JSON 空白（词法允许 token 间任意空白，serde_json 忽略）把合法
`set_meridian_target`（meridian="lung"）填充到恰好 32767 字节，照常解出并被
handler 处理，回推「已收到经脉目标」聊天确认。若解码器把上限误降到 32767 或更低，
该请求会被整包丢弃、确认不出现 —— 覆盖 review finding 1 指出的缺测边界。
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


def _valid_payload_padded_to(size: int) -> bytes:
    """恰好 ``size`` 字节的结构合法 ClientRequestV1（set_meridian_target）。

    JSON 词法允许 token 间任意空白（空格/TAB/LF/CR），serde_json 忽略之 —— 在合法
    请求最后一个值后填充空格到恰好 ``size`` 字节，语义不变：meridian="lung" 被
    handler 接受并回推「已收到经脉目标」聊天确认。32768 字节即被 valence 解码层
    整包丢弃，故 32767 是「解码通过 + 被处理」能同时成立的最大尺寸 —— 正向锁定
    边界（review finding 1）。
    """
    base = json.dumps({"v": 1, "type": "set_meridian_target", "meridian": "lung"})
    assert len(base) < size
    return (base[:-1] + " " * (size - len(base)) + base[-1:]).encode("utf-8")


def run(env) -> None:
    from ._inventory_helpers import latest_inventory_snapshot, wait_join_and_inventory
    from ._rejection_helpers import (
        _relative_now,
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
        inventory_fingerprint,
    )

    with env.new_bot("Big") as bot:
        pre = wait_join_and_inventory(bot)
        pre_fingerprint = inventory_fingerprint(pre)

        def make_oversized_send(size: int):
            def send() -> None:
                # 帧层 varint 长度前缀能承载任意大小；payload 是**结构合法**的
                # set_meridian_target（JSON 空白填充到 size 字节，meridian="lung"）。
                # 上限若被移除/放宽，请求进入 serde/handler → 回推「已收到经脉目标」
                # → 探针窗口标记副作用；协议层拒绝仍在则整包丢弃、零副作用。
                # 用语义合法请求作超长探针，才能把协议层拒绝与下游 serde 拒绝区分开。
                bot.send_payload("bong:client_request", _valid_payload_padded_to(size))

            return send

        probes = [(label, make_oversized_send(size)) for label, size in PROBES]
        fire_probes_and_keep_connection(
            bot, "超长 payload", probes, baseline_snapshot=pre
        )

        post = latest_inventory_snapshot(bot)
        post_fingerprint = inventory_fingerprint(post)
        if post_fingerprint != pre_fingerprint:
            raise BotAssertionError(
                "超长 payload 探针后背包快照指纹变化：某个超长包被部分处理了，"
                f"探针前={pre_fingerprint} 探针后={post_fingerprint}"
            )

        # ---- 正向边界：恰好 MAX_PAYLOAD_SIZE(32767) 字节的结构合法请求必须被接受。
        # JSON 空白填充到恰好 32767 字节（serde_json 忽略空白），meridian="lung"
        # 照常解出 → handler 回推「已收到经脉目标」聊天确认（t > sent_at 保证确系
        # 本次请求的响应）。若解码器把上限误降到 32767 或更低，该请求被整包丢弃、
        # 确认不出现 —— 补上 review finding 1 指出的缺测正向边界。
        # 锚点必须取**发送时刻**（与 event.t 同一相对时钟），不能取 bot.events[-1].t：
        # 事件流安静时最后一条事件的 t 会停留在旧值，窗口起点早于发送时刻，探针阶段
        # 被错误接受产生的「已收到经脉目标」广播若恰在此刻到达，会冒充本请求的确认
        # （review finding：正向边界接受契约未被严格对应到本次请求）。
        boundary_sent_at = _relative_now(bot)
        bot.send_payload(
            "bong:client_request", _valid_payload_padded_to(MAX_PAYLOAD_SIZE)
        )
        bot.wait_for(
            lambda e: e.kind == "chat"
            and "已收到经脉目标" in e.data["text"]
            and e.t > boundary_sent_at,
            timeout=10.0,
            description=f"恰好 {MAX_PAYLOAD_SIZE}B 的合法请求被接受（收到经脉目标聊天确认）",
        )
        bot.assert_alive("恰好 32767B 边界合法请求被接受后")

        assert_valid_request_still_works(bot)
