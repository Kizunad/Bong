# BugHunt: 动物空中生成与无重力悬空 — 地表生成契约断链 v1

> **一句话主题**：修复凡兽与地面型妖兽从 ambient / 生态入口生成时把玩家、zone 或目标点的 Y 原样当作实体脚点，导致动物在高台/空中出生；同时补齐 NPC 地面贴合的失败路径，让服务端不依赖不存在的 vanilla 实体重力，保证所有地面型动物最终落到 runtime terrain surface，而鲸、诡影等明确飞行实体保持原语义。

**状态**：Skeleton，2026-07-22 起草。问题已由多路 Sonnet 只读调查交叉确认；未修改玩法代码。

| 阶段 | 主题 | 状态 |
|---|---|---|
| P0 | ambient 主路径：候选点 Y 不再继承玩家高度，生成前接入地表契约 | ⬜ |
| P1 | 地面型动物的服务端贴地兜底：补齐 Navigator / path-empty / terrain fallback 门禁 | ⬜ |
| P2 | 同源生态入口审计与修复：兽潮、采集吸引、教程鼠、繁衍、hydrate 等 | ⬜ |
| P3 | 集成回归与故意飞行实体隔离：server + bot e2e + 文档契约 pin | ⬜ |

---

## Bug 摘要

玩家站在高台、悬崖或被抬到较高 Y 时，周围自然生成的牛、猪、羊、鸡、兔、蛙、狐、狼，以及 ambient 鼠/妖兽，可能直接出现在玩家同一高度的空气中。它们随后通常不会像 vanilla Minecraft 生物一样通过服务端重力自然下落；服务端没有 NPC 的 `Velocity.y -= gravity` 积分，Bong 只通过 `Navigator` 的离散地面 snap 纠正 Y。只要 spawn 时未贴地，或后续 snap 因 idle/path/chunk/terrain 条件被跳过或失败，实体就会保持错误高度。

这是一个**真实可达的 server gameplay bug**，不是把鲸或诡影等设计为飞行的实体误报为 bug。

### 最小确定证据链

```text
玩家 Position.y
  → sample_ambient_ring_position(anchor)
      → x/z = 玩家周围 24..64 格环带
      → y   = anchor.y（玩家当前 Y）
  → ambient_scheduler_system::pool_fn(spawn_pos, spawn_pos, ...)
  → mundane_pool_fn / ambient_threat_pool_fn 原样传递 spawn_pos
  → spawn_mundane_fauna_at / spawn_rat_npc_at / spawn_beast_npc_at
  → Position.y + Transform.y 原样写入
  → 只剩 navigator 的离散 snap 尝试；无服务端连续重力积分
```

关键落点（以实际实施分支的最新行号复核为准）：

- `server/src/npc/spawn/ambient_scheduler.rs:446-476`：`sample_ambient_ring_position` 的候选点为 `DVec3::new(cx, anchor.y, cz)`。
- `server/src/npc/spawn/ambient_scheduler.rs:778-790`：调度器把同一个 `spawn_pos` 原样传给 pool 函数。
- `server/src/fauna/mundane.rs:348-367`：凡兽 pool 不调用地表吸附。
- `server/src/npc/spawn/mundane.rs:69-81,160-165`：spawn 函数按调用方给定 Y 写入 `Position` / `Transform`，注释明确把 snap 责任交给调用方。
- `server/src/npc/spawn/ambient_scheduler.rs:334-360`：威胁/鼠 pool 与凡兽走同样的原样传递模式。
- `server/src/npc/spawn/common.rs:219-229`：`snap_spawn_y_to_surface` 已存在，但只在少数命令/教程/压力路径调用；失败时当前会保留原位置，可能把错误的空中 Y 原样放行。
- `server/src/npc/navigator.rs:272-274,366-385,459-465,1047-1117`：NPC “重力”是 idle/Dormant 或移动过程中的地面 snap，不是速度积分；path 为空、无 layer/terrain、非 Overworld、被 yield/Immobilized 等路径可不写 Y。
- `server/src/npc/sync.rs:5-20`：`Position → Transform` 单向同步，Transform 不是独立的物理修复源。
- Valence `Velocity` / `NoGravity` 仅承担协议组件/metadata 语义；Bong 没有为 NPC 运行实体重力积分系统。
- worldgen height 链本身使用绝对 world Y，`raster` / `query_surface` 语义一致；ambient 主路径绕开了 surface query，而不是 sea-level 单位错位。

### 可复现条件

1. 在 Overworld 一个有 ambient zone 的区域登录。
2. 站到明显高于周围地表的位置，或用测试 harness 将玩家 Y 抬到约 150/200，而环带地形仍在约 66–72。
3. 等待 ambient 调度节流与对应 fauna/threat budget 放行。
4. 在玩家 24–64 格水平环带观察：动物初始 Y 接近玩家 Y，而不是 `query_surface(floor(x), floor(z)).y + 1`。
5. 若 Navigator 的地面解析不可用、路径为空或实体被跳过，动物会继续悬空；若 idle fallback 成功，则会出现“先空中生成、随后突然落地”的可见闪烁。

### 影响范围

- **主路径**：ambient 凡兽与 ambient threat（鼠/妖兽），影响最大、最直接。
- **同源路径**：兽潮边缘点使用 `zone.center().y`；采集吸引使用植物目标 Y；教程鼠潮使用玩家 Y；领地繁衍使用领地中心 Y；hydrate 使用 snapshot/schedule/home 坐标。它们不一定每次都空中，但都没有统一的地面型动物生成契约。
- **重力观感**：服务端无 NPC 连续重力，客户端也不能被当作地面型自定义实体的权威物理兜底；服务端 Position 是最终同步来源。
- **不夸大**：已有 Navigator idle/Dormant snap 在部分正常 Overworld 条件下能把实体拉回地面，所以症状可能是短暂悬空而非永久悬空；这不能洗掉错误 spawn 首帧和 snap 失败时的持续悬空。

---

## 已有方案 / 非重复立项核验

本 skeleton 不重复已有文档，而是补它们之间的生产链断点：

- `docs/finished_plans/plan-ambient-threat-v1.md`：已实现 ambient 调度核，但没有定义或兑现 surface-aware spawn Y。
- `docs/finished_plans/plan-mundane-fauna-v1.md`：P0 文档要求“位置过 `snap_spawn_y_to_surface`”，代码却把责任交给 ambient caller，而 ambient caller 没有接 terrain provider；其 Finish Evidence 未覆盖此缺口。
- `docs/finished_plans/plan-npc-fixups-v3.md`：修复过 NPC idle snap 和若干 spawn 入口，但不是 ambient fauna / threat 统一地面契约。
- `docs/plans-skeleton/plan-bughunt-spawn-safe-y-surface-drift-v1.md`：只处理玩家出生 `safe_y` 与地表漂移，不处理动物生成。
- `docs/plans-skeleton/plan-bughunt-spawn-tutorial-poi-y-drift-v1.md`：只处理教学 POI 高度，不处理动物。
- `docs/reminder.md`：未发现本问题的同名承接项。

历史判断：`31bd564e4`（2026-07-04）引入 ambient 环带的 `anchor.y` 语义，`33c2509c7`（2026-07-05）凡兽复用该路径；相对 ambient 初版这不是“原本正确后回归”，而是实现缺口。`c25d10107` 已有地面 snap 工具和 idle 修复，说明地面契约本身已有先例但没有接通所有生产入口。

---

## 接入面 Checklist

- **进料**：
  - `Position`、`NpcMarker`、`Navigator`、`MovementController`、`TerrainProvider` / `query_surface`。
  - `AmbientSchedulerState<M>` / `AmbientSchedulerConfig<M>` / `ambient_scheduler_system::<M>`。
  - 现有 `snap_spawn_y_to_surface` 与 `snap_to_ground_with_fallback`，不另造第二套高度公式。
  - `MundaneFaunaKind` / `MundaneFaunaMarker`、`AmbientThreatMarker`、`FaunaTag` 及现有动物/妖兽 spawn 函数。
  - 兽潮、botany harvest、tutorial、territory reproduction、NPC hydrate 的现有坐标来源。
  - worldgen raster 的绝对 world-Y surface 语义；`SEA_LEVEL` 不作为刷怪高度。

- **出料**：
  - 所有纳入范围的地面型动物实体以 `surface_y + 1`（或经决议锁定的统一安全脚点）进入 ECS；没有有效可站立地表时跳过/重采样，而不是保留空中候选 Y。
  - Navigator 在错误初始高度、idle、path-empty、terrain fallback 等边界能按统一契约纠正或明确记录失败。
  - server→Valence 的 `Position` / move 包最终反映地面位置；client 不需要新增实体类型、payload 或重力 hack。
  - 回归测试与 bot e2e 能区分“生成即贴地”“短暂后贴地”“故意飞行”三类行为。

- **共享类型 / event**：
  - 复用 `Position`、`Transform`、`Navigator`、`NpcMarker`、`Despawned`、`TerrainProvider`、`AmbientMarkerData` 等现有类型。
  - 不新增替代 `SurfaceProvider`、`Gravity`、`Grounded` 或重复 `SpawnPosition` event；若需要表达空中/飞行白名单，优先复用现有 `NoGravity` / whale 专用控制语义，新增 component 必须在 P0 决议中证明现有类型不能表达。
  - 不改动 `TsySpawnRequested`、QiTransfer 或任何与本 bug 无关的事件。

- **跨仓库契约**：
  - **server**：本 plan 的全部生产修复和测试落点。
  - **client**：预期零改动；客户端只渲染 server 权威位置。若实测证明 custom fauna client 有独立重力/NoGravity 误设，才另列明确的 client P3 变更，不用 vanilla entity hack。
  - **agent/schema/Redis**：无新增字段、channel、schema 或 narration；动物位置 bug 不应通过 agent 修补。
  - **bot harness**：新增/扩展 `scripts/bot/scenarios/` 的 server 可见行为场景，具体文件名以现有场景注册表为准。

- **worldview 锚点**：纯正确性修复，不新增世界观；沿用 `docs/worldview.md §一:22`（死域连野兽都活不了）、§七既有生物生态语义及已完成 `plan-mundane-fauna-v1` 的凡兽地面生态。鲸/诡影的故意飞行继续按既有设计，不以本 plan 改写正典。

- **qi_physics 锚点**：无。该 plan 不生成、吸收、释放或衰减真元，不引入物理常数；所有 qi 行为保持既有路径。

---

## 设计轴心 / 边界

- 地面型动物的**生成位置必须 surface-aware**；不接受“先在空中生成，等重力慢慢修”的正常路径。
- “重力”在 Bong 中定义为 server-side ground reconciliation，不是假装 Valence 会替 NPC 积分 `Velocity`。
- 地形缺失、列不可走、水/岩浆、空 spans 等必须是显式拒绝/重采样/可观察 fallback 分支；禁止静默把原始玩家 Y 继续写入实体。
- 只修地面型动物/地面型妖兽；不把所有 `MarkerEntityBundle` 一刀切成贴地。
- 非目标：`spawn_whale` / `whale_flight_system` 的飞鲸、`ghost` 的漂移、纯视觉 marker、玩家出生 `safe_y`、worldgen 浮岛/建筑布局、真正的物理引擎、驯化/繁殖玩法本身。
- 任何 secondary path 若经核验属于故意空中行为，写入排除清单并补负向测试；不得因名字含 fauna 就强行 snap。

---

## P0：ambient 主路径 surface contract ⬜

**目标**：先堵住最常见的“玩家 Y → 动物 Y”断链，确保 ambient 凡兽与 ambient threat 共用同一地表门禁。

### 交付物

- 在 `server/src/npc/spawn/ambient_scheduler.rs` 的采样后、pool 之前或等价的唯一共用边界接入现有 terrain surface 查询；不得分别在 mundane/threat pool 中复制两套逻辑。
- 明确 X/Z 仍使用环带采样结果并以 `floor(x/z)` 查询 surface；实体站立 Y 统一为 runtime `surface_y + 1`，或在 §8.1 收口后采用仓库已存在的更保守统一口径。
- TerrainProvider 不可用、surface 不可走、列为空、查询失败时：有界重采样/跳过本候选并保留预算一致性；不得把 `anchor.y` 作为 fail-open fallback。
- 若调度器当前不持有 `TerrainProviders`，调整 system 资源注入或抽出纯函数，使依赖方向仍是 scheduler → terrain query，不把 worldgen 公式复制进 fauna 模块。
- `spawn_mundane_fauna_at`、`spawn_rat_npc_at`、`spawn_beast_npc_at` 的调用契约必须通过测试固定：调用方传入的位置已经是地面位置；spawn 函数不能 silently 重新恢复玩家 Y。

### 饱和测试

- 玩家 Y=200、surface=66 → ambient mundane entity Y=67；ambient threat/rat 同样为 67。
- 玩家 Y=40、surface=66 → 仍为 67，不把玩家低 Y 当作地下 spawn 高度。
- X/Z 负数与 tile 边界 → surface 查询使用 `floor` / `rem_euclid` 合同，不出现一格偏移。
- surface 不可走、terrain provider 缺失、空列、超出 world Y → 候选被跳过或走明确 fallback，绝不输出原始空中 Y。
- 环带距离、alive budget、zone bounds、seed determinism 与现有测试保持不变。
- 同一调度核分别注入 mundane/threat pool，验证两条生产链都命中地表门禁，而不是只测 helper。

---

## P1：服务端 ground reconciliation / “重力”兜底 ⬜

**目标**：修复 spawn 初始值正确性之外的悬空不动条件，但不引入新的连续物理系统。

### 交付物

- 审计并补齐 `server/src/npc/navigator.rs` 的 idle / Dormant / active path / path-empty / `snap_to_ground_with_fallback` 分支：对带 `NpcMarker + Navigator` 的地面型动物，允许在首 tick 或下一可用地形 tick 做一次地面 reconciliation。
- 地形有 `ChunkLayer` 时复用现有实体碰撞/站立检查；只有 raster fallback 时才走 `TerrainProvider::query_surface`，保持 surface 公式单一。
- 无 chunk、无 terrain、非 Overworld、`MovementController` yield、`Immobilized` 等条件必须有明确语义：不 panic；若暂时保持 Y，记录可诊断状态/日志并在下一可用 tick 重试，不把“无查询能力”当作已落地。
- path 为空或反复 repath 失败时不能让错误 Y 永久绕过地面 reconciliation；但不得抢占 dash/knockback/飞行控制器的 Position 写权，遵守每 tick 单一 Position writer 契约。
- 对明确飞行白名单（至少 whale；ghost/视觉 marker 不应进入本 query）补负向测试，证明 P1 不会把飞行实体拉回地面。

### 饱和测试

- idle 高空动物 + chunk surface → 一 tick 后脚点 `surface+1`。
- idle 高空动物 + 仅 terrain provider → fallback 后落地。
- 已在地面 → 不 double snap、不产生 Position 抖动。
- active goal/path-empty/repath failure → 不永久卡住错误 Y；下一次可用 terrain tick 能落地。
- 无 layer/无 provider → 不 panic、不伪造高度；记录/重试契约 pin。
- movement override / Immobilized → 不与 override 写者冲突，恢复后可贴地。
- 普通 humanoid/beast 既有寻路行为、step height、Transform 同步不回归。

---

## P2：同源生态入口 surface audit ⬜

**目标**：不让 P0 修完 ambient 后，另一条生态入口继续用 zone/target/player Y 复制造成同一 bug。

按入口逐条完成“来源 → 地表查询 → 有效候选门禁 → spawn → 集成测试”，只纳入经核验属于地面型动物的路径：

- `server/src/world/events.rs`：`beast_spawn_position_on_zone_edge` 使用 `zone.center().y` 的兽潮边缘点。
- `server/src/botany/hazard.rs`：`attracted_mob_position` 继承植物 Y 并抖动 X/Z，必须在最终 X/Z 重新 surface query。
- `server/src/world/spawn_tutorial.rs`：教程鼠潮从玩家 Y 派生的 rat 位置，与已正确 snap 的 tutorial rogue 对照。
- `server/src/npc/territory.rs` / reproduction request：幼兽从领地中心 Y 派生的 spawn 点。
- `server/src/npc/hydrate/`：mundane/beast snapshot/schedule/home 坐标 hydrate 后的落地契约；已悬空的旧 snapshot 不应永久复活为空中实体。
- 其它 Sonnet 审计发现的 ground-fauna caller，按同一表格补齐，不扩展到无关 NPC。

### 交付物

- 建立一份地面型 spawn caller 清单，明确每个 caller 是“已 surface-aware / 待修 / 故意空中”。
- 尽量把 caller 收敛到一个可复用的 `ground_fauna_spawn_position` 入口；若不能统一，必须让每个 caller 调用现有 helper，并有专属回归。
- 对 `snap_spawn_y_to_surface` 当前 fail-open 行为做决议：地面型候选无有效 surface 时拒绝/重试，不保留原始 Y；对水体/岩浆等不可走列保留既有设计语义或明确 skip。
- hydrate 只修新 hydrate 的位置合法性；不无条件改写玩家/NPC 持久化中明确属于飞行或剧情定位的坐标。

### 饱和测试

- 每个入口至少一条高 Y→低 surface、低 Y→高 surface、无 provider/不可走列、边界 X/Z 的专属 case。
- 兽潮、教程鼠、采集吸引、繁衍、hydrate 的生产集成测试断言最终脚点，而非只断言实体种类/数量。
- hydrate round-trip 既验证 ground fauna 落地，也验证故意飞行快照保持飞行。
- 多入口同 tick 不突破现有 alive budget，不重复 spawn，不裸 `despawn` Valence layer entity。

---

## P3：端到端与防误修隔离 ⬜

### 交付物

- 在 `scripts/bot/scenarios/` 的现有 bot 场景框架增加“高处玩家等待 ambient 动物”场景：定位实体种类/位置，断言 `entity_y == surface_y + 1`（允许项目统一的明确误差），并覆盖“先生成后纠正”不得成为正常成功结果。
- server 端保留纯单测/集成测试；bot e2e 只验证真实 wire / Position 可见结果，不用测试手塞实体掩盖生产链。
- 负向测试：飞鲸仍能在预定空中位置飞行；诡影/纯视觉 marker 不被地面 fauna helper 或 Navigator 地面门禁误伤；玩家出生 `safe_y` skeleton 的测试不与本 plan 混用。
- 更新本 skeleton 的阶段状态、真实落点、commit hash、测试命令与跨仓库核验，完成后才允许迁入 `docs/finished_plans/`。

### 验收标准

- 玩家处于任意高度时，所有纳入范围的地面型动物不以玩家 Y 作为最终 spawn Y。
- surface provider 可用时，生成位置脚点严格位于可走地表上方；不可用时有明确重试/拒绝/可观测 fallback，不会静默空中放行。
- 服务端不依赖 client vanilla gravity；地面型动物最终由 server Position 落地。
- 故意飞行实体行为不变。
- server 完整门禁、bot e2e 与新增回归全绿。

---

## §8 开放问题（P0 决策门前收口）

1. **surface query 注入位置**：ambient scheduler 在采样后统一 snap，还是让一个新的 ground-fauna boundary wrapper 负责；必须确保 mundane + threat + future passive pool 共用一处，而非各 pool 复制。
2. **不可走表面策略**：`query_surface.passable=false` 时是有限次数重采样、跳过本次 spawn、还是选择最近可走列？优先拒绝/重采样，不能 fail-open 保留原 Y；需以现有 ambient budget/Poisson sampler 约束为依据收口。
3. **脚点口径**：统一使用 `surface_y + 1`，还是沿用某些恢复路径的 `surface_y + 2` 保守值；需核对 entity feet / full-block / slab / snow 语义并在测试中锁定一个唯一口径。
4. **P1 是否扩大 Navigator 全局地面 reconciliation**：若全局 query 会影响非动物 NPC，应限定为已有地面型 NPC 能力/marker 组合，不能把飞行/剧情定位实体拉地；需要按实际 components 和系统调度收口。
5. **非 Overworld 地面动物**：ambient 当前只收集 Overworld 玩家，但兽潮、hydrate、其它生态路径可能跨维度；各维度是否有 TerrainProvider、缺失时应 skip 还是保留持久化坐标，必须逐入口决定。
6. **hydrate 历史坏坐标迁移**：对已保存的悬空 ground-fauna snapshot 是 hydrate 时修复一次、dehydrate 时规范化，还是仅阻止新错误继续产生；不得误改飞行/剧情实体。
7. **bot e2e 位置观测**：现有 bot 是否能读取自定义 fauna 的真实 Position 与 runtime surface；如不能，需先复用现有实体跟踪/height probe，不允许通过新增 client-only debug 逻辑绕开 server 契约。

**原表保留以备追溯；全部开放问题必须在 §8.1 决议后才能启动 P0 实施，实施以 §8.1 为准。**

## §8.1 决议（pre-P0 收口）

> 待实施前由 3–4 路只读 Explore/Plan agent 依据 origin/main 代码逐条收口。每条决议必须包含“文件:行号 + 本 plan 章节”双锚点；未收口不得消费 P0。

- [ ] #1 surface query 注入位置：待决议
- [ ] #2 不可走表面策略：待决议
- [ ] #3 脚点口径：待决议
- [ ] #4 Navigator 影响范围：待决议
- [ ] #5 非 Overworld 语义：待决议
- [ ] #6 hydrate 历史坏坐标策略：待决议
- [ ] #7 bot e2e 位置观测：待决议

---

## §10 实施工作流

本 skeleton 预估 3 个实现 PR（P0 主路径 / P1 地面 reconciliation / P2+P3 生态入口与 e2e）；若 §8.1 决议证明 P2 caller 需要拆分，仍保持本 plan 内按依赖顺序序列化，不另起同主题 skeleton。

1. **PR-1 P0**：ambient surface contract + mundane/threat 生产集成测试。只改 server，先证明高处玩家不再让动物同高生成。
2. **PR-2 P1**：Navigator 地面兜底与失败路径，含 idle/path-empty/terrain fallback 和飞行实体负向测试。
3. **PR-3 P2/P3**：同源生态 caller audit、hydrate 迁移策略、bot e2e 与最终契约文档。
4. 每个 PR 用独立 subagent 在隔离 worktree 实施；主线只接收结论，按根 `CLAUDE.md` BugFix 工作流执行 validator、受影响栈完整门禁、合并主线复验与 review。该 skeleton 本身只登记问题，不在本 PR 实施修复。
5. 修复完成后补 `Finish Evidence`，阶段全部 ✅ 后由 BugFix 流程把 active plan 归档；本 skeleton 不自动改 `docs/worldview.md`、`docs/library/` 或 agent schema。

---

## Finish Evidence

> Skeleton 阶段留空；进入 active 并完成修复后填写。

### 落地清单

- P0：
- P1：
- P2：
- P3：

### 关键 commit

- 

### 测试结果

- 

### 跨仓库核验

- server：
- client：
- agent/schema：无变更（除非后续决议证明必要）

### 遗留 / 后续

- 
