# plan-copper-armor-v1 — 古铜札甲（铜甲）专属模型与渲染落地

> **一句话主题**：为 `server/assets/items/armor.toml` 中已定义的 4 件铜甲（`armor_copper_helmet` / `armor_copper_chestplate` / `armor_copper_leggings` / `armor_copper_boots`）实现专属 3D Blockbench 模型生成器（`modelScript/generators/gen_copper_armor.py`），通过标准参考图流水线与 3 轮打磨产出高质感中式古铜札甲/泡钉甲片资产，并接通客户端 ModelPart 渲染与测试闭环。

## 阶段总览

| 阶段 | 内容 | 状态 |
|----|------|----|
| P0 | 调研与管线设计（概念图、物品图标、MC体素三视图、爆炸分解图） | ✅ 2026-09-08 |
| P1 | Round 1 First Cut（生成器程序化建模 + 64×64 贴图 + 单测） | ✅ 2026-09-08 |
| P2 | Round 2 人工闸门（接触表 + 特征清单点名 9/9 通过） | ✅ 2026-09-08 |
| P3 | Round 3 终轮打磨 + PROMISE 担保 + 客户端接线与测试闭环 | ✅ 2026-09-08 |

## Integration Preflight

依据 `docs/CLAUDE.md` §一，于 2026-09-09 以当前任务分支 `HEAD=93c57eb64` 及已同步的 `origin/main=8380a5be8` 完成防孤岛检索；所有结论均来自实际 grep / 行号核对，不把作者资产或旧审计稿当作运行时事实。

- **`docs/worldview.md`**：检索 `护甲|防具|装备|铜甲|古铜|札甲|轻装|穿戴|手持`。`worldview.md §四 L256-L260` 将护甲作用锚定为既有部位伤口状态的防护，`worldview.md §十 L872-L880` 是灵气总量约束；`worldview.md §十三` 的既有区域命名与本 plan 无关。铜甲只沿用既有 `armor_copper_*` 身份和装备语义，本 plan 不新增境界、货币、zone、真元流动或 worldview 名词。
- **`docs/finished_plans/`**：`plan-armor-visual-v1:31-50,116-121` 已交付凡物甲的颜色、配方与 GUI icon，其中铜甲仍是古铜色视觉规格；`plan-armor-model-render-v1:76-81,87-91,141-145` 已将铜甲明确留在染色皮甲兜底，并要求后续材质另立资产 plan；`plan-model-asset-v1:120-129` 只有铜甲参考 prompt。未发现已归档 plan 已交付铜甲四槽 `ArmorModelRegistry`/`ArmorPartModel` 运行时链，因此本 plan 承接该明确后续，不重复铁/骨甲实现。
- **active `docs/plan-*.md`**：当前任务分支尚未包含 promotion 后的 active 文件，但 `git show origin/main:docs/plan-client-render-gap-v1.md` 已核实其状态为 Active（`:5`），P3 §6.2（`:327-332`）仍把 `hide`、`scroll_wrap`、`straw`、`copper`、`spirit_cloth` 列为五套运行时防具缺口。两计划不合并：铜甲四槽模型、贴图、客户端接线与本 PR 的验证归 `plan-copper-armor-v1` 单一 owner；`plan-client-render-gap-v1` 的 P3 §6.2 仅保留范围/路线引用，后续不得重复实现同一铜甲交付物。本 PR 不修改该 active plan。
- **`docs/plans-skeleton/` 与 `reminder.md`**：检索 `client|render|渲染|手持|防具|护甲|模型|weapon|armor|copper|铜甲`。`plan-held-item-registration-v1:18-30,74-92` 是手持物 render-only Item 与宿主解耦的另一条 owner，不覆盖防具四槽模型；未发现另一份铜甲专属骨架。仓库实际 reminder 为 `docs/plans-skeleton/reminder.md`，未命中铜甲或本 plan 的待办；`docs/reminder.md` 不存在。故不合并其它 skeleton、不回写 reminder。

## 接入面 Checklist

- **进料**：`server/assets/items/armor.toml` 中的 `armor_copper_*` 4 件模板（已存在，此前走染色皮甲兜底 #B87333）。
- **出料**：
  - `modelScript/assets/refs/ref_copper_armor_{concept,icon,three_view,exploded}.png` 4 张标准参考图；
  - `modelScript/manifests/CopperArmor.manifest.toml` 人写特征清单（9 项核心构件点名）；
  - `modelScript/generators/gen_copper_armor.py` 生成器；
  - `modelScript/models/armor/copper/*.bbmodel` 作者资产（单件 + 全身合模）；
  - `client/src/main/resources/assets/bong/textures/armor/copper_*/0.png` 运行时 64×64 贴图；
  - `client/src/main/java/com/bong/client/armor/ArmorPartModel.java` CUBE_TABLES 注入；
  - `client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java` 注册项接入。
- **共享类型 / event**：复用 `ArmorPartModel`、`ArmorModelSpec`、`ArmorFeatureRenderer`，零新增 server event / 零协议改动。
- **worldview 锚点**：
  - `worldview.md §四`（物理防护，轻装机动），`hammered copper scale armor, green patina`。
  - 配方：铜矿 ×4 + 兽皮 ×2（凝脉轻装，防御+7，耐久160）。
- **造型与差异化定位**：
  - **铁甲**：宽大厚重整块锻铁板 + 粗犷外露大铆钉 + 深灰铁锈，沉重硬直。
  - **骨甲**：异兽肋骨弯弧 + 脊椎骨节 + 粗麻骨钉缠绑，灰白骨相凸起。
  - **铜甲**：中式古铜札甲（鱼鳞铜叠片）+ 贯穿金铜脊梁 + 护额护耳 + 三阶护颈帏帘 + 双肩二阶披膊护肩 + 护心镜 + 铜泡钉 + 局部铜绿（patina）氧化斑 + 麻绳腰带系结，突出柔韧轻快与古韵。

## Finish Evidence

### 落地清单
- 运行时：
  - `client/src/main/java/com/bong/client/armor/ArmorPartModel.java`（新增 `copper_helmet`, `copper_chestplate`, `copper_leggings`, `copper_boots` 静态表）
  - `client/src/main/java/com/bong/client/armor/ArmorModelRegistry.java`（注册 4 件铜甲）
  - `client/src/main/resources/assets/bong/textures/armor/copper_{helmet,chestplate,leggings,boots}/0.png`（正式贴图）
- 作者资产与参考图：
  - `modelScript/assets/refs/ref_copper_armor_concept.png`
  - `modelScript/assets/refs/ref_copper_armor_icon.png`
  - `modelScript/assets/refs/ref_copper_armor_three_view.png`
  - `modelScript/assets/refs/ref_copper_armor_exploded.png`
  - `modelScript/manifests/CopperArmor.manifest.toml`
  - `modelScript/generators/gen_copper_armor.py`
  - `modelScript/models/armor/copper/*.bbmodel`（含 `CopperSetOnPlayer.bbmodel` 全身合模）
- 自动化测试：
  - `modelScript/tests/test_gen_copper_armor.py`（4 个 Python 单测通过）
  - `client/src/test/java/com/bong/client/armor/ArmorPartModelTest.java`（FNV-1a cube digest pin 锁死）
  - `client/src/test/java/com/bong/client/armor/ArmorModelRegistryTest.java`（12 件全注册覆盖与差异化断言）
