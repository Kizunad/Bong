"""延寿棺 Bot 场景共用的世界实体清场 helper。"""

from bot.scenarios._combat_helpers import last_event_time


COFFIN_MARKER_ENTITY_KIND = 160
_POSITION_TOLERANCE = 0.01


def _marker_position(lower: tuple[int, int, int]) -> tuple[float, float, float]:
    return (lower[0] + 1.0, float(lower[1]), lower[2] + 0.5)


def teardown_coffin(bot, lower: tuple[int, int, int], timeout: float = 10.0) -> None:
    """破坏场景创建的棺材，并以对应 marker 的 despawn 作为权威完成证据。"""
    expected = _marker_position(lower)
    marker = bot.wait_for(
        lambda e: e.kind == "entity_spawn"
        and e.data["type"] == COFFIN_MARKER_ENTITY_KIND
        and all(
            abs(e.data[axis] - coordinate) <= _POSITION_TOLERANCE
            for axis, coordinate in zip(("x", "y", "z"), expected)
        ),
        timeout=timeout,
        description=f"待清场的 mundane_coffin marker @ {expected}",
    )

    anchor = last_event_time(bot)
    bot.intent(
        {
            "type": "coffin_break",
            "v": 1,
            "x": lower[0],
            "y": lower[1],
            "z": lower[2],
        }
    )
    bot.wait_for(
        lambda e: e.kind == "entities_destroy"
        and e.t > anchor
        and marker.data["entity_id"] in e.data["entity_ids"],
        timeout=timeout,
        description=(
            f"coffin_break 后 marker #{marker.data['entity_id']} 应被 despawn"
        ),
    )
