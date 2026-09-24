"""loot 生命周期：两个协议 Bot 竞速 claim 同一 dropped loot。

黑盒断言面（都是 server 生产行为）：
1. 丢弃物对全服玩家可见：`dropped_loot_sync` 广播给同服所有 Bot
2. `pickup_dropped_item` 是原子独占：并发双 claim 恰好一个赢
   - 赢家 instance 进入其 inventory_snapshot
   - 输家 inventory_snapshot 无该 instance（resync 拒绝路径）
3. 竞速后 registry 条目被移除：双方都收到不再含该 instance 的 `dropped_loot_sync`
"""

from __future__ import annotations

from bot.bot import Bot, BotAssertionError

from ._zone_loot_helpers import (
    clear_inventory,
    discard_item,
    event_watermark,
    give_item,
    latest_inventory_snapshot,
    pickup_instance,
    snapshot_has_instance,
    teleport_to_zone,
    wait_dropped_loot_has,
    wait_dropped_loot_without,
    wait_inventory_snapshot_after,
    wait_join_settled,
    wait_zone_info,
)

DESCRIPTION = "两 Bot 竞速 claim 同一掉落物：恰好一个赢得 instance，另一个 inventory 无该实例"
MODULES = ["loot", "inventory", "multibot", "network"]

SPAWN = "spawn"


def run(env) -> None:
    with env.new_bot("Alc") as alice, env.new_bot("Bob") as bob:
        _race(alice, bob)


def _race(alice: Bot, bob: Bot) -> None:
    # 1. 两 Bot 落在同一点（zone 中心），保证 pickup 2.5 格射程内。
    #    先等各自 join 完成，tpzone 才不被出生点初始化覆盖。
    for bot in (alice, bob):
        wait_join_settled(bot)
        teleport_to_zone(bot, SPAWN)
        wait_zone_info(bot, SPAWN, after=0.0)
        clear_inventory(bot)

    # 2. Alice 制造唯一 instance 并丢弃入世界。
    #    每个 Bot 有独立 t0 时钟，跨 bot 水位不能互比：Bob 的广播水位必须在
    #    Alice 发 discard intent 之前取（广播严格晚于该时刻）。
    located = give_item(alice)
    instance_id = located["item"]["instance_id"]
    bob_drop_at = event_watermark(bob)
    drop_watermark = discard_item(alice, located)

    # 3. 双方都必须看到广播才开跑（Bob 也要在射程内见到掉落物）
    wait_dropped_loot_has(alice, instance_id, after=drop_watermark)
    wait_dropped_loot_has(bob, instance_id, after=bob_drop_at)

    # 4. 竞速 claim：两 intent 背靠背发出，恰好一个赢
    alice_pickup_at = pickup_instance(alice, instance_id)
    bob_pickup_at = pickup_instance(bob, instance_id)

    # 5. registry 条目被移除：双方都收到不含该 instance 的广播（各自时钟）
    wait_dropped_loot_without(alice, instance_id, after=alice_pickup_at)
    wait_dropped_loot_without(bob, instance_id, after=bob_pickup_at)

    # 6. 双方都拿到竞速后的 resync，核对唯一赢家（各自时钟）
    wait_inventory_snapshot_after(alice, alice_pickup_at)
    wait_inventory_snapshot_after(bob, bob_pickup_at)
    alice_has = snapshot_has_instance(latest_inventory_snapshot(alice), instance_id)
    bob_has = snapshot_has_instance(latest_inventory_snapshot(bob), instance_id)

    if alice_has == bob_has:
        raise BotAssertionError(
            f"[ALICE/BOB] 竞速 claim 必须恰好一个赢家；"
            f"alice_has={alice_has} bob_has={bob_has} instance_id={instance_id}"
        )

    alice.assert_alive("竞速 claim 后")
    bob.assert_alive("竞速 claim 后")
