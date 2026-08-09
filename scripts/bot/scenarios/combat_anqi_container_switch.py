"""暗器链：战斗容器切换（anqi_container_switch）的黑盒契约。

流程：显式切到 quiver（事件 from=hand_slot/to=quiver，暴露窗口 tick+10）→
同目标重复切换静默（不发事件）→ to=None 轮换（quiver→pocket_pouch）→
切到 fenglinghe 静默拒收（不允许战斗切换）→ 切回 hand_slot（
pocket_pouch→hand_slot）→ 同目标再次静默。

锁定的契约（server/src/combat/anqi_v2.rs switch_container_slot）：
- Fenglinghe 不允许 combat swap（allows_combat_swap=false）→ 返回 None，
  不发事件。
- 目标与当前 active 相同 → 返回当前槽位但不发事件。
- 实际切换 → switching_until_tick = tick + 10（CONTAINER_SWITCH_EXPOSURE_TICKS），
  发布 bong:anqi/container_swap（from/to/switching_until_tick/tick）。
- to=None → cycle_container_slot：next_combat_container 轮换
  HandSlot→Quiver→PocketPouch→HandSlot。
"""

import time

from bot.scenarios._combat_helpers import last_event_time
from bot.scenarios._inventory_helpers import wait_join_and_inventory
from bot._redis_helpers import RedisPubSub

DESCRIPTION = "暗器战斗容器切换：显式切换/轮换/拒收 fenglinghe/同目标静默"
MODULES = ["anqi", "combat"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

CH_SWAP = "bong:anqi/container_swap"


def _switch(bot, to) -> None:
    intent: dict = {"type": "anqi_container_switch", "v": 1}
    if to is not None:
        intent["to"] = to
    bot.intent(intent)


def _wait_swap(pubsub, from_kind: str, to_kind: str, after_tick: int, context: str) -> dict:
    evt = pubsub.wait_event(
        CH_SWAP,
        lambda e: e.get("from") == from_kind
        and e.get("to") == to_kind
        and int(e.get("tick", 0)) > after_tick,
        timeout=20.0,
        description=f"{context}：{from_kind} -> {to_kind}",
    )
    assert int(evt.get("switching_until_tick", 0)) == int(evt["tick"]) + 10, (
        f"{context}：暴露窗口应为 tick+10，实际 {evt!r}"
    )
    assert str(evt.get("carrier", "")).startswith("player:"), f"carrier 应为玩家线缆 id，实际 {evt!r}"
    return evt


def _expect_silent(pubsub, anchor_count: int, window_s: float, context: str) -> None:
    time.sleep(window_s)
    assert len(pubsub.events_for(CH_SWAP)) == anchor_count, (
        f"{context}：不应发布 container_swap 事件，"
        f"期望 {anchor_count} 条，实际 {len(pubsub.events_for(CH_SWAP))}"
    )


def run(env) -> None:
    pubsub = RedisPubSub.from_env()
    try:
        pubsub.subscribe(CH_SWAP)
        with env.new_bot("Switch") as bot:
            wait_join_and_inventory(bot)
            bot.assert_alive("连接后")

            # 1) 显式切 quiver：hand_slot -> quiver
            _switch(bot, "quiver")
            first = _wait_swap(pubsub, "hand_slot", "quiver", 0, "首次切换")
            first_tick = int(first["tick"])

            # 2) 同目标重复：静默（不发事件）
            _switch(bot, "quiver")
            _expect_silent(pubsub, 1, 3.0, "同目标重复切换")

            # 3) to=None 轮换：quiver -> pocket_pouch
            _switch(bot, None)
            _wait_swap(pubsub, "quiver", "pocket_pouch", first_tick, "轮换切换")

            # 4) fenglinghe：拒收，静默
            _switch(bot, "fenglinghe")
            _expect_silent(pubsub, 2, 3.0, "fenglinghe 拒收")

            # 5) 切回 hand_slot：pocket_pouch -> hand_slot
            _switch(bot, "hand_slot")
            _wait_swap(pubsub, "pocket_pouch", "hand_slot", first_tick, "切回手部")

            # 6) 同目标 hand_slot：静默
            _switch(bot, "hand_slot")
            _expect_silent(pubsub, 3, 3.0, "同目标 hand_slot")

            bot.assert_alive("容器切换全链后")
    finally:
        pubsub.stop()
