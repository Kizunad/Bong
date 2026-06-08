# Bong · plan-halfstep-buff-calibration-v1 · active

半步化虚 **buff 强度运营校准**——承接 `plan-halfstep-buff-v1` P1 首期占位值（`HALFSTEP_QI_MAX_BONUS = 0.10` / `HALFSTEP_LIFESPAN_BONUS_YEARS = 200.0`），在积累 ≥ 4 周运营数据后，依据遥测指标对 buff 常数做数据驱动的调整，保证半步化虚"有意义但不等同化虚"的 worldview 定位。

**前置条件**（全部满足才启动）：
- `plan-halfstep-buff-v1` ✅ merge（buff const + `TribulationMetrics` 遥测已落）
- `plan-halfstep-rechallenge-integration-v1` ✅ merge（重渡全链路闭合，遥测数据完整）
- 服务器积累 **≥ 4 周**运营数据，且满足以下任一观测阈值（决策门）：
  - `halfstep_stuck_duration_ticks`（半步修士平均滞留）> 服务器预期寿元的 30%——buff 过弱，修士"躺"在半步不想重渡
  - `halfstep_stuck_duration_ticks` < 服务器预期寿元的 5%——buff 过强，几乎无人等待名额，稀缺性失效
  - 运营数据显示 `ascension_quota_full_duration_ticks / total_ticks` > 80%（名额长期满，半步修士无出路）
  - 用户反馈"半步化虚 buff 感知不到"或"半步化虚比化虚还香"

**交叉引用**：`plan-halfstep-buff-v1.md` ✅（`HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` / `TribulationMetrics` / `/tribulation_debug` 命令）· `plan-tribulation-v1.md` ✅（化虚 buff 基线，校准时需对比）· `plan-cultivation-v1.md` ✅（`qi_max` / `lifespan_max` 字段范围）· `plan-qi-physics-v1.md` P1 ✅（守恒律——buff 修改 qi_max 走 ledger 标记）

**worldview 锚点**：
- **§三:78 化虚稀缺性**：半步化虚 buff 必须"有吸引力但不等同化虚"——校准目标是保持两者之间的质量差距（化虚 qi_max ≥ 1.5-3×，半步只是 1.1× 附近微调）
- **§三:124 NPC 与玩家平等**：buff 数值变更同时影响 dormant NPC，校准时需验证 NPC 半步修士的数值不会产生异常
- **§十:1013 寿元节奏**：`HALFSTEP_LIFESPAN_BONUS_YEARS = 200` 对应"多活半辈子"的直觉；校准不应打破"通灵约 500 年 → 半步 +200 = 700 年，仍远低于化虚 2000 年"的比例关系

**qi_physics 锚点**：
- buff 写入 `qi_max` 时走 `qi_physics::ledger::QiTransfer { reason: QiTransferReason::HalfStepBuff }`（audit-only，不动 balance）
- **校准时禁止新增物理常数**：只改 `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` 两个 const，其余路径不动

**前置依赖**：
- `plan-halfstep-buff-v1` ✅ — buff const + 遥测基础
- `plan-halfstep-rechallenge-integration-v1` ✅ — 重渡链路闭合后遥测才完整

**反向被依赖**：
- 无（校准是终态 plan）

---

## 接入面 Checklist

- **进料**：`/tribulation_debug` 命令遥测数据（`TribulationMetrics` / `QuotaFullTracker` / `halfstep_stuck_duration_ticks`）+ 运营日志（`halfstep_count` / `ascended_count` / quota 满时长占比）
- **出料**：更新后的 `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` const 值（`server/src/cultivation/tribulation.rs:107-108`）+ 回归测试全绿
- **共享类型**：无新增；const 修改后测试断言必须引用 const（禁止字面 0.10 / 200.0，plan-halfstep-buff-v1 P1 已强调）
- **跨仓库契约**：纯 server const 修改，agent / client 无需联动；但需通知运营"buff 数值变更"以更新游戏文档
- **worldview 锚点**：§三:78 稀缺性 + §十 寿元节奏
- **qi_physics 锚点**：ledger reason `HalfStepBuff` 不变，只改 amount（即 `qi_max * BONUS` 的 BONUS 值）

---

## §0 设计轴心

- **最小改动原则**：只改两个 const，不动 settlement 路径 / ledger 接口 / buff 应用逻辑
- **观测-决策-实施三段制**：P0 观测期（4 周数据）→ 决策新数值（人工拍板）→ P1 实施 + 回归
- **校准目标区间**：`qi_max_bonus` 维持在 `[0.05, 0.25]`（低于 0.05 感知不到；高于 0.25 接近化虚基线）；`lifespan_bonus` 维持在 `[100, 500]` 年（保持"多活半辈子"到"多活一辈子"区间）
- **测试断言锁 const 不锁字面量**：所有测试必须 `assert_eq!(bonus, HALFSTEP_QI_MAX_BONUS)` 而非 `assert_eq!(bonus, 0.10f32)`——这样校准只改 const，测试自动跟上

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 4 周数据观测报告 + 决策新数值（人工拍板）| 遥测数据满足任一观测阈值；人工确认新数值在合理区间 |
| **P1** | ⬜ | const 更新 + 回归测试全绿 | `cargo test cultivation::tribulation::halfstep` 全绿；CI green |

---

## P0 — 观测期 + 数值决策

- [ ] 运营 4 周后运行 `/tribulation_debug` 导出以下指标：
  - `halfstep_count` / `ascended_count` 比例（正常范围：半步约为化虚的 2-4×，末法时代稀缺）
  - `ascension_quota_full_duration_ticks / total_ticks`（名额满时长占比，目标 < 60%）
  - `halfstep_stuck_duration_ticks`（平均滞留，目标区间：寿元的 5%-25%）
  - `rechallenge_trigger_count` / `rechallenge_success_count` / `rechallenge_fail_count`（重渡参与率，目标 > 40% 触发重渡）
- [ ] 根据数据决策新数值，原则如下：
  - 满意度低（滞留 > 30% 寿元）→ 适当上调 `qi_max_bonus` 至 `[0.12, 0.18]`；或上调 `lifespan_bonus` 至 `[250, 350]` 年
  - 满意度过高（几乎无人等待名额）→ 适当下调，或维持不变
  - 保持"质变门槛"：`qi_max_bonus` 不超过 0.25（超过开始逼近化虚基线）
- [ ] 人工拍板新数值并写入 `## §8.1 决议（pre-P1 收口，YYYY-MM-DD）`

**P0 验收**：遥测报告产出；人工拍板新数值写入 §8.1；触发条件文档化（"为什么改 / 改到多少 / 期望效果"）

---

## P1 — Const 更新 + 回归

- [ ] 更新 `server/src/cultivation/tribulation.rs:107-108` 两个 const 至 §8.1 拍板值
- [ ] 验证所有 halfstep 测试引用 const（无字面量）：`grep -n '0\.10\|200\.0' server/src/cultivation/tribulation.rs` 应零命中
- [ ] `cd server && cargo test cultivation::tribulation::halfstep` 全绿（测试断言自动跟 const 更新）
- [ ] `cd server && cargo test` 完整回归（确认 0 regression）
- [ ] 更新 `plan-halfstep-buff-v1.md` §8 决策 Q1-Q5 表格底部注释（追加 "2026-XX-XX 校准到 BONUS=Y"）以保留历史值

**P1 验收**：`cargo test` 全绿；`cargo clippy -- -D warnings` 零告警；CI green

---

## §8 开放问题（P0 决策门收口）

1. **观测期结束判定**：4 周是硬性等待，还是数据异常时提前触发（如上线第 1 周就发现 buff 完全感知不到）？建议：满 2 周且观测阈值已触及可提前
2. **双变量联动调整**：`qi_max_bonus` 和 `lifespan_bonus` 是独立调整还是联动调整（如只调寿元不调真元上限）？建议：优先单变量调整，保持其他维度不变便于观察效果
3. **NPC 影响评估**：校准前需评估 dormant NPC 半步修士数量及其影响（dormant 修士人数多时 bonus 数值变化影响更大）；需要单独统计 NPC vs 玩家的半步分布
4. **版本公告**：buff 数值变更属于运营事件，是否需要 narration 广播（"天道感知到修行壁垒有所松动..."）——属于 plan-halfstep-rechallenge-integration-v1 agent 侧扩展
