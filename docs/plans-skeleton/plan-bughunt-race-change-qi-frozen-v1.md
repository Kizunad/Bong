# plan-bughunt-race-change-qi-frozen-v1

> 来源 Issue：#1498。RaceChange 改写 `qi_max` 时没有收敛旧的 `qi_max_frozen`，可能把新角色的有效上限冻结为零。

## §0 摘要

`commit_race_change` 会写入新种族、新 `qi_max` 和 `qi_current`，但没有同步 `qi_max_frozen`。玩家此前因过载或突破失败留下的 frozen 值可能高于新种族上限；`tick.rs` 计算 `effective_max = (qi_max - qi_max_frozen).max(0.0)` 后，真元再生空间恒为零。`qi_zero_decay` 已有把 frozen 收敛到新上限比例的先例，RaceChange 漏掉了同一状态迁移。

## §1 实际游玩体验影响

- 换种族后玩家可能永远无法回复真元，直到另一个系统偶然覆盖 frozen。
- 新种族的 qi 上限显示正常，但有效回复上限为零，客户端难以解释。
- 这是状态元数据迁移问题，不涉及真元凭空转移。

## §2 复现路径

1. 令角色有 `qi_max_frozen` 接近旧 `qi_max`，再选择新 `qi_max` 更低的种族。
2. 运行 `precheck_race_change` 与 `commit_race_change`。
3. 观察 commit 后 `Cultivation.qi_max_frozen` 仍是旧值；下一次 `qi_regen` 计算 effective max 为 0。
4. 对比 `qi_zero_decay.rs:141-145` 的同场景收敛逻辑。

## §3 根因证据

- `server/src/cultivation/race_change.rs:250-276` 的 `commit_race_change` 只写 `race`、`qi_max`、`qi_current`，没有处理 `qi_max_frozen`。
- `server/src/cultivation/tick.rs` 的有效上限计算以 `qi_max - qi_max_frozen` 为准，旧 frozen 大于新 max 时会得到零回复空间。
- `server/src/cultivation/qi_zero_decay.rs:141-145` 已将 frozen 收敛到当前 `qi_max` 的比例，说明 RaceChange 应有明确迁移策略。

## §4 非重复比对

- `plan-bughunt-dugu-baomai-qi-max-shrink-ledger-v1.md` 处理 qi_max 缩容时 excess 的真元去向；本 issue 没有 current 减少，不应混成 ledger 修复。
- `docs/finished_plans/plan-bughunt-race-change-v1.md` 覆盖种族/经脉迁移和 excess 释放，但没有证明 frozen 元数据在 commit 后收敛。
- 不需要 agent/client schema 变更；现有 cultivation snapshot 已有 frozen 字段。

### 立项检查记录（2026-09-28）

- `docs/worldview.md`：检索“种族、境界、真元上限、经脉迁移”，核对 §三 L65-L77 与 §四 L357-L360。
- `docs/finished_plans/`：检索 `commit_race_change`、`qi_max_frozen`、`effective_max`；命中 race system 与 qimax 缩容文档，但没有 RaceChange frozen 收敛证据。
- active plan：检索 `RaceChange`、`qi_max_frozen`、`BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`；`plan-refactor-qi-ledger-v1.md` 只定义字段不变量，未覆盖 commit 漏写。
- `docs/plans-skeleton/`：检索 `qi_max_frozen`、`commit_race_change`、`race_change`；除本文件和无 zone excess 骨架外未发现同主题 skeleton。

## 接入面与跨仓契约

- **Inputs**：`RaceChangeCommitPlan`、`Cultivation { race, qi_max, qi_current, qi_max_frozen }`、新 `MeridianSystem`、`RaceRegistry`。
- **Outputs**：新种族状态、受新上限约束的 frozen 元数据，以及现有 cultivation detail snapshot。
- **共享类型 / 事件**：复用 `RaceChangeCommitPlan`、`Cultivation`、`CultivationDetail`；不新增跨仓事件。
- **server 契约符号**：`precheck_race_change`、`commit_race_change`、`qi_regen_and_zone_drain_tick`、`qi_zero_decay`。
- **agent**：无变更。RaceChange 的状态提交只在 server，agent 不计算有效上限。
- **client**：无变更。客户端继续消费已有 cultivation detail；修复只让既有字段保持一致。
- **worldview 锚点**：`docs/worldview.md §三 L65-L77`（境界与真元上限）；`§四 L357-L360`（经脉/过载代价）。
- **qi_physics**：本 issue 不移动真元，不调用 ledger；若同一 commit 同时处理 qi_max excess，必须另走 `Cultivation::resize_qi_max_and_release_excess`，用 `ReleaseToZone` 和 `SPIRIT_QI_TOTAL` 守恒断言，不得以 frozen 修复掩盖 excess 丢失。

## §5 修复骨架

### P0 元数据收敛

- 在 `RaceChangeCommitPlan` 中预先计算合法的 `qi_max_frozen`，commit 时与新 `qi_max` 原子写入；策略与现有 `qi_zero_decay` 保持一致。
- 新上限为零或旧 frozen 缺失时覆盖边界，避免 effective max 为负/NaN。

### P1 回归

- 覆盖 frozen 缺失、低于新 max、超过新 max、非人形种族和换回原种族。
- 断言下一 tick 的有效上限和 cultivation snapshot 一致，不增加 agent/client 适配。
