"""修炼突破 intent 链路 —— dev 铺垫后断言 production protobuf cinematic。

权威契约：
- C2S：`{"type":"breakthrough_request","v":1}`
- S2C：`bong:server_data` envelope oneof field 71 `BreakthroughCinematic`

VFX 与普通 chat 只算伴随反馈，不能冒充突破结果。
"""

import math

from bot.bot import BotAssertionError

from ._combat_helpers import last_event_time

DESCRIPTION = "breakthrough_request 产生完整 cinematic 阶段链，并与权威 player_state realm 一致"
MODULES = ["cultivation", "network"]

BREAKTHROUGH_REQUEST = {"type": "breakthrough_request", "v": 1}
PHASES = ("prelude", "charge", "catalyze", "apex", "aftermath")
TERMINAL_TIMEOUT_SECONDS = 35.0


def _cinematic_after(event, sent_at: float) -> bool:
    return (
        event.kind == "server_data"
        and event.t > sent_at
        and event.data.get("payload_type") == "breakthrough_cinematic"
    )


def _assert_initial_cinematic(bot, payload: dict) -> None:
    expected = {
        "phase": "prelude",
        "phase_tick": 0,
        "realm_from": "Awaken",
        "realm_to": "Induce",
        "interrupted": False,
    }
    actual = {key: payload.get(key) for key in expected}
    if actual != expected:
        raise BotAssertionError(
            f"[{bot.username}] 突破首帧字段漂移：期望 {expected}，实际 {actual}；"
            "检查 BreakthroughCinematicS2cV1 → envelope field 71"
        )

    if payload.get("result") not in {"success", "failure"}:
        raise BotAssertionError(
            f"[{bot.username}] 突破结果必须是 success/failure，实际 {payload.get('result')!r}"
        )
    if not payload.get("actor_id"):
        raise BotAssertionError(f"[{bot.username}] breakthrough_cinematic.actor_id 不得为空")
    if payload.get("phase_duration_ticks", 0) <= 0:
        raise BotAssertionError(
            f"[{bot.username}] phase_duration_ticks 必须 >0，实际 "
            f"{payload.get('phase_duration_ticks')!r}"
        )

    world_pos = payload.get("world_pos")
    if (
        not isinstance(world_pos, list)
        or len(world_pos) != 3
        or not all(isinstance(value, (int, float)) and math.isfinite(value) for value in world_pos)
    ):
        raise BotAssertionError(
            f"[{bot.username}] breakthrough_cinematic.world_pos 必须是有限三坐标，实际 {world_pos!r}"
        )
    if payload.get("visible_radius_blocks", 0.0) <= 0.0:
        raise BotAssertionError(
            f"[{bot.username}] visible_radius_blocks 必须 >0，实际 "
            f"{payload.get('visible_radius_blocks')!r}"
        )
    for field in ("particle_density", "intensity"):
        value = payload.get(field)
        if not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0.0:
            raise BotAssertionError(
                f"[{bot.username}] {field} 必须是有限正数，实际 {value!r}"
            )
    if not payload.get("style") or not payload.get("season_overlay"):
        raise BotAssertionError(
            f"[{bot.username}] cinematic 视觉身份不得为空，实际 "
            f"style={payload.get('style')!r} season_overlay={payload.get('season_overlay')!r}"
        )


def _is_matching_phase(event, after: float, identity: dict, phase: str) -> bool:
    if not _cinematic_after(event, after):
        return False
    payload = event.data.get("payload")
    return (
        isinstance(payload, dict)
        and payload.get("actor_id") == identity["actor_id"]
        and payload.get("realm_from") == identity["realm_from"]
        and payload.get("realm_to") == identity["realm_to"]
        and payload.get("result") == identity["result"]
        and payload.get("interrupted") is identity["interrupted"]
        and payload.get("phase") == phase
        and payload.get("phase_tick") == 0
    )


def _wait_cinematic_terminal(bot, initial_event):
    initial = initial_event.data["payload"]
    identity = {
        "actor_id": initial["actor_id"],
        "realm_from": initial["realm_from"],
        "realm_to": initial["realm_to"],
        "result": initial["result"],
        "interrupted": initial["interrupted"],
    }
    previous = initial_event
    phases = [initial]
    for phase in PHASES[1:]:
        current = bot.wait_for(
            lambda observed, expected=phase, after=previous.t: _is_matching_phase(
                observed, after, identity, expected
            ),
            timeout=TERMINAL_TIMEOUT_SECONDS,
            description=(
                "同一 breakthrough_cinematic 身份必须按序推进至 "
                f"{phase}/phase_tick=0"
            ),
        )
        payload = current.data["payload"]
        if payload.get("at_tick", 0) <= phases[-1].get("at_tick", 0):
            raise BotAssertionError(
                f"[{bot.username}] cinematic.at_tick 必须严格递增："
                f"上一阶段={phases[-1].get('at_tick')!r}，"
                f"{phase}={payload.get('at_tick')!r}"
            )
        phases.append(payload)
        previous = current
    return phases[-1]


def _wait_authoritative_realm(bot, sent_at: float, realm: str):
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > sent_at
        and event.data.get("payload_type") == "player_state"
        and event.data.get("payload", {}).get("realm") == realm,
        timeout=15.0,
        description=f"breakthrough_request 后新 player_state 须携带权威 realm={realm}",
    )


def run(env) -> None:
    with env.new_bot("Break") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        bot.cmd("realm set awaken")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.expect_chat("Awaken", timeout=10.0)

        bot.cmd("meridian open_all")
        bot.expect_chat("open_all does not auto-breakthrough", timeout=10.0)

        bot.cmd("qi set 20")
        bot.expect_chat("[dev] qi set", timeout=10.0)

        bot.cmd("zone_qi set spawn 1.00")
        bot.expect_chat("[dev] zone_qi `spawn`", timeout=10.0)

        sent_at = last_event_time(bot)
        bot.intent(BREAKTHROUGH_REQUEST)
        event = bot.wait_for(
            lambda observed: _cinematic_after(observed, sent_at),
            timeout=15.0,
            description=(
                "breakthrough_request 后 production bong:server_data/"
                "breakthrough_cinematic（envelope oneof field 71）"
            ),
        )
        _assert_initial_cinematic(bot, event.data["payload"])
        terminal = _wait_cinematic_terminal(bot, event)
        if terminal.get("result") != "success" or terminal.get("interrupted") is not False:
            raise BotAssertionError(
                f"[{bot.username}] 固定铺垫的醒灵→引气突破应成功完成，实际 "
                f"result={terminal.get('result')!r} interrupted={terminal.get('interrupted')!r}"
            )
        _wait_authoritative_realm(bot, sent_at, "Induce")
        bot.assert_alive("breakthrough_request 完整 cinematic 与权威 realm 链路执行后")
