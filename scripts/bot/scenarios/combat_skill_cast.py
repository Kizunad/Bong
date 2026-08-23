"""战斗技能链路 —— dev 铺垫 → skill_bar_bind/cast intent → 凝针专属反馈。"""

from __future__ import annotations

import json
import math

from bot.bot import BotAssertionError
from bot.scenarios._combat_helpers import (
    is_outgoing_positive_hit,
    last_event_time,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_ready,
    wait_for_skill_binding,
)

# CI 上这些等的是 server 对具体请求的响应，而不是本地计算：实测挂掉那次进程里已经
# 收了 3000~15000 条事件，10s 窗口在满载 runner 上不够用（同一 commit 重跑就换了个
# 场景挂）。放宽只在**失败**路径上多花时间——成功时 wait_for 收到事件立刻返回，
# 绿色用例的墙钟不变。
SERVER_RESPONSE_TIMEOUT = 25.0

DESCRIPTION = "/technique give 后用 skill_bar_bind/cast 施放 dugu.shoot_needle，并断言权威绑定、cast_sync、专属 VFX/战斗反馈"
MODULES = ["combat", "skill", "network", "cmd"]

SKILL_ID = "dugu.shoot_needle"
SKILL_ICON = "bong-client:textures/gui/items/skill_scroll_dugu_shoot_needle.png"
ANIMATION_ID = "bong:dugu_needle_throw"
PARTICLE_ID = "bong:dugu_needle_bolt"
AUDIO_RECIPE_ID = "dugu_cast"
AUDIO_FLAG = "dugu_shoot_needle"
SLOT = 0


def _wait_authoritative_qi_state(
    bot, anchor: float, expected_qi: float, expected_qi_max: float
):
    """等待 dev 命令后的精确修炼状态。

    共享修炼 helper 有意拒绝 ``qi_max == 0``，因为正常玩法不会暴露零容量玩家；
    本 dev-only 场景用零值构造真元不足夹具，因此在此保留独立谓词，但仍以 typed
    ``player_state`` payload 作为权威状态。
    """

    def matches(event) -> bool:
        if (
            event.kind != "server_data"
            or event.t <= anchor
            or event.data.get("payload_type") != "player_state"
        ):
            return False
        payload = event.data.get("payload")
        if not isinstance(payload, dict):
            return False
        qi = payload.get("spirit_qi")
        qi_max = payload.get("spirit_qi_max")
        if not (
            isinstance(qi, (int, float))
            and not isinstance(qi, bool)
            and isinstance(qi_max, (int, float))
            and not isinstance(qi_max, bool)
        ):
            return False
        qi_value = float(qi)
        qi_max_value = float(qi_max)
        return (
            math.isfinite(qi_value)
            and math.isfinite(qi_max_value)
            and abs(qi_value - expected_qi) <= 0.01
            and abs(qi_max_value - expected_qi_max) <= 0.01
        )

    return bot.wait_for(
        matches,
        timeout=SERVER_RESPONSE_TIMEOUT,
        description=(
            "t>%.3fs 后权威 player_state.spirit_qi=%.2f, "
            "spirit_qi_max=%.2f" % (anchor, expected_qi, expected_qi_max)
        ),
    )


def _is_dugu_audio_play(event, anchor: float) -> bool:
    if (
        event.kind != "payload"
        or event.t <= anchor
        or event.data.get("channel") != "bong:audio/play"
    ):
        return False
    raw = event.data.get("data")
    if not isinstance(raw, bytes):
        return False
    try:
        payload = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return False
    return (
        isinstance(payload, dict)
        and payload.get("v") == 1
        and payload.get("recipe_id") == AUDIO_RECIPE_ID
        and payload.get("flag") == AUDIO_FLAG
    )


def _assert_binding_feedback(bot, event) -> None:
    slot = event.data["payload"]["slots"][SLOT]
    if slot.get("icon_texture") != SKILL_ICON:
        raise BotAssertionError(
            f"[{bot.username}] 凝针槽位 icon_texture 漂移：期望 {SKILL_ICON!r}，"
            f"实际 {slot.get('icon_texture')!r}"
        )


def _wait_successful_cast_sequence(bot, anchor: float):
    casting = bot.wait_for(
        lambda event: event.kind == "server_data"
        and event.t > anchor
        and event.data.get("payload_type") == "cast_sync"
        and event.data.get("payload", {}).get("slot") == SLOT
        and event.data.get("payload", {}).get("phase") == "casting"
        and event.data.get("payload", {}).get("outcome") == "none",
        timeout=SERVER_RESPONSE_TIMEOUT,
        description="凝针施放须先收到 slot=0 phase=casting outcome=none 的 typed cast_sync",
    )

    seen_casting = False

    def is_complete_after_casting(event) -> bool:
        nonlocal seen_casting
        if event is casting:
            seen_casting = True
            return False
        return (
            seen_casting
            and event.kind == "server_data"
            and event.data.get("payload_type") == "cast_sync"
            and event.data.get("payload", {}).get("slot") == SLOT
            and event.data.get("payload", {}).get("phase") == "complete"
            and event.data.get("payload", {}).get("outcome") == "completed"
        )

    return bot.wait_for(
        is_complete_after_casting,
        timeout=SERVER_RESPONSE_TIMEOUT,
        description="凝针施放须终止于 slot=0 phase=complete outcome=completed",
    )


def run(env) -> None:
    with env.new_bot("Cast") as bot:
        wait_for_ready(bot)

        # 独孤凝针要求引气境和至少 1 真元；这些 dev 命令只做 bot e2e 铺垫。
        bot.cmd("realm set induce")
        bot.expect_chat("[dev] realm set", timeout=SERVER_RESPONSE_TIMEOUT)
        bot.cmd("qi max 20")
        bot.expect_chat("[dev] qi max", timeout=SERVER_RESPONSE_TIMEOUT)
        bot.cmd("qi set 10")
        bot.expect_chat("[dev] qi set", timeout=SERVER_RESPONSE_TIMEOUT)
        bot.cmd(f"technique give {SKILL_ID}")
        bot.expect_chat(f"[dev] technique give `{SKILL_ID}`", timeout=SERVER_RESPONSE_TIMEOUT)

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
        binding = wait_for_skill_binding(bot, bind_anchor, SLOT, SKILL_ID)
        _assert_binding_feedback(bot, binding)

        # 先锁定真元上限再清零，避免修炼恢复 tick 在施法解析前回填真元。
        max_zero_anchor = last_event_time(bot)
        bot.cmd("qi max 0")
        bot.expect_chat("[dev] qi max", timeout=SERVER_RESPONSE_TIMEOUT)
        _wait_authoritative_qi_state(bot, max_zero_anchor, 0.0, 0.0)
        set_zero_anchor = last_event_time(bot)
        bot.cmd("qi set 0")
        bot.expect_chat("[dev] qi set", timeout=SERVER_RESPONSE_TIMEOUT)
        _wait_authoritative_qi_state(bot, set_zero_anchor, 0.0, 0.0)
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
            timeout=SERVER_RESPONSE_TIMEOUT,
            description="真元清零后凝针须 typed cast_sync 明确拒绝为 reject_qi_insufficient",
        )

        # 拒绝分支不应写入 cooldown；补足真元后再走正分支，避免先成功施放时
        # resolver 按 OnCooldown→QiInsufficient 的既定门顺序遮住目标拒绝证据。
        max_restore_anchor = last_event_time(bot)
        bot.cmd("qi max 20")
        bot.expect_chat("[dev] qi max", timeout=SERVER_RESPONSE_TIMEOUT)
        _wait_authoritative_qi_state(bot, max_restore_anchor, 0.0, 20.0)
        set_restore_anchor = last_event_time(bot)
        bot.cmd("qi set 10")
        bot.expect_chat("[dev] qi set", timeout=SERVER_RESPONSE_TIMEOUT)
        _wait_authoritative_qi_state(bot, set_restore_anchor, 10.0, 20.0)
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
            and event.data.get("type") == "play_anim"
            and event.data.get("anim_id") == ANIMATION_ID,
            timeout=SERVER_RESPONSE_TIMEOUT,
            description=(
                "凝针 skill cast 后 typed VFX type=play_anim 且 anim_id 精确等于 "
                f"{ANIMATION_ID}"
            ),
        )
        bot.wait_for(
            lambda event: event.kind == "vfx_event"
            and event.t > anchor
            and event.data.get("type") == "spawn_particle"
            and event.data.get("event_id") == PARTICLE_ID,
            timeout=SERVER_RESPONSE_TIMEOUT,
            description=(
                "凝针 skill cast 后 typed VFX type=spawn_particle 且 event_id 精确等于 "
                f"{PARTICLE_ID}"
            ),
        )
        bot.wait_for(
            lambda event: _is_dugu_audio_play(event, anchor),
            timeout=SERVER_RESPONSE_TIMEOUT,
            description=(
                "凝针 skill cast 后 bong:audio/play JSON 须同时匹配 "
                f"recipe_id={AUDIO_RECIPE_ID} 与 flag={AUDIO_FLAG}"
            ),
        )
        bot.wait_for(
            lambda event: event.t > anchor and is_outgoing_positive_hit(event),
            timeout=SERVER_RESPONSE_TIMEOUT,
            description="凝针 skill cast 后本 Bot 的 combat_event hit/outgoing=true/amount>0",
        )
        bot.assert_alive("技能栏施放凝针真元不足拒绝分支与正分支之后")
