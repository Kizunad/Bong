"""Zone + dropped-loot 生命周期 bot 场景共享断言。

下划线前缀让 run_scenarios 跳过本模块（不当作独立场景发现）。

黑盒断言面契约（都是 server 生产行为）：
- `/tpzone <zone>` → chat `Teleported to zone `<zone>`.` 且随后权威 `zone_info`
- `inventory_discard_item` intent → `dropped_loot_sync` 广播（世界可见）
- `pickup_dropped_item` intent → 成功则 instance 进 inventory_snapshot、registry 移除
- zone 重入的 `zone_info` 关键字段必须与首次一致
"""

from __future__ import annotations

from typing import Any

from bot.bot import Bot, BotAssertionError

from ._inventory_helpers import (
    find_item,
    latest_inventory_snapshot,
    wait_inventory_snapshot_after,
)

GIVE_TEMPLATE = "starter_talisman"
SPIRIT_QI_TOLERANCE = 1e-3


def event_watermark(bot: Bot) -> float:
    with bot._lock:
        return bot.events[-1].t if bot.events else 0.0


def wait_join_settled(bot: Bot, timeout: float = 15.0) -> None:
    """等 join 完成（game_join + pos_look）。

    server 的 `initialize_joined_client` 在 join 早期把 Position 重置到出生点，
    若在此之前发 `/tpzone`，传送位置会被出生点初始化覆盖（实证：zone_info 停在
    spawn）。pos_look 到达说明出生点初始化已执行完毕，之后的 tpzone 才会生效。
    """
    bot.expect_event("game_join", timeout=timeout)
    bot.expect_event("pos_look", timeout=timeout)


def teleport_to_zone(bot: Bot, zone: str, timeout: float = 10.0) -> float:
    """/tpzone <zone> 并等待权威 chat 回执；返回命令发送前水位。"""
    sent_at = event_watermark(bot)
    bot.cmd(f"tpzone {zone}")
    bot.expect_chat(f"Teleported to zone `{zone}`.", timeout=timeout)
    return sent_at


def wait_zone_info(
    bot: Bot, zone: str, after: float, timeout: float = 15.0
) -> dict[str, Any]:
    """等待 after 之后第一条 zone=<zone> 的 zone_info（真实 zone transition 回执）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "zone_info"
            and e.t > after
            and e.data["payload"].get("zone") == zone
        ),
        timeout=timeout,
        description=f"t>{after:.3f}s 后 zone_info/{zone}（/tpzone 应触发 zone transition）",
    )
    return event.data["payload"]


def clear_inventory(bot: Bot, timeout: float = 10.0) -> None:
    bot.cmd("clearinv all")
    bot.expect_chat("[dev] clearinv PackAndHotbar revision=", timeout=timeout)


def give_item(
    bot: Bot, template: str = GIVE_TEMPLATE, timeout: float = 10.0
) -> dict[str, Any]:
    """/give 一件物品，返回 inventory_snapshot 中的定位（含 item 与 from location）。

    必须等快照「实际含该物品」而非「水位后第一条快照」：clearinv 的旧快照可能
    晚于 give 发送时刻到达（server 处理 give 前先 flush 了 pre-give 状态），
    用后一条会拿到空快照误判 give 失败（实证 revision=7 空快照）。
    """
    sent_at = event_watermark(bot)
    bot.cmd(f"give {template} 1")
    bot.expect_chat(f"[dev] gave {template} x1", timeout=timeout)
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > sent_at
            and find_item(e.data["payload"], template) is not None
        ),
        timeout=timeout,
        description=f"/give {template} 后 inventory_snapshot 含该物品",
    )
    snapshot = event.data["payload"]
    located = find_item(snapshot, template)
    return located


def discard_item(bot: Bot, located: dict[str, Any]) -> float:
    """丢弃到世界（inventory_discard_item intent）；返回发送前水位。"""
    sent_at = event_watermark(bot)
    bot.intent(
        {
            "type": "inventory_discard_item",
            "v": 1,
            "instance_id": located["item"]["instance_id"],
            "from": located["location"],
        }
    )
    return sent_at


def pickup_instance(bot: Bot, instance_id: int) -> float:
    """尝试拾取 dropped loot；返回发送前水位。"""
    sent_at = event_watermark(bot)
    bot.intent({"type": "pickup_dropped_item", "v": 1, "instance_id": instance_id})
    return sent_at


def sync_has_instance(payload: dict[str, Any], instance_id: int) -> bool:
    return any(drop.get("instance_id") == instance_id for drop in payload.get("drops", []))


def wait_dropped_loot_has(
    bot: Bot, instance_id: int, after: float, timeout: float = 15.0
) -> dict[str, Any]:
    """等待 after 之后一条含 instance_id 的 dropped_loot_sync（丢弃广播）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "dropped_loot_sync"
            and e.t > after
            and sync_has_instance(e.data["payload"], instance_id)
        ),
        timeout=timeout,
        description=(
            f"t>{after:.3f}s 后 dropped_loot_sync 含 instance_id={instance_id}（丢弃已入世界）"
        ),
    )
    return event.data["payload"]


def wait_dropped_loot_without(
    bot: Bot, instance_id: int, after: float, timeout: float = 15.0
) -> dict[str, Any]:
    """等待 after 之后一条不含 instance_id 的 dropped_loot_sync（已被拾取/移除）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "dropped_loot_sync"
            and e.t > after
            and not sync_has_instance(e.data["payload"], instance_id)
        ),
        timeout=timeout,
        description=f"t>{after:.3f}s 后 dropped_loot_sync 不再含 instance_id={instance_id}",
    )
    return event.data["payload"]


def latest_dropped_loot(bot: Bot) -> dict[str, Any] | None:
    for event in reversed(bot.events):
        if event.kind == "server_data" and event.data["payload_type"] == "dropped_loot_sync":
            return event.data["payload"]
    return None


def snapshot_has_instance(snapshot: dict[str, Any], instance_id: int) -> bool:
    for placed in snapshot.get("placed_items", []):
        if placed["item"].get("instance_id") == instance_id:
            return True
    for item in snapshot.get("hotbar", []):
        if item and item.get("instance_id") == instance_id:
            return True
    for value in snapshot.get("equipped", {}).values():
        items = value if isinstance(value, list) else [value]
        if any(item and item.get("instance_id") == instance_id for item in items):
            return True
    return False


def wait_inventory_has_instance(
    bot: Bot, instance_id: int, after: float, timeout: float = 15.0
) -> dict[str, Any]:
    """等待 after 之后一条含 instance_id 的 inventory_snapshot（拾取入包）。"""
    event = bot.wait_for(
        lambda e: (
            e.kind == "server_data"
            and e.data["payload_type"] == "inventory_snapshot"
            and e.t > after
            and snapshot_has_instance(e.data["payload"], instance_id)
        ),
        timeout=timeout,
        description=f"t>{after:.3f}s 后 inventory_snapshot 含 instance_id={instance_id}",
    )
    return event.data["payload"]


def assert_zone_reentry_consistent(
    bot: Bot, first: dict[str, Any], second: dict[str, Any], zone: str
) -> None:
    """离开 zone 再返回后，zone_info 的稳定字段必须一致（spirit_qi 允许守恒吸纳容差）。"""
    mismatches: dict[str, dict[str, Any]] = {}
    for key in ("zone", "danger_level", "status", "active_events"):
        if first.get(key) != second.get(key):
            mismatches[key] = {"first": first.get(key), "second": second.get(key)}
    qi1 = first.get("spirit_qi")
    qi2 = second.get("spirit_qi")
    if (
        not isinstance(qi1, (int, float))
        or not isinstance(qi2, (int, float))
        or abs(qi1 - qi2) > SPIRIT_QI_TOLERANCE
    ):
        mismatches["spirit_qi"] = {
            "first": qi1,
            "second": qi2,
            "tolerance": SPIRIT_QI_TOLERANCE,
        }
    if mismatches:
        raise BotAssertionError(
            f"[{bot.username}] zone `{zone}` 离开再返回后 zone_info 必须一致；"
            f"mismatches={mismatches!r} first={first!r} second={second!r}"
        )
