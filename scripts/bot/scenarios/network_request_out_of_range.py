"""`bong:client_request` 越界字段值 —— schema / version 门禁干净拒绝。

三档拒绝都在 handler 产生任何玩法副作用**之前**拦截：
- **serde 守卫**（`client_request.rs` 内 `deserialize_slot_index` /
  `deserialize_block_picker_count`）：hotbar/quick slot >8 或负数、block_picker
  count=0 或 >64、v 超出 u8、非法 enum 变体 —— 反序列化期拒绝；
- **version 门禁**（`client_request_handler.rs` SUPPORTED_VERSION=1）：v≠1 ——
  handler 入口拒绝。

本场景锁的是：每个越界值都被**干净**拒绝 —— 不崩、不踢、连接状态完好，
之后合法请求仍被正常处理。
"""

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 越界字段值(slot/count/v/变体) 被 schema+版本门禁干净拒绝"
MODULES = ["network"]

OUT_OF_RANGE_PROBES = [
    ("hotbar slot=9 越界", {"type": "use_quick_slot", "v": 1, "slot": 9}),
    ("hotbar slot=-1 负数", {"type": "use_quick_slot", "v": 1, "slot": -1}),
    (
        "block_picker_give count=0",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 0},
    ),
    (
        "block_picker_give count=65",
        {"type": "block_picker_give", "v": 1, "block_id": "stone_bricks", "count": 65},
    ),
    ("版本 v=0 低于支持版本", {"type": "breakthrough_request", "v": 0}),
    ("版本 v=255 高于支持版本", {"type": "breakthrough_request", "v": 255}),
    ("v 超出 u8 范围", {"type": "breakthrough_request", "v": 999999999}),
    (
        "botany_harvest mode 非法变体",
        {"type": "botany_harvest_request", "v": 1, "session_id": "x", "mode": "garbage"},
    ),
]


def run(env) -> None:
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
    )

    with env.new_bot("Rng") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        probes = [
            (label, lambda req=req: bot.intent(req))
            for label, req in OUT_OF_RANGE_PROBES
        ]
        fire_probes_and_keep_connection(bot, "越界字段值", probes)
        assert_valid_request_still_works(bot)
