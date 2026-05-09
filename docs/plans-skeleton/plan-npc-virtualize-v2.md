# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架 **三态扩展** —— 在 plan-npc-virtualize-v1（Hydrated↔Dormant 二态 MVP）基础上引入 **Drowsy 中间态**：ECS entity 仍存在，但仅跑 core system 白名单（1Hz FixedUpdate + 远视野可见），消除 64-256 格玩家穿越边界时的 spawn/despawn 撕裂感，并给飞行 / 灵兽坐骑玩家提供平滑 LOD 缓冲层。

**本 plan 是 v1 P3 实测评估的派生**。v1 §0 决策门 #1 验收后，出现以下任一情况时启动：
- hydrate / dehydrate 单次开销 > 5ms / NPC（频繁穿越边界时撕裂感强）
- 玩家飞行 / 灵兽坐骑快速穿越 64-256 格阈值，反复 spawn/despawn 抖动
- 玩家反馈"远视野空旷无 NPC"违和感（256 格外看不到任何 NPC 影响沉浸感）

**三态成本说明**：比二态多一套转换矩阵（H↔Dr / H↔D / Dr↔D 共 6 边 vs 二态 2 边）+ LOD gate 配置 + 3 套 FixedUpdate 调度（20Hz / 1Hz / 1/in-game-60s）。务必先验证 v1 实测数据，确认需要再启动本 plan。

**世界观锚点**：
- **§十一:947-970 散修江湖**：5000+ NPC 总量基础，远视野可见是"人来人往"叙事的视觉化身
- **§三:124-187 NPC 与玩家平等**：Drowsy NPC 仍按 rate_multiplier=0.7 老化 / qi regen，不因"玩家不在附近"豁免规则
- **§二 真元守恒**：Drowsy NPC 灵气流动仍走 `qi_physics::ledger::QiTransfer`，切换 FixedUpdate 频率不影响守恒律

**qi_physics 锚点**（同 plan-npc-virtualize-v1，Drowsy 态语义不变，仅频率降为 1Hz）：
- `qi_physics::ledger::QiTransfer::new(from, to, amount, reason)` —— 所有灵气转移记账
- `qi_physics::regen_from_zone(zone_qi, rate, integrity, room) -> (f64, f64)` —— Drowsy qi regen（1Hz tick）
- `qi_physics::qi_release_to_zone(amount, from, zone, zone_current, zone_cap)` —— Drowsy 老死释放灵气

**交叉引用**：
- `plan-npc-virtualize-v1.md` ⏳（二态 MVP 基础，P3 实测是本 plan 触发前提）
- `plan-npc-perf-v1.md` ✅（SpatialIndex + navigator 分桶 + FixedUpdate 节流，是 Drowsy LOD gate 的性能底盘）
- `plan-npc-ai-v1.md` ✅（big-brain Scorer / Action 框架，Drowsy core system 白名单从此 whitelist）
- `plan-npc-fixups-v3.md` ✅（ECS lifecycle 卫生规则：Without<Despawned> + Action 超时，Drowsy 态转换必须继承）

**前置依赖**：
- `plan-npc-virtualize-v1` ⏳ P3 全量完成 + 实测评估触发条件满足
- `plan-npc-perf-v1` ✅（SpatialIndex / navigator 分桶已落）
- `plan-qi-physics-v1` P1 ✅（ledger / excretion / release API 冻结）

**反向被依赖**：
- `plan-npc-virtualize-v3`（占位：dormant↔dormant 战争批量推演，仅在 v2 Drowsy 稳定后评估）
- `plan-narrative-political-v1`（远视野 Drowsy NPC 可见是"人来人往"叙事丰度的视觉前提）
- `plan-quest-v1`（占位：Drowsy NPC 触发 hydrate-on-demand 任务派发，v2 比 v1 迟钝度更低）

---

## 接入面 Checklist

- **进料**：
  - `NpcDormantStore` Resource（v1 已有，Drowsy NPC 仍是 ECS entity，不进 SoA）
  - `NpcLodTier` Component（v1 有 `Near` / `Far`；本 plan 加 `Drowsy` variant）
  - 玩家 Position（1Hz 采样，Drowsy 判断用）
  - v1 实测 hydrate/dehydrate 耗时 benchmark 数据（P0 决策门输入）
  - big-brain Action / Scorer 注册表（决定 core system 白名单）
- **出料**：
  - **`NpcLodTier::Drowsy`** variant（ECS entity 存在 + 仅核心 system 1Hz）
  - **`drowsy_tick_system`**（FixedUpdate 1Hz：跑 core system 白名单；不跑 Scorer / NavMesh）
  - **4 条状态转换边**：`hydrate_to_drowsy_system` / `drowsy_to_hydrated_system` / `drowsy_to_dormant_system` / `dormant_to_drowsy_system`（各带 Hysteresis 防抖）
  - **保留 2 条强制边**（不依赖距离）：D→H（渡劫强制 hydrate）/ H→D（服务器 shutdown）
  - Drowsy NPC 远视野可见（P0 决策门：发 Valence 实体 packet 简化渲染 vs 服务端 LOD 不发）
- **共享类型 / event**：
  - 扩展 `NpcLodTier` enum（新加 `Drowsy`，复用现有 `Near` / `Far`）
  - 复用 v1 `NpcDormantSnapshot` / `DormantBehaviorIntent`（Drowsy 转 Dormant 时沿用，Dormant→Drowsy 反向）
  - 复用 `DormantSeveredAt` event（Drowsy 期间 SEVERED 事件走相同通道）
  - **不新建** Drowsy 专属 SoA / Component（Drowsy 仍是完整 ECS entity）
- **跨仓库契约**：
  - server: `npc::dormant::lod` 模块扩展（`NpcLodTier` + 4 系统）
  - agent: 无变化（NpcDigest 通道 unchanged；Drowsy NPC 同 Hydrated 一样出 NpcDigest）
  - client: 远视野可见性取决于 P0 决策门选 A/B；如选 A（发实体 packet），client 收到 Valence 实体自然渲染，无额外改造
- **worldview 锚点**：见头部
- **qi_physics 锚点**：见头部

---

## §0 三态架构设计

### 状态定义

```
Hydrated  ── ECS 全量（Update 20Hz）── ≤ 64 格
    ↕                                     Hysteresis 防抖
Drowsy    ── ECS 核心白名单（1Hz）── 64-256 格
    ↕                                     Hysteresis 防抖
Dormant   ── SoA（GlobalTick 1/in-game-60s）── > 256 格
```

**v2 引入的 6 条转换边**（vs v1 的 2 边）：

| 边 | 触发 | 类型 |
|---|---|---|
| H→Dr | NPC 距最近玩家 > 64 格持续 N tick | 距离触发（带 Hysteresis） |
| Dr→H | NPC 距最近玩家 ≤ 64 格持续 N tick | 距离触发 |
| Dr→D | NPC 距最近玩家 > 256 格持续 N tick | 距离触发 |
| D→Dr | NPC 距最近玩家 ≤ 256 格持续 N tick | 距离触发 |
| D→H | 渡劫条件满足（强制 hydrate） | 事件触发（无距离要求） |
| H→D | 服务器 shutdown / 管理员命令 | 强制触发（不经 Drowsy） |

> Hysteresis 防抖参数（待 P0 决策门确认）：H↔Dr 持续 N=3 tick 才转换；Dr↔D 持续 N=5 tick。

### Drowsy core system 白名单（候选，P0 决策门确认）

**跑**（worldview §三 NPC 平等原则不豁免）：
- `LifespanComponent` 老化（rate_multiplier=0.7，介于 Hydrated 1.0 / Dormant 0.3 之间；待 P0 拍板）
- `Cultivation` qi regen（`regen_from_zone(...)` 1Hz 推演，守恒律不变）
- `Position` 惯性推移（保持最后移动向量 + 简单碰撞检测，不做 AI 寻路）
- `MeridianSeveredPermanent` 状态维护（Drowsy 期 SEVERED 事件仍要写入持久化）

**不跑**（省 tick 的核心）：
- big-brain Scorer / Action（Drowsy NPC 无新决策，位置靠惯性推移）
- NavMesh A\*（不寻路）
- VFX / Audio emit（节省 packet 带宽）
- `socialize_action_system` / `patrol_action_system`（非核心）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：v1 P3 实测数据 → 确认触发条件 + core system 白名单 + 远视野可见性选项 + Hysteresis 参数 | 实测数据满足触发条件 + 白名单文档 + P0 决策门 6 问收口 |
| **P1** ⬜ | 实装三态：`NpcLodTier::Drowsy` + `drowsy_tick_system`（白名单 1Hz）+ 4 条距离转换边 + Hysteresis 防抖 | 100 hydrated + 500 drowsy + 1000 dormant 5min ≥18 TPS；穿越 64/256 格边界无 spawn/despawn 抖动 |
| **P2** ⬜ | 远视野可见性：Drowsy NPC 发 Valence 实体 packet（简化渲染，无 AI 动画）+ 玩家视野内看得到 | 256 格范围内 Drowsy NPC 可见，玩家无"空旷违和感"；无撕裂 |
| **P3** ⬜ | e2e 验证：qi 守恒 + 老化正确 + 渡劫强制 H + 全 v1 回归 | 5000 dormant + 500 drowsy 1h in-game qi 守恒 e2e green；全部 v1 回归通过 |

---

## §2 数据契约

- [ ] `server/src/npc/dormant/lod.rs` — `NpcLodTier::Drowsy` variant 加入 enum（不破坏 Near/Far 现有 match 分支）
- [ ] `server/src/npc/dormant/drowsy_tick.rs` — `drowsy_tick_system`（FixedUpdate 1Hz，白名单 system 组合）
- [ ] `server/src/npc/dormant/transitions.rs` — 4 条距离转换边 system + Hysteresis 防抖计数器 component
- [ ] `server/src/npc/dormant/mod.rs` — 注册新 system 到 FixedUpdate set + LOD gate 配置
- [ ] `agent/packages/schema/` — NpcDigest 通道无变化（Drowsy NPC 同 Hydrated，均出完整 NpcDigest）

---

## §3 强约束（ECS lifecycle 卫生，继承自 plan-npc-fixups-v3 §3）

1. **所有转换 system query 必加 `Without<Despawned>`**（Drowsy→Dormant dehydrate 不能误操作已软删 entity）
2. **Drowsy core system 非白名单 system 运行 = 红旗**：FixedUpdate 1Hz set 里出现 `socialize_action_system` / `patrol_action_system` 等 → CI grep 拦截
3. **Drowsy 灵气流动必走 ledger**：白名单内 `regen_from_zone` 调用完必伴随 `QiTransfer::new(...)` 记账（不允许 `drowsy.cultivation.qi_current += X` 裸写）
4. **Drowsy↔Dormant 转换时 `NpcDormantSnapshot` 字段完整性**：任何 Drowsy 转 Dormant 必须 collect 全部持久化字段（含 `dimension: Identifier`，同 v1 P1 必加字段），漏字段 = 阻塞 merge

---

## §4 开放问题（P0 决策门收口）

- [ ] **远视野可见性**：选项 A = 发 Valence 实体 packet（有皮肤，无骨骼动画，服务端驱动位置）；选项 B = 服务端 LOD 仅省 tick，客户端 256 格外看不见任何 NPC。选项 A 消耗更多 client packet 带宽，选项 B 沉浸感更差。P0 拍板后决定 P2 的工作量
- [ ] **Drowsy 老化 rate_multiplier**：0.7（过渡值，Hydrated 1.0 / Dormant 0.3 中间）vs 1.0（Drowsy 已是 ECS entity 不应区别对待）？worldview §三 倾向 1.0（NPC 不豁免）但性能测试可能要求折中
- [ ] **Drowsy 渡劫触发**：出现渡劫条件时 Drowsy NPC 是否直接 hydrate（同 Dormant，D→H 强制边）？还是 Drowsy 内就地走 plan-tribulation ECS 流程（Drowsy 已是 ECS entity，理论可行）？
- [ ] **Hysteresis N tick 参数**：H↔Dr 用 N=3 vs N=5；Dr↔D 用 N=5 vs N=10。玩家移动速度基线（飞行 ~20 格/s）决定合理防抖窗口，P0 benchmark 后定
- [ ] **Drowsy↔Dormant 转换成本**：v1 H↔D 可能 1-5ms；Dr↔D 理论更轻（无 AI 状态需要 serialize）。P1 benchmark 验证是否需要分帧分批转换（类似 v1 P2 hydrate 批量）
- [ ] **docs/CLAUDE.md §四 是否加红旗**：「Drowsy NPC 跑非白名单 system（socialize / patrol）」—— 建议立即加，防止后续 plan 误接 Drowsy NPC 作为 Hydrated 使用

---

## §5 进度日志

- 2026-05-09：骨架创建。来源：plan-npc-virtualize-v1 §0 决策门 #1（v1 P3 实测触发条件）+ plans-skeleton/reminder.md Drowsy v2 条目整理。待 plan-npc-virtualize-v1 P3 完成 + 实测数据评估后，由人工 `git mv` 启动为 active plan。
