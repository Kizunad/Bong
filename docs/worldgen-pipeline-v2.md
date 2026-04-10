# 世界生成 Pipeline V2

> 适用于 `docs/worldview.md` 所定义的固定坐标、固定区域、作者指定布局的大地图。
> 本文档用于替代 `docs/plan-worldgen.md` 中“Datapack 预生成 + 大量后处理”作为主路径的思路。

---

## 一、问题定义

末法残土不是随机世界，而是一张**有明确坐标、明确地貌身份、明确旅行距离感**的地图。

已知约束：

- 初醒原固定在 `(0, 0)`
- 青云残峰固定在 `(-3000, -2000)`
- 灵泉湿地固定在 `(-2500, 2500)`
- 血谷固定在 `(3000, -2500)`
- 幽暗地穴固定在 `(2000, 3000)`
- 北荒固定在 `(0, -7000)`
- 区域之间相隔数千格，中间应是大面积荒野/死域缓冲

这意味着本项目需要的不是“无限随机探索世界”，而是“**作者指定布局 + 程序化细化**”的地图管线。

---

## 二、为什么 V1 思路不成立

V1 的直觉方案大致是：

1. 用 Datapack + Chunky 先生成一张基础世界
2. 用 `postprocess.py` 按 zone 去雕成残峰、湿地、血谷、北荒

这条路的问题在于：

### 1. Datapack `multi_noise` 不适合固定布局

`multi_noise biome source` 擅长：

- 无限随机世界
- 噪声驱动的 biome 分布
- 自然式过渡

但它不擅长：

- “某个 biome/区域必须出现在固定坐标”
- “血谷必须在 `(3000, -2500)`”
- “北荒必须在 `(0, -7000)`”

结论：Datapack 不能作为固定布局地图的主控层。

### 2. `postprocess.py` 不适合承担整片主地貌生成

`postprocess.py` 更适合做：

- 小范围 carve
- 洞口、塌陷口
- 局部裂缝
- 遗迹/结构放置
- 地表装饰和材质细修

但不适合做：

- 整片山系抬升
- 整片湿地盆地下压
- 整片高原荒化
- 整片主裂谷切割

如果让它承担整片大地貌，等于用 block 级脚本去硬抠整张地图，职责失衡，性能也会快速恶化。

结论：`postprocess.py` 应保留为局部增强层，而不是大地貌生成器。

---

## 三、V2 总体方案

V2 采用以下主链路：

```text
blueprint
  -> terrain profile generators
  -> zone terrain fields
  -> wilderness base field
  -> stitching / blending
  -> bake world
  -> local postprocess
```

职责划分：

- `blueprint`：地图上哪里是什么
- `profile generator`：每种区域长什么样
- `terrain fields`：区域中间结果，不直接等于方块
- `stitching`：把区域与荒野接起来
- `bake`：把 field 变成 Minecraft 世界
- `postprocess`：局部雕刻、结构、装饰、细修

---

## 四、核心原则

### 1. 拼 field，不拼 chunk

不要做：

- 每个区域先生成完整 Minecraft chunk/world
- 再把 chunk 生硬拼起来

因为这样会带来：

- 水位断裂
- 山体断裂
- 洞穴断头
- 材质层错位

正确做法是先拼接中间地貌数据，再统一烘焙成世界。

### 2. 先做 zone -> wilderness，不做 zone -> zone 直接拼接

第一版不处理“残峰直接接湿地”的复杂连续过渡。

原因：

- `worldview.md` 里几个主区域本来就相距数千格
- 中间应该是大面积荒野/死域缓冲
- 这与世界观一致，也能大幅降低拼接复杂度

所以第一版边界模型是：

```text
zone core -> transition band -> wilderness
```

### 3. 大地貌优先，局部细节后置

优先保证：

- 从远处一眼看出这是哪个区域
- 高差、色块、通行感明显不同

后面再补：

- 枯木
- 骨堆
- 遗迹
- 洞口
- 局部裂缝

---

## 五、Blueprint 层

当前 authoritative 蓝图文件：

- `server/zones.worldview.example.json`

它承担两种角色：

1. Server authoritative zones 输入
2. Worldgen 布局和 profile 参数输入

当前已包含：

- `name`
- `display_name`
- `aabb`
- `center_xz`
- `size_xz`
- `spirit_qi`
- `danger_level`
- `worldgen.terrain_profile`
- `worldgen.boundary`
- `worldgen.height_model`
- `worldgen.surface_palette`
- 其他 profile 专属参数，如 `rift_axis`、`water_model`、`negative_pressure_patches`

Blueprint 是整个 worldgen 的真相源，不再依赖 `multi_noise` 来决定宏观区域分布。

---

## 六、Terrain Profile 层

当前 profile 规范文件：

- `docs/worldgen-terrain-profiles.md`
- `worldgen/terrain-profiles.example.json`

第一版 profile：

- `spawn_plain`
- `broken_peaks`
- `spring_marsh`
- `rift_valley`
- `cave_network`
- `waste_plateau`

每种 profile 至少定义：

- 高度模型
- 表层材质倾向
- 水体倾向
- 通行风格
- 边界模式

如果两个相邻区域在“高度 / 水体 / 材质 / 通行”上差异不够大，就视为 profile 设计失败。

---

## 七、Terrain Fields 设计

Terrain field 是“还没变成 Minecraft 方块”的中间层数据。

推荐最小字段集合：

- `height`
- `surface_id`
- `subsurface_id`
- `water_level`
- `biome_id`
- `feature_mask`
- `boundary_weight`

建议补充字段：

- `cave_mask`
- `ceiling_height`
- `neg_pressure`
- `roughness`

### 字段含义

`height`
: 地表目标高度

`surface_id`
: 表层材质索引

`subsurface_id`
: 地下 3~5 格材质索引

`water_level`
: 水面高度，或无水标记

`biome_id`
: 最终写入世界的 biome 索引，用于天空/草色/雾色/环境声

`feature_mask`
: 用于后续触发局部 carve、泉眼、遗迹、骨堆、洞口等

`boundary_weight`
: 当前点受 zone 影响的强度，范围 `[0, 1]`

`cave_mask`
: 当前点下方是否属于地下网络宏观空洞区

`ceiling_height`
: 洞穴顶部高度，用于 `cave_network`

`neg_pressure`
: 负灵压强度，用于 `waste_plateau`

### 存储约束

注意：全图约 `20000 x 20000`，不能天真地使用全尺寸 dense `float32` 多图层。

建议：

- 使用 tile 存储：如 `512 x 512` 或 `1024 x 1024`
- `height` 优先 `int16` / `float16`
- `surface_id` / `biome_id` 用 `uint8`
- mask 尽量用 `uint8` 或 `float16`
- 可选 `numpy.memmap` / `zarr` / 分 tile `.npy`

---

## 八、Wilderness 基底

第一版不是让所有 zone 直接彼此拼接，而是每个 zone 都长在一张“荒野基底”上。

默认荒野建议：

- 高度：`Y 68 ~ 74`
- 地表：`stone`, `gravel`, `coarse_dirt`
- 水体：几乎无
- 植被：极少
- 视觉：贫瘠、空旷、赶路感强

作用：

- 作为 zone 外的默认世界
- 作为所有边界混合的兜底层
- 减少 zone ↔ zone 直接拼接复杂度

---

## 九、Stitching / Blending

推荐：distance field / signed distance field（SDF）驱动的边界混合。

每个 zone 根据边界模式得到一个权重 `w in [0, 1]`：

- `soft`：缓慢衰减
- `hard`：短距离快速衰减
- `semi_hard`：中等衰减

示意公式：

```text
final_height = lerp(wilderness_height, zone_height, w)
final_surface = blend(wilderness_surface, zone_surface, w)
final_water   = blend(wilderness_water, zone_water, w)
```

第一版的混合重点：

- 高度场混合
- 表层材质混合
- 水体存在与否混合

不要求第一版就做：

- 复杂 biome 逻辑混合
- 跨 zone 洞穴连续性
- 所有 feature mask 的高阶混合

---

## 十、Bake 层

Bake 的作用是把 stitched fields 转成 Minecraft 世界。

### 近期推荐：双后端策略

#### 后端 A：WorldPainter 烘焙（近期）

优点：

- 快速可视化验证
- 非常适合检验大地貌轮廓
- 适合第一阶段迭代

缺点：

- 自动化能力一般
- 难做纯脚本 CI

适用阶段：

- 规则还在频繁调整时

#### 后端 B：Python 直写 Anvil（中期）

优点：

- 全脚本化
- 可版本控制
- 可直接接入 pipeline

缺点：

- 需要自己维护更多 bake 逻辑

适用阶段：

- 大地貌规则趋于稳定后

### 不推荐作为当前主线

- 继续依赖 Datapack `multi_noise` 做宏观区域分布
- 让 Valence 运行时直接承担 terrain generation 主职责

---

## 十一、Postprocess 层职责

`postprocess.py` 保留，但职责严格收缩为局部增强。

### 应该做的

- 次级裂缝 / 次级峡谷 carve
- 天坑、洞口、塌陷口
- 遗迹、骨堆、断碑、枯木
- 地表散布和环境装饰
- 局部材质细修
- landmark 放置

### 不应该做的

- 整片青云残峰的山系塑形
- 整片灵泉湿地的盆地和水网
- 血谷主裂谷本体
- 北荒整体高原与荒化基盘
- 幽暗地穴地下网络主体

### 判断准则

如果一个改动会改变 100 格外看见的地平线轮廓，它就不是 postprocess。

---

## 十二、推荐目录结构

```text
worldgen/
  scripts/
    terrain_gen/
      __main__.py
      blueprint.py
      fields.py
      stitcher.py
      exporters.py
      bakers/
        worldpainter.py
        anvil.py
      profiles/
        wilderness.py
        spawn_plain.py
        broken_peaks.py
        spring_marsh.py
        rift_valley.py
        cave_network.py
        waste_plateau.py
    postprocess.py
```

说明：

- `terrain_gen` 负责大地貌
- `postprocess.py` 负责局部增强
- Datapack 退居为 biome visual/effects 支持层，而不是主地形生成层

---

## 十三、实施顺序

### Phase 1 — 建 field 生成层

先实现：

- `wilderness`
- `rift_valley`
- `spring_marsh`
- `broken_peaks`

输出：

- 高度图
- 材质图
- feature mask 预览图

目标：先验证“远景轮廓是不是对”。

### Phase 2 — Stitching

实现：

- zone -> wilderness 的边界混合
- `soft / hard / semi_hard` 三种模式

目标：确认边界过渡策略可用。

### Phase 3 — WorldPainter 烘焙

把 field 导成可导入的图层：

- heightmap
- material map
- water map

目标：快速看大地貌成图效果。

### Phase 4 — Postprocess 局部增强

在已烘焙世界基础上补：

- 入口
- 裂缝
- 遗迹
- 骨堆
- 装饰散布

### Phase 5 — 自动化 Anvil baker

待规则稳定后，补 Python 直写 Anvil 后端，减少对 WorldPainter 的依赖。

---

## 十四、与现有文档关系

- `docs/worldview.md`
  - 提供世界地理和体验目标
- `docs/worldgen-terrain-profiles.md`
  - 提供各 profile 的行为规则
- `server/zones.worldview.example.json`
  - 提供 authoritative 蓝图输入
- `docs/plan-worldgen.md`
  - 视为 V1 方案文档，保留参考，但不再作为主路径

---

## 十五、最终结论

对于末法残土这种“固定坐标 + 固定区域 + 作者指定布局”的世界：

- 不应依赖 Datapack `multi_noise` 作为宏观布局主控
- 不应让 `postprocess.py` 承担整片主地貌生成
- 应采用：

```text
blueprint -> terrain fields -> wilderness-aware stitching -> bake -> local postprocess
```

这是当前阶段最合理、最稳、也最可迭代的实现路径。
