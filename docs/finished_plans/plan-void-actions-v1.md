# Bong · plan-void-actions-v1 · Finished

化虚专属 action 四类：**镇压坍缩渊 / 引爆区域 / 阻挡道伥扩散 / 道统传承**。在 `plan-void-quota-v1` ✅ 立的化虚名额制下，化虚后玩家不再以"突破"为驱动（境界封顶 `Realm::Void`），切换到 4 大持续目标——**寿元延续 / 道统传承 / 世界影响 / 天道运维博弈**。每个 action 高消耗（真元走 `qi_physics::ledger` + 寿元走 `lifespan::deduct`）+ 全服可见（agent `scope: "broadcast"` 已铺）+ 不破红线（禁传送 / 禁复活 / 禁无成本生灵气；不消耗 quota slot 但反噬死亡会回流 slot）。

**世界观锚点**：`worldview.md §三 line 145-160`（化虚体验：天道无力承担更多 + 化虚后角色定位）· `§八 运维博弈`（天道行为准则 + 化虚者参与世界级调控）· `§十六 坍缩渊镇压`（道伥涌出 + 末土 zone 塌缩）· `§十二 道统传承`（死前一次性遗物指定 + 亡者博物馆刻名）

**前置硬依赖**：

- `plan-tribulation-v1` ✅ finished — `Realm::Void` / `TribulationKind::DuXu` / `DuXuOutcomeV1::{Ascended, Killed}` / 全服 narration 框架（`bong:tribulation/*` Redis channel）已可用
- `plan-void-quota-v1` ✅ finished — `check_void_quota` / `compute_void_quota_limit` / `release_ascension_quota_slot` hook 可直接调；本 plan 不消耗 quota slot 但反噬死亡走同一回流链
- `plan-tsy-lifecycle-v1` ✅ finished — `TsyLifecycle::{Active, Declining, Collapsing, Dead}` 阶段 + `TsyCollapseStarted` / `TsyCollapseCompleted` 事件 + `CH_TRIBULATION_COLLAPSE = "bong:tribulation/collapse"` channel 可订阅
- `plan-tsy-hostile-v1` ✅ finished — `TsyHostileMarker` / `DaoxiangInstinctCooldown` / `DaoxiangInstinctAction` 道伥 NPC 框架（`SuppressTsy` action 推回 Declining 阶段或抑制 Daoxiang spawn）
- `plan-death-lifecycle-v1` ✅ finished — `LifeRecord` @ `cultivation/life_record.rs` + `BiographyEntry` @ `death_lifecycle/intrusion_log.rs` + `life_record_snapshot` 广播框架（`LegacyAssign` action 扩 LifeRecord 字段写入）
- `plan-qi-physics-v1` ✅ finished — `WorldQiBudget` 守恒账本 + `qi_physics::ledger::QiTransfer` 真元流动 API（action 烧真元走 ledger，**禁止 `cultivation.qi_current -= cost` 直接扣**）

**交叉引用**：`plan-tribulation-v1` ✅ · `plan-tsy-lifecycle-v1` ✅ · `plan-void-quota-v1` ✅ finished · `plan-niche-defense-v1` ⬜（继承人选择 UI 候选并入，P3 决策门 #4）· `plan-death-lifecycle-v1` ✅（亡者博物馆 LifeRecord）· `plan-gameplay-journey-v1` §N (line 895-904 化虚四大目标 + line 902 化虚专属 action 缺口) · `plan-lifespan-v1` ✅（寿元 deduct API）

---

## 代码库考察（2026-05-08）

### A. 化虚 Realm + cultivation::void/ 模块

- **`Realm::Void` 实装** @ `server/src/cultivation/components.rs:19`（升级序列 `Realm::Spirit → Realm::Void`，前驱 L30）
- **`server/src/cultivation/void/` 目录不存在** —— 本 plan P1 新建子模块（含 `mod.rs` / `actions.rs` / `legacy.rs` / `ledger_hooks.rs`）
- **`VoidAction` / `VoidActionRequest` 不存在** —— 4 类 action enum 由本 plan P1 新建

### B. AscensionQuota / 化虚 quota 链路（plan-void-quota-v1 ✅ finished）

- **`compute_void_quota_limit(total_world_qi: f64, quota_k: f64) -> u32`** @ `server/src/cultivation/tribulation.rs:117`（commit 30c05f028）
- **`check_void_quota(occupied, &budget, &config) -> VoidQuotaCheckV1`** @ `server/src/cultivation/tribulation.rs:129`
- **`release_ascension_quota_slot`** @ `server/src/death_lifecycle/`（降境 / 死透 / `RealmRegressed { from: Void }` 路径已 hook）
- **`AscensionQuotaStore`** @ `server/src/npc/tribulation.rs:26` / **`AscensionQuotaV1`** schema @ `server/src/schema/server_data.rs`
- **本 plan 不消耗 quota slot**，但 action 反噬致死走 release hook 回流（不需扩 release 链路，只需 emit 既有 death cause）

### C. 道伥（tsy hostile）接入（plan-tsy-hostile-v1 ✅）

- **`TsyHostileMarker`** @ `server/src/npc/tsy_hostile.rs:58`
- **`DaoxiangInstinctCooldown`** @ L89 / **`DaoxiangInstinctAction`** @ L165
- **`TsyHostileArchetypeV1`** schema @ `server/src/schema/tsy_hostile.rs:9`
- **缺**：`SuppressTsy` 抑制语义（推回 Daoxiang spawn cooldown / 标记 zone 内 TsyHostile 进入 dormant）—— 本 plan P1 在 `actions.rs` 内实装 `cast_suppress_tsy` 并写入 `npc::tsy_hostile::cooldown_extend` 之类的 hook

### D. 坍缩渊 / Zone 接入（plan-tsy-lifecycle-v1 ✅）

- **`server/src/world/tsy_lifecycle.rs`**（1067 行）—— `TsyLifecycle::{Active, Declining, Collapsing, Dead}` 阶段 enum 完整
- **`TsyCollapseStarted` / `TsyCollapseCompleted` events** @ `world/tsy_lifecycle_integration_test.rs`
- **`CH_TRIBULATION_COLLAPSE = "bong:tribulation/collapse"`** Redis channel @ `schema/channels.rs:31`
- **`COLLAPSED_ZONE_EVENT_NAME = "realm_collapse"`** @ `world/zone.rs:22`
- **`DaoxiangOrigin`** 追踪 @ `world/tsy_lifecycle.rs:568`
- **缺**：`SuppressTsy` 推回 Collapsing → Declining 的逆推 API + `ExplodeZone` 强升 + 衰退到 0 的 6 month 持续 + `Barrier` 地理边界阻断
- **可复用**：本 plan 实装时调 `tsy_lifecycle::transition_to(Declining)` 之类的反向 transition（如已有则复用，无则 P1 加）

### E. 道统 / 亡者博物馆 / 死信箱（plan-death-lifecycle-v1 ✅）

- **`LifeRecord`** @ `server/src/cultivation/life_record.rs`（NPC + 玩家通用生平记录）
- **`BiographyEntry`** @ `server/src/death_lifecycle/intrusion_log.rs:1`
- **`life_record_snapshot`** 广播框架 @ `server/src/npc/lifecycle.rs:18`
- **缺**：`LegacyEntry` / `legacy_assign` / `legacy_letterbox`（死信箱）—— 本 plan P1 在 `void/legacy.rs` 新建，扩 `LifeRecord` 加 `legacy_inheritor: Option<PlayerId>` + `legacy_items: Vec<ItemId>` 字段
- **死信箱**（worldview line 848 `peoples-0007` 提到）—— 本 plan **新建** `legacy_letterbox` 持久化（SQLite 表 / 单行 per 化虚者，死亡时 finalize）

### F. 渡虚劫现状（plan-tribulation-v1 ✅）

- **`server/src/cultivation/tribulation.rs`**（4981 行）完整渡虚劫三段论
- **`TribulationKind::DuXu`** @ L150 / **`DuXuOutcomeV1::{HalfStep, Ascended, Failed, Killed}`** @ L1128+
- **本 plan 不改 tribulation 流程**，只在化虚后才生效（玩家 `Realm::Void` 状态读 cultivation component）

### G. 全服 narration 广播

- **agent `scope: "broadcast"`** 已多文件接入（`tribulation-runtime.ts` L85 等）
- **频道前缀 `bong:tribulation/*`** 已存（`bong:tribulation/omen` / `bong:tribulation/collapse`）
- **本 plan 复用同模式**：新增 `bong:void_action/{suppress_tsy, explode_zone, barrier, legacy_assign}` 四类频道（与现有命名规范对齐）
- **agent runtime 新建** `void-actions-runtime.ts` 处理 4 类 broadcast narration（"某化虚者镇压了 X 坍缩渊"等）

### H. 化虚专属 UI（client）

- **`realm-vision-void.sample.json`** @ `server/src/schema/realm_vision.rs:72`（perception-v1.1 完成，化虚视效已可用）
- **`AscensionQuotaHandler` / `AscensionQuotaStore`** @ `client/src/main/java/com/bong/client/combat/store/`（quota-v1 已铺）
- **缺**：`client/src/main/java/com/bong/client/cultivation/void/` 目录全空 —— 本 plan P3 新建 `VoidActionScreen.java` + `VoidActionStore.java` + `VoidActionHandler.java`
- **继承人选择 UI** —— 候选并入 `plan-niche-defense-v1` ⬜（@ `server/src/social/niche_defense.rs` 占位 + `social/mod.rs:163` register hook 已存），P3 决策门 #4

### I. library-web legacy 页面

- **`library-web/src/pages/deceased/` 已存**（含 `index.astro` + `view.astro`）—— 通用亡者列表
- **缺**：`library-web/src/pages/legacy/[name].astro` 化虚者一生专属页面 —— P3 决策门 #5：复用 deceased 目录加 query filter `?role=void`，还是新建 legacy/ 子目录

### J. plan-niche-defense-v1 (⬜) 现状

- **模块占位** @ `server/src/social/niche_defense.rs`
- **register hook** @ `server/src/social/mod.rs:163` 调 `niche_defense::register(app)`
- **`emit_niche_defense_server_data`** @ L551
- **未实装详细业务逻辑**——仍为骨架，P3 决策门 #4 决定"继承人选择 UI"是独立 plan 还是并入

### K. plan-gameplay-journey-v1 §N 引用上下文

- **L874**：`check_void_quota` 落点已写明
- **L886**：天道注意力 (`spiritual_sense::void`) ✅ 已实装
- **L895-898**：化虚四大目标（道统传承 / 世界镇压 / 天道博弈 / 传记书写）—— 本 plan 落地点
- **L902**：标注"缺 plan-void-actions-v1（化虚专属 action）"
- **L904**：标注"缺道统传承 UI（死信箱）"
- **本 plan 是 §N L902/L904 缺口的直接落地**

---

### 命名差异清单

| plan 文档写的 | 代码实名 / 状态 | 处理 |
|---|---|---|
| `VoidActionRequest` enum | 不存在 | P1 新建 @ `server/src/schema/void_actions.rs` |
| `LegacyEntry` | 不存在 | P1 新建 @ `void/legacy.rs`（扩 `LifeRecord` 字段） |
| `SuppressTsy` action | `TsyHostileMarker` 已有，无 suppress 语义 | P1 在 `actions.rs` 内调 `tsy_lifecycle::transition_to(Declining)` + `npc::tsy_hostile::cooldown_extend` |
| `ExplodeZone` action | `Zone` struct + `Zone.spirit_qi ∈ [-1, 1]` 浓度 | P1 走 `qi_physics::ledger` 强升 + 衰退队列 |
| `Barrier` action | 无现有阻断 component | P1 新建 `BarrierField` component（地理边界 + 持续 ticks） |
| `legacy_assign` action | `LifeRecord` 已有但无 inheritor 字段 | P1 扩 `LifeRecord.legacy_inheritor / legacy_items / legacy_letterbox` |
| `library-web/src/pages/legacy/[name].astro` | 现有 `deceased/` 目录 | P3 决策门 #5 选 deceased filter or 新建 legacy/ |

### 缺失项（P0/P1 必新建）

1. **`server/src/cultivation/void/`** 子模块（mod.rs / actions.rs / legacy.rs / ledger_hooks.rs）
2. **`server/src/schema/void_actions.rs`** + **`agent/packages/schema/src/void-actions.ts`** IPC schema 双端
3. **`agent/packages/tiandao/src/void-actions-runtime.ts`** 4 类 broadcast narration
4. **`client/src/main/java/.../cultivation/void/` 目录**（VoidActionScreen / VoidActionStore / VoidActionHandler）
5. **`library-web/src/pages/legacy/` 或 `deceased/` 扩** 化虚者一生页面（决策门 P3 #5）
6. **死信箱持久化**（`legacy_letterbox` SQLite 表 / 单行 per 化虚者）
7. **`BarrierField` component**（地理边界阻断）

### 可直接复用项

- `cultivation::Realm::Void` @ `cultivation/components.rs:19`
- `tribulation::check_void_quota` / `compute_void_quota_limit` @ `tribulation.rs:117/129`（quota-v1 已 ship）
- `release_ascension_quota_slot` hook @ `death_lifecycle/`（action 反噬死亡走同链）
- `world::tsy_lifecycle::TsyLifecycle::{Active, Declining, Collapsing, Dead}` @ `world/tsy_lifecycle.rs`
- `npc::tsy_hostile::TsyHostileMarker / DaoxiangInstinctCooldown` @ `npc/tsy_hostile.rs:58/89`
- `cultivation::life_record::LifeRecord` @ `cultivation/life_record.rs`
- `qi_physics::ledger::QiTransfer` + `WorldQiBudget`（action 烧真元走 ledger，禁直接扣 qi_current）
- `lifespan::deduct(years, reason)` @ plan-lifespan-v1 finished（reason: `VoidActionCost`）
- agent `scope: "broadcast"` + `bong:tribulation/*` Redis channel 命名规范

---

## §0 设计轴心

- [x] **化虚不是退休模式**：4 大持续目标——寿元延续（吃续命丹 / 高境功法）+ 道统传承（死前一次性遗物指定）+ 世界影响（4 类 action）+ 天道运维博弈（agent 评论化虚者作为 + 注意力累积）
- [x] **action 高消耗，走守恒律**：每个 action 烧大量真元（化虚池约 500 单位的 30%-50%）+ 寿元（30-100 年）。**真元走 `qi_physics::ledger::QiTransfer{from: caster, to: zone}`**，**寿元走 `lifespan::deduct(years, reason: VoidActionCost)`**，禁止任何 plan 内 `cultivation.qi_current -= X` 直接扣（守恒律红旗）
- [x] **全服可见，不可隐匿**：每个 action 触发 agent `scope: "broadcast"` narration（复用现有 `bong:tribulation/*` 命名规范，新增 `bong:void_action/{suppress_tsy, explode_zone, barrier, legacy_assign}` 四 channel）
- [x] **不破红线**：禁传送 / 禁复活 / 禁无成本生灵气；`ExplodeZone` 强升 zone qi 必须从 `WorldQiBudget` 借出（守恒），衰退 6 month 后归还原路径；`SuppressTsy` 不消除 TsyLifecycle，只逆推 Collapsing → Declining 阶段
- [x] **不消耗 quota slot，反噬死亡走 release**：化虚 action 不占用 `AscensionQuota` slot（化虚者已经在 slot 内）；但 action 反噬致死（如寿元归零）走 `release_ascension_quota_slot` hook 回流，让位给下一位渡虚劫者
- [x] **节奏对齐 quota-v1**：`compute_void_quota_limit` = `floor(WorldQiBudget.current_total / K)` 决定多少人**能**化虚；本 plan 决定化虚者**做什么**。两 plan 协同形成"末法天道无力承担多化虚 + 化虚者用力世界级 action"的总叙事

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-09 | 决策门：§4 八问题收口（K 值校准 / action 冷却 / 寿元上限 / Barrier 地理边界粒度 / 继承人 UI 归属 / library-web 页面归属 / Explode 衰退路径 / 化虚 PVP 推迟 v2） + 数值表锁定（4 类 action 真元/寿元/冷却 + 4 类 broadcast narration template） + 与 plan-void-quota-v1 同步 quota 不冲突 | 数值矩阵进 §2 / 决策记 §4 / 与 quota-v1 维护者拍板"action 不消耗 slot 但反噬走 release"边界 |
| **P1** ✅ 2026-05-09 | server `cultivation::void::*` 主实装：4 类 action handler（`cast_suppress_tsy` / `cast_explode_zone` / `cast_barrier` / `cast_legacy_assign`） + `BarrierField` component + `LifeRecord` 扩 inheritor/letterbox 字段 + `qi_physics::ledger` 接入 + `lifespan::deduct` 接入 + `release_ascension_quota_slot` 死亡 hook + IPC schema + ≥60 单测 | `cargo test cultivation::void` 全过 / `grep -rcE '#\[test\]' server/src/cultivation/void/` = 68 / 守恒断言（每 action 真元流向 ledger + zone 还原路径完整） |
| **P2** ✅ 2026-05-09 | agent `void-actions-runtime.ts` 4 类 broadcast narration（"某化虚者镇压了 X 坍缩渊" / "X 引爆 Y 区域，灵气尽失六月" / "X 在 Y 设阻断，道伥退散" / "X 临终遗令，传 Y 继承"） + tiandao 评论钩子（运维博弈：累积 action 数量 → 天道注意力增量） + IPC schema 双端 | tiandao runtime 单测 + schema generated artifacts 全过；4 类 fanout channel 独立订阅 |
| **P3** ✅ 2026-05-09 | client 化虚专属 UI：`VoidActionScreen.java`（4 action 列表 + 代价显示 + 冷却计时 + 真元 / 寿元预览） + `VoidActionStore.java` + `VoidActionHandler.java` + 道统继承人选择 UI（决策门 #4：独立模块 or 并入 niche-defense） + library-web 化虚者一生页面（决策门 #5） | Java 17 `./gradlew test build` 通过；library-web 复用 `deceased/?role=void` |
| **P4** ✅ 2026-05-09 | 化虚 action 历史记录写入 `LifeRecord` + library-web 公开页面 + telemetry 校准（4 类 action 触发频次 / 反噬死亡比例 / 继承人接受率） + 与 plan-niche-defense-v1 联调（如选择并入路径） | LifeRecord 写入 `void_actions` / biography；亡者页面渲染化虚行事与道统字段；telemetry 留后续运营校准 |

**P0 决策门**：完成前 §4 八问题必须有答案，否则 4 action 实装方向分裂。

---

## §2 关键 API / 公式

### action 触发前置检查（每 action 通用）

```rust
// 调用方：cast_<action>(caster: Entity, target: ActionTarget, world: &World)
fn precheck_void_action(caster: Entity, world: &World) -> Result<(), VoidActionError> {
    // 1. 境界 gate
    let realm = world.get::<Cultivation>(caster)?.realm;
    if realm != Realm::Void { return Err(VoidActionError::RealmTooLow); }

    // 2. 真元 gate（参考 §2 各 action 数值表）
    let qi_required = action.qi_cost();
    if cultivation.qi_current < qi_required {
        return Err(VoidActionError::QiInsufficient);
    }

    // 3. 寿元 gate（lifespan.current_years > years_cost）
    let years_cost = action.lifespan_cost();
    if lifespan.current_years <= years_cost {
        return Err(VoidActionError::LifespanInsufficient);
    }

    // 4. 冷却 gate
    if action.is_on_cooldown(caster) { return Err(VoidActionError::OnCooldown); }

    Ok(())
}
```

### 4 类 action 数值表

| Action | qi_cost | lifespan_cost | 冷却 | 主效果 | 反噬死亡触发 |
|---|---|---|---|---|---|
| `suppress_tsy` | 200 | 50 年 | 30 day | 目标 zone `TsyLifecycle::Collapsing` → `Declining`（推迟 1-3 month 塌缩）+ `cooldown_extend` Daoxiang spawn | lifespan 归零 → release quota |
| `explode_zone` | 300 | 100 年 | 90 day | 强升 zone qi → 6 month 后衰退到 `spirit_qi = 0`（守恒：从 WorldQiBudget 借出 + 6 month 后归还原 ledger） | lifespan 归零 → release quota |
| `barrier` | 150 | 30 年 | 7 day | 创建 `BarrierField` component（地理边界，矩形 / 多边形粒度由 P0 #4 决策门）持续 1 month → 道伥过线 dispel 50% qi | lifespan 归零 → release quota |
| `legacy_assign` | — | — | — (一次性) | 扩 `LifeRecord.legacy_inheritor / legacy_items` + 写 `legacy_letterbox` SQLite 表 + 死亡时 finalize 触发继承人 NPC dialog | — (临终行为不反噬) |

### 守恒律保证

```
ExplodeZone 守恒断言：
  borrow:  WorldQiBudget -= (强升量 = 300 + zone 自身 qi)
  refund:  WorldQiBudget += (归还量) at t = 6 month
  net:     6 month 周期内总 qi 守恒（仅 zone 内浓度震荡）

SuppressTsy 守恒断言：
  借: caster.qi_current -= 200 (走 ledger to zone)
  还: zone.spirit_qi += 200 流量（推回 Declining 阶段不消耗道伥真元）

Barrier 守恒断言：
  borrow:  caster.qi_current -= 150 (走 ledger to barrier_field)
  return:  1 month 后 BarrierField despawn → qi 归还所在 zone
```

### lifespan 扣减

```rust
// 接 plan-lifespan-v1 ✅ API
lifespan::deduct(
    caster,
    years_cost,
    DeductReason::VoidActionCost { action_kind: VoidActionKind::SuppressTsy },
);
```

---

## §3 数据契约

```
server/src/cultivation/void/
├── mod.rs                — Plugin 注册 + re-export + register_void_actions
├── actions.rs            — 4 cast_* fn + VoidActionKind enum +
│                          VoidActionRequest schema 入口 + precheck_void_action
├── legacy.rs             — LegacyAssign 扩 LifeRecord 字段 +
│                          legacy_letterbox SQLite 表 + finalize_at_death hook
├── ledger_hooks.rs       — qi_physics::ledger::QiTransfer 接入 +
│                          ExplodeZone 6 month 衰退队列 (BorrowReturnSchedule)
└── components.rs         — BarrierField component (geometry, ttl_ticks, owner)

server/src/schema/void_actions.rs
  — VoidActionRequest { kind: SuppressTsy{zone_id} | ExplodeZone{zone_id} |
                              Barrier{geometry} | LegacyAssign{inheritor, items} } +
    VoidActionResponse { accepted, reason } +
    VoidActionBroadcastV1 (4 类 narration payload)

agent/packages/schema/src/void-actions.ts  — TypeBox 双端对齐
agent/packages/tiandao/src/void-actions-runtime.ts
  — 4 类 broadcast narration (古意检测) +
    天道运维博弈钩子 (累积 action 数量 → 注意力增量)

client/src/main/java/com/bong/client/cultivation/void/
├── VoidActionScreen.java     — 4 action 列表 + 代价 + 冷却 UI
├── VoidActionStore.java      — Snapshot 状态 (cooldown / lifespan preview)
├── VoidActionHandler.java    — VoidActionRequest 发包 + Response 接收
└── LegacyAssignPanel.java    — 继承人选择 UI (决策门 P3 #4: 独立 or
                                并入 plan-niche-defense-v1)

library-web/src/pages/legacy/[name].astro (or deceased/?role=void)
  — 化虚者一生页面（4 类 action 时间线 + 道统遗物 + 继承人）
  (决策门 P3 #5)
```

**SkillRegistry 不涉及**——化虚 action 不是 SkillRegistry 注册的招式（不走 cooldown / cast 流程），是化虚专属新通道（VoidActionRequest payload）。

**LifeRecord 扩字段**（P1 任务）：

```rust
pub struct LifeRecord {
    // ... 现有字段
    pub void_actions: Vec<VoidActionLogEntry>,         // 化虚后所有 action 历史
    pub legacy_inheritor: Option<PlayerId>,            // 道统继承人（legacy_assign 后写入）
    pub legacy_items: Vec<ItemId>,                     // 临终遗物
    pub legacy_letterbox: Option<LegacyLetterbox>,     // 死信箱内容（继承人接收）
}
```

---

## §3.5 P1 测试矩阵（饱和化测试）

下限 **60 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `cast_suppress_tsy` | happy path（Collapsing → Declining）+ 目标 zone 状态校验（Active/Dead reject）+ qi 不足 reject + 寿元不足 reject + 冷却中 reject + Daoxiang cooldown_extend hook + 守恒断言 | 12 |
| `cast_explode_zone` | happy path（强升 + 6 month 衰退队列）+ WorldQiBudget 借出 / 归还守恒 + zone 状态进 Active/Dead reject + 反噬寿元归零触发 release_ascension_quota_slot | 12 |
| `cast_barrier` | BarrierField spawn + 几何边界检查（rect / polygon 决策门 #4）+ 1 month TTL despawn + qi 归还 zone + 道伥过线 dispel 50% qi | 10 |
| `cast_legacy_assign` | LifeRecord 扩字段写入 + legacy_letterbox SQLite 持久化 + 继承人接受 / 拒绝 / 24h 流落（决策门 #6）+ 死亡时 finalize 触发 dialog | 10 |
| `precheck_void_action` | 4 种 gate（realm / qi / lifespan / cooldown）正反测试 + 错误码 enum 完整 | 8 |
| `release_ascension_quota_slot` 联调 | 反噬致死 → quota slot 释放 + AscensionQuotaOpened 广播 | 4 |
| broadcast 守恒 | 4 类 action 触发 → agent broadcast narration emit 校验 + Redis channel 名正确 | 4 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/cultivation/void/` ≥ 60。守恒断言：每 action 触发后 `WorldQiBudget.current_total` 在 6 month / 1 month 周期后回归原值（误差 < 0.01）。

---

## §4 开放问题 / 决策门（P0 启动前必须收口）

- [x] **action 是否进 quota 上限检查？** **不进**。化虚 action 是化虚后行为，AscensionQuota 是化虚前门槛。两 plan 边界明确——quota-v1 决定多少人能化虚，本 plan 决定化虚者做什么。
- [x] **action 是否被反制（化虚 vs 化虚 PVP）？** **推迟 v2**。v1 不实装 action 反制，单化虚者 vs 世界对抗为主旋律。若化虚 vs 化虚 PVP 进 v2，需大量数值再校准。
- [x] **action 频繁是否触发更多天劫？** **不触发**。action 不进 `void_quota_exceeded` 路径。但累积 action 数量 → 天道注意力增量（agent 钩子，仅叙事影响），不影响 quota。
- [x] **K 值如何调整？** quota-v1 已锁 `BONG_VOID_QUOTA_K = 50.0`。本 plan 不动 K，只读 `compute_void_quota_limit` 结果用于反噬死亡 release 触发。
- [x] **#1 Barrier 地理边界粒度**：选 **circle**（中心+半径）。玩家心智简单，服务端 `BarrierGeometry::contains` 只做水平半径判定，后续若需要 polygon 另开 v2。
- [x] **#2 ExplodeZone 6 month 衰退路径**：选 **B 阶梯语义的 v1 简化实现**：触发时 zone qi 强升，`VoidQiReturnSchedule` 到 6 month 回流并将 zone qi 归零；中间分月资源层差异留 telemetry 后续校准。
- [x] **#3 反噬死亡寿元归零路径**：选 **A action 触发即扣寿元（确定性）**。扣减后若寿元归零，发送 `CultivationDeathCause::VoidActionBacklash`。
- [x] **#4 继承人选择 UI 归属**：选 **A 独立模块**。Java 包名落在 `client/.../cultivation/voidaction/`（`void` 是 Java 关键字，不能作为 package segment），含 `LegacyAssignPanel.java`。
- [x] **#5 library-web 化虚者一生页面**：选 **A 复用 deceased**。`/deceased/?role=void` 过滤化虚者，`deceased/view.astro` 增加化虚行事 / 道统字段渲染。
- [x] **#6 道统继承人是否可拒绝**：选 **B 24h 内可拒绝**。`LegacyLetterbox` 记录 `reject_until_tick` / `Pending` / `Rejected` / `Drifted` / `Finalized` 状态。
- [x] **#7 引爆区域 6 month 灵气 0 是否会逼走该 zone 全部玩家？** v1 不强制迁出玩家，保持世界影响但不做传送；具体玩家行为分布留 telemetry / v2 数值校准。
- [x] **#8 化虚 action 历史是否进 LifeRecord 公开**：选 **A 全部公开**。`LifeRecord.void_actions` 与 `BiographyEntry::VoidAction` 均可进入亡者页面。

---

## §5 进度日志

- **2026-05-01** 骨架创建。`plan-gameplay-journey-v1` §N L902/L904 派生（化虚专属 action + 道统传承 UI 缺口）。
- **2026-05-08** 实地核验后升 active。
  - 确认前置依赖：`plan-tribulation-v1` ✅ / `plan-void-quota-v1` ✅ finished（`check_void_quota` + 绝壁劫已落地）/ `plan-tsy-lifecycle-v1` ✅ / `plan-tsy-hostile-v1` ✅ / `plan-death-lifecycle-v1` ✅ / `plan-qi-physics-v1` ✅
  - 确认可直接复用：`Realm::Void` @ components.rs:19 / `check_void_quota` @ tribulation.rs:129 / `compute_void_quota_limit` @ L117 / `release_ascension_quota_slot` death hook / `TsyLifecycle::{Active, Declining, Collapsing, Dead}` / `TsyHostileMarker` / `LifeRecord` / `WorldQiBudget` / `qi_physics::ledger::QiTransfer` / `lifespan::deduct` / agent `scope: "broadcast"`
  - 确认缺失项（P0/P1 必新建）：`server/src/cultivation/void/` 子模块全空 / `VoidAction` / `LegacyEntry` / `BarrierField` / `legacy_letterbox` / 4 类 broadcast narration / `VoidActionScreen` UI / `library-web/legacy` 页面
  - 命名差异锁定（命名差异表 7 项）
  - §0 设计轴心补"不消耗 quota slot 但反噬死亡走 release"+"action 烧真元走 ledger 不直接扣"两条物理约束
  - §1 阶段总览 P0-P4 具体化（含验收命令）
  - §2 4 类 action 数值表 + 守恒律断言
  - §4 开放问题当时分为 4 项已决 + 8 项待决（含 P0 决策门 #1-#3/#6 + P3 #4-#5/#8 + P5 #7）
  - 单测下限定 60，守恒断言到 ledger 周期回归
- **2026-05-09** 完整落地并归档。
  - P0 决策门收口：circle barrier、6 month 到期归零回流、确定性寿元反噬、独立 client 继承 UI、复用 deceased `?role=void` 页面、继承人 24h 可拒绝、化虚历史公开入 LifeRecord。
  - P1 server 新增 `cultivation::void::*`、`VoidActionRequestV1` / `VoidActionBroadcastV1`、`legacy_letterbox` SQLite 表、`QiTransferReason::VoidAction`、`VoidActionBacklash` death cause。
  - P2 agent 新增 `void-actions-runtime.ts`，订阅 4 个 `bong:void_action/*` fanout channel 并发布 broadcast narration。
  - P3/P4 client + library-web 落地：Java package 使用 `cultivation/voidaction` 避免关键字，亡者博物馆渲染化虚行事 / 道统字段。

---

## Finish Evidence

- **落地清单**：
  - server：`server/src/cultivation/void/{mod,actions,components,ledger_hooks,legacy}.rs`、`server/src/schema/void_actions.rs`、`server/src/schema/{channels,client_request}.rs`、`server/src/network/{client_request_handler,redis_bridge}.rs`、`server/src/cultivation/life_record.rs`、`server/src/persistence/mod.rs`。
  - agent/schema：`agent/packages/schema/src/void-actions.ts`、`agent/packages/schema/generated/void-action-*.json`、`agent/packages/schema/generated/client-request-void-action-v1.json`、`agent/packages/tiandao/src/void-actions-runtime.ts`、`agent/packages/tiandao/src/main.ts`。
  - client/library：`client/src/main/java/com/bong/client/cultivation/voidaction/{VoidActionScreen,VoidActionStore,VoidActionHandler,LegacyAssignPanel,VoidActionScreenBootstrap,VoidActionKind}.java`（Java 包名避开关键字 `void`）、`client/src/main/java/com/bong/client/network/{ClientRequestProtocol,ClientRequestSender}.java`、`library-web/src/pages/deceased/{index,view}.astro`。
- **关键 commit**：
  - `381691326`（2026-05-09）`plan-void-actions-v1: 落地 server 化虚 action`：4 类 action handler、ledger 回流、LegacyLetterbox、LifeRecord 扩字段、SQLite `legacy_letterbox` 迁移、Redis fanout。
  - `bb15e3d10`（2026-05-09）`plan-void-actions-v1: 接入 agent 化虚叙事`：TypeBox schema / generated artifacts、4 channel narration runtime、tiandao bootstrap 与单测。
  - `530d606e2`（2026-05-09）`plan-void-actions-v1: 补化虚 client 与亡者页面`：client 协议 / store / screen / 继承面板、亡者博物馆 `?role=void` 与化虚行事渲染。
  - `eb552861e`（2026-05-09）`fix(void-actions): 修正 barrier 到期灵气回流`：review 修复 Barrier 到期不再给 `WorldQiBudget` 铸造 150，改为 `barrier:<zone>` ledger 账户转回真实 zone 账户并补回归。
- **测试结果**：
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → 3094 passed。
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test cultivation::void` → 68 passed；`grep -rcE '#\[test\]' server/src/cultivation/void/` → 68。
  - `cd agent && npm run build && (cd packages/tiandao && npm test) && (cd packages/schema && npm test)` → tiandao 284 passed；schema 327 passed。
  - `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL。
  - `cd library-web && LOCAL_LIBRARY_PATH=/home/kiz/Code/Bong/.worktree/plan-void-actions-v1/docs/library npm run build` → 41 pages built。
  - `git diff --check` → clean。
- **跨仓库核验**：
  - server：`VoidActionIntent` 由 `ClientRequestV1::VoidAction` 入队，`resolve_void_action_intents` gate `Realm::Void` / qi / lifespan / cooldown；`QiTransferReason::VoidAction` 保留 ledger 轨迹；`VoidQiReturnSchedule` 到期回流 `ExplodeZone` / `Barrier`；`CultivationDeathCause::VoidActionBacklash` 走 death arbiter。
  - agent：4 个 Redis fanout channel `bong:void_action/{suppress_tsy,explode_zone,barrier,legacy_assign}` 与 `VoidActionBroadcastV1` 对齐；runtime 输出 `scope: "broadcast"` narration。
  - client/library：client 发 `void_action` C2S payload；亡者博物馆按 `life_record.void_actions` / `legacy_inheritor` 过滤与渲染化虚者。
- **遗留 / 后续**：
  - v1 采用 circle barrier；polygon / rect 边界、化虚 vs 化虚 PVP、ExplodeZone 分月资源曲线、以及 zone 灵气归零后的玩家行为 telemetry 留 v2/运营校准。
  - `LegacyLetterbox` 已记录 24h 拒绝窗口和状态；继承人 NPC dialog / 实物领取结算可在后续 niche-defense 或 legacy follow-up 中接 UI 流程。
  - `debit_caster_qi_to_account` 仍沿用当前 `WorldQiAccount` / `Cultivation.qi_current` 同步桥接模式；后续若接入 craft 注释中的全局 sync system，应统一改成 `LedgerOutOfSync` fail-fast，避免各 action 自行 seed ledger。
