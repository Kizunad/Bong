# plan-mundane-fauna-v1 — 凡兽生态：把 1.20.1 原版动物用起来

> **一句话主题**：新增「凡兽」类目（牛/猪/羊/鸡/兔/山羊/青蛙/狐/狼等 MC 1.20.1 原版被动生物），走 Valence 原生 entity bundle（client 零改动、vanilla renderer 免费渲染），按 biome 分池游荡在残土上——补全「凡草/凡器/原生作物」叠加哲学缺失的动物侧：无灵、**低威胁但绝非无害**（威胁是谱系不是开关，见设计原则）、可猎（肉/皮/凡骨资源链复用现成屠宰系统），且作为灵气健康度的**可视化滞后指标**（死域无凡兽、负灵域入即枯萎化飞灰、凡兽绝迹=天道叙事的恶化征兆）。

**状态**：Active（升 active 2026-07-04）。§8 九条开放问题已在 §8.1 全部收口，实施以 §8.1 决议为准。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 凡兽底盘——原生 bundle spawn + 被动 AI 三件套 + ambient_scheduler 复用（升 active 2026-07-04） | ⬜ |
| P1 | 资源链——凡兽掉落分支（fauna_drop_system） / shelflife raw 肉血 profile 新建注册（升 active 2026-07-04） | ⬜ |
| P2 | 生态响应——qi 栖息门槛 / 负灵域灭杀 / preys_on 跨层捕食表 / 季节权重（升 active 2026-07-04） | ⬜ |
| P3 | 天道信号——FaunaEcologySnapshot 进 EcologyAnalyzer（narration 级，升 active 2026-07-04） | ⬜ |

**显式不做（scope 外）**：驯化 / 骑乘 / 圈养 / 繁殖配对 / 挤奶——畜牧是 worldview 纯空白（§862 无定居田园经济、§808 聚集触天道注视），做则先补正典（单独人工 PR），另立 plan（§8.1 #5 登记）。

---

## 背景与调研结论（2026-07-02）

- **技术零门槛**：Valence 2b705351 生成代码含全部 1.20.1 被动生物 bundle（`CowEntityBundle`/`PigEntityBundle`/... 共 248 个，`valence_entity` OUT_DIR entity.rs）；zombie NPC 已走同款 Rail A（`npc/spawn/zombie.rs:52` `ZombieEntityBundle { kind: EntityKind::ZOMBIE, .. }`），**协议层原生渲染，无需 fauna/visual.rs raw_id 对齐、无需客户端 GeckoLib 注册**——本 plan P0 照抄这个头部（**不照 `beast.rs:51` 的 `MarkerEntityBundle` custom visual 路数**，那条是妖兽专属视觉 shell）。当前全库零处使用 `EntityKind::COW/PIG/SHEEP`——全新接入面。
- **AI 现成**：commoner thinker（`npc/spawn/commoner.rs:26-32`）即被动三件套——`FearCultivatorScorer→FleeCultivatorAction`（逃修士）+ `HungerScorer→FarmAction`（原地进食回补 `Hunger`，`npc/hunger.rs` 已注册）+ `WanderScorer→GoToPoiAction`（`commoner.rs:32`，寻路去 POI；`WanderScorer→WanderAction` 是 `beast.rs` thinker 的用法，凡兽裁剪版按需取用其一）。仅"怕捕食者/仗着数量反抗"需新 scorer——**`is_prey_of`（`fauna/components.rs:121`）签名是 `(BeastKind, BeastKind) -> bool` 且内部用 `realm_tier()` 比较，凡兽无 `BeastKind`/无境界阶，此谓词无法直接扩展给凡兽用**，需新建跨层关系表（见 P2 与 §8.1 #2）。
- **资源链现成**：`raw_beast_meat`/`raw_beast_hide`（`server/assets/items/fauna.toml:98,109`，当前**均无 `shelflife_profile` 字段**）、屠宰工具链雏形（`fauna/butcher.rs` 判定工具决定产出；其 consumer `handle_butcher_requests` **已注册**为生产 `Update` system——`fauna/mod.rs:59`——但**全库无生产路径 `emit`/`send_event` 触发它**，仅测试代码 `send_event`。**真实缺口是缺生产侧 emit 触发，不是缺 consumer**：修正 Debate 审出的方向反——原判"无生产 system 侦听消费此事件"读反了，consumer 早已挂进 `Update`，缺的是谁来 emit）、`food.mundane.cooked_meat`（`food.toml:14`）、手搓台的 `tanned_hide`/`bone_chip_mat`/`bone_meal_mat`（`workbench_materials.toml`）已存在。**shelflife 的 `beast_meat_v1`/`beast_blood_v1` Spoil profile 全仓 grep 零命中，从未定义**（`shelflife/registry.rs` 实际已注册的是熟肉 `food_spoil_mundane_meat_v1`:107 与骨类 `fauna_bone_*_v1`:94-97，两者跟生肉/血无关）——本 plan P1 是**新建**这两份 profile，不是"翻注册开关"。
- **凡兽资源产出的真实生产路径**：击杀→掉落走已通产的 `fauna_drop_system`（`fauna/drop.rs:269`，侦听 `DeathEvent`）→写入 `DroppedLootRegistry`（`drop.rs:11,276`）；`drop_table_for(kind: BeastKind)`（`drop.rs:231`）当前键类型是 `BeastKind`，凡兽需要平行的 `MundaneFaunaKind` 掉落分支（P1 新增，非复用 `drop_table_for` 本体）。`ButcherRequest`/`fauna/butcher.rs` 因无生产触发，本 plan **不依赖**它作为凡兽资源链主路径（可选增强见 P1）。
- **地理数据现成**：`TerrainProvider.sample(x,z)` 运行时可查 `biome_id` + `is_peaks_biome/is_marsh_biome/is_spawn_biome/is_wastes_biome` 谓词（`server/src/world/terrain/raster.rs:364-380`，**非 `worldgen/raster.rs`**——那是 Python 侧无此文件），分池数据充分。
- **spawn 调度基建已就绪**：`plan-ambient-threat-v1`（PR #845，已 merge）在 `server/src/npc/spawn/ambient_scheduler.rs` 留了 `AmbientMarkerData` trait（`:502`，仅 `fn new(spawned_at, home_zone) -> Self` + `fn home_zone(&self) -> &str` 两函数）+ 泛型 `AmbientSchedulerState<M>`/`AmbientSchedulerConfig<M>`（`:534,564`）+ `ambient_scheduler_system::<M>`（`:604`），并留了 `TestFaunaMarker` 专属复用回归测试（`:1578-1967`，注释明写"plan-mundane-fauna-v1 复用场景最小复现"）钉死 `counts_against_threat_budget=false` 时独立预算互不干扰。本 plan P0 **零改调度核，纯复用**（见 §8.1 #2/#3）。
- **正典口径**（worldview 实证）：§七 生物全是妖兽/寄生/清理程序，「凡兽」是留白但与「凡草/低灵物种」（§十七:1637）「凡器」（journey D:451）命名法自洽的新类目；§一:22 死域「连野兽都活不了」、§七:759 负灵域「野兽材质瞬间枯萎化为飞灰」是凡兽生态响应的正典明文。

## 设计原则：威胁谱系（用户定调 2026-07-02）

**所有生物都有威胁，只是威胁多少的问题——不存在无害背景板。** 对齐 worldview §725「这个世界的生物不是经验包，而是竞争者、寄生虫，或天道的清理程序」。凡兽与妖兽的区别是**威胁量级与驱动方式**，不是"有无威胁"的开关。每个物种带 `ThreatTier`：

| tier | 行为模式 | 物种（草案） | 实现件 |
|------|---------|------------|--------|
| T0 惊扰反抗 | **全自动避险**：对一切更高威胁（捕食者/妖兽/修士/敌对 NPC）无差别保持距离；被逼入死角/持续追打时啄咬踢一下（皮肉伤级），然后继续逃 | 鸡/兔/蛙 | 新 `FleeThreatScorer`（泛化避险，取代仅逃修士的单一 scorer）+ 新 `CorneredScorer` → 复用妖兽 `MeleeAttackAction` 单次 → 回 Flee |
| T1 被击反抗 | 平时游荡避开捕食者与修士；被攻击必反击冲撞，脱战后回避 | 牛/猪/羊 | `FleeThreatScorer`（阈值更高）+ `RetaliateScorer`（受击记仇窗口）→ Melee/冲撞 |
| T1.5 主动冲撞 | 领地内主动 ram 落单目标（vanilla 山羊语义） | 山羊 | 复用领地 `TerritoryIntruderScorer` 低权重版 |
| T2 掠食骚扰 | 猎小型凡兽与鼠类；伺机偷袭、对玩家试探性骚扰 | 狐 | `PredatorScorer→HuntAction`（复用妖兽件）+ 低权重玩家骚扰 |
| T2.5 群体掠食 | 群体饥饿驱动，猎 T0/T1/鼠类，**主动猎杀低境界玩家**——凡兽里的真威胁 | 狼 | Hunt 件 + `GroupCohesion` + 饥饿加权（`Hunger` 低→攻击性升）；对高境界玩家忌惮回避 |

**tier 之间构成食物链，且与既有妖兽层打通成一张食物网（活的生态，非各自为政）**——统一进**新建**的 `preys_on` 关系表（**不扩展**现有 `is_prey_of` 谓词——该谓词签名锁定 `(BeastKind, BeastKind)` 且吃 `realm_tier()`，凡兽无此维度，无法直接扩展给跨层用，§8.1 #2 定案净新增一张表）：

```
妖兽层（HybridBeast/AshSpider…）──猎──→ 凡兽全档（凡兽=妖兽的口粮，不只盯玩家）
狼(T2.5) ──猎──→ 兔/鸡/羊(T0/T1) + 噬元鼠     狐(T2) ──猎──→ 兔/鸡/蛙(T0) + 噬元鼠
T0 ──避──→ 上表一切（含捕食者/妖兽/修士/敌对 NPC）
```

（狼/狐猎噬元鼠是本食物网的合法边——鼠患的天敌来自凡兽层，不受下条限制。**唯一被 §8.1 #8 关闭的边是反方向**：噬元鼠**不**腐食凡兽尸体——鼠患的"吃"与凡兽的"死"不构成边，正典口径见 §8.1 #8。）

捕食是**真消费**：猎杀成功回补捕食者 `Hunger`（觅食环闭合），被猎倒的凡兽走 `fauna_drop_system` mundane 分支自动掉落（v1 不引入短命尸体实体，§8.1 #9）。T0~T2.5 全档不进 ambient-threat 的 danger 威胁预算统计（`counts_against_threat_budget=false`，威胁贡献≈0 但行为上有牙，§8.1 #3 定案）。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：Valence 原生 entity bundles（Rail A，`ZombieEntityBundle` 范式，非 `beast.rs` 的 `MarkerEntityBundle`）；commoner 三件套裁剪（`FearCultivatorScorer→FleeCultivatorAction` / `HungerScorer→FarmAction` / `WanderScorer→GoToPoiAction`，`npc/spawn/commoner.rs:26-32`）；`Hunger`/`Navigator`/`MovementController`；`TerrainProvider` biome 查询（`world/terrain/raster.rs:364-380`）；`Zone.spirit_qi`（栖息门槛只读）；`snap_spawn_y_to_surface`（`spawn/common.rs:219`）；`plan-ambient-threat-v1` 的 `AmbientMarkerData` trait + `AmbientSchedulerState<M>`/`AmbientSchedulerConfig<M>`/`ambient_scheduler_system::<M>`（`npc/spawn/ambient_scheduler.rs:502,534,564,604`，已 merge 就绪、`TestFaunaMarker` 回归测试钉死复用场景）。
- **出料**：凡兽死亡→已通产的 `fauna_drop_system`（`fauna/drop.rs:269`，侦听 `DeathEvent`）新增 mundane 掉落分支 → 写入 `DroppedLootRegistry`（`drop.rs:11,276`）→ `raw_beast_meat`/`raw_beast_hide`/凡骨 进 inventory → 接 food-v1 烹制 buff 链 + workbench 材料链；P3 出 `FaunaEcologySnapshotV1` 给天道 narration；ambient 凡兽存量成为 beast-horde 迁徙素材。`fauna/butcher.rs` 的 `ButcherRequest` 链当前无生产触发（仅测试 `send_event`），本 plan 不依赖它作主路径，降级为可选增强（§8.1 #9）。
- **共享类型 / event**：新 `MundaneFaunaKind` enum（**不复用敌对 `BeastKind`**——正典要求凡/妖严格分层，掉落表独立）+ 新 `MundaneFaunaMarker`（实现 `AmbientMarkerData`）；复用 `FaunaTag` 挂载模式思路、`DropEntry` 表结构（凡兽走平行的 `drop_table_for_mundane(kind: MundaneFaunaKind)`，不复用 `drop_table_for(BeastKind)` 本体）；新建跨层 `preys_on` 关系表（**不扩展 `is_prey_of`**——该谓词签名锁定 `(BeastKind, BeastKind)` 且吃 `realm_tier()`，凡兽无此维度，§8.1 #2 定案净新增）；spawn 调度**纯复用** `plan-ambient-threat-v1` 已 merge 的 `ambient_scheduler` 泛型核（3 步接入，见 P0），独立 passive pool，T0~T2.5 全档 `counts_against_threat_budget=false`（§8.1 #3 定案，不与威胁预算表联动）。
- **跨仓库契约**：P0~P2 纯 server（vanilla 渲染，client 零改动，Rail A 原生 bundle 免费复用原版模型/贴图/音效同 zombie）。P3 新增 `FaunaEcologySnapshotV1`（TypeBox schema + samples，仿 `BotanyEcologySnapshotV1` 模板 `agent/packages/schema/src/botany.ts`）→ agent `EcologyAnalyzer.ingest*` 同款管线。
- **worldview 锚点**：§十七 凡/低灵物种命名法；§一:22 + §七:759 灵气-生物存亡明文；journey D:443-455「MC 原生叠加不取代」哲学。「凡兽」类目本身是留白新增——若收口判定需正典条目（凡兽=无灵生物、末法残土的凡俗生态层），worldview 补丁单独 PR 人工 review。
- **qi_physics 锚点**：**凡兽设为无灵**——不吸灵气、不放灵气、死亡无 qi 释放（规避守恒面，正典自洽：凡=无灵）。负灵域灭杀是纯 despawn+VFX，无 QiTransfer。凡兽饮食走 `Hunger`（NPC 体力系统）与灵气无关。**本 plan 引入零个 qi 常数**。

---

## P0 凡兽底盘 ⬜

新模块 `server/src/fauna/mundane.rs`（+`npc/spawn/mundane.rs`），`register` 挂进 `fauna::register`（`fauna/mod.rs:20`）。**数值/接线细节见 §8.1（#1/#2/#3/#4）**，此处只列交付物：

- `enum MundaneFaunaKind { Cow, Pig, Sheep, Chicken, Rabbit, Goat, Frog, Fox, Wolf }`（9 变体终表，§8.1 #1 锁定不增删）+ `fn bundle_for(kind)` 映射 Valence 原生 `<X>EntityBundle` + `EntityKind::COW/...`——**照 `npc/spawn/zombie.rs:52` 的 `ZombieEntityBundle { kind: EntityKind::ZOMBIE, .. }` Rail A 头部，不照 `beast.rs:51` 的 `MarkerEntityBundle` custom visual 路数**。
- `MundaneFaunaMarker` 实现 `AmbientMarkerData`（`npc/spawn/ambient_scheduler.rs:502`，仅 `fn new(spawned_at, home_zone) -> Self` + `fn home_zone(&self) -> &str` 两函数）。
- `spawn_mundane_fauna_at(kind, pos, zone)`：照 `beast.rs:51` 的组件挂载骨架（`Navigator`/`MovementController`/`Hunger`/`WanderState`/`NpcPatrol`）换成 Rail A bundle 头部 + 新 `MundaneFaunaMarker { kind, spawned_at, home_zone }`；位置过 `snap_spawn_y_to_surface`（`spawn/common.rs:219`）。**组件清单补全（Debate major 修正）**：照 `zombie.rs:52-77`/`beast.rs:51-90` 先例，**必须**同时挂 `NpcMarker`（`npc/spawn/common.rs`，两个先例都在 spawn 元组内直接插入）+ 战斗运行时组件（`npc_runtime_bundle(entity, NpcArchetype::Mundane)` 或等价——内含 `Wounds`/`CombatState`，`npc/lifecycle.rs:584-617`）——`fauna_drop_system`（`drop.rs:269`）的查询硬要求 `With<NpcMarker>` 且触发源是 `DeathEvent`（依赖 `Wounds` 归零判定），漏挂这两类组件 = 凡兽物理上打不死、`DeathEvent` 永不触发、`fauna_drop_system` 静默不落地，整条资源链孤岛。despawn 一律 `insert(Despawned)`（[[feedback_valence_despawn_layer_entity]]）——超距回收天然满足：调度核 alive query 硬编 `Option<&RatBlackboard>/Option<&MimicSpiderBlackboard>`（`ambient_scheduler.rs:604-611`），凡兽不挂这些组件恒为 `None`，无需改调度核。
- **thinker**：commoner 三件套裁剪（`FearCultivatorScorer→FleeCultivatorAction` / `HungerScorer→FarmAction` / `WanderScorer→GoToPoiAction`，均在 `npc/brain.rs`）；P0 最小可用接 **`FleeThreatScorer`**（净新增，泛化版 `FearCultivatorScorer`：对捕食者/妖兽/修士/敌对 NPC 无差别避让）+ **净新增 `CorneredScorer`（P0 最小版，Debate major 修正）**→ 复用现成 `MeleeAttackAction`——big-brain 是 `Scorer→Action` 强绑定（`Thinker::build().when(Scorer, Action)`，见 `zombie.rs:34`/`beast.rs:34-40` 现成 thinker 写法），"复用 `MeleeAttackAction` 兜底"这句话本身不会让任何物种反击：`MeleeAttackAction` 不挂 Scorer 就永远不会被 picker 选中，会产出"全程只逃不反抗"的无害背景板，直接违反 [[feedback_threat_spectrum]]。`CorneredScorer` 最小语义：被追打且判定为"逼入死角"（复用 `Navigator` 寻路连续失败 / 简单距离墙判定，具体阈值 P0 实施时定）时评分升高触发单次 `MeleeAttackAction`，命中/脱险后评分回落，thinker 转回 `FleeThreatScorer` 继续逃（即 T0 惊扰反抗语义：逃为主，逼急了咬一口）。威胁谱系其余更复杂件（T1 `RetaliateScorer` 受击记仇窗口、T1.5 山羊冲撞、T2.5 狼群饥饿加权等）仍推迟到 P2 落地（§8.1 #3 已定案），但**最低一档反抗（`CorneredScorer`→`MeleeAttackAction`）必须在 P0 就有真实 Scorer 接线**，不能只写"复用 Action"这句话空转。**威胁谱系硬约束**（[[feedback_threat_spectrum]]）：即便 P0 最小实现，也不允许任何物种是无害背景板。
- **spawner 接入**（**3 步纯复用 `plan-ambient-threat-v1` 已 merge 的调度核，零改调度核代码**，见 §8.1 #2）：
  1. `MundaneFaunaMarker impl AmbientMarkerData`（同上，重复列出以强调这是接入第一步）；
  2. `app.insert_resource(AmbientSchedulerState::<MundaneFaunaMarker>::default())` + `app.insert_resource(AmbientSchedulerConfig::<MundaneFaunaMarker>::new(mundane_passive_budget_fn, mundane_pool_fn, false))`（`counts_against_threat_budget=false`，§8.1 #3）；
  3. `app.add_systems(Update, ambient_scheduler_system::<MundaneFaunaMarker>)`。
  `mundane_pool_fn: AmbientPoolFn`（第 3 参 `&Zone`，`ambient_scheduler.rs:557`）内部按 `zone.spirit_qi` 做死域/负灵域栖息门槛过滤（P2 落地，P0 先占位返回 `None`）+ 按 `TerrainProvider` biome 谓词（`world/terrain/raster.rs:364-380`：`is_peaks_biome`/`is_marsh_biome`/`is_spawn_biome`/`is_wastes_biome`）分池——平原/spawn：鸡/兔/猪/羊；沼泽：蛙/兔；峰区：山羊/羊；荒原：兔/狐。`mundane_passive_budget_fn`：`max_alive=3~4`（§8.1 #4 保守拍小）、`pack_size_range=(1,1)`、`spawn_interval_ticks` 复用 `threat_budget` 同档 stride。
- **测试**：kind→bundle 映射全 9 variant pin；`AmbientMarkerData` 两函数契约；分池按 biome 命中专属 case（4 biome × 命中物种集）；spawn 落地吸附；despawn 走 `Despawned`；`counts_against_threat_budget=false` 下与 `AmbientThreatMarker` 预算互不干扰（仿 `TestFaunaMarker` 回归模式，`ambient_scheduler.rs:1578-1967`）；密度上限/回收边界；`FleeThreatScorer` 触发 pin（有威胁在场时优先于 `HungerScorer`）；**新增（Debate major 修正）**：组件清单 pin（`spawn_mundane_fauna_at` 产出的实体带 `NpcMarker` + `Wounds`/`CombatState`，9 variant 全覆盖）；死链闭环 pin（凡兽被打至 `Wounds` 归零 → 触发 `DeathEvent` → `fauna_drop_system` 掉落分支被调用，非静默孤岛）；`CorneredScorer` 反击 pin（凡兽被逼入死角时选中 `MeleeAttackAction` 而非纯 `FleeThreatScorer`，反击后评分回落转回逃跑）。

## P1 资源链 ⬜

**修正（Explore 核验，2026-07-04）**：skeleton 原文误称 `beast_meat_v1`/`beast_blood_v1` shelflife profile "已定义未注册"——全仓 grep 零命中，从未存在；`ButcherRequest` 链也无生产触发（仅测试 `send_event`）。本节按实际代码现状重写。

- **凡兽资源产出走 `fauna_drop_system` 新增分支**（不依赖 `ButcherRequest`）：`fauna/drop.rs:269` 的 `fauna_drop_system` 已通产（侦听 `DeathEvent` → 写 `DroppedLootRegistry`，`drop.rs:11,276`）；新增 `drop_table_for_mundane(kind: MundaneFaunaKind) -> &'static [DropEntry]`（平行于 `drop_table_for(kind: BeastKind)`，`drop.rs:231`，不复用其 `BeastKind` 键）+ `roll_mundane_fauna_drops`，`fauna_drop_system` 按实体挂的是 `FaunaTag`（妖兽）还是 `MundaneFaunaMarker`（凡兽）分流两套查表逻辑。产出映射 `raw_beast_meat`（`fauna.toml:98`）/ `raw_beast_hide`（`fauna.toml:109`）/ **凡骨**（新 item `fan_gu`，凡俗材料档：可磨 `bone_meal_mat`/削 `bone_chip_mat` 进手搓台料链，**不可封灵**——正典硬约束：骨币料仅限异变兽骨，worldview §846-848；`bone_coin.rs:39` 的 `bone_grade_for_template` 不加凡骨档，天然对未知 template_id 返回 `None` 拒绝，`craft_rejects_non_bone_or_invalid_qi`（`bone_coin.rs:379`）测试模式沿用）。
- `fauna/butcher.rs` 屠宰工具链（工具决定产出/惩罚判定）**降级为可选增强**：若接，凡兽尸体走屠宰产出比走纯自动掉落获得加成权重；若不接，P1 不阻塞——纯掉落分支已是完整可用资源链（§8.1 #9）。
- 按物种微调掉落权重（鸡多肉少皮、牛皮厚、蛙无皮有腿肉→统一映射 meat 键）。
- **新建并注册 shelflife profile**（`shelflife/registry.rs`）：新定义 `raw_beast_meat_v1`（Spoil，half_life≈1 游戏日，仿 `food_spoil_mundane_meat_v1` 的 `Linear` 公式档次但更快腐败——生肉先于熟肉腐败是正典生活常识）/ `raw_beast_blood_v1`（Spoil，half_life≈12 小时，若血液作为独立可掉落资源则同步产出该 item），两者写进 `register_production_profiles`（`registry.rs:164`）；`fauna.toml:98` 的 `raw_beast_meat` item 定义**新增 `shelflife_profile = "raw_beast_meat_v1"` 字段**（当前无此字段）。熟肉沿用现有 `food.mundane.cooked_meat` + `food_spoil_mundane_meat_v1`（`registry.rs:107`），不改动。
- **测试**：凡兽掉落矩阵（9 物种 × 掉落表专属 case）；凡骨喂 bone_coin 制作被拒 + 原因文案（沿用 `craft_rejects_non_bone_or_invalid_qi` 模式）；两份新 shelflife profile 正反 sample + 腐败曲线 pin（生肉腐败快于熟肉）；`register_production_profiles` 计数断言同步 +2（比照 `registry.rs:368` 现有 "expected N profiles" 断言模式）。

## P2 生态响应 ⬜

- **栖息门槛**：`zone.spirit_qi <= 0`（死域）不刷凡兽、存量迁离或消亡（§一:22），接线点是 P0 占位的 `mundane_pool_fn`（`&Zone` 第 3 参直接读 `zone.spirit_qi`）；`REALM_COLLAPSE` zone 跳过。
- **负灵域灭杀**（§七:759 正典明文）：凡兽进入 `spirit_qi < -0.2` 负灵域 → 3s 枯萎 → despawn。**视听**：枯萎期复用灰烬色 `BongSpriteParticle`（burst 8 粒 + continuous 1 粒/4tick，lifetime 12 tick，色 `#6E6A5E` 残灰，自实体身躯向下沉降 0.02 b/t），消亡瞬间 `entity.wither.hurt` pitch 1.6 vol 0.4；经 `VfxBootstrap.registerDefaults()` 注册核对。narration 不出（低优先事件）。
- **跨层食物网落地**：新建**跨层 `preys_on` 关系表**（净新增，**不扩展** `is_prey_of`——`fauna/components.rs:121` 的 `is_prey_of(prey: BeastKind, predator: BeastKind) -> bool` 签名锁定 `BeastKind` 对且内部用 `realm_tier()` 比较，凡兽无此维度，§8.1 #2 定案）：
  - 狼/狐挂 `PredatorScorer→HuntAction`（复用妖兽领地 Hunt 件），猎物集含 T0/T1 凡兽**和 `BeastKind::Rat` 噬元鼠**——鼠患的天敌来自凡兽层（这条边合法，§8.1 #8 只关闭反方向的"鼠腐食凡兽"边，见下）；
  - **妖兽猎凡兽**：既有 `HybridBeast`/`AshSpider` 等的 Hunt 目标集扩进凡兽（走新 `preys_on` 表的跨层边，不碰 `is_prey_of` 本体）——妖兽不再只盯玩家，凡兽是它们的口粮，玩家能撞见"妖兽扑杀羊群"的生态事件；
  - **捕食闭环**：猎杀成功回补捕食者 `Hunger`（复用 `FarmAction` 回补路径换触发源）；被猎倒的凡兽走 P1 已接的 `fauna_drop_system` mundane 分支自动掉落（**不引入短命尸体实体**，§8.1 #9 定案）——玩家/其他捕食者对同一具凡兽掉落物"先到先得"降级为 P2 增强而非 P0/P1 阻塞项；
  - **鼠不腐食凡兽尸体**（§8.1 #8 定案，与"狼狐猎鼠"是不同的边、不冲突）：v1 噬元鼠**不**腐食凡兽尸体——正典鼠噬元（吃灵气，§七:727），凡兽无灵，若强行加"鼠吃肉不产 qi"特例会让 `RatBlackboard.drained_qi` 账目不自洽；腐食/清理凡兽尸体的生态位仅归狐（T2 掠食）。狼狐同时猎鼠（压制鼠患密度）与猎凡兽（口粮）两条边独立生效，生态自平衡。
  **狼群按威胁谱系 T2.5 落地**：饥饿加权攻击性（`Hunger` 越低越敢），群体成型（≥3）时主动猎杀醒灵/引气期玩家，对高境界忌惮回避（读 realm 的忌惮判定复用 `FearCultivatorScorer` 境界衰减逻辑反向用）；T2.5 狼/狐与 T0~T1 一样 `counts_against_threat_budget=false`（§8.1 #3），威胁靠 P0 的 `max_alive` 保守上限兜底，不进 ambient-threat 的 danger 密度统计。
- **季节权重**：接 §十七 季节生态——夏耐热物种权重升、冬耐寒（兔/山羊）升，读现有季节源（`season_success_modifier` 同源）。
- **测试**：死域/负灵域/塌缩 zone 三门槛；枯萎计时→despawn；`preys_on` 表全边 pin（含跨层：妖兽→凡兽、狼狐→T0/T1、狼狐→噬元鼠）+ 反向断言"`preys_on` 表内不存在噬元鼠→凡兽尸体的腐食边"（锁定 §8.1 #8 决议只关闭这一个方向、不误伤"狼狐猎鼠"边，回归测试同时验证两者共存）；捕食闭环（猎杀回补 Hunger、mundane 掉落分支产出）；Scorer 优先级（逃跑压过觅食、反抗压过逃跑的触发边界）；季节权重分布 pin。

## P3 天道信号（narration 级）⬜

- 新 `FaunaEcologySnapshotV1`（zone→物种 count，TypeBox + samples，仿 `BotanyEcologySnapshotV1`）；server 周期发布，agent `EcologyAnalyzer` ingest。
- **定位为 narration 信号源，非天道决策输入**：凡兽绝迹/兽群奔逃=灵气恶化的可感征兆（与既有「兽鸣偏向=伪灵脉信号」定位一致，`agent/packages/tiandao/src/skills/ecology.md`）。narration 示例（≥2，zone scope，perception style）：「近来林间鸟兽声稀，连野兔都不见踪影——这片地的灵气怕是伤了根本。」／「山羊群弃了北坡，成群往南迁——兽比人先知道哪里活不下去。」
- **测试**：schema 正反 sample；analyzer ingest 分支；空 snapshot 不产 narration。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **物种终表与末法化命名**：P0 草案 9 种是否增删（蜂/猫/马？马涉骑乘留白，倾向不进 v1）；narration/物品文案用名（灰羊/野彘/瘦骨鸡这类残土风 vs 直白原名）。
2. **spawner 基建归属与落地顺序**：`plan-ambient-threat-v1` P0 调度器（距离环/预算/回收）与本 plan P0 谁先落地——共享 `spawn/ambient_scheduler` 模块由先行者建、后者复用；两 skeleton 同期，需人工排序。
3. **T2+ 与 danger 密度口径**：狼/狐留本 plan 落地（已按威胁谱系定案），但 T2+ 掠食者是否计入 ambient-threat 的 zone 威胁密度统计（防止"凡兽狼群+妖兽预算"叠出超预期压力）——两 plan 收口时对齐同一张预算表。
4. **密度/上限数值**：每 biome 每 zone 存活上限、刷新间隔、pack 大小；与 dormant 5000 + ambient-threat 预算共存的实体峰值预算。
5. **畜牧后续 plan 登记**：驯化/圈养/繁殖若做，先补 worldview（聚集 vs 天道注视 §808 的论证）+ 另立 plan-husbandry-v1；本条只登记不实施。
6. **种群自维持**：v1 靠 spawner 补充（消亡即补）还是简单繁殖（幼崽 `initial_age_ticks=0` 接口已有）——倾向 v1 纯 spawner，繁殖归畜牧 plan。
7. **library 配套书**：`/write-book ecology` 补一本凡兽志（锚 §十七 低灵物种 + ecology/index.md「草木鱼虫皆在吃灵气」卷首语），归 P3 还是独立小 PR。
8. **噬元鼠腐食凡兽尸体的正典口径**：鼠的正典设定是噬元（吃灵气，§七:727），凡兽无灵——鼠腐食凡兽尸体是"吃肉"还是"舔食残存生机"？需一句正典自洽的解释（候选：末法鼠类退化出杂食性，灵肉皆噬），或改为鼠不碰凡兽尸体、腐食仅归狐。
9. **尸体窗口实现形态与种群平衡**：尸体是短命实体（3~5s despawn）还是复用 DroppedLoot 容器；捕食致死率/繁补速率的种群动态参数（防止狼把一个 zone 的 T0 吃绝或 T0 无天敌爆种群）——v1 用 spawner 补充兜底，参数收口时定。

**全部已在 §8.1 收口，实施以 §8.1 决议为准**。原表保留以备追溯。

## §8.1 决议（pre-P0 收口，2026-07-04）

> 决议依据：Explore agent 对 origin/main 实地核验（`ambient_scheduler.rs`/`fauna/components.rs`/`fauna/drop.rs`/`shelflife/registry.rs`/`fauna/bone_coin.rs`/`world/terrain/raster.rs`），非拍脑袋。核心利好：`plan-ambient-threat-v1`（PR #845）已 merge，留了 `TestFaunaMarker` 专属回归测试钉死本 plan 的复用场景——接入面绿灯，零集成风险。

### #1 物种终表与末法化命名

**决议**：
1. v1 锁定 9 种：`Cow/Pig/Sheep/Chicken/Rabbit/Goat/Frog/Fox/Wolf`，全部存在于 Valence 原生 entities（`valence_entity` OUT_DIR entity.rs），不增删（不进蜂/猫/马——马涉骑乘属 scope 外 §14）。
2. narration/物品文案用残土风别名（灰羊/野彘/瘦骨鸡等），但 `EntityKind`/`bundle_for` 映射走原版 `EntityKind::COW/PIG/...`——client 零改，vanilla renderer 免费渲染原版模型/贴图/音效（同 zombie NPC 先例）。别名只在 narration 字符串层，不影响协议层 kind。

**落点**：`server/src/fauna/mundane.rs`（新建，`enum MundaneFaunaKind` 9 variant + `fn bundle_for`）/ plan §P0 第 1 条。

### #2 spawner 基建归属与落地顺序

**决议**：
1. `plan-ambient-threat-v1`（PR #845，已 merge）已建好通用调度核，本 plan **纯复用**，不需要"谁先落地"的排序决策——归属问题已因既成事实关闭。
2. 3 步接入：① `MundaneFaunaMarker` 实现 `AmbientMarkerData` trait（`server/src/npc/spawn/ambient_scheduler.rs:502`，仅 `fn new(spawned_at: u64, home_zone: String) -> Self` + `fn home_zone(&self) -> &str`）；② `app.insert_resource(AmbientSchedulerState::<MundaneFaunaMarker>::default())` + `app.insert_resource(AmbientSchedulerConfig::<MundaneFaunaMarker>::new(mundane_passive_budget_fn, mundane_pool_fn, false))`（`ambient_scheduler.rs:534,564,575`）；③ `app.add_systems(Update, ambient_scheduler_system::<MundaneFaunaMarker>)`（`ambient_scheduler.rs:604`）。
3. 复用即验证：`TestFaunaMarker` 专属回归测试（`ambient_scheduler.rs:1578-1967`，注释明写"plan-mundane-fauna-v1 复用场景最小复现"）已钉死独立 `counts_against_threat_budget=false` 配置下与 `AmbientThreatMarker` 预算互不干扰——本 plan P0 实施时对照此测试模式补一份 `MundaneFaunaMarker` 专属版即可，不需重新设计验证方式。
4. 超距回收零改造：调度核 alive query 硬编 `Option<&RatBlackboard>/Option<&MimicSpiderBlackboard>`（`ambient_scheduler.rs:604-611`），凡兽实体不挂这两个组件，恒为 `None`，超距即走 `insert(Despawned)`，正是凡兽期望语义。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:502,534,557,564,575,604` / `server/src/fauna/mod.rs:20`（`register` 挂 3 行）/ plan §P0。

### #3 T2+ 与 danger 密度口径

**决议**：
1. v1 全部凡兽（含 T2/T2.5 狐/狼）走**独立 passive 预算**，`AmbientSchedulerConfig::<MundaneFaunaMarker>::new(.., .., false)` 中 `counts_against_threat_budget` 恒为 `false`——不进 `plan-ambient-threat-v1` 的 zone 威胁密度统计表，避免"凡兽狼群 + 妖兽预算"叠出超预期压力。
2. 拒绝"T2+ 计入 danger"路线的理由：跨 plan 共用同一张 `threat_budget` 表意味着两个 plan 的任何一方改动都要同步核对另一方，是持续性集成风险；而 P0 的 `mundane_passive_budget_fn` 保守 `max_alive` 上限已经能兜住狼群密度威胁，不需要接入 danger 统计换取额外收益。
3. 狼群的"真威胁"（T2.5 主动猎杀低境界玩家）靠 P0/P2 的 `max_alive` 保守上限 + `Hunger` 饥饿加权兜底，不靠威胁预算表限流。
4. 后续若要让狼计入 danger 密度，需求方另开对齐 PR 同时改两个 plan 的调度配置，本 plan 不预留接口占位（YAGNI，`counts_against_threat_budget` 参数本身已是完整开关，届时改一个布尔值即可）。

**落点**：`server/src/npc/spawn/ambient_scheduler.rs:567,575`（`counts_against_threat_budget` 字段与构造参数）/ plan §P0「spawner 接入」第 2 步 / §P2「狼群按威胁谱系 T2.5 落地」末句。

### #4 密度/上限数值

**决议**：
1. `mundane_passive_budget_fn(danger: u8) -> ThreatBudget`（签名对齐 `threat_budget`，`ambient_scheduler.rs:89`，但凡兽不看 danger 分级——保守起见对所有 danger 返回同一档）：`max_alive = 3`（v1 起步，P2 视实测调至 4）、`pack_size_range = (1, 1)`（调度核当前每次巡检产 1 个，多产未消费，v1 不需要 pack）、`spawn_interval_ticks` 复用 `threat_budget(3)` 同档 stride（400 ticks）。
2. 峰值预算与既有 `dormant` 5000 实体 + `AmbientThreatMarker` 预算共存：v1 单 zone 凡兽上限 3（9 物种 × 多 zone 才可能同时在场，实测峰值受 biome 分池天然稀释），暂不设跨 zone 全局硬顶——若实测显示服务器实体总量压力，P2 阶段补一个全局软顶（本决议不预留占位常量，避免过早引入未验证参数）。
3. P0 落地这组数值先偏保守（拍小原则），P2 生态响应阶段根据实测 TPS/密度反馈校准，不在 P0 引入可调参数暴露给运维（v1 硬编常量，调参归后续小 PR）。

**落点**：`server/src/fauna/mundane.rs`（新建 `mundane_passive_budget_fn`）/ 参照 `server/src/npc/spawn/ambient_scheduler.rs:76-128`（`ThreatBudget` 结构 + `threat_budget` 现有档位表）/ plan §P0「spawner 接入」。

### #5 畜牧后续 plan 登记

**决议**：
1. **关闭，不在本 plan 实施**：驯化 / 圈养 / 繁殖 / 挤奶维持"显式不做"状态。
2. 若后续要做，先补 worldview（论证畜牧聚集是否触发 §808「聚集触天道注视」机制、与 §862「无定居田园经济」空白的关系），再另立 `plan-husbandry-v1`——两步顺序不可颠倒（worldview 补丁需人工 review，不可由 plan 实施 agent 自动改）。
3. 本决议只是登记，不产生本 plan 内的任何交付物。

**落点**：plan 头部「显式不做（scope 外）」段（已有，本决议维持原判）/ 无代码落点。

### #6 种群自维持

**决议**：
1. v1 纯 spawner 补充（消亡即补，由 `ambient_scheduler_system` 的距离环/预算机制自然维持种群），**不做繁殖**。
2. 幼崽 `initial_age_ticks=0` 接口虽已在 `spawn_beast_npc_at` 存在（`beast.rs:56` 参数），但本 plan 的 `spawn_mundane_fauna_at` 不复用此参数做繁殖用途——繁殖判定/幼崽成长曲线归 `plan-husbandry-v1`（若立项）。

**落点**：`server/src/npc/spawn/mundane.rs`（新建 `spawn_mundane_fauna_at`，不含繁殖分支）/ plan §P0。

### #7 library 配套书

**决议**：
1. 不阻塞本 plan 收口——归 P3 顺手写（若 P3 实施 PR 有余量）或独立小 PR，两者皆可，不影响 promote。
2. 若归 P3：素材锚点 §十七 低灵物种命名法 + `docs/library/`（`ecology/index.md`「草木鱼虫皆在吃灵气」卷首语一致的调性）。

**落点**：`docs/library/`（未来 `/write-book ecology` 产出路径，不预先创建占位文件）/ plan §P3（可选追加一条）。

### #8 噬元鼠腐食凡兽尸体的正典口径

**决议**：
1. **v1 鼠不腐食凡兽尸体**——正典鼠噬元设定（吃灵气，§七:727）与凡兽无灵（本 plan「qi_physics 锚点」段）直接冲突：若强行让鼠"吃肉"，需要在 `RatBlackboard.drained_qi` 账目里加一条"吃肉但不产 qi"的特例，破坏该账目的自洽性（所有鼠的进食行为当前统一产 qi 记账）。
2. 拒绝"末法鼠类退化出杂食性"候选解释的理由：这是本 plan 单方面追加的正典分支，未经 worldview 人工 review，且会立刻要求 `RatBlackboard` 结构改动（超出本 plan 边界，`RatBlackboard` 归 `plan-ambient-threat-v1`/更早的 rat 系 plan 所有）。
3. 腐食/清理凡兽尸体的生态位改为**仅归狐**（T2 掠食者，已有独立猎杀行为，追加"腐食"分支不与其他系统账目冲突）。

**落点**：plan §P2「跨层食物网落地」第 5 条（已按本决议改写）/ `server/src/fauna/components.rs`（`RatBlackboard` 定义处，不改动——本决议即是"不碰"）。

### #9 尸体窗口实现形态与种群平衡

**决议**：
1. v1 **不引入短命尸体实体**：凡兽死亡直接走已通产的 `fauna_drop_system`（`server/src/fauna/drop.rs:269`，侦听 `DeathEvent`）→ 新增 mundane 掉落分支 → 写入 `DroppedLootRegistry`（`drop.rs:11,276`）自动掉落，与妖兽死亡掉落走同一条已验证生产链路，零新实体生命周期管理负担。
2. 拒绝"短命尸体实体（3~5s despawn）"路线的理由：需要新的 component（尸体 marker）+ 新的 despawn 定时器 system + 与 `fauna_drop_system` 之间"谁先处理死亡"的竞态设计，而 `fauna_drop_system` 已是唯一权威死亡→掉落路径，另起一条会造成"两套死亡处理逻辑"的孤岛风险。
3. "腐食 vs 屠宰先到先得"降级为 P2 可选增强（狐腐食可实现为"狐对 `DroppedLootRegistry` 中凡兽掉落物有概率优先拾取/销毁"，不阻塞 P0/P1）。
4. 种群防绝种/爆种：不设专门的捕食致死率/繁补速率动态参数——v1 依赖 P0 `mundane_passive_budget_fn` 的保守 `max_alive` 上限兜底（狼吃再多，spawner 按预算持续补充；T0 无天敌也不会爆种群，因为 `max_alive` 本身就是硬顶）。

**落点**：`server/src/fauna/drop.rs:231,269`（`drop_table_for` 旁新增 `drop_table_for_mundane` + `fauna_drop_system` 内分流逻辑）/ plan §P1「凡兽资源产出走 `fauna_drop_system` 新增分支」/ §P2「捕食闭环」。

### 视听（补充决议，非 §8 原表条目）

**决议**：Rail A 原生 bundle → vanilla client 免费渲染（原版模型/贴图/音效，client 零改，同 zombie 先例）；凡兽本体 v1 不产出任何新视觉资产。P2 负灵域枯萎复用既有 `BongSpriteParticle` + `entity.wither.hurt`（规格已在 §P2 写至可实现精度，不改动）。

**落点**：plan §P2「负灵域灭杀」段（已有完整规格）/ 无新增落点。

## §10（升 active 时补）

scope 预估 4 PR（P0~P3 各一）。P2 含视听但全复用既有粒子/音效原语，无新资产；P3 跨 server+agent 契约，samples 随 PR 同出。按 docs/CLAUDE.md §六补完整工作流。
