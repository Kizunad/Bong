# plan-armor-model-render-v1 — 护甲上身专属模型渲染落地

> 主题：把 `OBJ_RENDER_READY=false` 的护甲占位链路变成真实穿甲外观。现状三件事实：① `plan-armor-visual-v1`（finished）用 vanilla 皮甲染色兜底交付了 6 材质凡物甲，遗留段明写"真实 BlockBench 模型留给后续 plan"；② `plan-depth-loop-v1` P1 建好了 client 三件接线（`ArmorFeatureRenderer` + `ArmorModelRegistry` + `MixinPlayerEntityArmor`），但 `render()` 在 `OBJ_RENDER_READY=false` 处 early-return（`client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java:73`），实际绘制从未实装；③ 注册表指向的 8 个 OBJ 资产（铁/骨 × 头胸腿靴）**全是占位 box**（盔/胸甲为单 box 8 顶点，腿/靴为左右双 box 16 顶点；`assets/bong/models/armor/bone_helmet/bone_helmet.obj` 文件头自注 "plan-depth-loop-v1 P4 placeholder"）。玩家穿铁甲/骨甲至今只能看到染色皮甲，两套材质在身上无法区分轮廓。
>
> 与 `plans-skeleton/plan-module-wiring-gaps-v2.md` 的关系：本 plan 是其 **T13「client/armor › ArmorFeatureRenderer」切片的可实施承接**——v2 是 report-only 决策菜单，其 §决策指引明确"推进某主题 = 开独立 plan 实施"，故不并入 v2 而单独立骨架。同时收口模型链路审计 B 组（护甲隐形）遗留。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 渲染路线收口 + ModelPart 烘焙底盘（headless 可测） | ✅ 2026-07-19 |
| P1 | 铁甲 4 件真模型资产（3 轮 PROMISE） | ✅ 2026-07-19 |
| P2 | 骨甲 4 件真模型资产（3 轮 PROMISE） | ✅ 2026-07-19 |
| P3 | 翻开渲染开关 + mixin 联动 + F5 真机 pivot/scale 校准 | ✅ 2026-07-19 |
| P4 | 饱和测试 + 视觉差异化回归 + 资源包 manifest 同步 | ✅ 2026-07-19 |

## 接入面 Checklist

- **进料**：`InventoryStateStore.snapshot().equipped()`（装备槽状态，已接）；`ArmorModelRegistry`（template_id → 模型规格，已接）；`plan-armor-visual-v1` 的 24 件凡物甲物品与穿戴同步链路（已 finished）。
- **出料**：玩家 TPV/F5 可见的专属护甲模型渲染；`MixinPlayerEntityArmor` 对已注册甲抑制皮甲染色兜底（未注册材质继续走染色，兜底路径保留不删）。
- **共享类型 / event**：零新增 server event、零 schema 改动——装备状态同步链路已存在，本 plan 纯 client 渲染层。复用 `WornPackModel` 的 Bedrock geo → vanilla `ModelPart` 烘焙思路与 `WornPackFeatureRenderer` 的骨骼局部系挂载模式（`getContextModel().body.rotate(matrices)`）；**坐标转换基准按槽独立标定**——`vanilla_y = 24 - bedrock_y`（`BEDROCK_BODY_TOP=24`）是 body 骨骼专用，仅 CHEST 可沿用，HEAD/LEGS/FEET 各按其骨骼局部系原点另定基准（见 P0）。**运行时唯一事实来源 = 版本控制的 cube 表**（见 §0 单一真相源），bbmodel/geo 仅为离线作者资产，不进运行时。
- **跨仓库契约**：无（server/agent 不动）。
- **worldview 锚点**：`worldview.md §四`（近身肉搏——护甲是物理防护，材质外观差异是战斗信息）、`§十`（资源匮乏——铁甲/骨甲的粗粝质感对应末法基调，命名用 残/碎/锈/粗 系意象）。
- **qi_physics 锚点**：无真元流动，纯视觉。

## §0 设计轴心（渲染路线收口，pre-P0 决议）

**推荐路线 = WornPack 先例的「geo → vanilla ModelPart 烘焙」，弃 OBJ 资产**：

- `WornPackModel`（`plan-tarkov-backpack-v1` P4 spike 结论）已实证：player `FeatureRenderer` 里驱动 GeckoLib `GeoModel` 结构性不可行（`GeoArmorRenderer` 依赖真实 vanilla equipped ItemStack、`PlayerEntityRenderer` 非 `GeoRenderer`、`GeckoLibCache` headless 为空）；纯 vanilla `ModelPart` 路线无 GL 依赖、headless 可单测、已在破草包上身渲染跑通。
- SML（SpecialModelLoader）走 `parent=sml:builtin/obj` **拦截 item model 管线**，不覆盖 entity FeatureRenderer 上身渲染——`ArmorFeatureRenderer.java:85` 的 TODO 注释所设想的 "SML baked model lookup" 路线实际不通，这是本 plan 必须重锚的原因。
- 既有 OBJ 资产全是占位 box，无保留价值；按「不写兼容层」惯例直接改新形状，OBJ 文件与 `modelPath` 字段删除，不做 OBJ/geo 双形态兼容。
- **单一真相源收口（运行时不携带任何模型来源标识）**：沿 `WornPackModel` 已验证的惯例——bbmodel/geo.json 是**离线作者资产**（`scripts/models/gen_*.py` 生成、`render_bbmodel.py` 预览、真机校准后回改），运行时唯一事实来源是**手工转写并受版本控制的 `ArmorPartModel` cube 表**，转写数值由 pin 测试锁死（`WornPackModel` 类注释「改 geo 时同步本表 + `WornPackModelTest` pin」即此防漂移契约）。`ArmorModelSpec` **不新增 geo 来源字段**、删除 `modelPath`，收敛为 `templateId → (slot, modelKey, texturePath)`；`modelKey → cube 表` 是 `ArmorPartModel` 内的静态映射。完整调用链：`InventoryStateStore.equipped()` → `ArmorModelRegistry.get(template_id)` → `ArmorModelSpec{slot, modelKey, texturePath}` → `ArmorPartModel.buildModelPart(modelKey)`（懒加载缓存）→ `ArmorFeatureRenderer` 按 slot 进对应骨骼局部系渲染——每一段都有 pin/单测锁（P0 测试清单）。
- 备选路线（构建期 bbmodel→cube 表代码生成器 + CI 漂移校验）在资产件数上到十位数时再评估；当前 2 套 × 4 件体量下，手工转写 + pin 的 WornPack 惯例成本最低且已验证。自写 OBJ mesh 解析渲染路线否决——凡物甲的粗粝箱体感恰好符合末法基调，cube 表足够。

## P0 — 渲染路线收口 + ModelPart 烘焙底盘 ✅ 2026-07-19

- `ArmorPartModel`（新，参 `WornPackModel` 结构）：`modelKey → cube 表` 静态映射 + **按槽独立的坐标转换基准**（CHEST 沿用 `BEDROCK_BODY_TOP=24`；HEAD/LEGS/FEET 各定基准常数——WornPack 的 `24 - y` 公式是 body 骨骼专用，四槽共用必错位）+ `buildModelPart(modelKey)` 烘焙器，纯数据构造 headless 可测；cube 表即运行时唯一事实来源（§0）。
- 四槽挂载骨骼映射：HEAD → `head.rotate(matrices)`（随头部转动）、CHEST → `body`、LEGS → `leftLeg`/`rightLeg` 双件、FEET → 腿骨末端偏移——腿/靴需要左右对称双 part（`WornPackFeatureRenderer` 只有 body 单挂载先例，这是本 plan 的新增量）。
- `ArmorModelRegistry` 重锚：`ArmorModelSpec` 收敛为 `templateId → (slot, modelKey, texturePath)`（删 `modelPath`，不新增任何模型来源字段）；删占位 OBJ 文件。
- `ArmorFeatureRenderer.render()` 实装真实绘制循环（`RenderLayer.getEntityCutoutNoCull` + 骨骼局部系），本阶段先用占位 cube 表跑通管线，正式开关仍为 false；另加 **dev-only 强制开关**（系统属性 `bong.armor_model_render`）绕过正式开关，供 P1/P2 资产落地时真机增量核验，不必等到 P3 才第一次端到端跑通。
- 测试（`ArmorPartModelTest` / `ArmorFeatureRendererTest`）：每槽一条坐标转换基准 pin、cube 表数值 pin（每 modelKey 一条，防手工转写漂移）、`modelKey` 全覆盖断言（Registry 中每个 spec 的 modelKey 都能在 `ArmorPartModel` 命中，杜绝注册表→模型断链）、四槽骨骼映射、`collectRenderable` 式纯逻辑过滤谓词（护甲槽 × 注册表命中 × 破损排除——判据 `InventoryItem.durability()==0`；server 侧 `ArmorDurabilityZero` 规则已拒装 0 耐久甲，本分支覆盖「穿戴中破碎」路径）。

## P1 — 铁甲 4 件真模型资产 ✅ 2026-07-19

- 产出：`scripts/models/gen_iron_armor.py` 生成 bbmodel（参 `gen_*_coffin.py` 先例）→ `scripts/models/render_bbmodel.py` 三视图预览 → cube 表转写进 `ArmorPartModel` + 贴图（粗铁质感，深灰 #555555 基调对齐 armor-visual-v1 色表）。
- **视觉资产纪律**：分部件做（`part_helmet()` / `part_chestplate()` / `part_leggings()` / `part_boots()` 逐件单独预览再拼）；3 轮打磨，commit 标 `(round N/3)`，终轮 commit 带 `<PROMISE>` 担保块。
- 造型基调：末法凡物甲——铆接粗铁板、边缘锈蚀缺口，禁精致仙风（worldview 命名禁词同样约束造型语言）。

## P2 — 骨甲 4 件真模型资产 ✅ 2026-07-19

- 同 P1 流程：`gen_bone_armor.py` → 预览 → cube 表 + 贴图（灰白 #D0C8B8 基调，兽骨拼扎质感、绳结绑缚细节）。
- 铁/骨两套轮廓必须远距可分（骨甲带肋条状凸起、铁甲平板铆钉），验收标准前置到本阶段自评。

## P3 — 翻开渲染开关 + 真机校准 ✅ 2026-07-19

- `OBJ_RENDER_READY` 更名 `MODEL_RENDER_READY` 并翻 `true`；`MixinPlayerEntityArmor` 联动逻辑复核（该 mixin 引用此常量做抑制判定，更名必须同步；已注册甲抑制染色兜底、未注册材质染色路径回归测试锁住）。
- 清理 SML 残留：删 `ArmorRenderBootstrap` 中 `SpecialModelLoaderEvents.LOAD_SCOPE`（`models/armor/`）注册与相关 import——OBJ 资产删除后这段是死码。
- F5 真机目测校准 pivot/scale/offset（`WornPackFeatureRenderer.OFFSET_*` 同款微调常量），WSLg `runClient` 截图记录；PlayerAnimator 姿态兼容验证（弯腰/游泳/潜行时甲随骨骼局部系摆动，torso/legs 不共祖的鞠躬补偿场景重点看胸甲-腿甲接缝）。
- 一/三人称双入口核对：FPV 手臂不渲染护甲属 vanilla 正确行为，明确记录不算缺陷。

## P4 — 饱和测试 + 差异化回归 + manifest 同步 ✅ 2026-07-19

- 全矩阵：2 材质 × 4 部位 × 穿/脱/破碎 渲染状态测试；破碎后不渲染（armor-visual-v1 的"破损不可穿"规则联动）。
- 视觉差异化回归：铁 vs 骨 vs 皮染色兜底（其余 4 材质）三类外观截图对比，远距可分辨。
- **资源包同步**：client 资产改动必须同步 `resourcepack.rs` + committed manifest 的 sha1/size，否则 Build resource pack CI 红。
- e2e + `./gradlew test build` 全绿。

## §8 开放问题（P0 决策门前需收口）

1. **其余 4 材质（铜/兽皮/灵布/残卷缠）是否本 plan 扩展**：推荐留 v2——本 plan 先用铁/骨两套打穿管线并保留染色兜底，4 材质补模型属纯资产增量。
2. **腿甲双 part 与步行动画**：`leftLeg`/`rightLeg` 独立摆动下腿甲件的挂载细节（是否拆左右两半 cube 表）需 P0 实测定案。
3. **head 槽与发型/头部装饰的 z-fighting**：头盔 cube 外扩比例（armor-visual 占位用 1.1x）需真机校准后定稿。
4. **未来灵器级护甲**：`plan-forge-v1` 的锻造甲走何种视觉升级（发光描边/附着物挂点）不在本 plan 范围，仅要求 `ArmorPartModel` 结构不堵死扩展。

## §8.1 决议

1. **其余 4 材质不扩入本 plan**：本版只注册铁甲/骨甲 8 件专属 `ModelPart`；铜甲、兽皮甲、灵布甲、残卷缠甲继续走染色皮甲兜底。边界是“只有 `ArmorModelRegistry` 命中才抑制兜底”，后续材质必须另立资产 plan，不得在本链路里暗加兼容判定。双锚点：`client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java:31-40`、`client/src/main/java/com/bong/client/mixin/MixinPlayerEntityArmor.java:47-58`；plan `P4` / `§8.1-1`。
2. **腿甲与靴子按左右骨骼拆件**：`LEGS` 固定映射 `LEFT_LEG + RIGHT_LEG`，`FEET` 固定映射 `LEFT_FOOT + RIGHT_FOOT`；渲染时每个 child 分别进入玩家左/右腿局部坐标系，因此步行、潜行时独立摆动，不存在横跨双腿的单一 cube child。边界是胸甲仍只挂 `BODY`，不用 `body` 代替腿骨骼驱动下装。双锚点：`client/src/main/java/com/bong/client/armor/ArmorPartModel.java:88-98`、`client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java:82-95,127-133`；plan `P0` / `§8.1-2`。
3. **头盔改为逐 cube 标定，不使用统一 1.1x scale**：真机定稿后，铁盔水平边界为 X `[-4.90, 4.90]` / Z `[-4.95, 5.30]`，骨盔为 X `[-4.75, 4.55]` / Z `[-4.90, 4.45]`；骨角只沿 Y 抬高到 `35.40`，不再水平放大头围。边界是后续任何改动不得超出上述已验边界而不重跑三视图 + F5 发型/z-fighting 校准。双锚点：`scripts/models/gen_iron_armor.py:24-45`、`scripts/models/gen_bone_armor.py:24-51`、`client/src/main/java/com/bong/client/armor/ArmorPartModel.java:165-181,255-278`；plan `P1` / `P2` / `P3` / `§8.1-3`。
4. **灵器级发光/附着物不做预留兼容层**：本 plan 的运行时规格刻意只保留 `templateId + slot + modelKey + texturePath`，当前统一走 `getEntityCutoutNoCull`。边界是未来发光层、挂点或材质 shader 必须在锻造/灵器 plan 中新增明确类型与差异化测试，不在本注册表塞 nullable 预留字段。双锚点：`client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java:18-25`、`client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java:75-95`；plan `§0` / `§8.1-4`。

## Finish Evidence

### 阶段验收

- **P0 ✅ 2026-07-19**：新增 `client/src/main/java/com/bong/client/armor/ArmorPartModel.java`，以版本控制 cube 表作为运行时唯一事实来源；`ArmorFeatureRenderer` 完成 HEAD/CHEST/LEGS/FEET 四槽骨骼局部系挂载，`ArmorModelRegistry.ArmorModelSpec` 收敛为 `templateId + slot + modelKey + texturePath`，8 组 OBJ/MTL/JSON 方盒占位已删除。
- **P1 ✅ 2026-07-19**：`scripts/models/gen_iron_armor.py`、`local_models/armor/iron/*.bbmodel`、三视图预览与 64×64 粗铁贴图落地；4 件铁甲完成 3 轮打磨及终轮 `<PROMISE>`。
- **P2 ✅ 2026-07-19**：`scripts/models/gen_bone_armor.py`、`local_models/armor/bone/*.bbmodel`、三视图预览与 64×64 兽骨贴图落地；4 件骨甲完成 3 轮打磨及终轮 `<PROMISE>`，肋笼/骨节轮廓与铁甲平板轮廓可远距区分。
- **P3 ✅ 2026-07-19**：`ArmorFeatureRenderer.MODEL_RENDER_READY=true`，`MixinPlayerEntityArmor` 与专属模型开关共用 `isModelRenderEnabled()`，注册甲抑制染色皮甲双层渲染；`ArmorRenderBootstrap` 的 SML OBJ scope 死接线已移除。F5 真机核验覆盖铁/骨四槽前后视、步行、潜行与穿脱，胸腿接缝及头盔 pivot 无明显断连；铜甲仍正确走染色兜底。
- **P4 ✅ 2026-07-19**：2 材质 × 4 部位 × 穿/脱/破碎矩阵、槽位/注册表/modelKey/cube digest pin 测试齐全；`ArmorModelRegistry.all()` 返回不可修改快照，运行时规格→cube 烘焙契约取代源码字符串检查；8 个 bbmodel 底面 UV 与实际 `sx/sz` 对齐并由 5 份/材质预览产出测试锁住。资源包 manifest 与 server 默认常量同步为 Ubuntu 24.04 / Info-ZIP 3.0 构建的 sha1 `261619bd64bd65cb65db043e36a77e9e995e9375`、size `72_322_787`、entity-model `file_count=290`。

### 落地清单

- 运行时：`ArmorPartModel`、`ArmorFeatureRenderer`、`ArmorModelRegistry`、`ArmorRenderBootstrap`、`MixinPlayerEntityArmor`。
- 作者资产与生成器：`scripts/models/armor_model_common.py`、`gen_iron_armor.py`、`gen_bone_armor.py`、`render_bbmodel.py`、`local_models/armor/{iron,bone}/*.bbmodel`、铁/骨各件三视图及总览图。
- 正式贴图：`client/src/main/resources/assets/bong/textures/armor/{iron,bone}_{helmet,chestplate,leggings,boots}/0.png`。
- 饱和测试：`ArmorPartModelTest`、`ArmorFeatureRendererTest`、`ArmorModelRegistryTest`、`test_gen_iron_armor.py`、`test_gen_bone_armor.py`。
- 资源包契约：`client/resourcepack/manifest.json` ↔ `server/src/network/resourcepack.rs::DEFAULT_RESOURCE_PACK_MANIFEST`。

Cube digest pin：

| modelKey | digest |
|---|---|
| `iron_helmet` | `3760e3b372a70fda` |
| `iron_chestplate` | `4393068e1f6bcc11` |
| `iron_leggings` | `4262b62a438d1088` |
| `iron_boots` | `0983cc85e5167381` |
| `bone_helmet` | `2f9d83e49d2b8dbb` |
| `bone_chestplate` | `a6d39dc53ace5bf3` |
| `bone_leggings` | `be2b47132fae568a` |
| `bone_boots` | `77b698ba4541e7fd` |

### 关键 commit

- `8e4f0ec4`（2026-07-18）：提升 skeleton 为 active plan。
- `40840a37`（2026-07-18）：实现 ModelPart 烘焙底盘、四槽挂载并移除 OBJ 占位链路。
- `d9b9aff3` / `b4af8739` / `e1584958`（2026-07-18）：粗铁甲 round 1/3 → 3/3。
- `6f5cd78f` / `7be950fa` / `06bd1a70`（2026-07-18）：兽骨甲 round 1/3 → 3/3。
- `cb62e7ad`（2026-07-18）：正式启用 ModelPart 渲染、移除 SML scope、补齐穿脱破碎矩阵。
- `2195c041`（2026-07-18）：锁定 8 套 cube 全字段 digest，并同步资源包 manifest/server 常量。
- `7a43109d`（2026-07-19）：按 Ubuntu 24.04 CI 产物修正资源包 SHA1，并补齐归档 plan 阶段状态。
- `dafc13ba`（2026-07-19）：收紧注册表不可变快照与运行时 ModelPart 烘焙契约测试。
- `7ce39fc4`（2026-07-19）：修正 bbmodel 底面 UV，补三视图实际角度日志与完整预览生成链路测试。

### 测试结果

- Client（Java 17）：`cd client && ./gradlew test build` — **4,128 tests，0 failure/error/skipped**（CodeRabbit 返工后、与最新 `origin/main` 对拍后复验）。
- Server：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 均 PASS；`cargo test` — **11,792 passed，6 ignored，0 failed**（合并 `origin/main` 后复验）。
- 资源包：`python3 -m unittest scripts/test_build_resourcepack.py` — **4 tests PASS**；Ubuntu 24.04 / Info-ZIP 3.0 同 CI 环境重建与 committed manifest/server 常量对拍 PASS，sha1 `261619bd64bd65cb65db043e36a77e9e995e9375`、size `72_322_787`；`cargo test network::resourcepack` — **16 passed，0 failed**。
- Python：`python -m unittest scripts.models.test_gen_iron_armor scripts.models.test_gen_bone_armor` — **15 tests PASS**；覆盖 down 面 UV `sx/sz`、单视图/三视图日志分支、铁/骨各 4 张三视图 + 1 张总览图的完整产出链路。
- 全链路：隔离运行时数据后执行 `bash scripts/smoke-test-e2e.sh` — **9 passed，0 failed，ALL PASS**；其中 Redis e2e **17 passed，0 failed**，100 NPC TPS gate `20.0 >= 15`，北境裂隙 dedicated preview bot PASS。
- 真机：WSLg F5 覆盖铁甲/骨甲四槽前后视、步行、潜行、穿脱；铜甲染色皮甲兜底回归 PASS，第一人称手臂不渲染护甲保持 vanilla 正确行为。

### 跨仓库核验

- **Client**：`ArmorModelRegistry.get(template_id)` → `ArmorPartModel.buildModelPart(modelKey)` → `ArmorFeatureRenderer` 四槽挂载；`MixinPlayerEntityArmor` 对同一 `isModelRenderEnabled()` 与注册表命中做染色兜底抑制。
- **Server**：`DEFAULT_RESOURCE_PACK_MANIFEST` 的 sha1/size 与 committed client manifest 完全一致，登录时 `bong:server_data` resource-pack prompt 日志命中新 hash。
- **Agent / Schema**：本 plan 按设计零新增协议 symbol、零 schema 变更；最终 smoke 中 schema check/test/generate、Tiandao check 及 **828 tests** 全绿，既有装备同步契约未漂移。

### 遗留 / 后续

- 铜甲、兽皮甲、灵布甲、残卷缠甲继续使用 `plan-armor-visual-v1` 的染色皮甲兜底；为其补专属 ModelPart 属后续纯资产增量。
- 灵器级护甲的发光描边、附着物挂点与锻造视觉升级不在本 plan 范围。
- 当前无已知阻塞缺陷。
