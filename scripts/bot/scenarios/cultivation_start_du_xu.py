"""`start_du_xu` 渡虚劫发起链路 —— 前置拒绝 + 发起成功 + 劫中重复请求拒绝。

审计项（DONE-W6-HEADLESSAUDIT §5 P0-4「突破/心魔/顿悟 UI 决策」）：
`server/src/schema/client_request.rs::ClientRequestV1::StartDuXu`（无场景覆盖）。

黑盒断言面：
- C2S：`bot.intent({"type":"start_du_xu","v":1})`
- dispatch：`client_request_handler.rs` → `StartDuXuRequest`
- 门禁：`cultivation/tribulation.rs::start_du_xu_request_system` —— 前置未满足
  （realm != Spirit / 经脉未全开）或已在劫中 → warn「start_du_xu rejected」+ 不改变状态
- 成功：`start_tribulation_system` → `TribulationAnnounce` → 玩家收到
  `bong:server_data` `tribulation_state` payload（kind=du_xu / phase=omen）+
  `bong:vfx_event` 预警雷 `bong:tribulation_lightning`
"""

from bot.bot import BotAssertionError  # noqa: F401

from ._cultivation_gap_helpers import (
    assert_no_server_data_after,
    assert_valid_request_still_works,
    du_xu_setup,
    wait_keepalive_after,
    wait_payload_containing,
)

DESCRIPTION = "start_du_xu：前置拒绝干净、渡虚劫启动有状态 payload+VFX、劫中重复请求被拒且连接保持"
MODULES = ["cultivation", "network"]

START_DU_XU = {"type": "start_du_xu", "v": 1}


def run(env) -> None:
    with env.new_bot("Dux") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        # 1. 前置未满足（醒灵新手、零经脉）→ 干净拒绝：不踢、继续心跳、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(START_DU_XU)
        bot.wait_for(
            lambda e: e.t > sent_at and e.kind == "keepalive",
            timeout=25.0,
            description="新手 start_du_xu 被拒后 server 仍继续心跳",
        )
        bot.assert_alive("前置拒绝后连接保持")
        assert_valid_request_still_works(bot)

        # 2. dev 铺垫：通灵 + 全经脉（du_xu_prereqs_met 前置）。
        du_xu_setup(bot)

        # 3. 发起渡虚劫 → 玩家收到 tribulation_state payload + 预警雷 VFX。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(START_DU_XU)
        state = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "tribulation_state"
            and e.t > sent_at,
            timeout=15.0,
            description="start_du_xu 后玩家收到 tribulation_state payload（kind=du_xu phase=omen）",
        )
        payload = state.data["payload"]
        if payload.get("kind") != "du_xu" or payload.get("phase") != "omen":
            raise BotAssertionError(
                f"[{bot.username}] 期望 tribulation_state 声明 kind=du_xu phase=omen，"
                f"实际 {payload}"
            )
        wait_payload_containing(
            bot,
            "bong:vfx_event",
            b"bong:tribulation_lightning",
            after=sent_at,
            timeout=15.0,
            description="渡虚劫开场预警雷 bong:tribulation_lightning（plan-particle-system-v1 §4.4）",
        )
        bot.assert_alive("渡虚劫启动后连接保持")

        # 4. 劫中重复 start → 仍被门禁拒绝：不得再广播第二个 tribulation_state
        #    （身份/状态不变——劫还是同一场），连接保持、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(START_DU_XU)
        assert_no_server_data_after(
            bot,
            "tribulation_state",
            after=sent_at,
            window=4.0,
            description="劫中重复 start_du_xu 不得再次广播 tribulation_state（同一场劫，身份不变）",
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("劫中重复 start_du_xu 拒绝后连接保持")
        assert_valid_request_still_works(bot)
