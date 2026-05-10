# Plan: Item Visual v1（物品视觉体系）

> 项目有 scripts/images/gen.py 图像生成管线（4 档画风 item/particle/hud/scene，cliproxy+OpenAI fallback）和 21 张武器贴图 + 39 张植物图标。但**大量物品类型缺少图标**（骨币/兽骨/矿石/丹药/灵石/法器/残卷/工具/伪灵皮/阵法道具），且物品栏内**所有物品看起来一样**——没有稀有度视觉区分、没有品质发光、tooltip 样式单一。

---

## 接入面 Checklist（防孤岛）

- **进料**：`scripts/images/gen.py` ✅ / `server/assets/items/*.toml` ✅（全物品定义）/ `ItemRarity` enum ✅ / `spirit_quality` 字段 ✅ / `BotanyHudPlanner.PLANT_ICON_PATHS` ✅（现有图标注册表）
- **出料**：物品贴图 → `client/src/main/resources/assets/bong/textures/item/` + `bong-client/textures/gui/items/` / 稀有度渲染器 → `client/src/main/java/com/bong/client/inventory/` / tooltip 增强 → `ItemTooltipRenderer.java`
- **共享类型/event**：不新增 event。纯 client 渲染增强
- **跨仓库契约**：server 物品 metadata（rarity / spirit_quality / charges）已通过 inventory sync 下发 → client 按 metadata 选择渲染样式
- **worldview 锚点**：§九 经济（骨币/灵石）/ §十六 上古遗物（脆化/反光逆转）/ §七 生态（兽骨/矿石）

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 核心物品图标批量生成（骨币/兽骨/矿石/丹药/灵石/工具） | ⬜ |
| P1 | 稀有度边框/发光 + tooltip 样式 | ⬜ |
| P2 | 上古遗物特效 + 3D 掉落物区分 + 法器/残卷图标 | ⬜ |

---

## P0 — 核心物品图标批量生成 ⬜

### 交付物

1. **图标生成批次脚本**（`scripts/images/gen_item_batch.py`）
   - 读取 `server/assets/items/*.toml` 全物品列表，对每个缺少贴图的 item_id 调用 `gen.py --style item`
   - prompt 模板：`"{item_cn_name}，末法残土风格，暗色调水墨，透明背景，64×64 icon"`
   - 输出到 `client/src/main/resources/assets/bong-client/textures/gui/items/{item_id}.png`

2. **第一批图标（~40 张）**
   - 骨币系列：`bone_coin_5` / `bone_coin_15` / `bone_coin_40`（三档大小的封灵骨币）
   - 兽骨系列：`shu_gu`(鼠骨) / `zhu_gu`(蛛骨) / `feng_he_gu`(缝合骨) / `yi_shou_gu`(异兽骨) / `jing_gu`(鲸骨) / `jing_sui`(鲸髓) / `jing_hun_yu`(鲸魂玉)
   - 矿石系列：`iron_ore` / `copper_ore` / `ling_shi_low`(低阶灵石) / `ling_shi_mid`(中阶灵石) / `ling_shi_high`(高阶灵石) / `xuan_tie`(玄铁)
   - 丹药系列：`kai_mai_dan`(开脉丹) / `ning_mai_san`(凝脉散) / `gu_yuan_dan`(固元丹) / 通用丹药瓶 × 3 档
   - 工具系列：`hoe_iron` / `hoe_lingtie` / `hoe_xuantie` / `knife` / `sickle` / `scraper`
   - 杂项：`beast_core`(异变兽核) / `fu_ya_hesui`(负压碎) / `zhen_shi_chu`(阵石碎)

3. **图标注册表更新**
   - 新增 `ItemIconRegistry.java`：item_id → texture path 映射，统一管理（替代分散在各 HudPlanner 中的 hardcode）
   - 所有现有 icon 引用迁移到统一注册表

### 验收抓手

- 测试：`scripts/images/test_gen_item_batch.py` 验证 toml 解析 + prompt 生成
- 手动：打开背包 → 每个物品有专属图标（不再是 vanilla 石头/木棍占位）

---

## P1 — 稀有度边框/发光 + tooltip ⬜

### 交付物

1. **稀有度边框渲染**（`client/src/main/java/com/bong/client/inventory/RarityBorderRenderer.java`）
   - 6 档颜色：Common=#808080 / Uncommon=#22CC22 / Rare=#2288FF / Epic=#AA44FF / Legendary=#FFAA00 / Ancient=#FF4444
   - 物品格子内层 1px 发光边框，颜色按 `ItemRarity` 匹配
   - Ancient 档：边框脉动呼吸效果（alpha 0.5→1.0 循环 2s）

2. **品质渐变条**（tooltip 内）
   - `spirit_quality` 0.0→1.0 渐变色条（灰→绿→金），长度 = tooltip 宽度，高 3px
   - 附文字："灵质 72%"

3. **tooltip 样式增强**（`ItemTooltipRenderer.java`）
   - 物品名颜色 = 稀有度颜色
   - 描述文字换行 + 统一中文字体间距
   - 底部追加：重量 / 格子尺寸 / 保质期剩余（如有 shelflife）

### 验收抓手

- 测试：`client::inventory::tests::rarity_border_color_mapping` / `client::inventory::tests::tooltip_spirit_quality_bar`
- 手动：打开背包 → Common 灰框 / Epic 紫框 / Ancient 红色脉动 → hover 看 tooltip 有品质条

---

## P2 — 上古遗物特效 + 3D 掉落物 + 法器图标 ⬜

### 交付物

1. **上古遗物反光逆转**（worldview §十六 视觉标记）
   - 物品栏内：图标 overlay 反色闪烁效果（每 3s 闪一次 invert color 0.2s）
   - tooltip 追加 `⚡ ×N` 充能次数 + "上古遗物·一次性"标签（红色警示）

2. **3D 掉落物视觉区分**
   - 地面掉落物按稀有度添加粒子：
     - Rare+：`BongSpriteParticle` qi_aura × 2 环绕（颜色=稀有度色）
     - Legendary：粒子 + 微弱光柱（高度 1 block）
     - Ancient：光柱 + 脉动 + 音效 `item_ancient_hum.json`（`minecraft:block.beacon.activate` pitch 2.0 volume 0.2 loop）
   - server emit：`DroppedLootSyncHandler` 已有 rarity metadata → client 按 rarity 添加粒子

3. **第二批图标（~20 张）**
   - 法器系列：飞剑/铜刀/骨匕/灵木杖 各品阶变体
   - 残卷系列：完整残卷 / 破损残卷 / 丹方碎片
   - 伪灵皮系列：轻/中/重/上古 4 档
   - 阵法道具：阵旗 / 预埋件 / 阵石

### 验收抓手

- 测试：`client::inventory::tests::ancient_relic_invert_flash` / `client::visual::tests::dropped_loot_rarity_particles`
- 手动：杀怪掉 Legendary 物品 → 地面有光柱 → 拾起 → 背包金框 → TSY 拿到上古遗物 → 反色闪烁 + ⚡ ×3

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-inventory-v1 | ✅ finished | ItemStack / inventory sync / grid 系统 |
| plan-inventory-v2 | ✅ finished | Tarkov grid / stacking |
| plan-shelflife-v1 | ✅ finished | 保质期 metadata |
| plan-fauna-v1 | ✅ finished | 兽骨物品定义 |
| plan-mineral-v1 | ✅ finished | 矿石物品定义 |
| plan-mineral-v2 | ✅ finished | 灵石分级 |
| plan-alchemy-v1 | ✅ finished | 丹药物品定义 |
| plan-tools-v1 | ✅ finished | 工具物品定义 |
| plan-tsy-loot-v1 | ✅ finished | 上古遗物 / ItemRarity::Ancient |
| plan-vfx-v1 | ✅ finished | VfxRegistry（掉落物粒子） |

**全部依赖已 finished，无阻塞。**

---

## Finish Evidence

### 落地清单

- P0 图标批次：新增 `scripts/images/gen_item_batch.py` / `scripts/images/test_gen_item_batch.py`，从 `server/assets/items/**/*.toml` 读取物品定义；默认接 `scripts/images/gen.py --style item --transparent`，并提供 `--placeholder` 作为无外部图像服务时的确定性离线生成模式。首批 42 张物品图标已落到 `client/src/main/resources/assets/bong-client/textures/gui/items/*.png`。
- P0 注册表：新增 `ItemIconRegistry.java`，统一 item texture path、fallback、scroll fallback，并把 `BotanyHudPlanner` 的植物 icon 表迁入统一注册入口。
- P1 物品栏视觉：新增 `RarityBorderRenderer.java`，`GridSlotComponent` 绘制 6 档 rarity 边框、Ancient 呼吸与反色闪烁 overlay；`ItemTooltipPanel` 增加稀有度颜色、灵质百分比与 3px 渐变条。
- P1/P2 metadata：`InventoryItem` 新增 `charges` / `isAncientRelic()` / `createFullWithVisualMeta(...)`；`InventorySnapshotHandler`、`InventoryEventHandler`、`DroppedLootSyncHandler` 保留 scroll / forge / alchemy / charges metadata。
- P2 掉落物视觉：新增 `DroppedLootRarityVisuals.java`；`DroppedItemWorldRenderer` 对 Rare+ 生成 qi aura，Legendary/Ancient 生成光柱，Ancient 周期播放 beacon hum。
- Review 修复：新增 `RarityVisuals.java` 收敛 6 档 rarity label/color/normalize；三个 inventory/dropped 入口严格拒绝非法 `charges`；图标 prompt 与资源契约统一到 128×128。
- Review 收敛：`ItemIconRegistry` 对 item id 做 lowercase normalize；`gen_item_batch.py` 对重复 item id fail-fast、对 `--ids` 去重；Ancient 掉落音效改为在物品世界坐标播放。

### 关键 commit

- `550f90132`（2026-05-10）`plan-item-visual-v1: 生成物品图标与注册表`
- `9193884a9`（2026-05-10）`plan-item-visual-v1: 增强物品栏稀有度视觉`
- `c385e9a95`（2026-05-10）`plan-item-visual-v1: 区分掉落物稀有度特效`
- `78fdaf7df`（2026-05-10）`fix(plan-item-visual-v1): 收敛稀有度和 charges 校验`
- `e888bb6d3`（2026-05-10）`fix(plan-item-visual-v1): 收敛图标脚本和掉落音效`

### 测试结果

- `python3 scripts/images/test_gen_item_batch.py` → 5 tests passed
- `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test --tests "com.bong.client.inventory.render.DroppedLootRarityVisualsTest" --tests "com.bong.client.network.DroppedLootSyncHandlerTest"` → passed
- Review fix 定向测试：`./gradlew test --tests "com.bong.client.inventory.component.ItemTooltipPanelTest" --tests "com.bong.client.inventory.InventoryItemTest" --tests "com.bong.client.inventory.ItemIconRegistryTest" --tests "com.bong.client.inventory.GeneratedItemIconAssetsTest" --tests "com.bong.client.inventory.RarityBorderRendererTest" --tests "com.bong.client.network.DroppedLootSyncHandlerTest" --tests "com.bong.client.network.InventorySnapshotHandlerTest" --tests "com.bong.client.network.InventoryEventHandlerTest"` → passed
- Review follow-up 定向测试：`./gradlew test --tests "com.bong.client.inventory.ItemIconRegistryTest" --tests "com.bong.client.inventory.render.DroppedLootRarityVisualsTest"` → passed
- `JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 PATH=/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH ./gradlew test build` → BUILD SUCCESSFUL；JUnit XML 汇总 `tests=1033 failures=0 errors=0`
- `git diff --check` → passed

### 跨仓库核验

- server：`server/assets/items/**/*.toml` 是批量图标脚本的 source of truth；未改 server runtime。
- agent/schema：沿用既有 `dropped_loot_sync` sample 与 server-data route；未新增事件类型。
- client：命中 `ItemIconRegistry`、`RarityBorderRenderer`、`InventoryItem.charges`、`InventorySnapshotHandler`、`InventoryEventHandler`、`DroppedLootSyncHandler`、`DroppedLootRarityVisuals`、`DroppedItemWorldRenderer`。

### 遗留 / 后续

- 本 plan 没有新增 server/agent contract，也没有改动生产配置。
- 首批资源可复现；如后续需要生产级 AI 图标，可直接用同一脚本默认模式重跑指定 id，把 `--placeholder` 资源替换为 `gen.py` 后端输出。
