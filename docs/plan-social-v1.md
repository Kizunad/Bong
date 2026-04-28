# Bong · plan-social-v1

**社交 / 关系 / 声名 / 匿名 / 灵龛专项**。HUD social channel 只给聊天，本 plan 定义匿名社会下的稀缺关系结构、灵龛机制、声名累积、暴露/隐藏规则。

**世界观锚点**：`worldview.md §一` line 4 "**没有宗门收你**"；`worldview.md §十一` "玩家之间默认敌对的陌生人"、修士之间**默认不显示名字**。本 plan **不得引入"正道/魔道阵营"、"掌门/长老/内门/外门弟子"等传统宗门层级**——末法残土是匿名利己的原子化社会。

**交叉引用**：`plan-npc-ai-v1.md §4`（派系是 NPC 内部，玩家侧极简）· `plan-death-lifecycle-v1.md §5`（生平卷/亡者博物馆公开）· `plan-persistence-v1.md §1`（RelationshipStore 落盘）· `plan-HUD-v1.md §2.3`（聊天 / 匿名显示）· `worldview.md §一/§十一/§十二`。

---

## §-1 现状（已实装，本 plan 不重做）

| 层 | 已有 | 文件 |
|---|---|---|
| **聊天采集** | `chat_collector.rs` 采集玩家聊天 → Redis `bong:player_chat` | server |
| **ChatMessageV1 schema** | `{ player, zone, raw, ts }` | `server/src/schema/chat_message.ts` |
| **Agent 聊天处理** | tiandao `chat-processor.ts` 读 chat 并做 intent/sentiment 分析 | agent |
| **query-player 工具** | agent 可读生平卷 | agent |

**尚未实装**（本 plan 工作范围）：
- 关系图（师承/同行/盟约/死仇）
- 灵龛机制（block / 保护 / 复活点 / 龛石）
- 匿名渲染（client 端不显示玩家名、境界模糊显示）
- 暴露触发（聊天/交易/天道点名/死亡）
- 声名累积（个人名号，非阵营声望）
- 玩家对 NPC 派系的加入/退出（极简版）

---

## §0 设计轴心

- [ ] **默认匿名 + 默认敌对**——不主动暴露任何玩家间身份信息
- [ ] **关系是稀缺品**：建立成本高（需要互动时间/事件触发）、暴露代价高（一旦暴露全服可查生平卷）
- [ ] **不做 MMO 公会 / 不做正魔阵营**——世界只有 NPC 派系（plan-npc-ai §4），玩家仅可"挂靠"
- [ ] **师承可遇不可求**：NPC 散修偶尔传一招，不做"拜师流程 UI"
- [ ] **灵龛不是安全区**：仅一个私有点，暴露即失效
- [ ] **声名 = 行为累积**，不是阵营值——你做什么，你就是什么

## §1 匿名系统

### §1.1 默认显示

- [ ] 其他玩家头顶**无名字**，仅显示人形 + 简化外观
- [ ] 外观按境界段分档（醒灵/引气看不出差异；凝脉/固元可感知；通灵/化虚有明显异象）
- [ ] 固元+ 玩家对他人可感知**模糊气息**（"此人气息在你之上 / 微弱 / 相当"），非精确境界

### §1.2 暴露触发（四种）

| 触发 | 暴露对象 | 暴露范围 |
|------|---------|---------|
| 主动聊天 | 发言者 | 同频道半径 50 格内玩家 |
| 交易 | 双方 | 仅交易对象 |
| 天道点名 | 被点名者 | 全服（narration / 渡劫广播 / 化虚公示） |
| 死亡 | 死者 | 杀手 + 50 格内观察者 |

- [ ] 暴露记录进 `ExposureLog`（生平卷字段）：时间戳 + 触发类型 + 见证者 CharId 列表
- [ ] 暴露不可撤销（与 plan-death §5 "不可篡改"对齐）
- [ ] 化名系统**不做**——"伪装身份"会破坏匿名的紧张感

### §1.3 终结后的全开放

- [ ] 角色终结 → 生平卷进亡者博物馆，**姓名随之公开**（library-web 可搜）
- [ ] 活着的玩家仍匿名

## §2 灵龛（私有藏点）

### §2.1 基本规则

- [ ] 每个玩家**出生自带一次**「龛石」（消耗性道具，放置后消失）
- [ ] 放置即设置灵龛坐标；每个玩家同时只能有**一个**灵龛
- [ ] 二次放置需**消耗新龛石**（稀有掉落 / 长时间苟修任务奖励），旧灵龛废除

### §2.2 效果

- [ ] 半径 5 格内：
  - NPC 不主动攻击（战斗中的 NPC 追进来仍可打）
  - 其他玩家**无法破坏方块**（但可进入、可攻击灵龛所有者）
  - **不提供灵气**（灵龛内不能修炼）
  - 死亡后优先复活于此（plan-death §4d 已确认 "灵龛 > 出生点"）
- [ ] 灵龛所有者离线时，保护仍生效（防晚上被扫荡）

### §2.3 失效条件

- [ ] **被动路过不触发揭露**——随机踩点不算
- [ ] 揭露仅在下列主动行为触发：
  - 在 5 格内**凝视灵龛方块 ≥ 3 秒**（视线聚焦计时）
  - **破坏尝试**（哪怕破坏失败，尝试本身即触发）
  - 主动"标记坐标"交互（指南针 / 日志条目）
- [ ] 失效后通知灵龛所有者（单次 narration："灵龛再无庇佑"）
- [ ] 被标记者是谁？**不告知**，维持追击悬念

### §2.4 负数区灵龛

- [ ] 允许放置在负灵域（`zone.spirit_qi < 0`）——worldview line 595 明示
- [ ] 高阶玩家换取绝对隐蔽，代价是建龛/取物时承受负压损伤

### §2.5 数据契约

```rust
pub struct SpiritNiche {
    pub owner: CharId,
    pub pos: BlockPos,
    pub placed_at_tick: Tick,
    pub revealed: bool,               // 被发现后为 true
    pub revealed_by: Option<CharId>,  // server 内部记录，不对外
    pub defense_mode: Option<DefenseModeId>, // 预留 hook，本 plan 不实装（见 reminder.md）
}
```

## §3 关系图（稀疏图，非组织架构）

### §3.1 关系类型

| 类型 | 建立方式 | 方向 | 过期/解除 |
|------|---------|------|----------|
| **师承** | NPC 主动传功 / 玩家求教事件成功 | 单向 有→无 | 师父老死 / 逐出 |
| **同行者** | 共同在线 50 格内累计 ≥ **5 小时** | 双向 | 一方超 30 天未互动自动褪色 |
| **盟约** | 双方显式立誓（UI 确认 + 生平卷写入） | 双向 | 任一方解盟（对方生平卷记"背盟"） |
| **死仇** | 一方杀死另一方 | 双向 + 时间戳 | 永久，不可解（只能等终结） |

- [ ] **稀疏图结构**：邻接表，`HashMap<CharId, Vec<Relationship>>`
- [ ] 落盘在 `relationships` 表（plan-persistence §1，多态 payload）
- [ ] **不做**：朋友请求 / 拉黑列表 / 队伍邀请 / 公会系统

### §3.2 师承特例

- [ ] 师承由 NPC 主动触发（通常基于声名 + 染色 + 距离）——玩家无法主动"拜师"按钮
- [ ] 师承影响：
  - 获得一次"传功"事件（学一个功法残篇 / 获得一次顿悟钩子）
  - NPC 师父死后，玩家**生平卷记师承**，不继承物品
- [ ] 坚守 plan-death 原则：师父角色终结后，徒弟不"继承遗物"

### §3.3 盟约

- [ ] 盟约需要**双方同时面对面 + 同时按确认键 + 口述盟约条款**（聊天栏输入，计入生平卷）
- [ ] 条款是自由文本，不做结构化字段（故意粗糙，背盟让天道 agent 解读）
- [ ] 盟约被目击即暴露双方身份（见 §1.2 交易同规则）
- [ ] 背盟代价：业力 +50 + 生平卷永久 "背盟者" tag

### §3.4 死仇

- [ ] 杀方 + 被杀方**互记死仇**，时间戳 + 地点
- [ ] 死仇关系永久（直到终结）
- [ ] **不做仇恨系统 buff**（不给攻击加成），纯叙事标记

## §4 声名

### §4.1 个人名号，不是阵营值

- [ ] `Renown { fame: i32, notoriety: i32 }` 双轴：
  - `fame` 善行累积：救人 / 助弱 / 化解冲突
  - `notoriety` 恶行累积：截胡 / 欺凌低境界 / 背盟
- [ ] 两者**不抵消**——一个人可以既有名又臭名
- [ ] **显示层**：HUD / 他人感知页面仅显示 **top 5 tags**（按"权重 × 时效衰减"排序）；其余写入生平卷 `renown_history`，agent 可引用
- [ ] Top 5 随行为实时更新（旧 tag 逐步滑出可见区）
- [ ] 与 karma 区分：karma 是天道系统的内部标记（plan-tribulation §5），renown 是 NPC/玩家口耳相传的公开标签

### §4.2 传播

- [ ] NPC 聚集时（`SocializeAction`）按距离传播声名（近者知细节，远者知梗概）
- [ ] 玩家间传播：通过聊天 / 目击暴露（自己不知道的名号会被别人叫出来）
- [ ] 生平卷公开时（终结），所有声名条目一并公开

### §4.3 影响

- [ ] NPC 态度基线：`reputation += fame/10 - notoriety/10`
- [ ] 高 notoriety → 派系弟子主动攻击 / 商人拒绝交易 / 散修警惕回避
- [ ] 高 fame → NPC 收徒概率提升 / 盟约建立门槛降低

### §4.4 染色谱 vs 声名

- [ ] 染色谱（worldview §六）是**修行选择的物理沉淀**，反映你**修了什么**
- [ ] 声名反映你**做了什么**
- [ ] 两者都公开、都影响 NPC 态度，但**独立存在**（染色血红 + 声名仁义 的矛盾组合完全可能）

## §5 玩家 ↔ NPC 派系挂靠（极简）

### §5.1 不走正式入门

- [ ] 玩家**不能主动申请加入**某个 NPC 派系
- [ ] 派系掌门 NPC 可在"考察 + 任务" 后主动邀请（plan-quest-v1 承接），被邀请玩家可接受/拒绝

### §5.2 挂靠后

- [ ] 玩家获得 `FactionMembership { faction, rank: 0=外门, loyalty: initial }`
- [ ] 享受派系 HQ 受保护 + 可接派系任务
- [ ] 派系声望可升降；背叛派系（攻击同门）立即除名 + notoriety +50

### §5.3 退出

- [ ] 主动请辞 → 派系声望 -20，无 notoriety
- [ ] 被逐出（业力 / 背叛）→ notoriety +50 + 死仇派系全员

### §5.4 叛变冷却（软限制）

- [ ] **单次叛变**：30 天内所有派系 NPC 拒绝邀请该玩家（NPC 侧硬拒绝，邀请流程根本不触发）
- [ ] **累计 3 次叛变**：
  - 业力 +100（worldview 重罪阈值，触发定向天罚）
  - 所有派系**永久**拒绝邀请（不再随时间恢复）
  - `renown.tags` 自动加"三叛之人"永久标签
- [ ] 无硬锁阻止叛变操作本身——自由选择，代价由天道和派系给

## §6 玩家 ↔ 玩家交互

### §6.1 切磋

- [ ] 发起方触发 → 向对方 push `SparringInvite` payload，弹出**邀请 UI**
- [ ] 邀请 UI（B 类 Screen）：
  - 显示发起方境界段（匿名时仅"气息感知"）+ 条款（无代价试炼）
  - 两个按钮：[应战 / 拒绝]
  - **10s 倒计时**，无响应自动取消
  - 对方可见明显 HUD 闪烁提示（防漏看）
- [ ] 接受后进入切磋模式：不掉装备、不扣寿、不走 death roll
- [ ] 切磋失败方进入 5min "谦抑" buff（真元回复 -30%）
- [ ] 切磋不计入死仇
- [ ] UI 草图待补（见 §11）

### §6.2 交易

- [ ] 双方面对面 + 拖拽物品 + 双方确认
- [ ] 交易双方身份互相暴露（§1.2）
- [ ] 交易信息**不广播**其他人
- [ ] 交易写入双方生平卷 `trades[]`（数量/物品名，不含对方详细身份）

### §6.3 PK（默认）

- [ ] 无"开关 PK"概念——所有玩家在野外默认可互相攻击
- [ ] 灵龛内保护（§2）
- [ ] 杀人即死仇（§3.4）+ notoriety +10（若被杀者 fame > 杀手 fame）

## §7 数据契约

### Components / Stores

```rust
pub struct Anonymity {
    pub displayed_name: Option<String>,  // None = 匿名
    pub exposed_to: HashSet<CharId>,     // 对这些玩家已暴露
}

pub struct Renown {
    pub fame: i32,
    pub notoriety: i32,
    pub tags: Vec<RenownTag>,            // 如"背盟者"/"戮道者"/"护弱"
}

pub struct Relationship {
    pub kind: RelationshipKind,          // Master/Disciple/Companion/Pact/Feud
    pub peer: CharId,
    pub since_tick: Tick,
    pub metadata: JsonValue,             // 盟约条款 / 死仇地点 等
}

pub struct SpiritNiche { /* 见 §2.5 */ }

pub struct ExposureLog(Vec<ExposureEvent>);
pub struct ExposureEvent {
    pub tick: Tick,
    pub kind: ExposureKind,              // Chat/Trade/Divine/Death
    pub witnesses: Vec<CharId>,
}
```

### Channel

- [ ] `bong:social/exposure` — 暴露事件（HUD 响应"已暴露给 X"提示）
- [ ] `bong:social/pact` — 盟约建立 / 解除
- [ ] `bong:social/feud` — 死仇建立
- [ ] `bong:social/renown_delta` — 声名变动

### 落盘（对接 plan-persistence §1）

**⚠️ plan-persistence §1 表清单目前未列这 4 张，落地时需同步补入 plan-persistence**：

- [x] `relationships` 表骨架已建（`server/src/persistence/mod.rs`:`RelationshipRecord` + DDL，`3ad73f90`；social 写入路径待接入）
- [ ] `exposures` 表（append-only，char_id / kind / witnesses JSON / tick）
- [ ] `renown` 表（char_id 主键，fame / notoriety / tags JSON）
- [ ] `spirit_niches` 表（owner 主键，pos / revealed / placed_at / defense_mode）

### 跨 plan 依赖风险

**plan-quest-v1 尚未立项**，但本 plan 的以下流程依赖它：
- §3.2 师承 NPC 主动触发 → plan-quest `EncounterEvent`
- §5.1 派系掌门邀请 → plan-quest 任务驱动
- §6.1 切磋的邀请弹窗未必归 quest，可能独立存在

实装顺序建议：plan-quest-v1 至少先立项并定义 `EncounterEvent` 接口，否则 social Phase 3/6 只能留 stub。

## §8 实施节点

**Phase 0 — 匿名渲染**
- [ ] Client 端 name tag 默认隐藏
- [ ] 境界外观模糊分档（醒灵/引气同形 → 凝脉/固元稍异 → 通灵/化虚异象）
- [ ] 固元+ 气息感知（"气息在你之上 / 微弱"）
- [ ] `Anonymity` Component：**server 权威**维护 `exposed_to: HashSet<CharId>`（暴露事件触发时写入）
- [ ] server 下发 `AnonymityPayload` 给每个 client，只含"对本人可见"的远端玩家子集 → client 据此显示/隐藏 name tag

**Phase 1 — 暴露管道**
- [ ] `bong:social/exposure` channel
- [ ] 聊天暴露（半径 50 格）
- [ ] 死亡暴露（杀手 + 观察者）
- [ ] 天道点名暴露（渡劫广播 / 化虚公示 自动触发）
- [ ] `ExposureLog` 持久化（plan-persistence Phase 2）
- [ ] HUD 提示"已向 X 暴露身份"

**Phase 2 — 灵龛**
- [ ] 「龛石」item + 放置交互
- [ ] `SpiritNiche` Component + 5 格保护区（NPC 不攻击 + 玩家不破坏方块）
- [ ] 复活点优先级（灵龛 > 出生点）
- [ ] **揭露判定**（主动三类触发，§2.3）：凝视 3s 视线聚焦计时 / 破坏尝试 / 标记坐标交互
- [ ] 被揭露后通知所有者（单次 narration，不告知揭露者）
- [ ] 负数区灵龛支持 + 建龛/取物**承受负压损伤**（`zone.spirit_qi < 0` 时按境界扣真元）

**Phase 3 — 关系图**
- [ ] `Relationship` Component + 稀疏图查询
- [ ] 同行者累计（**50 格内 5h**，服务器 tick 轮询 1Hz，每 tick 扫描玩家对）
- [ ] **单向关系存储**：Master/Disciple 各自 entity 的 `Relationship` 列表里都写一条对应 kind，查询时按 kind 过滤方向
- [ ] 死仇自动触发（死亡事件 → 双向写）
- [ ] 关系过期逻辑（同行者 30 天自动褪色）
- [ ] **师承触发入口**：`EncounterEvent` hook（plan-quest-v1 承接触发逻辑；本 plan 只提供 `Relationship::Master` 写入 API）

**Phase 4 — 盟约 / 切磋 / 交易 UI**
- [ ] 盟约：双确认 UI + 自由文本条款 + 生平卷写入
- [ ] 切磋：**邀请 UI（B Screen · 10s 倒计时 · 应战/拒绝 · HUD 闪烁提示）** + 无代价打一架 + 谦抑 buff
- [ ] 交易：面对面拖拽 UI + 物品交换
- [ ] 所有三者均触发相应暴露
- [ ] UI 草图：`docs/svg/social-ui-v1.svg`（见 §11）

**Phase 5 — 声名**
- [ ] `Renown` 组件 + fame/notoriety 双轴
- [ ] 善行 / 恶行事件钩子（救人 / 截胡 / 背盟 自动打 tag）
- [ ] **PK notoriety 规则**（§6.3）：被杀者 fame > 杀手 fame → 杀手 notoriety +10
- [ ] NPC 声名传播（`SocializeAction` 按距离衰减）
- [ ] **玩家间声名传播**：聊天 / 目击暴露事件时，若说话者 / 观察者自身已知该 tag，自动对目标 `exposed_to` 加一条 + 客户端 toast "听闻此人是…"
- [ ] **Top 5 tags 显示计算**（权重 × 时效衰减排序，实时更新）
- [ ] NPC reputation 公式接入 `fame/10 - notoriety/10`

**Phase 6 — 玩家派系挂靠**
- [ ] `FactionMembership` 复用 plan-npc-ai §4
- [ ] 掌门 NPC 发邀请流程（plan-quest-v1 任务驱动）
- [ ] 主动请辞 / 被逐出流程
- [ ] HUD 派系 tag（**仅自己可见**——同派系玩家间不因 membership 自动互认姓名，需走正常暴露路径如聊天/交易）
- [ ] **叛变冷却**（§5.4）：单次 30 天禁邀 / 累计 3 次业力 +100 + 永久禁入 + "三叛之人" tag

**Phase 7 — 终结后公开**
- [ ] 终结触发：ExposureLog / Relationship / Renown 全部写入亡者博物馆快照
- [ ] **姓名随生平卷一并公开**（§1.3），library-web 可按姓名搜索
- [ ] library-web 渲染公开页面（对接 plan-persistence §10.10 增量导出）

**Phase 8 — Agent 集成**
- [ ] Agent narration 可读 Renown / Relationship 做个性化叙事
- [ ] Agent 可推演派系内部关系变动（影响 NPC 派系，不直接改玩家关系）

## §9 已决定

- ✅ **默认匿名 + 默认敌对**：所有玩家头顶无名，境界外观模糊
- ✅ **暴露四种触发**：聊天 / 交易 / 天道点名 / 死亡（worldview 原样）
- ✅ **化名系统不做**：匿名是游戏核心张力，化名会削弱
- ✅ **灵龛一次性 + 被发现即废**：龛石稀缺获取
- ✅ **灵龛所在地不告知"被谁发现"**：维持追击悬念
- ✅ **关系类型限 4 种**：师承 / 同行 / 盟约 / 死仇，不扩展
- ✅ **不做朋友/拉黑/组队/公会**：原子化社会原则
- ✅ **声名双轴 fame / notoriety**，**不抵消**
- ✅ **染色谱 + 声名 独立**：一个反映修行、一个反映行为
- ✅ **玩家不能主动拜师**：师承由 NPC 事件触发
- ✅ **玩家派系挂靠极简**：无外门/内门/真传层级，仅 `rank: 0..=4` 与 plan-npc-ai 共用
- ✅ **PK 默认开启**：无 PVE/PVP 开关，灵龛是唯一保护
- ✅ **死仇不给战斗 buff**：纯叙事标记，不做系统性仇恨加成
- ✅ **切磋不计入死仇 / 不扣寿 / 不掉装**：安全试炼模式
- ✅ **同行者阈值 5h**（50 格内累计）
- ✅ **灵龛揭露仅主动触发**：凝视 3s / 破坏尝试 / 标记坐标；路过不算
- ✅ **切磋邀请 UI 10s 倒计时**；对方无响应自动取消，HUD 闪烁避免漏看
- ✅ **Renown 显示 top 5 tags**（权重 × 时效衰减），全史写生平卷
- ✅ **叛变软限制**：单次 30 天禁邀 / 累计 3 次业力 +100 + 永久禁入 + "三叛之人"永久 tag
- ✅ **同派系不自动互认姓名**：HUD 派系 tag 仅自己可见，互识仍靠正常暴露路径
- ✅ **师承触发管道归 plan-quest-v1**：本 plan 只提供 `Relationship::Master` 写入 API
- ✅ **Anonymity server 权威**：`exposed_to` 由 server 维护，AnonymityPayload 下发

## §10 剩余开放问题

_（无未决项，所有设计问题均已收口）_

## §11 UI 草图待补

- [ ] `docs/svg/social-ui-v1.svg`（或并入其他草图）——至少覆盖：
  - **切磋邀请弹窗**（B Screen · 10s 倒计时 · 应战/拒绝）
  - **盟约共立**（双方同屏 · 自由文本条款输入 · 双方确认）
  - **交易面板**（双方拖拽 · 暴露警示）
  - **暴露提示**（HUD 顶部 toast："身份已向 X 暴露 · 来源：聊天/死亡/交易"）
  - **声名自览**（K 键 inspect 面板一栏 · top 5 tags + fame/notoriety 数值）
  - **灵龛所有者视角**（被揭露时的一次性 narration 提示）
- 实装时机：Phase 4 与 Phase 5 UI 同步，不单独成一个 phase

---

## §12 进度日志

- 2026-04-25：核对实际代码，本 plan 主体（§0–§8 Phase 0–8）整体仍为纯设计，无玩家社交侧实装；server 仅已有 §-1 列出的聊天采集（`network/chat_collector.rs` + `schema/chat_message.rs` + Redis `bong:player_chat`），`persistence/mod.rs` 已建 `relationships` 表骨架（plan-persistence 落库，未接 social 写入路径）；server 内 `npc/faction.rs`（`Reputation`/`FactionMembership`/`FactionId`）与 `npc/social.rs`（`SocializeAction`/`FactionDuelScorer`）属 plan-npc-ai §4 NPC 内部派系，不是本 plan 的玩家匿名/关系/声名/灵龛系统，未勾任何 `[x]`。