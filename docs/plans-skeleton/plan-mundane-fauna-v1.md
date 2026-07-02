# plan-mundane-fauna-v1 — 凡兽生态：把 1.20.1 原版动物用起来

> **一句话主题**：新增「凡兽」类目（牛/猪/羊/鸡/兔/山羊/青蛙/狐/狼等 MC 1.20.1 原版被动生物），走 Valence 原生 entity bundle（client 零改动、vanilla renderer 免费渲染），按 biome 分池游荡在残土上——补全「凡草/凡器/原生作物」叠加哲学缺失的动物侧：无灵、**低威胁但绝非无害**（威胁是谱系不是开关，见设计原则）、可猎（肉/皮/凡骨资源链复用现成屠宰系统），且作为灵气健康度的**可视化滞后指标**（死域无凡兽、负灵域入即枯萎化飞灰、凡兽绝迹=天道叙事的恶化征兆）。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 凡兽底盘——原生 bundle spawn + 被动 AI 三件套 + biome 分池 | ⬜ |
| P1 | 资源链——凡兽掉落表 / 屠宰复用 / shelflife 肉血 profile 注册 | ⬜ |
| P2 | 生态响应——qi 栖息门槛 / 负灵域灭杀 / 小型捕食链 / 季节权重 | ⬜ |
| P3 | 天道信号——FaunaEcologySnapshot 进 EcologyAnalyzer（narration 级） | ⬜ |

**显式不做（scope 外）**：驯化 / 骑乘 / 圈养 / 繁殖配对 / 挤奶——畜牧是 worldview 纯空白（§862 无定居田园经济、§808 聚集触天道注视），做则先补正典（单独人工 PR），另立 plan（§8 #5 登记）。

---

## 背景与调研结论（2026-07-02）

- **技术零门槛**：Valence 2b705351 生成代码含全部 1.20.1 被动生物 bundle（`CowEntityBundle`/`PigEntityBundle`/... 共 248 个，`valence_entity` OUT_DIR entity.rs）；zombie NPC 已走同款 Rail A（`npc/spawn/zombie.rs:52` `ZombieEntityBundle`），**协议层原生渲染，无需 fauna/visual.rs raw_id 对齐、无需客户端 GeckoLib 注册**。当前全库零处使用 `EntityKind::COW/PIG/SHEEP`——全新接入面。
- **AI 现成**：commoner thinker（`npc/spawn/commoner.rs:26-32`）即被动三件套——`FearCultivatorScorer→FleeCultivatorAction`（逃修士）+ `HungerScorer→FarmAction`（原地进食回补 `Hunger`，`npc/hunger.rs` 已注册）+ `WanderScorer→WanderAction`（`Navigator` 寻路游荡）。仅"怕捕食者"需新 scorer（`is_prey_of` 谓词已在 `fauna/components.rs:121`）。
- **资源链现成**：`raw_beast_meat`/`raw_beast_hide`（`server/assets/items/fauna.toml:97,108`）、屠宰链（`fauna/butcher.rs`，工具决定产出）、`food.mundane.cooked_meat`（`food.toml:14`）、手搓台的 `tanned_hide`/`bone_chip_mat`/`bone_meal_mat`（`workbench_materials.toml`）全部已存在。shelflife 的 `beast_meat_v1`/`beast_blood_v1` Spoil profile **已定义未注册**（plan-shelflife-v1 §342 M7 遗留，注册权归 fauna 系——本 plan P1 承接）。
- **地理数据现成**：`TerrainProvider.sample(x,z)` 运行时可查 `biome_id` + `is_peaks/is_marsh/is_spawn/is_wastes_biome` 谓词（`worldgen/raster.rs:364-380`），分池数据充分。
- **正典口径**（worldview 实证）：§七 生物全是妖兽/寄生/清理程序，「凡兽」是留白但与「凡草/低灵物种」（§十七:1637）「凡器」（journey D:451）命名法自洽的新类目；§一:22 死域「连野兽都活不了」、§七:759 负灵域「野兽材质瞬间枯萎化为飞灰」是凡兽生态响应的正典明文。

## 设计原则：威胁谱系（用户定调 2026-07-02）

**所有生物都有威胁，只是威胁多少的问题——不存在无害背景板。** 对齐 worldview §725「这个世界的生物不是经验包，而是竞争者、寄生虫，或天道的清理程序」。凡兽与妖兽的区别是**威胁量级与驱动方式**，不是"有无威胁"的开关。每个物种带 `ThreatTier`：

| tier | 行为模式 | 物种（草案） | 实现件 |
|------|---------|------------|--------|
| T0 惊扰反抗 | flee-first；被逼入死角/持续追打时啄咬踢一下（皮肉伤级），然后继续逃 | 鸡/兔/蛙 | 新 `CorneredScorer`（被贴身追击 N tick 起评）→ 复用妖兽 `MeleeAttackAction` 单次 → 回 Flee |
| T1 被击反抗 | 平时游荡避人；被攻击必反击冲撞，脱战后回避 | 牛/猪/羊 | `RetaliateScorer`（受击记仇窗口）→ Melee/冲撞 |
| T1.5 主动冲撞 | 领地内主动 ram 落单目标（vanilla 山羊语义） | 山羊 | 复用领地 `TerritoryIntruderScorer` 低权重版 |
| T2 掠食骚扰 | 猎小型凡兽为主；伺机偷袭、对玩家试探性骚扰 | 狐 | `PredatorScorer→HuntAction`（复用妖兽件）目标限小型凡兽 + 低权重玩家骚扰 |
| T2.5 群体掠食 | 群体饥饿驱动，**主动猎杀低境界玩家**——凡兽里的真威胁 | 狼 | Hunt 件 + `GroupCohesion` + 饥饿加权（`Hunger` 低→攻击性升）；对高境界玩家忌惮回避 |

T0~T1 不进 ambient-threat 的 danger 威胁预算统计（威胁贡献≈0 但行为上有牙）；T2+ 是否计入 danger 密度口径见 §8 #3。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：Valence 原生 entity bundles（Rail A）；commoner 行为件（Scorer/Action，`npc/actions_life.rs`）；`Hunger`/`Navigator`/`MovementController`；`TerrainProvider` biome 查询；`Zone.spirit_qi`（栖息门槛只读）；`snap_spawn_y_to_surface`（`spawn/common.rs:219`）。
- **出料**：凡兽尸体→`fauna/butcher.rs` 屠宰（肉/皮/凡骨）→ `raw_beast_meat`/`raw_beast_hide` 进 inventory → 接 food-v1 烹制 buff 链 + workbench 材料链；P3 出 `FaunaEcologySnapshotV1` 给天道 narration；ambient 凡兽存量成为 beast-horde 迁徙素材。
- **共享类型 / event**：新 `MundaneFaunaKind` enum（**不复用敌对 `BeastKind`**——正典要求凡/妖严格分层，掉落表独立）；复用 `FaunaTag` 挂载模式、`ButcherRequest` 事件、`DropEntry` 表结构；spawn 调度与 `plan-ambient-threat-v1` P0 调度器**共享基建、独立 pool**（T0~T1 不占威胁预算，T2+ 掠食者与 danger 密度口径对齐见 §8 #3；落地顺序见 §8 #2）。
- **跨仓库契约**：P0~P2 纯 server（vanilla 渲染，client 零改动）。P3 新增 `FaunaEcologySnapshotV1`（TypeBox schema + samples，仿 `BotanyEcologySnapshotV1` 模板 `schema/src/botany.ts`）→ agent `EcologyAnalyzer.ingest*` 同款管线。
- **worldview 锚点**：§十七 凡/低灵物种命名法；§一:22 + §七:759 灵气-生物存亡明文；journey D:443-455「MC 原生叠加不取代」哲学。「凡兽」类目本身是留白新增——若收口判定需正典条目（凡兽=无灵生物、末法残土的凡俗生态层），worldview 补丁单独 PR 人工 review。
- **qi_physics 锚点**：**凡兽设为无灵**——不吸灵气、不放灵气、死亡无 qi 释放（规避守恒面，正典自洽：凡=无灵）。负灵域灭杀是纯 despawn+VFX，无 QiTransfer。凡兽饮食走 `Hunger`（NPC 体力系统）与灵气无关。**本 plan 引入零个 qi 常数**。

---

## P0 凡兽底盘 ⬜

新模块 `server/src/fauna/mundane.rs`（+`npc/spawn/mundane.rs`），`register` 挂进 `fauna::register`（`fauna/mod.rs`）：

- `enum MundaneFaunaKind { Cow, Pig, Sheep, Chicken, Rabbit, Goat, Frog, Fox, Wolf }`（草案，终表 §8 #1）+ `fn bundle_for(kind)` 映射 Valence 原生 bundle + `EntityKind::COW/...`。
- `spawn_mundane_fauna_at(kind, pos, zone)`：照 `spawn_beast_npc_at` 骨架换 Rail A bundle，挂 `Navigator/MovementController/Hunger/WanderState/NpcPatrol` + 新 `MundaneFaunaTag { kind }`；位置过 `snap_spawn_y_to_surface`。despawn 一律 `insert(Despawned)`（[[feedback_valence_despawn_layer_entity]]）。
- **thinker**：commoner 裁剪版——`FearCultivatorScorer→FleeCultivatorAction` + `HungerScorer→FarmAction`（觅食）+ `WanderScorer→WanderAction`；羊/鸡加 `GroupCohesionScorer→RegroupAction`（复用 rat 群聚件）；**每个物种按 ThreatTier 挂对应反抗/反击件**（T0 `CorneredScorer`、T1 `RetaliateScorer`，攻击动作全部复用妖兽 `MeleeAttackAction`）——没有纯挨打的物种。
- **biome 分池 spawner**：`fn mundane_pool(biome_id, zone) -> &[(MundaneFaunaKind, weight)]`——平原/spawn：鸡/兔/猪/羊；沼泽（marsh）：蛙/兔；峰区（peaks）：山羊/羊；荒原（wastes）：兔/狐；调度器复用 `plan-ambient-threat-v1` P0 的「距离环 24~64 + 密度上限 + 超 96 格回收」基建（共享 or 先行独立见 §8 #2），**独立 passive pool 不占威胁预算**。
- **测试**：kind→bundle 映射全 variant pin；分池按 biome 命中专属 case；spawn 落地吸附；despawn 走 Despawned；密度上限/回收边界；**ThreatTier 全档反抗行为 pin**（T0 逼角触发/脱身回逃、T1 受击记仇窗口边界、无任何物种缺反抗件）。

## P1 资源链 ⬜

- 凡兽掉落/屠宰：`MundaneFaunaTag` 死亡→尸体→复用 `ButcherRequest` 链，产出映射 `raw_beast_meat` / `raw_beast_hide` / **凡骨**（新 item `fan_gu`，凡俗材料档：可磨 `bone_meal_mat`/削 `bone_chip_mat` 进手搓台料链，**不可封灵**——正典硬约束：骨币料仅限异变兽骨（worldview §846-848），`plan_bone_coin_craft` 的 `BoneGrade` 不加凡骨档，喂凡骨显式拒绝 + 拒绝原因）。
- 按物种微调产出权重（鸡多肉少皮、牛皮厚、蛙无皮有腿肉→统一映射 meat）。
- **注册 shelflife profile**：`beast_meat_v1`（half_life≈1d）/`beast_blood_v1`（≈12h）正式注册（承接 plan-shelflife-v1 §342 M7 遗留），生肉走 Spoil、熟肉沿用 `food.mundane.cooked_meat` 现有 profile。
- **测试**：屠宰产出矩阵（物种×工具）；凡骨喂 bone_coin 制作被拒 + 原因文案；shelflife profile 生效（腐败曲线 pin）；徒手屠宰惩罚沿用。

## P2 生态响应 ⬜

- **栖息门槛**：`zone.spirit_qi <= 0`（死域）不刷凡兽、存量迁离或消亡（§一:22）；`REALM_COLLAPSE` zone 跳过。
- **负灵域灭杀**（§七:759 正典明文）：凡兽进入 `spirit_qi < -0.2` 负灵域 → 3s 枯萎 → despawn。**视听**：枯萎期复用灰烬色 `BongSpriteParticle`（burst 8 粒 + continuous 1 粒/4tick，lifetime 12 tick，色 `#6E6A5E` 残灰，自实体身躯向下沉降 0.02 b/t），消亡瞬间 `entity.wither.hurt` pitch 1.6 vol 0.4；经 `VfxBootstrap.registerDefaults()` 注册核对。narration 不出（低优先事件）。
- **小型捕食链**：狼/狐挂 `PredatorScorer→HuntAction`（复用妖兽领地 Hunt 件）猎兔/鸡；被猎物种新 `FleePredatorScorer`（`is_prey_of` 谓词已有，scorer 新写）。**狼群按威胁谱系 T2.5 落地**：饥饿加权攻击性（`Hunger` 越低越敢），群体成型（≥3）时主动猎杀醒灵/引气期玩家，对高境界忌惮回避（读 realm 的忌惮判定复用 `FearCultivatorScorer` 境界衰减逻辑反向用）。
- **季节权重**：接 §十七 季节生态——夏耐热物种权重升、冬耐寒（兔/山羊）升，读现有季节源（`season_success_modifier` 同源）。
- **测试**：死域/负灵域/塌缩 zone 三门槛；枯萎计时→despawn；捕食链 Scorer 优先级（逃跑压过觅食）；季节权重分布 pin。

## P3 天道信号（narration 级）⬜

- 新 `FaunaEcologySnapshotV1`（zone→物种 count，TypeBox + samples，仿 `BotanyEcologySnapshotV1`）；server 周期发布，agent `EcologyAnalyzer` ingest。
- **定位为 narration 信号源，非天道决策输入**：凡兽绝迹/兽群奔逃=灵气恶化的可感征兆（与既有「兽鸣偏向=伪灵脉信号」定位一致，`skills/ecology.md:52`）。narration 示例（≥2，zone scope，perception style）：「近来林间鸟兽声稀，连野兔都不见踪影——这片地的灵气怕是伤了根本。」／「山羊群弃了北坡，成群往南迁——兽比人先知道哪里活不下去。」
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

## §10（升 active 时补）

scope 预估 4 PR（P0~P3 各一）。P2 含视听但全复用既有粒子/音效原语，无新资产；P3 跨 server+agent 契约，samples 随 PR 同出。按 docs/CLAUDE.md §六补完整工作流。
