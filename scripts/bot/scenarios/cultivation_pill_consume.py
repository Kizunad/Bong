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

from bot.bot import BotAssertionError, Event
from bot.scenarios._combat_helpers import last_event_time, wait_for_ready
from bot.scenarios._cultivation_helpers import (
    QI_SET_TOLERANCE,
    _player_state_qi,
    _player_state_values,
    _set_qi_and_wait,
    _set_qi_max_and_wait,
)
from bot.scenarios._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    require_item,
)

DESCRIPTION = "回元丹双入口吃丹：qi_current 回升可观察 + 丹扣存 + 空丹宽容"
MODULES = ["alchemy", "cultivation", "inventory"]

PILL_ID = "huiyuan_pill"
PILL_QI_RECOVERY = 60.0
NON_CLAMP_EXPECTED_QI = 65.0
NON_CLAMP_QI_TOLERANCE = 0.05
SERVER_TICK_OBSERVATION_TICKS = 20
SERVER_TICK_FENCE_TIMEOUT_SECONDS = 15.0


def _extract_qi(node) -> float | None:
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


def _server_tick_from_event(event, anchor: float) -> int | None:
    if event.kind != "chat" or event.t <= anchor:
        return None
    prefix = "[dev] time now: "
    text = event.data.get("text", "")
    if not text.startswith(prefix):
        return None
    raw_tick = text[len(prefix) :]
    assert raw_tick.isascii() and raw_tick.isdigit(), (
        f"权威 time now 回执必须携带十进制 tick，实际 {text!r}"
    )
    return int(raw_tick)


def _query_server_tick(bot, timeout: float) -> int:
    anchor = last_event_time(bot)
    bot.cmd("time now")
    event = bot.wait_for(
        lambda candidate: _server_tick_from_event(candidate, anchor) is not None,
        timeout=timeout,
        description=f"t>{anchor:.3f}s 后收到权威 [dev] time now tick 回执",
    )
    tick = _server_tick_from_event(event, anchor)
    assert tick is not None
    return tick


def _snapshot_after_server_tick_fence(
    bot,
    minimum_ticks: int = SERVER_TICK_OBSERVATION_TICKS,
    timeout: float = SERVER_TICK_FENCE_TIMEOUT_SECONDS,
) -> list[Event]:
    if minimum_ticks <= 0:
        raise ValueError(f"minimum_ticks must be > 0, actual {minimum_ticks}")
    if timeout <= 0.0:
        raise ValueError(f"timeout must be > 0, actual {timeout}")

    deadline = time.monotonic() + timeout
    start_tick = _query_server_tick(bot, timeout)
    target_tick = start_tick + minimum_ticks
    current_tick = start_tick

    # 严格跨过 target_tick 后，再做两次顺序 round-trip。`handle_time` 与状态 emitter
    # 都在 Update，但没有互相排序：目标 tick 的组件若在 emitter 之后变化，payload
    # 会延到下一帧；该帧的第一条 marker 又可能排在 emitter 前。第二次 drain marker
    # 最早落在再下一帧，其 TCP 水位才必然覆盖前一帧 post-emit 的 payload。
    while current_tick <= target_tick:
        remaining = deadline - time.monotonic()
        if remaining <= 0.0:
            raise BotAssertionError(
                f"[{bot.username}] 期望 {timeout}s 内从权威 server tick {start_tick} "
                f"推进至少 {minimum_ticks} tick 并越过 fence {target_tick}，"
                f"实际停在 {current_tick}"
            )
        next_tick = _query_server_tick(bot, remaining)
        assert next_tick >= current_tick, (
            f"权威 server tick 不得回退：{current_tick} -> {next_tick}"
        )
        current_tick = next_tick

    crossed_tick = current_tick
    for drain_round in range(1, 3):
        remaining = deadline - time.monotonic()
        if remaining <= 0.0:
            raise BotAssertionError(
                f"[{bot.username}] 已跨过 fence {target_tick} 到 {crossed_tick}，"
                f"但未在 {timeout}s 内完成第 {drain_round}/2 次 post-emit drain"
            )
        next_tick = _query_server_tick(bot, remaining)
        assert next_tick >= current_tick, (
            f"权威 server tick 不得回退：{current_tick} -> {next_tick}"
        )
        current_tick = next_tick

    return bot.events_of("server_data")


def _has_departed_baseline(qi: float, baseline_qi: float, expected_qi: float) -> bool:
    if abs(expected_qi - baseline_qi) <= QI_SET_TOLERANCE:
        return False
    return abs(qi - baseline_qi) > QI_SET_TOLERANCE


def _expected_qi_after_pill(baseline_qi: float, authoritative_qi_max: float) -> float:
    return min(baseline_qi + PILL_QI_RECOVERY, authoritative_qi_max)


def _assert_settled_consumption(
    events,
    before_revision: int,
    before_count: int,
    baseline_qi: float,
    expected_qi: float,
    expected_qi_max: float,
) -> dict:
    snapshots = [
        event.data["payload"]
        for event in events
        if event.kind == "server_data"
        and event.data.get("payload_type") == "inventory_snapshot"
    ]
    assert snapshots, "服丹稳定窗口内必须至少收到一条 inventory_snapshot"
    expected_revision = before_revision + 1
    expected_count = before_count - 1
    consumed = False
    for index, snapshot in enumerate(snapshots):
        revision = int(snapshot["revision"])
        pill = find_item(snapshot, PILL_ID)
        count = 0 if pill is None else int(pill["item"]["stack_count"])
        qi = _extract_qi(snapshot)
        assert qi is not None, (
            f"服丹稳定窗口第 {index} 版 inventory_snapshot 必须携带 qi_current，"
            f"实际 snapshot={snapshot}"
        )

        if not consumed and revision == before_revision:
            assert count == before_count, (
                f"消费前 inventory revision={before_revision} 时丹药数量必须保持 "
                f"{before_count}，实际 {count}"
            )
            assert abs(qi - baseline_qi) <= NON_CLAMP_QI_TOLERANCE, (
                f"消费前 inventory qi 必须保持旧基线 {baseline_qi}±"
                f"{NON_CLAMP_QI_TOLERANCE}，实际 {qi}"
            )
            continue

        consumed = True
        assert revision == expected_revision, (
            f"消费后每版 inventory revision 必须保持 {expected_revision}，"
            f"实际第 {index} 版为 {revision}——不得重复推进或回滚"
        )
        assert count == expected_count, (
            f"消费后每版丹药数量必须保持 {expected_count}，"
            f"实际第 {index} 版为 {count}——一次服丹只能扣一枚"
        )
        assert abs(qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
            f"消费后每版 inventory qi 必须保持 {expected_qi}±"
            f"{NON_CLAMP_QI_TOLERANCE}，实际第 {index} 版为 {qi}"
        )

    assert consumed, (
        f"稳定窗口必须观察到 inventory revision {before_revision} -> "
        f"{expected_revision} 的消费转换，实际 revisions="
        f"{[int(snapshot['revision']) for snapshot in snapshots]}"
    )
    final_snapshot = snapshots[-1]

    authoritative_states = [
        state
        for event in events
        if (state := _player_state_values(event)) is not None
    ]
    departed = False
    for index, (qi, qi_max) in enumerate(authoritative_states):
        assert abs(qi_max - expected_qi_max) <= QI_SET_TOLERANCE, (
            f"服丹观察窗内权威 spirit_qi_max 应保持 {expected_qi_max}±"
            f"{QI_SET_TOLERANCE}，实际第 {index} 个为 {qi_max}"
        )
        if not departed and not _has_departed_baseline(
            qi, baseline_qi, expected_qi
        ):
            continue
        departed = True
        assert abs(qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
            f"服丹后权威 player_state 应持续稳定：首个及后续每个非基线值都必须为 "
            f"{expected_qi}±"
            f"{NON_CLAMP_QI_TOLERANCE}，实际第 {index} 个为 {qi}，"
            f"完整序列 {authoritative_states}——不得忽略错误中间态、重复生效或回滚"
        )
    assert departed, "服丹后必须收到离开旧基线的权威 player_state"
    return final_snapshot


def _consume_and_assert_once(bot, intent: dict, baseline_event) -> dict:
    before = latest_inventory_snapshot(bot)
    before_pill = require_item(before, PILL_ID)
    before_count = int(before_pill["item"]["stack_count"])
    baseline_state = _player_state_values(baseline_event)
    assert baseline_state is not None, "服丹前基线必须来自权威 player_state"
    baseline_qi, authoritative_qi_max = baseline_state
    expected_qi = _expected_qi_after_pill(baseline_qi, authoritative_qi_max)
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
        and int(event.data["payload"]["revision"]) == int(before["revision"]) + 1
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
    assert authoritative_qi is not None, "服丹后必须解析出权威 player_state.spirit_qi"
    assert abs(authoritative_qi - expected_qi) <= NON_CLAMP_QI_TOLERANCE, (
        f"服丹后首个新权威值应为 {expected_qi}±{NON_CLAMP_QI_TOLERANCE}，"
        f"实际 {authoritative_qi}"
    )

    event_snapshot = _snapshot_after_server_tick_fence(bot)
    settled_events = [
        event
        for event in event_snapshot
        if event.kind == "server_data" and event.t > anchor
    ]
    return _assert_settled_consumption(
        settled_events,
        int(before["revision"]),
        before_count,
        baseline_qi,
        expected_qi,
        authoritative_qi_max,
    )


def run(env) -> None:
    with env.new_bot("Pill") as bot:
        wait_for_ready(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        _set_qi_max_and_wait(bot, 100.0)
        bot.cmd(f"give {PILL_ID} 3")
        initial_inventory = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and (found := find_item(e.data["payload"], PILL_ID)) is not None
            and int(found["item"]["stack_count"]) == 3,
            timeout=10.0,
            description="give 后 inventory_snapshot 中 huiyuan_pill stack_count=3",
        ).data["payload"]
        initial_count = int(
            require_item(initial_inventory, PILL_ID)["item"]["stack_count"]
        )
        assert initial_count == 3, (
            f"give 后应持有 3 枚 {PILL_ID}，实际 {initial_count}"
        )

        # ── 入口①：alchemy_take_pill（template_id 路径）──────────
        first_baseline = _set_qi_and_wait(bot, 5.0)
        _consume_and_assert_once(
            bot,
            {"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID},
            first_baseline,
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
        )

        # ── 边界：qi 接近上限时吃丹 clamp 到 qi_max，不得溢出 ────
        clamp_baseline = _set_qi_and_wait(bot, 95.0)
        final_inventory = _consume_and_assert_once(
            bot,
            {"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID},
            clamp_baseline,
        )
        # 三丹吃完：inventory 不应再有 huiyuan_pill
        assert find_item(final_inventory, PILL_ID) is None, (
            "三枚回元丹逐次各扣一枚后 inventory 应为 0——不扣存 = 无限白嫖丹药"
        )

        # ── 负分支：背包无丹再吃（宽容不踢）──────────────────────
        bot.intent({"type": "alchemy_take_pill", "v": 1, "pill_item_id": PILL_ID})
        time.sleep(1.0)
        bot.assert_alive("空丹重复吃之后（宽容红线：不得踢线/panic）")
