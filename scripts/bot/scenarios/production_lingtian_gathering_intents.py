"""P4 生产系统：灵田 / 采集最小黑盒链路。

覆盖面：
- dev 铺垫：`/give hoe_iron`
- client_request：`inventory_move_intent` 装备锄头、`lingtian_start_till`
- chat 反馈：`/bong gather spirit_grass` → Gameplay action queued.
- payload 回流：`bong:server_data` 的 `lingtian_session`

灵田能否真正开垦取决于 live server 当前地形；场景只断言 wire 入口与 HUD
payload 回流，不把地形/作物状态当成 bot 脚本职责。
"""

from bot.bot import BotAssertionError

DESCRIPTION = "灵田/采集：锄头 give+装备 → lingtian_start_till payload；/bong gather chat 反馈"
MODULES = ["lingtian", "gathering", "inventory", "network"]


def _event_mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def run(env) -> None:
    with env.new_bot("ProdLG") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        mark = _event_mark(bot)
        bot.cmd("give hoe_iron 1")
        bot.expect_chat("[dev] gave hoe_iron x1", timeout=10.0)
        hoe = bot.expect_inventory_item("hoe_iron", timeout=10.0, after=mark)
        if hoe.location is None:
            raise BotAssertionError(
                f"[{bot.username}] 期望 hoe_iron 在 inventory_snapshot 中带 from 位置，"
                f"实际仅拿到 instance_id={hoe.instance_id}"
            )

        mark = _event_mark(bot)
        bot.intent(
            {
                "type": "inventory_move_intent",
                "v": 1,
                "instance_id": hoe.instance_id,
                "from": hoe.location,
                "to": {"kind": "equip", "slot": "main_hand", "state": "held"},
            }
        )
        bot.expect_server_data_payload("inventory_snapshot", timeout=10.0, after=mark)

        if bot.position is None:
            raise BotAssertionError("lingtian 场景需要 pos_look 后才能派生目标格")
        x, y, z = bot.position
        target = (int(x), max(1, int(y) - 1), int(z))
        mark = _event_mark(bot)
        bot.intent(
            {
                "type": "lingtian_start_till",
                "v": 1,
                "x": target[0],
                "y": target[1],
                "z": target[2],
                "hoe_instance_id": hoe.instance_id,
                "mode": "manual",
            }
        )
        bot.expect_server_data_payload("lingtian_session", timeout=10.0, after=mark)

        bot.cmd("bong gather spirit_grass")
        bot.expect_chat("Gameplay action queued.", timeout=10.0)
        bot.assert_alive("灵田 intent 与 gather 命令后")
