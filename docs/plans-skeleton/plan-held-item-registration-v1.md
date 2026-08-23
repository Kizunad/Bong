# plan-held-item-registration-v1 — 手持物改注册真 Item，废除 vanilla 宿主劫持

> **一句话主题**：给每个 Bong 手持物模板注册一个**自有的 render-only Fabric Item**，让模型直接落 `assets/bong/models/item/<template_id>.json`，从结构上废除「劫持 vanilla 宿主 item 的 model JSON」这套机制——现状 39 个注册项里有 **19 个在 7 个宿主上共享**（共宿主 ⇒ 必然同形，给其中任何一个做专属模型都会连带改掉其余），另有 **9 个 server 侧真武器/工具压根没注册 ⇒ 手持渲染是空手**。

**状态**：骨架（skeleton）。升 active 前按 docs/CLAUDE.md §五 收口 §9 的开放问题（**P0 的客户端单侧注册可行性是硬门，没验真之前不许升**）。

| 阶段 | 主题 | 状态 |
|------|------|------|
| P0 | 可行性验真 + 决策收口——client-only item 注册在 Valence 服下会不会掉线 | ⬜ |
| P1 | `BongHeldItemRegistry` 基础设施：`BongModelItem` + 数据化清单 + fail-fast | ⬜ |
| P2 | 39 个现存注册项迁移；删宿主劫持、删 `WeaponRenderBootstrap` 的 vanilla 分支 | ⬜ |
| P3 | 补齐 9 件未注册模板（含 `iron_dagger`）+ 小刀三件套装机 | ⬜ |
| P4 | 「借用别人模型」显式数据化，替代隐式共宿主 | ⬜ |
| P5 | 回归：资源包 sha1、bot e2e、清理死 override | ⬜ |

---

## 0. 接入面 checklist（docs/CLAUDE.md §二）

- **进料**：
  - `server/assets/items/*.toml` 的 `category = "weapon" | "tool"` 模板清单（43 件）——**新的唯一事实源**，取代现在硬编码在 Java 里的 39 条
  - `WeaponEquippedStore` / `EquippedShieldStore`（client 侧已装备态，现有）
  - `modelScript/generators/gen_*.py --install` 产出的 OBJ/MTL/贴图
- **出料**：
  - `client/src/main/resources/assets/bong/models/item/<template_id>.json`（每模板一份，不再有 `assets/minecraft/models/item/*.json`）
  - 注册进 `Registries.ITEM` 的 `bong:<template_id>`，供 `HeldItemStackResolver` / `WeaponVanillaIconMap` 造 stack
- **共享类型 / event**：复用现有 `BongWeaponModelRegistry.Entry` 的语义（template_id → 模型），**不新建 store、不新建 event**；`Entry` 本身会被改形（去掉 `hostItemSupplier` / `vanillaModelPath`，加 `ownItem` / `borrowsFrom`）
- **跨仓库契约**：**无新增 wire**。server 侧不动——它下发的一直是 `template_id`，本 plan 只改 client 怎么把 template_id 变成一个可渲染的 stack
- **worldview 锚点**：无（纯 client 渲染基建，不涉及境界/经济/修炼语义）
- **qi_physics 锚点**：无（不碰真元流动）

## 1. 为什么现在必须动

### 1.1 宿主机制的实际形状

Bong 的物品不是 vanilla item，`player.getMainHandStack()` 恒为 `EMPTY`。vanilla `HeldItemRenderer` 拿到空 stack 时**根本不进 item 渲染路径**，直接画空手动画。所以 `MixinHeldItemRenderer.bong$overrideHeldItemsForBongWeapons` 每 tick 塞一个 fake stack；那个 stack 必须是**已注册的 `net.minecraft.item.Item`**，于是借了 vanilla item 当载体，再用 SML 把该 item 的 model JSON 劫持成 Bong 的 OBJ（`WeaponRenderBootstrap.isBongManagedModel` 把 `minecraft:item/<host>` 纳入 LOAD_SCOPE）。

**关键性质：劫持粒度是「宿主 item → 一份 model JSON」。** 所以共宿主的模板必然长得一模一样，而且这个约束在代码里没有任何地方拦着——加一条共宿主的注册项不会报错，只会让两件物品在游戏里悄悄变成同一个样。

### 1.2 现状实测（`BongWeaponModelRegistry`，2026-08-24）

```
注册项 39      有自有 OBJ 14      借原版皮 25
共宿主的 host 7 个，牵涉 19 个模板：
  item/stone_sword      ×6  bone_sword, gua_dao, iron_sword_flawed,
                            qing_feng_sword_flawed, ling_feng_sword_flawed, stone_knife
  item/iron_sword       ×3  iron_sword, qing_feng_sword, ling_feng_sword
  item/bone             ×2  bone_dagger, bone_spike
  item/leather          ×2  hand_wrap, bing_jia_shou_tao
  item/stone_pickaxe    ×2  stone_pickaxe, pickaxe_copper
  item/stone_axe        ×2  stone_axe, axe_copper
  item/flint_and_steel  ×2  dun_qi_jia, gu_hai_qian
```

**server 侧 43 件 `category=weapon|tool` 模板，注册表只覆盖 34 件。未注册的 9 件手持渲染是空手：**

```
array_flag  blast_trap  bone_spike_crude  eclipse_needle_iron  herb_knife_iron
iron_dagger  slow_trap  warning_trap  wooden_club
```

其中 `iron_dagger`（凡铁匕首，`workbench_materials.toml`，`base_attack 4.0`）是手搓链上能做出来的真武器，做出来握在手里是空的。

另有一处已知腐烂：`minecraft/models/item/flint.json` 是指向 `crystal_shard_dagger` 的**孤儿 override**（SML 未注册该 scope → missing model），注册表注释里记着但没人敢动，因为删它要同步资源包 sha1。

### 1.3 触发本 plan 的具体事件

做小刀三件套（`modelScript/generators/gen_knife_trio.py`，参考 `orthograph #8/#9/#10`）时发现：石刃的宿主 `item/stone_sword` 被 6 个模板共用，装上模型会把 `bone_sword` / `gua_dao` / 三把 flawed 剑一起变成石刃；骨刺宿 `item/bone`，装上会把 `bone_dagger` 变成骨刺。**要么给三把刀各烧一个冷门 vanilla item（不 scale，剩下 22 件借皮的没那么多冷门 item 可烧），要么根治。** 用户 2026-08-24 裁决：根治。

### 1.4 与既有 plan 的关系（防孤岛）

- `plan-weapon-v1 §5.3.Y` 当年就把「加载器路径」挂起了，写明「评估时机（不是现在）：当 plan-armor-v1 或 plan-monster-v1 启动时重新评估……届时武器可顺便统一迁移到该加载器」。本 plan **不动加载器**（继续 SML + OBJ），只动「模型挂在谁身上」，与 §5.3.Y 的评估互不阻塞。
- `BongWeaponModelRegistry` 的类 javadoc 自己写着「便于后续替换为真正的 template_id -> baked model 查询」——本 plan 就是兑现这句。
- **不属于重构总纲 R1-R10 任何一轨**：R7 是 Screen/hud/keybind，R9 是 cast/AV 语义与 `SkillAvBinding`，`client/src/main/java/com/bong/client/weapon/` 不在 `plan-refactor-master-v1 §66` 的文件域归属表里。本 plan 按独立 feature/基建轨走，但需在 §5.6 冻结窗口生效期间与 R2（client store 生命周期）协调 —— `WeaponEquippedStore` 归 R2。
- 与 `plan-registry-datafication-v1`（基建轨，「硬编码配方/功法/方块表迁数据 + fail-fast」）**同族**：P1 的「模板清单数据化」应当复用它定下的 manifest 生成与 fail-fast 范式，而不是另造一套。**升 active 前必须先读那份 plan 对齐格式。**
- `plan-fpv-cast-av-v1`（active）动的是 FPV 手臂/施法动画，读同一条 `HeldItemRenderer` 链路但不碰 stack 来源。P2 删 `WeaponRenderBootstrap` vanilla 分支时需与其 in-flight PR 对表。

---

## 2. P0 — 可行性验真 + 决策收口

**这是硬门。** 下面第一条不成立的话整个 plan 报废，必须换回宿主方案或走「绕开 ItemRenderer 自己画」（§9 #2）。

### 2.1 开放问题

#### #1 client-only item 注册在 Valence 服下会不会掉线 —— **未决，P0 必须实测**

Bong 的 server 是 Rust/Valence，**不是** Fabric 服。client 单侧往 `Registries.ITEM` 注册 `bong:<id>` 之后：

- MC 1.20.1 的 item registry 是**静态注册表**，不参与 `RegistrySync`（只有 biome/dimension 那批 dynamic registry 走同步），理论上单侧注册不会触发登录期校验失败；
- 但 Fabric 的 registry-sync 模块自己会在登录时做一轮比对，行为取决于版本与配置；
- 且 Bong 从不把这些 item 放进 vanilla 背包（stack 是 client 侧合成的），协议上不该出现这些 id。

**验真方式**：写一个最小 spike——注册 1 个 `BongModelItem`，起 Valence 服（`export BONG_SKIP_SKIN_PREFETCH=1`）+ `gradle runClient` 连上，确认① 能进服不掉线 ② 该 item 的 model JSON 被加载 ③ 合成 stack 能在手里渲染 ④ GUI 图标能画。**四条全绿才算 PASS**，任一条挂就回 §9 #2。

**记入 bot e2e**：加一条协议级场景验「登录 + 装备武器 + 收到 weapon_spec」不因新注册表项而回归。

#### #2 item 会不会进创造模式搜索 / REI 之类

注册 item 默认会出现在创造物品栏搜索里。**决议方向**（P0 拍板）：不加进任何 `ItemGroup`，并确认 1.20.1 下未归组的 item 不会出现在搜索 tab；若仍出现，改为注册后从 `ItemGroups` 移除或加 `bong:hidden` 标记。玩家在创造栏看到一堆 Bong 内部 item 是不可接受的。

#### #3 lang 条目

未注册 lang 的 item 会显示 `item.bong.stone_knife` 原始 key。Bong 的物品名来自 server 的 `template.name`（如「石刃」），tooltip 走 Bong 自己的 UI。**决议方向**：不出 lang 文件，因为这些 item 的名字永远不该被 vanilla tooltip 读到；但 P0 要确认没有任何路径会把它们的 vanilla 名字画出来（尤其掉落物 hover、F3+H）。

#### #4 掉落物 / 物品实体渲染

`ground` display 槽今天就在 model JSON 里，但 Bong 的掉落物走的是自己的 `DroppedLoot` 链路。P0 确认这条链路是否也经 `ItemRenderer`；若经，本 plan 顺带修好；若不经，写进 §10 明确不在范围。

### 2.2 P0 交付物

- `client/src/main/java/com/bong/client/weapon/BongModelItem.java`（spike 版，1 个 item）
- spike 报告写进本文件 §2.1 各条下方（按 docs/CLAUDE.md §5.1 的 `§N → §N.1` 决议模式收口）
- `scripts/bot/scenarios/held_item_registration_login.py`（协议级登录回归）

---

## 3. P1 — `BongHeldItemRegistry` 基础设施

- **`BongModelItem extends Item`**（`client/src/main/java/com/bong/client/weapon/BongModelItem.java`）：render-only，`maxCount=1`，不可用、不可放置；持有 `templateId` 便于调试。
- **`BongHeldItemRegistry`**（同包，取代 `BongWeaponModelRegistry` 的注册职责）：
  - `static void registerAll()` —— client init 阶段按清单注册全部 `bong:<template_id>`
  - `static Optional<Item> itemFor(String templateId)`
  - `static Optional<Entry> get(String templateId)`；`Entry` 改形为
    `record Entry(String templateId, Item item, String modelPath, String borrowsFrom)`
- **清单数据化**：`client/src/main/resources/assets/bong/held_items.json`，由
  `scripts/gen_held_item_manifest.py` 从 `server/assets/items/*.toml` 生成（category=weapon/tool 全量）。
  **fail-fast**：client 启动时校验「清单里每个 id 都有 model JSON」「每个 model JSON 指向的 OBJ/MTL/贴图都存在」，缺一个就抛异常而不是渲染成 missing model —— 对齐 `plan-registry-datafication-v1` 的 fail-fast 范式。
- **测试**：`BongHeldItemRegistryTest` —— 清单与 server TOML 对拍（数量 + id 集合完全一致）、每项资源存在、无重复注册、`itemFor` 对未知 id 返回 empty。
- **兼容**：P1 阶段 `BongWeaponModelRegistry` 的公开 API 保持可用（内部委托给新注册表），`HeldItemStackResolver` / `WeaponVanillaIconMap` 不动，保证 P1 单独 merge 不改变任何现有表现。

## 4. P2 — 39 项迁移 + 拆宿主

- 每个模板一份 `assets/bong/models/item/<template_id>.json`（`sml:builtin/obj` + 自己的 OBJ + display）
- **删除** `client/src/main/resources/assets/minecraft/models/item/*.json` 全部 Bong 劫持 override（含 §1.2 那个 `flint.json` 孤儿）
- **删除** `WeaponRenderBootstrap.isBongManagedModel` 的 vanilla 命名空间分支 + `BongWeaponModelRegistry.vanillaModelPaths()`
- `HeldItemStackResolver` / `WeaponVanillaIconMap` 改用 `BongHeldItemRegistry.itemFor`
- **回归判据**：迁移前后对 39 个模板逐个截图对拍（`client/tools/render_held_item.py`，需先补 pyrender 依赖或换等价工具——见 §10），**共宿主的那 19 个必须从「同形」变成「各自的形」**，其余 20 个表现不变

## 5. P3 — 补齐 9 件未注册模板 + 小刀三件套装机

- §1.2 那 9 件全部进清单；没有专属模型的先用 P4 的显式借用指到一个合理的既有模型
- **`iron_dagger` 单独点名**：它是本 plan 的即时受益者，修好「手搓出来握在手里是空的」
- 小刀三件套（`stone_knife` / `iron_dagger` / `bone_spike`）接上 `gen_knife_trio.py --install` 的产物
  —— 资产已在 `feat/knife-trio-model` 分支完成 3 轮打磨，本阶段只做接线
- **`held_item_common.write_assets` 同步改形**：去掉写宿主 JSON 的分支，改为写 `assets/bong/models/item/<key>.json`

## 6. P4 — 借用关系显式化

25 件借原版皮的模板里，一部分是**有意复用**别人的模型（如 `bing_jia_shou_tao` 借 `hand_wrap.obj`，注释写着「同为手戴护具，造型最近」）。这类改成清单里的显式字段：

```json
{ "id": "bing_jia_shou_tao", "borrows_from": "hand_wrap" }
```

生成 model JSON 时指向被借者的 OBJ。**和共宿主的区别**：借用是**单向、显式、可查**的，改被借者不会意外改到别人；而共宿主是双向耦合且无声。

剩下真正只是「还没做模型」的，清单里标 `"placeholder": true`，P5 的 fail-fast 允许它们存在但列进启动日志，不许悄悄糊过去。

## 7. P5 — 回归与清理

- 资源包 sha1 同步（`server/src/network/resourcepack.rs` 的 `DEFAULT_RESOURCE_PACK_MANIFEST` + committed manifest；见 `scripts/build-resourcepack.sh`）
- `scripts/build-resourcepack.sh` / `test_build_resourcepack.py` 覆盖新路径
- bot e2e：装备各类武器/工具/盾 → 确认 `weapon_spec` 下发与 client 侧解析不回归
- `BongWeaponModelRegistryTest` 迁移/替换为 `BongHeldItemRegistryTest`
- 删掉 `Entry.hostItemSupplier` / `vanillaModelPath` 与相关注释

---

## 8. 可核验抓手（下游工具 grep 用）

| 类别 | 符号 / 路径 |
|---|---|
| 新类 | `BongModelItem` / `BongHeldItemRegistry` |
| 改形 | `BongWeaponModelRegistry.Entry`（去 `hostItemSupplier`/`vanillaModelPath`，加 `borrowsFrom`） |
| 删除 | `WeaponRenderBootstrap.vanillaModelPaths` 分支、`assets/minecraft/models/item/*.json` |
| 数据 | `client/src/main/resources/assets/bong/held_items.json`、`scripts/gen_held_item_manifest.py` |
| 生成器 | `modelScript/core/held_item_common.py::write_assets`（去宿主分支） |
| 测试 | `BongHeldItemRegistryTest`、`scripts/bot/scenarios/held_item_registration_login.py` |
| 资源包 | `server/src/network/resourcepack.rs::DEFAULT_RESOURCE_PACK_MANIFEST` |

## 9. 备选方案（P0 挂了才走）

#### #1 回退宿主方案，只给小刀三件套各烧一个冷门 vanilla item
`heart_of_the_sea` / `echo_shard` / `glow_ink_sac`（三者全仓 0 引用）。能立刻上线，但债照旧，且冷门 item 池会被逐渐耗尽。

#### #2 绕开 `ItemRenderer` 自己画
不塞 fake stack，直接在 `MixinHeldItemRenderer` / `MixinPlayerEntityHeldItem` 里按 template_id 画 Bong 模型。最彻底、连 Item 都不要，但 display 变换（FPV/TPV/GUI/ground 四套）、GUI 图标、掉落物渲染全部要自己实现一遍——即把 MC 已经做好的一大块重写。数天量级、风险高，只在 #1 也不可接受时考虑。

## 10. 已知遗留 / 不在本 plan 范围

- **加载器选型不动**：继续 SML + OBJ，不碰 `plan-weapon-v1 §5.3.Y` 的 GeckoLib fork 评估
- **`client/tools/render_held_item.py` 本机跑不了**（缺 `pyrender`）。P2 的截图对拍需要它，**升 active 前要么补依赖、要么换等价工具**（`modelScript/core/render_bbmodel.py` 能渲 bbmodel 但不还原 MC 的 display 变换与 item 光照）
- **贴图明度未经真机标定**：小刀三件套的 ×1.25 放亮系数是从既有手持物贴图反推的经验值，P3 装机后需实测再调
- **掉落物渲染**：见 §2.1 #4，P0 判定后再决定是否纳入
- **server 侧不动**：本 plan 全程不改 Rust
