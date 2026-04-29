# Bong · plan-mineral-v2 · 骨架

**矿物体系打磨与延伸**。`plan-mineral-v1` 主链路 M0–M6 已 ✅（2026-04-27 闭环）；本 plan 收拢 v1 范围外的 7 个独立子项：UX 反馈、采矿 gameplay、forge/alchemy 校验补完、shelflife 生产 profile、ClientResourcePack 推送、鲸落化石 structure。

**世界观锚点**：`worldview.md §六 经济层`（矿脉有限 / 货币层级）· `worldview.md §九 鲸落化石章`（白色巨型化石）· `worldview.md §四 距离衰减章`（镐头品阶分层意象）

**library 锚点**：`docs/library/ecology/矿物录.json`（v1 已落，正典 18 矿）· `docs/library/ecology/辛草试毒录.json`（dan_sha 解 Sharp 毒锚点）

**交叉引用**：
- `plan-mineral-v1`（前置；M0–M6 ✅，本 plan 不重造任何 v1 概念）
- `plan-forge-v1`（P2 炉阶 vs 主料品阶校验落地）
- `plan-alchemy-v1`（P3 dan_sha / zhu_sha / xiong_huang / xie_fen 配方）
- `plan-shelflife-v1`（P4 `ling_shi_*_v1` production profile 从 test-only → 正式注册）
- `plan-botany-v1`（P1 采矿 session 复用 `BotanyHarvestSession` 模式）
- `plan-worldgen-v3.x`（P6 鲸落化石 structure 接入）
- `plan-network-v1` / 待立 `plan-resourcepack-v1`（P5 Valence `ResourcePackPrompt` 接入）

**阶段总览**：
- P0 ⬜ UX 反馈链（chat 提示）
- P1 ⬜ 采矿 gameplay（镐头门槛 + session）
- P2 ⬜ forge 炉阶 vs 主料品阶 runtime 校验
- P3 🔄 alchemy 辅料配方实装（schema 层 `IngredientSpec.mineral_id` + `matches_mineral` 已就位，缺 4 份 JSON）
- P4 🔄 shelflife 灵石生产 profile 注册（四档 `ling_shi_*_v1` 已在 `build_default_registry` 生产注册，缺 runtime lookup 接线 + freshness 填充）
- P5 ⬜ ClientResourcePack 推送方案
- P6 ⬜ 鲸落化石 structure 算法

---

## §0 设计轴心

- [ ] **不引入 v1 没有的概念**：所有 P 都是 v1 已声明结构的"接线 / 实装"——不新增 mineral_id 域、不改 §1 矿物分类表、不变更 §4 vanilla block 映射
- [ ] **每个 P 独立可发**：7 个 P 之间无强依赖，可按优先级单独 PR；不强求一气呵成
- [ ] **优先级骨架**：P0/P1/P2 直接影响玩家体验（反馈 + 挖矿 + forge）；P3/P4 是数据 / 配方层补完；P5/P6 是延伸（可滑到 v3）
- [ ] **季节耦合**（worldview §十七）：`ling_shi_*_v1` 四档 shelflife profile 的 effective half_life 与 `Season::freshness_multiplier()` 联动（夏 ×1.5 / 冬 ×0.7 / 汐转 RNG ±30%）；本 plan P4 不重新实现修饰逻辑，仅复用 `plan-lingtian-process-v1` / `plan-shelflife-v1` 已暴露的 multiplier 通道——确保灵石的衰减节奏与作物 / 加工产物一致

---

## §1 P0 — UX 反馈链（v1 §2.2 极端情况）

> v1 §2.2 第 137 行：玩家拿到没 mineral_id NBT 的 vanilla item（打怪掉的 / creative 给的）→ alchemy/forge 拒绝 + chat "此为凡俗 X，不可入药/入炉"。**当前 forge `validate_with` 仅 log error，无玩家反馈**。

- [ ] **forge 反馈**：`server/src/forge/blueprint.rs:346` 校验失败时 emit chat event 给触发玩家（不仅 log）
- [ ] **alchemy 反馈**：`server/src/alchemy/recipe.rs` `matches_mineral` 返回 false 时同步 emit chat
- [ ] **chat payload**：复用现有 `bong:chat_message` 通道；`message_id` 域形如 `mineral.invalid_for_forge` / `mineral.invalid_for_alchemy` / `mineral.unknown_id`；中文模板："此为凡俗{vanilla_name}，不可入炉" / "此物未经矿录，无法入药"
- [ ] **测试**：每个 `message_id` 独立 pin 测试；feed 无 mineral_id 的 vanilla iron → 命中 `invalid_for_forge` chat sink

## §2 P1 — 采矿 gameplay（v1 §3 镐头门槛 + session）

> v1 §3 第 148 行（镐头品阶）+ 第 151 行（采矿 session 走 botany 同款）。

- [ ] **镐头品阶门槛**：`MineralRegistry` 类比 `forge_tier_min` 加 `pickaxe_tier_min: u8`；`break_handler.rs` 校验玩家手持 vanilla pickaxe `ToolMaterial::tier()` 与之比对，不达 → 拒绝挖掘 + chat
- [ ] **采矿 session**：复用 `plan-botany-v1` `BotanyHarvestSession` 模式，新增 `MiningSession { player, ore_pos, started_at_tick, ticks_total }` component；ticks_total 按 mineral tier（凡 20t / 灵 60t / 稀 120t / 遗 240t）
- [ ] **进度条通道**：`bong:mining_progress` 下发 0-100% 给客户端 HUD（HUD 渲染归 `plan-hud-*`）
- [ ] **取消条件**：玩家移动距离 > 1.5 格 / 切换物品 / 受伤 → session 中断、无 drop
- [ ] **测试**：tier 1 镐挖 tier 2 矿 → 拒 + chat；session 完成 → 触发 `BlockBreakEvent` 流转到 v1 已实装 drop 链；session 中断 → 无 drop；并发挖矿 session 互不干扰

## §3 P2 — forge 炉阶 vs 主料品阶 runtime 校验（v1 §5）

> v1 §5 第 210 行：`MineralId::forge_tier_min` 已是 data 层契约，**runtime 校验未接**。

- [ ] **校验点**：`ForgeBlueprint::validate_with` 增 `furnace.tier >= material.forge_tier_min` 校验（与 `is_valid_mineral_id` 并列）
- [ ] **`furnace_tier` 数据来源**：`server/assets/items/core.toml` 中 `furnace_fantie` / `furnace_lingtie` / `furnace_xitie` 各配 `tier: u8 = 1/2/3` 字段；炉物品 NBT 携带
- [ ] **失败行为**：emit `forge.tier_mismatch` chat："凡铁炉炼不动{material_name}，需升炉品至 {required_tier}"
- [ ] **测试**：凡铁炉（tier 1）接 ling_tie（tier 2）blueprint → 拒；灵铁炉接 sui_tie → 拒；稀铁炉接 sui_tie/can_tie/ku_jin → 通过；炉缺 tier 字段 → 视作 tier 1 + warn log

## §4 P3 — alchemy 辅料配方实装（v1 §6）

> v1 §6 第 217-220 行。schema 层 `IngredientSpec.mineral_id` v1 已就位（commit `a7050089`），**具体配方 JSON 未写**。

- [ ] **dan_sha 解毒丹**：`server/assets/alchemy/recipes/jie_du_dan_v1.json` — Mellow 辅料消费 dan_sha 解 Sharp 毒（接 `docs/library/ecology/辛草试毒录.json` 锚点）
- [ ] **zhu_sha 药引**：现有高阶丹方（如 peiyuan_dan / ningmai_dan）加 `auxiliary_materials[].mineral_id = "zhu_sha"` → 提升成丹率 + 附 Sharp 毒副作用
- [ ] **xiong_huang 驱邪辅料**（v2+ 半延后）：解蛊丹配方占位 JSON，依赖蛊毒系统立项
- [ ] **xie_fen 邪丹主料**（v2+ 延后）：需负灵域 / 魔修支线先立项，本 P 仅占位 §10 开放问题
- [ ] **测试**：consume 含 dan_sha 的 ingredient → 解毒 effect 修饰命中；feed 错矿（dan_sha 位填 ling_tie）→ recipe 拒 + chat

## §5 P4 — shelflife 灵石生产 profile 注册

> v1 `§-1 前置依赖` 第 23 行。四档 `ling_shi_*_v1` 已于 `shelflife::build_default_registry()` 注册（`shelflife/mod.rs:55` 生产挂载），**runtime lookup 接线 + freshness 实体填充待做**。

- [ ] **目标**：`shelflife/registry.rs` `register_production_profiles()` hardcode 四档 `ling_shi_fan_v1 / zhong_v1 / shang_v1 / yi_v1`，参数对齐 v1 §1.4 表（Exponential，half_life 3/5/7/14 days，禁止 Freeze）
- [ ] **lookup 链路**：`mineral/registry.rs:99-134` 已绑定四档 profile name；本 P 让 production `DecayProfileRegistry::get(name)` 在运行时命中（当前因未注册返回 None）
- [ ] **freshness 实体填充**：mineral drop → `inventory_grant` 写 NBT 时 lookup profile，命中后挂 `Freshness { profile_name, born_at_tick, initial_qi }`
- [ ] **测试**：`DecayProfileRegistry.get("ling_shi_fan_v1")` 返回 Some + Exponential + half_life ≈ 3 days；四档 ladder 单测；drop ling_shi_zhong → 实体 freshness profile_name == "ling_shi_zhong_v1"

## §6 P5 — ClientResourcePack 推送方案（v1 §4.3）

> v1 §4.3。14 张 ore 贴图已在 `client/src/main/resources/assets/minecraft/textures/block/` 落地（commit `f537f808`），**推送链路未实装**——玩家进服仍看 vanilla ore。

- [ ] **决策**：选 **方案 A · Valence `ResourcePackPrompt`**（推荐），方案 B（mod 内置可选）作 fallback；决策记录写本 plan §11
- [ ] **方案 A 接入**：`server/src/network/connection.rs`（或新 `resourcepack.rs`）玩家 join hook 下发 pack URL + sha1；强制接受 / 拒绝行为见下条
- [ ] **拒绝行为**：v2 选"降级"（玩家看 vanilla 贴图但仍可游戏；tooltip 仍走 mineral_id）而非 kick——选项放 §10 开放问题最终决策
- [ ] **pack 托管**：仓库 `client/resourcepack/bong-mineral-v1.zip`（CI 构建脚本 `scripts/build-resourcepack.sh`）+ 静态 host（GitHub Release 或内网 NGINX，由 deploy plan 协调）
- [ ] **测试**：mock client 进 join 流 → 收到 `ResourcePackPrompt` event 含正确 URL + sha1；sha1 mismatch 测试；client 拒绝 pack → server 记录但不 kick

## §7 P6 — 鲸落化石 structure 算法（v1 §2.1 + §9）

> v1 §2.1 第 121 行（鲸落化石映射 `sui_tie + ling_jing + yu_sui + ling_shi_shang/yi`）+ §9 第 256 行（生成算法待定）。

- [ ] **算法选型**：（a）独立 structure generator vs （b）借 vanilla `ancient_city` 变体，决策写本 plan §11
- [ ] **位置**：`worldgen/scripts/terrain_gen/structures/whale_fossil.py` — blueprint 驱动，固定 N 个 zone 候选位（鲸落遗骸 zone）
- [ ] **形态**：白色巨型化石（worldview §九 锚定）—— 几何核心 + 椎骨 / 肋骨布局；中心区域挂 mineral_anchor 富集 `sui_tie / ling_jing / yu_sui / ling_shi_shang or yi`
- [ ] **raster channel**：raster_export 加 `fossil_bbox` 通道（uint8 mask 或 AABB list）；server 启动期 `spawn_mineral_anchor_nodes` 等价 system 读 fossil bbox 写 mineral 富集
- [ ] **测试**：raster 生成后 fossil bbox 候选数 ≥ N；spawn 后矿物种类核心-外围分布正确（中心 sui_tie / 外围 yu_sui）

---

## §8 数据契约（按 P 汇总，下游 grep 抓手）

| P | 契约 | 位置 |
|---|------|------|
| P0 | `bong:chat_message` `message_id` 域新增 `mineral.invalid_for_*` / `mineral.unknown_id` | 现有 chat 通道 |
| P1 | `MineralRegistry::pickaxe_tier_min: u8` | `server/src/mineral/registry.rs` |
| P1 | `MiningSession` component + `bong:mining_progress` 通道 | `server/src/mineral/session.rs`（新增） |
| P2 | `furnace_tier: u8` | `server/assets/items/core.toml furnace_*` |
| P3 | 4 份 alchemy recipe JSON | `server/assets/alchemy/recipes/` |
| P4 | `register_production_profiles` 注册 `ling_shi_*_v1` 四档 | `server/src/shelflife/registry.rs` |
| P5 | `ResourcePackPrompt` 接线 + pack zip + sha1 校验 | `server/src/network/connection.rs` + `client/resourcepack/` |
| P6 | `whale_fossil.py` + raster `fossil_bbox` channel + spawn 接入 | `worldgen/scripts/terrain_gen/structures/` + `server/src/mineral/anchors.rs` |

## §9 实施节点

- [ ] **P0** UX 反馈链 — `mineral.invalid_for_*` chat emit + pin 测试
- [ ] **P1** 采矿 gameplay — `pickaxe_tier_min` 字段 + `MiningSession` + 进度条通道 + tier 校验
- [ ] **P2** forge 炉阶校验 — `furnace_tier` 字段 + `ForgeBlueprint::validate_with` 校验扩展
- [ ] **P3** alchemy 配方 — 4 份 JSON 落地 + `IngredientSpec.mineral_id` 实战校验
- [ ] **P4** shelflife profile — `register_production_profiles` 注册四档 + freshness 填充链路
- [ ] **P5** ClientResourcePack — 方案 A 接入 + sha1 验证 + pack zip CI 脚本
- [ ] **P6** 鲸落化石 structure — `whale_fossil.py` + raster fossil channel + mineral_anchor 接入

## §10 开放问题

- [ ] 镐头 vs 神识感知优先级：玩家无凝脉但镐头够品 → 能挖但 tooltip 看不到正典名？
- [ ] 凡铁镐挖灵铁矿是否给"挖崩了"小概率：失去 1 块镐头耐久 + 0 drop（增加凡→灵阶差挫败感）
- [ ] alchemy `xiong_huang` / `xie_fen` 归 v2 还是滑到 v3：负灵域 / 魔修支线未立项
- [ ] ClientResourcePack 拒绝玩家：kick 还是降级到 vanilla 贴图（v2 默认降级，最终决策待 §11）
- [ ] 鲸落化石生成节奏与 `plan-worldgen-v3.x` structure 系统的边界（独立 plan vs 沿用其 structure registry）
- [ ] CustomModelData 方案是否进 v2 让同 block 跨 biome 切贴图（v1 §4.3 末行延后项）
- [ ] 采矿 session 的真元消耗：v1 §3 默认不扣真元，但 tier 3+ 矿是否引入 cost 增加博弈

## §11 进度日志

- **2026-04-28**：实地核验修正 — P3 `IngredientSpec.mineral_id` / P4 `ling_shi_*_v1` 四档已在 v1 先行落地，阶段状态从 ⬜ 更正为 🔄；修正 alchemy recipe 资产路径 `recipes/alchemy/` → `alchemy/recipes/`。
- **2026-04-27**：骨架立项 — 承接 plan-mineral-v1 主链路收尾后的 7 项遗留（commit `c331850e` 之后剩余）。等优先级排序 + `/consume-plan mineral-v2` 升 active。
- **2026-04-29**：实地核验 + 升 active 准备。
  - 前置 plan 状态：`plan-mineral-v1` ✅（finished_plans）；`plan-forge-v1` ✅（finished_plans）；`plan-alchemy-v1` ✅（finished_plans）；`plan-shelflife-v1` active；`plan-botany-v1` ✅（finished_plans）；`plan-terrain-layer-query-v1` active（**新立**——为 P6 鲸落化石 raster fossil_bbox 通道预备共享 layer 查询接口）。
  - 代码侧核验：P3 `IngredientSpec.mineral_id` ✅（commit `a7050089`）；P4 `ling_shi_*_v1` 四档已在 `build_default_registry()` ✅；P5 14 张 ore 贴图 ✅（commit `f537f808`）—— 三档 schema/资产层就绪，**runtime 接线全缺**。
  - **同期升 active 的兄弟 plan**：`plan-lingtian-weather-v1` / `plan-lingtian-process-v1` —— 三者共享 worldview §十七 二季 + game-tick 驱动语义，本 plan §0 已加季节耦合一条作前置约定。
  - **决策推迟到 §10 开放问题**：P5 ResourcePack 拒绝行为（kick vs 降级）+ P6 鲸落化石与 plan-worldgen-v3.x 的 structure registry 边界——不阻塞 P0–P4 启动。
  - 补 `## Finish Evidence` 占位（active plan 范式）。准备 `git mv` 进 docs/ active。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：forge / alchemy chat emit 接入点（`forge/blueprint.rs:346` + `alchemy/recipe.rs`）+ 4 个 `message_id` pin 测试
  - P1：`MineralRegistry::pickaxe_tier_min` + `MiningSession` Component + `bong:mining_progress` 通道 + tier 校验
  - P2：`furnace_tier` 字段在 `core.toml` + `ForgeBlueprint::validate_with` 扩展
  - P3：4 份 alchemy recipe JSON（jie_du_dan / zhu_sha 增强 / xiong_huang 占位 / xie_fen 占位）
  - P4：`register_production_profiles` 注册四档 + drop → freshness 实体填充链路
  - P5：`ResourcePackPrompt` 接入 + pack zip CI 脚本 + sha1 校验（如启用）
  - P6：`whale_fossil.py` + raster `fossil_bbox` channel + `mineral_anchor` 接入（如启用）
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - server：`MineralRegistry.pickaxe_tier_min` / `MiningSession` / `register_production_profiles` / `ForgeBlueprint::validate_with` 扩展
  - agent：mineral chat message_id 模板 / processing 联动 schema
  - client：ResourcePack 接受 / 拒绝行为；HUD mining progress
  - worldgen：`fossil_bbox` raster channel（如 P6 落地）
- 遗留 / 后续：
  - `xiong_huang` / `xie_fen` 配方（依负灵域 / 魔修支线 plan，可能滑 v3）
  - CustomModelData 跨 biome 切贴图（v1 §4.3 末行延后项）
  - 采矿 session 真元消耗（§10 开放问题）
