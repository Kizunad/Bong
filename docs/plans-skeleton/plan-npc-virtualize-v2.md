# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架 **三态扩展** —— 在 v1 二态（Hydrated ECS ↔ Dormant SoA）之间插入 **Drowsy 中间层**（ECS entity 存活但仅核心 system 1Hz tick + 远视野可见）。承接 `plan-npc-virtualize-v1` ✅ finished（二态 MVP 已落，5000+ dormant NPC 守恒推演已验收）。**本 plan 由实测触发**，v1 P3 实测后出现以下任一条件才启动：

1. hydrate / dehydrate 单次开销 > 5ms / NPC（频繁穿越边界时撕裂感强）
2. 玩家飞行 / 灵兽坐骑快速穿越 64-256 格阈值，NPC 反复 spawn/despawn 明显抖动
3. 玩家反馈「远视野空旷无 NPC」违和感强（256 格外看不到任何 NPC 影响沉浸感）
4. v1 P3 压测数据显示三态成本收益为正（详 §0 成本分析）

**若 v1 P3 实测未触发上述任一条件，本 plan 无需启动。**

**worldview 锚点**：`worldview.md §三:124-187`（NPC 与玩家平等，远方修士同样有形）· `§十一:947-970`（散修江湖人来人往，5000+ NPC 需要在远视野有物理存在感）· `§二`（守恒律——三态灵气账本须与 v1 ledger 完全对齐，不允许 Drowsy 引入新的灵气生成/消失路径）

**交叉引用**：`plan-npc-virtualize-v1` ✅（二态 MVP，本 plan 在其三态方向扩展）· `plan-npc-ai-v1` ✅（所有 Scorer / Action；Drowsy NPC 只跑核心子集）· `plan-npc-perf-v1` ✅（spatial index + navigator 分桶，Drowsy NPC 应受益于已有 LOD gate）· `plan-qi-physics-v1` P1 ✅（ledger::QiTransfer API 冻结）· `plan-qi-physics-patch-v1` P0 ✅

---

## 接入面 Checklist

- **进料**：
  - `npc::dormant::NpcDormantStore`（v1 已建，本 plan 扩展三态状态机）
  - `npc::dormant::NpcDormantSnapshot`（v1 已建，本 plan 不改字段，仅增 `tier: NpcLodTier` 枚举值）
  - 玩家 Position（已有，FixedUpdate 1Hz 采样）
  - `NpcLodTier`（v1 已有 Hydrated / Dormant，本 plan 插入 Drowsy）
  - `qi_physics::ledger::QiTransfer`（守恒律，Drowsy 灵气消耗沿用 v1 同一接口）
- **出料**：
  - `NpcLodTier::Drowsy` 新变体（插入 Hydrated / Drowsy / Dormant 三值）
  - `drowsy_tick_system`（FixedUpdate 1Hz，只跑核心 system：寿元衰减 / 灵气吸收 / 境界自动推进；跳过战斗 / AI Scorer / navigator 完整路径规划）
  - `drowse_npc_system`（Hydrated → Drowsy 降级：玩家离开 64 格进入 64-256 格段）
  - `awaken_npc_system`（Drowsy → Hydrated 升级：玩家靠近 ≤ 64 格）
  - `drowsy_sink_system`（Drowsy → Dormant 降级：玩家退出 256 格）
  - `dormant_drowse_system`（Dormant → Drowsy 升级：玩家进入 64-256 格段；NPC 需远视野可见时使用）
  - LOD gate 配置扩展（`NpcLodConfig` 增 `drowsy_inner_threshold` / `drowsy_outer_threshold` 字段）
- **共享类型 / event**：
  - 复用 `NpcLodTier`（v1 已建）扩一个变体，**不另建 struct**
  - 复用 `qi_physics::ledger::QiTransfer`（Drowsy 灵气收支与 v1 同通道）
  - 复用 `bong:npc/death`（Drowsy 老死同通道）
  - **不新建** Drowsy 专属 Component / Event，所有状态统一由 `NpcLodTier` 枚举 + 现有 NpcDormantSnapshot 覆盖
- **跨仓库契约**：
  - server: `npc::lod::*` 三态调度核心（Drowsy 在 ECS 存在 → Valence 协议自然 spawn entity）
  - agent: 无新增 schema（NpcDigest 视 Drowsy 为 hydrated 的简化版，字段兼容）
  - client: 无感知变化（Drowsy ECS entity 走 Valence 协议，client 可见；全力 Hydrated 与 Drowsy 的渲染行为由 server 控制，client 无需区分）
- **worldview 锚点**：§三:124-187 · §十一:947-970 · §二 守恒律
- **qi_physics 锚点**：`qi_physics::ledger::QiTransfer`（Drowsy 灵气消耗必须走 ledger，与 v1 同账本）· `qi_physics::regen_from_zone`（Drowsy NPC 修炼吸收 zone qi，同 v1 dormant 路径）

---

## §0 设计轴心与成本分析

**三态成本（必须在 P0 决策门实测后确认收益为正）**

| 指标 | 二态（v1） | 三态（v2） | 阈值 |
|------|------------|------------|------|
| 状态转换矩阵边数 | 2（H↔D） | 6（H↔Dr / Dr↔D / H↔D 直接快速通道） | — |
| FixedUpdate 调度套数 | 2（Update 20Hz + GlobalTick 1/min） | 3（+FixedUpdate 1Hz Drowsy tick） | — |
| hydrate/dehydrate 开销 | 全量 ECS spawn/despawn | H↔Dr 仅改 LOD tier + 解注册 Scorer；Dr↔D 保留 ECS entity 只推演数据 | 目标 Dr↔D < 0.5ms |
| NPC 远视野可见度 | 0（>256 格全不可见） | Drowsy ECS entity 存在，可见（Valence 协议） | — |

**成本权衡红线**：若三态后 `drowse_tick_system` 单帧开销 > Drowsy NPC 数 × 0.1ms，或引入任何灵气守恒破缺，则本 plan **回退二态，关闭 Drowsy**。

**Drowsy 核心约束**：
- Drowsy NPC **不参与战斗**（ECS entity 存在但所有 CombatScorer 返回 0）
- Drowsy NPC **不执行完整 navigator A\***（无路径规划，仅朝意图方向 1Hz 微步，最多 2 格/tick）
- Drowsy NPC **灵气走 ledger**（不允许 `qi_current += X` 不记账）
- Drowsy NPC **受玩家攻击 = 立即升级 Hydrated**（与 v1 dormant 受攻击行为一致）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|------|------|------|
| **P0** ⬜ | **决策门：v1 P3 实测触发确认 + 三态成本评估** | 确认触发条件 ≥1 条 / 成本收益分析写入 plan §0 |
| **P1** ⬜ | **NpcLodTier 扩展 + 三态转换系统** | 6 条转换边正确触发；Drowsy ECS entity 可见 |
| **P2** ⬜ | **Drowsy 1Hz tick + 灵气守恒** | Drowsy NPC 寿元/灵气推演 / 老死通道 / ledger 全走；≥ 20 单测 |
| **P3** ⬜ | **性能验收 + Hysteresis 调参** | 飞行穿越边界无抖动；200 Hydrated + 500 Drowsy + 1000 Dormant ≥ 18 TPS |

---

## §2 数据模型变更

```rust
// v1 已有 NpcLodTier，v2 插入 Drowsy
pub enum NpcLodTier {
    Hydrated,  // ECS entity + 全 system（已有）
    Drowsy,    // ECS entity + 仅核心 system 1Hz（新增）
    Dormant,   // SoA 纯数据（已有）
}

// LOD 阈值配置扩展（v1 原 hysteresis_inner/outer）
pub struct NpcLodConfig {
    pub hydrated_inner:  f64,  // 进入 Hydrated 阈值（默认 64）
    pub hydrated_outer:  f64,  // 退出 Hydrated 阈值（默认 80，防抖）
    pub drowsy_inner:    f64,  // 进入 Drowsy 阈值（默认 100）
    pub drowsy_outer:    f64,  // 退出 Drowsy 阈值（默认 256）
    pub dormant_outer:   f64,  // 退出 Dormant 范围（默认 280，防抖）
}
```

状态机转换顺序（Drowsy 作为中间缓冲）：
```
玩家接近：Dormant → Drowsy（256格）→ Hydrated（64格）
玩家离开：Hydrated → Drowsy（80格）→ Dormant（280格）
强制升级：Dormant/Drowsy → Hydrated（攻击 / 渡虚劫）
```

---

## §3 核心 system 清单（Drowsy 层跑的子集）

| system | Drowsy 跑？ | 频率 | 说明 |
|--------|------------|------|------|
| `lifespan_aging_system` | ✅ | 1Hz | 寿元衰减（与 dormant 同逻辑） |
| `cultivation_regen_system` | ✅ | 1Hz | 灵气吸收（走 ledger） |
| `cultivation_auto_breakthrough_system` | ✅ | 1Hz | 自动境界推进（无 UI 默认路径） |
| `dormant_position_drift` | ✅ | 1Hz | 微步位移（朝 DormantBehaviorIntent 方向 ≤2格/tick） |
| `tribulation_check_system` | ✅ | 1Hz | 渡虚劫条件检查 → 触发强制 Hydrate |
| `npc_ai_scorer_*` | ❌ | — | Drowsy 不跑 big-brain Scorer |
| `navigator_astar` | ❌ | — | Drowsy 不做完整路径规划 |
| `combat_*` | ❌ | — | Drowsy 不参与战斗 |
| `social_* / faction_*` | ❌ | — | Drowsy 不做社交 AI |

---

## §4 灵气守恒约束（与 v1 §3 同级红线）

所有 Drowsy NPC 灵气消耗**必须**走 `qi_physics::ledger::QiTransfer`：

```rust
// drowsy_tick_system 内灵气修炼（与 v1 dormant_global_tick 对齐）
let (gain, drain) = qi_physics::regen_from_zone(
    zone.spirit_qi,
    npc_cultivation.regen_rate,
    npc_cultivation.meridian_integrity,
    zone.capacity_room(),
)?;
ledger.record(QiTransfer::new(
    QiAccountId::zone(zone_id),
    QiAccountId::npc(char_id),
    gain,
    TransferReason::CultivationRegen,
))?;
```

**禁止**：`npc.cultivation.qi_current += gain`（无 ledger 记账）  
**禁止**：Drowsy NPC 老死时灵气凭空消失（必须走 `qi_physics::qi_release_to_zone`）

---

## §5 开放问题（P0 决策门收口）

1. **NpcLodConfig 阈值粒度**：hydrated_outer=80 / drowsy_outer=256 的 Hysteresis 窗口是否足够防抖？还是需要更大间距（如 outer=300）？实测飞行速度 + server tick 率后定。
2. **Drowsy → Dormant 时 ECS entity 处理**：直接 despawn entity（与 v1 dehydrate 同）vs 保留 ghost entity 仅移除 component（避免 Valence 协议重 spawn 开销）？成本门槛 Dr↔D < 0.5ms 的哪种实现更优？
3. **远视野 Drowsy 可见性范围**：player chunk view distance 通常 8-12 chunk（128-192格），256格 Drowsy 阈值是否刚好在玩家视野边缘？需对照 server chunk 发包策略。
4. **Drowsy NPC 被攻击的即时 Hydrate 路径**：攻击命中 Drowsy entity → trigger `NpcHydrateRequest` → 同帧 Hydrate 还是下一 FixedUpdate 帧？延迟 1 tick 是否引起「攻击空气」的帧同步问题？
5. **docs/CLAUDE.md §四 红旗扩展**：是否加「Drowsy NPC 灵气未走 ledger」为新红旗？与 v1 dormant 同级？
6. **plan-npc-virtualize-v3 启动条件（决策门 #6）**：本 plan P3 实测后若发现 Drowsy NPC 间互动（同 zone 两 Drowsy NPC 相遇的碰撞 / 战斗意图）处理逻辑超出预期复杂度 → 启动 v3（dormant↔dormant / drowsy↔drowsy 批量推演）

---

## §6 前置 / 反向被依赖

**前置依赖**：
- `plan-npc-virtualize-v1` ✅（二态 MVP 已落，本 plan 在其之上扩展）
- `plan-npc-perf-v1` ✅（spatial index 已建，Drowsy LOD gate 受益）
- `plan-qi-physics-v1` P1 ✅（ledger API 冻结）
- `plan-qi-physics-patch-v1` P0 ✅（守恒律迁移完成）

**反向被依赖**：
- `plan-npc-virtualize-v3`（决策门 #6 触发后启动，见 §5.6）
- `plan-narrative-political-v1`（远视野 Drowsy NPC 存在感是叙事丰度物理基础）
- 任何依赖"玩家可远眺看到 NPC 存在"的体验 plan

---

## §7 验收指标

- **功能**：飞行穿越 64-256 格段，NPC 平滑从 Hydrated 降为 Drowsy，无 spawn/despawn 闪烁
- **性能**：200 Hydrated + 500 Drowsy + 1000 Dormant 同服，TPS ≥ 18
- **守恒律**：1h in-game 推演，所有 Drowsy NPC 灵气 ledger 收支与 zone.spirit_qi 收支完全匹配（delta ≤ f64 精度误差）
- **单测**：≥ 20 个覆盖：三态转换正确性 / 强制 Hydrate 路径（攻击+渡劫）/ Drowsy 老死 ledger 守恒 / LOD Hysteresis 防抖

---

## Finish Evidence

（本 plan 完成全部阶段并 merge 后填写）
