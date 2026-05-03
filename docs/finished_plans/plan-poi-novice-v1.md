# Bong · plan-poi-novice-v1

P1 引气期玩家活动范围（spawn ± 1500 格）**完全动态注入** 6 类新手 POI（Point of Interest = 地图上有功能/叙事意义的位置）——让玩家不必远跑就能完成"第一次炼器/炼丹/社交/战斗/采集/拾取知识"。

**Primary Axis**：**5 大玩法首次触发的步行可达性**（玩家从 spawn 出发 5 分钟步行内可见任一 POI 的概率）

## 阶段总览

| 阶段 | 状态 | 验收 |
|---|---|---|
| **P0** 6 类 POI 的 worldgen 动态选址 + 灵气浓度匹配（Q108: C / Q112: C）| ⬜ | 玩家从 spawn 出发 5 分钟内可见任一 POI |
| **P1** POI 内的 entity / loot 表（散修 NPC / 异变兽 / 残卷 / 灵草）+ 屠村信誉度 stub + 残卷一周刷新 + 兽巢 24h 刷新 | ⬜ | 各 POI 内容正常 drop / 交互 |
| P2 朽坏视觉（残灰、断壁、骨架方块组合）+ v1 收口 | ⬜ | 美术风格统一 |

> **vN+1 (plan-poi-novice-v2)**：更多 POI / 子区域细分 / 多种朽坏纹饰 / 散修 NPC 个性化对话 / POI 联动事件（散修被屠引发周边事件）

---

## 世界观 / library / 交叉引用

**worldview 锚点**：
- §十三 初醒原（spawn 灵气 0.3）
- §十三 青云残峰（broken_peaks 外围 0.4-0.5 灵气过渡区）
- §十一 安全与社交 / 身份与信誉（commit fe00532c 已正典化）— Q109 屠村信誉度接入

**library 锚点**：
- `peoples-0007 散修百态`（散修聚居形态 / 茅屋 / 死信箱）

**交叉引用**：
- `plan-spawn-tutorial-v1` ✅（2026-05-03 升 active，commit 521e3a81）— P0 前置；玩家完成醒灵→引气突破后进入 P1，本 plan 提供 P1 玩法 POI
- `plan-worldgen-v3.1` ✅（已落地）— `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` + `broken_peaks.py` POI hooks 扩展
- `plan-forge-leftovers-v1` ⬜（still skeleton，P1 必需）— 破败炼器台 client UI；本 plan 仅注入 forge Station POI，UI 走 forge-leftovers
- `plan-alchemy-client-v1` ⏳（部分实装）— 凡铁丹炉 client UI；本 plan 仅注入 alchemy Furnace POI
- `plan-fauna-v1` ⬜（still skeleton，P1 必需）— 异变兽实体；v1 用 zombie 占位 + 改难度参数
- `plan-baomai-v1` ✅（灵龛战 / 散修战斗）— 散修 NPC 走 baomai 战斗系统
- `plan-identity-v1` ⬜（**未立 plan，DEF 之一**）— Q109 屠村信誉度 vN+1 接入；v1 仅 stub
- `plan-narrative-v1` ⏳（部分实装）— 屠村 narration / 残卷拾取 narration
- `plan-tsy-loot-v1` ✅（已落地）— 残卷藏匿点的残卷接入 loot 系统
- `plan-gameplay-journey-v1` 🟡（still skeleton）— §P1 引气期玩家旅程

## 接入面 checklist（防孤岛 — 严格按 docs/CLAUDE.md §二）

- **进料**：spawn_plain + broken_peaks 现有 worldgen ✅ + plan-spawn-tutorial-v1 完成的初始环境 + 灵气浓度数据（zone.spirit_qi）+ 地形约束（坡度 / 水域避让）
- **出料**：spawn ± 1500 格内**完全动态选址**生成 6 类 POI（Q108: C），每个 POI 自带 entity / loot / 灵气浓度匹配 / 朽坏视觉
- **共享类型 / event**：复用 worldgen blueprint POI 体系 + `npc::scenario` ✅ NPC 聚集 + plan-tsy-loot 残卷系统；新增 `worldgen::poi_novice::PoiNoviceSelector` / `world::poi_novice::TrespassEvent`（屠村事件，stub）
- **跨仓库契约**：
  - server: `world::poi_novice::PoiNoviceLoader` system / `world::poi_novice::TrespassEvent` event（屠村触发）/ `world::poi_novice::respawn_tick`（异变兽 24h 刷新 + 残卷一周刷新）
  - worldgen: `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` + `broken_peaks.py` 扩展 POI hooks；新增 `worldgen/scripts/poi_novice_selector.py`（动态选址算法）；6 个 blueprint json `worldgen/blueprints/poi_novice/*.json`
  - schema: `agent/packages/schema/src/poi_novice.ts` → `TrespassEventV1` / `PoiSpawnedEventV1`（agent narration 触发）
  - client: 复用现有 forge / alchemy / NPC interaction 路径；新增 `bong:world/poi_state` (outbound) HUD 不显示（仅 LifeRecord 用）
- **沉默引导原则**（plan-spawn-tutorial-v1 沿用）：v1 严格无 UI 标记 / 无任务面板；POI 自然存在于地图上，玩家自己发现

---

## §A 概览（设计导航）

> P1 引气期玩家从 spawn 出发，**5 分钟步行**内必能见到至少一处 POI——破败炼器台/凡铁丹炉/散修聚居点/异变兽巢/残卷藏匿点/灵草谷。**完全动态选址**（Q108: C）按地形 + 灵气浓度 + 最小间距 1000 格算出 6 处。POI **一直存在**（Q113: B），玩家任何时刻可发现，但醒灵期用不上（功能门槛在引气+）。

### A.0 v1 实装范围（2026-05-03 拍板）

| 维度 | v1 实装 | 搁置 vN+1 |
|---|---|---|
| POI 数量 | **6 类**（破败炼器台 / 凡铁丹炉 / 散修聚居点 / 异变兽巢 / 残卷藏匿点 / 灵草谷）| 更多 POI / 子区域细分 |
| 选址 | **完全动态**（Q108: C；worldgen 按地形 + 灵气浓度 + 最小间距 1000 格算）| 半固定 / 玩家偏好驱动 |
| 服务半径 | **spawn ± 1500 格** | 范围调整 |
| 屠村惩罚 | **1 周 NPC 拒绝交易**（Q109: B；信誉度系统 stub，vN+1 接 plan-identity-v1）| 永久惩罚 / 全服 narration |
| 残卷刷新 | **一周一刷 real-time**（Q110: D）| 玩家专属 / 限一份独占 |
| 异变兽巢刷新 | **24h server 内时间**（Q111: A）| 击杀重生 / 玩家附近抑制 |
| 灵气匹配 | **POI 必须建在合适灵气区**（Q112: C；worldgen 选址要求）| POI 改造周围灵气 |
| 衔接 spawn-tutorial | **POI 一直存在**（Q113: B；醒灵期可见但用不上）| 突破后才显现 / 渐进解锁 |
| 异变兽巢难度 | **高难度**（Q114: C；凝脉+ 才能稳定单杀，引气期需协作）| 难度分级 / 渐进解锁 |
| 朽坏视觉 | 残灰 + 断壁 + 骨架方块组合 | 多种朽坏纹饰文化差异 |

### A.1 6 类 POI 规格（v1 P0/P1）

| POI 类型 | 服务玩法 | 选址灵气要求 | 必备 entity / loot | 刷新规则 |
|---|---|---|---|---|
| 破败炼器台 | 第一次炼器 | spirit_qi 0.4-0.6 | forge Station ✅（损坏，效率减半 0.5×）| 永久（不刷新，玩家修复后效率回 1.0×；vN+1 引入修复机制）|
| 凡铁丹炉 | 第一次炼丹 | spirit_qi 0.3-0.5 | alchemy Furnace ✅ + 凡铁锅 + 篝火 | 永久 |
| 散修聚居点 | 第一次社交 / 交易 | spirit_qi 0.4-0.6 | 2-3 名 Rogue NPC + 茅屋 + 死信箱 ✅ | NPC 死亡后 24h 重生（Q111 同期）；屠村触发 TrespassEvent → 1 周拒绝交易（Q109）|
| 异变兽巢 | 第一次猎兽核 | spirit_qi 0.5-0.7（高灵气吸引异变）| 缝合兽 + 灰烬蛛 nest（v1 用 zombie 占位 + 改 AI / HP）| 24h server 内时间刷新（Q111: A）；难度 = 凝脉+ 单杀（Q114: C）|
| 残卷藏匿点 | 第一次拾取知识 | spirit_qi 0.3-0.5 | cave_network 入口 + 1-2 张随机残卷 | 一周 real-time 刷新（Q110: D）；内容 each 刷新随机抽 |
| 灵草谷 | 第一次采集 | spirit_qi 0.4-0.7 | 5+ 种基础灵草集中区（plan-botany 已有 22 种 ✅）| 灵草自然生长（plan-botany tick）|

### A.2 完全动态选址算法（Q108: C，关键）

**worldgen `poi_novice_selector.py` 算法**：

```python
def select_poi_locations(
    spawn_center: Vec3,
    radius: int = 1500,
    spirit_qi_field: np.ndarray,
    terrain_field: np.ndarray,
) -> Dict[PoiType, List[Vec3]]:
    """
    完全动态选址（Q108: C）：
    - 按 POI 类型的灵气浓度要求过滤候选格
    - 最小间距 1000 格（Q108: C 锚定）
    - 地形约束（避水域 / 避陡坡 / 避负灵域）
    """
    candidates = {}
    for poi_type, qi_range in POI_QI_REQUIREMENTS.items():
        # 1. 灵气浓度过滤
        valid_cells = (
            (spirit_qi_field >= qi_range.min) &
            (spirit_qi_field <= qi_range.max) &
            (distance_to(spawn_center) <= radius) &
            (distance_to(spawn_center) >= 200)  # 至少 200 格远，避免 spawn 重叠
        )
        # 2. 地形约束
        valid_cells &= (terrain_field.slope < 0.3) & (terrain_field.water_mask == 0)
        # 3. 选址 + 最小间距
        candidates[poi_type] = poisson_disk_sample(valid_cells, min_distance=1000)
    return candidates

POI_QI_REQUIREMENTS = {
    PoiType.ForgeStation: QiRange(0.4, 0.6),
    PoiType.AlchemyFurnace: QiRange(0.3, 0.5),
    PoiType.RogueVillage: QiRange(0.4, 0.6),
    PoiType.MutantBeastNest: QiRange(0.5, 0.7),  # 高灵气吸引异变
    PoiType.ScrollHidden: QiRange(0.3, 0.5),
    PoiType.SpiritHerbValley: QiRange(0.4, 0.7),
}
```

**回退**：若选址失败（spawn ± 1500 格内无合适格）→ 放宽半径到 2000 格 → 仍失败放宽灵气 ±0.1 → 仍失败 fallback 到 skeleton 给的固定坐标（保证玩家有 POI 可去）。

### A.3 v1 实施阶梯

```
P0  6 类 POI worldgen 选址 + 灵气浓度匹配 + 5 分钟步行可达
       poi_novice_selector.py 完全动态选址（Q108: C）
       6 个 blueprint json (worldgen/blueprints/poi_novice/*)
       PoiNoviceLoader system（runtime POI 加载）
       灵气浓度匹配（Q112: C）
       ↓
P1  POI 内 entity / loot + 屠村信誉度 stub + 残卷一周刷 + 兽巢 24h 刷
       散修 NPC scenario（plan-baomai 战斗系统接入）
       异变兽巢 zombie 占位 + 高难度参数（Q114: C）
       残卷藏匿点 loot 表（plan-tsy-loot 接入）
       屠村 TrespassEvent + 1 周拒绝交易 stub（Q109: B；vN+1 接 plan-identity-v1）
       respawn_tick（异变兽 24h / 残卷一周 real-time）
       ↓ 饱和 testing
P2  朽坏视觉 + v1 收口
       朽坏方块组合（残灰 + 断壁 + 骨架）
       LifeRecord "X 在 N 时刻第一次炼器/炼丹/..." 事件
       agent narration: PoiSpawnedEventV1 / TrespassEventV1 触发
```

### A.4 v1 已知偏离正典

- [ ] **plan-fauna-v1 真实异变兽未立**（v1 zombie 占位 + 改 HP / AI 参数）—— vN+1 接入真实缝合兽 + 灰烬蛛
- [ ] **plan-identity-v1 信誉度系统未立**（Q109 屠村惩罚 v1 仅 stub TrespassEvent）—— vN+1 完整 NPC 信誉度反应
- [ ] **plan-forge-leftovers-v1 / plan-alchemy-client-v1 client UI 未完整**（炼器台 / 丹炉 POI 可注入但客户端 UI 不齐）—— 等这两 plan 完成
- [ ] **POI 朽坏视觉**（断壁 / 骨架方块组合）—— P2 实装但 vN+1 美术细化

### A.5 v1 关键开放问题

**已闭合**（Q108-Q114，7 个决策）：
- Q108 → C 完全动态选址（worldgen 按地形 + 灵气浓度 + 最小间距 1000 格）
- Q109 → B 屠村触发 1 周 NPC 拒绝交易（plan-identity-v1 vN+1 stub）
- Q110 → D 残卷一周一刷 real-time + 内容随机
- Q111 → A 异变兽巢 24h server 内时间刷新
- Q112 → C POI 必须建在合适灵气区
- Q113 → B POI 一直存在（玩家任何时刻可发现）
- Q114 → C 异变兽巢高难度（凝脉+ 才能稳定单杀）

**仍 open**（v1 实施时拍板）：
- [ ] **Q115. 选址失败的 fallback 阈值**：选址失败放宽多少次后 fallback 到固定坐标？建议**2 次放宽（半径 2000 / 灵气 ±0.1）后 fallback** —— P0 拟
- [ ] **Q116. POI 间最小间距 1000 格** vs **同类型 POI 最小间距**：6 类 POI 是否要求**两两间距 1000 格**还是仅**同类型 1000 格**？建议**同类型 1000 格**（异类可近，比如炼器台和丹炉相邻是合理的）—— P0 拟
- [ ] **Q117. 异变兽巢难度参数**（Q114: C）：HP / damage / spawn count 起手值？建议起手"3 体 zombie，单体 HP 60（普通 20×3），玩家凝脉期 qi_max 150 单杀勉强 / 引气期 40 必组队"—— P1 拟
- [ ] **Q118. 散修 NPC 商品 / 死信箱接入**：散修聚居点的死信箱具体接 plan-social-v1 的现有死信箱机制？还是新建 spawn 期专属死信箱？建议**复用 plan-social ✅ 现有死信箱**—— P1 拟
- [ ] **Q119. 灵草谷的灵草种类**：22 种 plan-botany 中挑哪 5+ 种放灵草谷？建议**凝脉草 + 引气草 + 解蛊蕊 + 安神果 + 清浊草**（journey §P1 引气期常用）—— P1 拟

### vN+1 留待问题（plan-poi-novice-v2 时拍）

- [ ] 更多 POI 类型（子区域细分 / 多种朽坏纹饰）
- [ ] 散修 NPC 个性化对话
- [ ] POI 联动事件（散修被屠引发周边事件）
- [ ] POI 改造周围灵气（vs Q112 C "POI 不影响灵气"）
- [ ] 玩家偏好驱动的选址（多周目 player profile）



## 接入面 Checklist

- **进料**：spawn_plain + broken_peaks 现有 worldgen + plan-spawn-tutorial-v1 完成的初始环境
- **出料**：spawn ± 1500 格内分散 6 处 POI
- **共享类型**：worldgen blueprint POI + `npc::scenario` ✅(NPC 聚集)
- **worldview 锚点**：§十三 spawn 0.3 → broken_peaks 外围 0.4-0.5 灵气过渡区

---

## §0 设计轴心

- [ ] **新手不远跑**：1500 格内能见到全部 5 大玩法的最低形态(炼器/炼丹/采集/战斗/社交)
- [ ] **POI 是种子**：每个 POI 给"第一次"体验,但要进阶必须远走
- [ ] **末法朽坏感**：所有 POI 都是末法残土风格——破败、半埋、有前修士尸骨

---

## §1 POI 清单

| POI | 位置(相对 spawn) | 内容 | 服务玩法 |
|---|---|---|---|
| 破败炼器台 | (300, _, 200) | 损坏的 forge Station,可用但效率减半 | 第一次炼器 |
| 凡铁丹炉 | (-400, _, 100) | 凡铁锅 + 篝火,可炼基础丹 | 第一次炼丹 |
| 散修聚居点 | (500, _, -300) | 2-3 名散修(`Rogue`),茅屋 + 死信箱 | 第一次社交/交易 |
| 异变兽巢 | (1200, _, 800) | 缝合兽 + 灰烬蛛 nest | 第一次猎兽核 |
| 残卷藏匿点 | (-800, _, -1200) | cave_network 入口,1-2 张残卷 | 第一次拾取知识 |
| 灵草谷 | (-300, _, 600) | 5+ 种基础灵草集中区 | 第一次采集 |

---

## §2 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ⬜ | 6 处 POI 的 worldgen blueprint 注入 | 玩家从 spawn 出发 5 分钟内可见任一 POI |
| **P1** ⬜ | POI 内的 entity/loot 表(散修、兽核、残卷) | 各 POI 内容正常 drop/交互 |
| **P2** ⬜ | 朽坏视觉(残灰、断壁、骨架方块组合) | 美术风格统一 |

---

## §3 数据契约

### v1 P0 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 选址算法 | `worldgen/scripts/poi_novice_selector.py` (新) | `select_poi_locations(spawn_center, radius=1500, spirit_qi_field, terrain_field)` 完全动态选址（Q108: C / Q112: C 灵气浓度匹配 / Q116 同类型最小间距 1000 格） |
| Worldgen profile 扩展 | `worldgen/scripts/terrain_gen/profiles/spawn_plain.py` + `broken_peaks.py` | POI hooks 调用 `poi_novice_selector` |
| 6 个 blueprint json | `worldgen/blueprints/poi_novice/{forge_station,alchemy_furnace,rogue_village,mutant_nest,scroll_hidden,spirit_herb_valley}.json` | 每类 POI 的方块组合 + entity 配置 + loot 配置 |
| Runtime POI loader | `server/src/world/poi_novice.rs` (新) | `PoiNoviceLoader` system（启动时读 worldgen export 加载 POI 到 server runtime） |
| Loot 表 | `server/src/inventory/poi_loot.rs` (新) | 残卷藏匿点 loot 表（接 plan-tsy-loot 已落地系统） |
| Fallback 选址 | `worldgen/scripts/poi_novice_selector.py` | Q115 起手值：2 次放宽（半径 2000 / 灵气 ±0.1）后 fallback 到固定坐标 |

### v1 P1 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 散修 NPC scenario | `server/src/npc/poi_rogue_village.rs` (新) | 2-3 名 Rogue NPC 在散修聚居点；接 plan-social 死信箱 ✅（Q118）+ plan-baomai 战斗系统 |
| 异变兽巢 spawner | `server/src/world/poi_mutant_nest.rs` (新) | zombie 占位 + Q117 高难度参数（3 体 / HP 60 / 凝脉单杀勉强 / 引气必组队）|
| Trespass event | `server/src/world/poi_novice.rs` | `TrespassEvent { village_id, player, killed_npc_count }` event + 1 周拒交易 stub（Q109: B；vN+1 接 plan-identity-v1） |
| Respawn tick | `server/src/world/poi_respawn_tick.rs` (新) | 异变兽 24h server 内时间刷新（Q111: A）/ 残卷一周 real-time 刷新（Q110: D）+ 内容随机抽 |
| 灵草谷布种 | `worldgen/blueprints/poi_novice/spirit_herb_valley.json` | Q119 起手 5 种：凝脉草 + 引气草 + 解蛊蕊 + 安神果 + 清浊草（plan-botany 22 种 ✅ 中挑） |
| Schema | `agent/packages/schema/src/poi_novice.ts` | `TrespassEventV1` / `PoiSpawnedEventV1` |
| Agent narration | `agent/packages/tiandao/src/poi-narration.ts` | 屠村 narration / 残卷拾取 narration / 兽巢遭遇 narration |

### v1 P2 落地清单

| 模块 | 文件路径 | 核心内容 |
|---|---|---|
| 朽坏视觉 | `worldgen/blueprints/poi_novice/*.json` | 残灰 + 断壁 + 骨架方块组合（每个 POI 朽坏风格） |
| LifeRecord | `server/src/lore/life_record.rs` | "X 在 N 时刻第一次炼器/炼丹/..."事件 |
| 单测 | `server/src/world/poi_novice_tests.rs` | 选址算法单测（Q115/Q116 边界）/ 屠村事件 / 刷新周期 / fallback 路径 |

---

## §4 开放问题

### 已闭合（2026-05-03 拍板，7 个决策）

- [x] **Q108** → C 完全动态选址（worldgen 按地形 + 灵气浓度 + 最小间距 1000 格）
- [x] **Q109** → B 屠村 1 周 NPC 拒交易（plan-identity-v1 vN+1 stub）
- [x] **Q110** → D 残卷一周一刷 real-time + 内容随机
- [x] **Q111** → A 异变兽巢 24h server 内时间刷新
- [x] **Q112** → C POI 必须建在合适灵气区
- [x] **Q113** → B POI 一直存在
- [x] **Q114** → C 异变兽巢高难度（凝脉+ 才能稳定单杀）

### 仍 open（v1 实施时拍板）

- [ ] **Q115. 选址失败 fallback 阈值**：建议 **2 次放宽（半径 2000 / 灵气 ±0.1）后 fallback 到固定坐标** —— P0 拟
- [ ] **Q116. 间距规则**：建议 **同类型 POI 最小间距 1000 格**（异类可近，炼器台和丹炉相邻合理） —— P0 拟
- [ ] **Q117. 异变兽巢难度参数**：建议 **3 体 zombie，单体 HP 60，凝脉单杀勉强 / 引气必组队** —— P1 拟
- [ ] **Q118. 死信箱接入**：建议 **复用 plan-social ✅ 现有死信箱**（不另建 spawn 期专属）—— P1 拟
- [ ] **Q119. 灵草谷的 5+ 种**：建议 **凝脉草 + 引气草 + 解蛊蕊 + 安神果 + 清浊草**（journey §P1 引气期常用）—— P1 拟

### vN+1 留待问题（plan-poi-novice-v2 时拍）

- [ ] 更多 POI 类型 / 子区域细分
- [ ] 散修 NPC 个性化对话
- [ ] POI 联动事件（散修被屠引发周边事件）
- [ ] POI 改造周围灵气（vs Q112 C "POI 不影响灵气"）
- [ ] 玩家偏好驱动的选址（多周目 player profile）
- [ ] 真实异变兽（plan-fauna-v1 落地后）
- [ ] 完整信誉度系统接入（plan-identity-v1 落地后）
- [ ] 朽坏视觉美术细化

## §5 进度日志

- 2026-05-01：骨架创建。plan-gameplay-journey-v1 §P1 派生。
- 2026-05-03：从 skeleton 升 active。§A 概览 + §3 v1 P0/P1/P2 数据契约落地（7 个决策点闭环 Q108-Q114，5 个 v1 实装时拍板 Q115-Q119）。primary axis = **5 大玩法首次触发的步行可达性**（玩家从 spawn 出发 5 分钟步行内可见任一 POI 的概率）。**完全动态选址**（Q108: C）取代 skeleton 给的固定坐标 — worldgen 按地形 + 灵气浓度 + 最小间距 1000 格算出 6 处。**沉默引导原则沿用**（plan-spawn-tutorial-v1 一致）：v1 严格无 UI 标记 / 无任务面板，POI 自然存在地图上。下一个候选：plan-fauna-v1（真实异变兽）/ plan-identity-v1（信誉度系统）/ plan-forge-leftovers-v1（炼器 client UI）。

## Finish Evidence

### 落地清单

- P0 worldgen：`worldgen/scripts/poi_novice_selector.py`、`worldgen/scripts/terrain_gen/bakers/raster_export.py`、`worldgen/blueprint/poi_novice/*.json` 接入 6 类新手 POI 动态选址；实际仓库目录为 `worldgen/blueprint/`，未新建平行 `blueprints/`。
- P1 server runtime：`server/src/world/poi_novice.rs`、`server/src/world/poi_respawn_tick.rs`、`server/src/world/poi_mutant_nest.rs`、`server/src/npc/poi_rogue_village.rs`、`server/src/inventory/poi_loot.rs` 接入 POI registry、屠村拒交易 stub、刷新周期、散修聚居点与兽巢参数。
- P1/P2 event bridge：`server/src/network/poi_novice_bridge.rs`、`server/src/network/redis_bridge.rs`、`server/src/schema/poi_novice.rs`、`agent/packages/schema/src/poi-novice.ts`、`agent/packages/schema/generated/*.json` 接入 `bong:poi_novice/event` 契约。
- P2 narration/LifeRecord：`server/src/world/poi_novice.rs` 接入 `PoiFirstActionEvent` → `LifeRecord` 记录；`agent/packages/tiandao/src/redis-ipc.ts` 和 `agent/packages/tiandao/src/narration/templates.ts` 接入 POI spawned / trespass narration。

### 关键 commit

- `8e4a8fa1` 2026-05-03 `feat(worldgen): 接入新手 POI 动态选址`
- `4e57b096` 2026-05-03 `feat(schema): 定义新手 POI 事件契约`
- `316195b9` 2026-05-03 `feat(server): 接入新手 POI runtime 与刷新 stub`
- `df8ee3c8` 2026-05-03 `feat(agent): 增加新手 POI 事件叙事`

### 测试结果

- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` → `2121 passed`
- `cd agent && npm run build` → schema / tiandao TypeScript build 通过
- `cd agent/packages/schema && npm test` → `258 passed`
- `cd agent/packages/tiandao && npm test` → `213 passed`
- `cd worldgen && python3 -m pytest "tests/test_poi_novice_selector.py"` → `3 passed`
- `cd worldgen && python3 -m scripts.terrain_gen --tile-size 512` → raster export 通过，`tiles synthesized: 208`

### 跨仓库核验

- worldgen：`select_poi_locations`、`SelectionStrategy.STRICT_RADIUS_1500`、`inject_dynamic_novice_pois`、`novice_pois` manifest payload。
- server：`PoiNoviceLoader`、`PoiNoviceRegistry`、`TrespassEvent`、`PoiRespawnStore`、`CH_POI_NOVICE_EVENT`、`RedisOutbound::PoiSpawned` / `PoiTrespass`。
- agent/schema：`PoiSpawnedEventV1`、`TrespassEventV1`、`PoiNoviceKindV1`、generated schema freshness gate。
- agent/tiandao：`RedisIpc.onPoiNoviceEvent`、`renderPoiSpawnedNarration`、`renderTrespassNarration`。

### 遗留 / 后续

- 真实异变兽仍依赖 `plan-fauna-v1`；本 plan 保留 zombie 高难度占位参数。
- 完整信誉度系统仍依赖 `plan-identity-v1`；本 plan 仅落地 1 周拒交易 stub 和 trespass event。
- 破败炼器台 / 凡铁丹炉 UI 仍依赖 `plan-forge-leftovers-v1` / `plan-alchemy-client-v1`。
- POI 联动事件、散修个性化对话、更多朽坏纹饰留给 `plan-poi-novice-v2`。
