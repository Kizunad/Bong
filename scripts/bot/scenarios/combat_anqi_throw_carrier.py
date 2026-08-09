"""暗器链：carrier 抛射（throw_carrier）的空手前置黑盒契约。

`throw_carrier_intents`（server/src/combat/carrier.rs）只从手槽持有的暗器载体读取
投掷物（清空手槽、生成弹道、命中耗尽发 `bong:combat/projectile_despawned`）。由于裸
暗器载体 `anqi_yibian_shougu` 在物品目录是 `category="misc"` 且 `validate_equip_to`
手槽档只放行 weapon/tool（见姊妹场景 combat_anqi_charge_carrier），真实客户端无法
把载体挂进 main_hand —— 因而抛射的合法命中路径同样不可端到端触达。

本场景锁定的是这条**空手护栏**：在无持载体状态下发出 throw_carrier intent，
服务器端静默不产生任何抛射——手上无伤、无 despawn 事件、无库存改动、玩家存活。
这证明抛射链在"无载体"输入下是搁置的非破坏性 no-op，不会误伤或误发事件。

**已知缺口（记录不隐瞒）**：合法抛射（清空手部 + 弹道耗竭事件）的成功分支同样只在
server 单测里用直接写槽的 `inventory_with_main_hand()` helper 覆盖；真"装弹→抛射"
链路请在物品目录把载体档位回填为 anqi/hidden_weapon 后再补该场景。
"""

import time

from bot.scenarios._inventory_helpers import (
    latest_inventory_snapshot,
    wait_join_and_inventory,
)
from bot._redis_helpers import RedisPubSub

DESCRIPTION = "抛射护栏：无持载体发出 throw_carrier → 静默 no-op（无 despawn 事件 / 无库存改动 / 存活）"
MODULES = ["anqi", "combat", "inventory"]
DEFAULT_ENABLED = False
REQUIRED_ENV = "BOT_E2E_ANQI_REDIS"
RUN_IN_ALL_WHEN_ENV = REQUIRED_ENV

DESPAWN_CH = "bong:combat/projectile_despawned"


def run(env) -> None:
    pubsub = RedisPubSub.from_env()
    try:
        pubsub.subscribe(DESPAWN_CH)
        with env.new_bot("Throw") as bot:
            wait_join_and_inventory(bot)
            before = latest_inventory_snapshot(bot)
            prev_rev = int(before["revision"])
            initial_held = (before.get("equipped", {}).get("main_hand_held") or {}).get(
                "item_id"
            )
            assert not initial_held.startswith("anqi_"), (
                f"前置：默认手槽应是非暗器武器（新手村 fixture 通常给 iron_sword），"
                f"实际 {initial_held!r}——若服务器默认就发暗器，本护栏前置失效需重选"
            )

            # 手持非暗器（默认武器）发出投掷 intent —— 无载体投掷应被静默忽略。
            # 选非零方向声明确会走的投放分支：若服务器真解析到载体，会产生 despawn
            # 事件或库存变化。
            bot.intent(
                {"type": "throw_carrier", "v": 1, "dir": [0.0, 0.0, 1.0], "power": 0.5}
            )

            # 无事件：窗口内不应冒出任何 projectile_despawned。
            time.sleep(3.0)
            fired = pubsub.events_for(DESPAWN_CH)
            assert not fired, f"空手抛射不应产生 despawn 事件，实际 {fired!r}"

            # 无库存改动：revision 不变，且手持项仍为同一非暗器武器（未被清空/替换）。
            after = latest_inventory_snapshot(bot)
            assert int(after["revision"]) == prev_rev, (
                f"空抛射不应改动 inventory，revision {prev_rev} -> {after['revision']}"
            )
            still_held = (after.get("equipped", {}).get("main_hand_held") or {}).get(
                "item_id"
            )
            assert still_held == initial_held, (
                f"空手抛射不应触碰手槽（仍应持 {initial_held!r}），实际 {still_held!r}"
            )

            bot.assert_alive("空手抛射后")
    finally:
        pubsub.stop()