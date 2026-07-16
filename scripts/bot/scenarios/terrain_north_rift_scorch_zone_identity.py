"""北荒渊口迁移后的生产坐标黑盒回归。

本场景必须由专用 preview server phase 执行：server 以
``BONG_PREVIEW_MODE=1`` 启动，runner 同时显式设置
``BOT_E2E_NORTH_RIFT_PREVIEW=1``。常规 bot ``--all`` 会由 runner 明确标为
SKIP，避免 preview 模式的 ViewDistance(32) 扩散到整套场景。

每个探针都走真实 MC 命令包发送 ``/preview_tp``，并要求同时观察到：

1. server authoritative ``PlayerPositionLook`` 回包精确落在目标坐标；
2. 命令前 watermark 之后的 protobuf ``zone_info`` 命中生产 zone；
3. 同一 watermark 之后的 ``bong:audio/ambient_zone`` 也报告相同 zone/坐标。

探针顺序刻意为 scorch -> rift -> scorch boundary，确保三次都是真实 zone
transition；若把两个 scorch 点相邻执行，第二点不会产生新的 zone_info，测试会
退化。
"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from typing import Any

from bot.bot import BotAssertionError

DESCRIPTION = (
    "preview_tp 权威对拍北荒渊口、旧焦土点与 inclusive 边界的 zone_info/ambient_zone"
)
MODULES = ["terrain", "world", "network", "audio"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_NORTH_RIFT_PREVIEW"

POSITION_TOLERANCE = 1e-4
SPIRIT_QI_TOLERANCE = 1e-3
TELEPORT_TIMEOUT = 20.0
ZONE_INFO_TIMEOUT = 15.0
AMBIENT_TIMEOUT = 15.0
AMBIENT_CHANNEL = "bong:audio/ambient_zone"


@dataclass(frozen=True)
class ZoneProbe:
    label: str
    pos: tuple[float, float, float]
    yaw: float
    pitch: float
    zone: str
    spirit_qi: float
    danger_level: int
    active_events: tuple[str, ...]
    perception_text: str | None


PROBES = (
    ZoneProbe(
        label="legacy rift entrance remains scorch",
        pos=(2000.0, 74.0, -7800.0),
        yaw=18.0,
        pitch=-7.0,
        zone="north_waste_east_scorch",
        spirit_qi=0.290146,
        danger_level=7,
        active_events=("tribulation_scorch", "tianjie_ascension_pit"),
        perception_text="灵气稀薄，引气如吸沙",
    ),
    ZoneProbe(
        # 精确 portal anchor z=-7300 位于 PORTAL_INTERACT_RADIUS=2 内，真实玩家
        # 可能同 tick 转入 TSY。z=-7303 仍在 production rift AABB 内且距 anchor
        # 3 格，适合稳定观察 overworld zone_info/ambient_zone；anchor 精确点由 Rust
        # integration pin 负责。
        label="relocated north rift identity sample outside portal radius",
        pos=(2000.0, 74.0, -7303.0),
        yaw=-36.0,
        pitch=11.0,
        zone="rift_mouth_north_002",
        spirit_qi=0.068602,
        danger_level=5,
        active_events=("rift_mouth_entry",),
        perception_text="灵气几近断绝，此地有不祥预感",
    ),
    ZoneProbe(
        label="inclusive scorch north boundary",
        pos=(2000.0, 74.0, -7500.0),
        yaw=91.0,
        pitch=3.0,
        zone="north_waste_east_scorch",
        spirit_qi=0.290146,
        danger_level=7,
        active_events=("tribulation_scorch", "tianjie_ascension_pit"),
        perception_text="此地灵气骤然浓郁，呼吸间元气盈满",
    ),
)


def _position_matches(event, probe: ZoneProbe, watermark: float) -> bool:
    if event.kind != "pos_look" or event.t <= watermark:
        return False
    expected = (*probe.pos, probe.yaw, probe.pitch)
    actual = (
        event.data.get("x"),
        event.data.get("y"),
        event.data.get("z"),
        event.data.get("yaw"),
        event.data.get("pitch"),
    )
    return all(
        isinstance(value, (int, float))
        and abs(float(value) - expected_value) <= POSITION_TOLERANCE
        for value, expected_value in zip(actual, expected, strict=True)
    )


def _zone_info_payload(event, probe: ZoneProbe, watermark: float) -> dict[str, Any] | None:
    if event.kind != "server_data" or event.t <= watermark:
        return None
    if event.data.get("payload_type") != "zone_info":
        return None
    payload = event.data.get("payload")
    if not isinstance(payload, dict) or payload.get("zone") != probe.zone:
        return None
    return payload


def _ambient_payload(event, probe: ZoneProbe, watermark: float) -> dict[str, Any] | None:
    if (
        event.kind != "payload"
        or event.t <= watermark
        or event.data.get("channel") != AMBIENT_CHANNEL
    ):
        return None
    try:
        payload = json.loads(bytes(event.data["data"]).decode("utf-8"))
    except (KeyError, TypeError, UnicodeDecodeError, json.JSONDecodeError):
        return None
    if not isinstance(payload, dict) or payload.get("zone_name") != probe.zone:
        return None
    return payload


def _assert_zone_info(bot, probe: ZoneProbe, payload: dict[str, Any]) -> None:
    actual_qi = payload.get("spirit_qi")
    if not isinstance(actual_qi, (int, float)) or (
        abs(actual_qi - probe.spirit_qi) > SPIRIT_QI_TOLERANCE
    ):
        raise BotAssertionError(
            f"[{bot.username}] {probe.label}: 期望 {probe.zone} 的 production spirit_qi "
            f"接近 {probe.spirit_qi}（允许玩家入场后微量守恒吸纳 "
            f"±{SPIRIT_QI_TOLERANCE}），"
            f"实际 payload={payload!r}"
        )
    expected = {
        "v": 1,
        "type": "zone_info",
        "zone": probe.zone,
        "danger_level": probe.danger_level,
        "status": "Normal",
        "active_events": list(probe.active_events),
        "perception_text": probe.perception_text,
    }
    mismatches = {
        key: {"expected": value, "actual": payload.get(key)}
        for key, value in expected.items()
        if payload.get(key) != value
    }
    if mismatches:
        raise BotAssertionError(
            f"[{bot.username}] {probe.label}: zone_info 必须完整保留生产 zone 契约，"
            f"字段不符={mismatches!r}, payload={payload!r}"
        )


def _assert_ambient(bot, probe: ZoneProbe, payload: dict[str, Any]) -> None:
    expected_pos = [int(value) for value in probe.pos]
    mismatches = {}
    if payload.get("pos") != expected_pos:
        mismatches["pos"] = {"expected": expected_pos, "actual": payload.get("pos")}
    if payload.get("ambient_recipe_id") != "ambient_wilderness":
        mismatches["ambient_recipe_id"] = {
            "expected": "ambient_wilderness",
            "actual": payload.get("ambient_recipe_id"),
        }
    if mismatches:
        raise BotAssertionError(
            f"[{bot.username}] {probe.label}: ambient_zone 应与 authoritative "
            "坐标/zone 同步，"
            f"字段不符={mismatches!r}, payload={payload!r}"
        )


def run(env) -> None:
    if os.environ.get(REQUIRED_ENV) != "1":
        raise BotAssertionError(
            f"场景只能由专用 preview phase 执行：需同时设置 {REQUIRED_ENV}=1，"
            "并以 BONG_PREVIEW_MODE=1 启动 server；常规 --all 应在 runner 层 SKIP"
        )

    with env.new_bot("NRift") as bot:
        bot.expect_event("game_join", timeout=15.0)
        initial_pos = bot.expect_event("pos_look", timeout=15.0)

        previous = initial_pos
        for probe in PROBES:
            watermark = previous.t
            x, y, z = probe.pos
            bot.cmd(f"preview_tp {x} {y} {z} {probe.yaw} {probe.pitch}")

            pos_event = bot.wait_for(
                lambda event: _position_matches(event, probe, watermark),
                TELEPORT_TIMEOUT,
                f"{probe.label} 的 authoritative pos_look={probe.pos} yaw={probe.yaw} "
                f"pitch={probe.pitch}；若只有 queued chat 而无回包，检查 preview "
                "server 是否"
                "以 BONG_PREVIEW_MODE=1 启动",
            )
            zone_event = bot.wait_for(
                lambda event: _zone_info_payload(event, probe, watermark) is not None,
                ZONE_INFO_TIMEOUT,
                f"{probe.label} 在命令 watermark={watermark:.3f}s 后的 zone_info/{probe.zone}",
            )
            zone_payload = _zone_info_payload(zone_event, probe, watermark)
            assert zone_payload is not None
            _assert_zone_info(bot, probe, zone_payload)

            ambient_event = bot.wait_for(
                lambda event: _ambient_payload(event, probe, watermark) is not None,
                AMBIENT_TIMEOUT,
                f"{probe.label} 在命令 watermark={watermark:.3f}s 后的 "
                f"{AMBIENT_CHANNEL}/{probe.zone}",
            )
            ambient_payload = _ambient_payload(ambient_event, probe, watermark)
            assert ambient_payload is not None
            _assert_ambient(bot, probe, ambient_payload)
            previous = max((pos_event, zone_event, ambient_event), key=lambda event: event.t)

        bot.assert_alive("北荒渊口/焦土三点 authoritative preview_tp 对拍完成后")
