# BugHunt: ambient 动物继承玩家 Y 导致空中生成 v1

> **一句话主题**：修复 `ambient_scheduler` 在玩家周围采样凡兽/威胁兽时把玩家当前 Y 原样当作实体脚点、且未查询 runtime surface 的生产断链；让 ambient mundane + threat 在进入 pool 前共用一次地表解析，地表不可用时跳过候选，禁止再 fail-open 到空中 Y。

**状态**：✅ 2026-07-26。此次实现前的权威代码/docs HEAD 是 `c390346c94620bd3286e6d158bc9669e8543b3fc`（其功能代码候选为 `88c8f25f8c1eea8abc7c91c36acfd5ba01b407b7`）；`662609339d69e14a06964b557497394fdeea03a5` 为功能候选祖先。P0/P1/P2 均已闭合；本次归档文件依 BugFix review 返工契约原地更新，不重复 promotion 或归档移动。PR 仍开放且未合并；本次测试与证据更新不得预先声称其新 HEAD 的 GitHub E2E、CodeRabbit、`/review` 或 `CLEAN` 状态，push 后须以当时 PR body/status 的 exact HEAD 记录为准。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | ambient 共用地表门禁：mundane + threat 生成前统一解析 runtime `ground_y + 1` / raster fallback | ✅ 2026-07-25 |
| P1 | 饱和回归：纯函数、真实 scheduler→pool 生产链、预算与错误分支 | ✅ 2026-07-25 |
| P2 | 确定性 bot 场景 + 非目标隔离 + 完整门禁 | ✅ 2026-07-26 |

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
  - `DimensionLayers.overworld`、runtime `ChunkLayer`、ambient 私有 strict 地面扫描。
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

**决议**：新增只供 ambient 使用的 strict resolver：`GroundLandingScan`、分类和扫描私有地落在 `server/src/npc/spawn/ambient_scheduler.rs`，不新增 `pub(crate)` Navigator API。ambient 的 runtime-first 算法为：

```rust
fn resolve_ambient_ground_position(
    candidate: DVec3,
    layer: Option<&ChunkLayer>,
    terrain: Option<&impl SurfaceProvider>,
) -> Option<DVec3>
```

解析顺序固定：

1. 对 candidate X/Z/Y 使用 `floor` 得到 world block 坐标与 `ref_y`；在 ambient 内以 `ref_y - 16 .. ref_y + 4`（并按 layer 高度夹取）扫描严格落点；
2. runtime loaded chunk 找到安全支撑 → `Some(x, ground_y + 1, z)`，不得再查询 raster；
3. chunk 未加载 → 才允许直接采用 passable raster 的 `surface_y + 1`；
4. loaded chunk 的严格扫描 miss → raster 只能提供候选支撑 Y，必须在 live `ChunkLayer` 精确复核 support/feet/head：三格均经统一 `contains_ambient_liquid` 判定，拒绝 Water/Lava kind 及任意 `Waterlogged=True` 属性态；support 还须 `blocks_motion()`，feet/head 不阻挡，三格均需位于 layer bounds；任一不满足即拒绝，禁止 stale raster、空气或水浸方块绕过；
5. runtime 与 raster 均缺失/不安全 → `None`。禁止保留 candidate/player Y，也禁止根据结果是否“恰好等于输入”猜成功/失败。

不复用 `snap_spawn_y_to_surface` 的 fail-open 返回值，也不复制 worldgen 高度公式。ambient 候选可以安全丢弃并等下一轮；保留错误玩家 Y 比少刷一只更坏。Navigator 的 legacy 扫描语义与 strict spawn admission 是可观察地不同的契约，强行共享规则源会回归 legacy caller；因此 ambient 私有实现由自身饱和测试锁住。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs` strict resolver / 本 plan P0、P1；`server/src/npc/spawn/common.rs:219-229` 仅作 fail-open 对照，不改既有 caller。

### #3 脚点口径

**决议**：使用 `surface_y + 1`。这是 `SurfaceInfo.y` 顶实心块语义、`snap_spawn_y_to_surface` 和 Navigator `ground_y + 1` 的现有 NPC 合同。玩家 `safe_y` skeleton 的 `+1/+2` 决策属于玩家碰撞安全，不在本 plan 另开分叉。

**落点**：`server/src/world/terrain/mod.rs:63-100` + `server/src/npc/spawn/common.rs:219-229` / 本 plan P0、P1。

### #4 provider / passable 错误策略

**决议**：scheduler 同时注入 `Query<&ChunkLayer>` 与既有 `Option<Res<TerrainProviders>>`，通过 `DimensionLayers.overworld` 取得权威 runtime layer；`TerrainProviders.overworld` 只作为标准 runtime 窗口 miss/unloaded chunk 后的次级 fallback。这样 raster 世界仍可兜底远离 loaded window 的自然地表，而没有 `TerrainProviders` 的 fallback Flat/Anvil 世界只要目标 chunk 已加载且有安全支撑就能正常刷新，不会永久停刷。

runtime/raster 都不能给出安全脚点时跳过本次 spawn；loaded runtime 明确不安全即 veto，loaded scan miss 的 raster Y 必须回到 live chunk 精确复核，只有 unloaded chunk 才允许直接采用 passable raster。失败时不调用 pool、不增加 `pending_spawns_by_zone`。保留现有 scheduler 周期重试，不新增永久 pending state。测试 fixture 必须分别覆盖 loaded `ChunkLayer` 无 provider 成功、unloaded chunk + passable raster 成功、loaded scan miss 的 stale/液体/空气/越界拒绝、双源失败、runtime 胜过冲突 raster，以及完整 `App → ambient_scheduler_system::<M> → real pool → ECS Position` 两条生产链。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:665-861`、`server/src/world/dimension.rs:37-57`、`server/src/world/terrain/raster.rs:412-444` / 本 plan P0、P1。

### #5 Navigator 与邻接 caller 边界

**决议**：本 PR 不修改 `server/src/npc/navigator.rs`：不改变 `navigator_tick_system`、idle/Dormant snap、深扫描 fallback、状态、caller、resolver 或测试，也不新增 `pub(crate)` Navigator API。历史功能候选 `7a0afaf05bcb8692aefa30516d4148dde3dd4340` 相对 `b398c4071042b091f1590989998568b49dce401e` 的 `server/src/npc/navigator.rs` diff 为空。严格 `GroundLandingScan`/分类/扫描及私有 resolver 只被 ambient 调用，ambient 自己拥有更严的 spawn admission 政策：运动支撑、液体/passthrough/叶方块拒绝，以及可读的 feet/head。Navigator legacy 语义保持原样；两者合同不同，强行共享会回归 legacy caller。也不修改兽潮、botany 吸引、教程鼠、繁衍、hydrate；它们是独立调用链的邻接风险，记录到 Finish Evidence 遗留。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs` 私有 strict resolver / 本 plan P0、P1 与非目标；`server/src/npc/navigator.rs` 无改动。

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

## P0：ambient 共用地表门禁（✅ 2026-07-25）

### 交付物

- `server/src/npc/spawn/ambient_scheduler.rs`：
  - system 注入 `Query<&ChunkLayer>` + `Option<Res<TerrainProviders>>`，通过 `DimensionLayers.overworld` 读取 runtime layer；
  - strict resolver 固定执行 runtime 标准窗口第一权威、passable raster 第二 fallback、双源失败返回 `None`；
  - strict resolver、`GroundLandingScan`、分类与 runtime 扫描私有地位于 ambient scheduler；仅 ambient 调用，且不复用或修改 Navigator API、状态、caller、resolver 或测试；
  - ring candidate 解析成功后才调用 `config.pool_fn`；
  - 失败时 continue，不写 pending budget；
  - X/Z 保持 ring sample，Y 只来自 runtime `ground_y + 1` 或 fallback raster `surface_y + 1`，永不继承 candidate/player Y。
- `server/src/npc/spawn/ambient_scheduler.rs` 私有持有 `GroundLandingScan::{Safe, Unsafe, Miss}` 与严格三格 runtime 分类：support、feet、head 均通过统一 `contains_ambient_liquid` 排除 Water/Lava kind 和任意 `Waterlogged=True`；`Unsafe` 表示 live runtime 已发现可站支撑但三格含液体或净空不安全，形成 stale raster 不得绕过的 ambient 权威否决；普通窗口缺失保持 `Miss`，按 ambient 自己的 strict resolver 决定后续 fallback。`server/src/npc/navigator.rs` 无改动。
- `server/src/npc/spawn/ambient_scheduler.rs` 以 `AmbientRuntimeGround::{Safe, LoadedUnsafe, NeedsRaster { loaded_chunk }}` 映射共享三态；loaded scan miss 的 raster Y 仍须回到 live `ChunkLayer` 精确复核，mundane/threat 共用同一 resolver 和提交边界。
- ring sampler 在 in-bounds 候选中保留既有按距离评分的 best-effort 最远 fallback；最小间距是偏好而非新增拒绝策略。
- scheduler 仅维护 tick 前活体位置与 `pending_spawns_by_zone` 数量预算；不新增 same-tick 空间占位或 strict Poisson 提交政策。

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
- `ambient_ground_position_liquid_runtime_and_impassable_raster_reject`：Water/Lava kind、非叶 `Waterlogged=True` 与不可走 raster 均不得形成 runtime 支撑或净空绕过。
- `contains_ambient_liquid_*`：锁住无属性石头、`waterlogged=false`、水浸 Oak stairs support 与水浸 Rail feet/head；direct 三格分类、runtime scan/resolve 与 loaded-raster 精确复核均命中该统一谓词。

### scheduler / pool 生产集成测试

- `ambient_scheduler_system_snaps_mundane_real_pool_on_loaded_chunk_without_raster`：真实 `App → ambient_scheduler_system::<MundaneFaunaMarker> → mundane_pool_fn`，玩家 Y 与 runtime surface 至少相差 13 格，最终 ECS entity Y=`ground_y+1`。
- `ambient_scheduler_system_snaps_threat_real_pool_on_loaded_chunk_without_raster`：同条件真实 `ambient_scheduler_system::<AmbientThreatMarker> → ambient_threat_pool_fn → rat` 产出。
- `ambient_scheduler_surface_rejection_does_not_call_pool`：双源缺失与 `passable=false` 分支不调用 pool。
- `ambient_scheduler_pool_none_does_not_tag_or_consume_pending_budget`：surface 成功但 pool 返回 `None` 时不挂 marker、不占 pending。
- `ambient_scheduler_surface_rejection_does_not_consume_pending_budget`：同 tick 失败候选不增加 `pending_spawns_by_zone`，后续合法候选仍可在预算内生成。
- 保留并复跑既有 `ring_sample_*`、`ambient_threat_pool_fn_spawns_*`、mundane pool/register 相关测试。

测试必须断言最终 ECS `Position.y`，真实 pool 的两条主用例必须实际运行 `ambient_scheduler_system::<M>`，不能只调用 private submission seam 或手工把已 snap 坐标塞进 spawn 函数。

---

## P2：确定性 bot 场景与门禁（✅ 2026-07-26）

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
- `scripts/bot-e2e.sh` 按两种显式模式执行：默认 **generic** 模式保留 caller 提供的 raster、spiritwood/state、Redis 与 `$ROOT/server` CWD；无 raster 时才传递高熵 fixture token。CI 选择 `BOT_E2E_AMBIENT_FIXTURE_MODE=1` 的 **owned** 模式，才创建私有 token/state、Redis Compose project/随机 host port 与 private runtime CWD，并以 strict marker、PID/listener ownership 验证 self-start。REUSE 使用既有 ordinary server、不得伪造 ownership，缺 listener fail-closed；cleanup 只清理本轮 owner 资源，不使用全局 `pkill`。
- 外部输入 contract concern 已获确认并由两模式修复；而 global `REQUIRED_ENV` finding 仍被证伪：仅两个声明，皆为 hard prerequisite，受支持 caller 会提供它，runner 语义未作超出既有分支的改动。尚未在 generic caller inputs 下运行真实 Docker/server E2E，当前 Bot 证据仅覆盖协议合同、shell/static 检查与 fresh revalidator。

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

- **P0**：`server/src/npc/spawn/ambient_scheduler.rs` 私有实现 runtime-first strict resolver 与 `AmbientRuntimeGround` tri-state；loaded runtime 明确不安全即拒绝，loaded scan miss 的 raster Y 必须回到 live `ChunkLayer` 验证 support/feet/head，三格统一经私有 `contains_ambient_liquid` 拒绝 Water/Lava kind 与所有 `Waterlogged=True`，双源失败不调用 pool、不占 pending budget。`ac8b2c5ab7f015c0db7dcdf35888b8aa87978cf4` 令运动阻塞水浸 support 成为权威 veto；`88c8f25f8c1eea8abc7c91c36acfd5ba01b407b7` 将其收束为仅结构性、非叶、非 passthrough 支撑，叶子仍为 scan `Miss`。严格 `GroundLandingScan`/分类/扫描只被 ambient 调用，Navigator 保持主线原有状态、caller、resolver、测试和 legacy 语义；功能候选 `88c8f25f8c1eea8abc7c91c36acfd5ba01b407b7` 相对 `b398c4071042b091f1590989998568b49dce401e` 的 `server/src/npc/navigator.rs` diff 为空。
- **P1**：`npc::spawn::ambient_scheduler::tests` 覆盖 runtime/raster 优先级、unloaded fallback、负坐标、扫描窗口、双格净空、默认/属性液体、空气/非运动支撑、layer 边界、pool `None` 与预算副作用；水浸闭合明确锁住无属性 stone、`waterlogged=false`、水浸 Oak stairs support 和水浸 Rail feet/head，并由 direct 三格分类、runtime scan/resolve 与 loaded-raster 精确复核共同覆盖；本轮额外锁住**窗口内运动阻塞的水浸非叶支撑**必须先归类 `LiquidObstructed`、映射 `LoadedUnsafe`、拒绝同列另一安全 raster 高度且 `SurfaceProvider` 零查询；另以真实 `ambient_scheduler_system::<M>` → mundane/threat pool → ECS `Position` 锁住两条生产链。
- **P2**：`server/src/cmd/dev/ambient_spawn.rs`、`server/src/cmd/dev/mod.rs`、`server/src/cmd/mod.rs`、`server/src/cmd/registry_pin.rs` 提供 X/Z-only 的 deterministic one-shot；生产默认不向 Brigadier command tree 注册 `ambient_spawn` root，只有显式 `BONG_DEV_MODE` 才注册命令并安装私有 `AmbientSpawnDevAccess` capability，handler 对内部伪造 event 仍在 resolver/pool 前 fail-closed。命令 handler 与生产 `ambient_scheduler_system` 均只接受未标记 `Despawned` 的真实 `ChunkLayer`；raster 只能替代有效 live layer 内未加载 chunk 的 surface，不能替代 pool 的目标 layer。`ac8b2c5ab7f015c0db7dcdf35888b8aa87978cf4` 令 `scripts/bot-e2e.sh` generic self-start 在未显式 `REDIS_URL` 时先探测/沿用 caller `127.0.0.1:6379` listener，缺失才启动并清理私有 Compose Redis；显式 `REDIS_URL` 原样保留。CI 显式选择 `BOT_E2E_AMBIENT_FIXTURE_MODE=1` 的 **owned** 模式，仍无条件创建私有 token/state/Redis/private CWD，并严格核验 marker、PID 与 listener ownership。REUSE 仅接管 ordinary existing server、无 listener fail-closed；cleanup owner-safe，保留无全局 `pkill`。除已有 fake-tool Redis 合同外，`scripts/bot/test_protocol.py` 现实际启动同一 `scripts/bot-e2e.sh` owned watcher 路径，以 fake cargo/runner、动态 localhost listener 和 test-owned PID cleanup 验证 runner 成功时 ownership 全程保持且 watcher complete；runner 阻塞时 owned server 退出或 listener 被非 `SERVER_PID` 树进程接管均 watcher lost、最终非零；runner/tee 原始退出码以 runner 优先。场景在发命令前逐字节核验 `(5,3)` 的 `spans_count.bin` / `spans.bin` / `surface_id.bin` / `water_level.bin`，证明同 token fixture 的 support `y=72`、feet/head `y=73/74` 净空且无液体，再通过真实协议包验证 Cow/Rat 脚点 `y=73`，拒绝继承执行者 `y=152`。

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
- `53683f362`（2026-07-26）：dev handler 改为私有，并移动 pack unused statement；行为不变。
- `b1ae488b8`（2026-07-26）：将 strict helper/tests 迁回 ambient scheduler；Navigator 恢复为 main。
- `7a0afaf05bcb8692aefa30516d4148dde3dd4340`（2026-07-26）：按历史 `/review` 的语义问题收口；将 `resolve_ambient_ground_position` 收回 ambient 私有边界，并以 property-insensitive `BlockKind` 对既有全部 10 类叶方块做严格拒绝。饱和测试覆盖 default、Oak `distance=1`、`persistent`、`waterlogged`、Mangrove 属性态，以及 runtime scan 和 loaded-raster 精确复核。
- `f9b590a7aa7ad0779d643749126dc3e6facd2608`（2026-07-26）：历史 latest-main 合并候选；将 `662609339d69e14a06964b557497394fdeea03a5` 合入 7a0，带入 docs/README 主线变更，未改变 server 代码树。
- `7e7c3ead20e4dc5ae8ff6169bf82bb327e552b2d`（2026-07-26）：将 Bot harness 收束为 explicit owned mode（CI 选择）与保留 caller 输入的 generic default；owned mode 保留私有 token/state/Redis/CWD、strict marker/PID ownership 和 owner-safe cleanup，REUSE 仅接管已有 ordinary server。
- `81f57ea44978e07673a87a7430b5b95b322110ca`（2026-07-26）：generic 无 raster 传递高熵 fixture token，owned mode 即使继承 `REDIS_URL` 仍强制私有 Redis，并在启动前拒绝非法 mode 值。
- `ac8b2c5ab7f015c0db7dcdf35888b8aa87978cf4`（2026-07-26）：PR #1254 review canonical 修复；将水浸结构支撑升级为权威 runtime veto，拒绝 raster 回退，并恢复 generic self-start 未显式 `REDIS_URL` 时对 caller 默认 `127.0.0.1:6379` Redis 的复用。
- `88c8f25f8c1eea8abc7c91c36acfd5ba01b407b7`（2026-07-26）：收束水浸 veto 边界；仅结构性、非叶、非 passthrough 支撑映射 `LoadedUnsafe`，property-variant waterlogged leaf 保持 `Miss`。
- `e4bb94a96d780218e79718516d438bf18d0b5d63`（2026-07-26）：修复 ambient 非叶水浸落点绕过；私有 `contains_ambient_liquid` 统一拒绝 Water/Lava kind 与 `Waterlogged=True`，覆盖 support、feet/head、direct 三格、runtime scan/resolve 与 loaded-raster 精确复核；isolated targeted test 2 passed / 0 failed / 12018 filtered。
- `c390346c94620bd3286e6d158bc9669e8543b3fc`（2026-07-27）：刷新此前功能代码候选的 docs evidence；其 GitHub exact-head E2E `30213880323` 已 SUCCESS（Smoke/E2E step 19、Bot step 20、`npc_ambient_surface_resolution` PASS，Bot summary total=33 / pass=32 / skip=1 / fail=0，Chat PASS），且 CodeRabbit Review completed、当时 PR 为 `MERGEABLE` / `CLEAN`。这是 c390 的历史精确证据，不能预支给后续 watcher 测试提交。
- 本次 review follow-up：原 server-exit fake runner 只在 `TERM` fake cargo shell 后固定 sleep，可能在 shell 仍处于 sleep 时让 runner 返回，watcher 因 stop 文件而写 `complete`。现将生产 watcher 的 terminal status 在 cleanup 前复制至 test-owned evidence；正常路径明确断言 `complete`，两条 fault 路径明确断言 `lost` 与 stderr 的 watcher-lost failure。fault runner 仅在确认故障布置成功后写 `watcher-lost-observed`：server-exit 轮询 parent PID 消失及端口关闭，replacement 轮询 listener ready、存活及 `lsof` PID ownership 对拍；任一 setup/handshake timeout 以 77/79/80/81 明确记录，不能拿非零冒充 watcher failure。临时副本将生产 `echo lost; exit 1` 变为 `echo complete; exit 0` 后，server-exit/replacement 定向测试均按 `watcher-lost-timeout` 失败。新 HEAD push 后仍必须重跑 GitHub gate、CodeRabbit、`/review` 与 CLEAN 核验。
- `b749b6cd0`（2026-07-26）：历史功能候选，已由后续 `7a0afaf05bcb8692aefa30516d4148dde3dd4340` 的可见性与叶属性态修复取代。
- `f2451ed73`、`5c1c62fc0`、`f0df73b96`（2026-07-25）：历史候选曾统一 Navigator/ambient 安全落点规则源、引入 `GroundLandingScan` 三态；随后以 `b1ae488b8` 恢复原始私有 ambient 边界，避免改变 Navigator legacy caller 合同。
- `1cd990838`、`18f5bad7f`、`84927434a`、`c2280e304`（2026-07-25）：修复 Bot dedicated 场景执行语义、runner/tee 失败传播、真实 pipeline 合同回归与显式缺 env 假绿。
- `2c6afd4f4`（2026-07-25）：将确定性 Cow witness 收紧为 `Season::Summer`，不冻结其他季节生态。
- `95891d20e`、`46f307132`、`7c5e3d880`、`de0da7c27`（2026-07-25）：补同 tick 数量预算、泛型调用修复与 scheduler→resolver→ECS 提交边界回归。
- 本次 review 返工（2026-07-25）：恢复 Navigator exact base resolver/caller 兼容性与 ambient ring best-effort fallback；移除新增的 strict Poisson/same-tick 空间占位政策，同时保留 ambient strict runtime/raster 落点门禁。

### 测试结果

#### 此次实现前 exact HEAD `c390346c94620bd3286e6d158bc9669e8543b3fc` 的外部门禁

- GitHub E2E run `30213880323` 对 c390 exact SHA 为 **SUCCESS**：Smoke/E2E workflow step 19 success、Bot step 20 success；`npc_ambient_surface_resolution` 明确 PASS，Bot summary 为 total=33 / pass=32 / skip=1 / fail=0，Chat PASS。
- CodeRabbit Review 为 completed；该时刻 PR 状态为 `MERGEABLE` / `CLEAN`。这些状态只证明 c390，不证明本次 watcher 测试的新 commit；新 HEAD push 后须重新绑定 GitHub E2E、CodeRabbit、`/review` 和 CLEAN。
- comment `5084794862` 的 owned Bot/Smoke evidence finding 已由上述 c390 exact-head E2E refute/closed（评论生成时的 race）；watcher string-pin finding 已由本次可执行覆盖 CONFIRMED→FIXED。

#### 当前功能候选 `88c8f25f8c1eea8abc7c91c36acfd5ba01b407b7` 证据

- **权威代码树**：`662609339d69e14a06964b557497394fdeea03a5` 是 `88c` 的祖先；权威 slot porcelain clean。
- **server 本地门禁**：`cargo fmt --check` PASS；`cargo clippy --all-targets -- -D warnings` PASS；完整 `cargo test` exit 0：lib 12019 passed / 0 failed / 2 ignored，main 12 passed，full-app 1 passed，Tarkov 4 passed，doc 5 ignored。
- **Bot 静态/协议门禁**：`bash -n scripts/bot-e2e.sh` PASS；`python3 scripts/bot/test_protocol.py` 158 passed / 0 failed、exit 0。owned watcher 的成功完整路径、server-exit、replacement listener 和 runner/tee 退出码优先级均为可执行合同：helper 在清理前返回 production watcher status，正常断言 `complete`、两条故障断言 `lost` 与 watcher-lost stderr；runner 另返回 `watcher-lost-observed`，从而拒绝 77/79/80/81 故障布置超时的假非零。server-exit/replacement 各连续 3 次 PASS；临时将 production watcher `lost` 变为 `complete` 后两条定向测试均失败（`watcher-lost-timeout`）。执行仍出现非失败 `ResourceWarning`，未予隐去。
- **ambient 定向**：`cargo test ... ambient_ -- --test-threads=1` 138 passed / 0 failed / 11883 filtered；状态转换 pin 覆盖 waterlogged structural support → `LiquidObstructed` → `LoadedUnsafe`，raster `SurfaceProvider` 查询计数为 0。
- **fresh 无上下文 validator**：目标 `88c`，PASS、0 blocker / 0 major。
- **review 裁决**：comment `5083912427` 的两个去重 major 均为 CONFIRMED→FIXED：水浸结构支撑的权威 veto 与 generic 默认 Redis listener 复用均已落地；但该评论针对旧 HEAD `83b` 且为 `REQUEST_CHANGES`，push 后必须对新 exact HEAD 重发 `/review`，不得预称复审通过。
- **外部门禁边界**：历史 `83b` E2E run `30205470993` 仅证明该历史 HEAD。push 后 GitHub E2E、CodeRabbit 与 PR `CLEAN` 必须绑定最终 docs commit 的 exact HEAD；generic 默认 listener 的真实 full Docker/server E2E 尚未单独运行，现有可执行 fake-tool shell 合同只证明不调用 Docker / 不 teardown caller Redis 与缺 listener 的 private self-start 分支。

#### 历史候选 7a0 / f9 证据（已被后续功能修复取代）

- **功能代码候选 `7a0afaf05bcb8692aefa30516d4148dde3dd4340` 的 authoritative slot gates**：targeted Sonnet worktree `41 passed / 0 failed`；slot `cargo fmt --check` PASS；slot `cargo clippy --all-targets -- -D warnings` PASS（3m32s）；slot 完整 `cargo test` PASS：lib 12014 passed / 0 failed / 2 ignored，main 12 passed，full-app 1 passed，Tarkov 4 passed，doc 5 ignored。
- **历史 latest-main 合并候选 `f9b590a7aa7ad0779d643749126dc3e6facd2608`**：父提交为 7a0 与 `662609339d69e14a06964b557497394fdeea03a5`；后者相对 7a0 仅带入 docs/README，和 server/Bot/本 plan 文件无重叠，合并干净。因此 7a0 的 server 代码树与上述 slot gate 没有变化；仍不可把它们伪称为 f9 的 fresh validator 或 GitHub gate。
- **历史 review `30193915963`**：preflight 与 review jobs SUCCESS，finalize 因 `REQUEST_CHANGES` FAILURE；重复 run `30193921319` SKIPPED。初始 reviewer 的 infrastructure 524 不是代码 finding；最终 rerun 确认两项语义问题，均由 7a0 修复：`resolve_ambient_ground_position` 曾为 public、违背 ambient 私有边界；以完整 `BlockState` 相等判断叶子会漏过带 `distance` / `persistent` / `waterlogged` 等属性态的 leaves 并将其视为支撑。
- **历史 Bot finding 裁决**：当时仅 `REQUIRED_ENV` global finding 被 refute；后续 review 已确认 generic external-input contract concern，不能沿用为“Bot finding 全部 refuted”。
- **历史 exact-head E2E**：`0f04aa2efe7af0eb63fd79b190f54a65156419b0` 的 E2E run `30193835466` SUCCESS，Client/schema/agent/server/Smoke/Bot/Chat stages 均 SUCCESS。它只证明 0f04，不能替代后续任何 HEAD。
- **历史 CodeRabbit**：0f04 status success，description 为 `Review rate limited`；这不是 approval，亦不能表述为 review fully passed。
#### 历史候选 b749 证据（已被后续功能修复取代）

- **server 本地门禁**：`cargo fmt --check` PASS；`cargo clippy --all-targets -- -D warnings` PASS；完整 `cargo test`：lib 12011 passed / 0 failed / 2 ignored，main 12/0/0，full-app 1/0/0，Tarkov 4/0/0，doc 0 passed / 5 ignored。
- **client**：Java 17 执行 `./gradlew test build`，`BUILD SUCCESSFUL`；3/3 GameTests，21 tasks。
- **fresh context-free validator**：目标为 b749 exact，确认 latest main 是其祖先；PASS，blockers none、majors none。
- **GitHub E2E**：run `30192040493`，`headSha` 为 b749 exact，SUCCESS；schema、agent、server、Smoke、Bot、Chat 等列出的 stage 均成功。
- **CodeRabbit**：状态 context success；描述为 `Review rate limited`，故没有可用的增量 finding，不将其表述为 approval。
- **PR 合并状态**：仅 b749 当时为 `MERGEABLE` 且 `mergeStateStatus=CLEAN`；该历史状态不适用于后续 HEAD。
- **`/review` 裁决**：run `30192039945` 已完成 review job 并返回 `REQUEST_CHANGES`。其真实的 Navigator scope finding 已在 b749 前修复；其新增的“必须共享 Navigator source”主张与 pre-P0 原始 #5 相矛盾，已拒绝。Bot handoff/scope 主张则逐项追至无受支持 caller break，以及该 exact E2E 的 Smoke→Bot→Chat 成功。此证据随后已由 review `30193915963` 的 7a0 修复与裁决更新。

#### 更早历史候选证据

- **server 完整门禁**：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过；lib 11943 passed / 0 failed / 2 ignored，main 12 passed，full-app startup 1 passed，Tarkov integration 4 passed，doc 5 ignored。
- **client**：Java `17.0.19` 执行 `./gradlew test build` 成功；3/3 Fabric GameTests passed，21 tasks，4m11s。
- **schema / agent**：schema 生成物 freshness 为 406 files，30 files / 898 tests passed；Tiandao 72 files / 840 tests passed。
- **协议 Bot 合同**：`python3 scripts/bot/test_protocol.py` 148/148 通过；覆盖 dedicated missing-env `ERROR`、`--all` opt-in、fixture ownership 以及 runner/`tee` 真实 pipeline 退出状态矩阵。
- **定向 ambient 协议 witness**：isolated `npc_ambient_surface_resolution` 1/1 PASS；Cow type 18 与 Rat type 126 均由真实 `entity_spawn`/`entity_pos` 证明位于 `(5,73,3)`，未继承玩家 `(0,152,0)`。证据绑定对应历史目标 SHA，persistent state `UNCHANGED`，25565 与私有 Redis `60493` 均已清理。
- **隔离全栈闭环 smoke**：PASS；先通过 dev-reload detach、schema、Tiandao 与无 listener full-app startup，再以 private runtime CWD、private Redis `56019`、当时 checkout debug server 和 deterministic Tiandao one-tick 跑跨进程闭环。独立 subscriber 观测 `bong:world_state`、`bong:agent_command`、`bong:agent_narrate`，server 观测两个 `command_anchor stage=end ... result=ok`；persistent state `UNCHANGED`，25565 与 Redis 端口均已清理。第一次尝试仅因 listener readiness race 失败，修正为 listener + PID-tree 轮询后重试成功；失败轮不计作 PASS。
- **共享 Bot harness**：GitHub E2E run `30151469631` 对历史候选 `77d8ec7c93645648c1fe05681295017146d6d2df` 产出 31 pass / 1 skip / 0 fail；该 artifact 不是当前候选的 exact-head CI gate。
- **历史证据边界**：`71e5ea3ab`、`da3196a35`、`7c4c068f8`、`77d8ec7c` 等旧树门禁只证明各自 SHA。`72b518685` 与 `f8141905` 为后续功能代码返工，已使此前 code gate、validator 与 E2E 对当前候选全部失效。

**post-update 外部门禁边界**：本次 follow-up 修改测试与证据，当前新 HEAD 的 fresh exact-HEAD validator 仍是 push 前必需门禁；push 后 GitHub E2E、CodeRabbit、`/review` 与 PR `CLEAN` 状态必须绑定当时的 current HEAD。PR status 与 PR body 是权威的 post-commit record，避免将尚不存在的自指 SHA 嵌入本文件。

### 跨仓库核验

- **server**：`resolve_ambient_ground_position`、`resolve_loaded_raster_landing`、`submit_ambient_spawn_candidate`、`submit_ambient_dev_spawn_once`、`AmbientSpawnCmd`、`AmbientSpawnDevAccess` 与 command graph pin 均落地；mundane/threat 共用同一 resolver 和提交边界。`ac8b2c5` / `88c8f25` 将仅结构性、非叶、非 passthrough 的 Water/Lava 或 `Waterlogged=True` support 映射为 `LiquidObstructed` / `LoadedUnsafe`，并使 leaf 保持 `Miss`；`BONG_DEV_MODE` 默认关闭命令树入口且 capability 拒绝伪造 event。
- **bot**：`scripts/bot/make_novice_raster_fixture.py` 为 owned mode fixture 写入 `ambient-surface-v1` token 与 support/feet/head 元数据；`ac8b2c5` 后 `scripts/bot-e2e.sh` 的 generic self-start 保留 caller raster/state/Redis 与 `$ROOT/server` CWD，显式 `REDIS_URL` 原样保留，未显式时先采用 caller `127.0.0.1:6379` listener、缺失才自起私有 Compose Redis。CI 显式选择 `BOT_E2E_AMBIENT_FIXTURE_MODE=1` 才启用 private fixture/Redis/CWD、strict marker/PID ownership 与 owner-safe cleanup。REUSE 仅使用已有 ordinary server，缺 listener fail-closed；无 global `pkill`。除 Redis fake-tool 合同外，`scripts/bot/test_protocol.py` 通过真实 `bot-e2e.sh` 生产主路径、fake cargo/runner、动态 localhost listener/PID tree 覆盖 watcher 的 complete、owned server exit、replacement listener lost 与 runner/tee exit priority；`bot-e2e.sh` 在删除 runtime status 前可选复制其 production terminal status 至 test-owned evidence，故正常断言 `complete`、fault 明确断言 `lost` 和 watcher-lost stderr。server-exit 先同步确认 parent PID 消失、端口关闭并等待 watcher `lost`，replacement 先同步确认 listener ready、存活并经 `lsof` PID 对拍实际持有动态端口后等待同一 handshake；runner result 必须为 `watcher-lost-observed`，77/79/80/81 的 setup/handshake failure 不能假充测试成功。cleanup 只终止测试自产 PID。`scripts/bot/scenarios/npc_ambient_surface_resolution.py` 先核验同 token 的真实 tile 二进制，再走 `/tpzone`、`/ambient_spawn once`、`entity_spawn` 与位置 mirror，不依赖随机 ambient tick。
- **client**：零代码改动；继续渲染 server 权威 `Position`，未加入 client gravity hack；Java 17 全门禁为历史候选证据。
- **agent/schema**：零代码改动、零 Redis/wire 变更；合并主线后的 schema/Tiandao 门禁为历史候选证据。

### 遗留 / 后续

- Navigator active-goal + path-empty/repath-fail 的 ground reconciliation 独立验真。
- 兽潮、botany 吸引、教程鼠、territory reproduction、hydrate 的最终 X/Z surface contract 按 archetype 分别验真；不得直接扩本 PR。
- `e2e-redis.sh` 默认复用 `server/data/bong.db`，静态地形 fixture 会被合法的 `zones_runtime` hydrate 覆盖；测试数据隔离属于独立 harness 改进，不混入本 gameplay 修复。
- fake-tool shell fault injection 是后续 harness 质量改进，不是当前 correctness blocker。
- CodeRabbit 对 `return_spider_drained_qi_to_zone` 的账本 finding 经独立验真为真实但 out-of-scope：该 helper 与 ambient 回收调用由 `31bd564e45` 引入，`mimic_spider.rs` 不在本 PR diff。后续应另立拟态蛛 ledger 修复，覆盖死亡/超距回收、`qi_release_to_zone` accepted/overflow、账户归零、满区与重复回收测试；本 PR 不修改该既有链。
