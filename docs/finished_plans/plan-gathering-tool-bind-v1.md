# plan-gathering-tool-bind-v1 — 草药捆保鲜挂载 + 草镰采集本职接通(active)

> 一句话:僵尸物品审计的两件"临门一脚"适配——herb_bundle(草药捆)挂上已存在的 shelflife profile,cao_lian(草镰)成为割手草本的 required_tool,补齐采集本职闭环。
>
> 来源:材料断链调查 workflow 2026-06-10(opus 抽查 5/5 证据属实);用户授权自治裁决:「该做适配的适配」。删除类 9 件见 [[plan-economy-zombie-cleanup-v1]] P3。

**依赖**:无。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | herb_bundle:去重配方 + shelflife 挂载 | ✅ 2026-07-27 |
| P1 | cao_lian:required_tool 接通(割手草本) | ✅ 2026-07-27 |

---

## 接入面(防孤岛 checklist)

- **进料**:
  - shelflife:`shelflife/registry.rs:208-216` `fresh_herb_v1` profile **已存在**,herb_bundle 只差模板字段挂载
  - 配方:`workbench_recipes.rs:346-351` 已注册 `workbench.process.herb_bundle`(`time_sec=10`,helper 会转成 `10 * 20` ticks)；`workbench_recipes.rs:1929-1935` 是 spot-check 测试用例,不是第二个注册点。`CraftRegistry::register()` 对重复 id 返回 `RegistryError::DuplicateId`(`registry.rs:45-49`),现状无重复注册,只需 P0 加唯一性/回归 pin 防止旧红旗复发。
  - 工具:`tools/kinds.rs:7,19,44` `ToolKind::CaoLian` 已注册(战斗兜底 1.10x),forge/workbench 双路可造
  - required_tool 机制:`botany/registry.rs:238-241` 是 `HarvestHazard::WoundOnBareHand { wound, required_tool }` 使用例(5 株已用:DunQiJia×2/GuaDao×2/BingJiaShouTao×1)+ `botany/harvest.rs:533-544` `required_tool_for()`(耐久消耗+受伤判定)
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
1. 保留 `workbench.process.herb_bundle` 的 `time=10s` 语义；真实注册行写的是 `time_sec=10`，由 `RecipeRow` helper 转换为 `10 * 20` ticks，不引入 `time=200s`。
2. 当前代码没有第二个真实注册点：`server/src/craft/workbench_recipes.rs:346-351` 是定义，`server/src/craft/workbench_recipes.rs:1929-1935` 是 spot-check 测试；当前无需删除操作，P0 做唯一性 pin 与 spot-check 强化即可。
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
2. 选择依据：`DuanJiCi` 当前 `base_mesh_ref = "sweet_berry_bush"` 且战场刺丛语义强；`XueSeMaiCao` 当前 `base_mesh_ref = "tall_grass"`、BloodValley 高密度丛生,更符合"锐叶割手/收割"。现有 hazard 分别位于 `server/src/botany/registry.rs:173-176` 与 `server/src/botany/registry.rs:177-180`，`XueSeMaiCao` v2 spec 位于 `server/src/botany/registry.rs:1032-1048`，可在不影响 SpiritGrass v1 基础草药的前提下叠加 Laceration。
3. 不选择 `spirit_grass`；其 v1 基础灵草定位已有 `server/src/botany/registry.rs:1635` 测试锁定。

**落点**：`server/src/botany/registry.rs:173` / `server/src/botany/registry.rs:1032` / `server/src/botany/registry.rs:1635` / plan P1。

---

## Finish Evidence

### 落地清单

- **P0**：`server/assets/items/workbench_materials.toml`（herb_bundle 挂 `shelflife_profile = "fresh_herb_v1"` + `shelflife_track = "spoil"`）；`server/src/craft/workbench_recipes.rs`（`herb_bundle_recipe_registered_exactly_once` / `re_registering_herb_bundle_recipe_id_is_rejected_as_duplicate` 唯一性回归 pin）；`server/src/inventory/mod.rs`（`herb_bundle_item_template_has_shelflife_profile_set` / `runtime_instance_from_template_attaches_freshness_for_herb_bundle` / `herb_bundle_decay_curve_reaches_spoiled_state_over_three_game_days` / `herb_bundle_decays_identically_regardless_of_stack_count`[PR #1293 review 修正：从"同 initial_qi 自比较"改为经生产 `runtime_instance_from_template` 分别构造 stack_count=1/50 实例对照，已用假实现验证会撞红] / `herb_bundle_freshness_ignores_stack_count` / `herb_bundle_expiry_drives_production_spoil_check_consumption_path`[PR #1293 review 新增：驱动生产入口 `shelflife::consume::spoil_check`（`food.rs::consume_food` 同款）走过 Safe→Warn→CriticalBlock 三段，锁住"过期行为"分支]）。
- **P1**：`server/src/botany/registry.rs`（`DuanJiCi` / `XueSeMaiCao` 叠加 `HarvestHazard::WoundOnBareHand{ Laceration, required_tool: CaoLian }`）；`server/src/botany/hazard.rs`（`apply_completion_hazards` 返回值改为 `bool`，标记徒手割手是否命中；PR #1293 review 修正：`bare_hand_wound_applied` 改为仅在 Wound 成功写入后才置位，补 wounds=None/Some 两条对照测试）；`server/src/botany/harvest.rs`（`HarvestTerminalEvent` 消费方计算 `bare_hand_wound` / `required_tool_used` / `required_tool_kind`[PR #1293 review 新增字段] 正交字段）；`server/src/botany/components.rs`（`HarvestTerminalEvent` 新增 `required_tool_kind: Option<ToolKind>` 字段，携带目标植物 hazard 声明的 required_tool，供下游甄别"是否真的是草镰"）。
- **视听接线**（P1 规格内联段，非独立阶段）：`server/assets/audio/recipes/cao_lian_harvest_swing.json` + `botany_bare_hand_wound.json`（新 audio recipe，走既有 server 权威运行时）；`server/src/network/audio_trigger.rs`（`emit_botany_audio_triggers` 扩展差异化 SFX 分支；PR #1293 review 修正：额外校验 `required_tool_kind == Some(ToolKind::CaoLian)`，收窄到草镰专属，防 DunQiJia/GuaDao/BingJiaShouTao 等既有 required_tool 草本泄漏草镰反馈，补 4 条正反向回归）；`server/src/network/vfx_animation_trigger.rs`（`emit_botany_harvest_visual_triggers` 同款收窄到 CaoLian；新增 `emit_spawn_particle_with_direction`，草镰挥砍分支传入"玩家→采集目标"水平方向向量，接通 plan P1"沿挥镰弧线"视听规格，补方向断言 + 正反向回归测试）；`client/src/main/java/com/bong/client/visual/particle/BotanyHarvestBurstPlayer.java`（PR #1293 review 新增 client 改动：消费 `direction` 时把 8 颗粒子的出射角约束到该方向 ±55° 扇形范围内，替代原全向 360° 随机 burst，无 direction 的调用——普通采集 burst / 徒手割手细红痕——保持原行为不变）；`server/src/network/event_stream_emit.rs`（`emit_botany_harvest_wound_to_event_stream` system，推「叶缘割手」HUD 提示；PR #1293 review 修正：同款收窄到 `required_tool_kind == Some(ToolKind::CaoLian)`，补 1 条负向回归）；`server/src/network/mod.rs`（system 注册）；`server/src/audio/mod.rs`（audio registry 计数 pin 271→273 + 2 条 recipe 数值回归测试）。

### 关键 commit

- `e2846ed50`（2026-07-27）P0：herb_bundle 挂载 fresh_herb_v1 保鲜 profile + 配方唯一性回归 pin。
- `51cb428d0`（2026-07-27）P1：草镰(cao_lian)接通 required_tool 本职——DuanJiCi/XueSeMaiCao 割手草本。
- `4515619ec`（2026-07-27）视听接线：草镰持镰收割 vs 徒手割手差异化 SFX/VFX/HUD。
- `971ad5c32`（2026-07-27）PR #1293 review 返工①：P0 捆vs单株对照改为 stack_count 判别式对照（经生产 `runtime_instance_from_template`，已用假实现验证撞红）+ 补腐坏产物分支生产路径测试（`shelflife::consume::spoil_check`）。
- `4b296384a`（2026-07-27）PR #1293 review 返工②：草镰专属反馈收窄到 `ToolKind::CaoLian`（`HarvestTerminalEvent` 加 `required_tool_kind`，audio_trigger.rs 门禁 + DunQiJia 正反向回归）。
- `cd488c158`（2026-07-27）PR #1293 review 返工②③：VFX 消费方同款收窄到 CaoLian + 草镰挥砍粒子接通"沿挥镰弧线"方向参数（server `emit_spawn_particle_with_direction` + client `BotanyHarvestBurstPlayer` ±55° 扇形约束）。
- `9c92c15e5`（2026-07-27）PR #1293 review 返工④：hazard.rs 仅在成功写入 Wounds 后才报告徒手割手实际命中，补 wounds=None/Some 对照测试。

### 测试结果

- 定向测试（consume 时跑过一次）：`inventory::` 508 passed、`craft::` 212 passed、`botany::` 149 passed、`network::vfx_animation_trigger::tests` 93 passed、`network::event_stream_emit::tests` 6 passed、`audio::tests` 17 passed（含 `loads_default_audio_recipes` 271→273 计数 pin），均 0 failed。
- PR #1293 review 返工后重跑（2026-07-27）：`inventory::` 509 passed、`botany::` 151 passed、`network::vfx_animation_trigger::` 97 passed、`network::event_stream_emit::` 7 passed、`network::audio_trigger::` 54 passed，均 0 failed。
- `cargo fmt --check` RC=0；`cargo clippy --all-targets -- -D warnings` RC=0；全量 `cargo test`（返工后完整跑一次）0 failed。
- 手动对抗注入（本轮核验，非自动化测试的一部分）：临时给 `runtime_instance_from_template` 的 `initial_qi` 加 `stack_count > 1` 时的固定加成，`herb_bundle_decays_identically_regardless_of_stack_count` 立即撞红（`left: 0.8, right: 0.82`），验证后已还原（无残留 diff）。
- 对抗验证（validator，无上下文、read-only）：初轮 `VALIDATOR VERDICT: PASS (SHA=4515619ec5b26402a1957bf24f0573fe6c001c70)`；PR #1293 review 返工闭环后追加一轮，见下方最新 SHA 记录（`git log` 可查）。

### 跨仓库核验

- **server**：见上方落地清单。
- **client**：`client/src/main/java/com/bong/client/visual/particle/BotanyHarvestBurstPlayer.java`（PR #1293 review 新增：消费 `VfxEventPayload.SpawnParticle.direction` 字段，约束粒子出射角到 ±55° 扇形，接通"沿挥镰弧线"）；`SoundRecipePlayer.java`（通用按 recipe id 播放，无需新分支）；event_stream World channel（既有通用 HUD 消费方）。`direction` 字段本身在 `VfxEventPayload` 中早于本 plan 存在（`sword_qi_slash` 等已在用），本次是新增消费方读取。
- **agent**：无接触。

### 遗留 / 后续

- 草镰速度/品质加成（`GatheringToolKind::Sickle` 变体 + 与 `bao_chu` 重新平衡）明确不在本 plan 范围内（§8.1 决议 #3），留待后续独立 plan。
