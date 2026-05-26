# Bong · plan-npc-virtualize-v2 · 骨架

NPC 虚拟化**三态扩展**——在 v1 二态（Hydrated ↔ Dormant）基础上补 Drowsy 中间态（64-256 格 ECS entity 但仅核心 system 1Hz tick + 远视野 LOD 可见），消除玩家快速穿越边界时的 spawn/despawn 撕裂感并补足远视野 NPC 稀薄问题。

**前置条件**（派生自 plan-npc-virtualize-v1 §1 P0 决策门 #1）：v1 上线后出现以下任一情况时启动本 plan：
- hydrate / dehydrate 单次开销 > 5 ms / NPC（飞行 / 灵兽坐骑频繁穿越 64-256 格边界时撕裂感强）
- 玩家反馈"256 格外看不到任何 NPC"违和感影响沉浸（worldview §十一 散修江湖人来人往物理化身不足）
- 压测 or 运营数据显示二态频繁转换是 TPS 瓶颈

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（二态框架 + NpcDormantStore + `bong:npc/dormant` Redis HASH + Hysteresis 阈值决策门 #2）· `plan-npc-perf-v1.md` ✅（hydrated NPC 性能基线）· `plan-qi-physics-v1.md` P1 ✅（ledger::QiTransfer）· `plan-npc-ai-v1.md` ✅（big-brain Action + NpcLodTier 框架）

**worldview 锚点**：
- **§三:124-187 NPC 与玩家平等**：Drowsy 期 NPC 仍按 1Hz 老化 / 修炼，不因"移动中间态"豁免规则
- **§十一:947-970 散修江湖人来人往**：远视野 64-256 格可见 Drowsy NPC = 江湖人口密度物理化身
- **§二 守恒律**：Drowsy 期灵气流动与 Dormant 同等要求，**必须走 `qi_physics::ledger::QiTransfer`**

**qi_physics 锚点**：Drowsy 期 1Hz tick 灵气吸收走 `qi_physics::regen_from_zone`（同 Dormant，不同在于 Drowsy NPC 是 ECS entity 可 query `Position` 实时距离）+ 老死/战斗释放走 `qi_physics::qi_release_to_zone`。

**前置依赖**：
- `plan-npc-virtualize-v1` ✅ — 二态框架 / NpcDormantStore / Hysteresis / Redis 持久化
- `plan-npc-perf-v1` ✅ — hydrated TPS 基线（Drowsy 必须在 hydrated 稳定后才能评估是否必要）
- `plan-qi-physics-v1` P1 ✅ — ledger API 冻结

**反向被依赖**：
- `plan-npc-virtualize-v3` — dormant↔dormant 战争批量推演（Drowsy 是 v3 的可见性前提）
- `plan-narrative-political-v1` — 远视野 Drowsy NPC 为叙事丰度基础

---

## 接入面 Checklist

- **进料**：`NpcDormantStore`（v1 SoA）+ `NpcLodTier`（v1 已有 Near/Far 二档，Drowsy 加 Mid）+ `AscensionQuotaStore` + v1 Hysteresis 阈值 config
- **出料**：`NpcLodTier::Mid` 新变体 + `drowsy_tick_system`（FixedUpdate 1Hz）+ 远视野 LOD entity（client 可见低 LOD NPC）+ 6 边转换矩阵（H↔Dr / Dr↔D / H↔D）
- **共享类型**：复用 `NpcDormantSnapshot`（Drowsy↔Dormant 转换复用同一快照）/ `QiTransfer` / `NpcLodTier`；新增 `NpcDrowsyState` component
- **跨仓库契约**：server 侧新增 Drowsy LOD entity publish 到 client；client 收到低 LOD NPC 包渲染简化模型；agent 的 NpcDigest 通道已兼容（v1 设计）无需变更
- **worldview 锚点**：§十一 散修可见性 + §三 平等 + §二 守恒律
- **qi_physics 锚点**：Drowsy 期 `regen_from_zone` / `qi_release_to_zone`（同 Dormant）

---

## §0 设计轴心

- **三态阈值**：Hydrated ≤ 64 格 / Drowsy 64-256 格 / Dormant > 256 格（与 v1 二态 Hysteresis 阈值对齐，Drowsy 引入不改变外边界）
- **Drowsy 执行频率**：FixedUpdate 1Hz（对比 Hydrated Update 20Hz / Dormant GlobalTick 1/min）—— 降频但保留 ECS entity 允许 query Position / 被玩家命中时正常战斗
- **远视野 LOD 可见**：Drowsy NPC 向 client 发低 LOD packet（仅 Position + EntityKind，无装备 / 动画），client 渲染为静止低 LOD 身影
- **转换矩阵 6 边**（H↔Dr / H↔D / Dr↔D）取代 v1 二态 2 边；LOD gate 配置独立 `NpcLodConfig { near: u32, mid: u32, far: u32 }`

---

## 阶段总览

| 阶段 | 状态 | 主要交付物 | 验收标准 |
|------|------|-----------|---------|
| **P0** | ⬜ | 决策门：启动条件确认 + Drowsy 数据模型 + 6 边转换矩阵设计 | v1 实测数据 / 用户反馈满足派生条件之一 |
| **P1** | ⬜ | `drowsy_tick_system` 1Hz + Drowsy 灵气守恒 | Drowsy NPC 1Hz 老化 / 修炼；qi 守恒单测 ≥ 15 |
| **P2** | ⬜ | 远视野 LOD entity + client 渲染 + LOD gate | 64-256 格玩家可见 Drowsy NPC 低 LOD 身影 |
| **P3** | ⬜ | 三态完整 e2e + 性能验收 | 100 Hydrated + 500 Drowsy + 1000 Dormant 18+ TPS |

---

## P0 — 决策门 + 数据模型

**派生触发验收**：
- [ ] v1 P4 实测 hydrate/dehydrate 开销录档（`server/src/npc/dormant/bench.rs` 或 telemetry log）
- [ ] 确认开销 > 5ms/NPC **或** 用户反馈远视野空旷 **或** 压测 TPS 瓶颈 → 三选一

**Drowsy 数据模型**：
- [ ] `NpcLodTier::Mid` 新变体（`server/src/npc/lod.rs`）
- [ ] `NpcDrowsyState { entity: Entity, char_id: CharId, last_tick: u64 }` component（`server/src/npc/virtualize/drowsy.rs`）
- [ ] `NpcLodConfig { near: u32, mid: u32, far: u32 }` resource（阈值可配）
- [ ] 6 边转换矩阵文档化（P0 决策门收口）：
  - H → Dr：玩家离开 64 格 + 进入 256 格内（Drowsy 化）
  - Dr → H：玩家进入 64 格内（hydrate，走 v1 hydrate 路径）
  - Dr → D：玩家离开 256 格（dehydrate to Dormant）
  - D → Dr：玩家进入 256 格内（hydrate to Drowsy）
  - H → D：直接 despawn（仅玩家瞬移 / server 崩溃恢复）
  - D → H：dormant 渡虚劫强制 hydrate（跳过 Drowsy，走 v1 路径）
- [ ] ≥ 8 单测（各转换边正确触发 / Drowsy 期被玩家攻击 → 强制 H / 守恒律 Drowsy 期灵气走 ledger）

**P0 验收**：数据模型 PR 合并 + 6 边转换矩阵文档 + 8 单测 green

---

## P1 — drowsy_tick_system 1Hz

- [ ] `drowsy_tick_system`（FixedUpdate 1Hz，`server/src/npc/virtualize/drowsy.rs`）：
  - 移动推演：按 `DormantBehaviorIntent`（复用 v1）每次 tick 移动 1 格
  - 灵气吸收：`qi_physics::regen_from_zone(zone.spirit_qi, rate, integrity, room)` + emit `QiTransfer`
  - 寿元衰减：按 archetype rate_multiplier（与 Dormant 相同系数）
  - 自动境界推进：满境界自动选默认（无 UI，同 Dormant）
  - 渡虚劫条件满足 → 强制 hydrate（跳过 Drowsy，走 v1 tribulation hydrate 路径）
- [ ] Drowsy 期被玩家攻击 = 强制 hydrate（同 Dormant 规则）
- [ ] ≥ 15 单测（移动正确性 / 灵气守恒：emit QiTransfer == zone 扣减 / 寿元到期 → 老死走 plan-death §4b / 渡虚劫 hydrate trigger）

**P1 验收**：`drowsy_tick_system` 单测 green + 500 Drowsy NPC 1h in-game 灵气守恒

---

## P2 — 远视野 LOD entity + client 渲染

- [ ] Drowsy NPC 向 client 发低 LOD packet（仅 Position + EntityKind，无装备/皮肤加载）
- [ ] Client 渲染为静止低 LOD 身影（`client/src/hud/npc_lod.java`，与普通 NPC 视觉区分）
- [ ] LOD gate：Drowsy NPC 进入玩家 64 格内触发 hydrate 时切换到完整 LOD
- [ ] ≥ 5 集成测试（Drowsy NPC packet 正确发出 / client 收到后不崩溃 / hydrate 时 LOD 切换无闪烁）

**P2 验收**：client 64-256 格内可见 Drowsy NPC 低 LOD 身影

---

## P3 — 三态 e2e + 性能验收

- [ ] scripts/start.sh 默认加 `BONG_DROWSY_SEED_COUNT=500`（500 Drowsy NPC）
- [ ] CI e2e：100 Hydrated + 500 Drowsy + 1000 Dormant 30s ≥ 18 TPS
- [ ] 边界穿越压测：玩家飞行速度穿越 64/256 格边界 100 次，记录平均 hydrate/dehydrate 延迟 < 2ms/NPC
- [ ] 三态完整链路 e2e：玩家从 spawn 走到 256 格外 → 远方 Drowsy NPC LOD 可见 → 继续走到 256 格外 → NPC 转 Dormant → 玩家折返 → NPC 转 Drowsy 可见 → 进入 64 格 → NPC hydrate 完整出现

**P3 验收**：CI e2e green + 三态链路 e2e 手测通过

---

## §8 开放问题（P0 决策门收口）

1. **Drowsy LOD 可见范围**：64-256 格全段可见 vs 128-256 格才可见（减少性能开销）
2. **Drowsy NPC 皮肤加载**：低 LOD 是否加载皮肤 or 纯 EntityKind 占位（网络包大小 vs 视觉质量）
3. **Dr↔D 转换快照**：Drowsy↔Dormant 是否复用 `NpcDormantSnapshot` or 需独立 component（简单 vs 干净）
4. **Drowsy 期被天道 agent 推演**：Drowsy NPC 出现在 NpcDigest 吗（v1 已含 hydrated + dormant，Drowsy 应加入 or 合并到 dormant 通道）
5. **docs/CLAUDE.md §四 红旗**：是否加"Drowsy 期灵气未走 ledger"独立红旗 or 合并入 dormant 灵气红旗
