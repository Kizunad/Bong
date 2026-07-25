"""ambient 地表解析的协议级回归。

`/ambient_spawn once` 必须复用生产 ambient scheduler 的 runtime-first / raster-fallback
地表解析，而不是把执行者空中的 Y 直接写给实体。固定 bot raster 在 (5, 3) 的
可行走 surface 为 y=72，故 Cow 和 Rat 的脚点都必须是 y=73。

本场景刻意不等待随机 ambient tick：每次观测均以命令发出前的事件水位为锚，只接受
对应命令之后、固定实体类型和固定坐标的 entity_spawn；随后再从 bot 的 spawn/move/
teleport 权威位置表复核。
"""

from __future__ import annotations

import json
import math
import os
import struct
from pathlib import Path

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time

DESCRIPTION = (
    "/ambient_spawn once 以 runtime/raster 权威地表生成 Cow/Rat，绝不继承玩家空中 Y"
)
MODULES = ["cmd", "npc", "fauna", "terrain"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_AMBIENT_FIXTURE_OWNED"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

SPAWN_ZONE = "spawn"
EXECUTOR_POSITION = (0.0, 152.0, 0.0)
SPAWN_CHUNK = (0, 0)
CANDIDATE_POSITION = (5.0, 73.0, 3.0)
CANDIDATE_COMMAND = (5, 3)
COW_ENTITY_TYPE = 18
RAT_ENTITY_TYPE = 126
POSITION_EPSILON = 1e-6
FIXTURE_KIND = "ambient-surface-v1"
FIXTURE_MANIFEST_ENV = "BOT_E2E_AMBIENT_FIXTURE_MANIFEST"
FIXTURE_TOKEN_ENV = "BOT_E2E_AMBIENT_FIXTURE_TOKEN"
FIXTURE_OWNED_ENV = "BOT_E2E_AMBIENT_FIXTURE_OWNED"
SPAN_SENTINEL = 32767
SPAN_STRIDE = 16


def _read_exact(path: Path, offset: int, size: int) -> bytes:
    try:
        with path.open("rb") as stream:
            stream.seek(offset)
            value = stream.read(size)
    except OSError as error:
        raise BotAssertionError(f"无法读取 ambient fixture 二进制 {path}: {error}") from error
    if len(value) != size:
        raise BotAssertionError(
            f"期望 fixture 文件 {path} 在 offset={offset} 有 {size} 字节，实际只有 {len(value)}；"
            "manifest/tile 不完整，不能作为本轮地表证据"
        )
    return value


def _assert_raster_fixture_contract() -> None:
    owned = os.environ.get(FIXTURE_OWNED_ENV)
    manifest_value = os.environ.get(FIXTURE_MANIFEST_ENV)
    expected_token = os.environ.get(FIXTURE_TOKEN_ENV)
    if owned != "1" or not manifest_value or not expected_token:
        raise BotAssertionError(
            "npc_ambient_surface_resolution 只接受 harness 本轮自建并标记 ownership 的 raster；"
            f"要求 {FIXTURE_OWNED_ENV}=1、{FIXTURE_MANIFEST_ENV} 与 {FIXTURE_TOKEN_ENV} 非空，"
            "复用/外部 server 无法证明其实际加载了该 fixture"
        )

    manifest_path = Path(manifest_value).resolve()
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BotAssertionError(
            f"无法读取本轮 ambient raster manifest {manifest_path}: {error}"
        ) from error

    fixture = manifest.get("bot_fixture")
    expected_fixture = {
        "kind": FIXTURE_KIND,
        "token": expected_token,
        "surface_y": 72,
        "support": "grass_block",
        "feet_y": 73,
        "head_y": 74,
    }
    if fixture != expected_fixture:
        raise BotAssertionError(
            f"期望本轮 manifest bot_fixture={expected_fixture!r}，实际 {fixture!r}；"
            "token 或 support/净空契约陈旧，拒绝运行协议断言"
        )
    if manifest.get("version") != 2 or manifest.get("tile_size") != 256:
        raise BotAssertionError(
            "ambient fixture 必须是 version=2、tile_size=256；"
            f"实际 version={manifest.get('version')!r} tile_size={manifest.get('tile_size')!r}"
        )

    tile = next(
        (
            row
            for row in manifest.get("tiles", [])
            if row.get("tile_x") == 0 and row.get("tile_z") == 0
        ),
        None,
    )
    if tile is None:
        raise BotAssertionError("ambient fixture manifest 缺少目标 tile (0,0)")
    if not tile.get("spans"):
        raise BotAssertionError("ambient fixture tile (0,0) 未声明 spans=true")

    tile_size = manifest["tile_size"]
    world_x, world_z = CANDIDATE_COMMAND
    index = world_z % tile_size * tile_size + world_x % tile_size
    tile_dir = manifest_path.parent / tile["dir"]
    span_count = _read_exact(tile_dir / "spans_count.bin", index, 1)[0]
    spans = struct.unpack(
        "<8h", _read_exact(tile_dir / "spans.bin", index * SPAN_STRIDE, SPAN_STRIDE)
    )
    surface_id = _read_exact(tile_dir / "surface_id.bin", index, 1)[0]
    water_level = struct.unpack(
        "<f", _read_exact(tile_dir / "water_level.bin", index * 4, 4)
    )[0]

    expected_spans = (-64, 72) + (SPAN_SENTINEL,) * 6
    if span_count != 1 or spans != expected_spans:
        raise BotAssertionError(
            f"期望候选列 ({world_x},{world_z}) 只有 solid span (-64,72) 且其余槽为 sentinel，"
            f"实际 count={span_count} spans={spans}; 无法证明 y=73/74 为净空"
        )
    palette = manifest.get("surface_palette", [])
    if surface_id >= len(palette) or palette[surface_id] != "grass_block":
        actual = palette[surface_id] if surface_id < len(palette) else None
        raise BotAssertionError(
            f"期望候选列 support surface_id 指向 grass_block，实际 id={surface_id} block={actual!r}"
        )
    if not math.isclose(water_level, -1.0, abs_tol=POSITION_EPSILON):
        raise BotAssertionError(
            f"期望候选列 water_level=-1.0（无液体），实际 {water_level}"
        )


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
    _assert_raster_fixture_contract()
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
