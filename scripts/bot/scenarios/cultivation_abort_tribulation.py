"""`abort_tribulation` 中止渡劫请求 —— 网络层明示忽略，劫照常进行。

审计项（DONE-W6-HEADLESSAUDIT §5 P0-4「突破/心魔/顿悟 UI 决策」）：
`server/src/schema/client_request.rs::ClientRequestV1::AbortTribulation`（无场景覆盖）。

黑盒断言面：
- C2S：`bot.intent({"type":"abort_tribulation","v":1})`
- dispatch：`client_request_handler.rs` —— 确认后不可取消，warn
  「abort_tribulation ignored ... DuXu cannot be cancelled after confirmation」，
  **不 emit 任何取消事件**
- 无效化证据：劫中发 abort 后，渡虚劫仍在进行 —— 再次 `start_du_xu` 仍被
  「已在劫中」门禁拒绝（warn log），不产生新的 tribulation_state 宣告
"""

from bot.bot import BotAssertionError  # noqa: F401

from ._cultivation_gap_helpers import (
    assert_valid_request_still_works,
    du_xu_setup,
    wait_keepalive_after,
)

DESCRIPTION = "abort_tribulation：空闲/劫中均被网络层明示忽略，渡虚劫不被取消、连接保持"
MODULES = ["cultivation", "network"]

ABORT_TRIBULATION = {"type": "abort_tribulation", "v": 1}
START_DU_XU = {"type": "start_du_xu", "v": 1}


def run(env) -> None:
    with env.new_bot("Abr") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        # 1. 无劫空闲态 abort → 干净忽略：不踢、继续心跳、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(ABORT_TRIBULATION)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("空闲态 abort_tribulation 后连接保持")
        assert_valid_request_still_works(bot)

        # 2. 起劫，等劫进入 Omen 阶段。
        du_xu_setup(bot)
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(START_DU_XU)
        bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "tribulation_state"
            and e.t > sent_at,
            timeout=15.0,
            description="start_du_xu 后玩家收到 tribulation_state payload",
        )
        bot.assert_alive("渡虚劫启动后连接保持")

        # 3. 劫中 abort → 仍被网络层忽略（warn log 证据）。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(ABORT_TRIBULATION)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("劫中 abort_tribulation 后连接保持")

        # 4. 无效化证据：渡虚劫未被取消 —— 再次 start_du_xu 仍被「已在劫中」拒绝，
        #    且不产生新的 tribulation_state 宣告。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(START_DU_XU)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("abort 后再次 start_du_xu 被拒、连接保持")
        if any(
            e.kind == "server_data"
            and e.data["payload_type"] == "tribulation_state"
            and e.t > sent_at
            for e in bot.events
        ):
            raise BotAssertionError(
                f"[{bot.username}] 期望 abort 后渡虚劫未被取消（start_du_xu 不再宣告新劫），"
                "实际收到了新的 tribulation_state payload"
            )
        assert_valid_request_still_works(bot)
