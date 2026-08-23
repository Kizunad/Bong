"""修炼场景共享的权威真元状态与 dev 命令同步工具。

下划线前缀使 runner 跳过本模块；具体场景只依赖这里的通用状态契约，
避免场景模块之间横向导入。
"""

from __future__ import annotations

import math

from bot.bot import Event
from bot.scenarios._combat_helpers import last_event_time

QI_SET_TOLERANCE = 0.01


def _player_state_values(event: Event) -> tuple[float, float] | None:
    if event.kind != "server_data" or event.data.get("payload_type") != "player_state":
        return None
    payload = event.data["payload"]
    qi = payload.get("spirit_qi")
    qi_max = payload.get("spirit_qi_max")
    assert isinstance(qi, (int, float)) and not isinstance(qi, bool), (
        f"权威 player_state.spirit_qi 必须是数值，实际 {qi!r}"
    )
    assert isinstance(qi_max, (int, float)) and not isinstance(qi_max, bool), (
        f"权威 player_state.spirit_qi_max 必须是数值，实际 {qi_max!r}"
    )
    qi_value = float(qi)
    qi_max_value = float(qi_max)
    assert math.isfinite(qi_value), (
        f"权威 player_state.spirit_qi 必须是有限数，实际 {qi_value!r}"
    )
    assert math.isfinite(qi_max_value) and qi_max_value > 0.0, (
        "权威 player_state.spirit_qi_max 必须是有限正数，"
        f"实际 {qi_max_value!r}"
    )
    return qi_value, qi_max_value


def _player_state_qi(event: Event) -> float | None:
    state = _player_state_values(event)
    return None if state is None else state[0]


def _wait_authoritative_qi(
    bot, anchor: float, expected: float, timeout: float = 12.0
) -> Event:
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


def _wait_authoritative_qi_max(
    bot, anchor: float, expected: float, timeout: float = 12.0
) -> Event:
    return bot.wait_for(
        lambda event: event.t > anchor
        and (
            lambda state: state is not None
            and abs(state[1] - expected) <= QI_SET_TOLERANCE
        )(_player_state_values(event)),
        timeout=timeout,
        description=(
            f"t>{anchor:.3f}s 后权威 player_state.spirit_qi_max="
            f"{expected}±{QI_SET_TOLERANCE}"
        ),
    )


def _is_qi_set_confirmation(event, anchor: float, value: float) -> bool:
    if event.kind != "chat" or event.t <= anchor:
        return False
    text = event.data.get("text", "")
    return text.startswith("[dev] qi set ") and text.endswith(f" -> {value:.1f}")


def _is_qi_max_confirmation(event, anchor: float, value: float) -> bool:
    if event.kind != "chat" or event.t <= anchor:
        return False
    text = event.data.get("text", "")
    return text.startswith("[dev] qi max ") and f" -> {value:.1f}; current=" in text


def _set_qi_and_wait(bot, value: float) -> Event:
    anchor = last_event_time(bot)
    bot.cmd(f"qi set {value}")
    bot.wait_for(
        lambda event: _is_qi_set_confirmation(event, anchor, value),
        timeout=10.0,
        description=f"t>{anchor:.3f}s 后收到本次 qi set 精确目标 -> {value:.1f} 的确认",
    )
    return _wait_authoritative_qi(bot, anchor, value)


def _set_qi_max_and_wait(bot, value: float) -> Event:
    anchor = last_event_time(bot)
    bot.cmd(f"qi max {value}")
    bot.wait_for(
        lambda event: _is_qi_max_confirmation(event, anchor, value),
        timeout=10.0,
        description=f"t>{anchor:.3f}s 后收到本次 qi max 精确目标 -> {value:.1f} 的确认",
    )
    return _wait_authoritative_qi_max(bot, anchor, value)
