# Bong · plan-npc-virtualize-v2 · 骨架

NPC 隐式更新框架**三态扩展**——在 v1 二态（Hydrated ↔ Dormant）基础上引入 **Drowsy 中间态**（64-256 格，ECS entity 仅核心 system 1Hz tick + 远视野可见）。派生自 plan-npc-virtualize-v1 §8 决策门 #1 的「v1 P3 实测后触发派生」条件。触发时满足任意一项即可启动本 plan：
- hydrate / dehydrate 单次开销 > 5 ms/NPC（频繁穿越 64-256 格边界时撕裂感强）
- 快速移动（飞行 / 灵兽坐骑）频繁穿越阈值，反复 spawn/despawn 抖动明显
- 玩家反馈「远视野空旷无 NPC」违和（256 格外完全看不到 NPC 影响沉浸感）

**世界观锚点**：`worldview.md §三:124-187`（NPC 与玩家平等，规则不豁免）· `§十一:947-970`（散修江湖人来人往，远处人影隐约可见）· `§二`（真元守恒，Drowsy NPC 修炼仍走 qi_physics::ledger）

**交叉引用**：`plan-npc-virtualize-v1.md` ✅（二态 MVP，本 plan 直接基础）· `plan-npc-perf-v1.md` ✅（hydrated 100 NPC 性能基础）· `plan-npc-ai-v1.md` ✅（big-brain Bundle / Scorer / Action 框架）· `plan-qi-physics-v1.md` ✅（ledger::QiTransfer 守恒账本）· `plan-npc-fixups-v3.md` ✅（ECS lifecycle 卫生规则，Drowsy 状态机亦全量继承）

---

## 接入面 Checklist

- **进料**：v1 的 `NpcVirtualizeState { Hydrated, Dormant }` enum → 扩展加 `Drowsy` 变体；`hysteresis_gate` 阈值对（现 64/256）→ 变为 64/128/256 三阈值；`dormant_global_tick` system（1/min）→ 并行 3 套调度
- **出料**：Drowsy ECS entity（精简 Bundle，仅保留 Position / Cultivation / Lifespan / NpcLodTier::Drowsy / FactionMembership）+ `drowsy_tick_system`（FixedUpdate 1Hz）+ 远视野 LOD gate（玩家可见 Drowsy entity）
- **共享类型**：扩展 `NpcVirtualizeState` 加 `Drowsy` 变体 + 新增 `NpcLodTier::Drowsy` + `DrowsyBundle`（精简版 HydratedBundle）
- **跨仓库契约**：agent 端 `NpcDigest` 已是压缩表示（v1 P3 schema v2 扩展），Drowsy NPC 也走 NpcDigest 通道；无新 IPC schema
- **worldview 锚点**：§三:124-187 平等规则 + §十一:947 远处人影
- **qi_physics 锚点**：Drowsy 修炼吸收走 `qi_physics::regen_from_zone`（与 dormant 同一 API）；Drowsy↔Hydrated 转换时无需特殊处理（ECS entity qi_current 字段直接继承）；Drowsy 老死走 `qi_physics::qi_release_to_zone`

---

## §0 三态设计轴心

```
距离  |  0-64格   |  64-256格            |  256格+
状态  | Hydrated  | ←—— Drowsy ——→       | Dormant
调度  | Update    | FixedUpdate 1Hz      | GlobalTick 1/min
Bundle| 完整      | 精简（核心 component）| SoA 数据
可见  | 完全      | 远视野 LOD（降细节）  | 不可见
```

**Hysteresis 防抖**（三阈值，防往返抖动）：

| 方向              | 阈值      | 逻辑                                     |
|------------------|-----------|------------------------------------------|
| Hydrated → Drowsy | 离开 >80格 | 倒计时 3s，仍 >80格才转                  |
| Drowsy → Hydrated | 进入 <60格 | 立即转（向玩家方向优先）                  |
| Drowsy → Dormant  | 离开 >270格 | 倒计时 5s                               |
| Dormant → Drowsy  | 进入 <240格 | 立即转（精简 Bundle spawn）             |

**转换矩阵**：6 条边（H↔Dr / H↔D / Dr↔D），v1 仅 2 条。新增成本：LOD gate 配置 + 3 套并行 schedule + 6 边转换代码

**视觉**：Drowsy NPC 远视野走 client `NpcLodRenderer`（降 skin 精度 / 隐藏 nameplate / 位置模糊 ±2 格）——让「远处人影依稀可见」而非「空地」（worldview §十一:947）

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 决策门：三阈值具体值 + DrowsyBundle 字段清单 + 3 套调度参数 + LOD gate 实现方案 + §5 五个开放问题收口 | 设计收口文档落 plan §2-§4 |
| **P1** ⬜ | `NpcVirtualizeState::Drowsy` 变体 + `DrowsyBundle` + `drowsy_tick_system`（FixedUpdate 1Hz）+ `hysteresis_gate` 三阈值 | `cargo test` 绿 + 100 NPC 跨三态 roundtrip e2e 无 panic |
| **P2** ⬜ | 远视野 LOD：client `NpcLodRenderer` Drowsy 降细节 + Drowsy→Dormant 视觉过渡（淡出而非瞬消） | client runClient 可见远处人影淡出 |
| **P3** ⬜ | 性能验证：100 hydrated + 500 drowsy + 5000 dormant 稳定 18+ TPS；hydrate/dehydrate 单次开销 < 5 ms/NPC | CI e2e 绿 + perf benchmark 输出 |

---

## §2 数据契约

- [ ] `server/src/npc/virtualize.rs` — `NpcVirtualizeState` 加 `Drowsy` 变体
- [ ] `server/src/npc/virtualize.rs` — `DrowsyBundle` struct（Position / Cultivation / Lifespan / NpcLodTier::Drowsy / FactionMembership）
- [ ] `server/src/npc/virtualize.rs` — `drowsy_tick_system`（FixedUpdate 1Hz，lifespan tick + qi regen + 渡劫条件检查）
- [ ] `server/src/npc/virtualize.rs` — `hysteresis_gate` 三阈值常数（HYDRATED_TO_DROWSY / DROWSY_TO_HYDRATED / DROWSY_TO_DORMANT / DORMANT_TO_DROWSY）
- [ ] `server/src/npc/virtualize.rs` — 6 边转换系统，每边均加 `Without<Despawned>` filter（plan-npc-fixups-v3 §3 强约束）
- [ ] `client/src/npc/lod_renderer.rs` — `NpcLodRenderer` Drowsy 降精度渲染 + 淡出过渡

---

## §3 qi_physics 守恒律强约束（继承 v1 §3 全量）

Drowsy NPC 与 Dormant 同等约束，不因「精简 Bundle」豁免：
- 修炼吸收走 `qi_physics::regen_from_zone` + `QiTransfer` 记账
- 老死走 `qi_physics::qi_release_to_zone` 归还 zone qi
- Drowsy 状态下 qi_current 变化禁止直接 `cultivation.qi_current +=`

---

## §4 ECS Lifecycle 强约束（plan-npc-fixups-v3 §3 全量继承）

- 所有 6 边转换均检查 `Without<Despawned>`
- `drowsy_tick_system` 内任何 `Executing` 状态必须有超时 deadline（默认 30s）
- 三态 deferred commands 不可信用 `Added<C>`，改用 explicit state enum

---

## §5 开放问题

- [ ] **三阈值具体数值**：(64/128/256) vs (48/128/192) vs (64/160/256)，P0 实测定
- [ ] **Drowsy tick 1Hz 是否足够**：lifespan 老化 + qi regen + 渡劫条件检查；1Hz 完整遍历 500 NPC 是否撑得住
- [ ] **远视野 LOD 客户端方案**：降 skin 精度 vs 仅位置可见 vs entity packet 按距离更新频率
- [ ] **Drowsy→Dormant 视觉过渡时长**：淡出 1s / 2s / 即时
- [ ] **plan-npc-virtualize-v3 触发条件**（dormant↔dormant 战争批量推演）：v1 §8 决策门 #6 选 C 时启动，暂列占位

## §6 进度日志

- 2026-05-10：骨架创建。源自 plan-npc-virtualize-v1 §8 决策门 #1 + reminder.md plan-npc-virtualize-v1 段 Drowsy 待办。v1 ✅ finished（二态 MVP）。
