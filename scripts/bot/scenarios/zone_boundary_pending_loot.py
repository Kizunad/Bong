"""zone + loot 生命周期：携带待拾取掉落物穿越 zone 边界。

黑盒断言面（都是 server 生产行为）：
1. `/tpzone` 到真实 zones.json zone 中心 → server 权威 `zone_info` transition 广播
2. `inventory_discard_item` → `dropped_loot_sync` 广播（世界可见，非玩家私有）
3. 穿越 zone 边界（spawn -> rift）不吞掉落物：pending loot 仍留在 `dropped_loot_sync`
4. 返回原 zone 后 `pickup_dropped_item` 仍能拾回同一 instance（loot 可 claim）

选 zone 硬约束：
- `/tpzone` 落点在 `center.y + 24`（cmd/dev/tpzone.rs），必须落在目标 zone AABB 内
  才解析为该 zone；y 纵向跨度 <48 的 zone（如 north_waste_east_scorch，
  max.y=100 < 80+24）会解析回 spawn 导致 transition 不触发。
- 丢弃/拾取必须发生在有真实地表的 zone（fixture 里是 spawn）：bot 不主动发
  MovePlayer，落在无地表的 void zone 时 server 权威 Position 会漂移（实证
  blood_valley_east_scorch tpzone 后 y 104→114→124），2.5 格拾取射程会失效。
  spawn 下 Position 稳定钉在 (0, 152, 0)，丢弃物落在 +0.35/+0.5/+0.35，往返后
  距离固定 <2.5 格，拾取确定可达。
"""

from __future__ import annotations

from bot.bot import Bot, BotAssertionError

from ._zone_loot_helpers import (
    clear_inventory,
    discard_item,
    give_item,
    latest_dropped_loot,
    pickup_instance,
    sync_has_instance,
    teleport_to_zone,
    wait_dropped_loot_has,
    wait_dropped_loot_without,
    wait_inventory_has_instance,
    wait_join_settled,
    wait_zone_info,
)

DESCRIPTION = "携带待拾取掉落物穿越 zone 边界：transition 不吞掉落物，返回原 zone 后仍可拾回"
MODULES = ["zone", "loot", "inventory", "network"]

# 基准 zone=spawn（fixture 唯一稳定地表，见 docstring）；rift 灵气低(0.07)/danger 5，
# y 纵向跨度 50 满足 /tpzone 落点 AABB 约束
SPAWN = "spawn"
RIFT = "rift_mouth_north_002"


def run(env) -> None:
    with env.new_bot("Zlb") as bot:
        _scenario(bot)


def _scenario(bot: Bot) -> None:
    # 0. 等 join 出生点初始化完成，再传送（否则 tpzone 会被 join 位置覆盖）
    wait_join_settled(bot)

    # 1. 进入 spawn（基准 zone），建立确定性 inventory
    base_at = teleport_to_zone(bot, SPAWN)
    wait_zone_info(bot, SPAWN, after=base_at)
    clear_inventory(bot)
    located = give_item(bot)

    # 2. 丢弃 → 世界广播含 instance
    instance_id = located["item"]["instance_id"]
    drop_watermark = discard_item(bot, located)
    wait_dropped_loot_has(bot, instance_id, after=drop_watermark)

    # 3. 携带 pending loot 穿越 zone 边界（spawn -> rift）
    cross_watermark = teleport_to_zone(bot, RIFT)
    wait_zone_info(bot, RIFT, after=cross_watermark)
    pending = latest_dropped_loot(bot)
    if pending is None or not sync_has_instance(pending, instance_id):
        raise BotAssertionError(
            f"[{bot.username}] zone transition 后 pending loot 必须仍在 dropped_loot_sync；"
            f"instance_id={instance_id} payload={pending!r}"
        )

    # 4. 返回 spawn，立即 claim 同一 instance（Position 稳定钉在基准点）
    back_watermark = teleport_to_zone(bot, SPAWN)
    wait_zone_info(bot, SPAWN, after=back_watermark)
    pickup_instance(bot, instance_id)
    wait_inventory_has_instance(bot, instance_id, after=back_watermark)
    wait_dropped_loot_without(bot, instance_id, after=back_watermark)

    bot.assert_alive("zone 边界穿越 + 拾回后")
