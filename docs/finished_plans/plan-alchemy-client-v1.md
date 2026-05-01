# Bong · plan-alchemy-client-v1

**炼丹系统 Fabric 客户端全接入**。plan-alchemy-v1 server 侧（P0–P5 + §1.2 放置炉）已闭环；本 plan 补全客户端拦截放置、多炉 session 路由、owo-lib 炉 UI 屏幕、agent schema 对齐、配方材料名正典化、炸炉结算 + skill XP 钩子，形成完整玩家端炼丹链路。

> **2026-05-01 决策**：确定 furnace_id（String，旧路由：每玩家一炉，仅作日志）→ furnace_pos（`(i32,i32,i32)`，新路由：世界坐标多炉并行），P1 进入执行状态。详见 §3 patch plan。

**世界观锚点**：`worldview.md §六 个性与差异化 / 真元染色 / 染色谱`（炼丹师 → 温润色；丹毒色对应代码 `ColorKind::Mellow / Sharp / Violent / Turbid` 与 worldview "温润色 / 锋锐色 / 暴烈色 / 浊乱色"对位）· `worldview.md §八 天道行为准则 / 灵物密度阈值`（高级炼丹炉触发天道注视：line ~604）· `worldview.md §九 经济与交易 / 顶级资产`（丹方残卷是顶级情报资产，"情报换命"）

> **注**：worldview 无独立"丹道/炼丹"章节——炼丹相关内容散落 §六 染色谱（炼丹师真元温润色）+ §八 天道运维（炼丹炉聚灵触发天道）+ §九 顶级资产（丹方残卷情报）。本 plan 整合三处线索，不是单一锚点。

**library 锚点**：`docs/library/ecology/末法药材十七种.json`（正典药材名 ci_she_hao / ning_mai_cao / chi_sui_cao / hui_yuan_zhi 等）· `docs/library/ecology/辛草试毒录.json`（辛度 → 丹毒色对照）· 待写 `crafting-XXXX 炉火诀`（炼丹炉操作规程 / 炸炉反噬逻辑）

**交叉引用**：
- `plan-alchemy-v1`（前置；server P0–P5 + §1.2 放置炉 ✅，本 plan 不重造 server 侧）
- `plan-botany-v1`（✅ 已完成，正典药材名来源）
- `plan-persistence-v1`（BlockEntity 持久化，炉方块重启后不丢失）
- `plan-inventory-v1`（丹方残卷 item 定义 + DragState 扩展 FURNACE_SLOT）
- `plan-skill-v1`（§7 skill XP 炼丹路径）
- `plan-combat-no_ui`（Explode 结算 → damage + meridian_crack）
- `plan-forge-leftovers-v1`（炉体是否与锻造共用的后续判断）

**阶段总览**：
- P0 ⬜ Fabric 客户端拦截放置（UseItemOnC2s → AlchemyFurnacePlace）
- P1 ⏳ 多炉 session 路由（furnace_id→furnace_pos 全链路迁移，详见 §3 patch plan）
- P2 ⬜ 炉方块右键 → 开炉 Screen（StartAlchemyRequest + 服务端响应）
- P3 ⬜ owo-lib 三列炉 UI Screen（DragState 扩展 FurnaceSlot + 投料 / 催火 / 干预三列）
- P4 ⬜ Redis channel `bong:alchemy/*` + agent TypeBox 对齐
- P5 ⬜ alchemy 配方材料名正典化（替换 3 份测试配方 + 正式配方占位）
- P6 ⬜ 炸炉结算（ResolvedOutcome::Explode → damage + meridian_crack 应用到施法者）
- P7 ⬜ furnace_fantie item 图标（128×128 PNG，路径 `client/src/main/resources/assets/bong-client/textures/gui/items/furnace_fantie.png`）

---

## §0 设计轴心

- [ ] **客户端不存丹法逻辑**：仅拦截 MC 原版 UseItemOnC2s，转为 Bong CustomPayload 发往 server；server 是唯一状态权威
- [ ] **多炉并行，按 BlockPos 路由**：废除"每玩家一炉"假设；payload 加 `furnace_pos: (i32, i32, i32)` 字段使多炉并行合法
- [ ] **炸炉反噬是高风险体验**：Explode 时不仅丢材料，还给施法者 damage + MICRO_TEAR——是玩家技术/耐心的扣分项，不是意外事故
- [ ] **正典药材名是信息资产**：配方 JSON 材料 ID 必须与 library 对齐；改错名 = 改情报

---

## §1 第一性原理（烬灰子四论挂点）

- **缚论·炉印封缚**：玩家手持炉物品右键地面，是在"封缚"真元镜印至方块；客户端识别这一动作本质是拦截"封缚意图"
- **音论·炉响**：炸炉是炉内多色"音"相撞最激烈的时刻——施法者受到的反噬是那股冲击波的物理传递（meridian_crack）
- **噬论·废丹速朽**：炸炉后剩余材料迅速噬散（由 plan-alchemy-recycle 处理废料路径），不是无损流失
- **影论·丹方残卷**：配方信息是比丹药本身更稳定的"次级投影"——知道怎么炼比有没有药材更值钱

---

## §2 P0 — Fabric 客户端拦截放置

> server 侧 `ClientRequestV1::AlchemyFurnacePlace` + `handle_alchemy_furnace_place` 已就绪；客户端仅需发正确 CustomPayload。

- [ ] **拦截点**：`BongClientMixin` 或 `UseItemOnBlockC2SPacket` mixin → 检查玩家手持物品的 `item_id` NBT 是否在炉物品白名单（`furnace_fantie` / `furnace_lingtie` 等）
- [ ] **白名单来源**：`BongItemRegistry.FURNACE_ITEMS`（客户端静态列表，与 `server/assets/items/core.toml` 中 `category = "furnace"` 同步）
- [ ] **payload 构造**：命中后取目标 BlockPos → 序列化 `AlchemyFurnacePlace { v:1, item_id, block_pos }` → 发 CustomPayload `bong:client_request`
- [ ] **原版事件取消**：命中后 cancel vanilla UseItemOnBlock（防止同时放下普通方块）
- [ ] **测试**：mock 手持 `furnace_fantie` 右键地面 → client 发 AlchemyFurnacePlace payload + vanilla 事件被取消；非炉物品右键 → vanilla 正常执行

---

## §3 P1 — furnace_id → furnace_pos 全链路迁移 (patch plan)

> **现状诊断（2026-05-01 实地核验）**：
> - `AlchemyFurnace` 已是独立 ECS entity（`furnace.rs:19`），含 `pos: Option<(i32,i32,i32)>` + `owner: Option<String>`。
> - `handle_alchemy_furnace_place`（`mod.rs:180`）通过 `commands.spawn(furnace)` 生成独立炉 entity，数据模型已支持多炉。
> - **但所有 handler 仍走旧路由**：`furnaces.get_mut(entity)` 其中 `entity=ev.client`（玩家 ECS entity），而非按坐标查独立炉 entity。
> - `AlchemyOpenFurnace.furnace_id: String` 仅写入 `tracing::info!` 日志（`client_request_handler.rs:427`），不参与路由。
> - `AlchemyIgnite` / `AlchemyIntervention` 根本没有 `furnace_id` 字段，路由全靠玩家 entity。
> - `AlchemyFeedSlot` / `AlchemyTakeBack` 只有一个空 catch-all（`client_request_handler.rs:609-611`），完全未接线。
>
> **结论**：`AlchemyFurnace` 组件上的 `pos` 字段是 dead code——已有坐标数据但 handler 不用它路由。迁移核心是把 handler 的 furnace lookup 从 `player entity → component` 改为 `world pos → entity`。

### 3.1 Rust Server Schema (`server/src/schema/client_request.rs`)

> 共 5 个 payload 变体需要加 `furnace_pos`；`AlchemyFurnacePlace` 已有 `x/y/z` 无需动；`AlchemyTurnPage` / `AlchemyLearnRecipe` / `AlchemyTakePill` 是配方书/服药操作非炉操作，不加。

- [ ] **`AlchemyOpenFurnace`**：`furnace_id: String` → 替换为 `furnace_pos: (i32, i32, i32)`
- [ ] **`AlchemyIgnite`**（当前仅有 `recipe_id: String`）：新增 `furnace_pos: (i32, i32, i32)`
- [ ] **`AlchemyIntervention`**（当前仅有 `intervention: AlchemyInterventionV1`）：新增 `furnace_pos: (i32, i32, i32)`
- [ ] **`AlchemyFeedSlot`**（当前有 `slot_idx, material, count`）：新增 `furnace_pos: (i32, i32, i32)`
- [ ] **`AlchemyTakeBack`**（当前有 `slot_idx`）：新增 `furnace_pos: (i32, i32, i32)`
- [ ] **serde 注解**：5 个变体均加 `#[serde(deny_unknown_fields)]`，确保 client/agent 字段名拼写错误被拒绝而非静默丢弃
- [ ] **单元测试更新**：`client_request.rs:787` 处 `AlchemyOpenFurnace` 的 deser 测试从 `furnace_id` 改为 `furnace_pos` 元组

### 3.2 Server Schema — 推送 payload (`server/src/schema/alchemy.rs`)

- [ ] **`AlchemyFurnaceDataV1`**（`alchemy.rs:103`）：`furnace_id: String` → `pos: Option<(i32, i32, i32)>`（对齐 ECS 组件 `AlchemyFurnace.pos`）
- [ ] **`alchemy_snapshot_emit.rs:76`**：测试 fixture 从 `furnace_id: "block_-12_64_38".into()` 改为 `pos: Some((-12, 64, 38))`

### 3.3 Server Handler 路由改造 (`server/src/network/client_request_handler.rs`)

> 核心变更：所有 furnace 查询从「查 player entity 上的 AlchemyFurnace 组件」→「遍历所有 AlchemyFurnace entity，按 `pos` 匹配 + `owner` 权限校验」。

- [ ] **`AlchemyOpenFurnace` handler**（line 415-428）：
  - 删除 `furnace_id` 日志行
  - 改为：遍历 `furnaces` query，匹配 `f.pos == Some((x,y,z))` → 校验 `f.owner == Some(player_name)` → emit snapshot
  - 炉不存在 → 发 `bong:system/error`（不 crash）
  - 权限不匹配 → 发 `bong:system/error` + warn 日志
- [ ] **`AlchemyIntervention` handler**（line 4318 `handle_alchemy_intervention`）：
  - 函数签名从 `entity: Entity` → `furnace_pos: (i32, i32, i32)`
  - 内部 from `furnaces.get_mut(entity)` → 遍历匹配 pos + owner
- [ ] **`AlchemyIgnite` handler**（当前在 catch-all line 609-611，无实际逻辑）：
  - 新增 `handle_alchemy_ignite(entity, furnace_pos, recipe_id, ...)`
  - 路由：匹配 pos → 校验 owner → 调用 furnace 的 ignite 方法
- [ ] **`AlchemyFeedSlot` handler**（当前空 catch-all）：
  - 新增 `handle_alchemy_feed_slot(entity, furnace_pos, slot_idx, material, count, ...)`
  - 路由：匹配 pos → 校验 owner → 投料
- [ ] **`AlchemyTakeBack` handler**（当前空 catch-all）：
  - 新增 `handle_alchemy_take_back(entity, furnace_pos, slot_idx, ...)`
  - 路由：匹配 pos → 校验 owner → 退料
- [ ] **`handle_alchemy_take_pill`**（line 4352）：**不加 furnace_pos**——服药是背包操作，不依赖特定炉。服用时 player entity 即可定位。

### 3.4 Agent TypeBox Schema (`agent/packages/schema/`)

- [ ] **`agent/packages/schema/src/client-request.ts`**：
  - `AlchemyOpenFurnaceRequestV1`（line 313）：`furnace_id: Type.String()` → `furnace_pos: Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()])`
  - `AlchemyIgniteRequestV1`（line 342）：新增 `furnace_pos: Type.Tuple([...])`
  - `AlchemyInterventionRequestV1`（line 353）：新增 `furnace_pos: Type.Tuple([...])`
  - `AlchemyFeedSlotRequestV1`（line 319）：新增 `furnace_pos: Type.Tuple([...])`
  - `AlchemyTakeBackRequestV1`（line 332）：新增 `furnace_pos: Type.Tuple([...])`
  - 建议：extract `BlockPosV1 = Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()])` 复用
- [ ] **`agent/packages/schema/src/server-data.ts`**：
  - `ServerDataAlchemyFurnaceV1`（line 444）：`furnace_id: Type.String()` → `pos: Type.Optional(Type.Tuple([Type.Integer(), Type.Integer(), Type.Integer()]))`
- [ ] **`agent/packages/schema/dist/`**：`npm run build` 自动重新生成 `.d.ts`

### 3.5 Java Client Protocol (`client/src/main/java/...`)

- [ ] **`ClientRequestProtocol.java`**（line 142）：
  - `encodeAlchemyOpenFurnace(String furnaceId)` → `encodeAlchemyOpenFurnace(BlockPos pos)`
  - JSON 字段 `furnace_id` → `furnace_pos: [x, y, z]`
  - 新增 `encodeAlchemyIgnite(BlockPos pos, String recipeId)`
  - 新增 `encodeAlchemyFeedSlot(BlockPos pos, int slotIdx, String material, int count)`
  - 新增 `encodeAlchemyTakeBack(BlockPos pos, int slotIdx)`
  - 更新 `encodeAlchemyIntervention(BlockPos pos, AlchemyInterventionV1 intervention)`
- [ ] **`ClientRequestSender.java`**（line 201）：
  - `sendAlchemyOpenFurnace(String)` → `sendAlchemyOpenFurnace(BlockPos)`
  - 对应新增/更新其他 sender 方法签名

### 3.6 权限模型决策

> plan §11 开放问题第一条「多炉权限」——迁移实施前必须定。

- [ ] **决策：放置者独享**。`AlchemyFurnace.owner` 字段已存在，handler 校验 `f.owner == Some(player_name)`。组队共用等 plan-social 立项后通过 `co_owners: Vec<String>` 扩展，本次不动。

### 3.7 测试

- [ ] `server/src/schema/client_request.rs`：5 个 payload 的 serde roundtrip + deny_unknown_fields
- [ ] `server/src/alchemy/mod.rs` 集成测试：两个玩家各放一炉 → 各自 `AlchemyIgnite` 不同 pos → 互不干扰
- [ ] `server/src/alchemy/mod.rs` 集成测试：玩家 A 试图 ignite 玩家 B 的炉 → 权限拒绝 + error channel
- [ ] `agent/packages/schema/`：npm test — TypeBox compile check + schema sample 对拍 5 个 payload
- [ ] Java client：协议 encode/decode 单元（若项目有 client test infrastructure）

---

## §4 P2 — 炉方块右键 → 开炉

> 放置成功后显示 `BlockState::FURNACE`，玩家需要右键该方块触发开炉屏幕。

- [ ] **客户端检测**：`BlockInteractionHandler` 拦截 `UseBlockC2SPacket` → 检查目标方块 `BlockState` 是否为 `FURNACE` 且坐标已在 `AlchemyFurnaceRegistry`（客户端维护本地炉位置缓存）
- [ ] **开炉请求**：客户端发 `AlchemyOpenFurnace { v:1, furnace_pos }` payload → server 校验炉存在 + 权限 → 返回 `AlchemyFurnaceState` snapshot
- [ ] **状态同步**：server 响应 `bong:alchemy/furnace_state` 通道下发当前炉料槽 / 状态 / 剩余时间
- [ ] **tests**：右键已注册炉 → 发 OpenFurnace + 收 FurnaceState；右键未注册坐标 → 无响应（vanilla 交互正常）；右键非炉方块 → vanilla 正常

---

## §5 P3 — owo-lib 三列炉 UI Screen

> plan-alchemy-v1 §3.3：三列 owo-lib UI（DragState 需扩展 FURNACE_SLOT target）。

- [ ] **DragState 扩展**：`DragTarget` 枚举加 `FURNACE_SLOT(slot_idx: u8)` 变体（允许背包 → 投料槽的拖拽）
- [ ] **Screen 布局**：`AlchemyFurnaceScreen extends DynamicXmlScreen`
  - 左列：投料槽（4 格，对应 IngredientSpec.slots）
  - 中列：炉状态（火焰进度条 / 炼丹阶段 / 当前丹方名）
  - 右列：成品槽（1 格）+ 当前 session 信息
- [ ] **SVG → owo-lib XML**：`client/src/main/resources/assets/bong-client/xml/alchemy_furnace.xml`；Cell size 按 `GridSlotComponent.CELL_SIZE=28` 而非草图 57×52
- [ ] **store 新增**：`AlchemyFurnaceStore`（当前炉状态，订阅 `bong:alchemy/furnace_state`）
- [ ] **拖拽目标接线**：`DragState.handleDrop` 分发 `FURNACE_SLOT` 目标 → 发 `AlchemyAddIngredient` payload
- [ ] **tests**：Screen open/close 生命周期；拖拽到 FURNACE_SLOT → 触发 AddIngredient 事件；无活 session 时投料槽禁用

---

## §6 P4 — Redis channel + agent TypeBox 对齐

> agent 侧需要订阅炼丹进展，实现天道 narration 介入。

- [ ] **server 新增 channel 发布**：
  - `bong:alchemy/session_start`（炼丹开始）
  - `bong:alchemy/session_end`（炼丹结束：成功 / 失败 / 炸炉）
  - `bong:alchemy/intervention_result`（干预结果）
- [ ] **Rust schema**：`server/src/schema/alchemy.rs` 新增对应 struct（`AlchemySessionStartV1` / `AlchemySessionEndV1` 等）
- [ ] **TypeBox 对齐**：`agent/packages/schema/src/alchemy.ts` + `schema-registry.ts` 注册新 type；`CHANNELS.ALCHEMY_SESSION` 等加入 channels.ts
- [ ] **agent 订阅**：`agent/packages/tiandao/src/redis-ipc.ts` 订阅 `bong:alchemy/session_end` → 触发天道 narration（炸炉是高戏剧性事件）
- [ ] **tests**：TypeBox schema sample 对拍；agent schema 单测；channel 名常量对照 Rust side

---

## §7 P5 — 配方材料名正典化

> 当前三份测试配方（kai_mai / hui_yuan / du_ming）使用 placeholder 材料 ID，需对齐 library 正典名。

- [ ] **正典名对照表**（来源 `末法药材十七种.json` + `辛草试毒录.json`）：
  | placeholder | 正典 ID | 辛度 / 丹毒色 |
  |---|---|---|
  | `kai_mai_cao` | `ci_she_hao` | 中辛 / Sharp |
  | `ning_mai_cao` | `ning_mai_cao` | 无变（已正典）|
  | `xue_cao` | `chi_sui_cao` | 高辛 / Violent |
  | `shou_gu` | `hui_yuan_zhi` | 低辛 / Mellow |
  | `huo_jing` | `huo_jing_cao` | 极高辛 / Violent |
  | `bai_cao` | `bai_lu_cao` | 低辛 / Mellow |
  | `ling_shui` | `ling_shui`（无变，液体类）| — |
- [ ] **更新三份 recipe JSON**：`server/assets/alchemy/recipes/{kai_mai,hui_yuan,du_ming}.json` 内 `ingredient_specs.material_id` 字段替换
- [ ] **更新辛度 → 丹毒色映射**：`server/src/alchemy/outcome.rs` 的辛度 → `ContamSource` 色系对照加注释引用 library
- [ ] **占位正式配方**：为 `ci_she_hao` / `ning_mai_cao` 等正典药材各起一份生产配方骨架 JSON（`server/assets/alchemy/recipes/draft/`，待 botany 平衡调参后上正式）
- [ ] **tests**：recipe 加载后 `ingredient_specs[*].material_id` 命中正典名；辛度 → 颜色 pin 测试

---

## §8 P6 — 炸炉结算

> `ResolvedOutcome::Explode` 当前只打 log，未对施法者造成实际伤害。

- [ ] **damage 应用**：`server/src/alchemy/resolver.rs` Explode 分支 → `AttackIntent` 事件（`attacker_id=None`，damage 由炉阶 × 投料灵气量决定，tier 1 最小 / tier 3 最大）
- [ ] **meridian_crack 应用**：Explode 同时 emit `MeridianCrackEvent { target: caster, severity: Minor }` → 接 plan-combat-no_ui 经脉伤害管线
- [ ] **AOE 可选项**（P6+ 延后）：炸炉方块本身 BlockState 重置 + 周围玩家受溅射伤害——需先讨论炉耐久设计，延后
- [ ] **tests**：tier 1 炉 Explode → caster damage > 0；tier 3 炉 Explode → damage > tier 1 值；meridian_crack event emit；无施法者（NPC 炉）→ 无 crash

---

## §9 P7 — furnace_fantie 图标资产

> `furnace_fantie` 背包图标路径 `client/src/main/resources/assets/bong-client/textures/gui/items/furnace_fantie.png` 缺失；Fabric 通路接通后玩家会看到 broken image。

- [ ] **生成图标**：用 `/gen-image item "凡铁炉，修仙丹道炼丹炉，深灰色铁质，正面有炉门，炉门内可见橘红火焰，古朴厚重，128×128"` 生成；或基于 MC 原版熔炉风格合成
- [ ] **路径**：`client/src/main/resources/assets/bong-client/textures/gui/items/furnace_fantie.png`（128×128 PNG）
- [ ] **高阶炉占位**：`furnace_lingtie.png` / `furnace_xiantie.png` 等待 forge-v1 品阶系统落地后批量生成；本 P 仅需 `furnace_fantie`
- [ ] **世界方块外观**：短期保持 vanilla `BlockState::FURNACE` 外观（可接受）；长期自定义 block model 等 Fabric mod 侧 block registry 成熟后重议
- [ ] **tests**：资源路径存在校验（CI 资源完整性检查）

---

## §10 数据契约（下游 grep 抓手）

| P | 契约 | 位置 |
|---|------|------|
| P0 | `BongItemRegistry.FURNACE_ITEMS` | `client/src/main/java/moe/bong/client/item/BongItemRegistry.java` |
| P1 | `furnace_pos: (i32,i32,i32)` in 5 payloads (Open/Ignite/Intervention/FeedSlot/TakeBack) + handler 按 pos 路由 + owner 校验 | `server/src/schema/client_request.rs` + `server/src/schema/alchemy.rs` (AlchemyFurnaceDataV1) + `server/src/network/client_request_handler.rs` + `agent/packages/schema/src/client-request.ts` + `agent/packages/schema/src/server-data.ts` + `client/.../ClientRequestProtocol.java` + `client/.../ClientRequestSender.java` |
| P2 | `AlchemyOpenFurnace` payload + `bong:alchemy/furnace_state` channel | `server/src/schema/channels.rs` |
| P3 | `DragTarget::FurnaceSlot(u8)` + `AlchemyFurnaceScreen` | `client/src/main/java/moe/bong/client/screen/AlchemyFurnaceScreen.java` |
| P4 | `bong:alchemy/session_start` + `session_end` + `intervention_result` | `server/src/schema/alchemy.rs` + `agent/packages/schema/src/alchemy.ts` |
| P5 | recipe JSON `ingredient_specs.material_id` 正典化 | `server/assets/alchemy/recipes/*.json` |
| P6 | `AttackIntent(attacker=None)` + `MeridianCrackEvent` in Explode branch | `server/src/alchemy/resolver.rs` |
| P7 | `furnace_fantie.png` 128×128 | `client/src/main/resources/assets/bong-client/textures/gui/items/` |

---

## §11 开放问题

- [ ] 多炉权限：炉是否绑定"放置玩家"独享，还是组队共用？（plan-social 的组队机制尚未立项）
- [ ] 炸炉方块耐久：炉本身是否有耐久（炸一次 -1，归零摧毁）？还是炸完炉依然完好仅丢材料？
- [ ] 高阶炉图标风格（灵铁炉 / 仙铁炉）：等 forge-v1 品阶定下来后批量跟进
- [ ] 丹方残卷损坏机制（§1.4 提过，未定数据结构）：残卷学到残缺版配方的 skip_slots 如何存？
- [ ] 续命丹（plan-death-lifecycle §4c）具体代价曲线：alchemy 有能力承载，但需 death plan 先定义接口
- [ ] AutoProfile 自动化炼丹（plan §1.3 预留口）：傀儡绑炉读曲线，等 NPC 系统成熟后

---

## §12 进度日志

- **2026-04-27**：骨架立项。从 `docs/plans-skeleton/reminder.md` plan-alchemy-v1 仍延后/依赖/开放问题节提炼。server 侧 P0–P5 + §1.2 放置炉已在 plan-alchemy-v1 完成（✅ 2026-04-15 / 2026-04-21）。
- **2026-05-01**：P1 实地核验 + patch plan 落地。确认 furnace_id（String，旧路由每玩家一炉）→ furnace_pos（`(i32,i32,i32)`，新路由世界坐标多炉并行），编写 §3 全链路迁移步骤（Rust server / TS agent / Java client 三栈 7 文件变更清单 + handler 路由改造 + 权限模型决策）。P1 进入执行状态（⏳）。

## Finish Evidence

- P0/P2/P3：客户端右键放置/打开真实炉，炼丹 UI 以 `furnace_pos` 发送 ignite/feed/take_back/intervention 请求。
- P1/P4：Rust/TS/Java 契约统一到 `furnace_pos`，新增炼丹 Redis `session_start` / `session_end` / `intervention_result` schema 与 tiandao `session_end` 订阅。
- P5：默认炼丹 recipe JSON 改用已注册正典材料 ID，并补配方 alias 防回退测试。
- P6：爆炉结算真实扣施法者 `Wounds`，按炉阶缩放反噬，并发 `MeridianCrackEvent` 写入经脉裂痕管线。
- P7：新增 `furnace_fantie.png` 128×128 图标及资源路径单测。
- 验证：`cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过，1883 tests passed。
- 验证：`cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` 通过。
- 验证：`cd agent/packages/schema && npm test && npm run check` 通过，231 tests passed。
- 验证：`cd agent/packages/tiandao && npm test` 通过，187 tests passed。
- 验证：`cd agent && npm run build` 通过。
