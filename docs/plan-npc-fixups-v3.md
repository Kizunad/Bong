# Bong · plan-npc-fixups-v3

NPC 系统**正确性 bug 集中修复合订本**（非性能，纯行为正确性）。合并 v1（navigator/spawn 物理层 3 bug）+ v2（ECS lifecycle / state machine 层 8 bug + Explore 二次探查占位），共 **11 bug + P4 二次探查**。**单 PR consume + 每 bug 独立 commit**（11 commit / 同 PR，git bisect 友好；reviewer 一次过同主题），饱和回归测试 pin 行为；按层分组排期：物理层先于 lifecycle 层（v2 P1 chase/flee 稳定测试依赖 v1 P0 重力修复）。**本 plan 是 plan-npc-perf-v1 P0 baseline 录档与 plan-npc-virtualize-v1 P1 hydrate 路径的隐式前提**——baseline 录档前必修 P0/P1（避免"NPC 不动 / 卡死 / 错位"污染性能数据）；hydrate 后 NPC 必须直接落地 + 不撞软删 race。**主题**：navigator/spawn 物理正确性 + ECS query 卫生 + Action 状态机超时 + register 优雅降级。

**交叉引用**：`plan-npc-ai-v1.md` ✅（基础 NPC 系统 + spawn 路径 + big-brain Action 状态机框架 + AscensionQuotaStore + AutoWavePacing）· `plan-spawn-tutorial-v1`（POI rogue 来源 + spawn position 决定）· `plan-npc-perf-v1.md` ⏳（perf P0 baseline 录档前应已修本 plan P0/P1，否则"NPC 不动 / 卡死"会污染性能基线）· `plan-npc-virtualize-v1.md` ⏳（hydrate 后 NPC 必须直接落地 + 不撞软删 race）· `plan-tribulation-v1.md` ✅（化虚名额 + 渡劫波次状态机，本 plan #9 #11 修其 ECS lifecycle race）· `plan-lingtian-npc-v1.md` ✅（道伥召唤来源 zone，本 plan #4 修多 zone 选错地块）

**worldview 锚点**：基本无（基础 bug fix 类，不引入玩法 / 不动经济物理）。两处间接关联：
- **#3 fallback villager**：跟 worldview §十一「散修江湖」的 NPC 视觉一致性——女巫是 vanilla MC 敌对 mob，散修不该长成女巫；村民是中性 NPC + 跟散修 commoner 视觉同源更合理
- **#9 auto_wave 软删 race**：跟 worldview §三:124-187 NPC 与玩家死亡平等 + 化虚名额是稀缺资源（默认 4）有间接关联——NPC 渡劫中战死必须按 plan-tribulation 规则失败而非成功，本 bug 让持久化层偶尔写错"化虚成功"

**qi_physics 锚点**：无（不动真元 / 守恒律 / 衰减常数）。

**前置依赖**：

- `plan-npc-ai-v1` ✅ → `spawn_rogue_npc_at` / `navigator_tick_system` / `fallback_rogue_commoner_kind` / `wander_action_system` / NpcLodTier 默认 Near + big-brain Action / AscensionQuotaStore / AutoWavePacing / NpcReproductionRequest 已实装
- 无其他依赖（纯 fix plan）

**反向被依赖**：

- `plan-npc-perf-v1.md` ⏳ → P0 baseline 录档前应已修本 plan P0（重力 + spawn Y）+ P1（lingtian + Melee），否则性能基线被污染
- `plan-npc-virtualize-v1.md` ⏳ → P1 hydrate spawn ECS entity 时若 spawn Y 在空中且 navigator 仍按 idle 跳过重力 → hydrate 后立刻悬空 + 不动；P3 dormant NPC 渡虚劫强制 hydrate 前应修 P3（auto_wave 软删 + quota race），避免 hydrate-on-tribulation 撞 quota race
- 任何后续 NPC spawn 玩法 plan（quest / faction wars / lingtian-npc 扩展）

---

## 接入面 Checklist

- **进料**：
  - **物理层**：`navigator_tick_system`（server/src/npc/navigator.rs:249-354） + `nav.current_goal` / `Position` / `Transform` / `ChunkLayer` / `snap_to_ground` / `compute_path` ; `spawn_rogue_npc_at`（server/src/npc/spawn.rs:657-694） + `spawn_rogue_commoner_base`（server/src/npc/spawn.rs:821-879） + 各 archetype spawn 入口 ; `fallback_rogue_commoner_kind`（server/src/npc/spawn.rs:963-971）; `spawn_tutorial.rs:282` POI rogue 入口
  - **lifecycle 层**：`LingtianPlot` Component（无 zone 字段，本 plan #4 要求加） + `ZonePressureCrossed` event + `ActiveLingtianSessions` Resource ; big-brain `ActionState`（Init/Requested/Executing/Success/Failure/Cancelled） + `Actor(entity)` + `BigBrainSet::Actions` schedule set ; Valence `Despawned` Component（软删 marker，1-tick 窗口内 entity 仍存在 + components 仍可 query） ; `AscensionQuotaStore` Resource（化虚名额 max=4） + `TribulationState` + `NpcTribulationPacing` + `TribulationWaveCleared` event ; `PendingRetirement` Component + `NpcRetireRequest` event + commands deferred 写入语义 ; `tsy_spawn_pools.json` / `tsy_drops.json` 数据文件 + `load_tsy_*_registry` panic 路径
  - **P4 二次探查**：`wander_target_for` 函数（zone_registry Option 缺失时无边界 clamp）; `socialize_action_system` 的 `SocializeState` query（thinker 重启后未插入旧 entity）; `spawn_commoner_npc_at` 繁殖路径（patrol_target 跟 spawn_position 重合）
- **出料**：
  - **#1**（P0）：`navigator_tick_system` idle 分支增加 `snap_to_ground` 调用（5-8 行）+ 回归单测（idle NPC 1 tick 落地）
  - **#2**（P0）：spawn entry 强制 Y snap 到 chunk heightmap（`spawn_rogue_commoner_base` 入口校验）+ `compute_path` 失败时 emit `tracing::warn!` log（含 entity / spawn_pos / goal / chunk_loaded 状态）
  - **#3**（P0）：`fallback_rogue_commoner_kind` 删除 `EntityKind::WITCH` 分支 → 全部 fallback 用 `EntityKind::VILLAGER` + 改测试 spawn.rs:1559 断言
  - **#4**（P1）：`LingtianPlot { zone: String, ... }` 字段 + `spawn_daoshen_on_pressure_high` 用 `plots.iter().find(|p| p.zone == event.zone)` 替代 `next()`
  - **#5**（P1）：`brain.rs:929-930` Executing query miss 改为 `*state = ActionState::Failure; continue;`（禁止 silent `continue`）+ 全 brain.rs / social.rs / territory.rs / relic.rs / farming_brain.rs grep 模式同样修复
  - **#6**（P2）：`chase_action_system` / `flee_action_system` / `flee_cultivator_action_system` query miss 时用独立 navigator query 强制 stop
  - **#7**（P2）：`tsy_hostile.rs:351,354` 改 `unwrap_or_else(|e| { tracing::error!(...); Default::default() })` + 后续 system 检查 pool 非空 warn
  - **#8**（P2）：`retire_action_system` 用 `Added<PendingRetirement>` 触发 NpcRetireRequest 而非在 action system 直接 send
  - **#9**（P3）：`npc_tribulation_auto_wave_tick` query 加 `Without<Despawned>` filter
  - **#10**（P3）：farming Action 加 `session_deadline_tick: u64` + Executing 超时 → Failure（TillAction / PlantAction / HarvestAction / ReplenishAction 各自）
  - **#11**（P3）：`release_quota_for_ended_tribulations` 的 `ongoing` query 加 `Without<Despawned>`
  - **新增 telemetry**（可选）：`tracing::warn!("npc action stuck in Executing for {n} ticks")` 当 Action Executing 超阈值（debug 用，可入 P4 / 派生 perf-v1 P3 telemetry）
  - **P4 待补**（Explore 二次探查回来后）：根据探查输出列剩余 bug + 修法 + 测试
- **共享类型 / event**：
  - 新增字段 `LingtianPlot.zone: String` + farming action `session_deadline_tick`，**复用** ActionState / TribulationState / Despawned / NpcMarker / Position / Transform / ChunkLayer 等已有类型
  - 不新增任何 Component / Event / Schema
- **跨仓库契约**：
  - server: 多模块改造（npc/navigator.rs / npc/spawn.rs / npc/lingtian_pressure.rs / npc/brain.rs / npc/tribulation.rs / npc/farming_brain.rs / npc/tsy_hostile.rs），无对外 schema 变化
  - agent: 无（NpcDigest / world_state 通道 unchanged）
  - client: **#3 视觉变化**（女巫 → 村民，玩家可见）；其他 lifecycle 类 bug 行为可见结果 = 卡死 / 错位 / 误升 realm 不再发生，玩家观感"更稳定"但不是新视觉
- **worldview 锚点**：仅 #3 + #9 间接关联（详见头部）
- **qi_physics 锚点**：无

---

## §0 设计轴心

- [ ] **单 PR consume + 每 bug 独立 commit**：v3 升 active 后的消费决策——整 plan 一次 consume 一个 PR，但 11 bug 拆 11 commit（git bisect 友好 / 单 commit 单回滚单 review chunk）。**commit 顺序按 P 段编号 + bug 序号**（P0 #1 → P0 #2 → P0 #3 → P1 #4 → P1 #5 → ... → P3 #11），每 commit 含该 bug 的修法 + 回归测试 + Cargo.toml 改动（如有）。原 v1/v2 「每 bug 独立 PR fastlane」轴心保留为历史决策（§6 日志），实际不兑现
- [ ] **每个 bug 配饱和化回归测试**（CLAUDE.md Testing 节）：必须 pin 行为，让任何回归立刻撞红。每个 bug 至少 ① happy path（修复后行为）② boundary（修法触发 / 不触发条件）③ 不要 regress 旁边逻辑。**重点：despawn race 类 bug 必须用模拟 Despawned 插入的测试场景**（不能仅靠"正常情况下不会 despawn"逃避）
- [ ] **修法保守**：最小变更 + 不顺手重构。比如 #3 只改 fallback 选择，不动 SkinPool 加载链；#5 只改 query miss 分支，不重构 Action state 机
- [ ] **改测试时同步改断言不删测试**：`spawn.rs:1559` `rogue_commoner_visual_kind_uses_player_only_for_real_skin` 测试要 pin 新行为（fallback → villager），不能删
- [ ] **不在本 plan 引入新功能**：比如 #2 修 spawn Y snap 时不顺手做"智能 spawn 位置选择"；#10 加 farming 超时不顺手做"farming 优先级调度"。新功能走单独 plan
- [ ] **状态机 silent `continue` 必须改 Failure**（§3 强约束 #2）：big-brain Action `Executing` 状态分支若 query miss 用 `continue` → 下次 tick 仍 Executing → silent 死锁。**强制规则**：所有 Action `Executing` 内 `let Ok(...) else { continue }` 必须改 `let Ok(...) else { *state = ActionState::Failure; continue }`，让 picker 能切别的 action
- [ ] **Despawned filter 是 ECS query 卫生**（§3 强约束 #1）：任何 query 涉及 NPC entity 的 Component 修改 / event 发送 → 必须加 `Without<Despawned>`（除非显式处理软删窗口）。违反 = 隐式 race
- [ ] **Action Executing 必须有超时**（§3 强约束 #3）：所有 Executing 等外部条件（session 完成 / event 接收 / state 转换）的 Action 必须有 `deadline_tick` / `tick_count`，超时 → Failure。否则一个 stuck = 永久占 NPC slot
- [ ] **panic 是部署灾难**（§3 强约束 #5）：register / load 路径的 panic 在 CI / 生产 = 服务无法启动。改 `tracing::error!` + `Default::default()` 让用户至少能跑（功能降级 != 服务崩溃）
- [ ] **deferred commands 跟同 tick 多次调用不安全**（§3 强约束 #4）：`commands.entity().insert(C)` 是 deferred，同一 tick 内 `query.contains(C)` 仍返回 false。**幂等触发用 `Added<C>` event reader 而非 action system 内 send**
- [ ] **P4 留给 Explore 二次探查**：sonnet agent 找剩余 bug 的输出回来后，每个 bug 单独评估纳入本 plan / 派生新 plan / 仅入 reminder

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **物理层 3 bug（v1 三独立 fastlane）**：① navigator.rs:275 idle 重力补 snap_to_ground / ② spawn_rogue_commoner_base 入口 Y snap + compute_path 失败 warn / ③ fallback_rogue_commoner_kind WITCH → VILLAGER + spawn.rs:1559 测试改断言 | `cargo test npc::navigator::tests::idle_npc_falls_to_ground` + `cargo test npc::spawn::tests::spawn_snaps_y_to_terrain` + `cargo test rogue_commoner_visual_kind_uses_player_only_for_real_skin` 全过 / WSLg 实测 spawn 区域 zombie 1 tick 内落地 + POI rogue spawn 1 分钟内 wander 跑起来 + rogue 是村民模型 |
| **P1** ⬜ | **lifecycle 高优先级 2 bug（玩家可见）**：④ LingtianPlot.zone 字段 + spawn_daoshen_on_pressure_high filter / ⑤ brain.rs:929 MeleeAction Executing query miss → Failure + 全模块 grep 模式同样修复 | `cargo test npc::lingtian_pressure::tests::multi_zone_picks_correct_plot` + `cargo test npc::brain::tests::melee_executing_query_miss_transitions_to_failure` 全过 / WSLg 实测多 zone 道伥召唤位置正确 + beast 战斗中击杀对手不再"冻僵" |
| **P2** ⬜ | **lifecycle 中 3 bug**：⑥ chase/flee_action_system Failure 时强制 stop navigator / ⑦ tsy_hostile.rs panic → error + Default fallback / ⑧ retire_action_system 用 Added<PendingRetirement> 触发事件 | `cargo test npc::brain::tests::flee_failure_stops_navigator` + `cargo test npc::tsy_hostile::tests::missing_json_warns_uses_default` + `cargo test npc::brain::tests::retire_action_no_double_send_on_double_tick` 全过 |
| **P3** ⬜ | **lifecycle 中-低 3 bug（lifecycle race + 超时缺失）**：⑨ tribulation auto_wave + ⑪ AscensionQuota query 加 Without<Despawned> / ⑩ farming Action 加 session_deadline_tick 超时 | `cargo test npc::tribulation::tests::auto_wave_skips_despawned_npc` + `cargo test npc::farming_brain::tests::till_action_times_out_when_session_stuck` + `cargo test npc::tribulation::tests::quota_releases_immediately_on_despawn` 全过 |
| **P4** ⬜ | **Explore 二次探查**：sonnet 已提到的 3 个未列（socialize_action_system 旧 entity Failure / spawn_commoner_npc_at patrol_target == spawn_position / wander_target_for zone 缺失越区）+ 派 agent 二次探查找剩余 | 二次探查输出 → 评估纳入本 plan / 派生 plan-npc-fixups-v4 / 仅入 reminder |

---

## §2 各 bug 根因 + 修法

> P 段顺序与 §1 一致：P0 物理层 (#1–#3) → P1 lifecycle 高 (#4–#5) → P2 lifecycle 中 (#6–#8) → P3 lifecycle 中-低 (#9–#11) → P4 二次探查 (#12+)。

### §2.1 bug #1 — navigator idle 重力失效（P0）

**file:line**：`server/src/npc/navigator.rs:275`

**根因**：

```rust
// navigator_tick_system 主循环
let Some(goal) = nav.current_goal else {
    continue; // idle  ← 这里直接 continue，snap_to_ground 不跑
};
```

`snap_to_ground`（navigator.rs:769）注释写明「Applies 'gravity'」，但只在路径推进路径上调用（line 714 + 764）。idle NPC（无 goal 时）整个函数被跳过 → 永远悬空在 spawn Y。

**症状**：spawn-tutorial 的 zombie / rogue / commoner / 任何无活跃 goal 的 NPC 永远悬空在 spawn 写入的 Position.y 上，不受重力影响。即使下方是空气也不掉。

**修法**（`layer` / `position` / `transform` 来自 `navigator_tick_system` 主 Query，见 navigator.rs:249-263：`Query<(&mut Position, &mut Transform, &mut Look, &mut HeadYaw, &mut Navigator, Option<&MovementController>), With<NpcMarker>>` + `layers: Query<&ChunkLayer, With<OverworldLayer>>`）：

```rust
let Some(goal) = nav.current_goal else {
    // idle NPC 也要被重力影响（worldview NPC 与玩家平等的物理化身）
    if let Some(layer_ref) = layer {
        let current = position.get();
        let snapped = snap_to_ground(current, Some(layer_ref));
        if (snapped.y - current.y).abs() > 1e-4 {
            position.set(snapped);
            transform.translation.y = snapped.y as f32;
        }
    }
    continue;
};
```

**回归测试**（饱和化，必须）：

- `idle_npc_in_air_falls_to_ground_in_one_tick`：spawn 在 Y=80 + 地面 Y=66 + 1 tick navigator → NPC.y == 66
- `idle_npc_already_on_ground_does_not_move`：spawn 在 Y=66 地面 + 1 tick → NPC.y 不变（不 double snap）
- `idle_npc_no_chunk_layer_does_not_panic`：layer = None → 不 panic 也不修改 Y
- `non_idle_npc_path_advance_unchanged`：有 goal + 路径推进 → Y snap 走原路径（不跟 idle 分支冲突）
- `idle_to_active_transition_no_jitter`：1 tick idle 落地 → 2 tick set_goal → 3 tick path 推进 → 不 stutter

### §2.2 bug #2 — A* 路径返回空 + spawn Y 校验（P0）

**file:line**：`server/src/npc/navigator.rs:288 / 327-330` + spawn position 入口 `server/src/npc/spawn.rs:821`

**根因**：

```rust
// navigator.rs:287-294
if nav.repath_countdown == 0 || destination_moved || nav.path_index >= nav.path.len() {
    let new_path = compute_path(current_pos, goal.destination, &nav, terrain, layer);
    nav.path = new_path;  // ← 可能是空 Vec
    ...
}

// navigator.rs:324-331
let target_pos = if let Some(waypoint) = nav.path.get(nav.path_index).copied() {
    waypoint
} else {
    nav.repath_countdown = 0;  // ← 下一 tick 重算（仍然空）
    continue;  // ← 永远 idle 但不挪
};
```

`compute_path` 在以下情况返回空：
1. spawn position **在空中**（POI `pos_xyz` 的 Y 跟 worldgen heightmap 不对齐 → A* 起点无 walkable block）
2. **chunk 没 load**（玩家未走近 → ChunkLayer 没生成 → A* 任何 block 查询返回 None）
3. **goal 与 spawn 距离超过 MAX_PATH_ITERS = 400 节点上限**

**症状**：rogue spawn 在 POI 后即使 wander 触发 set_goal，A* 永远找不到路径 → NPC 永远 idle 不动。

**修法（两步）**：

1. **spawn 时强制 Y snap 到 terrain heightmap**：在 `spawn_rogue_commoner_base`（spawn.rs:821）入口校验 `spawn_position.y`，如不在 ground level → snap 到 `TerrainProvider::sample_height(x, z)` 或 ChunkLayer 顶层 solid block + 1
2. **compute_path 失败时 emit log**：

```rust
let new_path = compute_path(current_pos, goal.destination, &nav, terrain, layer);
if new_path.is_empty() {
    tracing::warn!(
        "[bong][navigator] A* failed: entity={:?} from={:?} to={:?} dist={:.1} chunk_loaded={} terrain_provider={}",
        entity, current_pos, goal.destination,
        current_pos.distance(goal.destination),
        layer.is_some(),
        terrain.is_some(),
    );
}
nav.path = new_path;
```

**回归测试**（饱和化）：

- `spawn_position_above_ground_snaps_to_terrain`：spawn Y=200 + heightmap=66 → 写入的 Position.y == 66
- `spawn_position_below_terrain_snaps_to_terrain`：spawn Y=10 + heightmap=66 → 写入的 Position.y == 66
- `spawn_position_in_unloaded_chunk_uses_fallback`：chunk 未 load + heightmap fallback → Y == FALLBACK_SURFACE_Y
- `compute_path_failure_emits_warn`：使用 `tracing-test` crate / log capture 验证 warn 触发
- `compute_path_success_no_log`：正常路径不触发 warn

### §2.3 bug #3 — fallback skin 退化女巫 → villager（P0）

**file:line**：`server/src/npc/spawn.rs:963-971`

**根因**：

```rust
pub fn fallback_rogue_commoner_kind(skin: &Option<SignedSkin>) -> EntityKind {
    if skin.as_ref().is_some_and(|skin| !skin.is_fallback()) {
        EntityKind::PLAYER
    } else if skin.as_ref().is_some_and(SignedSkin::is_fallback) {
        EntityKind::WITCH  // ← MineSkin fallback skin → 女巫，破坏修仙观感
    } else {
        EntityKind::VILLAGER
    }
}
```

`MINESKIN_API_KEY` 未配置 → SkinPool 永远返回 fallback skin（不是 None）→ 100% rogue 退化为女巫。

**症状**：所有 rogue NPC（含 spawn-tutorial 的 1 rogue）都是女巫模型，跟散修身份完全不符。

**修法**：

```rust
pub fn fallback_rogue_commoner_kind(skin: &Option<SignedSkin>) -> EntityKind {
    if skin.as_ref().is_some_and(|skin| !skin.is_fallback()) {
        EntityKind::PLAYER
    } else {
        EntityKind::VILLAGER  // fallback skin / None 都用村民
    }
}
```

**回归测试**（同步改 spawn.rs:1559）：

- `rogue_with_real_skin_uses_player`：skin = Some(real) → PLAYER
- `rogue_with_fallback_skin_uses_villager`：skin = Some(fallback) → VILLAGER（**改自原 WITCH 断言**）
- `rogue_with_no_skin_uses_villager`：skin = None → VILLAGER
- `rogue_visual_kind_no_witch_in_any_path`：grep `EntityKind::WITCH` in spawn.rs ≤ 1（仅 match 分支保留 / 或归零删除）

**决策门 #1**：是否保留 spawn.rs:841-847 `EntityKind::WITCH` match 分支作为未来手动 override？
- A：保留 + match 不再被 fallback 触发，但留接口给未来"妖修女巫 archetype"
- B：删除 + 简化 match 到 PLAYER / VILLAGER 二选一
- 默认推 **A**（保留作未来扩展，反正 dead branch 不影响 binary 大小）

### §2.4 bug #4 — lingtian_pressure 多 zone 时道伥召唤打错地块（P1）

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

### §2.5 bug #5 — MeleeAttackAction Executing query miss 永久卡死（P1）

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
- **同样修法应用于全 Action Executing**：grep `ActionState::Executing => { let Ok(...) else { continue }` 模式遍历 brain.rs / social.rs / territory.rs / relic.rs / farming_brain.rs，每个加 Failure 转换 + 配测试（这是个**模式 bug**，可能不止 brain.rs:929 一处；P1 要做全局 grep + 修复）

### §2.6 bug #6 — Chase/Flee Failure 不停 Navigator（P2）

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

### §2.7 bug #7 — tsy_hostile JSON 加载失败 → 服务器 panic（P2）

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

### §2.8 bug #8 — RetireAction NpcRetireRequest double-send 配额穿透（P2）

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

### §2.9 bug #9 — npc_tribulation_auto_wave 软删 NPC 误升 realm（P3）

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

### §2.10 bug #10 — Farming Action Executing 无超时永久卡死（P3）

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

### §2.11 bug #11 — AscensionQuotaStore 软删 NPC 1-tick quota 占用泄漏（P3）

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

### §2.12 P4 占位：Explore 二次探查（P4）

sonnet Explore 已提到但未详细列出的 3 个：

1. `socialize_action_system` 的 `SocializeState` query 在 `disciple_npc_thinker` 实际启用后会对未插入该 component 的旧 entity 报 Failure（npc/social.rs，需要 P4 详查）
2. `spawn_commoner_npc_at` 繁殖时 `patrol_target == spawn_position`（无 patrol 范围偏移）导致新生儿 home 位置与死亡点重合 → 看似不动（npc/spawn.rs，需要 P4 详查）
3. `wander_target_for` 在 `zone_registry` 缺失时生成无边界约束的目标坐标（可能越区）（npc/brain.rs:1342-1371，需要 P4 详查）

**P4 阶段动作**：派 sonnet Explore 二次探查这 3 个 bug 的精确修法 + 找剩余未发现的 bug。结果回来后：
- 如发现 ≤ 3 个新 bug → 追加 §2.13+ 到本 plan
- 如发现更多 / 性质独立 → 派生 plan-npc-fixups-v4
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
  发 event 但 query 没有 Without<Despawned> filter → 必查 plan-npc-fixups-v3 §3 #1。
  Valence 软删 1-tick 窗口内对已删 entity 写 component / 发 event 会污染持久化数据
- **Action Executing 状态 silent continue**：big-brain Action `Executing` 分支用
  `let Ok(...) else { continue }` 而非 Failure → 必查 plan-npc-fixups-v3 §3 #2。
  picker 不会中断已 Executing action，silent continue 永久死锁
```

---

## §4 测试矩阵（饱和化）

下限 **≥60 单测 + 2 e2e 烟雾**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `navigator_tick_system idle gravity`（#1） | idle 落地 / 已在地面不动 / 无 layer 不 panic / idle→active 不 stutter | 5 |
| `spawn position Y snap`（#2） | 空中 → snap / 地下 → snap / 无 layer → fallback / heightmap 边界 | 4 |
| `compute_path failure log`（#2） | 失败 emit warn / 成功不 log / 含完整字段 | 3 |
| `fallback_rogue_commoner_kind`（#3） | 真皮 PLAYER / fallback skin VILLAGER / None VILLAGER / no_witch_grep | 4 |
| `lingtian_pressure 多 zone`（#4） | multi zone picks correct / single zone unchanged / no match warns | 4 |
| `MeleeAction Executing → Failure`（#5） | query miss → Failure / 正常 unchanged / **全模块 grep 模式同样修法的 Action 各自测试** | 6 |
| `Chase/Flee Failure 停 navigator`（#6） | chase / flee / flee_cultivator 各自 navigator stop / 切 wander 不抖动 | 6 |
| `tsy_hostile JSON 优雅降级`（#7） | 缺文件 / 损坏 / 正常 / empty registry 不 spawn | 5 |
| `RetireAction 幂等`（#8） | double-tick 仅 emit 1 / 正常 emit 1 / 多 NPC 各 1 / pending marker 持久 | 5 |
| `tribulation auto_wave Despawn filter`（#9） | despawned skip / 正常 unchanged / killed in tribulation logs Combat 不是 Ascension | 4 |
| `Farming Action 超时`（#10） | TillAction / PlantAction / HarvestAction / ReplenishAction 各自 timeout + normal complete + deadline reset | 12 |
| `AscensionQuota 立即释放`（#11） | 单 despawn 立即释放 / 正常释放不变 / 多 simultaneous | 3 |
| **e2e 烟雾 #1（物理层）**：spawn-tutorial 起 1 rogue + spawn 玩家 + 60s 观察 | rogue 模型 = villager / rogue 1 tick 内落地 / rogue 在 60s 内至少完成 1 次 wander 移动 ≥ 4 格 | 1（重） |
| **e2e 烟雾 #2（lifecycle）**：100 hydrated NPC + 4 Spirit realm 渡劫 + 战斗触发击杀 + 灵田道伥召唤 + commoner 老死繁殖 5min | 无 NPC 卡死 + quota 一致 + 道伥位置正确 + 繁殖不超配额 + auto_wave 不误升 realm | 1（重） |

**P0 验收**：`cargo test npc::navigator npc::spawn` 物理层 3 bug 全过；`grep -hroE '#\[test\]' server/src/npc/navigator.rs server/src/npc/spawn.rs | wc -l` ≥ 16 单测下限。

**P1 验收**：`cargo test npc::lingtian_pressure npc::brain::tests::melee_executing` 全过；`grep -hroE '#\[test\]' server/src/npc/lingtian_pressure.rs | wc -l` ≥ 3，brain.rs Executing 模式补丁全模块覆盖（grep `*state = ActionState::Failure` 至少出现于 brain.rs / social.rs / territory.rs / relic.rs / farming_brain.rs 五处中 ≥ 1，剩余无 silent continue 模式残留）。

**P2 验收**：`cargo test npc::brain::tests::flee npc::tsy_hostile npc::brain::tests::retire` 全过；`grep -hroE '#\[test\]' server/src/npc/brain.rs server/src/npc/tsy_hostile.rs | wc -l` ≥ 25。

**P3 验收**：`cargo test npc::tribulation npc::farming_brain` 全过；`grep -hroE '#\[test\]' server/src/npc/tribulation.rs server/src/npc/farming_brain.rs | wc -l` ≥ 18；`grep -rcE 'Without<Despawned>' server/src/npc/` 数量上升验证（baseline 当前 ~10 处 → 修复后应 +2 至少）。

**全量验收**：`cargo test npc::` 总数不下降；不让 plan-npc-ai-v1 / plan-tribulation-v1 / plan-lingtian-npc-v1 已有测试 regress。

---

## §5 开放问题 / 决策门

### #1 spawn.rs:841-847 EntityKind::WITCH match 分支处理（来自 v1）

- **A**：保留（match 仍含 WITCH 分支但 fallback 不再触发，留接口给未来妖修 archetype）
- **B**：删除（简化 match）

**默认推 A**

### #2 #2 修法是否需要扩展到全部 archetype 入口（来自 v1）

- **A**：仅 `spawn_rogue_commoner_base` 入口（其他 archetype 走相同 base 函数 → 自动覆盖）
- **B**：每个 archetype spawn 入口都加重复校验（防御性编程）

**默认推 A**（base 函数已是单一入口，DRY）

### #3 docs/CLAUDE.md §四 红旗加 §3 两条（来自 v2）

- **A**：加（强约束化，跟 qi_physics / meridian_severed 一致格调）
- **B**：仅本 plan 内强约束

**默认推 A** —— ECS lifecycle 是项目级底盘

### #4 全局 grep `Without<Despawned>` 漏挂检查脚本是否纳入 CI（来自 v2）

- **A**：纳入（每 PR 跑 grep 脚本，发现漏挂 → 阻塞 merge）
- **B**：仅本 plan 内手动审查 + reviewer 责任
- **C**：写成 clippy custom lint（成本高）

**默认推 A** —— grep 脚本 5 行 bash + CI 30s 接入，回归保护成本极低

### #5 Action Executing 超时 default 值（来自 v2）

- **A**：每 Action 独立 const（farming MAX_FARMING_SESSION_TICKS=400 / chase MAX_CHASE_TICKS=200 / ...）
- **B**：统一 const ACTION_EXECUTING_DEADLINE_TICKS=600（30s @ 20 tick）
- **C**：可配置 NpcActionTimeoutConfig Resource

**默认推 A** —— 各 Action 语义不同，统一值要么过紧要么过松

### #6 是否升级 `tracing::warn!` 为 metric 计数器（来自 v1）

- **A**：只 warn log（修复诊断已足够）
- **B**：加 `compute_path_failures_total` 计数器，用 prometheus / tracing metrics

**默认推 A**（首版 fix；如果 #2 修后 warn 仍频发 → 派生 plan-npc-perf-v1 P3 telemetry 整合）

---

## §6 进度日志

- **2026-05-07** v1 骨架立项。源自 plan-npc-perf-v1 5 路探查 + 主链路源码读取（navigator.rs:275 idle continue + spawn.rs:963 fallback witch + wander_action_system query 链路）。三个物理层 bug 由用户决策**独立 PR 修不并入 perf-v1**（保持 perf-v1 性能纯度）
- **2026-05-07** v1 P3 派 sonnet Explore 找剩余 NPC 正确性 bug（agentId a0ae9880b26f1815d，1147s 跑完）→ 输出 8 个 ECS lifecycle / state machine race / silent stuck / register panic 类 bug + 提到 3 个未列待二次探查 → 派生 v2 骨架
- **2026-05-07** v2 骨架立项。8 个 lifecycle 层 bug（lingtian 多 zone / Melee Executing / chase-flee navigator stop / tsy panic / Retire 幂等 / auto_wave Despawned / farming 超时 / quota 立即释放）+ §3 ECS lifecycle 强约束 6 条 + 决策门 #3 是否升 docs/CLAUDE.md §四 红旗
- **2026-05-07** v3 合并立项。**v1 + v2 合订本**（11 bug + P4 二次探查），按层分组重排（P0 物理层 → P1-P3 lifecycle 按优先级 → P4 Explore），决策门去重整合为 6 条；旧 v1/v2 骨架删除（git rm）
- **2026-05-07** v3 升 active（commit `c9861c6b1` `git mv` 入 docs/）；消费策略决策：**单 PR consume + 11 commit 拆开**（A 路线务实落地）——反 v1/v2 「每 bug 独立 PR fastlane」轴心，理由：consume-plan 框架是单 worktree 单 PR，拆 11 份 micro-plan 维护成本过高；commit 拆开足以覆盖 git bisect / 单 commit 回滚需求；reviewer 一次过同主题（NPC 正确性）上下文连续，比拆 11 PR 切换成本低

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：
  - **P0**（物理层）：`server/src/npc/navigator.rs:275` idle gravity 修复 + `server/src/npc/spawn.rs:821` spawn Y snap + compute_path warn + `server/src/npc/spawn.rs:963` fallback villager + spawn.rs:1559 测试更新
  - **P1**（lifecycle 高）：`server/src/npc/lingtian_pressure.rs` LingtianPlot.zone 字段 + spawn_daoshen filter / `server/src/npc/brain.rs:929` Melee Executing → Failure + 全模块 grep 修复
  - **P2**（lifecycle 中）：`server/src/npc/brain.rs` chase/flee Failure stop navigator + `server/src/npc/tsy_hostile.rs:351,354` panic → error fallback + `server/src/npc/brain.rs:593` Retire Added<C> 幂等
  - **P3**（lifecycle 中-低）：`server/src/npc/tribulation.rs:95-111` auto_wave Without<Despawned> + `server/src/npc/tribulation.rs:116-133` quota Without<Despawned> + `server/src/npc/farming_brain.rs:434-440` farming session_deadline_tick 超时
  - **P4**：Explore 二次探查 3 个未列 bug 的修复（如纳入本 plan）
- **关键 commit**：P0/P1/P2/P3/P4 各自 hash + 日期 + 一句话（每 bug 独立 PR）
- **测试结果**：`cargo test npc::` 数量（baseline 265 → 修复后应 +60 至少 = ≥325）/ e2e 烟雾 #1（物理层 1 rogue 60s 落地 + wander）+ #2（100 NPC 5min 无卡死无错位无超配额）
- **跨仓库核验**：server `npc::navigator::*` + `npc::spawn::*` + `npc::brain::*` + `npc::tribulation::*` + `npc::farming_brain::*` + `npc::tsy_hostile::*` + `npc::lingtian_pressure::*` 全模块改造 / agent 无变化（NpcDigest / world_state 通道 unchanged）/ client #3 视觉变化（村民模型 vs 女巫）+ 其他"更稳定"
- **遗留 / 后续**：
  - P4 Explore 二次探查若发现 > 3 bug → 派生 plan-npc-fixups-v4
  - docs/CLAUDE.md §四 红旗加两条（决策门 #3 = A 时）
  - CI grep 脚本（决策门 #4 = A 时）
  - tracing metrics 整合（决策门 #6 = B 时） → 并入 plan-npc-perf-v1 P3 telemetry
