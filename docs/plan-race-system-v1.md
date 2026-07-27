# plan-race-system-v1 — 通用身体构型（BodyPlan）与种族系统 + 固元「易形」功法

一句话主题：把「部位 / 经脉 = 人形硬编码」重构为按种族（BodyPlan）数据驱动的通用系统——每种 Entity 可定义各自的部位集合与经脉拓扑；装备 / 功法接入种族三档匹配（种族专属 / 人形通用 / 全通用）；固元境经残卷解锁「易形」功法（外观与装备形态层一对一互换，命中部位 / 经脉 / 碰撞箱不变），为玩家可扮演非人种族（如飞鲸）铺路。

> **状态：active（实施中，2026-07-10 升格）**。§8 全部开放问题已在 §8.1 收口（2026-07-10：#1/#2/#3/#6/#7 用户拍板，#4/#5/#8/#9 调研收口）；review 引擎 5 轮意见已全量吸收，第 5 轮后按用户指示封轮。
>
> 机制命名说明：用户原始需求称「化形功法」，但「化形」在仓库已被两处占用（剑修「剑意化形」= manifest 实体化；丹道材料「化形根 / 化形大丹」），本 plan 定名**「易形」**（2026-07-10 用户拍板，§8.1 #2）。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 种族 / 构型底盘：`BodyPlanRegistry` 数据驱动 + `Race` 字段 + 战斗部位消费点改造 | ⏳ | 待裁决（见复核结论） |
| P1 | 经脉系统通用化：`MeridianSystem` 去定长 + per-plan 拓扑与境界配额 + wire 开放化 | ✅ | 2026-07-12 |
| P2 | 动态部位 / 经脉面板：server 下发布局元数据，client 剪影与经脉图数据驱动 | ✅ | 2026-07-13 |
| P3 | 装备 / 功法种族三档匹配：`RaceGate` 三收拢点接线 + UI 反馈 | ✅ | 2026-07-13 |
| P4 | 易形功法：固元残卷解锁、外观一对一互换、玩家渲染替换链路 | ⏳ | 逐项核验后发现 2 项缺口，见 §P4 |
| P5 | 非人种族玩家入口 + 飞鲸 MVP 数据 + bot 场景 / e2e 收口 | ⏳ | — |

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
- **worldview 锚点**：修炼境界（固元 worldview.md §三 L70）、零出生论（§六 L590 / L719，冲突处理见 §8 #1）、上古大能（§二 L49 / §四 L292 / §十六 L1383）、残卷传承（§十 L890）、异兽词系（§七 L735 / §十 L892 / §十六 L1580）。涉及 worldview 增补（种族 / 易形正典化）必须**单独 PR 人工 review**
- **qi_physics 锚点**：本 plan 不新增任何物理常数。易形施放的 qi 消耗走既有 skill cast 扣费路径（含 zone 归还，qi 守恒清扫后的现行管线）；经脉容量对 qi_max 的贡献沿用 `MERIDIAN_CAPACITY_ON_OPEN` 既有常数（`meridian_open.rs:140-141`），per-plan 只改「哪些脉、几条」不改「每条值多少」

## 0. 调研摘要（2026-07-10，5 路并发核验，file:line 基于当日 origin/main）

**部位系统全链硬编码人形**：server `BodyPart` 8 段 unit enum（`combat/components.rs:32-41`）；命中判定唯一 hitbox 是 1.8m 人形 AABB（`raycast.rs:27-28,163-176`）+ 人体比例高度分类（`raycast.rs:193-236`，投射物同用 `carrier.rs:963-967`）；部位倍率 8 分支 match（`resolve.rs:1911-1920`）；臂 / 腿伤残废模块假设两臂两腿（`arm_wound.rs` / `movement/leg_wound.rs`，`MAIN_ARM=ArmR`）；client 16 段人形剪影 + 部位像素坐标全硬编码（`MiniBodyHudPlanner.java:117-181,365-384`、`BodyInspectComponent.java:283-314`）；server 7 段 → client 16 段 wire 映射写死（`wounds_snapshot_emit.rs:86-98`）。**唯一数据驱动先例**：护甲 `ArmorProfile.body_coverage` 从 `server/assets/combat/armor_profiles/*.json` 加载（`armor.rs:198-248`）——本 plan registry 模式范本。无 per-part HP（`Wounds.health_current` 全局单池），飞鲸已存在但被当 1.8m 直立人打（`spawn_whale.rs:297-298`）。

**经脉系统 20 条写死、镜像 5 处**：`MeridianId` 20 变体（12 正经 + 8 奇经，`cultivation/components.rs:66-89`），镜像 proto（`common.proto:23-47`）/ TS（`cultivation.ts:55-80`）/ client（`MeridianChannel.java` + `ClientRequestProtocol.java`）；`MeridianSystem` 定长数组 `[Meridian;12]+[Meridian;8]`（`components.rs:246-250`）物理装不下异种经脉数；拓扑单张 TCM 图（`topology.rs` `standard()`，注释已埋「后续可扩展异体」但无 hook）；境界配额写死 1/3/6/12/16/20 + 正奇子配额（`components.rs:37-46`、`breakthrough.rs:311-372`）；client 经脉图为硬编码人形折线（`BodyInspectComponent.java:827-912` `MERIDIAN_PATHS`）；玩家 / NPC / 凡兽均挂经脉（`npc/technique.rs:109-121`、`fauna/mundane.rs`）。经脉↔部位既有映射两处：`dugu.rs:532-540`（伤口污染注入）、`baomai_v4/dead_armor.rs:206-229`（死脉甲免疫）。

**功法 / 装备 / 持久化——种族纯增量**：全仓无任何 race/species 玩家概念。`TechniqueDefinition` 48 条 Rust const（`known_techniques.rs:100-114,133-934`）已有 `required_realm` + `required_meridians` 双门；习得唯一收拢点 `learn_technique_if_allowed`（`technique_scroll.rs:77-111`，scroll / 拜师 / 偷师 / insight 全走此处）；施放收拢点 `handle_skill_bar_cast`（`client_request_handler.rs:10092-10264`，拥有门 :10128-10140 → 经脉门 :10166）；穿戴唯一入口 `validate_equip_to`（`inventory/mod.rs:5349-5509`），现存唯一「谁能穿」先例是 false_skin 境界门散落特例（`client_request_handler.rs:11097-11130`，反面教材）。`Cultivation` 组件整体序列化进 `cultivation_json` bundle（`persistence/mod.rs:5324-5387`）→ **race 字段加 `#[serde(default)]` 即自动持久化，persistence 层零改动**。

**外观 / 化形链路——协议可抄、渲染空白**：物种全 enum 硬编码（`NpcArchetype` 12 族 + `BeastKind` 16 种）；现有 disguise 是「同实体换贴图」应用层旗标（道伥披 Steve 皮 / 拟态蛛披灰烬贴图），只作用于 NPC，从未碰玩家实体；协议骨架可复用（版本化 JSON payload + entity-id 集 + 周期全量 sync + 触发时 64 格半径广播）；`BongEntityModelKind` bbmodel raw-id 注册表现成（含 `expectedRawId` 运行时断言）；「NPC 以 `EntityKind::PLAYER` + MineSkin 装成玩家」反向先例已通。**四大空白**：玩家本体渲染替换 mixin（`FakePlayerRendererMixin` 是未注册空占位）、FPV 处理、动态碰撞箱、动物贴图通道（GameProfile texture 与 `textureForState` 两套材质系统不通）。

**撞车面与正典红线**（详见 §7 / §8）：dandao `MutationState/BodySlot/MutationStage(Bestial)` 是既有「身体改造」系统必须声明关系；「化形」「妖兽」两词违典 / 撞名；零出生论（worldview.md §六 L590 / L719）拦「创建期选种族」；传承断绝设定拦「上古功法完整流传」。

## P0 — 种族 / 构型底盘

**交付物**：

- 新模块 `server/src/body_plan/`：
  - `struct BodyPlan { id: BodyPlanId, display_name, is_humanoid: bool, parts: Vec<BodyPartDef>, hit_geometry: HitGeometry, equip_slots: Vec<EquipSlotV1>, meridian_profile: Option<MeridianProfile> }`——**易形配对不在 BodyPlan 内**（唯一真源 = races.json 全局 `morph_pairs`，见 P4，防双真源）；`is_humanoid` 是 P3 `RaceGate::Humanoid` 档的唯一判据（不做名单硬编码）；`meridian_profile` 带 `#[serde(default)]`（P0 缺省 `None`，P1 起 humanoid 必填并升校验）——**P0 的 humanoid.json 不含该字段即合法**，缺失 / 完整形态各一条 pin 测试；fail-fast 只打真非法（未知字段 / 引用悬空），不打阶段性缺省
  - `struct BodyPartDef { id: BodyPartId(string id), damage_mul, contam_mul, bleed_mul, consequence: PartConsequence }`——`PartConsequence` 枚举化现有「腿伤减速 / 头伤眩晕 / 臂伤六维」后果语义（`Locomotion / Sensory / Manipulator{main_hand} / Core`），非人形部位挂同枚举（如鲸尾鳍 = Locomotion）
  - `enum HitGeometry { HeightBands { aabb, bands, lateral_threshold }, PartBoxes(Vec<PartBox { part_id, offset, half_extents, priority }>) }`——`HeightBands` 是现 `classify_body_part` 高度带 + 横向阈值的参数化（humanoid 用，行为 bit-for-bit）；`PartBoxes` 逐部位局部 AABB、射线逐盒求交取最近命中（非人形用——单一直立 AABB + 人体比例高度带表达不了飞鲸横长构型，P5 whale 必须走此模式）；`PartBox` 坐标为实体局部系（原点 = 实体位置，+Z 沿实体 yaw 正前），求交前世界射线经实体 Transform 逆变换入局部系（P0 只支持 yaw，不做 pitch/roll）
  - `BodyPlanRegistry` 从 `server/assets/body_plans/plans/*.json` 加载（**glob 只认 plans/ 子目录**——`races.json` 与 `layouts/` 各有独立 loader，三类资源目录互不重叠，防异构 JSON 混入误解析；配「混装文件进错目录」反例测试），模式仿 `combat/armor.rs::load_dir`（重复 id / 缺字段 fail-fast）
- 运行时接线唯一入口：`BodyPlanPlugin`（注册 `BodyPlanRegistry` Resource + startup `load_body_plan_registry`，安装进 main.rs 根 App plugin 链，registry 间引用校验在 post-load system 统一 fail-fast）+ `resolve_body_plan(entity, purpose: BodyPlanPurpose) -> &BodyPlan` 唯一解析函数，`BodyPlanPurpose::{Intrinsic, Form}` 显式区分两套语义：**Intrinsic（本体）**——命中几何 / 伤残后果 / 经脉 / 功法门 / 伤口面板；**Form（当前形态）**——装备槽集合 / 穿戴 RaceGate / coverage 折算起点（未易形时 Form ≡ Intrinsic）。优先级：玩家走 `Cultivation.race`（**未知 RaceId = 拒载入错误态，不静默兜底 humanoid 白得权限**）→ NPC / fauna 走 `BeastKind→BodyPlanId` 派生 → 其余可受击实体兜底 humanoid。**所有消费点只准走此入口并逐点标注 purpose**，绕过直查 registry 的新调用按 review 红旗处理
- `validate_body_plan` 全图校验（P0 交付，P1/P4 随字段扩展）：part / channel id 唯一、`PartBox.part_id` / coverage / topology 端点全部存在、`realm_requirements` 单调且 ≤ channel 总数（races.json 的 `morph_pairs.part_mapping` 端点引用 BodyPlan 部位走**跨 registry post-load 校验**）；每类非法输入独立反例测试 + 带定位信息的错误消息
- `server/assets/body_plans/plans/humanoid.json` 首个条目：8 部位 / 倍率表 / 1.8m AABB 分段阈值 / 4 护甲槽，**与现状硬编码值逐项 bit-for-bit 对齐**；含 `mutation_slot_mapping`（dandao `BodySlot` → BodyPartId 全覆盖映射，§7 声明的落点，校验全变体覆盖 / 无悬空；查询 API `body_part_for_mutation_slot(plan, slot)`——server 消费点 = dandao 部位效果解析，client 消费点 = `MutationFeatureRenderer` 槽位定位适配；每 BodySlot 变体 + 悬空 / 缺失映射契约测试）
- `Race` 表示：`Cultivation.race: RaceId`（`#[serde(default)] = "human"`）；**RaceId → BodyPlanId 唯一真源 = `server/assets/body_plans/races.json`（`RaceRegistry`，独立单文件 loader，不进 plans glob）**，加载期校验 RaceId 唯一 / BodyPlanId 存在 / 必含 `human` 默认条目 / morph pair 引用不悬空，每类非法各一条反例测试，`RaceRegistry` 同注册进 `BodyPlanPlugin`（加载顺序 BodyPlan → Race → 跨 registry post-load 校验），持久化 bundle 反序列化后 RaceId 校验有显式拒载执行点（未知 id 载入错误态测试）；NPC / fauna 侧走 `BeastKind → RaceId` 唯一映射（races.json 含异兽种族条目）再查 registry，可易形实体挂显式 `IntrinsicRace` 组件供 `resolve_morph_pair` 取 from 端（默认全部落 human，P5 起给 whale 换）
- 战斗消费点改造（行为回归不变）：`body_part_multipliers` / `classify_body_part` / `standing_humanoid_aabb` / `carrier.rs` 投射物分支改为查询目标实体 BodyPlan；`arm_wound` / `leg_wound` 后果分派改走 `PartConsequence`
- **测试**：registry 加载饱和（happy / 重复 id / 缺字段 / 空目录）；humanoid 对拍 pin（逐部位倍率与 `resolve.rs:1911` 旧表全等断言）；raycast 分类回归（P1 直方图样本重跑）；`PartConsequence` 每变体专属 case；`PartBoxes` 射线契约饱和（最近距离优先 / 等距按 priority 稳定序 / 盒内起点 / 边界擦触 / 平行射线 / 空集合未命中 / yaw 0·90·180 旋转与平移后命中——断言返回的 part id + 距离，不绑内部迭代序）；`resolve_body_plan` 解析矩阵饱和（玩家已知·未知 race × NPC × fauna × 其他实体 × Intrinsic/Form × 未易形/易形——错误分支断言拒载而非 fallback）

## P1 — 经脉系统通用化

**交付物**：

- `MeridianSystem` 定长数组 → `Vec<Meridian>` keyed by `MeridianChannelId`（string id；人形保留现 20 条 id 的 snake_case 字符串形态）；`MeridianId` enum **退役防双真源**：全部 gameplay 消费方（吐纳 / 突破 / severed / burst / baomai / NPC 选招）在 P1 同阶段改从实体 BodyPlan 的 `MeridianProfile` 枚举 channel，旧 `ALL/REGULAR/EXTRAORDINARY` 常量只保留在旧存档迁移函数内（标注 migration-only），运行时新增引用按红旗处理
- `MeridianProfile`（进 body_plan json）：`channels: Vec<ChannelDef>`、`topology_edges`（替换 `topology.rs` 单张 standard 图）、`realm_requirements: [RealmMeridianReq; 6]`（人形曲线 1/3/6/12/16/20 + 正 / 奇配额搬进 humanoid.json；`Realm::required_meridians()` / `breakthrough_precondition_error` 改查 plan）
- NPC 经脉生成参数化：`npc_meridian_system_for_realm` 按实体 BodyPlan 生成
- wire 开放化：proto `MeridianId` enum → string channel id，**直接改新形状不留兼容层**（TS union / `MeridianChannel.java` / `ClientRequestProtocol.java` / `samples/*.json` 一次改齐）；`cultivation_detail` SoA 数组随 plan 变长（`cultivation_detail_emit.rs:51-140`），emit 时附 channel id 序
- `CombatBodyPartV1` 同批开放化为 string part id（`combat_event.rs:5-14` / `combat-event.ts:19-29`；`WoundEntry.part` proto 侧已是 string 不动），与 MeridianId 收进同一只 wire PR；wire 改形前提 = 全栈同版本原子部署（本仓惯例，不写兼容层不做版本协商），decoder 对旧形状配负向测试直接拒
- **旧存档显式迁移**：`cultivation_json` bundle 的 `meridians` 字段旧形态是定长 `[Meridian;12]+[Meridian;8]`，Vec 化后直接反序列化必崩——bundle 版本号 bump + 迁移函数（旧数组按 `REGULAR`/`EXTRAORDINARY` 序注入 humanoid channel id → 新 Vec 形态），`MeridianSeveredPermanent` 同批迁移；测试用真实 v31 存档 dump 的 bundle 样本对拍逐脉状态，禁止只测新形状
- 经脉↔部位映射数据化：`dugu.rs:532-540` 与 `baomai_v4/dead_armor.rs:206-229` 两张 match 表并入 humanoid plan 的 `ChannelDef.body_part` 字段（防第三张私表）
- **测试**：humanoid plan 下全消费点回归对拍（tick 吐纳 / breakthrough 配额 / severed 门控 / burst_meridian / baomai 邻接）；非人 plan 合成样本（6 脉构型）走通开脉 → 突破配额 → severed 全链；schema 正反 sample 对拍；`MeridianChannelId` 未知 id fail 分支

## P2 — 动态部位 / 经脉面板渲染补强

**交付物**：

- 新 payload `ServerDataPayloadV1::BodyPlanLayout`（`BodyPlanLayoutV1`）：以 `body_plan_id` 为主键，含部位剪影多边形顶点（归一化坐标）+ 部位锚点（伤口红点位）+ 经脉折线路径（替代 `MERIDIAN_PATHS` 硬编码）+ server 部位 id → 展示段映射（替代 `body_part_wire` 7→16 写死映射）。join 首帧随 `cultivation_detail` 下发；`cultivation_detail` 附带自身 `body_plan_id` 供 client 按 plan id 寻址缓存；实体 plan 变化（真实换 race）时重发，易形不触发（命中与面板仍走本体 plan，语义见 P4）
- 布局数据源：humanoid 布局从现 `MiniBodyHudPlanner` / `BodyInspectComponent` 硬编码坐标**原样抽取**进 `server/assets/body_plans/layouts/humanoid.json`（独立 layouts loader；首版渲染与现状像素级一致）
- client 重构：`BodyPlanLayoutStore`（以 `body_plan_id` 为键的多快照缓存 + listener，仿 `MeridianStateStore` 的 volatile + 订阅模式）；`MiniBodyHudPlanner` / `BodyInspectComponent`（剪影 + 经脉图 + `locatePart`）/ `WoundLayerBinding.resolvePart` 全改读 store；无 layout 时**仅视觉 fallback** humanoid 剪影（防御首帧竞态；gate 判定与视觉解耦，权威身份缺失时 fail-closed，见 P3）——首帧乱序 / 未知 plan id / 缺 layout 三测试
- 破损护甲裂纹 / 丹药部位框逻辑随部位 id 走（`body_part_resist:` status id 前缀机制保持）
- **测试**：schema 正反 sample；humanoid layout 渲染回归（`client/tools` render harness 截图对拍现状）；合成非人 layout（6 段构型）渲染不越界 / 红点落锚点；缺段 / 冗余段 wire 容错

## P3 — 装备 / 功法种族三档匹配

**交付物**：

- `enum RaceGate { Any, Humanoid, Species(&'static [RaceId]) }`（`body_plan` 模块，紧邻 RaceId；`&'static` 切片可入 48 条 const 功法表，`ItemTemplate` 运行时加载侧用 owned 形态 `RaceGateOwned`；wire = tagged 结构 `{kind: any|humanoid|species, species: string[]}`——kind 显式判别三档，`species` 仅 kind=species 时非空，未知 kind fail-closed；三端三变体正反 sample + 空 / 重复 / 未知 kind pin 测试）——三档语义：全通用 / 人形通用（所有 humanoid 构型种族可用）/ 种族专属；`Humanoid` 档判据 = 对应判定域 BodyPlan 的 `is_humanoid` 字段（**功法域按本体 plan、装备域按当前形态 plan**，矩阵见下方身份快照 bullet），不做种族名单硬编码
- 功法侧：`TechniqueDefinition.required_race: RaceGate`（48 条存量补默认 `Any`；Humanoid 划定标准已拍板 §8.1 #6——仅强依赖人体专属经脉拓扑 / 肢体机能者标 `Humanoid`，飞剑类神识驱动保持 `Any`，逐条清单升 active 时附）；习得门插 `learn_technique_if_allowed`（境界门后，新增 `ScrollReadOutcome::RaceMismatch`）；施放门插 `handle_skill_bar_cast` 拥有门后经脉门前（sword_path resolver 路径镜像一份，`skill_register.rs:862-877` 旁）
- 装备侧：`ItemTemplate.wearer_race: RaceGate`（TOML 可选字段默认 any）；校验统一进 `validate_equip_to`（槽位分支判定后、`Ok(())` 前），新增 `InventoryMoveRejectReason::RaceMismatch`；false_skin 散落境界门**不动**（不同轴，境界≠种族）
- client UI 反馈：不匹配装备格 / 功法条目置灰 + 点击 toast 带原因（对齐 #663 灰按钮 toast 先例）；种族不可用的功法 / 装备**不出现在推荐位**（HUD conditional display 原则）
- gate 数据下行（client 身份快照契约）：client 置灰不靠猜——① `cultivation_detail` 附带本体 `race_id` + 当前形态 `form_race_id` / `form_body_plan_id`（未易形时 = 本体值），快照直接下发 `intrinsic_is_humanoid` + `form_is_humanoid` 两权威布尔；判定归属唯一矩阵：**装备域** Species 判 `form_race_id` / Humanoid 判 `form_is_humanoid`，**功法域（习得+施放）** Species 判本体 `race_id` / Humanoid 判 `intrinsic_is_humanoid`（与 P0 `BodyPlanPurpose`、P4 四层语义、§8.1 #6 一致；两 RaceId 共享同一 BodyPlan 的反例测试）；② gate 依据走身份快照（权威），`BodyPlanLayoutV1` 的 `is_humanoid` 元数据仅供渲染——client 未取得权威身份数据时 **fail-closed 置灰**，不因 layout 缺失 / 首帧乱序误放行；③ 功法列表 payload 与物品 wire 数据补 `required_race` / `wearer_race` 字段（proto / TS / samples 同改）。更新时机全枚举：join 首帧 / 易形开始 / 易形解除（含死亡、下线重连）/ 真实 RaceChange——每个时机一条双端 sample + 状态转换测试，三档 gate × 本体 / 易形态矩阵 pin。server 端 `validate_equip_to` 仍是权威（client 判定仅 UX 预览）
- wire：reject reason 新变体过 proto / TS / samples；`RaceGate` serde 正反 pin
- **测试**：三档 × 三收拢点矩阵全覆盖（9 组合 happy + 拒绝）；`RaceMismatch` 双 reason 枚举 wire pin；48 条存量功法 gate 逐 skill_id pin 清单（清单 = §8 #6 决议产物，升 active 前定稿；不写笼统「全 Any」断言）；bot 场景：race gate 拒绝穿戴回执

## P4 — 易形功法（固元境解锁）

**⏳ 未完成欠账（2026-07-27 第三轮返工逐交付物核验；此前 #1278 与本 plan 早前几轮均只拿代表性文件当交付物证据，未逐条核对，与本 plan 用来回退 P5 的标准是同一套双重标准——本轮补做，对齐 review r5 blocker①）（2026-07-27 第四轮返工：review r7 四位 reviewer 一致指出「proto_min bot 解码」「morph TPV 渲染 harness 截图」两项 P4 明文测试交付物被本轮审计自行排除出统计——理由分别是「与 P5 同一缺口不重复登记」「非核心交付物」，这正是本 PR 用来推翻 #1278 的那条标准（写明缺口还标完成）的镜像版本：跨阶段去重可以在两处都登记同一根因，但不能让某一阶段自己的欠账消失。补记为欠账 3/4，并撤销「非核心」「未单独列欠账」两处自行降级，见下方结论重算）**：

1. **`morph.yixing` 的 `FormAnchor` 前置门在习得 / 施放两处生产收拢点均零集成测试**——现状：门控本身真实接线到位——习得侧 `ScrollReadOutcome::FormAnchorClosed` 在境界门后、经脉门前判定（`cultivation/technique_scroll.rs:137-144`），施放侧 `handle_skill_bar_cast` 在 race gate 后、经脉门前同判据拒绝并推送 `CastSyncV1{outcome: CastOutcomeV1::MeridianGated}`（`network/client_request_handler.rs:13995-14038`）；humanoid/whale 两个真实构型都已把对应奇经标 `form_anchor`（`server/assets/body_plans/plans/humanoid.json:190-201` 的 `ren`/`du`，`whale.json:126-138` 的 `keel_meridian`/`spine_meridian`）；底层纯函数 `form_anchors_open` 自身有 5 条覆盖态（`body_plan/morph.rs:530-586`：无声明/happy/单脉缺口/系统缺该 channel/SEVERED 优先于 stale opened）。**但没有任何测试真正调用 `learn_technique_if_allowed("morph.yixing", ...)` 或驱动 `handle_skill_bar_cast` 走到这两处判据触发拒绝**——`grep -rn "FormAnchorClosed" server/src/` 只命中定义 + 两处生产消费，零测试断言；`grep -rn "MeridianGated" server/src/network/client_request_handler.rs` 命中的全部测试都是通用经脉依赖门（`SkillMeridianDependencies`/严重断脉表），没有一条以 `morph.yixing`/`form_anchor` 为对象。plan 原文要求的「human / whale 各配习得、施放、缺脉、断脉正反测试」目前只在纯函数层面成立，两处生产收拢点层面完全没有验证。要落：至少 human 与 whale 各配「习得-缺脉拒绝」「习得-断脉拒绝」「cast-缺脉拒绝」「cast-断脉拒绝」共 8 类正反用例（或等价合并到更少但仍覆盖两构型 × 两收拢点 × 两拒绝原因的用例），断言真实触发 `ScrollReadOutcome::FormAnchorClosed` 与 `CastOutcomeV1::MeridianGated`，而不是只测底层纯函数。**（2026-07-27 第四轮返工补记，对齐 review r7 major——上述 8 类全部是拒绝分支，plan 原文明写「正反测试」，「正」的一半此前遗漏）**：另需 human 与 whale 各自在**习得**（`learn_technique_if_allowed("morph.yixing", ...)`）与**施放**（`handle_skill_bar_cast`）两个生产收拢点补 happy path 用例——`form_anchor` 全部脉已通且未断时，断言真实触发成功 outcome（对应习得成功 / cast 进入 `active` 状态转换）且既有 qi ledger 扣费恰好发生一次；上述 8 类拒绝分支用例额外断言不产生状态转换或账本副作用（不推进 morph 状态机、不触发 `QiTransfer`）。
2. **易形态玩家的名牌 / 队友标识实测未保留，与 plan 明文「名牌 / 队友标识保留」及代码自身注释的论证相反**——现状：`MixinMorphedPlayerRenderer`（`client/src/main/java/com/bong/client/mixin/MixinMorphedPlayerRenderer.java:50-73`）在 `EntityRenderDispatcher.render()` 方法 HEAD 处 `cancel()` 玩家本体渲染，转而递归调用 `dispatcher.render(proxy, ...)` 绘制 `MorphRenderProxy.whaleFor(player)`（`client/src/main/java/com/bong/client/morph/MorphRenderProxy.java:29-35`）产出的匿名 `WhaleEntity` 代理——该代理从未 `setCustomName`。本轮反编译验证 + 独立 validator 复核后按实际字节码路径重写（初版技术叙述有误，已订正）：① `EntityRenderDispatcher.render()` 只对目标实体调用一次专属 `EntityRenderer.render(...)`，玩家自身那一路因 `cancel()` 完全不会执行；② 反编译本仓库 `geckolib-fabric-1.20.1-4.4.9.jar` 确认 `GeoRenderer.defaultRender()` 末尾调用 `GeoEntityRenderer.renderFinal()`，而 `renderFinal()` 里有一处 `invokespecial EntityRenderer.render(entity, 0.0f, tickDelta, matrices, vertexConsumers, light)`（即 `super.render(...)`）——**标签渲染调用确实存在于这条链路里**，反编译 Yarn 映射的 `EntityRenderer.render()` 基类实现确认其内容正是 `if (hasLabel(entity)) renderLabelIfPresent(entity, entity.getDisplayName(), ...)`；③ 但 `hasLabel(T)` 基类默认实现 = `entity.shouldRenderName() && entity.hasCustomName()`（同样经反编译确认），而 `WhaleEntity` 继承自 `Entity`（非 `LivingEntity`），代理又从未 `setCustomName()`，`hasCustomName()` 恒为 `false` → `hasLabel()` 恒为 `false` → 标签调用链路虽然存在，但代理这条路径永远不会真正画出标签。净效果不变：易形期间玩家在他人客户端**完全没有名牌**，与「队友能认出这是谁、防 grief 误伤」的目的相反；该 Java 文件内注释自称「队友标识/名牌不受影响……递归调用 dispatcher.render 时 vanilla 名牌渲染逻辑仍会对 proxy 走一遍」——这句话字面上其实说对了一半（渲染逻辑确实会走一遍），但没意识到那一遍必然因代理无自定义名而空手而归，实际效果仍是零名牌。`client/src/test/java/com/bong/client/mixin/` 目录本身存在（含 3 个与本 mixin 无关的既有测试），但没有任何测试覆盖名牌/`MixinMorphedPlayerRenderer` 行为，`client/src/test/java/com/bong/client/morph/`/`hud/` 下现有测试同样未涉及名牌——零覆盖的结论不变。要落：让 `MorphRenderProxy` 承接玩家 `getDisplayName()`/team 信息并对代理调用 `setCustomName(...)`（这样上述已存在的 `hasLabel`/`renderLabelIfPresent` 链路会自然生效，无需额外改造渲染管线），或改用不完全 cancel 玩家渲染管线的方案（如只替换模型/贴图阶段）。验收（2026-07-27 第四轮返工升级，对齐 review r7 major——原验收只测 `hasCustomName()` 关不住本行自己论证的 `hasLabel` 判据）：新增测试锁住**标签判定谓词整体为真**——同时断言 `shouldRenderName()` 与 `hasCustomName()` 均为真（即 `hasLabel` 等价判据），且代理继承原玩家 `getDisplayName()` 与 scoreboard team；至少一组同队对照（队友能看到代理名牌）与一组异队对照（team 规则允许可见时同样能看到，不因走代理渲染路径丢失），无 headless 渲染环境时退而求其次锁该数据契约本身。**不扩展为 `always`/`never`/`hideForOtherTeams`/`hideForOwnTeam` 四态可见性矩阵**——一旦代理继承原玩家 team，这四态是 vanilla scoreboard 自身的行为，不在本 plan 交付物内；P4 原文契约止于「名牌 / 队友标识保留」，即代理触发的标签判定与原玩家一致，再往外扩是本轮类 2/3(a) 同款的范围加码。
3. **`proto_min` bot 解码与 `morph_state` payload `full` 模式 proto round-trip 测试均未落地（2026-07-27 第四轮返工新记，对齐 review r7 blocker）**——现状：① P4 原文测试段明写「payload 双端 sample + proto_min bot 解码」；`scripts/bot/scenarios/` 目录尚无任何 morph 场景（`grep -rln "morph" scripts/bot/scenarios/` 0 命中），当前只有双端 sample 落地（见下方已核验落地清单第 14 项），bot 解码这一半缺失。② `schema/proto_convert.rs` 目前只有 `delta`（易形解除广播）一条带字段级断言的 `morph_state` proto round-trip 测试（`proto_convert.rs:7455`）；`full`（join / 周期 sync 发的形态）分支只搭在通用 fixture 冒烟测试 `s2c_all_proto_variants_encode_without_panic`（`proto_convert.rs:7691-7744`）里，该测试只断言编码非空、能解码、payload 非空，**不做字段级 round-trip 断言**（不解回 Rust 结构体比对 `mode`/`entries` 等字段），也只在 client `MorphStateHandlerTest.java` 层面被间接覆盖——`full` 分支缺一条像 `delta` 那样的专属字段级 round-trip 测试。**这条与 P5 欠账 2（bot 场景 3 条只落 1 条，其中场景② 易形 cast → morph payload 解码正是本条缺失的 bot 端）共享同一根因**——两处都需要各自登记，闭环时可一并交付（P5 写 bot 场景②时顺带把 proto `full` 分支测起来），但共享根因不等于 P4 可以少算一项。要落：a) 新增至少一条 `scripts/bot/scenarios/` morph cast 场景并接入 CI bot e2e stage（与 P5 欠账 2 ②共享交付物）；b) 补一条 `full` 模式的 `morph_state` proto round-trip 测试（对齐现有 `delta` 一条的覆盖标准）。
4. **「渲染 harness：morph 玩家截图（TPV 兽形 + 名牌保留）」未落地（2026-07-27 第四轮返工新记，对齐 review r7 blocker）**——现状：`grep -rn "render_animation\|render_bbmodel" client/tools` 未见任何 morph 专用调用；`client/tools/render_animation.py` / `render_bbmodel.py` 现有能力覆盖姿态与模型静态渲染，但没有一条以易形后的 TPV 玩家（兽形 + 名牌）为对象的截图 harness 测试。这不是「非核心交付物」——是 P4 原文测试段明写的第二个 harness 类交付物（与 payload sample、bot 解码并列），此前审计以「细节缺口 / 非核心」为由排除出统计是自行降级，本轮撤销该降级。要落：新增 morph TPV 渲染 harness 用例（复用 `client/tools/render_animation.py` 或等价工具），产出对拍截图并断言兽形模型 + 名牌（配合欠账 2 的名牌修复）同时出现在渲染结果中。

**已核验落地清单（2026-07-27 第三轮返工，逐交付物 grep + 读码复核，不采信既有文档结论；下方数字对应本 plan 交付物段原始顺序）**：

1. `yixing_scroll` 物品 + `technique_scroll_spec.skill_id = "morph.yixing"`：✅ `server/assets/items/morph_scrolls.toml:1-15`
2. 两条掉落入口，各配真实产出集成测试：✅ 道伥击杀档 `server/src/npc/loot.rs:464-552`（`daozhan_loot_high_tier_has_yixing_scroll` + `yixing_scroll_loot_produces_real_item_landing_in_player_inventory`，真实走 `daozhan_death_loot_system`→`build_fauna_item_instance`→`add_item_to_player_inventory` 生产链路，`register_p3` 已挂进 `fauna/mod.rs:111` 真实 App）；tsy 遗迹档 `server/src/inventory/tsy_loot_spawn.rs:211-263` + `tsy_loot_integration_test.rs:197-291`，真实系统 `tsy_loot_spawn_on_enter` 已注册进 `inventory/mod.rs:894`
3. `TechniqueDefinition` 新增 `morph.yixing`（`required_realm = "Solidify"`）：✅ `server/src/cultivation/known_techniques.rs:1013-1028`
4. `ChannelDef.roles`/`form_anchor` 前置门机制 + humanoid/whale 数据：✅ 机制与数据均落地（见上方欠账 1 的具体 file:line），**但生产收拢点集成测试缺失**（欠账 1）
5. `MorphState { form, model_kind, since_tick }` 组件：✅ `server/src/body_plan/morph.rs:45-49`，字段与命名逐一对齐
6. `morph_pairs` 唯一真源 + `resolve_morph_pair` + 重复配对 fail-fast + 双向各自显式配置：✅ `server/src/body_plan/race_registry.rs`（`morph_pair`/`resolve_morph_pair`/`morph_targets_from` 定义于 209-241，14 条测试 717-1056 覆盖 happy/重复 from-to/悬空 from/悬空 to/等）；**机制与测试 ✅，但生产配置存在方向性缺口（2026-07-27 第五轮返工按 review r8 blocker 订正，此前判为「设计使然」是错的）**：生产 `races.json`（`server/assets/body_plans/races.json`）只声明 `human→whale` 一条 pair。此前的论证是「`cast_morph_yixing` 的解除分支走组件直接移除不查表，架构上不需要反向 pair」（`morph.rs:119-122` 注释自陈）——**该论证只覆盖「人易形为鲸之后解除、回到本体人形」这一种情形，不覆盖「本体即鲸族的玩家易形化人」**。而后者正是 P4 明文要保住的方向（P4 交付物段原文：经脉前置「不写死人形 channel id，否则飞鲸构型无 Ren/Du，非人种族永远无法易形化人——自断『动物→人』方向」），也是 P5「非人种族玩家入口」成立的前提。

**⏳ 欠账（挂 P4，与 P5 入口联动）**：`races.json` 缺 `whale→human` 显式 pair（含反向 `part_mapping`），本体非人的玩家当前无法易形化人。要落：在生产 `races.json` 增加独立的 `whale→human` pair（不假设可逆，按 plan「双向对两方向各自显式配置」的既定口径），并补一条**走生产配置**的集成测试断言该方向可解析、`part_mapping` 端点校验通过。**在此之前，「双向易形」不得记为已完成**
7. `part_mapping` 部分单射 + 8→6/6→8/缺项/重复目标/悬空引用测试：✅ `race_registry.rs:812-937`（`morph_pair_part_mapping_key_dangling_in_to_plan_rejected`/`_value_dangling_in_from_plan_rejected`/`_duplicate_intrinsic_value_rejected`/`_partial_injective_map_accepted` 等），生产 `part_mapping` 6 项人→鲸映射见 `races.json`
8. 死亡 / 濒死 / 下线自动解除：✅ 死亡即 `DeathEvent`→`NearDeath` 转换同一入口（`combat/lifecycle.rs:388-402` + 测试 `death_arbiter_tick_auto_releases_morph_state_on_death` L3069-3128，本游戏状态机里「死亡」与「濒死」是同一次事件触发，无独立的第二个事件）；下线 `player/mod.rs:366-373` + 测试 `disconnect_auto_releases_morph_state_before_persist_snapshot` L1262-1319
9. form 护甲 `body_coverage` 经 `part_mapping` 折算：✅ `combat/resolve.rs:99-115,3554` + 3 条测试（含 unmorphed 对照组 + 伤害对比组）
10. 解除易形时装备自动卸入背包 / 装不下 `dropped_loot`：✅ `inventory/mod.rs:5059-5130`（`enforce_intrinsic_gate_on_morph_release`）+ 8 处测试调用（`inventory/mod.rs:19409-19865`），`race_change.rs:390` 复用同一函数
11. `morph_state` payload 格式 + 6 场景（start/手动解除/死亡/重连/多实体并存/未知 plan id）：✅ 服务端 `network/morph_state_emit.rs` 全量+delta 节流测试（L270-549）；客户端 `client/src/test/java/com/bong/client/network/MorphStateHandlerTest.java` 全 7 用例覆盖 full 多实体/full 清空(对齐重连全量替换语义)/delta 插入(start)/delta 移除(手动解除·死亡·下线在 wire 层不可区分，同一断言覆盖三者)/混合增删/未知 mode 不崩溃/缺 entity_id 跳过；`client/src/test/java/com/bong/client/morph/MorphModelRegistryTest.java` 的 `unknownFormRaceIdHasNoModel` 覆盖未知 plan id 场景。proto round-trip 目前只有 delta release 一条（`schema/proto_convert.rs:7455`），full 模式缺对应 proto 测试——并入欠账 3（2026-07-27 第四轮返工升级，不再作为细节缺口豁免）
12. client 渲染替换（`MixinMorphedPlayerRenderer` 已注册 `bong-client.mixins.json:19`）：✅ 注册属实；FPV 保持 vanilla 手臂：✅ 架构性保证（`EntityRenderDispatcher.render` 本就不对 focused entity 在非-TPV 视角调用，注释准确）；名牌/队友标识保留：❌ 见上方欠账 2
13. 视听五项：图标 ✅（`known_techniques.rs:1026` + PNG 已存在于 `client/.../skill_scroll_morph_yixing.png` + `technique_icon_snapshot_test.rs` 双端 pin 框架已纳入、无例外条目）；`yixing_scroll` 物品图标走全仓统一的 scroll 后缀 fallback 机制（`ItemIconRegistry.fallbackTextureIdForItemId`，`_scroll` 后缀命中 `broken_artifact_scroll.png`）——与全仓其余全部 scroll 类物品一致（无一例外持有专属图标），是既定惯例而非本 plan 独有缺口；粒子/音效/HUD/动画/narration 均在 `body_plan/morph.rs:199-283` 落地并接生产事件总线，具体 vanilla 音源 id、动画 endTick 与 plan 骨架草案原文数值有出入（如三层音效改用自定义 `bong:skill.morph.yixing` 而非 `entity.evoker.prepare_wololo`、动画 `morph_cast.json` 改 endTick=20 而非 30），均系后续 PR 内注释自陈的刻意重制迭代，非漏项
14. 测试段：morph 状态机全转换（学→cast→active→手动解除/死亡解除/下线解除/重复 cast 幂等）✅（`morph.rs:716-900` 区间 + 上述 combat/player 测试）；gate 矩阵 ✅（race/realm/meridian 各自独立测试，`form_anchor` 分支例外见欠账 1）；payload 双端 sample ✅、proto_min bot 解码 ❌，见欠账 3；渲染 harness 截图 ❌ 未找到专门的 morph TPV 截图 harness 测试（`grep -rn "render_animation\|render_bbmodel" client/tools` 未见 morph 专用调用），见欠账 4；qi 扣费 ledger 契约 ✅（`morph.rs:716-900`：方向+精确金额+守恒断言+余额不足不转账+失败(InvalidTarget)不转账+成功仅记一次+重复请求幂等，7 项要求全部命中）

**结论（2026-07-27 第五轮返工重算，按 review r8 blocker 把第 6 项从「完全落地」降级）**：14 项交付物里第 1/2/3/5/7/8/9/10/13 共 **9 项**完全核验落地（含真实生产接线 + 真实测试）；第 4/6/11/12/14 共 **5 项**部分完成——机制 / 数据 / 多数测试已落地，但各自遗留缺口分别对应欠账 1（第 4 项：FormAnchor 生产收拢点集成测试，含 happy path）、欠账 2（第 12 项：易形态名牌 / 队友标识）、欠账 3（第 11、14 项：proto_min bot 解码 + `full` 模式 proto round-trip 测试）、欠账 4（第 14 项：morph TPV 渲染 harness 截图）、**欠账 5（第 6 项：生产 `races.json` 缺 `whale→human` pair，本体非人玩家无法易形化人——机制与 14 条 registry 测试均已落地，缺的是生产配置与走该配置的方向性集成测试）**。P4 状态维持 **⏳**，不再沿用「代表性文件即完成」的旧证据标准，也不再对任何一项明文测试交付物做「非核心」「跨阶段去重故不重复登记」的自行降级——同一根因可以在 P4 与 P5 两处同时登记，但不能因此从 P4 的统计中消失。

**交付物**：

- 解锁路径（对齐残卷传承正典 worldview.md §十 L890）：新残卷物品 `yixing_scroll`（TOML 进 `server/assets/items/`，`technique_scroll_spec.skill_id = "morph.yixing"`），掉落面收敛为两个已存在可核验入口——道伥击杀掉落表 + tsy 遗迹 loot 容器注册（`plan-tsy-loot-v1` 落地面），各配一条真实产出集成测试（击杀 / 开容器 → 背包出现 `yixing_scroll`），其余掉落面留后续；`TechniqueDefinition` 新增 `morph.yixing`（`required_realm = "Solidify"`）；经脉前置**不写死人形 channel id**（否则飞鲸构型无 Ren/Du，非人种族永远无法易形化人——自断「动物→人」方向）：`ChannelDef` 新增语义角色标签 `roles`（如 `form_anchor`），易形前置 = 「本体 plan 内全部 `form_anchor` 脉已通且未断」，humanoid 在自己 profile 里给 Ren/Du 标 `form_anchor`、whale 给对应奇经标注；human / whale 各配习得、施放、缺脉、断脉正反测试——**不改突破核心**，固元门槛由既有 `required_realm` 习得门天然承担
- 故事线锚点：功法描述文案引太初志「灵脉化形」存疑传闻作暗线（保留传闻外壳，不写实设定）；上古大能仅以「残卷来历不可考，笔迹类上古断代」措辞出现，不写「完整流传」
- server：`MorphState { form: RaceId, model_kind: u16, since_tick }` 组件；cast 走既有 skill 管线（qi_cost / cooldown / cast_ticks 走 `TechniqueDefinition` 既有字段，扣费归还走现行守恒路径）；易形配对唯一真源 = `RaceRegistry` 内全局 `morph_pairs` 索引（人↔指定异兽形，非任意变），查询唯一入口 `resolve_morph_pair(from, to)`，registry 校验重复配对冲突 fail-fast、双向对两方向**各自显式配置不假设可逆**；`part_mapping` 方向 = **form_part → intrinsic_part 的部分单射**（允许缺项——8 段人形对 6 段鲸形本就映不满；目标不重复），8→6 / 6→8 / 缺项 / 重复目标 / 悬空引用各配测试；死亡 / 濒死 / 下线自动解除
- **易形语义收敛（四层拆清，防「外观 only」自相矛盾）**：易形改变的是「外观 + 装备形态层」，不变的是「命中部位 / 经脉 / 碰撞箱 / 功法门」。具体：受击仍打本体 `HitGeometry`、伤口落本体部位（协议 hitbox 不动，绕开 valence 动态 EntityDimensions 深水区）；但**装备槽集合与 `validate_equip_to` 的 RaceGate 按当前形态（form plan）判定**——这就是「易形化人后可穿人形装备」的成立方式；form 形态所穿护甲的 `body_coverage` 经 `part_mapping` 折算回本体部位参与减免结算，无对应部位的 coverage 不生效
- 解除易形（手动 / 死亡 / 下线）时，不再满足本体 gate 的装备**自动卸入背包**（复用非空背包连货卸下路径），装不下走 dropped_loot——测试锁死「解除后无非法装备残留」
- 协议：新 payload `morph_state`（仿 daozhan disguise 骨架：版本化 JSON + join 全量 + 周期 sync + 变形瞬间 64 格半径广播），字段 `{v, mode: full|delta, entries: [{entity_id, model_kind, form_race_id, form_body_plan_id, active}]}`——join / 周期 sync 发 `full`（client 全量替换缓存），变形 / 解除瞬间半径广播发 `delta`（`active=false` 即删除项，解除即时生效不等周期拍）；respawn / 维度切换 / 重连的 entity_id 漂移靠下一拍 `full` 自愈；形态切换同拍更新 `cultivation_detail.form_body_plan_id`，目标 plan layout 未缓存时随 delta 补发；测试覆盖 start / 手动解除 / 死亡 / 重连 / 多实体并存 / 未知 plan id 的 decode→store→gate 状态转换
- client 渲染替换（本 plan 最大空白区，独立 PR）：
  - 激活并重写 `FakePlayerRendererMixin`：命中 morph 缓存的玩家实体 suppress vanilla `PlayerEntityModel` → 渲染 `BongEntityModelKind` 对应 geo 模型（fauna 渲染器已现成）；反向（异兽 NPC 化人形）复用 MineSkin + `EntityKind::PLAYER` 先例
  - FPV：第一版保持 vanilla 手臂不变（明确 scope，兽形 FPV 无先例不做）；TPV 自看生效
  - 名牌 / 队友标识保留（防 grief 误伤，对齐 disguise 半径过滤精神）
- 视听规格（active 前按 docs/CLAUDE.md 红线细化为逐招五项表格：animation / VFX / SFX / HUD / icon；此为骨架草案）：
  - 图标（仓库硬红线，每招必配）：`SkillDef.icon_id = "morph_yixing"` → `client/src/main/resources/assets/bong/textures/skill/<style>/morph_yixing.png`（视觉资产 PR 阶段 /gen-image item 档批量产出，骨架期标 [BLOCKED: 需 /gen-image 生成 morph_yixing icon]）+ schema / client `SkillIconRegistry` 注册 + SkillBar 显示回归 + 缺资源 fallback 测试；`yixing_scroll` 物品图标同批产出并配加载测试
  - 粒子：`BongRibbonParticle` 螺旋环绕 24 条、lifetime 30t、自下而上、#E8DFC8；`BongSpriteParticle` 白雾 burst 40 个、lifetime 20t、radial、复用 `mist` 贴图；`vfx_event = "bong:morph_yixing"`，新 `MorphVfxPlayer`
  - 音效：`audio_recipe yixing_cast.json`——layer1 `entity.evoker.prepare_wololo` pitch 0.8 vol 0.6 delay 0；layer2 `entity.illusioner.mirror_move` pitch 1.2 vol 0.5 delay 8t；layer3 `block.amethyst_block.chime` pitch 0.7 vol 0.4 delay 24t
  - HUD：左下形态图标（仅易形中显示，`HudRenderLayer` 新 morph 槽；结束淡出 10t）；施法期 vignette #FFFFFF opacity 0.15 fade-in 8t / fade-out 12t
  - 动画：`morph_cast.animation.json` endTick 30，蓄力鞠躬（torso pitch 0.35rad + legs 同向 0.35rad + body.z 前移 1.2，easeInOutQuad）
  - narration 示例（scope: zone / style: perception）：「灵光一敛，那道人影的轮廓塌了下去——再抬眼时，已是一头异兽伏在原地」；「你看见一头异兽的骨相在雾里折叠、拉长，最后立成了人形」
- **测试**：morph 状态机全转换（学→cast→active→手动解除 / 死亡解除 / 下线解除 / 重复 cast 幂等）；gate 矩阵（未习得 / 境界不足 / 经脉断 / 非法 pair 全拒）；payload 双端 sample + proto_min bot 解码；渲染 harness：morph 玩家截图（TPV 兽形 + 名牌保留）；qi 扣费锁 ledger 契约（断言玩家→zone 转账记录的方向与精确金额 + 守恒断言；覆盖余额不足不转账 / 施放失败不转账 / 成功仅记一次 / 重复请求幂等）

## P5 — 非人种族玩家入口 + 飞鲸 MVP + 收口

**⏳ 未完成欠账（2026-07-26 复核，2026-07-27 补充 #1 验收细化 + #4 措辞收窄，供下一次 consume 直接认领）（2026-07-27 第四轮返工：撤销 #1「双侧鳍环是原文范围要求」的错误推导，改为仅当产品需求明确时才拍板；#4 补做角色创建 / 首次登录完整走查并按实施形式分支验收，撤销「统一强制 RaceChange」）**：

1. **FinRing 真实消费链缺失**——现状：`grep -rn "FinRing\|fin_ring" server/src client/src agent/packages` 全仓 0 命中，`EquipSlotV1`（`server/src/schema/inventory.rs:39-54`）未新增该 enum 变体。要落：
   - server 装备容器存储 FinRing 槽位 + `validate_equip_to` 槽位分支 + 自动卸装路径 + wire 编解码：proto `EquipSlotV1` 新增变体 → TS union → `EquipSlot.java` 三端镜像，**新增变体需 pin**（proto 枚举数值 + TS union 字面量 + Java enum，同一 wire sample 三端解码对拍，防镜像漂移）
   - **实际物品**：新增鳍环物品模板（`ItemTemplate` TOML，`server/assets/items/`）+ 必要贴图 / 图标资源 + `wearer_race` / 对应 `EquipSlotV1` 槽位配置——不能只落空槽位基础设施而没有可穿的实物。**最小验收固定为：一个 `FinRing` enum 变体（三端 proto/TS/Java 镜像 + pin）+ 至少 1 件可真实穿戴的鳍环实物 + 完整消费链**（存储 / `validate_equip_to` 分支 / 自动卸装 / wire 编解码 / client 面板格）（2026-07-27 更正：本段此前写成「双侧左 / 右各一」，把原文的**可选区间**收紧成了**强制两件**——那是本 plan 之外的加码，已改回原文口径）
   - **双侧同时佩戴（2026-07-27 第四轮返工更正，对齐 review r7 major——此前一版把这段错误当成实施前必须拍板的架构前置，不是）**：`EquipSlotV1` 现状是**单个 enum 变体一件实物**——`equipped: HashMap<String, SlotContents>` 按 slot key 存取（`server/src/inventory/mod.rs:462,770`），`validate_equip_to` 对手部槽的多实例是靠精确 match 到不同变体分支实现的（`MainHand`/`OffHand`/`ExtraHand0`/`ExtraHand1`，`inventory/mod.rs:5817-5852`）。原文「装备件 **1-2 个**示范」是**跨槽位**（FinRing / Mouth / Back 等共同满足）的示范件数下限，不是「FinRing 必须能同时穿两只」的要求——原文从未提及左右鳍环同时佩戴，示范装备数量推不出单一槽位必须双实例。**仅当** P5 实施时产品需求确实要求左右鳍环同时独立穿戴，才需要在以下 A/B 之间拍板；否则单变体单件即满足原契约，不阻塞 P5 最小交付、也不是前置裁决。两个候选选项（仅供届时需要时参考）：
     - **选项 A**：拆两个独立变体 `FinRingL` / `FinRingR`，对齐 `MainHand`/`OffHand` 先例，三端（proto / TS / Java）镜像各加两个变体、各自 pin 测试；风险最低，直接复用现有「一变体一件」全部消费点模式。
     - **选项 B**：单变体 `FinRing` + 槽位实例序号，复用 `SlotContents.worn: Vec<ItemInstance>`（`inventory/mod.rs:660-665`，plan-layered-equip-v1 引入的分层穿戴栈）承载两件。**未核实**：该 `worn` 栈的分层语义（同类护甲基础层+外层叠穿）是否允许「同槽两件互不冲突的独立实物」而非「基础层/外层」这种层级关系，`validate_equip_to` 现有分层冲突规则会不会把第二只鳍环判成同类重复而拒绝——需要实施时逐分支核对，未核实前不能假定可行。
     - 若 B 不成立（分层校验拒绝同类重复），退回 A。本段只在「产品确实要求双侧同时佩戴」这一前提成立时才需要走 A/B 拍板，不替用户预先选，也不阻塞「单变体单件」这一最小验收的落地。
   - client 装备面板新槽位格 + 交互
   验收矩阵（固化，逐条断言；④⑤方向对齐 §P4「装备槽集合与 `validate_equip_to` 的 RaceGate 按当前形态（form plan）判定」——鲸形是本体、人形是易形后的 form，两条转换互为反向）：① 鲸形穿鳍环成功 ② 鲸形穿人形护甲 `RaceMismatch` 拒绝 ③ 易形成人后可穿人形护甲成功 ④ 易形起始（鲸→人，进入 form）：鲸形已穿的 FinRing 因不满足当前形态（人形）gate 自动卸入背包，满背包转 `dropped_loot` ⑤ 解除易形（人→鲸，回本体，含手动 / 死亡 / 下线三触发）：人形态所穿护甲因不满足本体（鲸形）gate 自动卸入背包，满背包转 `dropped_loot` ⑥ 持久化往返（存档→重载装备状态不丢） ⑦ 真实 bot e2e 穿卸场景（非 mock）。
2. **bot 场景 3 条只落 1 条**——现状：`scripts/bot/scenarios/` 下只有 `inventory_equip_wearer_race_reject.py`（对应①race gate 拒绝回执）。要落：② 易形 cast → morph payload 解码场景 ③ `body_plan_layout` 首帧解码场景，均接入 CI bot e2e stage。验收（2026-07-27 收紧；2026-07-27 第三轮返工再纠正 review r5 major——bot 是 Python 协议级脚本，只能断言它真正能观测到的 wire payload，不能断言 Java client 内部状态，此前一版验收把两者混了）：除脚本存在且被 e2e stage 真实引用外，每条场景必须断言**其能直接观测到的 wire payload**——② 断言收到的 `morph_state` payload 的 `mode`（`full`/`delta`）、`entries[].{entity_id, model_kind, form_race_id, form_body_plan_id, active}` 字段齐全且值符合该次 cast 的时序（含未缓存 plan id 时随 delta 补发那一拍的报文）；③ 断言首帧 `body_plan_layout` 的 plan id、部位集合与 server 端 `whale.json` / `humanoid.json` 一致。**`BodyPlanLayoutStore` / `MorphStateStore` 的缓存替换、监听器、未知 plan id 处理等 client 内部状态转换不属于 bot 场景验收范围，归 client 单测**——这部分**已经完成**，不是待落欠账：`client/src/test/java/com/bong/client/network/MorphStateHandlerTest.java`（full 多实体替换/full 清空/delta 插入/delta 移除/混合增删/未知 mode/缺 entity_id）+ `client/src/test/java/com/bong/client/morph/MorphModelRegistryTest.java`（`unknownFormRaceIdHasNoModel`）已覆盖对应场景，本条欠账只剩「bot 脚本本身尚未写」这一件事。**验收标准是「断言失败会撞红」，不是「脚本跑完退出 0」**。
3. **worldview 增补案未执行**——现状：`grep -c "易形" docs/worldview.md` = 0；worldview.md 最后一次改动是 `e69132fdf`（#836，2026-07-03），早于本 plan 立项（2026-07-10），说明种族后天路径 / 易形正典化 / 「异兽化形」词条消歧从未写入正典。这是本段落原文明写的**归档前硬前置**（见下方遗留 bullet 原文「归档前 land」）。要落：单独 PR + 人工 review，写入 worldview.md §六（零出生论 scope 澄清，对齐 §8.1 #1 决议）。验收：该 PR 合并，本 plan 补引其 commit hash。
4. **非人种族生产获得路径缺失**——现状（2026-07-27 第四轮返工补做完整走查，对齐 review r7 major——此前一版只核了「字段写入点 / 事件调用方 / 网络请求 / 物品消费」四类入口，自陈「未逐一走查角色创建 / 首次登录初始化流程本身」就把「已核验的四类入口未发现非 dev 手段」升级为确认欠账；本轮把这条走查真正做完）：
   - **默认构造**：`Cultivation` 的唯一 `Default` 实现（`server/src/cultivation/components.rs:650-664`）恒定 `race: default_race_id()` → `RaceId::new(HUMAN_RACE_ID)`（`components.rs:646-648`），没有任何分支能让默认构造产出非 human 值
   - **join / 首次登录生产入口**：`attach_cultivation_to_joined_clients`（`server/src/cultivation/mod.rs:566-678`，系统过滤器 `Or<(Added<Client>, Added<CurrentDimension>)>`，覆盖首次加入与维度切换两类触发）从 `let mut cultivation = Cultivation::default()`（`mod.rs:645`）起步，只有命中已持久化且 race 已知的 bundle 才会用 `serde_json::from_value::<Cultivation>` 覆盖（`mod.rs:665-673`）；未知 race 或首次加入（无 bundle）时保持默认值 human。持久化读取入口 `load_player_cultivation_bundle`（`server/src/persistence/mod.rs:6223-6247`）只回放此前已写入的 JSON，不产出新值
   - **角色创建 / 死后重开新角色**：全仓唯一带「新角色」语义的 client 请求是 `CombatCreateNewCharacter`（`agent/packages/schema/src/client-request.ts:380-387`，payload 只有 `{v, type}`，不带 race 字段），服务端路由到 `RevivalActionKind::CreateNewCharacter`（`server/src/network/client_request_handler.rs:2288-2296`）→ `reset_for_new_character`（`server/src/combat/lifecycle.rs:1703` 起）→ `cultivation::character_select::rotate_to_new_character`；`grep -n "race\|Race" server/src/cultivation/character_select.rs` 0 命中——新角色的出生位置 / 境界 / 寿命由该模块派生，但完全不涉及 race，新角色的 `Cultivation` 仍经上一条 join 路径以 `Cultivation::default()` 起步
   - **旧库迁移**：`backfill_legacy_player_cultivation`（`server/src/persistence/mod.rs:2354-2417`）迁移旧 sqlite 列时先 `Cultivation::default()`（`persistence/mod.rs:2387`）再覆盖 realm/qi_current/qi_max，race 字段不受影响，恒为 human
   - **唯一非 human 写入点**：`Cultivation.race` 生产代码里唯一赋值点是 `commit_race_change`（`server/src/cultivation/race_change.rs:257`），唯一生产调用方是 `/race set` dev 命令（`server/src/cmd/dev/race.rs:88`）；网络请求处理器 `grep -rln "RaceChangeRequest\|ChangeRace\|change_race" server/src client/src agent/packages` 0 命中，物品消费效果 `grep -rn "race" server/src/inventory server/src/alchemy` 命中均为 `wearer_race`/`yixing_scroll` 掉落注释，`grep -rn "秘法转生" server/src` 0 命中
   **结论（走查已完整覆盖默认构造 / join 首次登录 / 创建新角色 / 旧档迁移 / 唯一写入点五条链路，file:line 见上）**：确认全仓当前不存在任何非 dev 路径能让玩家 `Cultivation.race` 取到非 human 值——欠账成立，不是遗留的未走查免责声明。要落：按 §8.1 #1 决议在「创建期选择」或「后天秘法转生」二者中定形式并接出真实生产入口（非 dev 命令）。验收**按最终选定形式分支**（不再统一强制 `RaceChange`，对齐 review r7 major——创建期入口是初始身份建立，不存在旧种族到新种族的状态转换，强套 `RaceChange` 会把 §8.1 #1 授予的自由入口形式收窄成特定实现架构）：
   - 若选**创建期入口**：断言非 dev 的创建请求 / 配置校验、首次生成的 `Cultivation.race` 值、`BodyPlan` 解析、持久化落盘 + 重登恢复、`RaceRegistry` 校验拒绝非法 race id。
   - 若选**后天秘法转生**：断言 `RaceChange` prepare/commit 两阶段事务、状态转换、装备卸载、失败回滚（沿用 P5 `RaceChange` 事务交付物本身的测试要求）。

**交付物**：

- 非人种族获得路径（§8.1 #1 已拍板：零出生论只约束人族——入口形式自由，创建期选择或后天秘法转生皆合规，P5 实施时定形式）；`/race set <id>` dev 命令（brigadier，dev-only 绕过标注）
- `RaceChange` 两阶段事务（dropped_loot 实体与 ledger 转账均不可逆，**不承诺事后回滚**）：**阶段一纯预检**——只读计算装备去向（含背包容量核验 / 需掉落的清单）、经脉迁移结果（§8 #9 `meridian_mapping`）、`qi_max` 重算与 `qi_current` 超额 delta（qi 差额预检走 qi_physics 的 prepare / validated-transfer 接口，产出保证 apply 不失败的 `QiTransfer` plan——接口不存在则先扩 qi_physics 再 import，不在本 plan 内自实现）；任何检查不通过整体拒绝，世界零变更。**阶段二确定性提交**——阶段一产出完整已验证的 commit plan 后，阶段二只做**不返回错误的内存应用**（组件 race / 经脉替换 + 库存变更 + `QiTransfer` 应用，超额走 `qi_release_to_zone` 守恒归还 zone），掉落实体在提交完成后最后生成；**失败注入点只存在于提交前**（阶段一各检查），提交开始后无可失败步骤——不承诺也不需要事后回滚；预检与提交在**同一 Bevy exclusive system 调用内**完成（不跨 tick，期间无其他 system 可观察 / 修改相关组件，无需版本重验）。测试两组：① 逐预检子步注入失败，断言 race / 装备 / 背包 / 经脉 / qi_current / qi_max / zone 账本 / ledger 全部保持原值、世界无残留掉落实体；② 预检通过后提交，断言全量应用 + 守恒不变量成立；功法保留（习得史实不抹），cast 由 race gate 拦截
- `server/assets/body_plans/plans/whale.json` 完整数据：部位草案 6 段（颅 / 躯干 / 背鳍 / 左胸鳍 / 右胸鳍 / 尾鳍，尾鳍 = Locomotion、颅 = Sensory）；HitGeometry **只用 `PartBoxes` 模式**（与 P0 双模式定义一致，不再另写 AABB 分段——逐部位局部盒 + priority + 重叠规则，锚测试：左右鳍同高度区分 / 头尾纵向命中 / 边界擦触 / 最近交点）；经脉草案（条数与固元配额见 §8 #8）；`layouts/whale.json` 面板布局；装备槽草案（非人形首例：`FinRing` 类骨饰槽位，装备件 1-2 个示范；`EquipSlotV1` 以**新增 enum 变体**方式扩展 + proto / TS / Java 镜像同改——槽位集合小且稳定，不随部位 id 一起 string 开放化）
- FinRing 真实消费链（不止 wire enum）：server 侧装备容器存储 / `validate_equip_to` 槽位分支 / 自动卸装路径 / wire 编解码，client 侧装备面板新槽位格 + 交互；测试：穿 / 拒（RaceMismatch）/ 卸 / 满背包掉落 / 持久化往返 / bot e2e 完成一次真实穿卸
- 玩家以 whale 构型走通全链：面板渲染 / 受击部位判定 / 经脉开脉突破 / RaceGate 拒穿人形甲 / 易形化人后可穿
- bot 场景（`scripts/bot/scenarios/`，硬约定）：① race gate 拒绝回执 ② 易形 cast → morph payload 解码 ③ body_plan_layout 首帧解码；CI bot e2e stage 接入
- e2e：client 发易形 cast → server 结算 → 周边 client 收 morph_state → 渲染路径断言
- worldview 增补案（种族后天路径 + 易形正典化 + 「异兽化形」词条消歧）**单独 PR 人工 review**，归档前 land
- 遗留 / 后续（**不作为本 plan 验收与归档前置**）：`docs/library/` 配套馆藏（残卷来历考，/write-book 另行任务）；飞鲸可玩性另立 plan（§8.1 #7 已拍板；已定数据锚：**默认飞行、落地移速 0**——后续 plan 起点约束）

## §7 与既有系统关系声明（防近义重名红旗）

- **dandao 变异（`MutationState` / `BodySlot` / `MutationKind` / `MutationStage::Bestial`，`dandao/mutation.rs:97-113`）**：变异 = 永久·污染驱动·在人形构型上叠加改造（人形 + 兽征）；本 plan 种族 = 更换整套身体构型；易形 = 可逆·改「外观 + 装备形态层」（命中部位 / 经脉 / 碰撞箱 / 功法门不变——语义唯一定义见 P4，全文以彼为准）。三轴正交。**不复用 `MutationKind` 表达种族**——变异槽挂在人形 `BodySlot` 上，语义装不下异构构型。P0 给 humanoid plan 一张 `BodySlot → BodyPartId` 映射表，变异渲染继续走原链路；`MutationStage::Bestial` 与易形兽形的视觉区分在 P4 UI 层标注（变异 = 永久体征叠加，易形 = 整体模型替换）
- **拟态蛛 / 道伥 disguise（`SpiderDisguiseState` 等）**：NPC 侧换贴图伪装，与易形（跨端模型替换 + 玩家实体）不同层；仅复用其协议模式，不共享状态机
- **beast-horde（active）**：只读 `FaunaKind` 群体迁移，无撞面；本 plan 不改 `BeastKind` enum 本身，只加 `BeastKind → BodyPlanId` 派生
- **npc-skin 遗留坑**：`Beast / Disciple / GuardianRelic` archetype skin 未实装——P4 反向化形（异兽→人形）演示实体正好落其中一格
- **用词**：全 plan 用「异兽」不用「妖兽」；「化形」仅在引用剑修 manifest / 丹道材料时出现，不作本 plan 机制名

## §8 开放问题（升 active / P0 前必须补 §8.1 决议收口）

1. **零出生论 vs 玩家选种族**（worldview.md §六 L590 / L719 硬正典）。推荐：非人种族只能后天达成（固元后秘法转生 / 神魂夺舍异兽躯壳类路径），角色创建统一人族，把「选择」落在游玩行为里——合规且叙事更末法；若坚持创建期选择，需 worldview 修正案单独 PR 人工拍板。**用户拍板**
2. **机制命名**：「化形」被剑修 manifest（剑意化形）+ 丹道材料（化形根 / 化形大丹）占用。推荐「易形」（易形诀 / 易形残卷）。**用户拍板**
3. **上古大能措辞**：正典传承已断绝、只认残卷考古路径。推荐功法以残卷散佚形式出现，来历「笔迹类上古断代、不可考」，太初志「灵脉化形」传闻作暗线（保留存疑外壳）。已按此写入 P4，确认即可
4. **wire 开放化策略**：`MeridianId` / `CombatBodyPartV1` enum → string id，直接改新形状不留 dual-form（老存档仅需 humanoid 默认注入迁移）。风险：一次动 proto/TS/Java/samples 四面，P1 独立 PR 吸收
5. **易形后 gate 判定基准**：推荐**装备按当前形态**（易形核心收益——兽修化人穿人形甲；槽位与穿戴 gate 走 form plan，护甲 coverage 经 `morph_pairs.part_mapping` 折算回本体部位，解除时非法装备自动卸下——P4 已按此收敛语义）、**功法按本体 race**（经脉没变，功法门不该变）。用户需求原文「可以搭配不同种族装备」与此推荐一致，确认语义边界。**用户拍板**
6. **首批 Humanoid 档功法划定**：48 条存量全标 `Humanoid`，还是只标肢体强相关（剑修 / 体修 / 爆脉 / 卧牛）其余留 `Any`？推荐后者 + 逐条清单过目
7. **飞鲸可玩性边界**：本 plan 只到「数据 + 面板 + gate + 易形演示」；游泳 / 飞行移动、碰撞箱、出生点适配等可玩性问题**不进本 plan**（另立 plan），P5 验收允许 whale 玩家形态在测试场景内静态验证。确认 scope
8. **非人经脉数值曲线**：whale 经脉几条、各境界配额多少？推荐 P1 只锁机制（per-plan 曲线数据位），数值 P5 结合 halfstep buff 校准表拍；固元「12 正经全通」的人形语义在非人构型下如何等价（按比例 or 全通）需在 §8.1 定公式归属
9. **race 变更时的经脉迁移规则**（P5 `RaceChange` 阶段一的规则来源）：已开经脉如何处理？迁移表用**独立 `meridian_mapping`（channel id → channel id，一对一，registry 校验端点存在与唯一、禁一对多 / 多对一）**——不复用 `part_mapping`（那是部位对应表，表达不了经脉对应）。候选：a) 映射保留（有对应者继承 opened / progress / integrity / severed 全字段，无对应者丢弃）b) 全部重置（换构型 = 重修）c) 总进度折算注入。推荐 a)，`qi_max` 差额走 P5 守恒路径；未映射 severed 脉「丢弃 = 洗白永久断脉？」须在 §8.1 一并定死；与 #8 数值曲线联动收口

## §8.1 决议（用户拍板，2026-07-10，部分收口）

### #1 零出生论 vs 玩家选种族

**决议**：
1. 零出生论的约束范围是**人族**（人族玩家之间的差异只来自选择与经历），不覆盖所有生物——玩家扮演非人种族不构成违典。
2. 种族获得入口形式（创建期选择 / 后天转生）不再受零出生论约束，具体形式 P5 实施时定。
3. P5 worldview 增补案仍需把该 scope 澄清写进 §六（单独 PR 人工 review），因现行 L590 / L719 字面是「所有玩家差异」。

**落点**：§P5 入口 bullet / worldview.md §六 L590-L719（增补案）

### #2 机制命名

**决议**：定名**「易形」**（易形诀 / 易形残卷 / `morph.yixing`）。头部保留「为何不用化形」消歧说明，全文暂名标记移除。

**落点**：plan 头部命名说明 / §P4 标题

### #3 上古大能措辞

**决议**：确认残卷考古路径写法（来历不可考 + 太初志「灵脉化形」存疑传闻作暗线），维持 P4 现文。

**落点**：§P4 解锁路径 / 故事线锚点 bullet

### #6 Humanoid 档功法划定标准

**决议**：
1. 只有**强依赖人体专属结构**的功法才标 `Humanoid`——判据：依赖人体专属经脉拓扑语义，或人形肢体机能（持械双臂 / 腿法类）。
2. 飞剑类神识 / 真元驱动、不依赖人体结构的功法保持 `Any`。
3. 逐条清单按此标准由实施者判定，升 active 时附于 P3（配逐 skill_id pin 测试）。

**落点**：§P3 功法侧 bullet / §8 #6

### #7 飞鲸边界

**决议**：
1. 飞鲸可玩性（移动 / 出生点 / 碰撞箱适配）单独立 plan，本 plan 到静态验证为止。
2. 已定两条数据锚：**飞鲸默认飞行；落地移速 0**——作为后续飞鲸 plan 的起点约束，本 plan 的 whale body plan 不实现移动逻辑。

**落点**：§P5 whale / 遗留 bullet / plan-whale-playable（待立）

### #4 wire 开放化策略（2026-07-10 调研收口）

**决议**：
1. `MeridianId` / `CombatBodyPartV1` enum → string id **直改新形状**，不做 dual-form / 版本协商（全栈同版本原子部署惯例，对齐「不写兼容层」硬约束）。
2. 旧存档兼容只发生在反序列化迁移层（P1 bundle 迁移函数），wire 上不留旧形状；proto / TS / Java / samples 同一只 PR 改齐，decoder 对旧形状负向测试直接拒。

**落点**：§P1 wire 开放化 + 旧存档迁移 bullets

### #5 易形后 gate 判定基准（2026-07-10 收口）

**决议**：
1. **装备域按当前形态**（Species 判 `form_race_id`，Humanoid 判 `form_is_humanoid`）；**功法域按本体**（经脉未变，功法资格不随外形走）。
2. 护甲 coverage 经 `part_mapping`（form→intrinsic 部分单射）折算回本体部位结算；解除易形时不满足本体 gate 的装备自动卸入背包。
3. 依据：用户需求原文「可以搭配不同种族装备」+ #6 决议（功法门以人体结构依赖为准）自洽，用户对 P3/P4 矩阵无异议。

**落点**：§P3 身份快照矩阵 / §P4 易形语义收敛 bullet

### #8 非人经脉曲线（2026-07-10 调研收口）

**决议**：
1. 境界配额**公式即数据**：每构型在自身 `MeridianProfile.realm_requirements` 直接声明各境界所需 channel 数与子配额，「12 正经全通」这类人形语义由 humanoid.json 数据自表达，**不设全局换算公式**。
2. whale 草案（6 脉）：醒灵 1 / 引气 2 / 凝脉 3 / 固元 6（全通）/ 通灵 6 / 化虚 6；数值 P5 结合 halfstep buff 校准表定稿。

**落点**：§P1 `MeridianProfile` / §P5 whale 数据

### #9 race 变更经脉迁移规则（2026-07-10 调研收口）

**决议**：
1. 采用 a) `meridian_mapping` 映射保留：channel→channel 一对一，有对应者继承 opened / progress / integrity / severed 全字段。
2. 无对应通道的记录**不删除不洗白**：以原 channel id 挂入休眠登记（`MeridianSeveredPermanent` 同构扩展），换回原构型时按 id 恢复——**永久断脉不可通过换 race 洗白**。
3. `qi_max` 差额走 P5 两阶段守恒路径。

**落点**：§P5 RaceChange 阶段一 / §P1 `MeridianSeveredPermanent` 迁移

> §8 全部 9 项已收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §10 实施工作流（骨架草案，升 active 时按 docs/CLAUDE.md §六模板细化）

- 预计 6 PR 序列化（单 plan 多 PR，不拆 plan）：PR-1 P0 底盘 → PR-2 P1 经脉 + wire → PR-3 P2 面板 → PR-4 P3 匹配 → PR-5 P4 易形（server+协议 与 client 渲染可再拆两只）→ PR-6 P5 收口；worldview 增补案独立 PR 人工 review，P5 归档前 land
- 每 PR 走 consume-plan 通用流程 + push 前对峙自检 workflow；实施 subagent 全 sonnet，verify 用高档模型
- 渲染 / 布局类交付（P2 humanoid layout、P4 渲染替换）适用 3 轮打磨 + `<PROMISE>` 担保

## 复核结论（2026-07-26，2026-07-27 第三轮返工更新 P0/P4 状态）

**本 plan 曾被 #1278（`e2a2158b5`，「plan 整理审计」）单方面追认归档，本 PR 回退**。归档前该 commit 的父提交（`e2a2158b5^`）里 P4/P5 阶段总览仍是 `⬜ | —`，审计 diff 只有 14 行：把两行状态翻 `✅`、加一段「验证结论」、`git mv` 进 `finished_plans/`——**没有拿 P5 段落自己写的「交付物」清单逐条核对代码**。失效模式：审计以「P5 有 commit 落地（PR-6a/6b/6c）」为完成判据，但那三个 PR 只交付了 P5 清单里的**机制底盘**一部分，清单里另外四项（见下）在 origin/main 上一行代码、一个文件都不存在。

**P0–P4 落地实况**（2026-07-27 第三轮返工更新：P1/P2/P3 抓手仍为下方核验过的正确文件、状态不变；P0/P4 本轮各自发现问题，状态改 ⏳，详见下方「P0 契约裁决结论」与 §P4「⏳ 未完成欠账」——不再是「本次复核未改动这五阶段的 ✅」）：
- P0 种族 / 构型底盘：`server/src/body_plan/`（`BodyPlanRegistry` 等）+ `server/assets/body_plans/plans/humanoid.json`（#1160）——底盘真实落地，但「战斗部位消费点改造」交付物的完成口径未决，见下方裁决结论
- P1 经脉系统通用化：`MeridianSystem` 去定长 + wire 开放化（#1180，#1182 bughunt 修复）
- P2 动态部位 / 经脉面板：`BodyPlanLayoutV1` payload（#1184）
- P3 装备 / 功法种族三档匹配：`RaceGate`（#1198）
- P4 易形功法：本轮逐交付物核验（14 项交付物 11 项落地确认，2 项确认缺口），完整清单见 §P4「已核验落地清单」，不再只引 `morph.rs`/`MixinMorphedPlayerRenderer`/单条 coverage 测试这三个代表性文件

**注意**：#1278 归档时把 P4 client 渲染证据引成了 `client/src/main/java/com/bong/client/daozhan/FakePlayerRendererMixin.java`——那是 #436（道伥，2026-06-08，早于本 plan 立项 2026-07-10）留下的**未注册占位**，不属于本 plan 交付物，与 P0 调研摘要 §0「四大空白」里点名的「未注册空占位」是同一个文件。本次复核已在上面改引正确文件。

**P5 已落**（机制底盘部分）：
- PR-6a #1204：`/race set` dev 命令 + `RaceChange` 两阶段事务（`server/src/cultivation/race_change.rs`）+ qi_physics prepare + meridian_mapping
- PR-6b #1203：突破配额 + dugu 经脉路由换轨（非人构型通用化）
- PR-6c #1206：`server/assets/body_plans/plans/whale.json` + `IntrinsicRace` 接线 + 投射物半径派生

**P5 未落**（四项，详细欠账见下方 P5 段落清单）：FinRing 真实消费链、bot 场景（3 条只有 1 条）、worldview 增补案（P5 原文明写「归档前 land」的硬前置）、非人种族生产获得路径。

#1250（2026-07-23）是一处经脉显示回归修复，与本 plan 主体交付判定无关，不作为证据引用。

**遗留 / 后续**（承接 #1278 删掉的 Finish Evidence 里那条「status_snapshot 8 段 id 发散小 PR」——原措辞不准，此处按代码实况重写，不随归档回退一起丢账；**2026-07-27 纠正**：本段此前整段照抄了下方引用的 P0 期注释清单，未逐项核实其现状，见下方「纠错声明」）：

- **纠错声明**：`server/src/combat/components.rs:57-71` 那条「仍以 legacy `BodyPart` 工作的人形专属子系统……本轮不跟进迁移」的注释写于 P0 review r2（BLOCKING-2 收口）阶段，是 **P0 期状态快照**；P1b/P1c 已把其中列出的部分子系统迁移为数据驱动，但该注释本身未随之更新（stale）。本 plan 此前一版把这条注释当作「当前仍未迁移」的现状清单整段照抄，未逐项核实——这是错误，以下按 2026-07-27 实测重写。
- **P1 交付物已落地（wire 开放化 + 映射数据化，§P1 原文「交付物」bullet 3/5 对应）**：
  - `CombatBodyPartV1`（wire）已开放为 string：`server/src/schema/combat_event.rs:9` `pub struct CombatBodyPartV1(pub String)`，注释自陈「plan-race-system-v1 P1c」
  - `MeridianId`（wire）已开放为 string：`agent/packages/schema/src/cultivation.ts:58` `Type.String(...)`
  - `dugu.rs` 经脉↔部位映射已数据化：`cultivation/dugu.rs:539` 的 `body_part_to_meridian` **函数签名保留**（既有调用点无需改写）但内部实现改为查询 `body_plan::dugu_injection_channel`（数据唯一真源 = `humanoid.json meridian_profile.dugu_injection`），不再是硬编码 match 表；注释自陈「plan-race-system-v1 P1b —— 私表退役」
  - `dead_armor.rs` 经脉↔部位映射已数据化：`combat/baomai_v4/dead_armor.rs:213` 的 `meridian_to_body_part` 同样签名保留、内部改查 `body_plan::channel_body_part`（数据源 = `humanoid.json meridian_profile.channels[].body_part`）；注释自陈「plan-race-system-v1 P1b —— 私表退役」
- **P4 交付物已落地（form 护甲 coverage 折算，§P4 原文「易形语义收敛」bullet 对应）**：form 形态所穿护甲的 `body_coverage` 经 `part_mapping` 折算回本体部位已实现并有专项测试：`server/src/combat/resolve.rs:3554` `morphed_target_gets_real_armor_mitigation_via_part_mapping_fold_back`
- **范围收窄的权威来源（回应 review r3 P0 边界之争）**：`server/src/combat/components.rs:57-71` 那条注释自陈写于「plan-race-system-v1 P0 review r2（BLOCKING-2 收口）」——即「本轮只迁 `Wound.location` 及其伤残后果消费点（`combat::arm_wound` / `movement::leg_wound` 等），其余人形专属子系统本轮不跟进迁移」这个范围收窄，**是 P0 阶段自己的 review 在当时做出并接受的决议**，有 review 记录可查，不是本 PR 事后追加、也不是本 PR 新划的边界。本 PR 的性质是**回退 P5 被误归档的问题**，不是重开 P0 当年的验收边界；若认为 P0 当年这处范围收窄本身不当（该类子系统本该在 P0 一并迁移），应对应另立 plan 重新讨论收口，而不是在一次「回退误归档」的 PR 里追溯改判 P0 的验收范围。
- **⚠️ 本条存在未收口的对立意见（review r3/r4 连续三轮维持，r4 升为 blocker，r5 再次确认为 blocker）**：review 方的论点是——**「写在代码注释里的范围收窄」不等于「plan 交付物被修订」**。P0 的交付物文字（§P0「战斗部位消费点改造」）是契约，实施 PR 单方面在注释里缩小它、即便当轮 review 放行，也不构成对 plan 契约的正式修订；按这个口径，只要非人部位仍会退化到 legacy fallback，P0 就不该维持 ✅。**这个论点与本 plan 回退 #1278 所依据的原则同源**（#1278 的错误正是「实施侧的既成事实不能替代交付物清单的逐项核对」），因此不能简单当作过严意见驳回。
  - **P0 契约裁决结论（2026-07-27 第三轮返工，回应 review r5 blocker②「文内写着『P0 契约待用户裁决』却同时维持 P0 ✅」自相矛盾）**：认下——**改为 P0 ⏳，裁决后再定终态**，不再「维持 P0 ✅ 不变」。理由：P0「战斗部位消费点改造」交付物是否完成，取决于这个尚未裁决的口径问题本身——代码注释里的范围收窄（P0 review r2 BLOCKING-2 收口）是否构成对 plan 交付物的正式修订；这个口径没有被用户拍板之前，不能单方面把 P0 记为已完成状态发布进度。**P1 不受影响、维持 ✅**——P1 的两项交付物（wire 开放化 + dugu/dead_armor 映射数据化）本轮已逐条核验（见上方「P1 交付物已落地」），与 P0 这处范围收窄是否被正式修订无关，P1 自己的交付物边界从未包含下方五类遗留内部类型。
  - **待用户裁决的二选一（保留不变）**：(a) 认可 P0 当年的范围收窄，则应把它从代码注释**升格为本 plan 的正式 amendment 段落**（写明收窄内容、发生在 P0 review r2、以及未覆盖子系统的清单），让契约与实现一致，P0 可改回 ✅；(b) 不认可，则下方五类消费点需要在 P0 范围内补齐迁移（或另立 plan 收口），P0 在此之前维持 ⏳。**裁决前不要再由 agent 单方面改动 P0 的状态**（本轮 ✅→⏳ 是执行用户在本次返工任务里的显式指示，不是 agent 自行改判；此后的状态变化仍需同样的显式授权）。
- **P0 待确认欠账**：以下均是 Rust **内部类型 / 字段**仍以 legacy `BodyPart` 作 key（非 wire——wire 层 `CombatBodyPartV1` 已如上确认是 string，`CombatEvent` 是纯战斗运行时事件结构体、从不上 wire），且均**不在 P1（wire 开放化 + dugu/dead_armor 映射数据化）或 P4（form 护甲 coverage 折算）任一交付物清单内**（对照 §P1/§P4 原文「交付物」段，P1/P4 本身的交付物边界不受影响）：
  - `CombatEvent.body_part`（`combat/events.rs:204`，战斗内部事件结构体字段，非 wire）
  - `DerivedAttrs.defense_profile`（`combat/components.rs:320`，`HashMap<(BodyPart, WoundKind), f32>`）
  - `DeadMeridianArmor.immune_regions`（`combat/baomai_v4/dead_armor.rs:182`，`HashSet<BodyPart>`）
  - 状态效果 `StatusEffectKind::BodyPartResist` / `BodyPartWeaken`（`combat/events.rs:146,148`，枚举变体参数类型）
  - dandao 变异伤害倍率 `mutation_damage_multiplier_for_part`（`dandao/mutation.rs:249`，`part: BodyPart` 参数）
  非人形构型在上述子系统里仍会退化：whale 的 `tail_fin` 之类 id 转 legacy 失败即走各消费点的兜底分支（`body_plan::id_to_legacy_body_part` 返回 `None`）。这五项是否需要在 P0 范围内补齐迁移，取决于上方裁决结论——裁决前登记为 P0 待确认欠账，不再是「不影响 P0 ✅」的旁注。
