"""战斗近战链路 —— passive NPC → Bot 左键攻击 → typed hit → production terminal。"""

from __future__ import annotations

import time

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import (
    is_outgoing_positive_hit,
    last_event_time,
    move_to_melee_range,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
    wait_for_target_destroyed,
)

DESCRIPTION = "确定性 passive NPC 上断言左键攻击 typed outgoing hit，并由生产死亡链路精确销毁目标"
MODULES = ["combat", "npc", "network"]


def run(env) -> None:
    with env.new_bot("Atk") as bot:
        wait_for_ready(bot)

        # 清掉上一轮同服复用遗留的 scenario NPC，避免攻击到旧实体导致断言漂移。
        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        move_to_melee_range(bot, spawn)
        target_id = int(spawn.data["entity_id"])

        first_anchor = last_event_time(bot)
        bot.attack_entity(target_id)
        bot.wait_for(
            lambda event: event.t > first_anchor and is_outgoing_positive_hit(event),
            timeout=10.0,
            description="近战命中后本 Bot 的 combat_event hit/outgoing=true/amount>0",
        )

        # passive_target 有固定有限生命；真实伤害会让目标产生协议可见 knockback，
        # 因而每轮都按 Bot 最新观察到的实体坐标重新贴近，不能把首击位置当终局坐标。
        # 每次 C2S 攻击仍必须命中同一协议实体，直至生产
        # NearDeath→Terminated→Despawned 链向客户端发送 entities_destroy。
        terminal_anchor = last_event_time(bot)
        for _ in range(48):
            if bot.entity_pos(target_id) is None:
                break
            time.sleep(0.55)  # 玩家近战 GCD=10 tick；不靠无效 spam 伪造击杀。
            move_to_melee_range(bot, spawn)
            bot.attack_entity(target_id)
        if bot.entity_pos(target_id) is not None:
            raise BotAssertionError(
                f"重复真实近战后 passive target entity_id={target_id} 仍未进入销毁链"
            )
        wait_for_target_destroyed(bot, terminal_anchor, target_id)
        bot.assert_alive("近战 typed hit 与精确 NPC terminal 之后")
