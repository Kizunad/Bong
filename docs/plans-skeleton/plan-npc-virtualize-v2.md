# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架 **Drowsy 三态扩展**。承接 plan-npc-virtualize-v1 ✅（Hydrated ↔ Dormant 二态 MVP）—— v2 在 64-256 格区间引入 **Drowsy 中间态**（ECS entity 存活但仅核心 system 1Hz tick + 远视野可见），解决 v1 二态在高速玩家穿越 / 远视野体验场景的撕裂感。

**触发本 plan 的条件（v1 P3 实测后出现以下任一）**：

1. hydrate / dehydrate 单次开销 > 5 ms / NPC（频繁边界穿越时撕裂感强）
2. 玩家高速移动（飞行 / 灵兽坐骑）频繁穿越 64-256 格阈值，反复 spawn/despawn 抖动
3. 玩家反馈"远视野空旷无 NPC"违和（256 格外看不到任何 NPC 影响沉浸感）

**如无以上情况，本 plan 可无限期推迟。**

---

**世界观锚点**：`worldview.md §十一:947-970 散修江湖人来人往`（远视野可见 NPC 的叙事质感）· `§三:124-187 NPC 与玩家平等`（Drowsy NPC 仍须按规则老化，不因 LOD 豁免）· `§二 真元守恒`（Drowsy 态灵气流动仍须走 ledger）

**前置依赖**：

- `plan-npc-virtualize-v1` ✅ P3 实测完成 → 触发条件出现后才启动本 plan
- `plan-npc-perf-v1` ✅（hydrated 性能基线稳定）
- `plan-npc-fixups-v1` ✅ + `plan-npc-fixups-v2` ✅（ECS lifecycle 卫生规则已立）
- `plan-qi-physics-v1` P1 ✅ / `plan-qi-physics-patch-v1` P0/P1/P2 ✅

**反向被依赖**：

- `plan-npc-ai-v1` §3.3 1000 NPC stretch goal（Drowsy 让 1000 NPC 中 ~300 处于 Drowsy，视野更丰富）
- `plan-narrative-political-v1` → 远视野可见 NPC 提升世界叙事密度

---

## 接入面 Checklist

- **进料**：plan-npc-virtualize-v1 的 `HydratedNpc` / `DormantSnapshot` / `NpcVirtualizationConfig`（Hysteresis 阈值）/ `qi_physics::ledger::QiTransfer`（Drowsy 态灵气 1Hz tick）/ `NpcLodTier`（plan-npc-ai-v1 已有，扩展新档）
- **出料**：`DrowsyNpc` marker component 🆕 / `DrownedToHydratedEvent` 🆕 / `DrowsyToDormantEvent` 🆕 / Drowsy → Hydrated / Dormant 双向转换 system / 远视野可见渲染接口（client payload）
- **共享类型**：扩展 `NpcLodTier` enum 加 `Drowsy` 变体；复用 v1 `NpcVirtualizationConfig` 加 Drowsy 阈值字段
- **跨仓库契约**：
  - server: `npc::virtualize_v2::DrowsySystem` + `NpcLodTier::Drowsy` 变体
  - client: 远视野 LOD 渲染（仅外观，不跑 AI / physics）
  - agent: NpcDigest 远视野可见字段（`is_drowsy: bool`）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：Drowsy NPC 1Hz tick 走 `qi_physics::regen_from_zone` 吸收 + `qi_physics::ledger::QiTransfer` 记账（不得绕过，同 dormant 规则）

---

## §0 设计轴心

### 三态状态机

```
Dormant (SoA, > 256 格)
    ↑ hydrate-on-approach
    |
Drowsy (ECS entity, 64-256 格, 1Hz core tick only)
    ↑ full-hydrate
    |
Hydrated (ECS entity, ≤ 64 格, 20Hz full tick)
```

**Hysteresis（防抖）**：
- Dormant → Drowsy：玩家进入 < 240 格（不是 256，避免边界抖动）
- Drowsy → Hydrated：玩家进入 < 56 格（不是 64）
- Hydrated → Drowsy：玩家退出 > 72 格（迟滞 8 格）
- Drowsy → Dormant：玩家退出 > 272 格（迟滞 16 格）

### Drowsy 系统调度（FixedUpdate 1Hz）

| System | 频率 | 说明 |
|---|---|---|
| `drowsy_aging_system` | 1Hz | 寿元老化（rate_multiplier=0.1，比 hydrated 0.3 更慢） |
| `drowsy_qi_regen_system` | 1Hz | `regen_from_zone` 灵气吸收 + ledger 记账 |
| `drowsy_tribulation_check` | 1Hz | 渡劫条件满足 → 强制 hydrate（同 v1 dormant 规则） |
| `drowsy_visibility_update` | 1Hz | 向 client 发送远视野可见位置（位置 + 外观只，无 AI 状态） |

### 三态 vs 二态成本权衡

| 维度 | 二态 v1 | 三态 v2 |
|---|---|---|
| 转换矩阵 | H↔D 2 边 | H↔Dr / Dr↔D / H↔D 6 边 |
| LOD gate | 1 套阈值 | 3 套阈值（双 Hysteresis） |
| FixedUpdate 调度 | 2 套（20Hz / 1/min） | 3 套（20Hz / 1Hz / 1/min） |
| 实现复杂度 | 低 | 中 |
| 远视野效果 | 无 NPC 可见 | Drowsy NPC 可见 |

---

## §1 阶段规划

| 阶段 | 内容 | 状态 |
|---|---|---|
| P0 | 决策门：v1 P3 实测触发条件确认 → 本 plan 解锁；三态阈值拍板；DrowsyNpc component 定义 | ⬜ |
| P1 | Dormant → Drowsy 转换（ECS spawn + 核心 component 恢复，无 AI system）+ Drowsy 1Hz tick（aging / qi）| ⬜ |
| P2 | Drowsy → Hydrated 快速 full-hydrate（复用 v1 hydrate 逻辑）+ Drowsy → Dormant dehydrate（序列化 → SoA）| ⬜ |
| P3 | 远视野可见渲染接口（client payload + NpcDigest `is_drowsy`）+ 实测性能基线对比 v1 | ⬜ |

## §2 验收标准

- 100 hydrated + 300 drowsy + 1000 dormant 5min 18+ TPS（与 v1 基线相比不退步）
- 高速穿越测试：飞行玩家从 0 → 300 格来回 10 次 → hydrate/dehydrate 次数 ≤ drowsy buffer（抖动消除）
- 远视野：玩家在 256 格处可见 drowsy NPC 外观占位（无 AI 动作，仅位置）
- qi_physics 守恒：5000 tick drowsy qi 守恒 e2e（与 v1 dormant 守恒测试同格调）

## §3 开放问题（P0 决策门收口）

1. **Hysteresis 阈值**：240/56/72/272 是占位，实测后调整；是否从 `NpcVirtualizationConfig` 动态配置
2. **Drowsy aging rate**：0.1 vs 0.2（比 dormant 0.05 快但比 hydrated 0.3 慢——中间态哲学）
3. **远视野可见渲染**：client 侧是真实 entity 还是假占位 particle（影响 client PR 范围）
4. **Drowsy ↔ Dormant 互动边界**：两个 Drowsy NPC 在 64-256 格范围相遇，是否需要简单碰撞推演（v1 dormant 靠 sequential regen 天然规避，Drowsy 有 ECS entity 可能要显式处理）
5. **`docs/CLAUDE.md §四`**：是否加"Drowsy NPC 灵气未走 ledger"红旗（与 v1 dormant 同级）
6. **派生 plan-npc-virtualize-v3 条件**：v2 P3 实测后若 dormant↔dormant 战争批量推演需求强烈 → 启动 v3
