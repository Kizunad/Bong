"""妖兽近战命中动画链：`/npc_scenario beast_melee` 生成真实 GreenSpider → 贴身近战
命中 bot → 攻击者侧 typed VFX play_entity_anim 精确为 animation.bong.green_spider.attack。

生物动画接线第二阶段任务 1：server `emit_beast_melee_animation_triggers`
（network/vfx_animation_trigger.rs）读已结算 CombatEvent，按攻击者 FaunaVisualKind
表驱动映射到对应 GeckoLib 攻击段。`/npc_scenario fight` 等旧场景生成的是无
FaunaVisualKind 的 zombie 假靴，走不到这张表——必须用真实带 FaunaVisualKind 的
beast_melee 场景才能观察到 play_entity_anim。

依赖：在线服务端 + bot 客户端（`BOT_E2E_*` 环境）。本地无环境时不能直接跑。
"""

from __future__ import annotations

from bot.scenarios._combat_helpers import (
    last_event_time,
    queue_npc_scenario,
    wait_for_ready,
)

DESCRIPTION = "妖兽近战命中：beast_melee 场景生成真实 GreenSpider → typed vfx play_entity_anim"
MODULES = ["combat", "npc", "vfx"]

EXPECTED_ANIM = "animation.bong.green_spider.attack"


def run(env) -> None:
    with env.new_bot("BeastAnim") as bot:
        wait_for_ready(bot)

        # 清掉上一轮同服复用遗留的 scenario NPC，避免观察到旧实体的动画事件。
        queue_npc_scenario(bot, "clear")

        anchor = last_event_time(bot)
        # beast_melee 贴着 bot 当前位置生成（偏移 1 格），不需要额外移动即可进入近战。
        queue_npc_scenario(bot, "beast_melee")

        spawn = bot.wait_for(
            lambda e: e.kind == "entity_spawn"
            and e.t > anchor
            and e.data.get("entity_id") != bot.entity_id,
            timeout=15.0,
            description="/npc_scenario beast_melee 后应出现 GreenSpider entity_spawn",
        )
        spider_id = spawn.data["entity_id"]

        hit_anim = bot.wait_for(
            lambda event: event.kind == "vfx_event"
            and event.t > anchor
            and event.data.get("type") == "play_entity_anim"
            and event.data.get("anim") == EXPECTED_ANIM,
            timeout=30.0,
            description=(
                f"GreenSpider 近战命中 bot 后应广播 typed VFX play_entity_anim 精确为 {EXPECTED_ANIM}"
            ),
        )
        assert hit_anim.data.get("entity_id") == spider_id, (
            f"play_entity_anim.entity_id 应等于生成的 GreenSpider entity_id={spider_id}，"
            f"实际 {hit_anim.data.get('entity_id')}"
        )
        duration = hit_anim.data.get("duration_ticks")
        assert isinstance(duration, int) and duration > 0, (
            f"play_entity_anim.duration_ticks 应为正整数，实际 {duration!r}"
        )

        bot.assert_alive("观察妖兽近战命中动画之后")

        # 清理本轮生成的 scenario NPC，不留给下一个场景。
        queue_npc_scenario(bot, "clear")
