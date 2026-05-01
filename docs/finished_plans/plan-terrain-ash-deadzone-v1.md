# Bong · plan-terrain-ash-deadzone-v1

**余烬死域**（`ash_dead_zone`）。真正的 `qi_density = 0` 死地——不是 waste_plateau 的 0.05 灵气贫瘠，而是 worldview §二 死域章原文："环境与你互不干涉，真元只随时间自然流失。没有任何资源，只有无尽的赶路与消耗。" 没有生物（除了拟态灰烬蛛），脚下踩出可追踪脚印的残灰方块，唯一物资是历代过路死者的干尸残骸。

**世界观锚点**：
- `worldview.md §二 灵压环境 · 死域`（"环境与你互不干涉，真元只随时间自然流失。死域是安全与绝望并存的地带——没人会来，但你也什么都得不到"）
- `worldview.md §二 馈赠区 · 残灰方块`（"灵气下降导致植物枯萎，土石沙化为「残灰方块」（踩上去减速、留下可追踪的脚印）"——死域是残灰方块的总宿主）
- `worldview.md §七 拟态灰烬蛛`（"死域边缘的伏击者。外观材质与残灰方块完全一致"——死域专属生态位）
- `worldview.md §十 资源与匮乏`（"灵气会被先来的人吸干，好地方是会被用完的，然后变成废地"——死域是衰减终点）
- `worldview.md §十七 地形对季节的响应`（**死域 / 余烬死地 = 恒 0，不受季节影响** —— 死地是绝对死地，节律对它无意义；天道吐纳已绕开此区。冬季不会让死域 qi 上来，汐转期也不会引起波动）

**library 锚点**：
- `ecology-0003 灵气零和记`（"末法之活法乃负数之活法——非求增，乃求少减"——死域是这条法则的物理终极）
- `world-0002 末法纪略`（第二变 灵气枯竭——死域是渐枯曲线的尾端）
- `geography-0002 六域舆图考`（六域之间荒野"灵气近零之荒野"——本 plan 把"近零"细分出真正的"零"区）

**交叉引用**：
- `plan-cultivation-v1`（真元回复仅靠灵气浓度——死域内真元只损不增，自动 tick 流失）
- `plan-mineral-v1`（死域不刷矿——但历代死者掉落的退活骨币、磨损装备是少量 loot 源）
- `plan-tsy-zone-v1`（坍缩渊塌缩成死坍缩渊——死域是其地表对应物，共享"零灵气"美学）
- `plan-perception-v1`（死域不刷生物 → 玩家境界感知"50 格内生物气息"在此为静默 → 反而成为安全信号）
- `plan-shelflife-v1`（死域内灵草/丹药漏失速率 +200%——零灵气环境对真元载体最不友好）

**阶段总览**：
- P0 ⬜ `terrain-profiles.example.json` 注册 + 一个固定死域 zone（建议放南荒外缘 (-3000, 8000)）
- P1 ⬜ `AshDeadZoneGenerator` 实装（地形 field + 装饰物 + 残灰方块铺地）
- P2 ⬜ 死域专属 tick：玩家进入后真元自然流失 +50%、灵草丹药 shelflife 漏失 ×3、生物 spawn ban
- P3 ⬜ 拟态灰烬蛛 spawn rule（边缘 50 格密度峰）+ 干尸/骸骨 loot pile structure

---

## §0 设计轴心

- [ ] **真"零"**：核心区 `qi_density = 0.0`（不是接近零，是清零）；waste_plateau 是 0.05 ~ 0.15 退化高原，二者并存且**死域更极端**
- [ ] **绝对寂静**：mob spawn ban（除拟态灰烬蛛在边缘 50 格内可刷）；ambient effect 全部移除——客户端无音效、无粒子、连风都不吹
- [ ] **残灰方块为主**：地表 95%+ 用一种"灰化"基底 block——`coarse_dirt + gravel + sand` 混叠，配合 §二"踩上去减速 + 留下可追踪脚印"
- [ ] **唯一资源是死者**：表层散布"干尸堆 / 骸骨堆"小型 structure，凡铁、退活骨币、干灵草——loot 全是历代过路者的遗物（呼应 §十六 坍缩渊的 99% 探索者遗物逻辑，但死域版是地表暴露态）
- [ ] **不能修炼，只能赶路**：死域内静坐**真元不回**（zone.spirit_qi=0 → cultivation tick 公式 `× 0`）；玩家应该感觉到"这里待越久越亏"
- [ ] **季节免疫**（worldview §十七）：死域不参与 §十七 二季 / 汐转节律——`Season::*_modifier()` 在死域 zone 内一律 `× 0` 短路；不写 ZoneSeasonState（节省存储）；客户端 HUD 显示 "无节律" 而非具体季节标签——玩家该知道这里"时间停了"

## §1 世界观推断逻辑（为何此地必然存在）

> 灵气零和（ecology-0003）+ 末法每纪衰减 1-3%（worldview §一 / world-0002）= **历史上必有些区域被吸干至零**，且天道的灵气重分配速度远低于消耗速度（worldview §十）。

死域是这条曲线的**逻辑末态**：
- **过度开采的旧馈赠区**：曾是宗门时代的灵气富集地（0.6+），万人同坐三十年后吸干 → 灵脉断裂 → 永久零灵气
- **天道不修复**：worldview §八 天道"温和手段：在无人区慢慢恢复灵气"——但**修复速度远低于消耗**，部分死域是修复速率为 0 的"绝育区"（灵脉通道彻底闭合）
- **死域 ≠ 负灵域**：死域是"灵气=0"，环境**不抽**真元，只是不补；负灵域是"灵气<0"，环境**主动抽**。死域是绝望的安全（不会主动死你）；负灵域是杀戮的悖论（高境者更危险）。worldview §二 明确区分二者
- **死域作为路径**：六域之间的近零荒野是 0.05-0.15，死域是其中**真正断脉**的内核——典型死域常嵌在 waste_plateau 内或位于已耗尽的旧馈赠区故地

## §2 特殊机制

| 机制 | 触发 | 效果 |
|---|---|---|
| **真元零回复 + 统一被动流失** | 玩家在死域内（`qi_density < 0.01`）| `qi_per_tick = 0`；被动流失**统一速率** −1 真元 / 分钟（不分境界——worldview §二 死域章原文"环境与你互不干涉，真元只随时间自然流失"明确**不抽**，只是不补 + 自然挥发；与 worldview §四"真元 = 0 → 10 分钟内不补充就降境界"baseline 对齐。**境界相关的非线性抽吸只属于负灵域 / 渊口荒丘，不属于死域**。） |
| **shelflife 漏失加速** | 物品在死域内 | `decay_rate × 3.0` zone multiplier——**plan-shelflife-v1 已归档**，本 plan P2 顺手扩展 shelflife 模块加 `zone_multiplier_lookup` 钩子（详见 §6）；新接口对所有 profile 透明 |
| **生物 spawn ban** | 全 zone 内 | 普通 mob 不刷；只有拟态灰烬蛛 / 道伥（坍缩渊塌缩外溢，§十六.六）可在边缘出现 |
| **脚印追踪** | 玩家走过残灰地表 | 留下持续 5-15 分钟的足迹（block tag 或 entity）——成为追兵线索 |
| **拟态灰烬蛛伏击** | 玩家穿过边缘 50 格圈 | 灰烬蛛伪装为残灰方块，玩家踩上 → 暴起；密度按 `flora_variant_id == ASH_SPIDER_LAIR` 决定 |
| **死者遗留 loot pile** | structure 生成 | 干尸 / 骸骨堆 → 凡铁 + 退活骨币 + 干灵草；3-5 秒搜刮（与坍缩渊 §十六.三 容器分层共享美学） |
| **境界感知反信号** | 凝脉+ 玩家进入 | "50 格内生物气息" = 0 → narration: "此处寂静如深井底"；老玩家用此**作为 GPS**判断进了死域 |

## §3 独特装饰物（DecorationSpec 预填）

```python
ASH_DEAD_ZONE_DECORATIONS = (
    DecorationSpec(
        name="cantan_block_drift",
        kind="shrub",
        blocks=("coarse_dirt", "gravel", "sand"),
        size_range=(1, 2),
        rarity=0.85,
        notes="残灰堆：粗土 + 沙砾 + 沙——三件混叠的小灰堆。"
              "地表覆盖率极高（视觉基底）。脚印实装在 server 层，"
              "本装饰只负责 visual 灰白颗粒感。",
    ),
    DecorationSpec(
        name="dried_corpse_mound",
        kind="boulder",
        blocks=("bone_block", "dirt", "dead_bush"),
        size_range=(2, 3),
        rarity=0.20,
        notes="干尸堆：骨块为骨架 + 半埋灰土 + 枯灌木——历代死者残骸。"
              "loot 锚：凡铁 / 退活骨币 / 干灵草（rarity 反比稀有度）。"
              "高境感知能读出生前境界轮廓（叙事 hook）。",
    ),
    DecorationSpec(
        name="petrified_tree_stump",
        kind="tree",
        blocks=("polished_diorite", "stripped_oak_log", "dead_bush"),
        size_range=(2, 4),
        rarity=0.30,
        notes="石化枯桩：闪长岩石化树干 + 剥皮原木残留 + 顶端枯灌木。"
              "曾是高灵气区的活树，灵气抽干后木质石化。死域专属符号。",
    ),
    DecorationSpec(
        name="ash_spider_lair",
        kind="boulder",
        blocks=("coarse_dirt", "cobweb", "gray_concrete_powder"),
        size_range=(1, 2),
        rarity=0.10,
        notes="灰烬蛛巢：粗土 + 蛛网 + 灰混凝土粉。视觉与 cantan_block_drift "
              "几乎一致，只多一缕极淡蛛丝——这是诱饵。server 在此 spawn "
              "拟态灰烬蛛（§七）。",
    ),
    DecorationSpec(
        name="silent_obelisk",
        kind="boulder",
        blocks=("smooth_stone", "stone", "andesite"),
        size_range=(3, 5),
        rarity=0.08,
        notes="无声碑：光滑石 + 普通石 + 安山岩——没有任何文字 / 雕刻。"
              "极稀少。象征'天道不再眷顾此地'——连碑文都不肯刻。",
    ),
    DecorationSpec(
        name="vanished_path_marker",
        kind="shrub",
        blocks=("cobblestone_wall", "torch", "stone_button"),
        size_range=(1, 1),
        rarity=0.05,
        notes="消亡路标：圆石墙 + 火把（实际不点燃，只是结构）+ 石按钮。"
              "前人留下的导航标，火把熄灭已久——'这条路曾有人走，"
              "走过的人没回来'。",
    ),
)
```

`ambient_effects = ()`——死域**主动留空**。客户端读到空 effect 列表 → 关闭风声、关闭粒子。

## §4 完整 profile 配置

### `terrain-profiles.example.json` 追加

```json
"ash_dead_zone": {
  "height": { "base": [70, 82], "peak": 88 },
  "boundary": { "mode": "hard", "width": 64 },
  "surface": ["coarse_dirt", "gravel", "sand", "smooth_stone", "stone"],
  "water": { "level": "none", "coverage": 0.0 },
  "passability": "high",
  "spawn_blacklist": "all_natural_mobs",
  "ambient_effects": [],
  "structure_density": { "dried_corpse_mound": 0.04, "silent_obelisk": 0.005 }
}
```

### Blueprint zone 候选（首版固定一处）

```json
{
  "name": "south_ash_dead_zone",
  "display_name": "南荒余烬",
  "aabb": { "min": [-2200, 60, 7000], "max": [-200, 110, 9000] },
  "center_xz": [-1200, 8000],
  "size_xz": [2000, 2000],
  "spirit_qi": 0.0,
  "danger_level": 5,
  "worldgen": {
    "terrain_profile": "ash_dead_zone",
    "shape": "irregular_blob",
    "boundary": { "mode": "hard", "width": 64 },
    "landmarks": ["silent_obelisk", "vanished_path_marker"]
  }
}
```

> 选址理由：南荒（worldview §十三 "渐变荒原，世界边界"）地理上对应"被遗弃的边境"——死域放此处不冲突任何已有 zone，且远离出生点（强迫玩家有意识赶路）。

### 数值梯度（按距离中心 r / 半径归一化的 `t`）

| 区位 | t | qi_density | mofa_decay | qi_vein_flow | flora_density | spawn |
|---|---|---|---|---|---|---|
| 死域核心 | 0-0.5 | **0.00** | 0.95 | 0 | 0.10（仅 cantan / 干尸 / 石化桩）| ban all |
| 主体 | 0.5-0.8 | 0.02 | 0.90 | 0 | 0.20 | ban all |
| 边缘 | 0.8-1.0 | 0.05 | 0.75 | 0 | 0.30（含 ash_spider_lair）| 拟态灰烬蛛 only |
| 渐变 | 1.0-1.2 | 0.10 | 0.55 | 0 | 0.40 | 普通荒野 mob |

## §5 LAYER_REGISTRY 字段映射

```python
extra_layers = (
    "qi_density",
    "mofa_decay",
    "qi_vein_flow",       # 始终 0（死域无灵脉）——但写入显式 0 让 stitcher maximum blend 不漏
    "flora_density",
    "flora_variant_id",
    "feature_mask",       # 用于"残灰场"覆盖率（client 渲染脚印能见度）
)
```

`qi_vein_flow ≡ 0` 是死域的**契约**——任何越界写入非零都视为 bug。raster_check.py 加一条：`assert ash_dead_zone tile.qi_vein_flow.max() == 0.0`。

## §6 数据契约（下游 grep 抓手）

| 阶段 | 抓手 | 位置 |
|---|---|---|
| P0 | `ash_dead_zone` profile | `worldgen/terrain-profiles.example.json` |
| P0 | zone `south_ash_dead_zone` | blueprint `zones.worldview.example.json` |
| P1 | `class AshDeadZoneGenerator` + `fill_ash_dead_zone_tile` | `worldgen/scripts/terrain_gen/profiles/ash_dead_zone.py`（新增） |
| P1 | `ASH_DEAD_ZONE_DECORATIONS` 6 项 | 同上 |
| P2 | `struct DeadZoneTickHandler { qi_drain_per_minute = 1.0, shelflife_zone_multiplier = 3.0 }` | `server/src/cultivation/dead_zone.rs`（新增） |
| P2 | `MobSpawnFilter::ban_in_dead_zone` | `server/src/world/mob_spawn.rs` |
| P2 | shelflife zone multiplier `× 3.0` 钩子（**plan-shelflife-v1 已归档**——本 plan P2 顺手扩展 `server/src/shelflife/decay.rs::tick` 加 `zone_multiplier_lookup(item, zone_qi_density) -> f32` 接口；现 `decay_per_tick` 是 profile 固定参数无法 zone-scope 调速；新接口对所有 profile 透明，不破坏现有 registry 行为） | `server/src/shelflife/decay.rs::tick` + `server/src/shelflife/registry.rs` |
| P3 | `dried_corpse_mound` structure spawner | `worldgen/scripts/terrain_gen/structures/corpse_mound.py`（新增） |
| P3 | 拟态灰烬蛛密度规则 `flora_variant_id == 4 → spawn weight ×8` | `server/src/mob/ash_spider.rs` |
| P3 | raster_check pin: `qi_vein_flow.max() == 0` for dead zone tiles | `worldgen/scripts/terrain_gen/harness/raster_check.py` |

## §7 实施节点

- [ ] **P0** profile + zone — 验收：`pipeline.sh` 通过；raster manifest 包含 `south_ash_dead_zone`；`Season` enum 在此 zone 内 short-circuit 测试通过（`Season::*_modifier()` 返回 1.0 / 0.0 而非真实季节修饰）
- [ ] **P1** generator + 装饰物 — 验收：raster 中心 `qi_density.max() < 0.01`；`flora_variant_id` 命中全部 6 种；`ambient_effects` 数组为空（client 静音验证）；6 种装饰物每种至少 ≥ 1 个单测覆盖出现
- [ ] **P2** tick + spawn ban + shelflife 钩子 — 验收：
  - 单测：玩家在 zone 内 60 秒真元下降 = **1 真元**（−1 / minute × 1 min；不分境界）
  - 单测：醒灵 / 引气 / 凝脉 / 固元 / 通灵 / 化虚 6 境界各跑一遍，速率全部恒为 −1 / min（验证"不分境界"）
  - 单测：mob spawn list 中无 zombie/skeleton/creeper（whitelist 仅含 ash_spider / 道伥）
  - e2e：带入的 fresh herb（half_life=72h base profile）在死域 1 game-day 后 freshness ≈ `(1 - 1/72)³` ≈ 0.96（×3 zone multiplier 验证）
  - e2e：同一 herb 在普通 zone 1 game-day 后 freshness ≈ 0.986（基线对照）
- [ ] **P3** corpse_mound + 灰烬蛛规则 + raster_check — 验收：
  - smoke test 跑通；
  - 玩家在边缘 50 格内 100 次穿越中至少 3 次触发蛛伏击（statistical: 3-15 次为通过，> 15 次报警密度过高）；
  - raster_check pin: `assert ash_dead_zone tile.qi_vein_flow.max() == 0.0`
  - corpse_mound loot 命中 ≥ 3 种（凡铁 / 退活骨币 / 干灵草，各至少 1 个 fixture spawn）

## §8 开放问题

- [ ] 死域内"自动慢老化"是否要做（worldview §十二 寿元——死域内修养几乎不衰减，因没东西损耗）？倾向 **不做**（增加苟在死域的悖论收益）
- [ ] 残灰方块脚印的实现：用一个新自定义 block tag 还是直接用 sculk_vein 改色？（首版选 sculk_vein 改色 + entity overlay，避免新方块注册）
- [ ] 干尸堆 loot pool：退活骨币的"退活"状态如何在 inventory 表达？（依赖 plan-shelflife-v1 freshness profile "expired" 状态）
- [ ] 死域是否允许动态扩张（玩家在边缘大量修炼 → 灵气吸干 → 边缘也死掉）？倾向 **首版不做**（动态扩张需要额外的 zone 边界 mutation 系统）
- [ ] 高阶玩家进死域的视觉提示——是否给 narration "气息全无，此地已死"？（建议给，但仅凝脉+ 触发，作为境界识别能力的奖励）
- [ ] 死域内死亡结算**已由 worldview §十二 正典明定**：死域不满足"死亡地点不在死域/负灵域"运数条件 → **直接进入概率期 Roll**（不是无 Roll，也不是无惩罚——按 §十二 P(重生) 公式正常结算）。本 plan 直接按此实施，不新增特殊规则。

## §9 进度日志

- 2026-04-28：骨架立项。世界观钩子完整对齐 worldview §二 死域章 + §七 拟态灰烬蛛 + 末法纪略灵气衰减叙事。等优先级排序与 plan-shelflife-v1（漏失 ×3 钩子）+ plan-death-lifecycle-v1（死域死亡结算）+ plan-perception-v1（境界静默信号）协调。
- 2026-04-28（自查修订）：
  - **strong-5** 修：删"境界越高真元流失越快"——worldview §二 死域章明确"环境与你互不干涉，真元只随时间自然流失"，**不抽** + 不分境界；境界相关的非线性抽吸只属于负灵域 / 坍缩渊。改为统一速率 −1 真元 / 分钟。
  - **strong-6** 修：§8 死域死亡 "无 Roll" 开放问题改为"按 worldview §十二 正典直接进入概率期 Roll"——不是开放点，已是正典。
  - **mid-8** 修：§2 + §6 明确 shelflife `× 3.0` 是 zone multiplier 接口扩展，**依赖 plan-shelflife-v1 P+ 引入 `zone_multiplier_lookup`**（现 `decay_per_tick` 是 profile 固定参数无法 zone-scope）；不双计、不绕过现 profile registry。
- **2026-04-29**：实地核验 + 升 active 准备。
  - 前置 plan 状态：`plan-shelflife-v1` ✅ **已归档** finished_plans（代码 2770 行已落，但 `zone_multiplier_lookup` 接口未实装——归档不动，**改由本 plan P2 顺手在 server/src/shelflife/decay.rs 加该接口**，作为 ash-deadzone 的工程附加项；本 plan §6 数据契约表已显式标接口扩展点）；`plan-cultivation-v1` 代码已落但无独立 plan 文档（不阻塞——本 plan 真元流失走通用 cultivation tick 而非 cultivation 主公式扩展）；`plan-perception-v1` 仅骨架（不阻塞——本 plan §2 "境界感知反信号" 是被动叙事 hook，不依赖 perception 主动 API）；`plan-death-lifecycle-v1` ✅ finished_plans。
  - **用户决策**（2026-04-29）：死域恒 0 不受季节影响（A 选项）—— 理由：死地是绝对死地，节律对它无意义；选项 B（汐转噪声）破坏"死=不变"语义。已写入 §0 设计轴心 + 头部 worldview §十七 锚点 + §7 P0 验收。
  - 测试阈值数量化（§7）：6 境界统一速率 / shelflife ×3 zone multiplier 公式 / 蛛伏击概率 3-15 次 / 100 / corpse_mound loot ≥ 3 种。
  - 补 `## Finish Evidence` 占位。准备 `git mv` 进 docs/ active。

---

## Finish Evidence

- 落地清单：
  - P0：`worldgen/terrain-profiles.example.json` 加 `ash_dead_zone` profile；`server/zones.worldview.example.json` 加 `south_ash_dead_zone` 固定 zone，带 `active_events=["no_cadence"]`。
  - P1：`worldgen/scripts/terrain_gen/profiles/ash_dead_zone.py` 实装 `AshDeadZoneGenerator`、`fill_ash_dead_zone_tile`、6 种装饰物、恒 0 `qi_vein_flow`。
  - P2：`server/src/cultivation/dead_zone.rs` 接入 `DeadZoneTickHandler`；`server/src/world/mob_spawn.rs` 禁普通自然刷；`server/src/shelflife/compute.rs` / `sweep.rs` / `variant.rs` 接入 `zone_multiplier_lookup`。
  - P3：`worldgen/scripts/terrain_gen/structures/corpse_mound.py` 导出干尸堆 loot 锚；`server/src/mob/ash_spider.rs` 落地边缘 50 格与 `flora_variant_id == 4` 权重规则；`raster_check.py` 已 pin 死域 `qi_vein_flow == 0`。
  - client：`zone_info.active_events` 解析 `no_cadence`，两套 HUD 均显示“无节律”。
- 关键 commit：
  - `ac554099 feat(worldgen): 实装余烬死域地形剖面`
  - `217b8b7e feat(server): 接入余烬死域损耗与禁刷规则`
  - `8aaafddb feat(worldgen): 导出死域干尸堆搜刮锚点`
  - `3ddf2cf3 feat(client): 显示死域无节律状态`
- 测试结果：
  - server：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` ✅（1882 tests passed）
  - client：`JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ./gradlew test build` ✅
  - worldgen：`python3 -m unittest scripts.terrain_gen.test_ash_dead_zone -v` ✅（3 tests）
  - worldgen：`python3 -c "from scripts.terrain_gen.structures import corpse_mound; corpse_mound._test_corpse_mound_loot_pool_contains_three_required_fixtures(); corpse_mound._test_corpse_mounds_emit_only_for_ash_dead_zone()"` ✅
  - worldgen：`bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-ash-smoke raster` ✅（208 tiles synthesized）
  - worldgen：`validate_rasters('generated/terrain-gen-ash-smoke/rasters')` ✅（All 208 tiles passed validation）
- 跨仓库核验：
  - worldgen：`ash_dead_zone` profile / `south_ash_dead_zone` zone / `ASH_DEAD_ZONE_DECORATIONS` / `corpse_mound.py` / raster manifest `corpse_mounds`。
  - server：`DeadZoneTickHandler` / `MobSpawnFilter::ban_in_dead_zone` / shelflife `zone_multiplier_lookup` 接入。
  - client：HUD “无节律” 显示。
- 遗留 / 后续：
  - 死域动态扩张（§8 开放问题——首版不做）
  - 自动慢老化（§8 开放问题——首版不做）
  - 凝脉+ 境界感知 narration（依 plan-perception-v1 / plan-narrative-v1 推进）
