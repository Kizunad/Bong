# plan-gathering-tool-bind-v1 — 草药捆保鲜挂载 + 草镰采集本职接通(active)

> 一句话:僵尸物品审计的两件"临门一脚"适配——herb_bundle(草药捆)挂上已存在的 shelflife profile,cao_lian(草镰)成为割手草本的 required_tool,补齐采集本职闭环。
>
> 来源:材料断链调查 workflow 2026-06-10(opus 抽查 5/5 证据属实);用户授权自治裁决:「该做适配的适配」。删除类 9 件见 [[plan-economy-zombie-cleanup-v1]] P3。

**依赖**:无。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | herb_bundle:去重配方 + shelflife 挂载 | ⬜ |
| P1 | cao_lian:required_tool 接通(割手草本) | ⬜ |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - shelflife:`shelflife/registry.rs:208-216` `fresh_herb_v1` profile **已存在**,herb_bundle 只差模板字段挂载
  - 配方:`workbench_recipes.rs:346-351` 已注册 `workbench.process.herb_bundle`(`time=10s`)；`workbench_recipes.rs:1929-1935` 是 spot-check 测试用例,不是第二个注册点。`CraftRegistry::register()` 对重复 id 返回 `RegistryError::DuplicateId`(`registry.rs:45-49`),现状无重复注册,只需 P0 加唯一性/回归 pin 防止旧红旗复发。
  - 工具:`tools/kinds.rs:7,19,44` `ToolKind::CaoLian` 已注册(战斗兜底 1.10x),forge/workbench 双路可造
  - required_tool 机制:`botany/registry.rs:238-241` `HarvestHazard::WoundOnBareHand { wound, required_tool }`(5 株已用:DunQiJia×2/GuaDao×2/BingJiaShouTao×1)+ `botany/harvest.rs:533-544` `required_tool_for()`(耐久消耗+受伤判定)
- **出料**:草药捆进保鲜循环(批量存放减损耗);草镰成为割手草本的安全采集工具(徒手采=Laceration 受伤,持镰=免伤+耐久消耗)
- **共享类型 / event**:全部复用 shelflife profile / HarvestHazard / ToolKind,零新枚举零新系统
- **跨仓库契约**:纯 server(TOML + registry 常量);client 无改动(受伤/耐久 HUD 已有通道)
- **worldview 锚点**:§十 资源与匮乏(草药保鲜与采集风险);shelflife 体系正典(plan-shelflife-v1,finished)
- **qi_physics 锚点**:无。保鲜衰减走 shelflife 既有 profile(其底层已对齐 qi_physics),本 plan 只挂载不调参。

---

## P0 — herb_bundle 去重 + shelflife 挂载

- **唯一性回归**(旧红旗 R1):确认 `CraftRegistry::register()` 对重复 id 的行为,并 pin `workbench.process.herb_bundle` 只有一个注册结果；当前 `workbench_recipes.rs:346-351` 为唯一真实定义,`workbench_recipes.rs:1929-1935` 只是测试 spot-check。
- `workbench_materials.toml:142-150` herb_bundle 加 shelflife 字段,挂 `fresh_herb_v1` + `shelflife_track = "spoil"`；不新增 `bundled_herb_v1`,避免本 plan 自定保鲜数值。
- 测试:重复 id 行为 pin 测试(去重后 registry 单一命中);herb_bundle 实例随时间衰减曲线;捆 vs 单株衰减对照;过期行为(腐坏产物)分支

## P1 — cao_lian required_tool 接通

- 给 1-2 株"丛生/锐叶割手"类草本(候选:具备 v2 spec 且 required_tool=None 的丛生草本;**不选 spirit_grass**——最基础灵草不应设工具门槛,registry.rs:668-678 其 v2=None 维持)加 `HarvestHazard::WoundOnBareHand { wound: Laceration, required_tool: Some(ToolKind::CaoLian) }`
- 兼容性(已核验):required_tool(ToolKind 系统,受伤/耐久)与 gather_time(GatheringToolSpec 系统,速度/品质)**互不影响**;其他植物 required_tool 仍 None,徒手流程不破
- **不做**速度/品质加成(GATHERING_TOOL_SPECS 加 Sickle 变体)——与 bao_chu 已覆盖 Herb 目标的定位冲突需重新平衡,留作后续(见 §8 #3)
- 视听:持镰收割 SFX `block.grass.break`(pitch 0.8,vol 0.9)+ 草屑横扫粒子(BongSpriteParticle burst 8 颗,沿挥镰弧线,lifetime 8t,#7FA86A);徒手采割手瞬间 SFX `entity.player.hurt`(vol 0.5)+ 细红痕粒子(burst 3 颗 #C04848)+ HUD 事件流「叶缘割手」
- 测试:持镰免伤+耐久递减 / 徒手 Laceration 命中 / 镰耐久归零后等同徒手 / 目标植物外徒手不受伤(回归);每株候选专属用例

---

## §8 开放问题(P0 决策门前需收口)

1. **herb_bundle 配方真实意图**:time=10(:386)还是 time=200(:1812)?去重保留哪侧(建议 10s——捆扎是轻加工)
2. **bundle 保鲜倍率**:直接挂 fresh_herb_v1 还是派生 `bundled_herb_v1`(衰减减半,体现"批量存放减损耗"的设计语义)——倾向后者;plan-shelflife-v1 已归档无主,新 profile 数值**本 plan 内自决**(registry.rs 加一条 profile 常量,沿用 fresh_herb_v1 结构减半)
3. **草镰速度/品质加成**:本 plan 不做;若将来做,需 GatheringToolKind 加 Sickle 变体并与 bao_chu 重新平衡(登记待办)
4. **目标植物名单**:候选丛生草本的最终 1-2 株(实施前 grep v2 spec 现状定名单)

全部已在 §8.1 收口。原表保留以备追溯，**实施时以 §8.1 决议为准**。

## §8.1 决议（pre-P0 收口，2026-06-14）

### #1 herb_bundle 配方真实意图

**决议**：
1. 保留 `workbench.process.herb_bundle` 的 `time=10s` 语义，不引入 `time=200s`。
2. 当前代码没有第二个真实注册点：`server/src/craft/workbench_recipes.rs:346-351` 是定义，`server/src/craft/workbench_recipes.rs:1929-1935` 是 spot-check 测试；P0 做唯一性 pin 与 spot-check 强化，不做不存在的删除。
3. `CraftRegistry::register()` 已在 `server/src/craft/registry.rs:45-49` 对重复 id 返回 `RegistryError::DuplicateId`，并由 `server/src/craft/registry.rs:131-138` 测试锁住；P0 不改注册语义。

**落点**：`server/src/craft/workbench_recipes.rs:346` / `server/src/craft/workbench_recipes.rs:1929` / `server/src/craft/registry.rs:45` / plan P0。

### #2 bundle 保鲜倍率

**决议**：
1. P0 直接给 `herb_bundle` 挂 `fresh_herb_v1` + `shelflife_track = "spoil"`，不派生 `bundled_herb_v1`。
2. 理由：本 plan 标题与接入面限定为"挂上已存在的 shelflife profile"，`fresh_herb_v1` 已在 `server/src/shelflife/registry.rs:208-215` 注册；新增 profile 会引入新衰减数值,超出"只挂载不调参"的 qi_physics 锚点。
3. 若后续需要"草药束半速衰减"的运营差异,另立平衡 plan 调整 shelflife profile,不要塞进本次临门适配。

**落点**：`server/assets/items/workbench_materials.toml:142` / `server/src/shelflife/registry.rs:208` / plan P0。

### #3 草镰速度/品质加成

**决议**：
1. 本 plan 不新增 `GatheringToolKind::Sickle`，不做速度/品质加成，只接 `HarvestHazard::WoundOnBareHand { required_tool: Some(ToolKind::CaoLian) }`。
2. `ToolKind::CaoLian` 已在 `server/src/tools/kinds.rs:7`、`server/src/tools/registry.rs:16-24` 映射；`required_tool_for()` 已在 `server/src/botany/harvest.rs:533-544` 消费 required_tool。
3. 与 `bao_chu` 等采集速度/品质平衡的冲突留后续 plan；P1 只做安全采集工具本职。

**落点**：`server/src/tools/kinds.rs:7` / `server/src/tools/registry.rs:16` / `server/src/botany/harvest.rs:533` / plan P1。

### #4 目标植物名单

**决议**：
1. P1 选择 `DuanJiCi` 与 `XueSeMaiCao` 两株 v2 草本加草镰 required_tool。
2. 选择依据：`DuanJiCi` 当前 `base_mesh_ref = "sweet_berry_bush"` 且战场刺丛语义强；`XueSeMaiCao` 当前 `base_mesh_ref = "tall_grass"`、BloodValley 高密度丛生,更符合"锐叶割手/收割"。二者现有 hazard 分别位于 `server/src/botany/registry.rs:173-180`、`server/src/botany/registry.rs:1032-1048`，可在不影响 SpiritGrass v1 基础草药的前提下叠加 Laceration。
3. 不选择 `spirit_grass`；其 v1 基础灵草定位已有 `server/src/botany/registry.rs:1635` 测试锁定。

**落点**：`server/src/botany/registry.rs:173` / `server/src/botany/registry.rs:1032` / `server/src/botany/registry.rs:1635` / plan P1。
