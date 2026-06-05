# plan-npc-return-home-freeze-v1：散修脱战站桩死锁修复（home 锚点 + 寻路优雅降级）

> 散修打完架"突然不动了"。根因不是战斗、不是渲染——是它的"家"被设成了大半个 zone 外的 zone 几何中心，脱战回家时 A* 永远走不到，于是原地站死。本 plan 修两层：① 家用 NPC 自己的落点；② 寻路够不到时朝目标走而不是站死。纯 server 逻辑 bugfix，不引入新玩法。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 根因修：`home_base` 取 NPC 自身持久化落点，不取远处 `patrol_target`（zone 中心） | ⬜ |
| P1 | navigator 优雅降级（A* 失败返回朝目标的 best-effort 部分路径）+ `ReturnHome` 超时兜底 | ⬜ |
| P2 | 饱和测试 + e2e（撒点边缘 NPC 脱战归位不刷 A* failed，不站桩） | ⬜ |

> 验收日期待填（全 P ✅ + Finish Evidence 后迁 `finished_plans/`）。

---

## 接入面

> 本 plan 是对既有 NPC AI 系统（`plan-npc-ai-v1` / `plan-offscreen-war-v1` / `plan-npc-virtualize-v1`）的**正确性修复**，非新玩法，不新增 component / event / schema，不自成孤岛。

### 进料

- `npc::dormant::NpcDormantSnapshot`（`server/src/npc/dormant/mod.rs`）— `position`（R2 撒点落点）/ `patrol.current_target`
- `npc::schedule::NpcHomeBase` / `home_base_for_archetype`（`server/src/npc/schedule.rs`）— 家锚点
- `npc::navigator::Navigator` + `compute_path`（`server/src/npc/navigator.rs`）— 寻路
- `npc::brain::actions_life::return_home_action_system`（`server/src/npc/brain/actions_life.rs`）— 归位 action
- `world::zone::Zone::center`（`server/src/world/zone.rs`）— zone 几何中心（当前被误用为 home）

### 出料

- 修复后 `NpcHomeBase.center()` ≈ NPC 实际落点 → `ReturnHome` 目标永远在 A* 可达范围内
- navigator 在远目标场景产出非空部分路径 → NPC 朝目标移动而非站桩
- 行为面：日志不再刷 `[bong][navigator] A* failed` 洪水；散修脱战后正常归位/休息

### 共享类型 / event

- **复用**（不新增）：`NpcHomeBase`、`Navigator`、`ReturnHomeAction` / `ReturnHomeScorer`、`HuntAction` / `TerritoryPatrolAction`（仅参考其 `*_MAX_TICKS` 超时范式）
- **新增常数**：`RETURN_HOME_MAX_TICKS`（`actions_life.rs`，对齐 `HUNT_ACTION_MAX_TICKS` / `TERRITORY_PATROL_MAX_TICKS` 的兜底范式）

### 跨仓库契约

- **server only**。NPC AI（big-brain Utility AI）、寻路、hydration 全在 server。
- agent / client **不涉及**（无 IPC schema / Redis key / CustomPayload 变更）。

### worldview 锚点

- 末法残土散修生态（`plan-offscreen-war-v1` 离屏散修 + `plan-npc-virtualize-v1` dormant/hydrate 虚拟化）。本 plan 只让既有"散修在残土游荡/归巢"的行为按设计正确运行，不新增世界观名词。

### qi_physics 锚点

- **无**。纯寻路 / 归位状态机修复，不涉及真元 / 灵气 / 衰减 / 守恒。无 `*_DECAY*` / `QiTransfer` / ledger 变更。

### 视听规格

- **无**（纯 server 逻辑 bugfix，不新增粒子 / 音效 / HUD / 动画 / narration；仅消除"站桩"异常行为）。

---

## 背景 · 根因链（调研已确认，代码锚点附后）

三个 bug 叠加，按因果顺序：

1. **家被设到大半个 zone 外。** dormant 散修播种时
   `server/src/npc/dormant/mod.rs:1283-1284`：
   ```rust
   let position = dormant_seed_scatter_position(zone, zone_local_index); // R2 撒满整个 zone XZ 包围盒
   let patrol_target = zone.center();                                    // 巡逻锚点 = zone 几何中心
   ```
   hydrate 时 `server/src/npc/hydrate/mod.rs:458`：
   ```rust
   let home_base = home_base_for_archetype(snapshot.archetype, patrol_target); // home ← 远处 zone 中心
   ```
   于是每个散修的"家" = zone 中心，但本体被均匀撒在整个 zone，撒在边缘者离家常 >1000 格。
   实测 `from≈(-1600,101,3980)` / `home=zone.center≈(-2500,128,2500)`（y=128 = `(min.y+max.y)/2` 垂直中点，非地形高度，正是误用 zone 中心的指纹）。

2. **远目标 A* 必然失败 → navigator 站死。** `MAX_PATH_ITERS = 400`（`navigator.rs:76`）只够搜几百格；1700 格目标超预算，`compute_path` 返回空路径，navigator 在 `navigator.rs:440` 故意站住（"don't blindly walk into obstacles"）。

3. **`ReturnHome` 无超时 → 永久站死。** `return_home_action_system`（`actions_life.rs:629`）`Executing` 分支只要没到 home 就每 tick 重发目标，**无 `MAX_TICKS` 兜底**（hunt=300 / territory_patrol 都有）。配合 navigator 指数退避（最多每 ~16s 重试，仍失败），NPC 永久站桩、偶尔抽动，日志刷屏 `A* failed`。

> 为什么不只调大 `MAX_PATH_ITERS`：治标不治本。zone 可达数千格宽，预算再大也有够不到的目标，且大预算会拖上千 NPC 的每 tick CPU。正确做法是 ① 把目标拉回可达范围（P0）+ ② 够不到时也优雅降级（P1）。

---

## P0 — 根因修：home 取 NPC 自身落点

**目标**：`NpcHomeBase` 反映 NPC 实际所在，而非远处巡逻锚点，使 `ReturnHome` 目标恒在 A* 可达范围。

### 交付物

- **`server/src/npc/hydrate/mod.rs`** `fn spawn_from_snapshot`（~437-466）：
  `home_base` 改由 `snapshot.position_vec()` 派生，不再用 `patrol_target`：
  ```rust
  // 改前： let home_base = home_base_for_archetype(snapshot.archetype, patrol_target);
  let home_base = home_base_for_archetype(snapshot.archetype, snapshot.position_vec());
  ```
  `patrol_target`（= `zone.center()`）保留仅供巡逻行为使用（`spawn_*_npc_at(..., patrol_target)` 不变），与"家"解耦。
- 保持 `hydrate_position_for`（`server/src/npc/schedule.rs`）签名与既有 rest-night→home / forage-dawn→poi 分支语义不变（home 现在 ≈ position，rest-night 落点与非 rest 一致，无回归）。
- **不改** `dormant_rogue_seed_snapshot` 的 `patrol_target = zone.center()`（巡逻聚集向心是 offscreen-war 既定行为，不在本 plan 范围）。

### 测试声明（`npc::hydrate::tests` / `npc::schedule::tests`）

- `home_base_uses_scatter_position_not_zone_center`：构造 zone 边缘撒点的 dormant snapshot（position 距 zone.center >800 格），hydrate 后断言 `NpcHomeBase.center()` 距 `snapshot.position` ≤ `RETURN_HOME_ARRIVAL_DISTANCE`（不再 ≈ zone 中心）。
- `home_within_astar_reach_after_hydrate`：断言 hydrate 后 `home.center()` 与实体落点 XZ 距离 ≤ A* 可达上界（`GOAL_REACH_XZ` 量级），即 `ReturnHome` 不会触发 `A* failed` 分支。
- `rest_night_hydrate_still_spawns_at_home`：rest+night 活动下 `hydrate_position_for` 仍返回 `home.center()`（home 现= 自身位置），行为不回归。

---

## P1 — navigator 优雅降级 + ReturnHome 超时兜底

**目标**：任何"目标超出寻路预算"的场景（远巡逻 / 远猎物 / 异常远 home）都朝目标移动而非站死；`ReturnHome` 不再可能无限期挂起。

### 交付物

- **`server/src/npc/navigator.rs`** `fn compute_path`（~523-585）：A* 到不了目标时，不返回空路径，而是返回**朝目标方向的 best-effort 部分路径**——取 A* 搜索中启发值最小（最接近 goal）的已展开节点的回溯路径；该节点无可用时回退到 `step_toward_with_collision` 的一格 clamped beeline 航点。使 `navigator.rs:437-444` 分支拿到非空 `target_pos`，NPC 朝目标推进到走不动为止。
  - 失败计数 / 指数退避 / `A* failed` WARN 语义保留（仍记录"未抵达完整目标"），但不再等于"站死"。
- **`server/src/npc/brain/actions_life.rs`** `fn return_home_action_system`（~629-685）：`Executing` 分支加 `rest.elapsed_ticks` / 独立计数的 `RETURN_HOME_MAX_TICKS` 超时——到点未抵达 home 则 `navigator.stop()` + `*state = ActionState::Failure`（交还 big-brain 重新评分，避免独占站死）。常数对齐 `HUNT_ACTION_MAX_TICKS=300` / `TERRITORY_PATROL_MAX_TICKS` 范式。

### 测试声明（`npc::navigator::tests` / `npc::brain::actions_life::tests`）

- `compute_path_returns_partial_path_toward_far_goal`：goal 距起点 > `MAX_PATH_ITERS` 可达范围时，`compute_path` 返回**非空**路径，且首航点在起点→goal 方向上（点积 > 0）。
- `partial_path_advances_npc_not_freeze`：连续 tick 下 NPC 位置朝 goal 单调推进（对比修前站死的 `from` 原地微动）。
- `return_home_times_out_when_home_unreachable`：home 设为不可达远点，`return_home_action_system` 在 `RETURN_HOME_MAX_TICKS` 内由 `Executing` → `Failure`，且 `navigator` 被 `stop()`。
- `return_home_succeeds_when_home_reachable`：home 在身边时仍正常 `Executing`→（到达+休息满）`Success`，不被超时误杀。
- 回归：`consecutive_path_failures` 退避、`empty_path_does_not_bypass_countdown_after_failure` 等既有用例不变红（部分路径不改写退避语义）。

---

## P2 — 饱和测试 + e2e

**目标**：端到端锁住"撒点边缘散修脱战归位"链路，防回归。

### 交付物 / 测试声明

- **e2e（`npc::hydrate` 或 `npc::dormant` 集成测试）** `dormant_edge_seed_returns_home_without_astar_flood`：
  ① 在大 zone 边缘播一个 dormant 散修 → ② hydrate → ③ 触发 hunt→脱战→`ReturnHome` → ④ 断言：home 可达、`ReturnHome` 在有限 tick 内 `Success`、整个过程不进入 `consecutive_path_failures > 2` 的 `A* failed` 洪水分支。
- **边界**：zone 极大（数千格）+ position 撒在四角；position == zone.center（退化重合）两种极端均归位正常。
- **状态机**：`ReturnHome` 三态（到达 Success / 超时 Failure / 取消 Cancelled）各一命中用例。
- 全量 `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`（`npc::*` 子树）绿。

---

## 关联

- 修复对象：`plan-offscreen-war-v1`（dormant 散修播种）/ `plan-npc-virtualize-v1`（dormant↔hydrate）/ `plan-npc-ai-v1`（big-brain 行为）。
- 不与活跃 `plan-territory-v1` / `plan-tiandao-hunt-v1` 撞车（二者为新增 ZoneInfluence / TiandaoAttention 玩法系统，全 ⬜，不碰 navigator / hydrate home / ReturnHome）。
