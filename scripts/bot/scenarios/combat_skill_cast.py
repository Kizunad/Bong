"""战斗技能链路 —— dev 铺垫 → skill_bar_bind/cast intent → 凝针专属反馈。"""

from __future__ import annotations

from bot.scenarios._combat_helpers import (
    is_outgoing_positive_hit,
    last_event_time,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
    wait_for_skill_binding,
)

DESCRIPTION = "/technique give 后用 skill_bar_bind/cast 施放 dugu.shoot_needle，并断言权威绑定、cast_sync、专属 VFX/战斗反馈"
MODULES = ["combat", "skill", "network", "cmd"]

SKILL_ID = "dugu.shoot_needle"
SLOT = 0


def _wait_successful_cast_sequence(bot, anchor: float):
    casting = bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > anchor
        and event.data.get("payload_type") == "cast_sync"
        and event.data.get("payload", {}).get("slot") == SLOT
        and event.data.get("payload", {}).get("phase") == "casting"
        and event.data.get("payload", {}).get("outcome") == "none",
        timeout=10.0,
        description="凝针施放须先收到 slot=0 phase=casting outcome=none 的 typed cast_sync",
    )
    return bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > casting.t
        and event.data.get("payload_type") == "cast_sync"
        and event.data.get("payload", {}).get("slot") == SLOT
        and event.data.get("payload", {}).get("phase") == "complete"
        and event.data.get("payload", {}).get("outcome") == "completed",
        timeout=10.0,
        description="凝针施放须终止于 slot=0 phase=complete outcome=completed",
    )


def run(env) -> None:
    with env.new_bot("Cast") as bot:
        wait_for_ready(bot)

        # 独孤凝针要求引气境和至少 1 真元；这些 dev 命令只做 bot e2e 铺垫。
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.cmd("qi max 20")
        bot.expect_chat("[dev] qi max", timeout=10.0)
        bot.cmd("qi set 10")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        bot.cmd(f"technique give {SKILL_ID}")
        bot.expect_chat(f"[dev] technique give `{SKILL_ID}`", timeout=10.0)

        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)

        bind_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_bar_bind",
                "v": 1,
                "slot": SLOT,
                "binding": {"kind": "skill", "skill_id": SKILL_ID},
            }
        )
        wait_for_skill_binding(bot, bind_anchor, SLOT, SKILL_ID)

        bot.cmd("qi set 0")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        reject_anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_bar_cast",
                "v": 1,
                "slot": SLOT,
                "target": f"entity:{spawn.data['entity_id']}",
            }
        )
        bot.wait_for(
            lambda event: event.kind == "server_data"
            and event.t > reject_anchor
            and event.data.get("payload_type") == "cast_sync"
            and event.data.get("payload", {}).get("slot") == SLOT
            and event.data.get("payload", {}).get("phase") == "idle"
            and event.data.get("payload", {}).get("outcome") == "reject_qi_insufficient",
            timeout=10.0,
            description="真元清零后凝针须 typed cast_sync 明确拒绝为 reject_qi_insufficient",
        )

        # 拒绝分支不应写入 cooldown；补足真元后再走正分支，避免先成功施放时
        # resolver 按 OnCooldown→QiInsufficient 的既定门顺序遮住目标拒绝证据。
        bot.cmd("qi set 10")
        bot.expect_chat("[dev] qi set", timeout=10.0)
        anchor = last_event_time(bot)
        bot.intent(
            {
                "type": "skill_bar_cast",
                "v": 1,
                "slot": SLOT,
                "target": f"entity:{spawn.data['entity_id']}",
            }
        )

        _wait_successful_cast_sequence(bot, anchor)
        bot.wait_for(
            lambda event: event.kind == "vfx_event"
            and event.t > anchor
            and event.data.get("event_id") == "bong:dugu_needle_bolt",
            timeout=10.0,
            description="凝针 skill cast 后 typed VFX event_id 精确等于 bong:dugu_needle_bolt",
        )
        bot.wait_for(
            lambda event: event.t > anchor and is_outgoing_positive_hit(event),
            timeout=10.0,
            description="凝针 skill cast 后本 Bot 的 combat_event hit/outgoing=true/amount>0",
        )
        bot.assert_alive("技能栏施放凝针真元不足拒绝分支与正分支之后")
