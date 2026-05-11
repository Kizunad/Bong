# plan-spirit-treasure-v1：灵宝系统（全服唯一 + 器灵聊天对话 + 附近可见）

> 灵宝是坍缩渊产出的全服唯一装备，每件有自己的器灵——一个独立 LLM 人格。玩家在聊天栏输入 `@灵宝名` 与器灵对话，器灵的回话**附近所有人都能看到**——你的灵宝暴露了你拥有它的事实。首发一件稳定灵宝：**寂照镜**。

## 阶段总览

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | SpiritTreasure 核心组件 + 全服唯一注册表 + 装备/背包触发检测 | ⬜ |
| P1 | 器灵对话 LLM runtime（schema + Redis channel + skill prompt + 模型配置） | ⬜ |
| P2a | 聊天栏 `@灵宝名` 路由 + 器灵回话 zone 广播 + 附近可见渲染 | ⬜ |
| P2b | 灵宝专属面板 UI（T 键，owo-lib Tab 页，每件灵宝独立 tab 设计） | ⬜ |
| P3 | 首发灵宝「寂照镜」实装（server 效果 + 器灵人格 + 视听） | ⬜ |
| P4 | 饱和测试（唯一性 + 对话 + 聊天可见性 + 装备/背包切换） | ⬜ |

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
| server | `network/chat_collector.rs` — 扩展 `@灵宝名` 路由（拦截 → 走器灵管线，不走天道 player_chat） |
| agent | `SpiritTreasureDialogueRequestV1` / `SpiritTreasureDialogueV1` schema |
| agent | `SpiritTreasureDialogueRuntime` — 器灵 LLM runtime |
| agent | `skills/spirit-treasure-jizhaojing.md` — 寂照镜器灵 prompt |
| client | `SpiritTreasureChatRenderer` — 聊天栏器灵消息特殊渲染（颜色 + 名字格式） |
| client | `SpiritTreasureScreen` / `SpiritTreasureScreenBootstrap` — T 键打开，owo-lib Tab 面板 |
| client | `SpiritTreasureTabPanel`（抽象类）→ `JiZhaoJingTabPanel` 子类 |
| client | `JiZhaoJingMirrorRenderer` — 64×64 镜面动态渲染组件（涟漪 + 好感度渐变） |
| client | `SpiritTreasureStateStore` / `SpiritTreasureDialogueStore` — 灵宝状态 + 对话记忆 |

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

#### 1. 玩家主动对话（`@灵宝名`）

- 玩家在聊天栏输入 `@寂照镜 你好` → chat_collector.rs 拦截 → 组装 request → Redis → agent → 回话
- **玩家的 @消息先 zone 广播**（附近所有人看到你在跟灵宝说话）
- **器灵的回话再 zone 广播**（附近所有人看到器灵说了什么）
- 冷却：每件灵宝 `dialogue_cooldown_s`（寂照镜 = 30s）
- 冷却期内再 @ → server 回 "§8[灵宝] §7器灵尚在回应中。"（仅本人可见）
- 空消息（只写 `@寂照镜`）→ 器灵自行决定说什么（trigger=player, player_message=""）

#### 2. 随机触发对话

- server 系统 `spirit_treasure_random_dialogue_tick()`
- 每 tick 检查所有 `ActiveSpiritTreasure`
- 对每件灵宝：距上次对话 ≥ `random_dialogue_interval_s` 的随机值 → 组装 request（trigger=random）
- 寂照镜：300-900s 随机间隔（5-15 分钟说一句话）
- 器灵的话直接出现在聊天栏，**附近所有人可见**——旁人会看到一个修士腰间的青铜镜突然"说话"了
- 器灵也可能返回 tone=silent → 不发消息（它选择了沉默）

#### 3. 事件触发对话

- 监听关键 event（`BreakthroughEvent` / `PlayerDeathEvent` / `TsyEnterEmit` / `CombatEvent` 等）
- 事件发生时，检查玩家是否持有灵宝 → 组装 request（trigger=event, recent_events 含事件描述）
- 不受冷却限制（紧急事件器灵会主动开口）
- **事件触发的对话也 zone 广播**——你在渡劫时灵宝突然说"小心东北方"，旁观者都听得到

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

## P2：聊天栏 `@灵宝名` 路由 + 附近可见

### 交互方式

**玩家在聊天栏输入 `@寂照镜 东北方向有什么？`**——不需要专属 UI，直接用 MC 原生聊天框。

器灵的回话以特殊格式出现在聊天栏，**同 zone 内所有玩家都能看到**。

### 聊天流程

```
玩家输入：@寂照镜 东北方向有什么？
  ↓
client → C2S chat packet（原生 MC 聊天）
  ↓
server chat_collector.rs：
  检测 "@" 前缀 → 解析灵宝名
  ├── 匹配失败 → 当普通聊天处理（RPUSH bong:player_chat）
  └── 匹配成功 → 拦截，不进 player_chat
       ├── 检查玩家是否持有该灵宝（equipped 或 backpack）
       │   └── 未持有 → 聊天栏回："你并未持有此物。"（仅本人可见）
       ├── 检查冷却 → 冷却中 → "器灵尚在回应中。"（仅本人可见）
       └── 通过 → 组装 SpiritTreasureDialogueRequestV1
            ├── 玩家的 @消息 先广播给同 zone 玩家（所有人看到你在跟灵宝说话）
            ├── Redis PUBLISH bong:spirit_treasure_dialogue_request
            └── agent runtime 处理 → Redis PUBLISH bong:spirit_treasure_dialogue
                 ↓
            server spirit_treasure_bridge.rs 接收
                 ↓
            格式化为聊天消息，scope=zone 广播
                 ↓
            同 zone 所有玩家聊天栏看到器灵回话
```

### 聊天栏渲染格式

**玩家对灵宝说话**（所有人可见）：
```
§7[散修] §f@寂照镜 §7东北方向有什么？
```
- 灰色玩家名 + 白色 @灵宝名 + 灰色消息内容
- 与普通聊天区别：`@灵宝名` 部分高亮

**器灵回话**（同 zone 所有人可见）：
```
§b[寂照镜] §3死域之下，未必无生。
```
- 青色 `§b` 灵宝名 + 深青 `§3` 消息内容
- 与天道 narration（`§7` 灰色）、普通聊天（`§f` 白色）明确区分

**器灵主动说话**（随机/事件触发，同 zone 所有人可见）：
```
§b[寂照镜] §3此地灵压有异。
```
- 与玩家触发的格式完全一样——旁人分不清是玩家问的还是器灵主动说的

**仅本人可见的系统提示**（错误/冷却等）：
```
§8[灵宝] §7你并未持有此物。
§8[灵宝] §7器灵尚在回应中，稍后再试。
§8[灵宝] §7器灵沉睡中，未有回应。
```

### 信息暴露的战术含义

> 这是**故意设计**——与灵宝对话暴露以下信息给附近所有人：
>
> 1. **你持有灵宝**——全服唯一物品，相当于告诉所有人"我身上有好东西"
> 2. **灵宝名字**——暴露了灵宝类型和大致能力
> 3. **对话内容**——器灵说的信息（坍缩渊线索、宗门秘史等）附近人也能听到
>
> 这与 worldview §十一 匿名系统和 §十五 #2 "信息比装备更值钱"完全一致——你要么冒着暴露的风险跟器灵交流获取信息，要么保持沉默让器灵好感度慢慢下降。
>
> **聪明的持有者会跑到荒野无人处才跟灵宝说话。**

### `@` 路由实现

```rust
// server/src/network/chat_collector.rs — 扩展

const SPIRIT_TREASURE_PREFIX: &str = "@";

pub fn try_route_spirit_treasure_chat(
    raw: &str,
    sender: Entity,
    registry: &SpiritTreasureRegistry,
    active_treasures: &ActiveSpiritTreasures,
) -> Option<SpiritTreasureChatRoute> {
    // 1. trim + 检查 "@" 前缀
    let trimmed = raw.trim();
    if !trimmed.starts_with('@') { return None; }
    
    // 2. 按第一个空格分割：灵宝名 + 消息内容
    let after_at = &trimmed[1..];  // 去掉 @
    let (treasure_name, message) = match after_at.find(' ') {
        Some(idx) => (&after_at[..idx], after_at[idx+1..].trim()),
        None => (after_at, ""),  // 只 @了名字没说话 → 空消息
    };
    
    // 3. 在 registry.defs 中按 display_name 匹配
    let def = registry.defs.values()
        .find(|d| d.display_name == treasure_name)?;
    
    // 4. 检查玩家是否持有
    let entry = active_treasures.treasures.iter()
        .find(|t| t.template_id == def.template_id)?;
    
    Some(SpiritTreasureChatRoute {
        treasure_template_id: def.template_id.clone(),
        player_message: message.to_string(),
        equipped: entry.equipped,
    })
}
```

### 客户端数据

```
SpiritTreasureStateStore（全局单例，无独立 UI）
  ├── treasures: Map<template_id, TreasureClientState>
  │     ├── displayName, equipped, affinity, sleeping
  │     └── passiveEffects（在 inspect 装备栏 tooltip 内展示）
  └── 监听 S2C bong:spirit_treasure_state payload

器灵对话直接走聊天栏，不需要 DialogueStore。
```

---

## P2b：灵宝专属面板 UI

### 架构

**T 键**打开灵宝面板（`SpiritTreasureScreenBootstrap`）。面板是纯**查看 + 状态展示**用途，**对话仍走聊天栏 @**。

- 每件灵宝一个 tab，tab 设计**因灵宝而异**（不同灵宝有不同布局/视觉主题）
- 无灵宝时 T 键弹出空面板："天地间无灵宝与你相伴。"
- 灵宝离手 → tab 消失；拾取 → tab 出现

### 通用面板框架

```java
// client/src/main/java/com/bong/client/spirittreasure/SpiritTreasureScreen.java
public class SpiritTreasureScreen extends BaseOwoScreen<FlowLayout> {

    private static final int PANEL_W = 280;
    private static final int PANEL_H = 220;
    private static final int TAB_ACTIVE = 0xFFD4C8A0;   // 暖金（灵宝通用）
    private static final int TAB_INACTIVE = 0xFF555555;
    private static final int BG_OUTER = 0xFF0E0E12;     // 深墨底
    private static final int BG_INNER = 0xFF1A1820;     // 内容区
    private static final int BORDER = 0xFF3A3540;       // 淡紫灰边框

    private int activeTab = 0;
    private final List<SpiritTreasureTabPanel> tabPanels = new ArrayList<>();

    @Override
    protected OwoUIAdapter<FlowLayout> createAdapter() {
        return OwoUIAdapter.create(this, Containers::verticalFlow);
    }

    @Override
    protected void build(FlowLayout root) {
        root.surface(Surface.VANILLA_TRANSLUCENT);
        root.horizontalAlignment(HorizontalAlignment.CENTER);
        root.verticalAlignment(VerticalAlignment.CENTER);

        FlowLayout outer = Containers.verticalFlow(Sizing.fixed(PANEL_W), Sizing.fixed(PANEL_H));
        outer.surface(Surface.flat(BG_OUTER).and(Surface.outline(BORDER)));
        outer.padding(Insets.of(2));

        // ── tab 栏 ──
        FlowLayout tabBar = buildTabBar();
        outer.child(tabBar);

        // ── 内容区（每个 tab 是一个 SpiritTreasureTabPanel 子类）──
        for (SpiritTreasureTabPanel panel : tabPanels) {
            outer.child(panel.root());
        }

        root.child(outer);
        switchTab(0);
    }

    private void switchTab(int idx) {
        activeTab = idx;
        for (int i = 0; i < tabPanels.size(); i++) {
            tabPanels.get(i).root().positioning(
                i == idx ? Positioning.layout() : Positioning.absolute(-9999, -9999)
            );
        }
    }
}
```

每件灵宝实现自己的 `SpiritTreasureTabPanel` 子类，决定内部布局、色调、动态渲染。

### 寂照镜 Tab 设计

**视觉主题**：冷青铜色调 + 镜面反射意象。与 inspect 的暖绿色、cultivation 的暗金色形成区分。

**配色常量**：
```java
// JiZhaoJingTabPanel.java
static final int MIRROR_SURFACE = 0xFF1C2830;    // 镜面深青（内容底色）
static final int MIRROR_BORDER = 0xFF3A6878;     // 青铜边框
static final int MIRROR_TEXT = 0xFFA0C8D0;        // 青白文字
static final int MIRROR_DIM = 0xFF506068;         // 暗青文字（次要信息）
static final int MIRROR_ACCENT = 0xFF70B8C8;      // 亮青强调
static final int AFFINITY_FILL = 0xFF4090A0;      // 好感度条填充
static final int AFFINITY_TRACK = 0xFF202828;     // 好感度条轨道
```

**布局**（280×200 内容区）：

```
┌─────────────────────────────────────────────────┐
│ [寂照镜]                          灵宝 · T 键   │  tab 栏（暖金文字）
├─────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────┐│
│ │                 MIRROR_SURFACE               ││
│ │  ┌────────┐                                  ││
│ │  │ 镜面   │  寂照镜              §ACCENT     ││
│ │  │ 动态   │  清风宗末代掌教遗器   §DIM       ││
│ │  │ 渲染   │                                  ││
│ │  │ 64×64  │  器灵：明虚           §TEXT      ││
│ │  │        │  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄              ││
│ │  └────────┘  好感 ███████░░░ 0.72  §ACCENT   ││
│ │              亲近                  §DIM       ││
│ │                                              ││
│ │  ─── 镜中所映 ───────────── §MIRROR_BORDER   ││
│ │  ✦ 感知 +30%  (×1.2)              §TEXT      ││
│ │  ✦ 匿探 +15%  (×1.2)              §TEXT      ││
│ │  ✦ 负压 -5%   (×1.2)              §TEXT      ││
│ │                                              ││
│ │  ─── 镜语 ─────────────── §MIRROR_BORDER     ││
│ │  "此地灵压有异。"         2 分钟前  §DIM      ││
│ │  "……"                    15 分钟前  §DIM      ││
│ │                                              ││
│ │  §DIM 聊天栏 @寂照镜 交谈 · 附近可闻         ││
│ └──────────────────────────────────────────────┘│
└─────────────────────────────────────────────────┘
```

### 寂照镜 Tab 各区块详解

#### 1. 镜面动态渲染（左上 64×64）

不是静态图标——是一个 **DrawContext 自绘区域**，模拟镜面效果：

```java
// JiZhaoJingMirrorRenderer.java（自定义 owo Component）
public class JiZhaoJingMirrorRenderer extends BaseComponent {

    @Override
    public void draw(OwoUIDrawContext ctx, int mouseX, int mouseY,
                     float partialTicks, float delta) {
        int x = this.x, y = this.y;

        // 1. 镜框：青铜色 2px 边框
        ctx.fill(x, y, x+64, y+64, MIRROR_BORDER);
        ctx.fill(x+2, y+2, x+62, y+62, MIRROR_SURFACE);

        // 2. 镜面底纹：根据 affinity 渐变
        //    低好感 = 深灰混沌；高好感 = 浅青清明
        int innerColor = lerpColor(0xFF151A1E, 0xFF2A4550, affinity);
        ctx.fill(x+3, y+3, x+61, y+61, innerColor);

        // 3. 灵气涟漪：从中心向外扩散的同心圆
        //    半径随 tick 脉动（2s 周期），颜色 MIRROR_ACCENT 半透明
        float phase = (System.currentTimeMillis() % 2000) / 2000f;
        float radius = 8f + phase * 20f;
        int rippleAlpha = (int)(60 * (1f - phase));  // 越远越淡
        drawRipple(ctx, x+32, y+32, radius, withAlpha(MIRROR_ACCENT, rippleAlpha));

        // 4. 器灵沉睡时：镜面覆盖灰色半透明遮罩 + "zzz" 文字
        if (sleeping) {
            ctx.fill(x+3, y+3, x+61, y+61, 0x80000000);
            ctx.drawTextWithShadow(tr, Text.literal("沉"), x+25, y+26, 0xFF404040);
        }

        // 5. 镜框角落：4 个青铜铆钉小方块（3×3）
        for (int[] corner : new int[][]{{x+1,y+1},{x+60,y+1},{x+1,y+60},{x+60,y+60}}) {
            ctx.fill(corner[0], corner[1], corner[0]+3, corner[1]+3, 0xFF5A8898);
        }
    }
}
```

**关键**：镜面不是贴图——是**每帧实时渲染的动态 canvas**。涟漪脉动节奏随好感度变化（高好感 = 平稳慢波；低好感 = 急促碎波）。这让每个玩家的寂照镜看起来都不一样。

#### 2. 好感度条

```java
// 水平条 120×6px，位于灵宝名下方
FlowLayout affinityBar = Containers.horizontalFlow(Sizing.fixed(120), Sizing.fixed(6));
// 轨道底色
affinityBar.surface(Surface.flat(AFFINITY_TRACK));
// 填充色叠加（宽度 = 120 * affinity）
FlowLayout fill = Containers.horizontalFlow(Sizing.fixed((int)(120 * affinity)), Sizing.fixed(6));
fill.surface(Surface.flat(AFFINITY_FILL));
affinityBar.child(fill);
```

右侧文字 `0.72` 用 `MIRROR_ACCENT` 色，下方小字"亲近"用 `MIRROR_DIM` 色。

好感度分档文字：
| 范围 | 文字 | 颜色 |
|------|------|------|
| 0.0-0.2 | 沉睡 | 0xFF404040（灰） |
| 0.2-0.4 | 冷淡 | 0xFF607080 |
| 0.4-0.6 | 中性 | MIRROR_DIM |
| 0.6-0.8 | 亲近 | MIRROR_ACCENT |
| 0.8-1.0 | 共鸣 | 0xFFB0E8F0（亮青白） |

#### 3. "镜中所映"——被动效果区

```java
// 分隔线：1px MIRROR_BORDER + 左侧"镜中所映"文字
FlowLayout effectSection = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
effectSection.child(Components.label(Text.literal("─── 镜中所映 ───")).color(Color.ofArgb(MIRROR_BORDER)));

for (PassiveEffect e : passiveEffects) {
    String line = "✦ " + e.name + " " + formatPercent(e.value)
        + "  (" + formatScale(affinityScale) + ")";
    effectSection.child(Components.label(Text.literal(line)).color(Color.ofArgb(MIRROR_TEXT)));
}
```

亲近加成倍率用**小字括号**标注。倍率 < 1.0 时颜色变橙色（0xFFD09030），提示好感度不够。

#### 4. "镜语"——最近对话摘要

显示器灵最近 3 条发言（从 `SpiritTreasureDialogueStore` 读取）：

```java
FlowLayout dialogueSection = Containers.verticalFlow(Sizing.fill(100), Sizing.content());
dialogueSection.child(Components.label(Text.literal("─── 镜语 ───")).color(Color.ofArgb(MIRROR_BORDER)));

for (DialogueEntry d : recentDialogues.subList(0, Math.min(3, recentDialogues.size()))) {
    String timeAgo = formatTimeAgo(d.ticksAgo);
    FlowLayout row = Containers.horizontalFlow(Sizing.fill(100), Sizing.content());
    row.child(Components.label(Text.literal(""" + d.text + """))
        .color(Color.ofArgb(MIRROR_TEXT))
        .sizing(Sizing.fill(70)));
    row.child(Components.label(Text.literal(timeAgo))
        .color(Color.ofArgb(MIRROR_DIM))
        .sizing(Sizing.fill(30)));
    dialogueSection.child(row);
}
```

#### 5. 底部提示

一行 `MIRROR_DIM` 色小字：`聊天栏 @寂照镜 交谈 · 附近可闻`——提醒玩家对话走聊天栏。

### 未来灵宝的 Tab 差异化

每件灵宝的 Tab **视觉主题完全不同**——寂照镜是冷青铜，下一件可能是：
- 火焰主题（红橙色调 + 火焰粒子渲染）
- 骨刻主题（骨白灰 + 裂纹纹理）
- 水墨主题（黑白 + 墨滴扩散动画）

通过 `SpiritTreasureTabPanel` 抽象类统一接口，每件灵宝继承后自定义 `buildContent()` + `getColorScheme()`。

### 数据刷新

```java
// build() 内注册 listener
stateListener = (state) -> {
    // 好感度条 / 被动效果倍率 / 沉睡状态刷新
    updateAffinityBar(state.affinity);
    updatePassiveEffects(state);
    mirrorRenderer.setAffinity(state.affinity);
    mirrorRenderer.setSleeping(state.sleeping);
};
SpiritTreasureStateStore.addListener(stateListener);

dialogueListener = (dialogues) -> {
    updateRecentDialogues(dialogues);
};
SpiritTreasureDialogueStore.addListener(dialogueListener);

// removed() 内清理
@Override
public void removed() {
    SpiritTreasureStateStore.removeListener(stateListener);
    SpiritTreasureDialogueStore.removeListener(dialogueListener);
    super.removed();
}
```

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

### 聊天 / 可见性测试

13. **@ 路由解析** → `@寂照镜 你好` 正确拦截；`@不存在 你好` 当普通聊天处理；`@寂照镜`（空消息）正常触发
14. **zone 广播** → 持有者 @灵宝 → 同 zone 另一玩家聊天栏看到玩家消息 + 器灵回话
15. **跨 zone 不可见** → 不同 zone 的玩家看不到器灵对话
16. **未持有拦截** → 玩家 @了别人持有的灵宝名 → 仅本人可见 "你并未持有此物"
17. **随机触发 zone 广播** → 器灵主动说话时附近玩家均可见
18. **渲染格式** → 器灵消息 §b[灵宝名] §3 与天道 narration / 普通聊天色调区分
19. **inspect tooltip** → 装备栏长按灵宝 → 正确显示好感度/被动效果/使用说明

### 面板 UI 测试

20. **T 键打开** → 持有灵宝时 T 键打开面板；无灵宝时显示空态文字
21. **Tab 动态增减** → 拾取灵宝 → tab 出现；丢弃 → tab 消失；多件灵宝多 tab
22. **镜面渲染** → 涟漪动画每帧刷新无闪烁；好感度变化时镜面颜色平滑过渡
23. **好感度条** → 实时反映 affinity 变化；分档文字（沉睡/冷淡/中性/亲近/共鸣）正确切换
24. **被动效果倍率** → 好感度变化后括号内倍率实时更新；< 1.0 时变橙色
25. **镜语区** → 最近 3 条对话正确显示 + 时间戳
26. **沉睡遮罩** → 好感度 < 0.2 时镜面覆盖灰色遮罩 + "沉"字
27. **Store 清理** → 关闭面板后 listener 正确 remove，无内存泄漏

### 守恒断言

17. **灵宝被动效果不生成/消耗真元** → 纯属性修正
18. **registry 一致性** → 全服灵宝总数 ≤ max_concurrent
19. **instance_id 全局唯一** → 跨坍缩渊不冲突
