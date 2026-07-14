"""使用丹药：huiyuan_pill 双入口（template / instance）吃丹 → qi_current 回升 + 扣存。

黑盒契约面：
- 两条 C2S 都要锁（client_request.rs）：
  ① `alchemy_take_pill{pill_item_id}`（按 template_id）
  ② `apply_pill{instance_id, target:{kind:"self"}}`（按 inventory instance）
  两者汇入 handle_alchemy_take_pill，效果一致。
- huiyuan_pill effect=qi_recovery magnitude=60（assets/items/pills.toml）：
  吃后 Cultivation.qi_current 上升且 **clamp 到 qi_max**（qi=95 吃 60 → 100
  专属边界用例），经快照可观察；inventory 中丹 -1。
- 负分支：吃不存在的丹（背包无此 template）不得踢线/panic（宽容红线）。
"""

import time

from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
)

DESCRIPTION = "回元丹双入口吃丹：qi_current 回升可观察 + 丹扣存 + 空丹宽容"
MODULES = ["alchemy", "cultivation", "inventory"]

PILL_ID = "huiyuan_pill"
NON_CLAMP_EXPECTED_QI = 65.0
NON_CLAMP_QI_TOLERANCE = 0.05
QI_SET_TOLERANCE = 0.01
SETTLE_WINDOW_SECONDS = 0.5


def _player_state_qi(event) -> float | None:
    if event.kind != "server_data" or event.data.get("payload_type") != "player_state":
        return None
    qi = event.data["payload"].get("spirit_qi")
    return float(qi) if isinstance(qi, (int, float)) else None


def _extract_qi(node):
    if isinstance(node, dict):
        for key, value in node.items():
            if key == "qi_current" and isinstance(value, (int, float)):
                return float(value)
            got = _extract_qi(value)
            if got is not None:
                return got
    elif isinstance(node, list):
        for value in node:
            got = _extract_qi(value)
            if got is not None:
                return got
    return None


def _wait_authoritative_qi(bot, anchor: float, expected: float, timeout: float = 12.0):
    return bot.wait_for(
        lambda event: event.t > anchor
        and (lambda qi: qi is not None and abs(qi - expected) <= QI_SET_TOLERANCE)(
            _player_state_qi(event)
        ),
        timeout=timeout,
        description=(
            f"t>{anchor:.3f}s 后权威 player_state.spirit_qi="
            f"{expected}±{QI_SET_TOLERANCE}"
        ),
    )


def _is_qi_set_confirmation(event, anchor: float, value: float) -> bool:
    if event.kind != "chat" or event.t <= anchor:
        return False
    text = event.data.get("text", "")
    return text.startswith("[dev] qi set ") and text.endswith(f" -> {value:.1f}")


def _has_departed_baseline(qi: float, baseline_qi: float, expected_qi: float) -> bool:
    midpoint = baseline_qi + (expected_qi - baseline_qi) / 2.0
    if expected_qi >= baseline_qi:
        return qi >= midpoint - QI_SET_TOLERANCE
    return qi <= midpoint + QI_SET_TOLERANCE


def _assert_settled_consumption(
    events,
    before_revision: int,
    before_count: int,
    baseline_qi: float,
    expected_qi: float,
) -> dict:
    snapshots = [
        event.data["payload"]
        for event in events
        if event.kind == "server_data"
        and event.data.get("payload_type") == "inventory_snapshot"
    ]
    assert snapshots, "服丹稳定窗口内必须至少收到一条 inventory_snapshot"
    final_snapshot = snapshots[-1]
    final_revision = int(final_snapshot["revision"])
    assert final_revision == before_revision + 1, (
        f"一次服丹只能推进一版 inventory revision：期望 {before_revision + 1}，"
        f"实际 {final_revision}——同一 intent 可能被重复处理"
    )

    final_pill = find_item(final_snapshot, PILL_ID)
    final_count = 0 if final_pill is None else int(final_pill["item"]["stack_count"])
    assert final_count == before_count - 1, (
        f"一次服丹只能扣一枚：期望 {before_count - 1}，实际 {final_count}"
    )
    final_qi = _extract_qi(final_snapshot)
    assert final_qi is not None, "服丹最终 inventory_snapshot 必须携带 qi_current"
    assert abs(final_qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
        f"服丹最终快照应稳定在 {expected_qi}±{NON_CLAMP_QI_TOLERANCE}，实际 {final_qi}"
    )

    authoritative_qi = [
        qi
        for event in events
        if (qi := _player_state_qi(event)) is not None
    ]
    first_changed = next(
        (
            index
            for index, qi in enumerate(authoritative_qi)
            if _has_departed_baseline(qi, baseline_qi, expected_qi)
        ),
        None,
    )
    assert first_changed is not None, "服丹后必须收到显著离开旧基线的权威 player_state"
    settled_qi = authoritative_qi[first_changed:]
    assert all(
        abs(qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE for qi in settled_qi
    ), (
        f"服丹后权威 player_state 应持续稳定在 {expected_qi}±"
        f"{NON_CLAMP_QI_TOLERANCE}，实际序列 {settled_qi}——可能重复生效或回滚"
    )
    return final_snapshot


def _set_qi_and_wait(bot, value: float):
    anchor = last_event_time(bot)
    bot.cmd(f"qi set {value}")
    confirmation = bot.wait_for(
        lambda event: _is_qi_set_confirmation(event, anchor, value),
        timeout=10.0,
        description=f"t>{anchor:.3f}s 后收到本次 qi set 精确目标 -> {value:.1f} 的确认",
    )
    return _wait_authoritative_qi(bot, anchor, value)


def _consume_and_assert_once(bot, intent: dict, baseline_event, expected_qi: float) -> dict:
    before = latest_inventory_snapshot(bot)
    before_pill = require_item(before, PILL_ID)
    before_count = int(before_pill["item"]["stack_count"])
    baseline_qi = _player_state_qi(baseline_event)
    assert baseline_qi is not None, "服丹前基线必须来自权威 player_state"
    anchor = max(last_event_time(bot), baseline_event.t)
    bot.intent(intent)

    def decremented_once(snapshot: dict) -> bool:
        found = find_item(snapshot, PILL_ID)
        if before_count == 1:
            return found is None
        return found is not None and int(found["item"]["stack_count"]) == before_count - 1

    consumed_event = bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > anchor
        and event.data["payload_type"] == "inventory_snapshot"
        and int(event.data["payload"]["revision"]) > int(before["revision"])
        and decremented_once(event.data["payload"]),
        timeout=10.0,
        description=f"本次服丹只扣一枚：{before_count} -> {before_count - 1}",
    )
    consumed = consumed_event.data["payload"]
    immediate_qi = _extract_qi(consumed)
    assert immediate_qi is not None, "服丹后的 inventory_snapshot 必须携带 qi_current"
    assert abs(immediate_qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
        f"服丹即时快照应为 {expected_qi}±{NON_CLAMP_QI_TOLERANCE}，实际 {immediate_qi}"
    )

    authoritative = bot.wait_for(
        lambda event: event.t > anchor
        and (lambda qi: qi is not None and _has_departed_baseline(
            qi, baseline_qi, expected_qi
        ))(_player_state_qi(event)),
        timeout=12.0,
        description=(
            f"服丹后显著离开旧基线 {baseline_qi} 的权威 player_state.spirit_qi"
        ),
    )
    authoritative_qi = _player_state_qi(authoritative)
    assert authoritative_qi is not None
    assert abs(authoritative_qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
        f"服丹后首个新权威值应为 {expected_qi}±{NON_CLAMP_QI_TOLERANCE}，"
        f"实际 {authoritative_qi}"
    )

    time.sleep(SETTLE_WINDOW_SECONDS)
    settle_end = last_event_time(bot)
    settled_events = [
        event
        for event in bot.events_of("server_data")
        if anchor < event.t <= settle_end
    ]
    return _assert_settled_consumption(
        settled_events,
        int(before["revision"]),
        before_count,
        baseline_qi,
        expected_qi,
    )


def run(env) -> None:
    with env.new_bot("Pill") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        bot.cmd("qi max 100")
        bot.cmd(f"give {PILL_ID} 3")
        initial_inventory = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and (found := find_item(e.data["payload"], PILL_ID)) is not None
            and int(found["item"]["stack_count"]) == 3,
            timeout=10.0,
            description="give 后 inventory_snapshot 中 huiyuan_pill stack_count=3",
        ).data["payload"]
        assert int(require_item(initial_inventory, PILL_ID)["item"]["stack_count"]) == 3

        # ── 入口①：alchemy_take_pill（template_id 路径）──────────
        first_baseline = _set_qi_and_wait(bot, 5.0)
        _consume_and_assert_once(
            bot,
            {"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID},
            first_baseline,
            NON_CLAMP_EXPECTED_QI,
        )

        # ── 入口②：apply_pill（instance_id 路径）─────────────────
        second_baseline = _set_qi_and_wait(bot, 5.0)
        snapshot = latest_inventory_snapshot(bot)
        pill = require_item(snapshot, PILL_ID)
        _consume_and_assert_once(
            bot,
            {
                "type": "apply_pill",
                "v": 1,
                "instance_id": int(pill["item"]["instance_id"]),
                "target": {"kind": "self"},
            },
            second_baseline,
            NON_CLAMP_EXPECTED_QI,
        )

        # ── 边界：qi 接近上限时吃丹 clamp 到 qi_max，不得溢出 ────
        clamp_baseline = _set_qi_and_wait(bot, 95.0)
        final_inventory = _consume_and_assert_once(
            bot,
            {"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID},
            clamp_baseline,
            100.0,
        )

        # 三丹吃完：inventory 不应再有 huiyuan_pill
        assert find_item(final_inventory, PILL_ID) is None, (
            "三枚回元丹逐次各扣一枚后 inventory 应为 0——不扣存 = 无限白嫖丹药"
        )

        # ── 负分支：背包无丹再吃（宽容不踢）──────────────────────
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        time.sleep(1.0)
        bot.assert_alive("空丹重复吃之后（宽容红线：不得踢线/panic）")
