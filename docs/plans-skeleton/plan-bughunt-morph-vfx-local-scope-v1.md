# plan-bughunt-morph-vfx-local-scope-v1（骨架）

> **来源 issue**：#1403、#1550、#1737。附近玩家的 VFX 不应把本地 HUD 状态当成施法者状态。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 易形 vignette 和爆脉 HUD 只对本地施法者生效 | ⬜ |
| P1 | payload 身份契约与旁观者回归 | ⬜ |

## §0 摘要

服务端把 `bong:morph_yixing` 以及爆脉视觉事件按 64 格广播，`SpawnParticle` 目前只有事件 ID、坐标和表现字段，没有施法者身份。客户端因此对收到的易形事件无条件 `MorphCastVignetteState.trigger`，爆脉 HUD 只用 1.5 格距离猜测 origin 是本地玩家；旁观者可能被他人的全屏 vignette 或 buff HUD 污染。

## §1 游玩影响

玩家在旁观他人易形时会闪白屏，站在他人爆脉施法点旁会看到自己并未获得的焚血、凡躯重铸或经脉龟裂状态。粒子本身可以继续对附近观众播放，但本地专属 HUD 必须隔离。

## §2 复现路径

1. 两名玩家相距 64 格以内，让一方施放易形；观察另一方 `MorphVfxPlayer.play` 触发全屏 tint。
2. 让第三方站在施法者 1.5 格内但不是施法者，观察 `BaomaiV3VfxPlayer.shouldUpdateLocalHud` 记录本地 HUD 状态。

## §3 今天 `origin/main` 的证据

- `server/src/network/vfx_event_emit.rs:32,115-120,391` 对普通粒子使用 `VFX_BROADCAST_RADIUS=64.0`；`server/src/schema/vfx_event.rs:136-163` 的 `SpawnParticle` 没有 caster/target 字段。
- `client/src/main/java/com/bong/client/visual/particle/MorphVfxPlayer.java:48-51` 收到粒子后无条件调用 `MorphCastVignetteState.trigger`；`MorphHudPlanner.java:38-46` 只按全局 alpha 绘制 tint。
- `client/src/main/java/com/bong/client/visual/particle/BaomaiV3VfxPlayer.java:31-48,51-70` 在记录三类 HUD 前只调用按坐标距离 `<=2.25` 的 `isLocalPlayerOrigin`，无法区分附近的 NPC/另一名玩家。

## §4 非重复比对

已查 `plan-race-system-v1`、`plan-combat-skill-feedback-bridges-v1`、finished/active plan 和 VFX skeleton；它们定义了粒子外观与 server 广播，却没有本地身份筛选。#1403/#1550 是同一 morph vignette 根因，#1737 是同一 payload 身份缺口在爆脉 HUD 的表现，合并处理。

## §5 立项检查记录

- `docs/worldview.md §四 L213-L382`：查战斗反馈、异体排斥和施法表现；不把旁观者误报成施法者。
- `docs/finished_plans/`：查 race VFX、baomai-v3 和 vfx-event schema；确认现有动画视觉规格不提供身份门。
- active plan：查 `VfxEventPayload.SpawnParticle`、`MorphVfxPlayer`、`BaomaiV3VfxPlayer`，未见 caster scope 修复。
- skeleton：查 `VFX_BROADCAST_RADIUS`、`MorphCastVignetteState`、`shouldUpdateLocalHud`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `VfxEventRequest` 的 event ID/origin 与本地玩家身份；现有粒子表现字段继续保留。
- **Outputs**：附近客户端仍可渲染公共粒子；只有 caster 匹配时更新 `MorphCastVignetteState`/`BaomaiV3HudStateStore`。
- **共享类型/事件**：复用 `VfxEventPayloadV1::SpawnParticle`、agent `VfxEventSpawnParticleV1`、client `VfxEventPayload.SpawnParticle` 与 `VfxEventRouter`；实施时若增加身份字段，必须同步这三份 schema/parser，或改用已有面向目标的事件。
- **三端契约符号**：server `vfx_event_emit::VfxEventRequest`/`VFX_BROADCAST_RADIUS`；agent **无变更**，当前 VFX 不经 agent Redis 推演；client `MorphVfxPlayer`、`BaomaiV3VfxPlayer`、`MorphCastVignetteState`。
- **worldview/qi_physics**：表现对应 `docs/worldview.md §四` 的战斗/变形反馈；真元消耗和爆脉结算仍走 server `qi_physics` ledger，本骨架不改数值。

## P0/P1 验收

- 非本地 caster 收到粒子时不触发本地 vignette 或 buff HUD。
- 本地 caster 仍得到原有反馈，远端公共粒子不被错误屏蔽。
- payload 身份缺失或无法核验时 fail closed，只保留公共表现。
