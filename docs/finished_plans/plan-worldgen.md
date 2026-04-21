# 世界生成方案 — 末法残土

> 目标：为 Valence 服务端提供风格化的末法残土地形，支持 AI 天道介入。
> 终态方案 4（Datapack + Amulet 混合），分阶段落地。

---

## 一、架构总览

```
┌──────────────────────────────────────────────────────────────┐
│                    Phase B: Amulet 后处理                     │
│  Python 脚本按 zones.json 做二次加工：                         │
│  · 替换方块材质（灵气退化 → 残灰/sculk）                       │
│  · 放置 Schematic 结构（废墟/灵眼/洞府）                       │
│  · 按 zone spirit_qi 调整植被密度                              │
└──────────────────────┬───────────────────────────────────────┘
                       │ 读写 Anvil .mca
┌──────────────────────▼───────────────────────────────────────┐
│                    Phase A: Datapack 预生成                    │
│  MC 1.20.1 Fabric Server + Chunky                            │
│  · 自定义 noise_settings / biome / feature (JSON)             │
│  · 定义末法残土三类生态 + 出生安全区                             │
│  · Chunky 预生成 → 输出 Anvil 世界存档                         │
└──────────────────────┬───────────────────────────────────────┘
                       │ region/ 目录
┌──────────────────────▼───────────────────────────────────────┐
│                    Valence 加载                                │
│  AnvilLevel::new(&path, &biomes)                              │
│  · 只读加载预生成世界                                          │
│  · 运行时方块替换由 Valence ECS 系统处理                        │
└──────────────────────────────────────────────────────────────┘
```

---

## 二、Phase A — Datapack Worldgen + Chunky 预生成

### 目标

用 MC 原生 worldgen 引擎生成高质量基础地形，输出 Anvil 存档供 Valence 加载。

### 为什么选 Datapack 而不是从零写

| MC 原生引擎白嫖 | 自己写需要实现 |
|----------------|---------------|
| 3D 噪声地形 + 洞穴雕刻 | ✗ |
| Biome 混合与过渡 | ✗ |
| 树/草/花/矿石放置 | ✗ |
| 水体 + 光照计算 | ✗ |
| JSON 驱动，AI 可生成 | ✗ |

### Datapack 结构

```
worldgen-mofa/
├── pack.mcmeta
└── data/
    └── mofa/
        ├── dimension_type/
        │   └── mofa_overworld.json          # 维度类型（高度范围、光照等）
        ├── dimension/
        │   └── overworld.json               # 引用自定义 noise_settings + biome_source
        └── worldgen/
            ├── noise_settings/
            │   └── mofa.json                # 地形形状（高度曲线、洞穴密度、矿层）
            ├── biome/
            │   ├── kui_zeng.json            # 馈赠区：稀疏丘陵、灵草、水源
            │   ├── kui_zeng_rich.json       # 高灵气馈赠区：茂密植被
            │   ├── si_yu.json              # 死域：平坦荒漠、无植被、沙/砂岩
            │   ├── fu_ling.json            # 负灵域：扭曲地形、sculk、深暗
            │   ├── fu_ling_abyss.json      # 坍缩渊：极端负灵域、end_stone
            │   └── spawn_haven.json        # 出生安全区
            ├── configured_feature/
            │   ├── dead_tree.json           # 枯树（死域专用）
            │   ├── spirit_grass.json        # 灵草（馈赠区专用）
            │   ├── ash_patch.json           # 残灰地表斑块
            │   ├── void_rift.json           # 虚空裂缝地物（负灵域）
            │   └── ancient_ruins.json       # 废墟结构入口
            ├── placed_feature/              # 地物放置规则（频率、高度约束）
            ├── density_function/            # 自定义密度函数（可选，高级调参）
            └── multi_noise_biome_source_parameter_list/
                └── overworld.json           # Biome 分布映射（温度/湿度/... → biome）
```

### Biome 设计对应世界观

| Biome | 世界观对应 | 地表 | 植被 | 色调 |
|-------|-----------|------|------|------|
| `spawn_haven` | 出生安全区 | 草地 + 石砖路 | 稀疏橡树 | 自然绿 |
| `kui_zeng` | 馈赠区（灵气 > 0） | 草地/泥土 | 灵草、矮树 | 黄绿 |
| `kui_zeng_rich` | 高灵气馈赠区 | 苔藓/草地 | 茂密树木、花 | 翠绿 |
| `si_yu` | 死域（灵气 = 0） | 沙子/砂岩/灰色混凝土粉末 | 枯灌木、无草 | 灰黄 |
| `fu_ling` | 负灵域（灵气 < 0） | Sculk/深板岩 | 无，偶有蘑菇 | 暗青 |
| `fu_ling_abyss` | 坍缩渊 | 末地石/黑曜石 | 无 | 紫黑 |

### 预生成脚本

```bash
#!/usr/bin/env bash
# scripts/worldgen.sh — 一键预生成末法残土世界
set -euo pipefail

WORLD_DIR="generated-world"
DATAPACK_DIR="worldgen-mofa"
SERVER_JAR="fabric-server-1.20.1.jar"
RADIUS_CHUNKS=64   # 预生成半径（chunk），64 = 1024 格

echo "=== 末法残土世界生成 ==="

# 1. 准备服务端目录
mkdir -p "$WORLD_DIR"
cp -r "$DATAPACK_DIR" "$WORLD_DIR/datapacks/"

# 2. 接受 EULA
echo "eula=true" > eula.txt

# 3. 配置 server.properties
cat > server.properties << 'EOF'
level-name=generated-world
level-type=mofa\:mofa_overworld
enable-command-block=true
spawn-protection=0
max-tick-time=-1
EOF

# 4. 启动服务端 + Chunky 预生成
java -Xmx2G -jar "$SERVER_JAR" --nogui &
SERVER_PID=$!

# 等待服务端就绪
echo "等待服务端启动..."
tail -f logs/latest.log 2>/dev/null | grep -q "Done" || sleep 60

# 通过 RCON 或 stdin pipe 发送命令
send_cmd() {
    # 如果配了 RCON 用 mcrcon，否则用 screen/tmux attach
    echo "$1"
}

send_cmd "chunky radius $RADIUS_CHUNKS"
send_cmd "chunky start"

echo "预生成中... 半径 ${RADIUS_CHUNKS} chunks"
# 等待 Chunky 完成（监控日志）
tail -f logs/latest.log 2>/dev/null | grep -q "Task finished" || sleep 300

send_cmd "save-all flush"
sleep 5
send_cmd "stop"
wait $SERVER_PID

echo "=== 生成完成：$WORLD_DIR/region/ ==="
echo "将此目录配置给 Valence 服务端的 AnvilLevel 路径即可"
```

### Valence 集成

```rust
// server/src/world/terrain.rs（新增）
use valence_anvil::AnvilLevel;

const WORLD_PATH: &str = "generated-world";

pub fn setup_anvil_world(
    mut commands: Commands,
    dimensions: Res<DimensionTypeRegistry>,
    biomes: Res<BiomeRegistry>,
    server: Res<Server>,
) {
    let layer = LayerBundle::new(ident!("overworld"), &dimensions, &biomes, &server);
    let level = AnvilLevel::new(WORLD_PATH, &biomes);
    commands.spawn((layer, level));
}
```

---

## 三、Phase B — Amulet 后处理层

### 目标

在 Phase A 生成的基础地形上，用 Python 脚本做末法残土定制化加工。

### 为什么需要后处理

Datapack 能控制宏观地形，但以下需求超出其能力：

| 需求 | Datapack 能做？ | Amulet 能做？ |
|------|----------------|---------------|
| 按 zones.json 边界精确控制 biome | ✗（MC 用噪声分布） | ✓ 逐 chunk 改写 |
| spirit_qi 值 → 方块退化梯度 | ✗ | ✓ 按灵气值插值替换 |
| 放置 Schematic 结构 | 有限（structure 系统笨重） | ✓ 任意位置任意结构 |
| zone 边界过渡带混合 | ✗ | ✓ 程序化混合 |
| 标记特殊位置（灵眼、伪灵脉） | ✗ | ✓ 放置标记方块/实体 |

### 技术栈

```
pip install amulet-core amulet-nbt
```

- **Amulet-Core**：读写 MC Java/Bedrock 存档，支持 1.12+
- 输入：Phase A 生成的 `generated-world/` 目录 + `zones.json`
- 输出：修改后的同一目录（原地覆写）

### 后处理脚本骨架

```python
#!/usr/bin/env python3
"""scripts/worldgen_postprocess.py — 末法残土 Amulet 后处理"""

import json
import amulet
from amulet.api.block import Block
from pathlib import Path

WORLD_PATH = "generated-world"
ZONES_PATH = "zones.json"

# 方块调色板：spirit_qi 值 → 地表方块替换规则
DECAY_PALETTE = {
    # qi >= 0.7: 不替换，保持原生
    0.3: {  # 0.3 <= qi < 0.7: 部分退化
        "grass_block": "coarse_dirt",
        "oak_leaves": "air",
    },
    0.0: {  # 0 <= qi < 0.3: 严重退化
        "grass_block": "sand",
        "dirt": "sandstone",
        "oak_log": "stripped_oak_log",  # 枯树
        "oak_leaves": "air",
    },
    -999: {  # qi < 0: 负灵域
        "grass_block": "sculk",
        "dirt": "deepslate",
        "stone": "deepslate",
        "oak_log": "air",
        "water": "air",  # 负灵域无水
    },
}

def load_zones(path: str) -> list[dict]:
    with open(path) as f:
        return json.load(f)["zones"]

def get_palette_for_qi(qi: float) -> dict:
    for threshold in sorted(DECAY_PALETTE.keys(), reverse=True):
        if qi < threshold:
            continue
        return DECAY_PALETTE[threshold]
    return DECAY_PALETTE[-999]

def process_zone(level, zone: dict):
    """对单个 zone AABB 范围内的方块做退化替换"""
    qi = zone["spirit_qi"]
    if qi >= 0.7:
        return  # 高灵气区不改

    palette = get_palette_for_qi(qi)
    aabb = zone["aabb"]
    # ... 遍历 AABB 内的 chunk，替换方块
    # (具体实现时用 level.get_chunk + chunk.blocks 操作)

def place_schematics(level, zone: dict):
    """在 zone 内放置预制结构"""
    for feature in zone.get("features", []):
        schem_path = f"schematics/{feature}.schem"
        # Amulet 支持加载 schematic 并粘贴到指定位置
        # ...

def main():
    zones = load_zones(ZONES_PATH)
    level = amulet.load_level(WORLD_PATH)

    try:
        for zone in zones:
            print(f"后处理 zone: {zone['name']} (qi={zone['spirit_qi']})")
            process_zone(level, zone)
            place_schematics(level, zone)
        level.save()
    finally:
        level.close()

    print("后处理完成")

if __name__ == "__main__":
    main()
```

### Schematic 管理

```
schematics/
├── ancient_ruins_01.schem    # 破碎洞府
├── ancient_ruins_02.schem    # 残垣断壁
├── spirit_spring.schem       # 灵泉（水 + 荧石 + 标记方块）
├── void_rift.schem           # 虚空裂缝（屏障 + 空气柱）
├── dead_tree_large.schem     # 大型枯树
└── trap_formation.schem      # 天道陷阱阵法
```

来源：
- 手工在 MC 里搭建 → Litematica 导出 `.litematic` → 转 `.schem`
- 或 AI 生成 JSON 蓝图 → 脚本转 schematic

---

## 四、完整 Pipeline

```bash
#!/usr/bin/env bash
# scripts/worldgen-full.sh — 完整世界生成流水线
set -euo pipefail

echo "===== Phase A: Datapack 预生成 ====="
bash scripts/worldgen.sh

echo "===== Phase B: Amulet 后处理 ====="
python3 scripts/worldgen_postprocess.py

echo "===== 复制到 Valence 工作目录 ====="
cp -r generated-world/region server/world/region

echo "===== Done ====="
echo "启动 Valence: cd server && cargo run"
```

### AI 天道介入点

| 阶段 | 介入方式 | 示例 |
|------|---------|------|
| **开服前 · 蓝图** | Agent 生成/修改 datapack JSON | 调整 biome 分布权重 |
| **开服前 · 加工** | Agent 修改 zones.json 参数 | 设定各 zone 初始灵气 |
| **开服前 · 结构** | Agent 选择/生成 schematic 放置方案 | 决定灵眼/废墟位置 |
| **运行时 · 退化** | Valence ECS 系统替换方块 | 灵气耗尽 → 草地变沙地 |
| **运行时 · 事件** | AgentCommand 触发地形变化 | 天劫劈出焦土坑 |
| **大版本 · 重生成** | 重跑 pipeline + 新参数 | 赛季重置/地图扩展 |

---

## 五、实施计划

### Step 1 — 先跑通 Phase A [当前]

- [x] 搭 datapack 骨架（pack.mcmeta + 1 个自定义 biome）
- [x] 本地 Fabric 1.20.1 服务端 + Chunky 验证生成效果
- [x] 迭代 biome 参数直到视觉风格满意
- [x] 测试 Valence `AnvilLevel` 加载生成的存档

### Step 2 — 补全 Datapack

- [x] 完成 6 个 biome 定义
- [x] 配置 multi_noise biome 分布
- [ ] 添加自定义 feature（枯树、残灰斑块等）
- [x] 编写预生成脚本 `scripts/worldgen.sh`

### Step 3 — Phase B 后处理

- [x] 搭建 Python 环境（venv + mcworldlib，放弃 amulet-core 因 rocksdb 依赖问题）
- [x] 实现 packed block state 编解码器（兼容 1.18+ 格式）
- [x] 实现地表检测 + 分层装饰散布（`scripts/postprocess.py`）
- [ ] 实现 zone → 方块退化替换逻辑（需 zones.json）
- [ ] Schematic 加载与放置
- [x] 完整 pipeline 脚本

### Step 4 — Valence 集成

- [x] `server/src/world/terrain.rs` 接入 AnvilLevel
- [ ] 运行时方块替换系统（灵气退化）
- [ ] AgentCommand 扩展地形控制指令

---

## 六、依赖清单

| 工具 | 版本 | 用途 |
|------|------|------|
| Fabric Server | 1.20.1, Loader 0.16.10 | 预生成宿主 |
| Chunky | Fabric 版 latest | chunk 预生成 |
| Python | 3.10+ | 后处理脚本 |
| mcworldlib | 2023.7.13 (PyPI) | 读写 Anvil region 文件 |
| nbtlib | 2.0.4 (PyPI) | NBT 数据结构（mcworldlib 依赖） |
| Litematica | 1.20.1 | Schematic 制作（可选） |
| valence_anvil | 随 Valence pinned rev | Rust 端加载 |
