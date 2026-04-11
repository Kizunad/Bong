# 末法残土 — 世界生成

## 概述

当前主流程已经切到 `terrain_gen`：

- `blueprint` 定义固定坐标大地图布局
- `terrain profiles` 生成区域 field
- `stitching` 负责 `zone -> wilderness` 过渡
- `preview exporters` 输出总览图与分区近景图
- `bakers/` 目录当前提供 `worldpainter` 调试导出和 `raster` 运行时地形导出

Datapack + Chunky + postprocess 仍保留为旧流程参考，但不再是固定布局大地图的主路径。

当前建议把 `../server/zones.worldview.example.json` 作为世界蓝图输入：

- Server 用它定义 authoritative zones
- `scripts/postprocess.py` 用它筛选需要处理的 zone/chunk
- 后续 worldgen/postprocess 可继续消费其中的 `worldgen.*` 扩展字段

第一版地貌规则见：

- `../docs/worldgen-terrain-profiles.md`
- `../docs/worldgen-pipeline-v2.md`
- `terrain-profiles.example.json`

---

## 目录结构

```
worldgen/
├── README.md                          ← 本文档
├── worldgen.sh                        ← Phase A 一键预生成脚本
├── pipeline.sh                        ← Phase A + Phase B 串联脚本
├── scripts/
│   └── postprocess.py                 ← Phase B Python 后处理脚本
├── .venv/                             ← Python 虚拟环境（不提交到 git）
├── worldgen-mofa/                     ← Datapack（放入 world/datapacks/）
│   ├── pack.mcmeta                    ← Datapack 声明（pack_format: 15 = MC 1.20）
│   └── data/
│       ├── minecraft/dimension/
│       │   └── overworld.json         ← 覆盖 overworld 维度，替换 biome_source
│       └── mofa/worldgen/
│           ├── biome/
│           │   ├── spawn_haven.json       ← 出生安全区
│           │   ├── kui_zeng.json          ← 馈赠区（普通）
│           │   ├── kui_zeng_rich.json     ← 馈赠区（高灵气）
│           │   ├── si_yu.json             ← 死域
│           │   ├── fu_ling.json           ← 负灵域
│           │   └── fu_ling_abyss.json     ← 坍缩渊
│           └── noise_settings/
│               └── blood_valley.json      ← 血谷地形（自定义噪声设置）
└── server/                            ← Fabric 1.20.1 预生成服务端（不提交到 git）
    ├── fabric-server-launch.jar
    ├── mods/
    │   ├── fabric-api.jar
    │   └── chunky-fabric.jar
    └── mofa-world/region/             ← 生成的世界存档
```

---

## 快速开始

```bash
cd worldgen

# 1) 直接跑 terrain_gen 主流程（默认导出 raster）
bash pipeline.sh ../server/zones.worldview.example.json generated/terrain-gen-smoke raster

# 2) 查看重点预览
#   generated/terrain-gen-smoke/focus-layout-preview.png
#   generated/terrain-gen-smoke/focus-surface-preview.png
#   generated/terrain-gen-smoke/focus-height-preview.png

# 3) 查看分区近景
#   generated/terrain-gen-smoke/zone-blood_valley-surface-preview.png
#   generated/terrain-gen-smoke/zone-qingyun_peaks-height-preview.png
#   generated/terrain-gen-smoke/zone-north_wastes-layout-preview.png
```

也可以直接调用模块：

```bash
cd worldgen
python3 -m scripts.terrain_gen \
  --blueprint ../server/zones.worldview.example.json \
  --output-dir generated/terrain-gen-smoke \
  --backend raster
```

当前关键输出：

- `generated/terrain-gen-smoke/terrain-plan.json`
- `generated/terrain-gen-smoke/terrain-fields-summary.json`
- `generated/terrain-gen-smoke/focus-*.png`
- `generated/terrain-gen-smoke/zone-*.png`
- `generated/terrain-gen-smoke/rasters/manifest.json`

如果要让 server 直接消费 raster 并在运行时生成 chunk：

```bash
cd worldgen
python3 -m scripts.terrain_gen \
  --blueprint ../server/zones.worldview.example.json \
  --output-dir generated/terrain-gen-raster \
  --backend raster

# 然后让 server 直接读取 manifest
cd ../server
BONG_TERRAIN_RASTER_PATH=/home/kiz/Code/Bong/worldgen/generated/terrain-gen-raster/rasters/manifest.json cargo run
```

`worldpainter` 后端仍可用于预览/调试 raster 是否合理，但正式运行路径已经切到
`raster -> Rust TerrainProvider -> 按需 chunk 生成`。

---

## 核心原理

### 1. 覆盖 overworld 维度

`data/minecraft/dimension/overworld.json` 是关键入口。当 datapack 中存在这个文件时，MC 会用它**替代默认的 overworld 生成逻辑**。

```json
{
  "type": "minecraft:overworld",       // 维度类型（决定光照、天空、高度范围）
  "generator": {
    "type": "minecraft:noise",         // 使用噪声生成器（不是 flat 或 debug）
    "settings": "minecraft:overworld", // 地形形状用原版 overworld 的 noise_settings
    "biome_source": {                  // ← 这里替换了 biome 来源
      "type": "minecraft:multi_noise", // 多噪声 biome 选择器
      "biomes": [...]                  // 自定义 biome 列表
    }
  }
}
```

**关键点**：`settings` 仍然引用 `minecraft:overworld`，所以地形的高度曲线、洞穴、海平面等和原版完全一致。我们只替换了 biome 选择逻辑——用自定义的 6 个 biome 替代原版 60+ 个 biome。

### 2. Multi-Noise Biome 选择

MC 1.18+ 使用 **6 维噪声空间** 来决定每个位置生成哪个 biome：

| 参数 | 含义 | 范围 |
|------|------|------|
| `temperature` | 冷 ↔ 热 | -1.0 ~ 1.0 |
| `humidity` | 干 ↔ 湿 | -1.0 ~ 1.0 |
| `continentalness` | 海洋 ↔ 内陆 | -1.0 ~ 1.0 |
| `erosion` | 侵蚀强 ↔ 弱（影响地形平坦度） | -1.0 ~ 1.0 |
| `weirdness` | 正常 ↔ 奇异（影响山峰/河谷） | -1.0 ~ 1.0 |
| `depth` | 地表 ↔ 地下 | 0.0 ~ 1.0 |

每个 biome 声明自己在这 6 维空间中的"舒适区"（一个范围），MC 会为每个方块位置采样噪声值，找到**距离最近**的 biome。

**`offset`** 是一个偏移量——值越小，该 biome 越容易被选中（0.0 = 最高优先级）。`spawn_haven` 设了 0.05 的微小偏移，确保出生点附近大概率是安全区。

### 3. 我们的 Biome 分布策略

```
                     hot (+temp)
                        │
          si_yu         │       kui_zeng_rich
        (干+热+侵蚀)    │       (湿+热+未侵蚀)
                        │
   dry ─────────────────┼──────────────────── wet
                        │
          fu_ling       │       spawn_haven
        (冷+干+奇异)    │       (中温+中湿+大陆+平静)
                        │
                     cold (-temp)

   fu_ling_abyss = 极端角落（极冷+极干+海洋侧+低侵蚀+高奇异）
   kui_zeng      = 温热+半干+负奇异（savanna 风格）
```

**世界观映射**：
- **温度 → 灵气活跃度**：高温 = 灵气曾经充沛（现在被抢光了）
- **湿度 → 灵气残留**：高湿 = 还有灵气（富灵区），低湿 = 枯竭
- **大陆性 → 安全度**：越内陆越安全（spawn_haven），海洋侧是负灵域
- **奇异度 → 扭曲度**：高奇异 = 负灵域的扭曲地形

---

## Biome 配置详解

### 每个 biome JSON 的结构

```json
{
  "has_precipitation": true/false,     // 是否降雨/降雪
  "temperature": 0.7,                  // 温度（影响草色、是否降雪）
  "downfall": 0.5,                     // 降水量（影响草/树叶颜色）

  "effects": {
    "sky_color": 7907327,              // 天空颜色（十进制 RGB）
    "fog_color": 12638463,             // 雾颜色
    "water_color": 4159204,            // 水颜色
    "water_fog_color": 329011,         // 水下雾颜色
    "grass_color": 7842607,            // 草方块顶部颜色（覆盖默认计算）
    "foliage_color": 7842607,          // 树叶颜色
    "mood_sound": {...}                // 环境音效（洞穴声等）
  },

  "carvers": {                         // 地形雕刻器（洞穴）
    "air": "minecraft:cave"
  },

  "features": [                        // 11 个生成步骤（generation steps）
    [],  // 0: RAW_GENERATION
    [],  // 1: LAKES
    [],  // 2: LOCAL_MODIFICATIONS
    [],  // 3: UNDERGROUND_STRUCTURES
    [],  // 4: SURFACE_STRUCTURES
    [],  // 5: STRONGHOLDS
    [],  // 6: UNDERGROUND_ORES
    [],  // 7: UNDERGROUND_DECORATION
    [],  // 8: FLUID_SPRINGS
    [],  // 9: VEGETAL_DECORATION ← 树/草/花在这里
    []   // 10: TOP_LAYER_MODIFICATION
  ],

  "spawners": {...},                   // 生物刷怪表
  "spawn_costs": {}                    // 生物密度控制
}
```

### 6 个 Biome 对比

| Biome | 世界观 | 降雨 | 温度 | 地表风格 | 植被 | 怪物 | 色调 |
|-------|--------|------|------|---------|------|------|------|
| `spawn_haven` | 出生安全区 | ✓ | 0.7 | 草地 | 橡树+花+草 | 无怪 | 自然绿 #77A62F |
| `kui_zeng` | 馈赠区 | ✓ | 0.9 | 稀树草原 | 金合欢+草+枯灌木 | 僵尸、蜘蛛 | 黄绿 #A3D354 |
| `kui_zeng_rich` | 高灵气馈赠区 | ✓ | 0.8 | 茂密丛林 | 橡树+花+草 | 僵尸、骷髅 | 翠绿 #4CB850 |
| `si_yu` | 死域 | ✗ | 2.0 | 沙漠 | 仅枯灌木 | 尸壳 | 灰黄 #B7B248 |
| `fu_ling` | 负灵域 | ✗ | 0.2 | 深板岩质感 | 无 | 末影人、骷髅 | 暗青 #637F43 |
| `fu_ling_abyss` | 坍缩渊 | ✗ | 0.0 | 末地石质感 | 无 | 末影人 | 紫黑 #404040 |

### 颜色值转换

biome JSON 中的颜色是十进制整数，和十六进制的对应关系：

```
十六进制 → 十进制
#77A62F  → 7842607   (spawn_haven 草色)
#A3D354  → 10735444  (kui_zeng 草色)
#9D9F97  → 10329495  (si_yu 雾色)
#5E6E93  → 6186090   (fu_ling 天空色)

转换公式: parseInt("77A62F", 16) = 7842607
反向:     (7842607).toString(16) = "77a62f"
```

---

## 预生成流程

### 依赖

| 工具 | 版本 | 说明 |
|------|------|------|
| Java | 17+ | 服务端运行环境（项目已有 Java 21） |
| Fabric Server | 1.20.1 + Loader 0.16.10 | MC 服务端 |
| Fabric API | 0.92.7+1.20.1 | Chunky 依赖 |
| Chunky | 1.3.146 (Fabric) | chunk 预生成 mod |

### 手动流程

```bash
cd worldgen/server

# 1. 安装 datapack
mkdir -p mofa-world/datapacks
cp -r ../worldgen-mofa mofa-world/datapacks/

# 2. 启动服务端
java -Xmx2G -jar fabric-server-launch.jar --nogui

# 3. 在控制台输入 Chunky 命令
chunky radius 512      # 设置半径（格数）
chunky start           # 开始预生成
# 等待 "Task finished" 日志

# 4. 保存并退出
save-all flush
stop
```

### 一键脚本

```bash
cd worldgen && bash worldgen.sh 512  # 参数是半径格数
```

### 输出

生成的世界在 `server/mofa-world/region/` 目录，包含 `.mca` 文件（Anvil 格式）。

---

## 肉眼验证

```bash
# 启动服务端（端口 25566，离线模式）
cd worldgen/server
java -Xmx2G -jar fabric-server-launch.jar --nogui

# MC Java 1.20.1 客户端连接 localhost:25566
# /gamemode creative → 飞行查看地形
# F3 调试界面可以看到当前所在 biome（如 mofa:si_yu）
```

---

## 调参指南

### 想改 biome 分布比例？

修改 `overworld.json` 中各 biome 的 `parameters` 范围：
- 扩大范围 = 该 biome 出现更多
- 缩小范围 = 出现更少
- `offset` 越小 = 越优先（同一区域有多个 biome 竞争时）

### 想改 biome 外观？

修改对应的 `biome/*.json`：
- `grass_color` / `foliage_color` → 草/叶颜色
- `sky_color` / `fog_color` → 天空/雾
- `features[9]` → 植被（树/草/花）
- `spawners` → 刷怪

### 想加矿石/洞穴？

在 `features` 数组对应的步骤里添加原版 placed_feature：
- `features[6]` = 矿石（如 `"minecraft:ore_iron_upper"`）
- `features[8]` = 泉水（如 `"minecraft:spring_water"`）
- `carvers.air` = 洞穴（如 `["minecraft:cave", "minecraft:cave_extra_underground"]`）

**注意**：所有 biome 的同一步骤内，feature 顺序必须一致，否则会报 "Feature order cycle" 错误。

### 想要全新地形形状（不是原版高度曲线）？

在 `data/mofa/worldgen/noise_settings/` 下创建自定义 noise_settings JSON，然后在 `overworld.json` 中把 `"settings": "minecraft:overworld"` 改为 `"settings": "mofa:custom"`。可用 [Misode 生成器](https://misode.github.io/worldgen/noise-settings/) 可视化编辑。

---

## Phase B — Python 后处理

Phase A 生成基础地形后，用 Python 脚本在 Anvil (.mca) 文件上做方块级装饰增强。

### 环境搭建

```bash
cd worldgen
python3 -m venv .venv
source .venv/bin/activate
```

### 运行后处理

```bash
cd worldgen
source .venv/bin/activate

# 确保 MC 服务端已关闭（session.lock 不能被占用）
python3 scripts/postprocess.py              # 默认处理 server/mofa-world
python3 scripts/postprocess.py path/to/world  # 或指定路径
```

### 后处理内容

脚本扫描每个 chunk 的地表（最高固体方块上方为空气的位置），按 Y 高度分层散布装饰：

| 层 | Y 范围 | 装饰方块 | 概率 |
|---|--------|---------|------|
| high_peak | >200 | skeleton_skull, cobweb, soul_lantern | 0.1~0.5% |
| mid_slope | 80~200 | dead_coral_fan (3种), blackstone_button, cobweb | 0.2~0.4% |
| low_valley | 30~80 | sculk_sensor, dead_bush, brown_mushroom | 0.2~0.5% |
| abyss | <30 | sculk, sculk_sensor, sculk_vein | 0.3~0.8% |

**特殊规则**：岩浆块 (`magma_block`) 上方 30% 概率放发光地衣 (`glow_lichen`)，5% 放菌光体 (`shroomlight`)。

### 技术细节

MC 1.18+ chunk section 的 `block_states` 使用 packed long array 存储 palette 索引：
- `palette`：方块类型列表（Compound + Name + Properties）
- `data`：64-bit long 数组，每个 entry 占 `max(4, ceil(log2(palette_size)))` bits
- Entry 不跨越 long 边界，索引顺序为 YZX（`y*256 + z*16 + x`）
- 脚本中 `decode_packed_indices()` / `encode_packed_indices()` 实现了完整的编解码

### 调参

编辑 `scripts/postprocess.py` 中的 `DECORATIONS` 字典和 `MAGMA_NEIGHBORS_DECORATIONS` 列表：

```python
# 示例：在中层增加一个新装饰
"mid_slope": {
    "y_min": 80, "y_max": 200,
    "surface_scatter": [
        (block("dead_brain_coral_fan", {"waterlogged": "false"}), 0.004),
        # 新增：
        (block("candle", {"candles": "1", "lit": "false", "waterlogged": "false"}), 0.002),
    ],
},
```

`SOLID_BLOCKS` 集合决定哪些方块被视为"地表"——如果换了 noise_settings 的方块调色板，记得同步更新。

---

## 完整 Pipeline

```bash
cd worldgen

# Phase A: Datapack 预生成
bash worldgen.sh 512                       # 半径 512 格

# Phase B: Python 后处理
source .venv/bin/activate
python3 scripts/postprocess.py             # 装饰增强

# 验证: 启动服务端查看效果
cd server && java -Xmx2G -jar fabric-server-launch.jar --nogui
# MC 1.20.1 客户端连 localhost:25566
```

---

## 后续规划

1. **Valence 集成** — `AnvilLevel::new()` 加载预生成世界
2. **AI 天道介入** — Agent 生成/修改 datapack JSON + 运行时方块替换
3. **Zone 系统** — 按 zones.json 做精确区域控制（方块退化、结构放置）

详见 [docs/plan-worldgen.md](../docs/plan-worldgen.md)。
