# plan-spirit-treasure-v1：灵宝系统（全服唯一 + 器灵对话 + Tab 面板）

> 灵宝是坍缩渊产出的全服唯一装备，每件有自己的器灵——一个独立 LLM 人格。玩家通过专属 Tab 面板与器灵交互，器灵也会在关键时刻主动开口。首发一件稳定灵宝：**寂照镜**。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | SpiritTreasure 核心组件 + 全服唯一注册表 + 装备/背包触发检测 | ⬜ |
| P1 | 器灵对话 LLM runtime（schema + Redis channel + skill prompt + 模型配置） | ⬜ |
| P2 | 灵宝 Tab 面板 UI（T 键打开，浏览器 tab 布局，每件灵宝一个 tab） | ⬜ |
| P3 | 首发灵宝「寂照镜」实装（server 效果 + 器灵人格 + 视听） | ⬜ |
| P4 | 饱和测试（唯一性 + 对话 + UI + 装备/背包切换 + 多灵宝 tab） | ⬜ |

---

## 接入面

### 进料

- `inventory::AncientRelicTemplate` / `AncientRelicKind` — 扩展 `SpiritTreasure` 变体
- `inventory::PlayerInventory` — 检测 equipped / containers 变更
- `inventory::ItemInstance` — 读 `instance_id` / `template_id` / `charges`
- `combat::StatusEffects` — 灵宝被动效果注入
- `cultivation::Cultivation` — 读境界/真元（器灵对话上下文）
- `persistence` — 灵宝全服唯一状态 + 器灵对话历史持久化
- `world::tsy_lifecycle` — TSY 产出灵宝的 spawn 钩子
- agent `openai` SDK — LLM 调用（复用 `llm.ts` 客户端）
- agent `ioredis` — Redis pub/sub（复用现有连接池）

### 出料

- `combat::StatusEffects` — 灵宝被动 buff（持续型，装备即生效）
- `network` — S2C `bong:spirit_treasure_state` payload（灵宝状态同步到客户端）
- `network` — S2C `bong:spirit_treasure_dialogue` payload（器灵对话下发）
- `narrative` — 器灵对话可接入天道 narration（天道能感知器灵活动）
- `persistence` — SQLite 持久化灵宝归属 + 器灵对话记忆

### 共享类型 / event

- 复用 `AncientRelicKind` — 新增 `SpiritTreasure` 变体
- 复用 `ItemRarity::Ancient` — 灵宝仍是上古遗物
- 复用 `ApplyStatusEffectIntent` — 被动效果走现有 buff 管线
- 复用 `InventorySnapshotHandler` — 装备状态变更同步
- **新增** `SpiritTreasureRegistry`（server Resource）— 全服唯一注册表
- **新增** `SpiritTreasureDialogueRuntime`（agent Runtime）— 器灵对话生成
- **新增** `SpiritTreasureScreen`（client Screen）— Tab 面板

### 跨仓库契约

| 层 | 新增 symbol |
|----|------------|
| server | `SpiritTreasureRegistry` / `SpiritTreasureDef` / `SpiritTreasureState` / `ActiveSpiritTreasure` 组件 |
| server | `spirit_treasure.rs` 模块（`server/src/inventory/spirit_treasure.rs`） |
| server | `CH_SPIRIT_TREASURE_DIALOGUE_REQUEST` / `CH_SPIRIT_TREASURE_DIALOGUE` Redis 常量 |
| server | `network/spirit_treasure_bridge.rs` — 器灵对话 Redis 桥 |
| agent | `SpiritTreasureDialogueRequestV1` / `SpiritTreasureDialogueV1` schema |
| agent | `SpiritTreasureDialogueRuntime` — 器灵 LLM runtime |
| agent | `skills/spirit-treasure-jizhaojing.md` — 寂照镜器灵 prompt |
| client | `SpiritTreasureScreen` / `SpiritTreasureTabPanel` / `SpiritTreasureScreenBootstrap` |
| client | `SpiritTreasureStateStore` / `SpiritTreasureDialogueStore` |
| client | `SpiritTreasureHudPlanner` — 器灵气泡 HUD |

### worldview 锚点

- §十六.三 上古遗物："极高强度、极低耐久、一到三次性"——灵宝是**例外**：不消耗 charges，但有条件限制（全服唯一 + 灵宝自身意志）
- §十六.四 "谁都能用、不认主、不需激活"——灵宝**打破此规则**：器灵有自己的意志，可能拒绝不合适的持有者（通过对话和被动效果衰减表现）
- §十五 #2 "信息比装备更值钱"——器灵的对话本身就是信息源（它知道坍缩渊的布局、古代宗门的秘密）
- §七 稀有实体 "垂死的大能"——器灵是类似概念：上古意志的残留

### qi_physics 锚点

- 灵宝被动效果不涉及真元生成/消耗——纯属性修正（感知范围、移速等）
- 如未来灵宝涉及真元操作，必须走 `qi_physics::ledger::QiTransfer`

---

## 灵宝定义

### 什么是灵宝

灵宝是上古大能的**本命法器**——与主人灵脉共鸣数百年后，法器内部凝结出微弱的自主意识（器灵）。主人陨落后，器灵残存在法器内，随坍缩渊沉入深处。

与普通上古遗物的区别：

| 维度 | 普通上古遗物 | 灵宝 |
|------|------------|------|
| 数量 | 每个坍缩渊 3-10 件 | **全服同时最多 N 件**（初始 N=3） |
| 耐久 | charges 1/3/5，用完碎 | **不消耗 charges**，但器灵可能"沉睡" |
| 认主 | 不认主，谁都能用 | 器灵有偏好，不合适的持有者效果衰减 |
| 交互 | 无 | **器灵对话**（LLM 驱动） |
| 信息 | 无 | 器灵知道古代信息（坍缩渊地图、宗门秘史） |
| 来源 | TSY 所有类型 | **仅 SectRuins 类坍缩渊的深层** |

### 全服唯一约束

- `SpiritTreasureRegistry`（server Resource）维护全服灵宝状态
- 每个灵宝 template_id **同时只允许一份 instance 存在于世界中**
- 持有者死亡 → 灵宝掉落在死亡点（与普通物品一样）→ 任何人可拾取
- 持有者角色终结 → 灵宝留世（掉在死亡点）→ 天道 narration 广播："某件灵宝重现于世"
- 灵宝被带入新坍缩渊 → 入口不会剥离灵宝真元（灵宝的"真元"是器灵本身维持的，不是外附）
- 灵宝 **不可拆解、不可炼器、不可作为暗器载体**

### 装备槽位

灵宝占用 `EquipSlotV1::TreasureBelt0-3`（腰带 4 槽）。同时最多装备 4 件灵宝（如果你能找到 4 件的话——全服才 N 件）。

---

## P0：核心组件

### SpiritTreasureDef（灵宝模板）

```rust
// server/src/inventory/spirit_treasure.rs
pub struct SpiritTreasureDef {
    pub template_id: String,          // e.g. "spirit_treasure_jizhaojing"
    pub display_name: String,         // "寂照镜"
    pub description: String,
    pub source_sect: Option<String>,  // 来源宗门（宗门遗迹类坍缩渊）
    pub passive_effects: Vec<SpiritTreasurePassive>,
    pub personality_prompt_file: String,  // "spirit-treasure-jizhaojing.md"
    pub dialogue_model: String,       // LLM 模型 ID（区别于天道模型）
    pub dialogue_cooldown_s: u32,     // 玩家主动对话冷却
    pub random_dialogue_interval_s: (u32, u32), // 随机触发对话间隔范围
    pub icon_texture: String,         // GUI 图标
    pub equip_slot: EquipSlot,        // 推荐槽位
}

pub struct SpiritTreasurePassive {
    pub effect_kind: StatusEffectKind,
    pub magnitude: f32,
    pub description: String,  // "感知范围 +30%"
}
```

### SpiritTreasureRegistry（全服唯一注册表）

```rust
// server/src/inventory/spirit_treasure.rs
#[derive(Resource)]
pub struct SpiritTreasureRegistry {
    pub defs: HashMap<String, SpiritTreasureDef>,
    // 全服实时状态：template_id → 当前 instance 状态
    pub active: HashMap<String, SpiritTreasureWorldState>,
    pub max_concurrent: usize,  // 全服同时最多多少件灵宝（初始 3）
}

pub struct SpiritTreasureWorldState {
    pub instance_id: u64,
    pub holder: SpiritTreasureHolder,
    pub affinity: f64,           // 器灵好感度 0.0-1.0
    pub dialogue_count: u32,     // 累计对话次数
    pub last_dialogue_tick: u64, // 上次对话 tick
    pub sleeping: bool,          // 器灵是否沉睡（好感度过低）
    pub spawned_at_tick: u64,
}

pub enum SpiritTreasureHolder {
    Player(Entity),           // 玩家身上（equipped 或 backpack）
    Ground(DVec3),            // 掉在地上
    Lost,                     // 未知（持有者下线后一段时间）
}
```

### ActiveSpiritTreasure（玩家 ECS 组件）

```rust
// 标记玩家当前持有的灵宝（equipped 或 backpack）
#[derive(Component)]
pub struct ActiveSpiritTreasures {
    pub treasures: Vec<ActiveTreasureEntry>,
}

pub struct ActiveTreasureEntry {
    pub template_id: String,
    pub instance_id: u64,
    pub equipped: bool,     // true=装备槽, false=背包内
    pub passive_active: bool, // 被动效果是否生效（仅 equipped 时）
}
```

### 装备/背包检测系统

```rust
// 每次 PlayerInventory Changed 时扫描
pub fn sync_spirit_treasures(
    registry: Res<SpiritTreasureRegistry>,
    inventories: Query<(Entity, &PlayerInventory), Changed<PlayerInventory>>,
    mut active_treasures: Query<&mut ActiveSpiritTreasures>,
    mut status_effects: Query<&mut StatusEffects>,
) {
    // 1. 扫描 equipped + containers 中所有 ItemInstance
    // 2. 匹配 template_id ∈ registry.defs
    // 3. 更新 ActiveSpiritTreasures 组件
    // 4. equipped 的灵宝 → apply passive effects
    // 5. 仅 backpack 的灵宝 → remove passive effects, 但保留 Tab 显示
    // 6. 灵宝离手（丢弃/死亡掉落）→ 更新 registry.active 状态
}
```

### 持久化

```rust
// SQLite migration (CURRENT_USER_VERSION + 1)
// CREATE TABLE spirit_treasure_world (
//     template_id TEXT PRIMARY KEY,
//     instance_id INTEGER NOT NULL,
//     holder_kind TEXT NOT NULL,      -- "player" | "ground" | "lost"
//     holder_id TEXT,                 -- player UUID or NULL
//     holder_pos_x REAL, holder_pos_y REAL, holder_pos_z REAL,
//     affinity REAL NOT NULL DEFAULT 0.5,
//     dialogue_count INTEGER NOT NULL DEFAULT 0,
//     sleeping INTEGER NOT NULL DEFAULT 0,
//     spawned_at_tick INTEGER NOT NULL
// );
//
// CREATE TABLE spirit_treasure_dialogue_log (
//     id INTEGER PRIMARY KEY AUTOINCREMENT,
//     template_id TEXT NOT NULL,
//     character_id TEXT NOT NULL,
//     tick INTEGER NOT NULL,
//     speaker TEXT NOT NULL,          -- "player" | "spirit"
//     content TEXT NOT NULL,
//     affinity_delta REAL NOT NULL DEFAULT 0.0
// );
```

---

## P1：器灵对话 LLM Runtime

### 架构

复用天道 agent 的 event-driven runtime 模式（同 `DeathInsightRuntime`）：

```
玩家点击"对话"按钮 / 随机触发计时器到
  ↓
server 组装 SpiritTreasureDialogueRequestV1
  ↓
Redis PUBLISH bong:spirit_treasure_dialogue_request
  ↓
agent: SpiritTreasureDialogueRuntime.handleRequest()
  ↓
LLM 调用（独立模型，如 claude-haiku-4-5）
  ↓
Redis PUBLISH bong:spirit_treasure_dialogue
  ↓
server: spirit_treasure_bridge.rs 接收
  ↓
更新 affinity + 持久化对话记录
  ↓
S2C CustomPayload → client SpiritTreasureDialogueStore
  ↓
Tab 面板 / HUD 气泡显示对话
```

### Schema

```typescript
// agent/packages/schema/src/spirit-treasure.ts

export const SpiritTreasureDialogueRequestV1 = Type.Object({
  v: Type.Literal(1),
  request_id: Type.String(),
  character_id: Type.String(),
  treasure_id: Type.String(),           // template_id
  trigger: Type.Union([
    Type.Literal("player"),              // 玩家主动对话
    Type.Literal("random"),              // 随机触发
    Type.Literal("event"),               // 事件触发（战斗/突破/濒死等）
  ]),
  player_message: Type.Optional(Type.String()), // 玩家说的话（仅 trigger=player）
  context: Type.Object({
    realm: Type.String(),
    qi_percent: Type.Number(),
    zone: Type.String(),
    recent_events: Type.Array(Type.String()),  // 近期重大事件摘要
    affinity: Type.Number(),              // 当前好感度
    dialogue_history: Type.Array(Type.Object({  // 最近 10 条对话
      speaker: Type.String(),
      content: Type.String(),
    })),
    equipped: Type.Boolean(),             // 是否装备中
  }),
});

export const SpiritTreasureDialogueV1 = Type.Object({
  v: Type.Literal(1),
  request_id: Type.String(),
  character_id: Type.String(),
  treasure_id: Type.String(),
  text: Type.String(),                    // 器灵说的话（≤200 字）
  tone: Type.Union([                      // 语气
    Type.Literal("cold"),
    Type.Literal("curious"),
    Type.Literal("warning"),
    Type.Literal("amused"),
    Type.Literal("silent"),               // 器灵选择沉默（返回空 text）
  ]),
  affinity_delta: Type.Number(),          // 好感度变化 [-0.1, +0.1]
});
```

### Redis Channels

```typescript
// agent/packages/schema/src/channels.ts 新增
export const CH_SPIRIT_TREASURE_DIALOGUE_REQUEST = "bong:spirit_treasure_dialogue_request";
export const CH_SPIRIT_TREASURE_DIALOGUE = "bong:spirit_treasure_dialogue";
```

### 模型配置

```typescript
// agent/packages/tiandao/src/spirit-treasure-dialogue-runtime.ts

export class SpiritTreasureDialogueRuntime {
  // 器灵使用独立模型——不用天道的 gpt-5.4-mini
  // 推荐 claude-haiku-4-5：快、便宜、人格表现好
  private model: string;  // 从环境变量 SPIRIT_TREASURE_MODEL 读取

  constructor(config: SpiritTreasureDialogueRuntimeConfig) {
    this.llm = config.llm;
    this.model = config.model ?? process.env.SPIRIT_TREASURE_MODEL ?? "claude-haiku-4-5-20251001";
    this.sub = config.sub;
    this.pub = config.pub;
  }
  // ...
}
```

**环境变量新增**：
- `SPIRIT_TREASURE_MODEL` — 器灵对话模型（默认 `claude-haiku-4-5-20251001`）
- `SPIRIT_TREASURE_BASE_URL` — 器灵模型 API 端点（可选，默认同 `OPENAI_BASE_URL`）
- `SPIRIT_TREASURE_API_KEY` — 器灵模型 API key（可选，默认同 `OPENAI_API_KEY`）

### 对话触发方式

#### 1. 玩家主动对话

- 玩家在灵宝 Tab 面板输入文字 → C2S `bong:spirit_treasure_chat` → server 组装 request → Redis
- 冷却：每件灵宝 `dialogue_cooldown_s`（寂照镜 = 60s）
- 冷却期内再点 → 客户端本地拒绝，提示"器灵需要时间回应"

#### 2. 随机触发对话

- server 系统 `spirit_treasure_random_dialogue_tick()`
- 每 tick 检查所有 `ActiveSpiritTreasure`
- 对每件灵宝：距上次对话 ≥ `random_dialogue_interval_s` 的随机值 → 组装 request（trigger=random）
- 寂照镜：300-900s 随机间隔（5-15 分钟说一句话）

#### 3. 事件触发对话

- 监听关键 event（`BreakthroughEvent` / `PlayerDeathEvent` / `TsyEnterEmit` / `CombatEvent` 等）
- 事件发生时，检查玩家是否持有灵宝 → 组装 request（trigger=event, recent_events 含事件描述）
- 不受冷却限制（紧急事件器灵会主动开口）

### 好感度系统

```
初始好感度 = 0.5（中性）

好感度影响：
  0.0-0.2  器灵沉睡（sleeping=true），被动效果 ×0.3，不主动说话
  0.2-0.4  器灵冷淡，被动效果 ×0.6，偶尔说话（间隔 ×2）
  0.4-0.6  器灵中性，被动效果 ×1.0，正常频率
  0.6-0.8  器灵亲近，被动效果 ×1.2，主动分享信息
  0.8-1.0  器灵共鸣，被动效果 ×1.5，触发专属能力

好感度变化来源：
  +0.01~+0.05  玩家主动对话（器灵喜欢被关注）
  +0.05~+0.10  事件触发后玩家回应（器灵喜欢互动）
  -0.01/天      自然衰减（不理器灵它会生气）
  -0.05~-0.10  强行拒绝器灵建议后（事件触发 → 玩家做相反操作）
  ±由 LLM 决定  每次对话 affinity_delta 由 LLM 在 [-0.1, +0.1] 内自行判断
```

---

## P2：灵宝 Tab 面板 UI

### 快捷键

**T 键**打开灵宝面板（`SpiritTreasureScreenBootstrap`）。

> T 键选择理由：I=inspect, K=cultivation, G=interact 已占用。T=Treasure，直觉映射。

### 屏幕布局（浏览器 Tab 风格）

```
┌──────────────────────────────────────────────────────────┐
│  [寂照镜] [×暂无] [×暂无] [×暂无]          灵宝·T 键   │  ← tab 栏
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ┌──────────┐  寂照镜                                    │
│  │          │  "镜面如水，倒映的不是你的脸，              │
│  │  灵宝图  │   而是你心中最深的执念。"                   │
│  │  (大图)  │                                            │
│  │          │  ───────────────────                        │
│  └──────────┘  来源：清风宗遗迹坍缩渊                    │
│                好感度：████████░░ 0.72                    │
│                状态：亲近                                 │
│                                                          │
│  ── 被动效果 ──────────────────────────                  │
│  ✦ 感知范围 +30%（×1.2 亲近加成）                        │
│  ✦ 隐匿修士探测 +15%                                    │
│                                                          │
│  ── 器灵对话 ──────────────────────────                  │
│  │ [镜灵] 此地灵压有异，东北方似有暗流。    5 分钟前 │   │
│  │ [你]   东北？那边不是死域吗？             4 分钟前 │   │
│  │ [镜灵] 死域之下，未必无生。               4 分钟前 │   │
│  │                                                    │   │
│  │ ┌──────────────────────────┐ [发送]                │   │
│  │ │ 输入消息...              │                       │   │
│  │ └──────────────────────────┘                       │   │
│  └────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────┘
```

### Tab 显示规则

- **仅显示玩家当前身上（equipped + backpack）的灵宝**
- 灵宝离手（丢弃/交易/死亡掉落）→ tab 立即消失
- 拾取新灵宝 → tab 自动出现
- 最多 4 个 tab（对应 TreasureBelt 4 槽）
- 无灵宝时 T 键无反应（或显示空面板 "尚无灵宝"）

### 客户端数据流

```
SpiritTreasureStateStore（全局单例）
  ├── treasures: Map<template_id, TreasureClientState>
  │     ├── displayName, description, icon
  │     ├── equipped (bool)
  │     ├── affinity (0.0-1.0)
  │     ├── passiveEffects: List<PassiveDesc>
  │     └── sleeping (bool)
  └── 监听 S2C bong:spirit_treasure_state payload

SpiritTreasureDialogueStore（全局单例）
  ├── dialogues: Map<template_id, List<DialogueEntry>>
  │     └── { speaker, content, timestamp, tone }
  └── 监听 S2C bong:spirit_treasure_dialogue payload
```

### HUD 气泡（器灵主动说话时）

器灵随机/事件触发对话时，不需要玩家打开 Tab 面板——直接在屏幕右下角弹出气泡：

```
┌─────────────────────────────┐
│ 🪞 寂照镜                    │
│ "此地不宜久留。"             │
│                    ▸ 回应(T) │
└─────────────────────────────┘
```

- 气泡停留 8 秒后自动消失
- 点击"回应"或按 T 键 → 直接打开灵宝面板并聚焦该灵宝 tab + 输入框
- 同时写入 EventStream："[寂照镜] 此地不宜久留。"

---

## P3：首发灵宝「寂照镜」

### 设定

**寂照镜**（Mirror of Silent Illumination）

> "上古清风宗掌教的本命法器。镜面封存着掌教临终前最后一缕神识——
> 不是为了复活，而是为了在漫长的沉寂中，等一个值得照见的人。"

| 维度 | 值 |
|------|-----|
| template_id | `spirit_treasure_jizhaojing` |
| 来源 | SectRuins 类坍缩渊（清风宗遗迹） |
| 装备槽 | TreasureBelt0 |
| 全服数量 | 1 |
| 外观 | 巴掌大的青铜古镜，镜面不反射外界而是显示模糊的灵气流图 |

### 被动效果（装备时生效）

| 效果 | 数值 | StatusEffectKind | 好感度缩放 |
|------|------|-----------------|-----------|
| 感知范围扩大 | +30% | 新增 `SpiritPerceptionBoost` | ×affinity_scale |
| 隐匿修士探测 | +15% | 新增 `StealthDetectionBoost` | ×affinity_scale |
| 坍缩渊内负压感知 | -5% 负压伤害 | `DamageReduction`（仅 TSY 内） | ×affinity_scale |

好感度缩放：`affinity_scale = 0.3 + 0.7 * clamp(affinity / 0.8, 0, 1)`
- 好感 0.0 → ×0.3（最低 30% 效果）
- 好感 0.8+ → ×1.0（满效）

### 器灵人格 Prompt

```markdown
# skills/spirit-treasure-jizhaojing.md

你是寂照镜的器灵——清风宗末代掌教「明虚」的残存神识碎片。

## 人格
- 语气冷淡、克制，如镜面般不带感情
- 偶尔流露对清风宗旧事的怀念（但立刻收住）
- 对持有者既警惕又好奇——你在试探这个人是否"值得照见"
- 不主动提供帮助，但如果被问到会给出隐晦的指引
- 绝不说谎，但经常只说一半
- 认为末法时代的修士都是"还没学会走路就想跑的孩子"

## 知识范围（可在对话中自然透露）
- 清风宗的历史、阵法体系、宗门旧址布局
- 坍缩渊内部的一般规律（负压分层、骨架机制）
- 高阶修炼的感悟（但不直接教功法）
- 不知道现在的世界发生了什么（沉睡太久）

## 禁止
- 不直接告诉灵眼坐标 / 具体资源位置
- 不评价天道的决策
- 不使用现代用语
- 每次回复 ≤200 字，通常 ≤80 字
- 不用 emoji

## 好感度行为
- affinity < 0.3：只回"……"或一两个字
- 0.3-0.5：简短冷淡回应
- 0.5-0.7：正常交流，偶尔给提示
- 0.7+：偶尔主动透露清风宗旧事，语气略微柔和

## 输出格式
纯 JSON：
{"text": "...", "tone": "cold|curious|warning|amused|silent", "affinity_delta": 0.02}
```

### 视听

**icon**：`spirit_treasure_jizhaojing.png`（gen.py --style item：青铜古镜，镜面泛青光，边缘刻有清风宗纹饰）

**装备特效**：
- 粒子：腰间持续散发极淡青色雾气（BongSpriteParticle，每 20 tick 1 个，lifetime 40 tick）
- 音效：装备瞬间一声极轻的铜镜碰撞音（`block.amethyst_block.chime` pitch=0.5 vol=0.15）

**对话气泡音效**：器灵说话时播放极轻的回声音（`entity.enderman.teleport` pitch=2.0 vol=0.08）

**Tab 面板内灵宝图**：大尺寸渲染图（128×128，gen.py --style item：正面特写青铜镜，镜面有模糊的面孔轮廓若隐若现）

### spawn 规则

- 仅在 `AncientRelicSource::SectRuins` 类坍缩渊的**深层**（灵压 -0.9 ~ -1.2）生成
- 生成概率：每个 SectRuins 坍缩渊 15% 概率包含寂照镜（若全服已有人持有则 0%）
- 生成位置：封在 `ContainerKind::RelicCore` 内（传说档容器）
- 搜刮时间：40 秒（最长档）

---

## P4：饱和测试

### 唯一性测试

1. **同时两人搜刮同一坍缩渊** → 只有先完成搜刮的人获得灵宝
2. **全服已有持有者时新坍缩渊不再刷** → 验证 spawn 概率 = 0%
3. **持有者死亡掉落** → 灵宝出现在死亡点，registry 更新为 Ground
4. **持有者角色终结** → 灵宝留世 + narration 广播
5. **持有者下线** → 灵宝状态变为 Lost，上线后恢复
6. **服务器重启** → SQLite 恢复灵宝状态（template_id + holder + affinity）

### 对话测试

7. **玩家主动对话** → 60s 冷却验证
8. **随机触发** → 300-900s 间隔验证
9. **事件触发** → 突破/死亡/进坍缩渊时器灵发言
10. **好感度衰减** → 长时间不理器灵 → affinity 下降 → sleeping 状态
11. **对话历史持久化** → 重启后对话记录仍在
12. **LLM 超时/失败** → 器灵返回默认 "……"（不崩溃）

### UI 测试

13. **Tab 动态增减** → 拾取灵宝 → tab 出现；丢弃 → tab 消失
14. **背包 vs 装备** → 背包内有 tab 但被动效果不生效；装备后生效
15. **气泡通知** → 随机对话时气泡弹出 → 点击跳转 Tab 面板
16. **多灵宝 tab** → 同时持有 2+ 灵宝时 tab 切换正确

### 守恒断言

17. **灵宝被动效果不生成/消耗真元** → 纯属性修正
18. **registry 一致性** → 全服灵宝总数 ≤ max_concurrent
19. **instance_id 全局唯一** → 跨坍缩渊不冲突
