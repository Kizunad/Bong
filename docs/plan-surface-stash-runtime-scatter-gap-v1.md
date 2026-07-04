# plan-surface-stash-runtime-scatter-gap-v1

> **Active**（骨架 → active 升级 2026-07-04，来源 PR #852 骨架，opus 已核过真问题+防孤岛）。一句话主题：修复 `SurfaceStash` 新手地表遗缴的**零生成**断链。当前代码把 `SurfaceStash` 的 enum/schema/搜索/VFX/respawn/loot pool 都接好了，但**主线没有任何 runtime 生产路径**，导致 spawn 区玩家正常探索时根本遇不到散修遗缴，也拿不到这条引导链承诺的入门资源。
>
> **玩家影响**：`docs/finished_plans/plan-onboarding-loop-v1.md:620-622` 明确把 `ling_shui` 标成**入门阶段唯一获取路径**，来源就是 `surface_stash_craft`；一旦 `SurfaceStash` 零生成，新手探索、手搓引导、配方碎片/灵水掉落链都会直接断掉。

## 阶段总览（按“先证据收口，再接生产，再补回归”拆）

| 阶段 | 主题 | 路由 | 状态 |
|------|------|------|------|
| P0 | 证据收口：确认 `SurfaceStash` 只有消费侧、没有生产侧 | fix_pr | ✅ 2026-07-02（骨架阶段已收口，见下） |
| P1 | runtime scatter 真接线：spawn `PoiNoviceSite` + `LootContainer` + 生成参数/坐标校正 | fix_pr | ⬜ |
| P2 | respawn / 搜索 / 引导资源回归，避免“补了首刷又漏复活” | fix_pr | ⬜ |

## 接入面（防孤岛 checklist）

- **进料**：
  - `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` 烘焙的 tutorial POI 清单（`providers.overworld.pois()`，raw manifest）—— 本 plan **只读**，用于避让检测，不新增 worldgen 侧导出。
  - `server/src/world/terrain/mod.rs::SurfaceProvider::query_surface`（既有 trait，`TerrainProvider` 已实现）—— 用于 Y 轴 snap + 可通行性判定。
  - `server/zones.json` 的 `spawn` zone AABB（`min:[-750,-64,-750]`/`max:[750,320,750]`）—— 用于散布半径合法性校验。
- **出料**：
  - `PoiNoviceRegistry`（`server/src/world/poi_novice.rs`）新增站点 → `PoiSpawned` event（既有 event，`network/poi_novice_bridge.rs` / `network/redis_bridge.rs` 已消费，天道 agent 可感知新 POI）。
  - `LootContainer{ kind: SurfaceStash }` 实体（既有组件，`world/tsy_container.rs`）→ 既有 `sync_tsy_container_visuals`（`world/entity_model.rs:502`）自动渲染 `BongVisualKind::DryCorpse` 外观；既有 `PoiRespawnStore`（`world/poi_respawn_tick.rs`）自动纳管 respawn 计时。
- **共享类型 / event**：全部复用既有类型，**不新增**任何 component / event / schema —— `PoiNoviceKind::SurfaceStash`、`ContainerKind::SurfaceStash`、`ContainerKindV1::SurfaceStash`、`PoiSpawned`、`LootContainer`、`PoiRespawnStore` 均已在 `plan-onboarding-loop-v1` 落地，本 plan 只补运行时生产调用点。
- **跨仓库契约**：**无新增**。client 的 `ContainerKindV1::SurfaceStash` 分支（搜索动画/toast）、agent 的 `ContainerKindV1` TypeBox variant、VFX `SurfaceStashOpen` 均已在 `plan-onboarding-loop-v1` P0.1/P0.2 完成并验收（消费侧完整，见 P0 证据）。本 plan 属于纯 server 模块补丁——只是把已存在的 server 端纯函数接进 `App` 调度，不改跨仓库 wire 格式，符合 `docs/CLAUDE.md` §四“跨仓库契约缺一面”例外条款（确实是纯服务端收口，非新玩法）。
- **worldview 锚点**：`docs/worldview.md` §十「资源与匮乏」→「"搜打撤"循环」（地表可见容器、无需钥匙、低搜索门槛的入门资源点）；`ling_shui` 的入门期唯一来源属于 §九「经济与交易」资源匮乏设计的落地一环。
- **qi_physics 锚点**：不适用。`SurfaceStash` 是纯物资容器（loot pool 掉落），不涉及真元 / 灵气数值、衰减或守恒变更，本 plan 不引入任何 `qi_physics` 相关常数或公式。

## P0 — 证据收口：主线零生成，不是"刷得少"（✅ 已收口）

- **worldgen 不会导出 `surface_stash` POI**：`worldgen/scripts/poi_novice_selector.py:13-20` 的 `PoiType` 只有 6 种，`build_novice_poi_manifest_payload()`（241-287）也只循环这 6 种；没有 `surface_stash`。
- **spawn profile 只下发 tutorial POI**：`worldgen/scripts/terrain_gen/profiles/spawn_plain.py:180-230` 只生成 `spawn_tutorial_coffin` / `tutorial_chest` / `tutorial_rogue_anchor` / `tutorial_rat_path` / `tutorial_lingquan`。
- **server 只消费上面这些 tutorial POI**：`server/src/world/spawn_tutorial.rs:462-537`（`spawn_tutorial_poi_markers`）的 `match poi.kind.as_str()` 只处理 `spawn_tutorial_coffin` / `tutorial_lingquan` / `tutorial_chest` / `tutorial_rogue_anchor`；没有 `surface_stash` 分支，`_ => {}` 兜底吞掉。
- **`SurfaceStash` scatter 仍停在"后续集成"**：`server/src/world/poi_novice.rs:445-447` 注释直写"Startup system 接入在后续集成"，实际只有纯函数 `scatter_surface_stashes()`（`poi_novice.rs:491`）与单测（`poi_novice.rs:751-895`），函数本体及其所有参数常数（`SURFACE_STASH_COUNT`/`SURFACE_STASH_MIN_DIST`/`SURFACE_STASH_MIN_POI_DIST`/`SPAWN_CENTER_X`/`Z`/`BASIC_RADIUS`/`SCROLL_RADIUS`/`CRAFT_RADIUS`/`BASIC_COUNT`/`SCROLL_COUNT`/`CRAFT_COUNT`）全部打了 `#[allow(dead_code)]`（`poi_novice.rs:452-479`），`server/src/world/poi_novice.rs::register()`（`poi_novice.rs:267-282`）的 `Startup`/`Update` 调度里**没有任何**调用 `scatter_surface_stashes` 的系统。
- **finished plan 自报"已实现 scatter"与现状不符**：`docs/finished_plans/plan-onboarding-loop-v1.md:210-218` 设计要求 server-side runtime scatter；`714-718` 的 Finish Evidence 也把 `poi_novice.rs (PoiNoviceKind::SurfaceStash + scatter)` 列为已落地——这是文档 ⚠️ 红旗，代码只有纯函数层，从未接入调度。
- **消费侧确认完整（不在本 plan 范围）**：`ContainerKind::SurfaceStash`（`world/tsy_container.rs:39` 起全部 match 臂已补齐）、搜索/respawn（`world/poi_respawn_tick.rs:33` `SURFACE_STASH_RESPAWN_TICKS`）、视觉（`world/entity_model.rs:593` `container_visual_kind` 已映射 `SurfaceStash → BongVisualKind::DryCorpse`，且 `sync_tsy_container_visuals`（`entity_model.rs:502`）是通用系统，任何带 `LootContainer`+`Position`+`EntityLayerId` 的实体都会自动挂视觉——不需要新写方块摆放函数）均已存在，P1 只需产出实体，视觉/respawn 会自动生效。

## P1 — runtime scatter 真接线

**交付物 1：`PoiNoviceRegistry` 支持增量注册（不清空既有站点）**

- `server/src/world/poi_novice.rs:120-123`（`impl PoiNoviceRegistry`）新增方法：
  ```rust
  pub fn extend(&mut self, sites: Vec<PoiNoviceSite>) {
      self.sites.extend(sites);
  }
  ```
  （`replace_all` 语义不变，供 `PoiNoviceLoader::load` 继续使用；新 scatter 系统改用 `extend`，避免清空 manifest 加载的 11 种既有 novice POI —— 见 §8.1 #2 决议。）

**交付物 2：坐标常数校正 + 生成参数收口**

- `server/src/world/poi_novice.rs:460,462`：`SPAWN_CENTER_X`/`SPAWN_CENTER_Z` 从 `128.0`/`128.0` 改为 `0.0`/`0.0`（对齐真实 spawn zone 中心，见 §8.1 #1 决议——修前的常数会把全部 12 个遗缴点系统性偏移，且叠加 `CRAFT_RADIUS=1000.0` 会冲出 `spawn` zone AABB）。
- `server/src/world/poi_novice.rs:452-479` 移除全部 `#[allow(dead_code)]`（scatter 系统真实接线后这些常数不再是死代码）。
- `server/src/world/poi_novice.rs:491` `scatter_surface_stashes` 签名扩展为：
  ```rust
  pub fn scatter_surface_stashes(seed: u64, existing_poi_xz: &[(f64, f64)]) -> Vec<ScatteredStash>
  ```
  拒绝采样循环新增两条判据（与既有 `too_close` 判据同级并列）：
  1. 候选点与 `existing_poi_xz` 中任一点的距离 `< SURFACE_STASH_MIN_POI_DIST`（100.0）→ 拒绝重采样。
  2. 候选点 `x.abs() > 700.0 || z.abs() > 700.0`（`spawn` zone AABB 半径 750 减 50 安全边距）→ 拒绝重采样。
  既有 3 个 test（`poi_novice.rs:751`/`790`/`811`/`818`/`872`）同步改调用签名传 `&[]`（无既有 POI 场景下行为不变，用于回归既有 12 点/最小间距/quota 断言）。

**交付物 3：新增 Startup 一次性生产系统**

- `server/src/world/poi_novice.rs` 新增常数：
  ```rust
  pub(crate) const SURFACE_STASH_SCATTER_SEED: u64 = 0x5343_4159_5F31_3200;
  ```
  （固定字面量，取代此前函数签名里"谁传 seed 谁负责"的空白——见 §8.1 #1 决议，本仓不存在可复用的"world seed"权威来源，因此不依赖任何运行时 seed 来源，直接写死常量，保证同一构建每次重启散布结果完全一致。）
- `server/src/world/poi_novice.rs` 新增函数 `pub fn scatter_and_spawn_surface_stashes`，signature：
  ```rust
  pub fn scatter_and_spawn_surface_stashes(
      mut commands: Commands,
      providers: Option<Res<TerrainProviders>>,
      layers: Option<Res<DimensionLayers>>,
      mut registry: ResMut<PoiNoviceRegistry>,
      mut spawned: EventWriter<PoiSpawned>,
  )
  ```
  行为：
  1. `providers`/`layers` 任一 `None` 时直接 `return`（与 `PoiNoviceLoader::load` 同一容错模式，测试 App 不装载完整世界插件时不 panic）。
  2. `existing_poi_xz` 取自 **`providers.overworld.pois()` 原始 manifest**（不是 `registry.sites()`）——因为 `spawn_tutorial_coffin`/`tutorial_chest`/`tutorial_rogue_anchor`/`tutorial_lingquan` 不带 `poi_novice` tag（`poi_novice.rs:349` `site_from_manifest_poi` 的 tag 门禁），不会进入 `PoiNoviceRegistry`；只用 registry 会漏挡教程 POI，遗缴会刷到棺材/教程锚点脸上（见 §8.1 #2 决议）。
  3. 对 `scatter_surface_stashes(SURFACE_STASH_SCATTER_SEED, &existing_poi_xz)` 返回的每个 `ScatteredStash`：
     - 调 `providers.overworld.query_surface(x.floor(), z.floor())`（`SurfaceProvider` trait，`terrain/mod.rs:82`）；若 `!info.passable`（水域/岩浆列）**跳过该点并在 seed 序列里取下一个候选**（不是接受错误 y，而是继续拒绝采样，直到凑满 12 点——复用现有 while-loop 结构，只是把"避水"也变成一个拒绝判据，而非事后调用 `snap_spawn_y_to_surface` 被动接受）。
     - `pos = DVec3::new(x, f64::from(info.y + 1), z)`。
     - 构造 `PoiNoviceSite { id: format!("{}:surface_stash:{}", DEFAULT_SPAWN_ZONE_NAME, position_id_token([x as f32, (info.y+1) as f32, z as f32])), kind: PoiNoviceKind::SurfaceStash, zone: DEFAULT_SPAWN_ZONE_NAME.to_string(), name: "散修遗缴".to_string(), pos_xyz: [x as f32, (info.y+1) as f32, z as f32], selection_strategy: pool_id.clone(), qi_affinity: 0.0, danger_bias: 0, tags: vec!["poi_novice".to_string(), "poi_type:surface_stash".to_string()] }`，`registry.extend(vec![site.clone()])`，`spawned.send(PoiSpawned { site })`。
     - `commands.spawn((LootContainer::new(ContainerKind::SurfaceStash, DEFAULT_SPAWN_ZONE_NAME.to_string(), TsyDepth::Shallow, pool_id, 0), Position(pos), EntityLayerId(layers.overworld)))`（与 `spawn_tutorial.rs:495-507` 的 `tutorial_chest` 分支同模式；**不新增任何方块摆放函数**——`sync_tsy_container_visuals`（`entity_model.rs:502`）已通用处理任何 `LootContainer`+`Position`+`EntityLayerId` 实体的外观，`container_visual_kind`（`entity_model.rs:586-594`）已把 `SurfaceStash` 映射到 `BongVisualKind::DryCorpse`）。
  4. `tracing::info!` 打印实际生成数量（对齐 `spawn_tutorial_poi_markers` 末尾的日志风格）。
- 注册：`server/src/world/poi_novice.rs:267-282`（`pub fn register`）追加 `.add_systems(Startup, scatter_and_spawn_surface_stashes.after(PoiNoviceLoader::load))`（必须排在 `PoiNoviceLoader::load` 之后，否则 `existing_poi_xz` 读不到已加载的教程/其他 novice POI，避让判据形同虚设）。

**交付物 4：determinism 回归**

- 新增测试 `poi_novice.rs::tests::scatter_and_spawn_surface_stashes_is_deterministic_across_restarts`：两次独立构造相同 mock `TerrainProviders`/`DimensionLayers` 的 App，分别跑一次 `Startup`，断言两次产出的 12 个 `PoiNoviceSite.pos_xyz` 逐一相等（锁 `SURFACE_STASH_SCATTER_SEED` 固定字面量 + 同 provider 输入 → 同输出，覆盖"同 seed / 同 spawn 区边界 / 同现有 POI 集，散点结果稳定"的验收要求）。

## P2 — 回归与验收

- **首刷回归**：新增测试 `server/src/world/poi_novice_scatter_integration_test.rs`（新文件，仿 `server/src/world/tsy_lifecycle_integration_test.rs` 的"几个 system 串起来跑一个 App"集成测模式，`mod poi_novice_scatter_integration_test;` 登记进 `server/src/world/mod.rs:48` 旁）：构造最小 `App` + mock `TerrainProviders`/`DimensionLayers`，跑 `poi_novice::register(app)` 后 `app.update()` 一次，断言：
  1. `PoiNoviceRegistry` 里 `by_kind(PoiNoviceKind::SurfaceStash).count() == 12`；
  2. 世界中存在 12 个带 `LootContainer{kind: ContainerKind::SurfaceStash}` + `Position` + `EntityLayerId` 的实体（`app.world().query::<&LootContainer>()` 过滤计数）；
  3. 每个 `PoiNoviceSite.pos_xyz` 与对应 `LootContainer` 实体的 `Position` 一致（生产链闭环，而不是"注册表有 site 但没有对应实体"或反之的半接线）。
  这条测试就是 §8 开放问题 #3 的收口交付物——**永久锁住"生产路径存在性"**，防止以后再退回"只有 enum/schema/搜索侧"的状态。
- **复活回归**：`PoiRespawnStore`（`server/src/world/poi_respawn_tick.rs:87-108` `respawn_tick`）现在会对 `registry.sites()` 里所有站点（含新接线的 `SurfaceStash`）调 `ensure_site`；新增测试验证 P1 接线后，`SurfaceStash` 站点在 `respawn_tick` 跑过一次 `Update` 后确实出现在 `PoiRespawnStore` 内部状态里（`store.get(&poi_id).is_some()`），且 3600 tick 后 `is_server_tick_ready` 返回 `true`（既有单测 `poi_respawn_tick.rs:173-193` 已锁定纯函数行为，本条测试补的是"注册表里真的有 SurfaceStash 站点可供 respawn 系统读取"这一环，而不是重复纯函数断言）。
- **资源回归**：`surface_stash_craft` pool 的灵水/碎片/蓝图链——P1 落地后跑一次 `bash scripts/smoke-test.sh` 或本地 `cargo run` 起服 + `/give` 走一遍手动验证：新号进服后能在 spawn 附近搜到至少一个 `surface_stash` 容器并开出 `surface_stash_basic`/`scroll`/`craft` 三种 pool 之一的产出，确认新手引导"唯一来源"链路重新可达。

## §8 开放问题（P0 决策门前需收口，已在 §8.1 全部收口）

1. runtime scatter 的 world seed 应从哪条现有权威来源取值，避免每次重启重新洗点。
2. 需要不要复用 `spawn_tutorial` 现有 surface snap / blocked tile 约束，避免遗缴刷进水里、石棺上或教程 POI 脸上。
3. 是否补一条"`SurfaceStash` 生产路径存在性"集成测试，防止以后再回到"只有 enum/schema/搜索侧"的半接线状态。

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-07-04）

### #1 runtime scatter 的 world seed 来源

**决议**：
1. 全仓不存在任何"world seed"权威资源——grep `server/src/` 未发现 `WorldSeed`/`TerrainSeed` 一类的运行时 Resource；`giant_sword.rs` 用固定 cell 坐标 hash（`hash_coords(cell_x, cell_z, salt)`）驱动 determinism，`tsy_container_spawn.rs`/`tsy_dev_command.rs` 的 seed 是调用方逐次传入的局部值，都不是"全局世界种子"。同时 `worldgen/tests/test_spawn_tutorial_profile.py:26` 与 `server/zones.json:675`（`spawn` zone `aabb.min/max = [-750,-64/-750] / [750,320,750]`）共同证实真实 spawn 中心是 `(0, 0)`，而 `poi_novice.rs:460,462` 现有的 `SPAWN_CENTER_X`/`SPAWN_CENTER_Z = 128.0` 是与真实坐标不符的过时 fallback 常数。
2. 采用固定字面量常数 `SURFACE_STASH_SCATTER_SEED: u64`（新增，`poi_novice.rs` scatter 常数区）驱动 `scatter_surface_stashes`，同时把 `SPAWN_CENTER_X`/`Z` 改为 `0.0`/`0.0`。字面量常数是本仓既有约定的自然延伸（`SPAWN_CENTER_X`/`Z` 本身也是硬编常数），不需要新增任何持久化/配置面就能保证"同一构建每次重启散布结果完全一致"。
3. 拒绝方案："读取存档 DB 里某个 seed 字段"——sqlite 里唯一相关的 `loot_seed`（`persistence/mod.rs:322`）是**每次战场遗物事件**的局部确定性种子（`char_id + created_tick + loot_seed` 复合键），语义上是战斗掉落而非"世界生成种子"，误用会引入不相关的耦合；"用进程启动时间做种子"则直接违反"避免每次重启重新洗点"的验收要求，两者均不采用。

**落点**：`server/src/world/poi_novice.rs:452-479`（常数区，新增 `SURFACE_STASH_SCATTER_SEED` + 改 `SPAWN_CENTER_X`/`Z`）/ plan §P1 交付物 2、3。

### #2 是否复用 `spawn_tutorial` 现有 surface snap / blocked tile 约束

**决议**：
1. **部分复用、部分加强**。`snap_spawn_y_to_surface`（`npc/spawn/common.rs:219-228`）在目标列不可通行（深水/岩浆）时**不会**改变传入的 y，只是原样放行——这是既有 rogue anchor 调用点（`spawn_tutorial.rs:501`）已存在的行为，直接照抄会让不可通行列的遗缴仍然刷出来（只是 y 坐标错误）。因此本 plan **不直接调用 `snap_spawn_y_to_surface`**，而是在 scatter 拒绝采样循环里内联同等语义：调 `SurfaceProvider::query_surface`（`terrain/mod.rs:82-99`，`TerrainProvider` 已实现）拿到 `passable`，`!passable` 时视为"太靠近"直接重采样，而不是事后被动接受。
2. `blocked_tiles`（`Zone` 字段）在生产环境几乎恒为空数组（grep 全仓所有非 test 赋值点均为 `vec![]`/`Vec::new()`），不是真实生效的约束面，**不纳入**本次避让判据，避免引入一个从未被写入过数据的死检查。
3. "石棺上或教程 POI 脸上"的真正风险点是 `spawn_tutorial_coffin`/`tutorial_chest`/`tutorial_rogue_anchor`/`tutorial_lingquan`——这些**不带** `poi_novice` tag（`poi_novice.rs:349` 的门禁），因此从不出现在 `PoiNoviceRegistry` 里；只查 registry 挡不住它们。正确做法是把 `existing_poi_xz` 判据的数据源定为 **`providers.overworld.pois()` 原始 manifest 全集**（不区分 tag），一次性同时覆盖"novice POI 互相避让"与"教程 POI 避让"两个子问题，不需要维护两条平行的避让逻辑。
4. 同时发现并顺手修正一个真实几何 bug：`CRAFT_RADIUS = 1000.0`（`poi_novice.rs:472`）+ 修正后的 spawn 中心 `(0,0)` 意味着采样点可能落在 `x`/`z` ∈ `[-1000, 1000]`，而 `spawn` zone AABB 半径只有 750（`zones.json:675`）——即坐标轴方向上的采样点会系统性越界超出 zone。加一条 `x.abs() > 700.0 || z.abs() > 700.0` 拒绝判据（750 减 50 安全边距）与上面两条判据同级并列。

**落点**：`server/src/world/poi_novice.rs:491`（`scatter_surface_stashes` 签名 + 拒绝采样循环）/ 新函数 `scatter_and_spawn_surface_stashes`（同文件，P1 交付物 3）/ plan §P1 交付物 2、3。

### #3 是否补"生产路径存在性"集成测试

**决议**：
1. 补，且不是可选项——这正是本 plan 存在的原因（P0 证据链的核心教训：finished plan 自报"已实现 scatter"却只有纯函数层，没有任何测试锁住"纯函数被 Startup 调度调用"这一环，才让半接线状态潜伏到现在才被发现）。
2. 新文件 `server/src/world/poi_novice_scatter_integration_test.rs`，仿照 `server/src/world/tsy_lifecycle_integration_test.rs`（已验证的"构造 `App::new()` + 挂系统 + `app.update()` + 断言世界状态"集成测模式，而非只测纯函数返回值）编写；`mod poi_novice_scatter_integration_test;` 登记进 `server/src/world/mod.rs:48` 附近（紧邻现有 `mod tsy_lifecycle_integration_test;`）。
3. 断言面覆盖三层，缺一不可：registry 计数（12 个 `SurfaceStash` site）→ 实体计数（12 个 `LootContainer{kind: SurfaceStash}` 实体）→ site↔entity 位置一致性。只测其中一层无法排除"注册表更新了但没生成实体"或"生成了实体但没写回注册表"两种新的半接线退化模式。

**落点**：`server/src/world/poi_novice_scatter_integration_test.rs`（新文件）/ `server/src/world/mod.rs:48` / plan §P2 首刷回归。

## 实施注意

- P1 三条交付物（常数区改动 / scatter 签名扩展 / 新 Startup 系统）耦合紧密，建议同一 PR 内完成，不拆分——中间态（比如只改了常数没接调度）没有独立验收意义。
- P2 的三类回归（首刷 / 复活 / 资源）可以同一 PR 收尾，测试代码量不大（预计 3-5 个新测试函数 + 1 个新集成测试文件），不需要单独开 PR。
- 本 plan 预计 1 个 PR 即可从 P1 走到 P2 完成，不适用 `docs/CLAUDE.md` §六"scope ≥ 4 PR"的多 PR 编排规范。
