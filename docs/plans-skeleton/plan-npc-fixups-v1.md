# Bong · plan-npc-fixups-v1 · 骨架

NPC 系统**三个独立正确性 bug 集中修复**（非性能，纯行为正确性）：① idle NPC 重力失效永远悬空 / ② A* 路径返回空时 NPC 永远 idle（spawn position 在空中或 chunk 没生成时）/ ③ MineSkin fallback 退化女巫 entity（破坏修仙观感）。三个 bug 由 plan-npc-perf-v1 5 路探查 + 主链路源码读取确认（非派生 plan，独立 PR 走 fastlane），与 plan-npc-perf-v1（性能优化）/ plan-npc-virtualize-v1（dormant 框架）无阻塞关系。每个 bug 一个独立 PR，互不耦合，配饱和回归测试 pin 行为。**P3 阶段预留 sonnet Explore 探查输出的剩余 bug**（异步进行中，结果回来后追加 §2）。

**交叉引用**：`plan-npc-ai-v1.md` ✅（基础 NPC 系统 + spawn 路径）· `plan-spawn-tutorial-v1`（POI rogue 来源 + spawn position 决定）· `plan-npc-perf-v1` ⏳（性能优化前提是 NPC 行为正确，本 plan 应在 perf-v1 P0 之前完成）· `plan-npc-virtualize-v1` ⏳（hydrate 后 NPC 必须直接落地，本 plan 修复 #1 是 virtualize P1 hydrate 路径的隐式依赖）

**worldview 锚点**：无（基础 bug fix，不引入玩法 / 不动经济物理）。仅 #3 fallback 改 villager 跟 worldview §十一「散修江湖」的 NPC 视觉一致性有间接联系——女巫是 vanilla MC 敌对 mob，散修不该长成女巫；村民是中性 NPC + 跟散修 commoner 视觉同源更合理。

**qi_physics 锚点**：无（不动真元 / 守恒律 / 衰减常数）。

**前置依赖**：

- `plan-npc-ai-v1` ✅ → `spawn_rogue_npc_at` / `navigator_tick_system` / `fallback_rogue_commoner_kind` / `wander_action_system` / NpcLodTier 默认 Near 已实装
- 无其他依赖（纯 fix plan）

**反向被依赖**：

- `plan-npc-perf-v1` ⏳ → P0 baseline 录档前应已修 #1 #2，否则"NPC 不动"会污染性能基线
- `plan-npc-virtualize-v1` ⏳ → P1 hydrate spawn ECS entity 时若 spawn Y 在空中且 navigator 仍按 idle 跳过重力 → hydrate 后立刻悬空 + 不动；本 plan 修 #1 是 virtualize P1 hydrate 路径的隐式前提
- 任何后续 NPC spawn 玩法 plan（quest / faction wars / lingtian-npc 扩展）

---

## 接入面 Checklist

- **进料**：
  - `navigator_tick_system`（server/src/npc/navigator.rs:249-354） + `nav.current_goal` / `Position` / `Transform` / `ChunkLayer` / `snap_to_ground`
  - `spawn_rogue_npc_at`（server/src/npc/spawn.rs:657-694） + `spawn_rogue_commoner_base` + 各 archetype 的 spawn 入口（commoner / disciple / scattered_cultivator / beast）
  - `fallback_rogue_commoner_kind`（server/src/npc/spawn.rs:963-971）
  - `compute_path`（server/src/npc/navigator.rs，A* 入口）
  - `spawn_tutorial.rs:282` POI rogue 入口（spawn position 来自 POI `pos_xyz`）
- **出料**：
  - **#1**：`navigator_tick_system` idle 分支增加 `snap_to_ground` 调用（5-8 行）+ 回归单测（idle NPC 1 tick 落地）
  - **#2**：spawn entry 强制 Y snap 到 chunk heightmap（`spawn_rogue_commoner_base` 入口校验）+ `compute_path` 失败时 emit `tracing::warn!` log（含 entity / spawn_pos / goal / chunk_loaded 状态）
  - **#3**：`fallback_rogue_commoner_kind` 删除 `EntityKind::WITCH` 分支 → 全部 fallback 用 `EntityKind::VILLAGER` + 改测试 spawn.rs:1559 断言
  - **P3 待补**（Explore 异步报告回来后）：根据探查输出列剩余 bug + 修法 + 测试
- **共享类型 / event**：
  - 不新增任何 component / event / schema
  - 复用 `NpcMarker` / `Position` / `Transform` / `ChunkLayer` / `ActionState`
- **跨仓库契约**：
  - server: 纯内部 fix，无对外 schema 变化
  - agent: 无（NpcDigest / world_state 通道 unchanged）
  - client: **#3 视觉变化**（女巫 → 村民，玩家可见）；其他无变化
- **worldview 锚点**：无
- **qi_physics 锚点**：无

---

## §0 设计轴心

- [ ] **每个 bug 独立 PR**（不打包）：三个 fix 互不依赖，分别走 fastlane。打包会让回归测试 churn + reviewer 上下文切换
- [ ] **每个 bug 配饱和化回归测试**（CLAUDE.md Testing 节）：必须 pin 行为，让任何回归立刻撞红。每个 bug 至少 ① happy path（修复后行为）② boundary（修法触发条件 / 不触发条件）③ 不要 regress 旁边逻辑
- [ ] **修法保守**：最小变更 + 不顺手重构。比如 #3 只改 fallback 选择，不动 SkinPool 加载链
- [ ] **改测试时同步改断言不删测试**：`spawn.rs:1559` `rogue_commoner_visual_kind_uses_player_only_for_real_skin` 测试要 pin 新行为（fallback → villager），不能删
- [ ] **不在本 plan 引入新功能**：比如 #2 修复 spawn Y snap 时不顺手做 "智能 spawn 位置选择"。新功能走单独 plan
- [ ] **P3 留给 Explore 探查**：sonnet agent 找剩余 bug 的输出回来后，每个 bug 单独评估是否纳入本 plan / 派生新 plan / 标 reminder

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | **bug #1 重力 idle 失效**：navigator.rs:275 idle continue 之前加 snap_to_ground 调用 + 单测（idle NPC 1 tick 后 Y 落地 / 非 idle 时不 double snap） | `cargo test npc::navigator::tests::idle_npc_falls_to_ground` 全过 / WSLg 实跑 spawn 区域 zombie 1 tick 后落地 |
| **P1** ⬜ | **bug #2 A\* 空路径 + spawn Y 校验**：`spawn_rogue_commoner_base` 入口加 spawn_position.y snap 到 terrain heightmap（用 `TerrainProvider::sample` 或 ChunkLayer heightmap）+ `compute_path` 返回空时 `tracing::warn!`（带 entity / spawn / goal / 距离 / chunk loaded 状态） | `cargo test npc::spawn::tests::spawn_snaps_y_to_terrain` / WSLg POI rogue spawn 1 分钟内 wander 跑起来 |
| **P2** ⬜ | **bug #3 fallback skin → villager**：`fallback_rogue_commoner_kind` 删 WITCH 分支 → 全 villager + 改 spawn.rs:1559 测试断言 + 顺手清理 spawn.rs:5 `WitchEntityBundle` import 若不再使用 + spawn.rs:841-847 `EntityKind::WITCH` match 分支若不再触发可加 `unreachable!` 或保留作为 future override（决策门 #1） | `cargo test rogue_commoner_visual_kind_uses_player_only_for_real_skin` 全过 / WSLg POI rogue 是村民模型 |
| **P3** ⬜ | **Explore 发现的剩余 bug**（异步 sonnet agent 探查中）：每个 bug 单独评估纳入本 plan / 派生新 plan / 仅入 reminder | 异步等待 |

---

## §2 各 bug 根因 + 修法

### #2.1 重力 idle 失效（bug #1 → P0）

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

**修法**：

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

### §2.2 A\* 路径返回空 → 永远 idle（bug #2 → P1）

**file:line**：`server/src/npc/navigator.rs:288 / 327-330` + spawn position 入口（多处）

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

### §2.3 fallback skin → 女巫（bug #3 → P2）

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

---

## §3 测试矩阵（饱和化）

下限 **15 单测 + 1 e2e 烟雾**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `navigator_tick_system idle gravity` | idle 落地 / 已在地面不动 / 无 layer 不 panic / idle→active 不 stutter | 5 |
| `spawn position Y snap` | 空中 → snap / 地下 → snap / 无 layer → fallback / heightmap 边界 | 4 |
| `compute_path failure log` | 失败 emit warn / 成功不 log / 含完整字段 | 3 |
| `fallback_rogue_commoner_kind` | 真皮 PLAYER / fallback skin VILLAGER / None VILLAGER / no_witch_grep | 4 |
| **e2e 烟雾**：spawn-tutorial 起 1 rogue + spawn 玩家 + 60s 观察 | rogue 模型 = villager / rogue 1 tick 内落地 / rogue 在 60s 内至少完成 1 次 wander 移动 ≥ 4 格 | 1（重） |

**P0/P1/P2 验收**：每阶段对应回归测试全过，且不让 plan-npc-ai-v1 已有测试 regress（`cargo test npc::` 总数不下降）。

---

## §4 P3 阶段：Explore 异步探查（已派生 → plan-npc-fixups-v2）

**2026-05-07 sonnet Explore（agentId a0ae9880b26f1815d，1147s 跑完）报告**：发现 8 个 ECS lifecycle / state machine race / silent stuck / register panic 类 bug + 3 个未列待二次探查。性质独立且数量多（共 ≥ 8） → 按 v1 §0「数量多 → 派生 v2」决策 → **已派生 `plan-npc-fixups-v2`**（同目录 plan-npc-fixups-v2.md）。

v2 范围摘要：

- **高 P0**：lingtian_pressure 多 zone 选错地块 / MeleeAction Executing query miss 卡死
- **中 P1**：chase/flee Failure 不停 navigator / tsy_hostile JSON panic / RetireAction double-send
- **中 P2**：tribulation auto_wave 软删 race / Farming Action 无超时 / AscensionQuota 软删
- **P3** 待补：socialize_action / spawn_commoner patrol_target / wander_target_for zone 缺失 + 二次探查

跟本 plan（v1）关系：**v1 修 navigator/spawn 物理层 → v2 修 ECS lifecycle / state machine 层**，互不阻塞，各自独立 PR fastlane。

---

## §5 开放问题 / 决策门

### #1 spawn.rs:841-847 EntityKind::WITCH match 分支处理

- **A**：保留（match 仍含 WITCH 分支但 fallback 不再触发，留接口给未来妖修 archetype）
- **B**：删除（简化 match）

**默认推 A**

### #2 #2 修法是否需要扩展到全部 archetype 入口

- **A**：仅 `spawn_rogue_commoner_base` 入口（其他 archetype 走相同 base 函数 → 自动覆盖）
- **B**：每个 archetype spawn 入口都加重复校验（防御性编程）

**默认推 A**（base 函数已是单一入口，DRY）

### #3 是否升级 `tracing::warn!` 为 metric 计数器

- **A**：只 warn log（修复诊断已足够）
- **B**：加 `compute_path_failures_total` 计数器，用 prometheus / tracing metrics

**默认推 A**（首版 fix；如果 #2 修后 warn 仍频发 → 派生 plan-npc-perf-v1 P3 telemetry 整合）

---

## §6 进度日志

- **2026-05-07** 骨架立项。源自 plan-npc-perf-v1 5 路探查 + 主链路源码读取（navigator.rs:275 idle continue + spawn.rs:963 fallback witch + wander_action_system query 链路）。三个 bug 由用户决策**独立 PR 修不并入 perf-v1**（保持 perf-v1 性能纯度）。
- 2026-05-07：派 sonnet Explore 找剩余 NPC 正确性 bug（异步），结果回来后追加 §2.4+ / 评估是否纳入本 plan / 派生 v2 / 仅入 reminder

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：

- **落地清单**：`server/src/npc/navigator.rs:275` idle gravity 修复 + `server/src/npc/spawn.rs:821` spawn Y snap + `server/src/npc/spawn.rs:963` fallback villager + spawn.rs:1559 测试更新 + Explore 报告剩余 bug 修复
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话（每个 bug 独立 PR）
- **测试结果**：`cargo test npc::navigator npc::spawn` 数量 / e2e 烟雾 1 rogue 60s 落地 + wander
- **跨仓库核验**：server `npc::navigator::*` + `npc::spawn::*` / agent 无变化 / client #3 视觉变化（村民模型 vs 女巫）
- **遗留 / 后续**：
  - Explore 发现的剩余 bug 数量超阈值 → 派生 plan-npc-fixups-v2
  - tracing metrics 整合（决策门 #3 选 B 时） → 并入 plan-npc-perf-v1 P3 telemetry
