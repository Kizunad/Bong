# plan-bughunt-void-erosion-wire-id-fallback-v1（骨架）

> **来源 issue**：#1639。虚蚀 HUD 与模型层对同一 server entity id 使用了不同的 fallback。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | HUD 复用已验证的 `resolveWireId` 映射 | ⬜ |

## §0 摘要

`VoidErosionHudOverlay.render` 直接拼 `offline:` 加 `getEntityName()` 查询 `VoidErosionVisualStore`，而 `VoidErosionModelAlphaRenderer.resolveWireId` 已按精确 key、小写 key 和裸名做 fallback。server offline entity id 的大小写或格式出现差异时，模型 ghost 仍可显示，HUD vignette 却静默消失。

## §1 游玩影响

本地玩家的虚蚀阶段文字和边缘扭曲会与模型表现不一致，玩家无法判断当前阶段；不影响 server 的虚蚀状态或真元结算。

## §2 复现路径

令 server payload 使用 `offline:{Username}`，而 client `getEntityName()` 大小写不同，或 payload 使用裸名；观察 `VoidErosionHudOverlay` 查不到状态，模型 renderer 却能通过 fallback 找到。

## §3 今天 `origin/main` 的证据

- `client/src/main/java/com/bong/client/visual/VoidErosionHudOverlay.java:102-112` 只构造 `offline:` 加本地实体名并调用 `snapshotForEntity`。
- `client/src/main/java/com/bong/client/visual/VoidErosionModelAlphaRenderer.java:123-151` 的 `resolveWireId` 已实现精确、小写和裸名三档 lookup。
- server 的虚蚀 server-data 仍以 entity wire id 作为 key；client `VoidErosionVisualStore` 保存 per-entity 状态。

## §4 非重复比对

已查 void erosion 的 finished/active plans、per-entity lifecycle skeleton 和 store cleanup；它们覆盖状态生命周期，不覆盖 HUD 与模型的 key resolver 复用。#1639 独立保留。

## §5 立项检查记录

- `docs/worldview.md §二 L30-L57`：查负灵域/虚蚀表现；不修改阶段规则。
- `docs/finished_plans/`：查 void erosion visual 与状态清理计划。
- active plan：查 `VoidErosionHudOverlay`、`VoidErosionModelAlphaRenderer`、`VoidErosionVisualStore`，未见统一 lookup 修法。
- skeleton：查 `resolveWireId`、`offline:`、`snapshotForEntity`；无同主题骨架。
- `docs/plans-skeleton/reminder.md`：仓内无该文件。

## §6 接入面与跨仓契约

- **Inputs**：server void-erosion entity id、client 本地实体名和 `VoidErosionVisualStore` snapshot。
- **Outputs**：HUD 与模型使用同一实体状态；找不到身份时 fail closed，不显示伪造本地状态。
- **共享类型/事件**：复用既有 void erosion server-data payload、`VoidErosionVisualStore.State`、`resolveWireId`；不新增字段。
- **三端契约符号**：server void-erosion emit 保持 wire id；agent **无变更**，该视觉状态不经 agent Redis；client `VoidErosionHudOverlay`/`VoidErosionModelAlphaRenderer`。
- **worldview/qi_physics**：表现锚定 `docs/worldview.md §二`；虚蚀资源与真元仍由 server ledger 处理，本骨架不改流动。

## P0 验收

- 精确、大小写差异、裸名三类合法 wire id 在 HUD 与模型中得到同一状态。
- 未知或空身份不串到另一玩家，也不创建新的状态条目。
