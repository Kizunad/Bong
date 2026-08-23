"""战斗 bot 场景 helper。

下划线前缀让 run_scenarios 跳过本模块。
"""

from __future__ import annotations

import json
import math
import time
from typing import Callable

from bot.bot import Bot, BotAssertionError, Event


def wait_for_ready(bot: Bot) -> None:
    bot.expect_event("game_join", timeout=15.0)
    bot.expect_event("pos_look", timeout=15.0)


def last_event_time(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def queue_npc_scenario(bot: Bot, scenario: str = "fight") -> None:
    bot.cmd(f"npc_scenario {scenario}")
    bot.expect_chat("Scenario queued.", timeout=10.0)


def queue_fight_target(bot: Bot) -> Event:
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成战斗 NPC，实际 position=None")

    anchor = last_event_time(bot)
    queue_npc_scenario(bot, "fight")

    spawn = bot.wait_for(
        lambda e: e.kind == "entity_spawn"
        and e.t > anchor
        and e.data.get("entity_id") != bot.entity_id
        and _distance(bot.position, _event_position(e)) <= 40.0,
        timeout=15.0,
        description="/npc_scenario fight 后 40 格内出现 NPC entity_spawn",
    )
    return spawn


def queue_passive_target(bot: Bot) -> Event:
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成被动靶，实际 position=None")

    anchor = last_event_time(bot)
    queue_npc_scenario(bot, "passive_target")

    return bot.wait_for(
        lambda e: e.kind == "entity_spawn"
        and e.t > anchor
        and e.data.get("entity_id") != bot.entity_id
        and _distance(bot.position, _event_position(e)) <= 40.0,
        timeout=15.0,
        description="/npc_scenario passive_target 后 40 格内出现被动靶 entity_spawn",
    )


def move_to_melee_range(bot: Bot, spawn: Event, distance: float = 1.8) -> None:
    if bot.position is None:
        raise BotAssertionError("move_to_melee_range 需要 bot.position，实际 None")

    bx, by, bz = bot.position
    tx, ty, tz = _event_position(spawn)
    dx, dz = bx - tx, bz - tz
    length = math.hypot(dx, dz)
    if length <= 0.001:
        dx, dz, length = 1.0, 0.0, 1.0
    goal_x = tx + dx / length * distance
    goal_z = tz + dz / length * distance
    bot.move_to(goal_x, ty, goal_z, speed=5.5)
    time.sleep(0.2)


def move_to_melee_target(
    bot: Bot, target_id: int, fallback_spawn: Event, distance: float = 1.8
) -> None:
    """贴近目标的最新协议坐标，避免 spawn 后的相对位移使用旧坐标。"""
    target_pos = bot.entity_pos(target_id)
    if target_pos is None:
        target_pos = _event_position(fallback_spawn)
    if bot.position is None:
        raise BotAssertionError("move_to_melee_target 需要 bot.position，实际 None")

    bx, by, bz = bot.position
    tx, ty, tz = target_pos
    dx, dz = bx - tx, bz - tz
    length = math.hypot(dx, dz)
    if length <= 0.001:
        dx, dz, length = 1.0, 0.0, 1.0
    goal_x = tx + dx / length * distance
    goal_z = tz + dz / length * distance
    bot.move_to(goal_x, ty, goal_z, speed=5.5)
    time.sleep(0.2)


def wait_for_server_data_after(
    bot: Bot,
    anchor: float,
    expected_json_types: set[str],
    timeout: float,
    description: str,
) -> Event:
    return bot.wait_for(
        lambda e: e.kind == "payload"
        and e.t > anchor
        and e.data.get("channel") == "bong:server_data"
        and _server_data_type_matches(e.data.get("data", b""), expected_json_types),
        timeout=timeout,
        description=description,
    )


def wait_for_payload_after(
    bot: Bot,
    anchor: float,
    predicate: Callable[[Event], bool],
    timeout: float,
    description: str,
) -> Event:
    return bot.wait_for(
        lambda e: e.kind == "payload" and e.t > anchor and predicate(e),
        timeout=timeout,
        description=description,
    )


def payload_text(event: Event) -> str:
    data = event.data.get("data", b"")
    if isinstance(data, bytes):
        return data.decode("utf-8", "replace")
    return str(data)


def _event_position(event: Event) -> tuple[float, float, float]:
    return (float(event.data["x"]), float(event.data["y"]), float(event.data["z"]))


def _distance(a: tuple[float, float, float], b: tuple[float, float, float]) -> float:
    return math.dist(a, b)


def _server_data_type_matches(data: bytes, expected_types: set[str]) -> bool:
    payload_type = _json_payload_type(data)
    # 生产态 server_data 是 protobuf。深解析是 P6 范围；这里遇到非 JSON bytes 时，
    # 把动作后的新 bong:server_data 当作黑盒信号。
    return payload_type is None or payload_type in expected_types


def _json_payload_type(data: bytes) -> str | None:
    try:
        decoded = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    if isinstance(decoded, dict):
        value = decoded.get("type")
        if isinstance(value, str):
            return value
    return None


def extract_floater_amounts(payload) -> list[float]:
    """从解码后的 combat_event payload 提取全部伤害浮字 amount（正数才算命中）。"""
    return [
        float(entry.get("amount", 0.0))
        for entry in payload.get("events", [])
        if isinstance(entry, dict) and isinstance(entry.get("amount"), (int, float))
    ]
