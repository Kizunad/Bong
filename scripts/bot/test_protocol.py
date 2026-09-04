#!/usr/bin/env python3
"""Bot 框架编解码底座 + runner 纯逻辑的单元测试（无需 server，纯 stdlib）。

跑法：python3 scripts/bot/test_protocol.py
bot-e2e.sh 在起 server 之前先跑本文件——编解码坏了没必要浪费一次 server 启动。
"""

from __future__ import annotations

import ast
import io
import json
import math
import os
import pathlib
import re
import shutil
import shlex
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import types
import unittest
import uuid
import zlib
from contextlib import redirect_stdout
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from bot import _redis_helpers  # noqa: E402
from bot import _server_log  # noqa: E402
from bot import mc_protocol as mc  # noqa: E402
from bot import make_novice_raster_fixture  # noqa: E402
from bot import proto_min  # noqa: E402
from bot import run_scenarios as scenario_runner  # noqa: E402
from bot._agent_ui_helpers import (  # noqa: E402
    DEFAULT_UI_XML,
    REQUEST_TIMEOUT_MAX,
    REQUEST_TIMEOUT_MIN,
    REQUEST_XML_MAX_BYTES,
    assert_request_shape,
    build_cmd,
    expect_agent_ui_close,
    expect_agent_ui_request,
    expect_redis_response,
    response_matches,
)
from bot._redis_client_helpers import (  # noqa: E402
    RedisClient,
    RespFrames,
    _INCOMPLETE_FRAME,
    _encode_command,
)
from bot.bot import Bot, BotAssertionError, _signed_12, _signed_26  # noqa: E402
from bot.server_data import decode_server_data_payload  # noqa: E402
from bot.scenarios._combat_helpers import (  # noqa: E402
    is_outgoing_positive_hit,
    wait_for_server_data_after,
)
from bot.scenarios._coffin_helpers import teardown_coffin  # noqa: E402
from bot.scenarios._cultivation_helpers import (  # noqa: E402
    _is_qi_max_confirmation,
    _is_qi_set_confirmation,
    _player_state_values,
    _set_qi_and_wait,
    _set_qi_max_and_wait,
)
from bot.scenarios._inventory_helpers import (  # noqa: E402
    inventory_signature,
    give_inventory_revision_barrier,
    latest_inventory_snapshot,
    require_pack_container,
    wait_inventory_revision_after,
    wait_inventory_revision_after_matching,
    wait_inventory_snapshot_after,
)
from bot.scenarios import network_session_token_stale as stale_session_scenario  # noqa: E402
from bot.scenarios import freshness_probe_paths as freshness_probe_scenario  # noqa: E402
from bot.scenarios._rejection_helpers import (  # noqa: E402
    assert_no_gameplay_side_effect_since,
    assert_valid_request_still_works,
    fire_probes_and_keep_connection,
    inventory_fingerprint,
    wait_keepalive_after,
)
from bot.scenarios import (  # noqa: E402
    production_forge_consecration_inject,
    production_forge_request,
)
from bot.scenarios.combat_skill_cast import (  # noqa: E402
    AUDIO_FLAG,
    AUDIO_RECIPE_ID,
    SKILL_ICON,
    _assert_binding_feedback,
    _is_dugu_audio_play,
    _wait_authoritative_qi_state,
    _wait_successful_cast_sequence,
)
from bot.scenarios.cultivation_breakthrough import (  # noqa: E402
    MIN_GATE_TPS as BREAKTHROUGH_MIN_GATE_TPS,
    PHASE_TIMEOUT_MARGIN_SECONDS,
    PHASES as BREAKTHROUGH_PHASES,
    _phase_timeout_seconds,
    _wait_authoritative_realm,
    _wait_cinematic_terminal,
)
from bot.scenarios.cultivation_realm_qi import (  # noqa: E402
    _chat_after,
    _successful_command_and_chat,
)
from bot.scenarios.cultivation_pill_consume import (  # noqa: E402
    NON_CLAMP_EXPECTED_QI,
    PILL_ID,
    PILL_QI_RECOVERY,
    SERVER_TICK_OBSERVATION_TICKS,
    _assert_settled_consumption,
    _expected_qi_after_pill,
    _has_departed_baseline,
    _server_tick_from_event,
    _snapshot_after_server_tick_fence,
)
from bot.scenarios.inventory_container_open_minimal import (  # noqa: E402
    _parse_storage_crate_source_kind,
)
from bot.scenarios.inventory_pack_move_intents import _uncover_pack  # noqa: E402
from bot.scenarios.npc_ambient_surface_resolution import (  # noqa: E402
    FIXTURE_MANIFEST_ENV,
    FIXTURE_OWNED_ENV,
    FIXTURE_TOKEN_ENV,
    _assert_raster_fixture_contract,
)
from bot.scenarios.network_lifecycle_abrupt_reconnect import (  # noqa: E402
    MOVE_DISTANCE,
    MOVE_VERIFY_TOLERANCE,
    POSITION_TOLERANCE,
    _assert_restored_position,
    _move_and_record,
    _position_from_authoritative_event,
)
from bot.scenarios.network_lifecycle_idle_timeout import (  # noqa: E402
    _wait_before_deadline,
)
from bot.scenarios.network_lifecycle_stale_session import (  # noqa: E402
    _assert_inactive_session_state,
)
from bot.scenarios.network_quickslot_config import (  # noqa: E402
    _expect_bind_response,
)
from bot.scenarios.network_skill_config_intent import (  # noqa: E402
    _expect_config_snapshot,
)
from bot.scenarios._npc_dialogue_helpers import (  # noqa: E402
    _scenario_spawn_matches,
    _trade_metadata,
)
from bot.scenarios import npc_dialogue_chain_trade as npc_dialogue_trade  # noqa: E402
from bot.scenarios.production_craft_disconnect_resume import (  # noqa: E402
    CRAFT_PROGRESS_OBSERVATION_TIMEOUT_SECONDS,
    DISCONNECT_SETTLE_SECONDS,
    _reconnectable_session,
)
from bot.scenarios.production_lingtian_gathering_intents import (  # noqa: E402
    BOTANY_FIXTURE_PREFIX,
    HERB_ID,
    _is_matching_gathering_terminal,
    _is_matching_harvest,
    _parse_botany_fixture,
    _surface_candidates,
    _valid_target_pos,
    _wait_gather_progress,
)
from bot.scenarios.production_spiritwood_full_inventory_drop import (  # noqa: E402
    LUMBER_TERMINAL_TIMEOUT_SECONDS,
)
from bot.scenarios.terrain_join_chunk_delivery import (  # noqa: E402
    EXPECTED_CI_CLUSTERS,
    MIN_CHUNKS_AFTER_CENTER,
    REQUIRED_ENV as FALLBACK_OWNED_ENV,
    _assert_expected_cluster,
)
from bot.scenarios.terrain_poi_novice_startup import (  # noqa: E402
    _selection_strategy,
)
from bot.scenarios.terrain_north_rift_scorch_zone_identity import (  # noqa: E402
    PROBES as NORTH_RIFT_PROBES,
    REQUIRED_ENV as NORTH_RIFT_REQUIRED_ENV,
    _assert_ambient as north_rift_assert_ambient,
    _position_matches as north_rift_position_matches,
)
from bot.scenarios.terrain_join_chunk_delivery import (  # noqa: E402
    EXPECTED_CI_CLUSTERS,
    _assert_expected_cluster,
)
from bot.run_scenarios import (  # noqa: E402
    ScenarioEnv,
    check_server_reachable,
    discover_scenarios,
    validate_scenario_module,
)


class VarIntTest(unittest.TestCase):
    def test_roundtrip_boundaries(self):
        for value in [0, 1, 127, 128, 255, 300, 25565, 2**21, 2**28, 2**31 - 1, -1, -(2**31)]:
            reader = mc.Reader(mc.write_varint(value))
            got = reader.varint()
            self.assertEqual(
                got, value, f"varint 往返应无损（编码后解码回原值），{value} 却变成 {got}"
            )

    def test_negative_one_canonical_bytes(self):
        # -1 的协议标准编码是 5 字节全 f + 0f —— 锁字节形状防止两端编码器漂移
        self.assertEqual(mc.write_varint(-1), b"\xff\xff\xff\xff\x0f")

    def test_overlong_varint_raises(self):
        with self.assertRaises(ValueError, msg="6 字节连续位 varint 应报流错位而非静默吞"):
            mc.Reader(b"\xff\xff\xff\xff\xff\xff").varint()


class StringTest(unittest.TestCase):
    def test_roundtrip(self):
        for text in ["", "abc", "骨币", "bong:client_request", "a" * 500]:
            reader = mc.Reader(mc.mc_string(text))
            self.assertEqual(reader.string(), text)


class BlockPositionTest(unittest.TestCase):
    def test_roundtrip_corners(self):
        cases = [
            (0, 0, 0),
            (18357644, 831, -20882616),  # wiki.vg 经典样例坐标
            (-1, -1, -1),
            (-33554432, -2048, -33554432),  # 各字段最小值
            (33554431, 2047, 33554431),  # 各字段最大值
        ]
        for x, y, z in cases:
            packed = struct.unpack(">Q", mc.block_position(x, y, z))[0]
            got = (
                _signed_26(packed >> 38),
                _signed_12(packed & 0xFFF),
                _signed_26((packed >> 12) & 0x3FFFFFF),
            )
            self.assertEqual(got, (x, y, z), f"Position 编码往返 {x, y, z} 变成 {got}")


class DiggingActionTest(unittest.TestCase):
    def test_start_digging_encodes_vanilla_player_action(self):
        bot = _bare_bot()
        sent = []
        bot._send = lambda packet_id, body=b"": sent.append((packet_id, body))

        bot.start_digging(1292, 73, 1519, face=1, sequence=7)

        self.assertEqual(len(sent), 1)
        packet_id, body = sent[0]
        self.assertEqual(packet_id, mc.C2S_PLAYER_ACTION)
        reader = mc.Reader(body)
        self.assertEqual(reader.varint(), 0, "action=0 才是 Start Destroy Block")
        packed = struct.unpack(">Q", reader.data[reader.pos : reader.pos + 8])[0]
        reader.pos += 8
        self.assertEqual(
            (
                _signed_26(packed >> 38),
                _signed_12(packed & 0xFFF),
                _signed_26((packed >> 12) & 0x3FFFFFF),
            ),
            (1292, 73, 1519),
        )
        self.assertEqual(reader.u8(), 1)
        self.assertEqual(reader.varint(), 7)
        self.assertEqual(reader.rest(), b"")

    def test_start_digging_rejects_invalid_face_and_sequence(self):
        bot = _bare_bot()
        bot._send = lambda *_args: self.fail("invalid digging request must not be sent")
        for face in (-1, 6):
            with self.assertRaises(ValueError):
                bot.start_digging(0, 0, 0, face=face)
        with self.assertRaises(ValueError):
            bot.start_digging(0, 0, 0, sequence=-1)
        with self.assertRaises(ValueError):
            bot.start_digging(0, 0, 0, sequence=0x80000000)

    def test_start_digging_accepts_maximum_non_negative_varint_sequence(self):
        bot = _bare_bot()
        sent = []
        bot._send = lambda packet_id, body=b"": sent.append((packet_id, body))

        bot.start_digging(0, 0, 0, sequence=0x7FFFFFFF)

        reader = mc.Reader(sent[0][1])
        self.assertEqual(reader.varint(), 0)
        reader.pos += 8
        self.assertEqual(reader.u8(), 1)
        self.assertEqual(reader.varint(), 0x7FFFFFFF)
        self.assertEqual(reader.rest(), b"")

    def test_player_action_response_decodes_sequence(self):
        bot = _bare_bot()
        bot._dispatch(
            mc.write_varint(mc.S2C_PLAYER_ACTION_RESPONSE) + mc.write_varint(17)
        )

        event = bot.events[-1]
        self.assertEqual(event.kind, "player_action_response")
        self.assertEqual(event.data, {"sequence": 17})


def _finite_float(value: object) -> bool:
    """oracle 自带的有限性判定（不调用生产 helper，保持 oracle 独立）：值可转成
    有限 float 才算合法；超界大整数 float() 抛 OverflowError，视作非有限。"""
    try:
        return math.isfinite(float(value))
    except OverflowError:
        return False


def _spawn_distribution_tiles(
    zones_path=make_novice_raster_fixture.DEFAULT_ZONES_PATH,
) -> set[tuple[int, int]]:
    """独立 tile 期望枚举：直接从 zones.json 读 spawn 分布，按每个簇锚点±半径的
    完整笛卡尔跨度算出全部 tile。绝不调用生产 make_novice_raster_fixture.spawn_fixture_tiles
    —— review finding：用生产 helper 做期望（圆形 oracle）会让只覆盖边界点的残缺
    实现自证其绿。

    review finding：oracle 展开前必须先做与生产契约一致的数值校验 + MAX_FIXTURE_TILES
    工作量封顶——被 mock 掉生产 helper 的测试会直接调本 oracle，巨大半径（如 1e9）
    的簇若直接进 range() 会跑万亿次迭代挂死 CI，而不是产生有界的校验失败。"""
    config = json.loads(zones_path.read_text(encoding="utf-8"))
    spawn_zone = next(
        zone for zone in config["zones"] if zone.get("name") == "spawn"
    )
    tiles = set()
    for index, cluster in enumerate(spawn_zone.get("spawn_distribution", [])):
        x, _, z = cluster["anchor"]
        radius = cluster["radius"]
        for value in (x, z, radius):
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise ValueError(
                    f"invalid spawn_distribution[{index}] in {zones_path}"
                )
        if not all(_finite_float(value) for value in (x, z, radius)) or radius < 0:
            raise ValueError(
                f"invalid spawn_distribution[{index}] in {zones_path}"
            )
        if not all(
            _finite_float(value)
            for value in (
                x - radius,
                x + radius,
                z - radius,
                z + radius,
            )
        ):
            raise ValueError(
                f"invalid spawn_distribution[{index}] in {zones_path}"
            )
        min_tile_x = math.floor(
            (x - radius) / make_novice_raster_fixture.TILE_SIZE
        )
        max_tile_x = math.floor(
            (x + radius) / make_novice_raster_fixture.TILE_SIZE
        )
        min_tile_z = math.floor(
            (z - radius) / make_novice_raster_fixture.TILE_SIZE
        )
        max_tile_z = math.floor(
            (z + radius) / make_novice_raster_fixture.TILE_SIZE
        )
        cluster_tile_count = (max_tile_x - min_tile_x + 1) * (
            max_tile_z - min_tile_z + 1
        )
        if cluster_tile_count > make_novice_raster_fixture.MAX_FIXTURE_TILES:
            raise ValueError(
                f"spawn_distribution[{index}] covers {cluster_tile_count} fixture "
                f"tiles, above safety limit "
                f"{make_novice_raster_fixture.MAX_FIXTURE_TILES}"
            )
        for tile_z in range(min_tile_z, max_tile_z + 1):
            for tile_x in range(min_tile_x, max_tile_x + 1):
                tiles.add((tile_x, tile_z))
                if len(tiles) > make_novice_raster_fixture.MAX_FIXTURE_TILES:
                    raise ValueError(
                        f"spawn_distribution union covers at least {len(tiles)} "
                        f"fixture tiles, above safety limit "
                        f"{make_novice_raster_fixture.MAX_FIXTURE_TILES}"
                    )
    return tiles


def _boundary_point_only_spawn_tiles(
    zones_path=make_novice_raster_fixture.DEFAULT_ZONES_PATH,
) -> set[tuple[int, int]]:
    """review finding 假设的残缺实现：每个簇只返回四个边界点所在 tile，而非完整
    笛卡尔跨度。独立 oracle 必须能抓住这种实现（旧圆形 oracle 会放过）。"""
    config = json.loads(zones_path.read_text(encoding="utf-8"))
    spawn_zone = next(
        zone for zone in config["zones"] if zone.get("name") == "spawn"
    )
    tiles = set()
    for cluster in spawn_zone.get("spawn_distribution", []):
        x, _, z = cluster["anchor"]
        radius = cluster["radius"]
        for point_x, point_z in (
            (x - radius, z),
            (x + radius, z),
            (x, z - radius),
            (x, z + radius),
        ):
            tiles.add(
                (
                    math.floor(point_x / make_novice_raster_fixture.TILE_SIZE),
                    math.floor(point_z / make_novice_raster_fixture.TILE_SIZE),
                )
            )
    return tiles


class NoviceRasterFixtureTest(unittest.TestCase):
    TOKEN = "unit-test-ambient-fixture-token"

    def _generate(self, root: pathlib.Path) -> pathlib.Path:
        return make_novice_raster_fixture.generate(root, self.TOKEN)

    def _contract_env(self, manifest_path: pathlib.Path, token: str | None = None):
        return mock.patch.dict(
            os.environ,
            {
                FIXTURE_OWNED_ENV: "1",
                FIXTURE_MANIFEST_ENV: str(manifest_path),
                FIXTURE_TOKEN_ENV: self.TOKEN if token is None else token,
            },
            clear=False,
        )

    def test_fixture_exposes_deterministic_spiritwood_seed_without_changing_poi_tile(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

            # review finding：期望 tile 集不得由生产 spawn_fixture_tiles 算出（圆形
            # oracle）—— 用独立枚举覆盖完整笛卡尔跨度，残缺实现才可能暴露。
            expected_tiles = (
                _spawn_distribution_tiles()
                | make_novice_raster_fixture.SPIRITWOOD_TILES
            )
            self.assertEqual(
                {(tile["tile_x"], tile["tile_z"]) for tile in manifest["tiles"]},
                expected_tiles,
            )
            # review finding：world_bounds 必须精确等于所有生成 tile 的外包盒 ——
            # 期望值**独立**从 tile 集算出，不得调用生产 _world_bounds helper（实现若
            # 加了多余一格 margin / 返回旧固定边界 / 过度放宽，委托 helper 算期望会
            # 跟着同病而假通过；包含性断言也测不出过宽边界）。
            min_tile_x = min(tile_x for tile_x, _ in expected_tiles)
            max_tile_x = max(tile_x for tile_x, _ in expected_tiles)
            min_tile_z = min(tile_z for _, tile_z in expected_tiles)
            max_tile_z = max(tile_z for _, tile_z in expected_tiles)
            self.assertEqual(
                manifest["world_bounds"],
                {
                    "min_x": min_tile_x * make_novice_raster_fixture.TILE_SIZE,
                    "max_x": (max_tile_x + 1) * make_novice_raster_fixture.TILE_SIZE - 1,
                    "min_z": min_tile_z * make_novice_raster_fixture.TILE_SIZE,
                    "max_z": (max_tile_z + 1) * make_novice_raster_fixture.TILE_SIZE - 1,
                },
            )
            palette = manifest["biome_palette"]
            self.assertEqual(palette[4], "minecraft:meadow")
            for tile in manifest["tiles"]:
                biome_ids = (
                    root / tile["dir"] / "biome_id.bin"
                ).read_bytes()
                self.assertEqual(len(biome_ids), make_novice_raster_fixture.TILE_SIZE**2)
                self.assertLess(max(biome_ids), len(palette))

            for tile_x, tile_z in _spawn_distribution_tiles():
                self.assertEqual(
                    set((root / f"tile_{tile_x}_{tile_z}" / "biome_id.bin").read_bytes()),
                    {0},
                )
            for tile_x, tile_z in make_novice_raster_fixture.SPIRITWOOD_TILES:
                self.assertEqual(
                    set((root / f"tile_{tile_x}_{tile_z}" / "biome_id.bin").read_bytes()),
                    {4},
                )
            spirit_biomes = (root / "tile_5_5" / "biome_id.bin").read_bytes()
            seed_index = (1519 - 5 * 256) * 256 + (1292 - 5 * 256)
            self.assertEqual(spirit_biomes[seed_index], 4)

    def test_fixture_rejects_spawn_and_spiritwood_biome_overlap(self):
        with tempfile.TemporaryDirectory() as temp_dir, mock.patch.object(
            make_novice_raster_fixture,
            "spawn_fixture_tiles",
            return_value={next(iter(make_novice_raster_fixture.SPIRITWOOD_TILES))},
        ):
            with self.assertRaisesRegex(ValueError, "biome ownership would be ambiguous"):
                self._generate(pathlib.Path(temp_dir))

    def test_spawn_fixture_tiles_rejects_oversized_cluster_before_expansion(self):
        config = {
            "zones": [
                {
                    "name": "spawn",
                    "spawn_distribution": [
                        {
                            "anchor": [0.0, 70.0, 0.0],
                            "radius": 1_000_000_000.0,
                            "weight": 1,
                            "safe_y": make_novice_raster_fixture.SURFACE_Y,
                        }
                    ],
                }
            ]
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "above safety limit"):
                make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_fixture_tiles_rejects_finite_values_overflowing_derived_bounds(self):
        # review finding：anchor=1e308、radius=1e308 都是**有限**值，通过逐字段有限性
        # 校验，但 anchor+radius 求值为 inf，math.floor(inf) 抛 OverflowError——非法分布
        # 以意外异常类型中止 fixture 生成。修复后派生边界 (anchor ± radius) 必须先证明
        # 有限，否则一律 ValueError 干净拒绝，绝不泄漏 OverflowError。
        cluster = {
            "anchor": [1e308, make_novice_raster_fixture.SURFACE_Y, 0.0],
            "radius": 1e308,
            "weight": 1,
            "safe_y": make_novice_raster_fixture.SURFACE_Y,
        }
        config = {"zones": [{"name": "spawn", "spawn_distribution": [cluster]}]}
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, r"invalid spawn_distribution\[0\]"):
                make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_tile_oracle_rejects_oversized_cluster_before_expansion(self):
        # review finding：独立 oracle 未加 MAX_FIXTURE_TILES 预检查——被测测试 mock 掉
        # 生产 spawn_fixture_tiles（生产 cap 被旁路）后直接调本 oracle，巨大半径的簇
        # 会在 range() 里跑万亿次迭代挂死 CI。oracle 必须在展开前与生产契约一致地
        # fail-fast；本测试能在真实时间约束下完成本身即证明有界。
        config = {
            "zones": [
                {
                    "name": "spawn",
                    "spawn_distribution": [
                        {
                            "anchor": [0.0, 70.0, 0.0],
                            "radius": 1_000_000_000.0,
                            "weight": 1,
                            "safe_y": make_novice_raster_fixture.SURFACE_Y,
                        }
                    ],
                }
            ]
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "above safety limit"):
                _spawn_distribution_tiles(zones_path)

    def test_spawn_tile_oracle_rejects_finite_values_overflowing_derived_bounds(self):
        # review finding：oracle 与生产契约对齐——派生边界 (anchor ± radius) 非有限时
        # 必须以 ValueError 拒绝，不得让 math.floor(inf) 泄漏 OverflowError。
        cluster = {
            "anchor": [1e308, make_novice_raster_fixture.SURFACE_Y, 0.0],
            "radius": 1e308,
            "weight": 1,
            "safe_y": make_novice_raster_fixture.SURFACE_Y,
        }
        config = {"zones": [{"name": "spawn", "spawn_distribution": [cluster]}]}
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, r"invalid spawn_distribution\[0\]"):
                _spawn_distribution_tiles(zones_path)

    def test_spawn_fixture_tiles_rejects_huge_json_integers_as_valueerror(self):
        # review finding：JSON 解析可产生任意大整数，`float(10**1000)` 在 isfinite 看到
        # 之前就抛 OverflowError —— 修复前巨大 anchor/radius 以意外异常类型中止 fixture
        # 生成，破坏"非法分布以 ValueError 干净拒绝、绝无文件写出"的契约。修复后巨大整数
        # 必须一律判非法并抛 ValueError，不得泄漏 OverflowError。
        for field in ("radius", "anchor"):
            with self.subTest(field=field):
                cluster = {
                    "anchor": [0.0, make_novice_raster_fixture.SURFACE_Y, 0.0],
                    "radius": 0.0,
                    "weight": 1,
                    "safe_y": make_novice_raster_fixture.SURFACE_Y,
                }
                if field == "radius":
                    cluster["radius"] = 10**1000
                else:
                    cluster["anchor"][0] = 10**1000
                config = {
                    "zones": [{"name": "spawn", "spawn_distribution": [cluster]}]
                }
                with tempfile.TemporaryDirectory() as temp_dir:
                    zones_path = pathlib.Path(temp_dir) / "zones.json"
                    zones_path.write_text(json.dumps(config), encoding="utf-8")
                    with self.assertRaises(ValueError):
                        make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_fixture_tiles_rejects_union_over_limit_during_construction(self):
        clusters = []
        for index in range(make_novice_raster_fixture.MAX_FIXTURE_TILES + 1):
            clusters.append(
                {
                    "anchor": [
                        float(index * make_novice_raster_fixture.TILE_SIZE * 10),
                        70.0,
                        0.0,
                    ],
                    "radius": 0.0,
                    "weight": 1,
                    "safe_y": make_novice_raster_fixture.SURFACE_Y,
                }
            )
        config = {
            "zones": [{"name": "spawn", "spawn_distribution": clusters}]
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(
                ValueError, "spawn_distribution union covers at least 65 fixture tiles"
            ):
                make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_fixture_rejects_total_tile_union_before_writing(self):
        spawn_tiles = {
            (index, 0)
            for index in range(make_novice_raster_fixture.MAX_FIXTURE_TILES)
        }
        with tempfile.TemporaryDirectory() as temp_dir, mock.patch.object(
            make_novice_raster_fixture,
            "spawn_fixture_tiles",
            return_value=spawn_tiles,
        ):
            root = pathlib.Path(temp_dir)
            with self.assertRaisesRegex(ValueError, "total tiles, above safety limit"):
                self._generate(root)
            self.assertFalse(
                any(root.iterdir()),
                "总 tile union 越界必须在创建 tile 目录和大文件前 fail closed",
            )

    def test_spawn_fixture_tiles_accepts_union_at_exact_maximum_tile_count(self):
        # review finding：cap 契约只有「超上限拒绝」的覆盖，缺「恰好等于
        # MAX_FIXTURE_TILES 也接受」的边界 pin —— 用 `>=` 的 off-by-one 实现会误拒合法
        # 分布。本测试构造恰好 64 个 tile 的 union（64 个 radius=0 簇，每簇占一格，
        # 间距 10 格互不重叠），断言成功返回且返回集就是这 64 格。
        clusters = []
        for index in range(make_novice_raster_fixture.MAX_FIXTURE_TILES):
            clusters.append(
                {
                    "anchor": [
                        float(index * make_novice_raster_fixture.TILE_SIZE * 10),
                        70.0,
                        0.0,
                    ],
                    "radius": 0.0,
                    "weight": 1,
                    "safe_y": make_novice_raster_fixture.SURFACE_Y,
                }
            )
        config = {
            "zones": [{"name": "spawn", "spawn_distribution": clusters}]
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            tiles = make_novice_raster_fixture.spawn_fixture_tiles(zones_path)
        expected = {
            (index * 10, 0)
            for index in range(make_novice_raster_fixture.MAX_FIXTURE_TILES)
        }
        self.assertEqual(tiles, expected)
        self.assertEqual(
            len(tiles),
            make_novice_raster_fixture.MAX_FIXTURE_TILES,
            "恰好在 cap 上的 union 必须被接受，`>=` 的 off-by-one 实现在此必红",
        )

    def test_fixture_covers_every_production_spawn_cluster_with_grass(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            generated_tiles = {
                (tile["tile_x"], tile["tile_z"]) for tile in manifest["tiles"]
            }

            # review finding：只抽样每簇四个边界点，残缺实现仍可漏掉跨度内部的 tile。
            # 用独立枚举的完整笛卡尔跨度做覆盖断言，内部 tile 缺失才会暴露。
            expected_spawn_tiles = _spawn_distribution_tiles()
            self.assertLessEqual(
                expected_spawn_tiles,
                generated_tiles,
                "每个生产 spawn 簇的完整笛卡尔跨度都必须落在生成的 fixture tile 集内",
            )
            for tile in expected_spawn_tiles:
                self.assertEqual(
                    set(
                        (
                            root
                            / f"tile_{tile[0]}_{tile[1]}"
                            / "surface_id.bin"
                        ).read_bytes()
                    ),
                    {0},
                    f"production spawn tile={tile} 必须以 surface palette 0=grass_block 覆盖",
                )

            bounds = manifest["world_bounds"]
            for tile_x, tile_z in generated_tiles:
                self.assertLessEqual(bounds["min_x"], tile_x * 256)
                self.assertGreaterEqual(bounds["max_x"], (tile_x + 1) * 256 - 1)
                self.assertLessEqual(bounds["min_z"], tile_z * 256)
                self.assertGreaterEqual(bounds["max_z"], (tile_z + 1) * 256 - 1)

    def test_spawn_tile_oracle_detects_boundary_point_only_enumeration(self):
        # review finding：圆形 oracle（期望 tile 集由生产 spawn_fixture_tiles 算出）
        # 下，残缺实现只返回每簇四边界点 tile 生成的 manifest 会与期望一致而假通过。
        # 独立 oracle 直接从配置枚举完整跨度，必须与这种残缺 manifest 分歧（更大），
        # 证明 oracle 不依赖生产枚举。
        with tempfile.TemporaryDirectory() as temp_dir, mock.patch.object(
            make_novice_raster_fixture,
            "spawn_fixture_tiles",
            return_value=_boundary_point_only_spawn_tiles(),
        ):
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            generated_tiles = {
                (tile["tile_x"], tile["tile_z"]) for tile in manifest["tiles"]
            }
            oracle_tiles = (
                _spawn_distribution_tiles()
                | make_novice_raster_fixture.SPIRITWOOD_TILES
            )
            self.assertNotEqual(
                generated_tiles,
                oracle_tiles,
                "边界四点式残缺实现的 manifest 必须与独立 oracle 分歧",
            )
            self.assertLess(
                len(generated_tiles),
                len(oracle_tiles),
                "残缺实现漏掉每簇内部 tile，manifest 应小于独立 oracle",
            )
            interior = oracle_tiles - generated_tiles
            self.assertTrue(
                interior,
                "独立 oracle 必须枚举完整笛卡尔跨度，而非只覆盖边界点",
            )

    def test_spawn_fixture_tiles_rejects_missing_empty_or_invalid_distribution(self):
        cases = [
            ({"zones": []}, "missing the spawn zone"),
            ({"zones": [{"name": "spawn", "spawn_distribution": []}]}, "has no spawn_distribution"),
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": -1.0,
                                    "weight": 1,
                                    "safe_y": 72.0,
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": 1.0,
                                    "weight": 1,
                                    "safe_y": make_novice_raster_fixture.SURFACE_Y + 1,
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
            # review finding：safe_y 契约只覆盖 SURFACE_Y+1 一个非法值，实现若错误接受
            # safe_y <= SURFACE_Y、缺失 safe_y、布尔或非有限 safe_y 会全绿。生产实现
            # `cluster.get("safe_y") != SURFACE_Y` 对下面每个方向都必须拒绝。
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": 1.0,
                                    "weight": 1,
                                    "safe_y": make_novice_raster_fixture.SURFACE_Y - 1,
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": 1.0,
                                    "weight": 1,
                                    # 缺省 safe_y：get 返回 None，必须拒绝
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": 1.0,
                                    "weight": 1,
                                    "safe_y": True,
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
            (
                {
                    "zones": [
                        {
                            "name": "spawn",
                            "spawn_distribution": [
                                {
                                    "anchor": [0.0, 70.0, 0.0],
                                    "radius": 1.0,
                                    "weight": 1,
                                    "safe_y": float("nan"),
                                }
                            ],
                        }
                    ]
                },
                "invalid spawn_distribution[0]",
            ),
        ]

        # 非有限数值边界：json.loads 默认接受 NaN/Infinity，必须同样以
        # invalid spawn_distribution[0] 拒绝，而不是让 tile 计算炸成 ValueError/OverflowError。
        for index, non_finite in enumerate(
            (float("nan"), float("inf"), float("-inf"))
        ):
            anchor = [0.0, 70.0, 0.0]
            anchor[index % 3] = non_finite
            cases.append(
                (
                    {
                        "zones": [
                            {
                                "name": "spawn",
                                "spawn_distribution": [
                                    {
                                        "anchor": anchor,
                                        "radius": 1.0,
                                        "weight": 1,
                                        "safe_y": make_novice_raster_fixture.SURFACE_Y,
                                    }
                                ],
                            }
                        ]
                    },
                    "invalid spawn_distribution[0]",
                )
            )
        for non_finite in (float("nan"), float("inf"), float("-inf")):
            cases.append(
                (
                    {
                        "zones": [
                            {
                                "name": "spawn",
                                "spawn_distribution": [
                                    {
                                        "anchor": [0.0, 70.0, 0.0],
                                        "radius": non_finite,
                                        "weight": 1,
                                        "safe_y": make_novice_raster_fixture.SURFACE_Y,
                                    }
                                ],
                            }
                        ]
                    },
                    "invalid spawn_distribution[0]",
                )
            )

        for config, expected_error in cases:
            with self.subTest(expected_error=expected_error), tempfile.TemporaryDirectory() as temp_dir:
                zones_path = pathlib.Path(temp_dir) / "zones.json"
                zones_path.write_text(json.dumps(config), encoding="utf-8")
                with self.assertRaisesRegex(ValueError, re.escape(expected_error)):
                    make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_fixture_tiles_accepts_exact_surface_y(self):
        # review finding 保留的正向对照：safe_y 恰等于 SURFACE_Y（整数与相等浮点）是
        # 合法分布，必须被接受并返回期望 tile —— 拒绝合法值同样违反契约。
        for safe_y in (
            make_novice_raster_fixture.SURFACE_Y,
            float(make_novice_raster_fixture.SURFACE_Y),
        ):
            config = {
                "zones": [
                    {
                        "name": "spawn",
                        "spawn_distribution": [
                            {
                                "anchor": [0.0, 70.0, 0.0],
                                "radius": 1.0,
                                "weight": 1,
                                "safe_y": safe_y,
                            }
                        ],
                    }
                ]
            }
            with self.subTest(safe_y=safe_y), tempfile.TemporaryDirectory() as temp_dir:
                zones_path = pathlib.Path(temp_dir) / "zones.json"
                zones_path.write_text(json.dumps(config), encoding="utf-8")
                self.assertEqual(
                    make_novice_raster_fixture.spawn_fixture_tiles(zones_path),
                    {(-1, -1), (-1, 0), (0, -1), (0, 0)},
                )

    def test_spawn_fixture_tiles_rejects_boolean_anchor_components(self):
        for index in range(3):
            anchor = [0.0, 70.0, 0.0]
            anchor[index] = True
            config = {
                "zones": [
                    {
                        "name": "spawn",
                        "spawn_distribution": [
                            {
                                "anchor": anchor,
                                "radius": 1.0,
                                "weight": 1,
                                "safe_y": make_novice_raster_fixture.SURFACE_Y,
                            }
                        ],
                    }
                ]
            }
            with self.subTest(index=index), tempfile.TemporaryDirectory() as temp_dir:
                zones_path = pathlib.Path(temp_dir) / "zones.json"
                zones_path.write_text(json.dumps(config), encoding="utf-8")
                with self.assertRaisesRegex(
                    ValueError, re.escape("invalid spawn_distribution[0]")
                ):
                    make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_fixture_tiles_rejects_boolean_radius(self):
        config = {
            "zones": [
                {
                    "name": "spawn",
                    "spawn_distribution": [
                        {
                            "anchor": [0.0, 70.0, 0.0],
                            "radius": True,
                            "weight": 1,
                            "safe_y": make_novice_raster_fixture.SURFACE_Y,
                        }
                    ],
                }
            ]
        }
        with tempfile.TemporaryDirectory() as temp_dir:
            zones_path = pathlib.Path(temp_dir) / "zones.json"
            zones_path.write_text(json.dumps(config), encoding="utf-8")
            with self.assertRaisesRegex(
                ValueError, re.escape("invalid spawn_distribution[0]")
            ):
                make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

    def test_spawn_fixture_tiles_matches_production_u32_weight_contract(self):
        invalid_weights = (None, 0, -1, 1.0, True, 1 << 32)
        for weight in invalid_weights:
            cluster = {
                "anchor": [0.0, 70.0, 0.0],
                "radius": 1.0,
                "safe_y": 72.0,
            }
            label = "missing"
            if weight is not None:
                cluster["weight"] = weight
                label = repr(weight)
            config = {
                "zones": [
                    {"name": "spawn", "spawn_distribution": [cluster]}
                ]
            }
            with self.subTest(weight=label), tempfile.TemporaryDirectory() as temp_dir:
                zones_path = pathlib.Path(temp_dir) / "zones.json"
                zones_path.write_text(json.dumps(config), encoding="utf-8")
                with self.assertRaisesRegex(
                    ValueError, re.escape("invalid spawn_distribution[0]")
                ):
                    make_novice_raster_fixture.spawn_fixture_tiles(zones_path)

        for weight in (1, (1 << 32) - 1):
            config = {
                "zones": [
                    {
                        "name": "spawn",
                        "spawn_distribution": [
                            {
                                "anchor": [0.0, 70.0, 0.0],
                                "radius": 1.0,
                                "weight": weight,
                                "safe_y": 72.0,
                            }
                        ],
                    }
                ]
            }
            with self.subTest(weight=weight), tempfile.TemporaryDirectory() as temp_dir:
                zones_path = pathlib.Path(temp_dir) / "zones.json"
                zones_path.write_text(json.dumps(config), encoding="utf-8")
                self.assertEqual(
                    make_novice_raster_fixture.spawn_fixture_tiles(zones_path),
                    {(-1, -1), (-1, 0), (0, -1), (0, 0)},
                )

    def test_fixture_pins_ambient_support_air_and_no_water_contract(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(
                manifest["bot_fixture"],
                {
                    "kind": "ambient-surface-v1",
                    "token": self.TOKEN,
                    "surface_y": 72,
                    "support": "grass_block",
                    "feet_y": 73,
                    "head_y": 74,
                },
            )
            with self._contract_env(manifest_path):
                _assert_raster_fixture_contract()

    def test_ambient_fixture_contract_rejects_stale_token_and_missing_ownership(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            manifest_path = self._generate(root)
            with self._contract_env(manifest_path, token="wrong-token"):
                with self.assertRaises(BotAssertionError, msg="stale token must not pin a prior fixture"):
                    _assert_raster_fixture_contract()
            with mock.patch.dict(
                os.environ,
                {
                    FIXTURE_OWNED_ENV: "0",
                    FIXTURE_MANIFEST_ENV: str(manifest_path),
                    FIXTURE_TOKEN_ENV: self.TOKEN,
                },
                clear=False,
            ):
                with self.assertRaises(BotAssertionError, msg="reuse/no-ownership must fail closed"):
                    _assert_raster_fixture_contract()

    def test_fixture_generator_requires_explicit_token(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            with self.assertRaises(TypeError):
                make_novice_raster_fixture.generate(pathlib.Path(temp_dir))

            with self.assertRaises(ValueError):
                make_novice_raster_fixture.generate(pathlib.Path(temp_dir), " ")


class ChatTextTest(unittest.TestCase):
    def test_variants(self):
        cases = [
            ('"plain"', "plain"),
            ('{"text":"a","extra":[{"text":"b"},"c"]}', "abc"),
            ('{"extra":[{"text":"x","extra":["y"]}]}', "xy"),
            ('["a",{"text":"b"}]', "ab"),
            ("not json at all", "not json at all"),
            ("null", ""),
        ]
        for raw, expected in cases:
            self.assertEqual(mc.chat_text_to_plain(raw), expected, f"raw={raw!r}")


class ServerDataDecodeTest(unittest.TestCase):
    def test_json_inventory_snapshot_payload_decodes(self):
        payload = b'{"v":1,"type":"inventory_snapshot","revision":7}'
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "inventory_snapshot")
        self.assertEqual(decoded["revision"], 7)

    def test_malformed_server_data_returns_none(self):
        self.assertIsNone(decode_server_data_payload(b"\xff\x00not protobuf"))

    def test_proto_narration_payload_decodes_routing_fields(self):
        decoded = decode_server_data_payload(_server_data_narration_bytes())

        self.assertEqual(decoded["type"], "narration")
        self.assertEqual(
            decoded["narrations"],
            [
                {
                    "text": "天道的注意力掠过，境界未至",
                    "scope": "player",
                    "style": "system_warning",
                    "target": "offline:Alice",
                    "kind": "realm_gate_rejected",
                },
                {
                    "text": "旧式全服旁白",
                    "scope": "broadcast",
                    "style": "narration",
                },
            ],
            "Bot 必须解出 narration text/scope/style/optional target/kind，"
            "否则双玩家黑盒无法区分目标提示与同轮无关旁白",
        )

    def test_bot_dispatch_emits_decoded_narration_event(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_CUSTOM_PAYLOAD)
            + mc.mc_string("bong:server_data")
            + _server_data_narration_bytes()
        )

        bot._dispatch(body)

        decoded_events = bot.events_of("server_data")
        self.assertEqual(len(decoded_events), 1)
        self.assertEqual(decoded_events[0].data["payload_type"], "narration")
        self.assertEqual(
            decoded_events[0].data["payload"]["narrations"][0]["target"],
            "offline:Alice",
            "真实 Bot reader 必须把 production protobuf narration 暴露为可断言事件",
        )

    def test_proto_coffin_state_enter_payload_decodes(self):
        decoded = decode_server_data_payload(
            _server_data_coffin_state_bytes(True, 0.9, "mundane")
        )

        self.assertEqual(
            decoded,
            {
                "v": 1,
                "type": "coffin_state",
                "in_coffin": True,
                "lifespan_rate_multiplier": 0.9,
                "coffin_grade": "mundane",
            },
            "enter 后 CoffinState 应解出 in_coffin=true、mundane grade 与 0.9 倍率",
        )

    def test_proto_coffin_state_leave_payload_decodes(self):
        decoded = decode_server_data_payload(
            _server_data_coffin_state_bytes(False, 1.0, None)
        )

        self.assertEqual(
            decoded,
            {
                "v": 1,
                "type": "coffin_state",
                "in_coffin": False,
                "lifespan_rate_multiplier": 1.0,
                "coffin_grade": None,
            },
            "leave 后 CoffinState 应解出 in_coffin=false、grade 缺席与 1.0 倍率",
        )

    def test_proto_coffin_state_omitted_multiplier_defaults_to_one(self):
        # 只有 field 1（in_coffin）的 payload：field 2 省略时必须解出声明默认 1.0，
        # 而不是 0.0 / None / 抛错。此前两个测试总是编码 field 2（_server_data_
        # coffin_state_bytes 恒发 _pb_fixed64(2, ...)），钉不住省略字段的解码契约。
        decoded = decode_server_data_payload(_pb_message(78, _pb_varint(1, 1)))

        self.assertEqual(
            decoded,
            {
                "v": 1,
                "type": "coffin_state",
                "in_coffin": True,
                "lifespan_rate_multiplier": 1.0,
                "coffin_grade": None,
            },
            "省略 field 2 的 CoffinState 应解出 lifespan_rate_multiplier 默认 1.0",
        )

    def test_bot_dispatch_emits_entity_metadata_invisible_flag(self):
        bot = _bare_bot()
        body = mc.write_varint(mc.S2C_ENTITY_METADATA) + mc.write_varint(0) + bytes(
            [0, 0, 0x20, 0xFF]
        )

        bot._dispatch(body)

        metadata_events = bot.events_of("entity_metadata")
        self.assertEqual(len(metadata_events), 1)
        self.assertEqual(metadata_events[0].data["entity_id"], 0)
        self.assertEqual(metadata_events[0].data["flags"], 0x20)
        self.assertTrue(metadata_events[0].data["flags"] & 0x20, "bit 5 = invisible")

    def test_bot_dispatch_entity_metadata_skips_non_flags_entry(self):
        bot = _bare_bot()
        # 首条目不是 flags（index 5 = custom name visible, BOOLEAN）：flags 应为 None，
        # 不得因未知条目崩掉 dispatch。
        body = mc.write_varint(mc.S2C_ENTITY_METADATA) + mc.write_varint(7) + bytes(
            [5, 8, 1, 0xFF]
        )

        bot._dispatch(body)

        metadata_events = bot.events_of("entity_metadata")
        self.assertEqual(len(metadata_events), 1)
        self.assertEqual(metadata_events[0].data["entity_id"], 7)
        self.assertIsNone(metadata_events[0].data["flags"])

    def test_bot_dispatch_entity_metadata_flags_after_non_flags_entry(self):
        bot = _bare_bot()
        # 首条目不是 flags（index 5, BOOLEAN），flags 条目在其后：必须扫描完整条目
        # 序列直到 0xFF，flags 在任意位置都能读到（review finding, run 31442491424：
        # 旧实现只读首条目，flags 非首条目即静默丢 None，invisible 状态切换读不回）。
        body = mc.write_varint(mc.S2C_ENTITY_METADATA) + mc.write_varint(0) + bytes(
            [5, 8, 1, 0, 0, 0x20, 0xFF]
        )

        bot._dispatch(body)

        metadata_events = bot.events_of("entity_metadata")
        self.assertEqual(len(metadata_events), 1)
        self.assertEqual(metadata_events[0].data["entity_id"], 0)
        self.assertEqual(metadata_events[0].data["flags"], 0x20)

    def test_bot_dispatch_entity_metadata_flags_after_varint_and_string_entries(self):
        bot = _bare_bot()
        # VARINT（index 1 = air）与 STRING（index 2 = custom name）条目先于 flags：
        # 按类型跳过（varint / 长度前缀字符串）不得错位，否则 flags 与 0xFF 终止符
        # 都读不回来。
        body = (
            mc.write_varint(mc.S2C_ENTITY_METADATA)
            + mc.write_varint(0)
            + bytes([1, 1, 5])  # index 1, type VARINT, 值 5
            + bytes([2, 4])
            + mc.mc_string("abc")  # index 2, type STRING, 值 "abc"
            + bytes([0, 0, 0x20, 0xFF])  # index 0, type BYTE, flags=0x20
        )

        bot._dispatch(body)

        metadata_events = bot.events_of("entity_metadata")
        self.assertEqual(len(metadata_events), 1)
        self.assertEqual(metadata_events[0].data["entity_id"], 0)
        self.assertEqual(metadata_events[0].data["flags"], 0x20)

    def test_bot_dispatch_entity_metadata_skips_pinned_fork_types_before_flags(self):
        # 编号与 Kizunad/valence pinned rev 的 Value::type_id() 一一对应。
        # PARTICLE(17) 的 fork payload 不携 variant ID，无法安全跳过，故保持 fail closed。
        nbt_with_string = (
            b"\x0a\x00\x00"  # TAG_Compound root + 空 root name (u16-BE)
            + b"\x08\x00\x04name"  # TAG_String + compound key
            + b"\x00\x03abc"  # TAG_String value length is u16-BE
            + b"\x00"  # TAG_End
        )
        item_stack_with_nbt = b"\x01" + mc.write_varint(1) + b"\x01" + nbt_with_string
        cases = [
            ("byte", 0, b"\x01"),
            ("integer", 1, mc.write_varint(300)),
            ("long", 2, struct.pack(">q", 123456789)),
            ("float", 3, struct.pack(">f", 20.0)),
            ("string", 4, mc.mc_string("abc")),
            ("text", 5, mc.mc_string('{"text":"abc"}')),
            ("optional_text", 6, b"\x01" + mc.mc_string('{"text":"abc"}')),
            ("empty_item_stack", 7, b"\x00"),
            ("item_stack_nbt", 7, item_stack_with_nbt),
            ("boolean", 8, b"\x01"),
            ("rotation", 9, struct.pack(">fff", 1.0, 2.0, 3.0)),
            ("block_pos", 10, struct.pack(">Q", 0)),
            ("optional_block_pos", 11, b"\x01" + struct.pack(">Q", 0)),
            ("facing", 12, mc.write_varint(3)),
            ("optional_uuid", 13, b"\x01" + bytes(range(16))),
            ("block_state", 14, mc.write_varint(42)),
            ("optional_block_state", 15, mc.write_varint(0)),
            ("nbt_compound", 16, nbt_with_string),
            ("villager_data", 18, b"\x02\x03\x04"),
            ("optional_int", 19, mc.write_varint(8)),
            ("entity_pose", 20, mc.write_varint(5)),
            ("cat_variant", 21, mc.write_varint(2)),
            ("frog_variant", 22, mc.write_varint(1)),
            ("optional_global_pos_unit", 23, b""),
            ("painting_variant", 24, mc.write_varint(4)),
            ("sniffer_state", 25, mc.write_varint(2)),
            ("vector3f", 26, struct.pack(">fff", 1.0, 2.0, 3.0)),
            ("quaternionf", 27, struct.pack(">ffff", 1.0, 2.0, 3.0, 4.0)),
        ]

        for name, mtype, value in cases:
            with self.subTest(name=name, mtype=mtype):
                bot = _bare_bot()
                body = (
                    mc.write_varint(mc.S2C_ENTITY_METADATA)
                    + mc.write_varint(0)
                    + bytes([9, mtype])
                    + value
                    + bytes([0, 0, 0x20, 0xFF])
                )

                bot._dispatch(body)

                metadata_events = bot.events_of("entity_metadata")
                self.assertEqual(len(metadata_events), 1)
                self.assertEqual(metadata_events[0].data["flags"], 0x20)

    def test_bot_dispatch_entity_metadata_empty_entries(self):
        bot = _bare_bot()
        body = mc.write_varint(mc.S2C_ENTITY_METADATA) + mc.write_varint(0) + b"\xFF"

        bot._dispatch(body)

        metadata_events = bot.events_of("entity_metadata")
        self.assertEqual(len(metadata_events), 1)
        self.assertEqual(metadata_events[0].data["entity_id"], 0)
        self.assertIsNone(metadata_events[0].data["flags"])

    def test_proto_zone_info_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_zone_info_bytes())

        self.assertEqual(
            decoded,
            {
                "v": 1,
                "type": "zone_info",
                "zone": "rift_mouth_north_002",
                "spirit_qi": 0.068602,
                "danger_level": 5,
                "status": "Normal",
                "active_events": ["rift_mouth_entry", "wind_warning"],
                "perception_text": "灵气骤薄",
            },
            "production protobuf zone_info 应完整解出标量、repeated 与 optional 字段",
        )

    def test_proto_breakthrough_cinematic_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_breakthrough_cinematic_bytes())

        expected = {
            "v": 1,
            "type": "breakthrough_cinematic",
            "actor_id": "offline:Break",
            "phase": "prelude",
            "phase_tick": 0,
            "phase_duration_ticks": 60,
            "realm_from": "Awaken",
            "realm_to": "Induce",
            "result": "success",
            "interrupted": False,
            "world_pos": [-240.5, 72.0, -160.25],
            "visible_radius_blocks": 96.0,
            "global": False,
            "distant_billboard": True,
            "particle_density": 0.75,
            "season_overlay": "calm",
            "style": "awaken_induce",
            "at_tick": 4242,
        }
        self.assertEqual(
            {key: decoded[key] for key in expected},
            expected,
            "production protobuf field 71 必须完整解出突破 cinematic 数值与视觉身份",
        )
        self.assertAlmostEqual(decoded["intensity"], 0.35, places=6)

    def test_breakthrough_decoder_contract_matches_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        cinematic = _proto_message_body(source, "BreakthroughCinematic")

        self.assertEqual(
            _proto_field_signature(envelope, "breakthrough_cinematic"),
            ("BreakthroughCinematic", proto_min.SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD),
            "Bot field 71 分发常量必须与权威 ServerDataEnvelope 对齐",
        )
        expected_fields = {
            "actor_id": ("string", 1),
            "phase": ("string", 2),
            "phase_tick": ("uint32", 3),
            "phase_duration_ticks": ("uint32", 4),
            "realm_from": ("string", 5),
            "realm_to": ("string", 6),
            "result": ("string", 7),
            "interrupted": ("bool", 8),
            "world_pos_x": ("double", 9),
            "world_pos_y": ("double", 10),
            "world_pos_z": ("double", 11),
            "visible_radius_blocks": ("double", 12),
            "global": ("bool", 13),
            "distant_billboard": ("bool", 14),
            "particle_density": ("float", 15),
            "intensity": ("float", 16),
            "season_overlay": ("string", 17),
            "style": ("string", 18),
            "at_tick": ("uint64", 19),
        }
        self.assertEqual(_proto_field_names(cinematic), set(expected_fields))
        for field_name, expected in expected_fields.items():
            with self.subTest(field=field_name):
                self.assertEqual(
                    _proto_field_signature(cinematic, field_name),
                    expected,
                    f"Bot BreakthroughCinematic.{field_name} 解码字段必须与权威 proto 对齐",
                )

    def test_bot_dispatch_emits_decoded_breakthrough_cinematic_event(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_CUSTOM_PAYLOAD)
            + mc.mc_string("bong:server_data")
            + _server_data_breakthrough_cinematic_bytes()
        )

        bot._dispatch(body)

        decoded_events = bot.events_of("server_data")
        self.assertEqual(len(decoded_events), 1)
        self.assertEqual(
            decoded_events[0].data["payload_type"],
            "breakthrough_cinematic",
            "真实 Bot reader 必须把 envelope field 71 暴露成结构化观察事件",
        )
        self.assertEqual(decoded_events[0].data["payload"]["realm_to"], "Induce")

    def test_breakthrough_cinematic_wrong_envelope_wire_type_is_not_dispatched(self):
        self.assertIsNone(
            proto_min.decode_server_data_envelope(
                _pb_varint(proto_min.SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD, 1)
            ),
            "oneof field 71 的 varint 不得冒充 BreakthroughCinematic message",
        )

    def test_breakthrough_cinematic_truncated_message_returns_none_publicly(self):
        malformed = (
            _pb_key(proto_min.SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD, 2)
            + _pb_raw_varint(4)
            + b"ab"
        )
        self.assertIsNone(
            decode_server_data_payload(malformed),
            "截断的 field 71 必须 fail closed，而不是让 Bot reader 产出假 cinematic",
        )

    def test_proto_inventory_snapshot_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_inventory_snapshot_bytes())

        self.assertEqual(decoded["type"], "inventory_snapshot")
        self.assertEqual(decoded["revision"], 12)
        self.assertEqual(decoded["containers"][0]["id"], "body_pocket")
        item = decoded["equipped"]["chest_worn"][0]
        self.assertEqual(item["item_id"], "worn_grass_pouch")
        self.assertEqual(item["mineral_id"], "za_gang")
        self.assertEqual(item["scroll_kind"], "skill_scroll")
        self.assertEqual(item["scroll_skill_id"], "forging")
        self.assertEqual(item["scroll_xp_grant"], 500)
        self.assertEqual(item["charges"], 7)
        self.assertAlmostEqual(item["forge_quality"], 0.75, places=6)
        self.assertEqual(item["forge_color"], 1)
        self.assertEqual(item["forge_side_effects"], ["brittle_edge", "qi_shear"])
        self.assertEqual(item["forge_achieved_tier"], 3)
        self.assertEqual(
            item["alchemy"],
            {
                "kind": "pill",
                "recipe_id": "qing_xin_dan",
                "quality_tier": 2,
                "effect_multiplier": 0.9,
                "consecrated": True,
                "side_effect": {
                    "tag": "qi_drain_mild",
                    "duration_s": 30,
                    "weight": 1,
                    "perm": False,
                    "color": 2,
                    "amount": 1.5,
                },
                "fragment": None,
                "hint": None,
                "residue_kind": None,
                "produced_at_tick": None,
                "expires_at_tick": None,
            },
        )
        self.assertEqual(
            item["freshness"],
            {
                "created_at_tick": 123,
                "initial_qi": 0.5,
                "track": "Decay",
                "profile": "mineral_decay_v1",
                "frozen_accumulated": 17,
                "frozen_since_tick": 140,
            },
        )

    def test_proto_inventory_event_moved_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_inventory_event_moved_bytes())

        self.assertEqual(decoded["type"], "inventory_event")
        self.assertEqual(decoded["kind"], "moved")
        self.assertEqual(decoded["revision"], 13)
        self.assertEqual(decoded["instance_id"], 99)
        self.assertEqual(decoded["from"]["container_id"], "body_pocket")
        self.assertEqual(decoded["to"]["slot"], "chest")
        self.assertEqual(decoded["to"]["state"], "worn")

    def test_proto_loot_container_open_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_open_bytes())

        self.assertEqual(decoded["type"], "loot_container_open")
        self.assertEqual(decoded["session_id"], 7)
        self.assertEqual(decoded["rows"], 3)
        self.assertEqual(decoded["cols"], 4)

    def test_proto_loot_container_update_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_update_bytes())

        self.assertEqual(
            decoded["type"],
            "loot_container_update",
            "expected type=loot_container_update so the bot dispatches the authoritative "
            f"update payload, actual={decoded['type']}",
        )
        self.assertEqual(
            decoded["session_id"],
            7,
            "expected session_id=7 so the update remains bound to its opened session, "
            f"actual={decoded['session_id']}",
        )
        self.assertEqual(
            decoded["placed_items"][0]["container_id"],
            "ext_7",
            "expected container_id=ext_7 so the update targets the session container, "
            f"actual={decoded['placed_items'][0]['container_id']}",
        )
        self.assertEqual(
            decoded["placed_items"][0]["item"]["instance_id"],
            99,
            "expected instance_id=99 so the update preserves item identity, "
            f"actual={decoded['placed_items'][0]['item']['instance_id']}",
        )

    def test_proto_loot_container_close_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_loot_container_close_bytes())

        self.assertEqual(
            decoded["type"],
            "loot_container_close",
            "expected type=loot_container_close so the bot dispatches the close payload, "
            f"actual={decoded['type']}",
        )
        self.assertEqual(
            decoded["session_id"],
            7,
            "expected session_id=7 so close invalidates the opened session, "
            f"actual={decoded['session_id']}",
        )
        self.assertEqual(
            decoded["reason"],
            "distance",
            "expected reason=distance so the bot observes the server rejection cause, "
            f"actual={decoded['reason']}",
        )

    def test_proto_p4_gathering_payloads_deep_decode_every_observable_field(self):
        botany = (
            _pb_string(1, "harvest:17")
            + _pb_string(2, "plant:qingcao")
            + _pb_string(3, "残青草")
            + _pb_string(4, "qingcao")
            + _pb_string(5, "careful")
            + _pb_fixed64(6, 0.625)
            + _pb_varint(7, 1)
            + _pb_varint(8, 0)
            + _pb_varint(9, 1)
            + _pb_varint(10, 0)
            + _pb_string(11, "被玩家移动打断")
            + _pb_string(12, "toxic_spore")
            + _pb_string(12, "sharp_leaf")
            + _pb_fixed64(13, -12.5)
            + _pb_fixed64(14, 73.0)
            + _pb_fixed64(15, 38.25)
        )
        self.assertEqual(
            decode_server_data_payload(_pb_message(25, botany)),
            {
                "v": 1,
                "type": "botany_harvest_progress",
                "session_id": "harvest:17",
                "target_id": "plant:qingcao",
                "target_name": "残青草",
                "plant_kind": "qingcao",
                "mode": "careful",
                "progress": 0.625,
                "auto_selectable": True,
                "request_pending": False,
                "interrupted": True,
                "completed": False,
                "detail": "被玩家移动打断",
                "hazard_hints": ["toxic_spore", "sharp_leaf"],
                "target_pos": [-12.5, 73.0, 38.25],
            },
        )

        gathering = (
            _pb_string(1, "gather:9")
            + _pb_varint(2, 39)
            + _pb_varint(3, 40)
            + _pb_string(4, "碎铁矿")
            + _pb_varint(5, 2)
            + _pb_varint(6, 5)
            + _pb_string(7, "pickaxe_iron")
            + _pb_varint(8, 0)
            + _pb_varint(9, 1)
        )
        self.assertEqual(
            decode_server_data_payload(_pb_message(30, gathering)),
            {
                "v": 1,
                "type": "gathering_session",
                "session_id": "gather:9",
                "progress_ticks": 39,
                "total_ticks": 40,
                "target_name": "碎铁矿",
                "target_type": "ore",
                "quality_hint": "perfect",
                "tool_used": "pickaxe_iron",
                "interrupted": False,
                "completed": True,
            },
        )

        lingtian = (
            _pb_varint(1, 1)
            + _pb_varint(2, 4)
            + _pb_varint(3, (1 << 64) - 2)
            + _pb_varint(4, 72)
            + _pb_varint(5, 31)
            + _pb_varint(6, 18)
            + _pb_varint(7, 20)
            + _pb_string(8, "qingcao")
            + _pb_fixed32(10, 0.35)
            + _pb_varint(11, 1)
        )
        decoded = decode_server_data_payload(_pb_message(31, lingtian))
        self.assertEqual(
            {key: decoded[key] for key in decoded if key != "dye_contamination"},
            {
                "v": 1,
                "type": "lingtian_session",
                "active": True,
                "kind": "harvest",
                "pos": [-2, 72, 31],
                "elapsed_ticks": 18,
                "target_ticks": 20,
                "plant_id": "qingcao",
                "source": None,
                "dye_contamination_warning": True,
            },
        )
        self.assertAlmostEqual(decoded["dye_contamination"], 0.35, places=6)

    def test_optional_numeric_fields_ignore_wrong_wire_type_and_use_last_typed_value(self):
        malformed_lingtian = _pb_varint(10, 1)
        decoded = decode_server_data_payload(_pb_message(31, malformed_lingtian))
        self.assertIsNone(
            decoded["dye_contamination"],
            "同字段号但错误 wire type 不能伪装成 optional float32=0.0",
        )

        malformed_alchemy = (
            _pb_varint(4, 1)
            + _pb_string(5, "not-a-double")
            + _pb_string(6, "not-an-enum")
        )
        decoded = decode_server_data_payload(_pb_message(14, malformed_alchemy))
        self.assertIsNone(
            decoded["quality"],
            "同字段号但错误 wire type 不能伪装成 optional double=0.0",
        )
        self.assertIsNone(decoded["toxin_amount"])
        self.assertIsNone(
            decoded["toxin_color"],
            "同字段号但错误 wire type 不能伪装成 optional enum unspecified",
        )

        mixed_lingtian = _pb_varint(10, 1) + _pb_fixed32(10, 0.25) + _pb_fixed32(10, 0.75)
        decoded = decode_server_data_payload(_pb_message(31, mixed_lingtian))
        self.assertAlmostEqual(
            decoded["dye_contamination"],
            0.75,
            places=6,
            msg="错误 wire 应按 unknown field 忽略，正确 float32 取最后一个 typed value",
        )

        mixed_alchemy = _pb_string(4, "ignored") + _pb_fixed64(4, 0.2) + _pb_fixed64(4, 0.8)
        decoded = decode_server_data_payload(_pb_message(14, mixed_alchemy))
        self.assertAlmostEqual(
            decoded["quality"],
            0.8,
            places=9,
            msg="错误 wire 应按 unknown field 忽略，正确 double 取最后一个 typed value",
        )

    def test_proto_alchemy_outcome_preserves_optional_presence_and_enum_identity(self):
        pill = (
            _pb_varint(1, 3)
            + _pb_string(2, "hui_yuan_pill_v0")
            + _pb_string(3, "hui_yuan_pill")
            + _pb_fixed64(4, 0.4)
            + _pb_fixed64(5, 0.8)
            + _pb_varint(6, 10)
            + _pb_fixed64(7, 18.0)
            + _pb_string(8, "qi_cap_perm_minus_1")
            + _pb_varint(9, 1)
        )
        self.assertEqual(
            decode_server_data_payload(_pb_message(14, pill)),
            {
                "v": 1,
                "type": "alchemy_outcome_resolved",
                "bucket": "flawed",
                "recipe_id": "hui_yuan_pill_v0",
                "pill": "hui_yuan_pill",
                "quality": 0.4,
                "toxin_amount": 0.8,
                "toxin_color": "turbid",
                "qi_gain": 18.0,
                "side_effect_tag": "qi_cap_perm_minus_1",
                "flawed_path": True,
                "damage": None,
                "meridian_crack": None,
            },
        )

        explode = (
            _pb_varint(1, 5)
            + _pb_fixed64(10, 12.0)
            + _pb_fixed64(11, 0.2)
        )
        self.assertEqual(
            decode_server_data_payload(_pb_message(14, explode)),
            {
                "v": 1,
                "type": "alchemy_outcome_resolved",
                "bucket": "explode",
                "recipe_id": None,
                "pill": None,
                "quality": None,
                "toxin_amount": None,
                "toxin_color": None,
                "qi_gain": None,
                "side_effect_tag": None,
                "flawed_path": False,
                "damage": 12.0,
                "meridian_crack": 0.2,
            },
            "absent proto optionals must remain None rather than counterfeit zero/empty values",
        )

    def test_p4_decoder_contract_matches_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        expected_envelope = {
            "botany_harvest_progress": ("BotanyHarvestProgress", 25, "single"),
            "gathering_session": ("GatheringSession", 30, "single"),
            "lingtian_session": ("LingtianSessionData", 31, "single"),
            "alchemy_outcome_resolved": ("AlchemyOutcomeResolved", 14, "single"),
        }
        for field_name, expected in expected_envelope.items():
            with self.subTest(envelope_field=field_name):
                self.assertEqual(_proto_field_metadata(envelope, field_name), expected)

        expected_messages = {
            "BotanyHarvestProgress": {
                "session_id": ("string", 1, "single"), "target_id": ("string", 2, "single"),
                "target_name": ("string", 3, "single"), "plant_kind": ("string", 4, "single"),
                "mode": ("string", 5, "single"), "progress": ("double", 6, "single"),
                "auto_selectable": ("bool", 7, "single"), "request_pending": ("bool", 8, "single"),
                "interrupted": ("bool", 9, "single"), "completed": ("bool", 10, "single"),
                "detail": ("string", 11, "single"), "hazard_hints": ("string", 12, "repeated"),
                "target_pos_x": ("double", 13, "optional"), "target_pos_y": ("double", 14, "optional"),
                "target_pos_z": ("double", 15, "optional"),
            },
            "GatheringSession": {
                "session_id": ("string", 1, "single"), "progress_ticks": ("uint64", 2, "single"),
                "total_ticks": ("uint64", 3, "single"), "target_name": ("string", 4, "single"),
                "target_type": ("GatheringTargetType", 5, "single"),
                "quality_hint": ("GatheringQualityHint", 6, "single"),
                "tool_used": ("string", 7, "optional"), "interrupted": ("bool", 8, "single"),
                "completed": ("bool", 9, "single"),
            },
            "LingtianSessionData": {
                "active": ("bool", 1, "single"), "kind": ("LingtianSessionKind", 2, "single"),
                "pos_x": ("int32", 3, "single"), "pos_y": ("int32", 4, "single"),
                "pos_z": ("int32", 5, "single"), "elapsed_ticks": ("uint32", 6, "single"),
                "target_ticks": ("uint32", 7, "single"), "plant_id": ("string", 8, "optional"),
                "source": ("string", 9, "optional"), "dye_contamination": ("float", 10, "optional"),
                "dye_contamination_warning": ("bool", 11, "single"),
            },
            "AlchemyOutcomeResolved": {
                "bucket": ("AlchemyOutcomeBucket", 1, "single"), "recipe_id": ("string", 2, "optional"),
                "pill": ("string", 3, "optional"), "quality": ("double", 4, "optional"),
                "toxin_amount": ("double", 5, "optional"), "toxin_color": ("ColorKind", 6, "optional"),
                "qi_gain": ("double", 7, "optional"), "side_effect_tag": ("string", 8, "optional"),
                "flawed_path": ("bool", 9, "single"), "damage": ("double", 10, "optional"),
                "meridian_crack": ("double", 11, "optional"),
            },
        }
        for message_name, fields in expected_messages.items():
            body = _proto_message_body(source, message_name)
            self.assertEqual(
                _proto_field_names(body),
                set(fields),
                f"{message_name} 权威字段集与手写 decoder 必须精确相等",
            )
            for field_name, expected in fields.items():
                with self.subTest(message=message_name, field=field_name):
                    self.assertEqual(_proto_field_metadata(body, field_name), expected)

    def test_proto_morph_state_full_payload_decodes(self):
        # plan-race-system-v1 P4 — field 142，mode="full"，一条 active=true entry。
        decoded = decode_server_data_payload(
            _server_data_morph_state_bytes(
                mode="full",
                entity_id=42,
                model_kind=1,
                form_race_id="whale",
                form_body_plan_id="whale",
                active=True,
            )
        )

        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["mode"], "full")
        self.assertEqual(len(decoded["entries"]), 1)
        entry = decoded["entries"][0]
        self.assertEqual(entry["entity_id"], 42)
        self.assertEqual(entry["model_kind"], 1)
        self.assertEqual(entry["form_race_id"], "whale")
        self.assertEqual(entry["form_body_plan_id"], "whale")
        self.assertTrue(entry["active"])

    def test_proto_morph_state_delta_release_payload_decodes(self):
        # mode="delta" + active=false —— 客户端应据此从本地缓存删除该 entity_id。
        decoded = decode_server_data_payload(
            _server_data_morph_state_bytes(
                mode="delta",
                entity_id=42,
                model_kind=0,
                form_race_id="",
                form_body_plan_id="",
                active=False,
            )
        )

        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["mode"], "delta")
        entry = decoded["entries"][0]
        self.assertEqual(entry["entity_id"], 42)
        self.assertFalse(entry["active"])

    def test_proto_morph_state_empty_entries_decodes(self):
        # 未易形 / 无实体时的常态：entries 为空，不应报错或返回 None。
        payload = _pb_message(
            142,
            _pb_varint(1, 1) + _pb_string(2, "full"),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "morph_state")
        self.assertEqual(decoded["entries"], [])

    def test_proto_tribulation_state_payload_name_decodes(self):
        # DONE-W6-HEADLESSAUDIT §5 P0-4 gap：渡虚劫状态 payload（envelope.proto:66）。
        # kind/phase 在 wire 上是 string 字段（envelope.proto:2374-2375），断言解出字面值。
        payload = _pb_message(
            66,
            _pb_varint(1, 1)
            + _pb_string(2, "offline:Alice")
            + _pb_string(3, "Alice")
            + _pb_string(4, "du_xu")
            + _pb_string(5, "omen")
            + _pb_varint(8, 2)
            + _pb_varint(9, 3),
        )
        self.assertEqual(proto_min.server_data_payload_name(payload), "tribulation_state")
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "tribulation_state")
        self.assertTrue(decoded["active"])
        self.assertEqual(decoded["char_id"], "offline:Alice")
        self.assertEqual(decoded["actor_name"], "Alice")
        self.assertEqual(decoded["kind"], "du_xu")
        self.assertEqual(decoded["phase"], "omen")
        self.assertEqual(decoded["wave_current"], 2)
        self.assertEqual(decoded["wave_total"], 3)

    def test_proto_insight_offer_payload_name_decodes(self):
        # DONE-W6-HEADLESSAUDIT §5 P0-4 gap：顿悟邀约 payload（envelope.proto:131）。
        # 断言 offer_id/trigger_id/character_id 字段号→输出键映射 + choices 计数。
        choice = _pb_string(2, "qi_regen_factor") + _pb_string(6, "converge")
        payload = _pb_message(
            131,
            _pb_string(1, "insight:5:first_breakthrough_to_Induce")
            + _pb_string(2, "first_breakthrough_to_Induce")
            + _pb_string(3, "offline:Alice")
            + _pb_message(4, choice)
            + _pb_message(4, choice)
            + _pb_message(4, choice),
        )
        self.assertEqual(proto_min.server_data_payload_name(payload), "insight_offer")
        self.assertIn(b"first_breakthrough_to_Induce", payload)
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "insight_offer")
        self.assertEqual(decoded["offer_id"], "insight:5:first_breakthrough_to_Induce")
        self.assertEqual(decoded["trigger_id"], "first_breakthrough_to_Induce")
        self.assertEqual(decoded["character_id"], "offline:Alice")
        self.assertEqual(len(decoded["choices"]), 3)

    def test_proto_death_screen_visible_payload_decodes(self):
        # field 72 DeathScreen：濒死决策已出（Fortune，stage=1），决策窗口开启。
        # 字段 3/4/5 一并钉死：luck_remaining(double)/final_words(repeated string)/
        # countdown_until_ms(uint64 varint)——与 server 侧 fixture（proto_gen.rs）对齐。
        payload = _pb_message(
            72,
            _pb_varint(1, 1)  # visible=true
            + _pb_string(2, "voluntary_retire")
            + _pb_fixed64(3, 0.3)  # luck_remaining=0.3
            + _pb_string(4, "你的修为到此为止")
            + _pb_string(4, "但愿来生...")  # final_words（repeated）
            + _pb_varint(5, 1700000030000)  # countdown_until_ms
            + _pb_varint(6, 1)  # can_reincarnate=true
            # can_terminate=false（Fortune 决策不可主动终结）
            + _pb_varint(8, 1)  # stage=FORTUNE
            + _pb_varint(9, 1)  # death_number=1
            + _pb_varint(10, 1),  # zone_kind=ORDINARY
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertTrue(decoded["visible"])
        self.assertEqual(decoded["cause"], "voluntary_retire")
        self.assertAlmostEqual(decoded["luck_remaining"], 0.3)
        self.assertEqual(
            decoded["final_words"], ["你的修为到此为止", "但愿来生..."],
            "repeated string 字段 4 应按序解出列表",
        )
        self.assertEqual(decoded["countdown_until_ms"], 1700000030000)
        self.assertTrue(decoded["can_reincarnate"])
        self.assertFalse(decoded["can_terminate"])
        self.assertEqual(decoded["stage"], 1)
        self.assertEqual(decoded["death_number"], 1)
        self.assertEqual(decoded["zone_kind"], 1)

    def test_proto_death_screen_tribulation_payload_decodes(self):
        # Tribulation（stage=2）决策：can_terminate=true。
        payload = _pb_message(
            72,
            _pb_varint(1, 1)
            + _pb_fixed64(3, 0.85)  # luck_remaining
            + _pb_string(4, "大限将至")  # final_words（repeated，单条）
            + _pb_varint(5, 1700000050000)  # countdown_until_ms
            + _pb_varint(6, 1)
            + _pb_varint(7, 1)
            + _pb_varint(8, 2)
            + _pb_varint(9, 4),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertTrue(decoded["visible"])
        self.assertAlmostEqual(decoded["luck_remaining"], 0.85)
        self.assertEqual(decoded["final_words"], ["大限将至"])
        self.assertEqual(decoded["countdown_until_ms"], 1700000050000)
        self.assertTrue(decoded["can_terminate"])
        self.assertEqual(decoded["stage"], 2)
        self.assertEqual(decoded["death_number"], 4)

    def test_proto_death_screen_hidden_payload_decodes(self):
        # 复活/终结后的收屏：visible=false，无 stage/death_number（optional 未填）。
        payload = _pb_message(72, _pb_varint(1, 0))
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "death_screen")
        self.assertFalse(decoded["visible"])
        self.assertNotIn("stage", decoded)
        self.assertNotIn("death_number", decoded)

    def test_proto_terminate_screen_visible_payload_decodes(self):
        # field 73 TerminateScreen：主动归隐终结后的终结屏。
        payload = _pb_message(
            73,
            _pb_varint(1, 1)  # visible=true
            + _pb_string(2, "此身止于此。")
            + _pb_string(3, "你选择了归隐与终结。")
            + _pb_string(4, "凡人"),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "terminate_screen")
        self.assertTrue(decoded["visible"])
        self.assertEqual(decoded["final_words"], "此身止于此。")
        self.assertEqual(decoded["epilogue"], "你选择了归隐与终结。")
        self.assertEqual(decoded["archetype_suggestion"], "凡人")

    def test_proto_terminate_screen_hidden_payload_decodes(self):
        # 新建角色后的收屏：visible=false。
        payload = _pb_message(73, _pb_varint(1, 0))
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "terminate_screen")
        self.assertFalse(decoded["visible"])
    def test_proto_tribulation_state_payload_decodes(self):
        # 全部新增字段都用非默认值编码（failed=true、half_step_on_success=true、
        # world_x/z、三个 tick），逐个断言 wire 号：解码器缺席/串号/恒 false 都会被抓。
        decoded = decode_server_data_payload(
            _server_data_tribulation_state_bytes(
                phase="wave",
                wave_current=3,
                wave_total=5,
                result=None,
                failed=True,
                half_step_on_success=True,
            )
        )

        self.assertEqual(decoded["type"], "tribulation_state")
        self.assertEqual(decoded["phase"], "wave")
        self.assertEqual(decoded["wave_current"], 3)
        self.assertEqual(decoded["wave_total"], 5)
        self.assertEqual(decoded["kind"], "du_xu")
        self.assertEqual(decoded["actor_name"], "BHDE6Cult")
        self.assertEqual(decoded["world_x"], 12.5)
        self.assertEqual(decoded["world_z"], -34.25)
        self.assertEqual(decoded["started_tick"], 1000)
        self.assertEqual(decoded["phase_started_tick"], 5000)
        self.assertEqual(decoded["next_wave_tick"], 5300)
        self.assertIn("du_xu:char-1", decoded["participants"])
        self.assertTrue(decoded["failed"])
        self.assertTrue(decoded["half_step_on_success"])
        self.assertIsNone(decoded["result"])

    def test_proto_tribulation_state_settle_payload_decodes(self):
        # 结算 payload：active=false + result=outcome label（ascended/failed/killed）
        decoded = decode_server_data_payload(
            _server_data_tribulation_state_bytes(
                phase="settle",
                wave_current=5,
                wave_total=5,
                result="ascended",
                active=False,
            )
        )

        self.assertEqual(decoded["type"], "tribulation_state")
        self.assertFalse(decoded["active"])
        self.assertEqual(decoded["result"], "ascended")
        self.assertEqual(decoded["wave_current"], 5)

    def test_proto_tribulation_state_asymmetric_failed_only(self):
        # review finding major-7：failed=true / half_step_on_success=false 必须落各自独
        # 立 wire 号——字段 13/14 互换的解码器在此撞红。
        decoded = decode_server_data_payload(
            _server_data_tribulation_state_bytes(
                phase="wave",
                wave_current=3,
                wave_total=5,
                result=None,
                failed=True,
                half_step_on_success=False,
            )
        )
        self.assertTrue(decoded["failed"])
        self.assertFalse(decoded["half_step_on_success"])

    def test_proto_tribulation_state_asymmetric_half_step_only(self):
        # reverse 组合：failed=false / half_step_on_success=true，把两字段双向锁死。
        decoded = decode_server_data_payload(
            _server_data_tribulation_state_bytes(
                phase="wave",
                wave_current=3,
                wave_total=5,
                result=None,
                failed=False,
                half_step_on_success=True,
            )
        )
        self.assertFalse(decoded["failed"])
        self.assertTrue(decoded["half_step_on_success"])

    def test_proto_tribulation_state_ignores_wrong_wire_participant(self):
        # review finding minor：participants 是 repeated string（wire 2）。误编码的 fixed64
        # (wire 1) field 15 必须被忽略，不得解成参与者文本。
        payload = _pb_message(
            66,
            _pb_string(5, "wave") + _pb_fixed64(15, 3.25),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "tribulation_state")
        self.assertEqual(decoded["participants"], [])

    def test_proto_heart_demon_offer_ignores_wrong_wire_choice(self):
        # review finding minor：choices 是 repeated message（wire 2）。误编码的 fixed64
        # (wire 1) field 9 必须被忽略（不得喂给嵌套解析器抛 ProtoDecodeError、丢观测）；
        # 合法 wire 2 choice 仍正常解码。
        choice = _pb_string(1, "heart_demon_choice_0") + _pb_string(2, "Composure")
        payload = _pb_message(
            69,
            _pb_string(3, "心魔劫临身") + _pb_fixed64(9, 1.5) + _pb_message(9, choice),
        )
        decoded = decode_server_data_payload(payload)
        self.assertEqual(decoded["type"], "heart_demon_offer")
        self.assertEqual(decoded["choices"], [{"choice_id": "heart_demon_choice_0", "category": "Composure", "title": "", "effect_summary": "", "flavor": "", "style_hint": ""}])

    def test_proto_heart_demon_offer_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_heart_demon_offer_bytes())

        self.assertEqual(decoded["type"], "heart_demon_offer")
        # 非对称 fixture 全字段精确断言（review finding major-7）：offer_id != trigger_id、
        # 配额不等、每个标量/嵌套字段独立 pin 到 wire 号。
        self.assertEqual(decoded["offer_id"], "heart_demon:7:1000")
        self.assertEqual(decoded["trigger_id"], "heart_demon:7:1111")
        self.assertNotEqual(decoded["offer_id"], decoded["trigger_id"])
        self.assertEqual(decoded["trigger_label"], "心魔劫临身")
        self.assertEqual(decoded["realm_label"], "渡虚劫 · 心魔")
        self.assertEqual(decoded["composure"], 0.85)
        self.assertEqual(decoded["quota_remaining"], 2)
        self.assertEqual(decoded["quota_total"], 3)
        self.assertNotEqual(decoded["quota_remaining"], decoded["quota_total"])
        self.assertEqual(decoded["expires_at_ms"], 1700000050000)
        self.assertEqual(
            [(c["choice_id"], c["category"], c["title"]) for c in decoded["choices"]],
            [
                ("heart_demon_choice_0", "Composure", "守本心"),
                ("heart_demon_choice_1", "Breakthrough", "斩执念"),
                ("heart_demon_choice_2", "Perception", "无解"),
            ],
        )
        self.assertEqual(
            [c["effect_summary"] for c in decoded["choices"]],
            [
                "稳住心神，回复少量当前真元",
                "若斩错心魔，将损当前真元并强化下一道开天雷",
                "承认无解，不得增益也不受真元惩罚",
            ],
        )
        self.assertEqual(
            [c["flavor"] for c in decoded["choices"]],
            ["心若磐石，外物不侵。", "抽刀断水，一念斩魔。", "心魔无解，人间一二。"],
        )
        self.assertEqual(
            [c["style_hint"] for c in decoded["choices"]],
            ["calm-blue", "edge-orange", "calm-grey"],
        )
        # 每个 choice 的嵌套字段必须各自可辨（不是同一串复读）。
        self.assertEqual(len({c["effect_summary"] for c in decoded["choices"]}), 3)
        self.assertEqual(len({c["flavor"] for c in decoded["choices"]}), 3)
        self.assertEqual(len({c["style_hint"] for c in decoded["choices"]}), 3)

    def test_bot_dispatch_emits_decoded_tribulation_state_event(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_CUSTOM_PAYLOAD)
            + mc.mc_string("bong:server_data")
            + _server_data_tribulation_state_bytes(
                phase="heart_demon",
                wave_current=4,
                wave_total=5,
                result=None,
            )
        )
        bot._dispatch(body)
        decoded_events = bot.events_of("server_data")
        self.assertEqual(len(decoded_events), 1)
        self.assertEqual(decoded_events[0].data["payload_type"], "tribulation_state")
        self.assertEqual(decoded_events[0].data["payload"]["phase"], "heart_demon")

    def test_proto_carrier_state_decodes_wire_id(self):
        # field 49 CarrierState —— carrier 是本 bot 的线缆 wire id（player:{uuid}）。
        # 场景靠它把 bong:anqi/container_swap 事件归属到本 bot（findings 修复）。
        decoded = decode_server_data_payload(
            _server_data_carrier_state_bytes(
                carrier="player:3f9a2c8e-4a1e-4b1f-9c2d-0a1b2c3d4e5f",
                phase=3,
                progress=0.5,
                sealed_qi=30.0,
                sealed_qi_initial=60.0,
                half_life_remaining_ticks=120,
                item_instance_id=9,
            )
        )

        self.assertEqual(decoded["type"], "carrier_state")
        self.assertEqual(decoded["carrier"], "player:3f9a2c8e-4a1e-4b1f-9c2d-0a1b2c3d4e5f")
        self.assertEqual(decoded["phase"], "charged")
        self.assertAlmostEqual(decoded["progress"], 0.5)
        self.assertAlmostEqual(decoded["sealed_qi"], 30.0)
        self.assertAlmostEqual(decoded["sealed_qi_initial"], 60.0)
        self.assertEqual(decoded["half_life_remaining_ticks"], 120)
        self.assertEqual(decoded["item_instance_id"], 9)

    def test_proto_carrier_state_decodes_every_charge_phase(self):
        # CARRIER_CHARGE_PHASE_NAMES 是本次引入的完整 wire→domain 契约：每个 phase
        # 值都必须解出正确名称，未知值回退 unspecified（findings：原测试只覆盖 phase=2，
        # idle/charging 映射错或未知值不回退都测不出来）。
        cases = {
            0: "unspecified",
            1: "idle",
            2: "charging",
            3: "charged",
            9: "unspecified",
        }
        for phase, expected in cases.items():
            with self.subTest(phase=phase):
                decoded = decode_server_data_payload(
                    _server_data_carrier_state_bytes(
                        carrier="player:3f9a2c8e-4a1e-4b1f-9c2d-0a1b2c3d4e5f",
                        phase=phase,
                        progress=0.5,
                        sealed_qi=30.0,
                        sealed_qi_initial=60.0,
                        half_life_remaining_ticks=120,
                        item_instance_id=None,
                    )
                )
                self.assertEqual(decoded["type"], "carrier_state")
                self.assertEqual(decoded["phase"], expected)
                # 构造边界本来就是 item_instance_id=None（field 7 缺省）；缺省的可选
                # protobuf 字段必须解成 None，而不是 0 / 残留旧值（review finding
                # [major]：absent item_instance_id is encoded but never asserted——
                # 错把缺省当 0 会让消费者把无实例快照误读为 instance 0）。
                self.assertIsNone(
                    decoded["item_instance_id"],
                    f"phase={phase} 时 field 7 缺省应解出 None，实际 {decoded['item_instance_id']!r}",
                )


class CombatServerDataGateTest(unittest.TestCase):
    def _event(self, t: float, payload_type: str, payload: dict) -> _FakeEvent:
        return _FakeEvent(
            t,
            "server_data",
            {"payload_type": payload_type, "payload": payload},
        )

    def test_wait_ignores_raw_heartbeat_unknown_and_malformed_payloads(self):
        combat = self._event(
            6.0,
            "combat_event",
            {
                "type": "combat_event",
                "events": [{"kind": "hit", "amount": 2.0, "outgoing": True}],
            },
        )
        bot = _FakeBot(
            [
                _FakeEvent(2.0, "payload", {"channel": "bong:server_data", "data": b"\x12\x00"}),
                _FakeEvent(3.0, "server_data", {"payload_type": "heartbeat", "payload": {"type": "heartbeat"}}),
                _FakeEvent(4.0, "server_data_decode_error", {"error": "truncated"}),
                _FakeEvent(5.0, "server_data", {"payload_type": "field_999", "payload": {"type": "field_999"}}),
                combat,
            ]
        )

        matched = wait_for_server_data_after(
            bot,
            anchor=1.0,
            expected_types={"combat_event"},
            timeout=0.1,
            description="strict combat event",
        )

        self.assertIs(matched, combat)

    def test_wait_ignores_matching_type_before_watermark(self):
        old = self._event(1.0, "combat_event", {"type": "combat_event", "events": []})
        new = self._event(3.0, "combat_event", {"type": "combat_event", "events": []})
        matched = wait_for_server_data_after(
            _FakeBot([old, new]),
            anchor=2.0,
            expected_types={"combat_event"},
            timeout=0.1,
            description="after-watermark combat event",
        )
        self.assertIs(matched, new)

    def test_outgoing_positive_hit_rejects_incoming_non_hit_and_zero(self):
        rejected = (
            self._event(1.0, "heartbeat", {"type": "heartbeat"}),
            self._event(
                2.0,
                "combat_event",
                {"events": [{"kind": "hit", "amount": 3.0, "outgoing": False}]},
            ),
            self._event(
                3.0,
                "combat_event",
                {"events": [{"kind": "heal", "amount": 3.0, "outgoing": True}]},
            ),
            self._event(
                4.0,
                "combat_event",
                {"events": [{"kind": "hit", "amount": 0.0, "outgoing": True}]},
            ),
            self._event(
                5.0,
                "combat_event",
                {"events": [{"kind": "hit", "amount": -1.0, "outgoing": True}]},
            ),
        )
        self.assertTrue(all(not is_outgoing_positive_hit(event) for event in rejected))

        accepted = self._event(
            6.0,
            "combat_event",
            {"events": [{"kind": "hit", "amount": 0.001, "outgoing": True}]},
        )
        self.assertTrue(is_outgoing_positive_hit(accepted))

def _server_data_skill_xp_gain_bytes() -> bytes:
    scroll = _pb_string(1, "skill_scroll_herbalism_baicao_can") + _pb_varint(2, 500)
    payload = (
        _pb_varint(1, 42)
        + _pb_varint(2, 1)  # SkillId HERBALISM
        + _pb_varint(3, 500)
        + _pb_message(5, scroll)  # XpGainSourceScroll
    )
    return _pb_message(7, payload)


def _server_data_quickslot_config_bytes() -> bytes:
    entry = (
        _pb_string(1, "guyuan_pill")
        + _pb_string(2, "固元丹")
        + _pb_varint(3, 1500)
        + _pb_varint(4, 3000)
        + _pb_string(5, "bong-client:textures/gui/items/pill.png")
    )
    bound_slot = _pb_message(1, entry)
    empty_slot = b""
    # review finding [5]：proto3 `repeated uint64` 默认 **packed**（wire type 2，
    # length-delimited blob 内连续 varint）。旧 builder 用 9 个独立 wire-0 varint
    # 构造、解码器只读 w==0，真实服务器生产的 packed 载荷被解成空列表——用 packed
    # 编码 + 非零时间戳钉死解码器必须走 packed 路径且逐值还原。
    packed_cooldowns = _pb_bytes(
        2,
        _pb_raw_varint(1)
        + _pb_raw_varint(2)
        + _pb_raw_varint(3)
        + _pb_raw_varint(4)
        + _pb_raw_varint(5)
        + _pb_raw_varint(6)
        + _pb_raw_varint(7)
        + _pb_raw_varint(8)
        + _pb_raw_varint(9),
    )
    payload = (
        _pb_message(1, bound_slot)
        + _pb_message(1, empty_slot) * 8
        + packed_cooldowns
        + _pb_string(3, "gap10-bind-1")
        + _pb_varint(4, 1)
    )
    return _pb_message(35, payload)


def _server_data_techniques_snapshot_bytes() -> bytes:
    meridian = _pb_string(1, "Lung") + _pb_fixed32(2, 0.5)
    # central-review 31438252846 finding [5]：proficiency(field 4) 与 qi_cost(field 10)
    # 取**非零互异**值——旧 fixture 两者都是 0.0，解码器完全忽略这两个 wire 字段、
    # 返回默认 0.0 也能通过断言。非零值 + 精确断言才能区分「正确解码」与「丢弃字段」。
    entry = (
        _pb_string(1, "sword.cleave")
        + _pb_string(2, "劈")
        + _pb_string(3, "common")
        + _pb_fixed32(4, 0.5)  # proficiency
        + _pb_string(5, "生疏")
        + _pb_varint(6, 1)
        + _pb_string(7, "基础劈砍。举剑过顶，顺势劈下。")
        + _pb_string(8, "Awaken")
        + _pb_message(9, meridian)
        + _pb_fixed32(10, 2.5)  # qi_cost
        + _pb_fixed32(11, 8.0)
        + _pb_varint(12, 16)
        + _pb_varint(13, 30)
        + _pb_fixed32(14, 3.0)
    )
    return _pb_message(37, _pb_message(1, entry))


def _server_data_skill_config_snapshot_bytes() -> bytes:
    entry = _pb_string(1, "zhenmai.sever_chain") + _pb_string(
        2, '{"backfire_kind":"tainted_yuan","meridian_id":"Pericardium"}'
    )
    return _pb_message(54, _pb_message(1, entry))


def _server_data_skill_scroll_used_bytes() -> bytes:
    payload = (
        _pb_varint(1, 42)
        + _pb_string(2, "skill_scroll_herbalism_baicao_can")
        + _pb_varint(3, 1)  # SkillId HERBALISM
        + _pb_varint(4, 500)
        + _pb_varint(5, 0)
    )
    return _pb_message(97, payload)


def _server_data_skill_snapshot_bytes() -> bytes:
    # central-review 31438252846 finding [4] + 31442475206 finding [9]：六个嵌套字段
    # 全部取**互异**非默认值——旧 fixture 用 500 同时填 xp 与 recent_gain_xp，解码器
    # 交换 field 2/field 6 在断言下仍观测相同；互异值才能暴露 field swap 与 drop。
    entry_snap = (
        _pb_varint(1, 3)      # lv
        + _pb_varint(2, 500)  # xp
        + _pb_varint(3, 1600) # xp_to_next
        + _pb_varint(4, 1440) # total_xp
        + _pb_varint(5, 30)   # cap
        + _pb_varint(6, 700)  # recent_gain_xp
    )
    snap_entry = _pb_string(1, "herbalism") + _pb_message(2, entry_snap)
    payload = (
        _pb_varint(1, 42)
        + _pb_message(2, snap_entry)
        + _pb_string(3, "skill_scroll_herbalism_baicao_can")
    )
    return _pb_message(98, payload)


class ServerDataSkillScrollDecodeTest(unittest.TestCase):
    def test_proto_skill_xp_gain_scroll_source_decodes(self):
        decoded = decode_server_data_payload(_server_data_skill_xp_gain_bytes())

        self.assertEqual(decoded["type"], "skill_xp_gain")
        self.assertEqual(decoded["char_id"], 42)
        self.assertEqual(decoded["skill"], "herbalism")
        self.assertEqual(decoded["amount"], 500)
        self.assertEqual(
            decoded["source"],
            {"kind": "scroll", "scroll_id": "skill_scroll_herbalism_baicao_can", "xp_grant": 500},
            "learn_skill_scroll 的 XP 来源必须是 scroll 且带 scroll_id，场景才可断言来源",
        )

    def test_proto_quick_slot_config_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_quickslot_config_bytes())

        self.assertEqual(decoded["type"], "quickslot_config")
        self.assertEqual(len(decoded["slots"]), 9, "固定 9 槽")
        bound = decoded["slots"][0]
        # central-review 31442475206 finding [5]：fixture 供应完整 5 字段绑定条目
        # （item_id/display_name/cast_duration_ms/cooldown_ms/icon_texture），旧测试
        # 只断言 item_id + cast_duration_ms——解码器丢弃字段 2/4/5 或返回默认值也能
        # 通过。逐字段 pin 全部 5 个可观测槽元数据。
        self.assertEqual(bound["item_id"], "guyuan_pill")
        self.assertEqual(bound["display_name"], "固元丹")
        self.assertEqual(bound["cast_duration_ms"], 1500)
        self.assertEqual(bound["cooldown_ms"], 3000)
        self.assertEqual(bound["icon_texture"], "bong-client:textures/gui/items/pill.png")
        self.assertIsNone(decoded["slots"][1], "未绑定槽应为 None")
        self.assertEqual(
            decoded["cooldown_until_ms"],
            [1, 2, 3, 4, 5, 6, 7, 8, 9],
            "packed repeated uint64 必须逐值还原（review finding [5]：旧解码器把 "
            "wire-2 packed 载荷解成空列表）",
        )
        self.assertEqual(decoded["ack_request_id"], "gap10-bind-1")
        self.assertIs(decoded["bind_accepted"], True)

    def test_proto_techniques_snapshot_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_techniques_snapshot_bytes())

        self.assertEqual(decoded["type"], "techniques_snapshot")
        self.assertEqual(len(decoded["entries"]), 1)
        entry = decoded["entries"][0]
        self.assertEqual(entry["id"], "sword.cleave")
        self.assertEqual(entry["display_name"], "劈")
        self.assertEqual(entry["required_realm"], "Awaken")
        self.assertEqual(entry["required_meridians"], [{"channel": "Lung", "min_health": 0.5}])
        self.assertEqual(entry["cast_ticks"], 16)
        self.assertEqual(entry["cooldown_ticks"], 30)
        # central-review 2012 #6 回归：fixture 供应了 grade/proficiency/
        # proficiency_label/active/description/qi_cost/stamina_cost/range 全部分解码
        # 字段，但旧测试只断言其中一部分——解码器对这些字段返回零值/丢弃也能通过。
        # 这些是新支持快照载荷的可观测字段，逐个 pin 完整解码契约。
        self.assertEqual(entry["grade"], "common")
        # central-review 31438252846 finding [5]：fixture 现以非零互异值编码
        # proficiency(0.5) 与 qi_cost(2.5)，精确断言其解码值——忽略任一 wire 字段的
        # 解码器会返回 0.0 并被此处抓红。
        self.assertEqual(entry["proficiency"], 0.5)
        self.assertEqual(entry["proficiency_label"], "生疏")
        self.assertIs(entry["active"], True)
        self.assertEqual(entry["description"], "基础劈砍。举剑过顶，顺势劈下。")
        self.assertEqual(entry["qi_cost"], 2.5)
        self.assertEqual(entry["stamina_cost"], 8.0)
        self.assertEqual(entry["range"], 3.0)

    def test_proto_skill_config_snapshot_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_skill_config_snapshot_bytes())

        self.assertEqual(decoded["type"], "skill_config_snapshot")
        self.assertEqual(len(decoded["configs"]), 1)
        entry = decoded["configs"][0]
        self.assertEqual(entry["skill_id"], "zhenmai.sever_chain")
        self.assertIn('"meridian_id":"Pericardium"', entry["json_config"])

    def test_proto_skill_scroll_used_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_skill_scroll_used_bytes())

        self.assertEqual(decoded["type"], "skill_scroll_used")
        self.assertEqual(decoded["char_id"], 42)
        self.assertEqual(decoded["scroll_id"], "skill_scroll_herbalism_baicao_can")
        self.assertEqual(decoded["skill"], "herbalism")
        self.assertEqual(decoded["xp_granted"], 500)
        self.assertIs(decoded["was_duplicate"], False)

    def test_proto_skill_snapshot_payload_decodes(self):
        decoded = decode_server_data_payload(_server_data_skill_snapshot_bytes())

        self.assertEqual(decoded["type"], "skill_snapshot")
        self.assertEqual(decoded["char_id"], 42)
        self.assertEqual(len(decoded["skills"]), 1)
        entry = decoded["skills"][0]
        self.assertEqual(entry["skill_name"], "herbalism")
        # central-review 31438252846 finding [4] + 31442475206 finding [9]：fixture
        # 供应全部六个嵌套字段且互异（xp=500 ≠ recent_gain_xp=700），旧 fixture 用
        # 500 同时填两个字段——交换 field 2/field 6 的解码器也观测相同。逐字段 pin
        # 完整解码契约（互异值暴露 field swap）。
        self.assertEqual(entry["entry"]["lv"], 3)
        self.assertEqual(entry["entry"]["xp"], 500)
        self.assertEqual(entry["entry"]["xp_to_next"], 1600)
        self.assertEqual(entry["entry"]["total_xp"], 1440)
        self.assertEqual(entry["entry"]["cap"], 30)
        self.assertEqual(entry["entry"]["recent_gain_xp"], 700)
        self.assertEqual(decoded["consumed_scrolls"], ["skill_scroll_herbalism_baicao_can"])

    def test_bot_dispatch_emits_quickslot_config_event(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_CUSTOM_PAYLOAD)
            + mc.mc_string("bong:server_data")
            + _server_data_quickslot_config_bytes()
        )

        bot._dispatch(body)

        decoded_events = bot.events_of("server_data")
        self.assertEqual(len(decoded_events), 1)
        self.assertEqual(decoded_events[0].data["payload_type"], "quickslot_config")
        self.assertEqual(
            decoded_events[0].data["payload"]["ack_request_id"],
            "gap10-bind-1",
            "真实 Bot reader 必须把 production protobuf quickslot_config 暴露为可断言事件",
        )


class InventoryHelperTest(unittest.TestCase):
    def test_inventory_signature_retains_complete_item_metadata(self):
        baseline = {
            "containers": [{"id": "body_pocket", "rows": 2, "cols": 3}],
            "placed_items": [
                {
                    "container_id": "body_pocket",
                    "row": 0,
                    "col": 1,
                    "item": {
                        "instance_id": 7,
                        "item_id": "guyuan_pill",
                        "durability": 0.61,
                        "freshness": {"current_qi": 0.4},
                        "forge_quality": 0.8,
                    },
                }
            ],
            "equipped": {"chest_worn": []},
            "hotbar": [None] * 9,
            "bone_coins": 12,
        }
        changed = json.loads(json.dumps(baseline))
        changed["placed_items"][0]["item"]["durability"] = 0.60
        self.assertNotEqual(
            inventory_signature(baseline),
            inventory_signature(changed),
            "拒绝路径签名必须捕获 durability/NBT-like metadata 的细微篡改",
        )

    def test_latest_inventory_snapshot_uses_newest_history(self):
        bot = _FakeBot(
            [
                _snapshot_event(1.0, 1, "old_pack"),
                _snapshot_event(2.0, 2, "new_pack"),
            ]
        )

        snapshot = latest_inventory_snapshot(bot)

        self.assertEqual(snapshot["revision"], 2)
        self.assertEqual(snapshot["marker"], "new_pack")

    def test_wait_inventory_snapshot_after_ignores_old_history(self):
        bot = _FakeBot(
            [
                _snapshot_event(1.0, 1, "old_pack"),
                _snapshot_event(3.0, 2, "after_unequip"),
            ]
        )

        snapshot = wait_inventory_snapshot_after(bot, after_t=2.0)

        self.assertEqual(snapshot["revision"], 2)
        self.assertEqual(snapshot["marker"], "after_unequip")

    def test_inventory_move_watermarks_skip_stow_snapshot_before_unequip(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 2, "after_give"),
                _snapshot_event(3.0, 3, "after_stow"),
                _snapshot_event(4.0, 4, "after_unequip"),
            ]
        )

        after_give_revision = 2
        stow_snapshot = wait_inventory_revision_after(bot, after_give_revision)
        unequip_snapshot = wait_inventory_revision_after(bot, stow_snapshot["revision"])

        self.assertEqual(stow_snapshot["revision"], 3)
        self.assertEqual(stow_snapshot["marker"], "after_stow")
        self.assertEqual(unequip_snapshot["revision"], 4)
        self.assertEqual(unequip_snapshot["marker"], "after_unequip")

    def test_wait_inventory_revision_after_accepts_skipped_revision(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 3, "skipped_revision"),
            ]
        )

        snapshot = wait_inventory_revision_after(bot, 1, timeout=0.01)

        self.assertEqual(snapshot["revision"], 3)
        self.assertEqual(snapshot["marker"], "skipped_revision")

    def test_wait_inventory_revision_after_matching_accepts_exact_revision(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 2, "command_final"),
                _snapshot_event(3.0, 3, "later_unrelated"),
            ]
        )

        snapshot = wait_inventory_revision_after_matching(
            bot,
            1,
            lambda payload: payload["marker"] == "command_final",
            "command_final marker",
        )

        self.assertEqual(snapshot["revision"], 2)
        self.assertEqual(snapshot["marker"], "command_final")

    def test_wait_inventory_revision_after_matching_accepts_matching_later_revision(self):
        bot = _FakeBot(
            [
                _snapshot_event(2.0, 2, "command_intermediate"),
                _snapshot_event(3.0, 3, "command_final"),
            ]
        )

        snapshot = wait_inventory_revision_after_matching(
            bot,
            1,
            lambda payload: payload["marker"] == "command_final",
            "command_final marker",
        )

        self.assertEqual(snapshot["revision"], 3)
        self.assertEqual(snapshot["marker"], "command_final")

    def test_give_revision_barrier_ignores_prior_mutation_snapshot(self):
        stale_receipt = _FakeEvent(
            1.5,
            "chat",
            {"text": "[dev] gave grass_fiber x1 revision=3"},
        )
        prior_request = _snapshot_event(3.0, 4, "prior_request")
        barrier_receipt = _FakeEvent(
            4.0,
            "chat",
            {"text": "[dev] gave grass_fiber x1 revision=5"},
        )
        barrier_snapshot = _snapshot_event(4.1, 5, "barrier")
        bot = _CommandFakeBot(
            [_snapshot_event(2.0, 3, "baseline"), stale_receipt],
            [prior_request, barrier_receipt, barrier_snapshot],
        )

        snapshot = give_inventory_revision_barrier(bot, "grass_fiber", timeout=0.01)

        self.assertEqual(bot.commands, ["give grass_fiber 1"])
        self.assertEqual(snapshot["revision"], 5)
        self.assertEqual(snapshot["marker"], "barrier")

    def test_give_revision_barrier_accepts_snapshot_after_exact_revision(self):
        bot = _CommandFakeBot(
            [_snapshot_event(1.0, 7, "baseline")],
            [
                _FakeEvent(
                    2.0,
                    "chat",
                    {"text": "[dev] gave grass_fiber x1 revision=8"},
                ),
                _snapshot_event(2.1, 9, "coalesced_later_revision"),
            ],
        )

        snapshot = give_inventory_revision_barrier(bot, "grass_fiber", timeout=0.01)

        self.assertEqual(snapshot["revision"], 9)
        self.assertEqual(snapshot["marker"], "coalesced_later_revision")


class ProductionScenarioContractTest(unittest.TestCase):
    @staticmethod
    def _worn_item(instance_id: int, item_id: str) -> dict:
        return {
            "instance_id": instance_id,
            "item_id": item_id,
            "grid_width": 1,
            "grid_height": 1,
        }

    @classmethod
    def _pack_snapshot(cls, worn_ids: list[int]) -> dict:
        names = {10: "worn_grass_pouch", 20: "fake_spirit_hide", 30: "cloth_wrap"}
        return {
            "revision": 1,
            "containers": [{"id": "pack_10", "rows": 2, "cols": 2}],
            "placed_items": [],
            "equipped": {
                "chest_worn": [
                    cls._worn_item(instance_id, names[instance_id])
                    for instance_id in worn_ids
                ]
            },
            "hotbar": [],
        }

    def test_lingtian_surface_candidates_probe_three_support_depths(self):
        bot = types.SimpleNamespace(position=(-0.2, 74.9, 3.8))

        candidates = _surface_candidates(bot)

        self.assertEqual(candidates[:3], [(-1, 73, 3), (-1, 72, 3), (-1, 71, 3)])
        self.assertEqual(len(candidates), 39)
        self.assertEqual(
            len(set(candidates)),
            39,
            "13 个水平点各自向下三层时不应重复，否则会浪费真实 intent 等待窗口",
        )

    def test_spiritwood_terminal_timeout_covers_240_ticks_at_two_tps(self):
        minimum_runtime = 240 / 2
        self.assertGreaterEqual(
            LUMBER_TERMINAL_TIMEOUT_SECONDS,
            minimum_runtime + 30,
            "全量 Bot gate 实测会降至 2 TPS；terminal timeout 必须覆盖 240 tick 加 stall 余量",
        )

    def test_craft_progress_timeout_covers_worst_global_emit_phase(self):
        interval = 20
        required_elapsed = 20
        worst_elapsed = max(
            next(
                elapsed
                for elapsed in range(required_elapsed, required_elapsed + interval)
                if (start_phase + elapsed) % interval == 0
            )
            for start_phase in range(interval)
        )

        self.assertEqual(
            worst_elapsed,
            39,
            "全局 20-tick emit 边界最坏要到 session 第 39 tick 才能观察 elapsed>=20",
        )
        self.assertGreaterEqual(
            CRAFT_PROGRESS_OBSERVATION_TIMEOUT_SECONDS,
            40 / 2 + 5,
            "全量 gate 的 2 TPS 下必须覆盖两段 emit cadence 并留 packet/I/O 余量",
        )

    def test_uncover_pack_moves_every_lifo_layer_in_authoritative_order(self):
        class InventoryMoveBot:
            username = "Fake"

            def __init__(self, snapshot: dict):
                self.snapshot = snapshot
                self.events = []
                self.intents = []

            def intent(self, payload: dict) -> None:
                self.intents.append(payload)
                candidate = json.loads(json.dumps(self.snapshot))
                worn = candidate["equipped"]["chest_worn"]
                moved = worn.pop()
                assert moved["instance_id"] == payload["instance_id"]
                destination = payload["to"]
                candidate["placed_items"].append(
                    {
                        "container_id": destination["container_id"],
                        "row": destination["row"],
                        "col": destination["col"],
                        "item": moved,
                    }
                )
                candidate["revision"] += 1
                self.snapshot = candidate
                self.events.append(
                    _FakeEvent(
                        float(candidate["revision"]),
                        "server_data",
                        {
                            "payload_type": "inventory_snapshot",
                            "payload": candidate,
                        },
                    )
                )

            def wait_for(self, predicate, timeout: float, description: str):
                for event in self.events:
                    if predicate(event):
                        return event
                raise AssertionError(f"未找到 {description}; events={self.events}")

        initial = self._pack_snapshot([10, 20, 30])
        bot = InventoryMoveBot(initial)

        final = _uncover_pack(bot, initial, 10, "pack_10")

        self.assertEqual([intent["instance_id"] for intent in bot.intents], [30, 20])
        self.assertEqual(
            [(intent["to"]["row"], intent["to"]["col"]) for intent in bot.intents],
            [(0, 0), (0, 1)],
        )
        self.assertEqual(
            [item["instance_id"] for item in final["equipped"]["chest_worn"]],
            [10],
        )
        self.assertEqual(
            sorted(item["item"]["instance_id"] for item in final["placed_items"]),
            [20, 30],
        )
        self.assertEqual(final["revision"], 3)

    def test_uncover_pack_is_noop_when_pack_is_already_top(self):
        snapshot = self._pack_snapshot([20, 10])
        bot = types.SimpleNamespace()

        self.assertIs(_uncover_pack(bot, snapshot, 10, "pack_10"), snapshot)

    def test_uncover_pack_rejects_snapshot_without_target_instance(self):
        snapshot = self._pack_snapshot([20, 30])
        with self.assertRaisesRegex(BotAssertionError, "找不到 pack instance=10"):
            _uncover_pack(types.SimpleNamespace(), snapshot, 10, "pack_10")


class NovicePoiScenarioParsingTest(unittest.TestCase):
    def test_selection_strategy_requires_exact_token_not_known_prefix(self):
        relaxed = "relaxed_radius_2000"
        qi_margin = "relaxed_radius_2000_qi_margin_0_1"

        self.assertEqual(
            _selection_strategy(f"[dev] novice_poi mutant_nest pos=1,2,3 selection={relaxed}"),
            relaxed,
        )
        self.assertEqual(
            _selection_strategy(
                f"[dev] novice_poi spirit_herb_valley pos=1,2,3 selection={qi_margin}"
            ),
            qi_margin,
        )
        self.assertNotEqual(_selection_strategy(f"selection={qi_margin}"), relaxed)
        self.assertNotEqual(_selection_strategy(f"selection={relaxed}"), qi_margin)

    def test_selection_strategy_rejects_missing_or_empty_value(self):
        self.assertIsNone(_selection_strategy("[dev] novice_poi mutant_nest pos=1,2,3"))
        self.assertIsNone(_selection_strategy("[dev] novice_poi mutant_nest selection="))


class NorthRiftScenarioContractTest(unittest.TestCase):
    def test_ambient_contract_accepts_only_state_matched_wilderness_recipes(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, ambient_recipe_id in (
            ("AMBIENT", "ambient_wilderness"),
            ("CULTIVATION", "cultivation_meditate"),
        ):
            with self.subTest(music_state=music_state):
                north_rift_assert_ambient(
                    bot,
                    probe,
                    {
                        "pos": expected_pos,
                        "music_state": music_state,
                        "ambient_recipe_id": ambient_recipe_id,
                    },
                )

    def test_ambient_contract_rejects_crossed_state_recipe_pairs(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, wrong_recipe in (
            ("AMBIENT", "cultivation_meditate"),
            ("CULTIVATION", "ambient_wilderness"),
        ):
            with self.subTest(music_state=music_state, wrong_recipe=wrong_recipe):
                with self.assertRaisesRegex(BotAssertionError, "ambient_recipe_id"):
                    north_rift_assert_ambient(
                        bot,
                        probe,
                        {
                            "pos": expected_pos,
                            "music_state": music_state,
                            "ambient_recipe_id": wrong_recipe,
                        },
                    )

    def test_ambient_contract_rejects_unexpected_or_missing_music_state(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")
        expected_pos = [int(value) for value in probe.pos]

        for music_state, production_recipe in (
            ("COMBAT", "combat_music"),
            ("TSY", "ambient_tsy"),
            ("TRIBULATION", "tribulation_atmosphere"),
            ("UNKNOWN", "ambient_wilderness"),
            (None, "ambient_wilderness"),
        ):
            with self.subTest(
                music_state=music_state, production_recipe=production_recipe
            ):
                payload = {
                    "pos": expected_pos,
                    "ambient_recipe_id": production_recipe,
                }
                if music_state is not None:
                    payload["music_state"] = music_state
                with self.assertRaisesRegex(BotAssertionError, "music_state"):
                    north_rift_assert_ambient(bot, probe, payload)

    def test_ambient_contract_still_rejects_wrong_authoritative_position(self):
        probe = NORTH_RIFT_PROBES[0]
        bot = types.SimpleNamespace(username="Fake")

        with self.assertRaisesRegex(BotAssertionError, "pos"):
            north_rift_assert_ambient(
                bot,
                probe,
                {
                    "pos": [int(probe.pos[0]), int(probe.pos[1]), int(probe.pos[2]) + 1],
                    "music_state": "CULTIVATION",
                    "ambient_recipe_id": "cultivation_meditate",
                },
            )

    def test_probes_pin_three_production_z_coordinates_and_zone_identity(self):
        actual = {probe.pos[2]: probe.zone for probe in NORTH_RIFT_PROBES}
        self.assertEqual(
            actual,
            {
                -7303.0: "rift_mouth_north_002",
                -7800.0: "north_waste_east_scorch",
                -7500.0: "north_waste_east_scorch",
            },
            "真实场景必须覆盖 portal 半径外的迁移后渊口、旧入口焦土点与 inclusive 边界",
        )
        rift_probe = next(
            probe for probe in NORTH_RIFT_PROBES if probe.zone == "rift_mouth_north_002"
        )
        self.assertGreater(
            abs(rift_probe.pos[2] - (-7300.0)),
            2.0,
            "真实 bot 的 rift identity 点必须避开 z=-7300 portal anchor 的 2 格传送半径",
        )

    def test_probe_order_forces_a_zone_transition_before_every_assertion(self):
        zones = [probe.zone for probe in NORTH_RIFT_PROBES]
        self.assertTrue(
            all(left != right for left, right in zip(zones, zones[1:])),
            f"相邻 probe 必须跨 zone 才会产生 after-watermark zone_info，实际 {zones}",
        )

    def test_probe_perception_text_matches_production_qi_transition_ratios(self):
        self.assertEqual(
            [probe.perception_text for probe in NORTH_RIFT_PROBES],
            [
                "灵气稀薄，引气如吸沙",
                "灵气几近断绝，此地有不祥预感",
                "此地灵气骤然浓郁，呼吸间元气盈满",
            ],
            "spawn→scorch→rift→scorch 的 production qi 比值必须钉住真实感知文本",
        )

    def test_authoritative_position_match_rejects_stale_and_wrong_packets(self):
        probe = NORTH_RIFT_PROBES[0]
        exact = types.SimpleNamespace(
            kind="pos_look",
            t=2.0,
            data={
                "x": probe.pos[0],
                "y": probe.pos[1],
                "z": probe.pos[2],
                "yaw": probe.yaw,
                "pitch": probe.pitch,
            },
        )
        self.assertTrue(north_rift_position_matches(exact, probe, watermark=1.0))
        self.assertFalse(
            north_rift_position_matches(exact, probe, watermark=2.0),
            "watermark 之前/同刻的历史 pos_look 不得冒充本次 authoritative 回包",
        )
        wrong = types.SimpleNamespace(
            kind="pos_look",
            t=3.0,
            data={**exact.data, "z": probe.pos[2] + 1.0},
        )
        self.assertFalse(
            north_rift_position_matches(wrong, probe, watermark=1.0),
            "坐标仅差一格也不得通过 production point 对拍",
        )


class BotE2eDevModeContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        root = pathlib.Path(__file__).parents[2]
        cls.source = (root / "scripts/bot-e2e.sh").read_text(encoding="utf-8")
        cls.compose_source = (root / "docker-compose.test.yml").read_text(
            encoding="utf-8"
        )
        cls.workflow_source = (root / ".github/workflows/e2e.yml").read_text(
            encoding="utf-8"
        )

    def test_fallback_readiness_pattern_matches_production_tracing_line(self):
        pattern_matches = re.findall(
            r"^BOT_FALLBACK_READY_PATTERN='([^']+)'$",
            self.source,
            re.MULTILINE,
        )
        self.assertEqual(
            len(pattern_matches),
            1,
            "bot-e2e.sh 必须恰好声明一个 canonical fallback readiness pattern："
            "重复声明会留下歧义的真实来源，grep 匹配到哪一条不可预期",
        )
        pattern = pattern_matches[0]
        production_line = (
            "2026-08-07T10:25:04.830194Z  INFO "
            "[bong][world] BOT_FALLBACK_FLAT_READY "
            "anchors=3 chunks=5002 view_distance_chunks=20"
        )
        production_lines = (
            production_line,
            "\x1b[2m2026-08-07T10:25:04.830194Z\x1b[0m "
            "\x1b[32m INFO\x1b[0m "
            "[bong][world] BOT_FALLBACK_FLAT_READY "
            "anchors=3 chunks=5002 view_distance_chunks=20",
        )
        for traced_line in production_lines:
            with self.subTest(traced_line=traced_line):
                matched = subprocess.run(
                    ["grep", "-Eq", "--", pattern],
                    input=f"{traced_line}\n",
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(
                    matched.returncode,
                    0,
                    "fallback readiness matcher 必须接受 production tracing 的完整日志行（含默认 ANSI 或 NO_COLOR 形态）",
                )

        for invalid_line in (
            "[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=5002 view_distance_chunks=20",
            production_line.replace("  INFO ", "  WARN "),
            production_line.replace("anchors=3", "anchors=0"),
            production_line.replace("chunks=5002", "chunks=0"),
            production_line.replace("view_distance_chunks=20", "view_distance_chunks=0"),
            f"noise {production_line}",
            f"{production_line} suffix",
        ):
            with self.subTest(invalid_line=invalid_line):
                rejected = subprocess.run(
                    ["grep", "-Eq", "--", pattern],
                    input=f"{invalid_line}\n",
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(
                    rejected.returncode,
                    1,
                    "fallback readiness matcher 必须拒绝缺少 production INFO envelope、零计数或额外前后缀的近似行",
                )

    def test_mode_contract_distinguishes_generic_inputs_from_owned_fixture_inputs(self):
        guard_end = self.source.index('\nmkdir -p "$EVIDENCE_ROOT"')
        guard = self.source[:guard_end]
        self.assertIn('AMBIENT_FIXTURE_MODE="${BOT_E2E_AMBIENT_FIXTURE_MODE:-0}"', guard)
        self.assertIn('FALLBACK_MODE="${BOT_E2E_FALLBACK_MODE:-0}"', guard)
        self.assertIn('BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_REUSE=1 互斥', guard)
        self.assertIn('BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_FALLBACK_MODE=1 互斥', guard)
        self.assertIn('BOT_E2E_FALLBACK_MODE=1 与 BOT_E2E_REUSE=1 互斥', guard)
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', guard)
        self.assertIn('if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', guard)
        self.assertIn('if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_WORLD_PATH:-}" ]; then', guard)
        self.assertIn('if [ "$FALLBACK_MODE" = "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', guard)
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', guard)
        self.assertNotIn('if [ "$REUSE" != "1" ] && [ -n "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', guard)
        self.assertNotIn('if [ "$REUSE" != "1" ] && [ -n "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', guard)

    def test_generic_mode_preserves_caller_inputs_and_skips_ambient_ownership(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        self.assertIn('if [ "$REUSE" != "1" ] && [ "$FALLBACK_MODE" != "1" ] && [ -z "${BONG_TERRAIN_RASTER_PATH:-}" ]; then', fixture)
        self.assertIn('BOT_FIXTURE_TOKEN="$(python3 -c', fixture)
        self.assertIn('--fixture-token "$BOT_FIXTURE_TOKEN"', fixture)
        state_start = self.source.index('# Dedicated world evidence pins state to its private runtime.')
        state_end = self.source.index('\nport_open() {', state_start)
        state = self.source[state_start:state_end]
        self.assertIn('if [ "$OWNED_WORLD_MODE" = "1" ]; then', state)
        self.assertIn('elif [ "$REUSE" != "1" ] && [ -z "${BONG_SPIRITWOOD_HARVESTED_PATH:-}" ]; then', state)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_OWNED', fixture)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_MANIFEST', fixture)
        self.assertIn('unset BOT_E2E_AMBIENT_FIXTURE_TOKEN', fixture)

        redis_start = self.source.index('# ---- redis ----')
        redis_end = self.source.index('\n# ---- server ----', redis_start)
        redis = self.source[redis_start:redis_end]
        redis_guard = 'elif [ "$REUSE" != "1" ] && { [ "$OWNED_WORLD_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then'
        self.assertIn(redis_guard, redis)
        self.assertNotIn('export REDIS_URL=', redis[:redis.index(redis_guard)])

    def test_dedicated_world_modes_keep_private_runtime_and_exact_marker_gate(self):
        runtime_start = self.source.index('if [ "$OWNED_WORLD_MODE" = "1" ]; then', self.source.index('SERVER_LOG='))
        runtime_end = self.source.index('\n# Owned-fixture mode generates', runtime_start)
        runtime = self.source[runtime_start:runtime_end]
        self.assertIn('SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"', runtime)
        self.assertIn('ln -s "$ROOT/server/assets" "$SERVER_RUNTIME_DIR/server/assets"', runtime)
        self.assertIn('export BONG_SERVER_DB="$SERVER_RUNTIME_DIR/server/data/bong.db"', runtime)

        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index('\n# ---- 场景 ----', readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        self.assertIn('grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"', readiness)
        self.assertIn('export BOT_E2E_AMBIENT_FIXTURE_OWNED=1', readiness)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', readiness)
        self.assertLess(readiness.index('grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"'), readiness.index('export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'))

    def test_generic_and_reuse_never_grant_ambient_ownership(self):
        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index('\n# ---- 场景 ----', readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        grant = 'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'
        self.assertIn('if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then', readiness)
        self.assertLess(readiness.index('if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then'), readiness.index(grant))
        self.assertIn('BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起', self.source)
        self.assertIn('$HOST:$PORT 已被占用。要对着现有 server 跑请设 BOT_E2E_REUSE=1', self.source)
        self.assertNotIn('pkill', self.source)

    def test_mode_contract_executes_early_rejections_before_harness_setup(self):
        root = pathlib.Path(__file__).parents[2]
        cases = (
            (
                {"BOT_E2E_AMBIENT_FIXTURE_MODE": "bogus"},
                "BOT_E2E_AMBIENT_FIXTURE_MODE 仅接受空值、0 或 1",
            ),
            (
                {"BOT_E2E_FALLBACK_MODE": "bogus"},
                "BOT_E2E_FALLBACK_MODE 仅接受空值、0 或 1",
            ),
            (
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BOT_E2E_FALLBACK_MODE": "1",
                },
                "BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_FALLBACK_MODE=1 互斥",
            ),
            (
                {"BOT_E2E_FALLBACK_MODE": "1", "BOT_E2E_REUSE": "1"},
                "BOT_E2E_FALLBACK_MODE=1 与 BOT_E2E_REUSE=1 互斥",
            ),
            (
                {
                    "BOT_E2E_FALLBACK_MODE": "1",
                    "BONG_TERRAIN_RASTER_PATH": "/caller/terrain.json",
                },
                "fallback mode 不接受 BONG_TERRAIN_RASTER_PATH",
            ),
            (
                {
                    "BOT_E2E_FALLBACK_MODE": "1",
                    "BONG_WORLD_PATH": "/caller/world",
                },
                "fallback mode 不接受 BONG_WORLD_PATH",
            ),
            (
                {
                    "BOT_E2E_FALLBACK_MODE": "1",
                    "BONG_SPIRITWOOD_HARVESTED_PATH": "/caller/harvested.json",
                },
                "fallback mode 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH",
            ),
            (
                {"BOT_E2E_AMBIENT_FIXTURE_MODE": "1", "BOT_E2E_REUSE": "1"},
                "BOT_E2E_AMBIENT_FIXTURE_MODE=1 与 BOT_E2E_REUSE=1 互斥",
            ),
            (
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BONG_TERRAIN_RASTER_PATH": "/caller/terrain.json",
                },
                "ambient fixture mode 不接受外部 BONG_TERRAIN_RASTER_PATH",
            ),
            (
                {
                    "BOT_E2E_AMBIENT_FIXTURE_MODE": "1",
                    "BONG_SPIRITWOOD_HARVESTED_PATH": "/caller/harvested.json",
                },
                "ambient fixture mode 不接受外部 BONG_SPIRITWOOD_HARVESTED_PATH",
            ),
            (
                {"BOT_E2E_FALLBACK_MODE": "bogus"},
                "BOT_E2E_FALLBACK_MODE 仅接受空值、0 或 1",
            ),
            (
                {"BOT_E2E_FALLBACK_MODE": "1", "BOT_E2E_REUSE": "1"},
                "BOT_E2E_FALLBACK_MODE=1 与 BOT_E2E_REUSE=1 互斥",
            ),
            (
                {
                    "BOT_E2E_FALLBACK_MODE": "1",
                    "BONG_TERRAIN_RASTER_PATH": "/caller/terrain.json",
                },
                "fallback mode 不接受 BONG_TERRAIN_RASTER_PATH",
            ),
        )
        isolated = (
            "BOT_E2E_AMBIENT_FIXTURE_MODE",
            "BOT_E2E_FALLBACK_MODE",
            "BOT_E2E_REUSE",
            "BOT_E2E_HOST",
            "BOT_E2E_PORT",
            "BONG_TERRAIN_RASTER_PATH",
            "BONG_WORLD_PATH",
            "BONG_SPIRITWOOD_HARVESTED_PATH",
            "BONG_E2E_PREBUILT_SERVER_MANIFEST",
            "REDIS_URL",
        )
        for overrides, expected in cases:
            with self.subTest(overrides=overrides):
                env = os.environ.copy()
                for name in isolated:
                    env.pop(name, None)
                env.update(overrides)
                result = subprocess.run(
                    ["bash", "scripts/bot-e2e.sh"],
                    cwd=root,
                    env=env,
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn(expected, result.stderr)

    def test_fallback_reuse_guard_precedes_reuse_normalization(self):
        normalization = self.source.index("  REUSE=0")
        guard = self.source.index("BOT_E2E_FALLBACK_MODE=1 与 BOT_E2E_REUSE=1 互斥")
        self.assertLess(
            guard,
            normalization,
            "fallback×reuse 互斥守卫必须先于 REUSE 归一化执行；"
            "归一化在前会把 REUSE 就地改成 0，守卫校验的是已变异值而非调用方原始请求，排除被绕过",
        )

    def test_generic_no_raster_generates_tokenized_fixture_before_tool_failure(self):
        root = pathlib.Path(__file__).parents[2]
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_bin = pathlib.Path(temp_dir) / "bin"
            fake_bin.mkdir()
            log_path = pathlib.Path(temp_dir) / "python.log"
            real_python = shutil.which("python3")
            (fake_bin / "python3").write_text(
                "#!/usr/bin/env bash\n"
                f"printf '%s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "if [[ \"$1\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"$1\" == *make_novice_raster_fixture.py ]]; then\n"
                "  test \"$3\" = \"--fixture-token\" || exit 41\n"
                "  test -n \"$4\" || exit 42\n"
                "  printf '%s/manifest.json\\n' \"$2\"\n"
                "  exit 0\n"
                "fi\n"
                f"exec {shlex.quote(real_python)} \"$@\"\n",
                encoding="utf-8",
            )
            (fake_bin / "python3").chmod(0o755)
            (fake_bin / "docker").write_text("#!/usr/bin/env bash\nexit 1\n", encoding="utf-8")
            (fake_bin / "docker").chmod(0o755)
            (fake_bin / "cargo").write_text("#!/usr/bin/env bash\nexit 46\n", encoding="utf-8")
            (fake_bin / "cargo").chmod(0o755)
            env = os.environ.copy()
            for name in (
                "BOT_E2E_AMBIENT_FIXTURE_MODE",
                "BOT_E2E_FALLBACK_MODE",
                "BOT_E2E_REUSE",
                "BOT_E2E_HOST",
                "BOT_E2E_PORT",
                "BONG_TERRAIN_RASTER_PATH",
                "BONG_SPIRITWOOD_HARVESTED_PATH",
                "BONG_E2E_PREBUILT_SERVER_MANIFEST",
                "REDIS_URL",
            ):
                env.pop(name, None)
            env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
            env["BONG_BUILD_TOKEN_TEST_MODE"] = "1"
            env["BONG_BUILD_TOKEN_DIR"] = str(pathlib.Path(temp_dir) / "build-token")
            result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
                check=False,
                timeout=20,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("--fixture-token", log_path.read_text(encoding="utf-8"))
            self.assertNotIn("BOT_RASTER_FIXTURE_READY", result.stderr)

    def test_ci_selects_explicit_ambient_fixture_mode(self):
        bot_stage = self.workflow_source[self.workflow_source.index('Bot e2e stage'):]
        self.assertIn('BOT_E2E_AMBIENT_FIXTURE_MODE: "1"', bot_stage)

    def test_anqi_all_gate_is_wired_and_reuse_log_is_preserved(self):
        # The three anqi scenarios are dedicated because they need Redis plus
        # a server log.  The harness must export the gate only after those
        # prerequisites are owned/available, and reuse mode must not overwrite
        # a caller-provided BONG_SERVER_LOG.
        self.assertIn('export BOT_E2E_ANQI_REDIS=1', self.source)
        self.assertIn('CALLER_SERVER_LOG="${BONG_SERVER_LOG:-}"', self.source)
        self.assertIn('SERVER_LOG="$CALLER_SERVER_LOG"', self.source)
        self.assertIn('unset BOT_E2E_ANQI_REDIS', self.source)
        for name in (
            "combat_anqi_charge_carrier",
            "combat_anqi_container_switch",
            "combat_anqi_throw_carrier",
        ):
            module = discover_scenarios()[name]
            self.assertFalse(module.DEFAULT_ENABLED)
            self.assertEqual(module.RUN_IN_ALL_WHEN_ENV, "BOT_E2E_ANQI_REDIS")

    def test_ci_runs_owned_rasterless_fallback_stage(self):
        stage_start = self.workflow_source.index('Bot fallback-flat e2e stage')
        stage_end = self.workflow_source.index(
            'Bot chat timestamp', stage_start
        )
        stage = self.workflow_source[stage_start:stage_end]
        self.assertIn('BOT_E2E_RUN_TAG: ci', stage)
        self.assertIn('BOT_E2E_FALLBACK_MODE: "1"', stage)
        self.assertIn('run: bash scripts/bot-e2e.sh', stage)

    def test_fallback_ownership_requires_exact_marker_and_continuous_listener_watch(self):
        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index('\n# ---- 场景 ----', readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        marker_match = "fallback_ready_marker_present"
        ownership = 'export BOT_E2E_FALLBACK_OWNED=1'
        self.assertIn(marker_match, readiness)
        self.assertIn(ownership, readiness)
        self.assertLess(readiness.index(marker_match), readiness.index(ownership))

        scenario = self.source[self.source.index('# ---- 场景 ----'):]
        self.assertIn(
            'if [ "$AMBIENT_FIXTURE_MODE" = "1" ] || [ "$FALLBACK_MODE" = "1" ]; then',
            scenario,
            "fallback 场景执行期间也必须持续盯住本轮 listener ownership",
        )
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', scenario)
        self.assertIn('echo lost >"$RUNTIME_WATCH_LOG"', scenario)

    def test_fallback_harness_selects_only_dedicated_join_scenario(self):
        scenario = self.source[self.source.index('# ---- 场景 ----'):]
        self.assertIn('SCENARIO_ARGS=(--all)', scenario)
        self.assertIn('if [ "$FALLBACK_MODE" = "1" ]; then', scenario)
        self.assertIn(
            'SCENARIO_ARGS=(--scenario terrain_join_chunk_delivery)',
            scenario,
            "fallback CI 不得重跑 gameplay --all 或依赖 dev commands",
        )
        self.assertIn(
            'elif [ -n "${BOT_E2E_SCENARIOS:-}" ]; then',
            scenario,
            "owned dev harness 应允许显式聚焦一组场景，避免 P2 验证被全套 P3-P5 噪声遮蔽",
        )
        self.assertIn('IFS=\',\' read -r -a requested_scenarios', scenario)
        self.assertIn('[[ "$BOT_E2E_SCENARIOS" == ,*', scenario)
        self.assertIn('"$BOT_E2E_SCENARIOS" == *,', scenario)
        self.assertIn('"$BOT_E2E_SCENARIOS" == *,,*', scenario)
        self.assertIn('trimmed_scenario="${scenario#', scenario)
        self.assertIn('[ "$trimmed_scenario" != "$scenario" ]', scenario)
        self.assertLess(
            scenario.index('if [ "$FALLBACK_MODE" = "1" ]; then'),
            scenario.index('elif [ -n "${BOT_E2E_SCENARIOS:-}" ]; then'),
            "fallback dedicated selector 优先级必须高于外部 focused selector",
        )
        self.assertIn(
            'python3 "$ROOT/scripts/bot/run_scenarios.py" "${SCENARIO_ARGS[@]}"',
            scenario,
        )

    def test_fallback_ci_cluster_witness_pins_exact_production_disks(self):
        self.assertEqual(
            EXPECTED_CI_CLUSTERS,
            {
                "J1": ((180.0, 140.0), 112.0, "east"),
                "J2": ((-240.0, -160.0), 96.0, "west"),
                "FC": ((24.0, -24.0), 80.0, "central"),
            },
        )
        for tag, ((anchor_x, anchor_z), radius, expected_name) in EXPECTED_CI_CLUSTERS.items():
            with self.subTest(tag=tag):
                self.assertEqual(
                    _assert_expected_cluster("ci", tag, (anchor_x + radius, anchor_z)),
                    expected_name,
                    "配置圆盘 inclusive 边界必须通过",
                )
                with self.assertRaises(BotAssertionError):
                    _assert_expected_cluster(
                        "ci", tag, (anchor_x + radius + 0.001, anchor_z)
                    )
        with self.assertRaises(BotAssertionError, msg="非 ci tag 不能冒充三簇稳定 witness"):
            _assert_expected_cluster("local", "J1", (180.0, 140.0))
        with self.assertRaises(BotAssertionError, msg="未知 tag 必须 fail closed"):
            _assert_expected_cluster("ci", "XX", (180.0, 140.0))

    def test_fallback_runtime_and_state_are_private(self):
        runtime_start = self.source.index('if [ "$OWNED_WORLD_MODE" = "1" ]; then', self.source.index('SERVER_LOG='))
        runtime_end = self.source.index('\n# Owned-fixture mode generates', runtime_start)
        runtime = self.source[runtime_start:runtime_end]
        self.assertIn('SERVER_RUNTIME_DIR="$(mktemp -d "$EVIDENCE_DIR/server-runtime.XXXXXX")"', runtime)

        state_start = self.source.index('# Dedicated world evidence pins state to its private runtime.')
        state_end = self.source.index('\nport_open() {', state_start)
        state = self.source[state_start:state_end]
        self.assertIn('if [ "$OWNED_WORLD_MODE" = "1" ]; then', state)
        self.assertIn('SPIRITWOOD_STATE_DIR="$SERVER_RUNTIME_DIR/server/data/spiritwood"', state)

    def test_fallback_mode_skips_novice_raster_generation(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        self.assertIn(
            'if [ "$REUSE" != "1" ] && [ "$FALLBACK_MODE" != "1" ] && [ -z "${BONG_TERRAIN_RASTER_PATH:-}" ]; then',
            fixture,
        )

    def test_self_started_server_builds_unique_exact_fixture_identity(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        for required in (
            'mktemp -d "$EVIDENCE_DIR/novice-raster.XXXXXX"',
            '--fixture-token "$BOT_E2E_AMBIENT_FIXTURE_TOKEN"',
            'export BOT_E2E_AMBIENT_FIXTURE_TOKEN',
            'export BOT_E2E_AMBIENT_FIXTURE_MANIFEST="$BONG_TERRAIN_RASTER_PATH"',
            "pathlib.Path(sys.argv[1]).resolve(strict=True)",
            'BOT_RASTER_READY_PAYLOAD="[bong][world] BOT_RASTER_FIXTURE_READY '
            'manifest=$BONG_TERRAIN_RASTER_PATH token=$BOT_E2E_AMBIENT_FIXTURE_TOKEN"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, fixture)
        self.assertNotIn(
            'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1', fixture,
            "生成本地文件不能授予 ownership；必须等 server exact ready marker",
        )

    def test_fixture_ownership_is_granted_only_after_exact_server_marker_and_port_ownership(self):
        readiness_start = self.source.index('BOOT_ANCHOR="spawned tsy dimension layer')
        readiness_end = self.source.index("\n# ---- 场景 ----", readiness_start)
        readiness = self.source[readiness_start:readiness_end]
        marker_match = 'grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"'
        ownership = 'export BOT_E2E_AMBIENT_FIXTURE_OWNED=1'

        self.assertIn(marker_match, readiness)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', readiness)
        self.assertIn('port_open "$HOST" "$PORT"', readiness)
        self.assertIn(ownership, readiness)
        self.assertLess(
            readiness.index(marker_match),
            readiness.index(ownership),
            "必须先由 server 日志对拍 canonical manifest+token，再允许场景声明 ownership",
        )
        self.assertLess(
            self.source.index(ownership, readiness_start),
            self.source.index('python3 "$ROOT/scripts/bot/run_scenarios.py" "${SCENARIO_ARGS[@]}"'),
            "场景只可在本轮 server fixture identity 与端口 ownership 同时成立后运行",
        )

    def test_bot_e2e_pipeline_propagates_runner_then_tee_status(self) -> None:
        scenario_start = self.source.index("set +e\n", self.source.index("# ---- 场景 ----"))
        scenario_end = self.source.index("\nset -e", scenario_start) + len("\nset -e")
        pipeline = self.source[scenario_start:scenario_end]
        runner = '''BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT" BONG_SERVER_LOG="$SERVER_LOG" \\
  python3 "$ROOT/scripts/bot/run_scenarios.py" "${SCENARIO_ARGS[@]}" 2>&1'''
        sink = 'tee "$SCENARIOS_LOG"'
        self.assertEqual(
            pipeline.count(runner),
            1,
            "场景块必须仅含一个 canonical runner，避免测试替换到错误命令",
        )
        self.assertEqual(
            pipeline.count(sink),
            1,
            "场景块必须仅含一个 canonical sink，避免测试替换到错误管道",
        )

        for runner_code, tee_code, expected in (
            (7, 0, 7),
            (7, 9, 7),
            (0, 9, 9),
            (0, 0, 0),
        ):
            with self.subTest(runner_code=runner_code, tee_code=tee_code):
                executable = pipeline.replace(
                    runner,
                    f"bash -c 'exit {runner_code}'",
                    1,
                ).replace(
                    sink,
                    f"bash -c 'exit {tee_code}'",
                    1,
                )
                result = subprocess.run(
                    ["bash", "-c", f'{executable}\nexit "$EXIT_CODE"'],
                    check=False,
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(
                    result.returncode,
                    expected,
                    "真实 bot-e2e status block 必须优先传播 runner 失败，并传播独立 sink 失败",
                )

    def test_fixture_runtime_ownership_is_rechecked_during_and_after_scenarios(self) -> None:
        helper_start = self.source.index("self_started_fixture_runtime_is_current() {")
        helper_end = self.source.index("\n}\n", helper_start)
        helper = self.source[helper_start:helper_end]
        for required in (
            'kill -0 "$SERVER_PID"',
            'grep -Fq -- "$BOT_RASTER_READY_PAYLOAD" "$SERVER_LOG"',
            'port_open "$HOST" "$PORT"',
            'port_owned_by_tree "$SERVER_PID" "$PORT"',
        ):
            with self.subTest(required=required):
                self.assertIn(
                    required,
                    helper,
                    "fixture runtime helper 必须重新绑定活 server、exact marker 与当前 listener",
                )

        scenario_start = self.source.index("# ---- 场景 ----")
        scenario = self.source[scenario_start:]
        runner = 'python3 "$ROOT/scripts/bot/run_scenarios.py" "${SCENARIO_ARGS[@]}"'
        self.assertIn('while true; do', scenario)
        self.assertIn('port_owned_by_tree "$SERVER_PID" "$PORT"', scenario)
        self.assertIn('echo lost >"$RUNTIME_WATCH_LOG"', scenario)
        self.assertIn('echo complete >"$RUNTIME_WATCH_LOG"', scenario)
        self.assertIn('pipeline_status=("${PIPESTATUS[@]}")', scenario)
        self.assertIn('EXIT_CODE=${pipeline_status[0]}', scenario)
        self.assertIn('EXIT_CODE=${pipeline_status[1]}', scenario)
        self.assertIn('wait "$WATCH_PID"', scenario)
        self.assertGreaterEqual(
            scenario.count("self_started_fixture_runtime_is_current"),
            2,
            "场景前和场景后都必须重验 server/fixture ownership，不能把一次 readiness 当永久授权",
        )
        self.assertLess(
            scenario.index("self_started_fixture_runtime_is_current"),
            scenario.index(runner),
            "运行 Bot 前必须重新验证本轮 server 仍持有 listener",
        )
        self.assertGreater(
            scenario.rindex("self_started_fixture_runtime_is_current"),
            scenario.index(runner),
            "Bot 结束后必须再次验证本轮 server 仍是证据主体",
        )

    def test_reuse_mode_clears_fixture_ownership_instead_of_claiming_external_server(self):
        fixture_start = self.source.index('# Owned-fixture mode generates')
        fixture_end = self.source.index('\nSERVER_PID=', fixture_start)
        fixture = self.source[fixture_start:fixture_end]
        for variable in (
            "BOT_E2E_AMBIENT_FIXTURE_OWNED",
            "BOT_E2E_AMBIENT_FIXTURE_MANIFEST",
            "BOT_E2E_AMBIENT_FIXTURE_TOKEN",
        ):
            with self.subTest(variable=variable):
                self.assertIn(f"unset {variable}", fixture)

    def test_reuse_without_listener_fails_closed_before_private_self_start(self):
        server_start = self.source.index("# ---- server ----")
        server_end = self.source.index("\n# ---- 场景 ----", server_start)
        server = self.source[server_start:server_end]
        rejection = 'BOT_E2E_REUSE=1 但 $HOST:$PORT 没有可复用的 server，拒绝退化为未隔离自起'
        self.assertIn(rejection, server)
        dispatch = server[server.index("server_ready=0"):]
        self.assertLess(
            dispatch.index(rejection),
            dispatch.index('if start_self_server_attempt "$attempt"'),
            "REUSE 缺少 listener 时必须在调用自起 server 之前 fail closed",
        )

    def test_owned_world_modes_use_private_cwd_and_only_ambient_enables_dev_commands(self):
        launch_start = self.source.index('  (\n    if [ "$OWNED_WORLD_MODE" = "1" ]; then')
        launch_end = self.source.index('  ) >>"$SERVER_LOG"', launch_start)
        launch = self.source[launch_start:launch_end]
        for required in (
            'cd "$SERVER_RUNTIME_DIR/server"',
            'cd "$ROOT/server"',
            'export BONG_DORMANT_ROGUE_SEED_COUNT="${BONG_DORMANT_ROGUE_SEED_COUNT:-0}"',
            'export BONG_ASSETS_DIR="$ROOT/server"',
            'if [ "$AMBIENT_FIXTURE_MODE" = "1" ]; then',
            'export BONG_DEV_MODE=1',
            'export BONG_OPERATORS="$BOT_E2E_OPERATORS"',
            'exec "$SERVER_BINARY"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, launch)
        self.assertIn(
            '"$ROOT/scripts/build-token.sh" cargo build --locked "${PROFILE_FLAG[@]}"',
            self.source,
        )
        self.assertIn('install -m 700 "$CARGO_TARGET_ROOT/$TARGET_PROFILE/bong-server" "$SERVER_BINARY"', self.source)
        self.assertIn('exec "$SERVER_BINARY"', launch)
        self.assertNotIn(
            'exec cargo run --locked',
            launch,
            "bot-e2e 自起 server 必须经过全机共享 cargo counting token",
        )

    def test_each_run_owns_private_evidence_and_persistent_logs(self):
        evidence_setup = self.source[:self.source.index('# Owned-fixture mode generates')]
        for required in (
            'EVIDENCE_ROOT="$ROOT/.sisyphus/evidence/bot-e2e"',
            'EVIDENCE_DIR="$(mktemp -d "$EVIDENCE_ROOT/run.XXXXXXXXXX")"',
            'SERVER_LOG="$EVIDENCE_DIR/server.log"',
            'SCENARIOS_LOG="$EVIDENCE_DIR/scenarios.log"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)
        self.assertIn('mkdir -p "$EVIDENCE_ROOT"', evidence_setup)
        self.assertNotIn('EVIDENCE_DIR="$ROOT/.sisyphus/evidence/bot-e2e"', self.source)

    def test_port_input_is_canonicalized_and_zero_rejected_before_harness_setup(self):
        port_start = self.source.index("# 自起模式并发安全：")
        port_end = self.source.index("\n# Ambient fixture ownership", port_start)
        port = self.source[port_start:port_end]
        launch_start = self.source.index('  (\n    if [ "$OWNED_WORLD_MODE" = "1" ]; then')
        launch_end = self.source.index('  ) >>"$SERVER_LOG"', launch_start)
        port += self.source[launch_start:launch_end]
        scenario_start = self.source.index("# ---- 场景 ----")
        port += self.source[scenario_start:]
        for required in (
            'sock.bind(("0.0.0.0", 0))',
            'canonicalize_port() {',
            'if ! PORT="$(canonicalize_port "$RAW_PORT")"; then',
            'export BONG_SERVER_PORT="$PORT"',
            'BOT_E2E_HOST="$HOST" BOT_E2E_PORT="$PORT"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, port)

        root = pathlib.Path(__file__).parents[2]
        env = os.environ.copy()
        env.update({"BOT_E2E_PORT": " 0 ", "BOT_E2E_AMBIENT_FIXTURE_MODE": "0"})
        result = subprocess.run(
            ["bash", "scripts/bot-e2e.sh"], cwd=root, env=env,
            capture_output=True, text=True, check=False, timeout=10,
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn("BOT_E2E_PORT 必须是 1-65535", result.stderr)
        self.assertNotIn("600s 内未同时满足", result.stderr)

    def test_auto_port_collision_uses_wildcard_probe_and_bounded_retry(self):
        server_start = self.source.index("# ---- server ----")
        server_end = self.source.index("\n# ---- 场景 ----", server_start)
        server = self.source[server_start:server_end]
        for required in (
            "MAX_PORT_RETRIES=3",
            "start_self_server_attempt()",
            'grep -Fq "failed to start TCP listener" "$SERVER_LOG"',
            'if [ "$attempt_status" -eq 75 ] && [ "$PORT_AUTO_ALLOCATED" = "1" ]',
            'PORT="$(canonicalize_port "$(allocate_free_port)")"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, server)
        allocation_start = self.source.index("allocate_free_port()")
        allocation_end = self.source.index("\n# 环境变量允许", allocation_start)
        self.assertIn('sock.bind(("0.0.0.0", 0))', self.source[allocation_start:allocation_end])

    def test_concurrent_invocations_scope_preflight_evidence_to_unique_namespaces(self):
        for required in (
            'EVIDENCE_ROOT="$BOT_E2E_EVIDENCE_ROOT"',
            'EVIDENCE_ROOT="$(mktemp -d "$EVIDENCE_ROOT/session.XXXXXXXXXX")"',
            'export BOT_E2E_EVIDENCE_ROOT="$EVIDENCE_ROOT"',
        ):
            with self.subTest(required=required):
                self.assertIn(required, self.source)
        protocol_source = pathlib.Path(__file__).read_text(encoding="utf-8")
        self.assertIn(
            'evidence_root = root / ".sisyphus/evidence/bot-e2e" / f"test-{uuid.uuid4().hex}"',
            protocol_source,
        )

    def test_redis_adopts_default_listener_or_starts_owned_private_instance(self):
        redis_start = self.source.index("# ---- redis ----")
        redis_end = self.source.index("\n# ---- server ----", redis_start)
        redis = self.source[redis_start:redis_end]
        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\ntrap cleanup EXIT", cleanup_start)
        cleanup = self.source[cleanup_start:cleanup_end]

        adopt_guard = 'if [ "$REUSE" != "1" ] && [ "$OWNED_WORLD_MODE" != "1" ] && [ -z "${REDIS_URL:-}" ] && port_open 127.0.0.1 6379; then'
        private_guard = 'elif [ "$REUSE" != "1" ] && { [ "$OWNED_WORLD_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then'
        self.assertIn(adopt_guard, redis)
        self.assertIn('沿用调用方默认 Redis 127.0.0.1:6379', redis)
        self.assertIn(private_guard, redis)
        self.assertLess(redis.index(adopt_guard), redis.index(private_guard))
        self.assertNotIn('export REDIS_URL=', redis[:redis.index(private_guard)])
        self.assertIn('if [ "$STARTED_REDIS" = "1" ] && [ -n "$REDIS_COMPOSE_PROJECT" ]; then', cleanup)

        root = pathlib.Path(__file__).parents[2]
        with tempfile.TemporaryDirectory() as temp_dir:
            fake_bin = pathlib.Path(temp_dir) / "bin"
            fake_bin.mkdir()
            log_path = pathlib.Path(temp_dir) / "tools.log"
            real_python = shutil.which("python3")
            (fake_bin / "python3").write_text(
                "#!/usr/bin/env bash\n"
                f"printf 'python3 %s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "if [[ \"$1\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"$1\" == *make_novice_raster_fixture.py ]]; then printf '%s/manifest.json\\n' \"$2\"; exit 0; fi\n"
                "if [[ \"$1\" == '-' ]] && [[ \"$2\" == '127.0.0.1' ]] && [[ \"$3\" == '6379' ]]; then exit \"${FAKE_DEFAULT_REDIS_OPEN:-1}\"; fi\n"
                f"exec {shlex.quote(real_python)} \"$@\"\n",
                encoding="utf-8",
            )
            (fake_bin / "python3").chmod(0o755)
            (fake_bin / "docker").write_text(
                "#!/usr/bin/env bash\n"
                f"printf 'docker %s\\n' \"$*\" >> {shlex.quote(str(log_path))}\n"
                "exit 47\n",
                encoding="utf-8",
            )
            (fake_bin / "docker").chmod(0o755)
            (fake_bin / "cargo").write_text("#!/usr/bin/env bash\nexit 46\n", encoding="utf-8")
            (fake_bin / "cargo").chmod(0o755)

            base_env = os.environ.copy()
            for name in (
                "BOT_E2E_AMBIENT_FIXTURE_MODE",
                "BOT_E2E_FALLBACK_MODE",
                "BOT_E2E_REUSE",
                "BOT_E2E_HOST",
                "BOT_E2E_PORT", "BONG_TERRAIN_RASTER_PATH",
                "BONG_SPIRITWOOD_HARVESTED_PATH", "BONG_E2E_PREBUILT_SERVER_MANIFEST",
                "REDIS_URL",
            ):
                base_env.pop(name, None)
            base_env["PATH"] = f"{fake_bin}{os.pathsep}{base_env['PATH']}"
            base_env["BONG_BUILD_TOKEN_TEST_MODE"] = "1"
            base_env["BONG_BUILD_TOKEN_DIR"] = str(pathlib.Path(temp_dir) / "build-token")

            adopted = base_env | {"FAKE_DEFAULT_REDIS_OPEN": "0"}
            adopted_result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"], cwd=root, env=adopted,
                capture_output=True, text=True, check=False, timeout=20,
            )
            adopted_tools = log_path.read_text(encoding="utf-8")
            self.assertIn("沿用调用方默认 Redis 127.0.0.1:6379", adopted_result.stdout)
            self.assertNotIn("docker ", adopted_tools)
            self.assertNotIn("REDIS_URL=", adopted_tools)
            self.assertNotIn("down -v", adopted_tools)

            log_path.unlink()
            private = base_env | {"FAKE_DEFAULT_REDIS_OPEN": "1"}
            private_result = subprocess.run(
                ["bash", "scripts/bot-e2e.sh"], cwd=root, env=private,
                capture_output=True, text=True, check=False, timeout=20,
            )
            private_tools = log_path.read_text(encoding="utf-8")
            self.assertEqual(private_result.returncode, 47, private_result.stderr)
            self.assertIn("docker compose", private_tools)
            self.assertIn("up -d redis --wait", private_tools)

    def test_redis_is_private_only_when_generic_caller_did_not_supply_url(self):
        redis_start = self.source.index("# ---- redis ----")
        redis_end = self.source.index("\n# ---- server ----", redis_start)
        redis = self.source[redis_start:redis_end]
        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\ntrap cleanup EXIT", cleanup_start)
        cleanup = self.source[cleanup_start:cleanup_end]
        self.assertIn('if [ "$REUSE" != "1" ] && [ "$OWNED_WORLD_MODE" != "1" ] && [ -z "${REDIS_URL:-}" ] && port_open 127.0.0.1 6379; then', redis)
        self.assertIn('elif [ "$REUSE" != "1" ] && { [ "$OWNED_WORLD_MODE" = "1" ] || [ -z "${REDIS_URL:-}" ]; }; then', redis)
        for required in (
            'REDIS_COMPOSE_PROJECT="bong-bot-e2e-${RUN_ID,,}"',
            'BONG_TEST_COMPOSE_PROJECT="$REDIS_COMPOSE_PROJECT" BONG_TEST_REDIS_PORT=0',
            'docker compose -f "$ROOT/docker-compose.test.yml" up -d redis --wait',
            'export REDIS_URL="redis://127.0.0.1:$redis_port"',
        ):
            self.assertIn(required, redis)
        self.assertIn('if [ "$STARTED_REDIS" = "1" ] && [ -n "$REDIS_COMPOSE_PROJECT" ]; then', cleanup)
        self.assertNotIn("pkill", self.source)
        self.assertNotIn("BOT_E2E_KILL_STALE", self.source)
        self.assertIn("name: ${BONG_TEST_COMPOSE_PROJECT:-bong-test}", self.compose_source)

    def _run_owned_fixture_runtime_case(
        self,
        runner_mode: str,
        *,
        runner_exit: int = 0,
        tee_exit: int | None = None,
        fallback_mode: bool = False,
    ) -> tuple[subprocess.CompletedProcess[str], str, str, str]:
        """Run the real bot-e2e shell path against only test-owned fake processes."""
        root = pathlib.Path(__file__).parents[2]
        real_python = shutil.which("python3")
        self.assertIsNotNone(real_python)

        with tempfile.TemporaryDirectory() as temp_dir:
            temp = pathlib.Path(temp_dir)
            fake_bin = temp / "bin"
            fake_bin.mkdir()
            runner_log = temp / "runner.log"
            runner_result_file = temp / "runner-result"
            server_pid_file = temp / "server.pid"
            listener_pid_file = temp / "listener.pid"
            replacement_pid_file = temp / "replacement.pid"
            replacement_ready_file = temp / "replacement-ready"
            watcher_status_file = temp / "watcher-status"
            runner_args_file = temp / "runner-args"
            redis_port = 39999
            port = self._unused_local_port()
            fake_python = fake_bin / "python3"
            fake_python.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                f"real_python={shlex.quote(real_python)}\n"
                "if [[ \"${1:-}\" == *test_protocol.py ]]; then exit 0; fi\n"
                "if [[ \"${1:-}\" == *make_novice_raster_fixture.py ]]; then\n"
                "  mkdir -p \"$2\"\n"
                "  : > \"$2/manifest.json\"\n"
                "  printf '%s/manifest.json\\n' \"$2\"\n"
                "  exit 0\n"
                "fi\n"
                "if [[ \"${1:-}\" == *run_scenarios.py ]]; then\n"
                f"  printf '%s\\n' \"$FAKE_RUNNER_MODE\" >> {shlex.quote(str(runner_log))}\n"
                f"  printf '%s\\n' \"$*\" > {shlex.quote(str(runner_args_file))}\n"
                "  wait_watcher_lost() {\n"
                "    local status\n"
                "    for _ in $(seq 1 200); do\n"
                "      status=$(find \"$FAKE_EVIDENCE_ROOT\" -path '*/runtime-watch.*/status' -type f -print -quit)\n"
                "      if test -n \"$status\" && test \"$(cat \"$status\" 2>/dev/null || true)\" = lost; then\n"
                "        printf 'watcher-lost-observed\\n' > \"$FAKE_RUNNER_RESULT_FILE\"\n"
                "        return 0\n"
                "      fi\n"
                "      sleep 0.01\n"
                "    done\n"
                "    printf 'watcher-lost-timeout\\n' > \"$FAKE_RUNNER_RESULT_FILE\"\n"
                "    return 1\n"
                "  }\n"
                "  port_owned_by_pid() {\n"
                "    kill -0 \"$1\" 2>/dev/null || return 1\n"
                "    \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY' || return 1\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "    ss -4 -H -ltnp \"sport = :$BOT_E2E_PORT\" 2>/dev/null | grep -qE \"pid=$1(,|\\))\"\n"
                "  }\n"
                "  case \"$FAKE_RUNNER_MODE\" in\n"
                "    success) printf 'runner-complete\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 0 ;;\n"
                "    runner-fail) printf 'runner-failed\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit \"$FAKE_RUNNER_EXIT\" ;;\n"
                "    kill-server)\n"
                "      kill -TERM \"$(cat \"$FAKE_SERVER_PID_FILE\")\"\n"
                "      for _ in $(seq 1 200); do\n"
                "        if ! kill -0 \"$(cat \"$FAKE_SERVER_PID_FILE\")\" 2>/dev/null \\\n"
                "          && ! \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "        then break; fi\n"
                "        sleep 0.01\n"
                "      done\n"
                "      if kill -0 \"$(cat \"$FAKE_SERVER_PID_FILE\")\" 2>/dev/null \\\n"
                "        || \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "      then printf 'fault-setup-timeout\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 79; fi\n"
                "      wait_watcher_lost || { printf 'watcher-lost-timeout\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 80; }\n"
                "      exit 0\n"
                "      ;;\n"
                "    replace-listener)\n"
                "      kill -TERM \"$(cat \"$FAKE_LISTENER_PID_FILE\")\"\n"
                "      for _ in $(seq 1 200); do\n"
                "        if ! \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY'\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.05):\n"
                "    pass\n"
                "PY\n"
                "        then break; fi\n"
                "        sleep 0.01\n"
                "      done\n"
                "      \"$real_python\" -u -c 'import os, socket, sys, time\ns = socket.socket()\ns.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\nfor _ in range(100):\n    try:\n        s.bind((\"127.0.0.1\", int(sys.argv[1])))\n        break\n    except OSError:\n        time.sleep(0.01)\nelse:\n    raise SystemExit(77)\ns.listen()\nopen(sys.argv[2], \"w\").write(str(os.getpid()))\nwhile True:\n    conn, _ = s.accept()\n    conn.close()' \"$BOT_E2E_PORT\" \"$FAKE_REPLACEMENT_READY_FILE\" </dev/null >/dev/null 2>&1 &\n"
                f"      printf '%s\\n' \"$!\" > {shlex.quote(str(replacement_pid_file))}\n"
                "      for _ in $(seq 1 200); do\n"
                "        test -s \"$FAKE_REPLACEMENT_READY_FILE\" && port_owned_by_pid \"$(cat \"$FAKE_REPLACEMENT_READY_FILE\")\" && break\n"
                "        sleep 0.01\n"
                "      done\n"
                "      test -s \"$FAKE_REPLACEMENT_READY_FILE\" && port_owned_by_pid \"$(cat \"$FAKE_REPLACEMENT_READY_FILE\")\" || { printf 'replacement-owner-timeout\\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 81; }\n"
                "      wait_watcher_lost || exit 80\n"
                "      exit 0\n"
                "      ;;\n"
                "    *) printf 'invalid-mode\n' > \"$FAKE_RUNNER_RESULT_FILE\"; exit 78 ;;\n"
                "  esac\n"
                "fi\n"
                "if [[ \"${1:-}\" == \"-\" && \"${2:-}\" == \"127.0.0.1\" && \"${3:-}\" == \"39999\" ]]; then exit 0; fi\n"
                "exec \"$real_python\" \"$@\"\n",
                encoding="utf-8",
            )
            fake_python.chmod(0o755)
            (fake_bin / "docker").write_text(
                "#!/usr/bin/env bash\n"
                "if [[ \"$*\" == *\" port redis 6379\" ]]; then printf '127.0.0.1:39999\\n'; fi\n"
                "exit 0\n",
                encoding="utf-8",
            )
            (fake_bin / "docker").chmod(0o755)
            fake_server = fake_bin / "fake-bong-server"
            fake_server.write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                f"real_python={shlex.quote(real_python)}\n"
                f"printf '%s\\n' \"$$\" > {shlex.quote(str(server_pid_file))}\n"
                "\"$real_python\" -u -c 'import socket, sys\ns = socket.socket()\ns.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\ns.bind((\"127.0.0.1\", int(sys.argv[1])))\ns.listen()\nwhile True:\n    conn, _ = s.accept()\n    conn.close()' \"$BOT_E2E_PORT\" &\n"
                "listener=$!\n"
                f"printf '%s\\n' \"$listener\" > {shlex.quote(str(listener_pid_file))}\n"
                "cleanup_listener() { kill \"$listener\" 2>/dev/null || true; wait \"$listener\" 2>/dev/null || true; exit 0; }\n"
                "trap cleanup_listener TERM INT\n"
                "for _ in $(seq 1 100); do\n"
                "  \"$real_python\" - \"$BOT_E2E_PORT\" <<'PY' && break\n"
                "import socket, sys\n"
                "with socket.create_connection((\"127.0.0.1\", int(sys.argv[1])), timeout=0.1):\n"
                "    pass\n"
                "PY\n"
                "  sleep 0.01\n"
                "done\n"
                "if [ \"${BOT_E2E_FALLBACK_MODE:-0}\" = 1 ]; then\n"
                "  printf '\\033[2m%s\\033[0m \\033[32m INFO\\033[0m %s\\n' '2026-08-11T23:25:37.123456Z' '[bong][world] BOT_FALLBACK_FLAT_READY anchors=3 chunks=1530 view_distance_chunks=4'\n"
                "else\n"
                "  printf '%s\\n' \"2026-08-11T23:25:37.123456Z  INFO [bong][world] BOT_RASTER_FIXTURE_READY manifest=$BONG_TERRAIN_RASTER_PATH token=$BOT_E2E_AMBIENT_FIXTURE_TOKEN\"\n"
                "fi\n"
                "while true; do sleep 1; done\n",
                encoding="utf-8",
            )
            fake_server.chmod(0o755)
            (fake_bin / "cargo").write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "test \"$1\" = build\n"
                "profile=debug\n"
                "for arg in \"$@\"; do test \"$arg\" != --release || profile=release; done\n"
                "mkdir -p \"$CARGO_TARGET_DIR/$profile\"\n"
                f"cp {shlex.quote(str(fake_server))} \"$CARGO_TARGET_DIR/$profile/bong-server\"\n",
                encoding="utf-8",
            )
            (fake_bin / "cargo").chmod(0o755)
            listener_lookup = (
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "pid=''\n"
                "for candidate_file in \"$FAKE_REPLACEMENT_READY_FILE\" \"$FAKE_LISTENER_PID_FILE\"; do\n"
                "  if test -s \"$candidate_file\"; then\n"
                "    candidate=$(cat \"$candidate_file\")\n"
                "    if kill -0 \"$candidate\" 2>/dev/null; then pid=$candidate; break; fi\n"
                "  fi\n"
                "done\n"
                "test -n \"$pid\" || exit 1\n"
            )
            (fake_bin / "ss").write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "[ \"$#\" -eq 4 ] && [ \"$1\" = -4 ] && [ \"$2\" = -H ] "
                "&& [ \"$3\" = -ltnp ] && [ \"$4\" = \"sport = :$BOT_E2E_PORT\" ] || exit 64\n"
                + "\n".join(listener_lookup.splitlines()[2:])
                + "\nprintf 'LISTEN 0 128 127.0.0.1:%s 0.0.0.0:* users:((\\\"python3\\\",pid=%s,fd=3))\\n' \"$BOT_E2E_PORT\" \"$pid\"\n",
                encoding="utf-8",
            )
            (fake_bin / "ss").chmod(0o755)
            (fake_bin / "lsof").write_text(
                "#!/usr/bin/env bash\n"
                "set -euo pipefail\n"
                "[ \"$#\" -eq 4 ] && [ \"$1\" = -nP ] "
                "&& [ \"$2\" = \"-iTCP:$BOT_E2E_PORT\" ] "
                "&& [ \"$3\" = -sTCP:LISTEN ] && [ \"$4\" = -Fp ] || exit 64\n"
                + "\n".join(listener_lookup.splitlines()[2:])
                + "\nprintf 'p%s\\n' \"$pid\"\n",
                encoding="utf-8",
            )
            (fake_bin / "lsof").chmod(0o755)
            if tee_exit is not None:
                (fake_bin / "tee").write_text(
                    f"#!/usr/bin/env bash\nexit {tee_exit}\n", encoding="utf-8"
                )
                (fake_bin / "tee").chmod(0o755)

            # Each bot-e2e invocation owns a private evidence namespace. The shell harness
            # exports this path to the pre-flight tests, so concurrent checkouts cannot
            # discover or delete one another's runtime-watch state.
            evidence_root = root / ".sisyphus/evidence/bot-e2e" / f"test-{uuid.uuid4().hex}"
            env = os.environ.copy()
            # Exercise isolation against a hostile ambient world override. The
            # fixture must remove it before entering dedicated fallback mode.
            env["BONG_WORLD_PATH"] = str(temp / "ambient-world-must-not-leak")
            for name in (
                "BOT_E2E_AMBIENT_FIXTURE_MODE", "BOT_E2E_REUSE", "BOT_E2E_HOST",
                "BOT_E2E_PORT", "BOT_E2E_FALLBACK_MODE",
                "BONG_TERRAIN_RASTER_PATH", "BONG_WORLD_PATH",
                "BONG_SPIRITWOOD_HARVESTED_PATH",
                "BONG_E2E_PREBUILT_SERVER_MANIFEST",
                "REDIS_URL", "BOT_E2E_AMBIENT_FIXTURE_OWNED",
                "BOT_E2E_FALLBACK_OWNED",
            ):
                env.pop(name, None)
            env.update(
                {
                    (
                        "BOT_E2E_FALLBACK_MODE"
                        if fallback_mode
                        else "BOT_E2E_AMBIENT_FIXTURE_MODE"
                    ): "1",
                    "BOT_E2E_RUN_TAG": "ci" if fallback_mode else "unit-test",
                    "BOT_E2E_PORT": str(port),
                    "FAKE_RUNNER_MODE": runner_mode,
                    "FAKE_RUNNER_EXIT": str(runner_exit),
                    "FAKE_SERVER_PID_FILE": str(server_pid_file),
                    "FAKE_LISTENER_PID_FILE": str(listener_pid_file),
                    "FAKE_EVIDENCE_ROOT": str(evidence_root),
                    "BOT_E2E_EVIDENCE_ROOT": str(evidence_root),
                    "FAKE_RUNNER_RESULT_FILE": str(runner_result_file),
                    "FAKE_REPLACEMENT_READY_FILE": str(replacement_ready_file),
                    "BOT_E2E_WATCH_STATUS_EVIDENCE_PATH": str(watcher_status_file),
                }
            )
            env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
            env["CARGO_TARGET_DIR"] = str(temp / "target")
            env["BONG_BUILD_TOKEN_TEST_MODE"] = "1"
            env["BONG_BUILD_TOKEN_DIR"] = str(temp / "build-token")
            evidence_before = set(evidence_root.glob("run.*")) if evidence_root.exists() else set()
            runner_output = ""
            runner_result = ""
            watcher_status = ""
            process: subprocess.Popen[str] | None = None
            try:
                process = subprocess.Popen(
                    ["bash", "scripts/bot-e2e.sh"],
                    cwd=root,
                    env=env,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    start_new_session=True,
                )
                try:
                    stdout, stderr = process.communicate(timeout=30)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        stdout, stderr = process.communicate(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        stdout, stderr = process.communicate(timeout=5)
                    evidence_after = (
                        set(evidence_root.glob("run.*")) if evidence_root.exists() else set()
                    )
                    server_logs = []
                    for evidence_dir in sorted(evidence_after - evidence_before):
                        server_log = evidence_dir / "server.log"
                        if server_log.exists():
                            server_logs.append(server_log.read_text(encoding="utf-8"))
                    self.fail(
                        "bot-e2e fixture exceeded 30s and its process group was stopped; "
                        f"stdout={stdout!r}; stderr={stderr!r}; "
                        f"runner_args={runner_args_file.read_text(encoding='utf-8') if runner_args_file.exists() else 'missing'!r}; "
                        f"server_pid={server_pid_file.read_text(encoding='utf-8') if server_pid_file.exists() else 'missing'!r}; "
                        f"listener_pid={listener_pid_file.read_text(encoding='utf-8') if listener_pid_file.exists() else 'missing'!r}; "
                        f"server_logs={server_logs!r}"
                    )
                result = subprocess.CompletedProcess(
                    process.args, process.returncode, stdout, stderr
                )
                runner_output = (
                    runner_log.read_text(encoding="utf-8") if runner_log.exists() else ""
                )
                runner_result = (
                    runner_result_file.read_text(encoding="utf-8")
                    if runner_result_file.exists()
                    else ""
                )
                watcher_status = (
                    watcher_status_file.read_text(encoding="utf-8")
                    if watcher_status_file.exists()
                    else ""
                )
                if fallback_mode:
                    self.assertTrue(
                        runner_args_file.exists(),
                        "fallback harness 未进入 scenario runner："
                        f"returncode={result.returncode} stdout={result.stdout!r} "
                        f"stderr={result.stderr!r}",
                    )
                    self.assertEqual(
                        runner_args_file.read_text(encoding="utf-8").strip(),
                        f"{root / 'scripts/bot/run_scenarios.py'} --scenario terrain_join_chunk_delivery",
                    )
            finally:
                if process is not None and process.poll() is None:
                    os.killpg(process.pid, signal.SIGTERM)
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        os.killpg(process.pid, signal.SIGKILL)
                        process.wait(timeout=5)
                for pid_file in (server_pid_file, listener_pid_file, replacement_pid_file):
                    if not pid_file.exists():
                        continue
                    pid = int(pid_file.read_text(encoding="utf-8").strip())
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        continue
                    deadline = time.monotonic() + 2
                    while time.monotonic() < deadline:
                        try:
                            os.kill(pid, 0)
                        except ProcessLookupError:
                            break
                        time.sleep(0.01)
                    else:
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                if evidence_root.exists():
                    for evidence_dir in set(evidence_root.glob("run.*")) - evidence_before:
                        shutil.rmtree(evidence_dir, ignore_errors=True)
            return result, runner_output, runner_result, watcher_status

    @staticmethod
    def _unused_local_port() -> int:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("127.0.0.1", 0))
            return int(sock.getsockname()[1])

    def test_owned_fixture_runtime_watcher_accepts_successful_owned_runner(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("success")
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(runner_output.strip(), "success")
        self.assertEqual(runner_result.strip(), "runner-complete")
        self.assertEqual(watcher_status.strip(), "complete")

    def test_fallback_runtime_accepts_realistic_tracing_readiness_everywhere(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("success", fallback_mode=True)
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(runner_output.strip(), "success")
        self.assertEqual(runner_result.strip(), "runner-complete")
        self.assertEqual(watcher_status.strip(), "complete")

    def test_owned_fixture_runtime_watcher_rejects_server_exit_during_runner(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("kill-server")
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(runner_output.strip(), "kill-server")
        self.assertEqual(runner_result.strip(), "watcher-lost-observed")
        self.assertEqual(watcher_status.strip(), "lost")
        self.assertIn("失去端口 ownership", result.stderr)

    def test_owned_fixture_runtime_watcher_rejects_replacement_listener(self):
        result, runner_output, runner_result, watcher_status = (
            self._run_owned_fixture_runtime_case("replace-listener")
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(runner_output.strip(), "replace-listener")
        self.assertEqual(runner_result.strip(), "watcher-lost-observed")
        self.assertEqual(watcher_status.strip(), "lost")
        self.assertIn("失去端口 ownership", result.stderr)

    def test_owned_fixture_runtime_preserves_runner_then_tee_failure_priority(self):
        for runner_exit, tee_exit, expected in ((7, None, 7), (7, 9, 7), (0, 9, 9)):
            with self.subTest(runner_exit=runner_exit, tee_exit=tee_exit):
                mode = "runner-fail" if runner_exit else "success"
                result, runner_output, runner_result, watcher_status = (
                    self._run_owned_fixture_runtime_case(
                        mode, runner_exit=runner_exit, tee_exit=tee_exit
                    )
                )
                self.assertEqual(result.returncode, expected, result.stderr)
                self.assertTrue(runner_output.strip())
                self.assertEqual(
                    runner_result.strip(),
                    "runner-failed" if runner_exit else "runner-complete",
                )
                self.assertEqual(watcher_status.strip(), "complete")


class FallbackScenarioPinTest(unittest.TestCase):
    """finding 6：CI 场景钉必须由 zones.json 权威数据 + 生产选择数学复现。

    EXPECTED_CI_CLUSTERS 是场景的验收定义，但钉本身必须能由权威配置独立推导：
    任何一端漂移（改锚点坐标/半径/权重、改 FNV 种子串、改 cluster 语义）都撞红。
    """

    @classmethod
    def setUpClass(cls) -> None:
        root = pathlib.Path(__file__).parents[2]
        with (root / "server/zones.json").open(encoding="utf-8") as fh:
            zones = json.load(fh)
        cls.spawn_zone = next(
            zone for zone in zones["zones"] if zone["name"] == "spawn"
        )
        cls.anchors = cls.spawn_zone["spawn_distribution"]

    def _config_anchor(self, x: float, z: float) -> dict:
        for anchor in self.anchors:
            if (
                abs(anchor["anchor"][0] - x) < 1e-6
                and abs(anchor["anchor"][2] - z) < 1e-6
            ):
                return anchor
        raise AssertionError(f"zones.json 没有 spawn_distribution 锚点 ({x},{z})")

    def test_pins_are_bijective_with_zones_json_anchors(self):
        self.assertEqual(
            len(EXPECTED_CI_CLUSTERS),
            len(self.anchors),
            "每个 CI pin 必须对应一个配置锚点，且数量必须一致",
        )
        pinned = set()
        for tag, (anchor, radius, cluster) in EXPECTED_CI_CLUSTERS.items():
            config = self._config_anchor(anchor[0], anchor[1])
            self.assertAlmostEqual(
                config["radius"],
                radius,
                places=6,
                msg=f"tag={tag} 的 pin radius 必须等于 zones.json 权威半径",
            )
            pinned.add((config["anchor"][0], config["anchor"][2]))
        for anchor in self.anchors:
            self.assertIn(
                (anchor["anchor"][0], anchor["anchor"][2]),
                pinned,
                "配置的每个出生锚点都必须被某个 CI pin 覆盖",
            )

    def _rust_fnv1a(self, seed: str) -> int:
        hash_value = 0xCBF29CE484222325
        for byte in f"InitialLogin:{seed}".encode("utf-8"):
            hash_value ^= byte
            hash_value = (hash_value * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
        return hash_value

    @staticmethod
    def _rotl(value: int, shift: int) -> int:
        return ((value << shift) | (value >> (64 - shift))) & 0xFFFFFFFFFFFFFFFF

    def _mirror_select(self, username: str) -> tuple[float, float]:
        """镜像 spawn_selector::select 的生产数学：FNV-1a → 加权随机 → 圆盘采样 → 钳制。"""
        hash_value = self._rust_fnv1a(username)
        total = sum(anchor["weight"] for anchor in self.anchors)
        pick = hash_value % total
        selected = None
        for anchor in self.anchors:
            if pick < anchor["weight"]:
                selected = anchor
                break
            pick -= anchor["weight"]
        assert selected is not None

        radius_bits = self._rotl(hash_value, 17) & 0xFFFF
        angle_bits = self._rotl(hash_value, 41) & 0xFFFF
        radius = selected["radius"] * math.sqrt(radius_bits / 65535.0)
        angle = (angle_bits / 65535.0) * 2.0 * math.pi
        x = selected["anchor"][0] + radius * math.cos(angle)
        z = selected["anchor"][2] + radius * math.sin(angle)

        bounds_min, bounds_max = self.spawn_zone["aabb"]["min"], self.spawn_zone["aabb"]["max"]
        x = min(max(x, bounds_min[0]), bounds_max[0])
        z = min(max(z, bounds_min[2]), bounds_max[2])
        blocked = [
            (tile[0], tile[1])
            for tile in self.spawn_zone.get("blocked_tiles", [])
        ]
        if (math.floor(x), math.floor(z)) in blocked:
            x = min(max(selected["anchor"][0], bounds_min[0]), bounds_max[0])
            z = min(max(selected["anchor"][2], bounds_min[2]), bounds_max[2])
        return x, z

    def test_ci_tags_mirror_production_selection_into_pinned_clusters(self):
        expected_chunks = {}
        for tag in ("J1", "J2", "FC"):
            username = f"Bci{tag}"
            x, z = self._mirror_select(username)
            # 走场景自己的验收函数（raise BotAssertionError），钉/半径/簇映射全部由它判定
            _assert_expected_cluster("ci", tag, (x, z))
            chunk = (math.floor(x / 16), math.floor(z / 16))
            self.assertNotIn(
                chunk,
                expected_chunks.values(),
                f"B{username} 必须命中与既有 tag 不同的出生 chunk",
            )
            expected_chunks[tag] = chunk
        self.assertEqual(len(expected_chunks), 3)

        # 同名玩家重连契约（#846 原始触发面）：同 seed 复算必须逐位稳定。
        for tag in ("J1", "J2", "FC"):
            username = f"Bci{tag}"
            self.assertEqual(
                self._mirror_select(username),
                self._mirror_select(username),
                f"B{username} 重连复算必须稳定落在同一出生点",
            )


class LawEngineSmokeHarnessContractTest(unittest.TestCase):
    def test_server_binary_launches_from_server_working_directory(self):
        source = (
            pathlib.Path(__file__).parents[2] / "scripts/smoke-law-engine.sh"
        ).read_text(encoding="utf-8")
        launch_start = source.index('echo "[run][server-start] timeout 20s')
        launch_end = source.index('echo "[server-start] exit=', launch_start)
        launch = source[launch_start:launch_end]

        self.assertIn('cd "$ROOT/server"', launch)
        self.assertLess(
            launch.index('cd "$ROOT/server"'),
            launch.index('timeout 20s "$server_binary"'),
            "law-engine server 必须先进入 server/ 再启动，避免相对 data 路径污染仓库根目录",
        )


class NorthRiftPreviewHarnessContractTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.source = (
            pathlib.Path(__file__).parents[2] / "scripts/e2e-redis.sh"
        ).read_text(encoding="utf-8")

    def test_manifest_tracks_dedicated_server_and_bot_evidence(self):
        start = self.source.index("write_manifest() {")
        end = self.source.index("\n}\n\nfinalize_failure()", start)
        manifest_body = self.source[start:end]

        for evidence_var in ("NORTH_RIFT_SERVER_LOG", "NORTH_RIFT_BOT_LOG"):
            with self.subTest(evidence=evidence_var):
                self.assertIn(
                    f'"${evidence_var}"',
                    manifest_body,
                    f"Finish evidence manifest 必须收录 {evidence_var}",
                )

    def test_preview_env_is_scoped_after_tps_gate_and_around_one_scenario(self):
        phase_start = self.source.index('CURRENT_STAGE="north-rift-preview"')
        phase_end = self.source.index('CURRENT_STAGE="summary"', phase_start)
        phase = self.source[phase_start:phase_end]

        self.assertEqual(
            self.source.count('start_server_process_group "$NORTH_RIFT_SERVER_LOG" 1'),
            1,
            "preview mode 只能传给 dedicated north-rift server launch",
        )
        first_stop = phase.index("if ! stop_server;")
        preview_env = phase.index(
            'start_server_process_group "$NORTH_RIFT_SERVER_LOG" 1'
        )
        scenario = phase.index("--scenario terrain_north_rift_scorch_zone_identity")
        second_stop = phase.index("if ! stop_server;", first_stop + 1)
        self.assertLess(first_stop, preview_env, "必须先停普通 100 NPC server 再开 preview")
        self.assertLess(preview_env, scenario, "专用 server 激活 preview 后才能运行真实 bot")
        self.assertLess(scenario, second_stop, "唯一场景结束后必须立即 stop_server")
        self.assertIn("BOT_E2E_NORTH_RIFT_PREVIEW=1", phase)
        self.assertIn(
            'BONG_ROGUE_SEED_COUNT="$([ "$preview_mode" -eq 1 ]',
            self.source,
            "dedicated preview launch 必须显式禁用 rogue seed",
        )

    def test_stop_server_delegates_shared_lifecycle_and_cleanup_fails_closed(self):
        start = self.source.index("stop_server() {")
        end = self.source.index("\n}\n\ncleanup()", start)
        stop_body = self.source[start:end]
        helper_branch = stop_body.index(
            "if bong_server_stop_owned_process_group_and_release_port"
        )
        empty_pid_port_check = stop_body.index(
            "bong_server_confirm_port_released 25565", helper_branch
        )
        clear_pid = stop_body.index('SERVER_PID=""', helper_branch)
        success_return = stop_body.index("return 0", clear_pid)
        capture_status = stop_body.index("stop_status=$?", success_return)
        failure_return = stop_body.index('return "$stop_status"', capture_status)
        branch_end = stop_body.index("\n  fi", failure_return)

        self.assertLess(
            helper_branch,
            clear_pid,
            "e2e stop_server 必须先由共享 helper 确认停树和端口释放，才能清空 PID",
        )
        self.assertLess(clear_pid, success_return)
        self.assertLess(success_return, capture_status)
        self.assertLess(capture_status, failure_return)
        self.assertLess(failure_return, branch_end)
        self.assertIn(
            'return "$stop_status"',
            stop_body,
            "共享 helper 的 forced/uncertain 状态必须原样传播，不能压平为普通失败",
        )
        self.assertIn(
            'if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then',
            stop_body,
            "只有 READY persistence cleanup 的空 PID 分支才必须获取新端口证据",
        )
        self.assertLess(
            stop_body.index('if [ "$PERSISTENCE_STASH_READY" -eq 1 ]; then'),
            empty_pid_port_check,
            "空 PID 的端口探测必须受 READY restore gate 约束",
        )
        self.assertEqual(
            stop_body.count('SERVER_PID=""'),
            1,
            "SERVER_PID 只能在共享 helper 成功分支清空一次",
        )
        self.assertEqual(
            stop_body.count('SERVER_PGID=""'),
            1,
            "SERVER_PGID 只能在整组确认退出后清空一次",
        )
        for authority_var, local_name in (
            ("SERVER_OWNER_STARTTIME", "owner_starttime"),
            ("SERVER_OWNER_EXECUTABLE_IDENTITY", "owner_executable_identity"),
        ):
            with self.subTest(authority=authority_var):
                self.assertIn(
                    f'local {local_name}="${authority_var}"',
                    stop_body,
                    "stop_server 必须把 pinned supervisor 身份传给共享 helper",
                )
                self.assertEqual(
                    stop_body.count(f'{authority_var}=""'),
                    1,
                    "supervisor authority 只能在整组确认退出后清空",
                )
        self.assertIn('if [ "$SERVER_AUTHORITY_UNCERTAIN" -ne 0 ]; then', stop_body)
        self.assertLess(
            stop_body.index('if [ "$SERVER_AUTHORITY_UNCERTAIN" -ne 0 ]; then'),
            helper_branch,
            "authority 尚未完整固定时必须先 fail closed，不能扫描裸 PGID",
        )
        self.assertNotIn("authority_path=", self.source)
        self.assertNotIn("setsid --fork", self.source)
        self.assertIn("bong-process-group-supervisor.py", self.source)
        for legacy_detail in (
            'kill_tree "$pid"',
            'wait "$pid"',
            "port_open 25565",
        ):
            with self.subTest(legacy_detail=legacy_detail):
                self.assertNotIn(
                    legacy_detail,
                    stop_body,
                    "stop_server 不得重新内联共享 lifecycle helper 的停树/等待/端口逻辑",
                )

        cleanup_start = self.source.index("cleanup() {")
        cleanup_end = self.source.index("\n}\n\ntrap cleanup EXIT", cleanup_start)
        cleanup_body = self.source[cleanup_start:cleanup_end]
        unconfirmed = cleanup_body.index("STOP_SERVER_CONFIRMED=0")
        stop_if = cleanup_body.index("if stop_server; then", unconfirmed)
        confirmed = cleanup_body.index("STOP_SERVER_CONFIRMED=1", stop_if)
        stop_else = cleanup_body.index("\n  else", confirmed)
        stop_fi = cleanup_body.index("\n  fi", stop_else)
        finalize = cleanup_body.index(
            "bong_server_finalize_preview_persistence_after_stop", stop_fi
        )
        finalize_end = cleanup_body.index("; then", finalize)
        finalize_call = cleanup_body[finalize:finalize_end]

        self.assertLess(unconfirmed, stop_if)
        self.assertLess(
            stop_if,
            confirmed,
            "cleanup 只能在 stop_server 成功分支标记停服已确认",
        )
        self.assertLess(confirmed, stop_else)
        self.assertEqual(
            cleanup_body.count("STOP_SERVER_CONFIRMED=1"),
            1,
            "cleanup 只能从 stop_server 成功分支产生唯一确认状态",
        )
        self.assertNotIn(
            "stop_server || true",
            cleanup_body,
            "停服失败不能被吞掉，否则可能错误恢复仍被 preview server 占用的 SQLite",
        )
        self.assertIn(
            '"$STOP_SERVER_CONFIRMED"',
            finalize_call,
            "cleanup 必须把停服确认结果交给共享 finalize helper 决定 restore 或 durable abort",
        )

    def test_preview_persistence_interval_holds_shared_lifecycle_lock(self):
        phase_start = self.source.index('CURRENT_STAGE="north-rift-preview"')
        phase_end = self.source.index('CURRENT_STAGE="summary"', phase_start)
        phase = self.source[phase_start:phase_end]
        fn_start = phase.index("run_north_rift_preview() {")
        fn_end = phase.index("\n}\n\nif ! bong_server_with_preview_persistence_lock", fn_start)
        preview_body = phase[fn_start:fn_end]
        lock_call = phase.index(
            "bong_server_with_preview_persistence_lock run_north_rift_preview",
            fn_end,
        )

        for required_step in (
            "if ! stop_server; then",
            "bong_server_confirm_port_released 25565",
            'bong_server_persistence_transaction_begin "$ROOT/server/data"',
            'bong_server_stash_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"',
            '--scenario terrain_north_rift_scorch_zone_identity',
            'bong_server_restore_persistence "$ROOT/server/data" "$NORTH_RIFT_DB_STASH"',
            "bong_server_persistence_transaction_complete",
        ):
            with self.subTest(step=required_step):
                self.assertIn(
                    required_step,
                    preview_body,
                    "preview lifecycle lock 的函数体必须覆盖完整 stash→run→restore 临界区",
                )
        self.assertGreater(
            lock_call,
            fn_end,
            "完整 preview transaction 必须经生产 start/reload 共用的 lifecycle lock 调用",
        )


class _FakeEvent:
    def __init__(self, t: float, kind: str, data: dict):
        self.t = t
        self.kind = kind
        self.data = data

    def __repr__(self) -> str:
        return f"_FakeEvent({self.t} {self.kind} {self.data})"


class _FakeBot:
    username = "Fake"

    def __init__(self, events: list[_FakeEvent]):
        self.events = events

    def events_of(self, kind: str) -> list[_FakeEvent]:
        return [event for event in self.events if event.kind == kind]

    def expect_server_data(self, payload_type: str, timeout: float = 5.0) -> _FakeEvent:
        return self.wait_for(
            lambda e: e.kind == "server_data" and e.data["payload_type"] == payload_type,
            timeout,
            f"server_data/{payload_type}",
        )

    def wait_for(self, predicate, timeout: float, description: str) -> _FakeEvent:
        for event in self.events:
            if predicate(event):
                return event
        raise AssertionError(f"未找到 {description}; events={self.events}")


class NetworkScenarioHelperTest(unittest.TestCase):
    def test_quickslot_rejection_defaults_to_empty_slot(self):
        payload = {
            "ack_request_id": "bad-item",
            "bind_accepted": False,
            "slots": [None] * 9,
        }
        bot = _FakeBot(
            [
                _FakeEvent(
                    1.0,
                    "server_data",
                    {"payload_type": "quickslot_config", "payload": payload},
                )
            ]
        )

        returned = _expect_bind_response(bot, "bad-item", False, 4)

        self.assertIs(returned, payload)

    def test_quickslot_rejection_can_require_preserved_binding(self):
        entry = {
            "item_id": "guyuan_pill",
            "display_name": "固元丹",
            "cast_duration_ms": 1500,
            "cooldown_ms": 3000,
            "icon_texture": "bong-client:textures/gui/items/pill.png",
        }
        payload = {
            "ack_request_id": "empty-item",
            "bind_accepted": False,
            "slots": [None, None, None, None, entry, None, None, None, None],
        }
        bot = _FakeBot(
            [
                _FakeEvent(
                    1.0,
                    "server_data",
                    {"payload_type": "quickslot_config", "payload": payload},
                )
            ]
        )

        _expect_bind_response(bot, "empty-item", False, 4, expected_entry=entry)

    def test_skill_config_snapshot_skips_stale_configuration(self):
        def snapshot(config: dict) -> _FakeEvent:
            return _FakeEvent(
                1.0 if config["meridian_id"] == "Lung" else 2.0,
                "server_data",
                {
                    "payload_type": "skill_config_snapshot",
                    "payload": {
                        "configs": [
                            {
                                "skill_id": "zhenmai.sever_chain",
                                "json_config": json.dumps(config),
                            }
                        ]
                    },
                },
            )

        expected = {"meridian_id": "LargeIntestine", "backfire_kind": "real_yuan"}
        bot = _FakeBot(
            [
                snapshot({"meridian_id": "Lung", "backfire_kind": "array"}),
                snapshot(expected),
            ]
        )

        event = _expect_config_snapshot(
            bot,
            0.0,
            lambda payload: json.loads(payload["configs"][0]["json_config"]) == expected,
        )

        self.assertEqual(event["configs"][0]["json_config"], json.dumps(expected))


class _ObservableLock:
    def __init__(self):
        self.held = False

    def __enter__(self):
        if self.held:
            raise AssertionError("fake lock unexpectedly re-entered")
        self.held = True
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        self.held = False


class _CommandFakeBot(_FakeBot):
    def __init__(
        self,
        events: list[_FakeEvent],
        pending: list[_FakeEvent],
        *,
        enforce_command_lock: bool = False,
    ):
        super().__init__(events)
        self._lock = _ObservableLock()
        self.enforce_command_lock = enforce_command_lock
        self.pending = list(pending)
        self.commands: list[str] = []

    def cmd(self, command: str) -> None:
        if self.enforce_command_lock and not self._lock.held:
            raise AssertionError("command dispatch must share the watermark lock")
        self.commands.append(command)

    def wait_for(self, predicate, timeout: float, description: str) -> _FakeEvent:
        while True:
            for event in self.events:
                if predicate(event):
                    return event
            if not self.pending:
                raise AssertionError(f"未找到 {description}; events={self.events}")
            self.events.append(self.pending.pop(0))


class LifecycleReviewContractTest(unittest.TestCase):
    def test_move_probe_rejects_partial_server_movement_even_when_local_mirror_reaches_target(self):
        class MoveProbeFakeBot:
            def __init__(self, authoritative_x: float):
                self.authoritative_x = authoritative_x
                self.events = [
                    _FakeEvent(
                        1.0,
                        "pos_look",
                        {"x": 100.0, "y": 72.0, "z": 8.0, "flags": 0},
                    ),
                    # Same Y as the /top response, but received before its chat
                    # watermark; this stale frame must not be consumed as the probe.
                    _FakeEvent(
                        1.9,
                        "pos_look",
                        {"x": 101.0, "y": 90.0, "z": 8.0, "flags": 0},
                    ),
                ]
                self.position = (100.0, 72.0, 8.0)

            def events_of(self, kind: str):
                return [event for event in self.events if event.kind == kind]

            def move_to(self, x: float, y: float, z: float, **_kwargs):
                # Model the local client mirror reaching x+5 even though the server
                # later reports only x+1 through the authoritative probe.
                self.position = (x, y, z)

            def cmd(self, command: str):
                assert command == "top"
                self.events.extend(
                    [
                        # This same-Y frame arrives after the old pre-command
                        # watermark but before the command feedback. A probe
                        # anchored before the chat would incorrectly consume it.
                        _FakeEvent(
                            1.9,
                            "pos_look",
                            {"x": 105.0, "y": 90.0, "z": 8.0, "flags": 0},
                        ),
                        _FakeEvent(2.0, "chat", {"text": "Teleported to top at Y=90."}),
                        _FakeEvent(
                            2.1,
                            "pos_look",
                            {
                                "x": self.authoritative_x,
                                "y": 90.0,
                                "z": 8.0,
                                "flags": 0,
                            },
                        ),
                    ]
                )

            def expect_chat(self, substring: str, timeout: float = 5.0):
                return self.wait_for(
                    lambda event: event.kind == "chat"
                    and substring in event.data["text"],
                    timeout,
                    f"chat/{substring}",
                )

            def wait_for(self, predicate, timeout: float, description: str):
                for event in self.events:
                    if predicate(event):
                        return event
                raise AssertionError(f"未找到 {description}; events={self.events}")

        with mock.patch(
            "bot.scenarios.network_lifecycle_abrupt_reconnect.time.sleep"
        ), self.assertRaisesRegex(AssertionError, "server 权威移动"):
            _move_and_record(MoveProbeFakeBot(authoritative_x=101.0))

        with mock.patch("bot.scenarios.network_lifecycle_abrupt_reconnect.time.sleep"):
            moved = _move_and_record(MoveProbeFakeBot(authoritative_x=105.0))
        self.assertEqual(
            moved,
            (105.0, 90.0, 8.0),
            "断连锚点必须使用 server PositionLook 的坐标，而不是客户端本地 mirror",
        )

    def test_restore_window_rejects_four_meter_reset_after_minimum_accepted_move(self):
        # A 4m authoritative move is exactly the lower edge accepted by the
        # 5m ±1m movement oracle. The restored-origin distance must nevertheless
        # exceed the narrower restore window, so this boundary cannot false-pass.
        partial_move = MOVE_DISTANCE - MOVE_VERIFY_TOLERANCE
        self.assertEqual(partial_move, 4.0)
        with self.assertRaisesRegex(AssertionError, "水平偏差"):
            _assert_restored_position(
                (100.0 + partial_move, 72.0, 8.0),
                (100.0, 90.0, 8.0),
            )
        _assert_restored_position(
            (100.0 + POSITION_TOLERANCE, 72.0, 8.0),
            (100.0, 90.0, 8.0),
        )

    def test_authoritative_position_requires_absolute_xyz_position_look(self):
        event = _FakeEvent(
            3.0,
            "pos_look",
            {"x": 105.0, "y": 72.0, "z": 8.0, "flags": 0},
        )
        self.assertEqual(
            _position_from_authoritative_event(event, "移动后"),
            (105.0, 72.0, 8.0),
        )
        with self.assertRaisesRegex(BotAssertionError, "绝对 XYZ"):
            _position_from_authoritative_event(
                _FakeEvent(3.0, "pos_look", {"x": 105.0, "y": 72.0, "z": 8.0, "flags": 1}),
                "移动后",
            )
        with self.assertRaisesRegex(BotAssertionError, "PositionLook"):
            _position_from_authoritative_event(
                _FakeEvent(3.0, "keepalive", {"id": 1}),
                "移动后",
            )

    def test_inactive_craft_session_requires_cleared_join_state(self):
        _assert_inactive_session_state(
            {
                "active": False,
                "recipe_id": None,
                "elapsed_ticks": 0,
                "total_ticks": 0,
                "completed_count": 0,
                "total_count": 0,
            },
            "最终重连",
        )
        with self.assertRaisesRegex(AssertionError, "active=false"):
            _assert_inactive_session_state(
                {
                    "active": True,
                    "recipe_id": "workbench.weapon.stone_knife",
                    "elapsed_ticks": 1,
                    "total_ticks": 10,
                    "completed_count": 0,
                    "total_count": 2,
                },
                "最终重连",
            )
        with self.assertRaisesRegex(AssertionError, "recipe_id"):
            _assert_inactive_session_state(
                {
                    "active": False,
                    "recipe_id": "stale",
                    "elapsed_ticks": 0,
                    "total_ticks": 0,
                    "completed_count": 0,
                    "total_count": 0,
                },
                "最终重连",
            )

class IdleTimeoutScenarioTest(unittest.TestCase):
    def test_idle_waits_share_one_absolute_deadline(self):
        class TimeoutCaptureBot:
            def __init__(self):
                self.timeouts = []

            def wait_for(self, _predicate, timeout: float, description: str):
                self.timeouts.append((timeout, description))
                raise AssertionError("测试 fake 不提供事件")

        bot = TimeoutCaptureBot()
        deadline = 130.0
        with mock.patch(
            "bot.scenarios.network_lifecycle_idle_timeout.time.monotonic",
            side_effect=[100.0, 112.0],
        ):
            with self.assertRaisesRegex(AssertionError, "测试 fake"):
                _wait_before_deadline(bot, lambda _event: True, deadline, "首个等待")
            with self.assertRaisesRegex(AssertionError, "测试 fake"):
                _wait_before_deadline(bot, lambda _event: True, deadline, "第二个等待")

        self.assertEqual(
            [timeout for timeout, _description in bot.timeouts],
            [30.0, 18.0],
            "第二个 idle 等待必须消费同一 absolute deadline 的剩余预算，不能重置为 30s",
        )

class _ReaderAlive:
    def __init__(self, alive: bool):
        self._alive = alive

    def is_alive(self) -> bool:
        return self._alive


class _ClockAdvancingList(list):
    """append 时把 bot 的模拟相对时钟推进到该事件时刻（max）。

    场景断言用 ``time.monotonic() - bot.t0`` 作相对时钟锚（与 ``event.t`` 同一帧）。
    fake 的 ``_now`` 是模拟相对时刻：初始 0，随事件 append 推进 —— 于是"发送后
    t>锚"的时序语义对 fake 可测（发送前锚 = 发送前最后事件时刻，发送中 append 的
    事件时刻会推进 probe_done_at，自然形成"探针期间 vs 探针完成后"的边界）。
    """

    def __init__(self, bot: "_RejectionFakeBot"):
        super().__init__()
        self._bot = bot

    def append(self, item: _FakeEvent) -> None:
        super().append(item)
        self._bot._advance_clock_to(getattr(item, "t", 0.0))

    def extend(self, items: list[_FakeEvent]) -> None:
        for item in items:
            self.append(item)


class _RejectionFakeBot(_FakeBot):
    """干净拒绝断言所需的最小 Bot 接口替身（无 socket）。

    - ``assert_alive`` 按 disconnect_reason / reader 存活判连接状态；
    - ``wait_for`` 在 events 里找不到时按顺序补充 ``pending`` 事件（模拟 server
      后续心跳 / 聊天响应），让"探针后新 keepalive 到达"这类时序可测；
    - ``t0`` 是模拟相对时钟（``time.monotonic() - t0 == self._now``），随事件
      append 推进 —— 让 ``time.monotonic() - bot.t0`` 锚与 ``event.t`` 同帧可测。
    """

    def __init__(
        self,
        events: list[_FakeEvent],
        *,
        pending: list[_FakeEvent] | None = None,
        disconnected: bool = False,
        reader_alive: bool = True,
    ):
        super().__init__([])
        self._now = 0.0
        self._clock_base = time.monotonic() - self._now
        self.events = _ClockAdvancingList(self)
        self.events.extend(events)
        self.pending = list(pending or [])
        self.disconnect_reason = "服务器主动断开" if disconnected else None
        self._reader_thread = _ReaderAlive(reader_alive)
        self.intents: list[dict] = []
        self.commands: list[str] = []

    @property
    def t0(self) -> float:
        """模拟相对时钟基座：让 ``time.monotonic() - t0 == self._now``。

        central-review 1993 #3：基座是**稳定存储值**（``_clock_base``），随事件推进
        由 ``_advance_clock_to`` 一次性重基，而不是每次读取都现场
        ``time.monotonic() - self._now``。旧实现按 property 每次读都取一次新
        monotonic：``time.monotonic() - bot.t0`` 先算左边再读 t0，第二次读比第一次
        晚 ~µs，``sent_at`` 被推到比当前 ``_now`` 略小，同一时刻（t==_now）的诱饵事件
        ``e.t > sent_at`` 成立被误收 —— 时序锚定失效、回归测试报出与因果窗口契约相反
        的结果。稳定基座下两种求值顺序都得到 ``sent_at == _now + 流逝``（≥_now），
        "t > 锚"语义与真实 bot（t0 创建时定死）一致。
        """
        return self._clock_base

    def _advance_clock_to(self, t: float) -> None:
        if t > self._now:
            self._now = t
            self._clock_base = time.monotonic() - self._now

    def rebase_clock_base(self) -> None:
        """把稳定基座重基到当前真实时刻，抵消自上次推进以来累积的真实流逝。

        模拟时钟只在事件 append 时推进（``_advance_clock_to``），真实时钟在场景的
        drain/sleep 期间照常流逝 —— 若不重基，``time.monotonic() - t0`` 会把真实流逝
        一起计入锚点，把「当前模拟时刻」抬到未来事件时刻之上（实跑：2s drain 后锚点
        ≈ 4.0，pending keepalive@3.0 反而不满足 ``t > 锚``）。锚点计算前重基一次，
        基座稳定（t0 在两次读取间不变）与「锚点 ≈ 当前模拟时刻 + 微抖动」两者兼得。
        """
        self._clock_base = time.monotonic() - self._now

    def intent(self, request: dict) -> None:
        self.intents.append(request)

    def cmd(self, command: str) -> None:
        self.commands.append(command)

    def expect_chat(self, substring: str, timeout: float = 5.0) -> _FakeEvent:
        return self.wait_for(
            lambda e: e.kind == "chat" and substring in e.data["text"],
            timeout,
            f"包含「{substring}」的聊天消息",
        )

    def assert_alive(self, context: str) -> None:
        if self.disconnect_reason is not None:
            raise BotAssertionError(
                f"期望连接保持（{context}），实际被服务器断开：{self.disconnect_reason!r}"
            )
        if not self._reader_thread.is_alive():
            raise BotAssertionError(f"期望连接保持（{context}），实际底层 socket 已断")

    def wait_for(self, predicate, timeout: float, description: str) -> _FakeEvent:
        while True:
            for event in self.events:
                if predicate(event):
                    return event
            if not self.pending:
                raise BotAssertionError(
                    f"未找到 {description}; events={self.events}"
                )
            self.events.append(self.pending.pop(0))


class RejectionHelperTest(unittest.TestCase):
    def test_freshness_realm_settle_excludes_late_sync_but_keeps_probe_oracle_strict(self):
        bot = _RejectionFakeBot([])
        clock = 0.0
        narration = _FakeEvent(
            1.5,
            "server_data",
            {"payload_type": "narration", "payload": {"text": "境界同步旁白"}},
        )
        narration_injected = False

        def monotonic() -> float:
            return clock

        def sleep(duration: float) -> None:
            nonlocal clock, narration_injected
            clock += duration
            if not narration_injected and clock >= narration.t:
                bot.events.append(narration)
                narration_injected = True

        with (
            mock.patch.object(freshness_probe_scenario.time, "monotonic", side_effect=monotonic),
            mock.patch.object(freshness_probe_scenario.time, "sleep", side_effect=sleep),
        ):
            anchor = freshness_probe_scenario._settle_realm_change(bot)

        self.assertTrue(narration_injected, "测试必须让迟到 narration 落入 realm 同步排空期")
        self.assertGreaterEqual(
            clock,
            narration.t + freshness_probe_scenario.SILENT_WINDOW,
            "请求锚点前必须观察完整静默窗，不能在 0.5s 假静默后提前发送",
        )
        self.assertGreaterEqual(anchor, narration.t, "realm set 的迟到 narration 必须早于请求锚点")

        bot.events.append(
            _FakeEvent(
                anchor + 0.1,
                "server_data",
                {"payload_type": "narration", "payload": {"text": "探针后旁白"}},
            )
        )
        with self.assertRaises(BotAssertionError):
            freshness_probe_scenario._scan_silent_violations(
                bot,
                anchor,
                "成功探针不得产生额外 narration",
                (),
            )

    def test_wait_keepalive_after_finds_later_keepalive(self):
        bot = _RejectionFakeBot([_FakeEvent(2.0, "keepalive", {"id": 7})])
        event = wait_keepalive_after(bot, after=1.0, timeout=1.0)
        self.assertEqual(event.data["id"], 7)

    def test_wait_keepalive_after_rejects_older_only_heartbeat(self):
        bot = _RejectionFakeBot([_FakeEvent(1.0, "keepalive", {"id": 7})])
        with self.assertRaises(BotAssertionError):
            wait_keepalive_after(bot, after=2.0, timeout=0.1)

    def test_fire_probes_keeps_connection_when_heartbeat_continues(self):
        # central-review 1993 #6：helper 现在要求**连续两个新 id** keepalive（第二个
        # 才是探针后生成的证据）—— 单条心跳的版本在此语义下必须失败。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "game_join", {})],
            pending=[
                _FakeEvent(3.0, "keepalive", {"id": 8}),
                _FakeEvent(4.0, "keepalive", {"id": 9}),
            ],
        )
        fired: list[str] = []
        fire_probes_and_keep_connection(
            bot, "测试", [("p1", lambda: fired.append("p1"))], settle_s=0.0
        )
        self.assertEqual(fired, ["p1"])
        self.assertEqual(bot.events[-1].kind, "keepalive")

    def test_fire_probes_fails_when_only_one_post_probe_keepalive(self):
        # central-review 1993 #6：单条「解码时刻晚于探针完成」的 keepalive 不构成
        # 「拒绝后持续心跳」证据 —— 它可能是探针前已生成、探针后才解码的在途心跳
        # （event.t 是客户端解码时刻，不是 server 生成边界）。helper 要求第二个新 id
        # 心跳（valence 只在收到对上一心跳的响应后才发下一个），没有则必须失败。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "game_join", {})],
            pending=[_FakeEvent(4.0, "keepalive", {"id": 9})],
        )
        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("p1", lambda: None)], settle_s=0.0
            )

    def test_fire_probes_fails_when_kicked(self):
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "game_join", {})], disconnected=True
        )
        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("p1", lambda: None)], settle_s=0.0
            )

    def test_fire_probes_fails_when_connection_forgotten(self):
        # 探针后无新 keepalive（server 单方面遗忘连接）→ 断言必须失败。
        bot = _RejectionFakeBot([_FakeEvent(2.0, "game_join", {})])
        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("p1", lambda: None)], settle_s=0.0
            )

    def test_fire_probes_fails_when_keepalive_predates_probe_completion(self):
        # 异步 reader 在 sent_at（探针前锚）之后、探针全部发出之前追加了一条周期性
        # keepalive：心跳断言必须以探针发出完成为锚，这条探针前 keepalive 不能冒充
        # 拒绝后的心跳 —— 若探针导致 server 遗忘连接且不再心跳，断言必须失败。
        bot = _RejectionFakeBot([_FakeEvent(2.0, "game_join", {})])

        def keepalive_during_probes():
            # 模拟 reader 在探针发送期间收到 keepalive（t=2.5 > sent_at=2.0，但
            # 早于探针发出完成时刻 probe_done_at）。
            bot.events.append(_FakeEvent(2.5, "keepalive", {"id": 7}))

        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("p1", keepalive_during_probes)], settle_s=0.0
            )

    def test_fire_probes_fails_when_probe_produces_gameplay_side_effect(self):
        # 探针触发了玩法反馈（server 在探针后回推 inventory_snapshot）→ 断言必须失败：
        # 说明坏请求被成功/部分处理了，而不是在副作用产生前被拒绝。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "game_join", {})],
            pending=[_FakeEvent(4.0, "keepalive", {"id": 9})],
        )

        def bad_probe():
            # 模拟 reader 在探针发出后收到玩法反馈（t > 窗口起点 sent_at=2.0）。
            bot.events.append(
                _FakeEvent(
                    3.0, "server_data", {"payload_type": "inventory_snapshot"}
                )
            )

        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("探针副作用", bad_probe)], settle_s=0.0
            )

    def test_fire_probes_fails_when_side_effect_arrives_during_keepalive_wait(self):
        # 拒绝后的心跳观察期（wait_keepalive_after，最多 ~25s）内到达的玩法副作用
        # 也必须计入拒绝判定：pending 依次出「玩法反馈 → 合法 keepalive」，若 helper
        # 只扫 settle 窗口就放行，这个 feedback 会被 keepalive 等待掩盖而假通过
        # （review finding：side-effect oracle 必须覆盖完整异步观察期）。
        # central-review 1993 #6：需要第三个 pending 事件（第二条新 id keepalive）让
        # helper 走完两个心跳等待 —— 否则失败发生在缺第二个心跳（测试语义漂移），
        # 而不是目标断言（心跳观察期内的玩法副作用被计入拒绝判定）。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "game_join", {})],
            pending=[
                _FakeEvent(3.0, "server_data", {"payload_type": "inventory_snapshot"}),
                _FakeEvent(4.0, "keepalive", {"id": 9}),
                _FakeEvent(5.0, "keepalive", {"id": 11}),
            ],
        )
        with self.assertRaises(BotAssertionError):
            fire_probes_and_keep_connection(
                bot, "测试", [("p1", lambda: None)], settle_s=0.0
            )

    def test_no_gameplay_side_effect_since_ignores_idle_traffic_but_flags_feedback(self):
        # 基础维护流量（keepalive/pos_look）不算副作用，只有玩法反馈通道才算。
        idle = _RejectionFakeBot(
            [
                _FakeEvent(2.0, "keepalive", {"id": 1}),
                _FakeEvent(2.5, "pos_look", {"x": 0, "y": 0, "z": 0}),
            ]
        )
        assert_no_gameplay_side_effect_since(idle, since_t=1.0, label="测试")
        feedback = _RejectionFakeBot(
            [
                _FakeEvent(2.0, "keepalive", {"id": 1}),
                _FakeEvent(
                    3.0,
                    "server_data",
                    {"payload_type": "inventory_snapshot", "payload": {"revision": 1}},
                ),
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(feedback, since_t=1.0, label="测试")

    def test_ambient_connection_sync_not_gameplay_side_effect(self):
        # 连接同步流量不算玩法副作用：解码的 status_snapshot（Changed<StatusEffects>
        # 驱动，探针场景的铺垫动作也会触发）与解码器读不动的 spawn 裸 payload（join
        # 突发）都不应触发窗口断言失败；只有响应式反馈（chat）才必须触发。
        ambient = _RejectionFakeBot(
            [
                _FakeEvent(1.5, "payload", {"channel": "bong:server_data", "data": b"spawn-bytes"}),
                _FakeEvent(3.8, "server_data", {"payload_type": "status_snapshot"}),
            ]
        )
        assert_no_gameplay_side_effect_since(ambient, since_t=1.0, label="测试")
        chat = _RejectionFakeBot([_FakeEvent(2.0, "chat", {"text": "已收到经脉目标"})])
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(chat, since_t=1.0, label="测试")

    def test_carrier_state_periodic_sync_is_not_gameplay_side_effect(self):
        # carrier_state 是无条件周期推送；拒绝场景的专用静默断言也必须复用
        # 共享 ambient 集合，否则同一环境同步会在不同场景产生不一致结果。
        bot = _RejectionFakeBot(
            [_FakeEvent(3.0, "server_data", {"payload_type": "carrier_state"})]
        )
        assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_heartbeat_periodic_sync_is_not_gameplay_side_effect(self):
        bot = _RejectionFakeBot(
            [_FakeEvent(3.0, "server_data", {"payload_type": "heartbeat"})]
        )
        assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_fire_probes_ignores_join_burst_ambient_traffic(self):
        # join 突发里既有无法解码的 spawn（裸 payload）也有解码的 status_snapshot，
        # 它们都落在探针窗口起点之前：drain 排干 + 类型排除后不误判成副作用。
        bot = _RejectionFakeBot(
            [
                _FakeEvent(1.5, "payload", {"channel": "bong:server_data", "data": b"spawn-bytes"}),
                _FakeEvent(2.0, "game_join", {}),
                _FakeEvent(3.0, "server_data", {"payload_type": "status_snapshot"}),
            ],
            pending=[
                _FakeEvent(4.0, "keepalive", {"id": 9}),
                _FakeEvent(5.0, "keepalive", {"id": 12}),
            ],
        )
        fired: list[str] = []
        fire_probes_and_keep_connection(
            bot, "测试", [("p1", lambda: fired.append("p1"))], settle_s=0.0
        )
        self.assertEqual(fired, ["p1"])

    def test_passive_vfx_ambient_and_any_responsive_vfx_in_window_flagged(self):
        # 被动/周期 vfx（灵气回充粒子 bong:cultivation_absorb，显式集合）不算副作用；
        # 响应式 vfx（如 combat_hit）只要落在探针窗口内就必须被标记 —— 窗口起点前
        # 见过同类型**不自证**它 ambient（同一 event_id 既可由 join 期间被动触发，
        # 也可由请求处理触发，不能按"见过"放行）。
        ambient = _RejectionFakeBot([_FakeEvent(6.5, "vfx_event", {"event_id": "bong:cultivation_absorb"})])
        assert_no_gameplay_side_effect_since(ambient, since_t=1.0, label="测试")
        seen_before_window = _RejectionFakeBot(
            [
                _FakeEvent(0.5, "vfx_event", {"event_id": "bong:combat_hit"}),
                _FakeEvent(3.0, "vfx_event", {"event_id": "bong:combat_hit"}),
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(seen_before_window, since_t=1.0, label="测试")
        fresh = _RejectionFakeBot([_FakeEvent(3.0, "vfx_event", {"event_id": "bong:combat_hit"})])
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(fresh, since_t=1.0, label="测试")


    def test_ambient_fauna_bite_in_probe_window_is_not_side_effect(self):
        # 回归锁：野生生物（实测噬元鼠）在探针窗口内咬 bot 会产生
        # combat_event(outgoing=false) + vfx_event(play_entity_anim)，二者都跟被拒
        # 请求无关。此前它们被判成"玩法副作用"，让干净拒绝场景在 CI 上随机变红
        # （同一 commit 连跑两次挂在不同场景）。
        bitten = _RejectionFakeBot(
            [
                _FakeEvent(
                    3.0,
                    "server_data",
                    {
                        "payload_type": "combat_event",
                        "payload": {
                            "v": 1,
                            "type": "combat_event",
                            "events": [{"kind": "qi_damage", "amount": 0.04, "outgoing": False}],
                        },
                    },
                ),
                _FakeEvent(
                    3.1,
                    "vfx_event",
                    {
                        "v": 1,
                        "type": "play_entity_anim",
                        "entity_id": 338,
                        "anim": "animation.bong.devour_rat.claw",
                        "duration_ticks": 12,
                    },
                ),
            ]
        )
        assert_no_gameplay_side_effect_since(bitten, since_t=1.0, label="测试")

    def test_full_rat_bite_triplet_is_ambient(self):
        # 一次环境咬击同时产生三件：combat_event(incoming) + play_entity_anim +
        # spawn_particle(bong:rat_bite_nip)。三件必须一起豁免——只补前两件的话
        # 第三件仍会让场景变红（CI 上实证过一次）。
        bitten = _RejectionFakeBot(
            [
                _FakeEvent(3.0, "server_data", {
                    "payload_type": "combat_event",
                    "payload": {"v": 1, "type": "combat_event",
                                "events": [{"kind": "qi_damage", "amount": 0.04, "outgoing": False}]},
                }),
                _FakeEvent(3.1, "vfx_event", {
                    "v": 1, "type": "play_entity_anim", "entity_id": 338,
                    "anim": "animation.bong.devour_rat.claw", "duration_ticks": 12,
                }),
                _FakeEvent(3.2, "vfx_event", {
                    "v": 1, "type": "spawn_particle", "event_id": "bong:rat_bite_nip",
                    "origin": [-187.7, 74.2, -178.1], "count": 4, "duration_ticks": 8,
                }),
            ]
        )
        assert_no_gameplay_side_effect_since(bitten, since_t=1.0, label="测试")

    def test_outgoing_damage_in_probe_window_is_still_a_side_effect(self):
        # 豁免必须只覆盖"bot 挨打"。bot **打出**伤害只可能来自被处理的请求，
        # 仍要撞红——否则这条豁免会把真 bug 一起放过。
        for events in (
            [{"kind": "hit", "amount": 3.0, "outgoing": True}],
            # 混合：挨打 + 打出，只要有一条 outgoing 就不算环境
            [
                {"kind": "qi_damage", "amount": 0.04, "outgoing": False},
                {"kind": "hit", "amount": 3.0, "outgoing": True},
            ],
        ):
            with self.subTest(events=events):
                bot = _RejectionFakeBot(
                    [
                        _FakeEvent(
                            3.0,
                            "server_data",
                            {
                                "payload_type": "combat_event",
                                "payload": {"v": 1, "type": "combat_event", "events": events},
                            },
                        )
                    ]
                )
                with self.assertRaises(BotAssertionError):
                    assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_malformed_combat_event_payload_stays_flagged(self):
        # 结构不符 / 空条目时无从归因 ⇒ 宁严勿松，仍算副作用。
        for payload in (
            None,
            {"v": 1, "type": "combat_event"},                      # 没有 events 字段
            {"v": 1, "type": "combat_event", "events": []},        # 空条目
            {"v": 1, "type": "combat_event", "events": [{"kind": "hit"}]},  # 缺 outgoing
            {"v": 1, "type": "combat_event", "events": "nope"},    # 类型不对
        ):
            with self.subTest(payload=payload):
                bot = _RejectionFakeBot(
                    [_FakeEvent(3.0, "server_data",
                                {"payload_type": "combat_event", "payload": payload})]
                )
                with self.assertRaises(BotAssertionError):
                    assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_other_vfx_types_are_not_exempted_by_the_fauna_anim_rule(self):
        # play_entity_anim 的豁免不能外溢：其它 vfx（无 event_id、或响应式 event_id）
        # 仍必须标记。
        for data in (
            {"v": 1, "type": "spawn_particles"},          # 非 play_entity_anim 且无 event_id
            {"event_id": "bong:combat_hit"},              # 响应式 vfx
        ):
            with self.subTest(data=data):
                bot = _RejectionFakeBot([_FakeEvent(3.0, "vfx_event", data)])
                with self.assertRaises(BotAssertionError):
                    assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_ambient_fauna_combat_before_window_does_not_mask_later_real_effect(self):
        # 环境挨打被豁免，但同窗口内的真副作用仍要撞红（豁免不是"整窗放行"）。
        bot = _RejectionFakeBot(
            [
                _FakeEvent(
                    2.0,
                    "server_data",
                    {
                        "payload_type": "combat_event",
                        "payload": {
                            "v": 1,
                            "type": "combat_event",
                            "events": [{"kind": "qi_damage", "amount": 0.04, "outgoing": False}],
                        },
                    },
                ),
                _FakeEvent(3.0, "chat", {"text": "某个坏请求的回执"}),
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_payload_type_seen_before_window_is_not_automatically_ambient(self):
        # 窗口起点前见过某 payload type **不自证**它是连接同步：inventory_snapshot
        # 既是 join 同步也是容器请求的 resync 响应 —— 若探针窗口内再次出现，说明某个
        # 坏请求被成功/部分处理并产生了响应，必须标记为副作用（拒绝路径断言点）。
        bot = _RejectionFakeBot(
            [
                _FakeEvent(0.5, "server_data", {"payload_type": "inventory_snapshot"}),
                _FakeEvent(3.0, "server_data", {"payload_type": "inventory_snapshot"}),
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_inventory_snapshot_without_baseline_always_flagged(self):
        # 无 baseline 时无从证明它是周期重发 ⇒ 一律标记（宁严勿松）。这锁定
        # inventory_snapshot **不在** ambient 集合（review finding 8）：payload type
        # 不能作为豁免依据 —— 它既是 join 同步/周期重发，也是容器请求的 resync 响应。
        bot = _RejectionFakeBot(
            [
                _FakeEvent(
                    3.0,
                    "server_data",
                    {"payload_type": "inventory_snapshot", "payload": {"revision": 7}},
                )
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_inventory_snapshot_fingerprint_equal_to_baseline_is_exempted(self):
        # 内容判别：带 baseline 时，指纹（revision + 全部内容字段）与基线一致的快照
        # 是周期 shelflife/Changed 驱动的无变更重发 —— 连接同步，豁免。若对一致快照
        # 也标记，周期重发会把真实场景全部误报成副作用。
        baseline = {
            "type": "inventory_snapshot",
            "revision": 7,
            "containers": [],
            "placed_items": [],
            "equipped": {},
            "hotbar": [],
            "bone_coins": 0,
        }
        bot = _RejectionFakeBot(
            [
                _FakeEvent(
                    3.0,
                    "server_data",
                    {"payload_type": "inventory_snapshot", "payload": dict(baseline)},
                )
            ]
        )
        assert_no_gameplay_side_effect_since(
            bot, since_t=1.0, label="测试", baseline_snapshot=baseline
        )

    def test_inventory_snapshot_fingerprint_changed_vs_baseline_is_flagged(self):
        # 内容判别：指纹与基线不同 ⇒ 请求引发的 mutation resync，必须标记副作用。
        baseline = {
            "type": "inventory_snapshot",
            "revision": 7,
            "containers": [],
            "placed_items": [],
            "equipped": {},
            "hotbar": [],
            "bone_coins": 0,
        }
        mutated = dict(baseline, revision=8)
        bot = _RejectionFakeBot(
            [
                _FakeEvent(
                    3.0,
                    "server_data",
                    {"payload_type": "inventory_snapshot", "payload": mutated},
                )
            ]
        )
        with self.assertRaises(BotAssertionError):
            assert_no_gameplay_side_effect_since(
                bot, since_t=1.0, label="测试", baseline_snapshot=baseline
            )

    def test_explicit_ambient_payload_type_in_window_is_not_side_effect(self):
        # 显式集合里的连接同步类型（derived_attrs_sync 为 Changed 驱动，见
        # derived_attrs_emit.rs）即使首次出现在窗口内也不算副作用 —— 只有显式集合
        # 才是 ambient 的依据，不是"窗口前见过"。
        bot = _RejectionFakeBot(
            [_FakeEvent(3.0, "server_data", {"payload_type": "derived_attrs_sync"})]
        )
        assert_no_gameplay_side_effect_since(bot, since_t=1.0, label="测试")

    def test_valid_request_ignores_earlier_chat_and_accepts_response_after_send(self):
        # 更早的同文广播（坏请求探针被错误接受时留下的副作用）已在 events 里，
        # 真实响应在 pending：时序锚定必须跳过诱饵，只接受发送时刻之后的响应。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "chat", {"text": "§a[修炼] 已收到经脉目标：肺经。"})],
            pending=[_FakeEvent(4.0, "chat", {"text": "§a[修炼] 已收到经脉目标：肺经。"})],
        )
        assert_valid_request_still_works(bot)
        self.assertEqual(
            bot.intents,
            [{"v": 1, "type": "set_meridian_target", "meridian": "lung"}],
        )
        self.assertEqual(bot.events[-1].t, 4.0)  # 返回的是发送后的响应，不是诱饵

    def test_valid_request_fails_when_only_earlier_chat_exists(self):
        # 只有一个更早的同文广播、发送后没有新响应 → 必须失败（旧广播不能冒充成功）。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "chat", {"text": "§a[修炼] 已收到经脉目标：肺经。"})]
        )
        with self.assertRaises(BotAssertionError):
            assert_valid_request_still_works(bot)

    def test_valid_request_fails_when_chat_never_comes(self):
        bot = _RejectionFakeBot([])
        with self.assertRaises(BotAssertionError):
            assert_valid_request_still_works(bot)

    def test_fake_clock_anchor_stable_across_read_orderings(self):
        # central-review 1993 #3：t0 是稳定存储基座。旧实现按 property 每次读都取新
        # monotonic，``time.monotonic() - bot.t0`` 先算左边再读 t0 时 sent_at 被推到
        # 比当前模拟时刻 _now 略小，同刻（t==_now）诱饵事件被 ``e.t > sent_at`` 误收。
        # 稳定基座下无论先求值哪边，锚点都必须 ≥ _now（"t > 锚"才不会被诱饵满足）。
        bot = _RejectionFakeBot(
            [_FakeEvent(2.0, "chat", {"text": "decoy"})],
        )
        self.assertEqual(bot.t0, bot.t0)  # 基座稳定：两次读取同值
        # monotonic 先求值（review 指出的求值顺序）：
        sent_at_left_first = time.monotonic() - bot.t0
        self.assertGreaterEqual(
            sent_at_left_first,
            2.0,
            "monotonic 先求值：锚点必须 ≥ 当前模拟时刻 _now=2.0",
        )
        # t0 先求值：
        t0_first = bot.t0
        sent_at_t0_first = time.monotonic() - t0_first
        self.assertGreaterEqual(
            sent_at_t0_first,
            2.0,
            "t0 先求值：锚点必须 ≥ 当前模拟时刻 _now=2.0",
        )

    def test_inventory_fingerprint_equal_when_zero_mutation_and_differs_on_revision(self):
        base = {
            "revision": 7,
            "containers": [],
            "placed_items": [],
            "equipped": {},
            "hotbar": [],
            "bone_coins": 0,
        }
        self.assertEqual(inventory_fingerprint(base), inventory_fingerprint(dict(base)))
        self.assertNotEqual(
            inventory_fingerprint(base), inventory_fingerprint(dict(base, revision=8))
        )

    def test_inventory_fingerprint_covers_each_content_mutation_field(self):
        # 拒绝 oracles 用 fingerprint 判零 mutation：若它只对 revision 敏感，一个「内容
        # 变了但 revision 没 bump」的实现（如只哈希 revision 的坏 fingerprint）会让
        # 零 mutation 断言假通过。逐字段独立变异，钉住 fingerprint 覆盖全部内容字段
        # （review finding：只测 revision 变化会漏掉其余五个 mutation 字段）。
        base = {
            "revision": 7,
            "containers": [],
            "placed_items": [],
            "equipped": {},
            "hotbar": [],
            "bone_coins": 0,
        }
        one_item = {"instance_id": 1, "template_id": "trade_crate", "count": 1}
        mutations = {
            # 每个字段换成非默认的「有内容」形态 —— 仅该字段与 base 不同。
            "containers": [{"container_id": "pack", "slots": [one_item]}],
            "placed_items": [{"entity_id": 5, "item": one_item}],
            "equipped": {"chest": one_item},
            "hotbar": [one_item],
            "bone_coins": 1,
        }
        for key, value in mutations.items():
            with self.subTest(mutation_field=key):
                mutated = dict(base)
                mutated[key] = value
                self.assertNotEqual(
                    inventory_fingerprint(base),
                    inventory_fingerprint(mutated),
                    f"fingerprint 必须对 {key} 内容变化敏感（即使 revision 未变）",
                )
        # 反向保证：指纹唯一性成立 —— 各字段单独变异得到的指纹互不相同，避免两个
        # 字段互相抵消（如坏实现把 containers 与 hotbar 拼进同一位）。
        fingerprints = {
            key: inventory_fingerprint(dict(base, **{key: value}))
            for key, value in mutations.items()
        }
        self.assertEqual(
            len(set(fingerprints.values())),
            len(mutations),
            f"各字段变异产生的指纹必须互异：{fingerprints}",
        )

    def test_inventory_snapshot_same_revision_content_mutation_is_flagged(self):
        # 完整内容指纹必须连到分类器（central-review 1993 #1）：仅当指纹（revision +
        # 全部内容字段）与基线一致才是周期重发、应豁免；内容变了但 revision 未 bump 的
        # 实现（如坏分类器只比 `snapshot["revision"] == baseline["revision"]`）会放过
        # 本快照，让「容器/放置物/装备/快捷栏/bone_coins 变了但没 bump revision」的
        # 拒绝场景报零副作用。上一测只把内容变异直接喂给 `inventory_fingerprint`，
        # 从未经 `assert_no_gameplay_side_effect_since` 走分类器 —— 逐字段独立变异并
        # 走分类器，把指纹契约钉到拒绝 oracle 的判定路径上。
        baseline = {
            "type": "inventory_snapshot",
            "revision": 7,
            "containers": [],
            "placed_items": [],
            "equipped": {},
            "hotbar": [],
            "bone_coins": 0,
        }
        one_item = {"instance_id": 1, "template_id": "trade_crate", "count": 1}
        mutations = {
            "containers": [{"container_id": "pack", "slots": [one_item]}],
            "placed_items": [{"entity_id": 5, "item": one_item}],
            "equipped": {"chest": one_item},
            "hotbar": [one_item],
            "bone_coins": 1,
        }
        for key, value in mutations.items():
            with self.subTest(mutation_field=key):
                mutated = dict(baseline)  # revision 保持 7 不变，仅该内容字段变异
                mutated[key] = value
                bot = _RejectionFakeBot(
                    [
                        _FakeEvent(
                            3.0,
                            "server_data",
                            {"payload_type": "inventory_snapshot", "payload": mutated},
                        )
                    ]
                )
                with self.assertRaises(BotAssertionError):
                    assert_no_gameplay_side_effect_since(
                        bot, since_t=1.0, label="测试", baseline_snapshot=baseline
                    )


def _stale_session_snapshot(
    revision: int, *, probe_count: int = 0, bone_coins: int = 0
) -> dict:
    placed_items = []
    if probe_count:
        placed_items.append(
            {
                "container_id": "body_pocket",
                "row": 0,
                "col": 0,
                "item": {
                    "instance_id": 99,
                    "item_id": stale_session_scenario.PROBE_ITEM_ID,
                    "stack_count": probe_count,
                },
            }
        )
    return {
        "type": "inventory_snapshot",
        "revision": revision,
        "containers": [],
        "placed_items": placed_items,
        "equipped": {},
        "hotbar": [],
        "bone_coins": bone_coins,
    }


def _stale_session_close_bot(probe_snapshot: dict) -> _RejectionFakeBot:
    pre = _stale_session_snapshot(7)
    return _RejectionFakeBot(
        [
            _FakeEvent(
                2.0,
                "server_data",
                {"payload_type": "inventory_snapshot", "payload": pre},
            )
        ],
        pending=[
            _FakeEvent(3.0, "chat", {"text": "§a[修炼] 已收到经脉目标：肺经。"}),
            _FakeEvent(
                4.0,
                "chat",
                {"text": f"[dev] gave {stale_session_scenario.PROBE_ITEM_ID} x1"},
            ),
            _FakeEvent(
                5.0,
                "server_data",
                {"payload_type": "inventory_snapshot", "payload": probe_snapshot},
            ),
        ],
    )


class StaleSessionScenarioTest(unittest.TestCase):
    def test_trade_crate_placement_stops_after_first_batch_hit(self):
        candidates = [(index, 64, 20) for index in range(10)]
        bot = mock.MagicMock()
        bot.t0 = 0.0
        bot.wait_for.return_value = _FakeEvent(
            2.0,
            "entity_spawn",
            {
                "entity_id": 1661,
                "type": stale_session_scenario.TRADE_CRATE_ENTITY_KIND,
                "x": 0.5,
                "y": 64.0,
                "z": 20.5,
            },
        )

        spawn, _sent_at = stale_session_scenario._place_trade_crate_until_marker(
            bot, candidates, item_instance_id=501
        )

        self.assertIsNotNone(spawn, "第一批命中 marker 时必须立即返回")
        self.assertEqual(
            bot.intent.call_count,
            stale_session_scenario.PLACEMENT_BATCH_SIZE,
            "第一批命中后不得继续向连接灌入后续候选请求",
        )
        bot.wait_for.assert_called_once()

    def test_trade_crate_placement_continues_after_batch_miss(self):
        candidates = [(index, 64, 20) for index in range(10)]
        bot = mock.MagicMock()
        bot.t0 = 0.0
        bot.wait_for.side_effect = [
            BotAssertionError("第一批无 marker"),
            _FakeEvent(
                2.0,
                "entity_spawn",
                {
                    "entity_id": 1661,
                    "type": stale_session_scenario.TRADE_CRATE_ENTITY_KIND,
                    "x": 8.5,
                    "y": 64.0,
                    "z": 20.5,
                },
            ),
        ]

        spawn, _sent_at = stale_session_scenario._place_trade_crate_until_marker(
            bot, candidates, item_instance_id=501
        )

        self.assertIsNotNone(spawn, "前批未命中时必须继续搜索下一批")
        self.assertEqual(bot.intent.call_count, len(candidates))
        self.assertEqual(bot.wait_for.call_count, 2)

    def test_trade_crate_placement_stops_when_shared_deadline_expires(self):
        candidates = [(index, 64, 20) for index in range(20)]
        bot = mock.MagicMock()
        bot.t0 = 0.0
        bot.wait_for.side_effect = BotAssertionError("批次无 marker")
        # start=0；第一批检查仍有预算，等待后时钟已到 5s，下一批不得重置 deadline。
        with mock.patch.object(
            stale_session_scenario.time,
            "monotonic",
            side_effect=[0.0, 0.0, 5.0],
        ):
            spawn, _sent_at = stale_session_scenario._place_trade_crate_until_marker(
                bot, candidates, item_instance_id=501
            )

        self.assertIsNone(spawn, "共享 deadline 到期后必须停止搜索")
        self.assertEqual(
            bot.intent.call_count,
            stale_session_scenario.PLACEMENT_BATCH_SIZE,
            "deadline 到期不得发送下一批候选请求",
        )
        bot.wait_for.assert_called_once()

    def test_trade_crate_spawn_predicate_requires_marker_type_and_candidate_position(self):
        candidates = [(10, 64, 20)]
        valid = _FakeEvent(
            2.0,
            "entity_spawn",
            {
                "entity_id": 1661,
                "type": stale_session_scenario.TRADE_CRATE_ENTITY_KIND,
                "x": 10.5,
                "y": 64.0,
                "z": 20.5,
            },
        )
        wrong_type = _FakeEvent(
            2.1,
            "entity_spawn",
            {"entity_id": 1651, "type": 165, "x": 10.5, "y": 64.0, "z": 20.5},
        )
        wrong_position = _FakeEvent(
            2.2,
            "entity_spawn",
            {
                "entity_id": 1662,
                "type": stale_session_scenario.TRADE_CRATE_ENTITY_KIND,
                "x": 11.5,
                "y": 64.0,
                "z": 20.5,
            },
        )

        self.assertTrue(
            stale_session_scenario._spawn_at_attempted(valid, candidates, sent_at=1.0),
            "候选格格心出现 type=166 的新 entity_spawn 才能作为货箱 marker",
        )
        self.assertFalse(
            stale_session_scenario._spawn_at_attempted(
                wrong_type, candidates, sent_at=1.0
            ),
            "坐标正确但类型错误的实体不得被当成 trade_crate marker",
        )
        self.assertFalse(
            stale_session_scenario._spawn_at_attempted(
                wrong_position, candidates, sent_at=1.0
            ),
            "类型正确但坐标不在本轮候选格的实体不得被当成 trade_crate marker",
        )

    def test_content_fingerprint_ignores_revision_but_detects_content(self):
        before = _stale_session_snapshot(7)
        after_give = _stale_session_snapshot(8)
        self.assertEqual(
            stale_session_scenario._inventory_content_fingerprint(before),
            stale_session_scenario._inventory_content_fingerprint(after_give),
            "close 已独立钉住 revision=R+1 后，内容指纹必须忽略该合法 revision 差异",
        )
        mutated = _stale_session_snapshot(8, bone_coins=1)
        self.assertNotEqual(
            stale_session_scenario._inventory_content_fingerprint(before),
            stale_session_scenario._inventory_content_fingerprint(mutated),
            "内容指纹必须继续覆盖真实背包 mutation",
        )

    def test_stale_close_accepts_exact_probe_revision_and_equal_content(self):
        bot = _stale_session_close_bot(
            _stale_session_snapshot(8, probe_count=1)
        )

        stale_session_scenario._assert_stale_close_rejected(bot, 41, "closed token")

        self.assertEqual(bot.commands, [f"give {stale_session_scenario.PROBE_ITEM_ID} 1"])
        self.assertEqual(
            bot.intents,
            [
                {"v": 1, "type": "external_container_close", "session_id": 41},
                {"v": 1, "type": "set_meridian_target", "meridian": "lung"},
            ],
            "helper 必须先发 stale close，再用合法请求建立处理屏障",
        )

    def test_stale_close_rejects_spurious_revision_bump(self):
        bot = _stale_session_close_bot(
            _stale_session_snapshot(9, probe_count=1)
        )
        with self.assertRaisesRegex(BotAssertionError, "revision 恰好 \\+1"):
            stale_session_scenario._assert_stale_close_rejected(
                bot, 41, "closed token"
            )

    def test_stale_close_rejects_content_mutation_after_probe_removed(self):
        bot = _stale_session_close_bot(
            _stale_session_snapshot(8, probe_count=1, bone_coins=1)
        )
        with self.assertRaisesRegex(BotAssertionError, "背包内容零 mutation"):
            stale_session_scenario._assert_stale_close_rejected(
                bot, 41, "closed token"
            )

    def test_run_reaches_all_stale_and_forged_paths(self):
        real_move = {
            "session_id": 41,
            "instance_id": 501,
            "from": {
                "kind": "container",
                "container_id": "ext_41",
                "row": 0,
                "col": 0,
            },
            "to": {
                "kind": "container",
                "container_id": "body_pocket",
                "row": 0,
                "col": 0,
            },
            "placed_pos": (1, 64, 1),
        }
        snapshot = _stale_session_snapshot(7)
        close_event = _FakeEvent(
            3.0,
            "server_data",
            {"payload_type": "loot_container_close", "payload": {"session_id": 41}},
        )
        bot = mock.MagicMock()
        bot.t0 = 0.0
        # MagicMock 保留 assert_* 名字给自身断言 API，场景的 Bot.assert_alive 必须显式安装。
        bot.assert_alive = mock.Mock()
        bot.expect_server_data.return_value = close_event
        context = mock.MagicMock()
        context.__enter__.return_value = bot
        context.__exit__.return_value = False
        env = mock.MagicMock()
        env.new_bot.return_value = context

        with (
            mock.patch.object(stale_session_scenario.time, "sleep"),
            mock.patch.object(
                stale_session_scenario, "_open_real_session", return_value=real_move
            ),
            mock.patch.object(
                stale_session_scenario, "_assert_stale_move_rejected_zero_mutation"
            ) as move_rejected,
            mock.patch.object(
                stale_session_scenario, "_assert_stale_close_rejected"
            ) as close_rejected,
            mock.patch.object(
                stale_session_scenario, "_teardown_placed_crate"
            ) as teardown,
            mock.patch(
                "bot.scenarios._inventory_helpers.latest_inventory_snapshot",
                return_value=snapshot,
            ),
            mock.patch("bot.scenarios._inventory_helpers.drain_inventory_snapshots"),
            mock.patch(
                "bot.scenarios._inventory_helpers.wait_inventory_snapshot_after",
                return_value=snapshot,
            ),
            mock.patch(
                "bot.scenarios._rejection_helpers.inventory_fingerprint",
                return_value="same",
            ),
            mock.patch(
                "bot.scenarios._rejection_helpers.fire_probes_and_keep_connection"
            ) as fire_probes,
            mock.patch(
                "bot.scenarios._rejection_helpers.assert_valid_request_still_works"
            ) as valid_request,
        ):
            stale_session_scenario.run(env)

        self.assertEqual(
            move_rejected.call_args_list,
            [
                mock.call(bot, 41, "replay 已关闭 session 的 move", real_move),
                mock.call(
                    bot,
                    stale_session_scenario.FORGED_SESSION_ID,
                    "stale move #1（forged token）",
                    real_move,
                ),
                mock.call(
                    bot,
                    stale_session_scenario.FORGED_SESSION_ID,
                    "stale move #2（重放 forged token）",
                    real_move,
                ),
            ],
            "真实 stale move 与两轮 forged move 必须全部执行",
        )
        self.assertEqual(
            close_rejected.call_args_list,
            [
                mock.call(bot, 41, "replay 已关闭 session 的 close"),
                mock.call(
                    bot,
                    stale_session_scenario.FORGED_SESSION_ID,
                    "stale close（forged token）",
                ),
            ],
            "真实 stale close 通过后必须继续执行 forged close",
        )
        fire_probes.assert_called_once()
        valid_request.assert_called_once_with(bot)
        teardown.assert_called_once_with(bot, (1, 64, 1), 501)


class _IntentFakeBot(_CommandFakeBot):
    def __init__(self, events: list[_FakeEvent], pending: list[_FakeEvent]):
        super().__init__(events, pending)
        self.intents: list[dict] = []

    def intent(self, payload: dict) -> None:
        self.intents.append(payload)
        if self.pending:
            self.events.append(self.pending.pop(0))


def _snapshot_event(t: float, revision: int, marker: str) -> _FakeEvent:
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "inventory_snapshot",
            "payload": {
                "type": "inventory_snapshot",
                "revision": revision,
                "marker": marker,
            },
        },
    )


class CoffinTeardownHelperTest(unittest.TestCase):
    @staticmethod
    def _spawn(t: float, entity_id: int, x: float, y: float, z: float) -> _FakeEvent:
        return _FakeEvent(
            t,
            "entity_spawn",
            {"entity_id": entity_id, "type": 160, "x": x, "y": y, "z": z},
        )

    def test_teardown_targets_matching_marker_and_waits_for_its_destroy(self):
        bot = _IntentFakeBot(
            [
                self._spawn(1.0, 41, 13.0, 64.0, -2.5),
                self._spawn(2.0, 42, 11.0, 64.0, -2.5),
            ],
            [_FakeEvent(3.0, "entities_destroy", {"entity_ids": [42, 99]})],
        )

        teardown_coffin(bot, (10, 64, -3), timeout=0.01)

        self.assertEqual(
            bot.intents,
            [{"type": "coffin_break", "v": 1, "x": 10, "y": 64, "z": -3}],
            "清场必须对实际 lower 坐标发送精确 coffin_break payload",
        )

    def test_teardown_rejects_destroy_for_another_marker(self):
        bot = _IntentFakeBot(
            [self._spawn(1.0, 42, 11.0, 64.0, -2.5)],
            [_FakeEvent(2.0, "entities_destroy", {"entity_ids": [99]})],
        )

        with self.assertRaisesRegex(AssertionError, "marker #42"):
            teardown_coffin(bot, (10, 64, -3), timeout=0.01)

        self.assertEqual(len(bot.intents), 1, "错误 destroy 证据不得阻止实际 break 请求发出")

    def test_teardown_does_not_break_when_expected_marker_was_never_observed(self):
        bot = _IntentFakeBot(
            [self._spawn(1.0, 41, 99.0, 64.0, -2.5)],
            [],
        )

        with self.assertRaisesRegex(AssertionError, "待清场"):
            teardown_coffin(bot, (10, 64, -3), timeout=0.01)

        self.assertEqual(bot.intents, [], "未锁定目标 marker 时不得盲目破坏其他棺材")


def _pill_snapshot_event(t: float, revision: int, count: int, qi: float) -> _FakeEvent:
    placed_items = []
    if count > 0:
        placed_items.append(
            {
                "container_id": "body_pocket",
                "row": 0,
                "col": 0,
                "item": {
                    "instance_id": 7,
                    "item_id": PILL_ID,
                    "stack_count": count,
                },
            }
        )
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "inventory_snapshot",
            "payload": {
                "type": "inventory_snapshot",
                "revision": revision,
                "placed_items": placed_items,
                "equipped": {},
                "hotbar": [],
                "qi_current": qi,
            },
        },
    )


def _player_state_event(t: float, qi: float, qi_max: float = 100.0) -> _FakeEvent:
    return _FakeEvent(
        t,
        "server_data",
        {
            "payload_type": "player_state",
            "payload": {
                "type": "player_state",
                "spirit_qi": qi,
                "spirit_qi_max": qi_max,
            },
        },
    )


class CombatSkillCastScenarioTest(unittest.TestCase):
    def test_authoritative_qi_state_accepts_zero_capacity_dev_fixture(self):
        authoritative = _player_state_event(2.0, 0.0, 0.0)
        bot = _FakeBot(
            [
                _FakeEvent(
                    1.0,
                    "chat",
                    {"text": "[dev] qi max 20.0 -> 0.0; current=0.0"},
                ),
                authoritative,
            ]
        )

        self.assertIs(
            _wait_authoritative_qi_state(bot, 1.0, 0.0, 0.0),
            authoritative,
            "零上限只用于 dev 拒绝夹具，仍必须以命令后的 typed player_state 为准",
        )

    def test_authoritative_qi_state_rejects_pre_command_snapshot(self):
        bot = _FakeBot(
            [
                _player_state_event(1.0, 0.0, 0.0),
                _player_state_event(2.0, 0.0, 20.0),
            ]
        )

        with self.assertRaisesRegex(AssertionError, "权威 player_state"):
            _wait_authoritative_qi_state(bot, 2.0, 0.0, 0.0)

    @staticmethod
    def _cast_sync(t: float, phase: str, outcome: str) -> _FakeEvent:
        return _FakeEvent(
            t,
            "server_data",
            {
                "payload_type": "cast_sync",
                "payload": {
                    "slot": 0,
                    "phase": phase,
                    "outcome": outcome,
                },
            },
        )

    def test_successful_cast_requires_complete_after_casting(self):
        casting = self._cast_sync(3.0, "casting", "none")
        complete = self._cast_sync(4.0, "complete", "completed")

        self.assertIs(
            _wait_successful_cast_sequence(_FakeBot([casting, complete]), 1.0),
            complete,
        )

    def test_successful_cast_accepts_same_timestamp_batch_in_event_order(self):
        casting = self._cast_sync(3.0, "casting", "none")
        complete = self._cast_sync(3.0, "complete", "completed")

        self.assertIs(
            _wait_successful_cast_sequence(_FakeBot([casting, complete]), 1.0),
            complete,
            "同一网络批次可共享时间戳，顺序应由事件列表中的 phase 转换确定",
        )

    def test_successful_cast_rejects_wrong_slot_or_outcome_identity(self):
        cases = (
            (self._cast_sync(2.0, "casting", "none"), "slot", 1),
            (self._cast_sync(2.0, "casting", "rejected"), "outcome", "rejected"),
            (self._cast_sync(3.0, "complete", "failed"), "outcome", "failed"),
            (self._cast_sync(3.0, "cancelled", "completed"), "phase", "cancelled"),
        )
        for invalid, field, value in cases:
            with self.subTest(field=field, value=value):
                invalid.data["payload"][field] = value
                events = (
                    [invalid, self._cast_sync(3.0, "complete", "completed")]
                    if invalid.data["payload"]["phase"] == "casting"
                    else [self._cast_sync(2.0, "casting", "none"), invalid]
                )
                expected_phase = (
                    "phase=casting"
                    if invalid.data["payload"]["phase"] == "casting"
                    else "phase=complete"
                )
                with self.assertRaisesRegex(AssertionError, expected_phase):
                    _wait_successful_cast_sequence(_FakeBot(events), 1.0)

    def test_successful_cast_rejects_complete_before_casting(self):
        bot = _FakeBot(
            [
                self._cast_sync(2.0, "complete", "completed"),
                self._cast_sync(3.0, "casting", "none"),
            ]
        )

        with self.assertRaisesRegex(AssertionError, "phase=complete"):
            _wait_successful_cast_sequence(bot, 1.0)

    @staticmethod
    def _audio_event(
        t: float,
        *,
        recipe_id: str = AUDIO_RECIPE_ID,
        flag: str = AUDIO_FLAG,
        channel: str = "bong:audio/play",
    ) -> _FakeEvent:
        return _FakeEvent(
            t,
            "payload",
            {
                "channel": channel,
                "data": json.dumps(
                    {"v": 1, "recipe_id": recipe_id, "flag": flag}
                ).encode("utf-8"),
            },
        )

    def test_dugu_audio_requires_exact_recipe_flag_channel_and_watermark(self):
        accepted = self._audio_event(2.0)
        self.assertTrue(_is_dugu_audio_play(accepted, 1.0))

        rejected = (
            self._audio_event(1.0),
            self._audio_event(2.0, recipe_id="dugu_needle_hiss"),
            self._audio_event(2.0, flag="dugu_infuse_poison"),
            self._audio_event(2.0, channel="bong:audio/stop"),
            _FakeEvent(
                2.0,
                "payload",
                {"channel": "bong:audio/play", "data": b'{"v":1,"recipe_id":'},
            ),
            _FakeEvent(
                2.0,
                "payload",
                {"channel": "bong:audio/play", "data": b"\xff"},
            ),
            _FakeEvent(
                2.0,
                "payload",
                {
                    "channel": "bong:audio/play",
                    "data": json.dumps(
                        {"v": 1, "recipe_id": AUDIO_RECIPE_ID}
                    ).encode("utf-8"),
                },
            ),
        )
        for event in rejected:
            with self.subTest(event=event):
                self.assertFalse(_is_dugu_audio_play(event, 1.0))

    def test_binding_feedback_requires_exact_dugu_icon(self):
        slot = {
            "kind": "skill",
            "skill_id": "dugu.shoot_needle",
            "icon_texture": SKILL_ICON,
        }
        event = _FakeEvent(
            2.0,
            "server_data",
            {"payload": {"slots": [slot]}},
        )
        _assert_binding_feedback(_FakeBot([]), event)

        event.data["payload"]["slots"][0]["icon_texture"] = "bong:wrong.png"
        with self.assertRaisesRegex(BotAssertionError, "icon_texture 漂移"):
            _assert_binding_feedback(_FakeBot([]), event)


class CultivationBreakthroughScenarioTest(unittest.TestCase):
    @staticmethod
    def _cinematic(
        t: float,
        phase: str,
        at_tick: int,
        *,
        actor_id: str = "offline:Break",
        realm_from: str = "Awaken",
        realm_to: str = "Induce",
        result: str = "success",
        interrupted: bool = False,
    ) -> _FakeEvent:
        return _FakeEvent(
            t,
            "server_data",
            {
                "payload_type": "breakthrough_cinematic",
                "payload": {
                    "type": "breakthrough_cinematic",
                    "actor_id": actor_id,
                    "phase": phase,
                    "phase_tick": 0,
                    "phase_duration_ticks": {
                        "prelude": 60,
                        "charge": 200,
                        "catalyze": 100,
                        "apex": 40,
                        "aftermath": 120,
                    }[phase],
                    "realm_from": realm_from,
                    "realm_to": realm_to,
                    "result": result,
                    "interrupted": interrupted,
                    "at_tick": at_tick,
                },
            },
        )

    def test_phase_timeout_covers_production_duration_at_gate_floor(self):
        for phase, duration_ticks in {
            "prelude": 60,
            "charge": 200,
            "catalyze": 100,
            "apex": 40,
            "aftermath": 120,
        }.items():
            with self.subTest(phase=phase):
                self.assertEqual(
                    _phase_timeout_seconds({"phase_duration_ticks": duration_ticks}),
                    duration_ticks / BREAKTHROUGH_MIN_GATE_TPS
                    + PHASE_TIMEOUT_MARGIN_SECONDS,
                )

        for invalid in (None, 0, -1, 2.5, True):
            with self.subTest(invalid=invalid):
                with self.assertRaises(BotAssertionError):
                    _phase_timeout_seconds({"phase_duration_ticks": invalid})

    def test_terminal_wait_requires_same_identity_and_ordered_aftermath(self):
        initial = self._cinematic(2.0, "prelude", 100)
        ignored = [
            self._cinematic(2.5, "charge", 160, actor_id="offline:Other"),
            self._cinematic(2.6, "charge", 160, realm_to="Condense"),
            self._cinematic(2.7, "charge", 160, result="failure"),
        ]
        phases = [
            self._cinematic(float(index + 3), phase, 100 + index * 10)
            for index, phase in enumerate(BREAKTHROUGH_PHASES[1:], 1)
        ]

        terminal = _wait_cinematic_terminal(_FakeBot(ignored + phases), initial)
        self.assertEqual(terminal["phase"], "aftermath")
        self.assertEqual(terminal["result"], "success")

    def test_terminal_wait_rejects_identity_drift_on_remaining_chain(self):
        initial = self._cinematic(2.0, "prelude", 100)
        cases = (
            {"realm_from": "Induce"},
            {"realm_to": "Condense"},
            {"result": "failure"},
            {"interrupted": True},
        )
        for drift in cases:
            with self.subTest(drift=drift):
                phases = [
                    self._cinematic(
                        float(index + 3),
                        phase,
                        100 + index * 10,
                        **(drift if phase == "charge" else {}),
                    )
                    for index, phase in enumerate(BREAKTHROUGH_PHASES[1:], 1)
                ]
                with self.assertRaisesRegex(AssertionError, "charge"):
                    _wait_cinematic_terminal(_FakeBot(phases), initial)

    def test_terminal_wait_accepts_failure_and_interrupted_identities(self):
        for result, interrupted in (("failure", False), ("interrupted", True)):
            with self.subTest(result=result, interrupted=interrupted):
                initial = self._cinematic(
                    2.0, "prelude", 100, result=result, interrupted=interrupted
                )
                phases = [
                    self._cinematic(
                        float(index + 3),
                        phase,
                        100 + index * 10,
                        result=result,
                        interrupted=interrupted,
                    )
                    for index, phase in enumerate(BREAKTHROUGH_PHASES[1:], 1)
                ]
                terminal = _wait_cinematic_terminal(_FakeBot(phases), initial)
                self.assertEqual(terminal["result"], result)
                self.assertIs(terminal["interrupted"], interrupted)

    def test_terminal_wait_rejects_missing_or_out_of_order_phase(self):
        initial = self._cinematic(2.0, "prelude", 100)
        without_catalyze = [
            self._cinematic(3.0, "charge", 110),
            self._cinematic(4.0, "apex", 120),
            self._cinematic(5.0, "aftermath", 130),
        ]
        with self.assertRaisesRegex(AssertionError, "catalyze"):
            _wait_cinematic_terminal(_FakeBot(without_catalyze), initial)

    def test_terminal_wait_rejects_non_monotonic_at_tick(self):
        initial = self._cinematic(2.0, "prelude", 100)
        phases = [
            self._cinematic(3.0, "charge", 110),
            self._cinematic(4.0, "catalyze", 110),
            self._cinematic(5.0, "apex", 120),
            self._cinematic(6.0, "aftermath", 130),
        ]
        with self.assertRaisesRegex(BotAssertionError, "at_tick 必须严格递增"):
            _wait_cinematic_terminal(_FakeBot(phases), initial)

    def test_authoritative_realm_wait_skips_old_and_wrong_realm(self):
        old = _FakeEvent(
            1.0,
            "server_data",
            {"payload_type": "player_state", "payload": {"realm": "Induce"}},
        )
        wrong = _FakeEvent(
            2.0,
            "server_data",
            {"payload_type": "player_state", "payload": {"realm": "Awaken"}},
        )
        expected = _FakeEvent(
            3.0,
            "server_data",
            {"payload_type": "player_state", "payload": {"realm": "Induce"}},
        )
        self.assertIs(
            _wait_authoritative_realm(_FakeBot([old, wrong, expected]), 1.0, "Induce"),
            expected,
        )


class InventoryHelperContractTest(unittest.TestCase):
    def test_require_pack_container_requires_canonical_id_and_owner(self):
        valid = {"containers": [{"id": "pack_42", "owner_instance_id": 42}]}
        self.assertEqual(require_pack_container(valid, 42), valid["containers"][0])
        for snapshot in (
            {"containers": [{"id": "main", "owner_instance_id": 42}]},
            {"containers": [{"id": "pack_42", "owner_instance_id": 7}]},
            {"containers": [{"id": "pack_42"}]},
        ):
            with self.subTest(snapshot=snapshot):
                with self.assertRaises(BotAssertionError):
                    require_pack_container(snapshot, 42)

    def test_wait_inventory_revision_after_matching_accepts_skipped_revision(self):
        bot = _FakeBot([_snapshot_event(2.0, 3, "skipped")])
        snapshot = wait_inventory_revision_after_matching(
            bot, 1, lambda payload: True, "any snapshot", timeout=0.01
        )

        self.assertEqual(snapshot["revision"], 3)
        self.assertEqual(snapshot["marker"], "skipped")

    def test_wait_inventory_revision_after_matching_ignores_late_duplicate_snapshot(self):
        stale = _snapshot_event(2.0, 5, "stale_duplicate")
        final = _snapshot_event(3.0, 6, "move_applied")
        final.data["payload"]["placed_items"] = [
            {"item": {"instance_id": 524}, "container_id": "body_pocket"}
        ]
        bot = _FakeBot([stale, final])

        snapshot = wait_inventory_revision_after_matching(
            bot,
            5,
            lambda payload: any(
                placed["item"]["instance_id"] == 524
                for placed in payload.get("placed_items", [])
            ),
            "move 后实例进入玩家背包",
        )

        self.assertEqual(snapshot["revision"], 6)
        self.assertEqual(snapshot["marker"], "move_applied")


class InventoryContainerSourceKindTest(unittest.TestCase):
    def test_accepts_semantically_equivalent_storage_crate_json(self):
        bot = types.SimpleNamespace(username="Fake")
        self.assertEqual(
            _parse_storage_crate_source_kind(
                bot, '{ "storage_crate" : { "is_herb" : false } }'
            ),
            {"storage_crate": {"is_herb": False}},
        )

    def test_rejects_malformed_or_wrong_source_kind(self):
        bot = types.SimpleNamespace(username="Fake")
        for raw_source_kind in (
            None,
            "not-json",
            '{"storage_crate":{"is_herb":true}}',
            '{"other":{"is_herb":false}}',
            '{"storage_crate":{}}',
        ):
            with self.subTest(raw_source_kind=raw_source_kind), self.assertRaises(
                BotAssertionError
            ):
                _parse_storage_crate_source_kind(bot, raw_source_kind)


class CultivationRealmQiScenarioTest(unittest.TestCase):
    def test_exact_rejection_does_not_accept_prefixed_or_suffixed_text(self):
        expected = "[dev] qi set rejected: value must be finite >= 0"
        exact = _FakeEvent(2.0, "chat", {"text": expected})
        prefixed = _FakeEvent(3.0, "chat", {"text": f"warning: {expected}"})
        suffixed = _FakeEvent(4.0, "chat", {"text": f"{expected}; retry"})

        self.assertIs(_chat_after(_FakeBot([exact]), 1.0, expected, exact=True), exact)
        for misleading in (prefixed, suffixed):
            with self.subTest(text=misleading.data["text"]), self.assertRaises(AssertionError):
                _chat_after(_FakeBot([misleading]), 1.0, expected, exact=True)

    def test_successful_command_rejects_matching_rejection_prefix(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "history"})],
            [_FakeEvent(2.0, "chat", {"text": "[dev] qi set rejected: [dev] qi set"})],
        )

        with self.assertRaisesRegex(BotAssertionError, "期望成功反馈"):
            _successful_command_and_chat(bot, "qi set 11", "[dev] qi set")

    def test_command_watermark_is_read_under_bot_lock(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "history"})],
            [_FakeEvent(2.0, "chat", {"text": "[dev] realm set Awaken -> Induce"})],
            enforce_command_lock=True,
        )

        result = _successful_command_and_chat(
            bot,
            "realm set induce",
            "[dev] realm set Awaken -> Induce",
        )

        self.assertEqual(bot.commands, ["realm set induce"])
        self.assertEqual(result.t, 2.0)


class LingtianScenarioFilteringTest(unittest.TestCase):
    PLAYER_POS = (10.0, 64.0, -4.0)
    FIXTURE_ID = "plant-42"
    FIXTURE_POS = [11.0, 64.0, -4.0]

    def test_fixture_parser_requires_exact_identity_fields(self):
        text = (
            f"{BOTANY_FIXTURE_PREFIX}plant-42 kind=spirit_grass "
            "pos=[11.00000000000000000,64.00000000000000000,-4.00000000000000000] zone=spawn"
        )
        self.assertEqual(
            _parse_botany_fixture(text), (self.FIXTURE_ID, self.FIXTURE_POS, "spawn")
        )
        with self.assertRaises(BotAssertionError):
            _parse_botany_fixture("[dev] botany_spawn accepted: plant_id=plant-nope")
        with self.assertRaises(BotAssertionError):
            _parse_botany_fixture(
                f"{BOTANY_FIXTURE_PREFIX}plant-42 kind=spirit_grass pos=[nan,64,-4] zone=spawn"
            )

    def test_harvest_predicate_requires_real_fixture_and_finite_in_range_position(self):
        def event(payload, t=4.0):
            return _FakeEvent(
                t,
                "server_data",
                {"payload_type": "botany_harvest_progress", "payload": payload},
            )

        base = {
            "session_id": "offline:Fake",
            "target_id": self.FIXTURE_ID,
            "target_name": HERB_ID,
            "plant_kind": HERB_ID,
            "target_pos": self.FIXTURE_POS,
            "completed": False,
            "interrupted": False,
            "progress": 0.25,
        }
        self.assertTrue(
            _is_matching_harvest(
                event(base), 1.0, self.FIXTURE_ID, self.FIXTURE_POS, self.PLAYER_POS
            )
        )
        for key, value in (
            ("session_id", ""),
            ("target_id", "plant-99"),
            ("target_name", "other_herb"),
            ("plant_kind", "other_herb"),
            ("target_pos", [11.5, 64.0, -4.0]),
            ("target_pos", [float("nan"), 64.0, -4.0]),
            ("target_pos", [18.0, 64.0, -4.0]),
        ):
            rejected = dict(base)
            rejected[key] = value
            self.assertFalse(
                _is_matching_harvest(
                    event(rejected),
                    1.0,
                    self.FIXTURE_ID,
                    self.FIXTURE_POS,
                    self.PLAYER_POS,
                ),
                f"invalid {key} must not satisfy fixture predicate: {rejected}",
            )
        self.assertFalse(_valid_target_pos({"target_pos": [None, None, None]}, self.PLAYER_POS))

    def test_gather_progress_skips_unrelated_session_and_herb(self):
        target = _FakeEvent(
            4.0,
            "server_data",
            {
                "payload_type": "botany_harvest_progress",
                "payload": {
                    "session_id": "offline:Fake",
                    "target_id": self.FIXTURE_ID,
                    "target_name": HERB_ID,
                    "plant_kind": HERB_ID,
                    "target_pos": self.FIXTURE_POS,
                    "progress": 0.25,
                    "completed": False,
                    "interrupted": False,
                },
            },
        )
        bot = _FakeBot(
            [
                _FakeEvent(
                    2.0,
                    "server_data",
                    {
                        "payload_type": "gathering_session",
                        "payload": {"session_id": "other", "target_type": "ore"},
                    },
                ),
                _FakeEvent(
                    3.0,
                    "server_data",
                    {
                        "payload_type": "botany_harvest_progress",
                        "payload": {
                            "session_id": "other",
                            "target_id": "plant-99",
                            "target_name": "other_herb",
                            "plant_kind": "other_herb",
                            "target_pos": self.FIXTURE_POS,
                            "progress": 0.2,
                        },
                    },
                ),
                target,
            ]
        )
        self.assertEqual(
            _wait_gather_progress(
                bot, 1.0, self.FIXTURE_ID, self.FIXTURE_POS, self.PLAYER_POS
            ),
            target.data["payload"],
        )

    def test_gathering_terminal_requires_same_session_and_completed_progress(self):
        good_payload = {
            "session_id": "s1",
            "target_type": "herb",
            "target_name": HERB_ID,
            "progress_ticks": 40,
            "total_ticks": 40,
            "completed": True,
            "interrupted": False,
        }
        good = _FakeEvent(
            6.0,
            "server_data",
            {"payload_type": "gathering_session", "payload": good_payload},
        )
        self.assertTrue(_is_matching_gathering_terminal(good, 1.0, "s1"))
        self.assertFalse(_is_matching_gathering_terminal(good, 6.0, "s1"))
        for key, value in (
            ("session_id", "s2"),
            ("target_type", "ore"),
            ("target_name", "other_herb"),
            ("completed", False),
            ("interrupted", True),
            ("total_ticks", 0),
            ("total_ticks", -1),
            ("progress_ticks", 39),
        ):
            wrong = dict(good_payload)
            wrong[key] = value
            self.assertFalse(
                _is_matching_gathering_terminal(
                    _FakeEvent(
                        7.0,
                        "server_data",
                        {"payload_type": "gathering_session", "payload": wrong},
                    ),
                    1.0,
                    "s1",
                )
            )


class CultivationPillScenarioTest(unittest.TestCase):
    def test_forge_consecration_uses_shared_cultivation_helpers(self):
        source_path = pathlib.Path(production_forge_consecration_inject.__file__)
        imports = {
            node.module
            for node in ast.walk(ast.parse(source_path.read_text(encoding="utf-8")))
            if isinstance(node, ast.ImportFrom)
        }

        self.assertIn(
            "bot.scenarios._cultivation_helpers",
            imports,
            "开光场景必须从共享修炼 helper 导入通用 qi 同步逻辑",
        )
        self.assertNotIn(
            "bot.scenarios.cultivation_pill_consume",
            imports,
            "开光场景不得横向依赖具体的服丹场景模块",
        )

    def test_qi_set_confirmation_is_anchored_to_exact_target(self):
        good = _FakeEvent(2.0, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})
        wrong_target = _FakeEvent(2.0, "chat", {"text": "[dev] qi set 5.0 -> 95.0"})
        misleading = _FakeEvent(2.0, "chat", {"text": "prefix [dev] qi set 5.0 -> 5.0"})
        at_anchor = _FakeEvent(1.0, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})
        non_chat = _FakeEvent(2.0, "server_data", {"text": "[dev] qi set 95.0 -> 5.0"})

        self.assertTrue(
            _is_qi_set_confirmation(good, 1.0, 5.0),
            "完整前后缀且目标为 5.0 的确认应被接受",
        )
        self.assertFalse(
            _is_qi_set_confirmation(wrong_target, 1.0, 5.0),
            "其他目标的历史确认不得满足本次 qi set 5",
        )
        self.assertFalse(
            _is_qi_set_confirmation(misleading, 1.0, 5.0),
            "仅在正文中包含 qi set 片段的聊天不得被误认成确认",
        )
        self.assertFalse(
            _is_qi_set_confirmation(at_anchor, 1.0, 5.0),
            "event.t == anchor 属于命令前水位，不得满足本次 qi set 确认",
        )
        self.assertFalse(
            _is_qi_set_confirmation(non_chat, 1.0, 5.0),
            "非 chat 事件即使正文相同也不得满足 qi set 确认",
        )

    def test_authoritative_qi_wait_uses_command_anchor_not_chat_order(self):
        authoritative = _player_state_event(1.1, 5.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史事件"})],
            [
                authoritative,
                _FakeEvent(1.2, "chat", {"text": "[dev] qi set 95.0 -> 5.0"}),
            ],
        )

        result = _set_qi_and_wait(bot, 5.0)

        self.assertIs(
            result,
            authoritative,
            "player_state 可能与 chat 同 tick 乱序，权威 qi 等待必须锚定发命令前水位线",
        )

    def test_authoritative_qi_wait_rejects_stale_event_before_anchor(self):
        bot = _CommandFakeBot(
            [
                _player_state_event(1.0, 5.0),
                _FakeEvent(2.0, "chat", {"text": "历史水位"}),
            ],
            [_FakeEvent(2.1, "chat", {"text": "[dev] qi set 95.0 -> 5.0"})],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state",
            msg="anchor 前的目标 qi 快照不得满足命令后的权威状态等待",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_set_wait_rejects_wrong_confirmation_target(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [_FakeEvent(1.1, "chat", {"text": "[dev] qi set 5.0 -> 95.0"})],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "精确目标",
            msg="错误目标的 chat 确认必须让 qi set 等待超时",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_set_wait_rejects_wrong_authoritative_value(self):
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] qi set 95.0 -> 5.0"}),
                _player_state_event(1.2, 6.0),
            ],
        )

        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state",
            msg="确认 chat 正确但权威 qi 错误时必须超时",
        ):
            _set_qi_and_wait(bot, 5.0)

    def test_qi_max_wait_consumes_authoritative_player_state(self):
        authoritative = _player_state_event(1.1, 5.0, 80.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                authoritative,
                _FakeEvent(
                    1.2,
                    "chat",
                    {"text": "[dev] qi max 100.0 -> 80.0; current=5.0"},
                ),
            ],
        )

        result = _set_qi_max_and_wait(bot, 80.0)

        self.assertIs(result, authoritative)
        self.assertEqual(bot.commands, ["qi max 80.0"])
        self.assertEqual(_player_state_values(result), (5.0, 80.0))

    def test_qi_max_confirmation_is_anchored_to_exact_target(self):
        good = _FakeEvent(
            2.0,
            "chat",
            {"text": "[dev] qi max 100.0 -> 80.0; current=5.0"},
        )
        wrong = _FakeEvent(
            2.0,
            "chat",
            {"text": "[dev] qi max 100.0 -> 90.0; current=5.0"},
        )
        self.assertTrue(_is_qi_max_confirmation(good, 1.0, 80.0))
        self.assertFalse(_is_qi_max_confirmation(wrong, 1.0, 80.0))
        self.assertFalse(_is_qi_max_confirmation(good, 2.0, 80.0))

    def test_clamp_target_comes_from_authoritative_non_default_qi_max(self):
        self.assertEqual(_expected_qi_after_pill(5.0, 80.0), 65.0)
        self.assertEqual(
            _expected_qi_after_pill(70.0, 80.0),
            80.0,
            "clamp 目标必须由权威 qi_max=80 推导，不能硬编码 100",
        )

    def test_settled_consumption_carries_non_default_qi_max_through_state_machine(self):
        final = _assert_settled_consumption(
            [
                _pill_snapshot_event(1.05, 10, 3, 70.0),
                _player_state_event(1.1, 70.0, 80.0),
                _pill_snapshot_event(1.2, 11, 2, 80.0),
                _player_state_event(1.3, 80.0, 80.0),
                _player_state_event(1.4, 80.0, 80.0),
            ],
            before_revision=10,
            before_count=3,
            baseline_qi=70.0,
            expected_qi=_expected_qi_after_pill(70.0, 80.0),
            expected_qi_max=80.0,
        )

        self.assertEqual(final["revision"], 11)

    def test_non_finite_authoritative_qi_fails_before_later_target(self):
        for invalid in (float("nan"), float("inf"), float("-inf")):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(
                AssertionError,
                "spirit_qi 必须是有限数",
                msg="5 -> 非有限值 -> 65 不得静默跳过非法中间态",
            ):
                _assert_settled_consumption(
                    [
                        _pill_snapshot_event(1.1, 11, 2, 65.0),
                        _player_state_event(1.15, 5.0),
                        _player_state_event(1.2, invalid),
                        _player_state_event(1.3, 65.0),
                    ],
                    before_revision=10,
                    before_count=3,
                    baseline_qi=5.0,
                    expected_qi=65.0,
                    expected_qi_max=100.0,
                )

    def test_missing_or_non_finite_authoritative_qi_max_fails(self):
        invalid_values = (None, 0.0, float("nan"), float("inf"), float("-inf"))
        for invalid in invalid_values:
            event = _player_state_event(1.0, 5.0)
            if invalid is None:
                event.data["payload"].pop("spirit_qi_max")
                message = "必须是数值"
            else:
                event.data["payload"]["spirit_qi_max"] = invalid
                message = "必须是有限正数"
            with self.subTest(invalid=invalid), self.assertRaisesRegex(
                AssertionError, message
            ):
                _player_state_values(event)

    def test_server_tick_fence_crosses_requested_tick_target_before_snapshot(self):
        self.assertEqual(
            SERVER_TICK_OBSERVATION_TICKS,
            20,
            "生产服丹观察窗口必须继续覆盖至少 20 个权威 server tick",
        )
        duplicate = _pill_snapshot_event(1.55, 12, 1, 65.0)
        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 101"}),
                _FakeEvent(1.3, "chat", {"text": "[dev] time now: 102"}),
                _FakeEvent(1.4, "chat", {"text": "[dev] time now: 103"}),
                _FakeEvent(1.5, "chat", {"text": "[dev] time now: 103"}),
                duplicate,
                _FakeEvent(1.6, "chat", {"text": "[dev] time now: 104"}),
            ],
        )

        snapshot = _snapshot_after_server_tick_fence(
            bot, minimum_ticks=2, timeout=1.0
        )

        self.assertIn(
            duplicate,
            snapshot,
            "首个 drain chat 后、第二个 drain marker 前的重复帧必须进入快照",
        )
        self.assertEqual(
            bot.commands,
            ["time now"] * 6,
            "100 起点 +2 后先跨到 103，再做两次 round-trip 才覆盖 post-emit 帧",
        )

    def test_server_tick_fence_rejects_malformed_or_rollback_tick(self):
        malformed = _FakeEvent(2.0, "chat", {"text": "[dev] time now: 12x"})
        with self.assertRaisesRegex(AssertionError, "十进制 tick"):
            _server_tick_from_event(malformed, 1.0)

        bot = _CommandFakeBot(
            [_FakeEvent(1.0, "chat", {"text": "历史水位"})],
            [
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 99"}),
            ],
        )
        with self.assertRaisesRegex(AssertionError, "不得回退"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=1.0)

    def test_server_tick_fence_rejects_invalid_bounds_and_deadlines(self):
        bot = _CommandFakeBot([], [])
        with self.assertRaisesRegex(ValueError, "minimum_ticks"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=0, timeout=1.0)
        with self.assertRaisesRegex(ValueError, "timeout"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=0.0)

        main_timeout_bot = _CommandFakeBot(
            [],
            [_FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"})],
        )
        with mock.patch(
            "bot.scenarios.cultivation_pill_consume.time.monotonic",
            side_effect=[0.0, 2.0],
        ), self.assertRaisesRegex(BotAssertionError, "推进至少"):
            _snapshot_after_server_tick_fence(
                main_timeout_bot, minimum_ticks=1, timeout=1.0
            )

        drain_timeout_bot = _CommandFakeBot(
            [],
            [
                _FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 102"}),
            ],
        )
        with mock.patch(
            "bot.scenarios.cultivation_pill_consume.time.monotonic",
            side_effect=[0.0, 0.1, 2.0],
        ), self.assertRaisesRegex(BotAssertionError, "post-emit drain"):
            _snapshot_after_server_tick_fence(
                drain_timeout_bot, minimum_ticks=1, timeout=1.0
            )

    def test_server_tick_fence_rejects_rollback_during_post_emit_drain(self):
        bot = _CommandFakeBot(
            [],
            [
                _FakeEvent(1.0, "chat", {"text": "[dev] time now: 100"}),
                _FakeEvent(1.1, "chat", {"text": "[dev] time now: 102"}),
                _FakeEvent(1.2, "chat", {"text": "[dev] time now: 101"}),
            ],
        )
        with self.assertRaisesRegex(AssertionError, "不得回退"):
            _snapshot_after_server_tick_fence(bot, minimum_ticks=1, timeout=1.0)

    def test_stale_same_tick_baseline_is_not_new_authoritative_value(self):
        self.assertFalse(
            _has_departed_baseline(5.0, 5.0, 65.0),
            "同 tick 残留的旧基线不得被当作服丹结果",
        )
        self.assertTrue(
            _has_departed_baseline(65.0, 5.0, 65.0),
            "真实恢复到 65 应离开旧基线",
        )
        self.assertTrue(
            _has_departed_baseline(20.0, 5.0, 65.0),
            "任何非基线权威值都必须进入校验，不能用中点过滤错误中间态",
        )
        self.assertFalse(
            _has_departed_baseline(65.0, 65.0, 5.0),
            "下降目标下旧基线 65 不得被当作新值",
        )
        self.assertTrue(
            _has_departed_baseline(5.0, 65.0, 5.0),
            "下降目标真实到达 5 时应越过变化屏障",
        )
        self.assertFalse(
            _has_departed_baseline(5.0, 5.0, 5.0),
            "目标等于基线时没有可观察状态转换，不得伪称已离开基线",
        )
        final = _assert_settled_consumption(
            [
                _pill_snapshot_event(1.05, 10, 3, 5.0),
                _player_state_event(1.1, 5.0),
                _pill_snapshot_event(1.2, 11, 2, 65.0),
                _player_state_event(1.3, 65.0),
                _player_state_event(1.4, 65.0),
            ],
            before_revision=10,
            before_count=3,
            baseline_qi=5.0,
            expected_qi=65.0,
            expected_qi_max=100.0,
        )
        self.assertEqual(
            final["revision"], 11,
            "正常一次消费应只把 inventory revision 从 10 推进到 11",
        )

    def test_huiyuan_asset_pins_expected_qi_recovery(self):
        asset = pathlib.Path(__file__).parents[2] / "server/assets/items/pills.toml"
        items = tomllib.loads(asset.read_text(encoding="utf-8"))["item"]
        huiyuan = next(item for item in items if item["id"] == PILL_ID)
        self.assertEqual(
            huiyuan["effect"]["kind"],
            "qi_recovery",
            f"{PILL_ID} 必须继续走 qi_recovery，实际 effect={huiyuan['effect']}",
        )
        self.assertEqual(
            float(huiyuan["effect"]["magnitude"]), PILL_QI_RECOVERY,
            f"场景药效推导常量应为 {PILL_QI_RECOVERY}，"
            f"实际 asset effect={huiyuan['effect']}",
        )
        self.assertEqual(
            _expected_qi_after_pill(5.0, 100.0),
            NON_CLAMP_EXPECTED_QI,
            f"qi=5、权威 qi_max=100 时应推导 {NON_CLAMP_EXPECTED_QI}",
        )

    def test_repeated_qi_effect_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state 应持续稳定",
            msg="第二次真元生效必须命中权威状态稳定性断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _player_state_event(1.3, 100.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_repeated_inventory_decrement_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="第二次扣存推进 revision 必须命中逐帧 revision 断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 12, 1, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_repeated_decrement_then_surface_rollback_still_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="11/2 -> 12/1 -> 11/2 的表面正确终态仍必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 12, 1, 65.0),
                    _pill_snapshot_event(1.4, 11, 2, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_consumed_snapshot_rollback_to_old_state_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory revision 必须保持",
            msg="消费后回滚到旧 revision/count 必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 10, 3, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_same_revision_count_change_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版丹药数量必须保持",
            msg="revision 未变但丹药再次减少也必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 11, 1, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_inventory_qi_regression_fails(self):
        with self.assertRaisesRegex(
            AssertionError,
            "消费后每版 inventory qi 必须保持",
            msg="库存快照 qi 回滚必须命中逐帧 qi 断言",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 65.0),
                    _pill_snapshot_event(1.3, 11, 2, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_authoritative_qi_regression_after_effect_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "权威 player_state 应持续稳定",
            msg="权威真元先恢复到 65 又回落到旧基线时必须失败",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.2, 5.0),
                    _player_state_event(1.3, 65.0),
                    _player_state_event(1.4, 5.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )

    def test_wrong_authoritative_qi_before_target_fails_settle_window(self):
        with self.assertRaisesRegex(
            AssertionError,
            "错误中间态",
            msg="5 -> 20 -> 65 中的 20 是消费后的错误权威态，不得被中点过滤",
        ):
            _assert_settled_consumption(
                [
                    _pill_snapshot_event(1.1, 11, 2, 65.0),
                    _player_state_event(1.15, 5.0),
                    _player_state_event(1.2, 20.0),
                    _player_state_event(1.3, 65.0),
                ],
                before_revision=10,
                before_count=3,
                baseline_qi=5.0,
                expected_qi=65.0,
                expected_qi_max=100.0,
            )


def _server_data_zone_info_bytes() -> bytes:
    zone_info = (
        _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "rift_mouth_north_002")
        + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.068602)
        + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 5)
        + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Normal")
        + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "rift_mouth_entry")
        + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "wind_warning")
        + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "灵气骤薄")
    )
    return _pb_message(proto_min.SERVER_DATA_ZONE_INFO_FIELD, zone_info)


def _server_data_inventory_snapshot_bytes() -> bytes:
    side_effect = (
        _pb_string(1, "qi_drain_mild")
        + _pb_varint(2, 30)
        + _pb_varint(3, 1)
        + _pb_varint(4, 0)
        + _pb_varint(5, 2)
        + _pb_fixed64(6, 1.5)
    )
    alchemy = (
        _pb_string(1, "pill")
        + _pb_string(2, "qing_xin_dan")
        + _pb_varint(3, 2)
        + _pb_fixed64(4, 0.9)
        + _pb_varint(5, 1)
        + _pb_message(6, side_effect)
    )
    freshness = (
        _pb_varint(1, 123)
        + _pb_fixed32(2, 0.5)
        + _pb_string(3, "Decay")
        + _pb_string(4, "mineral_decay_v1")
        + _pb_varint(5, 17)
        + _pb_varint(6, 140)
    )
    item = (
        _pb_varint(1, 9)
        + _pb_string(2, "worn_grass_pouch")
        + _pb_string(3, "破草包")
        + _pb_varint(4, 2)
        + _pb_varint(5, 2)
        + _pb_fixed64(6, 0.25)
        + _pb_string(7, "common")
        + _pb_string(8, "test")
        + _pb_varint(9, 1)
        + _pb_fixed64(10, 0.0)
        + _pb_fixed64(11, 0.3)
        + _pb_string(12, "za_gang")
        + _pb_string(13, "skill_scroll")
        + _pb_string(14, "forging")
        + _pb_varint(15, 500)
        + _pb_varint(16, 7)
        + _pb_fixed32(17, 0.75)
        + _pb_varint(18, 1)
        + _pb_string(19, "brittle_edge")
        + _pb_string(19, "qi_shear")
        + _pb_varint(20, 3)
        + _pb_message(21, alchemy)
        + _pb_message(22, freshness)
    )
    container = (
        _pb_string(1, "body_pocket")
        + _pb_string(2, "贴身口袋")
        + _pb_varint(3, 2)
        + _pb_varint(4, 3)
    )
    equipped = _pb_message(3, item)
    weight = _pb_fixed64(1, 1.0) + _pb_fixed64(2, 23.0)
    snapshot = (
        _pb_varint(1, 12)
        + _pb_message(2, container)
        + _pb_message(4, equipped)
        + _pb_message(7, weight)
    )
    return _pb_message(8, snapshot)


def _server_data_narration_bytes() -> bytes:
    target = (
        _pb_string(1, "天道的注意力掠过，境界未至")
        + _pb_string(2, "player")
        + _pb_string(3, "system_warning")
        + _pb_string(4, "offline:Alice")
        + _pb_string(5, "realm_gate_rejected")
    )
    broadcast = (
        _pb_string(1, "旧式全服旁白")
        + _pb_string(2, "broadcast")
        + _pb_string(3, "narration")
    )
    return _pb_message(3, _pb_message(1, target) + _pb_message(1, broadcast))


def _server_data_breakthrough_cinematic_bytes() -> bytes:
    cinematic = (
        _pb_string(1, "offline:Break")
        + _pb_string(2, "prelude")
        + _pb_varint(3, 0)
        + _pb_varint(4, 60)
        + _pb_string(5, "Awaken")
        + _pb_string(6, "Induce")
        + _pb_string(7, "success")
        + _pb_varint(8, 0)
        + _pb_fixed64(9, -240.5)
        + _pb_fixed64(10, 72.0)
        + _pb_fixed64(11, -160.25)
        + _pb_fixed64(12, 96.0)
        + _pb_varint(13, 0)
        + _pb_varint(14, 1)
        + _pb_fixed32(15, 0.75)
        + _pb_fixed32(16, 0.35)
        + _pb_string(17, "calm")
        + _pb_string(18, "awaken_induce")
        + _pb_varint(19, 4242)
    )
    return _pb_message(proto_min.SERVER_DATA_BREAKTHROUGH_CINEMATIC_FIELD, cinematic)


def _server_data_inventory_event_moved_bytes() -> bytes:
    from_location = _pb_message(
        1,
        _pb_string(1, "body_pocket") + _pb_varint(2, 0) + _pb_varint(3, 1),
    )
    to_location = _pb_message(2, _pb_varint(1, 2) + _pb_varint(2, 1))
    moved = (
        _pb_varint(1, 13)
        + _pb_varint(2, 99)
        + _pb_message(3, from_location)
        + _pb_message(4, to_location)
    )
    return _pb_message(80, _pb_message(1, moved))


def _server_data_loot_container_open_bytes() -> bytes:
    open_payload = (
        _pb_varint(1, 7)
        + _pb_string(2, '{"kind":"storage_crate","is_herb":false}')
        + _pb_varint(3, 3)
        + _pb_varint(4, 4)
    )
    return _pb_message(119, open_payload)


def _server_data_coffin_state_bytes(
    in_coffin: bool, multiplier: float, grade: str | None
) -> bytes:
    state = _pb_varint(1, 1 if in_coffin else 0) + _pb_fixed64(2, multiplier)
    if grade is not None:
        state += _pb_string(3, grade)
    return _pb_message(78, state)


def _server_data_loot_container_update_bytes() -> bytes:
    item = (
        _pb_varint(1, 99)
        + _pb_string(2, "refined_iron")
        + _pb_string(3, "精铁")
        + _pb_varint(4, 1)
        + _pb_varint(5, 1)
        + _pb_fixed64(6, 0.1)
        + _pb_string(7, "common")
        + _pb_string(8, "test")
        + _pb_varint(9, 2)
        + _pb_fixed64(10, 0.0)
        + _pb_fixed64(11, 1.0)
    )
    placed = (
        _pb_string(1, "ext_7")
        + _pb_varint(2, 0)
        + _pb_varint(3, 1)
        + _pb_message(4, item)
    )
    return _pb_message(120, _pb_varint(1, 7) + _pb_message(2, placed))


def _server_data_loot_container_close_bytes() -> bytes:
    return _pb_message(121, _pb_varint(1, 7) + _pb_string(2, "distance"))


def _server_data_tribulation_state_bytes(
    *,
    phase: str,
    wave_current: int,
    wave_total: int,
    result: str | None,
    active: bool = True,
    failed: bool = False,
    half_step_on_success: bool = False,
) -> bytes:
    """field 66 `tribulation_state`（见 proto/bong/envelope.proto `TribulationState`）。

    failed=field 13、half_step_on_success=field 14 都是独立 bool 字段，必须可设非默认值：
    解码测试要 pin 每一个新增字段的 wire 号，缺席/串号/恒 false 的解码器才会被抓住。
    """
    body = (
        _pb_varint(1, 1 if active else 0)
        + _pb_string(2, "du_xu:char-1")
        + _pb_string(3, "BHDE6Cult")
        + _pb_string(4, "du_xu")
        + _pb_string(5, phase)
        + _pb_fixed64(6, 12.5)
        + _pb_fixed64(7, -34.25)
        + _pb_varint(8, wave_current)
        + _pb_varint(9, wave_total)
        + _pb_varint(10, 1000)
        + _pb_varint(11, 5000)
        + _pb_varint(12, 5300)
        + _pb_varint(13, 1 if failed else 0)
        + _pb_varint(14, 1 if half_step_on_success else 0)
        + _pb_string(15, "du_xu:char-1")
    )
    if result is not None:
        body += _pb_string(16, result)
    return _pb_message(66, body)


def _server_data_heart_demon_offer_bytes() -> bytes:
    """field 69 `heart_demon_offer`（见 proto/bong/envelope.proto `HeartDemonOffer`）。

    全部**非对称**值（review finding major-7）：offer_id != trigger_id、quota_remaining !=
    quota_total、每个 choice 的 effect_summary/flavor/style_hint 各自可辨——把字段 13/14
    互换、读错 trigger_id、配额钞等错误解码器立刻撞红。
    """
    choice = lambda choice_id, category, title, effect, flavor, style: (  # noqa: E731
        _pb_string(1, choice_id)
        + _pb_string(2, category)
        + _pb_string(3, title)
        + _pb_string(4, effect)
        + _pb_string(5, flavor)
        + _pb_string(6, style)
    )
    body = (
        _pb_string(1, "heart_demon:7:1000")
        + _pb_string(2, "heart_demon:7:1111")
        + _pb_string(3, "心魔劫临身")
        + _pb_string(4, "渡虚劫 · 心魔")
        + _pb_fixed64(5, 0.85)
        + _pb_varint(6, 2)  # quota_remaining
        + _pb_varint(7, 3)  # quota_total（≠ remaining，锁独立映射）
        + _pb_varint(8, 1700000050000)  # expires_at_ms
        + _pb_message(
            9,
            choice(
                "heart_demon_choice_0",
                "Composure",
                "守本心",
                "稳住心神，回复少量当前真元",
                "心若磐石，外物不侵。",
                "calm-blue",
            ),
        )
        + _pb_message(
            9,
            choice(
                "heart_demon_choice_1",
                "Breakthrough",
                "斩执念",
                "若斩错心魔，将损当前真元并强化下一道开天雷",
                "抽刀断水，一念斩魔。",
                "edge-orange",
            ),
        )
        + _pb_message(
            9,
            choice(
                "heart_demon_choice_2",
                "Perception",
                "无解",
                "承认无解，不得增益也不受真元惩罚",
                "心魔无解，人间一二。",
                "calm-grey",
            ),
        )
    )
    return _pb_message(69, body)


def _server_data_morph_state_bytes(
    *,
    mode: str,
    entity_id: int,
    model_kind: int,
    form_race_id: str,
    form_body_plan_id: str,
    active: bool,
) -> bytes:
    """plan-race-system-v1 P4 — field 142 `morph_state`（见 proto/bong/common.proto
    `MorphState`/`MorphStateEntry`）。"""
    entry = (
        _pb_varint(1, entity_id)
        + _pb_varint(2, model_kind)
        + _pb_string(3, form_race_id)
        + _pb_string(4, form_body_plan_id)
        + _pb_varint(5, 1 if active else 0)
    )
    state = _pb_varint(1, 1) + _pb_string(2, mode) + _pb_message(3, entry)
    return _pb_message(142, state)


def _server_data_carrier_state_bytes(
    *,
    carrier: str,
    phase: int,
    progress: float,
    sealed_qi: float,
    sealed_qi_initial: float,
    half_life_remaining_ticks: int,
    item_instance_id: int | None,
) -> bytes:
    """field 49 `carrier_state`（见 proto/bong/envelope.proto:1896 `CarrierState`）。"""
    state = (
        _pb_string(1, carrier)
        + _pb_varint(2, phase)
        + _pb_fixed32(3, progress)
        + _pb_fixed32(4, sealed_qi)
        + _pb_fixed32(5, sealed_qi_initial)
        + _pb_varint(6, half_life_remaining_ticks)
    )
    if item_instance_id is not None:
        state += _pb_varint(7, item_instance_id)
    return _pb_message(49, state)


def _pb_key(field: int, wire: int) -> bytes:
    return _pb_raw_varint((field << 3) | wire)


def _pb_raw_varint(value: int) -> bytes:
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def _pb_varint(field: int, value: int) -> bytes:
    return _pb_key(field, 0) + _pb_raw_varint(value)


def _pb_fixed64(field: int, value: float) -> bytes:
    return _pb_key(field, 1) + struct.pack("<d", value)


def _pb_fixed32(field: int, value: float) -> bytes:
    return _pb_key(field, 5) + struct.pack("<f", value)


def _pb_bytes(field: int, value: bytes) -> bytes:
    return _pb_key(field, 2) + _pb_raw_varint(len(value)) + value


def _pb_string(field: int, value: str) -> bytes:
    return _pb_bytes(field, value.encode("utf-8"))


def _pb_message(field: int, value: bytes) -> bytes:
    return _pb_bytes(field, value)


def _pb_varint_field(number: int, value: int) -> bytes:
    return mc.write_varint(number << 3) + mc.write_varint(value)


def _pb_u64_varint_field(number: int, value: int) -> bytes:
    """protobuf uint64 字段（mc.write_varint 是 32 位 MC varint，装不下 u64）。"""
    body = bytearray()
    remaining = value
    while remaining >= 0x80:
        body.append((remaining & 0x7F) | 0x80)
        remaining >>= 7
    body.append(remaining)
    return mc.write_varint(number << 3) + bytes(body)


def _pb_len_field(number: int, value: bytes) -> bytes:
    return mc.write_varint((number << 3) | 2) + mc.write_varint(len(value)) + value


class ProtoMinTest(unittest.TestCase):
    def test_server_data_payload_name_reads_oneof_field(self):
        envelope = _pb_len_field(31, b"\x08\x01")
        self.assertEqual(proto_min.server_data_payload_name(envelope), "lingtian_session")
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(34, b"")),
            "cast_sync",
            "shallow payload table must not hide production field 34",
        )
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(36, b"")),
            "skillbar_config",
            "shallow payload table must not hide authoritative bind acknowledgement field 36",
        )
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(51, b"")),
            "combat_event",
            "shallow payload table must not hide production field 51",
        )
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(128, b"")),
            "sword_bond_hud_state",
            "SwordBond HUD field 128 must be recognized as an ambient payload",
        )
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(45, b"")),
            "dugu_poison_state",
            "Dugu poison HUD field 45 must be recognized as an ambient payload",
        )
        self.assertEqual(
            proto_min.server_data_payload_name(_pb_len_field(48, b"")),
            "poison_trait_state",
            "Poison trait HUD field 48 must be recognized as an ambient payload",
        )

    def test_false_skin_state_field_50_is_decoded_for_ambient_oracle(self):
        inner = (
            _pb_string(1, "offline:Bot")
            + _pb_varint_field(3, 0)
            + _pb_fixed64(4, 0.0)
            + _pb_fixed64(5, 0.0)
            + _pb_varint_field(6, 0)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(50, inner))
        self.assertEqual(decoded["type"], "false_skin_state")
        self.assertEqual(decoded["target_id"], "offline:Bot")

    def test_treasure_equipped_field_43_decodes_empty_slot(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(43, _pb_string(1, "main_hand"))
        )
        self.assertEqual(decoded["type"], "treasure_equipped")
        self.assertEqual(decoded["slot"], "main_hand")
        self.assertIsNone(decoded["treasure"])

    def test_server_data_decoder_table_is_named_and_dispatches(self):
        self.assertLessEqual(
            set(proto_min.SERVER_DATA_PAYLOAD_DECODERS),
            set(proto_min.SERVER_DATA_PAYLOAD_NAMES),
            "每个深解码 field 必须共享 shallow oneof name 的单一字段键",
        )
        sentinel = object()
        for field in proto_min.SERVER_DATA_PAYLOAD_DECODERS:
            with self.subTest(
                field=field, name=proto_min.SERVER_DATA_PAYLOAD_NAMES[field]
            ):
                with mock.patch.object(
                    proto_min,
                    "SERVER_DATA_PAYLOAD_DECODERS",
                    {field: lambda _payload: sentinel},
                ):
                    self.assertIs(
                        proto_min.decode_server_data_envelope(_pb_len_field(field, b"")),
                        sentinel,
                        f"field {field} 必须由 decoder table 分发",
                    )

    def test_server_data_payload_name_reads_skill_xp_gain_oneof_field(self):
        # central-review 2012 #1：field 7 必须在 SERVER_DATA_PAYLOAD_NAMES 中登记，
        # 否则 expect_server_data_payload("skill_xp_gain") 会把该 oneof 分类为未知。
        envelope = _server_data_skill_xp_gain_bytes()
        self.assertEqual(proto_min.server_data_payload_field(envelope), 7)
        self.assertEqual(proto_min.server_data_payload_name(envelope), "skill_xp_gain")

    def test_server_data_payload_name_falls_back_to_decoded_type(self):
        # decoder 认识但注册表未登记的 oneof 字段（field 34=cast_sync）：name 桥
        # 由 decoded payload 的 type 回退，与 decoder 保持一致，而不是退回 field_34。
        envelope = _pb_len_field(34, b"")
        self.assertEqual(proto_min.server_data_payload_name(envelope), "cast_sync")
        # 验证未在 SERVER_DATA_PAYLOAD_NAMES 注册但在 DECODERS 注册时的回退机制
        saved = proto_min.SERVER_DATA_PAYLOAD_NAMES.pop(34, None)
        try:
            self.assertEqual(proto_min.server_data_payload_name(envelope), "cast_sync")
        finally:
            if saved is not None:
                proto_min.SERVER_DATA_PAYLOAD_NAMES[34] = saved

    def test_inventory_snapshot_extracts_placed_item_location(self):
        item = (
            _pb_varint_field(1, 4242)
            + _pb_len_field(2, b"furnace_fantie")
            + _pb_len_field(3, "凡铁炉".encode("utf-8"))
        )
        placed = (
            _pb_len_field(1, b"main_pack")
            + _pb_varint_field(2, 1)
            + _pb_varint_field(3, 2)
            + _pb_len_field(4, item)
        )
        inventory = _pb_len_field(3, placed)
        envelope = _pb_len_field(8, inventory)

        refs = proto_min.inventory_item_refs(envelope)
        self.assertEqual(len(refs), 1)
        self.assertEqual(refs[0].instance_id, 4242)
        self.assertEqual(refs[0].item_id, "furnace_fantie")
        self.assertEqual(
            refs[0].location,
            {"kind": "container", "container_id": "main_pack", "row": 1, "col": 2},
        )

    def test_inventory_snapshot_extracts_equipped_item_location(self):
        item = _pb_varint_field(1, 77) + _pb_len_field(2, b"hoe_iron")
        equipped = _pb_len_field(10, item)
        inventory = _pb_len_field(4, equipped)
        envelope = _pb_len_field(8, inventory)

        refs = proto_min.inventory_item_refs(envelope)
        self.assertEqual(refs[0].location, {"kind": "equip", "slot": "main_hand", "state": "held"})

    def test_trade_offer_decodes_offer_and_item_summaries(self):
        offered = (
            _pb_u64_varint_field(1, 1001)
            + _pb_len_field(2, b"starter_talisman")
            + _pb_len_field(3, b"talisman")
            + _pb_varint_field(4, 1)
        )
        requested = (
            _pb_u64_varint_field(1, 2002)
            + _pb_len_field(2, b"huiyuan_pill")
            + _pb_len_field(3, b"pill")
            + _pb_varint_field(4, 3)
        )
        trade_offer = (
            _pb_len_field(1, b"trade:00000000-0000-7000-8000-000000000000")
            + _pb_len_field(2, b"char:alice")
            + _pb_len_field(3, b"char:bob")
            + _pb_len_field(4, offered)
            + _pb_len_field(5, requested)
            + _pb_u64_varint_field(6, 9876543210)
        )
        envelope = _pb_len_field(65, trade_offer)

        self.assertEqual(proto_min.server_data_payload_name(envelope), "trade_offer")
        payload = proto_min.decode_server_data_envelope(envelope)
        self.assertEqual(payload["type"], "trade_offer")
        self.assertEqual(payload["offer_id"], "trade:00000000-0000-7000-8000-000000000000")
        self.assertEqual(payload["initiator"], "char:alice")
        self.assertEqual(payload["target"], "char:bob")
        self.assertEqual(
            payload["offered_item"],
            {"instance_id": 1001, "item_id": "starter_talisman", "display_name": "talisman", "stack_count": 1},
        )
        self.assertEqual(
            payload["requested_items"],
            [{"instance_id": 2002, "item_id": "huiyuan_pill", "display_name": "pill", "stack_count": 3}],
        )
        self.assertEqual(payload["expires_at_ms"], 9876543210)

    def test_sparring_invite_decodes_all_fields(self):
        sparring_invite = (
            _pb_len_field(1, b"sparring:00000000-0000-7000-8000-000000000000")
            + _pb_len_field(2, b"char:alice")
            + _pb_len_field(3, b"char:bob")
            + _pb_len_field(4, b"condense_solidify")
            + _pb_len_field(5, "气息相试".encode())
            + _pb_len_field(6, "点到为止".encode())
            + _pb_u64_varint_field(7, 9876543210)
        )
        envelope = _pb_len_field(64, sparring_invite)

        self.assertEqual(proto_min.server_data_payload_name(envelope), "sparring_invite")
        payload = proto_min.decode_server_data_envelope(envelope)
        self.assertEqual(payload["type"], "sparring_invite")
        self.assertEqual(payload["invite_id"], "sparring:00000000-0000-7000-8000-000000000000")
        self.assertEqual(payload["initiator"], "char:alice")
        self.assertEqual(payload["target"], "char:bob")
        self.assertEqual(payload["realm_band"], "condense_solidify")
        self.assertEqual(payload["breath_hint"], "气息相试")
        self.assertEqual(payload["terms"], "点到为止")
        self.assertEqual(payload["expires_at_ms"], 9876543210)


def _bare_connection(threshold: int = -1) -> mc.Connection:
    """不开 socket 的 Connection —— 只测帧解析状态机。"""
    conn = mc.Connection.__new__(mc.Connection)
    conn.buf = b""
    conn.compression_threshold = threshold
    return conn


def _frame(payload: bytes) -> bytes:
    return mc.write_varint(len(payload)) + payload


class FrameParseTest(unittest.TestCase):
    def test_partial_buffers_preserved(self):
        conn = _bare_connection()
        payload = b"\x28" + b"x" * 300  # 两字节 varint 长度前缀
        frame = _frame(payload)
        # 任意截断点：半帧必须返回 None 且 buf 原样保留（timeout 落点安全）
        for cut in [0, 1, 2, 10, len(frame) - 1]:
            conn.buf = frame[:cut]
            self.assertIsNone(conn._try_parse_frame(), f"cut={cut} 时不该解析出帧")
            self.assertEqual(len(conn.buf), cut, f"cut={cut} 时半帧字节不能被消费")
        conn.buf = frame
        self.assertEqual(conn._try_parse_frame(), payload)
        self.assertEqual(conn.buf, b"")

    def test_two_frames_sequential(self):
        conn = _bare_connection()
        conn.buf = _frame(b"\x01aa") + _frame(b"\x02bb")
        self.assertEqual(conn._try_parse_frame(), b"\x01aa")
        self.assertEqual(conn._try_parse_frame(), b"\x02bb")
        self.assertIsNone(conn._try_parse_frame())

    def test_compressed_frames(self):
        conn = _bare_connection(threshold=64)
        # 阈值下未压缩：DataLength=0 + 原文
        small = b"\x17small"
        conn.buf = _frame(mc.write_varint(0) + small)
        self.assertEqual(conn._try_parse_frame(), small)
        # Valence treats an exact-threshold payload as uncompressed.
        exact = b"\x17" + b"x" * 63
        self.assertEqual(len(exact), 64)
        conn.buf = _frame(mc.write_varint(0) + exact)
        self.assertEqual(conn._try_parse_frame(), exact)
        # 达阈值 zlib：DataLength=原长 + 压缩体
        big = b"\x24" + b"y" * 200
        conn.buf = _frame(mc.write_varint(len(big)) + zlib.compress(big))
        self.assertEqual(conn._try_parse_frame(), big)


class ForgeSubscriberTest(unittest.TestCase):
    @staticmethod
    def _subscriber(sock):
        subscriber = production_forge_request._ForgeEventSubscriber.__new__(
            production_forge_request._ForgeEventSubscriber
        )
        subscriber.host = "127.0.0.1"
        subscriber.port = 6379
        subscriber._sock = sock
        subscriber._buf = bytearray()
        return subscriber

    def test_idle_timeout_retries_before_deadline(self):
        class FakeSocket:
            def __init__(self):
                self.calls = 0
                self.timeouts = []

            def settimeout(self, value):
                self.timeouts.append(value)

            def recv(self, _size):
                self.calls += 1
                if self.calls == 1:
                    raise socket.timeout
                return b"+ok\r\n"

        sock = FakeSocket()
        subscriber = self._subscriber(sock)
        with mock.patch.object(
            production_forge_request.time,
            "monotonic",
            side_effect=[10.0, 11.0, 12.0],
        ):
            frame = subscriber._recv_until_frame(20.0)

        self.assertEqual(frame, "ok", "deadline 内的空闲超时不能吞掉后续 Redis 帧")
        self.assertEqual(sock.calls, 2, "空闲超时后应继续 recv，而不是立即失败")
        self.assertEqual(sock.timeouts, [10.0, 8.0])

    def test_idle_timeout_at_deadline_raises(self):
        class TimeoutSocket:
            def settimeout(self, _value):
                pass

            def recv(self, _size):
                raise socket.timeout

        subscriber = self._subscriber(TimeoutSocket())
        with (
            mock.patch.object(
                production_forge_request.time,
                "monotonic",
                side_effect=[19.0, 20.0],
            ),
            self.assertRaisesRegex(TimeoutError, "订阅 socket 空闲超时"),
        ):
            subscriber._recv_until_frame(20.0)

    def test_run_closes_subscriber_after_success(self):
        subscriber = mock.Mock()
        env = object()
        with (
            mock.patch.object(
                production_forge_request,
                "_ForgeEventSubscriber",
                return_value=subscriber,
            ),
            mock.patch.object(production_forge_request, "_run_forge_scenario") as body,
        ):
            production_forge_request.run(env)

        body.assert_called_once_with(env, subscriber)
        subscriber.close.assert_called_once_with()

    def test_run_closes_subscriber_after_failure(self):
        subscriber = mock.Mock()
        env = object()
        with (
            mock.patch.object(
                production_forge_request,
                "_ForgeEventSubscriber",
                return_value=subscriber,
            ),
            mock.patch.object(
                production_forge_request,
                "_run_forge_scenario",
                side_effect=RuntimeError("scenario failed"),
            ),
            self.assertRaisesRegex(RuntimeError, "scenario failed"),
        ):
            production_forge_request.run(env)

        subscriber.close.assert_called_once_with()


def _scenario_default_enabled(tree: ast.Module) -> bool:
    for node in tree.body:
        if isinstance(node, ast.Assign):
            targets = node.targets
        elif isinstance(node, ast.AnnAssign):
            targets = [node.target]
        else:
            continue
        if not any(
            isinstance(target, ast.Name) and target.id == "DEFAULT_ENABLED"
            for target in targets
        ):
            continue
        if isinstance(node.value, ast.Constant) and node.value.value is False:
            return False
    return True


class RunnerLogicTest(unittest.TestCase):
    def test_fallback_run_tag_wins_when_north_rift_tag_is_also_present(self):
        observed_run_tags: list[str] = []
        scenario = types.SimpleNamespace(
            DESCRIPTION="tag precedence probe",
            MODULES=["terrain"],
            run=lambda env: observed_run_tags.append(env.run_tag),
        )
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"tag_precedence_probe": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(
                os.environ,
                {"BOT_E2E_RUN_TAG": "ci", "NORTH_RIFT_RUN_TAG": "nr123"},
                clear=False,
            ),
            mock.patch.object(
                sys,
                "argv",
                ["run_scenarios.py", "--scenario", "tag_precedence_probe"],
            ),
            redirect_stdout(io.StringIO()),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        self.assertEqual(
            observed_run_tags,
            ["ci"],
            "显式 fallback witness BOT_E2E_RUN_TAG 必须覆盖遗留 NORTH_RIFT_RUN_TAG",
        )

    def test_craft_reconnect_session_closes_before_settle_window(self):
        events = []

        class FakeBot:
            def __enter__(self):
                events.append("enter")
                return self

            def __exit__(self, *_exc):
                events.append("close")

        class FakeEnv:
            def new_bot(self, tag):
                events.append(("new_bot", tag))
                return FakeBot()

        with mock.patch(
            "bot.scenarios.production_craft_disconnect_resume.time.sleep",
            side_effect=lambda seconds: events.append(("sleep", seconds)),
        ):
            with _reconnectable_session(FakeEnv()):
                events.append("body")

        self.assertEqual(
            events,
            [
                ("new_bot", "Resume"),
                "enter",
                "body",
                "close",
                ("sleep", DISCONNECT_SETTLE_SECONDS),
            ],
            "同用户名重连前必须先关闭旧连接，再给 server cleanup 留出窗口",
        )

    def test_craft_reconnect_session_settles_after_body_failure(self):
        events = []

        class FakeBot:
            def __enter__(self):
                return self

            def __exit__(self, *_exc):
                events.append("close")

        class FakeEnv:
            def new_bot(self, tag):
                return FakeBot()

        with (
            mock.patch(
                "bot.scenarios.production_craft_disconnect_resume.time.sleep",
                side_effect=lambda seconds: events.append(("sleep", seconds)),
            ),
            self.assertRaisesRegex(RuntimeError, "scenario failed"),
        ):
            with _reconnectable_session(FakeEnv()):
                raise RuntimeError("scenario failed")

        self.assertEqual(
            events,
            ["close", ("sleep", DISCONNECT_SETTLE_SECONDS)],
            "场景异常也必须先释放旧连接并完成 cleanup 窗口",
        )

    def test_new_bot_rejects_long_username(self):
        env = ScenarioEnv("127.0.0.1", 1, run_tag="12345678901234")
        with self.assertRaises(ValueError, msg="用户名超 16 字符必须在连接前报错"):
            env.new_bot("LongTag")

    def test_bot_rejects_long_username_before_connect(self):
        with self.assertRaises(ValueError):
            Bot("A" * 17, host="127.0.0.1", port=1)

    def test_validate_scenario_module_contract(self):
        good = types.SimpleNamespace(DESCRIPTION="d", MODULES=["m"], run=lambda env: None)
        validate_scenario_module("good", good)  # 不应抛
        for missing in ("DESCRIPTION", "MODULES", "run"):
            attrs = {"DESCRIPTION": "d", "MODULES": ["m"], "run": lambda env: None}
            del attrs[missing]
            with self.assertRaises(RuntimeError, msg=f"缺 {missing} 应被契约校验拒绝"):
                validate_scenario_module("bad", types.SimpleNamespace(**attrs))

    def test_discover_scenarios_finds_committed_set(self):
        names = set(discover_scenarios())
        expected = {
            "agent_ui_realm_gate_private_narration",
            "cmd_dev_give_feedback",
            "cultivation_realm_qi",
            "network_client_request_tolerance",
            "network_session_tolerance",
            "terrain_join_chunk_delivery",
            "terrain_north_rift_scorch_zone_identity",
        }
        self.assertTrue(
            expected <= names,
            f"已提交场景应全部被发现（模块更新必配场景的 CI 抓手），实际 {names}",
        )

    def test_north_rift_scenario_is_explicitly_dedicated(self):
        scenario = discover_scenarios()["terrain_north_rift_scorch_zone_identity"]
        self.assertFalse(
            scenario.DEFAULT_ENABLED,
            "north-rift preview 场景不得进入常规 --all server，避免 ViewDistance(32) 扩散",
        )
        self.assertEqual(
            scenario.REQUIRED_ENV,
            NORTH_RIFT_REQUIRED_ENV,
            "runner SKIP 提示与场景执行门必须引用同一个显式环境变量",
        )

    def test_all_skips_north_rift_even_when_preview_env_is_present(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=NORTH_RIFT_REQUIRED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {NORTH_RIFT_REQUIRED_ENV: "1"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_not_called()
        self.assertIn("SKIP", output.getvalue())
        self.assertIn("skip=1", output.getvalue())

    def test_all_runs_ambient_only_when_fixture_ownership_is_declared(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="owned fixture",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=FIXTURE_OWNED_ENV,
            RUN_IN_ALL_WHEN_ENV=FIXTURE_OWNED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"npc_ambient_surface_resolution": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {FIXTURE_OWNED_ENV: "1"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_called_once()
        self.assertIn("PASS", output.getvalue())
        self.assertIn("pass=1", output.getvalue())

    def test_all_runs_fallback_only_when_owned_runtime_is_declared(self):
        self.assertGreaterEqual(
            MIN_CHUNKS_AFTER_CENTER,
            8,
            "fallback join gate 至少要观察 center 后 8 个真实 ChunkData",
        )
        scenario = discover_scenarios()["terrain_join_chunk_delivery"]
        self.assertFalse(scenario.DEFAULT_ENABLED)
        self.assertEqual(scenario.REQUIRED_ENV, FALLBACK_OWNED_ENV)
        self.assertEqual(scenario.RUN_IN_ALL_WHEN_ENV, FALLBACK_OWNED_ENV)

        run = mock.Mock()
        dedicated = types.SimpleNamespace(
            DESCRIPTION="owned fallback",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=FALLBACK_OWNED_ENV,
            RUN_IN_ALL_WHEN_ENV=FALLBACK_OWNED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_join_chunk_delivery": dedicated},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {FALLBACK_OWNED_ENV: "1"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_called_once()
        self.assertIn("PASS", output.getvalue())

    def test_all_skips_ambient_without_fixture_ownership(self):
        run = mock.Mock()
        scenario = types.SimpleNamespace(
            DESCRIPTION="owned fixture",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=FIXTURE_OWNED_ENV,
            RUN_IN_ALL_WHEN_ENV=FIXTURE_OWNED_ENV,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"npc_ambient_surface_resolution": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {FIXTURE_OWNED_ENV: "0"}, clear=False),
            mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0)
        run.assert_not_called()
        self.assertIn("SKIP", output.getvalue())

    def test_heart_demon_decision_scenario_declares_optin_env_hook(self):
        # review finding major-2：主链路场景必须声明 RUN_IN_ALL_WHEN_ENV（且不默认开启），
        # 由专用工作流下发该 env 才纳入 --all —— 没这个 hook 常规 CI 的 --all 永远跳过它。
        scenario = discover_scenarios()["cultivation_heart_demon_decision"]
        self.assertFalse(
            scenario.DEFAULT_ENABLED,
            "heart_demon_decision 是 30 分钟满进度门槛的长场景，不得进入常规 --all",
        )
        self.assertEqual(
            getattr(scenario, "RUN_IN_ALL_WHEN_ENV", None),
            "BONG_RUN_LONG_SCENARIOS",
            "场景必须声明统一的 RUN_IN_ALL_WHEN_ENV；bot-heart-demon-nightly 工作流下发它",
        )

    def test_heart_demon_decision_all_runs_only_when_optin_env_set(self):
        # pin runner 语义：env 未设/0 → SKIP（run 不被调用）；env=1 → 纳入 --all（run 被调）。
        run_env = "BONG_RUN_LONG_SCENARIOS"
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated heart-demon",
            MODULES=["cultivation"],
            DEFAULT_ENABLED=False,
            RUN_IN_ALL_WHEN_ENV=run_env,
            run=mock.Mock(),
        )
        for value, expected_included in (("1", True), ("0", False), ("", False)):
            scenario.run.reset_mock()
            output = io.StringIO()
            with (
                mock.patch.object(
                    scenario_runner,
                    "discover_scenarios",
                    return_value={"cultivation_heart_demon_decision": scenario},
                ),
                mock.patch.object(
                    scenario_runner, "check_server_reachable", return_value=True
                ),
                mock.patch.dict(os.environ, {run_env: value}, clear=False),
                mock.patch.object(sys, "argv", ["run_scenarios.py", "--all"]),
                redirect_stdout(output),
            ):
                result = scenario_runner.main()
            self.assertEqual(result, 0)
            if expected_included:
                scenario.run.assert_called_once()
                self.assertIn("pass=1", output.getvalue())
            else:
                scenario.run.assert_not_called()
                self.assertIn("skip=1", output.getvalue())

    def test_heart_demon_nightly_workflow_actually_sets_optin_env(self):
        # review finding major-2：pin「opt-in --all 环境钩子 + 工作流真正下发」的电线。
        # 工作流必须把 BONG_RUN_LONG_SCENARIOS 设为 1 并调用 bot-e2e.sh（--all），
        # 否则场景声明的 RUN_IN_ALL_WHEN_ENV 只是死钩子。
        wf_path = pathlib.Path(__file__).parent.parent.parent / ".github" / "workflows" / "bot-heart-demon-nightly.yml"
        text = wf_path.read_text(encoding="utf-8")
        self.assertIn("BONG_RUN_LONG_SCENARIOS", text)
        self.assertIn("BONG_RUN_LONG_SCENARIOS: \"1\"", text)
        self.assertIn("scripts/bot-e2e.sh", text)
        self.assertIn("on:", text)
        self.assertIn("workflow_dispatch", text)

    def test_explicit_scenario_without_required_env_fails_closed(self):
        run = mock.Mock()
        required_env = "BOT_E2E_TEST_DEDICATED"
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=required_env,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {required_env: "0"}, clear=False),
            mock.patch.object(
                sys,
                "argv",
                [
                    "run_scenarios.py",
                    "--scenario",
                    "terrain_north_rift_scorch_zone_identity",
                ],
            ),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 1, "显式场景缺 REQUIRED_ENV 必须以非零退出")
        run.assert_not_called()
        self.assertIn("ERROR", output.getvalue())
        self.assertIn(f"需 {required_env}=1", output.getvalue())

    def test_explicit_scenario_with_required_env_runs_normally(self):
        run = mock.Mock()
        required_env = "BOT_E2E_TEST_DEDICATED"
        scenario = types.SimpleNamespace(
            DESCRIPTION="dedicated",
            MODULES=["terrain"],
            DEFAULT_ENABLED=False,
            REQUIRED_ENV=required_env,
            run=run,
        )
        output = io.StringIO()
        with (
            mock.patch.object(
                scenario_runner,
                "discover_scenarios",
                return_value={"terrain_north_rift_scorch_zone_identity": scenario},
            ),
            mock.patch.object(scenario_runner, "check_server_reachable", return_value=True),
            mock.patch.dict(os.environ, {required_env: "1"}, clear=False),
            mock.patch.object(
                sys,
                "argv",
                [
                    "run_scenarios.py",
                    "--scenario",
                    "terrain_north_rift_scorch_zone_identity",
                ],
            ),
            redirect_stdout(output),
        ):
            result = scenario_runner.main()

        self.assertEqual(result, 0, "满足 REQUIRED_ENV 的显式场景应保留正常 PASS 语义")
        run.assert_called_once()
        self.assertIn("PASS", output.getvalue())
        self.assertIn("pass=1", output.getvalue())

    def test_scenario_default_enabled_recognizes_annotated_false(self):
        self.assertFalse(
            _scenario_default_enabled(
                ast.parse("DEFAULT_ENABLED: bool = False", mode="exec")
            )
        )
        self.assertTrue(
            _scenario_default_enabled(
                ast.parse("DEFAULT_ENABLED: bool = True", mode="exec")
            )
        )
        self.assertTrue(_scenario_default_enabled(ast.parse("VALUE = False", mode="exec")))

    def test_default_gameplay_scenarios_do_not_assert_raw_server_data_transport(self):
        scenarios_dir = pathlib.Path(__file__).parent / "scenarios"
        raw_server_data_calls = {
            "expect_server_data_payload",
            "server_data_payload_field",
            "server_data_payload_name",
        }
        failures: list[str] = []

        for path in scenarios_dir.glob("*.py"):
            if path.name.startswith("_"):
                continue
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            default_enabled = _scenario_default_enabled(tree)
            if not default_enabled:
                continue

            for node in ast.walk(tree):
                if not isinstance(node, ast.Call):
                    continue
                function_name = None
                if isinstance(node.func, ast.Attribute):
                    function_name = node.func.attr
                elif isinstance(node.func, ast.Name):
                    function_name = node.func.id
                if function_name in raw_server_data_calls:
                    # remains_loot 的负向契约必须扫描未注册的 RemainsSync(field 139)
                    # raw 帧；该场景同时扫描已解码 server_data，raw helper 只用于防止
                    # 未知 oneof 被静默吞掉。其他默认玩法场景仍禁止把 transport 当行为证据。
                    if (
                        path.name == "remains_loot_invalid_quiet_reject.py"
                        and function_name == "server_data_payload_name"
                    ):
                        continue
                    failures.append(
                        f"{path.name}:{node.lineno}: {function_name} 只识别 transport/oneof，"
                        "玩法验收必须断言 kind=server_data 的深解码字段"
                    )

        self.assertEqual(
            failures,
            [],
            "默认 gameplay 场景不得用 raw bong:server_data payload/oneof 充当行为证据：\n"
            + "\n".join(failures),
        )

    def test_decoder_acceptance_matrix_covers_every_default_server_data_assertion_type(self):
        scenarios_dir = pathlib.Path(__file__).parent / "scenarios"
        asserted_types: set[str] = set()

        for path in scenarios_dir.glob("*.py"):
            if path.name.startswith("_"):
                continue
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            default_enabled = _scenario_default_enabled(tree)
            if not default_enabled:
                continue

            for node in ast.walk(tree):
                if not isinstance(node, ast.Compare):
                    continue
                expressions = [node.left, *node.comparators]
                for expression in expressions:
                    if isinstance(expression, ast.Constant) and isinstance(expression.value, str):
                        if expression.value in set(proto_min.SERVER_DATA_PAYLOAD_NAMES.values()):
                            asserted_types.add(expression.value)

        deep_decoded_fields = set(proto_min.SERVER_DATA_PAYLOAD_DECODERS)

        field_for_name = {
            name: field for field, name in proto_min.SERVER_DATA_PAYLOAD_NAMES.items()
        }
        missing = sorted(
            payload_type
            for payload_type in asserted_types
            if field_for_name.get(payload_type) not in deep_decoded_fields
        )
        self.assertEqual(
            missing,
            [],
            "默认场景断言的 server_data 类型必须进入深解码 acceptance matrix；"
            f"asserted={sorted(asserted_types)}, missing={missing}",
        )

    def test_scenarios_do_not_reuse_literal_bot_tags(self):
        owners: dict[str, str] = {}
        scenarios_dir = pathlib.Path(__file__).parent / "scenarios"
        for path in scenarios_dir.glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if not (
                    isinstance(node, ast.Call)
                    and isinstance(node.func, ast.Attribute)
                    and node.func.attr == "new_bot"
                    and node.args
                    and isinstance(node.args[0], ast.Constant)
                    and isinstance(node.args[0].value, str)
                ):
                    continue
                tag = node.args[0].value
                previous = owners.setdefault(tag, path.name)
                self.assertEqual(
                    previous,
                    path.name,
                    f"bot tag {tag!r} 被 {previous} 与 {path.name} 跨场景复用；"
                    "持久玩家状态会污染后续场景",
                )

    def test_check_server_reachable(self):
        listener = socket.socket()
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        listener.settimeout(2.0)
        port = listener.getsockname()[1]
        accepted_connections: list[socket.socket] = []
        accept_errors: list[OSError] = []

        def accept_once() -> None:
            try:
                connection, _ = listener.accept()
            except OSError as error:
                accept_errors.append(error)
                return
            accepted_connections.append(connection)

        accepted = threading.Thread(target=accept_once)
        accepted.start()
        try:
            self.assertTrue(check_server_reachable("127.0.0.1", port, timeout=2.0))
        finally:
            accepted.join(timeout=2.5)
            listener.close()
            for connection in accepted_connections:
                connection.close()
        self.assertFalse(accepted.is_alive(), "reachability test accept helper must terminate")
        self.assertEqual(accept_errors, [])
        self.assertFalse(check_server_reachable("127.0.0.1", 1, timeout=0.5))

    def test_check_server_reachable_converts_socket_errors_to_false(self):
        with mock.patch("socket.create_connection", side_effect=OSError("unreachable")):
            self.assertFalse(check_server_reachable("invalid", 25565, timeout=0.01))

    def test_intent_payload_is_valid_json_utf8(self):
        # intent() 的 wire 形状：channel string + UTF-8 JSON —— 锁编码不锁语义
        body = mc.mc_string("bong:client_request") + json.dumps(
            {"v": 1, "type": "breakthrough_request"}
        ).encode("utf-8")
        reader = mc.Reader(body)
        self.assertEqual(reader.string(), "bong:client_request")
        self.assertEqual(
            json.loads(reader.rest()), {"v": 1, "type": "breakthrough_request"}
        )


def _bare_bot() -> Bot:
    """不开 socket 的 Bot——只测 _dispatch 解码与实体位置表状态机。"""
    import threading as _threading

    bot = Bot.__new__(Bot)
    bot.t0 = 0.0
    bot.events = []
    bot.entities = {}
    bot.player_names = {}
    bot.player_entity_uuids = {}
    bot._lock = _threading.RLock()
    bot._new_event = _threading.Condition(bot._lock)
    bot._send_lock = _threading.Lock()
    bot._closing = False
    bot.position = None
    bot.health = None
    bot.entity_id = None
    bot.disconnect_reason = None
    bot.chunk_count = 0
    return bot


class BotChatPacketTest(unittest.TestCase):
    def test_chat_packet_uses_unix_milliseconds_at_protocol_boundaries(self):
        cases = [
            (0, 0),
            (1_000_000_000, 1_000),
            (1_999_000_000, 1_999),
            (1_712_345_700_999_000_000, 1_712_345_700_999),
        ]

        for time_ns, expected_millis in cases:
            with self.subTest(expected_millis=expected_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )

                with mock.patch("bot.bot.time.time_ns", return_value=time_ns):
                    bot.chat("窗口验真")

                self.assertEqual(
                    len(sent),
                    1,
                    f"一次 Bot.chat 必须只发一帧 C2S chat，实际 sent={sent!r}",
                )
                packet_id, body = sent[0]
                self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)

                reader = mc.Reader(body)
                self.assertEqual(reader.string(), "窗口验真")
                self.assertEqual(
                    reader.i64(),
                    expected_millis,
                    "Minecraft 1.20.1 ChatMessageC2s timestamp 必须是 Unix 毫秒；"
                    "0/1000/1999 与真实 13 位值均不得误写成 Unix 秒",
                )
                self.assertEqual(reader.i64(), 0, "offline bot 的 salt 保持 0")
                self.assertFalse(reader.boolean(), "offline bot 不携带聊天签名")
                self.assertEqual(reader.varint(), 0, "message_count 保持 0")
                self.assertEqual(
                    reader.rest(),
                    b"\x00\x00\x00",
                    "acknowledged 必须保留协议 763 的 20-bit 固定 BitSet",
                )

    def test_chat_packet_can_model_a_forged_future_client_timestamp(self):
        forged_future_millis = 1_712_432_100_999
        bot = _bare_bot()
        sent: list[tuple[int, bytes]] = []
        bot._send = lambda packet_id, body, _sent=sent: _sent.append((packet_id, body))

        with mock.patch(
            "bot.bot.time.time_ns",
            side_effect=AssertionError("explicit protocol timestamps must not read the local clock"),
        ):
            bot.chat("未来时间验真", timestamp_millis=forged_future_millis)

        self.assertEqual(len(sent), 1)
        packet_id, body = sent[0]
        self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)
        reader = mc.Reader(body)
        self.assertEqual(reader.string(), "未来时间验真")
        self.assertEqual(
            reader.i64(),
            forged_future_millis,
            "测试 Bot 必须能按原版 C2S 字节精确模拟客户端未来时间，而不是在 harness 内预先钳制",
        )

    def test_chat_packet_encodes_signed_i64_timestamp_boundaries(self):
        cases = [
            -(2**63),
            -1,
            2**63 - 1,
        ]
        for timestamp_millis in cases:
            with self.subTest(timestamp_millis=timestamp_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )
                bot.chat("边界时间验真", timestamp_millis=timestamp_millis)
                self.assertEqual(
                    len(sent),
                    1,
                    f"合法 signed i64 必须原样发出，timestamp={timestamp_millis}, sent={sent!r}",
                )
                packet_id, body = sent[0]
                self.assertEqual(packet_id, mc.C2S_CHAT_MESSAGE)
                reader = mc.Reader(body)
                self.assertEqual(reader.string(), "边界时间验真")
                self.assertEqual(
                    reader.i64(),
                    timestamp_millis,
                    "signed i64 timestamp 边界必须可原样 encode/decode",
                )
                self.assertEqual(reader.i64(), 0, "offline bot 的 salt 保持 0")

    def test_chat_packet_rejects_signed_i64_timestamp_overflow(self):
        overflow_cases = [
            -(2**63) - 1,
            2**63,
        ]
        for timestamp_millis in overflow_cases:
            with self.subTest(timestamp_millis=timestamp_millis):
                bot = _bare_bot()
                sent: list[tuple[int, bytes]] = []
                bot._send = lambda packet_id, body, _sent=sent: _sent.append(
                    (packet_id, body)
                )
                with self.assertRaises(
                    (OverflowError, struct.error, ValueError),
                    msg=(
                        "越界 timestamp 必须在编码阶段失败，"
                        f"timestamp={timestamp_millis}"
                    ),
                ):
                    bot.chat("越界时间验真", timestamp_millis=timestamp_millis)
                self.assertEqual(
                    sent,
                    [],
                    f"越界 timestamp 不得发送任何包，timestamp={timestamp_millis}, sent={sent!r}",
                )


class RespawnDecodeTest(unittest.TestCase):
    def test_respawn_exposes_authoritative_dimension_names(self):
        for dimension in ("minecraft:overworld", "bong:tsy"):
            with self.subTest(dimension=dimension):
                bot = _bare_bot()
                body = (
                    mc.write_varint(mc.S2C_RESPAWN)
                    + mc.mc_string(dimension)
                    + mc.mc_string(dimension)
                )

                bot._dispatch(body)

                self.assertEqual(len(bot.events), 1)
                event = bot.events[0]
                self.assertEqual(event.kind, "respawn")
                self.assertEqual(event.data["dimension_type_name"], dimension)
                self.assertEqual(event.data["dimension_name"], dimension)


class EntityTrackingTest(unittest.TestCase):
    """实体位置表 pin：spawn 建 / rel-move 累积(Δ=i16/4096) / teleport 覆写 / destroy 删。

    近战场景（combat_weapon_equip_damage 追击式采样）依赖 entity_pos 追活体
    NPC——拿 spawn 坐标当靶在 CI 时序下必 whiff。"""

    ENTITY_UUID = "00112233-4455-6677-8899-aabbccddeeff"

    def _spawn(self, bot, eid=7, x=10.0, y=64.0, z=-3.0):
        body = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(eid)
            + uuid.UUID(self.ENTITY_UUID).bytes
            + mc.write_varint(1)
            + struct.pack(">ddd", x, y, z)
        )
        bot._dispatch(body)

    def test_spawn_registers_position_and_uuid(self):
        bot = _bare_bot()
        self._spawn(bot)
        self.assertEqual(
            bot.entity_pos(7), (10.0, 64.0, -3.0),
            "entity_spawn 应把实体坐标登记进位置表（追击采样的起点）",
        )
        self.assertEqual(bot.events[-1].data["uuid"], self.ENTITY_UUID)
        self.assertEqual(bot.events[-1].data["type"], 1)

    def test_rel_move_accumulates_quarter_4096(self):
        bot = _bare_bot()
        self._spawn(bot)
        # Δ = 4096 → +1.0 block（wiki.vg：delta 编码为 (cur-prev)*4096 的 i16）
        body = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(7)
            + struct.pack(">hhh", 4096, -2048, 0)
            + b"\x01"
        )
        bot._dispatch(body)
        x, y, z = bot.entity_pos(7)
        self.assertEqual(bot.events[-1].kind, "entity_move")
        self.assertEqual(bot.events[-1].data["entity_id"], 7)
        self.assertAlmostEqual(bot.events[-1].data["x"], 11.0, places=6)
        self.assertAlmostEqual(x, 11.0, places=6, msg="dx=4096/4096 应 +1.0")
        self.assertAlmostEqual(y, 63.5, places=6, msg="dy=-2048/4096 应 -0.5")
        self.assertAlmostEqual(z, -3.0, places=6)

    def test_rel_move_unknown_entity_is_ignored(self):
        bot = _bare_bot()
        body = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(99)
            + struct.pack(">hhh", 4096, 0, 0)
            + b"\x01"
        )
        bot._dispatch(body)  # 未知实体的 rel-move 无锚点，不得 KeyError
        self.assertIsNone(bot.entity_pos(99), "未 spawn 实体的 rel-move 应被忽略")

    def test_teleport_overwrites_absolute(self):
        bot = _bare_bot()
        self._spawn(bot)
        body = (
            mc.write_varint(mc.S2C_ENTITY_TELEPORT)
            + mc.write_varint(7)
            + struct.pack(">ddd", -100.0, 70.0, 200.0)
            + b"\x00\x00\x01"
        )
        bot._dispatch(body)
        self.assertEqual(
            bot.entity_pos(7), (-100.0, 70.0, 200.0),
            "teleport 应绝对覆写位置（不叠加）",
        )
        self.assertEqual(bot.events[-1].kind, "entity_move")
        self.assertEqual(
            bot.events[-1].data,
            {"entity_id": 7, "x": -100.0, "y": 70.0, "z": 200.0},
        )

    def test_destroy_removes_entity(self):
        bot = _bare_bot()
        self._spawn(bot)
        body = mc.write_varint(mc.S2C_ENTITIES_DESTROY) + mc.write_varint(1) + mc.write_varint(7)
        bot._dispatch(body)
        self.assertIsNone(bot.entity_pos(7), "destroy 后实体应从位置表移除（追击应停止）")


class PlayerIdentityTrackingTest(unittest.TestCase):
    """PlayerList + PlayerSpawn 共同提供用户名、UUID、协议 entity ID 的权威关联。"""

    PLAYER_UUID = "12345678-1234-5678-9abc-def012345678"

    def _player_list_add(self, bot, username="Alice"):
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + bytes([0x01])
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string(username)
            + mc.write_varint(0)
        )
        bot._dispatch(body)

    def _player_spawn(self, bot, entity_id=42):
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(entity_id)
            + raw_uuid
            + struct.pack(">ddd", 8.5, 66.0, -4.25)
            + bytes([64, 192])
        )
        bot._dispatch(body)

    def test_player_list_add_records_uuid_to_username(self):
        bot = _bare_bot()

        self._player_list_add(bot)

        self.assertEqual(bot.player_names[self.PLAYER_UUID], "Alice")
        event = bot.events[-1]
        self.assertEqual(event.kind, "player_list")
        self.assertEqual(
            event.data["entries"],
            [{"uuid": self.PLAYER_UUID, "username": "Alice", "properties": []}],
        )

    def test_player_spawn_joins_entity_id_uuid_username_and_position(self):
        bot = _bare_bot()
        self._player_list_add(bot)

        self._player_spawn(bot)

        self.assertEqual(bot.entity_pos(42), (8.5, 66.0, -4.25))
        self.assertEqual(bot.player_entity_uuids[42], self.PLAYER_UUID)
        event = bot.events[-1]
        self.assertEqual(event.kind, "player_spawn")
        self.assertEqual(event.data["username"], "Alice")
        self.assertEqual(event.data["yaw"], 64)
        self.assertEqual(event.data["pitch"], 192)

    def test_player_spawn_without_prior_list_is_still_observable(self):
        bot = _bare_bot()

        self._player_spawn(bot)

        event = bot.events[-1]
        self.assertEqual(event.kind, "player_spawn")
        self.assertIsNone(event.data["username"])
        self.assertEqual(event.data["uuid"], self.PLAYER_UUID)

    def test_destroy_removes_player_entity_identity_but_preserves_list_name(self):
        bot = _bare_bot()
        self._player_list_add(bot)
        self._player_spawn(bot)

        bot._dispatch(
            mc.write_varint(mc.S2C_ENTITIES_DESTROY)
            + mc.write_varint(1)
            + mc.write_varint(42)
        )

        self.assertIsNone(bot.entity_pos(42))
        self.assertNotIn(42, bot.player_entity_uuids)
        self.assertEqual(bot.player_names[self.PLAYER_UUID], "Alice")

    def test_player_remove_clears_stale_uuid_name_mapping(self):
        bot = _bare_bot()
        self._player_list_add(bot)
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes

        bot._dispatch(
            mc.write_varint(mc.S2C_PLAYER_REMOVE)
            + mc.write_varint(1)
            + raw_uuid
        )

        self.assertNotIn(self.PLAYER_UUID, bot.player_names)
        self.assertEqual(bot.events[-1].kind, "player_remove")
        self.assertEqual(bot.events[-1].data["uuids"], [self.PLAYER_UUID])

    def test_player_remove_clears_stale_uuid_name_mapping(self):
        bot = _bare_bot()
        self._player_list_add(bot)
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes

        bot._dispatch(
            mc.write_varint(mc.S2C_PLAYER_REMOVE)
            + mc.write_varint(1)
            + raw_uuid
        )

        self.assertNotIn(self.PLAYER_UUID, bot.player_names)
        self.assertEqual(bot.events[-1].kind, "player_remove")
        self.assertEqual(bot.events[-1].data["uuids"], [self.PLAYER_UUID])

    def test_player_remove_rejects_negative_count_without_emitting_event(self):
        bot = _bare_bot()
        with self.assertRaisesRegex(ValueError, "negative player remove count -1"):
            bot._dispatch(
                mc.write_varint(mc.S2C_PLAYER_REMOVE)
                + mc.write_varint(-1)
            )
        self.assertEqual(bot.events, [])


    def test_player_list_rejects_negative_property_count_without_partial_name(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x01"
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string("Alice")
            + mc.write_varint(-1)
        )
        with self.assertRaisesRegex(ValueError, "property count -1"):
            bot._dispatch(body)

        self.assertEqual(bot.events, [], "负 property count 不得产出 player_list 事件")
        self.assertEqual(
            bot.player_names,
            {},
            "完整 PlayerList entry 校验失败前不得写入 UUID→用户名映射",
        )

    def test_player_list_truncation_does_not_leave_partial_name_mapping(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x01"
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string("Alice")
            + mc.write_varint(1)
            + mc.mc_string("textures")
        )
        with self.assertRaises((IndexError, ValueError)):
            bot._dispatch(body)

        self.assertEqual(bot.events, [], "截断 PlayerList entry 不得产出事件")
        self.assertEqual(
            bot.player_names,
            {},
            "截断 PlayerList entry 不得残留 UUID→用户名映射",
        )

    def test_player_list_rejects_truncated_username_without_partial_mapping(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x01"
            + mc.write_varint(1)
            + raw_uuid
            + mc.write_varint(5)
            + b"Ali"
        )

        with self.assertRaisesRegex(ValueError, "string length 5 exceeds remaining bytes 3"):
            bot._dispatch(body)

        self.assertEqual(bot.events, [], "截断 username 不得产出 player_list 事件")
        self.assertEqual(bot.player_names, {}, "截断 username 不得污染 UUID→用户名映射")

    def test_player_list_rejects_negative_property_string_length(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x01"
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string("Alice")
            + mc.write_varint(1)
            + mc.write_varint(-1)
        )

        with self.assertRaisesRegex(ValueError, "string length -1 must be non-negative"):
            bot._dispatch(body)

        self.assertEqual(bot.events, [], "负 property string 长度不得产出 player_list 事件")
        self.assertEqual(
            bot.player_names,
            {},
            "负 property string 长度不得残留 UUID→用户名映射",
        )

    def test_player_list_rejects_truncated_display_name_transactionally(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x21"
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string("Alice")
            + mc.write_varint(0)
            + b"\x01"
            + mc.write_varint(8)
            + b'{"text"'
        )

        with self.assertRaisesRegex(ValueError, "string length 8 exceeds remaining bytes 7"):
            bot._dispatch(body)

        self.assertEqual(bot.events, [], "截断 display name 不得产出 player_list 事件")
        self.assertEqual(
            bot.player_names,
            {},
            "AddPlayer 同包的截断 display name 不得提前提交 UUID→用户名映射",
        )

    def test_combined_actions_follow_authoritative_valence_field_order(self):
        bot = _bare_bot()
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        # Valence PlayerListS2c encodes every selected action for an entry in bit order:
        # add_player → initialize_chat → game_mode → listed → latency → display_name.
        actions = 0x3F
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + bytes([actions])
            + mc.write_varint(1)
            + raw_uuid
            + mc.mc_string("Alice")
            + mc.write_varint(1)
            + mc.mc_string("textures")
            + mc.mc_string("base64")
            + b"\x01"
            + mc.mc_string("signed")
            + b"\x01"
            + raw_uuid
            + struct.pack(">q", 1234)
            + mc.write_varint(3)
            + b"key"
            + mc.write_varint(3)
            + b"sig"
            + mc.write_varint(1)
            + b"\x01"
            + mc.write_varint(37)
            + b"\x01"
            + mc.mc_string('{"text":"Alias"}')
        )

        bot._dispatch(body)

        self.assertEqual(bot.player_names[self.PLAYER_UUID], "Alice")
        self.assertEqual(bot.events[-1].data["actions"], actions)
        self.assertEqual(
            bot.events[-1].data["entries"],
            [
                {
                    "uuid": self.PLAYER_UUID,
                    "username": "Alice",
                    "properties": [
                        {
                            "name": "textures",
                            "value": "base64",
                            "signature": "signed",
                        }
                    ],
                    "initialize_chat": {
                        "has_chat_session": True,
                        "session_id": self.PLAYER_UUID,
                        "public_key_expiry": 1234,
                        "public_key": b"key",
                        "signature": b"sig",
                    },
                    "game_mode": 1,
                    "listed": True,
                    "latency": 37,
                    "display_name": '{"text":"Alias"}',
                }
            ],
        )

    def _initialize_chat_body(
        self,
        *,
        key_length: int,
        key_bytes: bytes = b"",
        signature_length: int | None = None,
        signature_bytes: bytes = b"",
    ) -> bytes:
        raw_uuid = uuid.UUID(self.PLAYER_UUID).bytes
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + b"\x02"
            + mc.write_varint(1)
            + raw_uuid
            + b"\x01"
            + raw_uuid
            + struct.pack(">q", 1234)
            + mc.write_varint(key_length)
            + key_bytes
        )
        if signature_length is not None:
            body += mc.write_varint(signature_length) + signature_bytes
        return body

    def test_initialize_chat_rejects_negative_key_length_without_emitting_entry(self):
        bot = _bare_bot()

        with self.assertRaisesRegex(ValueError, "chat key length -1"):
            bot._dispatch(self._initialize_chat_body(key_length=-1))

        self.assertEqual(bot.events, [], "畸形 chat key 长度不得产出 player_list 事件")

    def test_initialize_chat_rejects_key_length_beyond_remaining_bytes(self):
        bot = _bare_bot()

        with self.assertRaisesRegex(ValueError, "chat key length 4"):
            bot._dispatch(self._initialize_chat_body(key_length=4, key_bytes=b"key"))

        self.assertEqual(bot.events, [], "截断 chat key 不得产出 player_list 事件")

    def test_initialize_chat_rejects_negative_signature_length(self):
        bot = _bare_bot()

        with self.assertRaisesRegex(ValueError, "chat signature length -1"):
            bot._dispatch(
                self._initialize_chat_body(
                    key_length=3,
                    key_bytes=b"key",
                    signature_length=-1,
                )
            )

        self.assertEqual(bot.events, [], "负 chat signature 长度不得产出 player_list 事件")

    def test_initialize_chat_rejects_signature_length_beyond_remaining_bytes(self):
        bot = _bare_bot()

        with self.assertRaisesRegex(ValueError, "chat signature length 4"):
            bot._dispatch(
                self._initialize_chat_body(
                    key_length=3,
                    key_bytes=b"key",
                    signature_length=4,
                    signature_bytes=b"sig",
                )
            )

        self.assertEqual(bot.events, [], "截断 chat signature 不得产出 player_list 事件")

    def test_initialize_chat_accepts_exact_key_and_signature_boundaries(self):
        bot = _bare_bot()

        bot._dispatch(
            self._initialize_chat_body(
                key_length=3,
                key_bytes=b"key",
                signature_length=3,
                signature_bytes=b"sig",
            )
        )

        self.assertEqual(bot.events[-1].kind, "player_list")
        self.assertEqual(bot.events[-1].data["actions"], 0x02)
        self.assertEqual(
            bot.events[-1].data["entries"],
            [
                {
                    "uuid": self.PLAYER_UUID,
                    "initialize_chat": {
                        "has_chat_session": True,
                        "session_id": self.PLAYER_UUID,
                        "public_key_expiry": 1234,
                        "public_key": b"key",
                        "signature": b"sig",
                    },
                }
            ],
        )


def _pb_float32_field(number: int, value: float) -> bytes:
    import struct as _struct

    return mc.write_varint((number << 3) | 5) + _struct.pack("<f", value)


def _pb_int32_field(number: int, value: int) -> bytes:
    """protobuf `int32` 字段的正确 wire 编码——负值走 64-bit 补码变长编码（最长 10
    字节），不是 `mc.write_varint` 那种给实际 MC 协议 varint 用的 32-bit 掩码。
    forge station_pos_x/y/z 断言负坐标（末法残土常见）时必须用这个而非
    `_pb_varint_field`。"""
    return mc.write_varint(number << 3) + _pb_raw_varint(value & 0xFFFFFFFFFFFFFFFF)


def _pb_sint32_field(number: int, value: int) -> bytes:
    """protobuf `sint32`（zigzag）字段 wire 编码——WorkbenchOpen 坐标可为负。"""
    return mc.write_varint(number << 3) + _pb_raw_varint(
        (value << 1) ^ (value >> 63)
    )


def _proto_message_body(source: str, message_name: str) -> str:
    match = re.search(rf"\bmessage\s+{re.escape(message_name)}\s*\{{", source)
    if match is None:
        raise AssertionError(f"authoritative proto missing message {message_name}")
    start = match.end() - 1
    depth = 0
    for index in range(start, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return source[start + 1 : index]
    raise AssertionError(f"authoritative proto message {message_name} has no closing brace")


def _proto_field_signature(message_body: str, field_name: str) -> tuple[str, int]:
    match = re.search(
        rf"^\s*(?:optional\s+|repeated\s+)?([A-Za-z_][\w.]*)\s+"
        rf"{re.escape(field_name)}\s*=\s*(\d+)\s*;",
        message_body,
        flags=re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"authoritative proto missing field {field_name}")
    return match.group(1), int(match.group(2))


def _proto_field_names(message_body: str) -> set[str]:
    return {
        match.group(1)
        for match in re.finditer(
            r"^\s*(?:(?:optional|repeated)\s+)?[A-Za-z_][\w.]*\s+"
            r"([A-Za-z_][\w]*)\s*=\s*\d+\s*;",
            message_body,
            flags=re.MULTILINE,
        )
    }


def _proto_field_metadata(message_body: str, field_name: str) -> tuple[str, int, str]:
    match = re.search(
        rf"^\s*((?:optional|repeated)\s+)?([A-Za-z_][\w.]*)\s+"
        rf"{re.escape(field_name)}\s*=\s*(\d+)\s*;",
        message_body,
        flags=re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"authoritative proto missing field {field_name}")
    cardinality = (match.group(1) or "").strip() or "single"
    return match.group(2), int(match.group(3)), cardinality


class ZoneInfoProtoDecodeTest(unittest.TestCase):
    """zone_info field 4 的 wire 契约饱和 pin。"""

    def _decode(self, message: bytes) -> dict:
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_ZONE_INFO_FIELD, message)
        )
        self.assertIsNotNone(decoded, "envelope field 4 必须分发到 zone_info decoder")
        return decoded

    def test_decoder_constants_match_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        zone_info = _proto_message_body(source, "ZoneInfo")

        self.assertEqual(
            _proto_field_signature(envelope, "zone_info"),
            ("ZoneInfo", proto_min.SERVER_DATA_ZONE_INFO_FIELD),
            "Bot envelope field 4 常量必须与权威 ServerDataEnvelope.zone_info 对齐",
        )
        expected_fields = {
            "zone": ("string", proto_min.ZONE_INFO_ZONE_FIELD),
            "spirit_qi": ("double", proto_min.ZONE_INFO_SPIRIT_QI_FIELD),
            "danger_level": ("uint32", proto_min.ZONE_INFO_DANGER_LEVEL_FIELD),
            "status": ("string", proto_min.ZONE_INFO_STATUS_FIELD),
            "active_events": ("string", proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD),
            "perception_text": ("string", proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD),
        }
        for field_name, expected in expected_fields.items():
            with self.subTest(field=field_name):
                self.assertEqual(
                    _proto_field_signature(zone_info, field_name),
                    expected,
                    f"Bot ZoneInfo.{field_name} 常量必须与权威 proto 对齐",
                )

    def test_happy_path_decodes_negative_qi_repeated_events_and_perception(self):
        message = (
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "blood_valley")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, -0.42)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 3)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Collapsed")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "beast_tide")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "realm_collapse")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "灵气几近断绝")
        )

        self.assertEqual(
            self._decode(message),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "blood_valley",
                "spirit_qi": -0.42,
                "danger_level": 3,
                "status": "Collapsed",
                "active_events": ["beast_tide", "realm_collapse"],
                "perception_text": "灵气几近断绝",
            },
        )

    def test_empty_message_uses_protobuf_defaults(self):
        self.assertEqual(
            self._decode(b""),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "",
                "spirit_qi": 0.0,
                "danger_level": 0,
                "status": "",
                "active_events": [],
                "perception_text": None,
            },
            "空 ZoneInfo 是合法 protobuf；repeated 应为空表，optional 应保留 absent=None",
        )

    def test_optional_perception_distinguishes_absent_from_present_empty(self):
        absent = self._decode(_pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "spawn"))
        present_empty = self._decode(
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "spawn")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "")
        )

        self.assertIsNone(absent["perception_text"])
        self.assertEqual(
            present_empty["perception_text"],
            "",
            "proto3 optional string 的 present-empty 不得退化成 absent=None",
        )

    def test_duplicate_scalars_use_last_value_and_repeated_keeps_wire_order(self):
        message = (
            _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "old_zone")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "first")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.1)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 1)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "Normal")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "old")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "")
            + _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "new_zone")
            + _pb_fixed64(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 0.9)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 7)
            + _pb_string(proto_min.ZONE_INFO_STATUS_FIELD, "FutureStatus")
            + _pb_string(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, "first")
            + _pb_string(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, "new")
        )
        decoded = self._decode(message)

        self.assertEqual(decoded["zone"], "new_zone")
        self.assertEqual(decoded["spirit_qi"], 0.9)
        self.assertEqual(decoded["danger_level"], 7)
        self.assertEqual(decoded["status"], "FutureStatus")
        self.assertEqual(decoded["active_events"], ["first", "", "first"])
        self.assertEqual(decoded["perception_text"], "new")

    def test_danger_uint32_boundaries_decode_without_signed_wrap(self):
        for value in (0, 7, 2**32 - 1):
            with self.subTest(value=value):
                decoded = self._decode(
                    _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, value)
                )
                self.assertEqual(
                    decoded["danger_level"],
                    value,
                    f"uint32 danger_level={value} 不得发生有符号回绕",
                )

    def test_wrong_wire_types_are_ignored_instead_of_impersonating_fields(self):
        message = (
            _pb_fixed64(proto_min.ZONE_INFO_ZONE_FIELD, 1.0)
            + _pb_varint(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 1)
            + _pb_string(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, "7")
            + _pb_varint(proto_min.ZONE_INFO_STATUS_FIELD, 1)
            + _pb_fixed64(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, 1.0)
            + _pb_varint(proto_min.ZONE_INFO_PERCEPTION_TEXT_FIELD, 1)
        )

        self.assertEqual(
            self._decode(message),
            {
                "v": 1,
                "type": "zone_info",
                "zone": "",
                "spirit_qi": 0.0,
                "danger_level": 0,
                "status": "",
                "active_events": [],
                "perception_text": None,
            },
            "错误 wire type 必须按 protobuf unknown-field 语义忽略",
        )

    def test_unknown_fields_of_every_supported_wire_type_are_ignored(self):
        message = (
            _pb_varint(200, 99)
            + _pb_fixed64(201, 12.5)
            + _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "rift_mouth_north_002")
            + _pb_string(202, "future")
            + _pb_float32_field(203, 0.75)
            + _pb_varint(proto_min.ZONE_INFO_DANGER_LEVEL_FIELD, 5)
        )
        decoded = self._decode(message)

        self.assertEqual(decoded["zone"], "rift_mouth_north_002")
        self.assertEqual(decoded["danger_level"], 5)
        self.assertEqual(decoded["active_events"], [])

    def test_unknown_envelope_fields_do_not_hide_later_zone_info(self):
        envelope = (
            _pb_varint(199, 1)
            + _pb_bytes(200, b"\xff\x00future")
            + _pb_message(
                proto_min.SERVER_DATA_ZONE_INFO_FIELD,
                _pb_string(proto_min.ZONE_INFO_ZONE_FIELD, "north_waste_east_scorch"),
            )
            + _pb_float32_field(201, 0.5)
        )
        decoded = proto_min.decode_server_data_envelope(envelope)

        self.assertIsNotNone(decoded)
        self.assertEqual(decoded["type"], "zone_info")
        self.assertEqual(decoded["zone"], "north_waste_east_scorch")

    def test_zone_info_envelope_with_wrong_wire_type_is_not_dispatched(self):
        self.assertIsNone(
            proto_min.decode_server_data_envelope(
                _pb_varint(proto_min.SERVER_DATA_ZONE_INFO_FIELD, 1)
            ),
            "oneof message field 4 的 varint 不得冒充 zone_info",
        )

    def test_truncated_nested_fixed64_is_rejected(self):
        truncated = (
            _pb_key(proto_min.ZONE_INFO_SPIRIT_QI_FIELD, 1) + b"\x00" * 7
        )
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated fixed64",
            msg="ZoneInfo.spirit_qi fixed64 少一字节必须报协议错误",
        ):
            self._decode(truncated)

    def test_truncated_nested_string_is_rejected(self):
        truncated = (
            _pb_key(proto_min.ZONE_INFO_ACTIVE_EVENTS_FIELD, 2)
            + _pb_raw_varint(3)
            + b"ab"
        )
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated length-delimited field",
            msg="active_events 声明 3 字节却只给 2 字节必须报协议错误",
        ):
            self._decode(truncated)

    def test_unsupported_nested_wire_type_is_rejected(self):
        malformed = _pb_key(7, 3)
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "unsupported wire type 3",
            msg="group wire type 不在 minimal decoder 支持集内，必须显式报错",
        ):
            self._decode(malformed)

    def test_public_payload_decoder_turns_malformed_zone_info_into_none(self):
        malformed = (
            _pb_key(proto_min.SERVER_DATA_ZONE_INFO_FIELD, 2)
            + _pb_raw_varint(5)
            + b"ab"
        )
        self.assertIsNone(
            decode_server_data_payload(malformed),
            "Bot 公共观察面遇到截断 zone_info 应返回 None，而不是拖垮 reader thread",
        )


class ProdConsumeDecodeTest(unittest.TestCase):
    """三产三用 payload 解码 pin：envelope oneof tag 与字段号对齐 proto/bong/envelope.proto.

    这些解码器是 production_*/combat_*/cultivation_pill 场景的观察面地基——
    tag 或字段号漂移会让场景从「锁契约」退化成「永远超时」。
    """

    def test_player_state_decoder_constants_match_authoritative_proto(self):
        proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/envelope.proto"
        source = proto_path.read_text(encoding="utf-8")
        envelope = _proto_message_body(source, "ServerDataEnvelope")
        player_state = _proto_message_body(source, "PlayerState")

        self.assertEqual(
            _proto_field_signature(envelope, "player_state"),
            ("PlayerState", proto_min.SERVER_DATA_PLAYER_STATE_FIELD),
            "Bot envelope 分发常量必须与权威 ServerDataEnvelope.player_state 对齐",
        )
        self.assertEqual(
            _proto_field_signature(player_state, "realm"),
            ("Realm", proto_min.PLAYER_STATE_REALM_FIELD),
            "Bot realm 常量及 varint wire type 必须与权威 PlayerState 对齐",
        )
        self.assertEqual(
            _proto_field_signature(player_state, "spirit_qi"),
            ("double", proto_min.PLAYER_STATE_SPIRIT_QI_FIELD),
            "Bot spirit_qi 常量及 fixed64 wire type 必须与权威 PlayerState 对齐",
        )
        self.assertEqual(
            _proto_field_signature(player_state, "spirit_qi_max"),
            ("double", proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD),
            "Bot spirit_qi_max 常量及 fixed64 wire type 必须与权威 PlayerState 对齐",
        )

    def test_player_state_realm_decoder_matches_authoritative_enum(self):
        common_proto_path = pathlib.Path(__file__).parents[2] / "proto/bong/common.proto"
        common_source = common_proto_path.read_text(encoding="utf-8")
        realm_body = re.search(
            r"\benum\s+Realm\s*\{(?P<body>.*?)\}",
            common_source,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(realm_body, "权威 common.proto 必须声明 Realm enum")
        authoritative = {
            int(number): name.removeprefix("REALM_").title()
            for name, number in re.findall(
                r"^\s*(REALM_[A-Z_]+)\s*=\s*(\d+)\s*;",
                realm_body.group("body"),
                flags=re.MULTILINE,
            )
        }
        self.assertEqual(
            proto_min.PLAYER_STATE_REALM_NAMES,
            authoritative,
            "Bot PlayerState realm 名称映射必须完全派生自权威 Realm enum",
        )
        for wire_value, expected_name in authoritative.items():
            with self.subTest(wire_value=wire_value):
                msg = _pb_varint_field(proto_min.PLAYER_STATE_REALM_FIELD, wire_value)
                decoded = proto_min.decode_server_data_envelope(
                    _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
                )
                self.assertEqual(decoded["realm"], expected_name)

    def test_player_state_unknown_realm_stays_observable(self):
        msg = _pb_varint_field(proto_min.PLAYER_STATE_REALM_FIELD, 77)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertEqual(decoded["realm"], "unknown_77")

    def test_player_state_wrong_realm_wire_type_uses_default(self):
        msg = _pb_len_field(proto_min.PLAYER_STATE_REALM_FIELD, b"Induce")
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertEqual(decoded["realm"], "Unspecified")

    def test_player_state_tag5_decodes_authoritative_qi(self):
        msg = (
            _pb_varint_field(proto_min.PLAYER_STATE_REALM_FIELD, 2)
            + _pb_fixed64(proto_min.PLAYER_STATE_SPIRIT_QI_FIELD, 65.0)
            + _pb_fixed64(proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD, 100.0)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertIsNotNone(
            decoded,
            "envelope tag 5 必须解码为 player_state，实际返回 None",
        )
        self.assertEqual(
            decoded["type"],
            "player_state",
            f"envelope tag 5 应分发到 player_state，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["realm"],
            "Induce",
            f"PlayerState.realm 必须读取 varint field 2，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi"],
            65.0,
            f"PlayerState.spirit_qi 必须读取 fixed64 field 3，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi_max"],
            100.0,
            f"PlayerState.spirit_qi_max 必须读取 fixed64 field 11，实际 payload={decoded}",
        )

    def test_player_state_missing_qi_fields_use_protobuf_zero_defaults(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, b"")
        )
        self.assertIsNotNone(
            decoded,
            "空 player_state message 仍是合法 protobuf，实际返回 None",
        )
        self.assertEqual(
            decoded["realm"],
            "Unspecified",
            f"缺失 varint field 2 应使用 protobuf 默认 0，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi"],
            0.0,
            f"缺失 fixed64 field 3 应使用 protobuf 默认 0，实际 payload={decoded}",
        )
        self.assertEqual(
            decoded["spirit_qi_max"],
            0.0,
            f"缺失 fixed64 field 11 应使用 protobuf 默认 0，实际 payload={decoded}",
        )

    def test_player_state_wrong_qi_wire_type_is_ignored(self):
        msg = _pb_varint_field(
            proto_min.PLAYER_STATE_SPIRIT_QI_FIELD, 65
        ) + _pb_varint_field(proto_min.PLAYER_STATE_SPIRIT_QI_MAX_FIELD, 100)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, msg)
        )
        self.assertIsNotNone(
            decoded,
            "wire type 错误的 player_state envelope 仍应被识别，实际返回 None",
        )
        self.assertEqual(
            (decoded["spirit_qi"], decoded["spirit_qi_max"]),
            (0.0, 0.0),
            f"field 3/11 的 varint 不得冒充 fixed64，实际 payload={decoded}",
        )

    def test_player_state_truncated_fixed64_is_rejected(self):
        truncated = mc.write_varint(
            (proto_min.PLAYER_STATE_SPIRIT_QI_FIELD << 3) | 1
        ) + b"\x00" * 7
        with self.assertRaisesRegex(
            proto_min.ProtoDecodeError,
            "truncated fixed64",
            msg="PlayerState fixed64 field 3 截断时必须报协议错误",
        ):
            proto_min.decode_server_data_envelope(
                _pb_message(proto_min.SERVER_DATA_PLAYER_STATE_FIELD, truncated)
            )

    def test_craft_session_state_tag22(self):
        msg = (
            _pb_varint_field(3, 1)
            + _pb_len_field(4, b"workbench.weapon.stone_knife")
            + _pb_varint_field(5, 10)
            + _pb_varint_field(6, 400)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(22, msg))
        self.assertEqual(
            decoded["type"], "craft_session_state",
            "envelope tag 22 应分发到 craft_session_state（envelope.proto oneof）",
        )
        self.assertTrue(decoded["active"], "field3=1 应解为 active=True")
        self.assertEqual(
            decoded["recipe_id"], "workbench.weapon.stone_knife",
            "CraftSessionState.recipe_id 是 field 4（optional string）",
        )
        self.assertEqual(
            decoded["elapsed_ticks"], 10, "CraftSessionState.elapsed_ticks 是 field 5"
        )
        self.assertEqual(
            decoded["total_ticks"], 400, "CraftSessionState.total_ticks 是 field 6"
        )

    def test_craft_outcome_completed_tag23(self):
        completed = (
            _pb_len_field(3, b"workbench.weapon.stone_knife")
            + _pb_len_field(4, b"stone_knife")
            + _pb_varint_field(5, 1)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(23, _pb_len_field(1, completed))
        )
        self.assertEqual(
            decoded["type"], "craft_outcome", "envelope tag 23 应分发到 craft_outcome"
        )
        self.assertEqual(
            decoded["outcome"], "completed", "oneof field 1 = CraftOutcomeCompleted"
        )
        self.assertEqual(
            decoded["output_template"], "stone_knife",
            "CraftOutcomeCompleted.output_template 是 field 4",
        )
        self.assertEqual(decoded["output_count"], 1, "output_count 是 field 5")

    def test_craft_outcome_failed_branch(self):
        failed = (
            _pb_len_field(3, b"r")
            + _pb_varint_field(4, 2)
            + _pb_varint_field(5, 7)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(23, _pb_len_field(2, failed))
        )
        self.assertEqual(decoded["outcome"], "failed", "oneof field 2 = CraftOutcomeFailed")
        self.assertEqual(decoded["reason"], 2, "CraftOutcomeFailed.reason 是 field 4（enum）")
        self.assertEqual(
            decoded["material_returned"],
            7,
            "CraftOutcomeFailed.material_returned 是 field 5，Bot 必须核对真实退款数",
        )

    def test_dropped_loot_sync_tag81_decodes_pickup_identity(self):
        initial_qi_bits = 0x42C50001
        initial_qi = struct.unpack("<f", struct.pack("<I", initial_qi_bits))[0]
        freshness = (
            _pb_varint(1, 123)
            + _pb_fixed32(2, initial_qi)
            + _pb_string(3, "Decay")
            + _pb_string(4, "ling_mu_gun_v1")
            + _pb_varint(5, 17)
            + _pb_varint(6, 140)
        )
        item = (
            _pb_varint(1, 77)
            + _pb_string(2, "fan_tie")
            + _pb_varint(9, 1)
            + _pb_message(22, freshness)
        )
        entry = (
            _pb_varint(1, 77)
            + _pb_string(2, "overflow:fan_tie")
            + _pb_varint(3, 0)
            + _pb_varint(4, 0)
            + _pb_fixed64(5, 8.0)
            + _pb_fixed64(6, 65.0)
            + _pb_fixed64(7, -2.0)
            + _pb_message(8, item)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(81, _pb_message(1, entry))
        )
        self.assertEqual(
            decoded["type"],
            "dropped_loot_sync",
            "envelope tag 81 应分发到 dropped_loot_sync",
        )
        self.assertEqual(len(decoded["drops"]), 1, "sync field 1 应解出一条掉落")
        drop = decoded["drops"][0]
        self.assertEqual(drop["instance_id"], 77, "instance_id 用于 pickup intent")
        self.assertEqual(drop["item"]["item_id"], "fan_tie")
        self.assertEqual(drop["item"]["stack_count"], 1)
        self.assertEqual(drop["world_pos"], [8.0, 65.0, -2.0])
        self.assertEqual(
            drop["item"]["freshness"],
            {
                "created_at_tick": 123,
                "initial_qi": initial_qi,
                "track": "Decay",
                "profile": "ling_mu_gun_v1",
                "frozen_accumulated": 17,
                "frozen_since_tick": 140,
            },
            "dropped_loot_sync 必须保留完整 freshness，拾取后才能对拍同一实例 NBT",
        )
        self.assertEqual(
            struct.pack("<f", drop["item"]["freshness"]["initial_qi"]),
            struct.pack("<I", initial_qi_bits),
            "Bot 必须逐 bit 保留 Rust Freshness.initial_qi 的 f32 wire 值",
        )

    def test_inventory_item_without_freshness_decodes_none(self):
        item = _pb_varint(1, 77) + _pb_string(2, "fan_tie") + _pb_varint(9, 1)
        entry = _pb_varint(1, 77) + _pb_message(8, item)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(81, _pb_message(1, entry))
        )
        self.assertIsNone(decoded["drops"][0]["item"]["freshness"])

    def test_inventory_item_decodes_forge_quality_for_trade_identity_evidence(self):
        forge_quality = struct.unpack("<f", struct.pack("<I", 0x3F666667))[0]
        item = (
            _pb_varint(1, 77)
            + _pb_string(2, "forged_blade")
            + _pb_varint(9, 1)
            + _pb_fixed32(17, forge_quality)
        )
        entry = _pb_varint(1, 77) + _pb_message(8, item)
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(81, _pb_message(1, entry))
        )
        self.assertEqual(decoded["drops"][0]["item"]["forge_quality"], forge_quality)

    def test_lumber_progress_tag29_decodes_terminal_contract(self):
        progress = (
            _pb_string(1, "offline:wood")
            + _pb_varint(2, (1 << 64) - 1292)
            + _pb_varint(3, 73)
            + _pb_varint(4, 1519)
            + _pb_fixed64(5, 1.0)
            + _pb_varint(6, 0)
            + _pb_varint(7, 1)
            + _pb_string(8, "背包已满，灵木原木已落地 ×3")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(29, progress))
        self.assertEqual(decoded["type"], "lumber_progress")
        self.assertEqual(decoded["session_id"], "offline:wood")
        self.assertEqual(decoded["log_pos"], [-1292, 73, 1519])
        self.assertEqual(decoded["progress"], 1.0)
        self.assertFalse(decoded["interrupted"])
        self.assertTrue(decoded["completed"])
        self.assertIn("背包已满", decoded["detail"])

    def test_craft_outcome_unknown_fallback(self):
        # 空 CraftOutcome（无 oneof 分支）→ 解码器兜底 unknown，不 crash
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(23, b""))
        self.assertEqual(
            decoded["outcome"], "unknown",
            "无 completed/failed 分支的 CraftOutcome 应兜底 outcome=unknown（防解码 crash）",
        )

    def test_unknown_gameplay_enums_keep_wire_identity(self):
        gathering = _pb_varint_field(5, 99) + _pb_varint_field(6, 98)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(30, gathering))
        self.assertEqual(decoded["target_type"], "unknown_99")
        self.assertEqual(decoded["quality_hint"], "unknown_98")

        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(31, _pb_varint_field(2, 97))
        )
        self.assertEqual(decoded["kind"], "unknown_97")

        outcome = _pb_varint_field(1, 96) + _pb_varint_field(6, 95)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(14, outcome))
        self.assertEqual(decoded["bucket"], "unknown_96")
        self.assertEqual(decoded["toxin_color"], "unknown_95")

        missing_color = proto_min.decode_server_data_envelope(
            _pb_len_field(14, _pb_varint_field(1, 1))
        )
        self.assertIsNone(missing_color["toxin_color"])

    def test_alchemy_furnace_tag11_and_session_tag12(self):
        furnace = (
            _pb_varint_field(4, 1)
            + _pb_fixed64(5, 0.92)
            + _pb_fixed64(6, 1.0)
            + _pb_string(7, "Azure")
            + _pb_varint_field(8, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(11, furnace))
        self.assertEqual(
            decoded["type"], "alchemy_furnace", "envelope tag 11 应分发到 alchemy_furnace"
        )
        self.assertEqual(decoded["tier"], 1, "AlchemyFurnace.tier 是 field 4")
        self.assertAlmostEqual(decoded["integrity"], 0.92)
        self.assertAlmostEqual(decoded["integrity_max"], 1.0)
        self.assertEqual(decoded["owner_name"], "Azure")
        self.assertTrue(decoded["has_session"], "AlchemyFurnace.has_session 是 field 8")

        first_stage = (
            _pb_varint_field(1, 0)
            + _pb_varint_field(2, 0)
            + _pb_string(3, "spirit_grass×3")
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 0)
        )
        empty_stage = (
            _pb_varint_field(1, 80)
            + _pb_varint_field(2, 4)
            + _pb_string(3, "")
            + _pb_varint_field(4, 0)
            + _pb_varint_field(5, 1)
        )
        session = (
            _pb_len_field(1, b"ling_xi_wan_v1")
            + _pb_varint_field(2, 1)
            + _pb_varint_field(3, 44)
            + _pb_varint_field(4, 80)
            + _pb_fixed64(5, 0.30)
            + _pb_fixed64(6, 0.30)
            + _pb_fixed64(7, 0.15)
            + _pb_fixed64(8, 8.0)
            + _pb_fixed64(9, 5.0)
            + _pb_string(10, "炼制中")
            + _pb_message(11, first_stage)
            + _pb_message(11, empty_stage)
            + _pb_string(12, "§7InjectQi(8.0)")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(12, session))
        self.assertEqual(
            decoded["type"], "alchemy_session", "envelope tag 12 应分发到 alchemy_session"
        )
        self.assertEqual(
            decoded["recipe_id"], "ling_xi_wan_v1", "AlchemySession.recipe_id 是 field 1"
        )
        self.assertTrue(decoded["active"], "AlchemySession.active 是 field 2")
        self.assertEqual(decoded["elapsed_ticks"], 44)
        self.assertEqual(decoded["target_ticks"], 80)
        self.assertAlmostEqual(decoded["temp_current"], 0.30)
        self.assertAlmostEqual(decoded["temp_target"], 0.30)
        self.assertAlmostEqual(decoded["temp_band"], 0.15)
        self.assertAlmostEqual(decoded["qi_injected"], 8.0)
        self.assertAlmostEqual(decoded["qi_target"], 5.0)
        self.assertEqual(decoded["status_label"], "炼制中")
        self.assertEqual(len(decoded["stages"]), 2)
        self.assertEqual(decoded["stages"][0]["at_tick"], 0)
        self.assertEqual(decoded["stages"][0]["window"], 0)
        self.assertEqual(decoded["stages"][0]["summary"], "spirit_grass×3")
        self.assertTrue(decoded["stages"][0]["completed"])
        self.assertEqual(decoded["stages"][1]["at_tick"], 80)
        self.assertEqual(decoded["stages"][1]["window"], 4)
        self.assertEqual(decoded["stages"][1]["summary"], "")
        self.assertTrue(decoded["stages"][1]["missed"])
        self.assertEqual(decoded["interventions_recent"], ["§7InjectQi(8.0)"])

    def test_alchemy_outcome_resolved_tag14(self):
        outcome = _pb_varint_field(1, 1) + _pb_len_field(3, b"ling_xi_wan")
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(14, outcome))
        self.assertEqual(
            decoded["type"], "alchemy_outcome_resolved",
            "envelope tag 14 应分发到 alchemy_outcome_resolved",
        )
        self.assertEqual(
            decoded["pill"], "ling_xi_wan", "AlchemyOutcomeResolved.pill 是 field 3"
        )

    def test_skillbar_config_tag36_decodes_empty_item_skill_and_packed_cooldowns(self):
        empty = b""
        item = (
            _pb_len_field(1, b"iron_sword")
            + _pb_len_field(2, "凡铁剑".encode("utf-8"))
            + _pb_varint_field(3, 250)
            + _pb_varint_field(4, 500)
            + _pb_len_field(5, b"")
        )
        skill = (
            _pb_len_field(1, b"dugu.shoot_needle")
            + _pb_len_field(2, "凝针".encode("utf-8"))
            + _pb_varint_field(3, 50)
            + _pb_varint_field(4, 600)
            + _pb_len_field(5, b"bong-client:textures/gui/items/needle.png")
        )
        config = (
            _pb_len_field(1, empty)
            + _pb_len_field(1, _pb_len_field(1, _pb_len_field(1, item)))
            + _pb_len_field(1, _pb_len_field(1, _pb_len_field(2, skill)))
            + _pb_len_field(2, _pb_raw_varint(0) + _pb_raw_varint(42))
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(36, config))

        self.assertEqual(decoded["type"], "skillbar_config")
        self.assertIsNone(decoded["slots"][0], "OptionalSkillBarEntry 无 entry 应解为 None")
        self.assertEqual(decoded["slots"][1]["kind"], "item")
        self.assertEqual(decoded["slots"][1]["template_id"], "iron_sword")
        self.assertEqual(decoded["slots"][2]["kind"], "skill")
        self.assertEqual(decoded["slots"][2]["skill_id"], "dugu.shoot_needle")
        self.assertEqual(
            decoded["cooldown_until_ms"],
            [0, 42],
            "proto3 repeated uint64 默认 packed，最小解码器必须逐个读取",
        )

    def test_cast_sync_tag34_outcome_names(self):
        # outcome=8 → meridian_gated（经脉门拒因，场景负分支断言依赖此命名）
        msg = _pb_varint_field(1, 3) + _pb_varint_field(2, 1) + _pb_varint_field(5, 8)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(34, msg))
        self.assertEqual(decoded["type"], "cast_sync", "envelope tag 34 应分发到 cast_sync")
        self.assertEqual(decoded["phase"], "complete", "CastPhase=3 → complete")
        self.assertEqual(decoded["slot"], 1, "CastSync.slot 是 field 2")
        self.assertEqual(
            decoded["outcome"], "meridian_gated",
            "CastOutcome=8 → meridian_gated（场景负分支断言依赖此命名）",
        )

    def test_inventory_move_rejected_tag137_race_mismatch_reason_no_extra_fields(self):
        # plan-race-system-v1 P3b —— field 137 此前未接入 decode_server_data_envelope
        # 白名单，任何 bot 场景断言 inventory_move_rejected（含新增的 race_mismatch）
        # 都会静默超时；本测试锁死该 payload_type 现已可解码。
        msg = _pb_string(1, "race_mismatch")
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(137, msg))
        self.assertEqual(
            decoded["type"], "inventory_move_rejected", "envelope tag 137 应分发到 inventory_move_rejected"
        )
        self.assertEqual(decoded["reason"], "race_mismatch")
        self.assertIsNone(decoded["required_realm"], "race_mismatch 不携带 required_realm")
        self.assertIsNone(decoded["slot"], "race_mismatch 不携带 slot")
        self.assertIsNone(decoded["cap"], "race_mismatch 不携带 cap")

    def test_inventory_move_rejected_tag137_worn_cap_full_carries_slot_and_cap(self):
        msg = _pb_string(1, "worn_cap_full") + _pb_string(3, "chest") + _pb_varint(4, 3)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(137, msg))
        self.assertEqual(decoded["reason"], "worn_cap_full")
        self.assertEqual(decoded["slot"], "chest")
        self.assertEqual(decoded["cap"], 3)
        self.assertIsNone(decoded["required_realm"])

    def test_combat_event_floater_tag51_amount_float32(self):
        entry = (
            _pb_len_field(1, b"damage")
            + _pb_float32_field(2, 12.5)
            + _pb_len_field(3, b"-12")
            + _pb_varint_field(7, 1)
        )
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(51, _pb_len_field(1, entry))
        )
        self.assertEqual(
            decoded["type"], "combat_event", "envelope tag 51 应分发到 combat_event"
        )
        self.assertEqual(len(decoded["events"]), 1, "repeated entries 应逐条解出")
        self.assertEqual(decoded["events"][0]["kind"], "damage", "entry.kind 是 field 1")
        self.assertAlmostEqual(
            decoded["events"][0]["amount"], 12.5, places=4,
            msg="entry.amount 是 field 2（float32 wire type 5，非 double）",
        )
        self.assertTrue(
            decoded["events"][0]["outgoing"],
            "entry.outgoing 是 field 7（bool）——方向标识，场景据此区分己方输出/承伤",
        )

    def test_combat_event_floater_outgoing_defaults_false(self):
        # 老 server 不发 field 7 → proto3 缺省 false（承伤视角），解码不得崩
        entry = _pb_len_field(1, b"hit") + _pb_float32_field(2, 3.0)
        decoded = proto_min.decode_server_data_envelope(
            _pb_len_field(51, _pb_len_field(1, entry))
        )
        self.assertFalse(
            decoded["events"][0]["outgoing"],
            "缺 field 7 时 outgoing 应缺省 False（proto3 bool 缺省），不得 KeyError",
        )

    def test_forge_station_tag17_decodes_pos_including_negative(self):
        # plan-forge-session-entry-wiring-v1 P2：station_pos_x/y/z 是 int32，末法残土
        # 常见负坐标——必须走 64-bit 补码 varint，不能用 32-bit 掩码的 _pb_varint_field。
        station = (
            _pb_string(1, "forge_station_Azure")
            + _pb_varint_field(2, 1)
            + _pb_float32_field(3, 0.75)
            + _pb_string(4, "Azure")
            + _pb_varint_field(5, 1)
            + _pb_int32_field(6, -12)
            + _pb_varint_field(7, 64)
            + _pb_int32_field(8, -8)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(17, station))
        self.assertEqual(
            decoded["type"], "forge_station", "envelope tag 17 应分发到 forge_station"
        )
        self.assertEqual(
            decoded["station_id"], "forge_station_Azure",
            "WeaponForgeStationDataV1.station_id 是 field 1",
        )
        self.assertEqual(decoded["tier"], 1, "field 2 是 tier")
        self.assertAlmostEqual(
            decoded["integrity"], 0.75, places=4,
            msg="field 3 是 integrity（float32 wire type 5）",
        )
        self.assertTrue(decoded["has_session"], "field 5 是 has_session（bool）")
        self.assertEqual(
            decoded["pos"], [-12, 64, -8],
            "station_pos_x/y/z 是 field 6/7/8；负坐标须按 int32 补码正确还原（不是 4294967284）",
        )

    def test_forge_session_tag18_decodes_tempering_step_state(self):
        tempering = (
            _pb_varint_field(2, 3)
            + _pb_varint_field(3, 5)
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 2)
            + _pb_fixed64(6, 1.5)
        )
        step_state = _pb_message(2, tempering)  # ForgeStepState.tempering = field 2
        session = (
            _pb_varint_field(1, 7)
            + _pb_string(2, "qing_feng_v0")
            + _pb_string(3, "青锋剑（测试）")
            + _pb_varint_field(4, 1)
            + _pb_varint_field(5, 2)  # ForgeStep.TEMPERING
            + _pb_varint_field(6, 1)
            + _pb_varint_field(7, 1)
            + _pb_message(8, step_state)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(18, session))
        self.assertEqual(
            decoded["type"], "forge_session", "envelope tag 18 应分发到 forge_session"
        )
        self.assertEqual(decoded["session_id"], 7, "field 1 是 session_id（uint64）")
        self.assertEqual(
            decoded["blueprint_id"], "qing_feng_v0", "field 2 是 blueprint_id"
        )
        self.assertTrue(decoded["active"], "field 4 是 active（bool）")
        self.assertEqual(
            decoded["current_step"], "tempering",
            "ForgeStep=2 应解为 tempering（枚举与 proto/bong/envelope.proto 对齐）",
        )
        self.assertEqual(decoded["step_index"], 1, "field 6 是 step_index")
        self.assertEqual(decoded["achieved_tier"], 1, "field 7 是 achieved_tier")
        self.assertEqual(
            decoded["step_state"],
            {
                "kind": "tempering",
                "beat_cursor": 3,
                "hits": 5,
                "misses": 1,
                "deviation": 2,
                "qi_spent": 1.5,
            },
            "oneof step_state 应分发到 tempering 分支（ForgeStepState.tempering=field 2）",
        )

    def test_forge_session_step_state_none_fallback(self):
        # Done session 的 step_state 是 ForgeStepState.none_state（field 5，bool）——
        # 解码器应兜底 kind=none，不得 crash。
        session = _pb_varint_field(5, 5)  # ForgeStep.DONE
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(18, session))
        self.assertEqual(decoded["current_step"], "done", "ForgeStep=5 应解为 done")
        self.assertEqual(
            decoded["step_state"], {"kind": "none"},
            "无 billet/tempering/inscription/consecration 分支时应兜底 kind=none",
        )

    def test_forge_outcome_tag19_perfect_bucket(self):
        outcome = (
            _pb_varint_field(1, 9)
            + _pb_string(2, "qing_feng_v0")
            + _pb_varint_field(3, 1)  # ForgeOutcomeBucket.PERFECT
            + _pb_string(4, "qing_feng_sword")
            + _pb_float32_field(5, 1.0)
            + _pb_string(7, "brittle_edge")
            + _pb_varint_field(8, 2)
            + _pb_varint_field(9, 0)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(19, outcome))
        self.assertEqual(
            decoded["type"], "forge_outcome", "envelope tag 19 应分发到 forge_outcome"
        )
        self.assertEqual(decoded["bucket"], "perfect", "ForgeOutcomeBucket=1 应解为 perfect")
        self.assertEqual(
            decoded["weapon_item"], "qing_feng_sword",
            "field 4 是 optional string weapon_item",
        )
        self.assertAlmostEqual(decoded["quality"], 1.0, places=4)
        self.assertEqual(
            decoded["side_effects"], ["brittle_edge"],
            "field 7 是 repeated string side_effects",
        )
        self.assertEqual(decoded["achieved_tier"], 2, "field 8 是 achieved_tier")
        self.assertFalse(decoded["flawed_path"], "field 9=0 → flawed_path=False")

    def test_forge_outcome_flawed_path_true_and_missing_weapon_item(self):
        # Waste bucket 无 weapon_item（proto optional 缺省不发 field 4）——不得 KeyError。
        outcome = (
            _pb_varint_field(1, 10)
            + _pb_varint_field(3, 4)  # ForgeOutcomeBucket.WASTE
            + _pb_varint_field(9, 1)  # flawed_path=True
        )
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(19, outcome))
        self.assertEqual(decoded["bucket"], "waste", "ForgeOutcomeBucket=4 应解为 waste")
        self.assertIsNone(
            decoded["weapon_item"], "缺 field 4 时 weapon_item 应为 None，不得 KeyError"
        )
        self.assertTrue(decoded["flawed_path"], "field 9=1 → flawed_path=True")
        self.assertEqual(
            decoded["side_effects"], [], "无 repeated 条目时应兜底空列表，不得 crash"
        )

    def test_forge_blueprint_book_tag20(self):
        entry = (
            _pb_string(1, "qing_feng_v0")
            + _pb_string(2, "青锋剑（测试）")
            + _pb_varint_field(3, 2)
            + _pb_varint_field(4, 2)
        )
        book = _pb_message(1, entry) + _pb_varint_field(2, 0)
        decoded = proto_min.decode_server_data_envelope(_pb_len_field(20, book))
        self.assertEqual(
            decoded["type"], "forge_blueprint_book",
            "envelope tag 20 应分发到 forge_blueprint_book",
        )
        self.assertEqual(len(decoded["learned"]), 1, "repeated ForgeBlueprintEntry 应逐条解出")
        self.assertEqual(decoded["learned"][0]["id"], "qing_feng_v0")
        self.assertEqual(decoded["learned"][0]["step_count"], 2)
        self.assertEqual(decoded["current_index"], 0)


class _FakeSocket:
    """把 socketpair 一端包成可控制 recv 的代理。

    Python 3.13 下 C socket 方法（recv/sendall）不可 mock.patch.object（只读
    属性）。测试要确定性制造 EOF / OSError 时，把 `_mode` 切成 eof / raise：
    - real：委托真 socket（正常读）；
    - eof：recv 返回 b""（对端关闭）；
    - raise：recv 抛 OSError（连接重置）。
    """

    def __init__(self, real: socket.socket):
        object.__setattr__(self, "_real", real)
        self._mode = "real"

    def recv(self, *args, **kwargs):
        if self._mode == "eof":
            return b""
        if self._mode == "raise":
            raise OSError("reset")
        return object.__getattribute__(self, "_real").recv(*args, **kwargs)

    def __getattr__(self, name):
        return getattr(object.__getattribute__(self, "_real"), name)


class RedisPubSubTest(unittest.TestCase):
    """_redis_helpers：RESP2 帧编解码 + SUBSCRIBE ack 等待 + 消息泵（纯 stdlib，无需 redis 服务）。"""

    def _pubsub_with_pair(self, max_events: int = 5000):
        server, client = socket.socketpair()
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=client):
            pubsub = _redis_helpers.RedisPubSub("127.0.0.1", 6379, max_events=max_events)
        return pubsub, server, client

    def _wait_for(self, pubsub, channel, predicate, timeout=5.0):
        """轮询 events_for 直到谓词命中（全历史匹配，无等待窗口语义）。

        泵线程是独立线程，send 后事件何时入队不确定；这里确定性等它入队，
        用于验证“投递”本身，与 wait_event 的窗口语义无关。
        """
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            for evt in pubsub.events_for(channel):
                if predicate(evt):
                    return evt
            time.sleep(0.05)
        raise AssertionError(f"事件未在 {timeout:.0f}s 内入队: channel={channel}")

    def test_resp_frames_simple_string_integer_null(self):
        frames = _redis_helpers.RespFrames()
        frames.feed(b"+PONG\r\n:42\r\n$-1\r\n")
        self.assertEqual(frames.next_frame(), "PONG")
        self.assertEqual(frames.next_frame(), 42)
        self.assertIsNone(frames.next_frame())

    def test_resp_frames_array_of_bulk(self):
        frames = _redis_helpers.RespFrames()
        frames.feed(b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n")
        self.assertEqual(frames.next_frame(), [b"subscribe", b"my_chan", 1])
        self.assertIsNone(frames.next_frame())

    def test_resp_frames_subscribed_pong_shape(self):
        # RESP2 subscribed PING replies as the two-element pubsub array
        # ["pong", ""], not the ordinary +PONG simple string.
        frames = _redis_helpers.RespFrames()
        frames.feed(b"*2\r\n$4\r\npong\r\n$0\r\n\r\n")
        self.assertEqual(frames.next_frame(), [b"pong", b""])
        self.assertIsNone(frames.next_frame())

    def test_subscribed_pong_establishes_delivery_barrier(self):
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")

            def server_loop():
                try:
                    while True:
                        data = server.recv(4096)
                        if not data:
                            return
                        server.sendall(b"*2\r\n$4\r\npong\r\n$0\r\n\r\n")
                except OSError:
                    return

            thread = threading.Thread(target=server_loop, daemon=True)
            thread.start()
            pubsub.settle(0.5)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_producer_fence_waits_for_matching_server_ack(self):
        # The fence must invoke a producer-side request and accept only the
        # matching ack; this is intentionally independent of socket timing.
        pubsub = _redis_helpers.RedisPubSub.__new__(_redis_helpers.RedisPubSub)
        pubsub._host = "127.0.0.1"
        pubsub._port = 6379
        pubsub._username = None
        pubsub._password = None
        pubsub._lock = threading.Lock()
        pubsub._fatal_error = None
        pubsub._event_seq = 0
        pubsub._events = []
        pubsub._oldest_retained_seq = 0
        pubsub._max_events = 5000
        pubsub._max_event_bytes = _redis_helpers.MAX_EVENT_BYTES
        pubsub._event_bytes = 0
        called = {}

        def fake_request(token, timeout):
            called["token"] = token
            called["timeout"] = timeout
            payload = json.dumps({"v": 1, "ok": True, "token": token}).encode()
            pubsub._handle_frame(
                [
                    b"message",
                    _redis_helpers.DELIVERY_FENCE_ACK_CHANNEL.encode(),
                    payload,
                ]
            )

        pubsub._publish_fence_request = fake_request
        ack = pubsub.producer_fence(timeout=1.0)
        self.assertTrue(ack["ok"])
        self.assertEqual(ack["token"], called["token"])
        self.assertEqual(called["timeout"], 1.0)

    def test_resp_frames_partial_feeds(self):
        frames = _redis_helpers.RespFrames()
        frames.feed(b"*3\r\n$9\r\nsub")
        self.assertIsNone(frames.next_frame())
        frames.feed(b"scribe\r\n$7\r\nmy_ch")
        self.assertIsNone(frames.next_frame())
        frames.feed(b"an\r\n:1\r\n")
        self.assertEqual(frames.next_frame(), [b"subscribe", b"my_chan", 1])

    def test_resp_frames_bulk_payload_split(self):
        frames = _redis_helpers.RespFrames()
        frames.feed(b"$5\r\nhel")
        self.assertIsNone(frames.next_frame())
        frames.feed(b"lo\r\n")
        self.assertEqual(frames.next_frame(), b"hello")

    def test_oversized_bulk_frame_rejected_at_length(self):
        # 回归：review finding [major]——解码器信任对端自报的 bulk 长度，为
        # `length + 2` 字节无限累积 _buf。封顶后，仅长度头（不送本体）就必须
        # 在长度阶段抛错，不得进入等待更多分片的缓冲路径。
        frames = _redis_helpers.RespFrames()
        frames.feed(b"$%d\r\n" % (_redis_helpers.MAX_BULK_LEN + 1))
        with self.assertRaises(ValueError) as ctx:
            frames.next_frame()
        self.assertIn("MAX_BULK_LEN", str(ctx.exception))

    def test_resp_buffer_cap_rejects_runaway_feed(self):
        # 回归：review finding [major] 的第二道兜底——即使没有单个超大 bulk，
        # 海量小分片同样不能无限撑大 _buf。
        frames = _redis_helpers.RespFrames()
        with self.assertRaises(ValueError) as ctx:
            frames.feed(b"x" * (_redis_helpers.MAX_BUF_LEN + 1))
        self.assertIn("MAX_BUF_LEN", str(ctx.exception))

    def test_non_object_json_payloads_are_ignored(self):
        # 回归：review finding [minor]——订阅的全局频道上任意发布者可投递合法
        # JSON 的非对象值（null/[]/"x"/数字）。事件契约要求对象：非 dict 必须
        # 被忽略，否则 wait_event 谓词与 events_for 推导里的 e.get() 会抛
        # AttributeError 让场景崩溃。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            for raw in (b"null", b"[]", b'"x"', b"42"):
                msg = (
                    b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n"
                    % (len(raw), raw)
                )
                server.sendall(msg)
            good = json.dumps({"tick": 7}).encode()
            msg = (
                b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n"
                % (len(good), good)
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 7)
            self.assertEqual(
                len(pubsub.events_for("my_chan")),
                1,
                "非 dict JSON 应被忽略，只留合法对象事件",
            )
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_oversized_bulk_on_subscribed_channel_is_fatal(self):
        # 回归：review finding [major]——超大 bulk 到达订阅连接时，泵线程不得
        # 静默死掉（那样 wait_event 只会一直超时）；必须封顶并记录致命错误，
        # 由 events_for/wait_event 上报为明确失败。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            huge = _redis_helpers.MAX_BULK_LEN + 1
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n" % huge
            server.sendall(msg)
            deadline = time.monotonic() + 5.0
            fatal_msg = None
            while time.monotonic() < deadline:
                try:
                    pubsub.events_for("my_chan")
                except AssertionError as exc:
                    fatal_msg = str(exc)
                    break
                time.sleep(0.05)
            self.assertIsNotNone(
                fatal_msg, "超大 bulk 帧应使 events_for 上报致命错误而非静默"
            )
            self.assertIn("MAX_BULK_LEN", fatal_msg)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_resp_negative_length_below_null_rejected(self):
        # 回归：review finding [minor]——RESP2 只允许 -1 表示 null；$-2/-3 等
        # 负长度与 *-2 负计数若当空值放行，结束偏移会倒退、后续流错位。
        for bad in (b"$-2\r\n", b"*-2\r\n", b"$-3\r\n"):
            frames = _redis_helpers.RespFrames()
            frames.feed(bad)
            with self.assertRaises(ValueError):
                frames.next_frame()

    def test_resp_bulk_missing_crlf_terminator_rejected(self):
        # 回归：review finding [minor]——bulk 载荷末尾必须跟 CRLF；只按声明长度
        # 跳两字节会吞掉畸形流并从错误边界继续解析。
        frames = _redis_helpers.RespFrames()
        frames.feed(b"$3\r\nabcXX")
        with self.assertRaises(ValueError):
            frames.next_frame()

    def test_resp_array_nesting_depth_bounded(self):
        # 回归：review finding [major]——*1 深嵌套链会触发 Python 递归爆栈；
        # 嵌套深度必须封顶，超限即致命协议错误。
        frames = _redis_helpers.RespFrames()
        frames.feed(b"*1\r\n" * (_redis_helpers.MAX_RESP_DEPTH + 1))
        with self.assertRaises(ValueError) as ctx:
            frames.next_frame()
        self.assertIn("MAX_RESP_DEPTH", str(ctx.exception))

    def test_resp_array_element_count_bounded(self):
        # 回归：review finding [major]——对端可自报海量小元素，在 16MiB 字节
        # 上限内膨胀出大得多的 Python list，且不完整数组每次 recv 重解析全部
        # 前驱元素（二次方 CPU）。元素总数封顶后超限即致命协议错误。
        count = _redis_helpers.MAX_RESP_ARRAY_ITEMS + 1
        body = b"*%d\r\n" % count + b":1\r\n" * count
        frames = _redis_helpers.RespFrames()
        frames.feed(body)
        with self.assertRaises(ValueError) as ctx:
            frames.next_frame()
        self.assertIn("MAX_RESP_ARRAY_ITEMS", str(ctx.exception))

    def test_resp_array_within_bounds_still_parses(self):
        # 上限不得误伤合法帧：订阅消息是长度 3 的浅数组，边界内照常解码。
        frames = _redis_helpers.RespFrames()
        frames.feed(b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n:1\r\n")
        self.assertEqual(frames.next_frame(), [b"message", b"my_chan", 1])

    def test_pump_eof_is_fatal(self):
        # 回归：review finding [major]——订阅连接被对端关闭（EOF）时，若泵线程
        # 只是静默退出，events_for/wait_event 会继续返回冻结事件列表，负向断言
        # （无事件）在死订阅上照常通过。EOF 必须记录为致命错误。
        server, client = socket.socketpair()
        fake = _FakeSocket(client)
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=fake):
            pubsub = _redis_helpers.RedisPubSub("127.0.0.1", 6379)
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            # 泵此刻阻塞在真 recv 上。先切 eof 再发一条无害 PONG 解阻塞：泵读到
            # PONG（_handle_frame 忽略非 message 帧）后循环，下一次 recv 走 fake
            # → EOF。切换先于解阻塞字节，故泵的第二次 recv 必然命中 eof。
            fake._mode = "eof"
            server.sendall(b"+PONG\r\n")
            deadline = time.monotonic() + 5.0
            fatal_msg = None
            while time.monotonic() < deadline:
                try:
                    pubsub.events_for("my_chan")
                except AssertionError as exc:
                    fatal_msg = str(exc)
                    break
                time.sleep(0.05)
            self.assertIsNotNone(fatal_msg, "EOF 应使 events_for 上报致命错误而非静默")
            self.assertIn("EOF", fatal_msg)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_pump_socket_error_is_fatal(self):
        # 回归：review finding [major]——recv 抛 OSError（连接重置/断线）同样
        # 不能静默退出泵线程，否则负向断言在死订阅上照常通过。
        server, client = socket.socketpair()
        fake = _FakeSocket(client)
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=fake):
            pubsub = _redis_helpers.RedisPubSub("127.0.0.1", 6379)
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            # 同 EOF 测试：先切 raise 再发 PONG 解阻塞泵的第一次真 recv。
            fake._mode = "raise"
            server.sendall(b"+PONG\r\n")
            deadline = time.monotonic() + 5.0
            fatal_msg = None
            while time.monotonic() < deadline:
                try:
                    pubsub.events_for("my_chan")
                except AssertionError as exc:
                    fatal_msg = str(exc)
                    break
                time.sleep(0.05)
            self.assertIsNotNone(fatal_msg, "socket 错误应使 events_for 上报致命错误")
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_auth_peer_close_is_immediate_error(self):
        # 回归：review finding [minor]——握手阶段对端关闭（EOF）时，旧代码会空读
        # 空转烧 CPU 直到 5s 超时；现在应立即抛错而非空转。recv 注入 EOF 使路径
        # 确定性落在 AUTH→EOF（AUTH 命令本身由真 socket sendall 发出）。
        server, client = socket.socketpair()
        fake = _FakeSocket(client)
        fake._mode = "eof"
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=fake):
            with self.assertRaises(RuntimeError) as ctx:
                _redis_helpers.RedisPubSub("127.0.0.1", 6379, password="hunter2")
        self.assertIn("EOF", str(ctx.exception))
        server.close()
        client.close()

    def test_subscribe_ack_wait_and_message_pump(self):
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            payload = json.dumps({"full_charge": False, "tick": 7}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 7)
            self.assertEqual(len(pubsub.events_for("my_chan")), 1)
            self.assertEqual(pubsub.events_for("other"), [])
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_wait_event_timeout_raises(self):
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            with self.assertRaises(AssertionError):
                pubsub.wait_event("my_chan", lambda e: False, timeout=0.5)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_pump_survives_idle_timeout_and_still_delivers_later_message(self):
        # 回归：review finding [2]——ack 循环临时装的超时若残留在长连接上，
        # _pump 会把 socket.timeout 当 OSError 永久退出，之后发布的订阅消息
        # 永远收不到。修复后：(a) subscribe() 把连接恢复为阻塞（无超时残留）；
        # (b) 即便仍有有限超时，_pump 也把 timeout 当非终止信号继续等。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            self.assertIsNone(
                client.gettimeout(),
                "subscribe 确认后长连接不应残留 ack 用有限超时（空闲 5s 会杀线程）",
            )

            # 人为模拟超时仍残留：旧代码 _pump 在首个空闲超时即退出。
            client.settimeout(0.1)
            time.sleep(0.35)

            payload = json.dumps({"tick": 42}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 42)
            self.assertEqual(pubsub.events_for("my_chan")[0]["tick"], 42)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_message_coalesced_with_final_subscribe_ack_is_delivered(self):
        # 回归：review finding [major]——ack 循环的一次 recv(4096) 可能同时读到
        # 「最终订阅确认 + 首条已发布消息」，但它只解析 ack 就退出（remaining 空）。
        # 若 _pump 先阻塞 recv 再解析缓冲，这条已入缓冲的消息要等下一次网络流量
        # 才会被处理；这是唯一一次发布时 wait_event 会一直超时。修复：_pump 先
        # 消费缓冲帧再收新数据。这里 ack 与消息同一次 sendall，且之后不再有
        # 任何网络流量——消息必须被泵线程立即解析入队。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            payload = json.dumps({"tick": 99}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(ack + msg)
            pubsub.subscribe("my_chan")
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 99, timeout=2.0)
            self.assertEqual(len(pubsub.events_for("my_chan")), 1)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_message_published_between_subscribe_acks_is_not_discarded(self):
        # 回归：review finding [minor]——依次 SUBSCRIBE 多频道时，发布者可在首个
        # ack 之后、末个 ack 之前发布消息。ack 循环若只认 subscribe 帧、把穿插的
        # message 帧当噪声丢弃，wait_event 会在这条真实消息上超时。订阅确认与
        # 穿插消息同批到达，subscribe 返回后消息必须已在事件流中。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack_a = b"*3\r\n$9\r\nsubscribe\r\n$4\r\nch_a\r\n:1\r\n"
            ack_b = b"*3\r\n$9\r\nsubscribe\r\n$4\r\nch_b\r\n:2\r\n"
            payload = json.dumps({"tick": 11}).encode()
            msg_a = b"*3\r\n$7\r\nmessage\r\n$4\r\nch_a\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            # 先 ack ch_a，穿插 ch_a 上的消息，再 ack ch_b——一次 recv 全到。
            server.sendall(ack_a + msg_a + ack_b)
            pubsub.subscribe("ch_a", "ch_b")
            self._wait_for(pubsub, "ch_a", lambda e: e.get("tick") == 11, timeout=2.0)
            self.assertEqual(len(pubsub.events_for("ch_a")), 1)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_parse_redis_url_strips_components_and_keeps_credentials(self):
        # 回归：review finding [2]/[3]——(a) REDIS_URL 带 db 路径/query 时转端口
        # 会 int('6379/0') ValueError；query-only（无路径）URL 的 '?x=1' 会残留
        # 在 authority 里同样炸端口解析；fragment 同理。SUBSCRIBE 不需要选库，
        # 应在 authority 边界（/ ? #）处剥离。(b) userinfo（user:pass@）不再丢弃，
        # 返回给 RedisPubSub 连接后 AUTH。
        cases = {
            "redis://127.0.0.1:6379/0": ("127.0.0.1", 6379, None, None),
            "redis://127.0.0.1:6379/0?x=1&y=2": ("127.0.0.1", 6379, None, None),
            "redis://127.0.0.1": ("127.0.0.1", 6379, None, None),
            "redis://127.0.0.1:6379?x=1": ("127.0.0.1", 6379, None, None),
            "redis://127.0.0.1:6379#frag": ("127.0.0.1", 6379, None, None),
            "redis://user:pass@127.0.0.1:6380/3": ("127.0.0.1", 6380, "user", "pass"),
            "redis://user@127.0.0.1:6380": ("127.0.0.1", 6380, "user", None),
            "redis://alice:s3cret@127.0.0.1:6379?x=1": (
                "127.0.0.1",
                6379,
                "alice",
                "s3cret",
            ),
            "": ("127.0.0.1", 6379, None, None),
        }
        for url, expected in cases.items():
            self.assertEqual(_redis_helpers._parse_redis_url(url), expected)

    def test_unsupported_redis_scheme_error_redacts_credentials(self):
        # 回归：review finding [major]——不支持的 scheme 报错若回填完整 URL，
        # `rediss://alice:s3cret@...` 会把密码打印进 CI/运维日志。修复后只报
        # scheme，不得泄漏 userinfo（用户名 / 密码）。
        with self.assertRaises(ValueError) as ctx:
            _redis_helpers._parse_redis_url("rediss://alice:s3cret@redis.example:6380")
        message = str(ctx.exception)
        self.assertIn(
            "rediss", message,
            "异常应点明不支持的 scheme，便于运维定位；实际={message!r}",
        )
        self.assertNotIn(
            "s3cret", message,
            "异常不得包含密码；实际={message!r}",
        )
        self.assertNotIn(
            "alice", message,
            "异常不得包含用户名；实际={message!r}",
        )
        self.assertNotIn(
            "redis.example", message,
            "异常不得包含完整 host（无关凭据也一并避免）；实际={message!r}",
        )

    def test_auth_issued_on_connect_with_credentials(self):
        # 回归：review finding [3]——带凭据的 REDIS_URL 之前静默丢弃 userinfo，
        # 连上后直接 SUBSCRIBE，认证服务器回 -NOAUTH 使场景全废；修复后连接即
        # AUTH（有用户名用 `AUTH <user> <pass>`，Redis 6+ ACL）。+OK 才继续。
        # 断言对齐 _frame_command 的 RESP2 数组框架（review finding [major]：
        # 旧 inline 文本 `AUTH alice s3cret` 会被空格拆参/注入，框架已改）。
        server, client = socket.socketpair()
        server.sendall(b"+OK\r\n")
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=client):
            pubsub = _redis_helpers.RedisPubSub(
                "127.0.0.1", 6379, username="alice", password="s3cret"
            )
        try:
            expected = _redis_helpers._frame_command(["AUTH", "alice", "s3cret"])
            cmd = b""
            while len(cmd) < len(expected):
                cmd += server.recv(1024)
            self.assertEqual(cmd, expected, "连接后应先发 RESP2 数组框架的 AUTH 而非 SUBSCRIBE")
            # AUTH 确认后仍能正常订阅并收到消息。
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            payload = json.dumps({"tick": 9}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 9)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_auth_password_only_uses_default_user(self):
        # 无用户名时退化为 `AUTH <pass>`（默认用户），同样走 RESP2 数组框架。
        server, client = socket.socketpair()
        server.sendall(b"+OK\r\n")
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=client):
            pubsub = _redis_helpers.RedisPubSub("127.0.0.1", 6379, password="hunter2")
        try:
            expected = _redis_helpers._frame_command(["AUTH", "hunter2"])
            cmd = b""
            while len(cmd) < len(expected):
                cmd += server.recv(1024)
            self.assertEqual(cmd, expected)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_auth_failure_raises_on_connect(self):
        # 回归：review finding [3]——错误凭据必须连接阶段立即暴露（RuntimeError），
        # 不能让 -NOAUTH/-WRONGPASS 错误帧被静默吞掉后场景跑起来才莫名失败。
        server, client = socket.socketpair()
        server.sendall(b"-NOAUTH Authentication required\r\n")
        with mock.patch("bot._redis_helpers.socket.create_connection", return_value=client):
            with self.assertRaises(RuntimeError):
                _redis_helpers.RedisPubSub("127.0.0.1", 6379, password="wrong")
        server.close()
        client.close()

    def test_events_buffer_is_bounded(self):
        # 回归：review finding [2]——_events 无上限会随全局频道累积撑爆内存；
        # max_events 裁剪应丢弃最旧条目。
        pubsub, server, client = self._pubsub_with_pair(max_events=2)
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            for i in range(3):
                payload = json.dumps({"seq": i}).encode()
                msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                    len(payload),
                    payload,
                )
                server.sendall(msg)
            # 等三条全部入队（seq 2 可见即裁剪已发生——裁剪与追加同锁），再断言。
            self._wait_for(pubsub, "my_chan", lambda e: e.get("seq") == 2)
            got = [e["seq"] for e in pubsub.events_for("my_chan")]
            self.assertEqual(len(got), 2)
            self.assertNotIn(0, got, "max_events 裁剪应丢弃最旧条目")
            with self.assertRaises(AssertionError) as ctx:
                pubsub.window_events("my_chan", after=0)
            self.assertIn("淘汰", str(ctx.exception))
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_events_byte_budget_trims_oldest_by_bytes(self):
        # 回归：review finding [major]——max_events 只按条数裁剪，单事件可接近
        # MAX_BULK_LEN（1 MiB），5000 条 × 近 1 MiB = 数 GiB 保留会耗尽 bot
        # runner 内存。累积字节预算把保留内存压到可控量级；超预算裁剪最旧条目。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$2\r\nch\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("ch")
            # 手工压低字节预算：3 条各 ~51 字节 wire 的载荷总和超预算即裁剪最旧。
            pubsub._max_event_bytes = 60
            for i in range(3):
                payload = json.dumps({"seq": i, "pad": "x" * 30}).encode()
                msg = b"*3\r\n$7\r\nmessage\r\n$2\r\nch\r\n$%d\r\n%s\r\n" % (
                    len(payload),
                    payload,
                )
                server.sendall(msg)
            self._wait_for(pubsub, "ch", lambda e: e.get("seq") == 2, timeout=2.0)
            got = [e["seq"] for e in pubsub.events_for("ch")]
            self.assertNotIn(0, got, "字节预算应裁剪最旧条目")
            self.assertLessEqual(len(got), 2)
            self.assertLessEqual(pubsub._event_bytes, 60)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_wait_event_returns_event_received_during_wait(self):
        # wait_event 的等待窗口从调用时起算：事件须在窗口内到达才被命中。
        # 用生产者线程在 wait 开始后再投递，避免“先 send 后 wait”时泵线程抢先
        # 入队、事件落出窗口（那正是 review finding [2] 要消除的重扫行为）。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")

            def _produce():
                time.sleep(0.2)
                payload = json.dumps({"tick": 7}).encode()
                msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                    len(payload),
                    payload,
                )
                server.sendall(msg)

            producer = threading.Thread(target=_produce)
            producer.start()
            evt = pubsub.wait_event(
                "my_chan", lambda e: e.get("tick") == 7, timeout=5.0
            )
            producer.join()
            self.assertEqual(evt["tick"], 7)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_wait_event_scans_only_events_since_wait_start(self):
        # 回归：review finding [2]——wait_event 若重扫全频道历史，等待开始前的
        # 旧事件也会被谓词命中；应只匹配本次等待期间新增的事件。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            payload = json.dumps({"seq": 0}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("seq") == 0)
            # 旧事件已入队但不在本次等待窗口内，不应再被命中。
            with self.assertRaises(AssertionError):
                pubsub.wait_event("my_chan", lambda e: e.get("seq") == 0, timeout=0.5)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_wait_event_honors_anchor_taken_before_action(self):
        # 回归：review finding [1]/[4]——"trigger 先于 wait_event"的调用顺序下，
        # 服务端响应可能在 wait_event 记录窗口前被泵线程入队，被 seq >= start_seq
        # 排除而超时。修复后调用方可在发触发 intent **之前** anchor() 取锚点并传
        # wait_event(after=anchor)：锚点与等待之间入队的事件同样被窗口包含。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")

            # 1) 发送触发「动作」前先锚定（场景里对应 _switch(intent) 之前）。
            anchor = pubsub.anchor()

            # 2) 动作触发后，事件在 wait_event 调用**之前**就被泵线程入队——
            #    正是 review finding [1] 指出的竞态窗口。
            payload = json.dumps({"tick": 7}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("tick") == 7)

            # 3) 窗口从调用时起算会漏掉它；after=anchor 让窗口从动作前的锚点起算。
            evt = pubsub.wait_event(
                "my_chan", lambda e: e.get("tick") == 7, timeout=5.0, after=anchor
            )
            self.assertEqual(evt["tick"], 7)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_wait_event_after_still_excludes_events_before_anchor(self):
        # after= 语义与默认窗口一致：锚点**之前**入队的事件仍被排除（只是把窗口
        # 起点从调用时前移到锚定时，不改变"只扫新增"的契约）。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            payload = json.dumps({"seq": 0}).encode()
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(payload),
                payload,
            )
            server.sendall(msg)
            self._wait_for(pubsub, "my_chan", lambda e: e.get("seq") == 0)
            # 锚定发生在 seq0 事件入队**之后**：该事件仍在窗口外。
            anchor = pubsub.anchor()
            with self.assertRaises(AssertionError):
                pubsub.wait_event(
                    "my_chan",
                    lambda e: e.get("seq") == 0,
                    timeout=0.5,
                    after=anchor,
                )
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_feed_appends_in_place_no_quadratic_rebuild(self):
        # 回归：review finding [minor]——_buf 若是不可变 bytes，feed 每次 `+=` 都
        # 复制整段累计缓冲，分片到达时呈二次方拷贝。改为 bytearray 后原地追加。
        frames = _redis_helpers.RespFrames()
        buf_ref = frames._buf
        for _ in range(50):
            frames.feed(b"x" * 4096)
        self.assertIs(
            frames._buf,
            buf_ref,
            "feed 必须原地追加，不得重建缓冲对象（bytearray 而非 bytes）",
        )
        self.assertEqual(len(frames._buf), 50 * 4096)

    def test_wait_event_excludes_events_enqueued_after_deadline(self):
        # 回归：review finding [minor]——最终扫描只用 seq 边界，无法区分「截止前
        # 到达」与「截止后入队」；事件入队时间戳 <= deadline 才被接受。
        pubsub = _redis_helpers.RedisPubSub.__new__(_redis_helpers.RedisPubSub)
        pubsub._lock = threading.Lock()
        pubsub._fatal_error = None
        pubsub._event_seq = 0
        pubsub._max_events = 5000
        pubsub._max_event_bytes = _redis_helpers.MAX_EVENT_BYTES
        pubsub._event_bytes = 0
        # 匹配事件在 deadline（≈now+0.01s）之后才入队：ts 在未来，必须被排除。
        pubsub._events = [
            (0, "my_chan", {"ok": True}, time.monotonic() + 1000.0, 4)
        ]
        with self.assertRaises(AssertionError):
            pubsub.wait_event("my_chan", lambda e: e.get("ok") is True, timeout=0.01)

    def test_wait_event_accepts_events_within_deadline(self):
        # 同一 seq 窗口内的匹配事件，入队时刻 <= deadline 时仍应被接受（过滤
        # 只排除截止后入队，不误伤截止前到达）。
        pubsub = _redis_helpers.RedisPubSub.__new__(_redis_helpers.RedisPubSub)
        pubsub._lock = threading.Lock()
        pubsub._fatal_error = None
        pubsub._event_seq = 0
        pubsub._max_events = 5000
        pubsub._max_event_bytes = _redis_helpers.MAX_EVENT_BYTES
        pubsub._event_bytes = 0
        pubsub._events = [
            (0, "my_chan", {"ok": True}, time.monotonic() - 1.0, 4)
        ]
        evt = pubsub.wait_event("my_chan", lambda e: e.get("ok") is True, timeout=5.0)
        self.assertEqual(evt["ok"], True)

    def test_window_events_bounds_by_anchor_and_enqueue_ts(self):
        # 负向断言的最终窗口化扫描基础：after 锚点之前的事件与 max_ts 之后入队的
        # 事件都被排除——最终扫描不得把窗口外事件误判为窗口内（review finding
        # [major]：最终 despawn 检查缺窗口化扫描）。
        pubsub = _redis_helpers.RedisPubSub.__new__(_redis_helpers.RedisPubSub)
        pubsub._lock = threading.Lock()
        pubsub._fatal_error = None
        pubsub._event_seq = 3
        pubsub._max_events = 5000
        pubsub._max_event_bytes = _redis_helpers.MAX_EVENT_BYTES
        pubsub._event_bytes = 0
        now = time.monotonic()
        pubsub._events = [
            (0, "ch", {"seq": "pre"}, now, 4),
            (1, "ch", {"seq": "in"}, now, 4),
            (2, "ch", {"seq": "late"}, now + 1000.0, 4),
        ]
        got = [
            e["seq"]
            for e in pubsub.window_events("ch", after=1, max_ts=now + 500.0)
        ]
        self.assertEqual(got, ["in"])

    def test_settle_drains_buffered_frames_as_delivery_barrier(self):
        # 回归：review finding [major]——负向断言的最终扫描若缺投递屏障，截止判定
        # 与 pump 入队之间的帧会被漏掉。settle 现在以 PING/PONG 正同步证明投递
        # 完成：PING 写入订阅连接后，pump 必须消费到应答 PONG（Redis 按入站命令
        # 顺序处理，PONG 之前的 message 帧字节必已先于 PONG 到达），屏障返回即
        # 已缓冲帧全部消费入队。此测试验证完整通路：真实 pump 线程 + 对端回 PONG。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")

            def server_loop():
                # 对客户端每次读取回一条 PONG（真 redis-server 按入站命令顺序
                # 应答，这里只模拟"读到命令必有应答"的应答面）。
                try:
                    while True:
                        data = server.recv(4096)
                        if not data:
                            return
                        server.sendall(b"+PONG\r\n")
                except OSError:
                    return

            thread = threading.Thread(target=server_loop, daemon=True)
            thread.start()

            # 模拟"截止前发布、字节已入订阅连接但 pump 尚未消费"的帧：直接推入
            # _frames（等价于 pump 已 recv 未 drain 的状态），settle 的 PONG 屏障
            # 返回后必须已被消费入队。
            frames = _redis_helpers.RespFrames()
            payload = json.dumps({"owner": "player:x", "tick": 1}).encode()
            frames.feed(
                b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n"
                % (len(payload), payload)
            )
            with pubsub._frame_lock:
                pubsub._frames = frames

            pubsub.settle(0.5)
            self.assertEqual(len(pubsub.events_for("my_chan")), 1)
            self.assertEqual(pubsub.events_for("my_chan")[0]["tick"], 1)
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_settle_fails_closed_when_pong_not_consumed(self):
        # 回归：review finding [major]——settle 的投递屏障未成立（PONG 未被 pump
        # 消费）时必须抛错而非静默返回：负向断言不得建立在未证明的投递完成上。
        # 对端不读不回，PING 的 PONG 应答永不出现，屏障必然超时。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            with self.assertRaises(AssertionError) as cm:
                pubsub.settle(0.05)
            self.assertIn("投递屏障未成立", str(cm.exception))
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_settle_fails_closed_on_fatal_connection(self):
        # 回归：review finding [major]——连接已 fatal（泵线程记录协议异常）时
        # settle 不得静默返回：负向断言不可信，必须抛错。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            pubsub._fatal_error = RuntimeError("protocol exploded")
            with self.assertRaises(AssertionError) as cm:
                pubsub.settle(0.05)
            self.assertIn("protocol exploded", str(cm.exception))
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_pump_deep_json_recursion_error_is_fatal(self):
        # 回归：review finding [major]——json.loads 对超深嵌套列表抛 RecursionError，
        # 不在 _handle_frame 的 ValueError/TypeError 捕获之列，此前会让泵线程静默
        # 退出、负向断言在死订阅上照常通过。泵线程兜底失败边界必须把它记为 fatal。
        pubsub, server, client = self._pubsub_with_pair()
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            deep = b"[" * 100000 + b"]" * 100000
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$%d\r\n%s\r\n" % (
                len(deep),
                deep,
            )
            server.sendall(msg)
            deadline = time.monotonic() + 5.0
            while time.monotonic() < deadline:
                try:
                    pubsub.events_for("my_chan")
                except AssertionError as exc:
                    # str(RecursionError) 是消息文本而非类型名；RecursionError 不是
                    # ValueError/TypeError 子类，必须落到泵线程的兜底 except Exception。
                    self.assertIn("maximum recursion depth", str(exc))
                    return
                time.sleep(0.05)
            self.fail("超深 JSON 载荷未让泵线程把 RecursionError 记为 fatal")
        finally:
            pubsub.stop()
            server.close()
            client.close()

    def test_pump_catch_all_marks_fatal_on_unexpected_frame_exception(self):
        # 兜底失败边界（review finding [major]）：泵线程捕获不到协议/socket 例外
        # 以外的任何异常时，必须 _set_fatal 而不是静默退出守护线程。用子类让
        # _handle_frame 抛非 ValueError/TypeError/OSError 的意外异常验证该边界。
        class Boom(_redis_helpers.RedisPubSub):
            def _handle_frame(self, frame):
                raise RuntimeError("boom")

        server, client = socket.socketpair()
        with mock.patch(
            "bot._redis_helpers.socket.create_connection", return_value=client
        ):
            pubsub = Boom("127.0.0.1", 6379)
        try:
            ack = b"*3\r\n$9\r\nsubscribe\r\n$7\r\nmy_chan\r\n:1\r\n"
            server.sendall(ack)
            pubsub.subscribe("my_chan")
            msg = b"*3\r\n$7\r\nmessage\r\n$7\r\nmy_chan\r\n$2\r\n{}\r\n"
            server.sendall(msg)
            deadline = time.monotonic() + 5.0
            while time.monotonic() < deadline:
                try:
                    pubsub.events_for("my_chan")
                except AssertionError as exc:
                    self.assertIn("boom", str(exc))
                    return
                time.sleep(0.05)
            self.fail("泵线程未把意外帧处理异常记为 fatal")
        finally:
            pubsub.stop()
            server.close()
            client.close()


class ThrowCarrierGuardMarkerTest(unittest.TestCase):
    """combat_anqi_throw_carrier：消费者 guard 日志按 carrier 线缆 id 归属解析。

    回归 review findings [major]：正向派发证据不得依赖不唯一的 payload 字节数，
    必须由 throw_carrier_intents 在空手早退处发出的 guard 日志按本 bot 线缆 id
    归属。此测试钉住解析逻辑（按 carrier 过滤 + reason 提取）。
    """

    def _scenario_module(self):
        from bot.scenarios import combat_anqi_throw_carrier as mod

        return mod

    def _write_log(self, lines: list[str]) -> str:
        fd, path = tempfile.mkstemp(suffix=".log")
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
        self.addCleanup(os.remove, path)
        return path

    def _scan(self, path: str, mod) -> "_server_log.ServerLogScanner":
        scanner = _server_log.ServerLogScanner(path, mod._GUARD_RE)
        scanner.scan()
        return scanner

    def test_guard_markers_filtered_to_own_carrier(self):
        mod = self._scenario_module()
        own = "player:3f9a2c8e-4a1e-4b1f-9c2d-0a1b2c3d4e5f"
        other = "player:11111111-2222-3333-4444-555555555555"
        path = self._write_log(
            [
                f"[bong][combat] throw_carrier guard carrier={own} slot=MainHand reason=no_anqi_imprint",
                f"[bong][combat] throw_carrier guard carrier={other} slot=MainHand reason=no_carrier_item",
                "[bong][network] client_request received entity=5v1 payload_bytes=77",
            ]
        )
        scanner = self._scan(path, mod)
        self.assertEqual(
            scanner.guard_markers(own),
            ["no_anqi_imprint"],
            "只应解析出归属本 bot 的 guard 标记",
        )
        self.assertEqual(
            scanner.guard_markers(other),
            ["no_carrier_item"],
        )
        self.assertEqual(scanner.guard_markers("player:missing"), [])

    def test_guard_reasons_whitelist_accepts_both_guards(self):
        mod = self._scenario_module()
        own = "player:3f9a2c8e-4a1e-4b1f-9c2d-0a1b2c3d4e5f"
        path = self._write_log(
            [
                f"[bong][combat] throw_carrier guard carrier={own} slot=MainHand reason=no_carrier_item",
                f"[bong][combat] throw_carrier guard carrier={own} slot=MainHand reason=no_anqi_imprint",
            ]
        )
        reasons = self._scan(path, mod).guard_markers(own)
        for reason in reasons:
            self.assertIn(
                reason,
                mod._GUARD_REASONS,
                f"reason {reason!r} 不在空手护栏白名单 {mod._GUARD_REASONS!r} 内",
            )


class ServerLogScannerTest(unittest.TestCase):
    """_server_log.ServerLogScanner：增量扫描 + user= 精确归属。"""

    def _write_log(self, lines: list[str]) -> str:
        fd, path = tempfile.mkstemp(suffix=".log")
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines) + "\n")
        self.addCleanup(os.remove, path)
        return path

    def test_incremental_scan_does_not_reprocess_old_lines(self):
        # 回归：review finding [minor]——guard 轮询若每次从头全扫整个无界 server
        # 日志，窗口内几十次全量读产生几十 GB I/O。按偏移增量扫描不得重复解析
        # 旧行，且追加段须在下一次 scan() 里被读到。
        path = self._write_log(
            [
                "[bong][combat] throw_carrier guard carrier=player:aaa "
                "slot=MainHand reason=no_carrier_item",
            ]
        )
        scanner = _server_log.ServerLogScanner(
            path,
            re.compile(
                r"throw_carrier guard carrier=(?P<carrier>\S+) "
                r".*reason=(?P<reason>\S+)"
            ),
        )
        scanner.scan()
        self.assertEqual(scanner.guard_markers("player:aaa"), ["no_carrier_item"])
        scanner.scan()  # 无新增：不得重复累计旧行
        self.assertEqual(
            scanner.guard_markers("player:aaa"),
            ["no_carrier_item"],
            "重复 scan 不得重新解析旧行",
        )
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(
                "[bong][combat] throw_carrier guard carrier=player:bbb "
                "slot=MainHand reason=no_anqi_imprint\n"
            )
        scanner.scan()
        self.assertEqual(scanner.guard_markers("player:aaa"), ["no_carrier_item"])
        self.assertEqual(scanner.guard_markers("player:bbb"), ["no_anqi_imprint"])

    def test_deserialize_failed_attribution_is_exact_not_substring(self):
        # 回归：review finding [minor]——`username in line` 子串匹配会让 Throw
        # 被 Throw2 的失败误归因。按结构化 user=<name> 字段精确匹配。
        path = self._write_log(
            [
                "[bong][network] client_request deserialize failed from 2v1 "
                "(user=Throw2): bad; payload_bytes=10",
            ]
        )
        scanner = _server_log.ServerLogScanner(
            path,
            re.compile(r"throw_carrier guard carrier=(\S+)"),
        )
        scanner.scan()
        self.assertEqual(
            scanner.deserialize_failed("Throw"),
            0,
            "Throw 不得被 Throw2 的 user= 字段子串命中",
        )
        self.assertEqual(scanner.deserialize_failed("Throw2"), 1)

    def test_same_inode_equal_or_larger_rewrite_resets_to_new_head(self):
        path = self._write_log(["old line A", "old line B"])
        scanner = _server_log.ServerLogScanner(
            path,
            re.compile(
                r"throw_carrier guard carrier=(?P<carrier>\S+) "
                r".*reason=(?P<reason>\S+)"
            ),
        )
        scanner.scan()
        with open(path, "r+b") as fh:
            fh.seek(0)
            replacement = (
                "throw_carrier guard carrier=player:same slot=MainHand "
                "reason=no_carrier_item\n" + "x" * 80 + "\n"
            ).encode()
            fh.write(replacement)
            fh.truncate()
        scanner.scan()
        self.assertEqual(
            scanner.guard_markers("player:same"),
            ["no_carrier_item"],
            "同 inode 且新内容不短于旧偏移时也必须识别 copytruncate/rewrite 并从头扫描",
        )

    def test_rotation_to_larger_replacement_resets_to_new_head(self):
        # 回归：review finding [minor]——日志被替换成更大的新文件（轮转）时，旧
        # 实现只按 offset <= size 判定文件身份，会 seek 到旧偏移 N 并跳过新文件
        # N 之前的全部行。把 guard 关键字横跨旧偏移 N 摆放：seek(N) 后 re.search
        # 只看到被截断的后缀，guard 证据永久丢失。修复后按 dev/inode 判定文件
        # 身份，替换后从头重读。
        fd, path = tempfile.mkstemp(suffix=".log")
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write("old line A\nold line B\n")  # 22 字节 → 旧偏移 N=22
        self.addCleanup(os.remove, path)
        scanner = _server_log.ServerLogScanner(
            path,
            re.compile(
                r"throw_carrier guard carrier=(?P<carrier>\S+) "
                r".*reason=(?P<reason>\S+)"
            ),
        )
        scanner.scan()
        # 轮转：旧文件改名移走，同路径新建（新 inode）。新文件更大，guard 关键字
        # 横跨字节 22（"HEADERZZZZ\n" 占 11 字节，throw_carrier 起于 11）——
        # seek(22) 只能读到 "er guard carrier=..." 后缀，re.search 找不到完整关键字。
        os.rename(path, path + ".rotated")
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(
                "HEADERZZZZ\n"
                "throw_carrier guard carrier=player:new slot=MainHand "
                "reason=no_carrier_item\n"
                "line C\nline D\nline E\n"
            )
        self.addCleanup(os.remove, path + ".rotated")
        scanner.scan()
        self.assertEqual(
            scanner.guard_markers("player:new"),
            ["no_carrier_item"],
            "轮转成更大的替换文件后必须从头重读，guard 不得被旧偏移跳过",
        )

    def test_partial_multibyte_line_rewinds_to_line_start(self):
        # 回归：review finding [minor]——文本模式的 fh.tell() 记录字节流 cookie，
        # 而旧 rewind 按 len(data)（字符数）回退：未写完的多字节 UTF-8 行会把偏移
        # 算到行中间，续写后 seek 从半行开始，guard 标记被跳过或只解出后缀。修复
        # 后按字节扫描，未完成行回退到字节级行起点（最后一个 \n 之后）。
        fd, path = tempfile.mkstemp(suffix=".log")
        self.addCleanup(os.remove, path)
        scanner = _server_log.ServerLogScanner(
            path,
            re.compile(
                r"throw_carrier guard carrier=(?P<carrier>\S+) "
                r".*reason=(?P<reason>\S+)"
            ),
        )
        head = "line one\n"
        # 未完成行：2 个多字节字符（玩家 = 6 字节）且无结尾换行。
        partial = "throw_carrier guard carrier=玩家"
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(head + partial)
        scanner.scan()
        # 行起点 = head 的字节长（9）；修复后 _offset 必须精确落在行起点。
        self.assertEqual(
            scanner._offset,
            len(head.encode("utf-8")),
            "未完成多字节行回退后偏移必须落在字节级行起点",
        )
        self.assertEqual(scanner.guard_markers("玩家"), [])
        # 续写完整行 + 换行：从行起点重读，guard 完整解析。
        with open(path, "a", encoding="utf-8") as fh:
            fh.write(" reason=no_carrier_item\n")
        scanner.scan()
        self.assertEqual(
            scanner.guard_markers("玩家"),
            ["no_carrier_item"],
            "未完成多字节行续写后必须从行起点重读，guard 不得被跳过",
        )

class NewServerDataDecoderContractTest(unittest.TestCase):
    """S1 拆分新增的深度解码器契约 pin（central-review finding 1 的补测）。

    S1 在 decode_server_data_envelope 注册了 botany_harvest_progress(25) /
    gathering_session(30) / lingtian_session(31) / skill_bar_config(36) /
    breakthrough_cinematic(71)，并改写了 player_state(5)（新增 realm）与
    alchemy_outcome_resolved(14)（bucket 转枚举名、可空字段、toxin_color）。
    这些解码器此前只在 test_protocol.py 初始化了 fixture，没有任何 field→值
    的 wire 契约断言——字段号/枚举映射/packed 解码错了场景照样静默观察错误数据。
    本类用与 authoritative proto 对齐的字段号逐项钉死解码输出。
    """

    def test_botany_harvest_progress_tag25_decodes_full_contract(self):
        msg = (
            _pb_string(1, "botany:session_7")
            + _pb_string(2, "spirit_grass_001")
            + _pb_string(3, "灵草")
            + _pb_string(4, "spirit_grass")
            + _pb_string(5, "manual")
            + _pb_fixed64(6, 0.42)
            + _pb_varint(7, 1)
            + _pb_varint(8, 1)
            + _pb_varint(9, 0)
            + _pb_varint(10, 0)
            + _pb_string(11, "正在采集")
            + _pb_string(12, "hint_a")
            + _pb_string(12, "hint_b")
            + _pb_fixed64(13, 10.0)
            + _pb_fixed64(14, 64.0)
            + _pb_fixed64(15, -3.0)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(25, msg))
        self.assertEqual(decoded["type"], "botany_harvest_progress")
        self.assertEqual(decoded["session_id"], "botany:session_7")
        self.assertEqual(decoded["target_id"], "spirit_grass_001")
        self.assertEqual(decoded["target_name"], "灵草")
        self.assertEqual(decoded["plant_kind"], "spirit_grass")
        self.assertEqual(decoded["mode"], "manual")
        self.assertEqual(decoded["progress"], 0.42)
        self.assertTrue(decoded["auto_selectable"])
        self.assertTrue(decoded["request_pending"])
        self.assertFalse(decoded["interrupted"])
        self.assertFalse(decoded["completed"])
        self.assertEqual(decoded["detail"], "正在采集")
        self.assertEqual(decoded["hazard_hints"], ["hint_a", "hint_b"])
        self.assertEqual(
            decoded["target_pos"],
            [10.0, 64.0, -3.0],
            "target_pos_x/y/z 是 field 13/14/15，三个 optional double 拆成平铺坐标",
        )

    def test_botany_harvest_progress_absent_target_pos_is_none_triple(self):
        msg = _pb_string(1, "botany:session_8") + _pb_fixed64(6, 0.5)
        decoded = proto_min.decode_server_data_envelope(_pb_message(25, msg))
        self.assertEqual(
            decoded["target_pos"],
            [None, None, None],
            "缺 field 13/14/15 时 target_pos 必须是 [None, None, None]，不得用 0 冒充",
        )
        self.assertEqual(decoded["hazard_hints"], [])
        self.assertEqual(decoded["progress"], 0.5)

    def test_gathering_session_tag30_decodes_enum_and_optional(self):
        msg = (
            _pb_string(1, "gather:42")
            + _pb_varint(2, 5)
            + _pb_varint(3, 20)
            + _pb_string(4, "精铁矿")
            + _pb_varint(5, 2)  # GATHERING_TARGET_TYPE_ORE
            + _pb_varint(6, 5)  # GATHERING_QUALITY_HINT_PERFECT
            + _pb_string(7, "iron_pickaxe")
            + _pb_varint(8, 0)
            + _pb_varint(9, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(30, msg))
        self.assertEqual(decoded["type"], "gathering_session")
        self.assertEqual(decoded["session_id"], "gather:42")
        self.assertEqual(decoded["progress_ticks"], 5)
        self.assertEqual(decoded["total_ticks"], 20)
        self.assertEqual(decoded["target_name"], "精铁矿")
        self.assertEqual(decoded["target_type"], "ore")
        self.assertEqual(decoded["quality_hint"], "perfect")
        self.assertEqual(decoded["tool_used"], "iron_pickaxe")
        self.assertFalse(decoded["interrupted"])
        self.assertTrue(decoded["completed"])

    def test_gathering_session_absent_tool_used_is_none(self):
        msg = _pb_string(1, "gather:43") + _pb_varint(5, 1)
        decoded = proto_min.decode_server_data_envelope(_pb_message(30, msg))
        self.assertIsNone(decoded["tool_used"], "缺 field 7 时 tool_used 应为 None")
        self.assertEqual(decoded["target_type"], "herb")

    def test_lingtian_session_tag31_decodes_kind_pos_and_optional(self):
        msg = (
            _pb_varint(1, 1)
            + _pb_varint(2, 3)  # LINGTIAN_SESSION_KIND_PLANTING
            + _pb_int32_field(3, 100)
            + _pb_int32_field(4, 72)
            + _pb_int32_field(5, -50)
            + _pb_varint(6, 12)
            + _pb_varint(7, 60)
            + _pb_string(8, "spirit_rice")
            + _pb_string(9, "player:Alice")
            + _pb_float32_field(10, 0.15)
            + _pb_varint(11, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(31, msg))
        self.assertEqual(decoded["type"], "lingtian_session")
        self.assertTrue(decoded["active"])
        self.assertEqual(decoded["kind"], "planting")
        self.assertEqual(
            decoded["pos"],
            [100, 72, -50],
            "pos_x/y/z 是 field 3/4/5（int32，含负坐标补码），平铺成 pos",
        )
        self.assertEqual(decoded["elapsed_ticks"], 12)
        self.assertEqual(decoded["target_ticks"], 60)
        self.assertEqual(decoded["plant_id"], "spirit_rice")
        self.assertEqual(decoded["source"], "player:Alice")
        self.assertAlmostEqual(decoded["dye_contamination"], 0.15, places=4)
        self.assertTrue(decoded["dye_contamination_warning"])

    def test_lingtian_session_empty_message_defaults(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(31, b""))
        self.assertFalse(decoded["active"])
        self.assertEqual(decoded["kind"], "unspecified")
        self.assertEqual(decoded["pos"], [0, 0, 0])
        self.assertIsNone(decoded["plant_id"])
        self.assertIsNone(decoded["source"])
        self.assertIsNone(decoded["dye_contamination"])
        self.assertFalse(decoded["dye_contamination_warning"])

    def test_skill_bar_config_tag36_decodes_item_skill_and_empty_slots(self):
        item = (
            _pb_string(1, "spirit_grass")
            + _pb_string(2, "灵草")
            + _pb_varint(3, 100)
            + _pb_varint(4, 200)
            + _pb_string(5, "bong:icons/spirit_grass")
        )
        skill = (
            _pb_string(1, "meditate")
            + _pb_string(2, "静心")
            + _pb_varint(3, 500)
            + _pb_varint(4, 3000)
            + _pb_string(5, "bong:icons/meditate")
        )
        slot_item = _pb_message(1, _pb_message(1, item))  # entry.item = field 1
        slot_skill = _pb_message(1, _pb_message(2, skill))  # entry.skill = field 2
        packed = _pb_bytes(2, _pb_raw_varint(123) + _pb_raw_varint(456))
        msg = (
            _pb_message(1, slot_item)
            + _pb_message(1, slot_skill)
            + _pb_message(1, b"")  # 空 OptionalSkillBarEntry → None
            + packed
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(36, msg))
        self.assertEqual(decoded["type"], "skillbar_config")
        self.assertEqual(
            decoded["slots"][0],
            {
                "kind": "item",
                "template_id": "spirit_grass",
                "display_name": "灵草",
                "cast_duration_ms": 100,
                "cooldown_ms": 200,
                "icon_texture": "bong:icons/spirit_grass",
            },
        )
        self.assertEqual(
            decoded["slots"][1],
            {
                "kind": "skill",
                "skill_id": "meditate",
                "display_name": "静心",
                "cast_duration_ms": 500,
                "cooldown_ms": 3000,
                "icon_texture": "bong:icons/meditate",
            },
        )
        self.assertIsNone(decoded["slots"][2])
        self.assertEqual(
            decoded["cooldown_until_ms"],
            [123, 456],
            "field 2 的 packed uint64（length-delimited 内连 varint）必须解包",
        )

    def test_skill_bar_config_cooldowns_accept_unpacked_wire(self):
        msg = _pb_varint(2, 1000) + _pb_varint(2, 2000)
        decoded = proto_min.decode_server_data_envelope(_pb_message(36, msg))
        self.assertEqual(decoded["type"], "skillbar_config")
        self.assertEqual(decoded["cooldown_until_ms"], [1000, 2000])

    def test_breakthrough_cinematic_tag71_decodes_full_contract(self):
        msg = (
            _pb_string(1, "player:Alice")
            + _pb_string(2, "lightning")
            + _pb_varint(3, 30)
            + _pb_varint(4, 120)
            + _pb_string(5, "spirit")
            + _pb_string(6, "void")
            + _pb_string(7, "success")
            + _pb_varint(8, 0)
            + _pb_fixed64(9, 100.0)
            + _pb_fixed64(10, 72.0)
            + _pb_fixed64(11, -200.0)
            + _pb_fixed64(12, 64.0)
            + _pb_varint(13, 1)
            + _pb_varint(14, 1)
            + _pb_float32_field(15, 0.8)
            + _pb_float32_field(16, 0.5)
            + _pb_string(17, "autumn")
            + _pb_string(18, "cinematic")
            + _pb_varint(19, 4242)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(71, msg))
        self.assertEqual(decoded["type"], "breakthrough_cinematic")
        self.assertEqual(decoded["actor_id"], "player:Alice")
        self.assertEqual(decoded["phase"], "lightning")
        self.assertEqual(decoded["phase_tick"], 30)
        self.assertEqual(decoded["phase_duration_ticks"], 120)
        self.assertEqual(decoded["realm_from"], "spirit")
        self.assertEqual(decoded["realm_to"], "void")
        self.assertEqual(decoded["result"], "success")
        self.assertFalse(decoded["interrupted"])
        self.assertEqual(
            decoded["world_pos"],
            [100.0, 72.0, -200.0],
            "world_pos_x/y/z 是 field 9/10/11，拆成平铺坐标",
        )
        self.assertEqual(decoded["visible_radius_blocks"], 64.0)
        self.assertTrue(decoded["global"])
        self.assertTrue(decoded["distant_billboard"])
        self.assertAlmostEqual(decoded["particle_density"], 0.8, places=4)
        self.assertAlmostEqual(decoded["intensity"], 0.5, places=4)
        self.assertEqual(decoded["season_overlay"], "autumn")
        self.assertEqual(decoded["style"], "cinematic")
        self.assertEqual(decoded["at_tick"], 4242)

    def test_player_state_tag5_decodes_realm_enum(self):
        msg = _pb_varint(2, 5) + _pb_fixed64(3, 65.0) + _pb_fixed64(11, 100.0)
        decoded = proto_min.decode_server_data_envelope(_pb_message(5, msg))
        self.assertEqual(decoded["type"], "player_state")
        self.assertEqual(decoded["realm"], "Spirit", "PlayerState.realm=5 应解为 Spirit")
        self.assertEqual(decoded["spirit_qi"], 65.0)

    def test_player_state_absent_realm_uses_unspecified(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(5, b""))
        self.assertEqual(decoded["realm"], "Unspecified", "缺 field 2 应解为 Unspecified")

    def test_alchemy_outcome_resolved_tag14_full_contract(self):
        msg = (
            _pb_varint(1, 1)  # ALCHEMY_OUTCOME_BUCKET_PERFECT
            + _pb_string(2, "ling_xi_wan_v1")
            + _pb_string(3, "灵犀丹")
            + _pb_fixed64(4, 0.95)
            + _pb_fixed64(5, 0.05)
            + _pb_varint(6, 10)  # COLOR_KIND_TURBID
            + _pb_fixed64(7, 2.0)
            + _pb_string(8, "meridian_sore")
            + _pb_varint(9, 1)
            + _pb_fixed64(10, 1.5)
            + _pb_fixed64(11, 0.2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(14, msg))
        self.assertEqual(decoded["type"], "alchemy_outcome_resolved")
        self.assertEqual(decoded["bucket"], "perfect")
        self.assertEqual(decoded["recipe_id"], "ling_xi_wan_v1")
        self.assertEqual(decoded["pill"], "灵犀丹")
        self.assertEqual(decoded["quality"], 0.95)
        self.assertEqual(decoded["toxin_amount"], 0.05)
        self.assertEqual(decoded["toxin_color"], "turbid")
        self.assertEqual(decoded["qi_gain"], 2.0)
        self.assertEqual(decoded["side_effect_tag"], "meridian_sore")
        self.assertTrue(decoded["flawed_path"])
        self.assertEqual(decoded["damage"], 1.5)
        self.assertEqual(decoded["meridian_crack"], 0.2)

    def test_alchemy_outcome_resolved_empty_message_defaults(self):
        decoded = proto_min.decode_server_data_envelope(_pb_message(14, b""))
        self.assertEqual(decoded["bucket"], "unspecified")
        self.assertIsNone(decoded["recipe_id"])
        self.assertIsNone(decoded["pill"])
        self.assertIsNone(decoded["quality"])
        self.assertIsNone(decoded["toxin_amount"])
        self.assertIsNone(decoded["toxin_color"])
        self.assertIsNone(decoded["qi_gain"])
        self.assertIsNone(decoded["side_effect_tag"])
        self.assertFalse(decoded["flawed_path"])
        self.assertIsNone(decoded["damage"])
        self.assertIsNone(decoded["meridian_crack"])

    def test_out_of_range_enum_values_fall_back_to_unknown(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(14, _pb_varint(1, 99))
        )
        self.assertEqual(decoded["bucket"], "unknown_99")
        gathering = proto_min.decode_server_data_envelope(
            _pb_message(30, _pb_varint(5, 7))
        )
        self.assertEqual(gathering["target_type"], "unknown_7")
        xp_gain = proto_min.decode_server_data_envelope(
            _pb_message(7, _pb_varint(2, 99))
        )
        self.assertEqual(xp_gain["skill"], "unknown_99")
        scroll_used = proto_min.decode_server_data_envelope(
            _pb_message(97, _pb_varint(3, 99))
        )
        self.assertEqual(scroll_used["skill"], "unknown_99")

    def test_new_registry_tags_dispatch_to_named_decoders(self):
        for field, expected_type in (
            (5, "player_state"),
            (14, "alchemy_outcome_resolved"),
            (25, "botany_harvest_progress"),
            (30, "gathering_session"),
            (31, "lingtian_session"),
            (36, "skillbar_config"),
            (71, "breakthrough_cinematic"),
        ):
            with self.subTest(field=field):
                decoded = proto_min.decode_server_data_envelope(
                    _pb_message(field, b"")
                )
                self.assertIsNotNone(
                    decoded,
                    f"envelope tag {field} 必须有深度解码器，实际返回 None",
                )
                self.assertEqual(decoded["type"], expected_type)
        self.assertIsNone(
            proto_min.decode_server_data_envelope(_pb_message(6, b"")),
            "无深度解码器的 known oneof tag（cultivation_detail）应返回 None，不得误分发",
        )


class PlayerPacketContractTest(unittest.TestCase):
    """S1 新增玩家身份包（Player List/Remove/Spawn）与两张身份表的契约 pin。

    central-review finding 2：bot.py 新增的 PlayerList/PlayerRemove/PlayerSpawn
    解析与 entity→player 身份映射没有任何 packet-level 断言——忽略 action mask、
    给 spawn 配错 UUID、destroy 不删身份映射，场景都会照样绿。本类逐项钉死
    这些正/负半面。
    """

    def _player_list(self, bot, actions: int, raw_entries: list[bytes]):
        body = (
            mc.write_varint(mc.S2C_PLAYER_LIST)
            + bytes([actions])
            + mc.write_varint(len(raw_entries))
            + b"".join(raw_entries)
        )
        bot._dispatch(body)

    def test_player_list_add_updates_names_and_emits_entries(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        self._player_list(
            bot,
            0x11,  # add_player | update_latency
            [
                alice.bytes + mc.mc_string("Alice") + mc.write_varint(0) + mc.write_varint(42),
                bob.bytes + mc.mc_string("Bob") + mc.write_varint(0) + mc.write_varint(7),
            ],
        )
        self.assertEqual(bot.player_names, {str(alice): "Alice", str(bob): "Bob"})
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["actions"], 0x11)
        self.assertEqual(len(event.data["entries"]), 2)
        self.assertEqual(event.data["entries"][0]["uuid"], str(alice))
        self.assertEqual(event.data["entries"][0]["username"], "Alice")
        self.assertEqual(event.data["entries"][1]["username"], "Bob")

    def test_player_list_non_add_entry_does_not_fabricate_username(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        self._player_list(bot, 0x10, [alice.bytes + mc.write_varint(99)])
        self.assertEqual(
            bot.player_names,
            {},
            "仅 update_latency 的 entry 不得登记用户名——实现若忽略 action mask 就会误读 99 当 username",
        )
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["entries"][0]["uuid"], str(alice))
        self.assertNotIn("username", event.data["entries"][0])

    def test_player_list_full_action_mask_parses_all_action_payloads(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        entry_alice = (
            alice.bytes
            + mc.mc_string("Alice") + mc.write_varint(1)  # add_player + 1 property
            + mc.mc_string("textures") + mc.mc_string("http://skin") + b"\x01" + mc.mc_string("sig")
            + b"\x01" + alice.bytes + struct.pack(">q", 1234)  # initialize_chat signed session
            + mc.write_varint(2) + b"\xab\xcd" + mc.write_varint(1) + b"\xef"  # chat key/signature
            + mc.write_varint(4)  # update_game_mode
            + b"\x01"  # update_listed
            + mc.write_varint(15)  # update_latency
            + b"\x01" + mc.mc_string("§aAlice")  # update_display_name
        )
        # 第二条 entry 用不同的值，任何一条变长 payload 错位都会让本条的
        # 字段对不上号，从而暴露第一条 entry 的解析越界。
        entry_bob = (
            bob.bytes
            + mc.mc_string("Bob") + mc.write_varint(0)  # add_player + 0 properties
            + b"\x00"  # initialize_chat: no chat session
            + mc.write_varint(3)  # update_game_mode
            + b"\x00"  # update_listed: false
            + mc.write_varint(7)  # update_latency
            + b"\x00"  # update_display_name: absent
        )
        self._player_list(bot, 0x3F, [entry_alice, entry_bob])
        self.assertEqual(
            bot.player_names,
            {str(alice): "Alice", str(bob): "Bob"},
            "两条 entry 都要登记 username",
        )
        event = bot.events_of("player_list")[-1]
        self.assertEqual(event.data["actions"], 0x3F)
        parsed = event.data["entries"][0]
        # 六个 action 的 payload 全部要解码出来并被断言，跳过任意一个都会让
        # 后面的字段读错（尤其变长 payload：properties / chat key/sig / display_name）
        self.assertEqual(parsed["uuid"], str(alice))
        self.assertEqual(
            parsed["username"],
            "Alice",
            "0x01 add_player 必须解出 username",
        )
        self.assertEqual(
            parsed["properties"],
            [{"name": "textures", "value": "http://skin", "signature": "sig"}],
            "0x01 add_player 的 property 三元组（name/value/可选 signature）必须逐项解出",
        )
        self.assertEqual(
            parsed["initialize_chat"],
            {
                "has_chat_session": True,
                "session_id": str(alice),
                "public_key_expiry": 1234,
                "public_key": b"\xab\xcd",
                "signature": b"\xef",
            },
            "0x02 initialize_chat 的 signed session（has/会话 id/过期时间/公钥/signature）必须逐项解出",
        )
        self.assertEqual(
            parsed["game_mode"],
            4,
            "0x04 update_game_mode 必须解出 varint 值",
        )
        self.assertEqual(
            parsed["listed"],
            True,
            "0x08 update_listed 必须解出 boolean 值",
        )
        self.assertEqual(
            parsed["latency"],
            15,
            "0x10 update_latency 必须解出 varint 值",
        )
        self.assertEqual(
            parsed["display_name"],
            "§aAlice",
            "0x20 update_display_name 必须解出变长 string",
        )
        bob_entry = event.data["entries"][1]
        self.assertEqual(
            bob_entry,
            {
                "uuid": str(bob),
                "username": "Bob",
                "properties": [],
                "initialize_chat": {"has_chat_session": False},
                "game_mode": 3,
                "listed": False,
                "latency": 7,
            },
            "第二条 entry 六字段齐全且各为独立值——第一条的任何变长 payload 错位都会在此暴露",
        )

    def test_player_remove_pops_names(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        bob = uuid.UUID(int=2)
        self._player_list(
            bot,
            0x01,
            [
                alice.bytes + mc.mc_string("Alice") + mc.write_varint(0),
                bob.bytes + mc.mc_string("Bob") + mc.write_varint(0),
            ],
        )
        remove = (
            mc.write_varint(mc.S2C_PLAYER_REMOVE)
            + mc.write_varint(1)
            + alice.bytes
        )
        bot._dispatch(remove)
        self.assertEqual(bot.player_names, {str(bob): "Bob"})
        event = bot.events_of("player_remove")[-1]
        self.assertEqual(event.data["uuids"], [str(alice)])

    def test_player_spawn_associates_entity_with_uuid_and_username(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        self._player_list(bot, 0x01, [alice.bytes + mc.mc_string("Alice") + mc.write_varint(0)])
        spawn = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(900)
            + alice.bytes
            + struct.pack(">ddd", 12.0, 64.0, -5.0)
            + b"\x00\x00"
        )
        bot._dispatch(spawn)
        self.assertEqual(bot.entity_pos(900), (12.0, 64.0, -5.0))
        self.assertEqual(
            bot.player_entity_uuids[900],
            str(alice),
            "player_spawn 必须把 entity_id 关联到正确的玩家 UUID",
        )
        event = bot.events_of("player_spawn")[-1]
        self.assertEqual(event.data["entity_id"], 900)
        self.assertEqual(event.data["uuid"], str(alice))
        self.assertEqual(event.data["username"], "Alice")
        self.assertEqual(event.data["yaw"], 0)
        self.assertEqual(event.data["pitch"], 0)

    def test_player_spawn_exact_event_and_entities(self):
        # central-review 2029 #1：packet 级 pin 完整解码契约——已知 uuid + 正负坐标，
        # 逐字段精确还原 emitted event 与 self.entities 位置登记；坐标读取提前跳
        # 15/17 字节等错误布局会在此撞红（不只是 entity_id 为正）。
        bot = _bare_bot()
        alice = uuid.UUID(int=424242)
        spawn = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(900)
            + alice.bytes
            + struct.pack(">ddd", 12.5, 64.0, -33.25)
            + b"\x2a\x4b"  # yaw=42, pitch=75
        )
        bot._dispatch(spawn)
        self.assertEqual(bot.entity_pos(900), (12.5, 64.0, -33.25))
        self.assertEqual(bot.player_entity_uuids[900], str(alice))
        event = bot.events_of("player_spawn")[-1]
        self.assertEqual(
            event.data,
            {
                "entity_id": 900,
                "uuid": str(alice),
                "username": None,
                "x": 12.5,
                "y": 64.0,
                "z": -33.25,
                "yaw": 42,
                "pitch": 75,
            },
            "player_spawn 必须逐字段精确还原（含正负坐标、UUID 与实体位置登记）",
        )

    def test_destroy_removes_entity_to_player_mapping(self):
        bot = _bare_bot()
        alice = uuid.UUID(int=1)
        spawn = (
            mc.write_varint(mc.S2C_PLAYER_SPAWN)
            + mc.write_varint(900)
            + alice.bytes
            + struct.pack(">ddd", 1.0, 2.0, 3.0)
            + b"\x00\x00"
        )
        bot._dispatch(spawn)
        self.assertIn(900, bot.player_entity_uuids)
        destroy = (
            mc.write_varint(mc.S2C_ENTITIES_DESTROY)
            + mc.write_varint(1)
            + mc.write_varint(900)
        )
        bot._dispatch(destroy)
        self.assertIsNone(bot.entity_pos(900))
        self.assertNotIn(
            900,
            bot.player_entity_uuids,
            "destroy 必须同时删除 entity→player 身份映射",
        )

    def test_entity_spawn_exposes_uuid(self):
        bot = _bare_bot()
        uid = uuid.UUID(int=77)
        body = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(10)
            + uid.bytes
            + mc.write_varint(3)
            + struct.pack(">ddd", 5.0, 6.0, 7.0)
        )
        bot._dispatch(body)
        self.assertEqual(bot.entity_pos(10), (5.0, 6.0, 7.0))
        event = bot.events_of("entity_spawn")[-1]
        self.assertEqual(event.data["uuid"], str(uid))

    def test_rel_move_emits_entity_move_event(self):
        bot = _bare_bot()
        spawn = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(7) + b"\x00" * 16 + mc.write_varint(1)
            + struct.pack(">ddd", 10.0, 64.0, -3.0)
        )
        bot._dispatch(spawn)
        move = (
            mc.write_varint(mc.S2C_ENTITY_POSITION)
            + mc.write_varint(7)
            + struct.pack(">hhh", 4096, -2048, 0)
            + b"\x01"
        )
        bot._dispatch(move)
        event = bot.events_of("entity_move")[-1]
        self.assertEqual(event.data["entity_id"], 7)
        self.assertAlmostEqual(event.data["x"], 11.0)
        self.assertAlmostEqual(event.data["y"], 63.5)
        self.assertAlmostEqual(event.data["z"], -3.0)

    def test_teleport_emits_entity_move_event(self):
        bot = _bare_bot()
        spawn = (
            mc.write_varint(mc.S2C_ENTITY_SPAWN)
            + mc.write_varint(7) + b"\x00" * 16 + mc.write_varint(1)
            + struct.pack(">ddd", 10.0, 64.0, -3.0)
        )
        bot._dispatch(spawn)
        teleport = (
            mc.write_varint(mc.S2C_ENTITY_TELEPORT)
            + mc.write_varint(7)
            + struct.pack(">ddd", -100.0, 70.0, 200.0)
            + b"\x00\x00\x01"
        )
        bot._dispatch(teleport)
        event = bot.events_of("entity_move")[-1]
        self.assertEqual(event.data, {"entity_id": 7, "x": -100.0, "y": 70.0, "z": 200.0})
class NpcDialogueHelperContractTest(unittest.TestCase):
    def test_scenario_spawn_requires_fixed_chase_geometry(self):
        origin = (10.0, 64.0, -3.0)
        valid = _FakeEvent(
            2.0,
            "entity_spawn",
            {"type": 118, "entity_id": 7, "x": 22.0, "y": 64.0, "z": -3.0},
        )
        ambient = _FakeEvent(
            2.1,
            "entity_spawn",
            {"type": 118, "entity_id": 8, "x": 18.0, "y": 64.0, "z": -3.0},
        )
        self.assertTrue(
            _scenario_spawn_matches(valid, origin),
            "chase 单体应只接受 origin +X 12 格的 type=118 出生点",
        )
        self.assertFalse(
            _scenario_spawn_matches(ambient, origin),
            "远处环境僵尸不得冒充 /npc_scenario chase 的返回实体",
        )

    def test_trade_metadata_requires_rogue_and_preserves_all_observed_offers(self):
        payload = {
            "type": "npc_metadata",
            "entity_id": 42,
            "archetype": "rogue",
            "trade_offers": [
                {"template_id": "skill_scroll_herbalism_baicao_can", "price_bone_coins": 30},
                {"template_id": "broken_artifact_scroll", "price_bone_coins": 40},
            ],
        }
        event = _FakeEvent(
            3.0,
            "payload",
            {"channel": "bong:npc_metadata", "data": json.dumps(payload).encode()},
        )
        self.assertEqual(
            _trade_metadata(event),
            (
                42,
                [
                    ("skill_scroll_herbalism_baicao_can", 30),
                    ("broken_artifact_scroll", 40),
                ],
            ),
            "场景必须按服务端 metadata 的实际库存覆盖高境界商品，而不是硬编码三件醒灵商品",
        )
        payload["archetype"] = "commoner"
        self.assertIsNone(
            _trade_metadata(
                _FakeEvent(
                    3.1,
                    "payload",
                    {"channel": "bong:npc_metadata", "data": json.dumps(payload).encode()},
                )
            ),
            "非 Rogue 的 metadata 不得被选为散修商贩",
        )

    def test_trade_range_recovery_resends_the_same_observed_offer(self):
        class TradeBot:
            def __init__(self):
                self._lock = threading.RLock()
                self.events: list[_FakeEvent] = []
                self.requests: list[dict] = []

            def intent(self, request):
                self.requests.append(request)

            def wait_for(self, predicate, timeout, description):
                text = (
                    npc_dialogue_trade.OUT_OF_RANGE_TRADE
                    if len(self.requests) == 1
                    else "§c[NPC] 骨币不足，需要 30 枚。"
                )
                event = _FakeEvent(float(len(self.requests)), "chat", {"text": text})
                self.events.append(event)
                if predicate(event):
                    return event
                raise AssertionError(f"未匹配 {description}: {event!r}")

        bot = TradeBot()
        with (
            mock.patch.object(npc_dialogue_trade, "request_and_assert_rogue"),
            mock.patch.object(npc_dialogue_trade, "approach_entity", return_value=True),
        ):
            npc_dialogue_trade._run_rogue_chain(
                bot, 77, [("skill_scroll_herbalism_baicao_can", 30)]
            )
        self.assertEqual(
            [request["requested_item_id"] for request in bot.requests],
            ["skill_scroll_herbalism_baicao_can", "skill_scroll_herbalism_baicao_can"],
            "越界恢复必须重发当前 metadata offer，不得 advance/结束该商品",
        )

# ── agent_ui 场景底座（_redis_client_helpers / _agent_ui_helpers）单测 ──────────────


class RespFramesTest(unittest.TestCase):
    """RESP2 编解码器：feed 字节流 → 按帧取回值（纯 stdlib，无需 redis）。"""

    def test_integer_reply(self):
        parser = RespFrames()
        parser.feed(b":1\r\n")
        self.assertEqual(parser.next_frame(), 1)
        self.assertIs(
            parser.next_frame(),
            _INCOMPLETE_FRAME,
            "帧取完后应返回 _INCOMPLETE_FRAME",
        )

    def test_simple_string(self):
        parser = RespFrames()
        parser.feed(b"+OK\r\n")
        self.assertEqual(parser.next_frame(), "OK")

    def test_error_string(self):
        parser = RespFrames()
        parser.feed(b"-ERR bad\r\n")
        self.assertEqual(parser.next_frame(), "ERR bad")

    def test_bulk_string(self):
        parser = RespFrames()
        parser.feed(b"$5\r\nhello\r\n")
        self.assertEqual(parser.next_frame(), b"hello")

    def test_empty_bulk_string(self):
        parser = RespFrames()
        parser.feed(b"$0\r\n\r\n")
        self.assertEqual(parser.next_frame(), b"")

    def test_nil_bulk_string(self):
        parser = RespFrames()
        parser.feed(b"$-1\r\n:42\r\n")
        frame = parser.next_frame()
        self.assertIsNone(frame, "nil bulk 应解析为 None")
        self.assertIsNot(
            frame,
            _INCOMPLETE_FRAME,
            "完整 nil 帧是已解析的值，不是数据不足哨兵",
        )
        self.assertEqual(
            parser.next_frame(),
            42,
            "nil bulk 帧必须被消费，后续帧才能继续解析",
        )

    def test_nil_bulk_distinct_from_incomplete(self):
        # finding：完整 nil 帧（$-1）返回 None 但已消费，与「还需更多字节」
        # 必须可区分。旧实现两者都返回 None，调用方无法判断收全了一条 nil。
        parser = RespFrames()
        self.assertIs(
            parser.next_frame(),
            _INCOMPLETE_FRAME,
            "空缓冲 = 数据不足，不应出帧",
        )
        parser.feed(b"$-1\r\n")
        self.assertIsNone(parser.next_frame(), "完整 nil 帧应解析为 None")
        self.assertIs(
            parser.next_frame(),
            _INCOMPLETE_FRAME,
            "nil 帧消费后回到数据不足",
        )

    def test_malformed_bulk_terminator_rejected(self):
        # finding：$3\r\nabcXX 的 body 后两字节不是 \r\n，是协议违例，必须拒绝；
        # 旧实现只确认有 2 字节就消费，把 b"abc" 当合法帧返回。
        parser = RespFrames()
        parser.feed(b"$3\r\nabcXX")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_malformed_bulk_terminator_desync_rejected(self):
        # finding：body 后两字节非 \r\n 会让帧边界错位；拼接输入也必须整体拒绝，
        # 不得把后面的 :1 当作独立整数帧吞掉。
        parser = RespFrames()
        parser.feed(b"$3\r\nabcXX:1\r\n")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_array_frame(self):
        parser = RespFrames()
        parser.feed(b"*3\r\n$9\r\nsubscribe\r\n$6\r\nbong:x\r\n:1\r\n")
        self.assertEqual(parser.next_frame(), [b"subscribe", b"bong:x", 1])

    def test_bulk_negative_length_below_nil_rejected(self):
        # finding：RESP2 长度只允许 -1（nil）或非负。$-2 会被误当成 body 长度 0
        # 把头部 CRLF 认作终止符返回 b''；必须拒绝而不是静默接受畸形线值。
        parser = RespFrames()
        parser.feed(b"$-2\r\n")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_array_negative_count_below_nil_rejected(self):
        # finding：*-2 的 range(-2) 不迭代，被误解析为空数组 []；RESP2 不允许，
        # 必须拒绝而不是静默接受畸形线值。
        parser = RespFrames()
        parser.feed(b"*-2\r\n")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_message_frame(self):
        parser = RespFrames()
        parser.feed(b"*3\r\n$7\r\nmessage\r\n$6\r\nbong:x\r\n$5\r\nhello\r\n")
        self.assertEqual(parser.next_frame(), [b"message", b"bong:x", b"hello"])

    def test_partial_feeds(self):
        parser = RespFrames()
        raw = b"*3\r\n$7\r\nmessage\r\n$6\r\nbong:x\r\n$5\r\nhello\r\n"
        for i in range(0, len(raw), 3):
            self.assertIs(
                parser.next_frame(),
                _INCOMPLETE_FRAME,
                "字节未喂全时不应出帧",
            )
            parser.feed(raw[i : i + 3])
        self.assertEqual(parser.next_frame(), [b"message", b"bong:x", b"hello"])

    def test_concatenated_frames(self):
        parser = RespFrames()
        parser.feed(b":1\r\n*3\r\n$7\r\nmessage\r\n$6\r\nbong:x\r\n$0\r\n\r\n")
        self.assertEqual(parser.next_frame(), 1)
        self.assertEqual(parser.next_frame(), [b"message", b"bong:x", b""])

    def test_unknown_marker_rejected(self):
        parser = RespFrames()
        parser.feed(b"?nonsense\r\n")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_json_payload_in_message_frame(self):
        payload = b'{"request_id":"req_1","action":"button_click","params":{}}'
        raw = (
            b"*3\r\n$7\r\nmessage\r\n$22\r\nbong:agent_ui_response\r\n"
            + b"$"
            + str(len(payload)).encode("ascii")
            + b"\r\n"
            + payload
            + b"\r\n"
        )
        parser = RespFrames()
        parser.feed(raw)
        frame = parser.next_frame()
        self.assertEqual(frame[0], b"message")
        self.assertEqual(json.loads(frame[2].decode("utf-8"))["request_id"], "req_1")

    def test_fragmented_large_bulk(self):
        # 大 bulk 分 4096 块喂入：多次 partial feed 后仍须正确拼回。feed 走
        # bytearray.extend（摊还线性）；若退回 ``bytes += `` 就会逐块整拷积压缓冲，
        # 大消息退化为二次拷贝。
        payload = b"x" * (64 * 1024)
        raw = b"$" + str(len(payload)).encode("ascii") + b"\r\n" + payload + b"\r\n"
        parser = RespFrames()
        for i in range(0, len(raw), 4096):
            parser.feed(raw[i : i + 4096])
        self.assertEqual(parser.next_frame(), payload)

    def test_bulk_declared_oversize_rejected(self):
        # 单条 bulk 声明长度超上限：帧未收全也必须立刻拒绝（防超限帧占用合法化）。
        parser = RespFrames(max_bulk_size=8)
        parser.feed(b"$9\r\n123456789\r\n")
        with self.assertRaises(ValueError):
            parser.next_frame()

    def test_bulk_declared_at_max_accepted(self):
        # 上界合法等值边界：声明长度恰好 == max_bulk_size 必须完整接受（> 上限才拒绝）。
        # 缺此断言时，把 >= 当 > 的 off-by-one 实现能通过全套却拒绝恰好到顶的合法载荷。
        parser = RespFrames(max_bulk_size=8)
        parser.feed(b"$8\r\n12345678\r\n")
        self.assertEqual(parser.next_frame(), b"12345678")

    def test_buffer_oversize_rejected(self):
        # 永不收尾的帧反复 feed，积压缓冲越过上限必须抛错（防外部消息撑爆进程内存）。
        parser = RespFrames(max_buffer_size=16)
        parser.feed(b"$100\r\n0123456789")  # 15 bytes，帧未完整
        with self.assertRaises(ValueError):
            parser.feed(b"0123456789")  # 累计 25 > 16


class RedisCommandEncodeTest(unittest.TestCase):
    """PUBLISH / SUBSCRIBE 命令编码（与 RESP2 协议字节精确对拍）。"""

    def test_publish_encoding(self):
        self.assertEqual(
            _encode_command("PUBLISH", "bong:agent_ui_cmd", '{"a":1}'),
            b'*3\r\n$7\r\nPUBLISH\r\n$17\r\nbong:agent_ui_cmd\r\n$7\r\n{"a":1}\r\n',
        )

    def test_subscribe_encoding(self):
        self.assertEqual(
            _encode_command("SUBSCRIBE", "bong:agent_ui_response"),
            b"*2\r\n$9\r\nSUBSCRIBE\r\n$22\r\nbong:agent_ui_response\r\n",
        )


class _FakeTimeoutSocket:
    """recv 先按 settimeout 配置真实阻塞再抛 TimeoutError 的假 socket。

    若 recv 恒立即抛错，wait_message 把 socket 超时设成整个 io_timeout（或漏调
    settimeout）也无法被测出——墙钟只由循环里的 deadline 检查决定。这里 recv
    实际睡 self.timeout 秒再抛，超时值超界会把等待拉长到可断言；同时记录每次
    settimeout 值，供测试校验其受剩余 deadline 约束。
    """

    def __init__(self) -> None:
        self.timeout: float | None = None
        self._settimeouts: list[float] = []

    def settimeout(self, timeout: float) -> None:
        self.timeout = timeout
        self._settimeouts.append(timeout)

    def recv(self, _size: int) -> bytes:
        if self.timeout is None:
            raise AssertionError(
                "recv 被调用但未先 settimeout（wait_message 漏设 socket 超时）"
            )
        time.sleep(self.timeout)
        raise TimeoutError("timed out")


class _FakeDripPartialSocket:
    """recv 每次成功返回一小块「永不收尾」的部分 RESP 帧字节的假 socket。

    帧重组路径：分片持续到达（每次 recv 都有数据、从不抛 TimeoutError），socket
    超时永远触发不了，只有 ``_next_frame`` 内部逐次核对绝对 deadline 才能让
    ``wait_message`` 按时返回——靠外层循环 deadline 检查兜底的实现会无限 recv 下去。
    首块喂声明 2MB bulk 的长度头，之后每块只给 512B body，单帧永远重组不完。
    记录每次 settimeout 值供测试校验其受剩余 deadline 约束；recv 设上限兜底，防
    回归实现让测试无限挂起。
    """

    def __init__(self) -> None:
        self.timeout: float | None = None
        self._settimeouts: list[float] = []
        self._recvs = 0

    def settimeout(self, timeout: float) -> None:
        self.timeout = timeout
        self._settimeouts.append(timeout)

    def recv(self, _size: int) -> bytes:
        if self.timeout is None:
            raise AssertionError("recv 被调用但未先 settimeout")
        self._recvs += 1
        if self._recvs > 2000:
            raise AssertionError(
                "部分帧被持续喂入但绝对 deadline 未被强制：wait_message 应按时返回"
            )
        # 每块在 socket 超时内到达（睡极短时间），触发不了 socket 级 TimeoutError。
        time.sleep(min(self.timeout, 0.001))
        if self._recvs == 1:
            return b"$2000000\r\n"
        return b"z" * 512


class RedisClientWaitDeadlineTest(unittest.TestCase):
    """wait_message 的 recv 窗口必须受调用方 deadline 约束。

    回归：io_timeout(10s) 大于负向断言窗口(2s)时，固定 10s 阻塞 recv 会直接抛
    TimeoutError 崩掉场景，而不是按 deadline 返回 None / 抛 AssertionError。
    """

    def _redis_with_timeout_socket(self, timeout: float = 2.0) -> RedisClient:
        redis = RedisClient(io_timeout=timeout)
        redis._sub_sock = _FakeTimeoutSocket()  # type: ignore[assignment]
        return redis

    def _assert_timeouts_bounded(self, sock: _FakeTimeoutSocket, window: float) -> None:
        """socket 超时值必须受调用方 deadline（window）约束，不能退化成 io_timeout。"""
        self.assertTrue(
            sock._settimeouts,
            "wait_message 必须按剩余 deadline 调用 settimeout，不能漏设",
        )
        self.assertLessEqual(
            max(sock._settimeouts),
            window + 0.05,
            "socket 超时绝不能超过调用方 deadline，不能设成整个 io_timeout",
        )

    def test_negative_window_returns_none_after_deadline(self):
        redis = self._redis_with_timeout_socket(timeout=2.0)
        sock = redis._sub_sock
        started = time.monotonic()
        got = redis.wait_message(
            "bong:agent_ui_response", lambda payload: False, timeout=0.2, expect=False
        )
        elapsed = time.monotonic() - started
        self.assertIsNone(got, "负向窗口超时应返回 None，而不是抛 TimeoutError")
        self.assertGreaterEqual(
            elapsed, 0.2, "负向窗口必须等满 deadline 才返回，不能提前返回"
        )
        self.assertLess(
            elapsed, 1.5, "负向窗口应约在 deadline 处返回，不拖满 io_timeout"
        )
        self._assert_timeouts_bounded(sock, 0.2)

    def test_positive_window_raises_assertion_after_deadline(self):
        redis = self._redis_with_timeout_socket(timeout=2.0)
        sock = redis._sub_sock
        started = time.monotonic()
        with self.assertRaisesRegex(AssertionError, "期望 0.2s 内收到"):
            redis.wait_message(
                "bong:agent_ui_response",
                lambda payload: False,
                timeout=0.2,
                expect=True,
            )
        elapsed = time.monotonic() - started
        self.assertGreaterEqual(
            elapsed, 0.2, "正窗口断言必须等满 deadline 才抛，不能提前抛"
        )
        self._assert_timeouts_bounded(sock, 0.2)

    def test_non_dict_json_payload_skipped_not_crashed(self):
        # finding：bong:agent_ui_response 上合法的非对象 JSON（null/数组/字符串/数字）
        # 会让 expect_redis_response.matches 对 payload 调 .get() 抛 AttributeError
        # 崩掉整个场景。必须当作无关发布者消息跳过（与频道不符/无效 JSON 同款），
        # 而不是传给 predicate。喂一帧 payload 为 JSON 数组的 message 帧，随后窗口
        # 走满：无崩溃 + 负向返回 None，即证明非 dict 被跳过。
        redis = RedisClient(io_timeout=2.0)
        redis._sub_sock = _FakeTimeoutSocket()  # type: ignore[assignment]
        json_bytes = b'["not","a","dict"]'
        frame = (
            b"*3\r\n$7\r\nmessage\r\n$22\r\nbong:agent_ui_response\r\n"
            + b"$"
            + str(len(json_bytes)).encode("ascii")
            + b"\r\n"
            + json_bytes
            + b"\r\n"
        )
        redis._frames.feed(frame)
        started = time.monotonic()
        got = redis.wait_message(
            "bong:agent_ui_response",
            lambda payload: True,
            timeout=0.2,
            expect=False,
        )
        elapsed = time.monotonic() - started
        self.assertIsNone(
            got, "非 dict JSON 消息应被跳过（继续等满窗口），而不是抛 AttributeError"
        )
        self.assertGreaterEqual(elapsed, 0.2, "跳过非 dict 后仍须等满负向窗口")

    def test_partial_frame_respects_absolute_deadline_negative(self):
        # finding：deadline 测试只覆盖完全空闲的 socket（recv 即抛 TimeoutError），
        # 帧重组路径（一次 _next_frame 内多次成功 recv）没测到——分片持续喂入、
        # socket 超时永不触发，只有绝对 deadline 被强制才按 0.2s 返回 None；
        # io_timeout=10s 明显大于窗口，若按 io_timeout 兜底会拖满 10s。
        redis = RedisClient(io_timeout=10.0)
        sock = _FakeDripPartialSocket()
        redis._sub_sock = sock  # type: ignore[assignment]
        started = time.monotonic()
        got = redis.wait_message(
            "bong:agent_ui_response", lambda payload: False, timeout=0.2, expect=False
        )
        elapsed = time.monotonic() - started
        self.assertIsNone(got, "部分帧持续喂入时负向窗口仍须按时返回 None")
        self.assertGreaterEqual(
            elapsed, 0.2, "负向窗口必须等满 deadline 才返回，不能提前返回"
        )
        self.assertLess(
            elapsed, 3.0, "部分帧不得拖过调用方 deadline（io_timeout=10s 更大）"
        )
        self.assertGreater(
            len(sock._settimeouts), 1, "帧重组应多次 recv/settimeout，而非单次阻塞"
        )
        self.assertLessEqual(
            max(sock._settimeouts),
            0.2 + 0.05,
            "帧重组期间的每次 socket 超时都必须受剩余 deadline 约束",
        )

    def test_partial_frame_respects_absolute_deadline_positive(self):
        # 正向路径同款：部分帧持续喂入时，断言必须在 deadline 处抛 AssertionError，
        # 而不是被无限重组拖住。
        redis = RedisClient(io_timeout=10.0)
        sock = _FakeDripPartialSocket()
        redis._sub_sock = sock  # type: ignore[assignment]
        started = time.monotonic()
        with self.assertRaisesRegex(AssertionError, "期望 0.2s 内收到"):
            redis.wait_message(
                "bong:agent_ui_response",
                lambda payload: False,
                timeout=0.2,
                expect=True,
            )
        elapsed = time.monotonic() - started
        self.assertGreaterEqual(elapsed, 0.2, "正窗口断言必须等满 deadline 才抛")
        self.assertLess(elapsed, 3.0, "部分帧不得拖过调用方 deadline（正向断言）")
        self.assertGreater(
            len(sock._settimeouts), 1, "帧重组应多次 recv/settimeout，而非单次阻塞"
        )


class _AgentUiFakeBot(_FakeBot):
    """wait_for 模拟真实 Bot 的 temporal wait 契约，供负向/时序断言测。

    真实 ``Bot.wait_for``（bot.py:389）以调用时刻为起点等到 deadline，窗口内
    到达即命中、窗口关闭后才到达不得命中。本 fake 用事件时间戳 ``t`` 建模同一条：
    ``t <= start`` 的事件视为观测前已送达（顺序消费，不得二次命中），``t > deadline``
    的事件视为窗口关闭后才到。这样负向断言能区分「窗口内迟到事件」（必须失败）
    与「窗口外事件」（返回 None），而不是对固定事件列表做一次扫描——后者会让
    「helper 立即返回 / 零超时 / 只快照当前事件」的错实现照样绿。
    """

    def __init__(self, events: list[_FakeEvent]) -> None:
        super().__init__(events)
        self._now = 0.0

    def wait_for(self, predicate, timeout: float, description: str):
        start = self._now
        deadline = start + timeout
        for event in self.events:
            if start < event.t <= deadline and predicate(event):
                self._now = event.t
                return event
        self._now = deadline
        raise BotAssertionError(f"未找到 {description}; events={self.events}")


def _req_event(t: float, request_id: str, **payload_overrides) -> _FakeEvent:
    payload = json.dumps(
        {
            "request_id": request_id,
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
            **payload_overrides,
        }
    ).encode("utf-8")
    return _FakeEvent(
        t, "payload", {"channel": "bong:agent_ui_request", "data": payload}
    )


class AgentUiHelperTest(unittest.TestCase):
    """_agent_ui_helpers 纯逻辑：cmd 构造 / S2C 形状断言 / 回执匹配 / 负向等待。"""

    def test_build_cmd_exact_six_fields(self):
        cmd = build_cmd("req_1", "offline:Fake")
        self.assertEqual(
            set(cmd),
            {
                "request_id",
                "target_player",
                "xml",
                "timeout_ticks",
                "realm_gate",
                "allowed_button_ids",
            },
            "AgentUiRequestCommandV1 六个字段全必填（server serde deny_unknown_fields）",
        )
        self.assertEqual(cmd["request_id"], "req_1")
        self.assertEqual(cmd["allowed_button_ids"], ["enter_realm", "cancel"])
        self.assertEqual(cmd["realm_gate"], 0)
        self.assertEqual(cmd["timeout_ticks"], 600, "默认 timeout_ticks 钉死 600")
        self.assertEqual(cmd["xml"], DEFAULT_UI_XML, "默认 xml 钉死 DEFAULT_UI_XML")

    def test_build_cmd_overrides(self):
        cmd = build_cmd(
            "req_2",
            "offline:Fake",
            timeout_ticks=20,
            realm_gate=6,
            allowed_button_ids=("ok",),
            xml="<x/>",
        )
        self.assertEqual(cmd["timeout_ticks"], 20)
        self.assertEqual(cmd["realm_gate"], 6)
        self.assertEqual(cmd["allowed_button_ids"], ["ok"])
        self.assertEqual(cmd["xml"], "<x/>")

    def test_build_cmd_json_roundtrip(self):
        cmd = build_cmd("req_3", "offline:Fake")
        self.assertEqual(json.loads(json.dumps(cmd)), cmd)

    def test_assert_request_shape_accepts_valid_payload(self):
        payload = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        assert_request_shape(payload, "req_1")

    def test_assert_request_shape_rejects_security_field_leak(self):
        base = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "realm_gate": 3}, "req_1")
        with self.assertRaises(BotAssertionError):
            assert_request_shape(
                {**base, "allowed_button_ids": ["enter_realm"]}, "req_1"
            )

    def test_assert_request_shape_rejects_wrong_request_id(self):
        payload = {
            "request_id": "other",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        with self.assertRaises(BotAssertionError):
            assert_request_shape(payload, "req_1")

    def test_assert_request_shape_rejects_empty_or_nonstr_target_player(self):
        base = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "target_player": ""}, "req_1")
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "target_player": 7}, "req_1")

    def test_assert_request_shape_rejects_empty_or_nonstr_xml(self):
        base = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "xml": ""}, "req_1")
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "xml": 7}, "req_1")

    def test_assert_request_shape_rejects_non_int_timeout_ticks(self):
        base = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": 600,
        }
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "timeout_ticks": "600"}, "req_1")
        with self.assertRaises(BotAssertionError):
            assert_request_shape({**base, "timeout_ticks": True}, "req_1")

    def test_assert_request_shape_pins_closed_fields_and_boundaries(self):
        base = {
            "request_id": "req_1",
            "target_player": "offline:Fake",
            "xml": "<owo-ui/>",
            "timeout_ticks": REQUEST_TIMEOUT_MIN,
        }
        assert_request_shape(base, "req_1")
        assert_request_shape(
            {**base, "timeout_ticks": REQUEST_TIMEOUT_MAX}, "req_1"
        )
        for bad_ticks in (REQUEST_TIMEOUT_MIN - 1, REQUEST_TIMEOUT_MAX + 1):
            with self.subTest(timeout_ticks=bad_ticks), self.assertRaisesRegex(
                BotAssertionError, "timeout_ticks"
            ):
                assert_request_shape({**base, "timeout_ticks": bad_ticks}, "req_1")

        assert_request_shape(
            {**base, "xml": "x" * REQUEST_XML_MAX_BYTES}, "req_1"
        )
        with self.assertRaisesRegex(BotAssertionError, "UTF-8 字节数"):
            assert_request_shape(
                {**base, "xml": "x" * (REQUEST_XML_MAX_BYTES + 1)}, "req_1"
            )
        with self.assertRaisesRegex(BotAssertionError, "额外"):
            assert_request_shape({**base, "unexpected": True}, "req_1")

    def test_assert_request_shape_compares_published_command_values(self):
        cmd = build_cmd(
            "req_cmd",
            "offline:Expected",
            timeout_ticks=2400,
            xml="<owo-ui><label>expected</label></owo-ui>",
        )
        payload = {field: cmd[field] for field in (
            "request_id",
            "target_player",
            "xml",
            "timeout_ticks",
        )}
        assert_request_shape(payload, "req_cmd", expected_cmd=cmd)
        for field, wrong in (
            ("target_player", "offline:Other"),
            ("xml", "<owo-ui><label>wrong</label></owo-ui>"),
            ("timeout_ticks", 20),
        ):
            with self.subTest(field=field), self.assertRaisesRegex(
                BotAssertionError, field
            ):
                assert_request_shape(
                    {**payload, field: wrong}, "req_cmd", expected_cmd=cmd
                )

    def test_expect_redis_response_rejects_wrong_same_request_before_expected(self):
        class FakeRedis:
            def wait_message(self, _channel, predicate, **_kwargs):
                for payload in (
                    {
                        "request_id": "req_same",
                        "action": "dismissed",
                        "params": {},
                    },
                    {
                        "request_id": "req_same",
                        "action": "timeout",
                        "params": {},
                    },
                ):
                    if predicate(payload):
                        return payload
                return None

        with self.assertRaisesRegex(BotAssertionError, "不匹配的同请求回执"):
            expect_redis_response(FakeRedis(), "req_same", action="timeout")

    def test_response_matches_action_and_params(self):
        self.assertTrue(
            response_matches(
                {
                    "request_id": "r",
                    "action": "button_click",
                    "params": {"button_id": "ok"},
                },
                action="button_click",
            )
        )
        self.assertTrue(
            response_matches(
                {"action": "error", "params": {"reason": "x"}},
                params_subset={"reason": "x"},
            )
        )
        self.assertTrue(
            response_matches(
                {"action": "error", "params": {"reason": "x", "extra": "y"}},
                params_subset={"reason": "x"},
            ),
            "params 子集匹配",
        )
        self.assertFalse(
            response_matches({"action": "dismissed"}, action="button_click")
        )
        self.assertFalse(
            response_matches(
                {"action": "error", "params": {"reason": "x"}},
                params_subset={"reason": "y"},
            )
        )
        self.assertFalse(
            response_matches(
                {"action": "error", "params": "not-a-dict"},
                params_subset={"reason": "x"},
            )
        )
        self.assertTrue(
            response_matches({"action": "timeout", "params": {}}, params_subset={}),
            "空子集要求 params 恰好为空",
        )
        self.assertFalse(
            response_matches(
                {"action": "timeout", "params": {"unexpected": "value"}},
                params_subset={},
            ),
            "声明空 params 时，额外参数必须拒绝",
        )
        self.assertFalse(
            response_matches({"action": "timeout"}, params_subset={}),
            "声明空 params 时，缺 params 字段必须拒绝",
        )
        self.assertFalse(
            response_matches({"action": "error"}, params_subset={"reason": "x"}),
            "声明 params 子集时，缺 params 字段必须拒绝",
        )
        self.assertFalse(
            response_matches(
                {"action": "error", "params": {}},
                params_subset={"reason": None},
            ),
            "子集匹配要求 key 存在：缺 reason 不能把缺失当显式 null 放行",
        )
        self.assertTrue(
            response_matches(
                {"action": "error", "params": {"reason": None}},
                params_subset={"reason": None},
            ),
            "显式 null 的 reason 应匹配（key 存在且值为 None）",
        )

    def test_response_matches_params_exact(self):
        # 原样转发契约：params 必须逐键精确相等，额外字段即违约（subset 会放行）。
        self.assertTrue(
            response_matches(
                {"action": "parse_error", "params": {"reason": "owo_parse_failed"}},
                action="parse_error",
                params_exact={"reason": "owo_parse_failed"},
            )
        )
        self.assertFalse(
            response_matches(
                {
                    "action": "parse_error",
                    "params": {"reason": "owo_parse_failed", "button_id": "injected"},
                },
                params_exact={"reason": "owo_parse_failed"},
            ),
            "精确匹配拒绝额外字段：注入 button_id 不得满足原样转发契约",
        )
        self.assertFalse(
            response_matches(
                {"action": "parse_error", "params": {"reason": "other"}},
                params_exact={"reason": "owo_parse_failed"},
            )
        )
        self.assertFalse(
            response_matches(
                {"action": "parse_error"}, params_exact={"reason": "owo_parse_failed"}
            ),
            "精确匹配要求 params 字段存在",
        )
        self.assertFalse(
            response_matches(
                {"action": "parse_error", "params": "not-a-dict"},
                params_exact={"reason": "owo_parse_failed"},
            )
        )
        with self.assertRaises(ValueError):
            response_matches(
                {"action": "error", "params": {"reason": "x"}},
                params_subset={"reason": "x"},
                params_exact={"reason": "x"},
            )

    def test_expect_agent_ui_request_positive(self):
        bot = _AgentUiFakeBot([_req_event(1.0, "req_1")])
        payload = expect_agent_ui_request(bot, "req_1", timeout=5.0)
        self.assertEqual(payload["request_id"], "req_1")
        self.assertEqual(payload["target_player"], "offline:Fake")

    def test_expect_agent_ui_request_negative_when_truly_absent(self):
        # 窗口内完全没有 payload 事件：负向断言超时返回 None（干净）。
        bot = _AgentUiFakeBot([])
        self.assertIsNone(
            expect_agent_ui_request(bot, "req_missing", timeout=5.0, expect=False),
            "窗口内无任何 payload 时负向断言应返回 None",
        )

    def test_expect_agent_ui_request_negative_fails_when_present(self):
        bot = _AgentUiFakeBot([_req_event(1.0, "req_1")])
        with self.assertRaises(BotAssertionError):
            expect_agent_ui_request(bot, "req_1", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_fails_for_other_request_id(self):
        # finding：拒绝路径契约是「面板不下发」，不是「不下发指定 request_id 的面板」。
        # request_id 与期望不符的 payload 在窗口内到达仍是错误下发，负向断言必须失败
        # —— 不能靠 request_id 过滤把它当干净拒绝（唯一 request_id 防跨 run 碰撞，
        # 但不能豁免同 bot 收到的其它下发）。
        bot = _AgentUiFakeBot([_req_event(1.0, "req_other")])
        with self.assertRaisesRegex(BotAssertionError, "不应出现任何"):
            expect_agent_ui_request(bot, "req_missing", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_fails_for_missing_request_id(self):
        # 漏掉 request_id 字段的 payload 仍是错误下发，必须失败并报告形状。
        raw = (
            b'{"target_player":"offline:Fake","xml":"<owo-ui/>","timeout_ticks":600}'
        )
        bot = _AgentUiFakeBot(
            [
                _FakeEvent(
                    1.0,
                    "payload",
                    {"channel": "bong:agent_ui_request", "data": raw},
                )
            ]
        )
        with self.assertRaisesRegex(BotAssertionError, "不应出现任何"):
            expect_agent_ui_request(bot, "req_expected", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_fails_for_corrupted_request_id(self):
        # request_id 被损坏（非字符串）的 payload 仍是错误下发，必须失败并报告形状。
        raw = (
            b'{"request_id":42,"target_player":"offline:Fake",'
            b'"xml":"<owo-ui/>","timeout_ticks":600}'
        )
        bot = _AgentUiFakeBot(
            [
                _FakeEvent(
                    1.0,
                    "payload",
                    {"channel": "bong:agent_ui_request", "data": raw},
                )
            ]
        )
        with self.assertRaisesRegex(BotAssertionError, "不应出现任何"):
            expect_agent_ui_request(bot, "req_expected", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_fails_for_non_object_json(self):
        # 合法非对象 JSON（数组）到达：既不能崩（对非 dict 调 .get 抛 AttributeError），
        # 也不能被 request_id 过滤当干净——它是错误下发的 payload，必须失败并报告形状。
        raw = b'["not","a","dict"]'
        bot = _AgentUiFakeBot(
            [
                _FakeEvent(
                    1.0,
                    "payload",
                    {"channel": "bong:agent_ui_request", "data": raw},
                )
            ]
        )
        with self.assertRaisesRegex(BotAssertionError, "而非 JSON 对象"):
            expect_agent_ui_request(bot, "req_expected", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_fails_for_invalid_json(self):
        raw = b"not json at all"
        bot = _AgentUiFakeBot(
            [
                _FakeEvent(
                    1.0,
                    "payload",
                    {"channel": "bong:agent_ui_request", "data": raw},
                )
            ]
        )
        with self.assertRaisesRegex(BotAssertionError, "不是合法 JSON"):
            expect_agent_ui_request(bot, "req_expected", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_positive_skips_non_dict_payloads(self):
        # 正向匹配器对非对象 JSON 不得崩（.get 抛 AttributeError）：跳过噪音，
        # 继续等真正匹配的 payload。
        bot = _AgentUiFakeBot(
            [
                _FakeEvent(
                    1.0,
                    "payload",
                    {"channel": "bong:agent_ui_request", "data": b'["noise"]'},
                ),
                _req_event(2.0, "req_real"),
            ]
        )
        payload = expect_agent_ui_request(bot, "req_real", timeout=5.0)
        self.assertEqual(payload["request_id"], "req_real")
        self.assertEqual(bot._now, 2.0, "正向匹配必须消费到真正的匹配事件")

    def test_expect_agent_ui_request_after_mark(self):
        # 同一 request_id 在 marker 前后各一：request-id 过滤无法区分二者，
        # 只有 after 语义能选中 marker 之后那条——忽略 after 的实现会错拿 marker 前的事件。
        bot = _AgentUiFakeBot(
            [
                _req_event(1.0, "req_same", timeout_ticks=100),
                _req_event(5.0, "req_same", timeout_ticks=900),
            ]
        )
        payload = expect_agent_ui_request(bot, "req_same", after=2.0, timeout=5.0)
        self.assertEqual(payload["request_id"], "req_same")
        self.assertEqual(payload["timeout_ticks"], 900, "必须取 marker 之后的同 request_id 事件")

    def test_expect_agent_ui_request_after_mark_ignores_predating_only_match(self):
        # 唯一匹配发生在 marker 之前：after 语义应拒绝它，返回 None。
        bot = _AgentUiFakeBot([_req_event(1.0, "req_predates")])
        self.assertIsNone(
            expect_agent_ui_request(bot, "req_predates", after=2.0, timeout=5.0, expect=False),
            "marker 之前唯一匹配应被 after 拒绝",
        )

    def test_expect_agent_ui_request_negative_fails_for_late_match_in_window(self):
        # 匹配事件在观测窗口内（t=3.0 ∈ (0, 5.0]）才送达：负向断言必须失败。
        # 若 fake 只做一次扫描、或 helper 立即返回 None / 零超时，这条会漏过——
        # 正是「迟到、安全敏感的下发在断言窗口内到达仍产生 false passing audit」的点。
        bot = _AgentUiFakeBot([_req_event(3.0, "req_late")])
        with self.assertRaises(BotAssertionError):
            expect_agent_ui_request(bot, "req_late", timeout=5.0, expect=False)

    def test_expect_agent_ui_request_negative_ok_when_match_after_window(self):
        # 匹配事件在窗口关闭后（t=6.0 > deadline 5.0）才送达：负向断言返回 None，
        # 证明观察窗口真的以 timeout 为界，而不是把任何时刻的事件都算作命中。
        bot = _AgentUiFakeBot([_req_event(6.0, "req_late")])
        self.assertIsNone(
            expect_agent_ui_request(bot, "req_late", timeout=5.0, expect=False),
            "窗口外事件不得使负向断言失败",
        )

    def test_expect_agent_ui_request_positive_rejects_match_after_window(self):
        # 匹配事件在窗口外到达：正向 wait 必须超时抛错，而不是错拿窗口外事件。
        # 钉死 timeout 参数确实约束了观察边界（finding：无断言记录传给 wait_for 的 timeout）。
        bot = _AgentUiFakeBot([_req_event(6.0, "req_late")])
        with self.assertRaises(BotAssertionError):
            expect_agent_ui_request(bot, "req_late", timeout=5.0)

    def test_expect_agent_ui_close_reason_assert(self):
        # 负向断言必须因 reason 失配而红。旧版只有一条事件：正向断言消费它之后，
        # 第二条断言只是"事件耗尽超时"，忽略 reason 的实现照样绿。改喂两条事件，
        # 第二条 reason 明确不符（非 null），让第二条断言真正在 reason 值上失配。
        close_ok = _FakeEvent(
            1.0,
            "payload",
            {
                "channel": "bong:agent_ui_close",
                "data": b'{"request_id":"req_1","reason":"invalid_button_id"}',
            },
        )
        close_wrong = _FakeEvent(
            2.0,
            "payload",
            {
                "channel": "bong:agent_ui_close",
                "data": b'{"request_id":"req_1","reason":"other_button_id"}',
            },
        )
        bot = _AgentUiFakeBot([close_ok, close_wrong])
        expect_agent_ui_close(bot, "req_1", reason="invalid_button_id", timeout=5.0)
        with self.assertRaises(BotAssertionError):
            expect_agent_ui_close(bot, "req_1", reason=None, timeout=5.0)

    def test_expect_agent_ui_close_reason_null(self):
        close = _FakeEvent(
            1.0,
            "payload",
            {
                "channel": "bong:agent_ui_close",
                "data": b'{"request_id":"req_1","reason":null}',
            },
        )
        bot = _AgentUiFakeBot([close])
        expect_agent_ui_close(bot, "req_1", reason=None, timeout=5.0)

    def test_expect_agent_ui_close_reason_absent_means_replaced(self):
        # 契约（schema AgentUiClosePayloadV1 的 reason=Type.Optional；shared wire
        # fixture agent-ui-close.channel-wire.sample.json 的 replaced case）：reason
        # 缺省 = Replaced，期望 None 时必须放行。期望具体 reason 时必须失败，且值
        # 分支真正检查到 payload（_now 前移到 close_2.t=2.0），而非事件耗尽超时凑绿。
        close_1 = _FakeEvent(
            1.0,
            "payload",
            {
                "channel": "bong:agent_ui_close",
                "data": b'{"request_id":"req_1"}',
            },
        )
        close_2 = _FakeEvent(
            2.0,
            "payload",
            {
                "channel": "bong:agent_ui_close",
                "data": b'{"request_id":"req_2"}',
            },
        )
        bot = _AgentUiFakeBot([close_1, close_2])
        expect_agent_ui_close(bot, "req_1", reason=None, timeout=5.0)
        with self.assertRaises(BotAssertionError):
            expect_agent_ui_close(bot, "req_2", reason="invalid_button_id", timeout=5.0)
        self.assertEqual(bot._now, close_2.t, "第二条断言必须检查到 close_2 事件")

class ProbePayloadDecodeTest(unittest.TestCase):
    """实体/空间探知流 S2C 解码 pin（field 74/77/129/130/132）。

    这些 payload_type 此前不在 proto_min 白名单里，bot 即使收到包也解不出
    `server_data` 事件，`expect_server_data("event_alert" / "qi_color_observed" /
    "mineral_probe_result" / "freshness_update" / "workbench_open")` 会静默超时。
    本类锁定字段号与字段映射，防 wire/label 失配。
    """

    def test_workbench_open_field_132_decodes_position_zigzag(self):
        inner = (
            _pb_varint_field(1, 999)
            + _pb_sint32_field(2, -3)
            + _pb_sint32_field(3, 71)
            + _pb_sint32_field(4, -8)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(132, inner))
        self.assertEqual(decoded["type"], "workbench_open")
        self.assertEqual(decoded["entity_id"], 999)
        self.assertEqual(decoded["position"], [-3, 71, -8], "sint32 负坐标应 zigzag 还原")

    def test_burst_meridian_event_field_70_decodes_optional_target_and_doubles(self):
        inner = (
            _pb_string(1, "burst_meridian.beng_quan")
            + _pb_string(2, "offline:caster")
            + _pb_string(3, "offline:target")
            + _pb_varint_field(4, 12345)
            + _pb_fixed64(5, 1.5)
            + _pb_fixed64(6, 0.75)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(70, inner))
        self.assertEqual(decoded["type"], "burst_meridian_event")
        self.assertEqual(decoded["skill"], "burst_meridian.beng_quan")
        self.assertEqual(decoded["caster"], "offline:caster")
        self.assertEqual(decoded["target"], "offline:target")
        self.assertEqual(decoded["tick"], 12345)
        self.assertAlmostEqual(decoded["overload_ratio"], 1.5)
        self.assertAlmostEqual(decoded["integrity_snapshot"], 0.75)

    def test_burst_meridian_event_field_70_omits_absent_target(self):
        inner = _pb_string(1, "burst_meridian.beng_quan") + _pb_string(2, "offline:caster")
        decoded = proto_min.decode_server_data_envelope(_pb_message(70, inner))
        self.assertNotIn("target", decoded)

    def test_combat_hud_state_field_9_decodes_percentages_and_derived_flags(self):
        inner = (
            _pb_fixed32(1, 0.75)
            + _pb_fixed32(2, 0.5)
            + _pb_fixed32(3, 0.25)
            + _pb_message(4, _pb_varint_field(3, 1))
            + _pb_varint_field(5, 1)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(9, inner))
        self.assertEqual(decoded["type"], "combat_hud_state")
        self.assertAlmostEqual(decoded["hp_percent"], 0.75)
        self.assertAlmostEqual(decoded["qi_percent"], 0.5)
        self.assertAlmostEqual(decoded["stamina_percent"], 0.25)
        self.assertTrue(decoded["combat_active"])
        self.assertEqual(
            decoded["derived"],
            {"flying": False, "phasing": False, "tribulation_locked": True},
        )

    def test_qi_color_observed_field_74_decodes_all_color_kinds(self):
        # central-review 2029 #4：只 pin (field3=3→Mellow) 会让「其余 9 个 ColorKind
        # 映射错误」的实现也通过——穷举全部 10 个枚举成员逐一 pin 名字映射。
        kinds = {
            1: "Sharp",
            2: "Heavy",
            3: "Mellow",
            4: "Solid",
            5: "Light",
            6: "Intricate",
            7: "Gentle",
            8: "Insidious",
            9: "Violent",
            10: "Turbid",
        }
        for raw, expected in kinds.items():
            inner = (
                _pb_string(1, "offline:BGD9QiH")
                + _pb_string(2, "offline:BGD9QiV")
                + _pb_varint_field(3, raw)
                + _pb_varint_field(5, 0)
                + _pb_varint_field(6, 0)
                + _pb_varint_field(7, 2)
            )
            decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
            self.assertEqual(decoded["type"], "qi_color_observed")
            self.assertEqual(
                decoded["main"],
                expected,
                f"field3={raw} 应解码为 {expected}",
            )
            self.assertNotIn(
                "secondary",
                decoded,
                "未携带 secondary（field4 缺省）时必须省略键，而非显式 None（presence 契约）",
            )
            self.assertEqual(decoded["is_chaotic"], False, "field5=0 应解码为 is_chaotic=false")
            self.assertEqual(decoded["is_hunyuan"], False, "field6=0 应解码为 is_hunyuan=false")
            self.assertEqual(decoded["realm_diff"], 2)

    def test_qi_color_observed_field_74_decodes_unspecified_default(self):
        # central-review 2029 #1：ColorKind 是 protobuf enum，field3 合法地可以缺省
        # （默认值 0）或显式编码为 0。`_varint(fields, 3)` 缺省时返回 0，
        # ColorKind=0 是合法默认值，映射为 "unspecified"；穷举 1..=10
        # 只 pin 非默认 gameplay 色，会放走「0 也映射成某种玩法色 / 大小写错误 /
        # 直接 raise」的坏解码器——合法 default-valued 载荷会产出错误的
        # qi_color_observed.main。两个子形都要 pin：显式 0 与字段缺省。
        for label, field3_bytes in (
            ("field3=0（protobuf enum 默认值）", _pb_varint_field(3, 0)),
            ("field3 缺省", b""),
        ):
            inner = (
                _pb_string(1, "offline:BGD9QiH")
                + _pb_string(2, "offline:BGD9QiV")
                + field3_bytes
                + _pb_varint_field(5, 0)
                + _pb_varint_field(6, 0)
                + _pb_varint_field(7, 2)
            )
            decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
            self.assertEqual(decoded["type"], "qi_color_observed")
            self.assertEqual(
                decoded["main"],
                "unspecified",
                f"{label} 应解码为 main=unspecified（默认/兜底映射），实际 {decoded['main']}",
            )
            self.assertNotIn(
                "secondary",
                decoded,
                "未携带 secondary（field4 缺省）时必须省略键，而非显式 None（presence 契约）",
            )
            self.assertEqual(decoded["is_chaotic"], False, "field5=0 应解码为 is_chaotic=false")
            self.assertEqual(decoded["is_hunyuan"], False, "field6=0 应解码为 is_hunyuan=false")
            self.assertEqual(decoded["realm_diff"], 2)

    def test_qi_color_observed_field_74_decodes_unknown_enum_values(self):
        # central-review 2029 #2：ColorKind 是 protobuf enum，未来/未知的非零 wire 值
        # （如 11）是合法载荷。只 pin 1..=10 与 0/缺省，会放走「特殊处理 0 但对非零
        # 未知值用 COLOR_KIND_PASCAL_NAMES[raw] 直取/raise」的坏解码器——它 decode 到
        # 未知枚举值时崩溃，qi_color_observed 事件根本送不到。main/secondary 两个槽位
        # 的未知值身份都要 pin；secondary 携带未知值应映射 "unknown_N" 而非 None
        # （None 只留给字段缺省，proto_min.py 同款 presence 约定）。
        inner = (
            _pb_string(1, "offline:BGD9QiH")
            + _pb_string(2, "offline:BGD9QiV")
            + _pb_varint_field(3, 11)  # 未知 main
            + _pb_varint_field(5, 0)
            + _pb_varint_field(6, 0)
            + _pb_varint_field(7, 2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
        self.assertEqual(decoded["type"], "qi_color_observed")
        self.assertEqual(
            decoded["main"],
            "unknown_11",
            "未知 main ColorKind 应保留 unknown_N 身份，实际值不可伪装成默认 unspecified",
        )
        self.assertNotIn(
            "secondary",
            decoded,
            "未携带 secondary（field4 缺省）时必须省略键，而非显式 None（presence 契约）",
        )

        inner = (
            _pb_string(1, "offline:BGD9QiH")
            + _pb_string(2, "offline:BGD9QiV")
            + _pb_varint_field(3, 3)  # main=MELLOW
            + _pb_varint_field(4, 11)  # 未知 secondary
            + _pb_varint_field(5, 0)
            + _pb_varint_field(6, 0)
            + _pb_varint_field(7, 2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
        self.assertEqual(decoded["type"], "qi_color_observed")
        self.assertEqual(decoded["main"], "Mellow")
        self.assertEqual(
            decoded["secondary"],
            "unknown_11",
            "未知 secondary ColorKind 应保留 unknown_N 身份（None 只留给缺省）",
        )

    def test_qi_color_observed_field_74_wrong_wire_optional_secondary_is_absent(self):
        inner = (
            _pb_string(1, "offline:BGD9QiH")
            + _pb_string(2, "offline:BGD9QiV")
            + _pb_varint_field(3, 3)
            + _pb_string(4, "wrong-wire-secondary")
            + _pb_varint_field(5, 0)
            + _pb_varint_field(6, 0)
            + _pb_varint_field(7, 2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
        self.assertEqual(decoded["main"], "Mellow")
        self.assertNotIn(
            "secondary",
            decoded,
            "wrong-wire optional secondary must be ignored, not fabricated as unspecified",
        )

    def test_qi_color_observed_field_74_decodes_present_secondary(self):
        # central-review 2029 #4：secondary 是可选字段，缺省路径之上还须 pin 携带
        # 路径——恒返回 None 的错误实现（present 也丢）会在此撞红。
        inner = (
            _pb_string(1, "offline:BGD9QiH")
            + _pb_string(2, "offline:BGD9QiV")
            + _pb_varint_field(3, 3)  # main=MELLOW
            + _pb_varint_field(4, 9)  # secondary=VIOLENT
            + _pb_varint_field(5, 0)
            + _pb_varint_field(6, 0)
            + _pb_varint_field(7, 2)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
        self.assertEqual(decoded["type"], "qi_color_observed")
        self.assertEqual(decoded["main"], "Mellow")
        self.assertEqual(
            decoded["secondary"],
            "Violent",
            "携带 secondary(field4=9) 时应解码为 Violent",
        )
        self.assertEqual(decoded["is_chaotic"], False, "field5=0 应解码为 is_chaotic=false")
        self.assertEqual(decoded["is_hunyuan"], False, "field6=0 应解码为 is_hunyuan=false")
        self.assertEqual(decoded["realm_diff"], 2)

    def test_qi_color_observed_field_74_decodes_boolean_flags(self):
        # central-review 2029 #4：此前 fixtures 一律 field5/field6=0 且不断言，
        # 恒硬编码两字段为 false / 掉字段 / 交换编号的实现全都会通过。逐档 pin：
        # field5=1→is_chaotic=true、field6=1→is_hunyuan=true；单真 + 双真全解形，
        # 并断言完整解码形状（main/secondary/realm_diff/两 canonical id）。
        cases = [
            (True, False),
            (False, True),
            (True, True),
        ]
        for exp_chaotic, exp_hunyuan in cases:
            inner = (
                _pb_string(1, "offline:BGD9QiH")
                + _pb_string(2, "offline:BGD9QiV")
                + _pb_varint_field(3, 3)  # main=MELLOW
                + _pb_varint_field(4, 9)  # secondary=VIOLENT
                + _pb_varint_field(5, 1 if exp_chaotic else 0)
                + _pb_varint_field(6, 1 if exp_hunyuan else 0)
                + _pb_varint_field(7, 2)
            )
            decoded = proto_min.decode_server_data_envelope(_pb_message(74, inner))
            self.assertEqual(decoded["type"], "qi_color_observed")
            self.assertEqual(decoded["main"], "Mellow")
            self.assertEqual(decoded["secondary"], "Violent")
            self.assertEqual(
                decoded["is_chaotic"],
                exp_chaotic,
                "field5 布尔值应解码为 is_chaotic（map 到字段号 5）",
            )
            self.assertEqual(
                decoded["is_hunyuan"],
                exp_hunyuan,
                "field6 布尔值应解码为 is_hunyuan（map 到字段号 6）",
            )
            self.assertEqual(decoded["realm_diff"], 2)
            self.assertEqual(decoded["observer"], "offline:BGD9QiH")
            self.assertEqual(decoded["observed"], "offline:BGD9QiV")

    def test_event_alert_field_77_decodes_message(self):
        inner = (
            _pb_varint_field(1, 0)
            + _pb_string(2, "神识未及，凝脉方可感知保鲜")
            + _pb_varint_field(4, 70)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(77, inner))
        self.assertEqual(decoded["type"], "event_alert")
        self.assertEqual(decoded["event"], "unspecified")
        self.assertIn("神识未及", decoded["message"])
        self.assertIsNone(decoded["zone"], "未携带 zone 时保持 None")
        self.assertEqual(decoded["duration_ticks"], 70)

    def test_event_alert_field_77_absent_duration_stays_none(self):
        # central-review 2029 #7：duration_ticks 是可选字段（_optional_varint 缺省
        # 返回 None）。现有 fixtures 都携带 field4，用普通 defaulting varint 访问器
        # 的坏解码器（缺省返回 0）能通过全部旧断言——合法无 duration 的告警会被
        # 解码成 duration_ticks=0 的错误形状。缺省分支必须 pin：absent→None；另 pin
        # 携带但值为 0 → 0（与缺省可区分，ordinary defaulting 访问器两者都出 0）。
        absent = proto_min.decode_server_data_envelope(
            _pb_message(77, _pb_string(2, "神识未及，凝脉方可感知保鲜"))
        )
        self.assertEqual(absent["type"], "event_alert")
        self.assertIn("神识未及", absent["message"])
        self.assertIsNone(absent["duration_ticks"], "缺省 duration_ticks 必须为 None")

        present_zero = proto_min.decode_server_data_envelope(
            _pb_message(
                77,
                _pb_string(2, "神识未及，凝脉方可感知保鲜") + _pb_varint_field(4, 0),
            )
        )
        self.assertEqual(present_zero["type"], "event_alert")
        self.assertEqual(
            present_zero["duration_ticks"],
            0,
            "携带 field4=0 时应解码为 duration_ticks=0（区别于缺省 None）",
        )

    def test_event_alert_field_77_unknown_event_kind_preserves_identity(self):
        decoded = proto_min.decode_server_data_envelope(
            _pb_message(77, _pb_varint_field(1, 99))
        )
        self.assertEqual(
            decoded["event"],
            "unknown_99",
            "未知 EventKind 必须保留诊断身份，不能伪装成 unspecified",
        )

    def test_event_alert_field_77_wrong_wire_optional_fields_are_absent(self):
        # field 1/4 are varint and field 3 is length-delimited; malformed wire
        # values must not fabricate presence or default values.
        inner = (
            _pb_string(2, "告警")
            + _pb_string(1, "wrong-wire-event")
            + _pb_varint_field(3, 7)
            + _pb_string(4, "wrong-wire-duration")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(77, inner))
        self.assertEqual(decoded["event"], "unspecified")
        self.assertIsNone(decoded["zone"])
        self.assertIsNone(decoded["duration_ticks"])

    def test_mineral_probe_result_field_129_wrong_wire_optional_fields_are_absent(self):
        inner = (
            _pb_string(1, "denied")
            + _pb_varint_field(2, 7)
            + _pb_string(3, "wrong-wire-count")
            + _pb_varint_field(4, 8)
            + _pb_varint_field(5, 9)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(129, inner))
        self.assertIsNone(decoded["mineral_id"])
        self.assertIsNone(decoded["remaining_units"])
        self.assertIsNone(decoded["display_name_zh"])
        self.assertIsNone(decoded["denial_reason"])

    def test_event_alert_field_77_decodes_event_kind_and_zone(self):
        inner = (
            _pb_varint_field(1, 1)  # EVENT_KIND_THUNDER_TRIBULATION
            + _pb_string(2, "雷劫将至")
            + _pb_string(3, "jiuzong_taichu_ruin")
            + _pb_varint_field(4, 120)
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(77, inner))
        self.assertEqual(decoded["type"], "event_alert")
        self.assertEqual(
            decoded["event"],
            "thunder_tribulation",
            "event kind 应映射为 server/agent 契约的 snake_case canonical name",
        )
        self.assertIn("雷劫将至", decoded["message"])
        self.assertEqual(decoded["zone"], "jiuzong_taichu_ruin", "present-zone 应还原 field 3")
        self.assertEqual(decoded["duration_ticks"], 120)

    def test_mineral_probe_result_field_129_decodes_denial_reason(self):
        inner = _pb_string(1, "denied") + _pb_string(5, "not_mineral_ore")
        decoded = proto_min.decode_server_data_envelope(_pb_message(129, inner))
        self.assertEqual(decoded["type"], "mineral_probe_result")
        self.assertEqual(decoded["kind"], "denied")
        self.assertEqual(decoded["denial_reason"], "not_mineral_ore")
        self.assertIsNone(decoded["mineral_id"])
        self.assertIsNone(decoded["remaining_units"])
        self.assertIsNone(decoded["display_name_zh"])

    def test_mineral_probe_result_field_129_decodes_found_variant(self):
        inner = (
            _pb_string(1, "found")
            + _pb_string(2, "mineral.ore.lingshi")
            + _pb_varint_field(3, 42)
            + _pb_string(4, "灵石矿")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(129, inner))
        self.assertEqual(decoded["type"], "mineral_probe_result")
        self.assertEqual(decoded["kind"], "found")
        self.assertEqual(decoded["mineral_id"], "mineral.ore.lingshi")
        self.assertEqual(decoded["remaining_units"], 42)
        self.assertEqual(decoded["display_name_zh"], "灵石矿")
        self.assertIsNone(decoded["denial_reason"])

    def test_freshness_update_field_130_decodes_float_profile(self):
        inner = (
            _pb_string(1, "59")
            + _pb_float32_field(2, 0.75)
            + _pb_string(3, "food_spoil_mundane_meat_v1")
        )
        decoded = proto_min.decode_server_data_envelope(_pb_message(130, inner))
        self.assertEqual(decoded["type"], "freshness_update")
        self.assertEqual(decoded["item_uuid"], "59")
        self.assertAlmostEqual(decoded["freshness"], 0.75, places=4)
        self.assertEqual(decoded["profile_name"], "food_spoil_mundane_meat_v1")
if __name__ == "__main__":
    unittest.main(verbosity=1)
