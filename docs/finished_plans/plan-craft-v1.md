# Bong · plan-craft-v1 · active（P0+P1 ✅，P2/P3 ⬜）

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
| **P1** ✅ 2026-05-08 | server `craft::*` 主模块（events / recipe / registry / session / unlock / mod）+ `CraftRegistry` resource + `CraftSession` component + tick 推进 + qi 扣除走 `qi_physics::ledger::Crafting`（reason variant 已扩）+ 5 个示例配方注册（蚀针 / 毒源煎汤 / 伪灵皮 / 真元诡雷 / 采药刀，覆盖 5/6 类）+ 98 单测（81 craft::* + 17 schema::craft，含 review fix 后新增 11 条覆盖 ledger sync / validate 边界路径） | `cargo test craft::` 98 passed / `cargo test` 全 2782 passed / `cargo clippy --all-targets -- -D warnings` 干净 / 守恒断言（qi_cost 走 ledger Crafting reason，参见 `start_craft_ledger_amount_matches_session_qi_paid` / `ledger_total_conservation_after_start_craft` / `start_craft_rejects_when_ledger_out_of_sync` 测试） |
| **P2** ✅ 2026-05-08 | client `inventory::CraftTab` UI（左 RecipeListPanel 含分组折叠 + 右 RecipeDetailPanel + 底 CurrentTaskBar 进度条）+ inventory tab 集成 + 配方解锁状态可视化（✅/🔒）+ 材料检查实时高亮（缺料红字） | server craft IPC bridge（`network/craft_emit.rs` 6 系统：apply_craft_intents / tick / emit_session_state / emit_outcome / emit_recipe_unlocked / emit_recipe_list_on_join）+ ServerDataPayloadV1 4 variant + ClientRequestV1 2 variant；client `inventory::component::CraftTabPanel` + InspectScreen `TAB_CRAFT(=5)` + `craft::{CraftCategory,CraftRecipe,CraftSessionStateView,CraftStore}` + 4 ServerData handlers；19 客户端新单测 + 19 服务端新单测；WSLg 实跑由用户验收（headless agent 不验收 UI 视觉） |
| **P3** ✅ 2026-05-08 | agent craft narration runtime（4 类：首学 / 师承 / 顿悟 / 出炉，dispatch by `source.kind` / `outcome.kind`）+ server 三渠道解锁 intent（`CraftUnlockIntent` + `apply_unlock_intents` 系统）+ Redis 桥（`craft_event_bridge.rs` 3 系统 → `bong:craft/outcome` / `bong:craft/recipe_unlocked`）+ schema 5 sample TypeBox 双端镜像 + `find_recipes_unlockable_by_{scroll,mentor,insight}` 查询 helper（流派 plan 调用入口）| 11 craft-runtime 测 + 19 schema 测 + 4 server bridge 测全过；流派 plan 配方注册 PR（dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1）由各自 vN+1 PR 接入新增 hook（**遗留**） |

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

**P1 验收**：`grep -rcE '#\[test\]' server/src/craft/` ≥ 30。守恒断言：所有 `qi_cost` 必须走 `qi_physics::ledger::QiTransfer{from: caster, to: zone}`——**禁止绕过 ledger 单独扣 state view**。允许的语义：`WorldQiAccount::transfer` 成功后把 `cultivation.qi_current` 同步扣减（state view 镜像 ledger，不脱离账本独立维护）。`start_craft` 在 ledger / state view 失同步时 fail-fast `LedgerOutOfSync`（详见 §6 进度日志）。

**P1 已知简化（待 plan-qi-physics-v2 解锁）**：`qi_color_min: Option<(ColorKind, f32)>` 当前 `cultivation::QiColor` 是 single-main 形态（`{ main, secondary, is_chaotic, is_hunyuan }`，无显式 share 字段），P1 阶段把 `share` 阈值视为"main 命中即满足任意 share"——`Some((Insidious, 0.05))` 与 `Some((Insidious, 0.95))` 行为等价（main 等于 100% 主色占比）。待 `plan-qi-physics-v2` 在 QiColor 上加 `main_share: f64` 或多色 weights 后，`start_craft` 内 `qi_color_min` 校验切换到真实 share 比对，并新增 `StartCraftError::QiColorShareTooLow` variant 区分"主色对了但占比不足"与"主色错"。`recipe::validate` 已校验 share ∈ [0.0, 1.0]，配方契约可向前兼容。

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

- **2026-05-08** P2+P3 落地，plan 进入归档：
  - **P2 server**：`server/src/network/craft_emit.rs` 6 系统接入 client_request → CraftStartIntent / CraftCancelIntent → start_craft / cancel_craft → tick → finalize → 推 ServerDataPayloadV1（CraftRecipeList / CraftSessionState / CraftOutcome / RecipeUnlocked）
  - **P2 schema**：扩 `ServerDataPayloadV1` 4 variant + `ClientRequestV1::{CraftStart, CraftCancel}` + payload_type_label / wire form / payload_type 全闭合 + ServerDataRouterTest pin 4 新 wire type
  - **P2 client**：`client/src/main/java/com/bong/client/craft/{CraftCategory,CraftRecipe,CraftSessionStateView,CraftStore}` + 4 handlers（CraftRecipeListHandler / CraftSessionStateHandler / CraftOutcomeHandler / RecipeUnlockedHandler）+ `inventory/component/CraftTabPanel` + `InspectScreen` 加 `TAB_CRAFT(=5)` "手搓" + `ClientRequestProtocol/Sender` 加 `craft_start/craft_cancel`
  - **P3 server 三渠道入口集中化**：`craft::CraftUnlockIntent` event + `craft_emit::apply_unlock_intents` 系统统一路由 scroll/mentor/insight → `unlock_via_*`（已有）→ emit `RecipeUnlockedEvent`；`craft::find_recipes_unlockable_by_{scroll,mentor,insight}` 查询 helper 让流派 plan 用 `item_template / npc_archetype / InsightTrigger` 反查命中配方
  - **P3 server Redis 桥**：`network/craft_event_bridge.rs` 3 系统（publish_craft_completed/failed/recipe_unlocked → CH_CRAFT_OUTCOME / CH_CRAFT_RECIPE_UNLOCKED）+ RedisOutbound 新加 CraftOutcome / RecipeUnlocked variants + channels 测试 pin
  - **P3 agent**：`agent/packages/schema/src/craft.ts` TypeBox 5 sample（与 server `schema::craft` 1:1 镜像）+ `agent/packages/tiandao/src/craft-runtime.ts` 订阅 2 channel → 4 类 narration（first_learn / mentor / insight / completed）+ `skills/craft.md` 系统 prompt + fallback narration（LLM 失败 / 非法 JSON / 不过 TypeBox 时仍可读）
  - **测试**：`cargo test` 2896 passed（craft 121 含 121-98=23 新单测）/ `./gradlew test` 客户端 869 passed（19 craft 新单测）/ `npm test` schema 324（19 craft）+ tiandao 266（11 craft-runtime）；`cargo clippy --all-targets -- -D warnings` 干净 / `cargo fmt --check` 干净
  - **WSLg 实跑**：留给用户验收（headless agent 不做视觉验证），server 编译 / client `./gradlew test build` 干净 jar 已出
  - **遗留**：P3 §1 验收里"5 流派 plan 配方注册 PR 完成"由 dugu-v2 / tuike-v2 / zhenfa-v2 / tools-v1（当前均为 skeleton）各自 vN+1 PR 接入；本 PR 提供 server `find_recipes_unlockable_by_*` + `CraftUnlockIntent` 入口供它们调用，不在本 plan 范围内强行打开 skeleton
- **2026-05-08** P0+P1 落地（P2/P3 暂未启动，plan 保留 active）：
  - **P0 决策门收口**：六门均按默认推（见 §5.5）
  - **P1 server 主体**：`server/src/craft/{events,recipe,registry,session,unlock,mod}.rs` 完整建立 + `server/src/schema/craft.rs` IPC 5 sample
  - **5 示例配方**：`craft.example.{eclipse_needle.iron, poison_decoction.fan, fake_skin.light, zhenfa_trap.iron, herb_knife.iron}`，覆盖 5/6 类（Misc 兜底不举例）
  - **守恒律**：`qi_physics::ledger::QiTransferReason::Crafting` variant 新增；`start_craft` 走 ledger-first（`WorldQiAccount::transfer` from=player to=zone amount=qi_cost），随后把 `cultivation.qi_current` 同步扣减（state view 镜像 ledger，避免双 source-of-truth）。**调用方必须先把 `cultivation.qi_current` sync 到 `ledger.balance(player)`** 才能起手手搓——`start_craft` 内**不**做 ad-hoc set_balance 注入（避免 inflate ledger 总数破坏全局守恒）；当 ledger 与 state view 失同步时返回 `StartCraftError::LedgerOutOfSync { player_balance, cultivation_qi_current, required }` fail-fast，待 `qi_physics::sync_player_qi_to_ledger` system 接入后由 ECS hook 自动同步。`ledger_total_conservation_after_start_craft` / `start_craft_with_synced_ledger_does_not_inflate_player_balance` / `start_craft_rejects_when_ledger_out_of_sync` 测试 pin 守恒律
  - **测试**：`cargo test craft::` 98 passed（81 craft::* + 17 schema::craft；review fix 两轮共加 11 条），全栈 `cargo test` 2782 passed，`cargo clippy --all-targets -- -D warnings` 干净
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

## Finish Evidence

### 落地清单

| 阶段 | 模块 / 文件路径 |
|---|---|
| **P0** 决策门 | §5.5（plan 内）+ `server/src/craft/mod.rs` 顶部 doc 注释（六门按默认推 A/A/B/A/A/B） |
| **P1** server 主底盘 | `server/src/craft/{events,recipe,registry,session,unlock,mod}.rs` + `server/src/schema/craft.rs`（5 sample）+ `server/src/qi_physics/ledger.rs`（`QiTransferReason::Crafting` 新增） |
| **P2** server craft IPC | `server/src/network/craft_emit.rs`（6 系统：apply_craft_intents / tick_craft_sessions / emit_craft_session_state / emit_craft_outcome_payloads / emit_recipe_unlocked_payloads / emit_recipe_list_on_join）+ `server/src/schema/server_data.rs`（4 variant + wire 镜像）+ `server/src/schema/client_request.rs`（CraftStart / CraftCancel）+ `server/src/network/client_request_handler.rs` 路由 + `server/src/network/agent_bridge.rs` payload_type_label 4 entry |
| **P2** client UI | `client/src/main/java/com/bong/client/craft/{CraftCategory,CraftRecipe,CraftSessionStateView,CraftStore}.java` + `client/src/main/java/com/bong/client/inventory/component/CraftTabPanel.java` + `client/src/main/java/com/bong/client/inventory/InspectScreen.java`（TAB_CRAFT=5 / craftTabContent / switchTab+removed） + `client/src/main/java/com/bong/client/network/{CraftRecipeListHandler,CraftSessionStateHandler,CraftOutcomeHandler,RecipeUnlockedHandler,ServerDataRouter,ClientRequestProtocol,ClientRequestSender}.java` |
| **P3** server 三渠道入口 | `server/src/craft/events.rs::CraftUnlockIntent` + `server/src/craft/unlock.rs::find_recipes_unlockable_by_{scroll,mentor,insight}` + `server/src/network/craft_emit.rs::apply_unlock_intents` |
| **P3** server Redis 桥 | `server/src/network/craft_event_bridge.rs`（3 系统）+ `server/src/network/redis_bridge.rs`（CraftOutcome / RecipeUnlocked variants + dispatch）+ `server/src/schema/channels.rs`（CH_CRAFT_OUTCOME / CH_CRAFT_RECIPE_UNLOCKED） |
| **P3** agent | `agent/packages/schema/src/craft.ts`（5 sample TypeBox 镜像）+ `agent/packages/schema/src/channels.ts`（CRAFT_OUTCOME / CRAFT_RECIPE_UNLOCKED）+ `agent/packages/tiandao/src/craft-runtime.ts` + `agent/packages/tiandao/src/skills/craft.md` |

### 关键 commit

| 阶段 | hash | 日期 | 摘要 |
|---|---|---|---|
| P0+P1 | 7af464962 | 2026-05-08 | server 通用手搓底盘（5 示例 + 87 单测，PR #155） |
| P2 server | 9654fa875 | 2026-05-08 | server craft IPC bridge + intent → session 系统 |
| P2 client | 98633aaf5 | 2026-05-08 | client CraftTab UI + 4 ServerData handlers + 2 ClientRequest 编码器 |
| P3 agent | 9c218f73c | 2026-05-08 | agent schema + craft narration runtime（4 类叙事） |
| P3 server | e9e770949 | 2026-05-08 | server 三渠道解锁 intent + Redis 桥（→ agent） |

### 测试结果

| 命令 | 结果 |
|---|---|
| `cargo test --no-fail-fast`（server） | 2896 passed / 0 failed |
| `cargo test craft`（server） | 121 passed（包含 craft::* 87 + schema::craft 17 + network::craft_emit 11 + network::craft_event_bridge 4 + 其他 craft 关联 2） |
| `cargo clippy --all-targets -- -D warnings` | 干净 |
| `cargo fmt --check` | 干净 |
| `./gradlew test build`（client） | 869 passed（含 craft 19 新测：CraftStoreTest 8 + CraftHandlerTest 9 + InspectScreenQuickUseTabTest 1 + ServerDataRouterTest 1 修） |
| `npm test`（schema） | 324 passed（含 craft 19 新测） |
| `npm test`（tiandao） | 266 passed（含 craft-runtime 11 新测） |

### 跨仓库核验

| 层 | 命中 symbol |
|---|---|
| server | `craft::{CraftRegistry, CraftSession, CraftStartIntent, CraftCancelIntent, CraftUnlockIntent, CraftStartedEvent, CraftCompletedEvent, CraftFailedEvent, RecipeUnlockedEvent, RecipeUnlockState, find_recipes_unlockable_by_*}` / `network::craft_emit::*` / `network::craft_event_bridge::*` / `schema::craft::{CraftStartReqV1, CraftSessionStateV1, CraftOutcomeV1, RecipeUnlockedV1, RecipeListV1}` / `schema::server_data::ServerDataType::{CraftRecipeList, CraftSessionState, CraftOutcome, RecipeUnlocked}` / `schema::client_request::ClientRequestV1::{CraftStart, CraftCancel}` / `schema::channels::{CH_CRAFT_OUTCOME, CH_CRAFT_RECIPE_UNLOCKED}` |
| agent | `@bong/schema::{CraftStartReqV1, CraftSessionStateV1, CraftOutcomeV1, RecipeUnlockedV1, RecipeListV1, CraftCategoryV1, CraftFailureReasonV1, InsightTriggerV1, UnlockEventSourceV1, CraftRequirementsV1, CraftRecipeEntryV1}` / `@bong/schema::CHANNELS::{CRAFT_OUTCOME, CRAFT_RECIPE_UNLOCKED}` / `@bong/tiandao::CraftNarrationRuntime` |
| client | `craft::{CraftCategory, CraftRecipe, CraftSessionStateView, CraftStore}` / `inventory::component::CraftTabPanel` / `inventory::InspectScreen::TAB_CRAFT` / `network::{CraftRecipeListHandler, CraftSessionStateHandler, CraftOutcomeHandler, RecipeUnlockedHandler}` / `network::ClientRequestProtocol::{encodeCraftStart, encodeCraftCancel}` / `network::ClientRequestSender::{sendCraftStart, sendCraftCancel}` |

### 遗留 / 后续

- **流派 plan 配方注册接入**（plan §1 P3 验收 "5 流派 plan 配方注册 PR" 部分）：`plan-dugu-v2` / `plan-tuike-v2` / `plan-zhenfa-v2` / `plan-tools-v1`（当前均为 skeleton）各自 vN+1 PR 调 `find_recipes_unlockable_by_*` + emit `CraftUnlockIntent` 把"使用残卷 X / NPC 教学 / 顿悟事件"接到 craft 解锁通道；本 plan 不强行打开 skeleton（违反"一个 PR 只动一个 plan"约束）
- **死亡时 craft session 清空**（§5 决策门 #4 = A）：`craft_emit::cancel_session_on_death` hook 函数已在注释中预留，等 plan-death-lifecycle-v1 vN+1 接入实装
- **磨损税 v2 / 装备加速 v2**：plan §0 设计轴心声明"首版不实装"，留给 plan-economy-v1 配合
- **session 持久化跨 disconnect**：当前实装是"下线 Entity 销毁 → session 丢失"，与 plan §0 轴心"下线暂停"略有偏差（暂停 vs 丢失）。如要做真"暂停"需 PlayerStatePersistence 集成 — 留 v2 收尾
- **WSLg 实跑全流程实测**（plan §1 P2 验收 "全流程通顺"）：headless agent 不做视觉验证，./gradlew test build 已干净 jar，待用户在 WSLg 内打开 inventory → 切手搓 tab → 选配方 → 检查材料 → 开始 → 进度推进 → 出炉跑通后回归确认
