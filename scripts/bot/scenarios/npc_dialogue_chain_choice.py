"""`npc_dialogue_choice` 的协议级回归（对话链路 P1 覆盖）。

黑盒锚点（server/src/network/client_request_handler.rs NpcDialogueChoice 分支）：
- option "inspect" → "§7[NPC] 你端详了一眼 {display_name}。"
- option "trade"：can_trade 才 "摊开了随身货物"；zombie 非商贩 → 落到兜底拒绝。
- 未知 option → "§c[NPC] {display_name} 不愿回应这个选择。"（含 "trade" 对非商贩）。
- option "leave" → 无任何 [NPC] chat 回显（用锚点窗口内 chat 缺席断言）。

zombie（display_name="游尸·醒灵"）由 `/npc_scenario fight` 确定性生成，
不依赖随机播种；商贩分支（"摊开了随身货物"）由 npc_dialogue_chain_trade 的
BOT_E2E_ROGUE_TRADE 阶段承接。
"""

DESCRIPTION = "npc_dialogue_choice：inspect/trade/未知选项逐字回显 + leave 无回显"
MODULES = ["npc", "dialogue"]

from ._npc_dialogue_helpers import (
    OUT_OF_RANGE_CHOICE,
    expect_no_npc_chat_after,
    last_event_time,
    queue_fight_zombie,
    request_and_assert,
)

ZOMBIE_DISPLAY = "游尸·醒灵"
CHOICE_INSPECT = f"§7[NPC] 你端详了一眼 {ZOMBIE_DISPLAY}。"
CHOICE_TRADE_REFUSE = f"§c[NPC] {ZOMBIE_DISPLAY} 不愿回应这个选择。"


def run(env) -> None:
    with env.new_bot("NpcCh") as bot:
        bot.expect_event("game_join", timeout=15.0)
        bot.expect_event("pos_look", timeout=15.0)

        spawn = queue_fight_zombie(bot)
        zombie_id = spawn.data["entity_id"]
        base = {"type": "npc_dialogue_choice", "v": 1, "npc_entity_id": zombie_id}

        request_and_assert(
            bot,
            {**base, "option_id": "inspect"},
            zombie_id,
            CHOICE_INSPECT,
            'choice option="inspect" 的逐字回显',
            OUT_OF_RANGE_CHOICE,
        )
        request_and_assert(
            bot,
            {**base, "option_id": "trade"},
            zombie_id,
            CHOICE_TRADE_REFUSE,
            '非商贩 zombie 对 choice option="trade" 落到兜底拒绝',
            OUT_OF_RANGE_CHOICE,
        )
        request_and_assert(
            bot,
            {**base, "option_id": "dance"},
            zombie_id,
            CHOICE_TRADE_REFUSE,
            '未知 option="dance" 的逐字拒绝',
            OUT_OF_RANGE_CHOICE,
        )

        anchor = last_event_time(bot)
        bot.intent({**base, "option_id": "leave"})
        expect_no_npc_chat_after(
            bot, anchor, 1.5, 'option="leave" 后无 [NPC] chat 回显'
        )
        bot.assert_alive("dialogue choice 链路检查后")
