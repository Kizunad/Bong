"""禁用的历史浅场景：占位 instance_id=0 不能证明炼丹炉或炼器砧生产链路。

真实炼丹链路由 `production_alchemy_brew_pill` 覆盖；真实炼器链路由
`production_forge_full_cycle` 覆盖。本文件仅保留旧请求形状供人工诊断，默认验收矩阵
不得运行，防止 generic server_data / assert_alive 被误记为 P4 证据。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "炼丹炉/炼器砧：dev give 入口 → client_request 入口 → 炼丹错误 chat"
MODULES = ["alchemy", "forge", "inventory", "network"]
DEFAULT_ENABLED = False


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
