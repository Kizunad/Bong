"""使用功法：剑技 sword.cleave 施放全反馈（cast_sync+挥动 SFX）+ 经脉门拒因。

黑盒契约面：
- `/technique give sword.cleave`（Awaken、无经脉/真元前置，最易施放的剑技）→
  `skill_bar_bind{slot,binding:{kind:"skill",skill_id}}` → `skill_bar_cast{slot,target}`。
- 施放成功反馈（[[skill-av-diff]] 差异化硬约束）：`bong:audio/play` 含
  `sword_cleave_swing`（PR #1068 劈砍破空声回归锁）+ `bong:vfx_event`。
  **发现记录（2026-07-07 实测）**：sword.* resolver 路径不发 cast_sync
  （generic 技能路径才发）——「skill cast queued/resolver started」只落
  server log，施法进度对 client 不可观察，待补接线。
- 经脉门负分支：`burst_meridian.beng_quan`（Induce + required_meridians 非空）
  未开脉直接 cast → cast_sync 拒（MeridianGated 家族），玩家必须拿到拒因
  而非静默 no-op。
"""

import time

from bot.scenarios._combat_helpers import (
    last_event_time,
    move_to_melee_range,
    payload_text,
    queue_fight_target,
    queue_npc_scenario,
    wait_for_payload_after,
    wait_for_ready,
)

DESCRIPTION = "sword.cleave 施放三反馈（cast_sync+cleave挥动SFX）+ 崩拳未开脉 MeridianGated 拒因"
MODULES = ["cultivation", "combat", "audio"]

SWORD_SKILL = "sword.cleave"
GATED_SKILL = "burst_meridian.beng_quan"


def _bind_and_cast(bot, slot: int, skill_id: str, target_id: int | None) -> float:
    bot.intent(
        {
            "type": "skill_bar_bind",
            "v": 1,
            "slot": slot,
            "binding": {"kind": "skill", "skill_id": skill_id},
        }
    )
    time.sleep(0.3)
    anchor = last_event_time(bot)
    cast = {"type": "skill_bar_cast", "v": 1, "slot": slot}
    if target_id is not None:
        cast["target"] = f"entity:{target_id}"
    bot.intent(cast)
    return anchor


def run(env) -> None:
    with env.new_bot("Cast") as bot:
        wait_for_ready(bot)

        bot.cmd("realm set induce")  # 覆盖 Awaken(sword) 与 Induce(beng_quan) 双前置
        bot.expect_chat("[dev] realm set", timeout=10.0)
        bot.cmd("qi max 20")
        bot.cmd("qi set 20")
        bot.cmd(f"technique give {SWORD_SKILL}")
        bot.expect_chat(f"technique give `{SWORD_SKILL}`", timeout=10.0)
        bot.cmd(f"technique give {GATED_SKILL}")
        bot.expect_chat(f"technique give `{GATED_SKILL}`", timeout=10.0)

        queue_npc_scenario(bot, "clear")
        spawn = queue_fight_target(bot)
        target_id = spawn.data["entity_id"]
        move_to_melee_range(bot, spawn, 1.8)

        # ── 正分支：sword.cleave 音效+vfx 反馈 ────────────────────
        anchor = _bind_and_cast(bot, 0, SWORD_SKILL, target_id)

        wait_for_payload_after(
            bot,
            anchor,
            lambda e: e.data.get("channel") == "bong:audio/play"
            and b"sword_cleave_swing" in e.data.get("data", b""),
            timeout=12.0,
            description=(
                "cleave 施放应触发 bong:audio/play 且配方为 sword_cleave_swing"
                "（PR #1068 挥动破空声回归锁）——无声即 AV 断链"
            ),
        )
        wait_for_payload_after(
            bot,
            anchor,
            lambda e: e.data.get("channel") == "bong:vfx_event",
            timeout=12.0,
            description="cleave 施放应有 bong:vfx_event（剑势视觉反馈）",
        )

        # ── 负分支：beng_quan 未开脉 → cast_sync 拒因 ────────────
        anchor = _bind_and_cast(bot, 1, GATED_SKILL, target_id)
        gated = bot.wait_for(
            lambda e: e.kind == "server_data"
            and e.data["payload_type"] == "cast_sync"
            and e.t > anchor,
            timeout=10.0,
            description=(
                "未开脉施放崩拳应收到 cast_sync 拒绝回执——静默 no-op 是"
                "AGENTS.md §15.2 不可观察红旗"
            ),
        )
        outcome_blob = str(gated.data["payload"])
        assert "eridian" in outcome_blob or "gated" in outcome_blob.lower(), (
            f"崩拳未开脉的 cast_sync 应携带经脉门拒因（MeridianGated 家族），"
            f"实际 payload={outcome_blob[:300]}"
        )
        bot.assert_alive("功法正负分支之后")
