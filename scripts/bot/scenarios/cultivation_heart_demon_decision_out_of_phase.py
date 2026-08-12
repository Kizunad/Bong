"""`heart_demon_decision` 心魔抉择回执 —— 空闲/渡虚 Omen 阶段一律干净忽略。

注：与并行分支 bot-scen/gap6 的 `cultivation_heart_demon_decision.py`（真实心魔相全链路）
互补：本场景只锁「非心魔相门禁」协议面（含 Omen 阶段 + 连接健康探针）。

审计项（DONE-W6-HEADLESSAUDIT §5 P0-4「突破/心魔/顿悟 UI 决策」）：
`server/src/schema/client_request.rs::ClientRequestV1::HeartDemonDecision`（无场景覆盖）。

黑盒断言面：
- C2S：`bot.intent({"type":"heart_demon_decision","v":1,"choice_idx":N})`（N 或 null）
- dispatch：`client_request_handler.rs` → `HeartDemonChoiceSubmitted`
- 门禁：`cultivation/tribulation.rs::heart_demon_choice_system` —— 仅
  `TribulationPhase::HeartDemon` 阶段消费；其余阶段（含空闲、Omen）静默丢弃，
  **不产生任何心魔化解副作用**，渡虚劫照常进行
- 说明：心魔阶段位于渡虚劫尾段（Omen 60s + Lock 30s + 数波之后，全程数分钟），
  无 dev 快进命令，e2e 只锁「非心魔阶段决策被干净忽略」这一协议面
"""

from bot.bot import BotAssertionError  # noqa: F401

from ._cultivation_gap_helpers import (
    assert_no_server_data_after,
    assert_valid_request_still_works,
    du_xu_setup,
    wait_keepalive_after,
)

DESCRIPTION = "heart_demon_decision：空闲/渡虚 Omen 阶段均被心魔门禁干净忽略（含连接健康探针）"
MODULES = ["cultivation", "network"]

HEART_DEMON_IDX = {"type": "heart_demon_decision", "v": 1, "choice_idx": 1}
HEART_DEMON_NULL = {"type": "heart_demon_decision", "v": 1, "choice_idx": None}
START_DU_XU = {"type": "start_du_xu", "v": 1}


def run(env) -> None:
    with env.new_bot("Hdm") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        # 1. 空闲态（无劫）抉择 → 心魔门禁静默丢弃：不踢、继续心跳、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(HEART_DEMON_IDX)
        bot.intent(HEART_DEMON_NULL)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("空闲态 heart_demon_decision 后连接保持")
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

        # 3. Omen 阶段抉择 → 仍被门禁忽略（非 HeartDemon phase）；渡虚劫照常进行。
        #    absence 断言必须覆盖有界 post-request 观察窗：若实现坏到真处理了抉择、
        #    渡虚劫状态因此变化，新的 tribulation_state 宣告要到后续 server/ECS tick
        #    才 emit，瞬时历史扫描看不到，keepalive 也证明不了 dispatch 已完成。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(HEART_DEMON_IDX)
        bot.intent(HEART_DEMON_NULL)
        assert_no_server_data_after(
            bot,
            "tribulation_state",
            after=sent_at,
            window=4.0,
            description="Omen 阶段 heart_demon_decision 不得产生新的 tribulation_state 宣告（渡虚劫照常）",
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("Omen 阶段 heart_demon_decision 后连接保持")
        assert_valid_request_still_works(bot)
