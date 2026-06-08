# Bong · plan-first-playable-world-cleanup-v1

> 状态：active draft / 待消费。目标是在不扩写新大系统的前提下，把“玩家进服第一分钟”从调试沙盒纠偏为可玩的生存开局，并清理 worldgen 中由 AI 惰性堆叠产生的怪异装饰，同时给 `dan_zong_yi_yuan` 建筑刷新提供硬证据和可 TP 验收命令。

## Worldview Anchors

- `worldview.md §一 L13-L22`：本 plan 继承“末法残土 / 灵气不会凭空产生 / 区域会被吸干”的基调；`GameMode` 默认从 Creative 改 Survival 只是调试闸门纠偏，不改变灵气守恒或修炼资源来源。
- `worldview.md §三 L61-L81`：首次可玩体验仍从六境界的醒灵开局出发，不新增境界、不改突破条件、不使用练气/筑基/金丹等旧称。
- `worldview.md §四 L213-L313`：生存开局必须让体表伤害、经脉损伤、真元消耗等战斗模型可触达；Creative 免伤只能作为显式 dev 模式。
- `worldview.md §七 L725-L750`：随机出生只在 spawn / 初醒区域内做安全分散，不把玩家直接送到异变兽、负灵域或中高阶威胁中心。
- `worldview.md §九 L836-L858`：本 plan 不改骨币经济，只确保 Survival 开局后掉落、采集、交易前置循环能被触发。
- `worldview.md §十 L862-L872`：decor 清理和丹宗刷新不得新增凭空资源；`placement_manifest` 只证明遗址/结构可见，不额外注入灵气或物资。
- 边界声明：`/tpzone` / `/tppoi` 是 dev/test 验收工具，不是正典传送能力；`dan_zong_yi_yuan` 沿用既有 zone id，不新增 canon zone 名称。

## 问题声明

- [ ] **默认 creative 导致无玩法**：新玩家进服默认 `GameMode::Creative`，战斗受伤、NPC 追踪、掉落等核心链路只对 `Survival` 生效，导致“进游戏无玩法”。
- [ ] **出生点单点堆叠**：首次登录、新生命、`/spawn`、坠落救援都回到固定 `[8.0, 150.0, 8.0]`，没有按玩家/人生 seed 分散，也没有按地表高度落点。
- [ ] **decorations 语义懒堆**：多个 profile 把“茅屋/碑/尸堆/钟楼/兵器林”等语义物塞进 `boulder/tree/shrub/crystal`，最终由通用几何堆成怪方块团。
- [ ] **丹宗医苑刷新证据缺失**：NBT/layout/loader 链路存在，但当前运行产物缺 `placement_manifest.json` 时服务端会静默加载 `0 placements`，不能证明建筑真正进世界。
- [ ] **缺明确 TP 验收入口**：已有 `/tpzone dan_zong_yi_yuan`，但只能去 zone 中心，缺可直达主殿/石棺/毒泉等 POI 的稳定测试命令。

## 代码证据

| 问题 | 证据 | 说明 |
|---|---|---|
| Creative 默认 | `server/src/player/mod.rs:125` | `initial_game_mode()` 直接返回 `GameMode::Creative`，测试也钉死该行为。 |
| Creative 短路玩法 | `server/src/combat/mod.rs:82`、`server/src/npc/brain/mod.rs:374`、`server/src/world/block_drop.rs:71` | 受伤、NPC 追踪、掉落只对 `Survival` 玩家生效。 |
| 固定出生 | `server/src/player/mod.rs:38`、`server/src/player/mod.rs:117`、`server/src/player/mod.rs:297` | `SPAWN_POSITION` 固定为 `[8.0, 150.0, 8.0]`，入服 defaults 直接设置。 |
| 多周目仍固定 | `server/src/cultivation/character_select.rs:40`、`server/src/combat/lifecycle.rs:1597` | 新生角色 spec 仍直接使用 `spawn_position()`。 |
| spawn zone 有范围无出生分布 | `server/zones.json:675` | `spawn` AABB 已存在，但没有 `spawn_points` / `spawn_distribution` 字段。 |
| decoration 通用几何 | `worldgen/scripts/terrain_gen/profiles/base.py:10`、`server/src/world/terrain/flora.rs:283` | `DecorationSpec` 只有 kind/block/size/rarity；Rust 按通用几何放置。 |
| 怪异 decor 高危源 | `worldgen/scripts/terrain_gen/profiles/tsy_gaoshou_hermitage.py:28`、`worldgen/scripts/terrain_gen/profiles/jiu_zong_ruin.py:28`、`worldgen/scripts/terrain_gen/profiles/waste_plateau.py:18` | “茅屋/钟楼/鲸骨树”等语义被塞进 `boulder/tree`。 |
| 未映射 block 静默丢弃 | `server/src/world/terrain/blocks.rs:17`、`server/src/world/terrain/flora.rs:296` | `bell/armor_stand/iron_ingot/glass_bottle` 等不是可解析方块，运行时被过滤后更怪。 |
| 丹宗 NBT 资产存在 | `server/structures/dan_zong/dan_zong_great_hall.nbt` | 主殿等 NBT 文件存在且非空。 |
| 丹宗 layout 存在 | `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py:185` | layout 定义约 50 个 placement。 |
| 运行产物缺口 | `worldgen/generated/terrain-gen/rasters/placement_manifest.json` | 当前未找到该 sidecar；服务端缺失时会静默返回空 placement index。 |
| 现有 TP | `server/src/cmd/dev/tpzone.rs:49`、`server/zones.json:744` | `/tpzone dan_zong_yi_yuan` 传到 zone center + 24；按当前 AABB 约 `-1600 136 4000`。 |

## 非目标

- [ ] 不重做完整 100h 主线；本 plan 只修“进服第一分钟”和世界可见缺陷。
- [ ] 不重写所有 terrain profile；只先清理高危 AI 惰性 decor，并建立防回归契约。
- [ ] 不新造丹宗建筑；除非发现 NBT/layout 本身损坏，否则只要求重烤、加载、可见性和 TP 验收。
- [ ] 不把 `/gm`、`/tpzone`、`/tppoi` 变成玩家正式玩法；它们仍是 dev/test 验收工具。

## 阶段总览

| 阶段 | 主题 | Status | Validated At | 完成标准 |
|---|---|---|---|---|
| P0 | 生存开局闸门 | Draft | TBD | 新玩家默认 `Survival`，核心玩法链路不再被 creative 短路。 |
| P1 | 出生分布选择器 | Draft | TBD | 首次登录/新生命按 seed 分散在安全 spawn zone，已存档玩家不漂移。 |
| P2 | decorations 清理与契约 | Draft | TBD | 高危语义懒堆禁用/迁移，未映射方块和不合法 kind 测试失败。 |
| P3 | 丹宗医苑真实刷新声明 | Draft | TBD | `placement_manifest.json` 被生成、服务端加载 `N > 0 placements`，主殿 chunk 可见。 |
| P4 | TP/POI 测试命令 | Draft | TBD | `/tpzone dan_zong_yi_yuan` 和新增 POI TP 命令可稳定到达验收点。 |
| P5 | 端到端冒烟 | Draft | TBD | `runClient` 进服为 Survival、出生分散、世界无高危怪堆、丹宗可 TP 可见。 |

## P0 — 生存开局闸门

- [ ] **默认模式改为 Survival**：`server/src/player/mod.rs::initial_game_mode()` 返回 `GameMode::Survival`；更新现有默认模式测试。
- [ ] **日志修正**：`init_clients` 日志输出真实 game mode，不再写死 “Adventure” 或与实际值不一致。
- [ ] **重登语义明确**：已有持久化位置/状态不应每次登录被 creative/default 覆盖；无持久化的新玩家才走默认 Survival。
- [ ] **dev `/gm` 门控决策**：保留调试能力，但 plan 必须明确 `/gm` 是否要求 `BONG_DEV_MODE` / 权限门控；默认正式开局不能依赖手动 `/gm s`。
- [ ] **玩法冒烟测试**：新增 server 测试覆盖 Survival 新玩家可受伤、可被 NPC 追踪、方块 `Stop` 产生掉落；Creative 仍保持调试免伤/瞬破。

### P0 验收命令

```bash
cd server
cargo test player:: -- --nocapture
cargo test combat:: -- --nocapture
```

验收点：新玩家默认模式断言为 `Survival`；相关测试不得靠 `/gm s` 预处理才能通过。

## P1 — 确定性随机出生分布

- [ ] **新增 `PlayerSpawnSelector` / `SpawnLocator`**：纯函数输入 `ZoneRegistry`、玩家 id / character id / life_index、用途枚举，输出确定性坐标。
- [ ] **扩展 spawn 数据源**：在 `server/zones.json` 的 `spawn` zone 增加 `spawn_points` 或 `spawn_distribution`，至少支持 anchor、radius、weight、safe_y 策略。
- [ ] **区分用途**：`InitialLogin`、`NewLifeBirth`、`DevSpawnCommand`、`FallRecovery` 分别定义策略；`/spawn` 不应无意改变新生命分布规则。
- [ ] **地表高度接入**：出生点不再固定 `Y=150`；必须查 runtime/worldgen surface helper 或保守安全高度，并经过地表/碰撞安全检查。
- [ ] **持久化不漂移**：首次创建采样后持久化；已有 player slice 恢复原位置，不因重登重新抽样。
- [ ] **多周目重新采样**：`next_character_spec()` 使用统一 selector，新生命按 life seed 重新抽点，但同一 life 内稳定。
- [ ] **兜底常量降级**：保留 emergency fallback，但命名和注释必须说明它不是唯一出生点。

### P1 测试矩阵

- [ ] 同一玩家同一 life seed 输出稳定。
- [ ] 不同玩家/不同 life 分散，不能全落 `[8,150,8]`。
- [ ] 所有输出在 `spawn` AABB 内，避开 blocked tiles / 水 / 深坑 / 高危事件点。
- [ ] 有持久化位置时不重新抽样。
- [ ] 无 registry / 无分布配置时 fallback 不崩，并产生告警日志。

## P2 — decorations 高危筛选与清理

- [ ] **建立 profile lint**：worldgen 测试读取所有 `DecorationSpec.blocks`，调用服务端等价 block name 映射，未知/非方块条目必须失败，不再运行时静默过滤。
- [ ] **建立 kind 白名单契约**：`tree` 只能木+叶，`boulder` 只能石/土/骨等自然实心块，`flower/shrub` 只能单格植物或低矮自然物，`crystal` 只能晶体类可见方块。
- [ ] **先处理高危清单**：禁用、降权或迁移以下条目：`bone_mountain`、`thatched_hermitage`、`daily_artifact_cache`、`broken_pillar`、`ruined_bell_tower`、`moss_lain_statue`、`forgotten_stele_garden`、`scripture_pile`、`sect_stele`、`whalefall_rib_tree`、`glass_fulgurite`。
- [ ] **语义建筑迁移规则**：房子、钟楼、碑、祭坛、棋盘、器物、尸堆、兵器林不得走 density flora；要么删除，要么走 `placement_manifest` / layout / NBT authored structures。
- [ ] **大尺寸/高频 review gate**：`size_range.max >= 8` 或 `rarity >= 0.45` 的非植物装饰必须人工白名单，否则测试失败。
- [ ] **preview 单独标注**：`worldgen/preview/decorations.json` 的 `end_rod` 柱只在 `BONG_PREVIEW_MODE=1` 生效；若保留，必须在测试/文档标明不属于普通世界生成。

### P2 验收抓手

- [ ] `worldgen` profile lint 失败样例覆盖 `bell/armor_stand/iron_ingot/glass_bottle` 等非 placeable item。
- [ ] `global_decoration_palette` 不再包含高危语义懒堆条目，或这些条目被迁移到 authored placement。
- [ ] 生成后抽样 dump 至少 3 个 profile，确认没有“半球茅屋”“石砖树冠钟楼”“鲸骨大树冠”等怪堆。

## P3 — 丹宗医苑真实刷新声明

- [ ] **重烤 worldgen 产物**：执行 terrain-gen，使 `worldgen/generated/terrain-gen/rasters/placement_manifest.json` 被实际写出。
- [ ] **manifest 硬断言**：`placement_manifest.json` 必须包含 `dan_zong_great_hall.nbt`，并满足 `structures.length >= 50`、总 `blocks.length > 0`。
- [ ] **服务端加载硬断言**：启动日志或测试必须证明 raster sidecar 加载后 `placements > 0`；缺 sidecar 不允许“静默假绿”。
- [ ] **chunk 实物断言**：在 `dan_zong_yi_yuan` 主殿附近 dump chunk，断言存在 authored blocks（如 `stone_bricks` / `mossy_cobblestone` / 主殿 palette），而不是只看自然地形。
- [ ] **三点可见性验收**：主殿 `dan_zong_great_hall`、至少一个药圃 `stamp_radial`、一段中轴大道 `block_grid` 必须在世界中出现。
- [ ] **失败回归**：删除或损坏 `placement_manifest.json` 时，新增测试/脚本应失败并提示“丹宗 placement sidecar 缺失”，避免当前 `0 placements` 静默兼容。

### P3 视觉纪律

- [ ] 若本阶段改动 NBT 或 layout 坐标/形状，必须按视觉资产纪律做 3 轮打磨：结构 dump / ASCII 平面投影 / 截图或 render 证据，终轮 commit 带 `<PROMISE>`。
- [ ] 若仅重烤 manifest 且不改视觉形状，也必须产出至少 1 份结构 dump 或 chunk sample 作为“真正刷新”证据。

## P4 — TP 到丹宗验收点

- [ ] **记录现有命令**：`/tpzone dan_zong_yi_yuan` 必须写入验收文档；按当前 zone AABB，预期落点约为 `x=-1600, y=136, z=4000`。
- [ ] **新增 POI 命令**：新增 server dev 命令 `/tppoi <zone:string> <poi:string>`，首批支持：`/tppoi dan_zong_yi_yuan great_hall`、`/tppoi dan_zong_yi_yuan master_sarcophagus`、`/tppoi dan_zong_yi_yuan poison_spring_main`。
- [ ] **复用现有架构**：新增 `server/src/cmd/dev/tppoi.rs`，在 `server/src/cmd/dev/mod.rs` 注册，并更新 `server/src/cmd/registry_pin.rs`。
- [ ] **坐标来源清晰**：POI offset 以 `worldgen/scripts/terrain_gen/layouts/dan_zong_compound.py` 为准；不得手填与 layout 脱节的魔法坐标。
- [ ] **命令测试**：覆盖 happy path、未知 zone、未知 poi、缺 executor 不移动、registry pin 更新。

### P4 手动测试命令

```text
/zones
/tpzone dan_zong_yi_yuan
/tppoi dan_zong_yi_yuan great_hall
/tppoi dan_zong_yi_yuan master_sarcophagus
/tppoi dan_zong_yi_yuan poison_spring_main
```

验收点：`/tpzone` 能到丹宗 zone，`/tppoi` 能到可见建筑/POI 旁边，且 chunk 内能看到 authored structure blocks。

## P5 — 首次可玩体验 E2E

- [ ] **启动约束**：headless/CI 启服必须设置 `BONG_SKIP_SKIN_PREFETCH=1`，避免 skin prefetch 干扰。
- [ ] **玩家路径**：新存档启动 → 进服即 `Survival` → 随机安全出生 → 可受伤/可采集掉落 → 不在 spawn 单点堆叠 → 可用 TP 命令到丹宗验收。
- [ ] **世界观路径**：开局仍在 spawn 大区/初醒区域，不引入上古命名，不使用禁词命名新 zone。
- [ ] **decor 抽样路径**：抽样加载 `spawn_plain`、`jiu_zong_ruin`、`tsy_gaoshou_hermitage`、`waste_plateau` 等高危 profile，确认无 AI 惰性方块团。
- [ ] **丹宗路径**：进入 `dan_zong_yi_yuan` 后能看到主殿/药圃/中轴大道至少三类 authored structures。

### P5 验收命令

```bash
cd server
cargo test player:: cmd::dev::tpzone cmd::dev:: -- --nocapture

cd ../worldgen
python -m scripts.terrain_gen
python -m pytest worldgen/tests/test_p2_dan_zong_activation.py
```

若新增专用 smoke 脚本，命名建议：`scripts/smoke-test-first-playable.sh`。该脚本只验证本 plan 的五个核心问题，不替代 100h journey E2E。

## 数据契约 / 下游 grep 抓手

| 名称 | 类型 | 位置 | 用途 |
|---|---|---|---|
| `PlayerSpawnSelector` | Rust module/struct | `server/src/player/spawn_selector.rs` | 统一首次登录/新生命/救援出生策略。 |
| `SpawnPurpose` | Rust enum | `server/src/player/spawn_selector.rs` | 区分 `InitialLogin` / `NewLifeBirth` / `DevSpawnCommand` / `FallRecovery`。 |
| `spawn_distribution` | zone config | `server/zones.json` | spawn zone 的 anchor/radius/weight 数据源。 |
| `DecorationSpec` lint | Python test | `worldgen/tests/` | 禁止非 placeable item 与语义懒堆进入 palette。 |
| `placement_manifest.json` | generated sidecar | `worldgen/generated/terrain-gen/rasters/` | 丹宗建筑刷新硬证据。 |
| `tppoi` | dev command | `server/src/cmd/dev/tppoi.rs` | 直达丹宗 POI 验收点。 |
| `dan_zong_yi_yuan` | zone id | `server/zones.json` | 现有丹宗 zone 与 `/tpzone` 验收入口。 |

## 风险与阻塞

- [ ] **worldgen 产物是否应入库**：若 `placement_manifest.json` 很大，需要决定是否提交产物、提交压缩 fixture，或只提交生成脚本 + 测试 fixture。
- [ ] **POI offset 坐标系**：`dan_zong_compound.py` layout offset 与 runtime zone center / raster 坐标必须实测对齐，不能只靠静态推导。
- [ ] **Survival 默认对旧调试流程影响**：现有预览/开发流程若依赖 creative，需通过 `/gm c` 或 env config 显式进入，而不是保留正式默认 creative。
- [ ] **decor 清理可能改变地形视觉密度**：禁用高危条目后要抽样确认 profile 不变空；必要时用低矮自然物替换，而非全删。

## Evidence Tracking（归档前暂存）

- [ ] 关键 commit 暂存：消费 plan 时逐阶段补入，归档前再迁移为正式完成证据。
- [ ] 测试命令与结果暂存：记录 P0-P5 每阶段实际命令、输出摘要和失败修复。
- [ ] 生成产物证据暂存：`placement_manifest.json` 统计、服务端 placement 加载日志、chunk dump 样例。
- [ ] 手动验收暂存：`/tpzone dan_zong_yi_yuan` 与 `/tppoi ...` 截图/坐标记录。
- [ ] 遗留后续暂存：若有无法在本 plan 完成的装饰迁移或丹宗视觉打磨，必须列明且不得影响 P0-P5 验收。

## 进度日志

- 2026-06-08：根据进服 creative、固定 spawn、怪异 decorations、丹宗医苑刷新证据与 TP 命令需求创建 active draft。
