# plan-item-model-channel-v1 — 物品 3D 模型正式通道：template_id 直查 baked model，撤掉 vanilla 宿主

> **一句话主题**：把 Bong 物品从「借 vanilla ItemStack + 覆盖 vanilla model JSON」迁到
> `template_id → Bong 自有 baked model` 的正式查询通道，先建立能提供完整变换的通道，
> 再逐批迁移存量，最后删除所有宿主覆盖。
>
> **状态**：骨架（skeleton）。升 active 前由人工收口 P0 的加载器、渲染入口、transform
> 来源和 GUI/掉落边界；骨架阶段保留开放问题，不擅自把实现方案写成既成事实。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 直查 baked model 的可行性 spike、transform 来源和场景边界 | ⬜ |
| P1 | 建立单一 canonical registry、资源定义和 fail-fast 校验 | ⬜ |
| P2 | 迁移 39 条存量 entry，并撤掉 15 个 vanilla model 覆盖 | ⬜ |
| P3 | 迁移五套 VanillaIconMap 的 Bong 路径和 FPV/TPV 消费者 | ⬜ |
| P4 | 接入丹药 8 项、灵植 9 项，收口 3D 场景范围 | ⬜ |
| P5 | 防复发门禁、资源包同步和全场景回归 | ⬜ |

## 接入面

- **进料**：服务端现有 `ServerDataPayloadV1::WeaponEquipped` / `WeaponEquippedV1`
  的 `template_id`（`server/src/schema/combat_hud.rs:290-383`、
  `server/src/network/weapon_equipped_emit.rs:99-190`）进入客户端
  `WeaponEquippedHandler` → `WeaponEquippedStore`；手持渲染再由
  `HeldItemStackResolver` 和 FPV/TPV mixin 消费。模型 manifest 的模板集合来自
  `server/assets/items/*.toml`，几何、MTL、贴图和模型 definition 来自
  `client/src/main/resources/assets/bong/` 及 `modelScript/core/held_item_common.py::write_assets`。
  背包 GUI 的既有 2D 输入仍来自 `ItemIconRegistry`，不因 3D 通道而改成模型输入。
- **出料**：canonical registry 输出按 `template_id` 查到的 Bong `BakedModel` 和各
  `ModelTransformation` context，供 `MixinHeldItemRenderer`（FPV）与
  `MixinPlayerEntityHeldItem`（TPV）直接渲染；纳入范围的 GUI/fixed/ground 场景写入
  各自 owner。资源文件最终进入 `assets/bong/` 的资源包并由
  `server/src/network/resourcepack.rs::DEFAULT_RESOURCE_PACK_MANIFEST` 校验。本 plan
  不向 gameplay、inventory 状态、Redis 或 server event 产出新副作用。
- **共享类型 / event**：复用已有 `WeaponEquippedV1`、`template_id`、
  `WeaponEquippedStore`、`EquippedShieldStore`、`ItemIconRegistry` 以及现有 SML/模型
  加载契约；不另建 `ItemStack` host 映射、装备态 store、wire event 或 schema。新的
  model definition/registry 是 client 渲染内部类型，若实现需要新增类型，必须让 FPV、
  TPV 和资源 reload 共用同一份，而不是为测试另开可见性 seam。
- **跨仓库契约**：server 侧命中
  `network::weapon_equipped_emit::emit_weapon_equipped_payloads`、
  `ServerDataPayloadV1::WeaponEquipped` / `WeaponEquippedV1.template_id`；client 侧命中
  `ProtoServerDataBridge`、`WeaponEquippedHandler`、`WeaponEquippedStore`。agent 侧
  **不适用**：模型查询发生在 client resource/render 层，不经过 agent、Redis channel
  或新的 IPC；本 plan 不改 server↔agent↔client 的 wire/schema，P5 只用既有
  `weapon_equipped` 登录/装备场景做回归。
- **worldview 锚点**：**不适用**。这是 client 资源加载和物品模型挂载基建，不新增
  境界、经济、传承、阵法、区域或物品 gameplay 语义；丹药/灵植的世界观含义由既有
  item/botany owner 维护，P4 只接模型，不在本 plan 改 `docs/worldview.md`。
- **qi_physics 锚点**：**不适用**。模型 lookup、OBJ/MTL/贴图加载和 transform 不读写
  真元/灵气、不实现衰减/逸散/距离损耗，也不改变任何 ledger；若未来要为模型添加会
  影响真元数值的视觉/玩法效果，必须另行接入既有 `qi_physics`，不能在本 plan 自定
  物理常数。

## 0. 不可逆裁决、范围与完成定义

### 0.1 所有者裁决（本 plan 的前提，不是待投票事项）

- 新通道的 canonical 查询是 `template_id → Bong 自有 baked model`。渲染消费点直接
  根据 `template_id` 查模型，不合成 fake vanilla `ItemStack`，不把任何 vanilla item
  当宿主，也不依赖 `assets/minecraft/models/item/*.json` 覆盖 Bong 外观。
- 存量也必须迁移：当前 `BongWeaponModelRegistry` 的 39 条 entry 和已覆盖的 vanilla
  model 都是过渡债，不得以「新物品走新路、旧物品永久留宿主」作为终态。
- 每个 Bong model definition 必须自己提供所需的 first-person / third-person
  transform；不能继续从被借用的 vanilla JSON 的 `display` 块白嫖。GUI、fixed、ground
  等实际使用的 context 也必须有明确来源，不能因移除宿主而静默退回默认姿态。
- P0 → P1 → P2 的顺序是硬约束。通道尚未能渲染一个真实模板以前，不得删除 vanilla
  覆盖；否则玩家手中的武器会直接退回原版外观或 missing model。
- 不新增 server wire、template_id 协议或真元语义；本 plan 的默认范围是 client 渲染
  基建与资源。若 P0 证明现有协议不足，先停在开放问题交人工，不在实现阶段私自扩展
  server/agent 契约。
- 不引入与本通道无关的新依赖。loader 或 Minecraft/Fabric API 的可行性必须先由 P0
  spike 证明，再由 active plan 固化。

### 0.2 触发本 plan 的实证

现有做法覆盖 `minecraft/models/item/trident.json` 时，真正的原版三叉戟外观也会被
改变。这个副作用不是可接受的命名或资源整理问题，而是「宿主 item → 一份 vanilla
model」粒度错误的直接证据；因此本 plan 明确否掉「挑一个冷门 vanilla item 继续烧」
的修补方向。

### 0.3 完成定义

本 plan 全部完成时必须同时成立：

1. 所有在范围内的 Bong `template_id` 都有可核验的自有 model definition、几何/贴图
   来源和 transform；有意复用也通过显式单向关系表达。
2. FPV、TPV 和其他纳入范围的消费点走同一个生产 registry/model lookup；测试不能靠
   一个生产路径不会调用的 test-only `pub` seam 通过。
3. `BongWeaponModelRegistry.Entry` 不再承载 `hostItemSupplier` / `vanillaModelPath`，
   `WeaponRenderBootstrap` 不再以 vanilla namespace 覆盖 Bong model。
4. Bong 资源不再新增或依赖 `assets/minecraft/models/item/*.json` 覆盖；15 个现存
   覆盖全部逐个清理并验证对应原版物品恢复。
5. 未知或缺失 `template_id` 不得静默显示另一件物品；资源缺失、借用环、transform
   缺失都要在启动/资源 reload 或明确的 missing 状态中可见，而不是伪装成功。

## 1. 现状证据与迁移清单

### 1.1 当前渲染链（迁移前基线）

当前链路是：

```text
server 下发玩家手持 template_id
  → WeaponEquippedHandler / WeaponEquippedStore 保存装备态
  → HeldItemStackResolver 按 fallback 链合成 fake vanilla ItemStack
  → MixinHeldItemRenderer（FPV，updateHeldItems @TAIL）
    或 MixinPlayerEntityHeldItem（TPV，getMainHandStack/getOffHandStack @RETURN）
  → vanilla item renderer
  → vanilla model JSON
  → Special Model Loader（SML）把被覆盖的宿主路径接到 Bong OBJ
```

可回查的接入点：

- `client/src/main/java/com/bong/client/weapon/WeaponEquippedHandler.java:35-71`：
  `weapon_equipped` 的模板状态进入客户端装备态。
- `client/src/main/java/com/bong/client/weapon/HeldItemStackResolver.java:55-130`：
  主手/副手 fallback 以及 `weaponFake` / shield fake stack 的当前入口。
- `client/src/main/java/com/bong/client/mixin/MixinHeldItemRenderer.java:47-63`：
  FPV 把 resolver 返回值塞入 vanilla `HeldItemRenderer`。
- `client/src/main/java/com/bong/client/mixin/MixinPlayerEntityHeldItem.java:46-65`：
  TPV 改写 `getMainHandStack` / `getOffHandStack` 返回值。
- `client/src/main/java/com/bong/client/weapon/WeaponRenderBootstrap.java:24-39`：
  注册 SML `LOAD_SCOPE`，当前同时接管 `bong:*` 和被列入的 vanilla model 路径。
- `client/src/main/java/com/bong/client/BongClient.java:157-160`：渲染 bootstrap 的客户端
  初始化接线。

P0 必须把这条链改成「template_id → model lookup → 直接模型渲染」的生产路径；仅在
   mixin 中另开一个只供测试调用的入口，不算完成。

### 1.2 `BongWeaponModelRegistry` 的 39 条存量 entry

`client/src/main/java/com/bong/client/weapon/BongWeaponModelRegistry.java:13-18,20-238`
当前 `Entry` 含 `hostItemSupplier`、`vanillaModelPath`、`bongObjModelPath`，共 39 条：

```text
iron_sword, rusted_blade, bronze_saber, bone_dagger, hand_wrap,
bone_sword, lingmu_sword, wooden_staff, spirit_sword, flying_sword_feixuan,
axe_bone, pickaxe_bone, axe_iron, pickaxe_iron, stone_pickaxe, stone_axe,
pickaxe_copper, axe_copper, hoe_iron, hoe_lingtie, hoe_xuantie, bao_chu,
cai_yao_dao, cao_lian, dun_qi_jia, gua_dao, gu_hai_qian, bing_jia_shou_tao,
bone_spike, poison_needle, zhenyuan_mine, iron_sword_flawed, qing_feng_sword,
qing_feng_sword_flawed, ling_feng_sword, ling_feng_sword_flawed, stone_knife,
wooden_shield, bone_shield
```

现状统计要在 active 阶段重新以代码为准复核，但本骨架基线记录为：

- server `assets/items/*.toml` 中 `category = "weapon" | "tool"` 共 43 个模板；registry
  覆盖其中 34 个，缺失 9 个：`array_flag`、`blast_trap`、`bone_spike_crude`、
  `eclipse_needle_iron`、`herb_knife_iron`、`iron_dagger`、`slow_trap`、
  `warning_trap`、`wooden_club`。
- registry 另有 5 个不在上述 43 项统计口径内的条目：`bone_shield`、`hoe_iron`、
  `hoe_lingtie`、`hoe_xuantie`、`wooden_shield`。这 5 个不能在迁移时漏掉。
- 当前有意或无意共享宿主的规模为：`STONE_SWORD ×6`、`IRON_SWORD ×3`，以及
  `BONE`、`LEATHER`、`STONE_AXE`、`STONE_PICKAXE`、`FLINT_AND_STEEL` 各 ×2，
  合计 19 个模板。新通道的显式 borrow 不得复刻这种无声的共宿主耦合。
- 当前 39 条中约 14 条已有 Bong OBJ、25 条借用或白嫖原版形态；迁移时必须给每条
  一个明确 model source，不能用未知模板 fallback 掩盖缺口。

### 1.3 vanilla 覆盖资源

当前 `client/src/main/resources/assets/minecraft/models/item/` 下被 Bong 覆盖的 15 个
文件为：

```text
bone.json                 flint.json
leather.json              nether_star.json
nautilus_shell.json       phantom_membrane.json
totem_of_undying.json     diamond_sword.json
golden_sword.json         iron_sword.json
netherite_sword.json      iron_axe.json
iron_pickaxe.json         wooden_axe.json
wooden_pickaxe.json
```

其中 `flint.json` 是指向 `crystal_shard_dagger` 的孤儿 override，当前 SML scope 并未
真正接管它，可能表现为 missing model；清理时仍须把它作为独立条目核验，不能因它看似
不生效就漏删。P2 删除一个覆盖就回归对应真实 vanilla item，至少包含「原版三叉戟不
受 Bong 模型影响」的回归证据。

### 1.4 五套 VanillaIconMap 与边界

当前 Bong 模板仍从下列类合成 vanilla stack 或借用 vanilla 映射：

| 类 | 当前职责 | 迁移要求 |
|---|---|---|
| `BlockVanillaIconMap`（`HOST_ITEMS` 当前 15 项） | Bong 方块预览；另支持 `vanilla:<short>` 的真正 vanilla 方块预览 | Bong template 路径改为正式 model lookup；`vanilla:<short>` 真实 vanilla 预览必须和 Bong 路径分界，不能误删 |
| `HoeVanillaIconMap`（当前 4 项） | 锄头模板的 vanilla host 映射 | Bong 锄头不再走 host；保留真实 vanilla 预览边界 |
| `ScrollVanillaIconMap`（当前 1 项） | 可阅读残卷的 vanilla fallback | Bong 模板路径迁移，非 Bong 的 vanilla fallback 另行保持 |
| `ShieldVanillaIconMap` | 盾牌模板的 host 映射 | 盾牌走同一个 registry，不开第二套 SML scope |
| `WeaponVanillaIconMap` | 武器/工具/暗器模板的 fake stack 缓存 | 删除 `template_id → host Item` 逻辑，改为模型查询/渲染适配 |

`HeldItemStackResolver` 当前的优先级语义仍是契约：主手按武器 → 方块 → 锄头 → 残卷，
副手按武器 → 盾牌；其中「已装备武器但没有模型时不下探到下一类」的 gate 也要在新
渲染适配中保留。迁移不能为了去 stack 而改变游戏状态或 fallback 选择。

### 1.5 新接入对象：丹药与灵植

当前这些对象有 2D 图标或世界植物渲染，但没有本通道承诺的完整 3D 场景；P4 必须逐项
建立 model source 矩阵，不得把「有 icon」写成「已有 3D 模型」。

**丹药 8 项**（server item id）：

```text
guyuan_pill, kaimai_dan, huiyuan_pill, anti_spirit_pressure_pill,
huo_xue_dan, jin_zhong_dan, ji_feng_dan, hui_li_dan
```

**灵植 9 项**：

```text
chi_sui_cao, gu_yuan_gen, hei_gu_jun, ling_yan_shi_zhi, hua_xing_gen,
shou_xin_cao, tui_gu_teng, xu_yuan_rui, xue_po_lian
```

当前核验结果：

- `chi_sui_cao`、`gu_yuan_gen`、`hei_gu_jun`、`ling_yan_shi_zhi`、`xue_po_lian`
  有对应 server item id；另四个 `hua_xing_gen`、`shou_xin_cao`、`tui_gu_teng`、
  `xu_yuan_rui` 当前是 `server/assets/botany/plants.toml` 的植物 id，不是同口径
  的 server item。两类不得在 manifest 里混为一谈。
- 2D 图标在 `client/src/main/java/com/bong/client/inventory/ItemIconRegistry.java`
  及其 `bong-client:textures/gui/...` 资源中；默认保留 GUI 2D 图标，3D 通道的接入
  不应为了统一而牺牲背包性能和可读性。
- 世界植物当前由 `BotanyPlantRenderProfile`、`BotanyPlantStageWorldRenderer`
  （`client/src/main/java/com/bong/client/botany/BotanyPlantStageWorldRenderer.java`）
  和 `BotanyPlantEntityRenderer` 等 profile 驱动链路负责。P4 如果要替换这条已运行的
  世界渲染，必须拆成单独可验收的子阶段；默认只给采收后的 item 形态接模型通道。

### 1.6 掉落物和非手持边界

`client/src/main/java/com/bong/client/dropped/DroppedItemWorldRenderer.java:27-38,55-68`
当前从 `DroppedItemStore` 直接画 billboard，并非 vanilla `ItemRenderer`。P0 必须确认
它是否需要新 registry；若不需要，ground/drop 继续由现有 owner 负责，并在 plan 的
验收矩阵中明确「未纳入」；不得为了让每种场景看起来统一而偷偷替换掉这条链路。

## 2. P0 — 直查 baked model 的可行性 spike与决策收口

P0 是硬门。它不是把旧 fake stack 改个名字，而是证明 Minecraft/Fabric/SML 当前版本
能让 Bong 根据 `template_id` 取得并渲染一个真正的 `BakedModel`。P0 未通过时不得升
active，也不得删任何 vanilla override；失败时由人工选择另一种**无 vanilla 宿主**的
直接绘制方案，不能退回继续烧宿主。

### 2.1 canonical API 与模型加载

需要用最小 spike 验证并记录以下事实：

1. 选择并冻结 `BongItemModelRegistry`（或同等唯一 registry）的查询契约，例如
   `template_id → ModelDefinition → BakedModel`；名称可在 active 前调整，但不得出现
   `BongWeaponModelRegistry` 与另一份 registry 各自为真源的状态。
2. 确认 Fabric 的模型加载入口（例如 `ModelLoadingPlugin` / `ModelResourceProvider`
   或当前 SML 可用的等价入口）能加载 `bong:<template_id>`，并在 resource reload
   后更新/失效 baked model 缓存。不要假定某个 API 在 1.20.1 已存在，必须把版本实测
   结果写入决议证据。
3. OBJ/MTL/贴图的资源 location、namespace、路径规范和 loader scope 必须全程在
   `bong` 命名空间内；不得通过 `minecraft:item/<host>` 作为中转。
4. FPV 的 `MixinHeldItemRenderer` 和 TPV 的 `MixinPlayerEntityHeldItem` 都必须能
   消费同一个生产 model lookup。若 Minecraft 的 vanilla renderer API 只接受
   `ItemStack`，P0 要验证直接模型渲染的 adapter/自绘入口，而不是偷偷注册一个只为
   测试或宿主替代而存在的 fake vanilla stack。

### 2.2 transform 来源是 P0 硬门

去掉宿主后，第一/第三人称的 transform 不会凭空存在。P0 必须用一个真实模型证明：

- `firstperson_righthand`、`firstperson_lefthand`、`thirdperson_righthand`、
  `thirdperson_lefthand` 的旋转、平移、缩放来自 Bong 自己的 model definition 或
  等价的 Bong-owned transform 表，而不是 vanilla host JSON 的 `display`；
- 实际纳入范围的 `gui`、`fixed`、`ground`（以及需要时的 `head`）context 也有
  明确来源；未纳入的 context 必须写明 owner 和原因；
- 借用几何不等于借用变换：`borrows_from` 只表达单向几何/OBJ 关系，借用者仍拥有
  自己的 transform，避免一个模型的姿态修改无声影响所有借用者；
- FPV、TPV 左右手和资源 reload 后的姿态都能被截图或可重复的渲染断言锁住。

任何仍从 `Items.*` 宿主 model 取得 display 变换的实现均判 P0 FAIL。

### 2.3 场景和错误语义 spike

P0 至少覆盖一个自有模型和一个显式 borrow 候选，并记录：

- 装备 `template_id` 后 FPV/TPV 均显示正确 geometry 和 transform；
- GUI/物品检视、创造栏/搜索、REI（如本客户端存在）是否能看到或误看到内部模型；
  内部 Bong model 不得因技术注册而污染 vanilla 创造栏；
- 2D icon、lang/tooltip 和 model lookup 的责任边界；不因 missing lang 而显示内部
  `item.bong.<id>` 名称；
- `DroppedItemWorldRenderer` 是否使用本通道；若不使用，明确保持现状；
- unknown id、缺 JSON、缺 OBJ/MTL/贴图、borrow cycle、缺 transform 的行为。任何
  不能渲染的条目必须 fail-fast 或进入可观测的 missing 状态，禁止随机借另一物品的
  模型。

### 2.4 P0 交付物与放行条件

- 一个最小真实 `template_id` spike，使用生产将要走的 model lookup 和渲染 adapter；
- 一份决议记录，逐条回答 §2.1–§2.3，并带实现 file:line、资源 location 和运行结果；
- FPV/TPV、左右手、至少 GUI 或 fixed/ground 中纳入范围的 context 的可复核证据；
- 明确 client-only 注册、ItemGroup/REI、lang、掉落物是否需要额外阶段；
- 明确没有新 fake vanilla host、没有 `assets/minecraft` Bong override、没有 test-only
  可见性 seam。

## 3. P1 — 单一 canonical registry 与资源契约

P0 通过后，建立一个唯一的 `template_id → ModelDefinition/BakedModel` 真源。具体类名
可以在人工转 active 时依据 spike 调整，但不能保留两套平行 registry。

- registry 的 entry 至少表达：`template_id`、Bong model resource location、几何/贴图
  来源、各使用 context 的 transform，以及可选的显式 `borrows_from`。不得再有
  `hostItemSupplier`、`vanillaModelPath` 或隐式共享 host 字段。
- `registerAll`/资源 reload 的生产路径和 FPV/TPV lookup 使用同一份 entry；测试只验证
  真实生产契约，不增设专供测试的 `pub`。
- 用 manifest/生成校验覆盖当前范围，并与 `server/assets/items/*.toml` 的模板 id
  对拍。复用 `plan-registry-datafication-v1` 已定的 manifest/fail-fast 范式，升 active
  前先与该 plan 对齐格式，不另造互相矛盾的清单。
- 每个 manifest id 都必须有 model definition；每个 definition 引用的 OBJ/MTL/贴图
  都必须存在；每条 borrow 必须指向已知 id、不能形成环，且 borrow 者必须有自己的
  transforms。缺失在启动或 resource reload 时显式失败。
- unknown `template_id` 返回明确的 empty/error/missing 结果并留下可查日志，不能
  fall through 到 `STONE_SWORD`、`BONE` 等默认宿主。
- registry 缓存必须正确处理资源 reload；不能把第一次 bake 的旧 `BakedModel` 永久
  留在 `ConcurrentHashMap` 中而不失效。

P1 的测试至少包括：manifest 与 server id 集合/数量对拍、每项资源存在、重复 id、
unknown id、borrow 目标和环、各 transform context、reload 后 lookup，以及一个 FPV/TPV
    同时使用生产 lookup 的集成样例。

## 4. P2 — 迁移 39 条 entry，并撤掉 15 个 vanilla 覆盖

P2 只有在 P1 的最小模型通道和回归证据成立后开始。顺序是「一批 entry 有 Bong model
definition → FPV/TPV 对拍通过 → 再移除该批依赖的 vanilla override」，不能先删资源再
等待通道补齐。

- 39 条 entry 逐条迁移，保留原有独立 OBJ；原先白嫖 vanilla 几何的条目改为显式
  `borrowed_from` 或明确的 Bong-owned placeholder，不得把 `hostItemSupplier` 原样
  藏在新类里。
- 共享宿主的 19 个模板逐条验收：迁移后每个 template 的 geometry 和 transform 独立
  可查，修改 `stone_knife` 不会连带 `bone_sword`、`gua_dao` 或 flawed swords。
- 每个 model definition 都要有自己的 first/third-person 变换；即使几何借用，也不
  能复用宿主 `display` 或继承没有声明的姿态。
- 把 `BongWeaponModelRegistry.Entry` 迁成新语义，删除 `vanillaModelPaths()` 以及
  依赖这些路径的 `WeaponRenderBootstrap` vanilla namespace 分支；SML 若仍用于 OBJ
  加载，只接管 Bong namespace 的正式资源。
- `modelScript/core/held_item_common.py::write_assets` 的输出路径同步为
  `assets/bong/models/item/<template_id>.json`（或 P0 决定的 Bong-owned 等价路径），
  不再写宿主 JSON。生成器的输出、资源包 manifest 和 committed manifest 要能对拍。
- 15 个 vanilla 覆盖在对应迁移批次完成后逐个删除；每个删除都验证真实 vanilla item
  恢复，特别记录 `flint.json` 孤儿和原版三叉戟副作用回归。
- 迁移期间不动 server 的 `template_id` 下发和 gameplay 逻辑；出现协议不兼容先停下
  由人工处理，不以渲染 fallback 掩盖。

P2 交付物是逐条迁移矩阵（39/39：model source、transform、回归证据、override 清理
状态）和 FPV/TPV 截图或等价可重复渲染报告。当前 `client/tools/render_held_item.py`
缺少 `pyrender`，升 active 前要么补齐受批准的测试依赖，要么选能还原 MC display
变换的等价工具；不能把无法还原 transform 的 `modelScript/core/render_bbmodel.py`
截图当成完整验收。

## 5. P3 — 迁移五套映射与所有消费者

P3 处理「已有装备态如何进入新模型通道」，不是重新发明一套状态 store。

- `WeaponVanillaIconMap`、`ShieldVanillaIconMap`、`HoeVanillaIconMap`、
  `ScrollVanillaIconMap` 和 `BlockVanillaIconMap` 中所有 Bong template 路径改为
  canonical model lookup/渲染 adapter；不再为 Bong template 合成 fake vanilla
  `ItemStack`。
- `BlockVanillaIconMap` 的 `vanilla:<short>` 是真正的 vanilla 方块预览，属于独立
  owner；它可以继续使用真实 vanilla item 的预览机制，但不能被误判为 Bong template
  的宿主路径，也不能因清理 `HOST_ITEMS` 而破坏 `/place` 的 client gate。
- `HeldItemStackResolver` 需要改成或委托给模型 resolver，同时保留 §1.4 的优先级、
  主/副手语义、空模型的 gate 和 vanilla fallback；不把渲染迁移误改成 gameplay 状态
  或背包数据迁移。
- `MixinHeldItemRenderer` 和 `MixinPlayerEntityHeldItem` 必须都调用同一个生产 lookup；
  不能 FPV 已迁而 TPV 仍偷偷向 vanilla stack 注入，或反之。
- `WeaponEquippedHandler` / `WeaponEquippedStore` 继续作为模板状态的 owner；本阶段不
  新建重复 store/event。消费层只替换「模板到可渲染模型」这一步。

验收覆盖：主手/副手、武器/工具/盾/锄/残卷 fallback、未知 id、真正 vanilla block
preview、其他玩家 TPV 和本地玩家 FPV；每个分支都要有外部可观察的渲染结果或明确的
empty/missing 结果。

## 6. P4 — 丹药与灵植的 3D 接入

P4 不是把 17 个名字强行塞进同一套场景。升 active 时人工决定范围，并为每一项填表：
server/item 或 botany source、model source、transform、使用场景、2D icon 是否保留、
缺资源时的可见状态。

### 6.1 丹药 8 项

逐项覆盖 `guyuan_pill`、`kaimai_dan`、`huiyuan_pill`、`anti_spirit_pressure_pill`、
`huo_xue_dan`、`jin_zhong_dan`、`ji_feng_dan`、`hui_li_dan`。P4 必须先回答 3D 是否用于：

- 背包格/物品检视；
- 第一/第三人称手持；
- 世界掉落物；
- 炼丹炉成品展示。

场景不同就拆成可验收子阶段。默认保留已有 `ItemIconRegistry` 2D icon 作为 GUI 路径，
3D 接入不应让每个背包格都承担模型渲染成本。

### 6.2 灵植 9 项

逐项覆盖 `chi_sui_cao`、`gu_yuan_gen`、`hei_gu_jun`、`ling_yan_shi_zhi`、
`hua_xing_gen`、`shou_xin_cao`、`tui_gu_teng`、`xu_yuan_rui`、`xue_po_lian`。

- 有 server item id 的 5 项可按 item 形态接入正式 registry。
- 当前只有 botany plant id 的 `hua_xing_gen`、`shou_xin_cao`、`tui_gu_teng`、
  `xu_yuan_rui`，先明确采收物是否要新增 item model definition；不能把 plant world
  profile 当成 item model source。
- 如果产品要求替换 `BotanyPlantStageWorldRenderer` / `BotanyPlantEntityRenderer` 的
  世界植物渲染，必须另列「世界植物 renderer 迁移」阶段并验收所有成长阶段；P4 默认
  不替换既有 profile 驱动链路。

### 6.3 P4 的错误与借用规则

无专属几何时可以使用显式单向 `borrows_from`，但借用者仍必须有自己的 transform 和
场景声明；不能用 vanilla host 或 silent fallback。若仅有占位资源，manifest 必须把
`placeholder` 状态暴露给启动日志/测试，并列入后续清单，不得把开放问题伪装成完成。

## 7. P5 — 防复发门禁与全场景回归

P5 将资源结构问题变成可自动核验的门禁：

- 静态检查拒绝新增 `client/src/main/resources/assets/minecraft/models/item/*.json`
  作为 Bong model，并拒绝任何新 `hostItemSupplier`、`vanillaModelPath` 或 Bong
  `template_id → vanilla host` 映射；终态证明现有 15 个 Bong override 已清零。
- registry/manifest 与 server item source 的 id 集合对拍；39 条存量逐条有 model source，
  后续 9 条缺口和 P4 17 项有独立状态，不以一个默认模型蒙混。
- 借用关系必须单向、无环、可追踪；修改被借几何时，借用者的 transform 和预期外观
  有独立回归，不发生共宿主式连带变形。
- 资源包生成和校验覆盖新路径：`scripts/build-resourcepack.sh`、
  `scripts/test_build_resourcepack.py`、`server/src/network/resourcepack.rs` 的
  `DEFAULT_RESOURCE_PACK_MANIFEST`/sha1 同步；模型、OBJ、MTL、贴图变更不能让 committed
  manifest 静默过期。
- FPV/TPV 左右手、GUI、fixed/ground（若纳入）、真实 vanilla block preview、掉落物
  owner 边界分别回归；不能以单张手持截图代替整个 transform 矩阵。
- bot e2e 验证「登录 → 收到 `weapon_spec`/装备 template_id → 客户端解析并显示」的
  协议链路不回归。server 不因 client model registry 变化而掉线；若 P0 判定有 client-only
  registry 登录风险，必须把它变成显式 e2e 门。
- unknown/missing/cycle/缺 transform 的负例必须失败且带修复线索；不得显示另一件
  物品或悄悄回到 vanilla 宿主。

建议验收矩阵：

| 断言 | 证据 |
|------|------|
| 39 条存量全部可查 | registry/manifest 对拍 + 逐条 source matrix |
| 15 个覆盖已撤 | 资源目录检查 + 原版物品回归（含三叉戟） |
| 无 fake host | static grep + FPV/TPV 运行日志/调用链 |
| transform 不依赖 host | 每 context 的 model definition/渲染对拍 |
| 五套 map 无 Bong fake stack | 类级静态检查 + 主副手/fallback 集成回归 |
| 资源包一致 | build/test resourcepack + sha1 manifest 对拍 |
| 协议不回归 | bot e2e 登录、装备、`weapon_spec` 场景 |

## 8. 接入面、依赖关系与 owner 边界

### 8.1 既有 plan 的关系

**Integration preflight 记录（2026-09-16）**：本次预检已检 `docs/worldview.md`（client 渲染基建，无对应章节）、
`docs/finished_plans/`、`docs/plan-*.md`（active）、`docs/plans-skeleton/plan-*.md` 和
`docs/plans-skeleton/reminder.md`（已检，41 行，无物品模型/渲染相关待办），对照相关 finished plan、active
plan 和 `docs/plans-skeleton/` 的现有 owner，沿 `BongWeaponModelRegistry`、
`WeaponRenderBootstrap`、`HeldItemStackResolver`、五套 `VanillaIconMap` 和
`HeldItemRenderer` 接入面逐项核对。结果不是「没有重叠」，而是确认旧的手持注册骨架和
craft-chain skeleton 都把 vanilla 宿主当成既定路径；它们必须在升 active 前与本 plan 的
所有者裁决做 supersede/redirect 对表，不能让两个 plan 同时继续写同一条模型通道。

- `docs/plans-skeleton/plan-held-item-registration-v1.md` 当前提出「每模板注册
  render-only Fabric Item，再由 fake stack 走 vanilla HeldItemRenderer」。这与本所有者
  已否决的 fake vanilla host 方向冲突；本 plan 是「物品模型来源/消费通道」主题的新唯一
  owner。升 active 前由人工给旧 skeleton 写 supersede/redirect 或明确拆分边界；本 PR
  只新增本 skeleton，不修改旧 plan。
- `docs/plans-skeleton/plan-craft-chain-items-v1.md:11,22,81` 也命中同一接口面：其 P2
  把「导出 OBJ → `BongWeaponModelRegistry` 条目」写成既定模型链路，并要求检查
  `vanillaModelPaths` 后继续选择未占用的 vanilla item，甚至点名 `Items.TRIDENT` 候选。
  这与所有者已经否掉的 vanilla 宿主方向直接冲突，且三叉戟正是宿主覆盖会污染真实
  vanilla 物品的实证边界；该 skeleton 在人工 supersede/redirect 前不得按原文实施。
  craft-chain 的物品接入应依赖本 plan 建成的 canonical `template_id → BakedModel`
  通道。本 PR 只记录 preflight 和 owner 边界，不修改该旧 skeleton。
- `plan-weapon-v1` / `plan-weapon-v1.1` 的武器 gameplay、OBJ 产物和既有加载器评估不
  在本 plan 内重做；本 plan 只接管模型挂载和渲染消费路径。其 `§5.3.Y` 的加载器评估
  仍需在 P0 对表，不能把「继续 SML」当未验证的永恒承诺。
- `plan-armor-model-render-v1` 的护甲模型产物不与本 plan 的手持物 registry 混为一谈；
  若未来共享 OBJ loader，需通过明确公共契约接入，不复制 registry。
- `plan-tarkov-backpack-v1` / `plan-client-render-gap-v1` 的背包图标和 client render
  接入只作为 P4/P5 场景依赖，不在本 plan 中改其既有 UI owner。
- `plan-registry-datafication-v1` 提供清单数据化与 fail-fast 的范式；本 plan 复用它的
  约定，但不另造一份相互独立的模板真源。
- `plan-fpv-cast-av-v1` 可能触及同一条 `HeldItemRenderer` 链路；P2/P3 开始前必须
  对表其 in-flight 变更，保证两个 plan 不各自重写同一个 mixin。
- `plan-refactor-client-store-lifecycle-v1` 的 `WeaponEquippedStore` owner 保持不变；
  本 plan 不新建并行装备态 store。

### 8.2 非目标

- 不改 Rust server 的 item 经济、战斗、真元、schema 或 wire event；已有
  `template_id` 是输入，模型通道不凭空引入新的 gameplay 语义。
- 不改 `docs/worldview.md`、命名正典、骨币经济或任何 qi ledger；本 plan 没有 worldview
  / qi_physics 锚点。
- 不把 `DroppedItemWorldRenderer`、botany 世界成长渲染、护甲 renderer 或背包 UI 在
  没有 P0/P4 明确决议时顺手替换。
- 不以「再找三个冷门 vanilla item」为过渡交付；也不把旧 host 逻辑藏进一个看似新的
  `BongModelItem` 适配层。

### 8.3 升 active 前的人工收口

人工需要根据 P0 spike 结果决定：

1. Fabric/SML 1.20.1 的直接 baked-model 加载和 FPV/TPV 绘制 API；
2. model JSON `display` 与自有 transform 表的最终 schema；
3. 是否需要 Bong-owned registry object、是否进入 ItemGroup/REI，以及如何保证不污染
   vanilla 创造栏；
4. GUI、fixed、ground、掉落物和 botany 世界渲染的明确 owner；
5. 旧 `plan-held-item-registration-v1` 的 supersede/redirect 文字和与 active plan 的
   文件级冲突窗口；
6. P2 截图工具缺 `pyrender` 时采用的、能还原 Minecraft display 变换的验收工具。

这些开放问题不阻止本 skeleton 作为规划入口，但在 P0 证据落盘前不得宣称通道已可用，
也不得提前删除 vanilla override。
