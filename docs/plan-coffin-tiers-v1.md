# Bong · plan-coffin-tiers-v1（Active）

**延寿棺建模接入 + 灵材四档**——把延寿棺从双 `BlockState::CHEST` 占位换成 GeckoLib 建模实体（照搬物资棺渲染链），并落地正典 `plan-coffin-v1 §遗留` 的灵材分档：凡木 ×0.9 / 寒玉 ×0.7 / 玄石 ×0.5 / 青铜 ×0.3。源模型已就绪（`local_models/{Mundane,Jade,Stone,Bronze}Coffin.bbmodel`，**以 bbmodel 为基准**）。

| 阶段 | 内容 | 状态 | 验收日期 |
|------|------|------|----------|
| P0 | 分档底盘：`CoffinGrade` enum + 倍率表 + item_id ×4 + schema tier 字段（双端 serde） | ✅ | 2026-06-04 |
| P1 | 资产管线：4 档 bbmodel → `geo.json` + 贴图 PNG + `animation.json`（**含 UV/贴图收尾，见 §视觉资产**） | ✅ | 2026-06-04 |
| P2 | Server 建模实体：放置时 spawn `BongVisual` marker（按 grade）替代双 CHEST；`CoffinEntity` 持有实体 id | ✅ | 2026-06-04 |
| P3 | Client 渲染 + 交互迁移：`BongEntityModelKind` ×4 + 渲染器壳 + bootstrap；**marker-entity 交互三件套 C2S intent 接线**（进棺→`CoffinEnterRequest`、攻击破坏→`CoffinBreakRequest`、G菜单→`CoffinMenuReclaimRequest`，server handler+event 已在 P2 就位，P3 只换 emit 源）；退役 CHEST 右键 mixin；HUD 档位徽章 | ✅ | 2026-06-04 |
| P4 | 三新档 item/recipe（灵材配方）+ 端到端集成 + dev 命令 + 平衡 | ⬜ | |

**世界观锚点**：`worldview.md §十二 死亡重生与一生记录`（寿元上限 / 续命代价）+ `§十四 玩家循环`（灵龛挂机安全设施）。灵材等级越高、封存寿元越强 = 续命代价的被动梯度。

**交叉引用**：
- `plan-coffin-v1`（finished）——本 plan 的基底：`coffin/mod.rs` 全套（放置/进出棺/破坏/`CoffinComponent`/`coffin_state`），`§遗留` 明列灵材棺 ×0.7/0.5/0.3
- `plan-supply-coffin-v1`（finished）——**渲染链模板**：`BongEntityModelKind` / `BongModeledEntity` / `entity_model.rs::spawn_visual_marker` / `SupplyCoffinInteractIntentHandler`
- `plan-entity-model-v1`（finished）——`BongVisualKind` / raw_id 注册管线 / `BongVisualState` tracked data（idx 8）
- `plan-lifespan-v1`（finished）——寿元 tick（`cultivation/lifespan.rs::lifespan_aging_tick` 三因子连乘，coffin 因子在 `:440`）

---

## 接入面 Checklist

- **进料**：
  - `coffin::handle_coffin_place_requests`（`:218`，玩家持 coffin item 右键 → 放置）→ 现取 `MUNDANE_COFFIN_ITEM_ID`，扩成 4 个 item_id
  - `inventory` / `ItemRegistry`——4 档 coffin item 模板 + recipe（灵材材料，见 §8 #2）
  - `cultivation/lifespan.rs::lifespan_aging_tick`——按 `CoffinComponent.grade` 取倍率
- **出料**：
  - `coffin_state` CustomPayload（`bong:server_data`）→ client：新增 `coffin_grade` 字段供 HUD 徽章
  - `entity_model::spawn_visual_marker(...)`（**实为 `entity_model.rs:176` 私有 6 参 fn `(commands, layer, source, kind, pos, visual_state)`，P2 须先改 `pub` 或加 coffin wrapper**）→ client GeckoLib 渲染
  - 倍率耗损仍走 `plan-coffin-v1` 既有 `player_lifespan` 落盘 + 离线回算（**不新建寿元路径**）
- **共享类型 / event**：复用 `BongVisualKind`（加 4 variant）/ `CoffinComponent`（加 `grade` 字段）/ `CoffinEntity`（加 `grade` + `marker_entity` 字段）/ `CoffinStateV1`（加 `coffin_grade`）。**不新建**寿元 component/event
- **跨仓库契约**：
  - server：`coffin::CoffinGrade`、`entity_model::{CoffinMundane,CoffinJade,CoffinStone,CoffinBronze}` EntityKind raw_id **161-164**（160 已占用，见 §8.1 #1）
  - schema：`CoffinStateV1.coffin_grade`（TS `server-data.ts` + Rust `schema/server_data.rs` 对拍 + `coffin.test.ts` sample）
  - client：`BongEntityModelKind.COFFIN_{MUNDANE,JADE,STONE,BRONZE}`（raw_id 须 1:1 对齐 server）、`CoffinModeledInteractIntentHandler`
- **worldview 锚点**：见上（§十二 + §十四）
- **qi_physics 锚点**：**无**——本 plan 只调寿元 tick 倍率（`lifespan` 系统），不涉真元/灵气流动、不碰守恒律。⚠️ 正典 `plan-coffin-v1 §遗留` 提到的「符文棺附加灵气微回复」**明确不在本 plan 范围**；若后续做，必走 `qi_physics::ledger::QiTransfer`，另立 plan，不在此私写。

---

## 边界

| 维度 | 做 | 不做 |
|------|----|------|
| 视觉 | 4 档延寿棺 GeckoLib 建模实体（替换双 CHEST） | 七流派外观定制（`plan-coffin-v1 §遗留` 另列）|
| 分档 | 4 档寿元倍率 0.9/0.7/0.5/0.3 + 4 item/recipe | 棺材符文 / 灵气微回复 / 梦境训练（涉 qi_physics，另立 plan）|
| 交互 | 进/出/放置/破坏迁到 marker-entity intent | 双人/道侣共享棺（需改 `CoffinRegistry` 1:1 映射，另列）|
| 资产 | geo/texture/animation ×4 落地（形体以 bbmodel 为基准） | bbmodel 几何重做（已定稿，本 plan 只导出+接入）|

---

## P0 — 分档底盘（schema + tier model）

**交付物**：
- `server/src/coffin/mod.rs`：
  - 新增 `enum CoffinGrade { Mundane, Jade, Stone, Bronze }`（`:32` 附近，替 `COFFIN_LIFESPAN_FACTOR=0.9` 单常数）
  - `fn CoffinGrade::lifespan_factor(self) -> f64`：0.9 / 0.7 / 0.5 / 0.3 查表
  - `fn CoffinGrade::item_id(self) -> &str`：`mundane_coffin` / `jade_coffin` / `stone_coffin` / `bronze_coffin`
  - `CoffinComponent`（`:37`）加 `grade: CoffinGrade`；`CoffinEntity`（`:43`）加 `grade: CoffinGrade`
  - `coffin_lifespan_multiplier`（`:209-215`）签名改 `Option<CoffinGrade>`，按 grade 查表（保持离线回算 `offline_lifespan_multiplier` 一致乘算）
- `server/src/cultivation/lifespan.rs:440`：`coffin_lifespan_multiplier(coffin.map(|c| c.grade))`
- **schema**：`agent/packages/schema/src/server-data.ts:1132` `CoffinStateV1` 加 `coffin_grade: Union(['mundane','jade','stone','bronze'])`；`server/src/schema/server_data.rs:78` 对应 serde enum；`agent/packages/schema/tests/coffin.test.ts` 加 4 档正反 sample 对拍
- **测试**：`coffin::grade` 4 档倍率单测（含离线 ×0.09/×0.07/×0.05/×0.03 = OFFLINE 0.1 × factor）；schema 4 档 sample roundtrip

## P1 — 资产管线（bbmodel → geo/texture/animation）

> **以 `local_models/*.bbmodel` 为基准**（Mundane/Jade/Bronze 已 Blockbench 精修 fmt5.0，Stone 仍生成器原版 fmt4.10）。

**交付物**：
- `client/src/main/resources/assets/bong/geo/{mundane,jade,stone,bronze}_coffin.geo.json`：4 个，identifier `geometry.bong.<id>_coffin`
- `client/src/main/resources/assets/bong/textures/entity/{...}_coffin_intact.png`：4 个
- `client/src/main/resources/assets/bong/animations/{...}_coffin.animation.json`：4 个，至少 `animation.bong.<id>_coffin.idle`（lid 骨骼掀盖/微浮 idle，参 `coffin_common.animation.json`）

**资产导出（§8.1 #5：贴图已就绪，直接导出，不占位）**：
- **贴图直接从 bbmodel 内嵌取**——用户已在 Blockbench 把 4 档模型贴图做好（内嵌于 `.bbmodel`），P1 直接抽出存 PNG，**不做占位、不分两步**
- **GeckoLib 尺寸硬约束**：`geo.json` 的 `texture_width/height` **必须等于**导出 PNG 实际尺寸（不自动缩放）。按 bbmodel resolution 导出（当前 64×64）
- **新增 `scripts/models/export_coffin_assets.py`**：复用 `render_bbmodel.py` 的 bbmodel 解析 → 批量抽内嵌贴图存 `textures/entity/*_coffin_intact.png` + 转 geckolib `geo.json`（免手工 Blockbench 导出）。Stone 档若仍 fmt4.10 生成器原版，同样直接导出（用户认可现状）
- animation：4 档至少 `idle`（lid 微浮/掀盖，参 `coffin_common.animation.json`）
- 接入后用 `render_bbmodel.py` + 实机比对核验渲染（**真长相**，非平涂示意）；走 §10.1 多轮 review（贴图既已定，重点核 geo 转换正确 + 实机无破面/UV 错位）

## P2 — Server 建模实体（替换双 CHEST）

**交付物**：
- `server/src/world/entity_model.rs`：
  - `:34` 附近加 `COFFIN_{MUNDANE,JADE,STONE,BRONZE}_ENTITY_KIND = EntityKind::new(161..164)`（160 已占用）
  - `:54-72` `enum BongVisualKind` 加 4 variant + `entity_kind()` 映射
  - `CoffinGrade::visual_kind()`（仿 `SupplyCoffinGrade::visual_kind` `:95-105`）
- `server/src/coffin/mod.rs`：
  - `handle_coffin_place_requests`（`:218`；删双 `BlockState::CHEST` set_block `:318-321`）：改调 `spawn_visual_marker`——**该 fn 现为 `entity_model.rs:176` 私有 6 参 `(commands, layer, source, kind, pos, visual_state)`，P2 须先改 `pub`/加 coffin wrapper**，传全 6 参（commands / layer / `Some(source)` / `grade.visual_kind()` / lower_pos / visual_state）；`CoffinEntity` 记 `marker_entity: Entity`
  - `handle_coffin_breaks`：破坏（左键攻击棺，体验同破坏方块）→ despawn marker + **随机返还合成材料**（调通用 fn `craft::reclaim::recipe_reclaim_drops(recipe, Break)`，见 §8.1 #3）；离开仍 server 读 sneak；重连恢复重建 marker（`reclaim_occupied` 对齐 grade）
  - 新增 `handle_coffin_menu_reclaim`：菜单 [回收] → despawn marker + 返还材料（`recipe_reclaim_drops(recipe, Reclaim)`，比破坏返还更全）
  - `coffin_state` 推送（`:637`）加 `coffin_grade`
- 新增 `server/src/craft/reclaim.rs`（**通用工具**）：`recipe_reclaim_drops(recipe: &CraftRecipe, mode: ReclaimMode) -> Vec<(ItemId,u32)>`，Break=随机部分返还 / Reclaim=较全返还；对所有 workbench 合成的可放置物通用（棺材/未来家具）
- **测试**：放置→marker spawn（visual_kind 正确）；破坏→marker despawn + 随机材料返还（Break 模式数量在 [0,count] 内）；[回收]→较全返还；`recipe_reclaim_drops` 纯函数单测（含空配方/边界）；重连→marker 重建

## P3 — Client 渲染 + 交互迁移

**交付物**：
- `client/.../entity/BongEntityModelKind.java`（`:15-47`）：加 `COFFIN_{MUNDANE,JADE,STONE,BRONZE}`（raw_id 161-164，texture_width/height 按各 geo）
- `client/.../entity/Coffin{Mundane,Jade,Stone,Bronze}Renderer.java`：4 渲染器壳（仿 `CoffinCommonRenderer`）
- `client/.../entity/BongEntityRenderBootstrap.java`（`:18-34` BINDINGS）：加 4 行绑定
- **交互模型**（用户拍板，参 memory `feedback_no_vanilla_hacks`：marker entity 交互走 C2S IntentHandler）：
  - **G 键 → 棺材菜单**（仿 `client/.../cultivation/voidaction/VoidActionScreen` 动作选择屏）：选项 **[入眠]**（= `coffin_enter` 延寿）/ **[回收]**（返还材料 + despawn）/ 预留扩展位。G → C2S `CoffinMenuRequest` → server 发菜单 payload → client 开 `CoffinMenuScreen`；各选项走 C2S intent（`CoffinMenuActionRequest`）
  - **破坏**：左键攻击棺 marker 实体（**体验同破坏方块**）→ client 发 C2S intent → server emit **`CoffinBreakRequest`**（P2 已就位的 server handler `handle_coffin_breaks` 消费它）→ 随机返还材料（§8.1 #3）。⚠️ **棺已是 marker 实体无方块，不能走 `DiggingEvent`/挖方块**——必须走实体攻击 intent（P2 已把 break handler 从 DiggingEvent 改为 `CoffinBreakRequest`，P3 只补 client 攻击→intent 这一段）。放置仍 item-use；出棺仍 server 读 sneak
  - 退役 `MixinClientPlayerInteractionManagerAlchemy` 的右键-CHEST-进棺路径（双 CHEST 已废）；进棺改为 client 与 marker 实体交互 → 发 `CoffinEnterRequest`（server handler 已就位）。三件套 C2S intent（`CoffinEnterRequest`/`CoffinBreakRequest`/`CoffinMenuReclaimRequest`）server 端 event+handler 均 P2 落地，P3 只换 client emit 源
  - `CoffinStateHandler` / `CoffinStateStore`：读新 `coffin_grade`
  - `CoffinHudPlanner`：HUD「卧棺·寿火徐燃」面板加档位徽章 + 对应倍率文字
- **测试**：`CoffinStateHandlerTest` 加 grade 解析；G→菜单→[入眠]/[回收] e2e；`BongEntityModelKind` raw_id 1:1 对齐 server（pin 测试）；HUD planner 4 档徽章

## P4 — Item/recipe + 集成 + 平衡

> **制作台已落地**（`plan-workbench-recipes-v1`）：`server/src/craft/workbench.rs::WorkbenchBlock` + `WorkbenchScreen`，3 格交互。配方走 `CraftRecipe`（`craft/recipe.rs:170`），`station: Option<CraftStationKind>`（目前仅 `Workbench`，`recipe.rs:28`）。凡物棺现 `station: None` 徒手（`coffin/mod.rs:192-206`，灵木板×6+灵木棍×2，`scroll_mundane_coffin` 解锁）。

> 材料与配方由 sonnet workflow（`coffin-tier-materials`）调查 + 用户拍板，全部收口于 **§8.1 #2**。

**交付物（材料）**：
- `server/assets/items/minerals.toml`：补 **`yu_sui`（玉髓）** + **`wu_yao`（乌曜石）** ItemTemplate（二者已在 `server/src/mineral/registry.rs:94-95` 注册为矿物，仅缺 item 模板）。`spirit_quality_initial`：yu_sui 0.8 / wu_yao 0.85。category=misc
- **`gu_tong_pian`（古铜片）** 全新材料：item 模板（category=misc, rarity=legendary, `spirit_quality_initial=0.6`「灵力已散」）+ loot 双挂——坍缩渊深层 `ancient_relic` 池 ~3-5% + 暴龙王据穴古迹层 ~1-2%
- **雪魄莲采集（已实装，复用）**：`xue_po_lian` 的 EnvLock 现成——`server/src/botany/registry.rs:233 ENV_XUE_PO_LIAN = [SnowSurface, QiVeinFlow{0.3}]`（用于 `:1141`），雪面采集已可用。**无需新增**（`FrostBelt` 变体不存在，前述属笔误），直接复用，不走 `an_shen_guo` 替代
- **3 配方卷轴** item 模板 `scroll_{jade,stone,bronze}_coffin` + loot 挂点：jade 卷→漆棺（Rare grade loot）/ stone 卷→祭坛棺（Precious grade）/ bronze 卷→坍缩渊深层古遗物池
- **物品图标**（memory `feedback_item_icon_gen`：新 ItemTemplate 配 `/gen-image item`）：为新 item 生成图标——`yu_sui`(青白温润玉髓) / `wu_yao`(漆黑赤纹乌曜石) / `gu_tong_pian`(饕餮纹古铜碎片) / 3 张 `scroll_*_coffin`(残卷)。注：**棺材模型贴图≠材料图标**，前者已就绪(§8.1 #5)，此处仅新材料的 2D item 图标

**交付物（配方，扩 `coffin::register_craft_recipes` `:192`，全 `station: Some(CraftStationKind::Workbench)` + `category: Misc`）**：

| 档 | id | 材料 | qi_cost | time | 解锁 | realm_min（craft 门控）|
|----|----|------|---------|------|------|------|
| 寒玉 ×0.7 | `coffin.jade_coffin` | ling_mu_ban×4 + yu_sui×3 + xue_po_lian×2 | 2.0 | 120s(2400t) | Scroll | None |
| 玄石 ×0.5 | `coffin.stone_coffin` | xuan_iron×4 + zhen_shi_zhong×2 + wu_yao×2 | 4.0 | 150s(3000t) | Scroll + Mentor(地师流) | None |
| 青铜 ×0.3 | `coffin.bronze_coffin` | xuan_iron×3 + ling_mu_jing×2 + gu_tong_pian×4 + zhen_shi_gao×1 | 6.0 | 180s(3600t) | Scroll + Mentor(炼器流) | **Some(Realm::Induce)** |

- **门控原则（用户拍板）**：门控只在**制作**端（青铜棺 craft 需引气境，首个 realm 硬门控配方先例）；**进棺/使用任何档棺材无境界门控**（不改 `coffin::handle_coffin_enter_requests` `:343`）
- **灵质守恒**：三档已过 `workbench_recipes.rs` 守恒 pin test 估算（input×0.95 ≥ output，见 §8.1 #2）

**交付物（集成）**：
- dev 命令：`/give <coffin_id>` 已覆盖；按需加 `/coffin grade <id>` 直写测试档
- e2e：`scripts/e2e/coffin-lifecycle.sh` 扩 4 档（放置→进入→寿命倍率验证×4→破坏）
- 平衡梯度：倍率 0.9→0.7→0.5→0.3 · 材料 common→uncommon→rare→very_rare · qi 0→2→4→6 · 解锁 无→Scroll→+Mentor→+Mentor+realm

---

## §8 开放问题（P0 决策门前需收口）

> 实施前必须并行起 Explore agent 核查代码现状，追加 `## §8.1 决议（pre-P0 收口）` 双锚点（文件:行号 + 章节）。

1. **#1 raw_id 区间** ✅ **已收口，见 §8.1 #1**（160 已占用 → 改用 161-164）
2. **#2 灵材配方** ✅ **已收口，见 §8.1 #2**（sonnet workflow 调查 + 用户 2026-06-03 拍板）
3. **#3 破坏/回收机制** ✅ **已收口，见 §8.1 #3**（破坏=攻击掉随机材料 / G→菜单[入眠][回收] / 通用 reclaim fn）
4. **#4 mundane 迁移** ✅ **已收口，见 §8.1 #4**（全迁）
5. **#5 棺材模型贴图** ✅ **已收口，见 §8.1 #5**（bbmodel 内嵌贴图直接导出，不占位）
6. **#6 schema 兼容** ✅ **已收口，见 §8.1 #6**（`coffin_grade` optional + 默认 mundane）

## §8.1 决议（pre-P0 收口，2026-06-03）

### #2 灵材配方（已收口）

**决议**：
1. 三档配方 + 材料梯度照 `coffin-tier-materials` workflow 提案落地，**完整规格见 P4 表**。难度单调递进 common→uncommon→rare→very_rare、qi 0→2→4→6、time 90→120→150→180s。
2. **材料**：尽量复用现成（ling_mu_ban / xuan_iron / zhen_shi_zhong / zhen_shi_gao / ling_mu_jing / xue_po_lian）；`yu_sui`(玉髓)、`wu_yao`(乌曜石) 已在 `mineral/registry.rs:94-95` 注册仅补 toml；仅 `gu_tong_pian`(古铜片) 为全新 legendary 材料。
3. **主题锚点**：玉髓=寒玉「玉·温润冷灵」(矿物录「青白温润略似古玉·末法审美代表」+ `shelflife/compute.rs:101` 玉盒 0.5 倍率类比)；玄铁+阵石+乌曜石=玄石「玄·阵·镇魂」(矿物录乌曜石「漆黑如墨·欺天阵之核·久存聚阴」)；古铜片=青铜「上古·饕餮·镇魂」(末法去上古自洽：坍缩渊深层上古礼器碎块，兽纹犹存灵力已散，散修「借壳不借力」二次激活——锚 worldview §十六.三 + 「上古遗物无灵易碎」`npc/social.rs:340`)。
4. **门控（用户拍板）**：门控只加**制作**端——青铜棺 `realm_min: Some(Realm::Induce)`（首个 realm 硬门控配方先例）；jade/stone 仅 Scroll/Mentor 软门控；**进棺/使用任何棺无境界门控**。
5. **古铜片 loot（用户「都行」→双挂）**：坍缩渊深层 `ancient_relic` 池 ~3-5% + 暴龙王据穴古迹层 ~1-2%。
6. **配方卷轴 loot（用户「认可」）**：`scroll_jade_coffin`→漆棺(Rare) / `scroll_stone_coffin`→祭坛棺(Precious) / `scroll_bronze_coffin`→坍缩渊深层。
7. **雪魄莲（用户「同时实装」）**：用 `xue_po_lian`；其采集 EnvLock **已实装**（`botany/registry.rs:233 ENV_XUE_PO_LIAN = SnowSurface+QiVeinFlow{0.3}`，用于 `:1141`），直接复用、**无需新增**（`FrostBelt` 不存在），不走 `an_shen_guo` 替代。
8. **灵质守恒**：三档经 `workbench_recipes.rs` 守恒 pin test 估算通过（input spirit_quality × 0.95 ≥ output；新材料 spirit_quality_initial：yu_sui 0.8 / wu_yao 0.85 / gu_tong_pian 0.6）。

**落点**：`server/src/coffin/mod.rs:192`（`register_craft_recipes` 扩 3 配方）/ `server/assets/items/minerals.toml`（yu_sui/wu_yao/gu_tong_pian item 模板）/ `server/src/botany/registry.rs:233`（xue_po_lian EnvLock 已现成，复用）/ supply_coffin loot + tsy deep loot + dandao cavern loot（卷轴/古铜片挂点）/ plan §P4（规格表）。

### #3 破坏 / 回收 / 交互（已收口）

**决议**（用户拍板）：
1. **破坏 = 同破坏方块**：左键攻击棺 → despawn + **随机返还合成材料**。
2. 此返还做成**通用工具 fn** `craft::reclaim::recipe_reclaim_drops(recipe, mode)`：`Break` 随机部分 / `Reclaim` 较全；对所有 workbench 可放置物通用（不止棺材）。
3. **G 键 → 棺材菜单**（仿 `VoidActionScreen`）：**[入眠]**（进棺延寿 = `coffin_enter`）/ **[回收]**（较全返还 + despawn）/ 预留扩展。退役右键-CHEST-进棺。
4. 边界：破坏与回收返还量不同（破坏随机惩罚、回收较全奖励主动回收），具体概率/比例 P4 平衡时定。

**落点**：`server/src/coffin/mod.rs::handle_coffin_breaks` + 新增 `handle_coffin_menu_reclaim` / 新增 `server/src/craft/reclaim.rs` / `client/.../coffin/CoffinMenuScreen`（仿 `cultivation/voidaction/VoidActionScreen`）/ plan §P2 §P3。

### #4 mundane 迁移（已收口）

**决议**：**全迁**——4 档统一走 marker entity 管线，退役双 `BlockState::CHEST` 占位（用户拍板）。

**落点**：plan §P2（`handle_coffin_place_requests` `:218` 删 set_block CHEST `:318-321`）/ §P3（退役 Mixin 右键路径）。

### #5 棺材模型贴图（已收口）

**决议**（用户拍板）：**模型贴图已就绪于 bbmodel，直接导出接入，不做占位、不分两步**。P1 = 从 `local_models/*.bbmodel` 内嵌贴图直接导出 `textures/entity/{tier}_coffin_intact.png` + 导出 geo.json（`texture_width/height` = 导出 PNG 实际尺寸）。

**落点**：plan §P1 / 新增 `scripts/models/export_coffin_assets.py`（复用 `render_bbmodel.py` 的 bbmodel 解析：抽内嵌贴图 + 转 geckolib geo.json）。

### #1 raw_id 区间（已收口）

**决议**：grep `EntityKind::new` 实测——125-160 全占用（160 重复占用于 `boss_spawn.rs:617` + `visual.rs:29`），空闲从 **161** 起。4 档延寿棺用 **raw_id 161/162/163/164**（mundane/jade/stone/bronze）。client `BongEntityModelKind` 4 档 `expectedRawId()` 须 1:1 对齐。

**落点**：`server/src/world/entity_model.rs:34`（`EntityKind::new(161..164)`）/ `client/.../BongEntityModelKind.java`。
> 旁注：160 现被 boss_spawn 与 visual.rs 双重占用，疑似既存冲突——非本 plan 范围，仅避开。

### #6 schema 兼容（已收口）

**决议**：`CoffinStateV1.coffin_grade` 设为 **optional**，缺省按 `mundane` 解析——老 client / 旧落盘向后兼容。TS（`@Optional`）+ Rust serde（`#[serde(default)]` → Mundane）+ `coffin.test.ts` 加"无 grade 字段 → mundane"sample。
⚠️ 注：`server/src/schema/server_data.rs` 该 struct 带 `#[serde(deny_unknown_fields)]`——缺字段由 `serde(default)` 兜（OK），但**测试须覆盖"旧 payload 缺 coffin_grade → 默认 mundane"** 这条；新增字段本身不违反 deny（deny 拦的是多余未知字段）。

**落点**：`agent/packages/schema/src/server-data.ts:1132` / `server/src/schema/server_data.rs:78` / `coffin.test.ts`。

**§8 全部收口** —— 可升 active。

## §10 实施工作流

scope ≥ 4 PR，按 `docs/CLAUDE.md §六` 执行：
- **§10.1 视觉资产 3 轮 + PROMISE**：P1 的 geo/texture/animation 走 round 1/3 → 2/3 → 3/3，终轮 commit 写 `<PROMISE>`（拼写 PROMISE）。渲染核验用 `scripts/models/render_bbmodel.py`（真长相，非平涂示意）
- **§10.2 PR 序列化**（依赖序，前一个 merge 再开下一个）：
  1. **PR-1 P0**：schema + tier 底盘（纯 server 逻辑 + IPC 对拍）
  2. **PR-2 P1**：资产管线（geo/texture/animation，3 轮 + PROMISE）
  3. **PR-3 P2**：server marker 实体替换双 CHEST
  4. **PR-4 P3**：client 渲染 + 交互迁移 + HUD
  5. **PR-5 P4**：item/recipe + e2e + 平衡
- **§10.4 PR 用独立 subagent**（`subagent_type:"claude"`, `model:"opus"`, prompt 末 `ultrathink`，共享主 worktree）
- **§10.5 CodeRabbit ScheduleWakeup 等待**：每 PR `gh pr checks` pending → `ScheduleWakeup 1200s`，最多 3 回合；修完重等 re-review（memory `feedback_wait_coderabbit_approve`）
- **§10.N**：用户 `/consume-plan` 后全自动到 merge，醒来看是否归档 `finished_plans/`

---

## Finish Evidence

> 全阶段 ✅ + 本节填好后，`git mv` 入 `docs/finished_plans/`。（未完成，留空）
