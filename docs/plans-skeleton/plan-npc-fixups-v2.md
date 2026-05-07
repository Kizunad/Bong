# Bong · plan-npc-fixups-v2 · 骨架

NPC 第二批正确性 bug fastlane（plan-npc-fixups-v1 ⏳ 三 bug 后续，**v1 仍在骨架阶段未实施**），承接 sonnet Explore 异步探查（2026-05-07）输出 **8 个 ECS query miss / Valence Despawned 软删 race / Action Executing 无超时 / 多 zone plot 选择错乱** 类 bug + 提到 3 个未列待二次探查（→ P3 / 可能 v3）。延续 v1 「每 bug 独立 PR + 饱和回归测试 pin 行为」节奏。**主题统一为 ECS lifecycle 与状态机正确性**——大多数 bug 共性是 query 缺 `Without<Despawned>` filter + Action `Executing` 状态分支没有超时 / 没有 silent fallback → Failure 转换 → silent stuck。

**交叉引用**：`plan-npc-fixups-v1.md` ⏳（前置三 bug + Explore 探查派生本 plan）· `plan-npc-ai-v1.md` ✅（基础 NPC 系统 + big-brain Action 状态机框架 + AscensionQuotaStore + AutoWavePacing）· `plan-npc-perf-v1.md` ⏳（perf P0 baseline 录档前应已修本 plan #1 #2，否则"NPC 卡死/错位"会污染性能基线）· `plan-tribulation-v1.md`（化虚名额 + 渡劫波次状态机，本 plan #6 #8 修其 ECS lifecycle race）· `plan-lingtian-npc-v1.md`（道伥召唤来源 zone，本 plan #1 修多 zone 选错地块）

**worldview 锚点**：无（基础 bug fix 类，不引入玩法）。仅 #6（化虚 NPC 渡劫中被击杀偶发"化虚成功"误记录）跟 worldview §三:124-187 NPC 与玩家死亡平等有间接关联——化虚名额是稀缺资源（默认 4）+ NPC 渡劫中战死必须按 plan-tribulation 规则失败而非成功；本 bug 让持久化层偶尔写错 → 跟 worldview 一致性受损

**qi_physics 锚点**：无（不动真元 / 守恒律 / 衰减常数）

**前置依赖**：

- `plan-npc-fixups-v1` ⏳ → 完成 v1 P0 重力 idle 修复后，本 plan P1 #3 chase/flee Failure 不停 navigator 才能稳定测试（v1 修了 idle 重力 + navigator 行为，v2 修 Action 状态机层）
- `plan-npc-ai-v1` ✅ → big-brain Action / AscensionQuotaStore / AutoWavePacing / NpcReproductionRequest 等已实装
- 无其他依赖

**反向被依赖**：

- `plan-npc-perf-v1` ⏳ → P0 baseline 录档前应已修本 plan #1 #2 #6 #8（避免持久化错乱 + NPC 卡死污染性能基线）
- `plan-npc-virtualize-v1` ⏳ → P3 dormant NPC 渡虚劫强制 hydrate 前应修 #6 #8（避免 hydrate-on-tribulation 时撞 quota race）
- `plan-tribulation-v1` ✅（已完成）→ 本 plan #6 #8 是其 ECS lifecycle 缺失补强
- `plan-lingtian-npc-v1` ✅（已完成）→ 本 plan #1 是其多 zone 部署的隐式前提

---

## 接入面 Checklist

- **进料**：
  - `LingtianPlot` Component（无 zone 字段，本 plan #1 要求加）+ `ZonePressureCrossed` event + `ActiveLingtianSessions` Resource
  - big-brain `ActionState`（Init/Requested/Executing/Success/Failure/Cancelled）+ `Actor(entity)` + `BigBrainSet::Actions` schedule set
  - Valence `Despawned` Component（软删 marker，1-tick 窗口内 entity 仍存在 + components 仍可 query）
  - `AscensionQuotaStore` Resource（化虚名额 max=4）+ `TribulationState` + `NpcTribulationPacing` + `TribulationWaveCleared` event
  - `PendingRetirement` Component + `NpcRetireRequest` event + commands deferred 写入语义
  - `tsy_spawn_pools.json` / `tsy_drops.json` 数据文件 + `load_tsy_*_registry` panic 路径
  - `wander_target_for` 函数（`zone_registry` Option 缺失时无边界 clamp）
  - `socialize_action_system` 的 `SocializeState` query（thinker 重启后未插入旧 entity）
  - `spawn_commoner_npc_at` 繁殖路径（patrol_target 跟 spawn_position 重合）
- **出料**：
  - **#1**：`LingtianPlot { zone: String, ... }` 字段 + `spawn_daoshen_on_pressure_high` 用 `plots.iter().find(|p| p.zone == event.zone)` 替代 `next()`
  - **#2**：`brain.rs:929-930` Executing query miss 改为 `*state = ActionState::Failure; continue;`（禁止 silent `continue`）
  - **#3**：`chase_action_system` / `flee_action_system` / `flee_cultivator_action_system` query miss 时用独立 navigator query 强制 stop（或 Cancelled 路径覆盖）
  - **#4**：`tsy_hostile.rs:351,354` 改 `unwrap_or_else(|e| { tracing::error!(...); Default::default() })` + 后续 system 检查 pool 非空 warn
  - **#5**：`retire_action_system` 用 `Added<PendingRetirement>` 触发 NpcRetireRequest 而非在 action system 直接 send（幂等保护）
  - **#6**：`npc_tribulation_auto_wave_tick` query 加 `Without<Despawned>` filter
  - **#7**：farming Action 加 `session_deadline_tick: u64` + Executing 超时 → Failure
  - **#8**：`release_quota_for_ended_tribulations` 的 `ongoing` query 加 `Without<Despawned>`
  - **新增 telemetry**（可选）：`tracing::warn!("npc action stuck in Executing for {n} ticks")` 当 Action Executing 超过阈值（debug 用，P3 可补）
- **共享类型 / event**：
  - 新增字段 `LingtianPlot.zone: String` + farming action `session_deadline_tick`，**复用** ActionState / TribulationState / Despawned 等已有类型
  - 不新增 Component / Event / Schema
- **跨仓库契约**：
  - server: 多模块改造（npc/lingtian_pressure.rs / npc/brain.rs / npc/tribulation.rs / npc/farming_brain.rs / npc/tsy_hostile.rs），无对外 schema 变化
  - agent: 无（NpcDigest 通道 unchanged）
  - client: 无（行为可见结果 = 卡死 / 错位 / 误伤升 realm 不再发生，玩家观感"更稳定"但不是新视觉）
- **worldview 锚点**：无
- **qi_physics 锚点**：无

---

## §0 设计轴心

- [ ] **每个 bug 独立 PR**（继承 v1 节奏）：8 个 fix 互不依赖，分别走 fastlane。打包 = reviewer 上下文切换 + regress 风险翻倍
- [ ] **饱和化回归测试 pin 行为**：每个 bug 至少 ① happy path（修复后行为）② 边界（修法触发 / 不触发）③ 不要 regress 旁边逻辑。**重点：despawn race 类 bug 必须用模拟 Despawned 插入的测试场景**（不能仅靠"正常情况下不会 despawn"逃避）
- [ ] **状态机 silent `continue` 必须改 Failure**：big-brain Action `Executing` 状态分支若 query miss 用 `continue` → 下次 tick 仍 Executing → silent 死锁。**强制规则**：所有 Action `Executing` 内 `let Ok(...) else { continue }` 必须改 `let Ok(...) else { *state = ActionState::Failure; continue }`，让 picker 能切别的 action
- [ ] **Despawned filter 是 ECS query 的卫生**：任何 query 涉及 NPC entity 的 Component 修改 / event 发送 → 必须加 `Without<Despawned>`（除非显式处理软删窗口）。**这是 §3 强约束源头**，违反 = 隐式 race
- [ ] **Action Executing 必须有超时**：所有 Executing 等外部条件（session 完成 / event 接收 / state 转换）的 Action 必须有 `deadline_tick` / `tick_count`，超时 → Failure。否则一个 stuck = 永久占 NPC slot
- [ ] **panic 是部署灾难，`tracing::error!` + `Default::default()` 是优雅降级**：register / load 路径的 panic 在 CI / 生产 = 服务无法启动。改 error log + Default 让用户至少能跑（功能降级 != 服务崩溃）。统一日志级别为 `error`（同 §2.4 修法 line 414-417 实现一致）
- [ ] **deferred commands 跟同 tick 多次调用不安全**：`commands.entity().insert(C)` 是 deferred，同一 tick 内 `query.contains(C)` 仍返回 false。**幂等触发用 `Added<C>` event reader 而非 action system 内 send**

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **bug #1（lingtian 多 zone 选错地块）+ bug #2（MeleeAttack Executing query miss 卡死）— 玩家可见高优先级**：`LingtianPlot.zone` 字段 + `spawn_daoshen_on_pressure_high` filter / `brain.rs:929-930` continue → Failure | `cargo test npc::lingtian_pressure::tests::multi_zone_picks_correct_plot` / `cargo test npc::brain::tests::melee_executing_query_miss_transitions_to_failure` 全过 / WSLg 实测多 zone 道伥召唤位置正确 + beast 战斗中击杀对手不再"冻僵" |
| **P1** ⬜ | **bug #3 + #4 + #5 — 中优先级**：`chase/flee_action_system` Failure 时强制 stop navigator + `tsy_hostile.rs` panic → warn fallback + `retire_action_system` 用 `Added<PendingRetirement>` 触发事件 | `cargo test npc::brain::tests::flee_failure_stops_navigator` + `cargo test npc::tsy_hostile::tests::missing_json_warns_uses_default` + `cargo test npc::brain::tests::retire_action_no_double_send_on_double_tick` 全过 |
| **P2** ⬜ | **bug #6 + #7 + #8 — 中-低优先级（lifecycle race + 超时缺失）**：tribulation auto_wave + AscensionQuota 加 `Without<Despawned>` + farming Action 加 session_deadline_tick 超时 | `cargo test npc::tribulation::tests::auto_wave_skips_despawned_npc` + `cargo test npc::farming_brain::tests::till_action_times_out_when_session_stuck` + `cargo test npc::tribulation::tests::quota_releases_immediately_on_despawn` 全过 |
| **P3** ⬜ | **剩余未列 bug + 二次探查**：sonnet Explore 已提到的 3 个未列（socialize_action_system 旧 entity Failure / spawn_commoner_npc_at patrol_target == spawn_position / wander_target_for zone 缺失越区）+ 派 agent 二次探查找剩余 | 二次探查输出 → 评估纳入本 plan / 派生 plan-npc-fixups-v3 / 入 reminder |

---

## §2 各 bug 根因 + 修法

### §2.1 bug #1 — lingtian_pressure 多 zone 时道伥召唤打错地块（高 → P0）

**file:line**：`server/src/npc/lingtian_pressure.rs:30`

**根因**：

```rust
let Some(target_plot) = plots.iter().next() else { ... };
// e.zone = "lingquan_marsh"，但 plots.iter() 返回的可能是 "spawn" zone 的地块
let center = DVec3::new(target_plot.pos.x as f64, ...);
```

`plots: Query<&LingtianPlot>` 无过滤，多 zone 时 ECS 遍历顺序决定哪个地块被命中，与触发事件的 zone 无关。

**症状**：灵田压力触发时 9 个道伥生成在**另一个** zone 的农场，玩家可见 NPC 凭空出现在错误位置，正确 zone 无应激反应。

**修法**：

```rust
// 1. LingtianPlot 加 zone 字段
pub struct LingtianPlot {
    pub zone: String,  // ← 新增
    pub pos: BlockPos,
    // ...
}

// 2. spawn_daoshen_on_pressure_high 改 filter
let Some(target_plot) = plots.iter().find(|p| p.zone == event.zone) else {
    tracing::warn!("[lingtian-pressure] zone={} 无对应 plot，跳过道伥召唤", event.zone);
    continue;
};
```

**回归测试**：
- `multi_zone_picks_correct_plot`：spawn 2 zone × 各 1 plot，触发 zone A 事件 → 道伥在 zone A 不在 zone B
- `single_zone_works_unchanged`：1 zone 1 plot，行为跟以前一致
- `event_zone_no_match_warns`：触发不存在的 zone 事件 → emit warn + 不 spawn 道伥

### §2.2 bug #2 — MeleeAttackAction Executing query miss 永久卡死（高 → P0）

**file:line**：`server/src/npc/brain.rs:929-930`

**根因**：

```rust
ActionState::Executing => {
    let Ok((_npc_pos, mut bb, profile, _)) = npcs.get_mut(*actor) else {
        continue;  // ← 应为 *state = ActionState::Failure; continue;
    };
    // ...
}
```

`Requested` 时（line 922-926）同一 query 用 `if let Ok` 不设 Failure（合理：等下一 tick 重试），而 `Executing` 时直接 `continue`，下一 tick 仍 Executing → silent 死锁。Valence 软删窗口（Despawned 已插入但 entity 1 tick 内仍 query 命中）+ 某些 filter 失败 → Action 永卡。**big-brain `FirstToScore` picker 不会中断已 Executing 的 action**。

**症状**：极端情况（战斗 NPC 在攻击中途被击杀过渡期）beast / disciple 永久冻结在 MeleeAttack Executing，无法再次移动 / 攻击。

**修法**：

```rust
ActionState::Executing => {
    let Ok((_npc_pos, mut bb, profile, _)) = npcs.get_mut(*actor) else {
        *state = ActionState::Failure;  // ← 新增
        continue;
    };
    // ...
}
```

**回归测试**：
- `melee_executing_query_miss_transitions_to_failure`：模拟 entity Despawned 插入但 query miss → 下一 tick state == Failure（不再 Executing）
- `melee_executing_normal_unchanged`：query 正常 → 行为不变
- **同样修法应用于全 Action Executing**：grep `ActionState::Executing => { let Ok(...) else { continue }` 模式遍历 brain.rs / social.rs / territory.rs / relic.rs / farming_brain.rs，每个加 Failure 转换 + 配测试（这是个**模式 bug**，可能不止 brain.rs:929 一处；P0 要做全局 grep + 修复）

### §2.3 bug #3 — Chase/Flee Failure 不停 Navigator（中 → P1）

**file:line**：`server/src/npc/brain.rs:831-836`（chase_action_system）+ flee_action_system + flee_cultivator_action_system 类似

**根因**：

```rust
let Ok((..., mut navigator)) = npcs.get_mut(*actor) else {
    *state = ActionState::Failure;  // ← 拿不到 navigator，无法 stop
    continue;
};
```

QueryItem 解构失败时根本拿不到 navigator，故没有 `navigator.stop()`。NPC 保持向旧目标位移；big-brain 同时已选定新 action（如 Wander），两个 action 的 navigator goal 相互覆盖 → 抖动 / 鬼走。

**症状**：FleeAction query miss（含 Despawn 过渡期）时 NPC 在"逃跑 + 漫步"之间出现 1 tick 漂移抖动。低频但 NPC 密集时累积。

**修法**：

```rust
// 改用独立 navigator query 强制 stop
let Ok((..., mut navigator)) = npcs.get_mut(*actor) else {
    if let Ok(mut nav) = navigators.get_mut(*actor) {
        nav.stop();
    }
    *state = ActionState::Failure;
    continue;
};
```

或者更系统的做法：`Cancelled` 分支统一覆盖 stop 逻辑 + Failure 也走 Cancelled 路径。

**回归测试**：
- `chase_failure_stops_navigator`：query miss → navigator goal cleared
- `flee_failure_stops_navigator`：同上
- `flee_cultivator_failure_stops_navigator`：同上
- `chase_to_wander_no_stutter`：模拟 chase Failure → next tick wander Requested → 1 tick 内不 jitter

### §2.4 bug #4 — tsy_hostile JSON 加载失败 → 服务器 panic（中 → P1）

**file:line**：`server/src/npc/tsy_hostile.rs:351,354`

**根因**：

```rust
let spawn_pools = load_tsy_spawn_pool_registry().unwrap_or_else(|error| {
    panic!("[bong][tsy-hostile] failed to load tsy_spawn_pools.json: {error}")
});
```

数据文件缺失或 JSON 错误 → 服务器启动 panic，无降级路径。

**症状**：CI 缺文件时服务器无法启动；生产配置误改后服务崩溃，无回退。

**修法**：

```rust
let spawn_pools = load_tsy_spawn_pool_registry().unwrap_or_else(|error| {
    tracing::error!("[bong][tsy-hostile] failed to load tsy_spawn_pools.json: {error} — using empty registry, tsy hostile spawning disabled");
    Default::default()
});
// 后续 system 检查 pool 非空时 warn
```

**回归测试**：
- `missing_spawn_pools_warns_uses_default`：mock 文件缺失 → 不 panic + emit error + registry empty
- `malformed_json_warns_uses_default`：mock 损坏 JSON → 同上
- `valid_json_loads_unchanged`：正常加载行为不变
- `empty_registry_no_spawn`：empty registry → tsy_hostile_spawn_system 不 spawn 任何 entity（不 panic）

### §2.5 bug #5 — RetireAction NpcRetireRequest double-send 配额穿透（中 → P1）

**file:line**：`server/src/npc/brain.rs:593-598`

**根因**：

```rust
ActionState::Requested => {
    if pending_retirement.is_none() {
        commands.entity(*actor).insert(PendingRetirement);  // deferred
        retire_requests.send(NpcRetireRequest { entity: *actor }); // 立即
    }
    *state = ActionState::Executing;
}
```

`PendingRetirement` 本 tick 尚未写入，下一个 `Requested` 调用前查询仍为 `None`。big-brain 在某些版本可能 double-tick → 事件重发 → commoner 生出两个新生儿 → NPC 总量静默超配额。

**症状**：低概率凡人老死触发两次 `NpcReproductionRequest`，NPC 总量静默超 `max_npc_count`。

**修法**：

```rust
// 用 Added<PendingRetirement> event reader 在独立 system 触发 NpcRetireRequest
fn emit_retire_request_on_pending_added(
    query: Query<Entity, Added<PendingRetirement>>,
    mut requests: EventWriter<NpcRetireRequest>,
) {
    for entity in &query {
        requests.send(NpcRetireRequest { entity });
    }
}

// retire_action_system 仅 insert + 转 Executing
ActionState::Requested => {
    if pending_retirement.is_none() {
        commands.entity(*actor).insert(PendingRetirement);
    }
    *state = ActionState::Executing;
}
```

**回归测试**：
- `retire_action_no_double_send_on_double_tick`：模拟同一 tick action 触发两次 → NpcRetireRequest 仅 emit 1 次
- `retire_action_normal_emits_once`：正常 retire 流程 → emit 1 次
- `multiple_npcs_retire_same_tick`：3 NPC 同 tick retire → 3 个独立事件
- `pending_retirement_marker_persists`：首次 Requested 后 PendingRetirement 一直存在直到死亡（防止重新触发）

### §2.6 bug #6 — npc_tribulation_auto_wave 软删 NPC 误升 realm（中 → P2）

**file:line**：`server/src/npc/tribulation.rs:95-111`

**根因**：

```rust
pub(crate) fn npc_tribulation_auto_wave_tick(
    mut npcs: Query<(Entity, &TribulationState, &mut NpcTribulationPacing), With<NpcMarker>>,
    // ↑ 缺 Without<Despawned>
    mut cleared: EventWriter<TribulationWaveCleared>,
) {
```

NPC 在渡劫中被战斗击杀（Despawned 插入），系统在 entity 真正删除前最后 1 tick 仍推进 `wave_current` → 发送 `TribulationWaveCleared` → 消费方 `tribulation_wave_system` 升 realm → `handle_npc_terminated` 随后插入 Despawned → realm 升级写入已标记删除的 entity，持久化层可能记错"化虚成功"。

**症状**：NPC 渡劫中被击杀偶发被错误记录"化虚成功"，死亡日志与实际境界不一致；`AscensionQuotaStore` 收到两次 release（double-release，无害但不干净）。

**修法**：

```rust
pub(crate) fn npc_tribulation_auto_wave_tick(
    mut npcs: Query<
        (Entity, &TribulationState, &mut NpcTribulationPacing),
        (With<NpcMarker>, Without<Despawned>),  // ← 加 filter
    >,
    // ...
)
```

**回归测试**：
- `auto_wave_skips_despawned_npc`：模拟 NPC 在渡劫中插入 Despawned → 该 tick 不推进 wave_current + 不 emit TribulationWaveCleared
- `auto_wave_normal_npc_unchanged`：未 despawn NPC 行为不变
- `npc_killed_during_tribulation_logs_failure_not_success`：e2e 模拟渡劫中击杀 → death log cause = Combat 不是 Ascension

### §2.7 bug #7 — Farming Action Executing 无超时永久卡死（中 → P2）

**file:line**：`server/src/npc/farming_brain.rs:434-440`（TillAction，PlantAction / HarvestAction / ReplenishAction 类似）

**根因**：

```rust
ActionState::Executing => {
    if !sessions.has_session(*actor) {  // 无超时 tick 计数器
        cultivator.record_farming_success();
        *state = ActionState::Success;
    }
    // 否则永远 continue
}
```

若 `ActiveLingtianSessions` 因 bug 未移除 actor session（如 plot 被外部清除但 session 未 cleanup），NPC 永续 Executing，picker 不会中断 → NPC 停在地里不动直到服务器重启。

**症状**：散修灵田 NPC 偶发"冻住"——停在地块中心不动，不耕不种，NPC 计数仍占配额。

**修法**：

```rust
// ScatteredCultivator（或 farming action state struct）加：
pub session_deadline_tick: Option<u64>,

// Requested 时记录
ActionState::Requested => {
    cultivator.session_deadline_tick = Some(current_tick + MAX_FARMING_SESSION_TICKS);
    // ...
}

// Executing 检查超时
ActionState::Executing => {
    if !sessions.has_session(*actor) {
        cultivator.record_farming_success();
        *state = ActionState::Success;
    } else if cultivator.session_deadline_tick.map_or(false, |dl| current_tick > dl) {
        sessions.remove_session(*actor); // 强制清理
        *state = ActionState::Failure;
        tracing::warn!("[farming] action timeout actor={:?}", actor);
    }
}
```

**回归测试**：
- `till_action_times_out_when_session_stuck`：模拟 session 永不结束 + 推进 MAX_FARMING_SESSION_TICKS → state == Failure + session 被清理
- `till_action_normal_completes`：正常 session 完成 → Success
- `till_action_deadline_resets_on_requested`：连续两次 Requested → deadline 重新设
- **同样修法应用于 PlantAction / HarvestAction / ReplenishAction**：每个独立测试

### §2.8 bug #8 — AscensionQuotaStore 软删 NPC 1-tick quota 占用泄漏（低 → P2）

**file:line**：`server/src/npc/tribulation.rs:116-133`

**根因**：

```rust
ongoing: Query<Entity, (With<NpcMarker>, With<TribulationState>)>,
// ↑ 缺 Without<Despawned>
let still_tribulating: HashSet<Entity> = ongoing.iter().collect();
// Despawned NPC 仍出现在 still_tribulating → 当 tick 不释放
```

Valence 软删窗口内已插入 `Despawned` 但未真正删除的 NPC 依然被认为在渡劫，quota 延迟 1 tick 释放。

**症状**：渡劫 NPC 被击杀时全服化虚名额（默认 4）在额外 1 tick 内少 1 个空位。1 tick 内其他 Spirit realm NPC 的 `StartDuXuAction::Requested` 若执行会因 quota 满被拒（Failure），下次重试。低频且自愈，但 4 NPC 同时渡劫 + 混战场景下可见。

**修法**：

```rust
ongoing: Query<Entity, (With<NpcMarker>, With<TribulationState>, Without<Despawned>)>,
```

**回归测试**：
- `quota_releases_immediately_on_despawn`：模拟渡劫 NPC 插入 Despawned → 同 tick quota 释放
- `quota_normal_release_on_tribulation_end`：正常渡劫结束行为不变
- `multiple_simultaneous_despawns`：4 NPC 同时被 despawn → quota 全部释放（=4）

### §2.9 P3 占位：剩余未列 + 二次探查（→ P3）

sonnet Explore 已提到但未详细列出的 3 个：

1. `socialize_action_system` 的 `SocializeState` query 在 `disciple_npc_thinker` 实际启用后会对未插入该 component 的旧 entity 报 Failure（npc/social.rs，需要 P3 详查）
2. `spawn_commoner_npc_at` 繁殖时 `patrol_target == spawn_position`（无 patrol 范围偏移）导致新生儿 home 位置与死亡点重合 → 看似不动（npc/spawn.rs，需要 P3 详查）
3. `wander_target_for` 在 `zone_registry` 缺失时生成无边界约束的目标坐标（可能越区）（npc/brain.rs:1342-1371，需要 P3 详查）

**P3 阶段动作**：派 sonnet Explore 二次探查这 3 个 bug 的精确修法 + 找剩余未发现的 bug。结果回来后：
- 如发现 ≤ 3 个新 bug → 追加 §2.10+ 到本 plan
- 如发现更多 / 性质独立 → 派生 plan-npc-fixups-v3
- 仅边角细节 → 入 reminder

---

## §3 ECS lifecycle 强约束（CLAUDE.md 风格规则）

> **本节是后续所有 NPC 模块代码必守的底盘约束**。违反 = 隐式 race / silent stuck，docs/CLAUDE.md §四 应加红旗（决策门 #3）。

### 强约束规则

1. **NPC entity Component 修改 / event 发送类 query 必须加 `Without<Despawned>`**（除非显式处理软删窗口）。grep 模式：`Query<.*With<NpcMarker>` 后跟 `&mut C` / `EventWriter<E>` 必查
2. **所有 big-brain Action `Executing` 状态分支的 query miss 必须 `*state = ActionState::Failure; continue;`**（禁止 silent `continue`）。grep 模式：`ActionState::Executing => {` 后跟 `let Ok(...) else { continue }` 必查
3. **所有 Executing 等外部条件的 Action 必须有 `deadline_tick` 超时**。等 session / event / external state → 必须有 max ticks fallback Failure
4. **`commands.entity().insert(C)` 后同 tick `query.contains(C)` 不可信**（deferred 语义）。幂等触发用 `Added<C>` event reader 在独立 system 处理
5. **register / load 路径不允许 panic**。失败用 `tracing::error!` + `Default::default()` 优雅降级；运行时必须能起
6. **多 zone 资源选择必须按 zone 字段 filter**，不可 `iter().next()` 取首个

### docs/CLAUDE.md §四 红旗候选（决策门 #3 决定是否升级到项目级）

```text
- **NPC ECS query 缺 Without<Despawned>**：scorer/action system 内修改 NPC component 或
  发 event 但 query 没有 Without<Despawned> filter → 必查 plan-npc-fixups-v2 §3 #1。
  Valence 软删 1-tick 窗口内对已删 entity 写 component / 发 event 会污染持久化数据
- **Action Executing 状态 silent continue**：big-brain Action `Executing` 分支用
  `let Ok(...) else { continue }` 而非 Failure → 必查 plan-npc-fixups-v2 §3 #2。
  picker 不会中断已 Executing action，silent continue 永久死锁
```

---

## §4 测试矩阵（饱和化）

下限 **45 单测 + 1 e2e 烟雾**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `lingtian_pressure 多 zone` | multi zone picks correct / single zone unchanged / no match warns | 4 |
| `MeleeAction Executing → Failure` | query miss → Failure / 正常 unchanged / **全模块 grep 模式同样修法的 Action 各自测试** | 6 |
| `Chase/Flee Failure 停 navigator` | chase / flee / flee_cultivator 各自 navigator stop / 切 wander 不抖动 | 6 |
| `tsy_hostile JSON 优雅降级` | 缺文件 / 损坏 / 正常 / empty registry 不 spawn | 5 |
| `RetireAction 幂等` | double-tick 仅 emit 1 / 正常 emit 1 / 多 NPC 各 1 / pending marker 持久 | 5 |
| `tribulation auto_wave Despawn filter` | despawned skip / 正常 unchanged / killed in tribulation logs Combat 不是 Ascension | 4 |
| `Farming Action 超时` | TillAction / PlantAction / HarvestAction / ReplenishAction 各自 timeout + normal complete + deadline reset | 12 |
| `AscensionQuota 立即释放` | 单 despawn 立即释放 / 正常释放不变 / 多 simultaneous | 3 |
| **e2e 烟雾**：100 hydrated NPC + 4 Spirit realm 渡劫 + 战斗触发击杀 + 灵田道伥召唤 + commoner 老死繁殖 5min | 无 NPC 卡死 + quota 一致 + 道伥位置正确 + 繁殖不超配额 + auto_wave 不误升 realm | 1（重） |

**P1 验收**：`grep -hroE '#\[test\]' server/src/npc/lingtian_pressure.rs server/src/npc/brain.rs server/src/npc/tsy_hostile.rs | wc -l` ≥ 25（`-h` 抑制文件名 / `-o` 仅输出匹配 / `wc -l` 算总数，避免 `-rcE` 多文件每文件一行计数的歧义）。

**P2 验收**：`grep -hroE '#\[test\]' server/src/npc/tribulation.rs server/src/npc/farming_brain.rs | wc -l` ≥ 18。

---

## §5 开放问题 / 决策门

### #1 全局 grep `Without<Despawned>` 漏挂检查脚本是否纳入 CI

- **A**：纳入（每 PR 跑 grep 脚本，发现漏挂 → 阻塞 merge）
- **B**：仅本 plan 内手动审查 + reviewer 责任
- **C**：写成 clippy custom lint（成本高）

**默认推 A** —— grep 脚本 5 行 bash + CI 30s 接入，回归保护成本极低

### #2 Action Executing 超时 default 值

- **A**：每 Action 独立 const（farming MAX_FARMING_SESSION_TICKS=400 / chase MAX_CHASE_TICKS=200 / ...）
- **B**：统一 const ACTION_EXECUTING_DEADLINE_TICKS=600（30s @ 20 tick）
- **C**：可配置 NpcActionTimeoutConfig Resource

**默认推 A** —— 各 Action 语义不同，统一值要么过紧要么过松

### #3 docs/CLAUDE.md §四 红旗加 §3 两条

- **A**：加（强约束化）
- **B**：仅本 plan 内强约束

**默认推 A** —— 跟 qi_physics / meridian_severed 一致格调；ECS lifecycle 是项目级底盘

---

## §6 进度日志

- **2026-05-07** 骨架立项。源自 plan-npc-fixups-v1 P3 阶段 sonnet Explore 异步探查（agentId a0ae9880b26f1815d，1147s 跑完）输出 8 个 ECS lifecycle / state machine / lifecycle race / panic 类 bug + 提到 3 个未列：
  - **高 P0**：bug #1 lingtian_pressure 多 zone 选错地块 + bug #2 MeleeAction Executing query miss 卡死
  - **中 P1**：bug #3 chase/flee Failure 不停 navigator + bug #4 tsy_hostile JSON panic + bug #5 RetireAction double-send
  - **中 P2**：bug #6 auto_wave 软删 race + bug #7 Farming Action 无超时
  - **低 P2**：bug #8 AscensionQuota 软删
  - **P3 待补**：剩余 3 个未列（socialize_action / spawn_commoner patrol_target / wander_target_for zone 缺失）+ 二次探查
  - 主题：ECS query miss + Despawned 软删 race + Action Executing silent continue + 无超时 + register panic
  - **§3 强约束新立**：6 条 ECS lifecycle 卫生规则 + 决策门 #3 是否升级到 docs/CLAUDE.md §四 红旗
- 2026-05-07：派生跟 plan-npc-fixups-v1 平行的 fastlane plan，每 bug 独立 PR + 饱和回归测试

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：8 个 bug 各自 file:line 修改 + §3 强约束在所有 NPC 模块 grep 检查 + 决策门 #3 落地路径（CLAUDE.md §四 / 仅本 plan）
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话（每 bug 独立 PR）
- **测试结果**：`cargo test npc::` 数量 / e2e 烟雾 100 NPC 5min 无卡死无错位无超配额 / `grep -rcE 'Without<Despawned>' server/src/npc/` 数量上升验证
- **跨仓库核验**：server `npc::` 多模块改造 / agent 无变化 / client 无变化（仅"更稳定"）
- **遗留 / 后续**：
  - P3 sonnet Explore 二次探查若发现 > 3 bug → 派生 plan-npc-fixups-v3
  - docs/CLAUDE.md §四 红旗加两条（决策门 #3 = A 时）
  - CI grep 脚本（决策门 #1 = A 时）
