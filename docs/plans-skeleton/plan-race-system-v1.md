# plan-race-system-v1 — 通用身体构型（BodyPlan）与种族系统 + 固元「易形」功法

一句话主题：把「部位 / 经脉 = 人形硬编码」重构为按种族（BodyPlan）数据驱动的通用系统——每种 Entity 可定义各自的部位集合与经脉拓扑；装备 / 功法接入种族三档匹配（种族专属 / 人形通用 / 全通用）；固元境经残卷解锁「易形」功法（外观一对一互换、部位经脉不变），为玩家可扮演非人种族（如飞鲸）铺路。

> **状态：骨架（skeleton）**。升 active 前必须完成 §8 开放问题收口（补 §8.1 决议），其中 #1（零出生论冲突）、#2（机制命名）、#5（易形后 gate 判定基准）需用户拍板。
>
> 机制暂名说明：用户原始需求称「化形功法」，但「化形」在仓库已被两处占用（剑修「剑意化形」= manifest 实体化；丹道材料「化形根 / 化形大丹」），本 plan 全文暂用**「易形」**，最终命名见 §8 #2。

## 阶段总览

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 种族 / 构型底盘：`BodyPlanRegistry` 数据驱动 + `Race` 字段 + 战斗部位消费点改造 | ⬜ |
| P1 | 经脉系统通用化：`MeridianSystem` 去定长 + per-plan 拓扑与境界配额 + wire 开放化 | ⬜ |
| P2 | 动态部位 / 经脉面板：server 下发布局元数据，client 剪影与经脉图数据驱动 | ⬜ |
| P3 | 装备 / 功法种族三档匹配：`RaceGate` 三收拢点接线 + UI 反馈 | ⬜ |
| P4 | 易形功法：固元残卷解锁、外观一对一互换、玩家渲染替换链路 | ⬜ |
| P5 | 非人种族玩家入口 + 飞鲸 MVP 数据 + bot 场景 / e2e 收口 | ⬜ |

## 接入面（docs/CLAUDE.md §二）

- **进料**：
  - `cultivation`：`Realm`（固元 = `Solidify`，`components.rs:15-22`）、`MeridianSystem`（`components.rs:246`）、`BreakthroughOutcome` 事件（`breakthrough.rs:142-147`）、`Cultivation` 组件持久化 bundle（race 字段搭车）
  - `combat`：`BodyPart` / `Wounds`（`combat/components.rs:32-73`）、`classify_body_part` / `standing_humanoid_aabb`（`combat/raycast.rs`）、`body_part_multipliers`（`combat/resolve.rs:1911-1920`）、`ArmorProfile.body_coverage` 数据驱动先例（`combat/armor.rs`）
  - `inventory`：`ItemTemplate`（`inventory/mod.rs:120-160`，TOML 驱动）、`validate_equip_to` 唯一穿戴校验入口（`inventory/mod.rs:5349-5509`）
  - `fauna / npc`：`BeastKind`（含 `Whale`，`fauna/components.rs:9-33`）、`NpcArchetype`（`npc/lifecycle.rs:91-158`）、道伥 / 拟态蛛 disguise 协议骨架（`network/daozhan_disguise_emit.rs`）
  - `skin`：MineSkin 管线 + `EntityKind::PLAYER` NPC 先例（`npc/spawn/common.rs:279-322`，易形反向「异兽→人形」复用）
- **出料**：
  - client 面板：`cultivation_detail` / `wounds_snapshot` 既有 payload + 新增 `body_plan_layout` payload → `MiniBodyHudPlanner` / `BodyInspectComponent` 数据驱动重构
  - 功法 / 装备 gate：`learn_technique_if_allowed`（`technique_scroll.rs:77-111`）、`handle_skill_bar_cast`（`client_request_handler.rs:10092-10264`）、`validate_equip_to` 三处新增 `RaceGate` 校验
  - 易形状态：新 `morph` payload（协议骨架仿 daozhan disguise）→ client 玩家渲染替换（`FakePlayerRendererMixin` 激活重写 + `BongEntityModelKind` geo 模型）
- **共享类型 / event**：复用 `BreakthroughOutcome`（不另造突破事件）、`EquipSlotV1`（`schema/inventory.rs:39-54`）、`ScrollReadOutcome` / `InventoryMoveRejectReason`（各加 `RaceMismatch` 变体）、`BongEntityModelKind` raw-id 模型注册表；**不复用** dandao `MutationKind/BodySlot` 表达种族（理由见 §7）
- **跨仓库契约**：proto `MeridianId`（`common.proto:23-47`）与 `CombatBodyPartV1`（`combat_event.rs:5-14`）开放化为 string id；TS `agent/packages/schema/src/cultivation.ts:55-80` / `combat-event.ts:19-29` 同步；client `MeridianChannel.java` / `BodyPart.java` / `ClientRequestProtocol.java` 同步；新增 `BodyPlanLayoutV1` / `MorphStateV1` schema + samples 双端对拍
- **worldview 锚点**：§一境界（固元 `worldview.md:70`）、§六零出生论（`:590` / `:719`，冲突处理见 §8 #1）、上古大能（`:49` / `:292` / `:1383`）、残卷传承（`:890`）、异兽词系（`:735` / `:892` / `:1580`）。涉及 worldview 增补（种族 / 易形正典化）必须**单独 PR 人工 review**
- **qi_physics 锚点**：本 plan 不新增任何物理常数。易形施放的 qi 消耗走既有 skill cast 扣费路径（含 zone 归还，qi 守恒清扫后的现行管线）；经脉容量对 qi_max 的贡献沿用 `MERIDIAN_CAPACITY_ON_OPEN` 既有常数（`meridian_open.rs:140-141`），per-plan 只改「哪些脉、几条」不改「每条值多少」

## 0. 调研摘要（2026-07-10，5 路并发核验，file:line 基于当日 origin/main）

**部位系统全链硬编码人形**：server `BodyPart` 8 段 unit enum（`combat/components.rs:32-41`）；命中判定唯一 hitbox 是 1.8m 人形 AABB（`raycast.rs:27-28,163-176`）+ 人体比例高度分类（`raycast.rs:193-236`，投射物同用 `carrier.rs:963-967`）；部位倍率 8 分支 match（`resolve.rs:1911-1920`）；臂 / 腿伤残废模块假设两臂两腿（`arm_wound.rs` / `movement/leg_wound.rs`，`MAIN_ARM=ArmR`）；client 16 段人形剪影 + 部位像素坐标全硬编码（`MiniBodyHudPlanner.java:117-181,365-384`、`BodyInspectComponent.java:283-314`）；server 7 段 → client 16 段 wire 映射写死（`wounds_snapshot_emit.rs:86-98`）。**唯一数据驱动先例**：护甲 `ArmorProfile.body_coverage` 从 `server/assets/combat/armor_profiles/*.json` 加载（`armor.rs:198-248`）——本 plan registry 模式范本。无 per-part HP（`Wounds.health_current` 全局单池），飞鲸已存在但被当 1.8m 直立人打（`spawn_whale.rs:297-298`）。

**经脉系统 20 条写死、镜像 5 处**：`MeridianId` 20 变体（12 正经 + 8 奇经，`cultivation/components.rs:66-89`），镜像 proto（`common.proto:23-47`）/ TS（`cultivation.ts:55-80`）/ client（`MeridianChannel.java` + `ClientRequestProtocol.java`）；`MeridianSystem` 定长数组 `[Meridian;12]+[Meridian;8]`（`components.rs:246-250`）物理装不下异种经脉数；拓扑单张 TCM 图（`topology.rs` `standard()`，注释已埋「后续可扩展异体」但无 hook）；境界配额写死 1/3/6/12/16/20 + 正奇子配额（`components.rs:37-46`、`breakthrough.rs:311-372`）；client 经脉图为硬编码人形折线（`BodyInspectComponent.java:827-912` `MERIDIAN_PATHS`）；玩家 / NPC / 凡兽均挂经脉（`npc/technique.rs:109-121`、`fauna/mundane.rs`）。经脉↔部位既有映射两处：`dugu.rs:532-540`（伤口污染注入）、`baomai_v4/dead_armor.rs:206-229`（死脉甲免疫）。

**功法 / 装备 / 持久化——种族纯增量**：全仓无任何 race/species 玩家概念。`TechniqueDefinition` 48 条 Rust const（`known_techniques.rs:100-114,133-934`）已有 `required_realm` + `required_meridians` 双门；习得唯一收拢点 `learn_technique_if_allowed`（`technique_scroll.rs:77-111`，scroll / 拜师 / 偷师 / insight 全走此处）；施放收拢点 `handle_skill_bar_cast`（`client_request_handler.rs:10092-10264`，拥有门 :10128-10140 → 经脉门 :10166）；穿戴唯一入口 `validate_equip_to`（`inventory/mod.rs:5349-5509`），现存唯一「谁能穿」先例是 false_skin 境界门散落特例（`client_request_handler.rs:11097-11130`，反面教材）。`Cultivation` 组件整体序列化进 `cultivation_json` bundle（`persistence/mod.rs:5324-5387`）→ **race 字段加 `#[serde(default)]` 即自动持久化，persistence 层零改动**。

**外观 / 化形链路——协议可抄、渲染空白**：物种全 enum 硬编码（`NpcArchetype` 12 族 + `BeastKind` 16 种）；现有 disguise 是「同实体换贴图」应用层旗标（道伥披 Steve 皮 / 拟态蛛披灰烬贴图），只作用于 NPC，从未碰玩家实体；协议骨架可复用（版本化 JSON payload + entity-id 集 + 周期全量 sync + 触发时 64 格半径广播）；`BongEntityModelKind` bbmodel raw-id 注册表现成（含 `expectedRawId` 运行时断言）；「NPC 以 `EntityKind::PLAYER` + MineSkin 装成玩家」反向先例已通。**四大空白**：玩家本体渲染替换 mixin（`FakePlayerRendererMixin` 是未注册空占位）、FPV 处理、动态碰撞箱、动物贴图通道（GameProfile texture 与 `textureForState` 两套材质系统不通）。

**撞车面与正典红线**（详见 §7 / §8）：dandao `MutationState/BodySlot/MutationStage(Bestial)` 是既有「身体改造」系统必须声明关系；「化形」「妖兽」两词违典 / 撞名；零出生论（`worldview.md:590/719`）拦「创建期选种族」；传承断绝设定拦「上古功法完整流传」。

## P0 — 种族 / 构型底盘

**交付物**：

- 新模块 `server/src/body_plan/`：
  - `struct BodyPlan { id: BodyPlanId, display_name, is_humanoid: bool, parts: Vec<BodyPartDef>, hit_geometry: HitGeometry, equip_slots: Vec<EquipSlotV1>, meridian_profile: MeridianProfile(P1 填), morph_pairs(P4 填) }`——`is_humanoid` 是 P3 `RaceGate::Humanoid` 档的唯一判据（不做名单硬编码）
  - `struct BodyPartDef { id: BodyPartId(string id), damage_mul, contam_mul, bleed_mul, consequence: PartConsequence }`——`PartConsequence` 枚举化现有「腿伤减速 / 头伤眩晕 / 臂伤六维」后果语义（`Locomotion / Sensory / Manipulator{main_hand} / Core`），非人形部位挂同枚举（如鲸尾鳍 = Locomotion）
  - `enum HitGeometry { HeightBands { aabb, bands, lateral_threshold }, PartBoxes(Vec<PartBox { part_id, offset, half_extents, priority }>) }`——`HeightBands` 是现 `classify_body_part` 高度带 + 横向阈值的参数化（humanoid 用，行为 bit-for-bit）；`PartBoxes` 逐部位局部 AABB、射线逐盒求交取最近命中（非人形用——单一直立 AABB + 人体比例高度带表达不了飞鲸横长构型，P5 whale 必须走此模式）
  - `BodyPlanRegistry` 从 `server/assets/body_plans/*.json` 加载，模式仿 `combat/armor.rs::load_dir`（重复 id / 缺字段 fail-fast）
- `server/assets/body_plans/humanoid.json` 首个条目：8 部位 / 倍率表 / 1.8m AABB 分段阈值 / 4 护甲槽，**与现状硬编码值逐项 bit-for-bit 对齐**
- `Race` 表示：`Cultivation.race: RaceId`（`#[serde(default)] = "human"`，RaceId → BodyPlanId 映射）；NPC / fauna 侧 `BeastKind → BodyPlanId` 派生函数（默认全部落 humanoid，P5 起给 whale 换）
- 战斗消费点改造（行为回归不变）：`body_part_multipliers` / `classify_body_part` / `standing_humanoid_aabb` / `carrier.rs` 投射物分支改为查询目标实体 BodyPlan；`arm_wound` / `leg_wound` 后果分派改走 `PartConsequence`
- **测试**：registry 加载饱和（happy / 重复 id / 缺字段 / 空目录）；humanoid 对拍 pin（逐部位倍率与 `resolve.rs:1911` 旧表全等断言）；raycast 分类回归（P1 直方图样本重跑）；`PartConsequence` 每变体专属 case

## P1 — 经脉系统通用化

**交付物**：

- `MeridianSystem` 定长数组 → `Vec<Meridian>` keyed by `MeridianChannelId`（string id；人形保留现 20 条 id 的 snake_case 字符串形态）；`MeridianId` enum 降级为 humanoid 常量集（`ALL` 从 humanoid plan 派生），存量代码引用逐步换 id
- `MeridianProfile`（进 body_plan json）：`channels: Vec<ChannelDef>`、`topology_edges`（替换 `topology.rs` 单张 standard 图）、`realm_requirements: [RealmMeridianReq; 6]`（人形曲线 1/3/6/12/16/20 + 正 / 奇配额搬进 humanoid.json；`Realm::required_meridians()` / `breakthrough_precondition_error` 改查 plan）
- NPC 经脉生成参数化：`npc_meridian_system_for_realm` 按实体 BodyPlan 生成
- wire 开放化：proto `MeridianId` enum → string channel id，**直接改新形状不留兼容层**（TS union / `MeridianChannel.java` / `ClientRequestProtocol.java` / `samples/*.json` 一次改齐）；`cultivation_detail` SoA 数组随 plan 变长（`cultivation_detail_emit.rs:51-140`），emit 时附 channel id 序
- `CombatBodyPartV1` 同批开放化为 string part id（`combat_event.rs:5-14` / `combat-event.ts:19-29`；`WoundEntry.part` proto 侧已是 string 不动），与 MeridianId 收进同一只 wire PR
- **旧存档显式迁移**：`cultivation_json` bundle 的 `meridians` 字段旧形态是定长 `[Meridian;12]+[Meridian;8]`，Vec 化后直接反序列化必崩——bundle 版本号 bump + 迁移函数（旧数组按 `REGULAR`/`EXTRAORDINARY` 序注入 humanoid channel id → 新 Vec 形态），`MeridianSeveredPermanent` 同批迁移；测试用真实 v31 存档 dump 的 bundle 样本对拍逐脉状态，禁止只测新形状
- 经脉↔部位映射数据化：`dugu.rs:532-540` 与 `baomai_v4/dead_armor.rs:206-229` 两张 match 表并入 humanoid plan 的 `ChannelDef.body_part` 字段（防第三张私表）
- **测试**：humanoid plan 下全消费点回归对拍（tick 吐纳 / breakthrough 配额 / severed 门控 / burst_meridian / baomai 邻接）；非人 plan 合成样本（6 脉构型）走通开脉 → 突破配额 → severed 全链；schema 正反 sample 对拍；`MeridianChannelId` 未知 id fail 分支

## P2 — 动态部位 / 经脉面板渲染补强

**交付物**：

- 新 payload `ServerDataPayloadV1::BodyPlanLayout`（`BodyPlanLayoutV1`）：以 `body_plan_id` 为主键，含部位剪影多边形顶点（归一化坐标）+ 部位锚点（伤口红点位）+ 经脉折线路径（替代 `MERIDIAN_PATHS` 硬编码）+ server 部位 id → 展示段映射（替代 `body_part_wire` 7→16 写死映射）。join 首帧随 `cultivation_detail` 下发；`cultivation_detail` 附带自身 `body_plan_id` 供 client 按 plan id 寻址缓存；实体 plan 变化（真实换 race）时重发，易形不触发（外观 only）
- 布局数据源：humanoid 布局从现 `MiniBodyHudPlanner` / `BodyInspectComponent` 硬编码坐标**原样抽取**进 `server/assets/body_plans/humanoid_layout.json`（首版渲染与现状像素级一致）
- client 重构：`BodyPlanLayoutStore`（仿 `MeridianStateStore` 单快照 + listener）；`MiniBodyHudPlanner` / `BodyInspectComponent`（剪影 + 经脉图 + `locatePart`）/ `WoundLayerBinding.resolvePart` 全改读 store；无 layout 时 fallback humanoid（老 server 兼容不需要——同版本发布，fallback 仅防御首帧竞态）
- 破损护甲裂纹 / 丹药部位框逻辑随部位 id 走（`body_part_resist:` status id 前缀机制保持）
- **测试**：schema 正反 sample；humanoid layout 渲染回归（`client/tools` render harness 截图对拍现状）；合成非人 layout（6 段构型）渲染不越界 / 红点落锚点；缺段 / 冗余段 wire 容错

## P3 — 装备 / 功法种族三档匹配

**交付物**：

- `enum RaceGate { Any, Humanoid, Species(Vec<RaceId>) }`（`body_plan` 模块，紧邻 RaceId）——三档语义：全通用 / 人形通用（所有 humanoid 构型种族可用）/ 种族专属；`Humanoid` 档判据 = 目标形态 BodyPlan 的 `is_humanoid` 字段（P0），不做种族名单硬编码
- 功法侧：`TechniqueDefinition.required_race: RaceGate`（48 条存量补默认 `Any`，首批 Humanoid 划定见 §8 #6）；习得门插 `learn_technique_if_allowed`（境界门后，新增 `ScrollReadOutcome::RaceMismatch`）；施放门插 `handle_skill_bar_cast` 拥有门后经脉门前（sword_path resolver 路径镜像一份，`skill_register.rs:862-877` 旁）
- 装备侧：`ItemTemplate.wearer_race: RaceGate`（TOML 可选字段默认 any）；校验统一进 `validate_equip_to`（槽位分支判定后、`Ok(())` 前），新增 `InventoryMoveRejectReason::RaceMismatch`；false_skin 散落境界门**不动**（不同轴，境界≠种族）
- client UI 反馈：不匹配装备格 / 功法条目置灰 + 点击 toast 带原因（对齐 #663 灰按钮 toast 先例）；种族不可用的功法 / 装备**不出现在推荐位**（HUD conditional display 原则）
- gate 数据下行：client 置灰不靠猜——功法列表 payload 与物品 wire 数据补 `required_race` / `wearer_race` 字段（proto / TS / samples 同改），client 侧结合自身 race + 当前形态本地判定；server 端 `validate_equip_to` 仍是权威（client 判定仅 UX 预览）
- wire：reject reason 新变体过 proto / TS / samples；`RaceGate` serde 正反 pin
- **测试**：三档 × 三收拢点矩阵全覆盖（9 组合 happy + 拒绝）；`RaceMismatch` 双 reason 枚举 wire pin；48 条存量功法默认值不回归（全 `Any` 断言防漏标）；bot 场景：race gate 拒绝穿戴回执

## P4 — 易形功法（暂名，固元境解锁）

**交付物**：

- 解锁路径（对齐残卷传承正典 `worldview.md:890`）：新残卷物品 `yixing_scroll`（TOML 进 `server/assets/items/`，`technique_scroll_spec.skill_id = "morph.yixing"`），掉落面：坍缩渊遗迹 / 道伥 / 高危遗迹容器；`TechniqueDefinition` 新增 `morph.yixing`（`required_realm = "Solidify"`，required_meridians 含 Ren+Du，理由：奇经主形）——**不改突破核心**，固元门槛由既有 `required_realm` 习得门天然承担
- 故事线锚点：功法描述文案引太初志「灵脉化形」存疑传闻作暗线（保留传闻外壳，不写实设定）；上古大能仅以「残卷来历不可考，笔迹类上古断代」措辞出现，不写「完整流传」
- server：`MorphState { form: RaceId, model_kind: u16, since_tick }` 组件；cast 走既有 skill 管线（qi_cost / cooldown / cast_ticks 走 `TechniqueDefinition` 既有字段，扣费归还走现行守恒路径）；一对一映射表 `morph_pairs: Vec<{from_race, to_race, model_kind, part_mapping: Vec<{from_part, to_part}>}>` 进 body_plan json（人↔指定异兽形，非任意变；`part_mapping` 是两构型部位的一对一对应表，装备折算的依据）；死亡 / 濒死 / 下线自动解除
- **易形语义收敛（四层拆清，防「外观 only」自相矛盾）**：易形改变的是「外观 + 装备形态层」，不变的是「命中部位 / 经脉 / 碰撞箱 / 功法门」。具体：受击仍打本体 `HitGeometry`、伤口落本体部位（协议 hitbox 不动，绕开 valence 动态 EntityDimensions 深水区）；但**装备槽集合与 `validate_equip_to` 的 RaceGate 按当前形态（form plan）判定**——这就是「易形化人后可穿人形装备」的成立方式；form 形态所穿护甲的 `body_coverage` 经 `part_mapping` 折算回本体部位参与减免结算，无对应部位的 coverage 不生效
- 解除易形（手动 / 死亡 / 下线）时，不再满足本体 gate 的装备**自动卸入背包**（复用非空背包连货卸下路径），装不下走 dropped_loot——测试锁死「解除后无非法装备残留」
- 协议：新 payload `morph_state`（仿 daozhan disguise 骨架：版本化 JSON + join 全量 + 周期 sync + 变形瞬间 64 格半径广播），字段 `{v, entries: [{entity_id, model_kind}]}`；client 缓存以 entity_id 为键，周期 sync 为**全量替换非增量**——respawn / 维度切换 / 重连后的 entity_id 漂移靠下一拍全量自愈，不做增量补丁协议
- client 渲染替换（本 plan 最大空白区，独立 PR）：
  - 激活并重写 `FakePlayerRendererMixin`：命中 morph 缓存的玩家实体 suppress vanilla `PlayerEntityModel` → 渲染 `BongEntityModelKind` 对应 geo 模型（fauna 渲染器已现成）；反向（异兽 NPC 化人形）复用 MineSkin + `EntityKind::PLAYER` 先例
  - FPV：第一版保持 vanilla 手臂不变（明确 scope，兽形 FPV 无先例不做）；TPV 自看生效
  - 名牌 / 队友标识保留（防 grief 误伤，对齐 disguise 半径过滤精神）
- 视听规格（active 前按 docs/CLAUDE.md 红线细化，此为骨架草案）：
  - 粒子：`BongRibbonParticle` 螺旋环绕 24 条、lifetime 30t、自下而上、#E8DFC8；`BongSpriteParticle` 白雾 burst 40 个、lifetime 20t、radial、复用 `mist` 贴图；`vfx_event = "bong:morph_yixing"`，新 `MorphVfxPlayer`
  - 音效：`audio_recipe yixing_cast.json`——layer1 `entity.evoker.prepare_wololo` pitch 0.8 vol 0.6 delay 0；layer2 `entity.illusioner.mirror_move` pitch 1.2 vol 0.5 delay 8t；layer3 `block.amethyst_block.chime` pitch 0.7 vol 0.4 delay 24t
  - HUD：左下形态图标（仅易形中显示，`HudRenderLayer` 新 morph 槽；结束淡出 10t）；施法期 vignette #FFFFFF opacity 0.15 fade-in 8t / fade-out 12t
  - 动画：`morph_cast.animation.json` endTick 30，蓄力鞠躬（torso pitch 0.35rad + legs 同向 0.35rad + body.z 前移 1.2，easeInOutQuad）
  - narration 示例（scope: zone / style: perception）：「灵光一敛，那道人影的轮廓塌了下去——再抬眼时，已是一头异兽伏在原地」；「你看见一头异兽的骨相在雾里折叠、拉长，最后立成了人形」
- **测试**：morph 状态机全转换（学→cast→active→手动解除 / 死亡解除 / 下线解除 / 重复 cast 幂等）；gate 矩阵（未习得 / 境界不足 / 经脉断 / 非法 pair 全拒）；payload 双端 sample + proto_min bot 解码；渲染 harness：morph 玩家截图（TPV 兽形 + 名牌保留）；qi 扣费守恒断言（zone 等额）

## P5 — 非人种族玩家入口 + 飞鲸 MVP + 收口

**交付物**：

- 非人种族获得路径（按 §8 #1 决议实施；推荐形态：后天秘法 / 夺舍类行为达成，角色创建仍统一人族——零出生论合规）；`/race set <id>` dev 命令（brigadier，dev-only 绕过标注）
- `RaceChange` 原子状态机：换 race 单事务完成 ① 卸下不满足新 gate 的装备（复用易形解除卸装路径）② 经脉集按 §8 #9 规则迁移 ③ `qi_max` 按新 plan 重算，`qi_current` 超出部分走 `qi_release_to_zone` 守恒归还 zone（ledger 断言测试，禁止真元蒸发 / 凭空）④ 功法保留（习得史实不抹），cast 由 race gate 拦截；任一步失败整体回滚不落半态
- `server/assets/body_plans/whale.json` 完整数据：部位草案 6 段（颅 / 躯干 / 背鳍 / 左胸鳍 / 右胸鳍 / 尾鳍，尾鳍 = Locomotion、颅 = Sensory）；HitGeometry 长扁 AABB 分段；经脉草案（条数与固元配额见 §8 #8）；`whale_layout.json` 面板布局；装备槽草案（非人形首例：`FinRing` 类骨饰槽位，装备件 1-2 个示范；`EquipSlotV1` 以**新增 enum 变体**方式扩展 + proto / TS / Java 镜像同改——槽位集合小且稳定，不随部位 id 一起 string 开放化）
- 玩家以 whale 构型走通全链：面板渲染 / 受击部位判定 / 经脉开脉突破 / RaceGate 拒穿人形甲 / 易形化人后可穿
- bot 场景（`scripts/bot/scenarios/`，硬约定）：① race gate 拒绝回执 ② 易形 cast → morph payload 解码 ③ body_plan_layout 首帧解码；CI bot e2e stage 接入
- e2e：client 发易形 cast → server 结算 → 周边 client 收 morph_state → 渲染路径断言
- worldview 增补案（种族后天路径 + 易形正典化 + 「异兽化形」词条消歧）**单独 PR 人工 review**；`docs/library/` 配套馆藏 1 篇（残卷来历考，走 /write-book）

## §7 与既有系统关系声明（防近义重名红旗）

- **dandao 变异（`MutationState` / `BodySlot` / `MutationKind` / `MutationStage::Bestial`，`dandao/mutation.rs:97-113`）**：变异 = 永久·污染驱动·在人形构型上叠加改造（人形 + 兽征）；本 plan 种族 = 更换整套身体构型；易形 = 可逆·外观 only。三轴正交。**不复用 `MutationKind` 表达种族**——变异槽挂在人形 `BodySlot` 上，语义装不下异构构型。P0 给 humanoid plan 一张 `BodySlot → BodyPartId` 映射表，变异渲染继续走原链路；`MutationStage::Bestial` 与易形兽形的视觉区分在 P4 UI 层标注（变异 = 永久体征叠加，易形 = 整体模型替换）
- **拟态蛛 / 道伥 disguise（`SpiderDisguiseState` 等）**：NPC 侧换贴图伪装，与易形（跨端模型替换 + 玩家实体）不同层；仅复用其协议模式，不共享状态机
- **beast-horde（active）**：只读 `FaunaKind` 群体迁移，无撞面；本 plan 不改 `BeastKind` enum 本身，只加 `BeastKind → BodyPlanId` 派生
- **npc-skin 遗留坑**：`Beast / Disciple / GuardianRelic` archetype skin 未实装——P4 反向化形（异兽→人形）演示实体正好落其中一格
- **用词**：全 plan 用「异兽」不用「妖兽」；「化形」仅在引用剑修 manifest / 丹道材料时出现，不作本 plan 机制名

## §8 开放问题（升 active / P0 前必须补 §8.1 决议收口）

1. **零出生论 vs 玩家选种族**（`worldview.md:590/719` 硬正典）。推荐：非人种族只能后天达成（固元后秘法转生 / 神魂夺舍异兽躯壳类路径），角色创建统一人族，把「选择」落在游玩行为里——合规且叙事更末法；若坚持创建期选择，需 worldview 修正案单独 PR 人工拍板。**用户拍板**
2. **机制命名**：「化形」被剑修 manifest（剑意化形）+ 丹道材料（化形根 / 化形大丹）占用。推荐「易形」（易形诀 / 易形残卷）。**用户拍板**
3. **上古大能措辞**：正典传承已断绝、只认残卷考古路径。推荐功法以残卷散佚形式出现，来历「笔迹类上古断代、不可考」，太初志「灵脉化形」传闻作暗线（保留存疑外壳）。已按此写入 P4，确认即可
4. **wire 开放化策略**：`MeridianId` / `CombatBodyPartV1` enum → string id，直接改新形状不留 dual-form（老存档仅需 humanoid 默认注入迁移）。风险：一次动 proto/TS/Java/samples 四面，P1 独立 PR 吸收
5. **易形后 gate 判定基准**：推荐**装备按当前形态**（易形核心收益——兽修化人穿人形甲；槽位与穿戴 gate 走 form plan，护甲 coverage 经 `morph_pairs.part_mapping` 折算回本体部位，解除时非法装备自动卸下——P4 已按此收敛语义）、**功法按本体 race**（经脉没变，功法门不该变）。用户需求原文「可以搭配不同种族装备」与此推荐一致，确认语义边界。**用户拍板**
6. **首批 Humanoid 档功法划定**：48 条存量全标 `Humanoid`，还是只标肢体强相关（剑修 / 体修 / 爆脉 / 卧牛）其余留 `Any`？推荐后者 + 逐条清单过目
7. **飞鲸可玩性边界**：本 plan 只到「数据 + 面板 + gate + 易形演示」；游泳 / 飞行移动、碰撞箱、出生点适配等可玩性问题**不进本 plan**（另立 plan），P5 验收允许 whale 玩家形态在测试场景内静态验证。确认 scope
8. **非人经脉数值曲线**：whale 经脉几条、各境界配额多少？推荐 P1 只锁机制（per-plan 曲线数据位），数值 P5 结合 halfstep buff 校准表拍；固元「12 正经全通」的人形语义在非人构型下如何等价（按比例 or 全通）需在 §8.1 定公式归属
9. **race 变更时的经脉迁移规则**（P5 `RaceChange` 状态机第②步的规则来源）：已开经脉如何处理？候选：a) 按 `part_mapping` 同源映射保留开脉进度（有对应者继承、无对应者丢弃）b) 全部重置（换构型 = 重修，叙事最自洽但最狠）c) 总进度折算按比例注入新经脉集。推荐 a) 映射保留 + 无对应丢弃，`qi_max` 差额走 P5 守恒路径；与 #8 数值曲线联动收口

## §10 实施工作流（骨架草案，升 active 时按 docs/CLAUDE.md §六模板细化）

- 预计 6 PR 序列化（单 plan 多 PR，不拆 plan）：PR-1 P0 底盘 → PR-2 P1 经脉 + wire → PR-3 P2 面板 → PR-4 P3 匹配 → PR-5 P4 易形（server+协议 与 client 渲染可再拆两只）→ PR-6 P5 收口；worldview 增补案独立 PR 人工 review，P5 归档前 land
- 每 PR 走 consume-plan 通用流程 + push 前对峙自检 workflow；实施 subagent 全 sonnet，verify 用高档模型
- 渲染 / 布局类交付（P2 humanoid layout、P4 渲染替换）适用 3 轮打磨 + `<PROMISE>` 担保
