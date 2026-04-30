# Bong · plan-alchemy-client-v1 · 骨架

**炼丹系统 Fabric 客户端全接入**。plan-alchemy-v1 server 侧（P0–P5 + §1.2 放置炉）已闭环；本 plan 补全客户端拦截放置、多炉 session 路由、owo-lib 炉 UI 屏幕、agent schema 对齐、配方材料名正典化、炸炉结算 + skill XP 钩子，形成完整玩家端炼丹链路。

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
- P1 ⬜ 多炉 session 路由（payload 加 furnace_pos，AlchemyOpenFurnace / Ignite / Intervention 全部更新）
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

## §3 P1 — 多炉 session 路由

> 当前 `AlchemyIntervention` / `AlchemyIgnite` / `AlchemyOpenFurnace` payload 均靠 `ev.client`（玩家 Entity）定位唯一炉，需改为 `furnace_pos` 路由。

- [ ] **所有 alchemy payload 加字段**：在 `ClientRequestV1` 的 `AlchemyOpenFurnace` / `AlchemyIgnite` / `AlchemyIntervention` / `AlchemyTerminate` 中加 `furnace_pos: (i32, i32, i32)`
- [ ] **TypeBox 同步**：`agent/packages/schema/src/alchemy.ts` 中对应 Type.Object 同步加字段
- [ ] **server handler 改路由**：`handle_alchemy_ignite` 等从 `furnaces.get_mut(ev.client)` 改为 `furnaces.get_by_pos(ev.furnace_pos)` + 权限校验（该炉是否属于本玩家或允许的范围）
- [ ] **tests**：两个玩家各自操作不同炉坐标 → 互不干扰；同炉坐标不同玩家 → 权限拒绝（炉属主校验）

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
| P1 | `furnace_pos: (i32,i32,i32)` in all alchemy payloads | `server/src/schema/client_request.rs` + `agent/packages/schema/src/alchemy.ts` |
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

- **2026-04-27**：骨架立项。从 `docs/plans-skeleton/reminder.md` plan-alchemy-v1 仍延后/依赖/开放问题节提炼。server 侧 P0–P5 + §1.2 放置炉已在 plan-alchemy-v1 完成（✅ 2026-04-15 / 2026-04-21）。本 plan 全 P 均为 server 以外的接入工作，等 `/consume-plan alchemy-client-v1` 升 active。
