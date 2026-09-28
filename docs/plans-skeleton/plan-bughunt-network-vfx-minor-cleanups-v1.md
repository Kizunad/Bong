# plan-bughunt-network-vfx-minor-cleanups-v1（骨架）

> **来源 issue**：#1927、#1586。两条都发生在 `server/src/network/vfx_animation_trigger.rs`，但分别是 payload 数值越界和缺坐标时的错误 fallback；合并为同一份 VFX 边界清理计划。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | VFX strength 受 schema 约束，缺 `Position` 时不在原点造粒子 | ⬜ |

## §0 摘要

`emit_tribulation_settled_vfx_triggers` 发送 `strength: Some(1.2)`，而 `VfxEventPayloadV1` 的校验上限不接受该值；多个暗器分支在 caster 没有 `Position` 时 `unwrap_or_default()`，把视觉错误地放到世界原点。修复必须保持 VFX 纯 cosmetic，不改变战斗或真元结算。

## §1 游玩影响

渡劫完成的光柱可能被客户端 schema 丢弃；断线或未加载位置的施法者会在原点看到暗器粒子，造成跨玩家、跨区域的错误视觉。影响 server→client 视觉链路，agent 无 gameplay 状态变化。

## §2 复现路径

1. 触发 `TribulationSettled` 的 `Ascended`/`HalfStep` 分支，观察 `bong:breakthrough_pillar` payload 的 `strength=1.2`。
2. 为 `QiInjectionEvent`/`MultiShotEvent` 等暗器事件构造没有 `Position` 的 caster。
3. 观察第一条被校验丢弃，第二条仍生成 `[0,0,0]` 原点粒子，而不是跳过该效果。

## §3 今天 `origin/main` 的证据

- `server/src/network/vfx_animation_trigger.rs:528-538` 在 `SpawnParticle` 中写死 `strength: Some(1.2)`。
- `server/src/network/vfx_animation_trigger.rs:965-1170` 的暗器视觉分支用 `positions.get(...).map(...).unwrap_or_default()`。
- `server/src/schema/vfx_event.rs:181-229` 对 `SpawnParticle.strength` 做有限性和 `[0,1]` 范围校验；VFX 事件经 `bong:vfx_event` 交给客户端注册的 player。

## §4 非重复比对

两条 issue 均来自 #1927、#1586；已检查 `docs/plans-skeleton/`、`docs/plan-*.md`、`docs/finished_plans/` 的 `breakthrough_pillar`、`unwrap_or_default`、`VfxEventRequest`，没有覆盖这两个边界的现有骨架。该计划只改 VFX 发射边界，不重复任何 combat 或 qi plan。

## §5 立项检查记录

- `docs/worldview.md`：查 `六境界`、渡劫视觉和 `真元` 关键词；本计划不改正典或真元。
- `docs/finished_plans/`：查 `plan-vfx-v1`、`breakthrough_pillar`、`AnqiVfxPlayer`，确认复用现有通道。
- active plan：查 `docs/plan-*.md` 中 `VfxEventRequest`、`VfxEventPayloadV1`，未发现同一修复入口。
- skeleton：查 `vfx_animation_trigger`、`bong:vfx_event`、#1927、#1586，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，未找到这两个 issue 的条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：server 读取 `TribulationSettled`、暗器结果事件和可选 `Position`。
- **Outputs**：server 发 `VfxEventRequest` 的 `VfxEventPayloadV1::SpawnParticle`，经 `bong:vfx_event`；缺坐标时跳过该粒子。
- **共享类型/事件**：复用 `VfxEventRequest`、`VfxEventPayloadV1`、`PlayerAnimTargetItem`，不新造 event。
- **三端契约符号**：server `emit_tribulation_settled_vfx_triggers` / `emit_anqi_visual_triggers`；agent **无变更**（该通道不是 Redis agent 业务 payload）；client `VfxBootstrap`、`AnqiVfxPlayer`、`BreakthroughPillarPlayer` 消费同一 event id。
- **worldview 锚点**：`docs/worldview.md` 的六境界/渡劫表现；本计划不增加物理常数，也不触碰 `qi_physics`。

## P0 验收

- schema 接受范围内的 strength 才能发出，渡劫光柱不再发送 1.2。
- caster 缺 `Position` 时没有原点粒子；有坐标时 origin 与 caster 坐标一致。
- 视觉修复不写入战斗、Cultivation 或 ledger。
