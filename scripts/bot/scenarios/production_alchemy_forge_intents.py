"""P4 生产系统：炼丹炉 / 炼器砧 client_request 最小黑盒链路。

覆盖面：
- dev 铺垫：`/give furnace_fantie`、`/give fan_iron_anvil`，以库存 payload 同步
- client_request：`alchemy_furnace_place` → `alchemy_open_furnace`，
  `alchemy_ignite` 错误反馈，`forge_station_place`
- 观察面：`bong:server_data` inventory/alchemy_furnace 回流 + `[炼丹]` chat

`bong:server_data` 生产线是 protobuf；本场景只做类型级浅断言，字段级数值留 P6。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "炼丹炉/炼器砧：dev give → client_request 放置/打开 → server_data/chat 回流"
MODULES = ["alchemy", "forge", "inventory", "network"]


def _event_mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _unique_pos(bot, salt: int) -> tuple[int, int, int]:
    if bot.position is None:
        raise BotAssertionError("production 场景需要 pos_look 后才能派生唯一放置坐标")
    x, y, z = bot.position
    spread = (sum(ord(ch) for ch in bot.username) % 13) + salt
    return (int(x) + spread, max(1, int(y)), int(z) + spread)


def _give_and_find(bot, item_id: str):
    mark = _event_mark(bot)
    bot.cmd(f"give {item_id} 1")
    item = bot.expect_inventory_item(item_id, timeout=10.0, after=mark)
    if item.location is None:
        raise BotAssertionError(
            f"[{bot.username}] 期望 {item_id} 在 inventory_snapshot 中带位置，"
            f"实际仅拿到 instance_id={item.instance_id}"
        )
    return item


def run(env) -> None:
    with env.new_bot("ProdAF") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        furnace = _give_and_find(bot, "furnace_fantie")
        furnace_pos = _unique_pos(bot, 3)
        mark = _event_mark(bot)
        bot.intent(
            {
                "type": "alchemy_furnace_place",
                "v": 1,
                "x": furnace_pos[0],
                "y": furnace_pos[1],
                "z": furnace_pos[2],
                "item_instance_id": furnace.instance_id,
            }
        )
        bot.expect_server_data_payload("inventory_snapshot", timeout=10.0, after=mark)

        mark = _event_mark(bot)
        bot.intent({"type": "alchemy_open_furnace", "v": 1, "furnace_pos": list(furnace_pos)})
        bot.expect_server_data_payload("alchemy_furnace", timeout=10.0, after=mark)

        bot.intent(
            {
                "type": "alchemy_ignite",
                "v": 1,
                "furnace_pos": list(furnace_pos),
                "recipe_id": "bot_e2e_no_such_recipe",
            }
        )
        bot.expect_chat("[炼丹] 未知丹方：bot_e2e_no_such_recipe", timeout=10.0)

        anvil = _give_and_find(bot, "fan_iron_anvil")
        station_pos = _unique_pos(bot, 7)
        mark = _event_mark(bot)
        bot.intent(
            {
                "type": "forge_station_place",
                "v": 1,
                "x": station_pos[0],
                "y": station_pos[1],
                "z": station_pos[2],
                "item_instance_id": anvil.instance_id,
                "station_tier": 1,
            }
        )
        bot.expect_server_data_payload("inventory_snapshot", timeout=10.0, after=mark)
        bot.assert_alive("炼丹炉与炼器砧 production intent 链路后")
