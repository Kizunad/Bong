# plan-bughunt-client-render-timing-minor-cleanups-v1（骨架）

> **来源 issue**：#1592、#1644、#1708。三个问题都是客户端渲染/视觉状态把 tick、过期状态或可选资源当成稳定的一次性条件。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 每帧效果按 tick 去重、过期 VFX 可重触发、粒子资源缺失时不入队 | ⬜ |

## §0 摘要

`DroppedItemWorldRenderer` 在每帧回调里用 `world.getTime()` 取模，导致同一 tick 多次生成粒子/音效；`VisualEffectController` 的重触发窗口只看上次开始时间，不先判断旧效果是否已过期；`BreakthroughPillarPlayer` 在 sprite provider 为 null 时仍把无 sprite 粒子加入队列。

## §1 游玩影响

高 FPS 下掉落物特效和音效重复，顿悟金光在短窗口内可能吞掉下一次合法触发，资源初始化时突破光柱可能在渲染线程 NPE。均是 client 表现稳定性问题。

## §2 复现路径

1. 在同一 `world.getTime()` tick 内触发多帧 dropped-item render，观察 rarity 粒子/音效重复。
2. 让 enlightenment flash 持续 1.5 秒后、6 秒窗口内再次到达，观察第二次被 `acceptIncoming` 丢弃。
3. 在 `BongParticles.*Sprites` 尚未初始化时播放 `breakthrough_pillar`，观察粒子仍被加入并在 geometry 读取空 sprite。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/inventory/render/DroppedItemWorldRenderer.java:65-74,124,153-180` 每帧注册 `AFTER_ENTITIES`，并用 `world.getTime()` 的 12/8/80 取模触发效果。
- `client/src/main/java/com/bong/client/visual/VisualEffectController.java:19-28,34-44` 只按 `retriggerWindowMillis` 判定，未调用 `VisualEffectState.isActiveAt`；`VisualEffectProfile.ENLIGHTENMENT_FLASH` 的 duration/retrigger 是 1500/6000ms。
- `client/src/main/java/com/bong/client/visual/particle/BreakthroughPillarPlayer.java:84-89` 只有在 provider 非 null 时设置 sprite，但无论如何都 `addParticle`。

## §4 非重复比对

已查 inventory render、visual effect、particle provider 的 finished/active plans 与 skeleton；没有一份 plan 同时覆盖 frame/timing 去重、过期重触发和资源缺失 fail closed。三条均是客户端渲染边界的小型运行时防护，分别保留回归。

## §5 立项检查记录

- `docs/worldview.md §四 L213-L382`：查战斗反馈、掉落物和突破表现；不改变战斗结算。
- `docs/finished_plans/`：查 inventory visual、visual effects、breakthrough VFX；确认资源与时序约束来自现有 plan。
- active plan：查 `DroppedItemWorldRenderer`、`VisualEffectController`、`BreakthroughPillarPlayer`，未见同一时序 guard。
- skeleton：查 `world.getTime`、`isActiveAt`、`spriteProvider`、`addParticle`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：client render tick/frame、server `visual_effect` payload、`breakthrough_pillar` VFX payload 与 particle registry 状态。
- **Outputs**：每个契约 tick 至多一次本地特效；合法过期后触发可见；缺 sprite 时安全跳过。
- **共享类型/事件**：复用 `WorldRenderEvents.AFTER_ENTITIES`、`VisualEffectState`、`VfxEventPayload.SpawnParticle`、`SpriteProvider`；不新增 wire 字段。
- **三端契约符号**：server `VfxEventRequest`/`breakthrough_pillar` 与 visual effect emit 保持不变；agent **无变更**，表现 payload 不经 agent 推演；client 三个播放器与 controller 改动。
- **worldview/qi_physics**：视觉表现遵循 `docs/worldview.md §四`；突破/掉落的真元副作用仍是 server ledger 权威，本骨架不触碰 transfer。

## P0 验收

- FPS 变化不改变每个 tick 的粒子/音效数量；过期效果在窗口内再次触发成功。
- provider 缺失时不把无 sprite 粒子加入队列，且后续合法事件仍可播放。
