# Bong · plan-mineral-v1 · 骨架

**矿物材料专项**。forge / alchemy 的金属、灵晶、丹砂辅料统一在此。本 plan 承接 `docs/plan-forge-v1.md §3.2` / `§6` 的 placeholder 材料名（`xuan_iron` / `qing_steel`）下沉为正典，供 blueprint 引用。

**世界观锚点**：`worldview.md §四 219`（凡铁/木石作低品阶基础材料）· `worldview.md §九 429`（鲸落遗骸可藏固态灵矿）· `worldview.md §七 492`（挖到极品矿脉触发劫气标记）· `worldview.md §六 557`（矿脉有限「挖完就没」，硬通货）· `worldview.md §十 891/893`（青云残峰 / 血谷的矿脉分布 + 血谷"灵眼"）· `worldview.md §九 906`（鲸落化石固态灵矿）。

**交叉引用**：`plan-forge-v1.md §3.2 §6 §7`（blueprint 材料替换）· `plan-alchemy-v1.md`（丹砂/朱砂辅料）· `plan-worldgen-v3.1.md`（矿脉生成接入 LAYER_REGISTRY）· `plan-botany-v1.md`（与草药互补：本 plan 管矿物 / botany 管草药）· `plan-shelflife-v1.md`（灵石衰变 / 矿料挥发机制 — 本 plan 只声明 decay_profile 参数，机制归 shelflife）· `plan-fauna-v1.md`（待立 — 妖兽材料，与本 plan 并列）· `plan-spiritwood-v1.md`（待立 — 灵木材料）。

---

## §0 设计轴心

### 0.1 世界观硬约束

- [ ] **末法命名原则**（worldview §63 "末法修士不配用上古称呼"）：禁用 **玄 / 陨 / 星 / 仙 / 太 / 古** 等上古仙侠词头。优选 **残 / 碎 / 锈 / 杂 / 粗 / 髓 / 朴 / 枯** 等衰败 / 素朴意象。矿物命名须传达"挖到的多半是上古仙门遗落的残渣"而非"自古神金"
- [ ] **经济位四层金字塔**（worldview §518 锚点）：
  1. **骨币**（归 `plan-fauna-v1`）= 异变兽骨 + 阵法锁真元 → **唯一真·货币**（稀缺、可验真、携带真元）
  2. **矿物（金属 / 玉石）** = 有实用价值的**交易筹码**（worldview §六 557"交易硬通货"），可物物交换但不是货币 — 相当于把盐/铁条当硬通货的古代低货币化经济
  3. **灵石** = **末法劣质衰变物**（见下条，逻辑链展开）
  4. **金银** = 废土（Earth 本位失效）
- [ ] **灵石逻辑限制链**（对齐"骨币能当货币 ↔ 灵石比骨币更垃圾"的经济逻辑）：
  - **衰变机制**归 `plan-shelflife-v1`（挥发 Exponential 档，half_life ≈ 3 real-days）— 本 plan 不重造，只声明 `ling_shi` 的 `decay_profile` 参数
  - **易掺假**：末法市场流通的灵石 70% 含杂石 / 半废料，真灵石目测鉴别困难（需**神识感知**，走 shelflife §4）
  - **不便携称量**：不同矿脉的灵石灵气含量（`initial_qi`）飘忽 20-80%，不能作为标准化货币单位
  - **纯消费品**：一烧就没，和骨币里**阵法封印循环**的真元本质不同（骨币用 shelflife Linear + Freeze 档，衰变慢且可续印；灵石 Exponential 不可封印）
  - 真修士与正规商队只认**骨币 + 实物（丹药 / 矿物）换**；灵石只在**新手 / 凡俗小市**作小额以物易物
- [ ] 末法审美：资源包贴图方向是**褪色 / 锈蚀 / 朴拙**（汉代漆器、商周青铜锈），不是"七彩辉光 / 神光大作"

### 0.2 结构性轴心

- [ ] 矿物 = **有限资源**（worldview §六 557 矿脉挖完就没，与 botany 采集可再生形成对比）
- [ ] 四品阶：`凡 (1) → 灵 (2) → 稀 (3) → 遗 (4)` — "遗"指"上古遗物级"，强调稀有而非神圣
- [ ] **worldgen 层面固定锚点**（worldview 骨架大地图） + 程序生成脉（zone 内随机）双轨
- [ ] **原版方块改色重绘** — 不引入自定义方块模型，贴 1.20.1 vanilla ore 方块 ID + 客户端资源包换贴图色（方案 §4）
- [ ] 极品矿脉触发天道劫气标记（worldview §七 492）— 挖掘玩法有劫后自悔的结构
- [ ] 丹砂 / 朱砂作炼丹辅料（矿物跨 forge/alchemy 的唯一路径）

---

## §1 矿物分类表

> **命名重构**：原草案用 `xuan_tie` / `yun_tie` / `xing_yin` 违反末法命名原则（§0.1），全部替换为衰败系词汇。`qing_gang` 保留（"青钢"是真实冶金术语，不算仙侠化）。

### 1.1 金属系（forge 主干）

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `fan_tie`（凡铁） | 1 | 基础兵胎 / 凡铁炉 | 地表至 y=0，青云/血谷外层 | worldview §四 217 锚点 · 已在 `items/core.toml furnace_fantie` |
| `cu_tie`（粗铁） | 1 | 劣质护甲 / 锈蚀兵器 | 深岩层 y=-32 至 -64 | 代替原 `jing_tie` — "精铁"偏仙侠，改"粗铁"贴末法 |
| `za_gang`（杂钢） | 2 | 灵铁炉炉体 / 中阶剑胎 | 青云残峰矿脉 | forge 现用 `qing_steel` placeholder → 改 `za_gang`；末法时代钢含杂质是常态 |
| `ling_tie`（灵铁） | 2 | 储灵兵胎 / 可注真元法器 | 血谷矿脉 | "灵"字在 worldview 中性可用（见 灵气 / 灵田 / 灵眼） |
| `sui_tie`（髓铁） | 3 | 稀铁炉炉体 / 高阶剑胎 | 鲸落遗骸核心 / 血谷灵眼 | 代替原 `xuan_tie` — 呼应 worldview §九 429 "中心包裹固态灵矿" 的"髓"意象 |
| `can_tie`（残铁） | 3 | 修复半仙器 / 旧式法器 | 上古遗迹 / event 掉落 | 代替原 `yun_tie` — 末法世界"陨铁"不存，只有"上古残铁"从废墟挖出 |
| `ku_jin`（枯金） | 4 | 渡劫兵 / 传承法器胎 | 虚空遗迹 / 鲸落深处 | 代替原 `xing_yin`（"星银"违禁）— "枯金"表达**金本该有光但已枯** |

### 1.2 非金属 / 灵晶系

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `ling_shi`（灵石） | 1-4 | 修炼 / 炼器 / 炼丹**燃料**（非货币 · 末法衰变物） | 全域（品阶按深度 / zone） | §0.1 经济链：末法衰变、易挥发、易掺假、不便携称量；凡俗小额以物易物用，真修士与正规商队不认 |
| `ling_jing`（灵晶） | 2 | 法宝核 / 阵法阵眼 | 青云 / 血谷 | 浊辉内敛（非七彩辉光） |
| `yu_sui`（玉髓） | 2 | 温润法器 / 护身符 / 炼丹容器内壁 | 鲸落化石 | 青白温润，末法审美代表 |
| `wu_yao`（乌曜石） | 3 | 镇邪 / 阵旗 | 血谷深岩 | 代替原 `xuan_yao` — "玄"字去除，"乌"字保留黑曜矿物朴素意象；与欺天阵相关 |

### 1.3 炼丹辅料

> 丹砂 / 朱砂 / 雄黄 / 硫磺 均为真实传统中医药学矿物名，不违反末法命名原则（worldview 的"禁用上古称呼"针对**境界名**如"筑基元婴"，不针对汉代以降已进入世俗医药的矿物）。

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `dan_sha`（丹砂） | 1 | 炼丹辅料 / 朱红染色 / 中和 Sharp 毒 | 洞穴 / 地表红岩 | 辛度 Mellow |
| `zhu_sha`（朱砂） | 2 | 高阶炼丹 / 药引 / 提升成丹率 | 火山 / 血谷 | 辛度 Sharp |
| `xiong_huang`（雄黄） | 2 | 驱邪辅料 / 解蛊丹原料 | 洞穴深层 / 尸骸附近 | 辛度 Sharp；v2+ |
| `xie_fen`（邪粉） | 3 | 邪丹主料（魔修支线） | 负灵域 | 辛度 Violent；代替原 `xie_yu`（邪玉粉）— "邪粉"更贴末法"挖到啥算啥"的粗朴感，v2+ |

---

## §2 分布与产出

- [ ] **worldgen 固定锚点**：青云残峰 / 血谷 / 鲸落遗骸 的矿脉位置由 `worldgen/blueprint` 写死（worldview §十 表格）
- [ ] **程序生成脉**：zone 内按 `LAYER_REGISTRY::mineral_density` 随机散布，密度曲线 vs 品阶反比（髓铁 / 残铁 / 枯金 极稀）
- [ ] **鲸落化石**（worldview 906）：特殊大型 structure，中心"固态灵矿"— 实装映射为 `sui_tie` + `ling_jing` + `yu_sui` 富集点；worldgen 生成时挂 AABB tag
- [ ] **矿脉有限性**：每脉初始储量 N 块，挖完 despawn 脉体 / 标记永久耗尽（持久化落地 data/minerals/exhausted.json，归 plan-persistence-v1）
- [ ] **血谷灵眼不固定**（worldview 893）— 灵眼实体等"灵眼系统"立项（见 `reminder.md §通用`）再挂 `sui_tie` / `ling_jing` 富集点
- [ ] **上古遗迹** — `can_tie` / `ku_jin` 仅在遗迹 structure 里出现，不走 zone 密度生成

---

## §3 开采方式

- [ ] 镐头品阶门槛：fan_tie pickaxe → 品 1，cu_tie → 品 2... 对标 `vanilla pickaxe tier`
- [ ] **神识感知**：修为 ≥ 凝脉 的玩家右键矿脉方块触发 `MineralProbeIntent` → 返回矿种 / 剩余储量（不扣真元，低冷却）
- [ ] **极品矿脉触发劫气**（worldview §七 492）：挖到品阶 ≥ 3 的矿块（`sui_tie` / `can_tie` / `ku_jin` / `wu_yao`）时，按概率推 `KarmaFlagIntent` 给天道 agent（负面事件概率 5% → 30%）
- [ ] 采矿动作走 `plan-botany-v1` 同款 session 模式（长按 / 进度条）
- [ ] **灵石衰变接入 `plan-shelflife-v1`**：`ling_shi` item NBT 挂通用 `Freshness { decay_profile: LingShi }`，`DecayProfile::LingShi = Exponential(half_life ≈ 3 real-days)`；神识感知走 shelflife §4 通用 probe 机制

---

## §4 原版方块改色方案（⭐️ 核心）

**原则**：不引入自定义 `BlockState` 或新的 block ID（避免 Valence 协议 763 兼容问题 + 无需客户端注册新方块）。通过 **客户端资源包重绘贴图** 把 vanilla ore 改成修仙观感。

### 4.1 MC 1.20.1 vanilla block 映射

> **末法审美原则**：所有改色方向以**褪色 / 锈蚀 / 内敛**为主，禁止"七彩辉光 / 鲜亮饱和"。挖到的矿物应**看起来像上古仙门遗落的残渣**，不是"神金天银"。

| 正典矿名 | vanilla block | 资源包改色方向 | 备注 |
|---|---|---|---|
| `fan_tie` | `iron_ore` / `deepslate_iron_ore` | 灰褐底 + 锈斑 / 颗粒粗糙 | y > 0 地表 |
| `cu_tie` | `deepslate_iron_ore` | 暗灰 + 结块状锈 / 无金属光泽 | y < 0（"粗铁"朴素） |
| `za_gang` | `copper_ore` | 暗青绿 / 斑驳锈蚀（像出土青铜） | — |
| `ling_tie` | `redstone_ore` | 冷紫 / **暗淡内敛**（不要闪光，用 low emissive） | 脉动灵气但内敛 |
| `sui_tie` | `ancient_debris` | 骨白 + 深褐纹理（像腐朽骨髓切面） | 鲸落化石意象 |
| `can_tie` | `obsidian` 变体 | 暗褐 + 风化碎裂纹 | 上古遗迹掉落 |
| `ku_jin` | `raw_gold_block` 变体 | **褪色的金黄 → 土黄** / 裂纹 | "枯金"核心美学 — 金本该亮但已枯 |
| `ling_shi` | `diamond_ore` | 青白 + 半透明 / 低亮度（非高亮钻石辉光） | 燃料非货币 |
| `ling_jing` | `emerald_ore` | 青翠偏暗 / 浊辉内敛（去七彩） | — |
| `yu_sui` | `lapis_ore` | 温润青白玉 / 去深蓝 | 末法审美代表 |
| `wu_yao` | `coal_ore` | 漆黑 + 赤红暗纹 | 镇邪 / 欺天阵材料 |
| `dan_sha` | `redstone_ore`（biome 区分） | 朱红偏暗 / 减光强 | vs ling_tie 的 zone 隔离 |
| `zhu_sha` | `nether_gold_ore` | 深朱红 + 硫磺黄晶簇 | 火山 biome |
| `xiong_huang` | `nether_gold_ore` 变体 | 硫磺黄 + 结晶面 | 洞穴深层，v2+ |
| `xie_fen` | `nether_quartz_ore` | 暗紫白裂纹 / 粉末感 | 负灵域，v2+ |

### 4.2 冲突消解

- `redstone_ore` 同时被 `ling_tie` 和 `dan_sha` 占用 → 用 **biome 隔离**：青云 / 血谷生成的 `redstone_ore` = `ling_tie`，地表洞穴生成的 = `dan_sha`。客户端通过 zone 查询切换 tooltip / mineral_id 解析（server 权威，客户端 HUD 只显示）
- `nether_gold_ore` 同时被 `zhu_sha` 和 `xiong_huang` 占用 → `zhu_sha` 走火山 biome，`xiong_huang` 走普通洞穴深层（v2+ 实装时按 zone 分流）
- 所有重绘不改 block ID / hitbox / sound — 仅贴图层变化，Valence 协议层无感

### 4.3 资源包交付

- [ ] `client/src/main/resources/assets/bong/textures/block/*.png` 重绘贴图
- [ ] `client/src/main/resources/assets/minecraft/models/block/*.json` 覆盖 vanilla 模型（仅贴图引用）
- [ ] 贴图风格参考：**汉代漆器暗朱** / **商周青铜锈绿** / **宋瓷朴素青白** / **敦煌壁画土色系**（避免 JRPG 鲜艳 / 明亮金属光）
- [ ] 避免：`emerald_ore` 原版七彩、`diamond_ore` 原版冰蓝高亮 — 改为低饱和 + 低亮度
- [ ] 延后：自定义 CustomModelData 让同 block 按 NBT 切换贴图（进阶 — v2）

---

## §5 forge 钩子

- [ ] `plan-forge-v1.md §3.2` 的 blueprint 材料名**批量替换**：`xuan_iron → sui_tie` / `qing_steel → za_gang` / 新增 `can_tie` / `ku_jin` / `yi_beast_bone` 移到 `plan-fauna-v1`
- [ ] `plan-forge-v1.md §6` inventory 扩展表的"载体材料"行删除 placeholder 警告，改引用本 plan §1
- [ ] `ForgeBlueprint.required[].material` 校验接入 `MineralRegistry::is_valid_mineral_id`
- [ ] 炉阶 vs 主料品阶：凡铁炉（tier 1）只接 `fan_tie` / `cu_tie`；灵铁炉（tier 2）接 `za_gang` / `ling_tie`；稀铁炉（tier 3，代替"仙铁炉"上古称呼）接 `sui_tie` / `can_tie` / `ku_jin`

---

## §6 alchemy 钩子

- [ ] `plan-alchemy-v1` 配方 JSON 新增 `auxiliary_materials[].mineral_id` 字段（现只有 botany 草药）
- [ ] 丹砂（`dan_sha`）作 Mellow 辅料：解 Sharp 毒 / 中和剧烈药性（见 `docs/library/ecology/辛草试毒录.json`）
- [ ] 朱砂（`zhu_sha`）作 Sharp 药引：提升高阶丹成丹率 + Sharp 毒副作用
- [ ] 雄黄（`xiong_huang`）作 Sharp 辅料：驱邪 / 解蛊丹原料（v2+）
- [ ] 邪粉（`xie_fen`）作 Violent 主料：邪丹（v2+，与负灵域 + 魔修支线绑）

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
- [ ] **M1 — 资源包改色**：客户端资源包 vanilla ore 重绘 15 种（§4.1 表），本地 runClient 目视验证；末法审美风格评审（朴素暗沉，禁七彩 / 高亮金属）
- [ ] **M2 — worldgen 接入**：`worldgen/blueprint` 加矿脉固定锚点 + `LAYER_REGISTRY::mineral_density` 程序生成脉
- [ ] **M3 — server 正典 runtime**：`MineralRegistry` + `MineralOreNode` + `MineralProbeIntent`
- [ ] **M4 — forge 钩子**：batch 替换 `plan-forge-v1` blueprint JSON placeholder 材料名
- [ ] **M5 — alchemy 辅料钩子**：`plan-alchemy-v1` 配方 JSON 加 `auxiliary_materials[].mineral_id`
- [ ] **M6 — 有限性 + 劫气钩子**：耗尽持久化 + 极品矿脉触发 KarmaFlag

---

## §9 开放问题

- [ ] 矿脉被挖完后是否 respawn（按世界观 §六 557 倾向：不 respawn，除非全服事件刷新）— 需设计长期经济平衡
- [ ] 玩家之间矿脉所有权 / 争夺：worldview §九"盲盒死信箱"文化下，先挖先得 vs 灵龛领地覆盖
- [ ] 经济位四层金字塔（§0.1）的落地路径：**骨币系统**（真货币，`plan-fauna-v1`）+ **矿物作交易筹码**（本 plan）+ **挥发衰变机制**（归 `plan-shelflife-v1`）— `plan-economy-v1` 是否单独立项合流，或下沉到 fauna 内
- [ ] 灵石鉴真体验：`FreshnessProbeIntent`（走 shelflife §4）返回真灵石 vs 死灵石。末法市场"假灵石"是否可刷入？刷入则需掺假经济学（plan-economy-v1 范畴）
- [ ] `can_tie`（残铁）/ `ku_jin`（枯金）只出遗迹的话，遗迹生成节奏如何 — 与 `plan-worldgen-v3.1.md` structure 系统协调
- [ ] 客户端资源包是否走**自动下载**（Valence `ResourcePackPrompt`）还是手动放入 — 延后到 client mod 发包时决定
- [ ] CustomModelData 方案：v1 先不碰，v2 看是否要同 block 跨 biome 切贴图
- [ ] **worldgen 层面：鲸落遗骸 structure 的生成算法**（worldview §九 906 "白色巨型化石方块"）— 是独立 structure generator 还是借 vanilla ancient_city 变体

---

> 本 plan 立项目标：取代 forge/alchemy placeholder 材料名 + 奠定 fauna / spiritwood 两份姊妹 plan 的结构模板。草案一经对齐立即迁到 `docs/plan-mineral-v1.md` 转为正式推进文档。
