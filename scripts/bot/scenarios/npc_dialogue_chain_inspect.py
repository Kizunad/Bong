"""`npc_inspect_request` 的协议级回归（对话链路 P1 覆盖）。

黑盒锚点（server/src/network/client_request_handler.rs）：
- 目标不可解析（不存在 / 超 6.0m / 跨维度 / Terminated）→ 无 § 前缀
  "[NPC] 目标已不在附近，无法查看。"，三个请求类型各自独立字符串。
- 命中目标 → "§7[NPC] {display_name}：{greeting}"。zombie 由 `/npc_scenario fight`
  确定性生成（display_name="游尸·醒灵"，greeting="游尸没有回应。"），
  玩家无需依赖随机播种；先负向后正向，锚定 entity_spawn 后的 entity_id。

本场景刻意不断言散修（villager）greeting——那属于随机播种路径，
由 npc_dialogue_chain_trade 的 BOT_E2E_ROGUE_TRADE 阶段承接。
"""

DESCRIPTION = "npc_inspect_request：不可解析目标拒绝 + zombie 逐字 greeting 回显"
MODULES = ["npc", "dialogue"]

from bot.bot import BotAssertionError

from ._npc_dialogue_helpers import (
    OUT_OF_RANGE_INSPECT,
    last_event_time,
    queue_fight_zombie,
    request_and_assert,
)

NONEXISTENT_NPC_ENTITY_ID = 999999
ZOMBIE_DISPLAY = "游尸·醒灵"
ZOMBIE_INSPECT = f"§7[NPC] {ZOMBIE_DISPLAY}：游尸没有回应。"


def run(env) -> None:
    with env.new_bot("NpcIn") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "npc_inspect_request",
                "v": 1,
                "npc_entity_id": NONEXISTENT_NPC_ENTITY_ID,
            }
        )
        event = bot.wait_for(
            lambda e: e.kind == "chat"
            and e.t > anchor
            and OUT_OF_RANGE_INSPECT in e.data["text"],
            timeout=8.0,
            description="不存在的 npc_entity_id → 不在附近拒绝",
        )
        if event.data["text"] != OUT_OF_RANGE_INSPECT:
            raise BotAssertionError(
                f"期望逐字 {OUT_OF_RANGE_INSPECT!r}，实际 {event.data['text']!r}"
            )

        spawn = queue_fight_zombie(bot)
        zombie_id = spawn.data["entity_id"]

        request_and_assert(
            bot,
            {"type": "npc_inspect_request", "v": 1, "npc_entity_id": zombie_id},
            zombie_id,
            ZOMBIE_INSPECT,
            f"inspect 僵尸（display_name={ZOMBIE_DISPLAY}）的逐字 greeting",
            OUT_OF_RANGE_INSPECT,
        )
        bot.assert_alive("inspect 对话链路检查后")
