# plan-craft-chain-items-v1 — 合成链物品扩展：矛族长兵 + 药线/食线延伸（手搓优先，效果+模型全到位）

> **一句话主题**：用"现有可合成物品作材料"的二级合成链新增 5-6 件物品——三档矛族（粗石矛/骨尖矛/锈铁矛，填 `WeaponKind::Spear` 整系空档）+ 消耗品延伸（药浸绷带/盐渍肉干）——**全部复用现有 ItemEffect / WeaponSpec / craft 链路，零新系统零新枚举变体**，手搓优先（5 件中 4 件 station: None），每件效果真接线、图标 + 3D 手持模型（bbmodel 分部件三轮打磨 → OBJ）全到位。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §8。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | server 数据落地——物品 TOML（矛族 WeaponSpec + 消耗品 effect）+ 配方 + 饱和测试 | ⬜ |
| P1 | 效果/战斗链路 e2e + bot 场景（矛命中伤害/双手占用/绷带治疗/肉干 buff） | ⬜ |
| P2 | 资产——bbmodel 分部件生成器 + OBJ + `BongWeaponModelRegistry` + 图标批产 + 资源包 sha1 | ⬜ |
| P3 | 平衡与经济锚——骨币定价 / loot pool 收录 / 数值对拍表 | ⬜ |

---

## 背景与调研结论（2026-07-18 实证）

- **spear 是整系空档**：`WeaponKind` 七系（`inventory/mod.rs:2348` sword/saber/staff/fist/spear/dagger/bow）中 spear 无任何落地物品（`assets/items/` 全 grep 零命中）；而双手占用判定已就绪（`mod.rs:759-763` `weapon_two_handed` 含 Spear，plan-layered-equip-v1 决议 #7）、TOML 解析已就绪（`mod.rs:2947` `"spear" => WeaponKind::Spear`）——矛族是"数据到位即玩"的现成缺口。bow 涉远程弹道无现成链路，本 plan 不做（§8 #4）。
- **消耗品医疗线已密，必须错位**：bandage（wound_heal 1.0 全身）/ 夹板（2.0 定向）/ meridian_salve / anti_gu_powder / qingzhuo_powder（contamination_cleanse 0.4）/ calming_tea（composure_restore 0.35）/ qi_guide_talisman（food_regen 0.30）——`workbench_materials.toml:347-470`。新消耗品只做**链条升级档**（吃现有产物作材料），不开新效果轴。
- **链条地基全是现有可合成物**：wood_handle（`workbench_recipes.rs:222`，产 4）、grass_rope（`:247`）、stone_knife（`HANDCRAFT_STONE_TOOLS` 手搓，`:21`）、spider_silk_cord / salt_crystal / rat_tail_oil / herb_bundle（workbench 加工产物）、bone_spike / iron_ingot（`materials.toml`）。
- **效果 = 纯数据**：TOML `effect = { kind = "wound_heal", ... }` 直映射 `ItemEffect`（`inventory/mod.rs:397`），消耗链路现成；WeaponSpec 同为 TOML 字段。本 plan 的"效果到位"= 复用现集合真接线，**发现必须新增 ItemEffect 变体即停下按 §8 裁决**（红线目标：零新变体）。
- **模型链路已有范本**：手持物 = `scripts/models/gen_*.py` 分部件 bbmodel（范本 `gen_wooden_shield.py`：part 函数拆件 + preview 渲染 + `local_models/*.bbmodel` Blockbench 手调源）→ 导出 OBJ → `client/assets/bong/models/item/<id>/<id>.obj` + `BongWeaponModelRegistry` 条目（`weapon/BongWeaponModelRegistry.java:21` Entry 四元组）；图标 = `/gen-image item` → `gui/items/<id>.png`（`ItemIconRegistry` 按 id 约定自动解析，零 Java 注册）。

## 物品清单草案（§8 #1 终审）

### 族 A：矛族（category Weapon，kind spear，全档 3D 手持模型）

| id | 名 | 档 | 配方（材料全为现有物品，⭐=现有 craft 产物） | 台 |
|----|----|----|----|----|
| `crude_stone_spear` | 粗石矛 | 凡器 | ⭐stone_knife×1 + ⭐wood_handle×2 + ⭐grass_rope×1 | 手搓 |
| `bone_tip_spear` | 骨尖矛 | 凡器+ | bone_spike×2 + ⭐wood_handle×2 + ⭐spider_silk_cord×1 | 手搓 |
| `rusted_iron_spear` | 锈铁矛 | 铁器 | iron_ingot×2 + ⭐wood_handle×2 + ⭐spider_silk_cord×1 | 制作台 |

- `weapon.kind = "spear"`，双手占用走现有判定；damage 基准对齐同档 sword ×0.9~0.95（换取长兵语义，具体数值 §8 #2）；耐久对齐同档现有武器。
- 命名走 §三 L63 素朴意象（粗/骨/锈），全禁词避开。

### 族 B：消耗品链条升级档（手搓）

| id | 名 | 配方 | 效果（错位现有档） | 台 |
|----|----|----|----|----|
| `medicated_bandage` | 药浸绷带 | ⭐bandage×2 + ⭐rat_tail_oil×1 + ⭐herb_bundle×1 → 出 2 | wound_heal 2.0 **无定向**（错位：bandage 1.0 全身 / 夹板 2.0 定向 / 本品 2.0 全身但材料贵） | 手搓 |
| `salted_jerky` | 盐渍肉干 | 生肉类掉落×2 + ⭐salt_crystal×1（生肉 id §8 #3 实证） | category food，food_regen 0.10 / 24000 ticks（错位 qi_guide_talisman 0.30/36000 的低档口粮位） | 手搓 |

**候选裁决项**（§8 #8，默认砍）：磨石（耐久修理耗材）——需新 ItemEffect 变体 + handler，越"零新系统"红线。

## 各件视听规格（内联，docs/CLAUDE.md §四 精度要求）

| 物品 | SFX（audio_recipe JSON） | 粒子 | HUD |
|------|--------------------------|------|-----|
| 矛族挥动 | 新 recipe `spear_swing.json`：layer1 `minecraft:entity.player.attack.sweep` pitch 0.72 vol 0.5；layer2 `minecraft:item.trident.hit_ground` pitch 1.35 vol 0.25 delay 2t | 复用现有武器挥动轨迹粒子链（内容无关，零新 sprite） | 现有命中浮字/耐久条 |
| 矛族命中 | 复用现有武器命中链（#1069 已修的挥动/命中 SFX 通路），重击追加 layer `minecraft:entity.player.attack.strong` pitch 0.9 vol 0.4 | 同上 | 同上 |
| 药浸绷带使用 | 新 recipe `medicated_bandage_use.json`：layer1 `minecraft:item.armor.equip_leather` pitch 0.85 vol 0.6；layer2 `minecraft:block.slime_block.place` pitch 1.4 vol 0.2 delay 3t | 复用现有治疗类 VfxPlayer（若无可复用者：`BongSpriteParticle` 8 粒 `#D8CBA8` lifetime 14t burst，`VfxBootstrap` 一行绑现有 player） | 现有 wound HUD（剪影红点消退） |
| 盐渍肉干 | 走 Food 消费现有 eat 链（`entity.generic.eat`），零新资产 | 零新 | 现有 buff 挂载提示 |

三矛必须**远观可辨**：石矛灰白粗石头、骨矛微弯骨尖 + 暗色缠扎、铁矛锈红宽刃——差异在 bbmodel 部件层做出来（P2），不靠贴图微调。

## 接入面（docs/CLAUDE.md §二 checklist）

- **进料**：`materials.toml` / `workbench_materials.toml` 现有材料与产物；`CraftRegistry`（配方注册）；`ItemRegistry` TOML 扫盘（`inventory/mod.rs:1634`）；fauna 掉落（生肉，§8 #3）；`/gen-image item` + bbmodel 工具链（`scripts/models/`）。
- **出料**：物品入 `PlayerInventory`；矛入 combat 武器链（WeaponSpec 倍率 / 双手 / 耐久 / 修理站）；消耗品走 `ItemEffect` 消费链（wound / food_regen 各自现有 system）；肉干入 shelflife 现有食物保质规则。
- **共享类型 / event**：**零新枚举变体为红线目标**——ItemCategory（Weapon/Misc/Food）、ItemEffect（wound_heal/food_regen）、WeaponKind（Spear）全在现集合内；发现必须新增 → 停下按 §8 裁决。
- **跨仓库契约**：client 仅 `BongWeaponModelRegistry` 3 条目 + 资源文件；零 proto/schema 改动（`inventory_snapshot` 按 item_id 字符串透传，新物品自动走通）。
- **worldview 锚点**：§三 L63 命名禁词（粗/骨/锈素朴意象）；§九 骨币经济定价（P3）；§一 匮乏基调——凡器长兵 = 无功法者的防身选择，对齐 wooden_shield（`gen_wooden_shield.py` 头注：凡人级物理防御）先例。
- **qi_physics 锚点**：**零 qi 流动**——矛为纯物理武器；消耗品效果均非 qi 类（wound/food_regen 走各自现有 system）。刻意避开 QiRecovery（该链路正被 plan-bughunt-qi-recovery-consumable-ledger-v1 修 ledger，不挂新增量）。零新常数。

## P0 server 数据落地 ⬜

- `assets/items/weapons.toml` 加矛族 3 条（weapon spec / durability / rarity / grid 尺寸——长兵 grid_h 建议 4~5 对齐 Tarkov 格惯例）；消耗品 2 条归属文件 §8 #5。
- 配方落点按 §8 #6 与 plan-registry-datafication-v1 协调：datafication P0 先 merge → 直接写 `assets/craft/recipes/` TOML；否则 → `workbench_recipes.rs` 表行（3 件手搓进 station None 语义、锈铁矛 Workbench）+ 该 plan 迁移时一并搬。
- 饱和测试（**以玩家可观察契约为主**）：① 5 件物品 craft/`/give` 后确实入包且 grid 占格正确；② 矛装备后攻击伤害吃 spear 倍率、双手契约生效（单一契约见 P1）；③ 消耗品使用后效果真实生效（wound 值变化 / buff 挂载）+ 错误分支行为固定（满血使用 / buff 叠加语义）；④ 材料不足 / 背包满的失败行为。辅助覆盖（非主要验收项）：模板加载 pin（数量/字段）、`weapon.kind` 解析命中 Spear、每 effect kind 反序列化正反 sample、配方材料引用存在性（含 ⭐链条物 id 全核验）。

## P1 效果/战斗链路 e2e + bot 场景 ⬜

- bot 场景（`scripts/bot/scenarios/`，CI bot e2e 硬约定）：① 手搓粗石矛 → 装备 → 攻击靶 NPC → 伤害断言吃武器倍率 + 双手**单一契约**两条分别断言：副手占用时装备**拒装并携带具体 reject reason**、成功装备后副手保持为空（语义实施前以现有 `weapon_two_handed_per_kind` 单测为准固定，不得写成模糊的"被清或拒装"）；② 合成药浸绷带（链条：先合 bandage 再升级）→ 受伤 → 使用 → wound 恢复断言；③ salted_jerky → food_regen buff 挂载断言（`CultivationAcceleration`）。
- server 集成测试：craft session 成功/失败分支**分别断言**——材料原子扣减（失败路径零扣减）、背包满回滚（材料完整退回）、成功产出入包（数量/位置）三条独立用例；三矛挥动 emit 的 audio/vfx 事件断言（防 AV 孤岛）。

## P2 资产（3 轮打磨 + PROMISE 纪律）⬜

- `scripts/models/gen_spear_family.py`：分部件 `part_head()` / `part_shaft()` / `part_binding()`（范本 `gen_wooden_shield.py`），三矛共骨架差异化部件；每件单独 preview PNG + `render_bbmodel.py` 真渲染核验；commit 按 `(round N/3)` + 终轮 `<PROMISE>` 块。
- 产物 `local_models/Spear*.bbmodel`（gitignored，Blockbench 手调源，勿重跑生成器覆盖手调稿）→ 导出 OBJ → `client/.../models/item/<id>/<id>.obj` + 贴图。
- `BongWeaponModelRegistry` 3 条目：host 选型必查 `vanillaModelPaths` 防撞 + 选 Bong 体系未占用的 vanilla item（候选 `Items.TRIDENT` 系；三矛不可共 host）；FPV+TPV 双入口核验（模型链路审计教训：双策略坑）。
- 图标：`/gen-image item` 批产 5 张（矛 3 + 绷带 + 肉干）→ `gui/items/<id>.png`；批产后程序化全量扫假透明并 `--force` 重生成；harness 跑不了 `/gen-image` 则占位 + `[BLOCKED: 需 /gen-image 生成 <清单>]` 标注，接线照做。
- 资源包：重打 zip + `resourcepack.rs` / `client/resourcepack/manifest.json` sha1+size **双处同步**（CI 红线）。
- 测试：图标资源存在性；OBJ 注册 pin（3 id 全命中，防"手持空手"回归）；资源包构建 CI。

## P3 平衡与经济锚 ⬜

- 骨币定价（worldview §九）：对齐同档现有物价签（石刀 / 铁镐 / bandage）；矛族 damage/耐久/攻速与 sword/dagger 档位对拍表格进 plan。
- 获取渠道 §8 #7：纯 craft，或同步收录 `loot_pools.json`（低危 zone pool 出粗石矛/绷带）/ NPC trade。
- 回归：数值改动全部走 TOML，测试断言取模板引用不写字面量。

---

## §8 开放问题（升 active / P0 决策门前收口）

1. **物品清单终审**：数量（5-6 件）/ 命名 / 是否追加第 6 件（候选：投掷标枪版矛——但涉 anqi 投掷链路复用度，需实证）。
2. **矛数值语义**：现 combat 是否吃 reach/攻距（grep 实证；若无 reach 概念则矛差异只体现在 damage/双手/攻速，不为矛新造 reach 系统）；damage multiplier / 攻速 / 耐久具体值。
3. **生肉 id 实证**：fauna 掉落表有无 raw meat 类物品；无则肉干改用现有 fauna 掉落物（rat_tail 系）或砍掉本件。
4. **bow 系**：确认不做（远程弹道无现成链路，违"零新系统"），是否登记 reminder 供将来立 plan。
5. **消耗品 TOML 归属**：`workbench_materials.toml` 追加 vs 新 `chain_items.toml`。
6. **与 plan-registry-datafication-v1 顺序**（该 plan §8 #7 对偶条目）：两 plan 不得同时改 `workbench_recipes.rs`；本 plan 后行则配方直接落 TOML。
7. **获取渠道**：纯 craft vs loot pool / NPC trade 同步收录（倾向 P3 收录低危 pool，强化"捡到半成品材料→手搓成器"的搜打撤循环）。
8. **磨石裁决**：默认砍（需新 ItemEffect 变体越红线）；若用户要，则单列小 P 并显式承认破例。

## §10（升 active 时补）

scope **固定 3 PR**（依赖序列化）：PR-1 = P0+P1（数据+链路+bot，纯逻辑）→ PR-2 = P2 资产（bbmodel 3 轮 + PROMISE + 资源包 sha1；`/gen-image` 不可用时图标以 `[BLOCKED]` 占位随同本 PR，不另拆）→ PR-3 = P3 平衡。升 active 时按 docs/CLAUDE.md §六 模板补全 §10 全文（含"单次 consume-plan 全自动到 merge"章节）后方可消费；升 active 前 §8 全收口（尤其 #1 清单终审 + #2/#3 两处实证）。**单 plan 边界与交接规则**：每个实施 PR 仅消费并修改本 plan（对齐「一个 PR 只动一个 plan」），不得在同一 PR 内交叉改动 plan-registry-datafication-v1 及其迁移产物；配方落点只取决于实施当刻 origin/main 现状——datafication P0 已 merge → 直接写 TOML，未 merge → 写 Rust 表行、后续迁移由对方 plan 自己的 PR 完成，本 plan 绝不代跑。
