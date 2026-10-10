"""妖兽近战出手动画链：`/npc_scenario beast_melee` 生成真实 GreenSpider → 贴身对 bot
发起近战 → 攻击者侧 typed VFX play_entity_anim 精确为 animation.bong.green_spider.attack。

生物动画接线第二阶段任务 1：server `emit_beast_melee_animation_triggers`
（network/vfx_animation_trigger.rs）读 `AttackIntent`（出手时就发，不等
`resolve_attack_intents` 判出命中结果——挥空也该有挥击动作），按攻击者
FaunaVisualKind 表驱动映射到对应 GeckoLib 攻击段。`/npc_scenario fight` 等旧场景
生成的是无 FaunaVisualKind 的 zombie 假靴，走不到这张表——必须用真实带
FaunaVisualKind 的 beast_melee 场景才能观察到 play_entity_anim。

本场景只断言"出手就 emit"，不依赖 NPC 对玩家近战的几何命中判定是否真的判成功——
后者是另一个独立的生产 bug（NPC 攻击者 raycast 稳定 miss，详见调度分发队列记录），
不在本接线范围内，也不应该让这条动画回归测试被它拖累变 flaky。

依赖：在线服务端 + bot 客户端（`BOT_E2E_*` 环境）。本地无环境时不能直接跑。
"""

from __future__ import annotations

from bot.scenarios._combat_helpers import (
    last_event_time,
    queue_npc_scenario,
    wait_for_ready,
)

DESCRIPTION = "妖兽近战出手：beast_melee 场景生成真实 GreenSpider → typed vfx play_entity_anim（出手即播，不要求命中）"
MODULES = ["combat", "npc", "vfx"]

EXPECTED_ANIM = "animation.bong.green_spider.attack"


def run(env) -> None:
    with env.new_bot("BeastAnim") as bot:
        wait_for_ready(bot)

        # 清掉上一轮同服复用遗留的 scenario NPC，避免观察到旧实体的动画事件。
        queue_npc_scenario(bot, "clear")

        # 主体从这里开始可能 wait/assert 失败；清理必须进 finally，否则异常会跳过
        # 下面的收尾 queue_npc_scenario(bot, "clear")，把本轮生成的 GreenSpider
        # 残留给下一个场景（Kody 行内意见，PR #2410）。
        primary_error: BaseException | None = None
        try:
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

            # 出手即播：GreenSpider 的 HuntAction 发 AttackIntent 后立刻 emit，不等
            # resolve_attack_intents 判出是否真的命中——所以这里不需要很长的等待窗口。
            attack_anim = bot.wait_for(
                lambda event: event.kind == "vfx_event"
                and event.t > anchor
                and event.data.get("type") == "play_entity_anim"
                and event.data.get("anim") == EXPECTED_ANIM,
                timeout=10.0,
                description=(
                    f"GreenSpider 对 bot 出手近战后应广播 typed VFX play_entity_anim 精确为 {EXPECTED_ANIM}"
                ),
            )
            assert attack_anim.data.get("entity_id") == spider_id, (
                f"play_entity_anim.entity_id 应等于生成的 GreenSpider entity_id={spider_id}，"
                f"实际 {attack_anim.data.get('entity_id')}"
            )
            duration = attack_anim.data.get("duration_ticks")
            assert isinstance(duration, int) and duration > 0, (
                f"play_entity_anim.duration_ticks 应为正整数，实际 {duration!r}"
            )

            bot.assert_alive("观察妖兽近战出手动画之后")
        except BaseException as error:
            primary_error = error
            raise
        finally:
            # 清理本轮生成的 scenario NPC，不留给下一个场景；主体异常优先保留，
            # 清理失败时把原因挂到主体异常的 note 上而不是吞掉主体异常。
            try:
                queue_npc_scenario(bot, "clear")
            except BaseException as cleanup_error:
                if primary_error is not None and cleanup_error is not primary_error:
                    primary_error.add_note(f"scenario NPC 清理失败: {cleanup_error}")
                elif primary_error is None:
                    raise
