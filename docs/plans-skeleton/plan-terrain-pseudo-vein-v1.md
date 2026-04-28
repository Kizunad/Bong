# Bong · plan-terrain-pseudo-vein-v1 · 骨架

**伪灵脉绿洲**（`pseudo_vein_oasis`）。荒野中突现的高灵气小绿洲——**天道刻意制造的陷阱**，灵气 0.7 异常浓郁，吸引修士聚集自相残杀以回收真元。30-90 分钟（按聚集人数动态加速）后消散，外缘伴生小型负灵游离风暴。

**世界观锚点**：
- `worldview.md §二 灵压环境`（馈赠区"天道陷阱（伪灵脉）"原文）
- `worldview.md §八 天道行为准则`（中等手段第 3 项 / 隐性手段"narration 暗示某个方向有机缘"）
- `worldview.md §十三 世界地理 · 荒野`（"荒野中偶尔出现天道临时生成的伪灵脉——维持 30 分钟的高灵气点（0.6），之后消散。这是天道的陷阱"）

**library 锚点**：
- `world-0004 天道口述残编`（"瓮口向天，瓮底向地""吾只收"——天道收割者性格）
- `ecology-0003 灵气零和记`（推演四"高境必稀"——伪灵脉高浓度反差越大，越要被天道压回零和总量）
- `world-0002 末法纪略`（第二变·灵气枯竭——伪灵脉是衰减总趋势中的局部畸变）

**交叉引用**：
- `plan-narrative-v1`（天道 narration "东荒深处传来一声低吼。是灵兽？是机缘？是坟墓？"——伪灵脉的引诱叙事 hook）
- `plan-tribulation-v1`（聚集→广播→天劫——伪灵脉是天劫诱饵的常见前置）
- `plan-perception-v1`（境界感知"50 格内生物气息"——决定玩家能否识破"这是陷阱"）
- `plan-cultivation-v1`（突破必需 0.5+ 灵气——伪灵脉是诱饵性突破点）

**阶段总览**：
- P0 ⬜ blueprint zone 注册 + `terrain-profiles.example.json` profile spec
- P1 ⬜ `PseudoVeinOasisGenerator` 实装（地形 field + 装饰物）
- P2 ⬜ 动态生命周期（聚集→灵气衰减加速→消散→外缘游离风暴）
- P3 ⬜ 天道 narration hook + 多人聚集探测 + 与天劫系统联动

---

## §0 设计轴心

- [ ] **临时地理**：与六域固定 zone 不同，伪灵脉是 transient zone——blueprint 不固定坐标，由天道 agent 在荒野（spawn_plain 外缘 / waste_plateau 内部）动态注入
- [ ] **诱饵性高灵气**：核心区 `qi_density = 0.7`，远高于荒野基线 0.12——这个反差就是诱饵
- [ ] **聚集即衰减**：在场修士每多一人，区内灵气衰减速度 +20%；3 人聚集时 30 分钟内消散，独行者可能撑 90 分钟
- [ ] **代偿负灵风暴**：消散瞬间，外缘 100-200 格随机生成 1-3 个负灵游离风暴 hot-spot（`neg_pressure` 层短期值 -0.4 ~ -0.6），按 §二"自然真空"原文实现
- [ ] **不可预设灵龛**：玩家不能在伪灵脉内放龛石（消散瞬间龛石被吞），符合 §十一"灵龛不能设置在活坍缩渊内"同源逻辑

## §1 世界观推断逻辑（为何此地必然存在）

> 末法残土的灵气总量恒定且零和（ecology-0003）。天道的工作是"减缓灵气消耗速度，延长世界寿命"（worldview §八）。它的中等手段是"在强者区域刷新异变兽（既是威胁也是诱饵）"和"发布天象预兆让修士自行迁移"。

伪灵脉是这两条手段的合成：
- **天道反直觉操作**：把灵气从无人荒野**临时聚集**到一个小点 → 反向制造一个高浓度反差 → 修士因感知到 0.7 灵气会本能聚集 → 聚集消耗加倍 → 天劫劈死最强者 → 灵气连同真元一起回收
- **零和守恒（生成期）**：伪灵脉的灵气不是"凭空生"，是从周围荒野临时调拨——所以伪灵脉外围 50 格内基线 `qi_density` **额外 -0.04** 作为代偿（visible "饥渴圈"）。这部分代偿在伪灵脉**整个生命周期内**保持，是天道"借"出去的灵气
- **零和守恒（消散期）**：消散瞬间灵气去向**完整闭环**——按 worldview §八 / §十 灵气总量恒定原则：
  - **70% 回灌饥渴圈**：消散瞬间核心 0.7 灵气向外扩散，饥渴圈（50-200 格）qi 临时 +0.05~+0.10，1 小时内缓慢扩散回基线。物理上是"还借走的"
  - **30% 被天道直接收回**：天道作为收割者吃掉这部分（worldview §八"吾只收"+ world-0004 锚定）；这部分灵气进入天道的"全服灵气调度池"，可能数小时后在另一处荒野作为新伪灵脉重现——总量守恒不破
- **代偿负灵风暴**：worldview §二"游离风暴：天道为代偿某处伪灵脉而在荒野随机制造的负能风暴"——伪灵脉消散瞬间，外缘 100-200 格随机生成 1-3 个负灵 hot-spot（持续 5-10 分钟），是上述 30% 灵气被收割过程中产生的局部负压扰动；不是永久负灵域，会随灵气回灌过程消退

## §2 特殊机制

| 机制 | 触发 | 效果 |
|---|---|---|
| **诱饵静坐加速** | 玩家在伪灵脉核心静坐修炼 | 经脉打通速度 +60%（伪灵脉真的有用，否则不算诱饵） |
| **聚集探测** | 50 格内 ≥ 2 名玩家 | 区内灵气衰减速度按人数 ×1.0 / 1.4 / 1.8 / 2.5 / 3.5 倍递增 |
| **消散预警** | 灵气从 0.7 跌至 0.4 时 | 全员收到 narration："此处灵气，似有异变"（不明示是陷阱） |
| **天劫诱饵** | 区内灵气总消耗 ≥ 阈值 | 天道 agent 触发劫气标记，区内最高境界玩家天劫概率 +30%（24h 内） |
| **消散瞬间** | 灵气 = 0 | 中心 `feature_mask` 残留 1 分钟"残灰圈" → 灰化为 `coarse_dirt + gravel`；70% 灵气回灌饥渴圈（qi 临时 +0.05~+0.10，1h 衰回基线）；30% 被天道收割（进全服灵气池）；外缘 100-200 格随机播种 1-3 个**短期负灵 hot-spot**（持续 5-10 分钟，使用 `neg_pressure` 层 + `anomaly_kind=2 (qi_turbulence)`，**不用** `spacetime_rift`——后者是 portal 锚点专用语义） |
| **龛石失效** | 玩家试图在区内放灵龛 | 龛石碎裂 + chat："此地灵脉飘忽，龛石不立" |

## §3 独特装饰物（DecorationSpec 预填）

```python
PSEUDO_VEIN_DECORATIONS = (
    DecorationSpec(
        name="false_spirit_lotus",
        kind="flower",
        blocks=("pink_petals", "warped_wart_block", "amethyst_cluster"),
        size_range=(1, 2),
        rarity=0.70,
        notes="伪灵莲：粉花瓣 + 扭曲菌块基底 + 紫晶花蕊。看似灵草，"
              "实则一摘即化粉（消散前不掉资源）。视觉为该地形最显眼标识。",
    ),
    DecorationSpec(
        name="phantom_qi_pillar",
        kind="crystal",
        blocks=("amethyst_cluster", "purple_stained_glass", "soul_lantern"),
        size_range=(4, 7),
        rarity=0.30,
        notes="幻灵柱：紫晶 + 紫玻 + 灵魂提灯，会缓慢呼吸式发光。"
              "中心区域 1-2 根。消散时整柱崩为 amethyst_shard 碎屑（不可拾）。",
    ),
    DecorationSpec(
        name="lush_grass_overlay",
        kind="shrub",
        blocks=("flowering_azalea_leaves", "pink_tulip", "lily_of_the_valley"),
        size_range=(1, 2),
        rarity=0.85,
        notes="异常茂盛草：开花杜鹃叶 + 粉郁金香 + 铃兰，密集铺地——"
              "异常茂盛是核心识别（荒野中突现一片花海）。",
    ),
    DecorationSpec(
        name="tiandao_seal_stele",
        kind="boulder",
        blocks=("sculk", "sculk_vein", "soul_sand"),
        size_range=(2, 3),
        rarity=0.15,
        notes="天道封纹石：sculk 包裹的小石碑，表面 sculk_vein 成纹。"
              "高境感知（凝脉+）可读出'瓮'字模糊轮廓——伪灵脉唯一警示。",
    ),
    DecorationSpec(
        name="false_vein_well",
        kind="boulder",
        blocks=("prismarine", "sea_lantern", "tube_coral_block"),
        size_range=(2, 4),
        rarity=0.20,
        notes="伪泉眼：海晶石 + 海晶灯 + 管珊瑚——发蓝绿光的小水洼。"
              "极个别伪灵脉中心会有一处，看似真灵眼（凝脉突破诱饵）。",
    ),
)
```

外缘"饥渴圈"无新装饰，但 `flora_density` 在该圈内**降至 0**——视觉上从花海突然变成裸土，制造视差。

## §4 完整 profile 配置

### `terrain-profiles.example.json` 追加

```json
"pseudo_vein_oasis": {
  "height": { "base": [68, 76], "peak": 80 },
  "boundary": { "mode": "soft", "width": 32 },
  "surface": ["grass_block", "moss_block", "flowering_azalea_leaves", "warped_wart_block"],
  "water": { "level": "low", "coverage": 0.04 },
  "passability": "high",
  "lifetime_minutes": [30, 90],
  "core_radius": 60,
  "rim_radius": 120
}
```

### Blueprint zone 模板（动态注入，非固定）

```json
{
  "name": "pseudo_vein_<id>",
  "display_name": "伪灵脉",
  "aabb": { "min": [<cx-150>, 60, <cz-150>], "max": [<cx+150>, 90, <cz+150>] },
  "center_xz": [<cx>, <cz>],
  "size_xz": [300, 300],
  "spirit_qi": 0.70,
  "danger_level": 4,
  "worldgen": {
    "terrain_profile": "pseudo_vein_oasis",
    "shape": "circular",
    "boundary": { "mode": "soft", "width": 32 },
    "landmarks": ["phantom_qi_pillar", "tiandao_seal_stele"]
  }
}
```

### 数值梯度（按距离中心 r / core_radius 归一化的 `t`）

| 区位 | t | qi_density | mofa_decay | qi_vein_flow | flora_density |
|---|---|---|---|---|---|
| 核心（伪泉眼）| 0-0.2 | 0.85 | 0.05 | 0.95 | 0.85 |
| 主体（花海）| 0.2-0.7 | 0.65 | 0.10 | 0.50 | 0.85 |
| 边缘 | 0.7-1.0 | 0.30 | 0.20 | 0.10 | 0.45 |
| 饥渴圈 | 1.0-2.0 | **0.08**（基线 -0.04 代偿）| 0.55 | 0 | 0 |
| 外荒野 | >2.0 | 0.12（恢复基线）| 0.40 | 0 | 0 |

## §5 LAYER_REGISTRY 字段映射

需要的 `extra_layers`：

```python
extra_layers = (
    "qi_density",
    "mofa_decay",
    "qi_vein_flow",
    "flora_density",
    "flora_variant_id",
    "neg_pressure",        # 消散后外缘短期负灵 hot-spot 写入（已存在层，maximum blend）
    "anomaly_intensity",   # 在场期 + 消散后游离风暴
    "anomaly_kind",        # 2 = qi_turbulence（生命周期内 + 消散后游离风暴均用此）
)
```

**anomaly_kind 选择**：生命周期内 + 消散外缘风暴**统一用 `qi_turbulence` (2)**——worldview §二 明确游离风暴是"负能风暴"（负灵性质），不是"时空裂痕"（rift_mouth 用 `spacetime_rift=1`）。强度由 `anomaly_intensity` 在场期按 t 衰减、消散后短期峰值落在 100-200 格圈。

**neg_pressure 层用法**：消散后外缘 hot-spot 持续 5-10 分钟内写入 `neg_pressure ∈ [0.4, 0.6]`（safe_default=0.0, maximum blend）；时窗结束后由 server tick 主动清零（覆盖 maximum 用 server-side override），不靠 raster 重生成。

## §6 数据契约（下游 grep 抓手）

| 阶段 | 抓手 | 位置 |
|---|---|---|
| P0 | `pseudo_vein_oasis` profile | `worldgen/terrain-profiles.example.json` |
| P0 | `BlueprintZone.name == "pseudo_vein_*"` 模板 | 动态注入接口 `server/src/worldgen/transient_zone.rs`（新增） |
| P1 | `class PseudoVeinOasisGenerator` + `fill_pseudo_vein_oasis_tile` | `worldgen/scripts/terrain_gen/profiles/pseudo_vein_oasis.py`（新增） |
| P1 | `PSEUDO_VEIN_DECORATIONS` 5 项 + `EcologySpec.notes` | 同上 |
| P2 | `struct PseudoVeinLifecycle { spawned_at, decay_rate, occupant_count }` | `server/src/worldgen/pseudo_vein.rs`（新增） |
| P2 | Redis key `bong:pseudo_vein:active` + payload `{id, center, qi_current, occupants}` | IPC schema 新增 `PseudoVeinSnapshot` |
| P2 | `bong:event_dissipate` event payload `{id, center, storm_anchors: [(x,z)]}` | 同上 |
| P3 | 天道 narration template `pseudo_vein.lure / pseudo_vein.warning / pseudo_vein.dissipate` | `agent/packages/tiandao/src/narration/templates.ts` |

## §7 实施节点

- [ ] **P0** blueprint + profile spec 注册（不动 generator） — 验收：`python -m scripts.terrain_gen` 不 panic 即可，伪灵脉 zone 走 wilderness fallback
- [ ] **P1** generator 实装 — 验收：手动 inject 一条 transient zone → raster_export 后 qi_density 中心 0.7 / 饥渴圈 0.08 / `flora_variant_id` 命中 5 种装饰
- [ ] **P2** 生命周期 — 验收：单元测试 3 名占位玩家在场 → 30 min 内 qi 跌到 0；消散事件触发 1-3 storm anchor 正确写入 `anomaly_kind=2 (qi_turbulence)` + `neg_pressure ∈ [0.4, 0.6]` 持续 5-10 分钟后清零
- [ ] **P3** 天道 narration + 天劫劫气标记 — 验收：占位玩家最高境界 → 24h 内天劫 roll 概率 +30%；narration 三档（lure / warning / dissipate）按阈值触发

## §8 开放问题

- [ ] 伪灵脉的"产卵节奏"——天道 agent 多久 spawn 一个？（建议按全服灵气总量监控：总量降幅超阈值时主动放饵）
- [ ] 玩家挖出的伪泉眼/幻灵柱方块带回基地是否保留？（建议 inventory 操作即化为 `gravel`——与"灵物磨损税"同源）
- [ ] 多个伪灵脉是否互相干涉？（首版禁止 500 格内并存）
- [ ] 与既有六域 zone 的边界处理——若伪灵脉生成在 broken_peaks 边缘（高低差大），是否强制贴地？（首版强制 base_y 取局部地表中位）
- [ ] anomaly 风暴的可视化——客户端粒子方案 vs 完全靠 HUD 灵压条提示？（依赖 plan-particle-system-v1）
- [ ] 玩家凝脉/固元期跑去伪灵脉真实修炼，被抓的概率统计——伪灵脉给的 +60% 加速是否要做总量上限（避免被刷）

## §9 进度日志

- 2026-04-28：骨架立项。世界观钩子来自 `worldview.md §二/§八/§十三` 的伪灵脉/天道陷阱原文 + `world-0004 天道口述残编` 收割者性格。等优先级排序与 plan-narrative-v1 / plan-tribulation-v1 接入时机协调。
- 2026-04-28（自查修订）：
  - **mid-10** 修：§1 补全灵气**消散期**零和闭环——70% 回灌饥渴圈（1h 衰回基线）+ 30% 天道收割（进全服灵气池可能在他处重现）。原版只说生成期 -0.04 代偿，未交代消散去向，违反 ecology-0003 灵气零和原则。
  - **weak-13** 修：消散外缘 anomaly_kind 改 `qi_turbulence (2)` + `neg_pressure` 层，**不用** `spacetime_rift (1)`——后者是 rift_mouth portal 专属语义；游离风暴按 worldview §二 是"负能风暴"，更贴 qi_turbulence + neg_pressure 组合。
