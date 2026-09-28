# plan-bughunt-combat-minor-cleanups-v1

> **来源 issue**：#1448、#1393。
> 一句话主题：暗器单射的杂色加成与涡流关闭的客户端状态清理没有共用各自已经存在的权威参数/意图通道。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | SingleSnipe 颜色门控、VortexCasting 关闭意图统一到生产链路 | ⬜ |
| P1 | server 事件与 client 状态快照回归，确认没有改变涡流真元路径 | ⬜ |

## §0 摘要

`anqi_v2::cast_anqi` 已在 `:481-489` 算出会把杂色玩家加成置零的 `color_matched`，并在 `:513-524` 传给 `emit_skill_event`，但 `SingleSnipe` 在 `:604-620` 重新硬编码 `true`，所以杂色玩家仍走高密度注入的匹配倍率。涡流技能条入口 `resolve_woliu_vortex_skill` 在 `woliu.rs:201-206` 调用 `resolve_vortex_toggle_in_world`；其 Off 分支 `:304-312` 只改 server 状态，没有像 `cast_vortex`/`resolve_vortex_toggle_parts` 的 `:402-418` 那样发送 `ApplyStatusEffectIntent` 清除客户端状态。

## §1 立项检查记录

- **worldview**：查 `docs/worldview.md` 中“杂色真元”“暗器”和“绝灵持涡/状态”的关键词；本骨架只修已有约束的接线，不改伤害倍率或涡流物理常量。
- **finished_plans**：查 `docs/finished_plans/` 的 anqi、woliu-v2、combat status 与 VFX 计划，确认已有计划没有同时覆盖这两个生产入口。
- **active plan**：查 `docs/plan-*.md` 的暗器、涡流、状态快照和 combat event 关键词；没有一份把 `resolve_woliu_vortex_skill` 的 Off 清除意图补上。
- **skeleton**：查 `plan-bughunt-carrier-imprint-lifecycle-v1.md`、`plan-bughunt-woliu-v2-state-replacement-v1.md` 和 client 状态清理骨架；它们分别处理载体/场替换，不覆盖本次颜色参数和状态意图遗漏。
- **reminder.md**：查 `docs/plans-skeleton/reminder.md` 的 `SingleSnipe`、`color_matched`、`VortexCasting`、`ApplyStatusEffectIntent`；仓内有该文件，但无这两条生产缺口的登记。

## §2 接入面与跨仓契约

- **Inputs**：`QiColor`、`AnqiSkillId::SingleSnipe`、`emit_skill_event` 的 `color_matched`；技能条 `resolve_woliu_vortex_skill`、`VortexField`、`StatusEffects`、`VortexToggle::Off`。
- **Outputs**：`QiInjectionEvent` 中 SingleSnipe 的 `high_density_inject` 使用权威颜色布尔值；关闭涡流时产生 `ApplyStatusEffectIntent { kind: VortexCasting, magnitude: 0.0, duration_ticks: 0 }`，随后状态快照不再显示“绝灵持涡”。
- **共享类型/事件**：复用 `QiInjectionEvent`、`ApplyStatusEffectIntent`、`StatusEffectKind::VortexCasting`、`StatusSnapshot`；不新增第二套客户端状态协议，也不改变涡流的 `QiTransfer` 维护路径。
- **三端契约符号**：server `combat::anqi_v2::{cast_anqi,emit_skill_event}`、`combat::woliu::{resolve_woliu_vortex_skill,cast_vortex,resolve_vortex_toggle_parts}` 和 `network::status_snapshot_emit::emit_status_snapshot_payloads`；client **无变更**，依据是现有状态快照已把 `VortexCasting` 映射为 `绝灵持涡`，可消费零持续时间清除；agent **无变更**，暗器/涡流是 server↔client gameplay 链路，不经过 Redis IPC。
- **worldview/qi**：颜色门控沿用 `worldview.md` 的杂色约束；本骨架不扣、不铸、不转移真元，守恒入口和 `QiTransferReason` 保持原样。

## §3 游玩影响与复现

1. 给玩家挂 `QiColor { is_chaotic: true }`，施放 `SingleSnipe`，观察 `QiInjectionEvent` 的 wound multiplier 仍按 `true` 计算，而同样颜色判定的 `SoulInject` 走 `false`。
2. 从技能条调用涡流开关再关闭，server 的 `StatusEffects` 已移除 `VortexCasting`，但关闭路径没有清除意图；客户端的状态快照仍可能保留“绝灵持涡”。

## §4 `origin/main` 根因证据

- `server/src/combat/anqi_v2.rs:481-489,513-524` 计算并传递 `color_matched`，但 `:604-620` 的 SingleSnipe 分支把第三个参数固定为 `true`；对照 `:640-655` 的 SoulInject 使用传入值。
- `server/src/combat/woliu.rs:174-206,287-312` 是技能条生产入口和直接 world resolver，Off 只移除 ECS 状态；`server/src/combat/woliu.rs:384-418` 的事件入口才发送关闭 `ApplyStatusEffectIntent`。
- `server/src/network/status_snapshot_emit.rs:164-176,211-220` 已有 `VortexCasting` 的 wire 名称和类别映射，说明 client 端缺的是清除事件，不是新字段。

## §5 非重复比对

`plan-bughunt-carrier-imprint-lifecycle-v1.md` 处理暗器载体归属和充能；`plan-bughunt-woliu-v2-state-replacement-v1.md` 处理场/心诀状态替换；本骨架只吸收两个独立的 minor 接线问题，不重复修改 qi ledger 或客户端 store 生命周期。

## §6 修复计划骨架

- **P0**：SingleSnipe 直接使用 `emit_skill_event` 收到的 `color_matched`；涡流技能条 Off 复用带 `ApplyStatusEffectIntent` 的统一 resolver，或让 world resolver 接受同一事件出口，确保关闭与 `close_vortex_without_backfire` 的 wire 语义一致。
- **P1**：增加杂色/非杂色 SingleSnipe 判别测试；增加技能条关闭后 `ApplyStatusEffectIntent` 为零持续时间且状态快照不再携带 `VortexCasting` 的回归测试；确认维护扣费仍只走原 `QiTransfer` 路径。

## §7 验证计划

实现后只跑 server 栈 fmt、clippy、cargo test，重点 `combat::anqi_v2`、`combat::woliu` 和 `network::status_snapshot_emit`；agent/client 无代码改动，不跑其门禁。
