"""P4 生产系统：炼丹炉 / 炼器砧 client_request 最小黑盒链路。

覆盖面：
- dev 铺垫：发送 `/give furnace_fantie`、`/give fan_iron_anvil`，断言 server_data 继续回流
- client_request：`alchemy_furnace_place` / `alchemy_ignite` / `forge_station_place`
- 观察面：`[炼丹]` unknown recipe chat + 连接保持

`bong:server_data` inventory_snapshot 目前不能稳定给 bot 提供刚 give 的
item_instance_id，生产模板 `/give` chat 反馈在 CI/复用服也不稳定；深 intent 断言
留后续 bug/coverage plan。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "炼丹炉/炼器砧：dev give 入口 → client_request 入口 → 炼丹错误 chat"
MODULES = ["alchemy", "forge", "inventory", "network"]


def _event_mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _unique_pos(bot, salt: int) -> tuple[int, int, int]:
    if bot.position is None:
        raise BotAssertionError("production 场景需要 pos_look 后才能派生唯一放置坐标")
    x, y, z = bot.position
    spread = (sum(ord(ch) for ch in bot.username) % 13) + salt
    return (int(x) + spread, max(1, int(y)), int(z) + spread)


def _give_and_expect_server_data(bot, item_id: str) -> None:
    mark = _event_mark(bot)
    bot.cmd(f"give {item_id} 1")
    bot.expect_server_data_payload(timeout=10.0, after=mark)


def run(env) -> None:
    with env.new_bot("ProdAF") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        _give_and_expect_server_data(bot, "furnace_fantie")
        furnace_pos = _unique_pos(bot, 3)
        bot.intent(
            {
                "type": "alchemy_furnace_place",
                "v": 1,
                "x": furnace_pos[0],
                "y": furnace_pos[1],
                "z": furnace_pos[2],
                "item_instance_id": 0,
            }
        )
        bot.intent(
            {
                "type": "alchemy_ignite",
                "v": 1,
                "furnace_pos": list(furnace_pos),
                "recipe_id": "bot_e2e_no_such_recipe",
            }
        )
        bot.expect_chat("[炼丹] 未知丹方：bot_e2e_no_such_recipe", timeout=10.0)

        _give_and_expect_server_data(bot, "fan_iron_anvil")
        station_pos = _unique_pos(bot, 7)
        bot.intent(
            {
                "type": "forge_station_place",
                "v": 1,
                "x": station_pos[0],
                "y": station_pos[1],
                "z": station_pos[2],
                "item_instance_id": 0,
                "station_tier": 1,
            }
        )
        bot.assert_alive("炼丹炉与炼器砧 production 入口链路后")
