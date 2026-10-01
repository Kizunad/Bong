"""P4 生产系统：野生灵草采集进度的黑盒链路。

黑盒契约面：
- fallback/raster 地表由服务器权威读取；场景只接受
  `botany_harvest_progress` 的非空 target_id + 有限 target_pos。
- `/bong gather spirit_grass` 必须绑定 dev fixture 创建的真实 ECS Plant；场景把同一
  session 推进到 `completed=true/interrupted=false`，最后对拍权威 inventory `spirit_grass +1`。
"""

import math

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import (
    find_item,
    wait_inventory_revision_after_matching,
    wait_join_and_inventory,
)

DESCRIPTION = "采集：真实 Plant → botany 进度与 gathering 终态 → 灵草入包"
MODULES = ["gathering", "inventory"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_AMBIENT_FIXTURE_OWNED"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

HERB_ID = "spirit_grass"
HARVEST_RADIUS_SQ = 6.0 * 6.0
HARVEST_TICKS = 40
MIN_GATE_TPS = 2.0
HARVEST_TIMEOUT_MARGIN_SECONDS = 10.0
HARVEST_TERMINAL_TIMEOUT_SECONDS = (
    HARVEST_TICKS / MIN_GATE_TPS + HARVEST_TIMEOUT_MARGIN_SECONDS
)
BOTANY_FIXTURE_PREFIX = "[dev] botany_spawn accepted: plant_id="


def _parse_botany_fixture(text: str) -> tuple[str, list[float], str]:
    prefix = BOTANY_FIXTURE_PREFIX
    if not text.startswith(prefix):
        raise BotAssertionError(f"botany fixture 确认前缀漂移，实际 {text!r}")
    try:
        plant_id, suffix = text[len(prefix) :].split(" kind=spirit_grass pos=[", 1)
        raw_pos, zone = suffix.split("] zone=", 1)
        position = [float(value) for value in raw_pos.split(",")]
    except (ValueError, TypeError) as error:
        raise BotAssertionError(f"botany fixture 确认字段无法解析，实际 {text!r}") from error
    if (
        not plant_id.startswith("plant-")
        or not plant_id.removeprefix("plant-").isdigit()
        or len(position) != 3
        or not all(math.isfinite(value) for value in position)
        or not zone
    ):
        raise BotAssertionError(f"botany fixture 身份/坐标/zone 无效，实际 {text!r}")
    return plant_id, position, zone


def _same_position(actual: object, expected: list[float]) -> bool:
    return (
        isinstance(actual, list)
        and len(actual) == 3
        and all(
            isinstance(value, (int, float))
            and math.isfinite(value)
            and math.isclose(float(value), float(want), rel_tol=0.0, abs_tol=1e-9)
            for value, want in zip(actual, expected)
        )
    )


def _valid_target_pos(payload: dict, player_position: tuple[float, float, float]) -> bool:
    target_pos = payload.get("target_pos")
    if (
        not isinstance(target_pos, list)
        or len(target_pos) != 3
        or not all(isinstance(value, (int, float)) and math.isfinite(value) for value in target_pos)
    ):
        return False
    return (
        sum(
            (float(actual) - float(expected)) ** 2
            for actual, expected in zip(target_pos, player_position)
        )
        <= HARVEST_RADIUS_SQ
    )


def _is_matching_harvest(
    event,
    after: float,
    fixture_plant_id: str,
    fixture_pos: list[float],
    player_position: tuple[float, float, float],
) -> bool:
    if (
        event.kind != "server_data"
        or event.t <= after
        or event.data.get("payload_type") != "botany_harvest_progress"
    ):
        return False
    payload = event.data.get("payload")
    return (
        isinstance(payload, dict)
        and bool(payload.get("session_id"))
        and payload.get("target_id") == fixture_plant_id
        and payload.get("target_name") == HERB_ID
        and payload.get("plant_kind") == HERB_ID
        and _same_position(payload.get("target_pos"), fixture_pos)
        and _valid_target_pos(payload, player_position)
    )


def _is_matching_gathering_terminal(event, after: float, session_id: str) -> bool:
    if (
        event.kind != "server_data"
        or event.t <= after
        or event.data.get("payload_type") != "gathering_session"
    ):
        return False
    payload = event.data.get("payload")
    return (
        isinstance(payload, dict)
        and payload.get("session_id") == session_id
        and payload.get("target_type") == "herb"
        and payload.get("target_name") == HERB_ID
        and payload.get("completed") is True
        and payload.get("interrupted") is False
        and isinstance(payload.get("total_ticks"), int)
        and payload["total_ticks"] > 0
        and payload.get("progress_ticks") == payload["total_ticks"]
    )


def _wait_gather_progress(
    bot,
    after: float,
    fixture_plant_id: str,
    fixture_pos: list[float],
    player_position: tuple[float, float, float],
) -> dict:
    event = bot.wait_for(
        lambda observed: _is_matching_harvest(
            observed,
            after,
            fixture_plant_id,
            fixture_pos,
            player_position,
        )
        and observed.data["payload"].get("completed") is False
        and observed.data["payload"].get("interrupted") is False,
        timeout=HARVEST_TERMINAL_TIMEOUT_SECONDS,
        description="/bong gather 后真实 fixture Plant 的非空 session/target_id/有限 target_pos 进度",
    )
    payload = event.data["payload"]
    progress = payload.get("progress")
    if not isinstance(progress, (int, float)) or not math.isfinite(progress) or not 0.0 <= progress < 1.0:
        raise BotAssertionError(f"active botany progress 必须位于 [0,1)，实际 {payload}")
    return payload


def _wait_gathering_terminal(bot, after: float, session_id: str) -> dict:
    event = bot.wait_for(
        lambda observed: _is_matching_gathering_terminal(observed, after, session_id),
        timeout=HARVEST_TERMINAL_TIMEOUT_SECONDS,
        description="同一采集 session 的 gathering_session terminal 须 completed=true/interrupted=false",
    )
    return event.data["payload"]


def _wait_gather_terminal(
    bot,
    after: float,
    initial: dict,
    fixture_plant_id: str,
    fixture_pos: list[float],
    player_position: tuple[float, float, float],
) -> dict:
    event = bot.wait_for(
        lambda observed: _is_matching_harvest(
            observed,
            after,
            fixture_plant_id,
            fixture_pos,
            player_position,
        )
        and observed.data["payload"].get("session_id") == initial["session_id"]
        and observed.data["payload"].get("completed") is True
        and observed.data["payload"].get("interrupted") is False,
        timeout=HARVEST_TERMINAL_TIMEOUT_SECONDS,
        description="同一真实 Plant/session 的 botany terminal 须 completed=true/interrupted=false",
    )
    payload = event.data["payload"]
    if payload.get("progress") != 1.0:
        raise BotAssertionError(f"完成采集 terminal.progress 必须精确为 1.0，实际 {payload}")
    return payload


def _item_count(snapshot: dict, item_id: str) -> int:
    found = find_item(snapshot, item_id)
    return 0 if found is None else int(found["item"]["stack_count"])


def run(env) -> None:
    with env.new_bot("ProdBotany") as bot:
        snapshot = wait_join_and_inventory(bot)
        bot.cmd("clearinv all")
        bot.expect_chat("[dev] clearinv", timeout=10.0)
        snapshot = wait_inventory_revision_after_matching(
            bot,
            snapshot["revision"],
            lambda candidate: not candidate.get("placed_items")
            and not any(candidate.get("hotbar", [])),
            "采集准备阶段背包与快捷栏为空",
        )

        if bot.position is None:
            raise BotAssertionError("采集 fixture 创建前必须有权威玩家坐标")
        player_position = bot.position
        bot.cmd("botany_spawn spirit_grass")
        fixture_chat = bot.expect_chat(BOTANY_FIXTURE_PREFIX, timeout=10.0)
        fixture_plant_id, fixture_pos, fixture_zone = _parse_botany_fixture(
            fixture_chat.data["text"]
        )
        if fixture_zone != "spawn":
            raise BotAssertionError(
                "dev fixture 必须位于玩家当前 production zone；"
                f"实际 zone={fixture_zone!r}"
            )
        if sum(
            (actual - expected) ** 2
            for actual, expected in zip(fixture_pos, player_position)
        ) > HARVEST_RADIUS_SQ:
            raise BotAssertionError(
                "dev fixture 必须把真实 Plant 放在 production 6 格 resolver 内；"
                f"player={player_position} fixture={fixture_pos}"
            )

        gather_anchor = last_event_time(bot)
        before_revision = snapshot["revision"]
        before_count = _item_count(snapshot, HERB_ID)
        bot.cmd(f"bong gather {HERB_ID}")
        bot.expect_chat("Gameplay action queued.", timeout=10.0)
        initial = _wait_gather_progress(
            bot, gather_anchor, fixture_plant_id, fixture_pos, player_position
        )
        bot.intent(
            {
                "type": "botany_harvest_request",
                "v": 1,
                "session_id": initial["session_id"],
                "mode": "manual",
            }
        )
        terminal = _wait_gather_terminal(
            bot,
            gather_anchor,
            initial,
            fixture_plant_id,
            fixture_pos,
            player_position,
        )
        gathering_terminal = _wait_gathering_terminal(
            bot, gather_anchor, initial["session_id"]
        )
        if terminal["progress"] < initial["progress"]:
            raise BotAssertionError(
                "同一采集 session 的 terminal progress 不得倒退；"
                f"initial={initial['progress']} terminal={terminal['progress']}"
            )
        if gathering_terminal["session_id"] != terminal["session_id"]:
            raise BotAssertionError(
                "botany 与 gathering terminal 必须共享同一 session_id；"
                f"botany={terminal['session_id']!r} gathering={gathering_terminal['session_id']!r}"
            )
        snapshot = wait_inventory_revision_after_matching(
            bot,
            before_revision,
            lambda candidate: _item_count(candidate, HERB_ID) == before_count + 1,
            f"真实 Plant 完成后 {HERB_ID} 精确 +1",
            timeout=HARVEST_TERMINAL_TIMEOUT_SECONDS,
        )
        if _item_count(snapshot, HERB_ID) != before_count + 1:
            raise BotAssertionError(
                f"采集产物必须入包精确 +1，before={before_count} actual={snapshot}"
            )
        bot.assert_alive("真实 Plant 采集终态/产物之后")
