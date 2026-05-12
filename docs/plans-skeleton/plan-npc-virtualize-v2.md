# Bong · plan-npc-virtualize-v2

NPC 隐式更新框架**三态扩展** —— 在 Hydrated（ECS 全量 20Hz）↔ Dormant（SoA 批量 1/min）二态之间补充 **Drowsy 中间态**（64-256 格：ECS entity 存活，仅核心 system 以 1Hz FixedUpdate tick，远视野可见）。承接 plan-npc-virtualize-v1 ✅（二态 MVP：5000+ dormant NPC 单服已实装）。

**激活前提**：本 plan 在 plan-npc-virtualize-v1 P3 实测后，满足以下**任一触发条件**才启动（否则二态 MVP 已满足需求，v2 不激活）：
1. hydrate / dehydrate 单次开销 > 5ms / NPC（频繁穿越边界时 TPS 抖动 / 视觉撕裂感强）
2. 玩家以飞行 / 灵兽坐骑快速移动，频繁穿越 64-256 格阈值，NPC 反复 spawn/despawn 抖动
3. 玩家反馈「256 格外看不到任何 NPC」违和感强，影响末法残土世界丰度沉浸感

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 触发评估 + 决策门 + 三态架构选型 | ⬜ | — |
| P1 | `DrowsyNpc` Component + 6 边转换系统 | ⬜ | — |
| P2 | FixedUpdate 3 套调度 + LOD gate 配置 | ⬜ | — |
| P3 | Hysteresis 防抖 + 视觉过渡 + e2e 性能验收 | ⬜ | — |

---

## 接入面 Checklist

- **进料**：
  - `plan-npc-virtualize-v1` ✅：`HydratedNpc` / `DormantNpcSnapshot` / `NpcVirtualizeConfig`（hydrate 64 格 / dehydrate 256 格阈值）/ `hydrate_system` / `dehydrate_system` / `GlobalTick` 批量推演已实装（`server/src/npc/virtualize.rs`）
  - `plan-npc-ai-v1` ✅：完整 NPC Bundle（`NpcArchetype` / `Cultivation` / `Lifespan` / `Navigator` / `NpcBlackboard` / big-brain `BrainPlugin`）+ `NpcLodTier` 分档
  - `plan-npc-perf-v1` ✅：`SpatialIndex` + navigator A* 分桶 + per-NPC FixedUpdate 节流 + `NpcPerfProbe`

- **出料**：
  - 新增 `DrowsyNpc` Component（`server/src/npc/virtualize.rs`）
  - 扩展 `NpcVirtualizeConfig`（加 Drowsy 进入 / 退出阈值 + LOD gate 配置）
  - 三态转换系统：`drowse_system`（Hydrated→Drowsy）/ `wakeup_system`（Drowsy→Hydrated）/ `dehydrate_from_drowsy_system`（Drowsy→Dormant）/ `hydrate_from_dormant_to_drowsy_system`（Dormant→Drowsy，先 Drowsy 再走 wakeup 标准路径）

- **共享类型 / event**：
  - 复用 v1 `HydrateEvent` / `DehydrateEvent`；新增 `DrowseEvent`（Hydrated↔Drowsy 过渡）
  - **禁止**新建 `Cultivation` / `Lifespan` / `FactionMembership` 的 Drowsy 副本（孤岛红旗）
  - Drowsy 态 `NpcDigest` 输出与 Hydrated 格式相同（agent 侧无需感知 LOD 态）

- **跨仓库契约**：
  - server：`npc/virtualize.rs` 扩展三态状态机，`npc/brain.rs` Drowsy system set 摘录
  - agent：无变化（NpcDigest 通道不变，Drowsy NPC 按 Hydrated 格式出 digest）
  - client：Drowsy NPC entity 在 64-256 格范围内可见（v1 spawn 路径决定渲染，client 不感知 LOD 态）

- **worldview 锚点**：
  - §三:124-187 NPC 与玩家平等：Drowsy NPC 仍走寿元老化（1Hz 推演，rate_multiplier=0.1，区别于 Hydrated 20Hz / Dormant 1/min）；满足突破条件强制 Wakeup → Hydrated 渡劫完整流程
  - §十一:947-970 散修江湖人来人往：三态支撑 5000+ NPC 同服，Drowsy 是 256 格视野内远景填充密度的物理基础

- **qi_physics 锚点**：
  - Drowsy 态 1Hz 推演灵气吸收 / 逸散仍走 `qi_physics::ledger::QiTransfer` 守恒账本（worldview §二 底线——Drowsy 期间灵气禁止凭空消失）
  - Drowsy 推演速率参考：`rate_multiplier=0.1`（Dormant 0.3 × 1/3，因 tick 频率 1Hz vs 1/min = ×60 倍，需折算为相同 in-game 时间进度）

---

## §0 设计轴心 / P0 决策门

- [ ] **三态成本权衡**：6 边转换矩阵（H↔Dr / H↔D / Dr↔D）+ LOD gate 配置 + FixedUpdate 3 套并行，总实现成本约为 v1 的 2×。**未触发任何激活条件 → 本 plan 不启动，reminder 保留触发条件登记**
- [ ] **Drowsy 阈值**：建议 64/128/256 不对称设计（玩家 < 64 格 → Hydrated；退出 > 128 格 → Drowsy；退出 > 256 格 → Dormant）。具体值由 P0 实测收口
- [ ] **Drowsy 核心 system 清单**：P0 决定哪些 system 属于"核心"进入 `DrowsySystemSet`（建议：寿元老化 / qi_physics 1Hz 推演 / BreakthroughTrigger 检查 / WorldQiAccount 记账）；**不跑**：Navigator A* / big-brain 完整 Scorers / 社交 / 战斗
- [ ] **FixedUpdate 3 套调度**：Hydrated（`Update` ≈ 20Hz）/ Drowsy（`FixedUpdate` 1Hz）/ Dormant（`GlobalTick` 1/min）。确认三套 schedule set 不互相争锁 + Bevy `Time<Fixed>` interval 配置方案
- [ ] **LOD gate**：Drowsy → Hydrated 强制 Wakeup 触发门清单（渡劫满足 / 天道 agent 点名 / 玩家进入 64 格 / 战斗事件溅射 NPC）。决策门收口 gate 清单是否与 v1 `hydrate_trigger` 完全一致
- [ ] **视觉过渡**：Dormant→Drowsy 路径：NPC entity 在玩家 256 格内就可见，进入 64 格 Wakeup 时是否需要 1-2 tick fade-in 防止 NPC 在近距离"突然弹出"

---

## P0 触发评估与决策

交付物：
- 从 plan-npc-virtualize-v1 `NpcVirtualizeConfig` bench 数据读取当前 hydrate/dehydrate 实测延迟（`server/src/npc/virtualize.rs`）
- `scripts/bench_virtualize.sh`：100 NPC 快速穿越 64-256 格边界，录 hydrate/dehydrate 延迟 P50/P99
- 决策文档（P0 结论写入本 plan 头部）：满足任一触发条件 → 继续 P1；否则记录"v1 性能满足需求，v2 不启动"收口并关闭本 plan

---

## P1 DrowsyNpc Component + 6 边转换矩阵

交付物：
- `DrowsyNpc` Component（`server/src/npc/virtualize.rs`）
- 三态枚举 `NpcLodState { Hydrated, Drowsy, Dormant }` 取代 v1 bool 标记（或扩展 v1 component，需确认迁移路径）
- `drowse_system`：Hydrated → Drowsy（玩家离开 64 格 + Hysteresis 防抖）
- `wakeup_system`：Drowsy → Hydrated（玩家进入 64 格 + LOD gate 强制 Wakeup）
- `dehydrate_from_drowsy_system`：Drowsy → Dormant（玩家离开 256 格 + Hysteresis）
- `hydrate_from_dormant_to_drowsy_system`：Dormant → Drowsy（玩家进入 256 格，先进 Drowsy 再由 wakeup_system 推 Hydrated）
- 单测：6 边转换路径各自 happy path + Hysteresis 防抖（短时间内多次穿越不反复抖动）+ 强制 Wakeup 门各触发情况

---

## P2 FixedUpdate 3 套调度 + LOD gate

交付物：
- Bevy `FixedUpdate` schedule 1Hz 配置（`Time<Fixed>` interval = 1s）
- `DrowsySystemSet`：仅含核心 system（寿元老化 / qi_physics 推演 / BreakthroughTrigger / WorldQiAccount 记账）
- `NpcLodGatePlugin`：LOD gate 实装（渡劫满足 / 天道 agent 点名 / 玩家进入阈值 → 强制 Wakeup）
- 性能 bench：1000 Drowsy NPC 1Hz tick，目标 < 2ms/tick（SpatialIndex 基础上）
- 单测：1Hz system 实际跑频率验证（`GameTick` 对齐 + `Time<Fixed>` 精度检查）

---

## P3 Hysteresis 防抖 + 视觉过渡 + e2e 验收

交付物：
- Hysteresis 防抖最终参数落地（由 P0 决策门给出初值，P3 实测调参）
- 视觉过渡（如 P0 判断必要）：Dormant→Drowsy → Wakeup 路径加 1-2 tick fade-in，防止 NPC 在视野内突然弹出
- e2e 测试：100 Hydrated + 2000 Drowsy + 3000 Dormant，5min TPS ≥ 18；快速穿越边界无可见抖动
- 守恒律 e2e：5000 NPC 混合态 1h in-game，WorldQiAccount 守恒误差 < 0.1%
- CI：`cargo test npc::virtualize` 覆盖三态转换 + Hysteresis + 守恒律检查 + LOD gate 触发

---

## §8 开放问题

1. **Drowsy 阈值最终值**：64/128/256 还是 80/160/320？取决于 v1 P3 实测撕裂感距离与玩家坐骑最大移速
2. **Drowsy 核心 system 是否含社交**：`socialize_scorer` 不含则 Drowsy NPC 互相不感知（256 格丰度感的代价）；含则 1Hz 社交质量够用吗？
3. **Drowsy→Hydrated 视觉过渡必要性**：玩家在 256 格外就能看见 Drowsy entity，进入 64 格 Wakeup 时如果 NPC 姿态突变是否明显违和
4. **NpcDigest 是否扩 lod_state 字段**：天道 agent 感知 Drowsy 态有助于叙事推演（"远方 NPC 处于模糊状态"），v2 是否需要扩展 agent 侧感知
5. **FixedUpdate 与 GameTick 对齐**：服务器 TPS 浮动时（WSL2 18-20 TPS），FixedUpdate 1Hz 实际跑频率保证方案（`Time<Fixed>` drift 处理）
6. **v1 → v2 迁移策略**：v1 的 `HydratedNpc` / dormant bool 标记如何平滑迁移到三态 `NpcLodState` enum，是否需要 migration system 处理存量 entity

---

## 前置依赖

- `plan-npc-virtualize-v1` ✅（二态 MVP 已实装：`HydratedNpc` / `DormantNpcSnapshot` / hydrate/dehydrate 系统 / `GlobalTick` 批量推演 / 守恒律账本接入）
- `plan-npc-perf-v1` ✅（`SpatialIndex` + navigator 分桶 + LOD gate baseline）
- `plan-npc-ai-v1` ✅（NPC 全套 Bundle + big-brain 框架）
- `plan-qi-physics-v1` ✅（`qi_physics::ledger::QiTransfer` 守恒账本）

## 反向被依赖

- `plan-npc-virtualize-v3` ⬜（dormant↔dormant 战争批量推演，三态稳定后再叠战争推演层，防 race condition 复杂化）
- `plan-narrative-political-v1`（世界丰度 / 师承代际，Drowsy 填充远视野丰度是政治叙事前提）
- `plan-quest-v1`（占位）（Drowsy NPC 派任务 → LOD gate hydrate-on-demand）
