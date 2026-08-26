# plan-bughunt-breakthrough-freeze-factor-align-v1（骨架）

> **骨架（草案）**。一句话主题：把突破失败写入 `qi_max_frozen` 的硬编码 `severity × 10.0` 对齐到 cultivation 既有 canonical `overload::FREEZE_FACTOR = 5.0`，保留 PR #597 已落地的 0.5×`qi_max` cap，并用同 severity 跨来源测试防止冻结系数再次分叉。

## 阶段总览

| 阶段 | 交付物 | 状态 |
|---|---|---|
| P0 | 第一性验真 + 突破失败复用 `FREEZE_FACTOR` + 更新单次精确值测试 | ⬜ |
| P1 | 突破/过载同 severity 对拍 + cap/累积/成功路径饱和回归 | ⬜ |

## 接入面

- **进料**：`server/src/cultivation/breakthrough.rs:503-593` 的 `try_breakthrough_with_profile`（`try_breakthrough` 经 `try_breakthrough_with_env_bonus` / `try_breakthrough_with_env_season_bonus` 包装到此）在扣费并 roll 失败后计算 `severity`、添加 `CrackCause::Backfire` 裂痕；`server/src/cultivation/overload.rs:14-16` 定义 cultivation 当前公开冻结系数 `FREEZE_FACTOR = 5.0`。
- **出料**：两条来源都累计到既有 `Cultivation.qi_max_frozen`；`server/src/cultivation/components.rs:666-675` 的 `recover_current_qi` 与 `server/src/cultivation/tick.rs:200-209` 的回气主循环按 `qi_max - qi_max_frozen` 缩小可用上限。
- **共享类型 / event**：复用 `Cultivation`、`MeridianSystem`、`MeridianCrack`、`CrackCause`、`BreakthroughError::RolledFailure` 与 `overload::FREEZE_FACTOR`；禁止在 breakthrough 再造第二个数值相同但可独立漂移的 freeze constant。
- **跨仓库契约**：纯 server 数值/状态对齐；`qi_max_frozen` 现有 server→agent snapshot schema 形状不变，不改 proto/client/Redis key。
- **worldview 锚点**：`worldview.md §四 L353-L360` 明确过载撕裂会造成经脉裂缝与真元上限永久扣除/临时冻结；`worldview.md §三 L136-L155` 规定突破是修炼主循环。`docs/finished_plans/plan-cultivation-v1.md:323-330` 明确 canonical 公式 `qi_max_frozen += severity × 5.0`。
- **qi_physics 锚点**：本 plan 不移动真元或灵气，只调整可用上限冻结量；突破扣费/zone ledger 路径不得改动。`qi_max_frozen` 是容量约束而非 ledger 账户，禁止把 5.0 误当 qi transfer amount 发起额外转账。

## 第一性验真（`origin/main @ 2310f6fd6d950a865eb15f649cf364994d5f03e9`，2026-07-29）

1. `server/src/cultivation/overload.rs:13-16,43-75` 的检测路径按 `severity * FREEZE_FACTOR` 累计冻结；`server/src/cultivation/overload.rs:140-163` 的 event-reader 路径也使用同一 `FREEZE_FACTOR`，并都 clamp 到 `qi_max * 0.5`。
2. `server/src/cultivation/breakthrough.rs:571-591` 的 `try_breakthrough_with_profile` 对相同含义的 failure `severity` 却硬编码 `severity * 10.0`，是 canonical 5.0 的 2×；该值随后进入同一个 `Cultivation.qi_max_frozen`，不是独立状态或不同单位。
3. PR #597 的 partial fix 仍在：`server/src/cultivation/breakthrough.rs:46-50` 定义 `BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO = 0.5`，失败写入在 `:589-590` 应用 cap。successor 只承接系数漂移，不能重写或删除 cap。
4. 现有测试把旧偏差焊死：`server/src/cultivation/breakthrough.rs:2513-2535` 明确期望 `severity 0.10 × 10.0 = 1.0`；`server/src/cultivation/overload.rs:229-264` 对相同 `severity=0.2` 则期望 `0.2 × 5.0 = 1.0`。
5. 冻结具有 runtime 影响：`server/src/cultivation/tick.rs:200-209` 直接用它缩小 `effective_max/qi_room`，`server/src/cultivation/components.rs:666-675` 也将恢复值 clamp 到有效上限。因此突破失败当前冻结惩罚确实是 canonical 的两倍。

## P0 — 对齐单次突破失败冻结系数

- [ ] 在 `server/src/cultivation/breakthrough.rs` 引用 `super::overload::FREEZE_FACTOR`（或同一模块内等价路径），把 `severity * 10.0` 改为 `severity * FREEZE_FACTOR`；不得复制字面量 5.0 或新增 parallel constant。
- [ ] 同步更新紧邻实现注释和 `single_breakthrough_failure_freezes_qi_within_cap`：基线 success rate 0.90 产生 severity 0.10 时，freeze add 应精确为 0.5，仍低于 50.0 cap。
- [ ] 保留 `BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO = 0.5` 及 `.min(cultivation.qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO)`；不得把本 PR 变成 cap 重构。
- [ ] 不改裂痕目标、integrity 损伤、composure 惩罚、突破成功率、扣费或 roll 顺序。
- [ ] 可核验 symbol：`FREEZE_FACTOR`、`BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`、`single_breakthrough_failure_freezes_qi_within_cap`。

**P0 测试声明**：`cd server && cargo test cultivation::breakthrough::tests::single_breakthrough_failure_freezes_qi_within_cap`；断言需从常量推导期望值并输出 severity/factor/actual，不能换成只判断“> 0”的弱断言。

## P1 — 跨来源一致性与饱和回归

- [ ] 新增 `breakthrough_and_overload_share_freeze_factor`（或同义 greppable 测试），驱动真实 `try_breakthrough` 失败路径与 `apply_meridian_overload_events` event-reader：给两者相同 severity、初始 frozen 与 `qi_max`，断言最终 `qi_max_frozen` 一致；禁止只对拍两段测试内复制的 `severity * FREEZE_FACTOR` 公式而绕过 production 写入路径。
- [ ] 强化 `repeated_breakthrough_failures_frozen_capped_at_half_qi_max`：足量失败后必须精确等于 `qi_max * BREAKTHROUGH_FAIL_FROZEN_CAP_RATIO`（浮点 epsilon 内），并断言有效上限精确保留 `qi_max * (1.0 - ratio)`；不能只用 `frozen <= cap` / `effective > 0` 的弱断言。
- [ ] 强化 `breakthrough_failure_does_not_exceed_cap_when_already_near_cap`：fixture 必须让 `pre-existing frozen + 新增量` 实际跨越 cap（当前 40 + 约 9 只到 49，未越界），并断言结果精确 clamp 到 cap。
- [ ] 保留 `successful_breakthrough_does_not_change_qi_max_frozen`，成功路径不因共享常量引入副作用。
- [ ] 运行 `cultivation::overload::tests`，锁定 detection 与 event-reader 两条路径仍用 factor 5，且同 tick 去重行为不变。
- [ ] 可核验 symbol：`breakthrough_and_overload_share_freeze_factor`、`repeated_breakthrough_failures_frozen_capped_at_half_qi_max`、`breakthrough_failure_does_not_exceed_cap_when_already_near_cap`、`successful_breakthrough_does_not_change_qi_max_frozen`。

**P1 测试声明**：`cd server && cargo test cultivation::breakthrough::tests` 与 `cd server && cargo test cultivation::overload::tests`；两个过滤器都必须实际列出并运行目标模块测试，禁止零测试假绿。最终 server gate 为 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 范围边界

- 不重修 PR #597 已落地的 0.5×cap，不改变 `qi_max_frozen` 持久化/schema/UI 形状。
- 不调整突破 success rate、材料 bonus、真元费用、裂痕 severity 或 composure 代价。
- 不处理 `QiCapPermMinus`、tribulation、QiZeroDecay 等其他上限来源；它们不是本次 10↔5 同源漂移。
- 纯 server 数值对齐，不新增玩家 A/V；玩家可观察差异仅是失败后有效真元上限冻结量从旧 2×偏差回到 canonical。

## §8 开放问题（P0 决策门前需收口）

1. 共享系数是否应继续由 `overload::FREEZE_FACTOR` 所有，还是迁到更中性的 cultivation constants 模块？推荐本窄修直接复用现有公开常量；迁常量会扩大 owner/diff 且不增加行为正确性。
2. 跨来源一致性测试是否值得抽生产纯函数？推荐先在测试夹具中对拍；只有实现出现第三个 freeze producer 时再提升为共享 production helper。
