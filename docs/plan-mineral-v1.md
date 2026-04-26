# Bong · plan-mineral-v1

**矿物材料专项**。forge / alchemy 的金属、灵晶、丹砂辅料统一在此。本 plan 承接 `docs/plan-forge-v1.md §3.2` / `§6` 的 placeholder 材料名（`xuan_iron` / `qing_steel`）下沉为正典，供 blueprint 引用。

**世界观锚点**（改用章节/关键词引用，对齐 2026-04-24 §四战力分层插入后的行号稳定性）：
- `worldview.md §四 距离衰减章` —— 凡铁/木石作低品阶基础材料（"飞 10 格损失 75% 真元"那段）
- `worldview.md §九 鲸落遗骸` —— "白色坚硬方块，中心包裹固态灵矿"
- `worldview.md §七 天道劫气` —— 突破 / 挖到极品矿脉触发劫气标记
- `worldview.md §六 经济层` —— 矿脉有限挖完就没；封灵骨币章；货币硬通货
- `worldview.md §十 青云残峰 + 血谷` —— zone 表里矿脉分布 + 血谷灵眼
- `worldview.md §九 鲸落化石` —— "偶现白色巨型化石方块，可能藏有固态灵矿"
- `worldview.md §三 末法命名原则` —— 禁用上古称呼（"练气筑基金丹元婴"那段）

**交叉引用**：`plan-forge-v1.md §3.2 §6 §7`（blueprint 材料替换）· `plan-alchemy-v1.md`（丹砂/朱砂辅料）· `plan-worldgen-v3.1.md`（矿脉生成接入 LAYER_REGISTRY）· `plan-botany-v1.md`（与草药互补：本 plan 管矿物 / botany 管草药）· `plan-shelflife-v1.md`（灵石衰变 / 矿料挥发机制 — 本 plan 只声明 decay_profile 参数，机制归 shelflife；2026-04-24 已升 active）· `plan-fauna-v1.md`（待立 — 妖兽材料，与本 plan 并列）· `plan-spiritwood-v1.md`（待立 — 灵木材料）。

---

## §-1 前置依赖实装状态（2026-04-24 audit）

| 依赖 | 状态 | 位置 | 备注 |
|------|------|------|------|
| `DecayProfileRegistry` resource | ✅ 已注册 | `server/src/shelflife/registry.rs:17` + `main.rs:80` shelflife::register 已挂 | 2770 行 shelflife 模块完整实装 |
| `ling_shi_fan_v1` profile 作 test fixture | ✅ test 存在，生产未注册 | `server/src/shelflife/registry.rs:65` | 本 plan M3 时要正式 hardcode 注册四档生产 profile（`ling_shi_fan_v1/zhong_v1/shang_v1/yi_v1`），同样格式 |
| `Freshness` NBT 在 `InventoryItem` | ✅ | `server/src/schema/inventory.rs:72` `freshness: Option<crate::shelflife::Freshness>` | mineral item 的 NBT 挂点已就位 |
| `furnace_fantie` 凡铁炉 | ✅ | `server/assets/items/core.toml:19` | §5 forge 钩子的 tier 1 炉已存在 |
| `xuan_iron` / `qing_steel` / `yi_beast_bone` placeholder | ✅ 仍存在 | `docs/plan-forge-v1.md:215,254,257,357` | 本 plan M4 批量替换为正典名 |
| `MineralId` / `MineralRegistry` / `MineralOreNode` | ✅ 已实装 | `server/src/mineral/{types,registry,components}.rs` | 18 条 mineral 全部登记（含灵石四档），test 全绿 |
| `BlockBreakEvent` 监听 | ✅ 已实装 | `server/src/mineral/break_handler.rs` | 监听 valence `DiggingEvent::Stop` → 发 `MineralDropEvent` + 概率发 `KarmaFlagIntent`；`MineralOreIndex` 启动空，待 §M2 worldgen 写入 |

---

## §0 设计轴心

### 0.1 世界观硬约束

- [ ] **末法命名原则**（worldview §三 "末法修士不配用上古称呼"）：禁用 **玄 / 陨 / 星 / 仙 / 太 / 古** 等上古仙侠词头。优选 **残 / 碎 / 锈 / 杂 / 粗 / 髓 / 朴 / 枯** 等衰败 / 素朴意象。矿物命名须传达"挖到的多半是上古仙门遗落的残渣"而非"自古神金"
- [ ] **经济位四层金字塔**（worldview §六 封灵骨币章 + 经济层锚点）— 三类矿物范畴**互斥**，不重叠：
  1. **骨币**（归 `plan-fauna-v1`）= 异变兽骨 + 阵法锁真元 → **唯一真·货币**（稀缺、可验真、携带真元）
  2. **矿物筹码** = `1.1 金属系` + `1.2 灵晶系（不含 ling_shi）` + `1.3 炼丹辅料` → 有实用价值的**交易筹码**（worldview §六 矿脉有限章"交易硬通货"），可物物交换但不是货币 — 相当于把盐/铁条当硬通货的古代低货币化经济
  3. **灵石燃料层** = **仅 `ling_shi` 一物**，从矿物筹码中独立出来（见下条逻辑链） — **不是**货币，也**不算**正规筹码
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
| `fan_tie`（凡铁） | 1 | 基础兵胎 / 凡铁炉 | 地表至 y=0，青云/血谷外层 | worldview §四 距离衰减章 锚点 · 已在 `items/core.toml furnace_fantie` |
| `cu_tie`（粗铁） | 1 | 劣质护甲 / 锈蚀兵器 | 深岩层 y=-32 至 -64 | 代替原 `jing_tie` — "精铁"偏仙侠，改"粗铁"贴末法 |
| `za_gang`（杂钢） | 2 | 灵铁炉炉体 / 中阶剑胎 | 青云残峰矿脉 | forge 现用 `qing_steel` placeholder → 改 `za_gang`；末法时代钢含杂质是常态 |
| `ling_tie`（灵铁） | 2 | 储灵兵胎 / 可注真元法器 | 血谷矿脉 | "灵"字在 worldview 中性可用（见 灵气 / 灵田 / 灵眼） |
| `sui_tie`（髓铁） | 3 | 稀铁炉炉体 / 高阶剑胎 | 鲸落遗骸核心 / 血谷灵眼 | 代替原 `xuan_tie` — 呼应 worldview §九 429 "中心包裹固态灵矿" 的"髓"意象 |
| `can_tie`（残铁） | 3 | 修复半仙器 / 旧式法器 | 上古遗迹 / event 掉落 | 代替原 `yun_tie` — 末法世界"陨铁"不存，只有"上古残铁"从废墟挖出 |
| `ku_jin`（枯金） | 4 | 渡劫兵 / 传承法器胎 | 虚空遗迹 / 鲸落深处 | 代替原 `xing_yin`（"星银"违禁）— "枯金"表达**金本该有光但已枯** |

### 1.2 灵晶系（不含灵石）

> 灵晶系**不含 `ling_shi`** — 灵石单独立作"燃料层"（见 §1.4）。本表只放具有结构性 / 阵眼用途的"硬质灵气结晶"。

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `ling_jing`（灵晶） | 2 | 法宝核 / 阵法阵眼 | 青云 / 血谷 | 浊辉内敛（非七彩辉光） |
| `yu_sui`（玉髓） | 2 | 温润法器 / 护身符 / 炼丹容器内壁 | 鲸落化石 | 青白温润，末法审美代表 |
| `wu_yao`（乌曜石） | 3 | 镇邪 / 阵旗 | 血谷深岩 | 代替原 `xuan_yao` — "玄"字去除，"乌"字保留黑曜矿物朴素意象；与欺天阵相关 |

### 1.3 炼丹辅料（矿物筹码层）

> 丹砂 / 朱砂 / 雄黄 / 硫磺 均为真实传统中医药学矿物名，不违反末法命名原则（worldview 的"禁用上古称呼"针对**境界名**如"筑基元婴"，不针对汉代以降已进入世俗医药的矿物）。

| 正典名 | 品阶 | 用途 | 世界分布 | 备注 |
|---|---|---|---|---|
| `dan_sha`（丹砂） | 1 | 炼丹辅料 / 朱红染色 / 中和 Sharp 毒 | 洞穴 / 地表红岩 | 辛度 Mellow |
| `zhu_sha`（朱砂） | 2 | 高阶炼丹 / 药引 / 提升成丹率 | 火山 / 血谷 | 辛度 Sharp |
| `xiong_huang`（雄黄） | 2 | 驱邪辅料 / 解蛊丹原料 | 洞穴深层 / 尸骸附近 | 辛度 Sharp；v2+ |
| `xie_fen`（邪粉） | 3 | 邪丹主料（魔修支线） | 负灵域 | 辛度 Violent；代替原 `xie_yu`（邪玉粉）— "邪粉"更贴末法"挖到啥算啥"的粗朴感，v2+ |

### 1.4 灵石燃料层（独立分类）

> 灵石**不属于矿物筹码**，单独立层。原因见 §0.1 经济金字塔第 3 条 + §0.1 灵石逻辑限制链。

| 正典名 | 品阶 | 初始灵气区间 | shelflife profile | 用途 | 备注 |
|---|---|---|---|---|---|
| `ling_shi_fan`（凡品灵石） | 1 | 5-15 | LingShi_T1 (Exp, half_life ≈ 3 days) | 凡修打坐辅料 / 低阶炼丹燃料 | 地表至 y=0 |
| `ling_shi_zhong`（中品灵石） | 2 | 30-60 | LingShi_T2 (Exp, half_life ≈ 5 days) | 中阶炼丹 / 灵铁炉燃料 | y=-32 至 -64 |
| `ling_shi_shang`（上品灵石） | 3 | 120-200 | LingShi_T3 (Exp, half_life ≈ 7 days) | 高阶炼丹 / 稀铁炉燃料 / 凝脉冲关辅 | 鲸落 / 血谷灵眼 |
| `ling_shi_yi`（遗品灵石） | 4 | 500+ | LingShi_T4 (Exp, half_life ≈ 14 days) | 渡劫资源 / 化虚境吸纳 | 上古遗迹，event-only |

**说明**：
- 同 `ling_shi_*` 物品 ID 的不同实例 `initial_qi` 在区间内随机（worldview §六 封灵骨币章 "不便携称量" 的实装表达）
- 不同品阶 half_life 不同：高品灵石封印更结实，挥发更慢但仍属 Exponential 路径，永远不可被冻结（区别于骨币）

---

## §2 分布与产出

### 2.1 worldgen 锚点

- [x] **worldgen 固定锚点**：青云残峰 / 血谷 / 鲸落遗骸 的矿脉位置由 `worldgen/blueprint` 写死（worldview §十 青云残峰 + 血谷 zone 表）✅ —— `worldgen/blueprint/mineral_anchors.json` 已落，含 zone × mineral_id × position × radius × max_units；运行时 spawn 待 §M2
- [x] **程序生成脉**：zone 内按 `LAYER_REGISTRY::mineral_density` 随机散布 ✅ layer 已注册（`worldgen/scripts/terrain_gen/fields.py:105`，`maximum` blend），密度曲线 vs 品阶反比待 §M2 接入
- [ ] **鲸落化石**（worldview §九 鲸落化石章）：特殊大型 structure，中心"固态灵矿"— 实装映射为 `sui_tie` + `ling_jing` + `yu_sui` + `ling_shi_shang/yi` 富集点；worldgen 生成时挂 AABB tag
- [x] **矿脉有限性**：每脉初始储量 N 块，挖完 despawn 脉体 / 标记永久耗尽（持久化落地 data/minerals/exhausted.json，归 plan-persistence-v1）✅ —— `server/src/mineral/persistence.rs` 已实装内存 log + 节流刷盘 + 启动 hydrate；despawn 在 `break_handler.rs:91-98`
- [ ] **血谷灵眼不固定**（worldview §十 血谷 zone 表）— 灵眼实体等"灵眼系统"立项（见 `reminder.md §通用`）再挂 `sui_tie` / `ling_jing` 富集点
- [ ] **上古遗迹** — `can_tie` / `ku_jin` / `ling_shi_yi` 仅在遗迹 structure 里出现，不走 zone 密度生成

### 2.2 ⭐ block ↔ item 的 mineral_id 流转

> **核心设计**：vanilla MC item 自带 ID 不够区分 mineral_id（如 `ling_tie` 与 `dan_sha` 都映射 `redstone_ore`，挖出来都是 redstone_dust）。本节定义全链路如何把 mineral_id 从 worldgen → server block → 玩家 inventory item NBT → alchemy/forge 消费。

- [ ] **worldgen 写入**：生成矿脉时，server 在 `MineralOreNode` 组件上挂 `mineral_id`（如 `MineralId::LingTie`），与方块 BlockPos 一一对应（不依赖 vanilla block ID 区分）— 组件已就位（`server/src/mineral/components.rs`），但 worldgen pipeline 尚未把 `mineral_anchors.json` 转化为 spawn OreNode + 写 `MineralOreIndex` 的 system，待 §M2
- [x] **挖块事件**（`BlockBreakEvent` 监听）✅：
  1. server 查 `MineralOreNode { mineral_id, remaining_units }`（`break_handler.rs:56-70`）
  2. **不让** vanilla loot table 决定掉落（重写 default drop）— 改 server 主动 spawn item entity，NBT 写 `bong:mineral_id = "ling_tie"` + `bong:freshness = { ... }`（接 shelflife）
  3. 客户端通过 ItemStack NBT 读到 mineral_id → tooltip 显示正典名 / 颜色档
- [x] **inventory schema 扩展**（`plan-inventory-v1`）✅：item NBT 新增 `mineral_id: Option<String>` 字段（`server/src/schema/inventory.rs:78-82`），凡是矿物来源的 item 都挂；非矿物 item 留 None
- [ ] **alchemy / forge 消费**：配方 JSON 的 `material` 字段接收 `mineral_id` 字符串（不是 vanilla item ID），消费时校验 inventory item NBT `mineral_id == required.material` — forge blueprint `qing_feng_v0` / `ling_feng_v0` 已用正典 `za_gang` / `sui_tie`，但配方校验侧是否走 `MineralRegistry::is_valid_mineral_id` 待审；alchemy 侧未接
- [ ] **极端情况**：玩家拿到没 mineral_id NBT 的 vanilla redstone（比如打怪掉的 / creative 给的）— 视作"无效矿物"，alchemy/forge 拒绝，配 chat 提示"此为凡俗 X，不可入药/入炉"
- [x] **数据契约**：mineral_id 字符串域和 §1 表正典名 1:1，由 `MineralRegistry` 统一登记，缺则返 None ✅（`registry.rs:57` `is_valid_mineral_id`）

### 2.3 server↔client 通道

- [ ] 现有 `bong:inventory_snapshot` 通道携带 item NBT 即可传 `mineral_id`；新增专用通道 `bong:mineral_meta`（待 `plan-ipc-schema-v1` 立项后协调）按需推送整体矿物注册表（客户端 tooltip 翻译用）

---

## §3 开采方式

- [ ] 镐头品阶门槛：fan_tie pickaxe → 品 1，cu_tie → 品 2... 对标 `vanilla pickaxe tier`
- [ ] **神识感知**：修为 ≥ 凝脉 的玩家右键矿脉方块触发 `MineralProbeIntent` → 返回矿种 / 剩余储量（不扣真元，低冷却）—— event 已声明（`server/src/mineral/events.rs:11`），listener 与 client request path 未接
- [x] **极品矿脉触发劫气**（worldview §七 天道劫气章）：挖到品阶 ≥ 3 的矿块（`sui_tie` / `can_tie` / `ku_jin` / `wu_yao` / `ling_shi_shang/yi`）时，按概率推 `KarmaFlagIntent` 给天道 agent ✅ —— `break_handler.rs:80-88` + `bridge.rs` 已 emit `GameEvent::MineralKarmaFlag`，tier 3 = 15% / tier 4 = 30%（plan §3 "5%→30%" 取下界=tier 3 = 15%）
- [ ] 采矿动作走 `plan-botany-v1` 同款 session 模式（长按 / 进度条）
- [x] **灵石衰变接入 `plan-shelflife-v1`**：四档 `ling_shi_*` registry 已带 `decay_profile: ling_shi_{fan,zhong,shang,yi}_v1` ✅（`registry.rs:99-134`）；shelflife 生产 profile 注册仍 test-only，drop 时 freshness 实体填充链路待 §M3 后续 / shelflife M3

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
| `ku_jin` | `gold_ore` / `deepslate_gold_ore` | **褪色的金黄 → 土黄** / 裂纹 | "枯金"核心美学 — 金本该亮但已枯；走原矿形态非冶炼块 |
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

### 4.3 资源包交付方案选型

> v1 选 **Resource Pack** 路线（用户安装侧），不走 Mod assets 强制覆盖 — 后者易与 OptiFine / Sodium / 其他材质 mod 冲突。

- [ ] **方案 A · Server Resource Pack 推送**（推荐）— Valence `ResourcePackPrompt` 在玩家加入时下发，强制接受才能继续。优点：版本统一；缺点：玩家首次进入需下载
- [ ] **方案 B · 客户端 mod 内置可选 pack**（备选）— Fabric mod jar 内附 resource pack 资源，玩家手动启用。优点：可选；缺点：版本碎片
- [ ] **`bong` 命名空间贴图**：`client/src/main/resources/assets/bong/textures/block/*.png`（mod 自有命名空间）
- [ ] **vanilla 命名空间覆盖**：`client/src/main/resources/assets/minecraft/textures/block/*.png` — **仅在 ResourcePack 内**做覆盖；不进 mod jar 的 assets 目录避免冲突
- [ ] 贴图风格参考：**汉代漆器暗朱** / **商周青铜锈绿** / **宋瓷朴素青白** / **敦煌壁画土色系**（避免 JRPG 鲜艳 / 明亮金属光）
- [ ] 避免：`emerald_ore` 原版七彩、`diamond_ore` 原版冰蓝高亮 — 改为低饱和 + 低亮度
- [ ] 延后：自定义 CustomModelData 让同 block 按 NBT 切换贴图（进阶 — v2）

---

## §5 forge 钩子

- [x] `plan-forge-v1.md §3.2` 的 blueprint 材料名**批量替换**（仅本 plan 范围）：`xuan_iron → sui_tie` / `qing_steel → za_gang` / 新增 `can_tie` / `ku_jin` ✅ —— `qing_feng_v0.json` 用 `za_gang`，`ling_feng_v0.json` 用 `sui_tie`
  - **依赖切分**：`yi_beast_bone` 替换属 fauna 范围，**不**在本 plan 的 M4 里做；fauna plan 立项后另作 PR —— 当前 `ling_feng_v0.json:357` 仍含 `yi_beast_bone` placeholder（带 fauna TODO 注释）
  - **方案**：M4 时若 fauna 未立，用 `// TODO[fauna]: yi_beast_bone → ?` 注释标记，避免 placeholder 遗忘
- [x] **inventory item NBT mineral_id 字段** 为 alchemy/forge 校验前提（见 §2.2）— alchemy/forge 升级配方校验前需先合并 `plan-inventory-v1` 的 NBT 扩展 ✅ `inventory/types.rs` + `schema/inventory.rs` 已扩
- [ ] `plan-forge-v1.md §6` inventory 扩展表的"载体材料"行删除 placeholder 警告，改引用本 plan §1
- [ ] `ForgeBlueprint.required[].material` 校验接入 `MineralRegistry::is_valid_mineral_id` —— registry API 已就位，但 forge 配方解析侧未调用，待 forge plan 升级
- [ ] 炉阶 vs 主料品阶：凡铁炉（tier 1）只接 `fan_tie` / `cu_tie`；灵铁炉（tier 2）接 `za_gang` / `ling_tie`；稀铁炉（tier 3，代替"仙铁炉"上古称呼）接 `sui_tie` / `can_tie` / `ku_jin` —— `MineralId::forge_tier_min` 已实装为 data 层契约，runtime 校验未接

---

## §6 alchemy 钩子

- [ ] `plan-alchemy-v1` 配方 JSON 新增 `auxiliary_materials[].mineral_id` 字段（现只有 botany 草药）
- [ ] 丹砂（`dan_sha`）作 Mellow 辅料：解 Sharp 毒 / 中和剧烈药性（见 `docs/library/ecology/辛草试毒录.json`）
- [ ] 朱砂（`zhu_sha`）作 Sharp 药引：提升高阶丹成丹率 + Sharp 毒副作用
- [ ] 雄黄（`xiong_huang`）作 Sharp 辅料：驱邪 / 解蛊丹原料（v2+）
- [ ] 邪粉（`xie_fen`）作 Violent 主料：邪丹（v2+，与负灵域 + 魔修支线绑）

---

## §7 数据契约

- [x] `MineralId` enum（服务端唯一 ID，按 1.1-1.3 正典名）✅ `server/src/mineral/types.rs:50` — 18 个 variant
- [x] `MineralRegistry` resource（tier / vanilla_block_id / biome_tag / forge_tier_min / alchemy_category）✅ `server/src/mineral/registry.rs`（biome_tag 字段未含 — zone 隔离当前由 worldgen anchor 决定，runtime registry 不持有）
- [x] `MineralOreNode` component（pos / mineral_id / remaining_units / exhausted_at_tick）✅ `server/src/mineral/components.rs`（`exhausted_at_tick` 落在 `ExhaustedEntry.tick`，组件本身只持 remaining_units）
- [x] `MineralProbeIntent` event（玩家神识感知触发）✅ event 声明就位（`events.rs:11`），listener 未接
- [x] `MineralExhaustedEvent` event（脉耗尽写 data/minerals/exhausted.json，归 persistence plan）✅ `events.rs:21` + `persistence.rs` 全套刷盘
- [x] 正典 JSON：`docs/library/ecology/矿物录.json`（对应 botany 的 `末法药材十七种.json`）✅

---

## §8 实施节点

- [x] **M0 — 正典定稿**：写 `docs/library/ecology/矿物录.json`（本 plan §1 四表）+ 与 worldview / botany / forge / alchemy 对齐命名 ✅
- [x] **M1 — 资源包改色**：客户端 ResourcePack vanilla ore 重绘 15 种（§4.1 表），本地 runClient 目视验证；末法审美风格评审（朴素暗沉，禁七彩 / 高亮金属）✅ —— PR #44 commit f537f808 已落地 14 张（client/src/main/resources/assets/minecraft/textures/block 下 ore 系列）
- [~] **M2 — worldgen 接入**：`worldgen/blueprint` 加矿脉固定锚点 ✅（`mineral_anchors.json`） + `LAYER_REGISTRY::mineral_density` ✅ 已注册；`MineralOreNode { mineral_id }` 与方块 BlockPos 对齐 ❌（运行时 spawn OreNode + 写 `MineralOreIndex` 的 system 缺失）
- [x] **M3 — server 正典 runtime + mineral_id 流转**：`MineralRegistry` + `MineralOreNode` + `MineralProbeIntent` + `BlockBreakEvent` 监听重写 drop 写 NBT mineral_id（§2.2）✅ —— 五项数据契约 + listener 全到位，inventory_grant 写 NBT 已实装；`MineralProbeIntent` 仅声明
- [~] **M4 — inventory NBT + forge 钩子**：`plan-inventory-v1` item NBT 扩 `mineral_id: Option<String>` ✅；batch 替换 `plan-forge-v1` blueprint JSON placeholder 材料名 ✅ `qing_feng_v0` / `ling_feng_v0` 已用 `za_gang`/`sui_tie`；`yi_beast_bone` 留 TODO ✅；`MineralRegistry::is_valid_mineral_id` 接入 forge 配方校验 ❌
- [x] **M5 — alchemy 辅料钩子**：`plan-alchemy-v1` 配方 JSON 加 `auxiliary_materials[].mineral_id`，消费时校验 inventory item NBT ✅ —— PR #44 commit a7050089 已接入 `IngredientSpec.mineral_id` 矿物辅料校验（alchemy/recipe.rs）
- [x] **M6 — 有限性 + 劫气钩子**：耗尽持久化 + 极品矿脉触发 KarmaFlag ✅ —— `persistence.rs` (record + flush + hydrate) + `bridge.rs` (KarmaFlag → agent_bridge GameEvent) 全链路就位

---

## §9 开放问题

- [ ] 矿脉被挖完后是否 respawn（按世界观 §六 557 倾向：不 respawn，除非全服事件刷新）— 需设计长期经济平衡
- [ ] 玩家之间矿脉所有权 / 争夺：worldview §九"盲盒死信箱"文化下，先挖先得 vs 灵龛领地覆盖
- [ ] 经济位四层金字塔（§0.1）的落地路径：**骨币系统**（真货币，`plan-fauna-v1`）+ **矿物作交易筹码**（本 plan）+ **挥发衰变机制**（归 `plan-shelflife-v1`）— `plan-economy-v1` 是否单独立项合流，或下沉到 fauna 内
- [ ] 灵石鉴真体验：`FreshnessProbeIntent`（走 shelflife §4）返回真灵石 vs 死灵石。末法市场"假灵石"是否可刷入？刷入则需掺假经济学（plan-economy-v1 范畴）
- [ ] `can_tie`（残铁）/ `ku_jin`（枯金）只出遗迹的话，遗迹生成节奏如何 — 与 `plan-worldgen-v3.1.md` structure 系统协调
- [ ] 客户端资源包是否走**自动下载**（Valence `ResourcePackPrompt`）还是手动放入 — 延后到 client mod 发包时决定
- [ ] CustomModelData 方案：v1 先不碰，v2 看是否要同 block 跨 biome 切贴图
- [ ] **worldgen 层面：鲸落遗骸 structure 的生成算法**（worldview §九 鲸落化石章 "白色巨型化石方块"）— 是独立 structure generator 还是借 vanilla ancient_city 变体
- [ ] **mineral_id NBT 持久化**：玩家 inventory item 的 mineral_id NBT 存档兼容（旧 vanilla item 视作 mineral_id=None；从 dropped loot 拾取回 inventory 时 mineral_id 正确流转）— 与 `plan-persistence-v1` 协调
- [ ] **mineral_id 在死亡掉落 / 容器转移 / 拾取的全链路**是否有遗漏（例：玩家死亡 inventory drop 到 dropped loot entity 时 NBT 是否携带）— 需 `plan-inventory-v1` 全链路 audit

---

> 本 plan 立项目标：取代 forge/alchemy placeholder 材料名 + 奠定 fauna / spiritwood 两份姊妹 plan 的结构模板。2026-04-24 audit 完成升 active（`docs/plan-mineral-v1.md`），下一步 `/consume-plan mineral` 按 §8 M0-M6 推进。

---

## §10 进度日志

- **2026-04-25**：实装审计 — server `mineral` 模块骨架完成 8 文件（types/registry/components/events/break_handler/inventory_grant/persistence/bridge），覆盖 M0/M3/M4/M6 主链路 + M2 锚点（`mineral_anchors.json` + `mineral_density` LAYER）；剩余缺口：M2 worldgen 实际 spawn `MineralOreNode` 入 ECS、M1 资源包改色、M5 alchemy 钩子、`MineralProbeIntent` listener、forge 配方运行时校验、shelflife 生产 profile (`ling_shi_*_v1`) 出 test。
- **2026-04-24**：PR #44 合并（merge commit 0530f0b6）—— M1/M2/M3/M4/M5/M6 全链路落地：M1 client 14 张 ore 改色（commit f537f808）/ M2 worldgen `mineral_anchors.json` + `mineral_density` LAYER（commit 4f788305）/ M3 server `mineral` 8 文件 runtime（commit 127a3ffd）/ M4 schema+forge `InventoryItem.mineral_id` NBT 流转 + forge placeholder 替换（commit 0209cb4c）/ M5 alchemy `IngredientSpec.mineral_id` 校验（commit a7050089）/ M6 矿脉耗尽持久化 + KarmaFlag agent bridge（commit 91fa3392）+ Codex P1+P2 修补 `MineralDropEvent` 消费者 + 启动期 hydrate（commit aff43c9a）+ library `矿物录` 馆藏（commit 88eca5d6）。剩余缺口：M2 worldgen 运行时 spawn `MineralOreNode` 入 ECS、`MineralProbeIntent` listener、forge 配方运行时 `is_valid_mineral_id` 校验、shelflife `ling_shi_*_v1` 生产 profile。
