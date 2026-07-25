# BugHunt: ambient 动物继承玩家 Y 导致空中生成 v1

> **一句话主题**：修复 `ambient_scheduler` 在玩家周围采样凡兽/威胁兽时把玩家当前 Y 原样当作实体脚点、且未查询 runtime surface 的生产断链；让 ambient mundane + threat 在进入 pool 前共用一次地表解析，地表不可用时跳过候选，禁止再 fail-open 到空中 Y。

**状态**：⏳ 最终 PR 闭环待验收（当前候选 HEAD：`25eb78ed2d567b8fe6ed0c0e9be63c8f41664fc3`）。P0/P1/P2 实现及定向验证已完成；前一目标代码树 `fb5c4cf4752f412b9c275a5a5904e034c72e34aa` 的 server、Java 17 client、协议 Bot、隔离 ambient 场景与隔离全栈闭环 smoke 已通过。当前文档证据提交后的 exact HEAD 尚待新的无上下文 validator、`/review` 与 CI 验收，因此不得把本文件表述为最终验收完成。归档文件依 BugFix review 返工契约原地更新，不重复 promotion 或归档移动。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | ambient 共用地表门禁：mundane + threat 生成前统一解析 runtime `ground_y + 1` / raster fallback | ✅ 2026-07-25 |
| P1 | 饱和回归：纯函数、真实 scheduler→pool 生产链、预算与错误分支 | ✅ 2026-07-25 |
| P2 | 确定性 bot 场景 + 非目标隔离 + 完整门禁 | ⏳ 实现与本地门禁完成，最终 PR gate 待验收 |

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
4. 当前实现会在玩家 24–64 格水平环带生成 `Position.y≈200` 的地面型动物，而正确脚点应优先来自该候选 X/Z 的 loaded `ChunkLayer` 安全支撑 `ground_y + 1`；runtime 标准窗口无法解析时才允许使用 passable raster `query_surface(...).y + 1`。
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
  - `DimensionLayers.overworld`、runtime `ChunkLayer`、Navigator 标准地面扫描 helper。
  - fallback `TerrainProviders.overworld`、`SurfaceProvider::query_surface`、`SurfaceInfo { y, passable }`。
  - `MundaneFaunaMarker` / `AmbientThreatMarker` 及既有 pool/spawn 函数。
  - 既有 NPC 脚点口径 `ground_y + 1` / `surface_y + 1`。

- **出料**：
  - scheduler 在 pool 调用前得到 surface-resolved `spawn_pos`；mundane 和 threat 共用同一个门禁。
  - runtime/raster 双源均不可用或不安全时，本次候选不 spawn；不回退到玩家 Y，不占用本 tick pending budget。
  - loaded runtime 支撑存在时不查询 raster；无 `TerrainProviders` 的 Flat/Anvil loaded chunk 仍能正常刷新。
  - spawn 成功后的 marker、zone 预算、era gate、ring radius、回收、qi 守恒逻辑保持不变。

- **共享类型 / event**：
  - 只复用 `DimensionLayers` / `ChunkLayer` / `TerrainProviders` / `SurfaceProvider` / `Position` / `AmbientMarkerData` 等已有类型。
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

## §8 开放问题（P0 决策门前需收口）

以下问题是本 skeleton 从调查结论进入实现前必须保留的决策记录；不得在实现 subagent 未重新核对代码锚点时擅自扩大范围。它们已由 6 路 Sonnet 只读调查、2 路 Sonnet 无上下文审查和生产注册/测试 fixture 复核收口，历史问题表保留以便追溯，实施时以 §8.1 为准。

### #1 ambient 地表门禁应放在哪一层？

必须选择 scheduler 采样成功后、`config.pool_fn` 调用前的单一边界，还是在 mundane/threat 各自 pool 内重复查询；同时确认不会复制 worldgen 高度公式。

### #2 缺失 provider / 不可走 surface 的语义是什么？

必须选择 fail-closed 丢弃候选，还是沿用 `snap_spawn_y_to_surface` 的 fail-open 原 Y；同时确认失败候选不调用 pool、不占 pending 预算。

### #3 地面型动物的最终 Y 采用哪套坐标合同？

必须确认 `SurfaceInfo.y` 是顶层实心块绝对 world Y，并决定实体脚点使用 `surface_y + 1`，不得混入玩家出生 `safe_y` 的碰撞安全偏移。

### #4 `TerrainProviders` 的生产注册与测试注入是否真实存在？

必须确认生产启动路径已插入 `TerrainProviders`，并为 ambient scheduler 的 `make_app` 及手工构建 `App` 的集成 fixture 安装 fake provider；资源缺失分支也必须有测试。

### #5 本 PR 的边界是否会吞并 Navigator 或邻接动物入口？

必须确认本原子修复只覆盖 ambient mundane + threat 共用链，不顺手改 Navigator、兽潮、botany、教程鼠、繁衍、hydrate、飞鲸或 ghost。

### #6 bot 验收如何避免随机 ambient 等待？

必须选择固定 seed + fake/fixture surface + 最小真实 scheduler/pool 接缝，禁止把随机刷新等待或 `/spawn_npc` 旁路当作生产链验收。

## §8.1 决议（pre-P0 收口，2026-07-22）

以下决议依据上列只读调查和代码锚点形成；active promotion 后，实施 subagent 仍须以 `origin/main` 重新核对行号与签名，若代码已漂移，先更新本节再进入 P0。原开放问题保留以备追溯，**实施时以本节决议为准**。

### #1 唯一修复边界

**决议**：在 `ambient_scheduler_system::<M>` 的环带采样成功后、`config.pool_fn` 调用前统一解析地表。不得在 mundane/threat pool 各写一份 snap，也不得把 worldgen height 公式复制进 fauna。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:614-635,778-790` / 本 plan P0。

### #2 helper 语义

**决议**：新增一个同时可注入 runtime `ChunkLayer` 与 raster `SurfaceProvider` 的 strict resolver：

```rust
fn resolve_ambient_ground_position(
    candidate: DVec3,
    layer: Option<&ChunkLayer>,
    terrain: Option<&impl SurfaceProvider>,
) -> Option<DVec3>
```

解析顺序固定：

1. 对 candidate X/Z/Y 使用 `floor` 得到 world block 坐标与 `ref_y`，优先复用 Navigator 既有标准窗口扫描（`ref_y - 16 .. ref_y + 4`，并按 layer 高度夹取）；该 helper 已统一 Euclidean chunk/local 坐标、支撑方块分类与双格净空规则，ambient 不复制规则；
2. runtime loaded chunk 找到安全支撑 → `Some(x, ground_y + 1, z)`，不得再查询 raster；
3. chunk 未加载 → 才允许直接采用 passable raster 的 `surface_y + 1`；
4. loaded chunk 的标准窗口 miss → raster 只能提供候选支撑 Y，必须在 live `ChunkLayer` 精确复核 support/feet/head：支撑需非液体且 `blocks_motion()`，双格净空需非液体且不阻挡，三格都需位于 layer bounds；任一不满足即拒绝，禁止 stale raster、空气或属性液体绕过；
5. runtime 与 raster 均缺失/不安全 → `None`。禁止保留 candidate/player Y，也禁止根据结果是否“恰好等于输入”猜成功/失败。

不复用 `snap_spawn_y_to_surface` 的 fail-open 返回值，也不复制 worldgen 高度公式。ambient 候选可以安全丢弃并等下一轮；保留错误玩家 Y 比少刷一只更坏。

**落点**：`server/src/npc/navigator.rs:1047-1057`（标准 loaded-chunk helper）+ `server/src/npc/spawn/common.rs:219-229`（fail-open 对照，不改既有 caller）+ `server/src/npc/spawn/ambient_scheduler.rs` strict resolver / 本 plan P0、P1。

### #3 脚点口径

**决议**：使用 `surface_y + 1`。这是 `SurfaceInfo.y` 顶实心块语义、`snap_spawn_y_to_surface` 和 Navigator `ground_y + 1` 的现有 NPC 合同。玩家 `safe_y` skeleton 的 `+1/+2` 决策属于玩家碰撞安全，不在本 plan 另开分叉。

**落点**：`server/src/world/terrain/mod.rs:63-100` + `server/src/npc/spawn/common.rs:219-229` / 本 plan P0、P1。

### #4 provider / passable 错误策略

**决议**：scheduler 同时注入 `Query<&ChunkLayer>` 与既有 `Option<Res<TerrainProviders>>`，通过 `DimensionLayers.overworld` 取得权威 runtime layer；`TerrainProviders.overworld` 只作为标准 runtime 窗口 miss/unloaded chunk 后的次级 fallback。这样 raster 世界仍可兜底远离 loaded window 的自然地表，而没有 `TerrainProviders` 的 fallback Flat/Anvil 世界只要目标 chunk 已加载且有安全支撑就能正常刷新，不会永久停刷。

runtime/raster 都不能给出安全脚点时跳过本次 spawn；loaded runtime 明确不安全即 veto，loaded scan miss 的 raster Y 必须回到 live chunk 精确复核，只有 unloaded chunk 才允许直接采用 passable raster。失败时不调用 pool、不增加 `pending_spawns_by_zone`。保留现有 scheduler 周期重试，不新增永久 pending state。测试 fixture 必须分别覆盖 loaded `ChunkLayer` 无 provider 成功、unloaded chunk + passable raster 成功、loaded scan miss 的 stale/液体/空气/越界拒绝、双源失败、runtime 胜过冲突 raster，以及完整 `App → ambient_scheduler_system::<M> → real pool → ECS Position` 两条生产链。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:665-861`、`server/src/world/dimension.rs:37-57`、`server/src/world/terrain/raster.rs:412-444` / 本 plan P0、P1。

### #5 Navigator 与邻接 caller 边界

**决议**：本 PR 不改变 `navigator_tick_system`、idle/Dormant snap、深扫描 fallback 或任何导航状态机行为；只把其既有标准 loaded-chunk 地面 helper 提升为 `pub(crate)` 供 ambient 复用，从而共享 Euclidean 坐标、支撑分类、双格净空与窗口夹取规则，禁止 ambient 复制一份近义实现。也不修改兽潮、botany 吸引、教程鼠、繁衍、hydrate；它们是独立调用链的邻接风险，记录到 Finish Evidence 遗留。

**落点**：`server/src/npc/navigator.rs:1047-1057`（仅扩大 helper 可见性，导航行为不变）+ `server/src/npc/spawn/ambient_scheduler.rs` strict resolver / 本 plan P0、P1 与非目标。

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

## P0：ambient 共用地表门禁（实现与本地验证完成）

### 交付物

- `server/src/npc/spawn/ambient_scheduler.rs`：
  - system 注入 `Query<&ChunkLayer>` + `Option<Res<TerrainProviders>>`，通过 `DimensionLayers.overworld` 读取 runtime layer；
  - strict resolver 固定执行 runtime 标准窗口第一权威、passable raster 第二 fallback、双源失败返回 `None`；
  - runtime 规则复用 `npc::navigator::resolve_ground_y_from_chunk`，不复制支撑/净空/坐标规则；
  - ring candidate 解析成功后才调用 `config.pool_fn`；
  - 失败时 continue，不写 pending budget；
  - X/Z 保持 ring sample，Y 只来自 runtime `ground_y + 1` 或 fallback raster `surface_y + 1`，永不继承 candidate/player Y。
- `server/src/npc/navigator.rs` 提供共享 `GroundLandingScan::{Safe, Unsafe, Miss}`：`Unsafe` 只表示 live runtime 已发现可站支撑但 feet/head 含液体，形成 stale raster 不得绕过的权威否决；普通窗口缺失保持 `Miss`，允许调用方沿既有 fallback。
- `server/src/npc/spawn/ambient_scheduler.rs` 以 `AmbientRuntimeGround::{Safe, LoadedUnsafe, NeedsRaster { loaded_chunk }}` 映射共享三态；loaded scan miss 的 raster Y 仍须回到 live `ChunkLayer` 精确复核，mundane/threat 共用同一 resolver 和提交边界。
- ring sampler 只在满足 `min_same_archetype_dist` 的合法候选中择优；候选全部越界或间距不足时返回 `None`，不再返回“最远但仍非法”的 fallback。
- scheduler 同时维护数量占位 `pending_spawns_by_zone` 与空间占位 `submitted_positions_by_zone`；只有 resolver 和真实 pool 均成功后才写入真实脚点，后续同 zone 玩家在同 tick 采样时把这些脚点并入 occupied set。
- `same_tick_same_anchor_keeps_submitted_positions_poisson_separated` 由两个显式同锚点玩家驱动真实 scheduler，并把最终 ECS `Position` 精确对拍到第一次 sampler 与 occupied-aware 第二次 sampler；第二次无合法候选时只生成一只才算合法。

### 验收

- 高处玩家和地下玩家都不能直接决定动物 Y；只在各自 `ref_y - 16 .. ref_y + 4` runtime 窗口内选安全支撑，窗口 miss 才使用 passable raster。
- loaded runtime 支撑优先于冲突 raster，且 Flat/Anvil loaded chunk 在无 `TerrainProviders` 时仍可生成。
- mundane 与 threat/rat 真实生产链都经过同一个 strict resolver。
- 双源缺失、液体/不可走或 pool `None` 均不 spawn，不污染 pending/alive budget。
- ring radius、zone bounds、seed determinism、era/danger/season gate、回收和 qi 守恒行为不回归。

### 玩家可感知反馈

- **粒子/VFX**：无新增、无颜色/数量/lifetime/spawn 模式变化；本 PR 只修正 server 权威首帧 `Position.y`，不发新的 `bong:vfx_event`。
- **音效**：无新增或变更的 `audio_recipe`、vanilla sound、pitch、volume、delay；动物既有生成/环境音保持原样。
- **HUD/屏幕效果**：游戏 HUD、client 渲染、layer、overlay、vignette、tint 与 shake 均不变；dev-only `/ambient_spawn once` 新增 accepted/rejected chat，仅回显 kind、请求 X/Z 与拒绝原因，不暴露 resolved Y。
- **环境/动画**：不改天空、雾、方块、terrain profile、动物动画或 Navigator 后续行为；只禁止错误的空中首帧。
- **narration**：不新增 broadcast/zone/player narration；既有事件流不变。

---

## P1：饱和回归（实现与本地验证完成）

测试名可按模块风格微调，但必须保留以下可 grep 的语义抓手：

### strict resolver 测试（`npc::spawn::ambient_scheduler::tests`）

- `ambient_ground_position_loaded_runtime_surface_beats_raster`：loaded runtime surface 与 raster 冲突时采用 runtime，且不查询 raster。
- `ambient_ground_position_high_reference_uses_runtime_standard_window` / `low_reference_*`：高/低 candidate 分别只按自己的 `ref_y - 16 .. ref_y + 4` 窗口选支撑。
- `ambient_ground_position_loaded_chunk_without_raster_succeeds`：Flat/Anvil 模式无 `TerrainProviders` 仍使用 loaded chunk。
- `ambient_ground_position_unloaded_chunk_uses_passable_raster` / `without_raster_rejects_candidate`：unloaded runtime 的 fallback 与双源 fail-closed。
- `ambient_ground_position_blocked_headroom_scans_farther_down` / `multi_layer_column_chooses_highest_safe_support`：双格净空与降序最高安全支撑。
- `ambient_ground_position_negative_fractional_xz_routes_runtime_chunk_euclidean`：负数/小数 XZ 使用 floor + Euclidean chunk/local 坐标，输出保留原小数 X/Z。
- `ambient_ground_position_liquid_runtime_and_impassable_raster_reject`：液体不算 runtime 支撑、不可走 raster 不得兜底。

### scheduler / pool 生产集成测试

- `ambient_scheduler_system_snaps_mundane_real_pool_on_loaded_chunk_without_raster`：真实 `App → ambient_scheduler_system::<MundaneFaunaMarker> → mundane_pool_fn`，玩家 Y 与 runtime surface 至少相差 13 格，最终 ECS entity Y=`ground_y+1`。
- `ambient_scheduler_system_snaps_threat_real_pool_on_loaded_chunk_without_raster`：同条件真实 `ambient_scheduler_system::<AmbientThreatMarker> → ambient_threat_pool_fn → rat` 产出。
- `ambient_scheduler_surface_rejection_does_not_call_pool`：双源缺失与 `passable=false` 分支不调用 pool。
- `ambient_scheduler_pool_none_does_not_tag_or_consume_pending_budget`：surface 成功但 pool 返回 `None` 时不挂 marker、不占 pending。
- `ambient_scheduler_surface_rejection_does_not_consume_pending_budget`：同 tick 失败候选不增加 `pending_spawns_by_zone`，后续合法候选仍可在预算内生成。
- 保留并复跑既有 `ring_sample_*`、`ambient_threat_pool_fn_spawns_*`、mundane pool/register 相关测试。

测试必须断言最终 ECS `Position.y`，真实 pool 的两条主用例必须实际运行 `ambient_scheduler_system::<M>`，不能只调用 private submission seam 或手工把已 snap 坐标塞进 spawn 函数。

---

## P2：确定性 bot 场景与门禁（实现完成；最终 validator/review/CI 待验收）

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

### P2 当前实现与定向证据（2026-07-25）

- `server/src/cmd/dev/ambient_spawn.rs` 注册确定性 `/ambient_spawn once mundane <x> <z>` 与 `/ambient_spawn once threat <x> <z>`；共享 `ambient_spawn once` wire 根、双 `double` 参数叶节点，并通过 `submit_ambient_dev_spawn_once` 复用 strict resolver、真实 `mundane_pool_fn` / `ambient_threat_pool_fn` 与真实 marker。
- handler 只从执行者权威 `Position.y` 构造 candidate reference，拒绝非玩家、非有限 X/Z、缺 Position、缺 CurrentDimension、非 Overworld、缺 zone/layer；缺 runtime 时保留 raster fallback，缺 provider 时保留 runtime 路径，双源失败稳定拒绝；accepted chat 不暴露 resolved Y。
- `scripts/bot/scenarios/npc_ambient_surface_resolution.py` 已自动发现并通过 Python 语法检查；场景固定 `/tpzone spawn` 后玩家 `(0,152,0)`、候选 `(5,3)`，在默认 `Season::Summer` 下按事件水位匹配 Cow type 18 / Rat type 126 的首帧 `y=73`，并用 `entity_pos` 二次复核、禁止随机 ambient 等待。
- `server/src/fauna/mundane.rs` 仅新增 `(spawn,5,3, Season::Summer)` 的 Cow witness pin；该 pin 不冻结 Winter 或转季时的生态权重与物种选择。
- dedicated 场景以 `REQUIRED_ENV = "BOT_E2E_AMBIENT_FIXTURE_OWNED"` 保护 fixture ownership；显式 `--scenario npc_ambient_surface_resolution` 缺该环境变量时，`scripts/bot/run_scenarios.py` 必须记录 `ERROR` 并令 runner 非零，禁止 `SKIP` 假绿。常规 `--all` 仅在 `RUN_IN_ALL_WHEN_ENV` 对应环境变量为 `1` 时 opt-in。
- `scripts/bot-e2e.sh` 读取 `PIPESTATUS`：runner 非零时优先返回 runner；runner 成功才采用 `tee` 状态。协议合同测试直接提取并执行真实 pipeline block，覆盖 runner/tee 四态及两者同时失败时的 runner 优先级。
- 已运行且实际命中测试：
  - `CARGO_BUILD_JOBS=1 cargo test --manifest-path server/Cargo.toml cmd::dev::ambient_spawn::tests -- --test-threads=1`：`running 11 tests`，11 passed。
  - `CARGO_BUILD_JOBS=1 cargo test --manifest-path server/Cargo.toml cmd::registry_pin::tests -- --test-threads=1`：`running 3 tests`，3 passed。
  - `CARGO_BUILD_JOBS=1 cargo test --manifest-path server/Cargo.toml 'cmd::tests::' -- --test-threads=1`：`running 4 tests`，4 passed。
  - `CARGO_BUILD_JOBS=1 cargo test --manifest-path server/Cargo.toml ambient -- --test-threads=1`：`running 119 tests`，119 passed。
  - `cargo fmt --manifest-path server/Cargo.toml -- --check`、`git diff --check`、`python3 -m py_compile scripts/bot/scenarios/npc_ambient_surface_resolution.py` 与场景自动发现检查均通过。
- gameplay merge `da3196a35684e54660d4dec739bc52fa2e38ffe4` 已完成 server `fmt + clippy -D warnings + cargo test`、schema/Tiandao、Java 17 client `test build`、确定性 ambient Bot 与隔离 runtime data 的 `smoke-test-e2e`；其后再次合并最新 `origin/main` 的 client/review-only 变化并复跑 Java 17 client 完整门禁。完整证据见 `## Finish Evidence`。
- `scripts/bot-e2e.sh` 后续按 review 证据收紧为每轮私有 evidence、Redis Compose project、随机 Redis host port 与 private runtime CWD；`data/**`、craft/mineral/NPC state、SQLite backup 与 `../library-web/public/deceased/**` 均不再写 checkout。botany/forge 的 CWD-relative `assets/**` 通过只读 checkout symlink 提供；外部 raster/spiritwood state、已有 25565 listener 与 REUSE 无 listener 均 fail-closed，禁止全局 `pkill`。

---

## 单 PR BugFix 实施约束

1. Subagent 原子 claim 本 skeleton，promotion 为 active 后第一性原理复验。
2. 单 PR 完成 P0–P2；代码修复、server 饱和测试、bot 场景可拆原子 commit，但不得拆成多个 PR。
3. 无上下文 validator 对最终候选 HEAD 验证：主证据链、strict fail-closed、真实两 pool、预算不泄漏、bot 不走假生产链。
4. 按受影响栈跑完整门禁；merge 最新 `origin/main` 后若 HEAD 变化，重新 validator + 门禁。
5. 全阶段 ✅、补齐 Finish Evidence 后归档；PR body 与 commit 带真实模型 trailer，并发独立 `/review` 评论。

---

## Finish Evidence

### 落地清单

- **P0**：`server/src/npc/spawn/ambient_scheduler.rs` 新增 runtime-first strict resolver 与 `AmbientRuntimeGround` tri-state；loaded runtime 明确不安全即拒绝，loaded scan miss 的 raster Y 必须回到 live `ChunkLayer` 验证 support/feet/head，双源失败不调用 pool、不占 pending budget。`server/src/npc/navigator.rs` 只把既有 `resolve_ground_y_from_chunk` 扩为 `pub(crate)`，未改变导航/重力行为。
- **P1**：`npc::spawn::ambient_scheduler::tests` 覆盖 runtime/raster 优先级、unloaded fallback、负坐标、扫描窗口、双格净空、默认/属性液体、空气/非运动支撑、layer 边界、pool `None` 与预算副作用；另以真实 `ambient_scheduler_system::<M>` → mundane/threat pool → ECS `Position` 锁住两条生产链。
- **P2**：`server/src/cmd/dev/ambient_spawn.rs`、`server/src/cmd/dev/mod.rs`、`server/src/cmd/mod.rs`、`server/src/cmd/registry_pin.rs` 提供 X/Z-only 的 deterministic one-shot；生产默认不向 Brigadier command tree 注册 `ambient_spawn` root，只有显式 `BONG_DEV_MODE` 才注册命令并安装私有 `AmbientSpawnDevAccess` capability，handler 对内部伪造 event 仍在 resolver/pool 前 fail-closed。命令 handler 与生产 `ambient_scheduler_system` 均只接受未标记 `Despawned` 的真实 `ChunkLayer`；raster 只能替代有效 live layer 内未加载 chunk 的 surface，不能替代 pool 的目标 layer。`scripts/bot-e2e.sh` 的自起服路径显式开启 dev mode，为每轮创建私有 fixture 目录并写入高熵 token；`TerrainProvider::load` 完整解析、验证并 mmap 所有 tile 后，server 才输出 canonical manifest path + token 的 `BOT_RASTER_FIXTURE_READY` marker，harness 同时对拍 exact marker、端口可连与 PID tree ownership 后才向 `scripts/bot/scenarios/npc_ambient_surface_resolution.py` 授予 ownership。场景在发命令前逐字节核验 `(5,3)` 的 `spans_count.bin` / `spans.bin` / `surface_id.bin` / `water_level.bin`，证明同 token fixture 的 support `y=72`、feet/head `y=73/74` 净空且无液体，再通过真实协议包验证 Cow/Rat 脚点 `y=73`，拒绝继承执行者 `y=152`。REUSE 模式不伪造 fixture ownership；REUSE 无 listener 时也不退化为缺少私有 runtime 的 self-start。self-start 为每轮独占 evidence、Redis Compose project/随机 host port 与 private runtime CWD，checkout `server/assets` 仅作 CWD-relative loader 的输入桥，所有相对持久化输出落在 evidence 内；运行前、运行中、运行后均校验 server/listener ownership，cleanup 只终止本轮进程树与本轮 Redis project。

### 关键 commit

- `7e0ae868f`（2026-07-22）：提升 skeleton 为 active plan。
- `e65707a10`（2026-07-22）：接入 ambient runtime-first 地表门禁与饱和回归。
- `5bcbf20b3`（2026-07-22）：补齐 deterministic one-shot 命令与 Bot witness。
- `e35a91d7e`、`3bd728680`（2026-07-22）：封堵默认液体与 property liquid 净空绕过。
- `359d9f26c`、`d5cc483f7`、`9ff30f675`（2026-07-22）：封堵 loaded scan miss、空气支撑、非运动支撑与 layer 边界绕过。
- `36e310152`（2026-07-22）：以 request object 收束 dev helper，恢复全量 clippy 参数门禁。
- `8114e7e3e`（2026-07-23）：修复 Bot 对迟到登录 `PositionLook` 的误匹配。
- `71e5ea3ab`（2026-07-23）：按完整 diff review 修复 loaded scan miss 的 raster 精确落点；附近更高支撑不再否决已通过 live support/feet/head 校验的 raster Y，并补提交边界回归。
- `da3196a35`（2026-07-23）：合并 `746794871` 基线，完成 server/schema/agent/Bot/smoke 全量验证。
- `2361b7ee3`（2026-07-23）：再次合并最新 `origin/main` 的 review API 配置与 craft_outcome client 网络线程修复；无冲突、未触及本 plan 的 server/Bot gameplay 文件，并复跑 Java 17 client 完整门禁。
- `aa822b447`（2026-07-24）：格式化主线带入的突破动画断言，恢复 server `cargo fmt --check`。
- `29b18cbdd`（2026-07-24）：隔离 Bot E2E 的 evidence、Redis、runtime CWD 与持久化输出，移除全局 stale-server kill，并补齐 144 条 harness 合同。
- `fb5c4cf47`（2026-07-25）：合并最新 `origin/main`（`5828a5510`）；目标代码树包含本轮全部 hardening 与主线最新音频/client 变化，无冲突。
- `f2451ed73`、`5c1c62fc0`、`f0df73b96`（2026-07-25）：统一 Navigator/ambient 安全落点规则源，引入 `GroundLandingScan` 三态并区分液体权威否决与普通扫描缺失。
- `1cd990838`、`18f5bad7f`、`84927434a`、`c2280e304`（2026-07-25）：修复 Bot dedicated 场景执行语义、runner/tee 失败传播、真实 pipeline 合同回归与显式缺 env 假绿。
- `2c6afd4f4`（2026-07-25）：将确定性 Cow witness 收紧为 `Season::Summer`，不冻结其他季节生态。
- `95891d20e`、`46f307132`、`7c5e3d880`、`de0da7c27`（2026-07-25）：补同 tick 真实脚点空间占位、泛型调用修复，并以双同锚点玩家锁定 sampler→resolver→ECS 精确结果。

### 测试结果（前一目标代码树 `fb5c4cf4752f412b9c275a5a5904e034c72e34aa`；仅文档随后变化）

- **server 完整门禁**：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过；lib 11943 passed / 0 failed / 2 ignored，main 12 passed，full-app startup 1 passed，Tarkov integration 4 passed，doc 5 ignored。
- **client**：Java `17.0.19` 执行 `./gradlew test build` 成功；3/3 Fabric GameTests passed，21 tasks，4m11s。
- **schema / agent**：schema 生成物 freshness 为 406 files，30 files / 898 tests passed；Tiandao 72 files / 840 tests passed。
- **协议 Bot 合同**：`python3 scripts/bot/test_protocol.py` 148/148 通过；覆盖 dedicated missing-env `ERROR`、`--all` opt-in、fixture ownership 以及 runner/`tee` 真实 pipeline 退出状态矩阵。
- **定向 ambient 协议 witness**：isolated `npc_ambient_surface_resolution` 1/1 PASS；Cow type 18 与 Rat type 126 均由真实 `entity_spawn`/`entity_pos` 证明位于 `(5,73,3)`，未继承玩家 `(0,152,0)`。证据绑定本节目标 SHA，persistent state `UNCHANGED`，25565 与私有 Redis `60493` 均已清理。
- **隔离全栈闭环 smoke**：PASS；先通过 dev-reload detach、schema、Tiandao 与无 listener full-app startup，再以 private runtime CWD、private Redis `56019`、当前 checkout debug server 和 deterministic Tiandao one-tick 跑跨进程闭环。独立 subscriber 观测 `bong:world_state`、`bong:agent_command`、`bong:agent_narrate`，server 观测两个 `command_anchor stage=end ... result=ok`；persistent state `UNCHANGED`，25565 与 Redis 端口均已清理。第一次尝试仅因 listener readiness race 失败，修正为 listener + PID-tree 轮询后重试成功；失败轮不计作 PASS。
- **完整 Bot suite（历史证据，非全绿）**：30 pass / 1 skip / 1 fail；唯一失败为本 plan 范围外的 `combat_weapon_equip_damage` 等待 NPC spawn 超时。不得将该轮写成全绿，也不得用它替代上述定向 witness。
- **历史证据边界**：`71e5ea3ab`、`da3196a35`、`7c4c068f8` 等旧树门禁只证明各自 SHA，不再称“最终 HEAD”；其 validator 也均因后续代码变更失效。
- **最终 PR 闭环待办**：当前候选 HEAD 为 `25eb78ed2d567b8fe6ed0c0e9be63c8f41664fc3`，相对 `fb5c4cf...` 仅修改本 plan 文档。必须对该 SHA 启动无上下文、read-only validator，并在 push 后重新触发独立 `/review`、等待 exact-head CI 与 CodeRabbit。三者完成前，本计划不宣称 Finished。

### 跨仓库核验

- **server**：`resolve_ambient_ground_position`、`resolve_loaded_raster_landing`、`submit_ambient_spawn_candidate`、`submit_ambient_dev_spawn_once`、`AmbientSpawnCmd`、`AmbientSpawnDevAccess` 与 command graph pin 均落地；mundane/threat 共用同一 resolver 和提交边界，`BONG_DEV_MODE` 默认关闭命令树入口且 capability 拒绝伪造 event。
- **bot**：`scripts/bot/make_novice_raster_fixture.py` 为每轮 manifest 写入 `ambient-surface-v1` token 与 support/feet/head 元数据；`scripts/bot-e2e.sh` 只为本轮自起 server 声明 ownership，REUSE 模式清除 ownership，且没有 listener 时 fail-closed。self-start 独占 evidence、Redis Compose project/随机端口与 private runtime CWD；checkout assets 只作为 botany/forge 输入桥，DB/backups/craft/mineral/spiritwood/NPC archive/亡者公开导出均留在本轮 evidence。`scripts/bot/scenarios/npc_ambient_surface_resolution.py` 先核验同 token 的真实 tile 二进制，再走 `/tpzone`、`/ambient_spawn once`、`entity_spawn` 与位置 mirror，不依赖随机 ambient tick。
- **client**：零代码改动；继续渲染 server 权威 `Position`，未加入 client gravity hack；Java 17 全门禁通过。
- **agent/schema**：零代码改动、零 Redis/wire 变更；合并主线后 schema/Tiandao 全门禁通过。

### 遗留 / 后续

- Navigator active-goal + path-empty/repath-fail 的 ground reconciliation 独立验真。
- 兽潮、botany 吸引、教程鼠、territory reproduction、hydrate 的最终 X/Z surface contract 按 archetype 分别验真；不得直接扩本 PR。
- `e2e-redis.sh` 默认复用 `server/data/bong.db`，静态地形 fixture 会被合法的 `zones_runtime` hydrate 覆盖；测试数据隔离属于独立 harness 改进，不混入本 gameplay 修复。
- CodeRabbit 对 `return_spider_drained_qi_to_zone` 的账本 finding 经独立验真为真实但 out-of-scope：该 helper 与 ambient 回收调用由 `31bd564e45` 引入，`mimic_spider.rs` 不在本 PR diff。后续应另立拟态蛛 ledger 修复，覆盖死亡/超距回收、`qi_release_to_zone` accepted/overflow、账户归零、满区与重复回收测试；本 PR 不修改该既有链。
