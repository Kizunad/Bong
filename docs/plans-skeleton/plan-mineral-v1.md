# Bong · plan-mineral-v1 · 骨架

**矿物材料专项**。forge / alchemy 的金属、灵晶、丹砂辅料统一在此。本 plan 承接 `docs/plan-forge-v1.md §3.2` / `§6` 的 placeholder 材料名（`xuan_iron` / `qing_steel`）下沉为正典，供 blueprint 引用。

**世界观锚点**：`worldview.md §四 219`（凡铁/木石作低品阶基础材料）· `worldview.md §九 429`（鲸落遗骸可藏固态灵矿）· `worldview.md §七 492`（挖到极品矿脉触发劫气标记）· `worldview.md §六 557`（矿脉有限「挖完就没」，硬通货）· `worldview.md §十 891/893`（青云残峰 / 血谷的矿脉分布 + 血谷"灵眼"）· `worldview.md §九 906`（鲸落化石固态灵矿）。

**交叉引用**：`plan-forge-v1.md §3.2 §6 §7`（blueprint 材料替换）· `plan-alchemy-v1.md`（丹砂/朱砂辅料）· `plan-worldgen-v3.1.md`（矿脉生成接入 LAYER_REGISTRY）· `plan-botany-v1.md`（与草药互补：本 plan 管矿物 / botany 管草药）· `plan-fauna-v1.md`（待立 — 妖兽材料，与本 plan 并列）· `plan-spiritwood-v1.md`（待立 — 灵木材料）。

---

## §0 设计轴心

- [ ] 矿物 = **有限资源**（矿脉挖完就没，与 botany 采集可再生形成对比）
- [ ] 四品阶：`凡 (1) → 灵 (2) → 仙 (3) → 玄 (4)`，每阶 forge `tier` 硬门槛
- [ ] **worldgen 层面固定锚点**（worldview 骨架大地图） + 程序生成脉（zone 内随机）双轨
- [ ] **原版方块改色重绘** — 不引入自定义方块模型，贴 1.20.1 vanilla ore 方块 ID + 客户端资源包换贴图色（方案 §4）
- [ ] 灵石 = 修炼 / 炼器 / 炼丹通用燃料，既是材料也是硬通货（worldview §六）
- [ ] 极品矿脉触发天道劫气标记（worldview §七 492）— 挖掘玩法有劫后自悔的结构
- [ ] 丹砂 / 朱砂作炼丹辅料（矿物跨 forge/alchemy 的唯一路径）

---

## §1 矿物分类表

### 1.1 金属系（forge 主干）

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `fan_tie`（凡铁） | 1 | 基础兵胎 / 凡铁炉 | 地表至 y=0，青云/血谷外层 | 已在 `items/core.toml furnace_fantie` |
| `jing_tie`（精铁） | 1 | 进阶兵胎 / 劣质护甲 | 深岩层 y=-32 至 -64 | fantie 冶炼升级 |
| `qing_gang`（青钢） | 2 | 灵铁炉 / 中阶剑胎 | 青云残峰矿脉 | forge 现用 placeholder |
| `ling_tie`（灵铁） | 2 | 储灵兵胎 / 法器 | 血谷矿脉 | 可注真元 |
| `xuan_tie`（玄铁） | 3 | 仙铁炉 / 高阶剑胎 | 血谷灵眼附近 / 鲸落化石 | forge 现用 placeholder |
| `yun_tie`（陨铁） | 3 | 飞剑 / 御空法器 | 陨石坑事件产物 | 稀有，event-only |
| `xing_yin`（星银） | 4 | 仙器胎 / 渡劫法器 | 虚空遗迹 | v2+ |

### 1.2 非金属 / 灵晶系

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `ling_shi`（灵石） | 1-4 | 修炼 / 炼器 / 炼丹燃料 · 硬通货 | 全域（品阶按深度 / zone） | 玩家可碎/合成 |
| `ling_jing`（灵晶） | 2 | 法宝核 / 阵法阵眼 | 青云 / 血谷 | 七彩灵气 |
| `yu_sui`（玉髓） | 2 | 温润法器 / 护身符 | 鲸落化石 | 炼丹容器内壁 |
| `xuan_yao`（玄曜石） | 3 | 镇邪 / 阵旗 | 血谷深岩 | 与欺天阵相关 |

### 1.3 炼丹辅料

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `dan_sha`（丹砂） | 1 | 炼丹辅料 / 朱红染色 | 洞穴 / 地表红岩 | 辛度 Mellow |
| `zhu_sha`（朱砂） | 2 | 高阶炼丹 / 药引 | 火山 / 血谷 | 辛度 Sharp |
| `xie_yu`（邪玉粉） | 3 | 邪丹主料 | 负灵域 | 辛度 Violent |

---

## §2 分布与产出

- [ ] **worldgen 固定锚点**：青云残峰 / 血谷 / 鲸落遗骸 的矿脉位置由 `worldgen/blueprint` 写死（worldview §十 表格）
- [ ] **程序生成脉**：zone 内按 `LAYER_REGISTRY::mineral_density` 随机散布，密度曲线 vs 品阶反比（玄铁极稀）
- [ ] **鲸落化石**（worldview 906）：特殊大型 structure，中心固态灵矿 — worldgen 生成时 AABB tag
- [ ] **矿脉有限性**：每脉初始储量 N 块，挖完 despawn 脉体 / 标记永久耗尽（持久化落地 data/minerals/exhausted.json，归 plan-persistence-v1）
- [ ] **血谷灵眼不固定**（worldview 893）— 灵眼实体等 `灵眼系统`立项（见 `docs/plans-skeleton/reminder.md §通用`）再挂玄铁/灵晶富集点

---

## §3 开采方式

- [ ] 镐头品阶门槛：fan_tie pickaxe → 品 1，jing_tie → 品 2... 对标 `vanilla pickaxe tier`
- [ ] **神识感知**：修为 ≥ 凝脉 的玩家右键矿脉方块触发 `MineralProbeIntent` → 返回矿种 / 剩余储量（不扣真元，低冷却）
- [ ] **极品矿脉触发劫气**（worldview §七 492）：挖到品阶 ≥ 3 的矿块时，按概率推 `KarmaFlagIntent` 给天道 agent（负面事件概率 5% → 30%）
- [ ] 采矿动作走 `plan-botany-v1` 同款 session 模式（长按 / 进度条）

---

## §4 原版方块改色方案（⭐️ 核心）

**原则**：不引入自定义 `BlockState` 或新的 block ID（避免 Valence 协议 763 兼容问题 + 无需客户端注册新方块）。通过 **客户端资源包重绘贴图** 把 vanilla ore 改成修仙观感。

### 4.1 MC 1.20.1 vanilla block 映射

| 正典矿名 | vanilla block | 资源包改色方向 | 备注 |
|---|---|---|---|
| `fan_tie` | `iron_ore` / `deepslate_iron_ore` | 保留灰褐底，加粗颗粒 | y > 0 地表 |
| `jing_tie` | `deepslate_iron_ore` | 偏蓝灰 + 金属光泽 | y < 0 |
| `qing_gang` | `copper_ore` | 改青绿 / 淡银（去氧化斑） | — |
| `ling_tie` | `redstone_ore` | 改冷紫 / 发光效果保留 | 脉动灵气 |
| `xuan_tie` | `ancient_debris` | 改幽蓝黑 | 纹理加符文 |
| `yun_tie` | `obsidian` 变体 | 改深灰 + 陨星光点 | event 掉落 |
| `ling_shi` | `diamond_ore` | 改半透明白 | — |
| `ling_jing` | `emerald_ore` | 改七彩辉光 | — |
| `yu_sui` | `lapis_ore` | 改温润青白玉 | 去深蓝 |
| `xuan_yao` | `coal_ore` | 改漆黑带红纹 | — |
| `dan_sha` | `redstone_ore`（单独 biome 区分） | 保持朱红 / 减光强 | vs ling_tie 的 zone 隔离 |
| `zhu_sha` | `nether_gold_ore` | 改深朱红 | 火山 biome |
| `xie_yu` | `nether_quartz_ore` | 改暗紫白裂纹 | 负灵域 |

### 4.2 冲突消解

- `redstone_ore` 同时被 `ling_tie` 和 `dan_sha` 占用 → 用 **biome 隔离**：青云/血谷生成的 `redstone_ore` = ling_tie，地表洞穴生成的 = dan_sha。客户端通过 zone 查询切换 tooltip / mineral_id 解析（server 权威，客户端 HUD 只显示）
- 所有重绘不改 block ID / hitbox / sound — 仅贴图层变化，Valence 协议层无感

### 4.3 资源包交付

- [ ] `client/src/main/resources/assets/bong/textures/block/*.png` 重绘贴图
- [ ] `client/src/main/resources/assets/minecraft/models/block/*.json` 覆盖 vanilla 模型（仅贴图引用）
- [ ] 贴图风格参考：国风水墨 / 汉代漆器朱红 / 商周青铜锈绿（避免 JRPG 鲜艳色）
- [ ] 延后：自定义 CustomModelData 让同 block 按 NBT 切换贴图（进阶 — v2）

---

## §5 forge 钩子

- [ ] `plan-forge-v1.md §3.2` 的 blueprint 材料名**批量替换**：`xuan_iron → xuan_tie` / `qing_steel → qing_gang` / `yun_tie` 新增 / `yi_beast_bone` 改到 `plan-fauna-v1`
- [ ] `plan-forge-v1.md §6` inventory 扩展表的 "载体材料" 行删除 placeholder 警告，改引用本 plan §1
- [ ] `ForgeBlueprint.required[].material` 校验接入 `MineralRegistry::is_valid_mineral_id`
- [ ] 炉阶 vs 主料品阶：凡铁炉（tier 1）只接 fan_tie/jing_tie；灵铁炉（tier 2）接 qing_gang/ling_tie；仙铁炉（tier 3）接 xuan_tie 及以上

---

## §6 alchemy 钩子

- [ ] `plan-alchemy-v1` 配方 JSON 新增 `auxiliary_materials[].mineral_id` 字段（现只有 botany 草药）
- [ ] 丹砂（`dan_sha`）作 Mellow 辅料：解 Sharp 毒 / 中和剧烈药性（见 `docs/library/ecology/辛草试毒录.json`）
- [ ] 朱砂（`zhu_sha`）作 Sharp 药引：提升高阶丹成丹率 + Sharp 毒副作用
- [ ] 邪玉粉（`xie_yu`）作 Violent 主料：邪丹（v2+，与负灵域 + 魔修支线绑）

---

## §7 数据契约

- [ ] `MineralId` enum（服务端唯一 ID，按 1.1-1.3 正典名）
- [ ] `MineralRegistry` resource（tier / vanilla_block_id / biome_tag / forge_tier_min / alchemy_category）
- [ ] `MineralOreNode` component（pos / mineral_id / remaining_units / exhausted_at_tick）
- [ ] `MineralProbeIntent` event（玩家神识感知触发）
- [ ] `MineralExhaustedEvent` event（脉耗尽写 data/minerals/exhausted.json，归 persistence plan）
- [ ] 正典 JSON：`docs/library/ecology/矿物录.json`（对应 botany 的 `末法药材十七种.json`）

---

## §8 实施节点

- [ ] **M0 — 正典定稿**：写 `docs/library/ecology/矿物录.json`（本 plan §1 三表）+ 与 worldview / botany / forge / alchemy 对齐命名
- [ ] **M1 — 资源包改色**：客户端资源包 vanilla ore 重绘 13 种（§4.1 表），本地 runClient 目视验证
- [ ] **M2 — worldgen 接入**：`worldgen/blueprint` 加矿脉固定锚点 + `LAYER_REGISTRY::mineral_density` 程序生成脉
- [ ] **M3 — server 正典 runtime**：`MineralRegistry` + `MineralOreNode` + `MineralProbeIntent`
- [ ] **M4 — forge 钩子**：batch 替换 `plan-forge-v1` blueprint JSON placeholder 材料名
- [ ] **M5 — alchemy 辅料钩子**：`plan-alchemy-v1` 配方 JSON 加 `auxiliary_materials[].mineral_id`
- [ ] **M6 — 有限性 + 劫气钩子**：耗尽持久化 + 极品矿脉触发 KarmaFlag

---

## §9 开放问题

- [ ] 矿脉被挖完后是否 respawn（按真实世界观：不 respawn，除非全服事件刷新）— 需设计长期经济平衡
- [ ] 玩家之间矿脉所有权 / 争夺：worldview §九"盲盒死信箱"文化下，先挖先得 vs 灵龛领地覆盖
- [ ] 灵石作通用货币的汇率：灵石 ↔ 普通矿 ↔ 丹药（经济设计要不要独立 `plan-economy-v1`）
- [ ] 陨铁 / 星银如何产出：event-driven（天劫落下陨石）vs 固定遗迹 — 与 `plan-tribulation-v1` 协调
- [ ] 客户端资源包是否走 **自动下载**（Valence `ResourcePackPrompt`）还是手动放入 — 延后到 client mod 发包时决定
- [ ] CustomModelData 方案：v1 先不碰，v2 看是否要同 block 跨 biome 切贴图

---

> 本 plan 立项目标：取代 forge/alchemy placeholder 材料名 + 奠定 fauna / spiritwood 两份姊妹 plan 的结构模板。草案一经对齐立即迁到 `docs/plan-mineral-v1.md` 转为正式推进文档。
