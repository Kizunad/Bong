"""`insight_decision` 顿悟邀约回执 —— 邀约下发 + 错配拒绝 + 有效抉择应用 + 拒绝/超时。

审计项（DONE-W6-HEADLESSAUDIT §5 P0-4「突破/心魔/顿悟 UI 决策」）：
`server/src/schema/client_request.rs::ClientRequestV1::InsightDecision`（无场景覆盖）。

黑盒断言面：
- 触发：真实 `breakthrough_request` 首次突破 → `insight_trigger_on_breakthrough`
  → `process_insight_request` 挂 `PendingInsightOffer` + `cultivation_insight_offer_emit`
  向玩家推 `bong:server_data` `insight_offer`（trigger_id = first_breakthrough_to_Induce，3 个候选）
- C2S：`bot.intent({"type":"insight_decision","v":1,"trigger_id":...,"choice_idx":N|null})`
- dispatch：`client_request_handler.rs` → `InsightChosen`
- 校验：`cultivation/insight_flow.rs::apply_insight_chosen` ——
  trigger 与当前挂着的 offer 不符 → warn「insight decision mismatch ... ignoring」（offer 保留）；
  idx 越界 → warn「chose invalid idx」并移除 offer；null → info「rejected insight offer」并移除 offer；
  无 pending offer 的闲散决策 → 静默丢弃
- 有效抉择 → `apply_choice` 应用 + `quota.apply_accumulation` + 移除 offer（无拒绝 warn）
"""

from ._cultivation_gap_helpers import (
    assert_valid_request_still_works,
    breakthrough_setup,
    wait_keepalive_after,
)

DESCRIPTION = "insight_decision：顿悟邀约下发、错配/越界/拒绝/闲散决策全链路，连接保持"
MODULES = ["cultivation", "network"]

TRIGGER_INDUCE = "first_breakthrough_to_Induce"
TRIGGER_OTHER = "first_breakthrough_to_Condense"


def _await_offer(bot, sent_at: float, timeout: float = 15.0):
    """等首次突破后的 insight_offer payload，断言 trigger_id 正确，返回该事件。"""
    offer = bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.data["payload_type"] == "insight_offer"
        and e.t > sent_at,
        timeout=timeout,
        description="首次突破后玩家收到 insight_offer payload（含 trigger_id + 候选）",
    )
    payload = offer.data["payload"]
    if payload.get("trigger_id") != TRIGGER_INDUCE:
        raise AssertionError(
            f"[{bot.username}] 期望 insight_offer trigger_id={TRIGGER_INDUCE}，"
            f"实际 {payload.get('trigger_id')!r}"
        )
    if len(payload.get("choices", [])) < 1:
        raise AssertionError(
            f"[{bot.username}] 期望 insight_offer 带候选选项，实际 choices 为空：{payload}"
        )
    return offer


def run(env) -> None:
    # ── Bot Ins：错配拒绝（offer 保留）→ 有效抉择（应用）→ 应用后闲散决策静默 ──
    with env.new_bot("Ins") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        sent_at = breakthrough_setup(bot)
        _await_offer(bot, sent_at)

        # 1. trigger 错配 → 干净拒绝（warn log：decision mismatch；offer 保留）。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {"type": "insight_decision", "v": 1, "trigger_id": TRIGGER_OTHER, "choice_idx": 1}
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("insight_decision trigger 错配被拒后连接保持")

        # 2. 有效抉择（正确 trigger + 合法 idx）→ 应用 + offer 移除（无拒绝 warn）。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {"type": "insight_decision", "v": 1, "trigger_id": TRIGGER_INDUCE, "choice_idx": 1}
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("insight_decision 有效抉择应用后连接保持")

        # 3. 应用后同 trigger 再发（idx 越界）→ 无 pending offer，静默丢弃，
        #    不出现「chose invalid idx」warn（log 证据）；连接仍完好。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {"type": "insight_decision", "v": 1, "trigger_id": TRIGGER_INDUCE, "choice_idx": 7}
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("offer 应用后闲散 insight_decision 静默丢弃、连接保持")
        assert_valid_request_still_works(bot)

    # ── Bot Rej：null 回执 = 拒绝/超时（offer 移除）→ 闲散决策静默 ──
    with env.new_bot("Rej") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        sent_at = breakthrough_setup(bot)
        _await_offer(bot, sent_at)

        # 4. choice_idx=null（拒绝/超时等价）→ info「rejected insight offer」+ offer 移除。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {"type": "insight_decision", "v": 1, "trigger_id": TRIGGER_INDUCE, "choice_idx": None}
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("insight_decision null 回执（拒绝）后连接保持")

        # 5. offer 已移除后闲散决策 → 静默丢弃（无 warn）；连接完好、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        bot.intent(
            {"type": "insight_decision", "v": 1, "trigger_id": TRIGGER_OTHER, "choice_idx": 0}
        )
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("offer 移除后闲散 insight_decision 静默丢弃、连接保持")
        assert_valid_request_still_works(bot)
