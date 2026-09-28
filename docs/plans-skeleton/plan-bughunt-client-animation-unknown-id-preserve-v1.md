# plan-bughunt-client-animation-unknown-id-preserve-v1（骨架）

> **来源 issue**：#1680。未知动画 ID 不能先取消当前 channel 的合法动画。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 新动画可播放确认成功后才替换旧 channel 动画 | ⬜ |

## §0 摘要

`AnimationLayerManager.playOnStack` 先停止 `byChannel` 中的旧动画，再调用 `BongAnimationPlayer.playOnStack`。资源包缺失或 server 发来未注册 animId 时新播放返回 false，旧动画已被删除，channel 进入空状态。

## §1 游玩影响

一次陈旧或恶意的 `play_anim` payload 就能取消玩家正在举盾等持续动作，直到 server 再次发送恢复动画；不会影响 server 战斗状态，但会让客户端反馈失真。

## §2 复现路径

在 channel 已有合法动画时发送未知 `anim_id`，观察 `stopOnStack` 成功后新 `playOnStack` 失败，`ACTIVE_BY_CHANNEL` 不再保存旧 owner。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/animation/AnimationLayerManager.java:128-163` 在校验新动画可播前调用 `BongAnimationPlayer.stopOnStack`、`byChannel.remove`；随后 `playOnStack` 失败只清理空 map。
- server `server/src/network/vfx_event_emit.rs:44-52`/`VfxEventPayloadV1::PlayAnim` 允许广播 anim id，client `VfxEventRouter` 将其交给动画 manager；资源注册失败是 client 可达路径。

## §4 非重复比对

已查 player animation、VFX schema、finished/active plans 与 skeleton；现有动画优先级/淡入淡出计划没有覆盖“新动画失败时旧动画保留”。#1680 独立保留。

## §5 立项检查记录

- `docs/worldview.md §四 L213-L382`：查战斗动作反馈；不把 client 动画状态当 server 结算。
- `docs/finished_plans/`：查 player animation 与 VFX event 计划。
- active plan：查 `AnimationLayerManager.playOnStack`、`BongAnimationPlayer`、`VfxEventRouter`，未见失败回滚修法。
- skeleton：查 `playOnStack`、`stopOnStack`、`byChannel`、未知 anim；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server `play_anim`/inline anim 的 target UUID、anim id、channel/priority 和 client registry。
- **Outputs**：新动画成功才替换旧 owner；未知 id 只记录/丢弃，不产生额外 server 回执。
- **共享类型/事件**：复用 `VfxEventPayloadV1::PlayAnim`、agent `VfxEventPlayAnimV1`、client `VfxEventPayload.PlayAnim`、`AnimationLayerManager`。
- **三端契约符号**：server `VfxEventRequest`/`vfx_event_emit` 无业务改动；agent **无变更**，schema 只传既有 anim id；client `VfxEventRouter`/`AnimationLayerManager` 增加本地保留语义。
- **worldview/qi_physics**：动作反馈对应 `docs/worldview.md §四`；不改真元、冷却或 server ledger。

## P0 验收

- 已有动画遇到未知/注册失败 anim id 后仍在原 channel 播放。
- 新动画成功时仍按 priority、fade 和 channel 规则替换旧动画；重复未知事件不产生空 owner。
