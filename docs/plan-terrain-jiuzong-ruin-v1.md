# Bong · plan-terrain-jiuzong-ruin-v1 · Active

> **状态**：⏳ active（2026-05-04 升级，user 拍板覆盖 2026-04-29 hard block 决议）。前 hard block：(a) library-jiuzong-history 7 篇宗门志、(b) 残卷功法绑定——**改列为 P0 任务跟进**，不阻塞升 active：lore 写作可在 active 状态下与 worldgen 代码并行。

**九宗故地**（`jiu_zong_ruin`）。末法纪略第一变之产物——上古九大宗门（青云/灵泉/血溪/幽暗/北陵/南渊/赤霞/玄水/太初）大斗后崩塌仅余其二，本 plan 把**已崩**的七个宗门抽象为一个公共 terrain profile：连片宗门废墟群（断殿/聚灵阵核/万人讲堂残基），灵气紊乱（0.4 但波动剧烈）、阵法残核可能短暂激活、散修守墓人 NPC 游荡。

**世界观锚点**：
- `worldview.md §一 末法时代`（"曾经的大能们早已飞升或陨落，留下的只有残破的洞府、失控的禁制"）
- `worldview.md §九 经济与交易 · 食腐者 / 游商傀儡`（散修守墓人是宗门废墟的本土 NPC 类型）
- `worldview.md §八 天道行为准则`（隐性手段"narration 暗示某个方向有机缘"——废墟中的阵核就是机缘饵）
- `worldview.md §十 资源与匮乏 · 残卷`（残卷来源："击杀道伥 / 遗迹探索"——遗迹探索的核心场景就是宗门废墟）
- `worldview.md §十七 地形对季节的响应`（**上古宗门遗迹 / 阵核 = 汐转期阵核激活率 ×2** —— 古阵感应天道情绪；汐转期天道节律紊乱，封死的阵核易被误触发；老玩家不在汐转期穿越宗门遗迹）

**library 锚点**：
- `world-0002 末法纪略` 第一变（九大宗门崩塌，仅存青云外门 + 灵泉丹宗——本 plan 是"另外七宗"的物理化）
- `world-0004 天道口述残编` 其三（"化虚者三。一在坍缩渊深处，尚在闭关。吾已忘其名"——化虚遗物可能藏在某宗废墟）
- `geography-0002 六域舆图考`（六域之外残存的"灵气近零之荒野"中，七宗废墟散落其间）
- `world-0003 骨币半衰录`（"青云残峰外门墙上之刻字：骨币会贬，记载不朽"——废墟壁文是潜在 lore 抓点）

**交叉引用**：
- `plan-tsy-zongmen-ruin-v1`（已 ✅，TSY 位面的"宗门遗迹"是同源叙事的位面化版本——本 plan 是其**主世界**对应物）
- `plan-mineral-v1`（残灵阵核可能含 ling_jing / yu_sui 矿物锚点）
- `plan-narrative-v1`（废墟壁文 narration / 阵核激活叙事）
- `plan-skill-v1`（残卷 = 学习术法核心 → 残卷掉落 hot-spot）
- `plan-baomai-v1`（爆脉流功法残篇据传出自血溪宗废墟——可作为 P3 lore hook）

**阶段总览**：
- P0 ⬜ profile 注册 + 7 处宗门废墟在 blueprint 候选位标记（每宗一个 zone，各自带 origin 字段）
- P1 ⬜ `JiuzongRuinGenerator` 实装（地形 + 残殿 structure + 装饰物）
- P2 ⬜ 灵气紊乱机制 + 阵核激活事件（小概率 + 可被 trigger 的 anomaly）
- P3 ⬜ 散修守墓人 spawn rule + 残卷 loot table + 七宗 origin 各自的特征材质 palette

---

## §0 设计轴心

- [ ] **七宗一 profile 共享 + origin 区分**：参考 TSY 已有的 `tsy_origin_id` 模式——一个 `jiu_zong_ruin` profile + `zongmen_origin_id` 层 (1=血溪 / 2=北陵 / 3=南渊 / 4=赤霞 / 5=玄水 / 6=太初 / 7=幽暗)，七宗共享地形骨架但各有特征装饰
- [ ] **灵气紊乱 ≠ 灵气浓**：均值 0.4，但**局部方差极大**——同一宗废墟内可能 0.1 至 0.7 起伏，因为聚灵阵残核没烂完仍在抽 / 排
- [ ] **阵核可激活**：每个废墟 1-3 个阵核（landmark），玩家投入特定材料（灵草 / 骨币 / 真元）→ 短期形成局部 0.6 灵气区（30 分钟）+ 高概率招异变兽 / 道伥；高风险高回报
- [ ] **残卷为核心 loot**：残卷只能从废墟内特定容器（藏经阁残基、长老坐化处）取得，不是地表散落。掘三铲都是凡铁、运气好挖到一卷功法残页
- [ ] **守墓人 NPC**：每个宗一个固定守墓人（散修，自称该宗后代或信徒），中立但若玩家激活阵核则敌对（"诸君何苦惊扰先师"）
- [ ] **季节响应**（worldview §十七）：阵核**自激活率**（玩家不投料的"自动启动"概率）= base × `Season::tide_multiplier()`（Summer / Winter ×1.0 / 汐转期 ×2.0）。汐转期穿越宗门遗迹有翻倍概率撞上"先前没人激活的阵核突然亮起"——是末法残土"老玩家避汐转 / 新手撞死"教学的具体场景

## §1 世界观推断逻辑（为何此地必然存在）

> world-0002 末法纪略明记九宗大斗后"九宗死伤过半，灵脉大伤"，"末法纪二百年前后，九宗已去其七"。残存的青云外门 + 灵泉丹宗已对应 broken_peaks / spring_marsh 两个 profile。**剩下七个宗门的物理废墟**，逻辑上必然存在于残土上，只是末法以来荒废。

七宗废墟的存在锚定了三个世界观功能：
- **残卷源头**（worldview §十）：除"击杀道伥"外，残卷的另一来源就是宗门遗迹挖掘——本 profile 提供地表化的、低门槛的探索点（不必下坍缩渊）
- **散修来源**（worldview §九）：拾荒散修 / 食腐者本质上是七宗后代败落转职——废墟是他们 spawn 的"历史正确"位置
- **天道叙事钩子**（worldview §八 隐性手段）：废墟阵核激活 → 短暂高灵气吸引修士聚集 → 又是天道的诱饵机制（与 pseudo_vein_oasis 同源逻辑，但本地有真东西）

**与 TSY 的边界**：plan-tsy-zongmen-ruin-v1 的"宗门遗迹"在 TSY 位面（独立 dimension），是被 family system 索引的精装版（多层 Y stratified、有结界、boss 守灵）。本 plan 是**主世界地表版**——更小、更破、loot 档次低，但门槛低（不需要破结界、不需要找 family 入口）。**两者关系类似坍缩渊 vs 死坍缩渊：本 plan 是 TSY 宗门遗迹的"地表风化版"**。

## §2 特殊机制

| 机制 | 触发 | 效果 |
|---|---|---|
| **灵气紊乱场** | 玩家在废墟内 | qi 局部值随时间在 [0.1, 0.7] 抖动，period ≈ 90 秒；修炼难以稳定（突破事件需 3 分钟稳定 0.5+ → 在此处通过率显著降低） |
| **阵核激活** | 玩家对阵核投入材料 | 30 分钟内中心 60 格 qi=0.6；激活时**区域 narration**（半径 1000 格内玩家可读 + 凝脉+ 境界 inspect 可读）"X 宗故地灵脉异动"——非全服广播（全服仅化虚渡劫级才用，见 worldview §八）；同时招 1-2 异变兽 + 0.3 几率招道伥 |
| **守墓人警戒** | 阵核被激活 / 玩家挖核心容器 | 守墓人 NPC 立即敌对，使用该宗特征流派招式（血溪 → 体修 / 北陵 → 阵法）|
| **残卷探索** | 玩家掘开宗门特征容器（藏经阁残基 / 长老坐化处） | 1-2% 几率掉落残卷（流派 / origin 决定残卷类型）；无激活阵核时几率为 0 |
| **壁文 narration** | 玩家近距离 (<3 格) 接近壁文方块 | 触发该宗历史 narration（七宗各自有 3-5 条片段） |
| **origin 染色亲和** | 玩家真元染色与该宗特征流派一致（如锋锐色 + 玄水宗剑修废墟）| 残卷掉率 ×1.5 + 阵核激活成本 -30% |

## §3 七宗 origin 与特征 palette

> **注**：worldview §三 / world-0002 末法纪略明列九宗为"青云、灵泉、血溪、幽暗、北陵、南渊、赤霞、玄水、太初"——青云 + 灵泉存（已对应 broken_peaks / spring_marsh），余七宗失。各宗的**特征流派**（血溪=体修 / 北陵=阵法 / ...）**为本 plan 推演**，正典 worldview / library 仅给出宗名，未明确各宗主修方向。P1 实施前建议先立 `library-jiuzong-history` 立 7 篇宗门志做 lore 锚（见 §9 开放问题）。

| origin_id | 宗名 | 特征流派（推演）| 主色 palette | 标志性装饰 |
|---|---|---|---|---|
| 1 | 血溪 | 体修 / 爆脉流 | red_terracotta / blackstone / netherrack | 万血斗台（祭坛）|
| 2 | 北陵 | 阵法 / 地师 | deepslate_bricks / lodestone / chiseled_stone | 阵法核心残柱 |
| 3 | 南渊 | 毒蛊 / 医道 | warped_planks / sculk / verdant_froglight | 蛊池残皿 |
| 4 | 赤霞 | 雷法 | copper_block / weathered_copper / gold_block | 引雷塔残基 |
| 5 | 玄水 | 御剑 / 飞剑流 | snow_block / packed_ice / iron_block | 试剑石碑林 |
| 6 | 太初 | 任督 / 全能型 | smooth_quartz / chiseled_quartz / amethyst_block | 太极阵盘 |
| 7 | **幽暗** | 暗器 / 隐遁 | cobbled_deepslate / soul_soil / soul_lantern | 影壁残基 |

> origin_id=7 是**九宗之一的幽暗宗**（非"附属"）。地理上与现代 `cave_network`（幽暗地穴）邻接是叙事自洽——现代地穴是古幽暗宗矿脉/秘境网络的演化遗留，但 zone profile 互相独立、boundary semi_hard 处理。

## §4 独特装饰物（DecorationSpec 预填，origin 共享 + 特化）

```python
JIU_ZONG_RUIN_DECORATIONS_COMMON = (
    DecorationSpec(
        name="broken_pillar",
        kind="boulder",
        blocks=("chiseled_stone_bricks", "stone_bricks", "mossy_stone_bricks"),
        size_range=(4, 8),
        rarity=0.55,
        notes="断柱：刻纹石砖 + 苔藓砖——倒卧或半埋的大殿石柱。"
              "通用骨架，所有 origin 共享。",
    ),
    DecorationSpec(
        name="ruined_bell_tower",
        kind="tree",
        blocks=("oak_log", "stone_bricks", "bell"),
        size_range=(7, 12),
        rarity=0.10,
        notes="残钟楼：橡木柱 + 石砖基座 + 顶端铜钟。"
              "近距离接触触发钟声 narration（'昔有万人闻钟'）。",
    ),
    DecorationSpec(
        name="moss_lain_statue",
        kind="boulder",
        blocks=("mossy_cobblestone", "cracked_stone_bricks", "armor_stand"),
        size_range=(2, 4),
        rarity=0.25,
        notes="苔卧像：苔石 + 裂砖 + armor_stand 残身。"
              "曾是某代长老雕像，面部已剥蚀。",
    ),
    DecorationSpec(
        name="formation_core_stub",
        kind="crystal",
        blocks=("lodestone", "chiseled_stone_bricks", "amethyst_cluster"),
        size_range=(3, 5),
        rarity=0.06,
        notes="阵核残柱：磁石 + 刻纹石 + 紫晶。"
              "**可激活的 landmark**——投入材料触发灵气抖动。"
              "每废墟限 1-3 个。",
    ),
    DecorationSpec(
        name="forgotten_stele_garden",
        kind="boulder",
        blocks=("polished_andesite", "chiseled_polished_blackstone", "sculk_vein"),
        size_range=(3, 6),
        rarity=0.18,
        notes="忘碑林：磨光安山岩 + 黑石刻砖 + 苔脉。"
              "壁文 narration 锚——靠近触发该宗历史片段。",
    ),
)

# origin-specific 装饰各 1-2 项，按 zongmen_origin_id 切换 variant_id
JIU_ZONG_ORIGIN_SPECIFIC = {
    1: DecorationSpec(  # 血溪
        name="bloodstream_altar",
        kind="boulder",
        blocks=("red_concrete", "blackstone", "redstone_lamp"),
        size_range=(3, 5),
        rarity=0.20,
        notes="万血祭坛：红混凝土 + 黑石 + 红石灯。"
              "血溪宗体修流派祭坛，近之心悸。",
    ),
    2: DecorationSpec(  # 北陵
        name="formation_anchor_pillar",
        kind="crystal",
        blocks=("lodestone", "deepslate_bricks", "chiseled_deepslate"),
        size_range=(4, 6),
        rarity=0.18,
        notes="阵眼锚柱：磁石 + 深板岩砖。北陵阵法核心。",
    ),
    3: DecorationSpec(  # 南渊
        name="poison_pool_basin",
        kind="boulder",
        blocks=("warped_planks", "sculk", "verdant_froglight"),
        size_range=(3, 4),
        rarity=0.15,
        notes="蛊池残皿：扭曲木板 + 苔脉 + 翠绿蛙明灯。"
              "南渊宗炼蛊废池，靠近触发轻微毒效。",
    ),
    4: DecorationSpec(  # 赤霞
        name="lightning_pylon_stub",
        kind="tree",
        blocks=("copper_block", "weathered_copper", "lightning_rod"),
        size_range=(6, 9),
        rarity=0.12,
        notes="引雷塔残：铜块 + 风化铜 + 避雷针。"
              "赤霞雷法宗的雷电吸引塔，雷雨天气会真的招雷。",
    ),
    5: DecorationSpec(  # 玄水
        name="trial_blade_stele",
        kind="boulder",
        blocks=("snow_block", "iron_block", "stone_bricks"),
        size_range=(2, 4),
        rarity=0.22,
        notes="试剑碑：雪 + 铁块 + 石砖。玄水剑宗弟子比试遗碑，"
              "上有剑痕（vanilla 无法表达深度，文字 narration 传达）。",
    ),
    6: DecorationSpec(  # 太初
        name="taiji_formation_disc",
        kind="boulder",
        blocks=("smooth_quartz", "polished_blackstone", "amethyst_block"),
        size_range=(4, 6),
        rarity=0.10,
        notes="太极阵盘：石英 + 黑石 + 紫晶——黑白对称大圆盘。"
              "太初宗任督全能流派标志。",
    ),
    7: DecorationSpec(  # 幽暗
        name="shadow_screen_wall",
        kind="boulder",
        blocks=("cobbled_deepslate", "soul_soil", "soul_lantern"),
        size_range=(3, 5),
        rarity=0.20,
        notes="影壁残基：深板岩 + 灵魂土 + 灵魂灯——半透气黑墙残段。"
              "幽暗宗暗器流隐遁训练场。",
    ),
}
```

`ambient_effects = ("distant_chime", "stone_dust_drift")`——远钟声 + 石尘飘动，营造宗门衰败感。

## §5 完整 profile 配置

### `terrain-profiles.example.json` 追加

```json
"jiu_zong_ruin": {
  "height": { "base": [72, 90], "peak": 100 },
  "boundary": { "mode": "semi_hard", "width": 96 },
  "surface": ["mossy_cobblestone", "stone_bricks", "cracked_stone_bricks", "coarse_dirt", "gravel"],
  "water": { "level": "very_low", "coverage": 0.03 },
  "passability": "medium",
  "origin_field": "zongmen_origin_id",
  "origins": ["bloodstream", "beilling", "nanyuan", "chixia", "xuanshui", "taichu", "youan"]
}
```

### Blueprint zone 模板（七处候选位）

```json
{
  "name": "jiuzong_<origin>_ruin",
  "display_name": "<宗名>故地",
  "aabb": { "min": [<cx-400>, 60, <cz-400>], "max": [<cx+400>, 110, <cz+400>] },
  "size_xz": [800, 800],
  "spirit_qi": 0.40,
  "danger_level": 6,
  "worldgen": {
    "terrain_profile": "jiu_zong_ruin",
    "shape": "irregular_blob",
    "boundary": { "mode": "semi_hard", "width": 96 },
    "extras": { "zongmen_origin_id": <1..7> },
    "landmarks": ["formation_core_stub", "forgotten_stele_garden"]
  }
}
```

候选位（与现有六域不冲突的远端坐标）：
- 血溪故地：(5500, -1000)（血谷东北外缘 ~3500 格）
- 北陵故地：(-1000, -8500)（北荒北部）
- 南渊故地：(0, 6000)（南荒中部）
- 赤霞故地：(6000, 4000)（东南远端）
- 玄水故地：(-6500, 1500)（西部远端）
- 太初故地：(0, -10000)（极北边界附近）
- 幽暗故地：(2800, 4500)（与现代 cave_network 邻接 → boundary semi_hard，叙事上是古幽暗宗的演化遗留）

### 数值梯度

| 区位 | qi_density 均值 | qi 抖动幅度 | mofa_decay | qi_vein_flow | flora_density |
|---|---|---|---|---|---|
| 阵核激活前 |  |  |  |  |  |
| 大殿核心 | 0.35 | ±0.25 | 0.55 | 0.20 | 0.40 |
| 长老坐化处 | 0.45 | ±0.30 | 0.50 | 0.30 | 0.35 |
| 万人讲堂残基 | 0.30 | ±0.15 | 0.60 | 0.10 | 0.50 |
| 外缘 | 0.20 | ±0.05 | 0.65 | 0 | 0.30 |
| 阵核激活后（30 分钟） |  |  |  |  |  |
| 中心 60 格 | 0.60 | ±0.05 | 0.30 | 0.70 | n/a |
| 该圈外溢 | 已激活区域 -0.10 嫁接 | | | | |

## §6 LAYER_REGISTRY 字段映射

```python
extra_layers = (
    "qi_density",
    "mofa_decay",
    "qi_vein_flow",
    "flora_density",
    "flora_variant_id",
    "ruin_density",         # 已存在层，用于 structure 密度（建筑残骸覆盖率）
    "anomaly_intensity",    # 阵核激活时局部峰值
    "anomaly_kind",         # 5 = wild_formation（已在 LAYER_REGISTRY 注释中预留 enum）
)

# 建议 LAYER_REGISTRY 新增（本 plan 提议）：
# "zongmen_origin_id": LayerSpec(safe_default=0.0, blend_mode="swap", export_type="uint8")
```

`zongmen_origin_id` 是新提议的 uint8 swap 层，与 TSY 的 `tsy_origin_id` 同模式但走主世界。也可以**复用 `tsy_origin_id` 的 8-15 段**（TSY 用 1-4，主世界宗门借用 8-14）以减少 schema 增长——具体决策见 §10。

## §7 数据契约（下游 grep 抓手）

| 阶段 | 抓手 | 位置 |
|---|---|---|
| P0 | `jiu_zong_ruin` profile + 7 zone | `worldgen/terrain-profiles.example.json` + blueprint |
| P1 | `class JiuzongRuinGenerator` + `fill_jiu_zong_ruin_tile` | `worldgen/scripts/terrain_gen/profiles/jiu_zong_ruin.py`（新增） |
| P1 | `JIU_ZONG_RUIN_DECORATIONS_COMMON` 5 项 + `JIU_ZONG_ORIGIN_SPECIFIC` 字典 | 同上 |
| P2 | `struct ZongFormationCore { activated_until, base_qi, charge_required }` | `server/src/worldgen/zong_formation.rs`（新增） |
| P2 | qi 紊乱 tick: `QiTurbulenceField` zone-scoped | `server/src/cultivation/qi_field.rs` |
| P2 | `bong:zong_core_activated` event | IPC schema 新增 `ZongCoreActivation` |
| P3 | 七位守墓人 NPC entity 注册 | `server/src/npc/zong_keeper.rs` |
| P3 | 残卷 loot table（按 origin） | `server/assets/loot/zong_canjuan_<origin>.json` |
| P3 | `forgotten_stele_garden` narration triggers | `agent/packages/tiandao/src/narration/zong_lore.ts` |

## §8 实施节点

- [ ] **P0** profile + 7 zone 注册 — 验收：raster manifest 含全部 7 zone；每 zone `extras.zongmen_origin_id` 值正确
- [ ] **P1** generator + 装饰物 — 验收：raster 中 origin_id 切换处装饰特征命中（如 origin=1 才出现 bloodstream_altar）；ruin_density 在大殿核心 > 0.6
- [ ] **P2** 灵气紊乱 + 阵核激活 — 验收：qi 抖动测量 90s period 命中 ±0.2 振幅；激活事件触发后 30 分钟 qi 稳定 0.6 + narration 全服广播
- [ ] **P3** 守墓人 + 残卷 + 壁文 narration — 验收：每宗守墓人独立流派招式触发；残卷掉率统计正确（活化后 ×1.5 → 1-2% × 1.5 = 1.5-3%）；壁文片段七宗各自不重样

## §9 开放问题

- [x] **Q-J1 ✅**（user 2026-05-04 A）：**强绑定** —— 残卷 = 该宗对应 plan 的功法。一一对应表：
  - 血溪 → plan-baomai-v1（爆脉流，残卷 = 体修功法）
  - 北陵 → plan-zhenfa-v1（阵法，残卷 = 阵法图）
  - 南渊 → plan-dugu-v1（毒蛊 / 医道，残卷 = 蛊术 / 丹方）
  - 赤霞 → plan-anqi-v1（雷法 / 暗器，残卷 = 雷符 / 暗器谱）
  - 玄水 → plan-zhenmai-v1（御剑 / 截脉，残卷 = 剑诀 / 截脉术）
  - 太初 → plan-multi-style-v1（任督全能，残卷 = 经脉拓扑图）
  - 幽暗 → plan-tuike-v1（暗器 / 隐遁 / 替尸，残卷 = 替尸 / 隐遁术）
  - **跨 plan 耦合**：每个流派 plan vN+1 都得加"接收残卷 → 解锁高阶招式"接口；§7 P3 抓手新增 `RecipeFragment::Style(StyleId)` 物品 dispatch 表
- [x] ~~`zongmen_origin_id` 新增独立层 vs 复用 `tsy_origin_id` 的高位段~~ **已决议**（用户 2026-04-29）：**新增独立层**——复用 tsy_origin_id 高位段会让 layer 语义混淆（地表宗门遗迹 vs TSY 位面遗迹是不同物理位置 / 不同探索路径），独立 layer 更清晰；schema 层成本可接受（uint8 + 8 个 origin 值 + safe_default=0 + swap blend）
- [x] **Q-J3 ✅**（user 2026-05-04，my recommendation A）：阵核激活复用 `anomaly_kind=5 (wild_formation)`，已预留。"野化阵法" 语义本身契合"阵核异常激活"；用 `anomaly_intensity` 字段携带强度（投入材料量决定）。独立 event 方案弃。
- [x] **Q-J4 ✅**（user 2026-05-04 A）：跨宗仇视守墓人 P3 不做。守墓人只对自宗废墟激活 / 自宗核心容器被挖时敌对。
- [x] **Q-J5 ✅**（user 2026-05-04 B）：本 plan **允许小概率高阶残卷**（不一律低阶）。掉率：低阶 1-2%（基础）/ 高阶 0.1-0.3%（仅在阵核激活后 30 分钟窗口期内 + origin 染色亲和加成时触发）。TSY plan-tsy-zongmen-ruin-v1 仍是高阶残卷主产地（高阶 5-15%），但本地形不是"绝对低阶"。
- [x] **Q-J2 ✅**（user 2026-05-04）：library-jiuzong-history 7 篇宗门志 **先立 plan-library-jiuzong-history-v1 skeleton 提取要点**（与 P0 worldgen 代码并行）。lore 入库后 unblock 壁文 narration / 残卷功法绑定细节。

## §10 进度日志

- 2026-04-28：骨架立项。锚定 world-0002 末法纪略第一变 + plan-tsy-zongmen-ruin-v1 已 ✅。等优先级排序与 plan-skill-v1（残卷功法学习）+ plan-mineral-v1（阵核矿物锚点）+ plan-narrative-v1（壁文 narration）协调。提议新增 `zongmen_origin_id` 层 schema 决策。
- 2026-04-28（自查修订）：
  - **strong-3** 修：origin_id=7 "幽暗附属（非九宗主流）" 改为"**幽暗**"——worldview §三 / world-0002 明列九宗，幽暗本身就是九宗之一，非附属；地理上与 cave_network 邻接是叙事自洽（现代地穴是古宗演化遗留）。
  - **mid-11** 修：阵核激活的 narration "全服广播" 改为"区域广播 (1000 格半径) + 凝脉+ inspect 可读"——全服仅留化虚渡劫级。
  - **weak-14** 修：§3 七宗特征流派表加注"以下流派为推演"提示 + 建议先立 `library-jiuzong-history` 锚 lore。
- **2026-04-29**：实地核验 + 决策标注（**保留骨架**，不升 active——schema 决策已锁但 7 宗 lore 仍缺 library 背书；前置 plan-tsy-zongmen-ruin-v1 状态自报有误，实际仅有 worldgen 侧 `tsy_zongmen_ruin.py` 生成器存在，无独立 plan 文档）。
- **2026-05-04**：skeleton → active 升级（user 拍板覆盖 04-29 决议）。原 hard block 改列为 P0 任务跟进，不阻塞升 active：
  - **P0 新增子任务 a**：起 `plan-library-jiuzong-history-v1` skeleton（7 篇宗门志要点提取，user Q-J2 决策）；可与 P0 worldgen 代码并行；lore 入库 + `/review-book` 通过后 unblock 壁文 narration
  - **P0 新增子任务 b**：与 7 流派 plan（baomai / zhenpa / dugu / anqi / zhenmai / tuike / multi-style）协调"残卷 = 该 plan 的功法"对齐（user Q-J1 强绑定，详 §9 决策表）
  - 下一步：起 P0 worktree（`zongmen_origin_id` LAYER_REGISTRY 层 + `jiu_zong_ruin` profile + 7 zone JSON，与 lore 写作并行）
- **2026-05-04**：§9 全部 5 决策闭环（Q-J1 强绑定 / Q-J3 复用 anomaly_kind=5 / Q-J4 P3 不做跨宗仇视 / Q-J5 允许小概率高阶残卷 / Q-J2 立 library 子 plan，详 §9）。
  - **季节联动**（用户决策 2026-04-29）：汐转期阵核激活率 ×2（worldview §十七 锚定）。已写入头部锚点 + §0 设计轴心。
  - **schema 决策**（用户决策）：`zongmen_origin_id` 新增独立 LAYER_REGISTRY 层（不复用 tsy_origin_id 高位段）。§9 第 2 项已标决议。
  - 7 宗特征流派 + 残卷功法绑定（§9 第 1 项）+ lore 来源（§9 第 6 项 `library-jiuzong-history`）仍待立——这是升 active 前的硬阻塞。建议**先用 `/write-book` 起草 7 篇宗门志**入 library，再立 lore-anchored plan。
  - 补 `## Finish Evidence` 占位。
  - 升 active 触发条件：（a）`library-jiuzong-history` 7 篇宗门志入库且通过 `/review-book`；（b）§9 第 1 项"残卷 = 该 plan 的功法"明确化（与 plan-baomai / plan-zhenfa / plan-skill-v1 对齐）。两件 done 后再升。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：`worldgen/scripts/terrain_gen/fields.py` LAYER_REGISTRY 加 `zongmen_origin_id: LayerSpec(0, "swap", "uint8")` + `worldgen/terrain-profiles.example.json` 加 `jiu_zong_ruin` profile + 7 zone JSON
  - P1：`worldgen/scripts/terrain_gen/profiles/jiu_zong_ruin.py`（JiuzongRuinGenerator + 七宗装饰物 palette）
  - P2：`server/src/worldgen/zong_formation.rs`（qi 紊乱 tick + 阵核激活 + 自激活率 × Season::tide_multiplier）
  - P3：`agent/packages/tiandao/src/narration/zong_lore.ts`（七宗壁文 narration）+ 残卷 loot table 接入
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - worldgen：`zongmen_origin_id` LAYER_REGISTRY / `JiuzongRuinGenerator` / 7 zone
  - server：`ZongFormationTick` / 阵核激活 system / 守墓人 NPC（每宗 1 个）
  - agent：七宗 narration template
- 遗留 / 后续：
  - 七宗 library 入库（前置：`library-jiuzong-history`）
  - 残卷功法绑定（与 plan-baomai / plan-zhenfa / plan-skill-v1 对齐）
  - 跨宗仇视守墓人（§9 第 4 项，P3+ 才考虑）
