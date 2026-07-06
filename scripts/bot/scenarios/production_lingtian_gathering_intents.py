"""P4 生产系统：灵田 / 采集最小黑盒链路。

覆盖面：
- dev 铺垫：`/give hoe_iron`，以库存 payload 同步
- client_request：`inventory_move_intent` 装备锄头、`lingtian_start_till`
- chat 反馈：`/bong gather spirit_grass` → Gameplay action queued.
- payload 回流：`bong:server_data` 的 `lingtian_session` / 采集进度

灵田能否真正开垦取决于 live server 当前地形；场景只断言 wire 入口与 HUD
payload 回流，不把地形/作物结算当成 bot 脚本职责。
"""

from bot.bot import BotAssertionError
from bot import proto_min

DESCRIPTION = "灵田/采集：锄头 give+装备 → lingtian_start_till；/bong gather → 采集进度回流"
MODULES = ["lingtian", "gathering", "inventory", "network"]

GATHER_PROGRESS_PAYLOADS = {"gathering_session", "botany_harvest_progress"}


def _event_mark(bot) -> float:
    return bot.events[-1].t if bot.events else 0.0


def _expect_gather_progress_payload(bot, after: float) -> None:
    def matches(event) -> bool:
        if (
            event.kind != "payload"
            or event.data["channel"] != "bong:server_data"
            or event.t <= after
        ):
            return False
        return proto_min.server_data_payload_name(event.data["data"]) in GATHER_PROGRESS_PAYLOADS

    bot.wait_for(
        matches,
        timeout=15.0,
        description=(
            "采集链路的 bong:server_data 进度回流"
            "（gathering_session 或 botany_harvest_progress）"
        ),
    )


def run(env) -> None:
    with env.new_bot("ProdLG") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        mark = _event_mark(bot)
        bot.cmd("give hoe_iron 1")
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

        mark = _event_mark(bot)
        bot.cmd("bong gather spirit_grass")
        bot.expect_chat("Gameplay action queued.", timeout=10.0)
        _expect_gather_progress_payload(bot, after=mark)
        bot.assert_alive("灵田 intent 与 gather 命令后")
