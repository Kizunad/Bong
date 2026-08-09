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
- 有效抉择 → `apply_choice` 应用 + `quota.apply_accumulation` + 移除 offer，
  且只在此分支发 `bong:enlightenment_pose` VFX（`bong:vfx_event`）——**这是判定应用与否的
  可观察状态转换**，本次任务把「只看连接健康」升级为「必须拦截正反两头姿态信号」：
  - 有效 → 姿态**必须出现**；
  - 错配 / 越界 / null / 已移除后闲散 → 姿态**绝不出现**。
"""

from ._cultivation_gap_helpers import (
    assert_no_payload_after,
    assert_valid_request_still_works,
    breakthrough_setup,
    wait_keepalive_after,
    wait_payload_containing,
)

DESCRIPTION = "insight_decision：顿悟邀约下发、错配/越界(null 拒绝)/闲散决策——姿态 VFX 正反两头可观察"
MODULES = ["cultivation", "network"]

VFX_CHANNEL = "bong:vfx_event"
ENLIGHTENMENT_POSE = b"bong:enlightenment_pose"


def _vfx_pose(bot, after: float, description: str):
    """等 after 之后出现顿悟姿态 VFX —— 有效抉择应用的可观察状态转换。"""
    return wait_payload_containing(
        bot,
        VFX_CHANNEL,
        ENLIGHTENMENT_POSE,
        after=after,
        timeout=12.0,
        description=description,
    )


def _no_vfx_pose(bot, after: float, description: str) -> None:
    """断言 after 之后无顿悟姿态 VFX（offer 未应用）：负路径不得发权威信号。"""
    assert_no_payload_after(
        bot, VFX_CHANNEL, after=after, window=3.0, description=description
    )


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


def _decision(bot, trigger_id, choice_idx):
    bot.intent(
        {"type": "insight_decision", "v": 1, "trigger_id": trigger_id, "choice_idx": choice_idx}
    )


def run(env) -> None:
    # ── Bot A：错配拒绝(offer 保留) → 有效抉择（应用 → 姿态可观察）→ 应用后闲散不复发 ──
    with env.new_bot("Ins") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        sent_at = breakthrough_setup(bot)
        _await_offer(bot, sent_at)

        # 1. trigger 错配 → 干净拒绝（offer 保留）：不得发姿态，连接不踢。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_OTHER, 1)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("insight_decision trigger 错配被拒后连接保持")
        _no_vfx_pose(bot, sent_at, "错配拒绝不得发出顿悟姿态 VFX")

        # 2. 有效抉择（正确 trigger + 合法 idx）→ 应用：必须发出顿悟姿态 VFX。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_INDUCE, 1)
        _vfx_pose(bot, sent_at, "有效 insight_decision 应用触发 bong:enlightenment_pose VFX")
        bot.assert_alive("insight_decision 有效应用后连接保持")

        # 3. 应用后 offer 已移除 → 再造闲散（越界 idx）不再姿态。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_INDUCE, 7)
        _no_vfx_pose(bot, sent_at, "应用后闲散决策不得再发顿悟姿态")
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("offer 应用后闲散 insight_decision 静默丢弃、连接保持")
        assert_valid_request_still_works(bot)

    # ── Bot B：offer PENDING 时发越界 idx → 不发姿态 + 该 idx 移除 offer ──
    with env.new_bot("Ins2") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        sent_at = breakthrough_setup(bot)
        _await_offer(bot, sent_at)

        # 4. offer 挂起时下发越界 idx（trigger 正确）→ 不发顿悟姿态。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_INDUCE, 7)
        _no_vfx_pose(bot, sent_at, "越界 idx（offer 挂起）不得触发顿悟姿态 VFX")
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("越界 idx 拒绝后连接保持")

        # 5. 越界 idx 已把 offer 移除 → 随后「触发正确 + idx 合法」的决策也绝不发姿态。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_INDUCE, 1)
        _no_vfx_pose(bot, sent_at, "越界已移除 offer，随后合法触发也不得再发姿态")
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("offer 移除后闲散 insight_decision 静默丢弃、连接保持")
        assert_valid_request_still_works(bot)

    # ── Bot Rej：null 回执 = 拒绝/超时（移除 offer，不发姿态）→ 闲散决策亦静默 ──
    with env.new_bot("Rej") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        sent_at = breakthrough_setup(bot)
        _await_offer(bot, sent_at)

        # 6. choice_idx=null（拒绝/超时等价）→ 不触发顿悟姿态（拒绝不是应用）。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_INDUCE, None)
        _no_vfx_pose(bot, sent_at, "null 回执（拒绝）不得触发顿悟姿态 VFX")
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("insight_decision null 回执（拒绝）后连接保持")

        # 7. offer 已移除后闲散决策 → 静默丢弃（无姿态）；连接完好、合法请求仍可用。
        sent_at = bot.events[-1].t if bot.events else 0.0
        _decision(bot, TRIGGER_OTHER, 0)
        wait_keepalive_after(bot, sent_at)
        bot.assert_alive("offer 移除后闲散 insight_decision 静默丢弃、连接保持")
        assert_valid_request_still_works(bot)
