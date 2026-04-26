# Cultivation 双头清理 · plan-cultivation-mvp-cleanup-v1

> Server 里有**两套并行的境界 / 突破 / 真元实现**：`player/gameplay.rs` + `player/progression.rs` 的 MVP 占位（旧 ladder：一套传统境界 key（15 档；已废弃））与 `cultivation/` 的真实六境（Awaken/Induce/Condense/Solidify/Spirit/Void），两套没对接——`/attempt_breakthrough` 走占位、cultivation 模块的 `try_breakthrough` 从没被玩家指令触发。本 plan 清理 MVP 占位，让 cultivation 成为 single source of truth；一并删除 `PlayerState` 里和 cultivation 并行的 4 个字段、Client 端传统仙侠体系的 `humanizeRealm` 映射、IPC schema 里 realm 字段的 MVP 源头。**karma 一字不改**——它不是境界并行，是独立系统（`cultivation/karma.rs` 独立模块 + worldview §十二 运数/劫数依赖）。
> 交叉引用：`worldview.md §三`（六境，末法去上古）· `worldview.md §四.零`（战力分层 —— 经脉打通是突破主路径）· `worldview.md §十二`（运数/劫数依赖 karma，本 plan 不碰）· `plan-cultivation-v1 §6.1`（BreakthroughEvent IPC）

---

## §-1 现有代码基线（2026-04-24 audit 完成）

### MVP 占位实现（两文件；已废弃）

| 能力 | 位置 | 档数 | 依赖字段 |
|------|------|------|------|
| `BREAKTHROUGH_RULES` 突破规则表 | `gameplay.rs:30-55` | 4 档（旧 ladder） | experience + karma + spirit_qi |
| `struct BreakthroughRule` | `gameplay.rs:148-155` | - | 规则结构 |
| `apply_breakthrough_action` | `gameplay.rs:372-416` | - | 写 `PlayerState.realm` |
| `validate_breakthrough` | `gameplay.rs:418-449` | - | 读 `PlayerState` 4 字段（含 karma 门槛） |
| `realm_display_name` | `gameplay.rs:480-488` | - | 中文输出旧体系境界名 |
| `breakthrough_rule` | `gameplay.rs:473-478` | - | 查表 |
| `REALM_LADDER` 15 档经验爬阶表 | `progression.rs:35-130` | 15 档（旧 ladder） | experience 阈值 |
| `apply_progression` / `apply_progression_in_place` | `progression.rs:179-225` | - | 按 experience/karma 自动爬阶 |
| `ProgressionInput` | `progression.rs:6-23` | - | experience_gain / karma_delta / spirit_qi_delta |
| `AttemptBreakthrough` action 消费 | `gameplay.rs:77, 202, 265-280` | - | 路由到 `apply_breakthrough_action` |
| `AttemptBreakthrough` action enqueue 源 | `chat_collector.rs:197,657` | - | 玩家聊天命令，**不改** |

### 真实实现（`server/src/cultivation/`）

| 能力 | 位置 | 备注 |
|------|------|------|
| `Realm` enum 六境 | `cultivation/components.rs:13-22` | Awaken / Induce / Condense / Solidify / Spirit / Void（对齐 worldview §三） |
| `Cultivation` struct 8 字段 | `cultivation/components.rs` | realm + qi_current + qi_max + qi_max_frozen + last_qi_zero_at + pending_material_bonus + composure + composure_recover_rate |
| `try_breakthrough` + 成功率公式 | `cultivation/breakthrough.rs`（488 行） | `success = base × meridian_integrity × composure × completeness × (1 + material_bonus)`；material_bonus 封顶 +0.30 |
| `base_success_rate` 六境基础成功率 | `breakthrough.rs:26-35` | Awaken=1.0 / Induce=0.9 / Condense=0.8 / Solidify=0.7 / Spirit=0.55 / Void=0.3 |
| `breakthrough_qi_cost` | `breakthrough.rs:37-47` | Void 需 800 qi |
| `qi_max_multiplier` | `breakthrough.rs:65-74` | 突破后 qi_max × 2.0-5.0 |
| `skill_cap_for_realm` | `breakthrough.rs:80+` | 境界软挂钩 skill |
| `meridian_open.rs` | `cultivation/meridian_open.rs:23-93` | 经脉打通流程 |
| `realm_to_string()` helper | `schema/cultivation.rs:166-174` | PascalCase（"Awaken"/"Induce"） |
| `CultivationSnapshotV1.realm` | `schema/cultivation.rs:20` | **已用 `realm_to_string`**，格式正确 ✅ |
| `BreakthroughEventV1` | `schema/breakthrough-event.ts` + Rust | Started/Succeeded/Failed |
| `CultivationDeathTrigger` 6 种 cause | `cultivation/death_hooks.rs:18-26,50` | 读 `cultivation.realm` 降境 |
| `insight_flow.rs` 多处 | `cultivation/insight_flow.rs:59,83,118,251` | 已读 `cultivation.realm` 做 Insight 触发 |
| `cultivation/karma.rs`（独立模块） | 存在 | **本 plan 不碰** |

### PlayerState 6 字段的并行情况（audit 完成）

`server/src/player/state.rs:27-34`:

| 字段 | 并行于 Cultivation? | 处理 |
|------|------|------|
| `realm: String` | ✅ `Cultivation.realm: Realm` | **删** |
| `spirit_qi: f64` | ✅ `Cultivation.qi_current` | **删** |
| `spirit_qi_max: f64` | ✅ `Cultivation.qi_max` | **删** |
| `karma: f64` | ❌ Cultivation 没 karma | **保留**（运数/劫数独立系统，worldview §十二） |
| `experience: u64` | ❌ 不并行，但只喂 `progression.rs` REALM_LADDER | **删**（ladder 删了 experience 成孤儿字段） |
| `inventory_score: f64` | ❌ gameplay 独有（组队战力估算） | **保留** |

### Client 侧传统仙侠 humanizeRealm

`client/src/main/java/com/bong/client/PlayerStateViewModel.java:98-132`：
- 映射旧 ladder 的 realmKey 到传统仙侠境界称谓（已与 world view 冲突）
- **传统仙侠体系，与 worldview §三"末法去上古，只有六境"直接冲突**
- 调用点：`PlayerStateViewModel:54` humanizeRealm(snapshot.realmKey())，`CultivationScreen.java:71` 直接 `playerState.realm()`
- 本 plan 替换为六境映射（不保留 fallback）

### IPC schema 的 realm 字段双轨（关键脱节点）

| 字段路径 | 现有格式 | 数据源 | 状态 |
|------|------|------|------|
| `CultivationSnapshotV1.realm` | PascalCase (`"Awaken"`) | `cultivation.realm` via `realm_to_string()` | ✅ 正确 |
| `BreakthroughEventV1.from_realm/to_realm` | PascalCase | cultivation | ✅ 正确 |
| `PlayerStatePayload.realm` (`schema/client_payload.rs:45`) | MVP（旧 ladder） | `PlayerState.realm.clone()` | ❌ 源头错 |
| `PlayerStateSnapshot.realm` (`schema/world_state.rs:26`) | MVP | 同上 | ❌ 同上 |
| `SnapshotTargetState.realm` (`schema/inventory.rs:171`) | MVP | 同上 | ❌ 同上 |

**Agent 和 Client 收到的 realm 字段格式取决于读哪条 IPC 路径** —— 两种格式同时在跑，造成 parse 逻辑分叉。

### 脱节点总结（现状）

1. **realm 字符串格式三套**：gameplay MVP 4 档、progression ladder 15 档、cultivation 六境 —— 三套都活着
2. **`AttemptBreakthrough` action 被 gameplay 占位吃掉**（`gameplay.rs:202,265`），从未转发到 `cultivation::breakthrough`
3. **经脉数量从不进入突破条件**：gameplay 只看 experience + karma + qi，cultivation `try_breakthrough` 看 meridian_integrity 但没被触发
4. **client humanizeRealm 映射传统仙侠体系**，与 worldview 六境冲突
5. **IPC schema realm 字段有五条路径格式不统一**：2 对 3 错
6. **`progression.rs` 的 `apply_progression` 无生产调用者**（audit 确认：grep 只命中内部 test），整块死代码 ~400 行

---

## §0 设计轴心（audit 后 scope 从单头 cleanup 扩到双头 + IPC 统一）

1. **`cultivation/` 是 single source of truth** —— 所有 realm / breakthrough / meridian / qi 状态走 cultivation，`player/gameplay.rs` 只做 intent 转发，不持并行数据
2. **一次性清理 MVP 占位**，不做灰度迁移 —— 旧 ladder 的 realmKey 字符串全部删除，不保留兼容字段
3. **`PlayerState` 删 4 字段**（`realm` / `spirit_qi` / `spirit_qi_max` / `experience`），保留 `karma` / `inventory_score` —— Cultivation component 是 realm/qi 权威，不搞数据双写
4. **`AttemptBreakthrough` action 转发为 Event** —— gameplay 把 queue intent 转换成 Bevy event `AttemptBreakthroughEvent`，cultivation 侧 handler 消费（`AttemptBreakthrough` 的 enqueue 源 `chat_collector.rs` 不动）
5. **显示名中文化在 client 侧** —— server 下发 PascalCase（`"Induce"`），client 的 `humanizeRealm` **替换**成六境映射（不 fallback 到传统仙侠）
6. **六境对齐 worldview §三**：Awaken(醒灵) / Induce(引气) / Condense(凝脉) / Solidify(固元) / Spirit(通灵) / Void(化虚)
7. **`progression.rs` 的 15 档 REALM_LADDER 整块删** —— 传统仙侠 ladder 与 worldview §三"末法去上古"冲突，且 `apply_progression` 无生产调用者，纯死代码
8. **IPC schema realm 字段源头统一到 Cultivation** —— `PlayerStatePayload.realm` / `PlayerStateSnapshot.realm` / `SnapshotTargetState.realm` 的产生处全部改 Query `Cultivation` + `realm_to_string()`，与 `CultivationSnapshotV1.realm` 对齐
9. **karma 一字不改** —— karma 不是境界并行（Cultivation 没 karma 字段），是独立系统。`gameplay.rs` 中 "karma 阈值影响突破" 这条规则因 `validate_breakthrough` 被整块删除而**自然失效**，不是主动移除 karma。`PlayerState.karma` 字段、`cultivation/karma.rs` 模块、worldview §十二 运数/劫数机制全部保持现状。未来若要改 karma 参与突破，另立 plan
10. **保留 MVP 占位的 gather / combat action queue 流程** —— 本 plan 只动 breakthrough 分支，`Gather` / `Combat` 的 action 消费逻辑不动

---

## §1 删除清单

### gameplay.rs（单头 MVP）

```
const BREAKTHROUGH_RULES                           gameplay.rs:30-55     ~25 行
struct BreakthroughRule                            gameplay.rs:148-155   ~10 行
enum match 中 AttemptBreakthrough validation 分支   gameplay.rs:202-205   ~5 行
enum match 中 AttemptBreakthrough apply 分支        gameplay.rs:265-280   ~20 行
fn apply_breakthrough_action                       gameplay.rs:372-416   ~50 行
fn validate_breakthrough                           gameplay.rs:418-449   ~35 行
fn breakthrough_rule                               gameplay.rs:473-478   ~5 行
fn realm_display_name                              gameplay.rs:480-488   ~10 行
```

小计 ~165 行。

### progression.rs（双头 MVP，新增清理范围）

```
struct RealmRule                                   progression.rs:26-33  ~8 行
const REALM_LADDER: [RealmRule; 15]                progression.rs:35-130 ~95 行
struct ProgressionInput + impl                     progression.rs:6-23   ~20 行
fn apply_progression / apply_progression_in_place  progression.rs:179-225 ~50 行
fn apply_experience_gain / apply_karma_delta       progression.rs       ~30 行
ladder helper fns                                  progression.rs       ~30 行
test mod (整块删，大部分针对死代码的测试)            progression.rs:300-475 ~175 行
```

小计 ~400 行。

**注**：`apply_karma_delta` 作为函数本体会被删，但 karma 字段本身（`PlayerState.karma`）保留不动。karma 的实际操作逻辑由 `cultivation/karma.rs` 承接（已存在）。

### PlayerState 字段（4 个）

`player/state.rs:27-34`:
- 删 `realm: String`
- 删 `spirit_qi: f64`
- 删 `spirit_qi_max: f64`
- 删 `experience: u64`
- `PlayerState::default()` 同步缩水
- `PlayerState::normalized()` 同步缩水
- `power_breakdown()` 内部读 qi 改 Query Cultivation
- SQL 持久化字段 binding 同步缩水（见 §2.7）

### Client humanizeRealm 传统仙侠映射

`client/.../PlayerStateViewModel.java:98-132`：
- 删旧 ladder 的映射（~35 行逻辑）
- 替换为六境映射（~15 行，见 §2.4）

---

## §2 新增 / 修改清单

### 2.1 gameplay.rs — 改成转发

```rust
match request.action {
    GameplayAction::AttemptBreakthrough => {
        // 新：转发为 Bevy event，让 cultivation::breakthrough 消费
        attempt_breakthrough_events.send(AttemptBreakthroughEvent {
            entity: player_entity,
            player: canonical_player,
            tick: event_tick,
        });
    }
    GameplayAction::Combat(..) => { /* 不动 */ }
    GameplayAction::Gather(..) => { /* 不动 */ }
}
```

验证失败（不在 Cultivation component 下、qi 不足、realm 已到 Void）由 cultivation 侧 handler 处理并 emit narration event，不在 gameplay 层做任何 validation。

### 2.2 cultivation/breakthrough.rs — 接收 intent

```rust
#[derive(Event)]
pub struct AttemptBreakthroughEvent {
    pub entity: Entity,
    pub player: String,
    pub tick: u64,
}

pub fn handle_attempt_breakthrough(
    mut events: EventReader<AttemptBreakthroughEvent>,
    mut players: Query<(&mut Cultivation, &MeridianSystem, &StatusEffects), With<Client>>,
    mut outcomes: EventWriter<BreakthroughOutcome>,
    mut vfx: EventWriter<VfxEventRequest>,
    clock: Res<CultivationClock>,
) {
    for ev in events.read() {
        let Ok((mut cultivation, meridian, status)) = players.get_mut(ev.entity) else {
            // narration: "无修为根基"，略
            continue;
        };
        // 复用现有 try_breakthrough 逻辑（breakthrough.rs 已有 488 行完整实装）
        let result = try_breakthrough(
            &mut cultivation,
            meridian,
            sum_breakthrough_boost(status),
            &clock,
        );
        match result {
            BreakthroughResult::Success { from, to } => { /* emit BreakthroughSucceeded + vfx */ }
            BreakthroughResult::Failed { reason } => { /* emit BreakthroughFailed + narration */ }
        }
        // clear_breakthrough_boost(status)
    }
}
```

**注**：`try_breakthrough` 已存在且完整，本 handler 是薄 wrapper。注册到 App `FixedUpdate` 之后。

### 2.3 PlayerState 裁剪

`player/state.rs` 新版：

```rust
#[derive(Clone, Debug, Component, Serialize, Deserialize, PartialEq)]
pub struct PlayerState {
    pub karma: f64,
    pub inventory_score: f64,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self { karma: 0.0, inventory_score: 0.0 }
    }
}
```

所有读 `player_state.realm/spirit_qi/spirit_qi_max/experience` 的代码改为 Query `Cultivation`。

**关键生产触点（audit 已出）**：
- `server/src/network/mod.rs:477,485,566,574` — WorldState 下发 realm/spirit_qi
- `server/src/network/inventory_snapshot_emit.rs:220` — inventory snapshot 下发 realm
- `server/src/player/mod.rs:125` — 持久化加载时 clone realm
- `server/src/player/state.rs` — SQL 列 binding（第 450/462/670/848/887 行附近）、`power_breakdown()` 读 qi 改 Query Cultivation

### 2.4 client 六境映射（替换 humanizeRealm）

`client/src/main/java/com/bong/client/PlayerStateViewModel.java:98-132` 整体替换：

```java
static String humanizeRealm(String realmKey) {
    Objects.requireNonNull(realmKey, "realmKey");
    String trimmed = realmKey.trim();
    if (trimmed.isEmpty()) return "凡体";  // Cultivation 未 spawn 的 fallback
    return switch (trimmed) {
        case "Awaken"   -> "醒灵";
        case "Induce"   -> "引气";
        case "Condense" -> "凝脉";
        case "Solidify" -> "固元";
        case "Spirit"   -> "通灵";
        case "Void"     -> "化虚";
        default         -> trimmed;  // 未知值直接透传，便于 debug
    };
}
```

所有 `humanizeRealm` 调用点不变。辅助函数 `parseStage` / `chineseStage`（传统仙侠的"一二三"数字生成）一并删除。`realmProgressScore` 若有按旧 ladder 做数值评分的代码段也要改（或改读 Cultivation 的 realm ordinal）。

### 2.5 Schema realm 源头统一到 Cultivation

改三处 IPC payload 构造点：

- `server/src/network/mod.rs:477,485,566,574` — Query 增加 `&Cultivation`，从 `realm_to_string(cultivation.realm).to_string()` 取 realm，替换 `normalized.realm.clone()` / `default_state.realm.clone()`
- `server/src/network/inventory_snapshot_emit.rs:220` — 同改
- `server/src/player/mod.rs:125` — 加载持久化时不再 clone `PlayerState.realm`，在 spawn 时直接从 Cultivation component 读

调整后 `PlayerStatePayload.realm` / `PlayerStateSnapshot.realm` / `SnapshotTargetState.realm` 与 `CultivationSnapshotV1.realm` 同格式（`"Awaken"/"Induce"/...`）。

### 2.6 Rust & Java tests 迁移

**Rust 侧**：
- `progression.rs` 整个 test mod 删除（~175 行）
- `gameplay.rs` 的 breakthrough test 删除
- `network/mod.rs:1826,3714,3821` 等旧 ladder 测试字符串改 PascalCase + 新建 Cultivation component
- `inventory_snapshot_emit.rs:491,499,572,600` 同改
- `state.rs` 内部 test 删 qi/realm 相关断言
- 新增 `cultivation/breakthrough.rs` 的 `handle_attempt_breakthrough` 单测（mock Cultivation + MeridianSystem）

**Java 侧**：
- `PlayerStateViewModelTest.java` 的 `"qi_refining_3"` → `"Induce"`
- `PlayerStateHandlerTest.java`（两个）的 `"foundation_1"` / `"qi_refining_3"` → `"Induce"` 或 `"Condense"`
- `BongNetworkHandlerPayloadFixtureTest.java` 同改
- `InventorySnapshotHandlerTest.java` 的 `"qi_refining_1"` 改 `"Awaken"`
- `state/PlayerStateViewModelTest.java` 的 `"foundation_establishment"` → `"Induce"` 或删测试（如果是专测 stage parse 的，整块删）
- `test/resources/bong/payloads/valid-player-state.json` 的 `"realm": "qi_refining_3"` → `"realm": "Induce"`

### 2.7 SQL 持久化 schema migration

`PlayerState` 删 4 字段后，SQL 表列定义同步：
- 新 migration 文件 `server/migrations/NNNN_cleanup_player_state_columns.sql`
- `ALTER TABLE player_core DROP COLUMN realm, DROP COLUMN spirit_qi, DROP COLUMN spirit_qi_max, DROP COLUMN experience;`
- Cultivation 持久化已独立（若已存在），否则本 plan 同步加 Cultivation 的 SQL 持久化；Q 新增（见 §5）

**注**：新 migration 编号跟现有序列。旧存档加载时 Cultivation 按 default spawn（玩家从 Awaken 境重新开始），playtest 阶段可接受。

---

## §3 跨模块影响面 audit（实际命中）

| 影响点 | 命中文件 + 行号 | 改动摘要 |
|------|------|------|
| `player_state.realm` 读取（生产） | `network/mod.rs:477,485,566,574`, `inventory_snapshot_emit.rs:220`, `player/mod.rs:125`, `state.rs` 内部 normalizer | Query Cultivation 替换 |
| `player_state.spirit_qi` / `_max` 读 | `state.rs:94,128,462,670,848,887`, `network/mod.rs` 733/891, power_breakdown | 改 Cultivation + zone spirit_qi 保留为 zone 侧 |
| `player_state.experience` 读 | `state.rs:96,453,891,426,768,851`, `progression.rs`（随删除消失） | 删字段后清理引用 |
| 旧 ladder realmKey 硬编码 | `progression.rs:37-91`（随 LADDER 删）, `network/mod.rs:1826,3714,3821` test, `inventory_snapshot_emit.rs:491,499,572,600` test, `gameplay.rs:280`（"突破" rejection 文案） | 删除或改 `"Induce"` 等 |
| 旧 ladder 后续阶段 realmKey | `progression.rs:97-121`（随 LADDER 删）, client tests | 删除或改六境 |
| `AttemptBreakthrough` action | enqueue: `chat_collector.rs:197,657`（**不改**）；消费: `gameplay.rs:77,202,265`（改转发） | enqueue 端不动，消费端转 event |
| `realm_display_name` 调用点 | `gameplay.rs:422`（自身），narration 路径若有 | 随删除，server 不再下发中文 |
| IPC schema realm 字段 | `schema/cultivation.rs:20,35` ✅, `schema/client_payload.rs:45`, `schema/world_state.rs:26`, `schema/inventory.rs:171` | §2.5 改源头，schema 结构不变（字段仍叫 realm） |
| Client realm 显示 | `PlayerStateViewModel.java:98-132` humanizeRealm, `CultivationScreen.java:71` 直接 playerState.realm(), `CultivationDetailHandler.java` | §2.4 替换 humanizeRealm + Screen 的直接读可保留（值已是 PascalCase 传过来的，humanize 会正确转） |
| Test fixtures (Rust) | `progression.rs tests`, `network/mod.rs tests`, `inventory_snapshot_emit.rs tests`, `state.rs tests` | §2.6 |
| Test fixtures (Java) | 7 个 test 文件 + `test/resources/bong/payloads/valid-player-state.json` / `invalid-player-state-missing-fields.json` | §2.6 |
| SQL migration | 新文件 `server/migrations/NNNN_cleanup_player_state_columns.sql` | §2.7 |
| `PowerBreakdown` 计算 | `state.rs` 内 `power_breakdown()` 现读 `normalized.spirit_qi` / `normalized.spirit_qi_max` / `normalized.experience` | 改 Query Cultivation 取 qi_current/qi_max；experience 项改为 realm ordinal 或删除该维度 |

---

## §4 风险评估

| 风险 | 缓解 |
|------|------|
| `PlayerState` 字段删除打断 SQL 反序列化 | 写迁移脚本 + playtest 阶段 reset 存档；确保 Cultivation 持久化已独立存表 |
| IPC schema realm 格式切换破坏 agent 消费 | `CultivationSnapshotV1.realm` 已是 PascalCase，agent 只要认 PascalCase 即可；本 plan 前 agent 可能混收两种格式，改完反而统一。验证 agent 现有 tests |
| Client 现有 realm display 逻辑和新 humanizeRealm 冲突 | §2.4 一次性替换，删旧映射 + 删 parseStage/chineseStage helpers；所有 test 同步改 |
| MVP demo 流程被破坏（player 无法突破） | `cultivation::breakthrough::try_breakthrough` 已完整实装 488 行，handler 是薄 wrapper；起稿时 mock Cultivation 走测试能过 |
| agent narration 从旧体系命名切到六境命名观感跳跃 | 迁移时一并检查 agent narration templates（如果有 hardcode 旧境界名） |
| `apply_progression` 删除后 experience 采集逻辑失去出口 | experience 字段整体删除，Gather/Combat action 里的 experience_gain 一并删（或改喂 plan-skill-v1 的 SkillProgress）。若后者，需 plan-skill-v1 同步 |
| Client Cultivation 数据不同步导致显示 "凡体" fallback | 首次登录要确保 Cultivation 在 `PlayerState` 之前 spawn；startup system 顺序校对 |
| karma 独立系统中的 breakthrough 门槛丢失 | 骨架 §0 轴心 9 明确 karma 不影响突破（自然副作用）；如果设计上想保留"大恶人突破难"，不在本 plan 做，另立 plan |
| SQL migration 在 playtest 阶段意外打到 prod 存档 | migration 加 dry-run 选项，或默认只跑在 test 存档；部署 runbook 标注 |

---

## §5 开放问题（audit 后 Q1-Q3 已答，Q4-Q9 保留）

### 已答

- **Q1** ✅ `cultivation/breakthrough.rs` 的突破条件：`try_breakthrough` 函数用 `success = base_rate × meridian_integrity × composure × completeness × (1 + material_bonus)`，`base_rate` 按六境查表（Awaken=1.0 / Induce=0.9 / ... / Void=0.3），`material_bonus` 封顶 +0.30。qi 消耗按境界查表（Void=800 qi）。化虚渡劫走 `tribulation.rs::initiate_tribulation`，本 handler 不处理
- **Q2** ✅ PlayerState 6 字段：realm/spirit_qi/spirit_qi_max 与 Cultivation 并行（删），experience 不并行但只喂 REALM_LADDER（随 ladder 删），karma/inventory_score 独立保留
- **Q3** ✅ client 端 `PlayerStateViewModel.humanizeRealm` 已存在但映射传统仙侠，替换为六境映射（见 §2.4）
- **Q9** ✅ Cultivation 持久化已独立（`server/src/persistence/mod.rs:1634/1931/3896/5698` 已 Query `&Cultivation`），§2.7 只需删 PlayerState 列，无需新建 Cultivation 存表（scope 不扩）

### 保留

- **Q4** `AttemptBreakthrough` 是否要求玩家主动 intent？现状：保留（走 chat 命令 `/attempt_breakthrough` via `chat_collector.rs:197,657`）。Cultivation 侧可增加"条件满足时自动建议突破" narration 但不自动触发，给玩家选择权
- **Q5** `BreakthroughEventV1.success_rate / severity` 字段语义：success_rate = `try_breakthrough` 计算出的最终成功率（0.0-1.0）；severity 留给反噬程度，MVP 阶段固定 `"normal"` 或依据失败类型映射。对齐 plan-cultivation-v1 §6.1
- **Q6** 突破失败 narration：由 cultivation::breakthrough 自己 emit，格式参照现有 `CultivationSnapshotV1`，不走 gameplay 路径。narration 措辞细节归 plan-narrative-v1
- **Q7** 境界跌落（`RealmRegressed` event from qi_zero_decay.rs）的 UI 反馈：现状 client 已消费 `CultivationSnapshotV1`，realm 字段变化自动刷新 UI，无需额外 event
- **Q8** `CultivationSnapshotV1.qi_max_frozen` 是 cultivation 内部状态（qi 归零后 qi_max 冻结 tick 阈值），client 可显示也可不显示 —— 留给后续 UI plan

---

## §6 实施规模预估（audit 后修正）

| 模块 | 规模（行） |
|------|-------|
| `gameplay.rs` 删除 + `AttemptBreakthrough` 转发 | -165 / +30 |
| `progression.rs` 整块删（REALM_LADDER + apply_progression + tests） | -400 / +0 |
| `PlayerState` 删 4 字段 + 影响点修复 | -40 / +80（散落） |
| `cultivation/breakthrough.rs` `AttemptBreakthroughEvent` + handler | +80 |
| `schema/*.rs` IPC 源头改 Cultivation Query | +80（散落） |
| Client `humanizeRealm` 替换 + 辅助函数删除 | -40 / +25 |
| Rust test 迁移 / 删除 | -200 / +100 |
| Java test fixture 改 | -30 / +30 |
| `test/resources/bong/payloads/*.json` | -10 / +10 |
| SQL migration + Cultivation 持久化（视 Q9） | +60 ~ +160 |
| **合计** | **-885 / +500 ≈ 净减 385 行**（触点约 20 文件） |

相对聚焦的 refactor，一次 worktree 能吃完。若 Q9 答"Cultivation 未独立存表"，再加 ~100 行。

---

## §7 Active 阶段执行检查表

骨架 → active 升级（2026-04-24 完成），以下项已就绪：

- [x] worldview §四.零 已 merged（战力分层描述）
- [x] `cultivation/breakthrough.rs` 突破条件稳定（Q1 答复）
- [x] `PlayerState` 字段 audit 完成（Q2 答复）
- [x] Client humanizeRealm audit 完成（Q3 答复）
- [x] progression.rs 死代码状态确认（无生产调用者，仅 test）
- [x] IPC schema realm 源头双轨问题 audit 完成
- [x] karma 保留决策确认（不并行，不纳入本 plan）
- [x] Q9 Cultivation 持久化独立性 audit（2026-04-25：`persistence/mod.rs:1634/1931/3896/5698` 已 Query `&Cultivation`，独立存表，§2.7 不需新建 Cultivation 持久化）

### active 阶段建议开工顺序（`/consume-plan cultivation-mvp-cleanup`）

1. **audit Q9**：grep `cultivation` 在 SQL / persistence/mod.rs 里的存表情况，决定 §2.7 规模
2. 加 `AttemptBreakthroughEvent` event + `handle_attempt_breakthrough` handler（空壳），注册到 App，跑通编译
3. 改 `gameplay.rs` 的 `AttemptBreakthrough` 分支为转发，**保留**旧 `apply_breakthrough_action` 暂不删
4. playtest：用 chat 命令触发突破，验证新路径（Cultivation 成功率 / qi 消耗生效）
5. 确认后删 `gameplay.rs` MVP 代码（§1 gameplay.rs 部分）
6. 删 `progression.rs` 的 REALM_LADDER + apply_progression + tests（§1 progression.rs 部分）
7. `PlayerState` 删 4 字段，改所有读点为 Query Cultivation（§2.3）
8. `schema/*.rs` IPC 源头改 Cultivation Query（§2.5）
9. client `humanizeRealm` 替换为六境映射（§2.4）
10. 改所有 test fixtures（Rust + Java + JSON，§2.6）
11. SQL migration（§2.7）
12. `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test` + `./gradlew test build` 全绿
13. 实机 playtest：/attempt_breakthrough 走完整流程、HUD 显示中文、IPC 同步

---

**下一步**：`/consume-plan cultivation-mvp-cleanup` 启动 active 阶段。Q9 已答（Cultivation 持久化独立），可直接从 §7 开工顺序第 2 步（加 `AttemptBreakthroughEvent`）开始。

---

## §8 进度日志

- 2026-04-25：Q9 audit 完成（`persistence/mod.rs` 已 Query `&Cultivation`，独立存表）；§7 检查表全绿，可直接进 active 实施（§1/§2 删除-新增清单尚未动）。
