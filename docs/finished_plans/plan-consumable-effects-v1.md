# plan-consumable-effects-v1 — 手搓消耗品空壳消杀（11 个产出物补齐使用闭环）

> **主题**：手搓系统 11 个"消耗品"产出物当前全是空壳——有 ItemTemplate、有配方产出，但**缺 `[item.effect]` 字段**，使用时 effect=None 直接 return，只扣库存不施加任何效果（典型僵尸物品）。本 plan 把这 11 个逐个补齐"嗑下去→真实施效"的闭环；对无法用现有机制覆盖的（外伤/骨折治疗、知识物、炼丹输入），如实分流到正确机制或后续 plan，绝不强行塞错 variant。

> **来源**：2026-06-10 手搓产出物使用流程审计（104 个 → 41✅/2⚠️/61💀），本 plan 处理其中"💊 消耗品空壳"一类。

> **状态**：Finished —— P0-P4 已完成并通过 server 测试矩阵，归档至 `docs/finished_plans/`。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | 实地调查收口 + 决策门：逐个锁定 canonical 意图 → effect 映射表（直接映射/新 variant/重分类/转后续 plan 四档） | ✅ 2026-06-10 |
| P1 | 直接映射批：能镜像现有 `ItemEffect` variant 的，补 `[item.effect]` + pin 测试 | ✅ 2026-06-10 |
| P2 | 新 variant 批：现有 enum 无对应语义的，加 `ItemEffect` 变体 + parser key + apply 路径 + 饱和测试 | ✅ 2026-06-10 |
| P3 | 重分类批：本就不是"嗑"的物品（炼丹输入 / 知识物），接到正确系统而非 effect | ✅ 2026-06-10 |
| P4 | e2e + 收尾：UseQuickSlot→consume→施效 端到端；明确分流到其他 plan 的遗留 | ✅ 2026-06-10 |

**待处理 11 个**（均定义于 `server/assets/items/workbench_materials.toml`，产出配方在 `server/src/craft/workbench_recipes.rs`）：
`bandage` `arm_splint` `leg_splint` `huiyuan_decoction` `calming_tea` `meridian_salve` `ningmai_prep_kit` `meridian_rubbing` `qi_guide_talisman` `anti_gu_powder` `qingzhuo_powder`

---

## 接入面（已实地确认的锚点 / 红线）

- **效果数据契约**：`ItemEffect` enum `server/src/inventory/mod.rs:257`（现有 10 变体：BreakthroughBonus / QiRecovery / MeridianHeal / ContaminationCleanse / LifespanExtension / AntiSpiritPressure / PoisonPill / CombatPill / FoodRegen / BeastCoreAbsorption）
- **toml 形状**：`ItemEffectToml` `server/src/inventory/mod.rs:1606`（字段仅 `kind / magnitude / target? / duration_ticks?`——超出此形状的 variant 需扩 struct）
- **解析器**：`parse_item_effect`（`kind` 字符串 → variant，`server/src/inventory/mod.rs:~1996`，每个 variant 一个 `"key" =>` 臂）
- **模板字段**：`ItemTemplate.effect: Option<ItemEffect>` `server/src/inventory/mod.rs:139`
- **样例**（直接抄形状）：`server/assets/items/pills.toml`
  - `effect = { kind = "qi_recovery", magnitude = 60.0 }`（huiyuan_pill）
  - `effect = { kind = "meridian_heal", magnitude = 0.2, target = "any_meridian" }`（ningmai_powder）
  - `effect = { kind = "contamination_cleanse", magnitude = 0.6 }`（huiyuan_pill_forbidden）
  - item 级可加 `cast_duration_ms` / `cooldown_ms`
- **施效管线**（⚠️ P0 必须实地 pin 确切 fn/line）：use 请求 → consume → `apply_item_effect`（审计指向 `server/src/network/cast_emit.rs:~309`，effect=None 时提前 return；现有 `MeridianHeal` apply 在 `cast_emit.rs:445`、`ContaminationCleanse` 在 `cast_emit.rs:480`）；丹药路径 `handle_alchemy_take_pill`（`client_request_handler.rs:~8679`，注释称 MeridianHeal/ContaminationCleanse "待对应 tick 系统就位"——两条路径有分歧，P0 须确认每个 effect 走哪条）
- **伤情系统（已存在，绷带/夹板复用，非新建）**：
  - 状态：`WoundKind` enum `server/src/combat/components.rs:44`（Cut/Blunt/Pierce/Burn/Concussion，**无专属 Fracture**，骨折用 Blunt/Concussion 兜）；`Wound` 结构 `:57`（severity/bleeding_per_sec/location）；`Wounds` 组件 `:68`（entries + health_current/max）
  - 治疗函数（已实装并接进 gameplay）：`apply_wound_heal(wounds, target, grades)` `server/src/alchemy/pill.rs:406`；`apply_severed_mend` 同文件（经脉接续）
  - 现有玩家触发：`client_request_handler.rs:9169-9182` —— `CombatPillKind::HuoXueDan`(活血丹)→`apply_wound_heal`、`XuGuGao`(续骨膏)→对最重部位疗伤、`DuanXuSan`(断续散)→`apply_severed_mend`；NPC 走 `npc/npc_skill.rs:178`
  - **缺口**：疗伤仅对硬编码 `CombatPillKind` 开放；手搓 `bandage`/`arm_splint`/`leg_splint` 既未进 `CombatPillKind` dispatch，`ItemEffect` 也无通用 `WoundHeal` 变体
- **先例（必读，照抄落地模式）**：`docs/finished_plans/plan-food-v1.md`（新增 `FoodRegen` variant + 消费路径），`plan-dandao-runtime-wiring-v1.md` / `plan-alchemy-v2.md`
- **🚩 红线**：
  1. **不强塞**：找不到语义匹配的现有 variant，就走 P2 加新 variant 或 P3 重分类，**严禁**把 `bandage` 硬塞成 `qi_recovery` 之类糊弄。
  2. **正典优先**：每个物品的 canonical 意图先查 `docs/worldview.md`（修经脉/解蛊/污染/安神等术语），效果方向/数值不得偏离正典。
  3. **category 一致性**：若施效路径要求 `category=pill`（见 `handle_alchemy_take_pill` 前置），改 effect 的同时确认 category 正确，否则补另一条 use 路径。

---

## P0 — 实地调查收口 + 决策门 ✅ 2026-06-10

**交付物**：一张逐物品映射表（11 行），每行锁定四档之一，附 file:line 依据。**由 field-investigation workflow 产出后填入此处。**

每行字段：`物品 id | 中文名 | canonical 意图(worldview 锚) | 候选现有 variant(mod.rs:行) | 镜像样例物品 | 档位 | 缺口`

四档：
- **①直接映射**：现有 variant 语义吻合 → 仅补 `[item.effect]`（进 P1）
- **②新 variant**：意图明确但 enum 无对应 → 加变体（进 P2）
- **③重分类**：本就不是"嗑"的（如 `ningmai_prep_kit`=炼丹输入、`meridian_rubbing`=知识/拓片）→ 接正确系统（进 P3）
- **④转后续 plan**：依赖尚不存在的机制（如外伤/骨折治疗系统，`WoundKind` 无 `Fracture`）→ 本 plan 范围外，登记 follow-up

**施效路径裁定（实地核实，load-bearing）**：misc/pill 消耗品经 QuickSlot 使用 → `apply_cast_item_effect`（`cast_emit.rs:565`）。该函数对 LifespanExtension/AntiSpiritPressure/PoisonPill/FoodRegen 特办，**其余 `_ =>` fall through 到 `apply_item_effect`（`cast_emit.rs:436`，由 `:727` 转发）**——故 `QiRecovery`(:502)/`MeridianHeal`(:445)/`ContaminationCleanse`(:480) 在 cast 路径**真实施效**。⚠️ **`CombatPill` 在 cast 路径 `:622` 是 no-op**（"ignored on generic cast path"）；其疗伤（`apply_wound_heal` pill.rs:406）**仅在 `AlchemyTakePill→handle_alchemy_take_pill` 的 `CombatPillKind` dispatch（`client_request_handler.rs:9174`）活**，要求 category=pill。**结论：QuickSlot 消耗品要疗伤，必须新增 cast-path `WoundHeal` 变体，不能复用 CombatPill。**

**P0 决策表（field-investigation `wvlyzs44i` 产出，已收口三处伤情类分歧）**：

| # | 物品 id | 中文 | canonical 意图 | 档 | 落地 |
|---|---------|------|----------------|----|------|
| 1 | `huiyuan_decoction` | 回元芷煎汤 | 回当前真元（不提上限），回元丹民间煎剂 | ①direct | `effect = { kind = "qi_recovery", magnitude = 40.0 }`（镜像 huiyuan_pill 60，降一档；apply cast_emit.rs:502）|
| 2 | `meridian_salve` | 养经膏 | 外敷推进经脉 crack 愈合 | ①direct | `effect = { kind = "meridian_heal", magnitude = 0.2, target = "any_meridian" }`（镜像 ningmai_powder；apply :445）|
| 3 | `meridian_rubbing` | 经脉图拓片 | 拓片辅助经脉裂痕自愈 | ①direct | `effect = { kind = "meridian_heal", magnitude = 0.15, target = "any_meridian" }`（低于精炼丹药；apply :445）|
| 4 | `qingzhuo_powder` | 清浊散 | 降经脉污染 contamination | ①direct | `effect = { kind = "contamination_cleanse", magnitude = 0.4 }`（uncommon，低于 legendary 禁药 0.6；apply :480）|
| 5 | `anti_gu_powder` | 解蛊散 | 解蛊=清毒蛊污染（worldview:532）| ①direct | `effect = { kind = "contamination_cleanse", magnitude = 0.4 }`（apply :480）|
| 6 | `qi_guide_talisman` | 灵气引导符 | 临时修炼加速（worldview:93）| ①direct | `effect = { kind = "food_regen", magnitude = 0.30, duration_ticks = 36000 }`（duration_ticks 必填；apply Noop 分支 cast_emit.rs:710 → CultivationAcceleration）|
| 7 | `calming_tea` | 安神茶 | 即时回心境 composure（worldview:330）| ②new variant | 新增 `ItemEffect::ComposureRestore{magnitude}` + parser key + cast arm（`cultivation.composure=(c+mag).min(1.0)`）；`effect = { kind = "composure_restore", magnitude = 0.35 }` |
| 8 | `bandage` | 止血绷带 | 压迫止血+轻伤恢复（凡物急救，无丹毒）| ②new variant | **WoundHeal**（见下）；`effect = { kind = "wound_heal", magnitude = 1.0 }`（无 target=全身）|
| 9 | `arm_splint` | 夹板·臂 | 固定臂部骨折、降 severity（≈续骨膏）| ②new variant | **WoundHeal**；`effect = { kind = "wound_heal", magnitude = 2.0, target = "arm_l/arm_r" }`；splinted 旗标降级后续 |
| 10 | `leg_splint` | 夹板·腿 | 固定腿部骨折、降 severity、解移速罚 | ②new variant | **WoundHeal**；`effect = { kind = "wound_heal", magnitude = 2.0, target = "leg_l/leg_r" }`；splinted 旗标降级后续 |
| 11 | `ningmai_prep_kit` | 凝脉散预制包 | 炼丹前驱包→炼丹炉产凝脉散 | ③reclassify | 新建 `server/assets/alchemy/recipes/ningmai_san_v1.json`，stage 输入 `ningmai_prep_kit`×1 → 产出 `ningmai_powder`（pill 已定义 pills.toml:26）|

> **WoundHeal 共用变体（覆盖 #8/#9/#10）**：现有 `apply_wound_heal`（pill.rs:406）已实装，但 cast 路径无 `Wounds` 访问（`CastItemEffectTargets` 仅 cultivation/meridians/contamination）。需：① `ItemEffect::WoundHeal{magnitude, target: Option<String>}`；② parser key `"wound_heal"`；③ `CastItemEffectTargets` 加 `wounds: Option<&mut Wounds>`（cast_emit.rs:70 + 调用点 :312 透传）；④ cast arm 调 `apply_wound_heal`。BodyPart 现有 `LegL/LegR` 等（combat/components.rs:31），`splinted` 旗标（worldview:260）代码无、降级为后续增强。

**锚点行号纠正**（field-check 实测）：`parse_item_effect` 在 `mod.rs:1992`（非草稿写的 ~1996）；`QiRecovery` apply 在 `cast_emit.rs:502`（草稿误指 :445，:445 实为 MeridianHeal）。

**决策门**：上表已定稿（含路径裁定 + 伤情分歧收口），P1/P2/P3 据此执行。

---

## P1 — 直接映射批（补 `[item.effect]`）✅ 2026-06-10

**交付物**：
- `server/assets/items/workbench_materials.toml`：P0 表中①档物品各加一行 `effect = { kind = "...", ... }`（+ 必要的 `cast_duration_ms`/`cooldown_ms`）
- 测试：每个①档物品一条 pin 测试，断言 `ItemRegistry` 加载后 `template.effect == Some(ItemEffect::X{..})`（参照 `server/src/inventory/mod.rs:4675+` 现有 effect 加载断言模式）
- 不改任何 Rust 逻辑（纯数据 + 测试）

---

## P2 — 新 variant 批 ✅ 2026-06-10

**交付物**（仅当 P0 判定②档非空时）：
- `server/src/inventory/mod.rs:257`：新增 `ItemEffect` 变体——`ComposureRestore { magnitude }`（安神）+ `WoundHeal { magnitude, target: Option<String> }`（疗伤，#8/#9/#10 共用）
- `parse_item_effect`（`mod.rs:1992`）：新增 `"composure_restore" =>` / `"wound_heal" =>` 臂；若字段超出 `ItemEffectToml` 形状（`mod.rs:1606`）则扩 struct
- 施效路径（`cast_emit.rs` apply_item_effect）：新增 variant 的 apply 分支，真实改 player 状态（参照现有 `MeridianHeal`/`ContaminationCleanse` 分支）；**疗伤类直接调既有 `apply_wound_heal`（pill.rs:406），不重写治疗逻辑**
- **疗伤绑定（P0 已裁定 → 方案 b）**：新增 `WoundHeal` variant 直连 `apply_wound_heal`。方案 (a) 复用 `CombatPill` **已否决**——`CombatPill` 在 QuickSlot cast 路径是 no-op（`cast_emit.rs:622` "ignored on generic cast path"），仅 `AlchemyTakePill→handle_alchemy_take_pill` 的 `CombatPillKind` dispatch（`client_request_handler.rs:9174`）活，而绷带/夹板走的正是 cast 路径，故不可复用。需把 `Wounds` 透传进 `CastItemEffectTargets`（`cast_emit.rs:70` 加 `wounds: Option<&mut Wounds>` + 调用点 `:312`）
- 饱和测试：parser 正反 sample 对拍 + apply 分支状态变化断言（伤情 severity 下降/愈合移除/回血）+ effect=None 仍提前 return 的回归

---

## P3 — 重分类批 ✅ 2026-06-10

**交付物**（仅当 P0 判定③档非空时）：
- `ningmai_prep_kit`（若判定为炼丹输入）：在对应 alchemy recipe JSON（`server/assets/alchemy/recipes/`）加它为 stage 输入，或在 dandao 加工链接入；补"被消耗"测试
- `meridian_rubbing`（若判定为知识/拓片物）：接学习/图谱系统或改 `category=scroll` 走 `TechniqueScrollUse` 路径；补对应 use 测试
- 不在本 plan 强行造 effect

---

## P4 — e2e + 收尾 ✅ 2026-06-10

**交付物**：
- e2e：对至少 3 个代表性物品（①/②各取样）走 client `UseQuickSlot` → server consume → effect applied → 可观察状态变化（qi/经脉/污染）的端到端用例
- 回归：consume 后库存正确扣减；effect=None 物品仍只扣库存不报错
- 遗留登记：P0 实判为④档（确认机制不存在）的物品，写入 `docs/plans-skeleton/reminder.md` 或新 skeleton，注明依赖的机制 plan（当前预期④档为空）

---

## Finish Evidence

- **落地清单**：
  - P1：`server/assets/items/workbench_materials.toml` 为 `huiyuan_decoction`、`meridian_salve`、`meridian_rubbing`、`qingzhuo_powder`、`anti_gu_powder`、`qi_guide_talisman` 补齐 `effect`；`server/src/inventory/mod.rs` 增加 10 个 item effect pin 断言。
  - P2：`server/src/inventory/mod.rs` 新增 `ComposureRestore` / `WoundHeal` parser；`server/src/network/cast_emit.rs` 新增心境恢复、疗伤施效和 QuickSlot 端到端测试；`server/src/network/client_request_handler.rs` 保持丹药路径对 QuickSlot-only effect 的显式 no-op。
  - P3：`server/assets/alchemy/recipes/ningmai_san_v1.json` 新增 `ningmai_prep_kit` → `ningmai_powder` 复炼配方；`server/src/alchemy/recipe.rs` pin 默认 registry 加载。
  - P4：`server/src/network/cast_emit.rs` 覆盖 `UseQuickSlot` → consume → `qi_recovery` / `composure_restore` / `wound_heal` 代表性 e2e。
- **关键 commit**：
  - `93b2dbc82` · 2026-06-10 · `实现手搓消耗品使用闭环`
- **测试结果**：
  - `cd server && cargo fmt --check`：通过。
  - `cd server && cargo test loads_item_registry_from_assets`：通过。
  - `cd server && cargo test load_registry_from_default_dir`：通过。
  - `cd server && cargo test composure_restore_clamps_to_full_composure`：通过。
  - `cd server && cargo test wound_heal_targets_all_wounds_when_target_missing`：通过。
  - `cd server && cargo test wound_heal_slash_target_filters_body_part_group`：通过。
  - `cd server && cargo test tick_casts_consumable`：3 passed。
  - `cd server && cargo clippy --all-targets -- -D warnings`：通过。
  - `cd server && cargo test`：8161 passed；0 failed；1 ignored。
- **跨仓库核验**：
  - server：命中并落地 `ItemEffect`、`parse_item_effect`、`apply_item_effect`、`CastItemEffectTargets`、`apply_wound_heal`、`RecipeRegistry` 默认加载。
  - client：本 plan 未改 client；入口复用既有 QuickSlot C2S cast 流程。
  - agent：本 plan 未改 agent/schema。
- **遗留 / 后续**：
  - P0 四档裁定中无 ④档物品；夹板的 `splinted` 持续旗标仍依赖后续伤情扩展 plan，本 plan 仅按既有 `apply_wound_heal` 做 severity 降级/愈合。
