# plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架**三态扩展**：在 plan-npc-virtualize-v1 二态 MVP（Hydrated ECS ↔ Dormant SoA）基础上引入 **Drowsy 中间态**（64-256 格），消除高速移动玩家引发的频繁 hydrate/dehydrate 抖动和远视野空旷违和感。

**启动条件（决策门 #1）**——本 plan 仅在 plan-npc-virtualize-v1 P3 实测后出现以下**任一**情况时方可启动：
- hydrate / dehydrate 单次开销 > 5 ms/NPC（频繁穿越 256 格边界时撕裂感强）
- 玩家高速移动（飞行 / 灵兽坐骑）频繁穿越 64-256 格阈值，反复 spawn/despawn 抖动
- 玩家反馈"远视野空旷无 NPC"违和（256 格外看不到任何 NPC 影响沉浸感）

**worldview 锚点**：`worldview.md` §二 真元守恒律 + §三:124-187 NPC 与玩家平等不豁免 + §十一:947-970 散修江湖人来人往（5000+ NPC 视野内可感知是物理基础）

**交叉引用**：`plan-npc-virtualize-v1`（二态 MVP 前置）· `plan-npc-perf-v1`（性能基线）· `plan-qi-physics-v1`（守恒律）· `plan-npc-ai-v1`（LOD gate）· `plan-narrative-political-v1`（NPC 丰度）

---

## 接入面 Checklist

- **进料**：plan-npc-virtualize-v1 的 `NpcVirtualState`（二态枚举）/ `HysteresisConfig`（64/256 阈值）/ Redis `bong:npc/dormant`（SoA 持久化）/ `NpcLodTier` + plan-npc-perf-v1 的 `SpatialIndex`
- **出料**：扩展三态 `NpcVirtualState::Drowsy` → plan-npc-ai-v1 LOD gate 消费 / plan-narrative-political-v1 远视野 NPC 可见性
- **共享类型**：`NpcVirtualState` 枚举扩展 Drowsy 变体；`NpcDrowsyData` 新增 component；`LodGateConfig` 扩展 drowsy 阈值字段
- **跨仓库契约**：纯 server 端模块，无新增 IPC schema / Redis key（Drowsy 态不持久化，仅 in-memory ECS）
- **worldview 锚点**：§二 守恒律（Drowsy↔Dormant 灵气转换必走 ledger）/ §十一 散修江湖
- **qi_physics 锚点**：`ledger::QiTransfer`（所有 Drowsy 态 qi 变动）/ `excretion::container_intake`（Drowsy 真元吸收，clamp 到 zone 下限）/ `release::release_to_zone`（Drowsy 老死 / 渡劫失败灵气归还）

---

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 决策门核查 + 三态状态机设计 | ⬜ | — |
| P1 | Drowsy component + 六边转换算子 | ⬜ | — |
| P2 | 1Hz FixedUpdate 节流 + 视觉锚定 | ⬜ | — |
| P3 | 性能验收 + 防抖调参 + 迁移收口 | ⬜ | — |

---

## P0 — 决策门核查 + 三态状态机设计

**可核验交付物**：

- P0 决策门答案写入本 plan §0 头部：引用 plan-npc-virtualize-v1 P3 实测数据（TPS / hydrate 耗时 / 玩家反馈），确认至少一条触发条件成立
- `server/src/npc/virtualize.rs`：`NpcVirtualState` 枚举新增 `Drowsy` 变体，三态状态机迁移矩阵注释
- 六边转换设计文档（inline 于本 plan §P1 前）：
  - H→Drowsy：玩家距离 > 64 格 + Hysteresis 防抖（进入阈值 192 格，出阈值 128 格）
  - Drowsy→H：玩家距离 < 128 格 / 渡劫就绪 / 受到攻击
  - Drowsy→Dormant：玩家距离 > 256 格 + Hysteresis（进入阈值 320 格，出阈值 256 格）
  - Dormant→Drowsy：玩家距离 < 256 格（非渡劫）
  - H→Dormant：仅允许经 Drowsy 中转，禁止直接跳变（防止转换代价积累）
  - Dormant→H：渡劫强制 hydrate（同 v1）
- `LodGateConfig` struct 扩展字段：`drowsy_enter_dist / drowsy_exit_dist / dormant_enter_dist / dormant_exit_dist`（Hysteresis 四参数）
- plan-npc-virtualize-v3 占位条件确认（决策门 #6：若 P3 实测 Drowsy↔Dormant 互动需要 NPC 间批量推演 → 启动 v3）

**开放问题**（P0 决策门收口）：
1. Drowsy 态保留哪些 ECS component？最小集（Position / NpcKind / NpcAffiliation）vs 扩展集（+ NpcLodTier / faction）权衡
2. 六边转换中 Drowsy→Dormant 转换是否需要延迟 N tick（防止玩家在 256 格边界反复触发 Dr↔D 跳变）？
3. 1Hz FixedUpdate Drowsy system 是否需要分桶（类比 plan-npc-perf-v1 navigator 分桶方案），防止同 tick 批量唤醒？
4. Drowsy NPC 被攻击 → 立即 hydrate（响应性优先）还是先 injury_check 再 hydrate（守恒律优先）？
5. 远视野可见性：Drowsy NPC 64-256 格内是否显示名称 / 头顶信息？（仅 entity 可见 vs 带 HUD 标注）

---

## P1 — Drowsy component + 六边转换算子

**可核验交付物**：

- `server/src/npc/virtualize.rs`：`NpcDrowsyData` component 定义（ECS entity 保留子集数据）
- 六边转换算子各实装：
  - `hydrate_from_drowsy(npc, world)` — Drowsy→H，完整 bundle 重建
  - `drowsy_from_hydrated(npc, world)` — H→Drowsy，多余 component despawn（战斗 / 社交 / 农田 component strip）
  - `drowsy_from_dormant(npc, soa_store, world)` — D→Drowsy，SoA 反序列化 + 最小 ECS bundle spawn
  - `dormant_from_drowsy(npc, soa_store, world)` — Drowsy→D，SoA 序列化 + entity despawn
  - H→Dormant / Dormant→H：路由到 v1 算子（禁止直接跳变规则由调用方保证）
- Drowsy 态 qi_physics 守恒：
  - Drowsy 老死走 `qi_physics::release::release_to_zone`（灵气归还 zone，同 Dormant 规则）
  - Drowsy 真元吸收走 `qi_physics::excretion::container_intake`
  - 所有 Drowsy qi_current 写入必须有 `qi_physics::ledger::QiTransfer` 记录
- 单测：`npc::virtualize::tests::drowsy_*`，覆盖六边转换 + qi 守恒断言（≥ 24 条）

---

## P2 — 1Hz FixedUpdate 节流 + 视觉锚定

**可核验交付物**：

- `server/src/npc/virtualize.rs`：`drowsy_tick_system` 注册到 `FixedUpdate`（1Hz），与 Hydrated Update（20Hz）/ Dormant GlobalTick（1/min）区分调度
- Drowsy 态仅运行以下核心 system（1Hz）：
  - `lifespan_aging_drowsy` — 寿元推进（走 `QiTransfer`）
  - `tribulation_ready_check_drowsy` — 渡劫就绪检查（触发 Drowsy→H hydrate）
  - `qi_regen_drowsy` — 真元自然回复（走 ledger，clamp 到 zone 浓度下限）
  - `drowsy_hydrate_trigger` — 监听玩家进入 128 格 / 受攻击事件 → 强制 H 转换
- 以下 system 在 Drowsy 态完全暂停：巡逻 / 社交 / 战斗 / 农田 / 灵田 / 任意 Scorer（除 `tribulation_ready`）
- 视觉规格（Drowsy 64-256 格内）：
  - NPC entity 保留（不 despawn），client 可见
  - 动画：仅 idle 骨骼（`PlayerAnimator` idle clip）
  - 名称 / 信息条：由 P0 决策门 #5 答案决定
  - **无粒子 / 无音效**（Drowsy 是内部 LOD 态，不向 client 广播额外事件）
- 集成测试：`npc::virtualize::integration::drowsy_transition_*`，覆盖 H→Dr→D→Dr→H 完整链路（≥ 8 条）

---

## P3 — 性能验收 + 防抖调参 + 迁移收口

**可核验交付物**：

- 性能验收（`scripts/bench/npc_virtualize_v2.sh`）：
  - 100 Hydrated + 500 Drowsy + 1000 Dormant 混合场景 TPS ≥ 18
  - 飞行玩家穿越 64-256 格边界 100 次，NPC 无 spawn/despawn 抖动（Drowsy 态维持）
  - H→Drowsy / Drowsy→H 单次转换耗时 < 3 ms/NPC（hydrate 代价比 v1 H→D 降低 50% 以上）
  - 5000 Dormant + 500 Drowsy 5min in-game qi 守恒误差 < 0.1%
- Hysteresis 防抖参数确认并写入 `LodGateConfig` 默认值：
  - `drowsy_enter=192 / drowsy_exit=128`（进入 Drowsy 阈值 / 退出 Drowsy 阈值）
  - `dormant_enter=320 / dormant_exit=256`（进入 Dormant 阈值 / 退出 Dormant 阈值）
- plan-npc-virtualize-v1 §8 决策门 #1 正式关闭（本 plan P3 通过即关）
- plan-npc-virtualize-v3 占位条件评估：若 Drowsy↔Dormant 互动需要 NPC 间批量推演，在决策门 #6 中选 C 并启动 v3 骨架；否则 v3 不立

**验收命令**：
```bash
cd server && cargo test npc::virtualize -- --test-threads=1
bash scripts/bench/npc_virtualize_v2.sh   # TPS + 转换耗时
```

---

## Finish Evidence

（待 P3 全部 ✅ 后填写，迁入 `docs/finished_plans/` 前必须完成）
