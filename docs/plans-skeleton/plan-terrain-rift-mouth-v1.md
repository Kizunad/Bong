# Bong · plan-terrain-rift-mouth-v1 · 骨架

**渊口荒丘**（`rift_mouth_barrens`）。worldview §十六 实现注："坍缩渊**不嵌在主世界坐标系内**，而是以独立位面实现。主世界各处的'裂缝'只是跨位面传送门的入口锚点"。本 plan 把"裂缝锚点"周边的**主世界地表景观**做成一个独立 profile：焦黑石原 + 寒气结晶 + 干尸残骸 + 局部负灵 hot-spot 散布——这是修士口中"坍缩渊在地表的痕迹"。

**世界观锚点**：
- `worldview.md §十六 秘境：活坍缩渊 · 世界层实现注`（"主世界各处的'裂缝'只是跨位面传送门的入口锚点，本身不占主世界的物理腔体；被吸入裂缝的修士被抛入坍缩渊位面"）
- `worldview.md §十六 入场`（"入口不标在地图上。活坍缩渊是天道的临时地理——地壳上一道看似普通的裂缝、洞穴尽头的异常寒气、幽暗地穴深处的新鲜崩石"）
- `worldview.md §十六 死亡结算 · 干尸 → 道伥`（"道伥是在活→死的塌缩过程中被喷到地表的——流落荒野作乱"——本地形是道伥涌出的物理位置）
- `worldview.md §二 灵压环境 · 负灵域`（"游离风暴：天道为代偿某处伪灵脉而在荒野随机制造的负能风暴"——渊口附近也有局部负灵）
- `worldview.md §十七 地形对季节的响应`（**渊口寒气带 / 负灵域边缘 = 冬季 ×1.3 / 夏季 ×0.7** —— 寒气本质是低温 + 负压复合：冬季"冬聚"放大封冻效应；夏季外散稍稀；汐转期 RNG 化）

**library 锚点**：
- `world-0001 诸渊录·卷一·枯木崖`（"渊口寸草不生，石色焦黑。往下看不到底——非是深，乃是暗。光入其中即被吞，如投石入墨"——直接美术参考）
- `geography-0004 北荒坍缩渊记`（"渊口方圆约八百格，呈不规则之坑。口沿寸草不生，石色焦黑"+"渊口周三十步之内，灵压约 -0.8"——尺寸/灵压数值参考）
- `world-0004 天道口述残编` 其三（"一在坍缩渊深处，尚在闭关"——叙事 hook）

**交叉引用**：
- `plan-tsy-dimension-v1`（已 ✅，TSY 位面系统——本 plan 是其**主世界入口锚点的地表表达**）
- `plan-tsy-worldgen-v1`（已 ✅，TSY 位面 manifest——渊口 zone 必须能声明对应的 TSY zone link）
- `plan-tsy-extract-v1`（已 ✅，撤离机制——主裂缝撤离点的主世界落点就在本地形内）
- `plan-tsy-hostile-v1`（已 ✅，道伥是塌缩外溢——本地形需 spawn 道伥从渊口走出）
- `plan-cultivation-v1`（境界感知 + 真元 tick——负灵 hot-spot 的抽吸机制）

**阶段总览**：
- P0 ⬜ profile 注册 + 3-7 处渊口 zone 在 blueprint 候选位（北荒、幽暗地穴深处、血谷地下，与 §十六.一"形成途径"对应）
- P1 ⬜ `RiftMouthBarrensGenerator` 实装（地形 field + 装饰物 + 焦黑石）
- P2 ⬜ 局部负灵 hot-spot tick（30 步内 -0.5 ~ -0.8 抽吸） + 寒气视觉效果
- P3 ⬜ 主世界 ↔ TSY 位面入口 link（玩家踏入裂缝中心 → 触发 portal → 进入 TSY 坍缩渊）+ 塌缩外溢道伥 spawn

---

## §0 设计轴心

- [ ] **不在主世界占体积**：worldview §十六 明确"裂缝本身不占主世界的物理腔体"——本 profile 不挖洞、不写 cave_mask；地表只画"裂缝锚点的视觉印记"（焦黑斑、干尸、寒气晶簇），实际 portal 由 plan-tsy-dimension-v1 系统接管
- [ ] **小尺度高密度**：单个渊口 zone 直径 200-400 格（比 §十六 描述的 800 格小一半，主世界部分只是边缘印记），但锚点密集（中心 30 格内灵压陡降）
- [ ] **本 profile 仅覆盖"地表暴露态"渊口**：worldview §十六 给三种入口形态——(a) 地壳裂缝（地表暴露）、(b) 洞穴尽头异常寒气（嵌 cave_network 内）、(c) 幽暗地穴深处新鲜崩石（嵌 cave_network / abyssal_maze 内）。**本 plan 只做 (a)**；(b)(c) 属其他 profile 内部 hot-spot，由那些 profile 通过 `anomaly_kind=1 (spacetime_rift)` + 局部 `neg_pressure` 表达，**不在本 plan 范围**——后续由 plan-tsy-zone-followup-v1 接管 cave/abyssal 内部入口
- [ ] **portal 入口看似普通**：worldview §十六 明文"地壳上一道**看似普通**的裂缝...只有靠近时**感知负压异常**的修士才认得出"——本 profile **不放显眼地标**作为 portal 入口。Portal 锚点用 zone.extras.portal_anchor_xz 坐标级显式给出，地表视觉是与普通 wilderness 裂缝几乎一致的 `cracked_floor_seam`（cobblestone + stone + tuff），靠 inspect / 凝脉+ 境界感知才能识别
- [ ] **道伥外溢 spawn pool**：渊口是道伥进入主世界的唯一物理点（worldview §十六.六）——本 profile spawn weight 系数对道伥 ×5；外溢节奏与对应 TSY 位面坍缩渊的塌缩事件 1:1 同步
- [ ] **不可灵龛**：与活坍缩渊内同源逻辑（worldview §十一）——龛石在渊口 30 格内被吞噬。注：worldview §十一 允许 -0.1 灵气区放灵龛（高阶玩家"拼着吃药"），但 -0.8 渊口远超此阈值
- [ ] **季节响应**（worldview §十七）：寒气强度 = base × `Season::frost_multiplier()`（冬季 ×1.3 / 夏季 ×0.7 / 汐转 RNG ±0.3）。寒气晶簇（ice_crystal_cluster）出现密度 + 真元 -10/sec 的 base rate 都按此修饰。霜骨苔等冬季限定物种只在 Winter + 冬汐转期 spawn（联动 plan-botany-v2 SeasonRequired hazard）

## §1 世界观推断逻辑（为何此地必然存在）

> worldview §十六 已硬性规定坍缩渊在独立位面，主世界只是入口锚点。**那么主世界中的入口锚点必然有可识别的地理特征**——否则修士无法用感知发现入口（§十六"入口不标在地图上...只有靠近时感知负压异常的修士才认得出"，但**视觉上仍然有迹可循**才符合直觉）。

地理特征的**物理来源**：
- **焦黑石**：上古大能陨落瞬间灵气抽干（world-0001 枯木崖原文 "三峰轰然倒插，地气逆涌"），地表岩石被高强度真元抽吸瞬间炭化
- **寒气结晶**：负灵压使空气中残存真元过冷，结晶析出蓝白冰晶（与 §二 "深海潜水"灵压差类比）
- **干尸残骸**：塌缩外溢时被喷出的修士遗骸（§十六.六），地表暴露态干尸
- **新鲜崩石**：地壳承受跨位面 portal 张力，长期产生小型崩塌——这就是 §十六 描述的"幽暗地穴深处的新鲜崩石"

**与既有 profile 的边界**：
- 与 `waste_plateau` 不同：waste 是大尺度退化高原（粒度大、灵气 0.05-0.15 平稳）；rift_mouth 是**小尺度高对比**斑点（300 格内灵压 0.05 → -0.8）
- 与 `cave_network` 不同：cave 是地下网络；rift_mouth 主要是**地表表征**，地下 portal 由 TSY 系统接管
- 与 `abyssal_maze` 不同：abyssal 是深渊式垂直洞穴（profile 自带三层）；rift_mouth 是 portal 锚点的地表"伤疤"，本身不下穿
- 与 `ancient_battlefield` 不同：古战场是大能们活着对打的地方；rift_mouth 是大能死后塌缩留下的地方——**前者有 anomaly_kind=3 (blood_moon_anchor)，后者有 anomaly_kind=1 (spacetime_rift)**

## §2 特殊机制

| 机制 | 触发 | 效果 |
|---|---|---|
| **负灵抽吸 hot-spot** | 玩家进入 rift center 30 格 | 真元抽吸 ×（境界系数）：引气 -2/sec / 凝脉 -5/sec / 固元 -10/sec / 通灵 -25/sec / 化虚 -60/sec（呼应 §二 非线性） |
| **寒气视觉** | 玩家在 hot-spot 内 | 客户端粒子 `frost_breath` + ambient sound `cold_wind`；HUD 灵压条显示负值 |
| **portal 触发** | 玩家踩中 zone.extras.portal_anchor_xz 坐标 ±2 格 | 生成 portal entity（plan-tsy-dimension-v1 标准入口）→ 跳转 TSY 坍缩渊位面。**坐标驱动而非 landmark 驱动**——避免地表显眼地标暴露 portal 位置，符合 worldview §十六"看似普通的裂缝" |
| **道伥外溢** | 对应 TSY 位面坍缩渊塌缩事件 | 渊口 spawn 1-N 个道伥（按塌缩时位面内未撤出修士数量），向外游荡 ≥ 24 小时 |
| **龛石失效** | 30 格内放置 | 龛石被吞 + chat："此地灵脉断裂，龛石不立"（与 pseudo_vein 同 chat key 模板） |
| **境界感知** | 凝脉+ 距渊口 ≤ 50 格 | narration: "此处空气微寒，似有缝隙"——与 §十六"靠近时感知负压异常"原文对齐。这是玩家**唯一**确认 portal 位置的可靠手段 |

## §3 独特装饰物（DecorationSpec 预填）

```python
RIFT_MOUTH_DECORATIONS = (
    DecorationSpec(
        name="charred_obelisk_shard",
        kind="boulder",
        blocks=("blackstone", "obsidian", "crying_obsidian"),
        size_range=(3, 7),
        rarity=0.45,
        notes="焦黑碑碎：黑石 + 黑曜 + 哭泣黑曜——焦炭化的小型石柱碎片。"
              "渊口标志。哭泣黑曜负责'寒气滴落'视觉。",
    ),
    DecorationSpec(
        name="frost_qi_cluster",
        kind="crystal",
        blocks=("packed_ice", "blue_ice", "amethyst_cluster"),
        size_range=(2, 5),
        rarity=0.35,
        notes="寒气晶簇：紧密冰 + 蓝冰 + 紫晶。负压使残存真元过冷析出。"
              "在地表暴露态渊口（本 profile）中等密度散布——"
              "高密度寒气仅在 cave_network / abyssal_maze 内部入口出现，由那些 profile 处理。",
    ),
    DecorationSpec(
        name="ganshi_drift",
        kind="boulder",
        blocks=("bone_block", "white_concrete", "soul_soil"),
        size_range=(2, 4),
        rarity=0.18,
        notes="干尸漂积：骨块 + 白混 + 灵魂土——塌缩外溢的修士干尸堆。"
              "近之心悸（HUD 灵压闪烁）。loot 含退活骨币 / 凡铁残装。",
    ),
    DecorationSpec(
        name="fresh_collapse_rubble",
        kind="boulder",
        blocks=("cobblestone", "tuff", "cobbled_deepslate"),
        size_range=(3, 5),
        rarity=0.30,
        notes="新鲜崩石：圆石 + 凝灰 + 深板岩圆石——刚塌不久的碎石堆。"
              "本 profile 中等密度——'幽暗地穴深处的新鲜崩石'入口形态在 "
              "cave_network 内部出现，由那个 profile 处理。表面苔藓覆盖率为 0（崭新）。",
    ),
    DecorationSpec(
        name="spacetime_scar",
        kind="crystal",
        blocks=("end_stone", "purpur_block", "shulker_box"),
        size_range=(2, 3),
        rarity=0.05,
        notes="时空疤：末地石 + 紫珀 + 潜影盒（外观，无实际 inv）——"
              "极稀有，标志该 rift 是高品质 portal（对应 TSY 大能陨落起源）。",
    ),
    DecorationSpec(
        name="dao_zhuang_corpse_pose",
        kind="boulder",
        blocks=("bone_block", "armor_stand", "stripped_oak_log"),
        size_range=(1, 2),
        rarity=0.08,
        notes="道伥姿干尸：骨块 + armor_stand + 剥皮原木。"
              "外溢未活化的道伥姿态，凝固在塌缩瞬间。"
              "靠近 30s + 玩家真元 < 20% 时可能"半激活"——闪一下消失（埋伏 hook）。",
    ),
    DecorationSpec(
        name="cracked_floor_seam",
        kind="boulder",
        blocks=("cobblestone", "stone", "tuff"),
        size_range=(1, 2),
        rarity=0.40,
        notes="裂缝石：圆石 + 石 + 凝灰岩——看似普通的地表裂缝。"
              "**与普通 wilderness 裂缝外观一致**——portal 入口的视觉印记，"
              "但**不显眼标识入口**。靠 inspect / 凝脉+ 境界感知才能识别为 portal "
              "锚点（worldview §十六'看似普通的裂缝'原文）。"
              "实际 portal 触发坐标由 zone.extras.portal_anchor_xz 给出，"
              "本装饰只是周边视觉氛围，**不是触发器**。",
    ),
)

# 注：原草稿曾有 `rift_mouth_marker`（obsidian + soul_lantern + respawn_anchor 显眼地标）——
# 违反 worldview §十六"看似普通的裂缝...靠近时感知负压异常的修士才认得出"原文：
# 显眼地标会让任何路过的玩家直接看见 portal 位置，破坏"必须感知才能找到入口"的核心机制。
# 自查修订删除——portal 锚点不靠装饰物表达，仅靠 zone.extras.portal_anchor_xz 坐标 + cracked_floor_seam 周边氛围。
```

`ambient_effects = ("frost_breath", "distant_void_hum", "cold_wind")`——气息凝白 + 极远虚空嗡鸣 + 冷风。

## §4 完整 profile 配置

### `terrain-profiles.example.json` 追加

```json
"rift_mouth_barrens": {
  "height": { "base": [60, 80], "peak": 88 },
  "boundary": { "mode": "hard", "width": 48 },
  "surface": ["blackstone", "obsidian", "tuff", "coarse_dirt", "packed_ice"],
  "water": { "level": "none", "coverage": 0.0 },
  "passability": "medium",
  "core_radius": 30,
  "outer_radius": 150,
  "portal_layer": "portal_anchor_sdf"
}
```

### Blueprint zone 模板（首版 4 处全固定 zone）

```json
{
  "name": "rift_mouth_<id>",
  "display_name": "渊口荒丘",
  "aabb": { "min": [<cx-150>, 50, <cz-150>], "max": [<cx+150>, 100, <cz+150>] },
  "size_xz": [300, 300],
  "spirit_qi": 0.05,
  "danger_level": 7,
  "worldgen": {
    "terrain_profile": "rift_mouth_barrens",
    "shape": "circular",
    "boundary": { "mode": "hard", "width": 48 },
    "extras": {
      "portal_anchor_xz": [<cx>, <cz>],   // **portal 触发坐标，不显示在地图上**
      "tsy_zone_link": "tsy_zongmen_ruin_<id>" /* 或对应 TSY zone */
    },
    "landmarks": ["spacetime_scar"]   // 仅极稀有装饰，非入口标识
  }
}
```

候选位（首版 4 处地表暴露态渊口，与 §十六.一 "形成途径"对应）：
- `rift_mouth_north_001`：(-500, -8500)（北荒，对应 §十六 "上古大能陨落"——枯木崖原型；portal_anchor 在 (-500, -8500)）
- `rift_mouth_north_002`：(2000, -7800)（北荒东陲，对应"近代高手战死"——也是 plan-terrain-tribulation-scorch-v1 化虚遗迹邻接区，注意 zone 互斥不重叠）
- `rift_mouth_blood_001`：(3200, -2800)（血谷东侧地下露头，对应 geography-0002 "血谷天劫多发"暗示）
- `rift_mouth_west_001`：(-3500, 5500)（西南远端，对应"上古宗门遗迹浮现"）

> **草稿曾有 5 处含 1 处 transient zone (`rift_mouth_drift_001`)**——transient + 固定混搭增加双倍系统复杂度。自查修订改为**首版全固定 4 处**，transient（天道动态生成的 portal 锚点）留 v2，由 plan-tsy-zone-followup-v1 接管。
>
> **cave_network / abyssal_maze 内部入口（worldview §十六 "洞穴尽头异常寒气" / "幽暗地穴深处新鲜崩石"）不在本 plan 范围**——由那些 profile 通过 `anomaly_kind=1 (spacetime_rift)` + 局部 `neg_pressure` 表达，是 plan-tsy-zone-followup-v1 的扩展项。

### 数值梯度（按距离中心 r / core_radius 归一化的 `t`）

| 区位 | t | qi_density | mofa_decay | qi_vein_flow | flora_density | neg_pressure |
|---|---|---|---|---|---|---|
| 渊心 | 0-0.3 | 0.00 | 0.95 | 0 | 0 | **-0.8** |
| 焦土圈 | 0.3-0.7 | 0.02 | 0.85 | 0 | 0.20（仅干尸/焦碑）| -0.4 |
| 寒气圈 | 0.7-1.0 | 0.05 | 0.65 | 0 | 0.40 | -0.15 |
| 外缘渐变 | 1.0-2.0 | 0.08 | 0.50 | 0 | 0.30 | 0 |

`neg_pressure` 是 LAYER_REGISTRY 已有层（`safe_default=0.0, blend_mode="maximum"`）——本 profile 是除"abyssal_maze 深层"外**第二个写入 neg_pressure 的 profile**。

## §5 LAYER_REGISTRY 字段映射

```python
extra_layers = (
    "qi_density",
    "mofa_decay",
    "qi_vein_flow",
    "flora_density",
    "flora_variant_id",
    "neg_pressure",         # 已存在层，本 profile 主要写入者
    "portal_anchor_sdf",    # **本 plan 提议新增层**：到最近 portal anchor 的欧氏距离场
    "anomaly_intensity",    # 玩家进入触发负灵 tick
    "anomaly_kind",         # 1 = spacetime_rift
)
```

**`portal_anchor_sdf` 必须新增独立层，不复用 `rift_axis_sdf`**——这是自查修订的关键决策：

- `rift_valley.py:225` 把 `rift_axis_sdf` 写为 `normalized_cross`（值域 [-1.5, 1.5] 的归一化横向距离 + safe_default=99.0 + minimum blend）；
- 本 plan 需要的是**到最近 portal anchor 的欧氏距离场**（值域 [0, +∞) 米单位）；
- 二者**值域和单位完全不同**。如果复用同一字段，rift_valley 与 rift_mouth zone 邻接时 stitcher minimum blend 会**取较小值**——rift_valley 边缘 normalized_cross=1.0 比任何 rift_mouth 数十米的欧氏距离都小，结果整片血谷边缘都被错误识别为 portal 锚点。
- 反之亦然：rift_mouth 中心 sdf=0 会污染血谷沿轴的 SDF 语义。

**新增 LAYER_REGISTRY 条目（本 plan P0 阶段提议）**：

```python
"portal_anchor_sdf":  LayerSpec(safe_default=999.0, blend_mode="minimum", export_type="float32"),
```

safe_default=999.0（远大于任何实际距离）+ minimum blend（多 zone 写入取最近的 portal）+ float32（精度足够米级距离场）。Rust 消费时通过该字段读取每列到最近 portal 的距离，hot-path mask `< 50` 决定是否触发境界感知 narration / 是否在 ±2 格内触发 portal 跳转。

## §6 数据契约（下游 grep 抓手）

| 阶段 | 抓手 | 位置 |
|---|---|---|
| P0 | `rift_mouth_barrens` profile + 4 zone（全固定）| `worldgen/terrain-profiles.example.json` + blueprint |
| P0 | LAYER_REGISTRY 新增 `portal_anchor_sdf: LayerSpec(999.0, "minimum", "float32")` | `worldgen/scripts/terrain_gen/fields.py:LAYER_REGISTRY` |
| P0 | raster_export 加 `portal_anchor_sdf` 到 manifest（与 `tsy_origin_id` 等同列） | `worldgen/scripts/terrain_gen/exporters.py` |
| P1 | `class RiftMouthBarrensGenerator` + `fill_rift_mouth_barrens_tile` | `worldgen/scripts/terrain_gen/profiles/rift_mouth_barrens.py`（新增） |
| P1 | `RIFT_MOUTH_DECORATIONS` 7 项（charred_obelisk_shard / frost_qi_cluster / ganshi_drift / fresh_collapse_rubble / spacetime_scar / dao_zhuang_corpse_pose / cracked_floor_seam）| 同上 |
| P1 | `portal_anchor_sdf` 写入：每 tile 计算到 zone.extras.portal_anchor_xz 的欧氏距离 | 同上 |
| P2 | `struct NegPressureField { center, max_pull, falloff }` zone-scoped | `server/src/cultivation/neg_pressure.rs`（新增） |
| P2 | `qi_drain_per_sec(realm) → f32` 抽吸公式 | `server/src/cultivation/cultivation.rs::tick_neg_pressure` |
| P3 | `RiftPortalEntity` + 触发逻辑 → call into TSY dim transfer | `server/src/portal/rift_portal.rs`（新增） |
| P3 | portal 触发条件：玩家位置 = zone.extras.portal_anchor_xz ±2 格（**不是踩 marker block**）| `server/src/portal/rift_portal.rs::check_anchor_proximity` |
| P3 | `tsy_zone_link` 字段消费 → portal 跳转目标查询 | `server/src/world/zone.rs::resolve_rift_link` |
| P3 | 道伥外溢 spawn rule（联动 TSY 塌缩 event） | `server/src/mob/dao_zhuang.rs::spawn_on_rift_collapse` |

## §7 实施节点

- [ ] **P0** profile + 4 zone（全固定）+ LAYER_REGISTRY 新增 `portal_anchor_sdf` — 验收：raster manifest 含全部 4 zone + `portal_anchor_sdf` 字段；fields.py LAYER_REGISTRY 新条目通过 raster_check 验证；与 rift_valley zone 邻接处 `rift_axis_sdf` / `portal_anchor_sdf` 互不污染（pin 测试：取一个 rift_valley + rift_mouth 邻接的 tile，验证两个字段语义独立）
- [ ] **P1** generator + 装饰物 — 验收：raster 中心 `qi_density.max() < 0.01`；`neg_pressure` 中心达 0.8（注：safe_default=0.0 + maximum blend，"-0.8 灵压"语义在 server tick 把 neg_pressure 解释为负值，raster 字段本身存储正值）；7 种装饰各自命中；`portal_anchor_sdf` 在 anchor 中心格 < 1.0
- [ ] **P2** 负灵 tick + 寒气视觉 — 验收：固元玩家进入 30 格 → 真元 -10/sec 命中（±10%）；客户端 frost_breath 粒子可见；HUD 显示 -0.8 灵压
- [ ] **P3** portal 跳转 + 道伥外溢 — 验收：玩家位于 zone.extras.portal_anchor_xz ±2 格 → 跳 TSY 位面成功（**不依赖任何 marker block**）；TSY 位面塌缩 event 在 30s 内触发主世界 zone spawn 1+ 个道伥

## §8 开放问题

- [ ] portal anchor 的 transient 行为：草稿曾有 `rift_mouth_drift_001` transient zone，本版**首版全固定 4 处**；transient（天道动态生成 portal）留 v2，由 plan-tsy-zone-followup-v1 接管"坍缩渊新生 → 主世界生成对应 rift_mouth zone"双向 link
- [ ] cave_network / abyssal_maze 内部入口（"洞穴尽头寒气"/"地穴深处崩石"）的接入方式——本 plan 不做，留 plan-tsy-zone-followup-v1 处理；建议在那些 profile 加 `anomaly_kind=1 (spacetime_rift)` + 局部 `neg_pressure` hot-spot，不必再开独立 zone
- [ ] 渊口外溢的道伥是否要继承 TSY 位面内死者的 origin / 流派？倾向 **是**（叙事一致性），但要做到 TSY death event payload 携带 player_realm + style 信息
- [ ] 寒气晶簇是否可挖（packed_ice / blue_ice 是 vanilla 可破坏方块）？倾向 **挖了 3 秒后融化**（不掉落，避免玩家 farm "免费灵气结晶"）
- [ ] 道伥外溢会不会让玩家把 rift_mouth 当作刷怪场（farm 道伥残骨当暗器载体——见 plan-anqi-v1）？建议加 cooldown：单 zone 24 小时内最多 spawn N 个道伥
- [ ] portal 跳转的 race-out（worldview §十六.七 塌缩裂口）落点——是回到主裂缝对应的 rift_mouth zone，还是随机落在主世界其他 rift_mouth？需与 plan-tsy-extract-v1 对齐
- [ ] portal 入口"看似普通"原则下，**新手玩家**几乎无法找到入口（醒灵/引气感知不到 -0.8 负压）——这是设计意图（高境界专属机会）还是过度门槛？倾向**前者**——worldview §十六 明文"低阶进不了浅层（高阶秒）→ 只能下深层淘金"，新手能进坍缩渊本身就是设计的反直觉点

## §9 进度日志

- 2026-04-28：骨架立项。锚定 worldview §十六 实现注 + world-0001 / geography-0004 描写。等优先级排序与 plan-tsy-dimension-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1（道伥 spawn）协调，本 plan 是 TSY 闭环的"地表 missing piece"。
- 2026-04-28（自查修订）：
  - **strong-1** 修：`rift_axis_sdf` **不**复用为 portal 距离场——新增独立 LAYER_REGISTRY 层 `portal_anchor_sdf: LayerSpec(999.0, "minimum", "float32")`。原 rift_valley 写归一化横向距离 [-1.5, 1.5]，本 plan 需欧氏距离 [0, +∞)，值域不同；复用会让 minimum blend 在两个 zone 邻接处把血谷边缘错误识别为 portal 锚点（反之亦然）。新增层是**结构性修订**，P0 阶段必须落地。
  - **strong-2** 修：删除显眼 `rift_mouth_marker` 装饰（obsidian + soul_lantern + respawn_anchor）——违反 worldview §十六"看似普通的裂缝...靠近时感知负压异常的修士才认得出"原文。改为不起眼 `cracked_floor_seam`（cobblestone + stone + tuff，与普通裂缝外观一致）作为周边视觉氛围；portal 触发坐标改为 `zone.extras.portal_anchor_xz` **坐标级显式给出**，不依赖任何 marker block——保留"必须感知才能找到入口"的核心机制。
  - **mid-9** 修：删除 chill / collapse 两种 rift_variant——worldview §十六 描述这两种入口在洞穴/地穴**内部**，做独立地表 zone 矛盾。本 profile 收窄为只覆盖"地表暴露态"渊口（worldview §十六 三种入口形态中的 (a) 地壳裂缝）；(b)(c) 留给 plan-tsy-zone-followup-v1 在 cave_network / abyssal_maze 内部加 `anomaly_kind=1` hot-spot 表达。
  - **weak-15** 修：草稿 5 zone 含 1 transient (`rift_mouth_drift_001`) → 简化为**首版全固定 4 处**；transient 留 v2 由 plan-tsy-zone-followup-v1 接管。
- **2026-04-29**：实地核验 + 决策标注（**保留骨架**，不升 active——P0 工程产出全缺：LAYER_REGISTRY 未加 `portal_anchor_sdf` / ColumnSample 缺字段 / profile JSON 无；需先动 P0 worldgen 代码再升）。
  - **季节联动**（用户决策 2026-04-29）：渊口寒气 = 冬季 ×1.3 / 夏季 ×0.7（worldview §十七 锚定）。已写入头部锚点 + §0 设计轴心。霜骨苔等冬季限定物种联动 plan-botany-v2 SeasonRequired。
  - 道伥继承 / cooldown / 寒气晶簇融化等 §8 开放问题保留待 P2/P3 实施时与 plan-tsy-* 系列共同决议。
  - 补 `## Finish Evidence` 占位。
  - 升 active 触发条件：（a）`worldgen/scripts/terrain_gen/fields.py` LAYER_REGISTRY 加 `portal_anchor_sdf`；（b）`server/src/world/terrain/raster.rs` ColumnSample 加字段；（c）`worldgen/terrain-profiles.example.json` 加 `rift_mouth_barrens` profile + 4 zone JSON。三件 done 后再升。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：`fields.py` LAYER_REGISTRY 加 `portal_anchor_sdf` + ColumnSample 字段 + `terrain-profiles.example.json` 加 `rift_mouth_barrens` profile + 4 zone JSON
  - P1：`worldgen/scripts/terrain_gen/profiles/rift_mouth_barrens.py`（generator + 7 装饰物）
  - P2：`server/src/cultivation/neg_pressure.rs`（NegPressureField）+ 客户端 frost_breath 粒子
  - P3：`server/src/portal/rift_portal.rs`（RiftPortalEntity + TSY 跳转 + 道伥外溢 spawn）
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - worldgen：`portal_anchor_sdf` LAYER_REGISTRY / `RiftMouthBarrensGenerator` / 4 zone
  - server：`NegPressureField` / `RiftPortalEntity` / 道伥 spawn weight ×5
  - client：frost_breath 粒子 / HUD -0.8 灵压显示
- 遗留 / 后续：
  - transient 渊口（v2，依 plan-tsy-zone-followup-v1）
  - cave_network / abyssal_maze 内部入口（plan-tsy-zone-followup-v1 接管）
  - 道伥继承 origin / 流派（依 plan-tsy-hostile-v1 death event payload）
  - 道伥外溢 cooldown（24h 单 zone N 个上限）
