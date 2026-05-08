# Bong · plan-craft-v1 · 骨架

通用手搓面 —— inventory 内集成「手搓」标签，列表式合成 UI（左配方列表 + 右详情面板 + 进度条）。服务多个流派的"轻度仪式化"合成需求（蚀针 / 自蕴煎汤 / 伪皮 / 阵法预埋件 / 凡器等），区别于 forge 的 4 步状态机 + alchemy 的火候模式 + vanilla 的摆放式合成。**无方块、无站**——纯 inventory 标签实装。**单任务**（同时只能 1 个手搓在跑，简化决策成本）。**in-game 时间推进**（玩家在线累积，下线暂停）。**配方解锁三渠道**（残卷 / 师承 / 顿悟，无流派自动解锁——worldview §九 信息差就是优势的物理化身）。**首版不实装磨损税 + 装备加速**（留 v2）。

**世界观锚点**：`worldview.md §六:654-707 顿悟（关键时刻人生选择，配方解锁源之一）`· `§九:843 信息比装备值钱（配方=信息）`· `§十 残卷掉落（道伥/坍缩渊 jackpot）`· `§十一 NPC 散修师承交易`

**library 锚点**：`peoples-0007 散修百态` 师承教学路 · `cultivation-0006 经脉浅述` 配方书残篇范本

**前置依赖**：

- `plan-skill-v1` ✅ + `plan-hotbar-modify-v1` ✅ → SkillSet 解锁状态可读
- `plan-inventory-v2` ✅ → inventory tab 框架（Tarkov grid 已实装，加 craft tab）
- `plan-input-binding-v1` ✅ → inventory 内键位绑定通用接口
- `plan-HUD-v1` ✅ → UI 渲染框架

**反向被依赖**（各流派 plan 注册自家配方）：

- `plan-dugu-v2` 🆕 skeleton → 蚀针（凡铁/异兽骨档）+ 毒源煎汤（凡毒/灵毒/上古毒档）
- `plan-tuike-v2` 🆕 → 伪灵皮（轻/中/重档）
- `plan-zhenfa-v2` 🆕 → 真元诡雷预埋件
- `plan-anqi-v1` ✅（vN+1 留）→ 暗器载体充能（已 anqi 内 UI，可选迁通用）
- `plan-tools-v1` ✅ → 凡器（采药刀/刮刀/镰刀）当前已实装但无专属 UI，迁入手搓
- `plan-tsy-loot-v1` ✅ → 残卷掉落物理依据
- `plan-narrative-v1` ✅ → 配方解锁 narration（首学 / 师承 / 顿悟）

---

## 现状对齐（2026-05-08 升 active 时核验）

| 接入面声明 | 代码实际状况 | 影响 |
|---|---|---|
| `craft::CraftRegistry` 通用手搓注册表 | ❌ 顶层 `server/src/craft/` 模块**尚未建** | P1 主体新建，符合 plan 设计 |
| `alchemy::RecipeRegistry` | ✅ `server/src/alchemy/recipe.rs` + `resolver.rs` 已存（丹药专用） | 命名空间分离：本 plan 用 `craft::CraftRegistry` 不与 alchemy 冲突 |
| `inventory::ItemInstance.spirit_quality / template_id` | ✅ `server/src/inventory/mod.rs:212` 实装 | P1 配方 `materials: Vec<(ItemId, u32)>` 直接读 |
| `botany::PlantRegistry` | 🔄 实名是 **`botany::PlantKindRegistry`**（`server/src/botany/registry.rs`，`npc::farming_brain.rs:475` / `lingtian::seed.rs:29` 已使用） | §3 数据契约用 `botany::PlantKindRegistry` 而非草稿的 `PlantRegistry` |
| `cultivation::Cultivation { realm, qi_color }` | 🔄 `Cultivation` 含 `realm`，**`QiColor` 是独立 component**（不是 Cultivation 内字段） | requirements `qi_color_min` 检查走 `Query<(&Cultivation, &QiColor)>` |
| `cultivation::QiColor` | ✅ `server/src/cultivation/components.rs` + `color.rs:105 evolve_qi_color` 已实装 | 染色 gate 直接读 |
| `qi_physics::ledger::QiTransfer` | ✅ `server/src/qi_physics/ledger.rs` 实装 | qi_cost 走 ledger 守恒律 P1 直接调 |
| `skill::SkillSet { learned_recipes }` | 🔄 `SkillSet` 在 `server/src/skill/components.rs:91` 已存，但**目前无 `learned_recipes` 字段** | P1 阶段需扩 SkillSet 加该字段（或新建 `RecipeUnlockState` resource，§3 已计划后者） |
| `craft::CraftSession` component | ❌ 不存 | P1 新建 |
| inventory CraftTab UI | ❌ `client/src/main/java/com/bong/client/` 下无 craft 相关类 | P2 新建 |

> **结论**：plan 设计与现状一致——本 plan 是新建底盘，无既有代码冲突。仅需在 §3 数据契约把 `botany::PlantRegistry` 改写为 `botany::PlantKindRegistry`，QiColor 接入按独立 component 处理；其余按草稿推进。

---

## 接入面 Checklist

- **进料**：`inventory::Inventory` / `cultivation::Cultivation` + 独立 `QiColor` component（realm gate / qi_color gate）/ `skill::SkillSet`（P1 扩 `learned_recipes` 或挂独立 `RecipeUnlockState` resource）/ `botany::PlantKindRegistry` / `mineral` 物品 ID / 各流派 plan 注册的配方
- **出料**：`craft::CraftRegistry` 全局配方注册表 + `CraftSession` component（玩家进行中单任务）+ `CraftCompletedEvent` / `CraftFailedEvent` / `RecipeUnlockedEvent` + IPC schema 5 sample（payload 类型）
- **共享类型**：
  ```rust
  pub struct CraftRecipe {
      pub id: RecipeId,
      pub category: CraftCategory,  // AnqiCarrier / DuguPotion / TuikeSkin /
                                    // ZhenfaTrap / Tool / Misc
      pub materials: Vec<(ItemId, u32)>,
      pub qi_cost: f64,             // 自身真元投入（一次性，不是维持）
      pub time_ticks: u64,          // in-game 推进时间
      pub output: (ItemId, u32),
      pub requirements: CraftRequirements {
          realm_min: Option<Realm>,    // 不强制 gate，但如有则必须满足
          qi_color_min: Option<(ColorKind, f32)>,  // 如阴诡色 ≥ 5%
          skill_lv_min: Option<u8>,
      },
  }
  ```
- **跨仓库契约**：
  - server: `craft::*` 主实装（RecipeRegistry resource + 各流派 plan 调 `register_recipe()` 注入）
  - agent: `tiandao::craft_runtime`（首学配方 narration / 师承获取 / 顿悟解锁 / 出炉叙事）
  - client: `inventory::CraftTab` UI（inventory tab 集成）
- **worldview 锚点**：见头部
- **qi_physics 锚点**：手搓**消耗自身真元**走 `qi_physics::ledger::QiTransfer{from: caster, to: zone, amount: qi_cost}`（一次性扣除，不是维持）。**禁止 plan 内 `cultivation.qi_current -= cost` 直接扣**——守恒律红旗

---

## §0 设计轴心

- [ ] **inventory 标签集成（无方块）**：所有手搓在 inventory 内「手搓」tab，玩家任何时候 inventory 打开即可手搓。区别于 forge/alchemy（需要专属台子）。简化交互成本——worldview §九「信息差」+ §十「资源匮乏」的产物：玩家不应被"找台子"绑死，配方本身才是稀缺
- [ ] **单任务（无并发）**：同时只能 1 个手搓在跑，新任务必须等当前完成或主动取消。简化 UI + 玩家决策成本——worldview §十"减少倒腾"原则的设计化身
- [ ] **in-game 时间推进**：玩家在线时累积 tick，**下线暂停**（不像 botany 灵田那样自然推进）。worldview §九 玩家在场是基本要求，避免"挂机刷物资"破坏经济
- [ ] **配方解锁三渠道（无流派自动）**：worldview §九:843「信息比装备值钱」+ §六:654 顿悟。玩家不能因为"我修了毒蛊"就自动会蚀针——必须通过：
  - **残卷**（worldview §十 道伥/坍缩渊掉落）：随机掉落，多人交易/掠夺流通
  - **师承**（worldview §十一 NPC 散修教学）：付 qi/物品 / Renown / 跑腿任务换教学
  - **顿悟**（worldview §六:658 关键事件触发）：首次突破 / 濒死生还 / 杀比自己强的对手等触发选项弹窗
- [ ] **首版不实装**：磨损税（worldview §十:805-808 暂留 plan-economy-v1 配合）/ 装备加速（炼器加成 / 灵田加成留 v2）。**保持 v1 简洁**，验完核心循环再加
- [ ] **不替代 forge / alchemy / vanilla**：通用手搓限定为"轻度仪式化"——单步、有时间消耗、无火候/状态机判断。法器（forge 4 步）和丹药（alchemy 火候）保留专属 UI；vanilla 工作台保留原生 3x3 摆放

---

## §1 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** ✅ 2026-05-08 | 决策门：UI mockup 锁定（§2）/ 配方 schema 定稿（§3 + `server/src/schema/craft.rs` 5 sample）/ 各流派 plan 配方分发协议（`CraftRegistry::register` API + 命名空间约定）/ 三渠道解锁机制 design（§3 `UnlockSource` enum + `unlock.rs` 三函数）/ §5 六决策门收口（见 §5.5） | schema + UI mockup + 协议落 plan §2-§4 ✅ |
| **P1** ✅ 2026-05-08 | server `craft::*` 主模块（events / recipe / registry / session / unlock / mod）+ `CraftRegistry` resource + `CraftSession` component + tick 推进 + qi 扣除走 `qi_physics::ledger::Crafting`（reason variant 已扩）+ 5 个示例配方注册（蚀针 / 毒源煎汤 / 伪灵皮 / 真元诡雷 / 采药刀，覆盖 5/6 类）+ 87 单测（70 craft::* + 17 schema::craft） | `cargo test craft::` 87 passed / `cargo test` 全 2771 passed / `cargo clippy --all-targets -- -D warnings` 干净 / 守恒断言（qi_cost 走 ledger Crafting reason，参见 `start_craft_ledger_amount_matches_session_qi_paid` 测试） |
| **P2** ⬜ | client `inventory::CraftTab` UI（左 RecipeListPanel 含分组折叠 + 右 RecipeDetailPanel + 底 CurrentTaskBar 进度条）+ inventory tab 集成 + 配方解锁状态可视化（✅/🔒）+ 材料检查实时高亮（缺料红字） | WSLg 实跑 inventory 打开 → 切手搓 tab → 选配方 → 检查材料 → 开始 → 进度推进 → 出炉，全流程通顺 |
| **P3** ⬜ | agent narration（首学配方 / 师承获取 / 顿悟解锁 / 出炉叙事）+ 三渠道解锁机制实装（残卷 ItemUse 触发 / 师承 NPC dialog 触发 / 顿悟 InsightTrigger 触发）+ schema 5 sample 完整 + 各流派 plan 注册自家配方（接 dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1） | narration-eval 4 类叙事过古意检测 / 5 流派 plan 配方注册 PR 完成 |

---

## §2 UI Mockup（inventory 内「手搓」标签）

```
┌─────────────────────────────────────────────────────────────────────┐
│ 背包  装备  经脉  [手搓]  神识                              [关闭] │
├──────────────────────────┬──────────────────────────────────────────┤
│ 配方列表                  │ 选中：蚀针（凡铁档）                     │
│ ──────────                │ ──────────────────                       │
│ ▼ 暗器载体                │ 类别：暗器 / 毒蛊                        │
│   蚀针（凡铁）✅          │ 解锁来源：残卷 / 毒蛊师师承              │
│   蚀针（异兽骨）🔒        │                                          │
│   骨刺（凡铁）✅          │ 材料：                                   │
│ ▼ 煎汤 / 自蕴             │   凡铁飞针 ×3      [已有 12]   ✓        │
│   毒源煎汤（凡毒）✅      │   赤髓草 ×1        [已有 4]    ✓        │
│   毒源煎汤（灵毒）🔒      │   自身真元 ×8      [当前 45/80] ✓       │
│ ▼ 伪皮 / 替尸             │                                          │
│   伪灵皮（轻档）✅        │ 耗时：3 min（in-game 时间）             │
│ ▼ 阵法预埋件              │ 产出：蚀针 ×3                            │
│   真元诡雷（凡铁）🔒      │                                          │
│ ▼ 凡器                    │ ──────────────────                       │
│   采药刀（凡铁）✅        │ 当前任务：                               │
│   刮刀（凡铁）✅          │ 毒源煎汤×1  [▓▓▓░░░░░]  1:32 剩余       │
│   镰刀（凡铁）✅          │                                          │
│                           │ [  开始手搓  ]  [取消任务]              │
└──────────────────────────┴──────────────────────────────────────────┘
```

**UI 要点**：
- 左列表按 6 类分组（AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc），可折叠
- 已解锁配方 ✅，未解锁 🔒（点击显示解锁来源提示）
- 右详情：类别 + 解锁来源 + 材料清单（实时高亮缺料 ✓/✗）+ 耗时 + 产出 + 当前任务进度（单任务）
- 当有任务进行中，新选中配方 [开始手搓] 灰显，提示"已有任务在跑"
- inventory 关闭后任务暂停（in-game 时间不推进）；重新打开继续（玩家选择何时投入时间）

**worldview 张力**：worldview §十:805 "天道是个贪婪的钱庄"——手搓需要专注（必须开 inventory 看着），强化"修士不是工厂"的设定。

---

## §3 数据契约

```
server/src/craft/
├── mod.rs              — Plugin + register_recipes(global) +
│                        re-export CraftRegistry / CraftSession
├── recipe.rs           — CraftRecipe struct + RecipeRegistry resource +
│                        Category enum (AnqiCarrier/DuguPotion/TuikeSkin/
│                                       ZhenfaTrap/Tool/Misc) +
│                        CraftRequirements + ItemId 类型导入
├── session.rs          — CraftSession component (recipe_id, started_at_tick,
│                                                  remaining_ticks, owner)
│                        + tick_session 推进（仅在线推进）
│                        + start_craft / cancel_craft / finalize_craft fns
├── unlock.rs           — RecipeUnlockState resource (per-character bitmap) +
│                        unlock_via_scroll(item_id) /
│                        unlock_via_mentor(npc_id, recipe_id) /
│                        unlock_via_insight(insight_event)
└── events.rs           — CraftStartedEvent / CraftCompletedEvent /
                          CraftFailedEvent / RecipeUnlockedEvent

server/src/schema/craft.rs  — IPC schema 5 sample
                              (CraftStartReqV1 / CraftSessionStateV1 /
                               CraftOutcomeV1 / RecipeUnlockedV1 /
                               RecipeListV1)

agent/packages/schema/src/craft.ts  — TypeBox 双端
agent/packages/tiandao/src/craft_runtime.ts  — 4 类 narration
                                              (首学 / 师承 / 顿悟 / 出炉)

client/src/main/java/.../inventory/craft_tab/
├── CraftTabScreen.java         — 主标签 UI 容器（tab 集成 inventory）
├── RecipeListPanel.java        — 左列表（分组折叠 + 解锁状态）
├── RecipeDetailPanel.java      — 右详情（类别 / 来源 / 材料 / 耗时 / 产出）
├── CurrentTaskBar.java         — 底部当前任务进度条（单任务）
└── RecipeUnlockToastPlanner.java  — 解锁动画（残卷使用 / 师承 / 顿悟时弹通知）
```

**RecipeRegistry 注册**（各流派 plan 在自己 P0 阶段注册）：

```rust
// 例：plan-dugu-v2 的 dugu_v2::register_recipes 内
pub fn register_recipes(registry: &mut CraftRegistry) {
    registry.register(CraftRecipe {
        id: RecipeId::new("dugu.eclipse_needle.iron"),
        category: CraftCategory::DuguPotion,
        materials: vec![(ItemId::IronNeedle, 3), (ItemId::ChixuiHerb, 1)],
        qi_cost: 8.0,
        time_ticks: 3 * 60 * 20,  // 3 min in-game
        output: (ItemId::EclipseNeedleIron, 3),
        requirements: CraftRequirements {
            realm_min: None,  // 无 gate，符合 worldview §五:537
            qi_color_min: Some((ColorKind::YinGui, 0.05)),
            skill_lv_min: None,
        },
    });
    // ...
}
```

**配方解锁三渠道**（每个 RecipeId 标注解锁源）：

```rust
pub enum UnlockSource {
    Scroll { item_id: ItemId },          // 残卷使用即解锁
    Mentor { npc_archetype: String },    // 师承 NPC dialog 选项
    Insight { trigger: InsightTrigger }, // 顿悟事件首选项
}
// 每个 CraftRecipe 关联 unlock_sources: Vec<UnlockSource>，玩家命中任一即解锁
```

---

## §4 客户端新建资产

| 类别 | ID | 来源 | 优先级 | 备注 |
|---|---|---|---|---|
| UI | `CraftTabScreen` | 新建 Java | P2 | inventory tab 集成主 UI |
| UI | `RecipeListPanel` | 新建 | P2 | 左列表（分组折叠 + 解锁可视化）|
| UI | `RecipeDetailPanel` | 新建 | P2 | 右详情面板（材料/耗时/产出）|
| UI | `CurrentTaskBar` | 新建 | P2 | 底部单任务进度条 |
| UI | `RecipeUnlockToastPlanner` | 新建 | P3 | 解锁通知动画（残卷/师承/顿悟）|
| 音效 | `recipe_unlock_chime` | recipe 复用 vanilla | P3 | layers: `[{ sound: "block.bell.use", pitch: 1.4, volume: 0.5 }, { sound: "block.amethyst_block.chime", pitch: 1.0, volume: 0.3, delay_ticks: 3 }]`（清脆开悟感）|
| 音效 | `craft_complete` | recipe 复用 vanilla | P3 | layers: `[{ sound: "block.smithing_table.use", pitch: 1.0, volume: 0.5 }]`（出炉声）|

**无新建动画 / 粒子** ——通用手搓不需要专属视觉，玩家行为是"打开 inventory + 等待"。各流派注册的配方如需"出炉特效"由流派 plan 自己挂（如 dugu 蚀针出炉自带 DUGU_DARK_GREEN_MIST，复用 plan-dugu-v2 资产）。

---

## §4.5 P1 测试矩阵（饱和化测试）

下限 **30 单测**：

| 模块 | 测试组 | 下限 |
|---|---|---|
| `register_recipe` | 注册 + 重复 ID 拒绝 + 类别枚举完整 + 解锁源枚举完整 | 5 |
| `start_craft` | 材料足够 happy / 缺料 reject / qi 不足 reject / 已有任务 reject / 配方未解锁 reject / 境界要求未满足 reject | 8 |
| `tick_session` | in-game 推进 + 下线暂停 + 完成触发 finalize + qi 走 ledger 守恒断言 | 5 |
| `cancel_craft` | 取消返还材料 ×0.7（70% 退款，30% 损耗惩罚）+ qi 不退 + 立即解除 session | 4 |
| `unlock_via_scroll` | 残卷使用解锁 + 已解锁 noop + 写 RecipeUnlockedEvent | 3 |
| `unlock_via_mentor` | NPC dialog 解锁 + Renown / qi 成本扣除 + 多重师承可选 | 3 |
| `unlock_via_insight` | InsightTrigger 触发选项弹窗 + 玩家选定后解锁 + 永久不可重选 | 2 |

**P1 验收**：`grep -rcE '#\[test\]' server/src/craft/` ≥ 30。守恒断言：所有 `qi_cost` 必须走 `qi_physics::ledger::QiTransfer{from: caster, to: zone}`，不允许 `cultivation.qi_current -= cost` 直接扣。

---

## §5 开放问题 / 决策门（P0 启动前必须收口）

### #1 配方分类 6 类够吗？

当前：AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc

- **A**：保留 6 类
- **B**：补 BaomaiSpecial（体修自损增益类）/ Spirit Eye Equipment（灵眼勘探类）

**默认推 A** ——首版简洁，新流派/系统按需补类别（plan vN+1 联动）

### #2 配方界面排序

- **A**：按类别分组 + 类别内按字母（默认推荐）
- **B**：按解锁顺序（最近解锁置顶）
- **C**：按使用频率（玩家学习曲线友好）

**默认推 A** —— 最稳定，玩家心智模型清晰

### #3 任务取消的材料返还比例

- **A**：100% 返还（无惩罚，玩家随意取消）
- **B**：70% 返还 30% 损耗（推荐——避免"反复取消刷材料")
- **C**：0% 返还（开始即沉没成本）

**默认推 B** —— worldview §十"零拷贝生存"思想精简版，避免取消滥用

### #4 玩家死亡时进行中的任务怎么处理？

- **A**：清空（重生后无任务，符合 worldview §十二 死亡是损失）
- **B**：保留进度（重生后继续，玩家友好）
- **C**：保留材料但 session 重置（材料返背包，需重新开始）

**默认推 A** —— 死亡惩罚更彻底，符合 worldview"死亡是学费"

### #5 跟 vanilla 工作台的边界

有些物品（如凡铁工具）vanilla 也能合成，谁优先？

- **A**：手搓 tab 不收录 vanilla 物品（只走 vanilla 工作台 3x3）
- **B**：手搓 tab 收录但标 "vanilla 可代替"（玩家自选）
- **C**：手搓 tab 完全替代 vanilla 工作台

**默认推 A** —— 严守边界（"修仙物 vs 凡物"），vanilla 留给凡物。但 plan-tools-v1 的凡器（采药刀/刮刀/镰刀）属凡物但带"修仙采集功能"，可破例收录手搓 tab

### #6 配方按境界 / 颜色等门槛是软还是硬

- **A**：硬 gate（不满足即不显示在列表）
- **B**：软 gate（显示但 [开始手搓] 灰显，提示原因）

**默认推 B** —— 玩家可见路径，符合 worldview §五:537「流派由组合涌现」精神

---

## §5.5 决策门收口（2026-05-08，P0 验收）

P0 决策门六条均按 plan 内"默认推 X"采纳，落代码处如下：

| # | 选项 | 落地处 |
|---|---|---|
| 1 配方分类 6 类 | A：保留 6 类 | `craft::CraftCategory`（AnqiCarrier / DuguPotion / TuikeSkin / ZhenfaTrap / Tool / Misc）+ `CraftCategory::ALL` 固定排序 |
| 2 配方界面排序 | A：按类别分组 + 类别内字母 | `CraftRegistry::grouped_for_ui()` 按 ALL 顺序 + `RecipeId::cmp` 字母升序 |
| 3 取消返还比例 | B：70% 返还 30% 损耗 | `craft::session::CANCEL_REFUND_RATIO = 0.7` + `cancel_craft` floor 计算 |
| 4 死亡进行中任务 | A：清空 | `CraftFailureReason::PlayerDied` reason 由 death-lifecycle 调用 `cancel_craft` 清 session |
| 5 vanilla 边界 | A：手搓 tab 不收 vanilla，凡器破例 | 5 示例之一 `craft.example.herb_knife.iron`（CraftCategory::Tool, qi_cost = 0） |
| 6 境界 / 颜色门槛 | B：软 gate（UI 灰显） | `CraftRequirements` 字段 + `start_craft` 内服务端硬校验防作弊 |

---

## §6 进度日志

- **2026-05-08** P0+P1 落地（P2/P3 暂未启动，plan 保留 active）：
  - **P0 决策门收口**：六门均按默认推（见 §5.5）
  - **P1 server 主体**：`server/src/craft/{events,recipe,registry,session,unlock,mod}.rs` 完整建立 + `server/src/schema/craft.rs` IPC 5 sample
  - **5 示例配方**：`craft.example.{eclipse_needle.iron, poison_decoction.fan, fake_skin.light, zhenfa_trap.iron, herb_knife.iron}`，覆盖 5/6 类（Misc 兜底不举例）
  - **守恒律**：`qi_physics::ledger::QiTransferReason::Crafting` variant 新增；`start_craft` 走 ledger-first（`WorldQiAccount::transfer` from=player to=zone amount=qi_cost），随后把 `cultivation.qi_current` 同步扣减（state view 镜像 ledger，避免双 source-of-truth）。**调用方必须先把 `cultivation.qi_current` sync 到 `ledger.balance(player)`** 才能起手手搓——`start_craft` 内**不**做 ad-hoc set_balance 注入（避免 inflate ledger 总数破坏全局守恒）；当 ledger 与 state view 失同步时返回 `StartCraftError::LedgerOutOfSync { player_balance, cultivation_qi_current, required }` fail-fast，待 `qi_physics::sync_player_qi_to_ledger` system 接入后由 ECS hook 自动同步。`ledger_total_conservation_after_start_craft` / `start_craft_with_synced_ledger_does_not_inflate_player_balance` / `start_craft_rejects_when_ledger_out_of_sync` 测试 pin 守恒律
  - **测试**：`cargo test craft::` 87 passed（70 craft::* + 17 schema::craft），全栈 `cargo test` 2771 passed，`cargo clippy --all-targets -- -D warnings` 干净
  - **接入留口**：P2 client UI 未动；P3 三渠道 hook（残卷 ItemUse / NPC dialog / BreakthroughEvent / 顿悟选项菜单）函数已暴露在 `craft::unlock` 但**未挂监听**——等流派 plan vN+1 各自接入时挂载
- **2026-05-08** 升 active。实地核验 `inventory::ItemInstance` ✅ / `cultivation::QiColor`（独立 component）✅ / `qi_physics::ledger` ✅ / `botany::PlantKindRegistry`（实名修正）✅ / `alchemy::RecipeRegistry`（命名空间不冲突）✅ / `craft` 顶层模块未建（符合 plan 设计）/ `SkillSet` 缺 `learned_recipes` 字段（P1 扩或挂 `RecipeUnlockState` resource）。"现状对齐"段落锁定差异；接入面 Checklist 进料行已修正（`PlantKindRegistry` + `QiColor` 独立 component）。
- **2026-05-06** 骨架立项。源自 plan-dugu-v2 起草过程中发现"蚀针 / 自蕴煎汤的手搓 UI 缺失"问题，上钻发现是通用问题（多个流派都有"轻度仪式化"合成需求），最终决定立通用 plan 而非各流派各自补 UI。
  - 设计轴心：inventory 标签集成（无方块）+ 单任务（无并发）+ in-game 时间推进（在线累积下线暂停）+ 三渠道解锁（残卷/师承/顿悟）+ 首版不实装磨损税和装备加速
  - 配方分类 6 类（AnqiCarrier/DuguPotion/TuikeSkin/ZhenfaTrap/Tool/Misc）
  - 跟 forge（4 步状态机）/ alchemy（火候模式）/ vanilla（3x3 摆放）边界明确
  - worldview 锚点对齐：§六:654 顿悟解锁路径 + §九:843 信息比装备值钱（配方=信息）+ §十 残卷掉落 + §十一 NPC 师承
  - qi_physics 锚点：qi_cost 走 ledger，不直接扣，守恒律红旗规避
  - SkillRegistry / inventory tab / HUD / Casting 全部底盘复用，无新建框架
  - 反向被依赖：plan-dugu-v2 / plan-tuike-v2 / plan-zhenfa-v2 / plan-tools-v1 各自 vN+1 注册自家配方
  - 待补：plan-economy-v1 配合实装磨损税（v2）/ 装备加速（v2）/ plan-narrative-v1 配合 4 类 narration template

---

## Finish Evidence（待填）

迁入 `finished_plans/` 前必须填：
- **落地清单**：`server/src/craft/` 主模块 + `client/src/.../inventory/craft_tab/` UI + `agent/.../craft_runtime.ts`
- **关键 commit**：P0/P1/P2/P3 各自 hash + 日期 + 一句话
- **测试结果**：`cargo test craft::` 数量 / 5 个示例配方完整流程实测 / WSLg inventory tab 实测
- **跨仓库核验**：server CraftRegistry / agent narration runtime / client CraftTab UI / 5 流派 plan 配方注册 PR
- **遗留 / 后续**：磨损税 v2 / 装备加速 v2 / 跨流派共享配方（如普适毒草煎汤可被非毒蛊师学）/ 多任务并发 v3
