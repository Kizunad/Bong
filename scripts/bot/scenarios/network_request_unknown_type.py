"""`bong:client_request` 未知/畸形 type 判别 —— schema 层干净拒绝。

`ClientRequestV1` 是 `#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]`
枚举：type 判别不存在 / 大小写不符 / 缺 type / 缺必需字段 / 多未知字段，都会被
`serde_json::from_str` 拒绝（warn log，`client_request_handler.rs` 入口），请求
不进 handler、连接保持。

本场景锁的是：每种未知/畸形 type 都被**干净**拒绝 —— 不崩、不踢、连接状态
完好，之后合法请求仍被正常处理。
"""

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 未知 type / 缺字段 / 多字段被 schema 干净拒绝且连接可用"
MODULES = ["network"]

UNKNOWN_TYPE_PROBES = [
    ("未知判别值", {"type": "no_such_request_variant", "v": 1}),
    ("判别值大小写不符", {"type": "BREAKTHROUGH_REQUEST", "v": 1}),
    ("空判别值", {"type": "", "v": 1}),
    ("缺 type 判别", {"v": 1}),
    ("已知类型缺必需字段 v", {"type": "breakthrough_request"}),
    ("已知类型多未知字段", {"type": "breakthrough_request", "v": 1, "extra_field": 1}),
]


def run(env) -> None:
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
    )

    with env.new_bot("Typ") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        probes = [
            (label, lambda req=req: bot.intent(req))
            for label, req in UNKNOWN_TYPE_PROBES
        ]
        fire_probes_and_keep_connection(bot, "未知 type", probes)
        assert_valid_request_still_works(bot)
