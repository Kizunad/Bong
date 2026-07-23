"""ambient 地表解析的协议级回归。

`/ambient_spawn once` 必须复用生产 ambient scheduler 的 runtime-first / raster-fallback
地表解析，而不是把执行者空中的 Y 直接写给实体。固定 bot raster 在 (5, 3) 的
可行走 surface 为 y=72，故 Cow 和 Rat 的脚点都必须是 y=73。

本场景刻意不等待随机 ambient tick：每次观测均以命令发出前的事件水位为锚，只接受
对应命令之后、固定实体类型和固定坐标的 entity_spawn；随后再从 bot 的 spawn/move/
teleport 权威位置表复核。
"""

from __future__ import annotations

import math

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = (
    "/ambient_spawn once 以 runtime/raster 权威地表生成 Cow/Rat，绝不继承玩家空中 Y"
)
MODULES = ["cmd", "npc", "fauna", "terrain"]

SPAWN_ZONE = "spawn"
EXECUTOR_POSITION = (0.0, 152.0, 0.0)
SPAWN_CHUNK = (0, 0)
CANDIDATE_POSITION = (5.0, 73.0, 3.0)
CANDIDATE_COMMAND = (5, 3)
COW_ENTITY_TYPE = 18
RAT_ENTITY_TYPE = 126
POSITION_EPSILON = 1e-6


def _chunk_is_loaded(bot, chunk: tuple[int, int]) -> bool:
    loaded = False
    with bot._lock:
        for event in bot.events:
            event_chunk = (event.data.get("x"), event.data.get("z"))
            if event.kind == "chunk_data" and event_chunk == chunk:
                loaded = True
            elif event.kind == "unload_chunk" and event_chunk == chunk:
                loaded = False
    return loaded


def _assert_position(actual, expected, context: str) -> None:
    if actual is None:
        raise BotAssertionError(f"期望 {context} 有权威位置，实际 entity_pos 返回 None")
    if not all(
        math.isclose(value, target, abs_tol=POSITION_EPSILON)
        for value, target in zip(actual, expected, strict=True)
    ):
        raise BotAssertionError(
            f"期望 {context} 坐标为 {expected}（surface y=72 时脚点必须 y=73），"
            f"实际 {actual}"
        )


def _assert_entity_surface_position(actual, label: str, entity_id: int) -> None:
    if actual is None:
        raise BotAssertionError(
            f"期望 {label} entity_id={entity_id} 在 spawn/move/teleport 位置表中可见，"
            "实际 entity_pos 返回 None"
        )
    if not math.isclose(actual[1], CANDIDATE_POSITION[1], abs_tol=POSITION_EPSILON):
        raise BotAssertionError(
            f"期望 {label} entity_id={entity_id} 的二次位置表复核仍在脚点 "
            f"y={CANDIDATE_POSITION[1]}，实际 position={actual}；"
            f"不得继承执行者空中 y={EXECUTOR_POSITION[1]}"
        )


def _wait_for_command_chat(bot, anchor: float, kind: str) -> None:
    expected = (
        f"[dev] ambient_spawn accepted: kind={kind} "
        f"x={CANDIDATE_COMMAND[0]:.3f} z={CANDIDATE_COMMAND[1]:.3f}"
    )
    reply = bot.wait_for(
        lambda event: event.kind == "chat" and event.t > anchor and event.data["text"] == expected,
        timeout=10.0,
        description=f"/ambient_spawn once {kind} 的稳定 accepted chat",
    )
    if " y=" in reply.data["text"]:
        raise BotAssertionError(
            f"命令反馈不得暴露 resolved Y（Y 属 scheduler 内部地表判定），"
            f"实际 chat={reply.data['text']!r}"
        )


def _wait_for_fixed_spawn(bot, anchor: float, entity_type: int, label: str):
    spawn = bot.wait_for(
        lambda event: event.kind == "entity_spawn"
        and event.t > anchor
        and event.data["type"] == entity_type
        and math.isclose(event.data["x"], CANDIDATE_POSITION[0], abs_tol=POSITION_EPSILON)
        and math.isclose(event.data["z"], CANDIDATE_POSITION[2], abs_tol=POSITION_EPSILON),
        timeout=10.0,
        description=(
            f"命令后 type={entity_type} 的 {label} entity_spawn，"
            f"坐标 X/Z 必须是 ({CANDIDATE_POSITION[0]}, {CANDIDATE_POSITION[2]})"
        ),
    )
    actual_y = spawn.data["y"]
    if not math.isclose(actual_y, CANDIDATE_POSITION[1], abs_tol=POSITION_EPSILON):
        raise BotAssertionError(
            f"期望 {label} entity_spawn 的脚点 y={CANDIDATE_POSITION[1]}"
            "（raster support y=72 + 1），"
            f"实际 y={actual_y}；不得把执行者空中 y={EXECUTOR_POSITION[1]} 继承给实体"
        )
    if math.isclose(actual_y, EXECUTOR_POSITION[1], abs_tol=POSITION_EPSILON):
        raise BotAssertionError(
            f"{label} 首帧错误继承了执行者 y={EXECUTOR_POSITION[1]}；"
            "ambient scheduler 必须先解析 runtime/raster 地表"
        )
    _assert_entity_surface_position(
        bot.entity_pos(spawn.data["entity_id"]),
        label,
        spawn.data["entity_id"],
    )
    return spawn


def run(env) -> None:
    with env.new_bot("AmbSur") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        teleport_anchor = last_event_time(bot)
        bot.cmd(f"tpzone {SPAWN_ZONE}")
        bot.wait_for(
            lambda event: event.kind == "chat"
            and event.t > teleport_anchor
            and event.data["text"] == f"Teleported to zone `{SPAWN_ZONE}`.",
            timeout=10.0,
            description=f"/tpzone {SPAWN_ZONE} 的确认 chat",
        )
        teleported = bot.wait_for(
            lambda event: event.kind == "pos_look"
            and event.t > teleport_anchor
            and math.isclose(
                event.data["x"], EXECUTOR_POSITION[0], abs_tol=POSITION_EPSILON
            )
            and math.isclose(
                event.data["y"], EXECUTOR_POSITION[1], abs_tol=POSITION_EPSILON
            )
            and math.isclose(
                event.data["z"], EXECUTOR_POSITION[2], abs_tol=POSITION_EPSILON
            ),
            timeout=10.0,
            description=(
                f"/tpzone {SPAWN_ZONE} 后匹配 {EXECUTOR_POSITION} 的 server 权威 "
                "PositionLook；不得把迟到的登录出生位置误认成命令回包"
            ),
        )
        _assert_position(
            (teleported.data["x"], teleported.data["y"], teleported.data["z"]),
            EXECUTOR_POSITION,
            f"/tpzone {SPAWN_ZONE} 后的执行者",
        )
        _assert_position(bot.position, EXECUTOR_POSITION, "执行者 position mirror")
        bot.wait_for(
            lambda _event: _chunk_is_loaded(bot, SPAWN_CHUNK),
            timeout=10.0,
            description=(
                f"/tpzone {SPAWN_ZONE} 后目标 chunk {SPAWN_CHUNK} 当前已加载；"
                "按 chunk_data/unload_chunk 重放判断，不能假设传送必重复投递已有 chunk"
            ),
        )

        mundane_anchor = last_event_time(bot)
        bot.cmd(f"ambient_spawn once mundane {CANDIDATE_COMMAND[0]} {CANDIDATE_COMMAND[1]}")
        _wait_for_command_chat(bot, mundane_anchor, "mundane")
        cow = _wait_for_fixed_spawn(bot, mundane_anchor, COW_ENTITY_TYPE, "Cow")

        threat_anchor = last_event_time(bot)
        bot.cmd(f"ambient_spawn once threat {CANDIDATE_COMMAND[0]} {CANDIDATE_COMMAND[1]}")
        _wait_for_command_chat(bot, threat_anchor, "threat")
        rat = _wait_for_fixed_spawn(bot, threat_anchor, RAT_ENTITY_TYPE, "Rat")

        if cow.data["entity_id"] == rat.data["entity_id"]:
            raise BotAssertionError(
                "期望 mundane Cow 与 threat Rat 是两次独立 ambient 生成，"
                f"实际复用了 entity_id={cow.data['entity_id']}"
            )
        bot.assert_alive("/ambient_spawn mundane/threat 地表解析与位置表复核后")
