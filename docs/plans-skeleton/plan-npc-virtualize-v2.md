# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架**三态扩展**：在 v1 二态（Hydrated ECS ↔ Dormant SoA，64 格 / 256 格双阈值）基础上引入 **Drowsy 中间态**（64-256 格范围：ECS entity 存续但仅运行核心 system，1Hz FixedUpdate tick + 保持远视野可见），消除频繁穿越 64 格边界时的 spawn/despawn 抖动与开销尖峰，并解决"256 格外完全无 NPC 可见"的违和感。

**触发条件（v1 P3 实测后，任一满足即启动本 plan）**：来源 `plan-npc-virtualize-v1` §0 / §8 决策门 #1：
1. hydrate / dehydrate 单次开销 > 5 ms / NPC（快速移动玩家频繁穿越边界撕裂感强）
2. 玩家移动快（飞行 / 灵兽坐骑）时，64 格阈值附近 NPC 反复 spawn/despawn 视觉抖动
3. 玩家反馈"远视野空旷无 NPC"影响沉浸感（256 格外完全看不到任何 NPC）

**前置依赖**：`plan-npc-virtualize-v1` ✅（二态 MVP 实装、P3 实测数据、`DormantNpcStore` SoA 基础设施齐备后才有启动意义）

**worldview 锚点**：
- §十一:947-970 散修江湖——远处 NPC 隐约可见，符合"人来人往"世界密度感知
- §三:124-187 NPC 与玩家平等——Drowsy NPC 仍受寿元老化 / 灵气消耗规则约束，不因距离豁免
- §二 真元守恒——Drowsy 状态 qi 消耗必须走 `qi_physics::ledger::QiTransfer`，不凭空消失

**qi_physics 锚点**：
- `ledger::QiTransfer`（Drowsy 核心 tick 中 qi 消耗/生成走账）
- `excretion::container_intake` / `release::release_to_zone`（Drowsy 被动修炼/逸散路径同 Dormant）
- `distance::attenuation`（Drowsy NPC 所在 zone 灵气距离衰减，tick 时按 zone center 查询）

---

## 接入面 Checklist

- **进料**：
  - `npc::virtualize::NpcVirtualizationState { Hydrated, Dormant }` → 扩展为三态 `{ Hydrated, Drowsy, Dormant }`
  - `npc::virtualize::HydrateRequest` / `DehydrateRequest`（v1 已建）→ 扩充 `DrownRequest` / `WakeRequest` / `RouzeRequest`（Dormant→Drowsy）
  - `npc::virtualize::DormantNpcStore`（SoA）→ 复用；Drowsy NPC 保留 ECS entity，不进 SoA
- **出料**：
  - Drowsy NPC ECS entity（仅含 `Position` / `NpcCoreStats` / `SkinId` / `NpcVirtualizationState` 核心 component，剥离 big-brain / combat / farming system）
  - 远视野 spawn packet（Drowsy NPC 位置广播给 ≤ 256 格内玩家客户端，每 20 tick 一次）
  - `NpcDrownedEvent` / `NpcWokenEvent` / `NpcRouzedEvent`（系统监控 + 可选 agent narration 触发点）
- **共享类型**：
  - 复用 v1 `DormantNpcStore` Hysteresis 机制（`HYDRATE_THRESHOLD` / `DEHYDRATE_THRESHOLD`）
  - 新增 `DROWN_THRESHOLD = 64.0` / `WAKE_THRESHOLD = 60.0` / `SLEEP_THRESHOLD = 280.0` / `ROUSE_THRESHOLD = 240.0`（防抖不对称，防止 64 格边界抖动）
- **跨仓库契约**：
  - server: `npc::virtualize` 三态状态机 + Drowsy FixedUpdate system（1Hz）+ 位置广播 system
  - client: 已有远视野 entity render 支持；需评估 1Hz 位置更新下 NPC 移动卡顿感（客户端线性插值 lerp 200ms 窗口）
  - agent: `NpcDrownedEvent` 对天道 agent 无感，不需要 agent 层改动
- **worldview 锚点**：§二 + §三:124 + §十一:947
- **qi_physics 锚点**：`ledger::QiTransfer` + `excretion` + `distance::attenuation`

---

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 决策门：v1 P3 实测数据收集，三触发条件评估，确认是否启动 | ⬜ |
| P1 | 三态状态机扩展（六边转换矩阵 + Hysteresis 防抖 + event 扩充）| ⬜ |
| P2 | Drowsy 核心 system（1Hz FixedUpdate：qi / 寿元 / 随机游走）| ⬜ |
| P3 | 远视野可见（Drowsy entity spawn packet + 评估 client 位置插值）| ⬜ |
| P4 | 性能验收（100 Hydrated + 500 Drowsy + 5000 Dormant，18+ TPS）| ⬜ |

---

## §0 设计轴心

**三态与六边转换矩阵**：

| 转换方向 | 触发距离 | Event | 说明 |
|----------|----------|-------|------|
| Hydrated → Drowsy | > 64 格 | `DrownRequest` | 正常移远 |
| Drowsy → Hydrated | ≤ 60 格（防抖）| `WakeRequest` | 玩家接近 |
| Drowsy → Dormant | > 280 格（防抖）| `DehydrateRequest` | 移得更远 |
| Dormant → Drowsy | ≤ 240 格（防抖）| `RouzeRequest` | 玩家接近中 |
| Hydrated → Dormant | 强制（server 关闭 / zone 卸载）| `ForceDehydrateRequest` | 批量强制 |
| Dormant → Hydrated | 关键事件（渡虚劫 / 叙事触发）| `ForceHydrateRequest` | 关键路径 |

**Drowsy system 极简原则（1Hz FixedUpdate）**：

- ✅ 寿元老化（`rate_multiplier = 0.15`，介于 Hydrated 0.3 / Dormant 0.1 之间）
- ✅ qi 被动消耗 / 生成（走 `ledger::QiTransfer`，zone EnvField 距离衰减）
- ✅ 随机游走（每 60s 随机偏移 ≤ 4 格，不走 A*）
- ✅ 位置广播（每 20 tick 发一次位置 packet 给 ≤ 256 格内玩家）
- ❌ big-brain Scorer / Action（全部停用）
- ❌ 战斗 / 招式 / NPC 间社交 / 灵田耕作
- ❌ 突破（突破必须 ForceHydrate 进入 ECS 完整执行）

**Drowsy ECS component 最小集（Drown 时剥离）**：

- 保留：`Position`、`NpcCoreStats`（qi / 寿元）、`SkinId`、`NpcVirtualizationState`
- 剥离：`Brain`、`NpcActionSet`、`Combatant`、`FarmingBrain`、`Navigator`

**P3 视觉规格（Drowsy 远视野）**：

- Drowsy NPC 向 ≤ 256 格玩家发 vanilla `spawn_entity` + `set_entity_metadata` packet（仅皮肤 + 位置）
- 1Hz 位置更新频率 → 客户端看到 NPC 每秒"瞬移"；需评估客户端 lerp 插值（200ms 窗口）是否需要 Fabric 侧改动
- Drowsy NPC 无 nameplate / 无 healthbar（与 Hydrated 区分，降低信息密度）
- 无新增粒子 / 音效（纯协议层变化，无 VFX 需求）

---

## §7 开放问题（P0 决策门收口）

1. Drowsy `rate_multiplier` 取 0.15（折中）还是直接复用 Dormant 的 0.1（简化实现）？
2. Drowsy ECS entity 在 server crash recovery 时如何恢复（v1 Dormant 走 Redis `bong:npc/dormant`，Drowsy 是否需要独立持久化槽位）？
3. Drowsy → Dormant 时 `Position` 写入 `DormantNpcStore` SoA 的原子性保证（避免写到一半 crash 导致 NPC 双份）
4. 客户端位置插值：是否需要 Fabric 侧改动，还是服务端 20-tick 位置广播间隔已足够平滑（20 tick ≈ 1s，等于 Drowsy tick 频率，可能已够）？
5. 化虚 NPC 在 Drowsy 状态下被其他玩家触发渡虚劫：应直接 Wake（正常 Drowsy→Hydrated）还是 ForceHydrate（跳过 Hysteresis 防抖立即上线）？
6. 两个 Drowsy NPC 距离很近时的互动：v1 选"全权交天道推演"，v2 是否升级为服务端 collision 检测？（→ 决策门 #6：选 C 时派生 plan-npc-virtualize-v3）
7. 派生 `plan-npc-virtualize-v3` 占位（Drowsy↔Dormant NPC 批量战争推演），决策门 #6 选 C 时启动
