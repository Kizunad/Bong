# plan-race-system-v1 — 通用身体构型（BodyPlan）与种族系统 + 固元「易形」功法

一句话主题：把「部位 / 经脉 = 人形硬编码」重构为按种族（BodyPlan）数据驱动的通用系统——每种 Entity 可定义各自的部位集合与经脉拓扑；装备 / 功法接入种族三档匹配（种族专属 / 人形通用 / 全通用）；固元境经残卷解锁「易形」功法（外观与装备形态层一对一互换，命中部位 / 经脉 / 碰撞箱不变），为玩家可扮演非人种族（如飞鲸）铺路。

> **状态：active（实施中，2026-07-10 升格）**。§8 全部开放问题已在 §8.1 收口（2026-07-10：#1/#2/#3/#6/#7 用户拍板，#4/#5/#8/#9 调研收口）；review 引擎 5 轮意见已全量吸收，第 5 轮后按用户指示封轮。
>
> 机制命名说明：用户原始需求称「化形功法」，但「化形」在仓库已被两处占用（剑修「剑意化形」= manifest 实体化；丹道材料「化形根 / 化形大丹」），本 plan 定名**「易形」**（2026-07-10 用户拍板，§8.1 #2）。

## 阶段总览

| 阶段 | 主题 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 种族 / 构型底盘：`BodyPlanRegistry` 数据驱动 + `Race` 字段 + 战斗部位消费点改造 | ✅ | 2026-07-11 |
| P1 | 经脉系统通用化：`MeridianSystem` 去定长 + per-plan 拓扑与境界配额 + wire 开放化 | ✅ | 2026-07-12 |
| P2 | 动态部位 / 经脉面板：server 下发布局元数据，client 剪影与经脉图数据驱动 | ✅ | 2026-07-13 |
| P3 | 装备 / 功法种族三档匹配：`RaceGate` 三收拢点接线 + UI 反馈 | ✅ | 2026-07-13 |
| P4 | 易形功法：固元残卷解锁、外观一对一互换、玩家渲染替换链路 | ✅ | 2026-07-26 |
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

**⏳ 未完成欠账（2026-07-26 复核，供下一次 consume 直接认领）**：

1. **FinRing 真实消费链缺失**——现状：`grep -rn "FinRing\|fin_ring" server/src client/src agent/packages` 全仓 0 命中，`EquipSlotV1` 未新增该 enum 变体。要落：server 装备容器存储 FinRing 槽位 + `validate_equip_to` 槽位分支 + 自动卸装路径 + wire 编解码（proto `EquipSlotV1` 新增变体 → TS union → `EquipSlot.java` 三端镜像）；client 装备面板新槽位格 + 交互。验收：穿 / 拒（RaceMismatch）/ 卸 / 满背包掉落 / 持久化往返 5 条测试 + 1 条 bot e2e 真实穿卸。
2. **bot 场景 3 条只落 1 条**——现状：`scripts/bot/scenarios/` 下只有 `inventory_equip_wearer_race_reject.py`（对应①race gate 拒绝回执）。要落：② 易形 cast → morph payload 解码场景 ③ `body_plan_layout` 首帧解码场景，均接入 CI bot e2e stage。验收：三个场景脚本存在且被 e2e stage 引用、CI 绿。
3. **worldview 增补案未执行**——现状：`grep -c "易形" docs/worldview.md` = 0；worldview.md 最后一次改动是 `e69132fdf`（#836，2026-07-03），早于本 plan 立项（2026-07-10），说明种族后天路径 / 易形正典化 / 「异兽化形」词条消歧从未写入正典。这是本段落原文明写的**归档前硬前置**（见下方遗留 bullet 原文「归档前 land」）。要落：单独 PR + 人工 review，写入 worldview.md §六（零出生论 scope 澄清，对齐 §8.1 #1 决议）。验收：该 PR 合并，本 plan 补引其 commit hash。
4. **非人种族生产获得路径缺失**——现状：`grep -rn "commit_race_change" server/src --include=*.rs | grep -v race_change.rs` 唯一命中 `server/src/cmd/dev/race.rs`（dev-only 命令）；`grep -rn "秘法转生" server/src` 0 命中——玩家在生产环境没有任何非 dev 手段获得非人种族，下方「`/race set` dev 命令」不能顶替这条。要落：按 §8.1 #1 决议在「创建期选择」或「后天秘法转生」二者中定形式并接出真实生产入口（非 dev 命令）。验收：新增一条非 dev 触发路径 + 集成测试覆盖该路径下的 `RaceChange` 全链路。

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

## 复核结论（2026-07-26）

**本 plan 曾被 #1278（`e2a2158b5`，「plan 整理审计」）单方面追认归档，本 PR 回退**。归档前该 commit 的父提交（`e2a2158b5^`）里 P4/P5 阶段总览仍是 `⬜ | —`，审计 diff 只有 14 行：把两行状态翻 `✅`、加一段「验证结论」、`git mv` 进 `finished_plans/`——**没有拿 P5 段落自己写的「交付物」清单逐条核对代码**。失效模式：审计以「P5 有 commit 落地（PR-6a/6b/6c）」为完成判据，但那三个 PR 只交付了 P5 清单里的**机制底盘**一部分，清单里另外四项（见下）在 origin/main 上一行代码、一个文件都不存在。

**P0–P4 落地实况**（本次复核未改动这五阶段的 ✅，抓手改为核验过的正确文件）：
- P0 种族 / 构型底盘：`server/src/body_plan/`（`BodyPlanRegistry` 等）+ `server/assets/body_plans/plans/humanoid.json`（#1160）
- P1 经脉系统通用化：`MeridianSystem` 去定长 + wire 开放化（#1180，#1182 bughunt 修复）
- P2 动态部位 / 经脉面板：`BodyPlanLayoutV1` payload（#1184）
- P3 装备 / 功法种族三档匹配：`RaceGate`（#1198）
- P4 易形功法：`server/src/body_plan/morph.rs`（`MorphState` / `cast_morph_yixing`，#1201）+ client `client/src/main/java/com/bong/client/mixin/MixinMorphedPlayerRenderer.java`（已注册于 `bong-client.mixins.json:19`，#1202）

**注意**：#1278 归档时把 P4 client 渲染证据引成了 `client/src/main/java/com/bong/client/daozhan/FakePlayerRendererMixin.java`——那是 #436（道伥，2026-06-08，早于本 plan 立项 2026-07-10）留下的**未注册占位**，不属于本 plan 交付物，与 P0 调研摘要 §0「四大空白」里点名的「未注册空占位」是同一个文件。本次复核已在上面改引正确文件。

**P5 已落**（机制底盘部分）：
- PR-6a #1204：`/race set` dev 命令 + `RaceChange` 两阶段事务（`server/src/cultivation/race_change.rs`）+ qi_physics prepare + meridian_mapping
- PR-6b #1203：突破配额 + dugu 经脉路由换轨（非人构型通用化）
- PR-6c #1206：`server/assets/body_plans/plans/whale.json` + `IntrinsicRace` 接线 + 投射物半径派生

**P5 未落**（四项，详细欠账见下方 P5 段落清单）：FinRing 真实消费链、bot 场景（3 条只有 1 条）、worldview 增补案（P5 原文明写「归档前 land」的硬前置）、非人种族生产获得路径。

#1250（2026-07-23）是一处经脉显示回归修复，与本 plan 主体交付判定无关，不作为证据引用。

**遗留 / 后续**（承接 #1278 删掉的 Finish Evidence 里那条「status_snapshot 8 段 id 发散小 PR」——原措辞不准，此处按代码实况重写，不随归档回退一起丢账）：

- **legacy `BodyPart` 8 段与通用 `BodyPartId`（string）并存的残留耦合**。P0 只把 `Wound.location` 及其伤残后果消费点（`combat::arm_wound` / `movement::leg_wound` / 减速 / 眩晕 / 脱手）迁到 `BodyPartId`；`server/src/combat/components.rs:57-71` 的注释自陈「本轮**不**跟进迁移」的人形专属子系统仍以 legacy enum 工作：`CombatEvent.body_part` wire、`DerivedAttrs.defense_profile`（`components.rs:320` 的 `HashMap<(BodyPart, WoundKind), f32>`）、护甲 `body_coverage`、`DeadMeridianArmor.immune_regions`、`dugu::body_part_to_meridian`、状态效果 `BodyPartResist` / `BodyPartWeaken`、dandao 变异伤害倍率（经 `body_plan::id_to_legacy_body_part` 转换，非 8 段 id 返回 `None`）。
- **不阻塞本 plan 归档**（P5 四项欠账才是阻塞项），但非人形构型在上述子系统里仍会退化：whale 的 `tail_fin` 之类 id 转 legacy 失败即走各消费点的兜底分支。真正需要非人形护甲 / 部位抗性 / 变异时，应另立 plan 收口，不要塞进本 plan 的 P5。
