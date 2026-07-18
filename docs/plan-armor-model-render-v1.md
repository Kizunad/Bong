# plan-armor-model-render-v1 — 护甲上身专属模型渲染落地

> 主题：把 `OBJ_RENDER_READY=false` 的护甲占位链路变成真实穿甲外观。现状三件事实：① `plan-armor-visual-v1`（finished）用 vanilla 皮甲染色兜底交付了 6 材质凡物甲，遗留段明写"真实 BlockBench 模型留给后续 plan"；② `plan-depth-loop-v1` P1 建好了 client 三件接线（`ArmorFeatureRenderer` + `ArmorModelRegistry` + `MixinPlayerEntityArmor`），但 `render()` 在 `OBJ_RENDER_READY=false` 处 early-return（`client/src/main/java/com/bong/client/armor/ArmorFeatureRenderer.java:73`），实际绘制从未实装；③ 注册表指向的 8 个 OBJ 资产（铁/骨 × 头胸腿靴）**全是占位 box**（盔/胸甲为单 box 8 顶点，腿/靴为左右双 box 16 顶点；`assets/bong/models/armor/bone_helmet/bone_helmet.obj` 文件头自注 "plan-depth-loop-v1 P4 placeholder"）。玩家穿铁甲/骨甲至今只能看到染色皮甲，两套材质在身上无法区分轮廓。
>
> 与 `plans-skeleton/plan-module-wiring-gaps-v2.md` 的关系：本 plan 是其 **T13「client/armor › ArmorFeatureRenderer」切片的可实施承接**——v2 是 report-only 决策菜单，其 §决策指引明确"推进某主题 = 开独立 plan 实施"，故不并入 v2 而单独立骨架。同时收口模型链路审计 B 组（护甲隐形）遗留。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 渲染路线收口 + ModelPart 烘焙底盘（headless 可测） | ⬜ |
| P1 | 铁甲 4 件真模型资产（3 轮 PROMISE） | ⬜ |
| P2 | 骨甲 4 件真模型资产（3 轮 PROMISE） | ⬜ |
| P3 | 翻开渲染开关 + mixin 联动 + F5 真机 pivot/scale 校准 | ⬜ |
| P4 | 饱和测试 + 视觉差异化回归 + 资源包 manifest 同步 | ⬜ |

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

## P0 — 渲染路线收口 + ModelPart 烘焙底盘 ⬜

- `ArmorPartModel`（新，参 `WornPackModel` 结构）：`modelKey → cube 表` 静态映射 + **按槽独立的坐标转换基准**（CHEST 沿用 `BEDROCK_BODY_TOP=24`；HEAD/LEGS/FEET 各定基准常数——WornPack 的 `24 - y` 公式是 body 骨骼专用，四槽共用必错位）+ `buildModelPart(modelKey)` 烘焙器，纯数据构造 headless 可测；cube 表即运行时唯一事实来源（§0）。
- 四槽挂载骨骼映射：HEAD → `head.rotate(matrices)`（随头部转动）、CHEST → `body`、LEGS → `leftLeg`/`rightLeg` 双件、FEET → 腿骨末端偏移——腿/靴需要左右对称双 part（`WornPackFeatureRenderer` 只有 body 单挂载先例，这是本 plan 的新增量）。
- `ArmorModelRegistry` 重锚：`ArmorModelSpec` 收敛为 `templateId → (slot, modelKey, texturePath)`（删 `modelPath`，不新增任何模型来源字段）；删占位 OBJ 文件。
- `ArmorFeatureRenderer.render()` 实装真实绘制循环（`RenderLayer.getEntityCutoutNoCull` + 骨骼局部系），本阶段先用占位 cube 表跑通管线，正式开关仍为 false；另加 **dev-only 强制开关**（系统属性 `bong.armor_model_render`）绕过正式开关，供 P1/P2 资产落地时真机增量核验，不必等到 P3 才第一次端到端跑通。
- 测试（`ArmorPartModelTest` / `ArmorFeatureRendererTest`）：每槽一条坐标转换基准 pin、cube 表数值 pin（每 modelKey 一条，防手工转写漂移）、`modelKey` 全覆盖断言（Registry 中每个 spec 的 modelKey 都能在 `ArmorPartModel` 命中，杜绝注册表→模型断链）、四槽骨骼映射、`collectRenderable` 式纯逻辑过滤谓词（护甲槽 × 注册表命中 × 破损排除——判据 `InventoryItem.durability()==0`；server 侧 `ArmorDurabilityZero` 规则已拒装 0 耐久甲，本分支覆盖「穿戴中破碎」路径）。

## P1 — 铁甲 4 件真模型资产 ⬜

- 产出：`scripts/models/gen_iron_armor.py` 生成 bbmodel（参 `gen_*_coffin.py` 先例）→ `scripts/models/render_bbmodel.py` 三视图预览 → cube 表转写进 `ArmorPartModel` + 贴图（粗铁质感，深灰 #555555 基调对齐 armor-visual-v1 色表）。
- **视觉资产纪律**：分部件做（`part_helmet()` / `part_chestplate()` / `part_leggings()` / `part_boots()` 逐件单独预览再拼）；3 轮打磨，commit 标 `(round N/3)`，终轮 commit 带 `<PROMISE>` 担保块。
- 造型基调：末法凡物甲——铆接粗铁板、边缘锈蚀缺口，禁精致仙风（worldview 命名禁词同样约束造型语言）。

## P2 — 骨甲 4 件真模型资产 ⬜

- 同 P1 流程：`gen_bone_armor.py` → 预览 → cube 表 + 贴图（灰白 #D0C8B8 基调，兽骨拼扎质感、绳结绑缚细节）。
- 铁/骨两套轮廓必须远距可分（骨甲带肋条状凸起、铁甲平板铆钉），验收标准前置到本阶段自评。

## P3 — 翻开渲染开关 + 真机校准 ⬜

- `OBJ_RENDER_READY` 更名 `MODEL_RENDER_READY` 并翻 `true`；`MixinPlayerEntityArmor` 联动逻辑复核（该 mixin 引用此常量做抑制判定，更名必须同步；已注册甲抑制染色兜底、未注册材质染色路径回归测试锁住）。
- 清理 SML 残留：删 `ArmorRenderBootstrap` 中 `SpecialModelLoaderEvents.LOAD_SCOPE`（`models/armor/`）注册与相关 import——OBJ 资产删除后这段是死码。
- F5 真机目测校准 pivot/scale/offset（`WornPackFeatureRenderer.OFFSET_*` 同款微调常量），WSLg `runClient` 截图记录；PlayerAnimator 姿态兼容验证（弯腰/游泳/潜行时甲随骨骼局部系摆动，torso/legs 不共祖的鞠躬补偿场景重点看胸甲-腿甲接缝）。
- 一/三人称双入口核对：FPV 手臂不渲染护甲属 vanilla 正确行为，明确记录不算缺陷。

## P4 — 饱和测试 + 差异化回归 + manifest 同步 ⬜

- 全矩阵：2 材质 × 4 部位 × 穿/脱/破碎 渲染状态测试；破碎后不渲染（armor-visual-v1 的"破损不可穿"规则联动）。
- 视觉差异化回归：铁 vs 骨 vs 皮染色兜底（其余 4 材质）三类外观截图对比，远距可分辨。
- **资源包同步**：client 资产改动必须同步 `resourcepack.rs` + committed manifest 的 sha1/size，否则 Build resource pack CI 红。
- e2e + `./gradlew test build` 全绿。

## §8 开放问题（P0 决策门前需收口）

1. **其余 4 材质（铜/兽皮/灵布/残卷缠）是否本 plan 扩展**：推荐留 v2——本 plan 先用铁/骨两套打穿管线并保留染色兜底，4 材质补模型属纯资产增量。
2. **腿甲双 part 与步行动画**：`leftLeg`/`rightLeg` 独立摆动下腿甲件的挂载细节（是否拆左右两半 cube 表）需 P0 实测定案。
3. **head 槽与发型/头部装饰的 z-fighting**：头盔 cube 外扩比例（armor-visual 占位用 1.1x）需真机校准后定稿。
4. **未来灵器级护甲**：`plan-forge-v1` 的锻造甲走何种视觉升级（发光描边/附着物挂点）不在本 plan 范围，仅要求 `ArmorPartModel` 结构不堵死扩展。
