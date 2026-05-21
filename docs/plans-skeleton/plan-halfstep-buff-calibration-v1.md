# Bong · plan-halfstep-buff-calibration-v1 · 骨架

**半步化虚 buff 强度运营校准**——基于 plan-halfstep-buff-v1 落地后积累的遥测数据，将占位值 `HALFSTEP_QI_MAX_BONUS=0.10` / `HALFSTEP_LIFESPAN_BONUS_YEARS=200.0` 调整为运营数据支撑的正式值，并引入"§8 决策门预设的 30% / 5% 观察阈值"触发校准 PR。

**启动条件**（满足任一）：
- 遥测显示 quota 满时长 > 30% server 在线时间，且半步修士平均滞留 > 7 days in-game
- 半步修士在 100h 实测中主观反映"buff 几乎感觉不到"（< 5% 渡劫成功率差异）
- plan-gameplay-acceptance-v1 P2 数据回填后 balance team 提出调整请求

**交叉引用**：`plan-halfstep-buff-v1` ✅（const 定义、遥测 API、重渡机制）· `plan-tribulation-v1` ✅（AscensionQuotaStore、渡虚劫基础）· `plan-qi-physics-v1` P1 ✅（qi_max 守恒修改 API）· `plan-gameplay-acceptance-v1` skeleton（提供 100h 实测数据）

**worldview 锚点**：
- **§三:78 化虚稀缺性**：buff 强度上限是"有吸引力但不等同化虚"——不能让半步修士实力接近化虚者
- **§十:1013 寿元节奏**：`HALFSTEP_LIFESPAN_BONUS_YEARS` 变动影响通灵层玩家生命周期节奏

**qi_physics 锚点**：
- 调整 `qi_max` bonus 时，`qi_physics::ledger::QiTransfer { reason: HalfStepBuff }` audit 记录不变
- 只改 const 值，不改 ledger 调用路径；`qi_max *= 1.0 + HALFSTEP_QI_MAX_BONUS` 的幂等守卫 (`buff_applied`) 已在 v1 实装

---

## 接入面 Checklist

- **进料**：`/tribulation_debug` 遥测数据（`halfstep_stuck_duration_ticks` / `ascension_quota_full_duration_ticks`）+ plan-gameplay-acceptance-v1 100h 实测日志
- **出料**：调整后的两个 const 值 + 对应 CLAUDE.md 测试约定注释（禁写字面值）
- **共享类型**：复用 `HALFSTEP_QI_MAX_BONUS` / `HALFSTEP_LIFESPAN_BONUS_YEARS` const（`server/src/cultivation/tribulation.rs`），无新增类型
- **跨仓库契约**：const 值变化仅影响 server 侧结算；agent / client 无感知（遥测数据通过 Redis `bong:world_state` 已可观测）
- **worldview 锚点**：§三:78 稀缺性 + §十:1013 寿元节奏

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 遥测数据收集 + 启动条件核验 | `/tribulation_debug` 数据满足启动条件之一 |
| **P1** | ⬜ | const 值校准 + 单测更新 | 回归测试全绿；`assert_eq` 引用 const 不写字面值 |
| **P2** | ⬜ | 100h 实测后复盘（可选第二轮校准）| gameplay-acceptance P2 数据回填；如需再调重开 P1 |

---

## P0 — 遥测数据收集与阈值核验

- [ ] 运行 `/tribulation_debug` 读取 `quota_full_duration_ticks` 占比 + `halfstep_stuck_duration_ticks` 中位数
- [ ] 对照 plan-halfstep-buff-v1 §8 决策门 Q1/Q5 预设观察阈值（30% 满时长 / 7d 平均滞留）核验是否触发
- [ ] 如未触发：记录快照并关闭本 plan（条件未到，延后）
- [ ] 如触发：进 P1 并说明触发原因

## P1 — const 值调整

- [ ] 分析遥测分布，提出新 `HALFSTEP_QI_MAX_BONUS` 候选值（建议范围 0.08-0.20）
- [ ] 提出新 `HALFSTEP_LIFESPAN_BONUS_YEARS` 候选值（建议范围 150-400）
- [ ] 更新 `server/src/cultivation/tribulation.rs` 两个 const
- [ ] 更新所有引用这两个 const 的单测断言（禁写字面值，引用 const 引用）
- [ ] 至少 3 单测覆盖新值：buff 应用后 qi_max 正确 / lifespan 正确增加 / 不叠加守卫仍生效

## §6 开放问题

1. **调整频率**：每次运营周期（约 30 天）校准一次，还是按事件触发（quota 满时长超阈值）才调？
2. **NPC vs 玩家差异**：dormant NPC 和玩家使用同一 const——是否需要分别定义？（当前 plan-halfstep-buff-v1 Q5 已决定同池，这里只讨论值是否分开）
3. **第二轮校准**：P2 回填若仍不满意，是重开 P1 还是立 plan-halfstep-buff-calibration-v2？

---

> 骨架创建日期：2026-05-21。派生自 plan-halfstep-buff-v1 §遗留 #4。
