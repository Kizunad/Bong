# Bong · plan-terrain-rift-mouth-v1 · Finished

> **状态**：✅ finished（2026-05-04 consume-plan 完成并归档）。核心 v1 已落地：主世界渊口 profile / 4 固定渊口 zone / portal_anchor_sdf / 负灵压抽吸 + frost_breath / 塌缩裂口随机甩回渊口 / HUD -0.8 灵压显示。派生项见 Finish Evidence 遗留。

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
- P0 ✅ profile 注册 + 4 处固定渊口 zone + `portal_anchor_sdf`
- P1 ✅ `RiftMouthBarrensGenerator` 实装（地形 field + 7 装饰物 + 焦黑石）
- P2 ✅ 局部负灵 hot-spot tick（30 步内 -0.5 ~ -0.8 抽吸）+ `frost_breath` 寒气视觉 + HUD 负灵压
- P3 ✅ 主世界入口通过 rift_mouth `rift_portal` POI 接入既有 TSY portal；塌缩裂口撤离随机甩回主世界渊口 zone；道伥外溢沿用既有 TSY lifecycle

---

## §0 设计轴心

- [x] **不在主世界占体积**：worldview §十六 明确"裂缝本身不占主世界坐标体积"——`rift_mouth_barrens` 不写 cave_mask；地表只画 portal 锚点伤痕，入口由既有 TSY portal entity 接管
- [x] **小尺度高密度**：4 个固定渊口 zone 均为 300×300，中心 30 格通过 `portal_anchor_sdf` + `neg_pressure` 形成陡降 hot-spot
- [x] **本 profile 仅覆盖"地表暴露态"渊口**：主 profile 只做地表暴露态；Q-R1 额外在 `cave_network` / `abyssal_maze` 内写局部 hot-spot 字段，不新开独立 zone
- [x] **portal 入口看似普通**：地表只保留 `cracked_floor_seam` / `spacetime_scar` 等氛围装饰；入口位置由 `pois.kind=rift_portal` + `portal_anchor_sdf` 表达，不放显眼 marker block
- [x] **道伥外溢 spawn pool**：渊口 zone 写 `dao_zhuang_spawn_weight_multiplier=5`；运行时外溢沿用 `tsy_lifecycle` 的干尸转道伥与塌缩喷出机制，冷却配置留后续细化
- [x] **不可灵龛**：v1 已通过 -0.8 hot-spot 表达禁龛语义；龛石放置 runtime gate 留后续接 `shrine`/social 放置链
- [x] **季节响应**（worldview §十七）：季节倍率作为世界观与数值锚点保留；v1 runtime 固定 base 抽吸，季节倍率留后续接 season 系统

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

- [x] **P0** profile + 4 zone（全固定）+ LAYER_REGISTRY 新增 `portal_anchor_sdf` — 验收：raster manifest 含 `portal_anchor_sdf`；4 个 `rift_mouth_*` zone 含 entry POI；`rift_axis_sdf` / `portal_anchor_sdf` 独立测试已覆盖
- [x] **P1** generator + 装饰物 — 验收：raster 中心 `qi_density` 归零、`neg_pressure` 中心达 0.8、7 种装饰各自命中、`portal_anchor_sdf` 在 anchor 中心格 < 1.0
- [x] **P2** 负灵 tick + 寒气视觉 — 验收：`tick_neg_pressure` 按境界抽吸真元并发 `bong:frost_breath`；client 注册 `FrostBreathPlayer`；HUD / 修炼界面显示 `-0.8` 局部灵压
- [x] **P3** portal 跳转 + 塌缩落点 — 验收：4 个渊口 zone 的 `rift_portal` POI 接入既有 `spawn_rift_portals` / `tsy_portal`；`CollapseTear` 撤离完成随机选主世界 `rift_mouth_*` zone 落点；道伥外溢沿用既有 `tsy_lifecycle`，冷却与继承细节留 follow-up

## §8 开放问题

- [x] **Q-R7 ✅**（user 2026-05-04 确认）：portal anchor transient 行为留 v2（plan-tsy-zone-followup-v1）。本 plan 全固定 4 处。
- [x] **Q-R1 ✅**（user 2026-05-04 B）：cave_network / abyssal_maze 内部入口 **本 plan 顺手在 cave 加 anomaly_kind=1 hot-spot**（不留 followup）。具体：cave_network 深 5+ 层随机 1-3 hot-spot（带 neg_pressure 局部峰值 + portal_anchor_sdf < 30）；abyssal_maze 第 3 层固定 1 个。**不开独立 zone**（仅 hot-spot 字段写入）。详 §3 新增机制。
- [x] **Q-R5 ✅**（user 2026-05-04 是）：渊口外溢道伥继承 TSY 位面内死者的 origin / 流派。前置：plan-tsy-hostile-v1 / plan-tsy-lifecycle-v1 的 death event payload 必须携带 `{ player_realm, style, qi_color_main }` —— 已部分实装（CorpseEmbalmed 已含 origin），P3 实施时核验 schema 完整性。
- [x] **Q-R3 ✅**（user 2026-05-04 改原"融化不掉落"）：寒气晶簇**可挖 + 需特殊容器保存**。新增 item `frost_keeper_jar`（凝灵罐，凝脉+ 制作 / 灵泉宗丹宗工艺）：装入后冰晶不融化；裸采 3 秒后融化（不掉落）。罐子配方走 plan-alchemy-v1 / plan-forge-v1 体系（**P2 决策**）。
- [x] **Q-R4 ✅**（user 2026-05-04 默认配置）：道伥外溢 cooldown **默认 N=5 道伥 / zone / 24h**（4 zone × 5 = 20 全图上限可接受）；后续可调（暴露为 config）。
- [x] **Q-R6 ✅**（user 2026-05-04 随机）：portal race-out（worldview §十六.七 塌缩裂口）落点 = 主世界**随机** rift_mouth zone。撤离玩家不知道会被甩到哪——加强"末法残土的不可控感"。需与 plan-tsy-extract-v1 联动。
- [x] **Q-R2 ✅**（user 2026-05-04 A + NPC 钩）：portal 入口"看似普通" → 新手 100% 找不到，**是设计意图**（worldview §十六"高阶秒 / 低阶进不了浅层"）。**新增 NPC 信息传递钩**（user 2026-05-04）：散修 NPC narration 偶发提及"听说北方某处寒气异常"——给新手"听说但不指路"的引导（不做地图标记），玩家自己摸索；P1+ 由 plan-narrative-v1 / plan-social-v1 narration template 实装。

## §9 进度日志

- 2026-04-28：骨架立项。锚定 worldview §十六 实现注 + world-0001 / geography-0004 描写。等优先级排序与 plan-tsy-dimension-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1（道伥 spawn）协调，本 plan 是 TSY 闭环的"地表 missing piece"。
- 2026-04-28（自查修订）：
  - **strong-1** 修：`rift_axis_sdf` **不**复用为 portal 距离场——新增独立 LAYER_REGISTRY 层 `portal_anchor_sdf: LayerSpec(999.0, "minimum", "float32")`。原 rift_valley 写归一化横向距离 [-1.5, 1.5]，本 plan 需欧氏距离 [0, +∞)，值域不同；复用会让 minimum blend 在两个 zone 邻接处把血谷边缘错误识别为 portal 锚点（反之亦然）。新增层是**结构性修订**，P0 阶段必须落地。
  - **strong-2** 修：删除显眼 `rift_mouth_marker` 装饰（obsidian + soul_lantern + respawn_anchor）——违反 worldview §十六"看似普通的裂缝...靠近时感知负压异常的修士才认得出"原文。改为不起眼 `cracked_floor_seam`（cobblestone + stone + tuff，与普通裂缝外观一致）作为周边视觉氛围；portal 触发坐标改为 `zone.extras.portal_anchor_xz` **坐标级显式给出**，不依赖任何 marker block——保留"必须感知才能找到入口"的核心机制。
  - **mid-9** 修：删除 chill / collapse 两种 rift_variant——worldview §十六 描述这两种入口在洞穴/地穴**内部**，做独立地表 zone 矛盾。本 profile 收窄为只覆盖"地表暴露态"渊口（worldview §十六 三种入口形态中的 (a) 地壳裂缝）；(b)(c) 留给 plan-tsy-zone-followup-v1 在 cave_network / abyssal_maze 内部加 `anomaly_kind=1` hot-spot 表达。
  - **weak-15** 修：草稿 5 zone 含 1 transient (`rift_mouth_drift_001`) → 简化为**首版全固定 4 处**；transient 留 v2 由 plan-tsy-zone-followup-v1 接管。
- **2026-04-29**：实地核验 + 决策标注（**保留骨架**，不升 active——P0 工程产出全缺：LAYER_REGISTRY 未加 `portal_anchor_sdf` / ColumnSample 缺字段 / profile JSON 无；需先动 P0 worldgen 代码再升）。
- **2026-05-04**：skeleton → active 升级（user 拍板覆盖 04-29 倒挂 block）。"先做 P0 再升"是倒挂条件——升 active 是开 P0 的入口，不是产出。前置 plan-tsy-dimension-v1 / plan-tsy-extract-v1 / plan-tsy-hostile-v1 全 ✅ finished。下一步起 P0 worktree（`portal_anchor_sdf` LAYER_REGISTRY 层 + ColumnSample 字段 + `rift_mouth_barrens` profile + 4 zone JSON）。
- **2026-05-04**：§8 全部 7 决策闭环（Q-R1/R2/R3/R4/R5/R6/R7 详 §8）。范围扩展：
  - Q-R1 决策"本 plan 顺手做 cave/abyssal hot-spot"——P0 范围扩到 cave_network / abyssal_maze 字段写入（**P0 抓手新增**）
  - Q-R3 寒气晶簇可挖 + 凝灵罐 `frost_keeper_jar` 新增 item（P2 跨 plan：与 alchemy / forge 协调）
  - Q-R6 portal race-out 落点随机（与 plan-tsy-extract-v1 P3 联动）
  - Q-R2 NPC 信息传递钩（P1+ 与 plan-narrative-v1 / plan-social-v1 联动）
  - **季节联动**（用户决策 2026-04-29）：渊口寒气 = 冬季 ×1.3 / 夏季 ×0.7（worldview §十七 锚定）。已写入头部锚点 + §0 设计轴心。霜骨苔等冬季限定物种联动 plan-botany-v2 SeasonRequired。
  - 道伥继承 / cooldown / 寒气晶簇融化等 §8 开放问题保留待 P2/P3 实施时与 plan-tsy-* 系列共同决议。
  - 补 `## Finish Evidence` 占位。
  - 升 active 触发条件：（a）`worldgen/scripts/terrain_gen/fields.py` LAYER_REGISTRY 加 `portal_anchor_sdf`；（b）`server/src/world/terrain/raster.rs` ColumnSample 加字段；（c）`worldgen/terrain-profiles.example.json` 加 `rift_mouth_barrens` profile + 4 zone JSON。三件 done 后再升。
- **2026-05-04**：consume-plan 完成。核心 v1 落地并通过 worldgen / server / client 三栈验证；active plan 迁入 `docs/finished_plans/`。

---

## Finish Evidence

- 落地清单：
  - P0：`worldgen/scripts/terrain_gen/fields.py` 新增 `portal_anchor_sdf: LayerSpec(999.0, "minimum", "float32")`；raster exporter / stitcher / server `ColumnSample` 支持该字段；`terrain-profiles.example.json` 注册 `rift_mouth_barrens`；`server/zones.worldview.example.json` 增加 4 个固定 `rift_mouth_*` zone 与 entry POI
  - P1：新增 `worldgen/scripts/terrain_gen/profiles/rift_mouth_barrens.py`，实现 `RiftMouthBarrensGenerator`、`fill_rift_mouth_barrens_tile`、7 个 `RIFT_MOUTH_DECORATIONS`、中心 `neg_pressure=0.8` 与独立 `portal_anchor_sdf`
  - P1 补充：`cave_network` / `abyssal_maze` 内部写入 `anomaly_kind=1`、`neg_pressure`、`portal_anchor_sdf` hot-spot，不新开 cave/abyssal 独立 zone
  - P2：新增 `server/src/cultivation/neg_pressure.rs`，按 `neg_pressure + portal_anchor_sdf` 对玩家真元抽吸并发送 `bong:frost_breath` VFX；client 新增 `FrostBreathPlayer` 并在 registry 注册
  - P2 HUD：server `player_state` payload 增加 `local_neg_pressure`；client `PlayerStateViewModel` / `PlayerStateHandler` / `CultivationScreen` / `BongHudOrchestrator` 显示 `局部灵压: -0.80` 与 HUD `灵压 -0.80`
  - P3：`server/src/world/extract_system.rs` 让 `CollapseTear` 完成撤离时从主世界 `rift_mouth_*` zone 中确定性随机选落点；普通出口继续回 `TsyPresence.return_to`
- 关键 commit：
  - `e846c8dd` `feat(worldgen): 增加渊口荒丘地形 profile`
  - `b7e90385` `feat(server): 接入渊口负灵压与塌缩落点`
  - `1a695abc` `feat(client): 注册渊口寒气粒子`
  - `9191df72` `feat(hud): 显示渊口局部负灵压`
- 测试结果：
  - worldgen：`python3 -m pytest` → `66 passed in 19.91s`
  - server：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → `2207 passed`
  - client：`JAVA_HOME="/usr/lib/jvm/java-17-openjdk-amd64" PATH="/usr/lib/jvm/java-17-openjdk-amd64/bin:$PATH" ./gradlew test build` → `BUILD SUCCESSFUL`
- 跨仓库核验：
  - worldgen：`portal_anchor_sdf` LAYER_REGISTRY / `RiftMouthBarrensGenerator` / 4 zone POI / cave+abyssal hot-spot / manifest `semantic_layers`
  - server：`ColumnSample.portal_anchor_sdf` / `tick_neg_pressure` / `CollapseTear` rift_mouth 落点 / `local_neg_pressure` payload
  - client：`FrostBreathPlayer` / VFX registry / HUD 与修炼界面 -0.8 灵压显示
- 遗留 / 后续：
  - transient 渊口（v2，依 plan-tsy-zone-followup-v1）
  - 寒气晶簇采集、裸采融化、`frost_keeper_jar` 凝灵罐配方（依 plan-alchemy-v1 / plan-forge-v1）
  - 散修 NPC 信息传递钩（依 plan-narrative-v1 / plan-social-v1）
  - 季节倍率 runtime 接入（冬季 ×1.3 / 夏季 ×0.7 / 汐转 RNG）
  - 龛石放置 runtime gate（渊口 30 格内吞龛 / 拒绝）
  - 道伥外溢 cooldown 与 origin / 流派继承细化（依 plan-tsy-hostile-v1 / plan-tsy-lifecycle-v1）
