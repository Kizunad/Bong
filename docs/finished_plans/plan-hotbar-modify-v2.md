# Bong · plan-hotbar-modify-v2

InspectScreen 功法工作台重整 + `SkillConfig` 通用配置底盘。承接 `plan-hotbar-modify-v1` finished（验收 2026-04-29），本 plan 做两件事：

1. 把 v1 的「战斗·修炼」工作台改成真正的「功法」tab：只展示战斗功法，不再混入技艺三行和重复状态块。
2. 建立 `SkillConfig`：每个招式可声明一组可配置字段，玩家在功法详情卡打开 floating window 配置，server 在 cast 开始时读取快照。

**状态**：2026-05-08 finished。v1 skillbar / techniques snapshot 基座已复用；v2 UI 清理、`SkillConfig` 通用底盘、floating window、`zhenmai.sever_chain` schema fixture 已落地。

**世界观锚点**：无新增物理 / 经济概念。`SkillConfig` 只保存玩家对招式参数的选择，不直接写物理公式；具体代价仍归消费者 plan。例如 zhenmai-v2 的「绝脉断链」永久 SEVERED 语义归 zhenmai-v2 + meridian-severed-v1，本 plan 只提供配置入口与快照机制。

**library 锚点**：无。纯 UI / 协议 / store 底盘。

---

## 现状对齐（2026-05-08）

### 已落地的 v1 基座

| 能力 | 代码现实 | 本 plan 处置 |
|---|---|---|
| 1-9 SkillBar 存储 | `server/src/combat/components.rs` 已有 `SkillSlot` / `SkillBarBindings` | P0/P1 复用，不重建 |
| 1-9 cast/bind 协议 | `ClientRequestV1::{SkillBarCast, SkillBarBind}` + `handle_skill_bar_cast` / `handle_skill_bar_bind` 已存在 | 继续复用 `skill_bar_bind`；`SkillConfig` 不新建平行 hotbar 协议 |
| F1-F9 QuickSlot | `UseQuickSlot` / `QuickSlotBind` server + TS schema 已补齐 | 本 plan 不删除 `快捷使用` 路径 |
| 已学功法快照 | `TechniquesSnapshotV1` / `techniques_snapshot` server-data 已存在，含 `required_meridians` / qi / cast / cd / range | 「功法」tab 直接消费 |
| v1 UI 工作台 | `client/src/main/java/com/bong/client/combat/inspect/CombatTrainingPanel.java` 仍被 `InspectScreen` 挂载为「战斗·修炼」 | P0 替换成 `TechniquesTabPanel` |
| 经脉缩略组件 | `MeridianMiniView.java` 已有文字型需求摘要 | P0 改名并扩成 `MeridianMiniSilhouette`；不删除核心 `MeridianChannel` enum |
| 详情卡 | `TechniqueDetailCard.java` 已显示描述 / 需求 / 数值 / 绑定槽 | P0 复用并加齿轮锚点，P2 接配置窗口 |

### 当前缺口

- 全仓无 `SkillConfig` / `SkillConfigStore` / `skill_config_intent` / `skill_config_snapshot` 实装。
- `InspectScreen.TAB_NAMES` 仍是 `["装备", "修仙", "技艺", "战斗·修炼", "快捷使用"]`。
- `CombatTrainingPanel` 仍把「功法」和 `SkillId.values()` 技艺行混在同一个工作台里，和「技艺」tab / `BottomInfoBar` 重复。
- 反向消费者 `plan-zhenmai-v2` / `plan-anqi-v2` / `plan-yidao-v1` / `plan-baomai-v3` 目前仍在 `docs/plans-skeleton/`，不是 active。它们是后续消费者，不作为本 plan P0/P1 的前置。

### 清理后的边界

- **保留** `快捷使用` tab 与 `quickUseStrip`。本 plan 只替换「战斗·修炼」为「功法」，不顺手删除 F1-F9 配置面。
- **不删除** `client/src/main/java/com/bong/client/inventory/model/MeridianChannel.java`。它是 InspectScreen / BodyInspect / tests 的核心枚举；要重命名的是 v1 的 `MeridianMiniView` 组件。
- **不新增** `bong:skill/config_intent` 独立 channel。客户端仍走既有 `bong:client_request`，新增 request type `skill_config_intent`；server 同步仍走既有 `bong:server_data`，新增 payload type `skill_config_snapshot`。
- **不实现** zhenmai-v2 绝脉断链 gameplay。P3 只做 `zhenmai.sever_chain` 契约 fixture / schema 样例，真实 cast 由 zhenmai-v2 consume 时接入。

---

## 依赖与被依赖

**前置依赖**

- `plan-hotbar-modify-v1` finished：SkillBar / QuickSlot / InspectScreen hotbar 绑定通路已就位。
- `plan-skill-v1` finished：`SkillSet` / `SkillSetStore` / 技艺熟练度数据源已就位。
- `plan-cultivation-canonical-align-v1` finished：20 经脉拓扑与 InspectScreen 经脉图已就位。
- `plan-HUD-v1` + `plan-input-binding-v1` finished：输入与 HUD cast 反馈底座已就位。
- `plan-meridian-severed-v1` active：SEVERED 可视化与招式依赖经脉强约束的上游底盘；本 plan 只读取/展示它同步出的状态。

**后续消费者**

- `plan-zhenmai-v2` skeleton：首个强消费者，`zhenmai.sever_chain` 需要 `meridian_id` + `backfire_kind`。
- `plan-anqi-v2` skeleton：后续可配置载体档 / 容器槽。
- `plan-yidao-v1` skeleton：后续可配置治疗优先级 / 患者筛选。
- `plan-baomai-v3` skeleton：后续可配置散功流量档或连招策略。

---

## 接入面 Checklist

- **进料**
  - client：`TechniquesListPanel.snapshot()` / `SkillBarStore.snapshot()` / `SkillSetStore.snapshot()` / `MeridianStateStore.snapshot()` / `CastStateStore.snapshot()`
  - server：`SkillRegistry` / `KnownTechniques` / `Casting` / `PlayerStatePersistence` / `ClientRequestV1`
  - schema：`combat-hud.ts` 的 `TechniquesSnapshotV1` / `client-request.ts` / `server-data.ts`
- **出料**
  - client：`TechniquesTabPanel` / `TechniqueSearchBar` / `TechniqueRowComponent` / `MeridianMiniSilhouette` / `SkillConfigFloatingWindow` / `SkillConfigPanelManager`
  - server：`skill::config::{SkillConfig, SkillConfigStore, SkillConfigSchema, handle_config_intent}`
  - schema：`SkillConfigIntentRequestV1` / `SkillConfigSnapshotV1`
  - runtime：`Casting.skill_config: Option<SkillConfig>` 快照
- **共享契约**
  - `skill_config_intent`：client -> server，走 `bong:client_request`
  - `skill_config_snapshot`：server -> client，走 `bong:server_data`
  - `SkillConfig` 只允许 JSON object；字段是否合法由 `SkillConfigSchema` 按 `skill_id` 校验

---

## §0 设计轴心

- [ ] **只清理重复 UI，不推翻 v1 基座**：SkillBar / QuickSlot / TechniquesSnapshot 已经可用，v2 只做更清晰的 tab 和配置底盘。
- [ ] **「功法」tab 只管战斗功法**：`HERBALISM/ALCHEMY/FORGING/...` 仍归「技艺」tab，避免同屏重复。
- [ ] **配置是招式附属，不是招式逻辑**：`SkillConfig` 保存输入，zhenmai/anqi/yidao/baomai 等消费者自己解释字段含义。
- [ ] **cast 读快照**：cast 开始时把当前配置拷进 `Casting.skill_config`；cast 中改配置不影响本次结算。
- [ ] **cast 期间不可改配置**：client 灰显齿轮并自动关闭窗口；server handler 仍要二次拒绝，防 client 伪造。
- [ ] **无 schema 不显示齿轮**：默认大多数招式无配置项，详情卡不增加噪音。
- [ ] **缺必填配置则拒绝 cast**：有 schema 且缺必填字段时，server 拒绝并推 HUD/日志反馈；不能默默使用危险默认值。

---

## §1 「功法」tab UI 规格

### 1.1 Tab 与文件重构

目标 tab：

```java
private static final int TAB_EQUIP = 0;
private static final int TAB_CULTIVATION = 1;
private static final int TAB_SKILL = 2;
private static final int TAB_TECHNIQUES = 3;
private static final int TAB_QUICK_USE = 4;
private static final String[] TAB_NAMES = {"装备", "修仙", "技艺", "功法", "快捷使用"};
```

重构动作：

- `CombatTrainingPanel.java` -> `TechniquesTabPanel.java`
- `MeridianMiniView.java` -> `MeridianMiniSilhouette.java`
- 新增 `TechniqueSearchBar.java`
- 新增 `TechniqueRowComponent.java`（或从 `TechniquesTabPanel` 中拆出 row builder；若代码量仍小，可保留包私有类，不为抽象而抽象）
- `InspectScreen` 字段与方法从 `combatTrainingPanel` / `TAB_COMBAT_TRAINING` 改名为 `techniquesTabPanel` / `TAB_TECHNIQUES`

### 1.2 布局

```text
功法 tab
├── 左列 40%
│   ├── 搜索框
│   ├── 功法列表（滚动）
│   │   └── 行：阶位 / 名称 / 熟练度 / 绑定槽 / 锁定态
│   └── 经脉剪影（按 selected/hover technique 高亮 required_meridians）
└── 右列 60%
    └── 详情卡
        ├── 名称 / 阶位 / 描述
        ├── 真元 / cast / cooldown / range
        ├── required_meridians 明细
        ├── 已绑定槽反查
        └── 齿轮按钮（仅该 skill_id 有 SkillConfig schema 时显示）
```

### 1.3 交互

- 搜索：P0 只做大小写不敏感 `contains`，匹配 `skill_id` / 显示名 / 别名。拼音首字母与 fuzzy 留后续。
- 选中：点击行设置 `selectedTechniqueId`；搜索过滤后当前选中不在结果内，则选第一项。
- Hover：hover 行时临时高亮该行 required_meridians；离开后恢复 selected。
- 绑定：选中功法后点击左侧 1-9 hotbar slot，复用 `ClientRequestSender.sendSkillBarBindSkill(slot, skill_id)`。
- 清空：右键左侧 1-9 skill slot，复用 `sendSkillBarBindClear(slot)`。
- 锁定态：境界不足 / required meridian SEVERED / schema 必填未配置时，行可见但灰显；drop 或 bind 被拒，状态行给原因。

### 1.4 跨 tab 经脉联动

P0 内只做 client-side 状态同步，不新增 server payload：

```text
TechniquesTabPanel.selectedTechniqueId
  -> TechniqueDetailCard.refresh(selected)
  -> MeridianMiniSilhouette.highlight(selected.required_meridians)
  -> BodyInspectComponent.setTechniqueHighlight(required_meridians)
  -> SkillBarStore.findSkill(selected.id()) 标记已绑定槽
```

`BodyInspectComponent` 现有 `setMeridianHighlight(MeridianChannel ch, boolean valid)` 是单经脉接口；P0 扩成可清空/设置多经脉的轻量 API，不新建 `MeridianFullView` 平行类。

---

## §2 SkillConfig 数据模型与协议

### 2.1 Rust 模型

```rust
// server/src/skill/config.rs
use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use valence::prelude::{Component, Resource};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillConfig {
    pub fields: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillConfigSnapshot {
    pub configs: BTreeMap<String, SkillConfig>,
}

#[derive(Debug, Default, Resource)]
pub struct SkillConfigStore {
    configs: HashMap<String /* player_id */, BTreeMap<String /* skill_id */, SkillConfig>>,
}

#[derive(Debug, Clone)]
pub struct SkillConfigSchema {
    pub skill_id: &'static str,
    pub fields: Vec<ConfigField>,
}

#[derive(Debug, Clone)]
pub struct ConfigField {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ConfigFieldKind,
    pub required: bool,
    pub default: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum ConfigFieldKind {
    Enum { options: Vec<&'static str> },
    MeridianId { allowed: Vec<crate::cultivation::components::MeridianId> },
    IntRange { min: i32, max: i32, step: i32 },
    FloatRange { min: f32, max: f32, step: f32 },
    Bool,
}
```

说明：

- wire format 用 JSON object，避免为每个字段类型扩一个嵌套 enum。
- schema 负责把 JSON 值校验成合法语义；consumer cast 时再按字段读取。
- store key 使用稳定 player id；实现时优先复用 `canonical_player_id(username)`，若接入当前角色 id，则用 `player_character_id(username, current_char_id)`，不能用临时 `Entity`。

### 2.2 持久化

默认实现走现有 `player_ui_prefs.prefs_json`，新增：

```rust
#[serde(default)]
pub skill_configs: BTreeMap<String, SkillConfig>,
```

理由：

- skillbar / quickslot 绑定已经在 `PlayerUiPrefs`，配置入口属于同一类玩家操作偏好。
- 不改 SQLite schema，只改 JSON 默认值，legacy prefs 可无痛解码。
- 真实不可逆效果仍由 cast consumer 写入 gameplay state；`skill_configs` 本身不产生 SEVERED / qi / damage。

验收必须覆盖：

- legacy prefs 没有 `skill_configs` 可 decode。
- 保存配置后重启可恢复。
- 清空配置后 snapshot 不再含该 skill_id。
- schema 升级时旧配置缺必填字段，cast/handler 明确拒绝。

### 2.3 C2S request

`agent/packages/schema/src/client-request.ts` 新增：

```ts
export const SkillConfigIntentRequestV1 = Type.Object({
  v: Type.Literal(1),
  type: Type.Literal("skill_config_intent"),
  skill_id: Type.String({ minLength: 1 }),
  config: Type.Record(Type.String({ minLength: 1 }), Type.Unknown()),
}, { additionalProperties: false });
```

Rust 对应：

```rust
ClientRequestV1::SkillConfigIntent {
    v: u8,
    skill_id: String,
    config: BTreeMap<String, serde_json::Value>,
}
```

handler：

```rust
fn handle_skill_config_intent(
    entity: Entity,
    skill_id: String,
    config: BTreeMap<String, Value>,
    casting: Option<&Casting>,
    store: &mut SkillConfigStore,
    schemas: &SkillConfigSchemas,
) -> Result<(), SkillConfigRejectReason>
```

拒绝条件：

- unknown `skill_id`
- 该 skill 无 schema
- 字段缺失 / 类型不符 / enum 值不在 allowed
- 当前 caster 正在 cast 同一个 `skill_id`
- config object 超过字段数上限或含未知字段（默认拒绝，避免 typo 静默保存）

### 2.4 S2C snapshot

`agent/packages/schema/src/server-data.ts` 新增 `skill_config_snapshot`：

```ts
export const SkillConfigSnapshotV1 = Type.Object({
  configs: Type.Record(
    Type.String({ minLength: 1 }),
    Type.Record(Type.String({ minLength: 1 }), Type.Unknown()),
  ),
}, { additionalProperties: false });
```

server-data payload：

```json
{
  "v": 1,
  "type": "skill_config_snapshot",
  "configs": {
    "zhenmai.sever_chain": {
      "meridian_id": "Pericardium",
      "backfire_kind": "tainted_yuan"
    }
  }
}
```

client 侧新增 `SkillConfigStore` + `SkillConfigSnapshotHandler`，与 `SkillBarConfigHandler` 同模式。

### 2.5 Casting 快照

`server/src/combat/components.rs::Casting` 增加：

```rust
pub skill_config: Option<crate::skill::config::SkillConfig>,
```

所有构造 `Casting` 的路径必须显式填字段：

- QuickSlot / item cast：`None`
- generic SkillBar cast：从 `SkillConfigStore` 按 `skill_id` 读取 clone
- 专属 skill resolver（如 `burst_meridian`）：当前无 schema，先填 `None`；未来有 schema 的 resolver 必须从 helper 创建 `Casting`

推荐 helper：

```rust
pub fn skill_config_snapshot_for_cast(
    store: Option<&SkillConfigStore>,
    player_id: &str,
    skill_id: &str,
) -> Option<SkillConfig>
```

---

## §3 SkillConfig Floating Window

### 3.1 显示规则

- 详情卡右下角齿轮只在 `SkillConfigSchemaRegistry.hasSchema(skill_id)` 为 true 时显示。
- cast 期间齿轮灰显，tooltip：「施法中不可改配置」。
- 保存后立即乐观更新 client store，并发送 `skill_config_intent`；server snapshot 回来后以 server 为准。

### 3.2 组件

新增：

- `client/.../combat/inspect/SkillConfigFloatingWindow.java`
- `client/.../combat/inspect/SkillConfigPanelManager.java`
- `client/.../combat/SkillConfigStore.java`
- `client/.../network/SkillConfigSnapshotHandler.java`

窗口行为：

- `Positioning.absolute(x, y)` 放在 InspectScreen root 最后一个 child。
- 初始位置靠近详情卡齿轮，右下对齐齿轮附近。
- 标题栏拖拽移动，限制在 screen bounds 内。
- X / ESC / 选择其他功法 / cast 开始都会关闭。
- 同一时刻只允许一个实例，打开新窗口先关闭旧窗口。

### 3.3 字段渲染

| schema kind | client 控件 | wire value |
|---|---|---|
| `Enum` | radio group 或 dropdown（选项 <= 4 用 radio） | string |
| `MeridianId` | dropdown，显示中文经脉名 + code | string，如 `Pericardium` |
| `IntRange` | slider + stepper | number |
| `FloatRange` | slider + numeric label | number |
| `Bool` | checkbox/toggle | boolean |

P2 只需实现 `Enum` + `MeridianId` + `Bool`。`IntRange` / `FloatRange` 可在 P3 补，除非 fixture 已用。

---

## §4 阶段总览

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0** | UI 清理：`CombatTrainingPanel` 改为 `TechniquesTabPanel`；删除混入的技艺行；加搜索；替换 tab 文案为「功法」；`MeridianMiniView` 改为 `MeridianMiniSilhouette`；跨 tab 经脉高亮 | client tests 通过；grep 无「战斗·修炼」运行态残留；`快捷使用` tab 保持可见 |
| **P1** | server/schema 底盘：`SkillConfig` / schema registry / request handler / snapshot payload / persistence / `Casting.skill_config` 快照 | server + schema tests 通过；legacy prefs 兼容；cast snapshot isolation 覆盖 |
| **P2** | client 配置窗口：floating window / manager / schema 渲染 / 保存 intent / snapshot handler / cast 期间关闭 | client tests 通过；窗口多实例互斥、关闭、拖拽、cast close 覆盖 |
| **P3** | 首个消费者契约 fixture + 文档收口：注册 `zhenmai.sever_chain` schema fixture（不实现 gameplay）；补后续 skeleton 接入说明；Finish Evidence | schema sample 双端校验；zhenmai/anqi/yidao/baomai skeleton 引用不再写错 active 状态 |

---

## §5 P0 详细交付

- [ ] `InspectScreen`：
  - `TAB_COMBAT_TRAINING` -> `TAB_TECHNIQUES`
  - `TAB_NAMES` 改为 `{"装备", "修仙", "技艺", "功法", "快捷使用"}`
  - 字段 `combatTrainingPanel` -> `techniquesTabPanel`
  - hotbar 绑定判断从 `activeTab == TAB_COMBAT_TRAINING` 改为 `TAB_TECHNIQUES`
- [ ] `CombatTrainingPanel.java` 改名 `TechniquesTabPanel.java`
  - 删除 `SkillId.values()` 技艺行
  - 保留 selected/bind/clear 逻辑
  - 列表只读 `TechniquesListPanel.snapshot()`
- [ ] `TechniqueSearchBar`
  - contains 匹配 `id/displayName/alias`
  - 空输入显示全部
  - 搜索结果为空时详情卡清空并显示「未找到功法」
- [ ] `TechniqueRowComponent`
  - 显示 grade / name / proficiency percent / bound slot
  - 锁定态灰显，原因来自 realm / meridian / missing config
- [ ] `MeridianMiniSilhouette`
  - 从 `MeridianMiniView` 演进，不删除 `MeridianChannel`
  - 显示 selected/hover technique 的 required meridians
  - 读取 `MeridianStateStore`，SEVERED / blocked 状态用不同颜色
- [ ] `BodyInspectComponent`
  - 新增多经脉 passive highlight API
  - 切出「功法」tab 再回「修仙」tab，高亮仍按当前 selectedTechniqueId 生效
- [ ] tests
  - tab names 回归
  - 「战斗·修炼」无运行态引用
  - 技艺行不出现在「功法」tab
  - 搜索过滤 / 空结果 / 选中保持
  - 绑定 / 清空仍发原 `skill_bar_bind`
  - required meridians 高亮 API 覆盖

---

## §6 P1 详细交付

- [ ] `server/src/skill/config.rs`
  - `SkillConfig`
  - `SkillConfigStore`
  - `SkillConfigSchema`
  - `SkillConfigSchemas` registry resource
  - `validate_skill_config(skill_id, config)`
  - `handle_config_intent`
- [ ] `server/src/skill/mod.rs` export config module
- [ ] `server/src/schema/client_request.rs`
  - 加 `SkillConfigIntent`
  - 版本号仍为 `v: 1`
- [ ] `server/src/schema/server_data.rs`
  - 加 `SkillConfigSnapshot`
- [ ] `server/src/player/state.rs`
  - `PlayerUiPrefs.skill_configs`
  - legacy decode/default tests
  - load join 时注入 `SkillConfigStore` 或按玩家 join 推 snapshot
- [ ] `server/src/network/client_request_handler.rs`
  - route `skill_config_intent`
  - cast 中拒绝同 skill 配置变更
  - 保存后推 `skill_config_snapshot`
- [ ] `server/src/combat/components.rs`
  - `Casting.skill_config`
  - 所有 `Casting { ... }` 构造点补字段
- [ ] tests
  - schema roundtrip
  - unknown skill reject
  - unknown field reject
  - missing required reject
  - enum invalid reject
  - currently casting same skill reject
  - cast 开始后 store 改动不影响 `Casting.skill_config`
  - persistence restart restore

---

## §7 P2 详细交付

- [ ] `agent/packages/schema/src/skill-config.ts`
  - TypeBox 定义复用到 `client-request.ts` / `server-data.ts`
  - sample：`agent/packages/schema/samples/skill_config_intent.json`
- [ ] `client/src/main/java/com/bong/client/combat/SkillConfigStore.java`
  - snapshot / replace / update local / resetForTests
- [ ] `ClientRequestProtocol`
  - `encodeSkillConfigIntent(skillId, JsonObject config)`
- [ ] `ClientRequestSender`
  - `sendSkillConfigIntent(skillId, config)`
- [ ] `SkillConfigSnapshotHandler`
  - parse `skill_config_snapshot`
  - bad payload no-op + reason
- [ ] `SkillConfigFloatingWindow`
  - `Enum` / `MeridianId` / `Bool` fields
  - save / cancel / close
  - title drag
- [ ] `SkillConfigPanelManager`
  - singleton
  - close on selected technique change
  - close on `CastStateStore` entering casting
- [ ] tests
  - request JSON exact match
  - snapshot handler happy/bad payload
  - window render model chooses correct controls
  - manager closes previous window
  - cast close

---

## §8 P3 消费者契约与文档收口

P3 不实现 zhenmai gameplay，只建立第一个真实 schema fixture，保证后续 `plan-zhenmai-v2` 可以直接消费。

```rust
SkillConfigSchema {
    skill_id: "zhenmai.sever_chain",
    fields: vec![
        ConfigField {
            key: "meridian_id",
            label: "选定经脉",
            kind: ConfigFieldKind::MeridianId { allowed: MeridianId::ALL.to_vec() },
            required: true,
            default: Some(json!("Lung")),
        },
        ConfigField {
            key: "backfire_kind",
            label: "反震加成攻击类型",
            kind: ConfigFieldKind::Enum {
                options: vec!["real_yuan", "physical_carrier", "tainted_yuan", "array"],
            },
            required: true,
            default: Some(json!("real_yuan")),
        },
    ],
}
```

文档同步：

- `docs/plans-skeleton/plan-zhenmai-v2.md`：升 active 时已把 hotbar v2 引用改为 active dependency，并修正 `skill_config_intent` 走既有 `bong:client_request` 的通道说明；P3 只需复核是否还需要补 schema fixture 说明。
- `docs/plans-skeleton/plan-anqi-v2.md` / `plan-yidao-v1.md` / `plan-baomai-v3.md`：只写「后续可接 SkillConfig」，不得写成本 plan 已 finished。
- `docs/plans-skeleton/reminder.md` 如已有同组 reminder，则登记「v2+ 流派按需注册 SkillConfig schema」。

---

## §9 已知风险 / 决策

- [x] **Q1 quick use 是否删除**：不删。`快捷使用` 仍是现有 F1-F9 配置面，删除会扩大 scope。
- [x] **Q2 齿轮翻面 vs floating window**：使用 floating window。详情卡保留，窗口叠加。
- [x] **Q3 cast 配置实时读 vs 快照**：快照。`Casting.skill_config` 在 cast 开始时 clone。
- [ ] **Q4 schema 升级缺字段**：默认拒绝 cast + 提示重配，不自动填危险默认值。P1 实现时若字段明确无风险，可只对非 required 字段填 default。
- [ ] **Q5 config 持久化是否归 UI prefs**：默认放 `PlayerUiPrefs.skill_configs`，P1 若发现角色隔离已有更合适 API，可切到 character scoped store，但必须保留 legacy/default 测试。
- [ ] **Q6 搜索锁定态是否显示**：显示并灰显，避免玩家不知道未来有哪些功法。
- [ ] **Q7 IntRange/FloatRange 是否 P2 必做**：不必。P2 先做 Enum/MeridianId/Bool，等真实消费者需要数值滑条再加。

---

## §10 验证矩阵

**server**

```bash
cd server
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test skill::config
cargo test network::client_request_handler
cargo test schema::client_request
cargo test schema::server_data
```

**agent/schema**

```bash
cd agent/packages/schema
npm test
cd ../..
npm run build
```

**client**

```bash
cd client
JAVA_HOME=<jdk17> ./gradlew test build
```

**grep 验收**

```bash
rg -n "战斗·修炼|CombatTrainingPanel|TAB_COMBAT_TRAINING" client/src/main/java client/src/test/java
rg -n "skill_config_intent|skill_config_snapshot|SkillConfigStore|Casting\\.skill_config" server agent client
```

第一条 grep 预期仅允许历史注释或测试 fixture；运行态引用必须清零。第二条必须命中 server/schema/client 三侧。

---

## §11 Finish Evidence 模板

完成后归档前补：

```markdown
## Finish Evidence

- P0 UI：<commit>；client tests：<命令 + 结果>
- P1 SkillConfig：<commit>；server/schema tests：<命令 + 结果>
- P2 FloatingWindow：<commit>；client tests：<命令 + 结果>
- P3 契约 fixture + 后续 skeleton 同步：<commit>
- 全量验证：server / agent / client 三栈结果
```

---

## §12 进度日志

- 2026-05-07：初版 skeleton 创建，范围包括「功法」独立 tab + `SkillConfig` 通用底盘。
- 2026-05-08：升 active 前清理。修正旧 skeleton 的状态漂移：后续消费者仍是 skeleton；`SkillConfig` 走既有 `bong:client_request` / `bong:server_data`，不是新 channel；保留 `快捷使用` tab；不删除核心 `MeridianChannel`；把「齿轮翻面」统一改为 floating window；补现状对齐、分阶段交付、测试矩阵与 Finish Evidence 模板；同步修正 zhenmai-v2 skeleton 中对本 plan 的旧引用。

## Finish Evidence

### 落地清单

- P0 UI 清理：`client/src/main/java/com/bong/client/combat/inspect/TechniquesTabPanel.java` 替代旧 `CombatTrainingPanel`，`InspectScreen` tab 文案改为「功法」，功法 tab 只消费 `TechniquesListPanel.snapshot()`；`MeridianMiniSilhouette` 和 `BodyInspectComponent` 多经脉高亮 API 已接入，`快捷使用` tab 保留。
- P1 SkillConfig server 底盘：`server/src/skill/config.rs` 新增 `SkillConfig` / `SkillConfigStore` / `SkillConfigSchemas` / intent 校验；`PlayerUiPrefs.skill_configs` 持久化；`ClientRequestV1::SkillConfigIntent`、`ServerDataPayloadV1::SkillConfigSnapshot`、`Casting.skill_config` 和 `skill_config_emit` 已接入。
- P2 schema/client 配置窗口：`agent/packages/schema/src/skill-config.ts`、client request/server data schema/generated artifacts/sample 已补齐；client `SkillConfigStore`、`SkillConfigSnapshotHandler`、`ClientRequestProtocol.encodeSkillConfigIntent`、`ClientRequestSender.sendSkillConfigIntent`、`SkillConfigFloatingWindow`、`SkillConfigPanelManager` 已落地。
- P3 契约 fixture：`SkillConfigSchemas::default()` 注册 `zhenmai.sever_chain`，`KnownTechniques` / `TECHNIQUE_DEFINITIONS` 包含 fixture；generic skill cast 对有 schema 的功法要求配置有效并在 cast 开始时快照；不实现 zhenmai-v2 gameplay。

### 关键 commit

- `3f63065d7` (2026-05-08) `docs(plan-hotbar-modify-v2): 升 active 并清理引用`：从 skeleton 升 active，清理后续 skeleton 旧引用与 plan 边界。
- `afecedb9c` (2026-05-08) `feat(client): 重整功法 tab 工作台`：完成 P0 功法 tab、搜索、绑定、经脉高亮与旧命名清理。
- `0d55d05f9` (2026-05-08) `feat(server): 接入 SkillConfig 底盘`：完成 P1 server store/schema/handler/persistence/cast snapshot 基座。
- `2891c8eca` (2026-05-08) `feat(schema): 对齐 SkillConfig 协议`：完成 TypeBox schema、sample 与 generated schema。
- `67846eb57` (2026-05-08) `feat(client): 接入功法配置浮窗`：完成 P2 client store、snapshot handler、floating window、manager 与 request sender。
- `bc64bed77` (2026-05-08) `feat(server): 注册绝脉断链配置契约`：完成 P3 `zhenmai.sever_chain` fixture 与 cast config gate。
- `da2973a8e` (2026-05-08) `fix(test): 补齐 SkillConfig 验证收口`：补齐 router type set 与 clippy 收口，恢复全量 server/client 验证。
- `4183103e5` (2026-05-08) `fix(skill-config): 修复快照回推与配置测试缺口`：按 review 修复重连/拒绝后的权威 snapshot 回推，补 cast snapshot isolation、IntRange/FloatRange validation 与 client real-open save 回归。
- `d5365a2da` (2026-05-08) `fix(skill-config): 收口 review 快照与恢复校验`：按二轮 review 移除 client 乐观落库、隔离 cast listener 异常、深拷贝 SkillConfig snapshot、成功 intent 回推权威 snapshot、恢复持久化配置时重新校验并丢弃坏数据。

### 测试结果

- `cd agent && npm run generate -w @bong/schema`：通过，已刷新 skill config generated schema artifacts。
- `cd agent && npm run build && npm test -w @bong/schema`：通过，schema 11 files / 308 tests passed。
- `cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings`：通过。
- `cd server && cargo test`：通过，2793 tests passed。
- `cd client && JAVA_HOME="/home/kiz/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test build`：通过，build successful。
- `cd server && cargo test skill::config && cargo test skill_bar_cast_requires_config_for_schema_fixture && cargo test reconnect_with_same_player_id_receives_fresh_snapshot`：通过。
- `cd server && cargo test valid_skill_config_intent_replies_with_authoritative_snapshot && cargo test meridian_all_matches_partition_without_duplicates && cargo test deserialize_server_data_samples`：通过。
- `cd client && JAVA_HOME="/home/kiz/.sdkman/candidates/java/17.0.18-amzn" ./gradlew test --tests '*SkillConfig*' --tests '*CastStateMachineTest' --tests '*TechniquesListPanelTest'`：通过。
- `git diff --check`：通过。

### 跨仓库核验

- `rg -n "战斗·修炼|CombatTrainingPanel|TAB_COMBAT_TRAINING" client/src/main/java client/src/test/java`：无匹配，运行态旧 tab / 旧类名 / 旧常量已清零。
- `rg -n "skill_config_intent|skill_config_snapshot|SkillConfigStore|Casting\.skill_config" server agent client`：命中 server / agent schema / client 三侧协议与 store surface。
- `rg -n "pub skill_config|skill_config:" server/src/combat/components.rs server/src/network/client_request_handler.rs`：命中 `Casting.skill_config` 字段与 cast 构造点。
- `rg -n "zhenmai\.sever_chain|SkillConfigSchemaRegistry|SkillConfigSchemas::default|MeridianId::ALL" server client agent/packages/schema`：命中 server fixture、client fixture、schema sample 与已知功法注册。

### 遗留 / 后续

- 本 plan 只提供通用配置底盘和 `zhenmai.sever_chain` 契约 fixture；真实绝脉断链 gameplay、代价与 SEVERED 语义仍归后续 `plan-zhenmai-v2` / `plan-meridian-severed-v1`。
- `IntRange` / `FloatRange` client 控件未在本 plan 强制落地；当前 fixture 只需要 `Enum` / `MeridianId` / `Bool`。
- `npm ci` 曾报告 2 个 audit vulnerabilities；未执行 `npm audit fix`，避免越出本 plan 依赖范围。
