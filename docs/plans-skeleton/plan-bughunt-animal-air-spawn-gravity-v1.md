# BugHunt: ambient 动物继承玩家 Y 导致空中生成 v1

> **一句话主题**：修复 `ambient_scheduler` 在玩家周围采样凡兽/威胁兽时把玩家当前 Y 原样当作实体脚点、且未查询 runtime surface 的生产断链；让 ambient mundane + threat 在进入 pool 前共用一次地表解析，地表不可用时跳过候选，禁止再 fail-open 到空中 Y。

**状态**：Skeleton，2026-07-22 起草；6 路 Sonnet 调查 + 2 路 Sonnet 无上下文审查已确认根因和最小边界。本 skeleton 由 BugFix 工作流以 **1 skeleton = 1 branch = 1 PR** 消费。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | ambient 共用地表门禁：mundane + threat 生成前统一解析 `surface_y + 1` | ⬜ |
| P1 | 饱和回归：纯函数、真实 scheduler→pool 生产链、预算与错误分支 | ⬜ |
| P2 | 确定性 bot 场景 + 非目标隔离 + 完整 server 门禁 | ⬜ |

---

## Bug 摘要

`ambient_scheduler_system::<M>` 以玩家为 anchor，在 24–64 格水平环带采样 X/Z，却把候选点 Y 直接设为 `anchor.y`。调度核随后把同一个 `spawn_pos` 原样传给 mundane/threat pool；pool 再把该坐标原样交给凡兽、鼠或妖兽 spawn 函数。整个主生产链没有调用已经存在的 `snap_spawn_y_to_surface`，也没有直接查询 `TerrainProvider::query_surface`。

因此，玩家站在高台、悬崖、跳跃点或测试高空位置时，牛、猪、羊、鸡、兔、山羊、蛙、狐、狼，以及 ambient 鼠/地面型妖兽会以玩家 Y 在远处空气中出生。

### 症状分级（避免夸大）

- **确定主症状**：spawn 首帧 Position.y 错误，玩家可见动物先在空中出现。
- **常见自愈**：ambient 凡兽/鼠/妖兽带 `Navigator` 且初始为 `NpcLodTier::Dormant`；Overworld terrain/layer 可用时，已有 idle/Dormant snap 通常会在后续 tick 把它拉到地面，所以可能表现为“空中闪现后瞬移落地”。
- **持续悬空子集**：地表解析不可用/不可走、active goal path 为空、movement override、`Immobilized` 等已有 snap 失败或跳过条件下，错误 Y 可持续。
- **架构背景**：Valence/Bong 没有为 NPC 执行 vanilla 式 `Velocity.y` 重力积分。Bong 的 NPC “重力”是 `Navigator` 离散贴地，不是客户端或服务端连续自由落体。P0 必须从源头修 spawn，不能把已有 Navigator 自愈当正常生成契约。

---

## 第一性原理证据链

```text
玩家 Position.y
  → sample_ambient_ring_position(anchor)
      x/z = 24..64 格环带
      y   = anchor.y
  → ambient_scheduler_system::pool_fn(..., spawn_pos, spawn_pos, ...)
  → mundane_pool_fn / ambient_threat_pool_fn
  → spawn_mundane_fauna_at / spawn_rat_npc_at / spawn_beast_npc_at
  → Position.y / Transform.y 原样写入
```

### 代码锚点

- `server/src/npc/spawn/ambient_scheduler.rs:446-476`：`sample_ambient_ring_position` 构造 `DVec3::new(cx, anchor.y, cz)`。
- `server/src/npc/spawn/ambient_scheduler.rs:614-635`：scheduler system 当前没有 `TerrainProviders` 资源参数。
- `server/src/npc/spawn/ambient_scheduler.rs:778-790`：调度器把采样结果原样传给 `pool_fn` 的 spawn/patrol 两个位置参数。
- `server/src/fauna/mundane.rs:348-367`：`mundane_pool_fn` 原样调用 `spawn_mundane_fauna_at`。
- `server/src/npc/spawn/ambient_scheduler.rs:334-360`：`ambient_threat_pool_fn` 原样调用鼠/自然妖兽 spawn。
- `server/src/npc/spawn/mundane.rs:69-81,160-165`：凡兽 spawn 注释明确由 caller 负责 snap，函数自身原样写 `Position` / `Transform`。
- `server/src/npc/spawn/common.rs:219-229`：现有 `snap_spawn_y_to_surface` 以 `floor(x/z)` 查询；`passable` 时返回 `surface_y + 1`，否则 fail-open 返回原位置。该 fail-open helper 不适合 ambient 的“候选可丢弃”语义。
- `server/src/world/terrain/mod.rs:63-100`：`SurfaceInfo.y` 是顶层实心块 Y；`passable` 表示 NPC 可站立（非深水/岩浆）。
- `server/src/network/command_executor.rs:639-647`：agent-issued NPC spawn 已有“最终 X/Z 后 snap 到真实地表，避免空中/地下生成”的正确先例。
- `server/src/npc/navigator.rs:286-385,459-465,496-507,1047-1117`：现有 idle/Dormant snap 与 heightmap fallback；说明后续自愈存在，也说明 path-empty/失败时不是连续物理重力。
- `server/src/npc/sync.rs:5-20`：Position→Transform 单向同步，Transform 不是额外物理来源。

### worldgen / 坐标契约裁决

多路调查已排除 sea-level、错轴和 raster 单位问题：Python height、spans ceiling、Rust `query_surface.y` 均是绝对 world block Y；实体脚点的现有 NPC 口径是 `surface_y + 1`。ambient 的问题不是高度算错，而是**根本没有接入高度查询**。

---

## 可达复现

1. 在 Overworld 的 ambient zone 内放置一个玩家。
2. 令玩家处于明显高于周围自然地表的位置，例如玩家 Y=200、环带地表 Y≈66–72。
3. 让 ambient mundane 或 threat 预算放行一次。
4. 当前实现会在玩家 24–64 格水平环带生成 `Position.y≈200` 的地面型动物，而正确脚点应为该候选 X/Z 的 `query_surface(...).y + 1`。
5. 下一 Navigator tick 可能突然纠正；若 ground snap 失败/跳过，则继续悬空。

测试实现不得依赖随机等待；必须用固定 seed、fake `SurfaceProvider` 和可控 scheduler/pool 测试接缝构造 witness。

---

## 已有 plan / skeleton 去重

本 skeleton 只补 ambient 生产边界，不重复下列工作：

- `docs/finished_plans/plan-ambient-threat-v1.md`：已实现通用 ambient scheduler，但未定义 surface-aware Y。
- `docs/finished_plans/plan-mundane-fauna-v1.md`：文档 P0 要求“位置过 `snap_spawn_y_to_surface`”，实现却把责任交给 caller，而 ambient caller 未兑现；这是本 bug 的直接契约漂移。
- `docs/finished_plans/plan-npc-fixups-v3.md`：已修 idle/Dormant 地面 snap；本 plan明确**不重做 Navigator idle gravity**。
- `docs/plans-skeleton/plan-bughunt-spawn-safe-y-surface-drift-v1.md`：处理玩家出生 `safe_y`，不是 fauna。该 skeleton 的 player `+1/+2` 未决不控制本 plan；动物沿用已经落地的 NPC `snap_spawn_y_to_surface` / `ground_y + 1` 口径。
- `docs/plans-skeleton/plan-bughunt-spawn-tutorial-poi-y-drift-v1.md`：处理教学 POI，不是 ambient fauna。
- `docs/reminder.md`：未发现本问题的现有承接项。

历史判断：`31bd564e4`（2026-07-04）引入 ambient 环带 `anchor.y`；`33c2509c7`（2026-07-05）凡兽复用该路径。它是初版即存在、且 mundane plan 未兑现的实现缺口，不是曾经正确后又回归。

---

## 接入面 Checklist

- **进料**：
  - `ambient_scheduler_system::<M>`、`sample_ambient_ring_position`、`AmbientSchedulerConfig<M>`、`AmbientMarkerData`。
  - `TerrainProviders.overworld`、`SurfaceProvider::query_surface`、`SurfaceInfo { y, passable }`。
  - `MundaneFaunaMarker` / `AmbientThreatMarker` 及既有 pool/spawn 函数。
  - 既有 NPC 脚点口径 `surface_y + 1`。

- **出料**：
  - scheduler 在 pool 调用前得到 surface-resolved `spawn_pos`；mundane 和 threat 共用同一个门禁。
  - provider 缺失或 `passable=false` 时，本次候选不 spawn；不回退到玩家 Y，不占用本 tick pending budget。
  - spawn 成功后的 marker、zone 预算、era gate、ring radius、回收、qi 守恒逻辑保持不变。

- **共享类型 / event**：
  - 只复用 `TerrainProviders` / `SurfaceProvider` / `Position` / `AmbientMarkerData` 等已有类型。
  - 不新增 `Gravity`、`Grounded`、第二套 height helper、spawn event 或近义 marker。
  - 不修改 `QiTransfer`、`TsySpawnRequested`、死亡/掉落事件。

- **跨仓库契约**：
  - **server**：唯一 gameplay 改动与主要测试落点。
  - **bot**：确定性 dev-only 测试接缝 + 场景，只验证真实 server Position 可见结果。
  - **client**：零改；继续渲染 server 权威 Position，不加入 client gravity hack。
  - **agent/schema/Redis**：零改。

- **worldview 锚点**：纯正确性修复，不新增玩法或世界观。沿用 `worldview.md §一:22` 与已归档 mundane/fauna 生态语义；飞鲸/诡影等既有飞行设计不变。

- **qi_physics 锚点**：无。地表选点不产生、转移、释放或衰减真元。

---

## 已收口的实施决议

### #1 唯一修复边界

**决议**：在 `ambient_scheduler_system::<M>` 的环带采样成功后、`config.pool_fn` 调用前统一解析地表。不得在 mundane/threat pool 各写一份 snap，也不得把 worldgen height 公式复制进 fauna。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:614-635,778-790` / 本 plan P0。

### #2 helper 语义

**决议**：新增或抽出一个可注入 `SurfaceProvider` 的纯函数，例如：

```rust
fn resolve_ambient_ground_position(
    candidate: DVec3,
    terrain: Option<&impl SurfaceProvider>,
) -> Option<DVec3>
```

- provider 存在且 `passable=true` → `Some(x, surface_y + 1, z)`；
- provider 缺失或 `passable=false` → `None`；
- 不复用 `snap_spawn_y_to_surface` 的 fail-open 返回值，除非先扩出明确的 strict variant；禁止根据函数结果是否“恰好等于输入”猜成功/失败。

**理由**：ambient 候选可以安全丢弃并等下一轮；保留错误玩家 Y 比少刷一只更坏。

**落点**：`server/src/npc/spawn/common.rs:219-229`（对照，不改变既有 caller 的 fail-open 语义）+ `ambient_scheduler.rs` 新 strict helper / 本 plan P0。

### #3 脚点口径

**决议**：使用 `surface_y + 1`。这是 `SurfaceInfo.y` 顶实心块语义、`snap_spawn_y_to_surface` 和 Navigator `ground_y + 1` 的现有 NPC 合同。玩家 `safe_y` skeleton 的 `+1/+2` 决策属于玩家碰撞安全，不在本 plan 另开分叉。

**落点**：`server/src/world/terrain/mod.rs:63-100` + `server/src/npc/spawn/common.rs:219-229` / 本 plan P0、P1。

### #4 provider / passable 错误策略

**决议**：scheduler 新增 `Option<Res<TerrainProviders>>`；生产环境只取 `providers.overworld`。资源缺失、查询列不可走时跳过本次 spawn；不调用 pool、不增加 `pending_spawns_by_zone`。保留现有 scheduler 周期重试，自然在后续巡检重新选候选，不新增永久 pending state。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:614-635,784-801` / 本 plan P0、P1。

### #5 Navigator 与邻接 caller 边界

**决议**：本 PR 不改 `navigator.rs`，不重做 `plan-npc-fixups-v3` 已完成的 idle/Dormant snap；也不修改兽潮、botany 吸引、教程鼠、繁衍、hydrate。它们是独立调用链的邻接风险，记录到 Finish Evidence 遗留；后续狩猎应按 entity archetype 单独验真，不能“凡 spawn 都 snap”。

**落点**：`server/src/npc/navigator.rs:366-385,496-507,1047-1117`（明确不改）/ 本 plan非目标与遗留。

### #6 bot 验收方式

**决议**：不采用“高处干等随机 ambient”的 flaky 场景。P2 为 bot 提供最小 dev-only 一次性测试接缝，直接复用 P0 strict resolver + 真实 mundane/threat pool，在固定 X/Z 和固定 mock/fixture surface 下触发一次；bot 通过现有实体 Position 跟踪断言动物不继承玩家 Y。接缝不得在生产自动玩法调用，不得绕过真实 resolver/pool；若现有 bot fixture 能直接驱动真实 scheduler，则优先复用而不新增命令。

**落点**：实施时先核查 `scripts/bot/scenarios/` 注册范式与现有 dev 命令树，选最小真实生产链接缝 / 本 plan P2。

---

## 非目标

- 不新增连续重力或 Velocity 积分系统；不把客户端当 NPC 物理权威。
- 不修改已有 idle/Dormant Navigator snap、path-empty/repath fail 分支。
- 不修兽潮 `zone.center().y`、botany 植物 Y、教程鼠玩家 Y、territory reproduction、hydrate 历史坐标；它们是后续独立 bughunt 候选，不与本原子 PR 混修。
- 不修改飞鲸：whale 故意无 `Navigator`，由 `whale_flight_system` + `NoGravity(true)` 控制。
- 不修改 ghost：其自有 position/drift，不进入 `With<NpcMarker> + Navigator` 地面导航链。
- 不修改纯视觉 marker、玩家出生 `safe_y`、worldgen 建筑/浮岛、TSY 飞行/剧情坐标。

---

## P0：ambient 共用地表门禁 ⬜

### 交付物

- `server/src/npc/spawn/ambient_scheduler.rs`：
  - system 注入 `Option<Res<TerrainProviders>>`；
  - 新 strict surface resolver（或放在 `spawn/common.rs` 并明确 strict 名称）；
  - ring candidate 解析成功后才调用 `config.pool_fn`；
  - 失败时 continue，不写 pending budget；
  - X/Z 保持 ring sample，Y 只来自 `query_surface(floor(x), floor(z)).y + 1`。
- mundane/threat pool 与 spawn 函数无需各自复制 surface 查询。
- 现有 `snap_spawn_y_to_surface` fail-open caller 保持兼容，不在本 PR 顺手改全局语义。

### 验收

- 高处玩家和地下玩家都不能决定动物 Y。
- mundane 与 threat/rat 真实生产链都经过同一个 strict resolver。
- provider/不可走失败不 spawn，不污染 pending/alive budget。
- ring radius、zone bounds、seed determinism、era/danger/season gate、回收和 qi 守恒行为不回归。

---

## P1：饱和回归 ⬜

测试名可按模块风格微调，但必须保留以下可 grep 的语义抓手：

### 纯函数测试（`npc::spawn::ambient_scheduler::tests`）

- `ambient_ground_position_high_anchor_uses_surface_plus_one`：candidate Y=200、surface=66 → 67。
- `ambient_ground_position_low_anchor_uses_surface_plus_one`：candidate Y=40、surface=66 → 67。
- `ambient_ground_position_floors_negative_fractional_xz`：负数/小数 XZ 查询坐标准确，输出保留原小数 X/Z。
- `ambient_ground_position_missing_provider_rejects_candidate`：`None` → `None`。
- `ambient_ground_position_impassable_rejects_candidate`：深水/岩浆语义 → `None`。

### scheduler / pool 生产集成测试

- `ambient_scheduler_snaps_mundane_pool_before_spawn`：固定 player Y=200 / fake surface=66，真实 mundane pool 产出 entity Y=67。
- `ambient_scheduler_snaps_threat_pool_before_spawn`：同条件真实 threat/rat pool 产出 Y=67。
- `ambient_scheduler_surface_rejection_does_not_call_pool`：provider 缺失与 `passable=false` 两分支均不调用 pool。
- `ambient_scheduler_surface_rejection_does_not_consume_pending_budget`：同 tick 失败候选不增加 `pending_spawns_by_zone`，后续合法候选仍可在预算内生成。
- 保留并复跑既有 `ring_sample_*`、`ambient_threat_pool_fn_spawns_*`、mundane pool/register 相关测试。

测试必须断言最终 ECS `Position.y`，不能只测试 strict helper 或手工把已 snap 坐标塞进 spawn 函数。

---

## P2：确定性 bot 场景与门禁 ⬜

- 新增/扩展一个 `scripts/bot/scenarios/` 场景，使用 §#6 的确定性 dev-only 接缝触发一次 mundane 和一次 threat/rat ambient 生成。
- 玩家测试高度与 fixture surface 至少相差 32 格；bot 从真实 spawn/move 包跟踪实体 Position，断言：
  - entity Y 等于 fixture `surface_y + 1`；
  - entity Y 不等于玩家 Y；
  - mundane 与 threat 各命中一次。
- 不通过睡眠等待随机 ambient budget；不新增 client-only debug payload；不以普通 `/spawn_npc` 代替 ambient 生产边界。
- 负向边界由 server 单测覆盖；本 PR 不需要为 whale/ghost 新建运行时测试，因为未修改 Navigator、whale 或 ghost query，但 validator 必须确认 diff 未触及它们。
- 完整门禁：
  - `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
  - 对应 bot scenario + `bash scripts/smoke-test-e2e.sh`（按 CI 现有入口执行，headless 设 `BONG_SKIP_SKIN_PREFETCH=1`）。

---

## 单 PR BugFix 实施约束

1. Subagent 原子 claim 本 skeleton，promotion 为 active 后第一性原理复验。
2. 单 PR 完成 P0–P2；代码修复、server 饱和测试、bot 场景可拆原子 commit，但不得拆成多个 PR。
3. 无上下文 validator 对最终 HEAD 验证：主证据链、strict fail-closed、真实两 pool、预算不泄漏、bot 不走假生产链。
4. 按受影响栈跑完整门禁；merge 最新 `origin/main` 后若 HEAD 变化，重新 validator + 门禁。
5. 全阶段 ✅、补齐 Finish Evidence 后归档；PR body 与 commit 带真实模型 trailer，并发独立 `/review` 评论。

---

## Finish Evidence

> Skeleton 阶段留空；BugFix 完成后填写。

### 落地清单

- P0：
- P1：
- P2：

### 关键 commit

- 待填写

### 测试结果

- 待填写

### 跨仓库核验

- server：
- bot：
- client：零改
- agent/schema：零改

### 遗留 / 后续

- Navigator active-goal + path-empty/repath-fail 的 ground reconciliation 独立验真。
- 兽潮、botany 吸引、教程鼠、territory reproduction、hydrate 的最终 X/Z surface contract 按 archetype 分别验真；不得直接扩本 PR。
