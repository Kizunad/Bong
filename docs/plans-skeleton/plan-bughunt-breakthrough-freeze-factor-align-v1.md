# plan-bughunt-breakthrough-freeze-factor-align-v1（骨架）

> 一句话主题：收口突破失败与过载系统的 `qi_max_frozen` 系数语义，解决突破路径 `severity × 10` 与 canonical overload `FREEZE_FACTOR = 5` 的 2× 规格漂移。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 数值决策：共享 `FREEZE_FACTOR` 或声明突破专属系数 | ⬜ |
| P1 | 统一常数归属/公式 + 边界矩阵 | ⬜ |
| P2 | server gate + 平衡回归 | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/breakthrough.rs` 失败结算、`server/src/cultivation/overload.rs::FREEZE_FACTOR`、`Cultivation.qi_max_frozen`。
- **出料**：有效 qi 上限与现有恢复/突破 HUD 路径；不新增状态字段。
- **共享类型 / event**：复用 `BreakthroughAttempt`/`Cultivation` 与 overload 常数；若语义必须独立，常数仍归 cultivation/qi 物理统一位置并写清理由。
- **跨仓库契约**：无 wire 形状变化。
- **worldview 锚点**：突破失败有代价但不得永久废人；六境界与 qi 上限语义不变。
- **qi_physics 锚点**：冻结只改变可用上限，不铸造/销毁 qi；不得把差额直接写入或移出 ledger。

## 当前证据（origin/main @ c625d5a5）

- `server/src/cultivation/breakthrough.rs:586-591` 仍把失败严重度乘 `10.0`。
- `server/src/cultivation/overload.rs:16,64` 的 canonical overload 冻结因子为 `FREEZE_FACTOR = 5.0`。
- PR #597 / commit `6db5b7d51` 已加入 `qi_max × 0.5` cap，因此“永久冻结到零”已修；当前剩余 finding 仅为两个路径的 2× 规格漂移。

## 验收

1. P0 对拍历史 plan/worldview，明确两个事件是否同一物理量；不得直接把 10 改 5 而无决议。
2. 覆盖 severity 最小/最大、连续失败到 cap、已有 frozen、成功突破不改 frozen、NaN/非法 severity 防线。
3. 测试引用常数而非复制字面值，并证明 cap 仍为 `qi_max × 0.5`。
4. 运行完整 server gate。

## 边界

- 不重新修 PR #597 已闭环的 cap。
- 不调整各境界 qi_max、突破成功率、材料加成或 overload 其他惩罚。
