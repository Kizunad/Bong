# Plan: Entity Model v1（游戏实体建模补全）

> 灵龛、灵眼、裂缝传送门、炼器台、丹炉、阵法核心、灵田地块、TSY 容器——这些核心游戏实体在服务端逻辑上全部完成，但**在游戏内全部是隐形 entity 或 vanilla 方块**。本 plan 为每种实体创建 BlockBench 自定义模型 + Fabric client 注册 + BlockEntity 渲染管线。

---

## 接入面 Checklist（防孤岛）

- **进料**：`social::SpiritNiche` ✅ / `spirit_eye::SpiritEye` ✅ / `tsy::RiftPortal` ✅ / `forge::ForgeStation` ✅ / `alchemy::Furnace` ✅ / `zhenfa::FormationCore` ✅ / `lingtian::LingtianPlot` ✅ / `tsy_poi_consumer` ✅（容器 entity）
- **出料**：BlockBench 模型 → `local_models/` + `client/src/main/resources/assets/bong/geo/` / Fabric entity renderer → `client/src/main/java/com/bong/client/entity/` / 贴图 → `client/src/main/resources/assets/bong/textures/entity/`
- **共享类型/event**：不新增 event。模型通过 server 下发的 entity metadata 选择渲染
- **跨仓库契约**：server spawn entity 时附带 `EntityKind::new(N)` 自定义 ID → client Fabric 注册对应 renderer
- **worldview 锚点**：§十一 灵龛（"此地记住了你"）/ §十 灵眼（稀有灵气高浓度点）/ §五 炼器/炼丹 / §八 阵法

---

## 阶段总览

| 阶段 | 目标 | 状态 |
|------|------|------|
| P0 | 灵龛 + 灵眼 + 裂缝传送门（3 最高频遭遇实体） | ⬜ |
| P1 | 炼器台 + 丹炉 + 阵法核心（3 产出工作台） | ⬜ |
| P2 | 灵田地块 + TSY 容器（4 类）（7 种交互物） | ⬜ |

---

## P0 — 灵龛 + 灵眼 + 裂缝传送门 ⬜

### 交付物

1. **灵龛模型**（`local_models/SpiritNiche.bbmodel`）
   - 外观：石质祭坛（1×1×1.5 block），顶部凹槽放置灵石，四周刻灵纹
   - 3 状态贴图：未激活(灰石) / 已激活(灵纹发光淡青) / 被入侵(灵纹发红)
   - `SpiritNicheRenderer.java` + `SpiritNicheRenderBootstrap.java`
   - 自定义 entity ID 注册（参照 WhaleRenderer 流程）

2. **灵眼模型**（`local_models/SpiritEye.bbmodel`）
   - 外观：地面裂缝中浮现半透明光球（1×1×2 block），周围有灵气涟漪
   - 光球颜色按灵气浓度：0.5→淡绿 / 0.7→亮绿 / 1.0→金色
   - 持续发光（emissive texture layer）+ 周围 `BongSpriteParticle` lingqi_ripple 常驻粒子
   - `SpiritEyeRenderer.java`

3. **裂缝传送门模型**（`local_models/RiftPortal.bbmodel`）
   - 外观：空间裂缝（2×3 block 垂直椭圆），边缘锯齿状，内部深紫/黑色旋涡贴图
   - 3 variant：MainRift(蓝紫稳定) / DeepRift(暗红不稳定) / CollapseTear(白闪抖动)
   - 不用 vanilla portal 方块，用自定义 entity + quad billboard 渲染
   - 配合 plan-tsy-experience-v1 P0 的 portal VFX 粒子

4. **通用 entity 注册管线**
   - `BongEntityRegistry.java`：统一注册自定义 entity type（ID 分配、renderer 绑定、spawn packet 处理）
   - 参照 WhaleRenderBootstrap 已有模式，提取为可复用 helper

### 验收抓手

- 测试：`client::entity::tests::spirit_niche_renders` / `client::entity::tests::spirit_eye_renders` / `client::entity::tests::rift_portal_renders`
- 手动：走到灵龛 → 看到石质祭坛（不是空气）→ 走到灵眼 → 看到发光光球 → 走到裂缝 → 看到紫色空间裂缝

---

## P1 — 炼器台 + 丹炉 + 阵法核心 ⬜

### 交付物

1. **炼器台模型**（`ForgeStation.bbmodel`）
   - 外观：铁砧+锤座组合（1×1×1 block），侧面有火焰槽
   - 2 状态：闲置(暗灰) / 使用中(火焰槽发光橙红 + 铁锤位移动画)
   - `ForgeStationRenderer.java`

2. **丹炉模型**（`AlchemyFurnace.bbmodel`）
   - 外观：三足铜鼎（1×1×1.5 block），鼎盖微开可见蒸汽
   - 2 状态：闲置(青铜色) / 炼制中(鼎口冒彩色蒸汽粒子 + 底部火焰)
   - `AlchemyFurnaceRenderer.java`

3. **阵法核心模型**（`FormationCore.bbmodel`）
   - 外观：嵌入地面的符文石碟（1×1×0.3 block），圆形，表面刻灵纹
   - 3 状态：未激活(灰石) / 激活中(灵纹旋转发光) / 耗竭(裂纹灰化)
   - 地面嵌入渲染（y offset -0.7 block）
   - `FormationCoreRenderer.java`

4. **工作台交互视觉反馈**
   - 靠近 5 格内：工作台发出微弱灵气吸引粒子（表示可交互）
   - 交互中：对应模型动画激活（铁锤敲击/鼎蒸汽/符文旋转）

### 验收抓手

- 测试：每种工作台 `renderer_registers` 测试
- 手动：找到炼器台 → 铁砧造型 → 开始炼器 → 锤动画 + 火光 → 丹炉同理

---

## P2 — 灵田地块 + TSY 容器 ⬜

### 交付物

1. **灵田地块自定义方块**（`LingtianPlotBlock.java` / `LingtianPlotBlockEntity.java`）
   - 外观：略低于地面的方形土畦（0.9×0.9×0.1 block），表面有灵纹沟渠
   - 4 状态贴图：未开垦(干裂土) / 已开垦(湿润+灵纹) / 种植中(幼苗+灵纹) / 成熟(植物+发光灵纹)
   - 注册为 Fabric 自定义方块（server 放置 + client 渲染）

2. **TSY 容器模型**（4 类）
   - `DryCorpse.bbmodel`：干尸（蜷缩人形骨架，半埋土中）
   - `BoneSkeleton.bbmodel`：散落骨架（骨碎片堆，有微弱残留灵光）
   - `StoragePouch.bbmodel`：储物袋（布囊，半开口可见内部发光）
   - `StoneCasket.bbmodel`：石匣（方形石盒，表面刻纹，需钥匙开启时纹路暗淡）
   - 各自 Renderer + 搜刮时开盖/翻动动画

3. **容器状态视觉**
   - 未搜刮：完整外观 + 微弱灵光（`BongSpriteParticle` qi_aura × 1）
   - 搜刮中：翻动/开盖动画 + dust 粒子
   - 已搜刮：灰化 + 灵光消失 + 外观碎裂

### 验收抓手

- 测试：全实体 `renderer_registers` 测试
- E2E：灵田种植全流程 → 自定义地块 / TSY 搜刮全流程 → 4 种容器外观可区分

---

## 前置依赖

| 依赖 plan | 状态 | 用到什么 |
|-----------|------|---------|
| plan-social-v1 | ✅ finished | SpiritNiche / SpiritNicheRevealBootstrap |
| plan-spirit-eye-v1 | ✅ finished | SpiritEye entity / spirit_qi threshold |
| plan-tsy-worldgen-v1 | ✅ finished | tsy_poi_consumer / RiftPortal entity |
| plan-forge-v1 | ✅ finished | ForgeStation / 4 步状态机 |
| plan-alchemy-v1 | ✅ finished | Furnace / brew session |
| plan-zhenfa-v1 | ✅ finished | FormationCore / activation state |
| plan-lingtian-v1 | ✅ finished | LingtianPlot / plot state |
| plan-tsy-container-v1 | ✅ finished | container archetypes / search session |
| plan-player-animation-v1 | ✅ finished | GeckoLib + Fabric entity 注册参考（WhaleRenderer） |

**全部依赖已 finished，无阻塞。**
