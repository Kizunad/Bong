"""战斗 bot 场景 helper。

下划线前缀让 run_scenarios 跳过本模块。
"""

from __future__ import annotations

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
    """Queue a deterministic production-combat target."""
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成战斗 NPC，实际 position=None")

    anchor = last_event_time(bot)
    queue_npc_scenario(bot, "passive_target")

    try:
        return bot.wait_for(
            lambda e: e.kind == "entity_spawn"
            and e.t > anchor
            and e.data.get("entity_id") != bot.entity_id
            and _horizontal_distance(bot.position, _event_position(e)) <= 40.0,
            timeout=15.0,
            description="/npc_scenario passive_target 后水平 40 格内出现 NPC entity_spawn",
        )
    except BotAssertionError as error:
        recent_spawns = [event for event in bot.events_of("entity_spawn") if event.t > anchor]
        recent_decode_errors = [
            event for event in bot.events_of("decode_error") if event.t > anchor
        ]
        raise BotAssertionError(
            f"{error}; bot_position={bot.position}; "
            f"post_anchor_entity_spawns={recent_spawns[-5:]}; "
            f"post_anchor_decode_errors={recent_decode_errors[-5:]}"
        ) from error


def queue_passive_target(bot: Bot) -> Event:
    if bot.position is None:
        raise BotAssertionError("期望已有 bot.position 后再生成被动靶，实际 position=None")

    anchor = last_event_time(bot)
    queue_npc_scenario(bot, "passive_target")

    return bot.wait_for(
        lambda e: e.kind == "entity_spawn"
        and e.t > anchor
        and e.data.get("entity_id") != bot.entity_id
        and _horizontal_distance(bot.position, _event_position(e)) <= 40.0,
        timeout=15.0,
        description="/npc_scenario passive_target 后 40 格内出现被动靶 entity_spawn",
    )


def wait_for_skill_binding(bot: Bot, anchor: float, slot: int, skill_id: str) -> Event:
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > anchor
        and event.data.get("payload_type") == "skillbar_config"
        and isinstance(event.data.get("payload"), dict)
        and len(event.data["payload"].get("slots", [])) > slot
        and isinstance(event.data["payload"]["slots"][slot], dict)
        and event.data["payload"]["slots"][slot].get("kind") == "skill"
        and event.data["payload"]["slots"][slot].get("skill_id") == skill_id,
        timeout=10.0,
        description=f"skillbar_config 槽 {slot} 权威确认绑定 skill `{skill_id}`",
    )


def wait_for_target_destroyed(bot: Bot, anchor: float, entity_id: int, timeout: float = 15.0) -> Event:
    return bot.wait_for(
        lambda event: event.kind == "entities_destroy"
        and event.t > anchor
        and entity_id in event.data.get("entity_ids", []),
        timeout=timeout,
        description=f"生产死亡链路销毁精确 NPC entity_id={entity_id}",
    )


def move_to_melee_range(bot: Bot, spawn: Event, distance: float = 1.2) -> None:
    if bot.position is None:
        raise BotAssertionError("move_to_melee_range 需要 bot.position，实际 None")

    bx, _by, bz = bot.position
    target_pos = bot.entity_pos(int(spawn.data["entity_id"]))
    if target_pos is None:
        target_pos = _event_position(spawn)
    tx, ty, tz = target_pos
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
    expected_types: set[str],
    timeout: float,
    description: str,
) -> Event:
    """Wait for a successfully decoded production server_data payload."""
    return bot.wait_for(
        lambda e: e.kind == "server_data"
        and e.t > anchor
        and e.data.get("payload_type") in expected_types
        and isinstance(e.data.get("payload"), dict),
        timeout=timeout,
        description=description,
    )


def is_outgoing_positive_hit(event: Event) -> bool:
    if event.kind != "server_data" or event.data.get("payload_type") != "combat_event":
        return False
    payload = event.data.get("payload")
    if not isinstance(payload, dict):
        return False
    return any(
        isinstance(entry, dict)
        and entry.get("kind") == "hit"
        and entry.get("outgoing") is True
        and isinstance(entry.get("amount"), (int, float))
        and float(entry["amount"]) > 0.0
        for entry in payload.get("events", [])
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


def _horizontal_distance(
    a: tuple[float, float, float], b: tuple[float, float, float]
) -> float:
    return math.hypot(a[0] - b[0], a[2] - b[2])


def extract_floater_amounts(payload) -> list[float]:
    """从解码后的 combat_event payload 提取全部伤害浮字 amount（正数才算命中）。"""
    return [
        float(entry.get("amount", 0.0))
        for entry in payload.get("events", [])
        if isinstance(entry, dict) and isinstance(entry.get("amount"), (int, float))
    ]
