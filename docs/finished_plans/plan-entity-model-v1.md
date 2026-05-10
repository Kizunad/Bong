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
| P0 | 灵龛 + 灵眼 + 裂缝传送门（3 最高频遭遇实体） | ✅ 2026-05-10 |
| P1 | 炼器台 + 丹炉 + 阵法核心（3 产出工作台） | ✅ 2026-05-10 |
| P2 | 灵田地块 + TSY 容器（4 类）（7 种交互物） | ✅ 2026-05-10 |

---

## P0 — 灵龛 + 灵眼 + 裂缝传送门 ✅ 2026-05-10

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

## P1 — 炼器台 + 丹炉 + 阵法核心 ✅ 2026-05-10

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

## P2 — 灵田地块 + TSY 容器 ✅ 2026-05-10

### 交付物

1. **灵田地块自定义方块**（`LingtianPlotBlock.java` / `LingtianPlotBlockEntity.java`）
   - 外观：略低于地面的方形土畦（0.9×0.9×0.1 block），表面有灵纹沟渠
   - 4 状态贴图：未开垦(干裂土) / 已开垦(湿润+灵纹) / 种植中(幼苗+灵纹) / 成熟(植物+发光灵纹)
   - 注册为 Fabric 自定义方块渲染面；服务端通过 `EntityKind::new(140)` 视觉 marker 同步灵田状态，避免 Valence 端放置未注册 Fabric block 造成协议漂移。

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

## Finish Evidence

### 落地清单

- P0 灵龛 / 灵眼 / 裂缝传送门：
  - `server/src/world/entity_model.rs`：下发 `EntityKind::new(134..136)` 视觉 marker，并用 DataTracker `VisualState` metadata 驱动状态贴图。
  - `client/src/main/java/com/bong/client/entity/BongEntityRegistry.java`
  - `client/src/main/java/com/bong/client/entity/BongEntityRenderBootstrap.java`
  - `client/src/main/java/com/bong/client/entity/SpiritNicheRenderer.java`
  - `client/src/main/java/com/bong/client/entity/SpiritEyeRenderer.java`
  - `client/src/main/java/com/bong/client/entity/RiftPortalRenderer.java`
  - `local_models/SpiritNiche.bbmodel`
  - `local_models/SpiritEye.bbmodel`
  - `local_models/RiftPortal.bbmodel`
  - `client/src/main/resources/assets/bong/geo/{spirit_niche,spirit_eye,rift_portal}.geo.json`
  - `client/src/main/resources/assets/bong/animations/{spirit_niche,spirit_eye,rift_portal}.animation.json`
  - `client/src/main/resources/assets/bong/textures/entity/{spirit_niche_*,spirit_eye_*,rift_portal_*}.png`
- P1 炼器台 / 丹炉 / 阵法核心：
  - `server/src/world/entity_model.rs`：下发 `EntityKind::new(137..139)`，按工作台 session / furnace busy / zhenfa anchor 状态同步视觉状态。
  - `client/src/main/java/com/bong/client/entity/ForgeStationRenderer.java`
  - `client/src/main/java/com/bong/client/entity/AlchemyFurnaceRenderer.java`
  - `client/src/main/java/com/bong/client/entity/FormationCoreRenderer.java`
  - `local_models/{ForgeStation,AlchemyFurnace,FormationCore}.bbmodel`
  - `client/src/main/resources/assets/bong/geo/{forge_station,alchemy_furnace,formation_core}.geo.json`
  - `client/src/main/resources/assets/bong/animations/{forge_station,alchemy_furnace,formation_core}.animation.json`
  - `client/src/main/resources/assets/bong/textures/entity/{forge_station_*,alchemy_furnace_*,formation_core_*}.png`
- P2 灵田地块 / TSY 容器：
  - `server/src/world/entity_model.rs`：下发 `EntityKind::new(140..144)`，按灵田 crop 成熟度与 TSY 容器 kind / searched / depleted 状态同步视觉状态。
  - `client/src/main/java/com/bong/client/entity/LingtianPlotRenderer.java`
  - `client/src/main/java/com/bong/client/entity/LingtianPlotBlock.java`
  - `client/src/main/java/com/bong/client/entity/LingtianPlotBlockEntity.java`
  - `client/src/main/java/com/bong/client/entity/DryCorpseRenderer.java`
  - `client/src/main/java/com/bong/client/entity/BoneSkeletonRenderer.java`
  - `client/src/main/java/com/bong/client/entity/StoragePouchRenderer.java`
  - `client/src/main/java/com/bong/client/entity/StoneCasketRenderer.java`
  - `local_models/{LingtianPlot,DryCorpse,BoneSkeleton,StoragePouch,StoneCasket}.bbmodel`
  - `client/src/main/resources/assets/bong/geo/{lingtian_plot,dry_corpse,bone_skeleton,storage_pouch,stone_casket}.geo.json`
  - `client/src/main/resources/assets/bong/animations/{lingtian_plot,dry_corpse,bone_skeleton,storage_pouch,stone_casket}.animation.json`
  - `client/src/main/resources/assets/bong/textures/entity/{lingtian_plot_*,dry_corpse_*,bone_skeleton_*,storage_pouch_*,stone_casket_*}.png`
- 通用注册与契约：
  - `server/src/world/mod.rs`：注册 `world::entity_model`，在现有 gameplay component 之外提供只负责视觉的同步层。
  - `client/src/main/java/com/bong/client/entity/BongEntityModelKind.java`：统一维护 11 个实体的 `bong:*` id、raw id、尺寸、状态贴图、geo/animation 资源路径。
  - `client/src/main/java/com/bong/client/BongClient.java`：在 `WhaleRenderBootstrap.register()` 与 `FaunaRenderBootstrap.register()` 之后注册新实体，避免破坏既有 `whale raw_id=125` / `fauna raw_id=126..133`。
  - `client/src/test/java/com/bong/client/entity/BongEntityModelRegistryTest.java`
  - `client/src/test/java/com/bong/client/entity/BongEntityModelAssetTest.java`

### 关键 commit

- `664383048`（2026-05-10）`feat(client): 接入游戏实体模型渲染注册`
- `1fe84a949`（2026-05-10）`feat(client): 补齐游戏实体模型资产`
- `79eddeaa6`（2026-05-10）`test(client): 锁定实体模型渲染资产`
- `2638eac1f`（2026-05-10）`feat(server): 接入实体模型视觉桥接`
- `4cc9dbdee`（2026-05-10）`fix(client): 收敛实体模型注册合约`
- `9a8be311d`（2026-05-10）`fix(assets): 替换实体模型占位资源`
- `0f160ae4e`（2026-05-10）`docs(client): 说明灵田方块注册边界`
- `068d5fc47`（2026-05-10）`fix(server): 同步实体视觉 metadata 时序`

### 测试结果

- `cd client && export JAVA_HOME="${JAVA_HOME:-$HOME/.sdkman/candidates/java/17.0.18-amzn}" && export PATH="$JAVA_HOME/bin:$PATH" && ./gradlew test`
  - `BUILD SUCCESSFUL`
  - `client/build/test-results/test`: `tests=1031 failures=0 errors=0 skipped=0`
  - 新增 `BongEntityModelRegistryTest`: `tests=11 failures=0 errors=0 skipped=0`
  - 新增 `BongEntityModelAssetTest`: `tests=4 failures=0 errors=0 skipped=0`
- `cd client && export JAVA_HOME="${JAVA_HOME:-$HOME/.sdkman/candidates/java/17.0.18-amzn}" && export PATH="$JAVA_HOME/bin:$PATH" && ./gradlew test build`
  - `BUILD SUCCESSFUL`
- `cd client && export JAVA_HOME="$HOME/.sdkman/candidates/java/17.0.18-amzn" && export PATH="$JAVA_HOME/bin:$PATH" && ./gradlew test --tests com.bong.client.entity.BongEntityModelRegistryTest`
  - `BUILD SUCCESSFUL`
- `cd server && cargo fmt --check && CARGO_BUILD_JOBS=1 cargo check --bin bong-server`
  - `Finished dev profile`
- `cd server && CARGO_BUILD_JOBS=1 CARGO_PROFILE_TEST_DEBUG=0 cargo test entity_model -- --nocapture`
  - `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 3647 filtered out`

### 跨仓库核验

- server 已有对应运行时实体 / 组件锚点：
  - `server/src/social/components.rs`：`SpiritNiche`
  - `server/src/world/spirit_eye.rs`：`SpiritEye`
  - `server/src/world/rift_portal.rs`：`RiftPortal`
  - `server/src/forge/station.rs`：`WeaponForgeStation`
  - `server/src/alchemy/furnace.rs`：`AlchemyFurnace`
  - `server/src/zhenfa/mod.rs`：`ZhenfaAnchor`
  - `server/src/lingtian/plot.rs`：`LingtianPlot`
  - `server/src/world/tsy.rs` / `server/src/world/tsy_container.rs`：`LootContainer`
- client 新增 `BongEntityModelKind` 将 11 个视觉实体固定在 `raw_id=134..144`，保持在既有 `WhaleEntities.EXPECTED_RAW_ID=125` 与 `fauna raw_id=126..133` 之后。
- server 新增 `world::entity_model` 将 gameplay component 映射到 `EntityKind::new(134..144)` 视觉 marker，`VisualState` 使用 DataTracker index `8`、type `INTEGER`、VarInt 编码，与 `BongModeledEntity.VISUAL_STATE` 对齐。
- 未新增 server_data / Redis event；视觉状态走实体 metadata 的 `VisualState` tracker 映射到状态贴图。

### 遗留 / 后续

- 已通过本地 client 测试覆盖 Fabric 注册表、GeckoLib 资源路径、BlockBench 源文件和状态贴图存在性；server 定向测试覆盖 raw id、DataTracker metadata 字节和同 tick metadata flush。实际 WSLg `runClient` 视觉走查仍属于人工验收，不在本次云端自动测试内。
- rebase 到含 `plan-fauna-experience-v1` 的 `origin/main` 后，`raw_id=126..133` 让给 fauna；本 plan 视觉实体整体后移到 `134..144`。
