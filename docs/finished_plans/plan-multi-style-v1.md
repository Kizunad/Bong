# Bong · plan-multi-style-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板 + 6 决策 + reframe）。前置 plan-cultivation-v1 ✅ + plan-cultivation-canonical-align-v1 ✅ finished + plan-style-vector-integration-v1 ⏳ active（PracticeLog 扩展依赖）。
>
> **2026-05-04 reframe**（Q-MS0 A）：原"UnlockedStyles 多激活"路径已撤销（与 worldview §五"流派由组合涌现" + plan-style-vector-integration-v1 PracticeLog vector 模型冲突）。**新模型**：multi-style = "PracticeLog 在 7 个 ColorKind 都有累积，且每个 < 25% 总量 → is_hunyuan = true"。**代价纯靠时间 + 学习成本自然消化**，不在机制层加战斗效率惩罚 / 突破真元池加成（Q-MS3 / Q-MS5 决议）。

全流派精通路径。玩家通过 PracticeLog 7 色均衡累积自然演化为混元色（is_hunyuan）。代价是修炼时间 + 学习每流派功法/招式/经脉的成本，不另加机制惩罚。**对应 plan-gameplay-journey-v1 §A.5**。

**世界观锚点**：`worldview.md §五 流派由组合涌现` · `§六 line 1`(路径倾向) · `§六.二 真元染色(混元色)`

**library 锚点**：`cultivation-0005 真元十一色考`(混元色章节)

**交叉引用**：`plan-style-vector-integration-v1` ⏳(PracticeLog vector 模型，本 plan 直接依赖) · `plan-cultivation-canonical-align-v1` ✅ · `plan-cultivation-v1` ✅(QiColor 染色 + evolve_qi_color 阈值演化已实装) · `plan-gameplay-journey-v1` §A.5/O.14 · `plan-style-balance-v1` ⬜(混元色不被克制的战略价值锚)

---

## 接入面 Checklist

- **进料**：`PracticeLog.weights: HashMap<ColorKind, f64>` ✅（cultivation-v1 已实装）+ 修炼 session 事件源（Q-MS2 决策：扩展 PracticeLog 接静坐 / 引气 session）
- **出料**：`is_hunyuan` 计算结果（基于 PracticeLog 7 色每个 < 25% threshold）+ client UI 显示 7 色分布
- **共享类型**：复用 `PracticeLog` ✅ + `QiColor::is_hunyuan` ✅（已实装于 evolve_qi_color）；**不**新增任何 component
- **worldview 锚点**：§五 流派由组合涌现 + §六 line 1 + §六.二

---

## §0 设计轴心

- [ ] **混元色 = 7 色均衡涌现**：基于 PracticeLog vector 自然演化，不需要"激活"动作
- [ ] **代价纯前置**（Q-MS3/Q-MS5 决议 2026-05-04）：
  - 时间成本：均衡 7 色需要分散修炼，单流派精进 100h → 7 流派均衡需要更长（每流派都要修）
  - 学习成本：每流派的功法 / 招式 / 经脉拓扑都要单独学习（每 plan 自己的 P0/P1）
  - **不加机制惩罚**：~~战斗效率 -20%~~ 删除；~~突破真元池 +4% per style~~ 删除
- [ ] **不做洗色渠道**（Q-MS4 决议）：玩家想"洗"自然方式 = 修炼其他流派让其他色累积过线 / vector-integration 已有 `decay_per_tick = 0.001` 自然衰减
- [ ] **混元色战略价值**（worldview §六.二 + plan-style-balance-v1 锚）：不被任何单流派克制（克制系数 ×1.0 全场）；这是玩家投入时间换来的回报，不需要额外补偿
- [ ] **不可绕过**：经脉拓扑 / 突破丹 / 渡虚劫等硬门槛不变（与 cultivation-v1 解耦）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 混元色判定 system: `is_hunyuan(weights)` = `weights.values().all(|v| v / total < 0.25)` 且 `weights.len() >= 5`（至少修过 5 种色） | 单元：7 色均衡 → true / 单色独大 → false / 仅 3 色 → false |
| **P1** ⬜ | PracticeLog 扩展接修炼 session 事件源（Q-MS2，依赖 vector-integration vN+1）：静坐 / 引气时按当前 active style 主色 +X/min（每分钟聚合 Q-MS1） | 单元：1h 静坐主修 baomai → PracticeLog["Heavy"] += 60×X |
| **P2** ⬜ | client UI: 显示当前 PracticeLog 7 色分布 bar chart + 是否 is_hunyuan + 距离混元缺哪几色 | 玩家可见 vector 演化路径 |
| **P3** ⬜ | 与 plan-style-balance-v1 telemetry 对接: 混元色玩家 PVP 数据回填 §P 矩阵（验证"不被克制" 是否真的不被克制） | telemetry 跑过 4 周后数据校准 |

---

## §2 关键公式

混元色判定（**唯一公式**，user Q-MS3/Q-MS5 决议后简化）：

```rust
// server/src/cultivation/color.rs（已实装 evolve_qi_color，本 plan 仅扩展）
pub fn is_hunyuan(log: &PracticeLog) -> bool {
    let total: f64 = log.weights.values().sum();
    if total < EPSILON || log.weights.len() < 5 {
        return false;  // 累积不足或修炼色种类 < 5
    }
    log.weights.values().all(|v| v / total < 0.25)
}
```

- 至少修过 5 种 ColorKind（防止"只修两色就混元")
- 任一色占比 < 25%（worldview §六.二 + library cultivation-0005 真元十一色考"无主色"判定）

~~突破要求加成~~ 删除（user Q-MS5 决议：代价已在前置时间 + 学习成本，不再加机制惩罚）。

---

## §3 数据契约

- [ ] `server/src/cultivation/color.rs::is_hunyuan` 混元色判定（在已实装 evolve_qi_color 旁边新增）
- [ ] `server/src/cultivation/practice_log.rs` 修炼 session 事件源扩展（Q-MS2，与 vector-integration vN+1 对接）
- [ ] `client/.../cultivation/QiColorVectorHud.java` 7 色分布 bar chart + is_hunyuan 指示

> **不做**：~~`style_count.rs` 多激活计数~~（vector 模型不需要）/ ~~`breakthrough.rs::required_qi_pool` 加成~~（取消）/ ~~`qi_color.rs::lock_main_color` 25% 阈值~~（自然演化即洗色，不锁主色）/ ~~战斗效率 -20% 应用~~（取消）

---

## §4 开放问题

- [x] **Q-MS0 ✅**（user 2026-05-04 A，reframe）：废弃 UnlockedStyles 多激活路径，全面对齐 vector-integration PracticeLog vector 模型。本 plan 不动 UnlockedStyles。
- [x] **Q-MS1 ✅**（user 2026-05-04 B）：修炼 session 累积按**每分钟聚合**（精度折中，体积可控）。
- [x] **Q-MS2 ✅**（user 2026-05-04 A）：时长占比按**真元修炼时长（静坐 / 引气 session）**统计——扩展 PracticeLog 接修炼 session 事件源（依赖 vector-integration vN+1，详 P1）。战斗事件 PracticeLog 累积仍由 vector-integration P0 负责。
- [x] **Q-MS3 ✅**（user 2026-05-04 取消）：~~混元色 -20% 战斗效率~~ 删除。代价已在前置时间 + 学习成本上自然消化。
- [x] **Q-MS4 ✅**（user 2026-05-04 不做）：不做专门洗色渠道。玩家"洗"= 修炼其他流派让其他色累积过线 / vector-integration `decay_per_tick = 0.001` 自然衰减。
- [x] **Q-MS5 ✅**（user 2026-05-04 取消）：~~+4% per style 突破真元池加成~~ 删除。"不是突破就能提升战力，每个功法都有学习成本"——代价已明了，不在突破环节再加机制惩罚。

> **本 plan 不再有未拍开放问题**——P0 可立刻起。

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §A.5 / O.14 决策落点。
- **2026-05-04**：skeleton → active 升级（user 拍板）。**6 决策闭环 + reframe**：
  - Q-MS0 reframe：废弃 UnlockedStyles 多激活路径（vector-integration PracticeLog 模型取代）
  - Q-MS1 修炼时长每分钟聚合
  - Q-MS2 PracticeLog 接修炼 session 事件源（vector-integration vN+1 扩展）
  - Q-MS3/Q-MS5 删除所有机制惩罚（战斗效率 -20% / 突破真元池 +4% per style 全砍）
  - Q-MS4 不做洗色渠道（自然演化即洗）
  - 范围大幅简化：原 5 phase（UnlockedStyles 多激活 + 加成 + 染色锁 + 战斗惩罚 + UI）→ 新 4 phase（is_hunyuan 判定 + PracticeLog 修炼 session 扩展 + UI + telemetry 对接 style-balance）
  - 下一步起 P0 worktree（is_hunyuan 函数 + 单元测试）。P1 必须等 vector-integration vN+1 PracticeLog 修炼 session 事件源扩展。
