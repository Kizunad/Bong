# plan-lootcrate-v1 — 末法 LootCrate：五变种世界散布战利品箱

> **一句话主题**：新增 5 变种世界散布 LootCrate（骨扎皮箱/符封遗匣/锈铁行军箱/藤蚀腐木箱/残灰陶瓮，bbmodel 资产已产出），**镜像供应棺全链路**（refresh tick 自动散布 + `ExternalContainer` 会话开箱 + 超时碎裂进冷却重刷），零新 proto——把"搜打撤捡箱子"体验从剑冢单 zone 扩展到全世界，按 zone 危险度分变种分品质。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。
**资产**：5 个 bbmodel 生成器已落地 `scripts/models/gen_loot_crates.py`（本 PR 附带，3 轮打磨 + 真渲染核验），`local_models/LootCrate*.bbmodel` 供 Blockbench 手调，渲染总览 `scripts/models/render_loot_crates_all.png`。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 底盘——EntityKind 169~173 + lootcrate 模块（refresh/interact/lifecycle）| ⬜ |
| P1 | loot 表——5 变种独立 loot pool（loot_pools.json 数据驱动，权重概率 roll）+ 分布配置 | ⬜ |
| P2 | client 渲染——BongEntityModelKind 5 注册 + renderer + 资源包 sha1 | ⬜ |
| P3 | 视听 + 平衡——出土/开箱/碎裂 AV、密度与冷却校准 | ⬜ |
| P4 | Loot UI 美化——LootContainerScreen 视觉升级（变种主题皮肤/品质边框/揭示动效）| ⬜ |

---

## 背景与调研结论（2026-07-03）

仓库已有两条战利品容器链路（Explore 实证，均活）：

- **供应棺 supply_coffin**（`server/src/supply_coffin/`，finished plan ×2）：`SupplyCoffinRegistry`（档位 max_active/cooldown，`mod.rs:38-233`）→ `refresh.rs:45-148` 每 tick 补位 spawn（zone AABB 随机 xz + `TerrainProvider::query_surface` 地表吸附 + 20 次重试拒水面/遮挡/间距<10）→ 玩家开箱 C2S `SupplyCoffinOpenReq`(87) → `interact.rs:70-280` roll loot 挂 `ExternalContainer` → S2C `LootContainerOpen`(119) → `lifecycle.rs` 超时碎裂进冷却 FIFO 重刷。**但只在 `giant_sword_sea` 一个 zone**。
- **tsy 容器搜刮**：`StartSearch`(82) 进度条式，`loot_pools.json` 数据驱动（`world/loot_pool.rs`），SurfaceStash 散布（`poi_novice.rs:491 scatter_surface_stashes`，Poisson 排斥采样）。

**留白**：全世界通用的、按 zone 分变种/分品质的散布 lootcrate 不存在；supply_coffin loot 表是硬编 Rust。本 plan 填这块，并把 loot 定义迁到数据文件。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`ExternalContainerRegistry`/`ExternalContainer`（容器抽象，与 supply_coffin/placeable-containers 共用）；`TerrainProvider::query_surface`（地表吸附）；`ZoneRegistry`（分布配置按 zone）；`loot_pools.json` + `LootPoolRegistry`（`world/loot_pool.rs:21-94`）；`scripts/models/export_container_assets.py`（bbmodel→client geo/texture 导出管线）。
- **出料**：loot 进 `PlayerInventory`（复用 `pack_loot_into_grid`）；开箱/碎裂 emit 既有音效/粒子事件；`SupplyCoffinOpened` 类事件供天道 narration（可选）。
- **共享类型 / event**：`ExternalContainerKind` 加 `LootCrate { variant: LootCrateVariant }`（不复用 SupplyCoffin 变体——lifecycle 分支语义不同档）；**C2S 复用 `SupplyCoffinOpenReq`(87) 或泛化前缀**（见 §8 #1）；S2C 复用 `LootContainerOpen`(119)/`LootContainerClose`(121)。**零新 proto oneof**。
- **跨仓库契约**：wire 零改动；client 仅新增实体渲染注册（EntityKind **169~173**，紧接 DEAD_DROP_BOX=168）+ 资源包资产。`entity_model.rs:655` 的 server↔client raw_id 契约测试同步扩展。
- **worldview 锚点**：§一 搜打撤/匮乏基调（「搜打撤、苟」）；五变种视觉语言各有正典出处——骨（§九 骨币文化）、符封（宗门废墟遗产）、锈铁（末法军旅残留）、藤蚀（自然回收）、陶瓮（民生窖藏）。
- **qi_physics 锚点**：无灵气流动——loot 是物品实例迁移，不产不销 qi。crate 内物品若带 spirit_quality 走既有 shelflife/excretion 规则，本 plan 零新常数。

## P0 server 底盘 ⬜

- `world/entity_model.rs`：`EntityKind` 常量 169~173（`LOOT_CRATE_BONE_LASH` 等）+ `BongVisualKind` 5 变种 + `entity_kind()` match + 契约测试期望数组扩展（`:687`）。
- 新模块 `server/src/lootcrate/`（镜像 supply_coffin 结构）：`LootCrateVariant` enum、`LootCrateRegistry`（per-variant per-zone-tier 的 max_active/cooldown/间距）、`refresh.rs`（spawn director，选点复用 `pick_valid_pos` 模式但 zone 集合来自分布配置而非单 zone）、`interact.rs`（距离≤4 校验/占用锁/roll→`ExternalContainer`）、`lifecycle.rs`（超时碎裂/离范围关箱/掉线释放锁——复用 supply_coffin 语义）。`main.rs` 注册。
- despawn 一律 `insert(Despawned)`。
- **测试**：镜像 supply_coffin 测试族——补位/冷却/间距/水面拒绝/占用互斥/超时碎裂/重刷；raw_id 对齐契约测试。

## P1 loot 表（数据驱动，用户定调 2026-07-03：每变种必须不同 + 概率刷法参照供应棺）⬜

- `loot_pools.json` 加 5 个**互不相同**的 pool：`lootcrate_bone_lash`（拾荒基础：材料/凡器/骨片）、`lootcrate_talisman`（宗门遗产：残卷/符纸/丹药，低概率功法）、`lootcrate_rust_trunk`（军旅：武器部件/护甲件/工具）、`lootcrate_vine_chest`（野外：植物/种子/腐坏食物——接 shelflife）、`lootcrate_ash_urn`（窖藏：陈酒/骨币小额/灵材）。
- **roll 语义对齐供应棺**（`supply_coffin/loot.rs:181-226`）：按权重**不重复**抽 `roll_count_range` 种、每种 min~max 数量、deterministic splitmix64 种子——落在 `roll_loot_pool`（`loot_pool.rs:94`）上如缺此语义则扩该函数（加 no-repeat + count range，供应棺后续迁移也能用）。启动校验 template 引用。
- **刷新完整性**：碎裂→冷却→refresh 补位重刷（P0 lifecycle）保证物资持续供给；每次重刷重新 roll（不是复位旧内容）。
- **分布配置**：变种×zone 危险度映射（低危出 vine/bone_lash，高危出 talisman/rust_trunk，遗迹 zone 出 ash_urn/talisman）——具体表 §8 #2 收口；danger 数据与 `plan-ambient-threat-v1` 同源。
- **测试**：pool 引用校验；roll 分布 pin；变种→zone 映射专属 case。

## P2 client 渲染 ⬜

- `BongEntityModelKind.java` 追加 5 enum（raw_id 169~173，textureState `intact`）+ 5 个 renderer + `BongEntityRenderBootstrap` deferred 注册。
- 资产管线：`export_container_assets.py` 的 CONTAINERS 元组加 5 项 → `assets/bong/{geo,textures/entity}`；**重打包资源包 zip + 同步 `resourcepack.rs` sha1/size**（CI 红线）。
- bbmodel 源以 `local_models/LootCrate*.bbmodel`（用户 Blockbench 手调后）为准，勿重跑生成器覆盖手调稿。
- **测试**：raw_id 对齐（双端契约测试）；资源包构建 CI。

## P3 视听 + 平衡 ⬜

- **出土**（refresh spawn 时）：复用供应棺 emerge 音效/粒子模式，按变种换贴合 SFX——骨扎 `entity.skeleton.ambient` pitch 0.6 vol 0.5、陶瓮 `block.decorated_pot.step` pitch 0.8、锈铁 `block.anvil.land` pitch 1.4 vol 0.35、余两种 `block.rooted_dirt.break` pitch 0.9；粒子统一复用棺出土尘土 burst（`BongGroundDecalParticle` 现有事件），色按变种主材（骨白 `#CEC4AC`/漆红 `#602822`/锈灰 `#7E8086`/腐木 `#706444`/陶灰 `#948A7C`）。
- **开箱**：`LootContainerOpen` 已有开箱 UI；补 lid 骨骼开盖动画（bbmodel 已带 lid/seal 独立骨骼 + 铰链 pivot，animation.json 由 export 管线产）+ 变种开盖 SFX（木 `block.chest.open`、铁 `block.iron_door.open` pitch 1.3、瓮 `block.decorated_pot.insert`）。
- **碎裂**：复用供应棺 break 链。
- **平衡**：全世界 max_active 总量、per-zone 密度、冷却时长、与 supply_coffin/SurfaceStash 的密度叠加预算——§8 #3 校准。
- **测试**：VFX 事件注册防孤岛（`VfxBootstrap` 核对）；SFX recipe JSON 校验。

## P4 Loot UI 美化（用户定调 2026-07-03）⬜

现状：`LootContainerScreen`/`LootContainerPanel`（`loot_container_open`(119) payload → Store → Bootstrap 打开，供应棺/放置容器/lootcrate 共用一屏）为功能性素屏。升级为主题化开箱体验，**惠及所有 ExternalContainer 来源**：

- **变种主题皮肤**：payload 已带 source_kind/variant → 屏按来源换主题（标题文案 + 顶栏配色 + 边框风格五套：骨白/漆红/锈灰/腐木绿/陶灰，与 bbmodel 主材呼应）；供应棺/放置容器各给默认主题。皮肤资产倾向程序化 nine-patch + 现有 GUI 纹理复用（不足则 gen-image 批产，走资源包 sha1 同步）。
- **品质边框**：loot 格子按物品 rarity 上色描边（复用 InspectScreen 既有 rarity 色映射），hover tooltip 沿用现有 ItemTooltipPanel。
- **揭示动效**：开箱时格子 stagger 逐个揭示（每格延迟 2~3 tick，淡入+微缩放），配翻捡 SFX tick；「全部拿取」按钮（一键收包，背包满则部分收+toast）。
- **owo 已知坑**：viewport 高度 `Sizing.fixed`（fill 顶飞坑）；内容刷新 diff 原地更新不 `clearChildren`（滚动回弹坑）。
- **测试**：主题按 source_kind 分派全 variant pin；rarity 边框映射；全部拿取的背包满/部分收边界；揭示动效不阻塞交互（动效中可点击）。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **C2S 复用形态**：直接复用 `SupplyCoffinOpenReq`(87)（handler 按目标实体 kind 分派，语义名不贴）vs client 前缀泛化（`lootcrate:<entityId>` 复制 `supply_coffin:` 命中逻辑）——倾向后者复制 intent handler、C2S 仍走 87 号带 kind 区分，实地核对 87 payload 是否含足够信息。
2. **变种×zone 分布表**：五变种落在哪些 zone/danger 档、各档 max_active/cooldown 数值；遗迹类 zone（jiuzong_*_ruin）是否给 talisman 专属加权。
3. **密度总预算**：与 supply_coffin（剑冢）、SurfaceStash（新手圈）、后续 ambient-threat 实体的世界实体峰值合账。
4. **符封遗匣的门槛**：talisman 变种是否要"撕符"前置（消耗动作/境界门槛/符纸反噬小惩罚）作为高价值箱的开箱代价——正典上封条是有主之物的封印。
5. **supply_coffin loot 硬编迁移**：顺手把 supply_coffin 三档 loot 迁 `loot_pools.json`（统一数据驱动）还是留原样——倾向留原样，本 plan 不动它（防 scope 蔓延），只登记后续待办。
6. **天道叙事**：开高价值箱是否 emit 事件进天道 narration 信号（低优先）。
7. **P4 皮肤资产形态**：程序化 nine-patch（零新贴图）够不够五套主题的质感，还是 gen-image 批产 GUI 纹理（进资源包，吃 sha1 同步）；`loot_container_open` payload 是否已带足 source_kind/variant 字段（不足则 server 补字段，wire 变更连 samples/.proto 一起改）。

## §10（升 active 时补）

scope 预估 5 PR（P0~P4 各一）。资产生成器与渲染总览已随本 skeleton PR 落地（3 轮打磨完成）；P2 的 Blockbench 手调稿与资源包 sha1、P4 的 GUI 皮肤属实施期交付物。
