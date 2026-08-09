"""渡虚劫心魔抉择 `heart_demon_decision` 全链路（双 bot，专用场景）。

黑盒断言面（全部走真实 wire 观察，不读 server 内部状态）：
- C2S `heart_demon_decision{v, choice_idx}`（`server/src/schema/client_request.rs`），
  dispatch 见 `client_request_handler.rs` → `HeartDemonChoiceSubmitted`。
- offer：`bong:server_data` oneof=69 `HeartDemonOffer`（`tribulation_heart_demon_offer_emit.rs`），
  心魔相开启时下发 3 个 choice（Composure/守本心、Breakthrough/斩执念、Perception/无解）。
- 决策语义（`tribulation.rs::heart_demon_outcome_for_choice`）：
  Some(0)=守本心（回少量 qi）、Some(2)=无解（无奖惩）、其余/None=心魔（损 30% 当前真元 +
  下一道开天雷 ×1.2）。断言锁「无解 → 开天雷 90 → 存活登仙」与「斩执念 → 30% 真元惩罚 →
  第 5 波开天雷满资源检查（effective qi_max = qi_max − 20% 冻锁）必然不过 → 渡劫失败结算」。
  注：惩罚后真元 ≤ 0.7×qi_max 恒小于 effective 0.8×qi_max，故心魔路径的 wire 结果是
  failed 结算而非 108 致死——×1.2 开天雷在满资源检查之后，实际不可达。
- 进度门槛：`du_xu_full_progress_ticks`（灵台突破/全经脉之后 36000 ticks=30 分钟）决定
  waves_total=5（含心魔相所在第 4 波），announce payload 的 wave_total 即此门槛的 wire 证据。

前置（dev 铺垫，与 cultivation_breakthrough.py 同风格）：
- realm set awaken → meridian open_all → qi max/set 500 → zone_qi set spawn 1.00，
  随后 4 次 breakthrough_request 逐级突破（醒灵→引气→凝液→固元→灵台，XorshiftRoll 确定性），
  成功以 bong:breakthrough_pillar vfx 观察、失败重试、severity≥0.7 致死则场景失败。
- 灵台达成后原地等待 30 分钟（keepalive 自动应答保持连接），再 start_du_xu：
  预兆 60s → 锁 30s → 第 1-4 波（每波 15s，伤害 18×波数、耗 35×波数）→ 第 4 波毕入心魔相
  （offer 30s 抉择窗）→ 决策后第 5 波开天雷（满资源检查 + 90/108 伤害 + 175 耗）→ 结算。
- 波间回血：每波 cleared 事件后 `health set 100` + `qi set 500`（下波 15s 后才落伤害）。
"""

from __future__ import annotations

import time

from bot.bot import Bot
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready

DESCRIPTION = (
    "双 bot 渡虚劫心魔相：NoSolution 无解登仙 + Breakthrough 斩执念触发满资源检查失败结算，"
    "断言 heart_demon_offer 形状与 wave_total=5（30 分钟满进度门槛）"
)
MODULES = ["cultivation", "tribulation", "network", "cmd", "multibot"]

DEFAULT_ENABLED = False  # 专用场景：30 分钟进度门槛，常规 --all 不执行（需显式 --scenario）

BREAKTHROUGH_REQUEST = {"type": "breakthrough_request", "v": 1}
START_DU_XU = {"type": "start_du_xu", "v": 1}
HEART_DEMON_DECISION = {"type": "heart_demon_decision", "v": 1}

REALM_STEPS = 4  # 醒灵→引气→凝液→固元→灵台
GATE_WAIT_SECONDS = 30 * 60 + 60  # 36000 ticks @20tps = 1800s，+60s 余量
CHAIN_ATTEMPT_CAP = 12


def _trib_state(bot: Bot, predicate, timeout: float, description: str):
    """等待匹配的 tribulation_state 解码 payload。"""
    return bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data.get("payload_type") == "tribulation_state"
            and e.data.get("payload") is not None
            and predicate(e.data["payload"])
        ),
        timeout=timeout,
        description=description,
    )


def _expect_phase(bot: Bot, phase: str, *, timeout: float = 90.0) -> dict:
    ev = _trib_state(
        bot,
        lambda p: p.get("phase") == phase,
        timeout,
        f"tribulation_state phase={phase}",
    )
    return ev.data["payload"]


def _expect_wave_cleared(bot: Bot, wave: int, *, timeout: float = 90.0) -> dict:
    ev = _trib_state(
        bot,
        lambda p: p.get("phase") == "wave" and p.get("wave_current") == wave,
        timeout,
        f"tribulation_state wave_current={wave}",
    )
    return ev.data["payload"]


def _breakthrough_vfx(bot: Bot, after: float) -> str:
    """等待一次突破 vfx，返回 "pillar"（成功）或 "fail"（失败，可重试）。"""
    ev = bot.wait_for(
        lambda e: (
            e.t > after
            and e.kind == "vfx_event"
            and e.data.get("event_id") in ("bong:breakthrough_pillar", "bong:breakthrough_fail")
        ),
        timeout=20.0,
        description="breakthrough_request 后的突破 vfx（bong:breakthrough_pillar / _fail）",
    )
    return "pillar" if ev.data.get("event_id") == "bong:breakthrough_pillar" else "fail"


def _breakthrough_to_spirit(bot: Bot) -> None:
    """dev 铺垫 + 4 次真实突破到灵台。失败按确定性 roll 重试；死亡则失败快出。"""
    bot.cmd("realm set awaken")
    bot.expect_chat("[dev] realm set", timeout=10.0)
    bot.expect_chat("Awaken", timeout=10.0)

    bot.cmd("meridian open_all")
    bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)

    bot.cmd("qi max 500")
    bot.expect_chat("[dev] qi max", timeout=10.0)
    bot.cmd("qi set 500")
    bot.expect_chat("[dev] qi set", timeout=10.0)

    bot.cmd("zone_qi set spawn 1.00")
    bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)

    succeeded = 0
    attempts = 0
    while succeeded < REALM_STEPS and attempts < CHAIN_ATTEMPT_CAP:
        attempts += 1
        bot.cmd("qi set 500")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        sent_at = last_event_time(bot)
        bot.intent(BREAKTHROUGH_REQUEST)
        result = _breakthrough_vfx(bot, sent_at)
        if result == "pillar":
            succeeded += 1
            continue
        # 突破失败（RolledFailure）：severity>=0.7 会致死；0.7 以下仅冻结 qi_max，可重试。
        try:
            bot.wait_for(
                lambda e: e.kind == "health" and e.data.get("health", 1.0) <= 0.0,
                timeout=3.0,
                description="突破失败后不应死亡（severity>=0.7 的 RolledFailure 会致死）",
            )
        except AssertionError:
            pass  # 存活（未收到 health=0）
        else:
            raise AssertionError(
                f"[{bot.username}] 突破失败 roll 致死（severity>=0.7）：确定性 roll 下需调整场景"
            )
    if succeeded < REALM_STEPS:
        raise AssertionError(
            f"[{bot.username}] {CHAIN_ATTEMPT_CAP} 次内未完成 {REALM_STEPS} 级突破（成功 {succeeded} 级）"
        )


def _heal(bot: Bot) -> None:
    """回满气血/真元（波间 15s 窗口与开天雷前满资源检查用）。"""
    bot.cmd("health set 100")
    bot.expect_chat("Queued /health set", timeout=10.0)
    bot.cmd("qi set 500")
    bot.expect_chat("[dev] qi set", timeout=10.0)


def _idle_until_gate(bot: Bot, gate_at: float) -> None:
    """原地挂机等待渡虚劫满进度门槛（keepalive 自动应答保持连接）。"""
    while time.monotonic() < gate_at:
        bot.assert_alive("满进度等待期间连接应保持（keepalive 自动应答）")
        time.sleep(30.0)
    bot.assert_alive("满进度等待结束后")


def _assert_offer_shape(offer: dict) -> None:
    assert offer.get("offer_id", "").startswith("heart_demon:"), (
        f"heart_demon_offer.offer_id 应为 heart_demon: 前缀，实际 {offer.get('offer_id')!r}"
    )
    assert offer.get("trigger_label") == "心魔劫临身", (
        f"trigger_label 应为 心魔劫临身，实际 {offer.get('trigger_label')!r}"
    )
    assert offer.get("realm_label") == "渡虚劫 · 心魔", (
        f"realm_label 应为 渡虚劫 · 心魔，实际 {offer.get('realm_label')!r}"
    )
    assert offer.get("expires_at_ms", 0) > time.time() * 1000 - 1000, (
        "expires_at_ms 应为未来时刻（30s 抉择窗）"
    )
    choices = offer.get("choices", [])
    assert len(choices) == 3, f"offer 应含 3 个 choice，实际 {len(choices)}"
    assert [(c.get("choice_id"), c.get("category")) for c in choices] == [
        ("heart_demon_choice_0", "Composure"),
        ("heart_demon_choice_1", "Breakthrough"),
        ("heart_demon_choice_2", "Perception"),
    ], f"choice_id/category 三元组不符，实际 {[(c.get('choice_id'), c.get('category')) for c in choices]}"
    assert [c.get("title") for c in choices] == ["守本心", "斩执念", "无解"], (
        f"choice 标题不符，实际 {[c.get('title') for c in choices]}"
    )


def _run_duxu_common(bot: Bot, choice_idx: int | None) -> None:
    """登仙/身死共同段：预兆→锁→1-4 波→心魔相 offer→抉择。返回前已发 decision。"""
    _heal(bot)
    bot.intent(START_DU_XU)

    announce = _expect_phase(bot, "omen", timeout=120.0)
    assert announce.get("kind") == "du_xu", f"announce kind 应为 du_xu，实际 {announce.get('kind')!r}"
    assert announce.get("wave_total") == 5, (
        "wave_total 应为 5（灵台突破+全经脉后 36000 ticks 满进度门槛生效），"
        f"实际 {announce.get('wave_total')!r}"
    )

    _expect_phase(bot, "lock", timeout=120.0)

    for wave in (1, 2, 3, 4):
        _expect_wave_cleared(bot, wave, timeout=90.0)
        if wave <= 3:
            _heal(bot)

    offer_ev = bot.expect_server_data("heart_demon_offer", timeout=90.0)
    _assert_offer_shape(offer_ev.data["payload"])

    _expect_phase(bot, "heart_demon", timeout=60.0)

    _heal(bot)
    bot.intent({**HEART_DEMON_DECISION, "choice_idx": choice_idx})
    _expect_wave_cleared(bot, 5, timeout=60.0)


def _expect_settle_result(bot: Bot, allowed: tuple[str, ...], *, timeout: float = 90.0) -> str:
    ev = _trib_state(
        bot,
        lambda p: p.get("result") is not None,
        timeout,
        "tribulation_state 结算（result 非空）",
    )
    result = ev.data["payload"].get("result")
    assert result in allowed, f"结算 result 应为 {allowed} 之一，实际 {result!r}"
    return result


def run(env) -> None:
    with env.new_bot("Ascend") as ascender:
        wait_for_ready(ascender)
        _breakthrough_to_spirit(ascender)
        ascender_gate_at = time.monotonic() + GATE_WAIT_SECONDS

        with env.new_bot("HdJue") as obsessed:
            wait_for_ready(obsessed)
            # 第二 bot 的突破链在 ascender 的 30 分钟等待窗口内完成，等待时段重叠。
            _breakthrough_to_spirit(obsessed)
            obsessed_gate_at = time.monotonic() + GATE_WAIT_SECONDS

            _idle_until_gate(ascender, ascender_gate_at)

            # bot A：无解（Perception）→ 开天雷 90 → 存活登仙。
            _run_duxu_common(ascender, choice_idx=2)
            _expect_settle_result(ascender, ("ascended",))
            ascender.assert_alive("无解登仙结算后连接应保持")

            # bot B：斩执念（Breakthrough）→ 心魔 → 30% 真元惩罚 →
            # 第 5 波开天雷满资源检查必失败 → failed 结算（不死）。
            _idle_until_gate(obsessed, obsessed_gate_at)
            _run_duxu_common(obsessed, choice_idx=1)
            _expect_settle_result(obsessed, ("failed",))
            obsessed.assert_alive("斩执念失败结算后玩家应存活（满资源检查失败不致死）")
