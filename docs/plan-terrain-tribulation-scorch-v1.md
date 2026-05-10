# Bong · plan-terrain-tribulation-scorch-v1

**烬焰焦土**（`tribulation_scorch`）。worldview §八 天道激烈手段："直接对高境界修士降天劫" + §十三 血谷"天劫多发"。**长期天劫频劈**会在某些区域累积出可识别的地理特征：玻璃化沙地、雷劈焦炭树、磁石矿露头、被劫劈出的玄武岩坑。本 profile 是天劫累积带的物理化——不是单次天劫地点，而是历代天劫积累形成的**焦土集中带**。雷雨天气仍会真的招雷，是雷法染色专修的天然圣地（也是普通修士的死亡陷阱）。

**世界观锚点**：
- `worldview.md §八 天道行为准则 · 激烈手段`（"直接对高境界修士降天劫" + "在灵气将尽的区域触发「域崩」"——焦土是历代降劫累积）
- `worldview.md §三 突破条件 · 通灵 → 化虚 / 渡虚劫`（"天道降下渡虚劫，强度极高，全服广播"——化虚渡劫死处必形成焦土）
- `worldview.md §十三 区域详情 · 血谷`（"高级野兽、天劫多发"——血谷已隐含一部分焦土特征，本 profile 是其专属化）
- `worldview.md §六 真元染色谱 · 雷法`（"暴烈色——真元带电，间歇放电。击穿护体真气+，可远程小威慑。阴雨/水域真元紊乱"——焦土对雷法染色专修是天然修炼场）
- `worldview.md §十七 地形对季节的响应`（**天劫焦地 / 渡劫遗痕 = 生态可缓慢恢复 / 灵气永久抹除** —— 天道劫气抹掉的真元不会回；焦地核心 qi_density 永久归 0；植被 / 微小生命可在数十 game-year 后慢慢回归。这是 §三 / §八 "天道学费"的物理实现：你被劈过的地方，地永远记得）

**library 锚点**：
- `world-0002 末法纪略` 第三变（"末法以来三百年无一人飞升"+"通灵者数人，化虚无一"——焦土上的化虚劫遗迹是上古积累，今天再生很慢）
- `world-0004 天道口述残编` 其二（"吾只收"——天劫即"收"的最直接形式）
- `cultivation 馆藏（如《六境要录》）`（境界突破必经天劫的章节，可作 P3 narration 抓点）

**交叉引用**：
- `plan-tribulation-v1`（天劫系统主线——本 profile 是其"地理后果"的具象化）
- `plan-cultivation-v1`（雷法染色 / 暴烈色）——焦土对暴烈色加成是核心 gameplay 钩子
- `plan-mineral-v1`（lodestone / copper / iron 露头——天劫遗下的雷磁矿）
- `plan-perception-v1`（雷雨预警 narration）
- `plan-vfx-v1`（玻璃化沙地 / 雷劈树 / 雷磁柱视觉）
- `plan-zone-weather-v1` ✅（zone weather profile / lightning override / style modifier 已落地）
- `plan-zone-environment-v1` ✅（WeatherEvent → EnvironmentEffect bundle / 客户端持续环境效果已落地）

**阶段总览**：
- P0 ⬜ worldgen profile 注册 + 2-3 处 zone 候选位（血谷东陲 + 北荒东陲呼应 world-0004 "二在北荒东陲已殒"），并对齐已存在的 server weather profile zone id
- P1 ⬜ `TribulationScorchGenerator` 实装（地形 field + 玻璃化沙地 + 焦炭树 + 雷磁柱）
- P2 ⬜ 复用现有 zone weather / environment / lightning hook，把焦土雷雨伤害、暴烈色 ×0.7、金属甲 ×1.5 串成一个真实机制
- P3 ⬜ 化虚渡劫遗迹 structure（极稀有，相当于本 profile 的"宝藏点"）+ 整合 plan-tribulation-v1 全服广播 hook；`虚劫残屑`用途在 P3 前决策

---

## 当前代码实地核验（2026-05-11）

- **旧阻塞已解除**：`plan-zone-weather-v1` / `plan-zone-environment-v1` 已在 `docs/finished_plans/`，不再是本 plan 升 active 的前置 blocker。
- **server weather 脚手架已存在**：`server/src/lingtian/weather_profile.rs` 已有 `ZoneWeatherProfile` / `ZoneWeatherProfileRegistry` 和 `lightning_strike_per_min_override`；`server/weather_profiles.json` 已包含 `blood_valley_east_scorch`、`north_waste_east_scorch`、`drift_scorch_001` 三个焦土 zone 的雷雨偏置。
- **物理/视觉 hook 已有入口**：`server/src/world/weather_to_environment.rs` 已有 `weather_to_environment_bundle`；`server/src/world/weather_physics/lightning.rs` 已有 `lightning_strike_at`；`server/src/cultivation/style_modifier.rs::for_zone_weather` 已落地暴烈色 ×0.7、金属甲 ×1.5 的乘性规则。
- **真正缺口在 worldgen 与结构物**：当前 `rg tribulation_scorch/TribulationScorchGenerator/ascension_pit` 仍只命中文档和 weather profile，不存在 `worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py`、地形 profile 注册、`tianjie_ascension_pit` structure spawner。
- **实现约束**：P2 不再新增平行的 `server/src/world/weather/scorch.rs` 体系；优先复用现有 `ZoneWeatherProfile`、`weather_to_environment_bundle`、`lightning_strike_at`、`style_modifier::for_zone_weather`，只在缺少编排函数时补薄 adapter。

**结论**：可升 active。P0/P1 是未落地的 worldgen 主体；P2 是把已完成的 weather/environment 能力接到焦土 zone；P3 的 `虚劫残屑`用途不阻塞 P0-P2。

---

## §0 设计轴心

- [ ] **历代累积，不是单次事件**：本 profile 不绑定 `plan-tribulation-v1` 的实时天劫渲染——它是"历代天劫已经劈过几百次"的地理结果，所以是固定 zone（不是 transient）
- [ ] **雷雨真招雷**：在 zone 内玩家头顶若是开放天空 + 雨天，每分钟 1-3 次 lightning_strike 事件——不是装饰，是真实伤害；玩家戴金属护甲提高命中率；**暴烈色降低命中率（抗劈，详见 §2）**——这是反 worldview §六"阴雨/水域真元紊乱"副作用的 narrative 弥补：暴烈色无法消除阴雨副作用，但能引导外来雷电入经脉而非劈穿肉身
- [ ] **修炼速度按 worldview §三 公式走**：本 profile **不改写** `cultivation 速度 ∝ zone.spirit_qi × (qi_current / qi_max)` 主公式（属 plan-cultivation-v1 范围）；染色亲和不进 cultivation 主公式——焦土对暴烈色的吸引力来自"敢站着不被劈穿"的生存优势，不来自双倍刷怪
- [ ] **真元紊乱外缘**：焦土外缘 50-100 格内是低概率"游离风暴"区（天劫余压），所有玩家（含暴烈色）真元自然漏失 +30%——worldview §六 雷法明文"阴雨/水域真元紊乱"，**暴烈色不豁免**
- [ ] **化虚渡劫遗迹（极稀有）**：每 zone 0-1 个 `tianjie_ascension_pit` **structure（非 DecorationSpec）**——上古化虚者渡虚劫死处，中心是巨型玄武岩坑 + 残破渡劫人形痕；可能爆出极稀有的"虚劫残屑"；用 zone.extras.ascension_pit_xz 强制单点定位
- [ ] **季节响应**（worldview §十七）：焦地 qi_density 不参与 §十七 二季 / 汐转节律——`Season::*_modifier()` 在焦地 zone 内一律 short-circuit（与死域同源）；但**生态层**（flora_density / 微小生命 spawn）可缓慢恢复——首版固定速率（每 game-year ×1.05 衰回基线，10 game-year 后接近原野）；不与季节系统直接耦合，避免冬季加速 / 夏季加速等"快速治愈"反 worldview 语义

## §1 世界观推断逻辑（为何此地必然存在）

> worldview §八 明确天劫是天道激烈手段，§三 把"渡虚劫"列为通灵→化虚的必经事件，§十三 血谷标"天劫多发"。**长期降劫的物理后果**：
> - 玻璃化沙地（沙地被高温雷弧瞬间熔化为玻璃）
> - 焦炭化树木（雷劈树木 → 黑色焦炭木）
> - 磁石露头（强电磁脉冲让铁矿石极化为天然 lodestone）
> - 玄武岩坑（雷劈穿地表形成熔岩冷却坑）
>
> 这些特征在 vanilla MC 都有对应方块（glass / coal_block / lodestone / basalt），世界观逻辑链完整。

**地理位置**：
- world-0004 天道口述残编明记"二在北荒东陲，昨日已殒"——上古化虚者死在北荒东陲，**此处必有焦土**
- worldview §十三 血谷"天劫多发"——血谷东陲是另一处天然候选
- 末法纪略说"二百二十八年前最后一次飞升"——古迹更可能位于人迹罕至区

**与既有 profile 的边界**：
- 与 `ancient_battlefield` 不同：古战场是横向战争（人对人），焦土是纵向天劫（天对人）；古战场撒满人骨/兵器，焦土撒满雷击痕/化虚遗迹
- 与 `rift_valley` 不同：血谷是峡谷地形，焦土是平原带；二者可能**邻接**（血谷东陲焦土带）但 profile 互斥

## §2 特殊机制

| 机制 | 触发 | 效果 |
|---|---|---|
| **雷雨实招雷** | zone 内 + 雨天 + 玩家头顶开放天空 | 每分钟 roll `lightning_strike`；基线命中率 P_base；按下条调整 |
| **暴烈色抗雷击 −30%** | 玩家**主色为暴烈色**（worldview §六.染色规则） | 命中率 ×0.7（暴烈色能引导雷电入经脉缓冲，**不是**亲和雷电——避免与 worldview §六"阴雨/水域真元紊乱"副作用反向）。暴烈色**不**减免漏失 +30%——副作用照常 |
| **金属护甲 +50% 命中** | 玩家护甲材质含 iron / copper / gold | 命中率 ×1.5（雷电导引），与暴烈色**乘性叠加**：暴烈色 + 重铁甲 = ×0.7 × 1.5 = ×1.05 ≈ baseline |
| **真元紊乱漏失 +30%（全员）** | 玩家在 zone 内（核心 + 主体 + 紊乱外缘）| 真元自然漏失 ×1.3——worldview §六 明确"雷法/暴烈色 阴雨/水域真元紊乱"，**所有玩家含暴烈色都吃这个副作用** |
| **雷磁矿露头** | 地表 mineral_density hot-spot | 表层即可挖到 lodestone / copper / iron（不需要凝脉感知）；本 profile 是新手能够入手低阶矿的稀有地表入口 |
| **化虚渡劫遗迹（structure）** | `tianjie_ascension_pit` structure | 中心 5-10 格是 basalt + obsidian 大坑 + 残破 armor_stand 渡劫姿；scan 半径 10 格内 0.5% 几率掉"虚劫残屑"（极稀有）；**每 zone 0-1 个，由 structure spawner 强制约束**——不走 `flora_density / flora_variant_id` 采样 |
| **天劫余烬粒子** | client 视觉 | 持续掉落微弱蓝白火星粒子（区分自然下雪） |
| **磁场紊乱** | 玩家近距离接近雷磁柱（5 格内） | 指南针失效 + 漂浮金属物品（趣味）+ 凝脉+境界 inspect 可读"此地磁场异常" |

## §3 独特装饰物（DecorationSpec 预填）

```python
TRIBULATION_SCORCH_DECORATIONS = (
    DecorationSpec(
        name="glass_fulgurite",
        kind="boulder",
        blocks=("sand", "glass", "tinted_glass"),
        size_range=(2, 5),
        rarity=0.40,
        notes="玻璃熔痕：沙 + 玻璃 + 染色玻璃——雷击瞬间高温熔沙形成。"
              "形态多为短粗管状或张开的喇叭口（vanilla 几何近似）。"
              "焦土最常见的视觉符号。",
    ),
    DecorationSpec(
        name="charred_husk_tree",
        kind="tree",
        blocks=("coal_block", "stripped_oak_log", "blackstone"),
        size_range=(5, 10),
        rarity=0.35,
        notes="焦炭枯木：煤块树干 + 剥皮原木 + 黑石根部——雷劈烧透的树。"
              "树冠完全没有，只有焦黑的躯干指向天空。"
              "夜晚靠近偶有低频 narration（'此树曾承一劫'）。",
    ),
    DecorationSpec(
        name="lightning_basalt_pit",
        kind="boulder",
        blocks=("basalt", "obsidian", "magma_block"),
        size_range=(4, 8),
        rarity=0.28,
        notes="劫雷玄武坑：玄武 + 黑曜 + 岩浆——雷劈穿地表的圆坑。"
              "中心可能是岩浆（视觉，不会真烧伤——除非有 plan-vfx 改）。",
    ),
    DecorationSpec(
        name="lodestone_vortex",
        kind="crystal",
        blocks=("lodestone", "copper_block", "weathered_copper"),
        size_range=(3, 6),
        rarity=0.18,
        notes="雷磁旋柱：磁石 + 铜块 + 风化铜——天然形成的极化磁石柱。"
              "进入 5 格内指南针失效。雷雨天会主动招雷（亮度增益）。",
    ),
    DecorationSpec(
        name="iron_lattice_slag",
        kind="shrub",
        blocks=("iron_block", "raw_iron_block", "deepslate"),
        size_range=(2, 4),
        rarity=0.22,
        notes="铁渣矩阵：铁块 + 粗铁块 + 深板岩——雷劈使铁矿石熔合的渣块。"
              "外观破碎，可挖（破坏后有概率掉 raw_iron）。",
    ),
    # 注：tianjie_ascension_pit **不**作为 DecorationSpec 出现——
    # rarity 采样无法保证"每 zone 0-1"约束（大 zone 内会出现 N>1）。
    # 改为独立 structure spawner（见 §6 数据契约 P3 + §4 zone.extras.ascension_pit_xz），
    # 由 zone 显式给坐标，worldgen structure 系统强制单点生成。
    DecorationSpec(
        name="blue_lightning_glass",
        kind="crystal",
        blocks=("blue_stained_glass", "light_blue_concrete", "sea_lantern"),
        size_range=(3, 5),
        rarity=0.10,
        notes="蓝雷晶：蓝玻 + 浅蓝混凝土 + 海晶灯——雷击间歇结晶产物。"
              "夜间发蓝光，是焦土夜景的视觉锚。可凿，掉 amethyst_shard 替代。",
    ),
)
```

`ambient_effects = ("distant_thunder_low", "ash_fall", "static_crackle")`——远雷低频 + 飘灰 + 静电劈啪声（即使无雨也持续）。

## §4 完整 profile 配置

### `terrain-profiles.example.json` 追加

```json
"tribulation_scorch": {
  "height": { "base": [70, 88], "peak": 96 },
  "boundary": { "mode": "hard", "width": 80 },
  "surface": ["coarse_dirt", "gravel", "sand", "blackstone", "basalt", "glass"],
  "water": { "level": "very_low", "coverage": 0.01 },
  "passability": "high",
  "ambient_hint": {
    "thunder_feel": "frequent",
    "lightning_visual_fx": "ash_drift_with_static_crackle"
  }
}
```

> **注**：原本草稿写过 `weather_overrides` 字段——但 MC 天气是**全局**的，没有 zone-scoped 系统消费。删除该字段（dead 字段会让人误以为已实装）。"雷雨实招雷"的实现路径见 §6 P2：复用已落地的 `ZoneWeatherProfile { thunderstorm_multiplier, drought_wind_multiplier, lightning_strike_per_min_override }`、`weather_to_environment_bundle`、`lightning_strike_at`、`style_modifier::for_zone_weather`，不再新建平行天气 override 系统。原"plan-weather-zone-override"占位作废。
```

### Blueprint zone 候选（首版 3 处）

```json
[
  {
    "name": "blood_valley_east_scorch",
    "display_name": "血谷东陲焦土",
    "aabb": { "min": [3500, 50, -2700], "max": [4500, 110, -2300] },
    "size_xz": [1000, 400],
    "spirit_qi": 0.30,
    "danger_level": 6,
    "worldgen": {
      "terrain_profile": "tribulation_scorch",
      "shape": "elongated",
      "boundary": { "mode": "hard", "width": 80 },
      "landmarks": ["lodestone_vortex", "lightning_basalt_pit"]
    }
  },
  {
    "name": "north_waste_east_scorch",
    "display_name": "北荒东陲焦土",
    "aabb": { "min": [1500, 60, -8500], "max": [2700, 100, -7500] },
    "size_xz": [1200, 1000],
    "spirit_qi": 0.28,
    "danger_level": 7,
    "worldgen": {
      "terrain_profile": "tribulation_scorch",
      "shape": "irregular_blob",
      "boundary": { "mode": "hard", "width": 80 },
      "landmarks": ["charred_husk_tree"],
      "extras": {
        "ascension_pit_xz": [2100, -8000]
      }
    }
  },
  {
    "name": "drift_scorch_001",
    "display_name": "游离焦土",
    "aabb": { "min": [-4500, 60, 3500], "max": [-3500, 95, 4500] },
    "size_xz": [1000, 1000],
    "spirit_qi": 0.32,
    "danger_level": 5,
    "worldgen": {
      "terrain_profile": "tribulation_scorch",
      "shape": "irregular_blob",
      "boundary": { "mode": "hard", "width": 80 },
      "landmarks": ["glass_fulgurite", "lodestone_vortex"]
    }
  }
]
```

> **化虚遗迹由 zone.extras.ascension_pit_xz 显式定位**——只有 north_waste_east_scorch 有此字段（呼应 world-0004 "二在北荒东陲已殒"），其他两 zone 不带 ascension_pit_xz → structure spawner 不在该 zone 生成遗迹。这种"配置驱动"模式避免 DecorationSpec rarity 采样无法保证"每 zone 0-1"的悖论。

### 数值梯度

| 区位 | qi_density | mofa_decay | qi_vein_flow | flora_density | mineral_density |
|---|---|---|---|---|---|
| 渡劫坑核心 | 0.40 | 0.60 | 0.50 | 0.10 | 0.30（lodestone 富集）|
| 焦土主体 | 0.30 | 0.50 | 0.20 | 0.30 | 0.15（铁/铜）|
| 雷磁柱周围 5 格 | 0.35 | 0.45 | 0.40 | 0.20 | 0.40 |
| 外缘紊乱圈 | 0.20 | 0.55 | 0 | 0.40 | 0.05 |
| 渐变带 | 0.18 | 0.45 | 0 | 0.50 | 0 |

`mofa_decay` 在焦土上是中等（0.50）——天劫劈过的地方理论上"清洁"过一次（雷火烧掉腐朽），但反复劈又重新引入了畸变残余。

## §5 LAYER_REGISTRY 字段映射

```python
extra_layers = (
    "qi_density",
    "mofa_decay",
    "qi_vein_flow",
    "flora_density",
    "flora_variant_id",
    "mineral_density",      # 已存在层；本 profile 是稀有"地表矿露头"profile
    "mineral_kind",
    "anomaly_intensity",    # 雷雨期 + 化虚遗迹周围峰值
    "anomaly_kind",         # **复用** 4 = cursed_echo（天劫余响 = 历代受劫者神识残留，语义最贴）
)
```

**anomaly_kind 选择 = 4 (cursed_echo)**：LAYER_REGISTRY 已有 enum 中 `4 = cursed_echo` 是"诅咒回响 / 神识残留"——最贴"历代天劫劈过的地方留下的精神残响"语义。**不**新增 `6 = tribulation_residue`（避免 enum 通胀；草稿曾提议新增，本版决策回收）。

化虚遗迹周围 30 格内 `anomaly_intensity ∈ [0.6, 0.9]`，`anomaly_kind = 4`；其他焦土主体 `anomaly_intensity ∈ [0.1, 0.3]` 散布峰值（雷击高频区）。

## §6 数据契约（下游 grep 抓手）

| 阶段 | 抓手 | 位置 |
|---|---|---|
| P0 | `tribulation_scorch` profile + 3 zone | `worldgen/terrain-profiles.example.json` + blueprint |
| P1 | `class TribulationScorchGenerator` + `fill_tribulation_scorch_tile` | `worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py`（新增） |
| P1 | `TRIBULATION_SCORCH_DECORATIONS` 7 项 | 同上 |
| P2 | 焦土 zone 读取 `ZoneWeatherProfile::lightning_strike_per_min_override` | `server/src/lingtian/weather_profile.rs` + `server/weather_profiles.json` |
| P2 | 雷雨视觉 bundle 复用现有 weather → environment 映射 | `server/src/world/weather_to_environment.rs::weather_to_environment_bundle` |
| P2 | 真实雷击落点 / 伤害入口 | `server/src/world/weather_physics/lightning.rs::lightning_strike_at` |
| P2 | 雷法染色加成查询 hook 命中本 zone | `server/src/cultivation/style_modifier.rs::for_zone_weather` |
| P3 | `tianjie_ascension_pit` structure spawner（读 zone.extras.ascension_pit_xz 单点生成）| `worldgen/scripts/terrain_gen/structures/ascension_pit.py`（新增） |
| P3 | `xujie_canxie`（虚劫残屑）item id + drop table | `server/assets/items/tianjie/xujie_canxie.toml` |
| P3 | plan-tribulation-v1 实时渡劫事件命中 zone → 在地表写入新 `glass_fulgurite` 记号 | `server/src/tribulation/scorch_record.rs` |
| P2 | 雷击命中率公式（暴烈色 ×0.7 + 金属甲 ×1.5，乘性叠加）| 复用 `style_modifier::for_zone_weather`，焦土编排层只负责传入天气/染色/护甲上下文 |

## §7 实施节点

- [ ] **P0** profile + 3 zone 注册 — 验收：raster manifest 含 3 zone；`ambient_hint` 字段通过 schema 校验（仅文档字段，不进 server 系统消费）
- [ ] **P1** generator + 装饰物 — 验收：raster `mineral_density` 表层（y > 70）峰值命中 lodestone / copper；6 种装饰各自命中 ≥ 1 处（化虚遗迹**不**走装饰，由 P3 structure 接入）
- [ ] **P2** 雷雨实招雷 + 染色加成 + 漏失副作用 — 验收：单测雨天 5 分钟内击中数 ∈ [5, 15]；暴烈色玩家命中率 ×0.7±10%；金属甲玩家 ×1.5±10%；**所有玩家**（含暴烈色）真元漏失 ×1.3±10%——契合 worldview §六 雷法 阴雨副作用对暴烈色不豁免；不得绕过现有 `ZoneWeatherProfile` / `style_modifier::for_zone_weather`
- [ ] **P3** 化虚遗迹 structure（zone.extras.ascension_pit_xz 单点生成）+ 虚劫残屑 + 实时天劫钩子 — 验收：smoke test 化虚 NPC 死亡触发 → zone 内地表新增 1 玻璃熔痕；虚劫残屑 0.5% 掉率统计正确；化虚遗迹仅在 north_waste_east_scorch 出现 1 个，其他 zone 0 个

## §8 开放问题

- [x] ~~zone-scoped 天气改写需立 plan-weather-zone-override~~ **2026-05-11 已解除**：`plan-zone-weather-v1` + `plan-zone-environment-v1` 已 finished；本 plan 只消费 `ZoneWeatherProfile`、`weather_to_environment_bundle`、`lightning_strike_at`、`style_modifier::for_zone_weather`，不新造平行天气系统。
- [ ] 虚劫残屑的用途：plan-skill-v1（雷法残卷材料）vs plan-weapon-v1（雷霆器修高阶载体）vs plan-cultivation-v1（暴烈色染色催化）？P3 启动前三选一；不阻塞 P0-P2。
- [ ] 雷击死亡的运数 / 寿元结算（worldview §十二）：本是天劫直接干预，是否应**算"突破反噬 / 天劫失败"**？倾向**否**——这是"环境天劫"，不是玩家主动渡劫，按正常死亡 Roll
- [ ] 玩家在化虚遗迹周围"假装渡劫"（armor_stand + 雷云）是否触发天劫 narration 误判？建议 P3 阶段做 narration 检测白名单
- [ ] 鞭炮 / TNT 等可触发 lightning_rod 的玩家行为是否破坏 immersion？倾向**禁止**主动招雷（仅天气 + lodestone_vortex 自然招雷）
- [ ] 焦土与 ancient_battlefield 的"空间相对位置"——是否设计某 ancient_battlefield zone 同时被天劫劈过形成"古战场焦土带"复合 zone？倾向**首版不做**（profile 互斥简单实现）
- [ ] 暴烈色 + 重铁甲 = ×0.7 × 1.5 ≈ baseline——这是设计意图（双方互相抵消，让"暴烈色穿 leather 抗劈最优"成为反直觉解）还是 bug（重甲玩家失去抗雷优势）？需要 plan-armor-v1 联动核对

## §9 进度日志

- 2026-04-28：骨架立项。锚点 worldview §八/§三/§十三/§六 + world-0002/0004 化虚叙事。等优先级排序与 plan-tribulation-v1（实时渡劫与本地形互写）+ plan-cultivation-v1（暴烈色加成查询接口）+ plan-mineral-v1（lodestone/copper 表层露头规则）协调。
- 2026-04-28（自查修订）：
  - **strong-4** 修：删"暴烈色 ×2 修炼速度"——违反 worldview §三 cultivation 主公式（`∝ zone.spirit_qi × (qi_current / qi_max)`，无染色项）+ §六 雷法"阴雨/水域真元紊乱"明确是**副作用**而非亲和。改为：暴烈色**抗雷击 ×0.7 命中率**（生存 buff，不进修炼公式）+ **不豁免** 阴雨漏失副作用 +30%（全员吃）。修炼速度照 zone.spirit_qi 走。
  - **mid-7** 修：化虚遗迹 `tianjie_ascension_pit` 从 DecorationSpec 移除（rarity 采样无法保证"每 zone 0-1"约束），改为独立 structure spawner，由 zone.extras.ascension_pit_xz 显式坐标驱动；只有 north_waste_east_scorch zone 有此字段（呼应 world-0004 "二在北荒东陲已殒"）。
  - **mid-12** 修：删 `weather_overrides` profile 字段——MC 天气全局，无 zone-scope 系统消费；改为纯文档 `ambient_hint`，"雷雨实招雷"走 server tick 强制 spawn lightning entity。
  - **weak-13** 修：`anomaly_kind` 复用现有 `4 = cursed_echo`（天劫余响 = 神识残留，语义最贴），删除草稿"提议新增 6 = tribulation_residue"——避免 enum 通胀。
- **2026-04-29**：实地核验 + 决策标注（当时仍处占位阶段，强阻塞——前置 plan-tribulation-v1 仅 schema + 化虚结算两条落地，P1 / P2 渡劫核心机制（预兆锁定 / 雷劫渲染 / location 广播 RPC）大量未实装；本 plan P3 "实时天劫钩子"无 emit 接口可挂；plan-cultivation-v1 无独立 plan 文档，P2 "暴烈色 ×0.7 命中率"无染色查询 API）。
  - **季节联动**（用户决策 2026-04-29）：焦地 qi_density 永久归 0，不参与季节节律；生态层缓慢恢复（每 game-year ×1.05），不与季节耦合。已写入头部锚点 + §0 设计轴心。
  - **§8 第 2 项虚劫残屑用途**：仍待 plan-skill-v1 / plan-weapon-v1 / plan-cultivation-v1 三选一立项；不阻塞 P0–P2，可在 P3 启动前 1 周决策。
  - 补 `## Finish Evidence` 占位。
  - 当时记录的升 active 触发条件已由 2026-05-11 实地核验覆盖；P0-P2 现在不再依赖这些旧阻塞，P3 再处理实时渡劫互写与虚劫残屑用途。
- **2026-05-08**：天气 / 视觉路径重设计。`plan-zone-environment-v1`（zone-scoped 持续视觉协议 + 客户端 emitter 注册表 + mixin 扩展）+ `plan-zone-weather-v1`（zone profile 概率覆写 + WeatherEvent → EnvironmentEffect 映射 + 物理 hook）接管原占位 "plan-weather-zone-override"。
- **2026-05-11**：实地核验发现 `plan-zone-environment-v1` / `plan-zone-weather-v1` 已 finished，且 server 已有三处焦土 weather profile、雷击物理 hook、雷法染色 multiplier；移除旧前置阻塞，升 active。剩余主体是 worldgen profile / generator / structure spawner，P3 的虚劫残屑用途改为阶段内决策。

---

## Finish Evidence

<!-- 全部阶段 ✅ 后填以下小节，迁入 docs/finished_plans/ 前必填 -->

- 落地清单：
  - P0：`worldgen/terrain-profiles.example.json` 加 `tribulation_scorch` profile + `xue_gu_dong_lu_scorch` / `north_waste_east_scorch` 等 zone JSON
  - P1：`worldgen/scripts/terrain_gen/profiles/tribulation_scorch.py`（generator + 7 装饰物 incl. glass_fulgurite / charred_tree / lodestone_pillar）
  - P2：焦土 zone tick / 编排层消费 `ZoneWeatherProfile` + `lightning_strike_at` + `style_modifier::for_zone_weather`（lightning 强制 spawn + 暴烈色 ×0.7 命中率 + 金属甲 ×1.5 + 阴雨漏失 +30%）
  - P3：`tianjie_ascension_pit` structure spawner + 虚劫残屑 loot + plan-tribulation-v1 实时天劫 hook
- 关键 commit：
- 测试结果：
- 跨仓库核验：
  - worldgen：`tribulation_scorch` profile / generator / `tianjie_ascension_pit` structure
  - server：lightning entity spawn / 暴烈色查询 / 化虚遗迹 trigger
  - agent：天劫 narration（依 plan-tribulation-v1 emit 接口）
- 遗留 / 后续：
  - 虚劫残屑用途（依 §8 第 2 项决策）
  - zone-scoped 天气改写已由 `plan-zone-weather-v1` + `plan-zone-environment-v1` 接管；本 plan 不再实现平行天气系统
  - 古战场焦土带复合 zone（§8 第 6 项，首版不做）
  - lightning_rod 主动招雷防御（§8 第 5 项，建议禁止）
