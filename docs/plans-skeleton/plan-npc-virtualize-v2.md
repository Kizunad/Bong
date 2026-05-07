# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架 **三态扩展：Drowsy 中间态**——在 plan-npc-virtualize-v1 二态 MVP（Hydrated ↔ Dormant）基础上，补入 64-256 格的 Drowsy 中间状态（ECS entity 常驻但仅核心 system 1Hz tick + 远视野可见），解决 v1 P3 实测中暴露的撕裂感 / 抖动 / 视野空旷等问题。

**启动条件（决策门 #1，来自 plan-npc-virtualize-v1）**：v1 P3 实测后出现以下**任意一条** → 启动本 plan：

1. hydrate / dehydrate 单次开销 > 5 ms / NPC（频繁穿越边界时撕裂感强）
2. 玩家移动快（飞行 / 灵兽坐骑）频繁穿越 64-256 格阈值，反复 spawn/despawn 抖动可感知
3. 玩家反馈"远视野空旷无 NPC"违和（256 格外看不到任何 NPC 影响沉浸感）

若 v1 P3 实测**三条均未触发**，本 plan 不启动，v1 二态 MVP 已足够。

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（二态 MVP + §8 决策门 #1 触发条件）· `plan-npc-perf-v1.md` ⏳（hydrated NPC 性能基础）· `plan-npc-ai-v1.md` ✅（所有 Bundle / Scorer / Action 注册）· `plan-qi-physics-v1.md` P1 ✅（ledger::QiTransfer）

**worldview 锚点**：

- **§三:124-187 NPC 与玩家平等**：Drowsy NPC 仍按正常频率（降频 × 0.1）老化和修炼，不因"远离玩家"豁免规则
- **§十一:947-970 散修江湖**：Drowsy 中间态是"远处有人影活动"的物理基础，解决 256 格外视野"死寂"违和感
- **§P 真元浓度场**：Drowsy NPC 距玩家 64-256 格，真元交互走 `qi_physics::distance::attenuation`（中距离衰减），不走全量 hydrated 路径

**qi_physics 锚点**（Drowsy 状态仅使用以下子集，严格不超出）：

- `qi_physics::ledger::QiTransfer` —— Drowsy 期灵气流动必须走 ledger，守恒律红线（同 v1）
- `qi_physics::regen_from_zone` —— Drowsy 修炼吸收（同 dormant，但每 1Hz tick 更新一次而非 dormant 的 1/分钟）
- `qi_physics::distance::attenuation` —— 64-256 格中距离衰减系数

**前置依赖**：

- `plan-npc-virtualize-v1` ✅ → 二态 MVP 落地 + P3 实测触发条件成立（必须 P3 完成后才确认是否启动本 plan）
- `plan-npc-perf-v1` ✅ → hydrated 100 NPC 跑通是前提
- `plan-npc-ai-v1` ✅ → 所有 Bundle / Scorer / Action
- `plan-qi-physics-v1` P1 ✅ → ledger API 冻结

**反向被依赖**：

- `plan-npc-virtualize-v3` → Drowsy 态是 dormant↔dormant 战争推演的过渡缓冲层（v3 依赖三态稳定后再启动）
- `plan-npc-ai-v1 §3.3` 1000 NPC stretch goal → Drowsy 是 hydrated 100 → 1000 过渡的中间支撑

---

## 接入面 Checklist

- **进料**：
  - `NpcDormantStore` Resource（v1 已实装，v2 新增 Drowsy 枚举值）
  - 玩家 Position（每 1Hz 采样，决定 Drowsy 边界）
  - `qi_physics::env::EnvField`（Drowsy zone 浓度场）
- **出料**：
  - `NpcVirtualizationState` enum 扩展为三态：`Hydrated` / `Drowsy` / `Dormant`（v1 二态基础上加 Drowsy 变体）
  - `drowsy_tick_system`（FixedUpdate 1Hz：仅跑核心 system subset：lifespan_aging / qi_regen / realm_check；跳过 big-brain AI / navigator / vfx）
  - `NpcDrowsyLodEntity` component（标记 Drowsy NPC 的 ECS entity，携带最小化 LOD 可视信息）
  - Drowsy → Hydrated / Hydrated → Drowsy / Dormant → Drowsy 三条转换路径（共 6 边 vs v1 二态 2 边）
  - LOD gate：Drowsy NPC 发送 entity metadata（名字 / 境界轮廓），但不发送完整 AI 状态
- **共享类型**：
  - 复用 `NpcDormantSnapshot`（Drowsy dehydrate 时写入相同字段）
  - 复用 `qi_physics::ledger::QiTransfer`
  - **禁止**为 Drowsy 单独造第三份 Cultivation / Lifespan 副本（孤岛红旗）
- **跨仓库契约**：
  - server: `npc::virtualize::DrowsyState` 子模块
  - agent: 无变化（NpcDigest 通道 unchanged；Drowsy NPC 同 dormant 出 NpcDigest）
  - client: Drowsy NPC 以 LOD entity 形式可见（低精度模型 / 无 AI 动画）

---

## 阶段概览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 三态决策门 + Drowsy 架构设计 + LoD gate 规格 | ⬜ |
| P1 | 三条 hydrate/drowsy/dormant 转换路径实装 | ⬜ |
| P2 | drowsy_tick_system + qi_physics 守恒律集成 | ⬜ |
| P3 | LOD entity client 可视 + e2e 验收 | ⬜ |

---

## §0 设计轴心

- [ ] **三态成本评估（P0 决策门）**：三态引入 6 边转换矩阵（H↔Dr / H↔D / Dr↔D 共 6 边 vs 二态 2 边）+ LOD gate 配置 + FixedUpdate 调度三套并行（Update 20Hz hydrated / FixedUpdate 1Hz drowsy / GlobalTick 1/min dormant）。P0 需评估：是否值得引入？替代方案（smooth hydrate preload / 预测性 prehydrate）是否更简单？
- [ ] **Drowsy system 子集定义（P0 拍板）**：Drowsy 1Hz tick 跑哪些 system？候选：lifespan_aging / qi_regen / realm_check（必须）/ nav waypoint 更新（可选）/ vfx（跳过）/ big-brain AI scorer（跳过）
- [ ] **Hysteresis 阈值调整**：v1 的 64/256 格不对称阈值在 Drowsy 引入后需拆为三级：hydrate 阈值（≤ 64 格）/ drowsy 阈值（≤ 256 格）/ dormant 阈值（> 256 格），防止 64-256 区间内三态反复切换抖动（需再设内层 Hysteresis）
- [ ] **守恒律维持**：Drowsy tick 1Hz 时 qi_regen 调用频率比 dormant 1/min 高 60×，需确认 zone qi 扣减不超速（可能需要 drowsy qi_regen_budget 分桶）

---

## §1 开放问题（P0 决策门收口）

- [ ] **Q1** Drowsy system subset：nav waypoint 是否加？（加了可以让 Drowsy NPC 缓慢向 POI 移动；不加则 Drowsy NPC 静止）
- [ ] **Q2** LOD entity 精度：Drowsy NPC 在 client 是否显示为模糊人影（降精度骨骼）还是完整模型（低刷新率）？
- [ ] **Q3** Drowsy → Dormant 转换：玩家从 64-256 格快速移动到 > 256 格时，是直接 dehydrate 到 dormant 还是先停在 Drowsy 保留 ECS entity？
- [ ] **Q4** qi_regen 频率：Drowsy 1Hz 每次调 `regen_from_zone` vs 保持 dormant 节奏（1/min 批量），哪个更好？守恒律两者均满足，但 1Hz 对 zone qi 压力更大
- [ ] **Q5** Drowsy 可见距离：远至 256 格可见还是缩短为 200 格避免 LOD entity 过多？

---

## §9 进度日志

- **2026-05-07** 骨架立项。源自 plan-npc-virtualize-v1 §8 决策门 #1 + reminder.md `plan-npc-virtualize-v1` 段 Drowsy 候补记录。本 plan **条件性启动**：v1 P3 实测三条触发条件任一成立方开始消费。

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/npc/virtualize/drowsy.rs` + `drowsy_tick_system` + Hysteresis 三级阈值 + LOD gate client 协议
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test npc::virtualize::drowsy` 数量 / 三态切换守恒律 e2e / LOD entity client 可见验收
- **跨仓库核验**：server `npc::virtualize::DrowsyState` / agent NpcDigest unchanged / client LOD entity 协议
- **遗留 / 后续**：`plan-npc-virtualize-v3`（dormant↔dormant 战争推演）
