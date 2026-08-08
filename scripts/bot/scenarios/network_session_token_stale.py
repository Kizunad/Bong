"""`bong:client_request` 过期 / 重放会话 token —— session 权威门禁干净拒绝。

`external_container_move` / `external_container_close` 的 `session_id` 指向服务端
`ExternalContainerRegistry::sessions`（`client_request_handler.rs`
`handle_external_container_move`）。伪造 / 已失效 / 被重放的 token 查不到 session →
handler **干净拒绝**：`external_container_move` 回推请求者背包快照
（`bong:server_data` inventory_snapshot，reason `external_container_resync`，
零 mutation），`external_container_close` 对未知 session 干净 no-op；连接保持。

本场景锁的是：stale / replayed token 被**干净**拒绝 —— 有可观测的拒绝响应、
无 mutation 副作用、连接状态完好，之后合法请求仍被正常处理。
"""

import time

from bot.bot import BotAssertionError  # noqa: F401

DESCRIPTION = "bong:client_request 过期/重放 external-container session token 干净拒绝并 resync 背包"
MODULES = ["network"]

# 伪造 session token：u64 高位全 1，不可能是服务端分配的真实 session id。
FORGED_SESSION_ID = 0xFFFF_FFFF_FFFF_FF00
FORGED_INSTANCE_ID = 0xDEAD_BEEF_CAFE_0001


def _move_request(session_id: int) -> dict:
    return {
        "v": 1,
        "type": "external_container_move",
        "session_id": session_id,
        "instance_id": FORGED_INSTANCE_ID,
        "from": {
            "kind": "container",
            "container_id": f"ext_{session_id}",
            "row": 0,
            "col": 0,
        },
        "to": {"kind": "container", "container_id": "main_pack", "row": 0, "col": 0},
    }


def _close_request(session_id: int) -> dict:
    return {"v": 1, "type": "external_container_close", "session_id": session_id}


def run(env) -> None:
    from ._rejection_helpers import (
        assert_valid_request_still_works,
        fire_probes_and_keep_connection,
    )

    with env.new_bot("Tok") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)
        # 等 join 时的初始快照突发放完，避免把 join snapshot 误判成拒绝响应。
        time.sleep(1.0)

        def expect_resync_after(after: float):
            bot.expect_server_data_payload(
                "inventory_snapshot",
                timeout=10.0,
                after=after,
            )

        # stale move #1 —— 可观测的干净拒绝：server 回推背包快照（零 mutation）。
        before = bot.events[-1].t
        bot.intent(_move_request(FORGED_SESSION_ID))
        expect_resync_after(before)
        bot.assert_alive("stale move #1 拒绝响应后")

        # stale move #2（重放同一 token）—— 同样干净拒绝，连接不坏。
        before = bot.events[-1].t
        bot.intent(_move_request(FORGED_SESSION_ID))
        expect_resync_after(before)
        bot.assert_alive("stale move #2 重放拒绝后")

        # stale close —— 同一 session 权威门禁：未知 token 干净 no-op，不崩不踢。
        bot.intent(_close_request(FORGED_SESSION_ID))
        time.sleep(1.0)
        bot.assert_alive("stale close 后")

        fire_probes_and_keep_connection(
            bot,
            "stale session",
            [("重放 close", lambda: bot.intent(_close_request(FORGED_SESSION_ID)))],
        )
        assert_valid_request_still_works(bot)
