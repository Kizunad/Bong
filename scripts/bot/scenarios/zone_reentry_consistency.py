"""zone 生命周期：离开后重入同一 zone，zone_info 关键状态保持一致。

黑盒断言面（都是 server 生产行为）：
1. 每次 `/tpzone` 真实 zone transition 都广播权威 `zone_info`
2. 离开 zone 再返回后，稳定字段必须与首次一致：
   zone / danger_level / status / active_events 逐字段相等，
   spirit_qi 允许守恒吸纳容差（±1e-3，避免真实灵气波动误报）
   （perception_text 依赖上一个 zone 的对比叙事，不属于稳定状态，不参与比对）

选 zone 硬约束：/tpzone 落点在 `center.y + 24`（cmd/tpzone.rs），必须落在目标 zone
AABB 内才解析为该 zone；y 纵向跨度 <48 的 zone 会解析回 spawn 导致 transition 不触发。

往返：south_ash_dead_zone -> rift_mouth_north_002 -> south_ash_dead_zone。
"""

from __future__ import annotations

from bot.bot import Bot

from ._zone_loot_helpers import (
    assert_zone_reentry_consistent,
    teleport_to_zone,
    wait_join_settled,
    wait_zone_info,
)

DESCRIPTION = "离开 zone 再重入：zone_info 关键状态（zone/danger/status/events/qi）保持一致"
MODULES = ["zone", "network"]

# dead zone 灵气负(-0.007)/danger 5/active_events=['no_cadence']；
# rift 灵气低(0.069)/danger 5。两者 y 纵向跨度均 ≥48，/tpzone 落点在 AABB 内。
REENTER = "south_ash_dead_zone"
RIFT = "rift_mouth_north_002"


def run(env) -> None:
    with env.new_bot("Zre") as bot:
        _scenario(bot)


def _scenario(bot: Bot) -> None:
    # 等 join 出生点初始化完成再传送（否则 tpzone 被 join 位置覆盖）
    wait_join_settled(bot)

    # 首次进入目标 zone，取权威 zone_info
    first_at = teleport_to_zone(bot, REENTER)
    first = wait_zone_info(bot, REENTER, after=first_at)

    # 离开：进入另一 zone（rift），确认 transition 发生
    leave_at = teleport_to_zone(bot, RIFT)
    wait_zone_info(bot, RIFT, after=leave_at)

    # 重入目标 zone，取重入后的 zone_info
    reenter_at = teleport_to_zone(bot, REENTER)
    second = wait_zone_info(bot, REENTER, after=reenter_at)

    assert_zone_reentry_consistent(bot, first, second, REENTER)

    bot.assert_alive("zone 重入一致性校验后")
