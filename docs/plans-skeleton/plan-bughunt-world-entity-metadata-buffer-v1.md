# plan-bughunt-world-entity-metadata-buffer-v1（骨架）

> **来源 issue**：#1667。`sync_bong_visual_state_metadata` 每次变化都 `insert_init_value`，把更新重复追加到实体 metadata buffer。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 初始 metadata 与后续 update 使用正确操作，缓冲不会随变化无界增长 | ⬜ |

## §0 摘要

`server/src/world/entity_model.rs` 在视觉状态变化时同时调用 `insert_init_value` 和 append update，导致每次变化都积累一份 init metadata。应区分首次初始化与增量更新，保留客户端可见的最终状态。

## §1 游玩影响

长时间运行或频繁换形的实体会产生越来越大的 metadata buffer，造成同步开销和渲染状态重复；最终状态语义不应改变。

## §2 复现路径

1. 对同一实体连续改变 Bong visual state。
2. 观察 `sync_bong_visual_state_metadata` 每次都 insert init，再 append update。
3. 检查 entity metadata entries 数量随变化增长，而不是保持一份 init 加当前 update。

## §3 今天 `origin/main` 的证据

- `server/src/world/entity_model.rs:526-536` 在 `:534` 每次调用 `insert_init_value`，`:535` 又 append update。

## §4 非重复比对

已查 entity model、client metadata handler、finished/active plans 和 skeleton 的 `sync_bong_visual_state_metadata`、`insert_init_value`；无现有骨架覆盖该缓冲边界。#1667 单独成因明确。

## §5 立项检查记录

- `docs/worldview.md`：查易形/视觉状态相关角色表现；不改正典。
- `docs/finished_plans/`：查 entity model、morph、metadata 同步计划。
- active plan：查 `sync_bong_visual_state_metadata`、metadata buffer、`insert_init_value`，未发现同入口改动。
- skeleton：查 #1667 及 entity metadata 关键词，无同主题骨架。
- `docs/plans-skeleton/reminder.md`：已检查，无 metadata buffer 条目；不改该文件。

## §6 接入面与跨仓契约

- **Inputs**：server entity visual state 的初次创建/增量变化。
- **Outputs**：实体 metadata 初始值与 update packet，最终由 client model/renderer 读取。
- **共享类型/事件**：复用 `sync_bong_visual_state_metadata`、现有 metadata key 和 client entity model metadata；不新增并行字段。
- **三端契约符号**：server `entity_model::sync_bong_visual_state_metadata`；agent **无变更**（实体 metadata 不经过 agent IPC）；client 现有 `BongEntityModelKind`/entity metadata 解析 **无 schema 变更**。
- **worldview 锚点**：角色形态/视觉表现；不涉及 qi_physics。

## P0 验收

- 首次创建恰有 init metadata，后续变化只更新当前值。
- 连续变化后缓冲有界，client 最终渲染状态与最后一次变化一致。
