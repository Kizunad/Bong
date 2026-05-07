# Bong · plan-hotbar-modify-v2 · 骨架

InspectScreen 工作台精简 + **「功法」独立 tab 立项** + **`SkillConfig` 通用底盘**。承接 `plan-hotbar-modify-v1` ✅ finished（验收 2026-04-29）—— v2 砍冗余 + 加新需求：

- **删「战斗·修炼」tab** —— v1 §4.1 五区联动工作台 ① 列表"技艺三行（不可拖纯展示）" + ③ 经脉缩略图 + ④ 修士状态 三处对其他 tab / `BottomInfoBar` 重复展示，整 tab 砍掉
- **新增「功法」独立 tab** —— 功法太多需要专属空间（搜索 + 滚动 + 详情 + 经脉联动 + 配置）
- **跨 tab 联动**：「修仙」tab 完整 20 经脉视图加"按「功法」tab 当前选中 / hover 功法被动高亮 `required_meridians`"，B 方案双视图同步
- **`SkillConfig` 通用底盘**（起源 zhenmai-v2 ⑤ 绝脉断链"InspectScreen 该招槽预选 meridian + backfire_kind"机制）—— 详情卡右下齿轮 → 翻面到配置视图，按招式专属字段（dropdown / 滑条等）配置；服务端新 `SkillConfigStore` + `bong:skill/config_intent` payload + `Casting.skill_config` 快照

**世界观锚点**：无新增物理 / 经济概念，纯 UI 重构 + 招式可配置化机制；接 worldview §1 经脉拓扑（功法详情卡显示 `required_meridians`）+ §三:78 化虚天道针对（zhenmai-v2 ⑤ SkillConfig 配置永久 SEVERED 经脉的伦理代价仍走 zhenmai-v2 plan）

**library 锚点**：无（纯客户端 UI / 服务端 store）

**前置依赖**：
- `plan-hotbar-modify-v1` ✅ finished → 1-9 SkillBar `[hotbarStrip 34px]` + F1-F9 quickUse `[quickUseStrip 34px]` + `SkillBarStore` + `SkillBarBind` payload + `markBoundSlots` 回路视觉接口
- `plan-skill-v1` ✅ → `SkillSet.skill_lv` 单招熟练度数据源（功法行 skill exp 进度条）
- `plan-cultivation-canonical-align-v1` ✅ → `MeridianSystem` 20 经脉拓扑（详情卡 / 经脉剪影联动数据源）
- `plan-HUD-v1` ✅ + `plan-input-binding-v1` ✅
- `plan-multi-style-v1` ✅ → 已学功法集合数据源（`SkillSetStore`）

**反向被依赖**：
- `plan-zhenmai-v2` 🆕 active ⑤ 绝脉断链 → 首个使用 `SkillConfig` 的招（meridian_id ∈ 20 经脉 / backfire_kind ∈ 4 类）
- `plan-anqi-v2` 🆕 active 详情卡内嵌入 `SkillConfig` 配置（如 vN+1 出现"载体档位预选 / 容器活跃槽预选"等变种）
- `plan-yidao-v1` 🆕 active 详情卡 / `SkillConfig`（如 ④ 续命术配置"优先恢复境界 vs qi_max"等变种）
- `plan-baomai-v3` 🆕 active 详情卡 / `SkillConfig`（如 ⑤ 散功配置"5s 内 flow_rate 倍率档"等变种）
- 后续所有 v2+ 流派 plan 详情卡 / 配置面板复用同一底盘

---

## 接入面 Checklist

- **进料**：`SkillSetStore.snapshot()`（已学功法 + skill_lv）/ `MeridianSystem`（20 经脉健康度 + SEVERED 状态）/ `SkillBarStore`（反查已绑定 slot）/ `Casting`（cast 期间禁用配置面板）/ `Cultivation { realm }`（境界锁定态判定）
- **出料**：`SkillBarBind`（v1 已实装，复用）/ `SkillConfigIntent`（**新 payload**：`bong:skill/config_intent { skill_id, config: HashMap<String, Value> }`）/ `SkillConfigSnapshot`（**新 payload**：server → client 同步当前配置）
- **共享类型**：`SkillConfig`（`HashMap<String, Value>`，按 skill_id schema 校验）/ `SkillConfigStore`（**新 server resource**：`HashMap<(player_id, skill_id), SkillConfig>` 持久化）/ `Casting.skill_config: Option<SkillConfig>`（**v2 加字段**，cast 开始时快照，避免 cast 期间配置被改）
- **跨仓库契约**：
  - server: `combat::Casting.skill_config` 字段 + `skill::config::SkillConfigStore` resource + `network::handle_config_intent` handler
  - schema: `agent/packages/schema/src/skill_config.ts` 新建 TypeBox 定义 + `samples/skill_config_intent.json` 双端校验
  - client: 删 `CombatTrainingPanel.java`（v1 「战斗·修炼」tab 实装代码）+ 新建 `TechniquesTabPanel.java`（「功法」tab）+ `SkillConfigFloatingWindow.java`（owo-lib Positioning.absolute 浮窗 + drag + close，详见 §1.4）+ `SkillConfigPanelManager.java`（singleton 多实例互斥 + cast 期间强制 close）+ `MeridianMiniSilhouette.java`（左下经脉剪影组件）
  - 「修仙」tab 加 `bindActiveTechniqueHighlight(SkillSetStore.observable())` —— 跨 tab 联动 client side 状态机
- **worldview 锚点**：见头部
- **qi_physics 锚点**：无（纯 UI 与 store；`SkillConfig` 只承载语义字段，不写物理公式）

---

## §1 「功法」tab 布局规范

### 1.1 整体布局

```
┌─ mainPanel · 「功法」tab (新第 4 个 tab) ─────────────────────────────┐
│ ┌── 左侧 ~40% verticalFlow ────┐ ┌── 右侧 ~60% verticalFlow ────────┐│
│ │ ┌──────────────────────────┐ │ │ 功法详情卡 (TechniqueDetailCard)  ││
│ │ │ 🔍 [搜索框 .........]   │ │ │   ┌──────────────┐              ││
│ │ └──────────────────────────┘ │ │   │ [崩] 崩拳    │ 黄阶 lv 12  ││
│ │ ┌──────────────────────────┐ │ │   └──────────────┘              ││
│ │ │ ↑ 功法列表 (verticalFlow │ │ │   描述 / 需求 / 招式数值        ││
│ │ │  + scrollbar)            │ │ │   ▸ 真元 30 / 360               ││
│ │ │ [崩] 崩拳 ▰▰▰▱▱ 62%   │ │ │   ▸ cast 8t · cd 60t            ││
│ │ │ [靠] 贴山靠 ▰▰▱▱▱ 41% │ │ │   ▸ 射程近身 1.8m              ││
│ │ │ [步] 血崩步 🔒境界锁    │ │ │   ▸ 依赖经脉：心包经 ✅ / 督脉⚠ ││
│ │ │ [逆] 逆脉护体 ⊙绑3 28% │ │ │   ▸ 已绑定 · 左侧槽 1            ││
│ │ │ ...（滚动）              │ │ │                                 ││
│ │ │ ↓                        │ │ │                                 ││
│ │ ├──────────────────────────┤ │ │                                 ││
│ │ │ 经脉剪影 (MiniSilhouette)│ │ │                            ⚙   ││
│ │ │ [人体剪影]               │ │ │                       (齿轮按钮)││
│ │ │ ▰LI ▰TE 高亮            │ │ │                                 ││
│ │ │   按选中功法              │ │ │                                 ││
│ │ │   required_meridians     │ │ │                                 ││
│ │ └──────────────────────────┘ │ │                                 ││
│ └─────────────────────────────┘ └─────────────────────────────────┘│
└────────────────────────────────────────────────────────────────────┘
```

### 1.2 区域职责

| 区 | 内容 | 数据源 | 新增组件 |
|---|---|---|---|
| **🔍 搜索框** | 模糊匹配 skill_id / 显示名 / 别称；输入即时过滤列表 | client 本地，不走网络 | `TechniqueSearchBar`（基于 owo-lib `TextBoxComponent`） |
| **功法列表** | 行 = `[图标] 名称 lv X · 阶 ▰▰▱▱ XP%`；锁定态灰显 + reject drop；toggle 类标 ⊙绑N | `SkillSetStore.snapshot()` + `SkillBarStore` 反查 | `TechniqueRowComponent`（v1 已有，复用 + 加 skill exp 进度条） |
| **经脉剪影（左下）** | 简化人体剪影 + 高亮"所选功法 `required_meridians`"对应经脉位置；legend 显示健康度 / SEVERED 状态 | `MeridianSystem` + 详情卡 selectedTechniqueId | `MeridianMiniSilhouette`（v1 §4.2 ③ `MeridianChannel` 重命名 + 单独 file） |
| **详情卡（右）** | 选中功法完整信息：描述 / 真元消耗 / cast / cd / 射程 / 依赖经脉文字（点击经脉名跳「修仙」tab）/ 已绑定 slot 反查 | `TechniquesSnapshotV1` + `SkillBarStore` | `TechniqueDetailCard`（v1 已有，复用） |
| **⚙ 齿轮（详情卡右下）** | 点击 → 弹出 **floating window** 浮窗（详情卡保留不变，浮窗叠加在上方）；浮窗按招式 schema 渲染 dropdown / 滑条 / radio + 标题栏可拖拽 + 右上 X 关闭 | `SkillConfigStore[skill_id]` | `SkillConfigFloatingWindow`（**新**，v2 P1 交付，详见 §1.5） |

### 1.3 跨 tab 联动（B 方案双视图同步）

```
client 端选中状态机（CombatTrainingPanel 重命名为 TechniquesTabPanel）：
  TechniquesTabPanel.selectedTechniqueId: String  // 仅 client，不走网络

  选中变化时（点击行 / 搜索过滤后第一项 / hover 高亮）：
    fire selectionChanged(skill_id)
      → TechniqueDetailCard.refresh(snapshot.findById(skill_id))
      → MeridianMiniSilhouette.highlight(detail.required_meridians)
        // 「功法」tab 内的左下经脉剪影同步高亮
      → MeridianFullView.highlightActiveTechnique(skill_id)
        // 「修仙」tab 完整 20 经脉视图也按当前选中被动高亮
        // —— 即使玩家此刻在「功法」tab，切到「修仙」tab 时高亮已就位
      → CombatHotbarStripObserver.markBoundSlots(SkillBarStore.findBySkillId(skill_id))
        // 左侧 hotbarStrip 已绑该功法的槽描金边（v1 已有）
```

无新 server payload —— `selectedTechniqueId` 是纯 client UI 状态。

### 1.4 SkillConfig Floating Window 规格

**实现路径**：基于 owo-lib `Positioning.absolute(x, y)` —— 现网 `InspectScreen.java:290 / 347` 已用此机制做隐藏 trick（`-9999, -9999`），同一 API 用正坐标即放置 floating window。`BotanyHudPlanner` 「采集浮窗」(`docs/svg/harvest-popup.svg`) 是同源现成模板。

**布局**：

```
┌─ SkillConfigFloatingWindow ─────────────────┐
│ ▓▓ 配置 · 绝脉断链 ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ × ││ ← 标题栏（drag handle + close button）
├──────────────────────────────────────────────┤
│ 选定经脉                                      │
│ [心包经 ▾]                                   │
│                                              │
│ 反震加成攻击类型                              │
│ ⦿ 真元类   ○ 物理载体类                     │
│ ○ 脏真元类 ○ 阵法类                         │
│                                              │
│ ┌─────┐ ┌─────────┐                        │
│ │ 取消 │ │ 保存配置 │                        │
│ └─────┘ └─────────┘                        │
└──────────────────────────────────────────────┘
```

**交互规格**：

- **触发**：详情卡右下齿轮按钮 click → server 不参与，client 本地创建组件 + `root.child(window)` + `window.positioning(Positioning.absolute(centerX, centerY))`
- **初始位置**：齿轮按钮坐标向左上偏移 (window.width, window.height) → 浮窗右下角对齐齿轮位置（visual cue 联动）
- **拖拽**：标题栏 mouseDragged 事件 → 累加到 `window.positioning(Positioning.absolute(x + dx, y + dy))`；mixin 走 owo-lib 内置 drag handler，无需自写
- **关闭**：标题栏右上 X 按钮 / ESC 键 / 详情卡选中其他功法 → `root.removeChild(window)`；保存按钮 click 同时关闭
- **z-index**：作为 `root` 的最后一个 child 自动渲染最上层，覆盖详情卡 / 列表 / 经脉剪影
- **保存语义**：保存按钮 click → 发 `bong:skill/config_intent { skill_id, config }`，server 校验 schema 后写入 `SkillConfigStore` 并推回 `bong:skill/config_snapshot` 同步
- **cast 期间自动收起**：client 监听 `Casting` 状态变化 → cast 开始时若浮窗打开则强制 close + tooltip "施法中不可改配置（详见 §2.3）"
- **不暂停世界**：MC 服务器一致语义，浮窗打开期间敌人继续攻击 / 玩家停手不能反应（自负风险，跟开 InspectScreen 同等级）
- **多实例**：同一时刻只允许一个 SkillConfigFloatingWindow（`SkillConfigPanelManager` client side singleton），打开新功法配置 → 自动关闭旧浮窗

**视觉规格**：

- `surface(Surface.flat(0xFF2A2A2A))` + 1px outline `0xFF606060`（区别于详情卡 `0xFF1A1A1A`，让浮窗更"浮"）
- 标题栏背景 `0xFF3A3A3A` + drag cursor (MC 不支持自定 cursor，用 hover 高亮代替)
- close 按钮：MC 原生 `×` 字符 + hover 红框
- 阴影：浮窗下方 +2px 半透明黑边模拟 drop shadow

### 1.5 拖拽流程（沿用 v1 §4.4，删冗余）

```
源：「功法」tab 列表中拖起一项
  - 功法行 → DragState.source = TECHNIQUE(skill_id)
  - 锁定态功法（境界不足 / 经脉 SEVERED）→ drop 即 reject + "境界不足"提示

目标：左侧 hotbarStrip 槽（DragState.target = SKILL_BAR(slot)）
  - 命中：本地 SkillBarStore.slots[slot] = Skill { skill_id }（乐观更新）
  - 发 SkillBarBind { slot, binding: { kind: "skill", skill_id } }（v1 已有）
  - server 验证 → 推 SkillBarConfigV1 → SkillBarConfigHandler 同步 / 回滚

技艺行不出现在「功法」tab —— 「技艺」tab 不动，HERB/ALCH/FORG 仍归原 tab
```

---

## §2 `SkillConfig` 通用底盘

### 2.1 数据模型

```rust
// server/src/skill/config.rs（新文件）

/// 招式专属配置，按 skill_id schema 校验。
/// HashMap<String, ConfigValue> 而非 enum —— 各招字段不同，运行时校验。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
    pub fields: HashMap<String, ConfigValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigValue {
    String(String),       // dropdown 选项 ID（如 meridian_id="Pericardium"）
    Int(i32),             // 滑条整数（如 echo_count_target=30）
    Float(f32),           // 滑条小数
    Bool(bool),           // checkbox / radio
    MeridianId(MeridianId), // 类型安全 dropdown
}

/// 服务端持久化（跨 server restart）。
#[derive(Resource, Default)]
pub struct SkillConfigStore {
    map: HashMap<(PlayerId, SkillId), SkillConfig>,
}
```

### 2.2 各招 schema 注册

```rust
// server/src/combat/zhenmai_v2/registry.rs（zhenmai-v2 P0 交付）
SkillConfigSchema::register("zhenmai.sever_chain", &[
    ("meridian_id",  ConfigField::MeridianId { default: MeridianId::Lung, allowed: ALL_20 }),
    ("backfire_kind", ConfigField::Enum {
        default: "real_yuan",
        options: &["real_yuan", "physical_carrier", "tainted_yuan", "array"],
    }),
]);

// server/src/combat/anqi_v2/registry.rs（anqi-v2 vN+1，可选）
SkillConfigSchema::register("anqi.fractal_decoy", &[
    ("preferred_carrier_grade", ConfigField::Enum {
        default: "shanggu_bone",   // 档 6 上古残骨默认
        options: &["mutant_bone", "lingmu_quiver", "dyed_bone", "fenglinghe_bone", "shanggu_bone"],
    }),
]);
```

各招 P0 交付时自带 schema 注册。schema 缺省 → 详情卡齿轮不显示（招式无可配置项）。

### 2.3 cast 期间配置不可改

```rust
// server/src/skill/config.rs
pub fn handle_config_intent(
    intent: SkillConfigIntent,
    casting: Option<&Casting>,
    store: &mut SkillConfigStore,
) -> Result<(), ConfigRejectReason> {
    if casting.is_some_and(|c| c.skill_id.as_deref() == Some(&intent.skill_id)) {
        return Err(ConfigRejectReason::CurrentlyCasting);
    }
    // ... schema 校验 + 写入
}

// Casting 创建时快照配置
fn create_casting(skill_id: &str, store: &SkillConfigStore, ...) -> Casting {
    Casting {
        skill_config: store.get(player_id, skill_id).cloned(),
        // ...
    }
}
```

### 2.4 schema 缺省 / 不合法配置的失败 cast

```
玩家按数字键 cast → server 检查：
  1. 该招有 schema？无 → 直接进 Casting（不需要配置）
  2. 该招有 schema 但 SkillConfigStore 无 entry → cast 失败 + HUD 红字
     "未配置 [zhenmai.sever_chain]：经脉 / 反震类型 必填"
  3. 有 entry 但 schema 升级后字段不全（旧配置） → cast 失败 + HUD 提示重配
```

---

## §3 阶段交付物

### P0 — 砍冗余 + 「功法」tab 骨架（3 周）

- [ ] **删除**：`client/src/main/java/com/bong/client/inspect/CombatTrainingPanel.java` 及其引用（v1 §4 实装产物）；从 `InspectScreen.tabs` 移除"战斗·修炼"项
- [ ] **新建**：`TechniquesTabPanel.java`（「功法」tab 容器，§1.1 双栏布局）+ `TechniqueSearchBar.java` + `MeridianMiniSilhouette.java`（独立 file 拆出，原 `MeridianChannel` 类删除）
- [ ] `TechniqueRowComponent.java` 加 skill exp 进度条渲染（沿用 `SkillSetSnapshot.Entry.progressRatio()`）
- [ ] 「修仙」tab 的 `MeridianFullView.java` 加 `highlightActiveTechnique(skill_id)` 接口（跨 tab 联动 client side 状态机）
- [ ] `InspectScreen.tabs = ["装备", "修仙", "技艺", "功法"]`
- [ ] 测试：client/test 「功法」tab 渲染回归 12 测 + 跨 tab 联动 4 测 + 删 v1 「战斗·修炼」tab 后无残留引用 grep 验证

### P1 — `SkillConfig` 通用底盘 + 齿轮翻面（3 周）

- [ ] `server/src/skill/config.rs` 新文件 —— `SkillConfig` / `SkillConfigStore` / `SkillConfigSchema` / `handle_config_intent`
- [ ] `combat::Casting.skill_config: Option<SkillConfig>` 字段 + cast 创建时快照
- [ ] `agent/packages/schema/src/skill_config.ts` TypeBox 定义 + `samples/skill_config_intent.json` 双端校验
- [ ] `bong:skill/config_intent` payload（client → server）+ `bong:skill/config_snapshot` payload（server → client，恢复界面态）
- [ ] client `SkillConfigFloatingWindow.java` —— owo-lib `Positioning.absolute` + 标题栏 drag handler + 右上 X close button + 按 schema dropdown / 滑条 / radio 渲染；保存按钮发 `SkillConfigIntent`（详细规格 §1.4）
- [ ] client `SkillConfigPanelManager.java` —— singleton 管理同时只允许一个浮窗；监听 `Casting` 状态变化 → cast 开始时强制 close
- [ ] cast 期间禁用配置（`Casting` 标记 → 齿轮按钮灰显 tooltip "施法中不可改配置" + 任何已打开浮窗强制 close）
- [ ] `Casting.skill_config` 快照实装（cast 创建时 `store.get(player_id, skill_id).cloned()`，cast 中改 store 不影响当前 cast）
- [ ] 测试：`SkillConfigStore` 跨 server restart 持久化 6 测 + schema 校验 / 缺字段 cast 失败 8 测 + floating window 拖拽 / 多实例互斥 / 关闭 / cast 期间强制 close 8 测 + `Casting.skill_config` 快照测试 4 测（cast 开始后改 store 不影响 cast 结算）

### P2 — 首个使用方 zhenmai-v2 ⑤ 接入 + e2e（2 周）

- [ ] zhenmai-v2 ⑤ 绝脉断链注册 schema（`meridian_id` + `backfire_kind`）—— 跟 zhenmai-v2 P0 交付门同步
- [ ] 「功法」tab 内绝脉断链行 → 详情卡 → 齿轮 → 配置面板 dropdown 选 20 经脉 + 4 攻击类型 → 保存 → 拖到 1-9 槽 → 战斗中按数字键 cast 完成永久 SEVERED 全链路 e2e
- [ ] 缺省配置 cast 失败的 HUD 红字提示 e2e
- [ ] 测试：e2e 2 套（happy path + cast 失败）+ 配置面板 dropdown 渲染 4 测

### P3 — 收口 + 后续 plan 接入面文档（1 周）

- [ ] 在 `plan-anqi-v2` / `plan-yidao-v1` / `plan-baomai-v3` 各 plan 头部接入面 checklist 加一行 "`SkillConfig` 通用底盘已就位 → vN+1 详情卡可加齿轮配置"
- [ ] `reminder.md` 登记 vN+1 待办：各 v2 流派 plan 收口时按需注册 schema
- [ ] Finish Evidence + 迁入 `docs/finished_plans/`

---

## §4 已知风险 / open 问题

- [ ] **Q1** 经脉剪影位置（左下）vs 详情卡位置（右）—— 选中功法时人眼焦点在右侧详情卡，左下剪影联动是否在视野盲区？P0 用户测一下 UX
- [x] **Q2** ~~齿轮翻面 vs modal 抽屉~~ → **2026-05-07 拍板：floating window 浮窗模式**。详情卡保留不变 + 浮窗叠加在上方（owo-lib `Positioning.absolute` + `BotanyHudPlanner` 采集浮窗同源）+ 标题栏可拖拽 + 右上 X 关闭。详细规格见 §1.4
- [ ] **Q3** schema 升级兼容（玩家旧配置缺新字段）—— 失败 cast + 提示重配 vs 自动用 default 填充？前者更安全（玩家审慎选）；后者更顺。P1 拍板
- [ ] **Q4** 「修仙」tab 跨 tab 联动 vs「功法」tab 内剪影 —— 双高亮信息冗余但保险；用户实测后是否单点保留即可？P2 收口时复盘
- [x] **Q5** ~~SkillConfig 快照 vs 实时读~~ → **2026-05-07 拍板：快照**。cast 开始时 server 端 `Casting.skill_config = store.get(player_id, skill_id).cloned()`，cast 中即使配置被改也不影响当前 cast 结算（且 cast 期间 client floating window 自动收起，store 实际无法被改，双重保险）
- [ ] **Q6** 搜索框是否支持中文 / 拼音首字母 / 模糊匹配 / fuzzy？P0 简单 contains 即可，复杂搜索 vN+1
- [ ] **Q7** 锁定态功法是否在搜索框内可被搜出？显示但 grayed reject drop 还是过滤掉？建议显示（玩家想看自己未解锁的功法），按 P0 简单方案处理

---

## §5 进度日志

- 2026-05-07：骨架创建。承接 plan-hotbar-modify-v1 ✅ finished（验收 2026-04-29）。v2 范围：删「战斗·修炼」tab + 新增「功法」独立 tab（搜索 + 列表 + 左下经脉剪影 + 右侧详情 + 齿轮配置）+ `SkillConfig` 通用底盘（zhenmai-v2 ⑤ 起源，后续 anqi/yidao/baomai 复用）+ 「修仙」tab B 方案双视图联动。删 v1 「战斗·修炼」tab 实装代码（plan finished 才两周无用户习惯积累，不留 deprecation 期）。
- 2026-05-07：Q2 / Q5 拍板。Q2 → SkillConfig 走 **floating window** 浮窗模式（不是翻面也不是 modal 抽屉），owo-lib `Positioning.absolute` + `BotanyHudPlanner` 采集浮窗同源；标题栏可拖拽 + 右上 X close + cast 期间强制收起 + 同时只允许一个浮窗（singleton）。Q5 → `Casting.skill_config` 用 **快照**（cast 开始时 server 端 `store.get().cloned()`），cast 中改 store 不影响当前 cast 结算（且 cast 期间 client 浮窗强制 close 双重保险）。
