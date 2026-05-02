# Bong · plan-forge-leftovers-v1

**收口 plan-forge-v1（已归档于 `docs/finished_plans/`）的五项自报遗留 + 一项红旗**——把炼器从"server 核心闭环 + client 占位"推到"全链路可玩"。

## 阶段总览

| Phase | 范围 | 状态 | 验收日期 |
|---|---|---|---|
| P0 | forge schema samples + Redis bridge publish + ForgeStationPlace | ⬜ | — |
| P1 | 装备 item 契约（quality/color/side_effects）+ forge → inventory 写入 + 残卷/载体 item | ⬜ | — |
| P2 | 客户端 Tempering 节奏轨道 UI | ⬜ | — |
| P3 | 客户端 Inscription 铭文槽 UI | ⬜ | — |
| P4 | 客户端 Consecration 真元注入条 UI | ⬜ | — |
| P5 | BlockEntity 持久化（依赖 `plan-persistence-v1`，本 plan 仅锚定接入位） | ⬜ | — |

> **本 plan 不重做** `plan-forge-v1` 已经 ✅ 的 server 核心（四步状态机、`resolve_*` 纯函数层、`forge_snapshot_emit`、`LearnedBlueprints`、`ForgeHistory`）。这些都已实装且有测试覆盖，本 plan 只在它们之上加桥/写入/UI。

---

## §0 与 plan-forge-v1 的关系

**已闭环（不动）**：
- `server/src/forge/` 12 模块 / 2565 行，包含 `BlueprintRegistry` (blueprint.rs) / `ForgeSession` (session.rs) / `WeaponForgeStation` (station.rs) / 七个 `resolve_*` 纯函数 (steps.rs) / `ForgeOutcomeEvent` (events.rs)
- agent `agent/packages/schema/src/forge.ts`（26 导出原子）
- client `client/src/main/java/com/bong/client/forge/{state,network/forge}/` 4 个 store + 4 个 handler + ServerDataRouter 注册

**已归档但未做**（本 plan 范围）：见上 P0-P5 + 红旗。

**红旗**（plan-forge-v1 §P0 声称"JSON Schema 生成 +24 份 forge"，实际 `agent/packages/schema/samples/` 内**零份 forge sample**）。本 plan P0 一并补齐。

---

## §1 P0 — 桥与红旗（独立 PR）

> 三件事互相独立，但同属"server 已生事件、外部接收方未接"的同类缺口，并入 P0 一次性收口。

### §1.1 Forge schema round-trip samples

**红旗**：plan-forge-v1 §P0 自报"24 份 forge JSON schema 生成"，但 `agent/packages/schema/samples/` 当前 0 份 forge sample（其他 schema 如 alchemy/inventory/cultivation 都有 .sample.json）。

**交付**：

- `agent/packages/schema/samples/server-data.forge-station.sample.json`（对齐 `WeaponForgeStationDataV1`）
- `agent/packages/schema/samples/server-data.forge-session.sample.json`（对齐 `ForgeSessionDataV1`，含三步示例）
- `agent/packages/schema/samples/server-data.forge-outcome.sample.json`（对齐 `ForgeOutcomeDataV1`，bucket=Perfect 一份 + Flawed 含 side_effects 一份）
- `agent/packages/schema/samples/server-data.forge-blueprint-book.sample.json`（对齐 `ForgeBlueprintBookDataV1`，列出三份测试图谱）
- `agent/packages/schema/samples/client-request.forge-start.sample.json`（StartForgeRequest）
- `agent/packages/schema/samples/client-request.forge-tempering-hit.sample.json`（TemperingHit）
- `agent/packages/schema/samples/client-request.forge-inscription-submit.sample.json`
- `agent/packages/schema/samples/client-request.forge-consecration-inject.sample.json`
- `agent/packages/schema/samples/client-request.forge-station-place.sample.json`（P0 新 schema，见 §1.3）

**测试**：现有 `agent/packages/schema/tests/round-trip.test.ts`（或同栈 sample 校验机制）扩进 forge——每份 sample `.parse(JSON.parse(fs.readFileSync(...)))` 不抛 + decode/encode 等价。≥9 个新 it()。

**Server 端对拍**（双端校验）：`server/src/schema/forge.rs` 的 `serde_json::from_str::<WeaponForgeStationDataV1>(sample_text)` 同样跑通——参考 alchemy 的 `tests/schema_samples.rs`（如有）或新增 `server/tests/forge_schema_samples.rs`。

### §1.2 Redis bridge：`bong:forge/start` + `bong:forge/outcome` publish

**现状**：
- `server/src/schema/channels.rs:45-46` 已定义常量 `CH_FORGE_START / CH_FORGE_OUTCOME`
- `agent/packages/schema/src/channels.ts:88-90` 同步定义
- `server/src/network/redis_bridge.rs:332` 已 publish `CH_FORGE_EVENT`（**注意：这是 cultivation 渡劫感知事件 `ForgeEventV1`，与本 plan 的"起炉/结算"不同**）
- `bong:forge/start` / `bong:forge/outcome` **零 publish 调用**

**交付**：

- `agent/packages/schema/src/forge-bridge.ts`（新文件）
  - `ForgeStartPayloadV1`：`{ v, session_id, blueprint_id, station_id, caster_id, materials: Vec<{material, count}>, ts }`
  - `ForgeOutcomePayloadV1`：`{ v, session_id, blueprint_id, bucket: "perfect"|"good"|"flawed"|"waste"|"explode", weapon_item: Option<string>, quality: number, color: Option<ColorKind>, side_effects: string[], achieved_tier: number, caster_id, ts }`
  - 新加 `REDIS_V1_CHANNELS` 列表条目（`agent/packages/schema/src/channels.ts:43`）
- `server/src/schema/forge_bridge.rs`（新文件，serde struct，与 ts 1:1 镜像）
- `server/src/network/forge_bridge.rs`（新文件，参考 `cultivation_bridge.rs`）
  - System: `publish_forge_start_on_session_create` — 监听 `EventReader<StartForgeRequest>` 起炉成功后（即 `ForgeSession::new` 调用点 mod.rs:171 之后），构造 `ForgeStartPayloadV1` → `RedisOutbound::ForgeStart(payload)`
  - System: `publish_forge_outcome` — 监听 `EventReader<ForgeOutcomeEvent>` (events.rs:57)，构造 payload → `RedisOutbound::ForgeOutcome(payload)`
- `server/src/network/redis_bridge.rs`：`RedisOutbound` enum 加两个变体 + match 分发到对应 channel
- 注册 system 到 `ForgePlugin` (server/src/forge/mod.rs)

**测试**：

- `server/src/network/redis_bridge.rs` 现有 `publishes_*_on_correct_channel` 模式扩两个 test：
  - `publishes_forge_start_on_correct_channel`
  - `publishes_forge_outcome_on_correct_channel`（含 perfect / flawed-with-side-effects 两 case）
- `server/src/network/forge_bridge.rs` system 单测：mock ForgeOutcomeEvent，断言 RedisOutbound 队列被推一条对应 payload
- agent schema 端 `ForgeStartPayloadV1` / `ForgeOutcomePayloadV1` round-trip + sample（合并到 §1.1）

**跨仓库契约 symbol**：`ForgeStartPayloadV1` / `ForgeOutcomePayloadV1`（server↔agent 双端镜像）。

### §1.3 ForgeStationPlace 方块放置

**参考**：`AlchemyFurnacePlace` 完整模板已在 `server/src/schema/client_request.rs:89-95` + `server/src/network/client_request_handler.rs:403-420`（17 行 handler）。

**交付**：

- `agent/packages/schema/src/client-request.ts`：新增 union 变体 `ForgeStationPlaceRequestV1 { v, x, y, z, item_instance_id, station_tier }`
- `server/src/schema/client_request.rs`：`ClientRequestV1::ForgeStationPlace { v, x, y, z, item_instance_id, station_tier }` + 现有 `from_json/to_json` round-trip 测试扩
- `server/src/network/client_request_handler.rs`：仿 line 403 写 `ClientRequestV1::ForgeStationPlace { ... }` 分支
  - 校验 `item_instance_id` 为合法砧类 item（消耗 1 个）
  - 构造 `PlaceForgeStationRequest { player, pos, item_instance_id, station_tier }`
  - 发到 `forge_params.place_station_tx` channel
- `server/src/forge/station.rs`：新增 system `handle_place_station_request` — spawn `WeaponForgeStation` entity at pos，tier 由 item template 决定（凡铁砧/灵铁砧/玄铁砧/道砧 = 1/2/3/4）
- `server/assets/items/forge.toml`（新文件）注册 4 种砧 item template：`fan_iron_anvil` / `ling_iron_anvil` / `xuan_iron_anvil` / `dao_anvil`，各自带 `[item.forge_station]` 子结构 `{ tier: 1..=4 }`
- `server/src/inventory/mod.rs`：扩 `ItemTemplate` 加 `forge_station_spec: Option<ForgeStationSpec>`，新增 `ForgeStationSpec { tier: u8 }` + `parse_forge_station_spec`
- 客户端：仿 alchemy 右键持砧 item → 发 `ForgeStationPlaceRequestV1`（接入到现有 inventory item 右键菜单或 use action）

**测试**：

- `server/src/network/client_request_handler.rs` 已有 round-trip 测试模式扩一个 `ForgeStationPlace` schema parse case
- `server/src/forge/station.rs` 加 3 测：
  - `place_station_consumes_item`
  - `place_station_rejects_non_anvil_item`
  - `place_station_tier_matches_item_template`
- `server/src/inventory/mod.rs::parse_forge_station_spec` 加 2 测（合法 / 缺字段）

### §1.4 P0 验收抓手

`/consume-plan` 的 subagent 应能 grep 到：

- 文件存在：`agent/packages/schema/src/forge-bridge.ts` · `server/src/schema/forge_bridge.rs` · `server/src/network/forge_bridge.rs` · `server/assets/items/forge.toml` · `agent/packages/schema/samples/server-data.forge-*.sample.json`（≥4）+ `client-request.forge-*.sample.json`（≥5）
- Symbol：`pub struct ForgeStartPayloadV1` · `pub struct ForgeOutcomePayloadV1` · `RedisOutbound::ForgeStart` · `RedisOutbound::ForgeOutcome` · `ClientRequestV1::ForgeStationPlace` · `pub struct ForgeStationSpec` · `fn handle_place_station_request`
- 测试：`publishes_forge_start_on_correct_channel` · `publishes_forge_outcome_on_correct_channel` · `place_station_consumes_item`
- Redis key：grep `bong:forge/start` 在 server publish 调用点命中 + agent `REDIS_V1_CHANNELS` 列表命中
- `agent/packages/schema/samples/` 内 `ls | grep forge | wc -l` ≥ 9

---

## §2 P1 — 装备 item 契约 + forge → inventory 写入

> 这是本 plan 最重的一阶段——把 forge 现在"算出 quality/color/side_effects 但没人接收"的状态收口。

### §2.1 现状缺口

- `server/src/forge/events.rs:57` 的 `ForgeOutcomeEvent` 已含字段 `weapon_item: Option<String>` / `quality: f32` / `color: Option<ColorKind>` / `side_effects: Vec<String>` / `achieved_tier: u8`
- `server/src/inventory/mod.rs:182-208` 的 `ItemInstance` **未含**这三/四个字段
- `server/src/inventory/mod.rs:109-117` 的 `WeaponSpec` 只有 `weapon_kind/base_attack/quality_tier/durability_max/qi_cost_mul`——是 template 静态字段，不含运行时品质
- forge resolve outcome 后没人把武器实例写进玩家背包（grep `forge` ↔ `give_item` / `spawn_inventory_item` 无命中）

### §2.2 ItemInstance 字段扩展

**交付**：

- `server/src/inventory/mod.rs::ItemInstance` 加：
  - `pub forge_quality: Option<f32>` (0.0..=1.0，None = 非 forge 产物的旧物)
  - `pub forge_color: Option<ColorKind>`
  - `pub forge_side_effects: Vec<String>`（空 vec = 无附加）
  - `pub forge_achieved_tier: Option<u8>` (1..=4，None = 非武器)
- `server/src/inventory/mod.rs::ItemInstance::new` 扩参——保持向后兼容：旧调用走默认空值
- `agent/packages/schema/src/inventory.ts::ItemInstanceV1` 加四个 optional 字段
- `client/src/main/java/com/bong/client/inventory/InventoryItem.java` 加四个字段（`forgeQuality: Double`, `forgeColor: ColorKind?`, `forgeSideEffects: List<String>`, `forgeAchievedTier: Integer?`）+ factory + tooltip 展示

**测试**：

- `server/src/inventory/mod.rs` round-trip：含 forge 字段的 ItemInstance ↔ JSON
- `agent/packages/schema/tests/inventory.test.ts` round-trip 扩
- `client/src/test/.../InventoryItemTest.java` 加字段 default + clamp 测试（plan-inventory-v1 §7.4 该测试本就 ⬜，本阶段顺手补上）

### §2.3 forge → inventory bridge

**交付**：

- `server/src/forge/inventory_bridge.rs`（新文件）
  - System: `forge_outcome_to_inventory` — 监听 `EventReader<ForgeOutcomeEvent>`，按 bucket 决定：
    - `Perfect`/`Good` → 调 `inventory.give_item(caster, weapon_item, ItemInstance { forge_quality, forge_color, forge_side_effects, forge_achieved_tier, ..default })`
    - `Flawed` → 同上但 weapon_item 来自 `flawed_fallback.weapon` + side_effects 已含 fallback 池抽样
    - `Waste` → 不发 item（材料已扣）
    - `Explode` → 不发 item + 已通过 `WeaponForgeStation::integrity` 扣分（已实装，不动）
- 注册到 `ForgePlugin`
- 兼容 LearnedBlueprints：图谱残卷 item 见 §2.4

**测试**：

- `server/src/forge/inventory_bridge.rs`：5 测覆盖五桶
  - `outcome_perfect_gives_weapon_with_quality`
  - `outcome_flawed_includes_side_effects`
  - `outcome_waste_gives_nothing`
  - `outcome_explode_only_wears_station`
  - `outcome_consecration_writes_color`

### §2.4 残卷 / 载体材料 item

**交付**：

- `server/assets/items/forge.toml`（§1.3 同文件）扩：
  - 图谱残卷：`blueprint_scroll_iron_sword` / `blueprint_scroll_qing_feng` / `blueprint_scroll_ling_feng`（与三份测试 blueprint 对应），`category = "misc"` + `[item.blueprint_scroll] blueprint_id = "..."`
  - 铭文残卷：`inscription_scroll_sharp_v0` / `inscription_scroll_qi_amplify_v0`，`[item.inscription_scroll] inscription_id = "..."`
  - 载体材料：`ling_wood` / `yi_beast_bone` / `xuan_iron` / `qing_steel`，category=misc，无子结构
- `server/src/inventory/mod.rs`：`ItemTemplate` 加 `blueprint_scroll_spec: Option<BlueprintScrollSpec { blueprint_id }>` + `inscription_scroll_spec: Option<InscriptionScrollSpec { inscription_id }>` + 解析函数
- 客户端 `BlueprintScrollStore`（已存在）连到真实学习流：拖图谱残卷 item 到 forge UI 的图谱卷轴区 → 发 `ForgeLearnBlueprintRequestV1`（已在 forge.ts schema 内）→ server 消耗 item + 加入 `LearnedBlueprints`
- 同理铭文残卷拖到铭文槽 → 发 `InscriptionScrollSubmitRequestV1`（forge.ts 已有）→ 已实装的 `apply_scroll` (steps.rs:172) 接收

**测试**：

- `server/src/inventory/mod.rs::parse_blueprint_scroll_spec` / `parse_inscription_scroll_spec` 各 2 测
- `server/src/forge/learned.rs`：扩 1 测 `learn_blueprint_consumes_scroll_item`
- `server/src/forge/steps.rs::apply_scroll`：扩 1 测 `apply_scroll_validates_inscription_item`

### §2.5 P1 验收抓手

- 文件：`server/src/forge/inventory_bridge.rs` · `server/assets/items/forge.toml`（含 ≥9 个 item template）
- Symbol：`ItemInstance.forge_quality` · `ItemInstance.forge_color` · `BlueprintScrollSpec` · `InscriptionScrollSpec` · `fn forge_outcome_to_inventory` · `pub fn parse_blueprint_scroll_spec`
- 测试：`outcome_perfect_gives_weapon_with_quality` 等 5 测 · 解析测试 · `learn_blueprint_consumes_scroll_item`
- 端到端：跑通"灵锋（ling_feng_v0）" — `StartForgeRequest` → 4 步 → `ForgeOutcomeEvent` → 玩家背包出现 `ling_feng_sword` instance with `forge_quality > 0.9`

---

## §3 P2 — Tempering 节奏轨道 UI

> 当前 `ForgeScreen.java` 97 行只有文本占位；alchemy 同位主屏 `AlchemyScreen.java` 814 行可作体量参考。本 plan 三块 UI 不做"大重构 ForgeScreen"，而是把 `ForgeScreen` 改造成 step view 切换器（按 `ForgeSessionStore.current_step` 渲染 Billet/Tempering/Inscription/Consecration 四视图之一）。

### §3.1 Tempering 视图

**位置**：`client/src/main/java/com/bong/client/forge/screen/TemperingTrackComponent.java`（新文件）

**owo-lib 组件**：基于 `BaseOwoScreen<FlowLayout>` 同栈，自定义 `Component` 子类（参考 alchemy 内 `BackpackGridPanel` 模式）。

**渲染**：

- 节奏轨道（横向滚动条）：从右往左滚的"音符"——L=蓝/H=红/F=黄三色 marker，命中线在屏幕中央
- 节奏 pattern 来源：`ForgeSessionStore.tempering_state.pattern_remaining`（schema 已有）
- 击键反馈：J/K/L 按下时弹 toast（"Light hit / Heavy hit / Fold hit / Miss"）
- 偏差条：屏幕底部，从 `ForgeSessionStore.tempering_state.deviation` 取
- combo 计数：`ForgeSessionStore.tempering_state.combo`

### §3.2 输入流

**位置**：`client/src/main/java/com/bong/client/forge/input/TemperingInputHandler.java`（新文件）

- 监听 J(GLFW=74) / K(GLFW=75) / L(GLFW=76)，仅在 ForgeScreen 打开 + current_step == Tempering 时触发
- 击键 → 发 `ClientRequestV1::TemperingHit { v, session_id, beat: "L"|"H"|"F" }`（schema 已有 `forge.ts::TemperBeat`）

### §3.3 服务端反馈接收

`ForgeSessionDataV1` 已在 server-data 推送中含 tempering_state 字段（plan-forge-v1 P0 已实装），客户端 `ForgeSessionStore` 收到 `forge_session` 类型的 server-data → 触发 store listener → TemperingTrackComponent 重渲染。**本阶段无需改 server**——只在 client 消费已有数据。

### §3.4 测试

- `client/src/test/.../TemperingInputHandlerTest.java` — 5 测：
  - `j_key_emits_light_hit_when_screen_open_and_step_tempering`
  - `j_key_ignored_when_screen_closed`
  - `j_key_ignored_when_step_billet`
  - `k_key_emits_heavy_hit`
  - `l_key_emits_fold_hit`
- `client/src/test/.../TemperingTrackComponentTest.java` — 4 测：
  - `renders_pattern_from_store`
  - `combo_displays_correctly`
  - `deviation_bar_at_max_shows_red`
  - `empty_pattern_shows_done_state`

### §3.5 P2 验收抓手

- 文件：`TemperingTrackComponent.java` · `TemperingInputHandler.java`
- Symbol：`class TemperingTrackComponent extends Component` · `private void onTemperingKey(int keyCode)` · `TemperBeat.L / H / F` 三 enum case 命中
- 测试：上述 9 测全过
- 端到端：手动跑 qing_feng_v0 图谱，节奏轨道渲染 + J/K/L 命中走通到 server outcome

---

## §4 P3 — Inscription 铭文槽 UI

### §4.1 视图

**位置**：`client/src/main/java/com/bong/client/forge/screen/InscriptionPanelComponent.java`（新文件）

- 铭文槽位：根据 `ForgeSessionStore.inscription_state.slots`（schema 已有，1-3 槽）渲染对应数量的空位
- 槽位接受 drag-drop：从右侧背包 grid（复用 `BackpackGridPanel`）拖 `InscriptionScroll` 类 item 到槽 → 发 `ClientRequestV1::InscriptionScrollSubmit`（schema 已有）
- 已填槽展示：铭文 id + tooltip（铭文效果描述，从 item template 拉）
- 失败概率指示：底栏显示 `inscription_state.fail_chance_remaining`

### §4.2 拖放接入

复用现有 inventory 拖放：`client/src/main/java/com/bong/client/inventory/DragState.java`

- 注册 InscriptionPanelComponent 为 drop target（参考 alchemy 炉槽接入模式）
- `onItemDrop(ItemInstance instance)` → 校验 template_id 是否含 `inscription_scroll_spec`（client 侧软校验，server 权威）→ 发 schema request

### §4.3 测试

- `InscriptionPanelComponentTest.java` — 6 测：
  - `renders_three_slots_for_double_handed_weapon`
  - `renders_one_slot_for_sword`
  - `accepts_inscription_scroll_drop`
  - `rejects_non_scroll_item_drop`
  - `slot_filled_state_shows_inscription_id`
  - `fail_chance_displayed_correctly`

### §4.4 P3 验收抓手

- 文件：`InscriptionPanelComponent.java`
- Symbol：`class InscriptionPanelComponent extends Component` · `private void onScrollDropped(ItemInstance scroll)` · 引用 `BlueprintScrollSpec` / `InscriptionScrollSpec`（来自 P1）
- 测试：6 测
- 端到端：在 Tempering 完成后切到 Inscription，拖铭文残卷成功填槽，server 收到 InscriptionScrollSubmit + apply_scroll

---

## §5 P4 — Consecration 真元注入条 UI

### §5.1 视图

**位置**：`client/src/main/java/com/bong/client/forge/screen/ConsecrationPanelComponent.java`（新文件）

- 真元进度条：从 `ForgeSessionStore.consecration_state.qi_injected` / `qi_required` 计算填充率
- 当前真元色预览：圆形 swatch，颜色来自 `ColorKind`（须有 `client/.../ColorKind.java` 类——若不存在，本阶段补；已知 cultivation client 端可能已有）
- 注入按钮：长按持续注入，发 `ClientRequestV1::ConsecrationInject { v, session_id, qi_amount }`（schema 已有），qi_amount 按 tick 计算（如 50/sec）
- 境界门槛提示：`consecration_profile.min_realm` < 当前境界 → 红字提示 + 按钮禁用

### §5.2 测试

- `ConsecrationPanelComponentTest.java` — 5 测：
  - `progress_bar_reflects_qi_ratio`
  - `color_swatch_matches_caster_color`
  - `inject_button_disabled_when_realm_insufficient`
  - `inject_button_held_emits_periodic_request`
  - `progress_full_emits_no_more_inject`

### §5.3 P4 验收抓手

- 文件：`ConsecrationPanelComponent.java` · `client/src/main/java/com/bong/client/cultivation/ColorKind.java`（若新建）
- Symbol：`class ConsecrationPanelComponent extends Component` · `enum ColorKind` 在 client 端可见
- 测试：5 测
- 端到端：跑灵锋全流程，Consecration 阶段进度条逐渐填满，最终 forge_outcome 带 color 字段写入 ItemInstance（联动 P1）

---

## §6 P5 — BlockEntity 持久化（依赖 plan-persistence-v1）

### §6.1 现状分析

`grep -rn 'BlockEntity 持久化' server/src/` 命中：
- `server/src/alchemy/mod.rs:21` — alchemy furnace 也未做
- `server/src/lingtian/mod.rs:20` — 灵田同
- `server/src/lingtian/systems.rs:13` — 注释 "依 plan-persistence-v1"

**结论**：BlockEntity 持久化是**项目级缺失**，由未立项的 `plan-persistence-v1` 负责。在 forge 单做会重复 alchemy/lingtian 该做的工作，且不闭环（重启仍丢失）。

### §6.2 本 plan 的责任

**只锚定接入位**，不实现持久化层：

- `server/src/forge/station.rs::WeaponForgeStation` 加注释 `// TODO(plan-persistence-v1): block_entity: Option<BlockEntityRef>`（参考 alchemy/mod.rs:261 注释模式）
- `server/src/forge/session.rs::ForgeSession` 加 `#[derive(Serialize, Deserialize)]`（如未加），保证 plan-persistence-v1 落地后能直接序列化
- `server/src/forge/mod.rs` 加文档注释列出"持久化需保存的 Resource：`ForgeSessions / BlueprintRegistry / LearnedBlueprints`"
- 在本 plan 的 Finish Evidence 段明确"P5 转交 plan-persistence-v1"

### §6.3 P5 验收抓手

- forge 模块所有需持久化类型 `#[derive(Serialize, Deserialize)]` ✓
- `grep 'plan-persistence-v1' server/src/forge/` 命中至少 3 处指向锚点
- 不需要新测试

### §6.4 后续

`plan-persistence-v1` 立项后，回头补 `forge` 的实际持久化接入，作为该 plan 的一个 P。**本 plan 无需等待 P5 完成即可归档**——只要 P0-P4 ✅ + P5 锚点注释到位即可迁入 finished_plans/。

---

## §7 测试与饱和化要求

CLAUDE.md 强约束"饱和测试"：

- 所有新 schema variant 双端 round-trip + 至少一份 invalid sample（如 `client-request.forge-station-place.invalid-missing-tier.sample.json`）
- 所有 enum/state-machine 变体每个一条专属 case（ForgeStep 五值 / bucket 五值 / TemperBeat 三值 / ColorKind 十值 / `WeaponSpec.weapon_kind` 全值）
- 所有 system 至少 3 测：happy / 边界 / 错误
- forge 端到端 e2e：`server/tests/forge_e2e.rs`（新文件） — 完整跑 ling_feng_v0 四步 → 验证最终背包出武器 + 字段对齐

---

## §8 跨 plan 钩子

| 来源 plan | 钩子点 | 触发阶段 |
|---|---|---|
| `plan-inventory-v1`（已 ✅） | ItemInstance 字段扩展 / 新 item template / 拖放 | P1 |
| `plan-cultivation-v1`（已 ✅） | ColorKind 来源 / min_realm 校验 | P1 / P4 |
| `plan-combat-v1` | 武器 quality/color/side_effects 影响伤害判定（**不在本 plan 范围**，由 combat plan 接入） | — |
| `plan-persistence-v1`（未立） | BlockEntity 持久化 | P5（转交） |
| `plan-botany-v1`（未立） | 灵木 / 异兽骨载体材料的真实采集（本 plan P1 仅注册 placeholder template） | — |

---

## §9 风险

| 风险 | 对策 |
|---|---|
| UI 三块（P2-P4）总量大（估 2000+ 行 Java），单 PR 困难 | 三阶段独立 PR，每个 PR ≤ 800 行 + 测试 |
| `ForgeScreen` 改造为 step view 切换器破坏现有占位行为 | 先在 P2 做切换器骨架（保留 Billet/Tempering 两 view），P3/P4 各自只加自己的 view |
| forge → inventory bridge 触发时机错（结算前没存够 caster 背包格） | `forge_outcome_to_inventory` system 必须在 `ForgeOutcomeEvent` 发出后同 frame 处理，且失败（背包满）时 fallback 到地面掉落（复用 inventory dropped_loot 机制） |
| plan-persistence-v1 久不立 | P5 仅锚点不阻塞归档，本 plan 可独立 ✅ |
| forge schema sample 与 server-data 现有 emit 行为对不上 | P0 sample 校验时 server 端 from_str round-trip 必须跑通真实 emit 输出（用现 emit 函数的 fixture 做 sample） |

---

## §10 阶段独立性

| Phase | 依赖 | 阻塞下游 |
|---|---|---|
| P0 | 无 | 无（P1+ 不依赖 P0） |
| P1 | 无（独立扩 ItemInstance） | P3（铭文槽要 InscriptionScrollSpec） · P5（无） |
| P2 | 无（消费已有 ForgeSessionStore） | 无 |
| P3 | P1 完成（要 InscriptionScrollSpec 类） | 无 |
| P4 | P1 完成（要 ColorKind 已在 ItemInstance） | 无 |
| P5 | 无 | 无（仅注释） |

**`/consume-plan` 推荐顺序**：P0 → P1 → P2 → P3 → P4 → P5。但 P0 / P2 可并行（不同子系统）。

---

## §11 进度日志

- 2026-04-28：plan 立项。基于 `plan-forge-v1` 已归档 Finish Evidence + 红旗（forge sample 缺失）+ 五项自报遗留拆出本 plan。

## Finish Evidence

### 落地清单

- P0：补齐 forge schema / sample / bridge 契约：`agent/packages/schema/src/forge-bridge.ts`、`agent/packages/schema/src/client-request.ts`、`agent/packages/schema/src/channels.ts`、`agent/packages/schema/samples/*forge*.sample.json`、`server/src/schema/forge_bridge.rs`、`server/src/network/forge_bridge.rs`、`server/src/network/redis_bridge.rs`。
- P0：接通 `ForgeStationPlace` 放砧链路：`server/src/schema/client_request.rs`、`server/src/network/client_request_handler.rs`、`server/src/forge/station.rs`、`client/src/main/java/com/bong/client/network/ClientRequestProtocol.java`、`client/src/main/java/com/bong/client/network/ClientRequestSender.java`、`client/src/main/java/com/bong/client/inventory/InspectScreen.java`。
- P1：扩展装备 item forge 运行时字段与 forge -> inventory 写入：`server/src/inventory/mod.rs`、`server/src/schema/inventory.rs`、`server/src/network/inventory_snapshot_emit.rs`、`server/src/forge/inventory_bridge.rs`、`agent/packages/schema/src/inventory.ts`、`client/src/main/java/com/bong/client/inventory/model/InventoryItem.java`、`client/src/main/java/com/bong/client/network/InventorySnapshotHandler.java`。
- P1：注册砧、图谱残卷、铭文残卷、载体材料：`server/assets/items/forge.toml`，并接入 `BlueprintScrollSpec` / `InscriptionScrollSpec` 解析与学习 / 投放校验。
- P2：实现 Tempering 节奏轨道与 J/K/L 输入：`client/src/main/java/com/bong/client/forge/ForgeScreen.java`、`client/src/main/java/com/bong/client/forge/screen/TemperingTrackComponent.java`、`client/src/main/java/com/bong/client/forge/input/TemperingInputHandler.java`。
- P3：实现 Inscription 铭文槽与 scroll drop：`client/src/main/java/com/bong/client/forge/screen/InscriptionPanelComponent.java`。
- P4：实现 Consecration 真元注入 UI 与颜色预览：`client/src/main/java/com/bong/client/forge/screen/ConsecrationPanelComponent.java`、`client/src/main/java/com/bong/client/cultivation/ColorKind.java`。
- P5：仅锚定持久化接入位，不在本 plan 扩面实现 BlockEntity 持久化：`server/src/forge/station.rs`、`server/src/forge/session.rs`、`server/src/forge/mod.rs` 已含 `plan-persistence-v1` 注释与 `Serialize, Deserialize` 派生。

### 关键 commit

- `4ba8f081` · 2026-04-28 · `plan-forge-leftovers-v1: 补齐炼器 schema 契约样本`
- `cef1900f` · 2026-04-28 · `plan-forge-leftovers-v1: 接通服务端炼器桥与放砧`
- `df758e17` · 2026-04-28 · `plan-forge-leftovers-v1: 写回炼器结算产物`
- `46fe1b14` · 2026-04-28 · `plan-forge-leftovers-v1: 补齐炼器残卷与载体契约`
- `85cf38a0` · 2026-04-28 · `plan-forge-leftovers-v1: 接入淬炼节奏轨道`
- `1b92a2c2` · 2026-04-28 · `plan-forge-leftovers-v1: 接入铭文槽投放`
- `6c2542a7` · 2026-04-28 · `plan-forge-leftovers-v1: 接入开光真元注入`
- `77acd3b9` · 2026-04-28 · `fix(forge): 收紧铭文投放校验`
- `5e272d3d` · 2026-04-30 · `feat(forge): 增加凡器工具图谱`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`：通过；`cargo test` 2063 passed。
- `cd agent && npm run build && cd packages/tiandao && npm test && cd ../schema && npm test`：通过；tiandao 26 files / 205 tests passed，schema 7 files / 246 tests passed。
- `cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build`：通过；Gradle test XML 汇总 787 tests / 0 failures / 0 errors / 0 skipped。

### 跨仓库核验

- Server：`ForgeStartPayloadV1` / `ForgeOutcomePayloadV1`、`RedisOutbound::ForgeStart` / `RedisOutbound::ForgeOutcome`、`ClientRequestV1::ForgeStationPlace`、`ForgeStationSpec`、`handle_place_station_request`、`forge_outcome_to_inventory`。
- Agent schema：`ForgeStartPayloadV1` / `ForgeOutcomePayloadV1`、`ForgeStationPlaceRequestV1`、`REDIS_V1_CHANNELS` 中的 `bong:forge/start` 与 `bong:forge/outcome`、14 份 forge sample。
- Client：`TemperingTrackComponent`、`TemperingInputHandler`、`InscriptionPanelComponent`、`ConsecrationPanelComponent`、`ColorKind`、`InventoryItem.forgeQuality/forgeColor/forgeSideEffects/forgeAchievedTier`。

### 遗留 / 后续

- `plan-combat-v1` 仍负责把 weapon `quality/color/side_effects` 纳入伤害判定；本 plan 只保证 forge 产物写入 inventory 并保留字段。
- BlockEntity 实际持久化仍不在本 plan 范围；P5 只保留 forge 模块序列化与 `plan-persistence-v1` 接入锚点，避免重复扩面。
