# Bong · plan-identity-v1 · 骨架

**身份与信誉系统**。把 worldview §十一 已正典化的"多 identity / NPC 信誉度分级 / 切换洗白 / 毒蛊师社会默认"实装为 server 玩家身份系统 + dugu RevealedEvent consumer + NPC AI 反应分支 + 6 流派 vN+1 通用 RevealedEvent trait + agent 通缉链路 + client 身份切换面板。**基础组件大半已实装于 plan-social-v1**（Renown / Anonymity / ExposureEvent / Relationships），本 plan 在其上扩**多 identity 包装 + 反应分级 + 切换机制**。

**世界观锚点**：
- `worldview.md §十一 安全与社交 / 身份与信誉` (commit fe00532c **本 plan 全部物理根基**——多 identity / 切换 = 洗白机会 / NPC 反应 4 档 / 暴露分类 / 毒蛊师 -50 baseline)
- `worldview.md §十一 安全与社交 / 灵龛` (灵龛 5 格安全空间——身份切换的物理位置)
- `worldview.md §十一 安全与社交 / 匿名系统` (默认匿名 → 暴露 → identity 关联)
- `worldview.md §五 末土后招原则` (流派识别 = 事件，接 §十一)
- `worldview.md §五 流派由组合涌现` ("解锁 = 使用过"，招式 tag 可被识破，接 §十一)

**library 锚点**：待写 `peoples-XXXX 拾名录`（散修视角多身份指南：什么时候洗白 / 哪些身份有保留价值 / 毒蛊师如何在不洗白前提下减少暴露）

**交叉引用**：
- `plan-social-v1`（✅ finished，**强前置**）—— `Renown { fame, notoriety, tags }` / `Anonymity` / `ExposureEvent` / `Relationship` / `Relationships` / `FactionMembership` / `SocialRenownDeltaEvent` 已实装；本 plan **复用不重定义**，每 IdentityProfile 内部嵌套 Renown
- `plan-dugu-v1`（✅ finished，**强前置**）—— `DuguRevealedEvent { revealed_player, witness, witness_realm, at_position, at_tick }` 已 emit 但无 consumer；`server/src/identity/stub.rs` 留好 stub；本 plan P2 接 consumer
- `plan-niche-defense-v1`（active ⏳ ~5%，**协同前置**）—— 灵龛 5 格安全空间是身份切换的物理位置；本 plan 提供 `WithinOwnNiche` precondition；client 切换面板 P5 走 niche UI 入口
- `plan-anqi-v1` / `plan-zhenmai-v1` / `plan-tuike-v1` / `plan-woliu-v1` / `plan-zhenfa-v1` / `plan-baomai-v1`（全 ✅ finished）—— 6 流派 vN+1 等本 plan 提供统一 `RevealedEvent` trait + consumer 模板（不绑定流派各自实装时间）
- `plan-jiezeq-v1`（active 2026-05-04）—— 切换冷却 1 game-day = 24000 ticks（vanilla MC day），用 `world::tick::GameTick`；不依赖节律 SeasonState
- `plan-multi-style-v1`（active）—— `PracticeLog` 与 RevealedTag 是平行系统：PracticeLog 是"修了多少"，RevealedTag 是"被识破多少"；不冲突，可共存
- `plan-persistence-v1`（✅ finished）—— `IdentityProfile` 数据落 SQLite（已有 `player_state` DB schema）
- `plan-narrative-v1`（✅ finished）—— agent 通缉令 narration 模板，参考 calamity skill 文风
- `plan-cultivation-v1`（✅ finished）—— `Realm` 用于 RevealedEvent.witness_realm（高境识破后果重于低境）

**阶段总览**：

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | `IdentityProfile` 数据模型 + `PlayerIdentities` Component + `IdentityRegistry` Resource + 首次进游戏 default identity 创建（弹提示选 name，默认 = MC username）+ persistence 接入 | ✅ 2026-05-08 |
| P1 | `/identity` slash command（list / new / switch / rename）+ `WithinOwnNiche` precondition + 切换冷却 1 game-day + `IdentitySwitchedEvent` / `IdentityCreatedEvent` | ✅ 2026-05-08 |
| P2 | `DuguRevealedEvent` consumer → 写 `RevealedTag::DuguRevealed`（permanent）→ `reputation_score` 公式 + 毒蛊师 -50 baseline + 切身份消除 | ✅ 2026-05-08 |
| P3 | `reputation_score` 4 档分级（High / Normal / Low / Wanted）+ `IdentityReactionChangedEvent` + NPC big-brain `IdentityReactionScorer` + NPC 拒交易 / 主动攻击行为分支 | ✅ 2026-05-09 |
| P4 | 通用 `RevealedEvent` trait + `RevealedTagKind` enum 全枚举 + 6 流派 vN+1 接入文档（本 plan 不实装各流派，仅留 hook + docs） | ✅ 2026-05-08 |
| P5 | NPC 间传话扩散（同 zone 概率）+ agent 接入（Wanted 档 emit `bong:wanted_player`）+ client 身份切换面板（灵龛 GUI + HUD 当前 identity 角标） | ✅ 2026-05-09 |

---

## 接入面 checklist（防孤岛）

| 维度 | 内容 |
|------|------|
| **进料** | player `Entity`（玩家本体） · 既有 `Renown` / `Anonymity` / `Relationships` / `FactionMembership` Components · `DuguRevealedEvent`（plan-dugu-v1 已 emit）· `world::tick::GameTick`（切换冷却计时）· `cultivation::Realm`（witness_realm 字段）· `world::niche::SpiritNiche`（5 格 precondition） |
| **出料** | `PlayerIdentities` Component（玩家持有多 identity）· `IdentityRegistry` Resource（全局 identity 索引）· `IdentitySwitchedEvent` / `IdentityCreatedEvent` / `IdentityReactionChangedEvent` / `RevealedEvent` trait · NPC big-brain `IdentityReactionScorer` 输入 · `bong:wanted_player` Redis pub（仅 Wanted 档）· client `bong:identity_panel_state` CustomPayload |
| **共享 event** | 复用 `SocialRenownDeltaEvent`（plan-social-v1 已实装，每 RevealedTag 写入时 emit）；新增 `IdentitySwitchedEvent` / `IdentityCreatedEvent` / `IdentityReactionChangedEvent`（仅本 plan 内部 + agent 推送）；新增 `RevealedEvent` trait（6 流派 vN+1 共用） |
| **跨仓库契约** | **server**：`IdentityProfile` struct / `IdentityId` newtype / `PlayerIdentities` Component / `IdentityRegistry` Resource / `RevealedTag` struct / `RevealedTagKind` enum / `RevealedEvent` trait / `IdentitySwitchedEvent` / `IdentityCreatedEvent` / `IdentityReactionChangedEvent` Bevy events / `IdentityReactionScorer` big-brain Component / `WithinOwnNiche` precondition / `/identity` slash command / `consume_dugu_revealed_to_identity_tag` system / `reputation_score` pure function<br>**agent**：`bong:wanted_player` Redis pub schema (TypeBox `WantedPlayerEventV1`) / `agent/packages/tiandao/src/skills/calamity.md` 加 `wanted_player` 处理段（agent 决策是否生成"通缉令" narration）<br>**client**：`IdentityPanelScreen` Java UI（灵龛内打开）/ `IdentityHudCornerLabel`（HUD 角落显示当前 identity display_name）/ `bong:identity_panel_state` CustomPayload（同步当前 identity + 列表给 client） |
| **worldview 锚点** | §十一 全节（身份/信誉/匿名/灵龛/毒蛊师默认）+ §五 末土后招原则（流派识别 = 事件 hook 至 §十一）+ §五 流派由组合涌现（识破 tag 接 §十一） |
| **红旗自查** | ❌ 自产自消（接 social / dugu / niche / 6 流派 / agent / client / persistence） · ❌ 近义重名（`IdentityProfile` 是新概念；`Renown` / `Relationships` 是复用） · ❌ 无 worldview 锚（§十一 全节直接锚定） · ⚠️ skeleton 同主题：plan-niche-defense-v1（灵龛系统协同） + plan-social-v1（已 finished 基础组件） —— 本 plan 是 consumer / 扩展，非另起 · ❌ 跨仓库缺面（server + agent + client 都涉及） |

---

## §0 设计轴心

- [ ] **多 identity 集中于玩家 entity**（Q1 A）—— `PlayerIdentities { identities: Vec<IdentityProfile>, active_identity_id }` 是单 Component，避免 entity 散落造成 query 复杂
- [ ] **仅玩家做 NPC 留 vN+1**（Q2 B）—— NPC 自己持有 identity（散修给自己起化名）是后续；本 plan NPC 仅做反应分支
- [ ] **首次进游戏弹提示选 name**（Q3 B）—— 默认 = MC username（玩家不改即继续用 MC ID）；玩家可改自定义 display_name；后续可在灵龛内通过 `/identity rename` 再改
- [ ] **`/identity` 限灵龛 5 格内执行**（Q4 A+C）—— worldview §十一 灵龛 = 安全空间，身份切换是"洗白仪式"；同时满足 dev test 方便（spawn 一个灵龛即可测试）；不做 op 权限分级
- [ ] **旧身份冻结待复用**（Q5 B）—— 切到其他 identity 后旧 identity `frozen = true`，但数据完整保留；未来切回时 NPC **仍认识旧身份**（"我又把这件外套穿上了"间谍式玩法）
- [ ] **可双向切换**（Q5b A）—— `/identity switch <id>` 可切回任意 frozen identity；不是单向 burning
- [ ] **切换冷却 1 game-day**（Q6b B）—— `last_switch_tick + 24000 <= now`；防玩家"打架前切，打完切回"完全规避后果
- [ ] **不影响物资**（Q6 撤销）—— worldview "人脉/物资关系归零" 中"关系"指社交关系层（Relationships / NPC 信誉度），不指物品；切身份不动背包 / 灵龛物资
- [ ] **毒蛊师 -50 baseline 永久**（Q9 C）—— `RevealedTag::DuguRevealed { permanent: true }`；不衰减；唯一消除路径 = 切身份（旧 identity frozen 仍带此 tag，未来切回会再触发）
- [ ] **玩家 identity 自己可见 ≠ §K 红线**（client 显式展示）—— 节律 §K 红线管"汐转 / 季节"完全不显式；identity 玩家自己要知道当下装在哪个面具，必须显式 HUD 展示。但 **NPC 信誉度分级（High/Normal/Low/Wanted）不显式**——玩家通过 NPC 反应自己悟"哦这家伙不卖给我了"
- [ ] **信誉度按 identity 独立**——同玩家两 identity 完全分别统计；切身份的本质就是"换一套 NPC 信誉度账本"
- [ ] **6 流派 vN+1 共用 RevealedEvent trait**（Q11 A）—— 一次设计好接口，6 流派 vN+1 实装时不用重新设计；本 plan P4 仅实装 dugu consumer 作为参考实现

---

## §1 第一性原理（worldview §十一 / §五 推导）

- **信息差是末法 PVP 核心**（§五 末土后招原则）—— 不藏后招的修士都死在变熟练之前；故"假装是别人"是基本生存技能
- **身份是工具不是约束** —— 修士主动维护多 identity 是常态（散修无门派不需要忠诚），与玩家 mc-account 解耦
- **洗白要有代价**（worldview 明文 "人脉/物资关系归零"）—— 切身份冻结旧人脉 + 冷却时长，否则毒蛊师可秒切免疫
- **流派识别 = 事件**（§五 §519）—— 不是状态字段；某次战斗 / inspect 在某时刻识破 → 写入 LifeRecord + 该 identity 的 RevealedTag
- **毒蛊社会默认严酷**（§十一）—— -50 baseline + 高境神识识破即追杀；这是世界观默认设定不是 stigma 累积

---

## §2 P0 — 数据模型

### 类型定义（`server/src/identity/mod.rs` 新模块，移除 `server/src/identity/stub.rs`）

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IdentityId(pub u32);  // per-player local，0 = 初始默认 identity

#[derive(Debug, Clone, Component, Serialize, Deserialize)]
pub struct PlayerIdentities {
    pub identities: Vec<IdentityProfile>,
    pub active_identity_id: IdentityId,
    pub last_switch_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProfile {
    pub id: IdentityId,
    pub display_name: String,           // 默认 = MC username, /identity rename 可改
    pub created_at_tick: u64,
    pub renown: Renown,                 // 复用 plan-social-v1 既有 Renown
    pub revealed_tags: Vec<RevealedTag>,
    pub frozen: bool,                   // 切到其他 identity 后 = true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevealedTag {
    pub kind: RevealedTagKind,
    pub witnessed_at_tick: u64,
    pub witness_realm: Realm,
    pub permanent: bool,                // 毒蛊师 = true；其他流派可衰减（vN+1）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevealedTagKind {
    DuguRevealed,         // 毒蛊师 -50 baseline 永久
    AnqiMaster,           // 暗器流（vN+1 hook）
    ZhenfaMaster,         // 阵法流
    BaomaiUser,           // 爆脉流
    TuikeUser,            // 替尸流
    WoliuMaster,          // 涡流流
    ZhenmaiUser,          // 截脉流
    SwordMaster,          // 通用招式 tag（非流派）
    ForgeMaster,          // 炼器名声
    AlchemyMaster,        // 炼丹名声
}

impl RevealedTagKind {
    pub const fn baseline_penalty(self) -> i32 {
        match self {
            Self::DuguRevealed => 50,   // worldview §十一 毒蛊师 -50 baseline
            _ => 0,                      // 其他流派 vN+1 自定 penalty
        }
    }
}

#[derive(Default, Resource)]
pub struct IdentityRegistry {
    // 全局索引：用于 NPC 间传话扩散 (P5) + agent wanted_player 查询
    pub by_player_uuid: HashMap<Uuid, IdentityRegistryEntry>,
}
```

### 首次进游戏 default identity 创建

- [ ] `system: spawn_default_identity_on_player_join`：玩家第一次连接 server → 检查是否已有 `PlayerIdentities` Component（persistence load）→ 若无 → 创建 `IdentityProfile { id: IdentityId(0), display_name: <MC username>, created_at_tick: now, renown: Renown::default(), revealed_tags: vec![], frozen: false }` + 设为 active
- [ ] **不弹强制 UI 提示**——直接默认 MC username，玩家想改通过 `/identity rename` 任意时机（限灵龛 5 格内）。这降低教学复杂度

### Persistence

- [ ] SQLite schema 加 `player_identities` 表：`player_uuid TEXT PRIMARY KEY, identities BLOB（serde_json）, active_id INT, last_switch_tick INT`
- [ ] 复用 `plan-persistence-v1` 既有 `player_state` DB connection

### 测试

- [ ] `default_identity_created_on_first_join_uses_mc_username`
- [ ] `existing_identity_loaded_from_persistence_on_rejoin`
- [ ] `player_identities_serializes_round_trip`
- [ ] `identity_profile_default_renown_is_zero`

---

## §3 P1 — `/identity` slash command + 切换机制

### Slash command 定义（接 plan-server-cmd-system-v1）

| 命令 | 功能 | precondition |
|---|---|---|
| `/identity list` | 列出玩家所有 identity（active / frozen 标记 + display_name + reputation_score）| 无 |
| `/identity new <display_name>` | 创建新 identity，立即切到该 identity | 灵龛 5 格内 + 切换冷却已过 |
| `/identity switch <id>` | 切到任意 identity（active / frozen 都可，frozen 会 unfreeze）| 灵龛 5 格内 + 切换冷却已过 |
| `/identity rename <new_name>` | 改当前 active identity 的 display_name | 灵龛 5 格内（无冷却限制）|

### `WithinOwnNiche` precondition

- [ ] 检查玩家位置 ↔ 其灵龛位置（`SpiritNiche` Component / niche-defense-v1 `NicheRegistry`）距离 ≤ 5 格
- [ ] 不在自己灵龛 5 格内 → 命令拒绝 + 提示"非灵龛内不可操心身份"
- [ ] **dev test 友好**：spawn 临时灵龛即可测试，不需 op 权限

### 切换冷却

- [ ] `last_switch_tick + IDENTITY_SWITCH_COOLDOWN_TICKS <= now`，常量 = 24000（vanilla 1 game-day = 20 实时分钟）
- [ ] 冷却未过 → 命令拒绝 + 提示"身份未稳，候片刻再易容"
- [ ] **rename 不算切换**——不消耗冷却

### 切换流程

```
/identity switch <id>:
  1. 检查 precondition (within_own_niche + cooldown_passed)
  2. 当前 active identity.frozen = true（冻结，待复用）
  3. target identity.frozen = false（unfreeze）
  4. active_identity_id = target id
  5. last_switch_tick = now
  6. emit IdentitySwitchedEvent { player, from: prev_id, to: target_id, at_tick: now }
  7. 更新 client HUD 当前 identity（CustomPayload bong:identity_panel_state）
```

### 测试

- [ ] `slash_identity_list_shows_all_identities_with_active_marker`
- [ ] `slash_identity_new_creates_identity_and_sets_active`
- [ ] `slash_identity_switch_freezes_old_unfreezes_new`
- [ ] `slash_identity_switch_within_own_niche_succeeds`
- [ ] `slash_identity_switch_outside_niche_rejected`
- [ ] `slash_identity_switch_within_cooldown_rejected`
- [ ] `slash_identity_rename_does_not_consume_cooldown`
- [ ] `identity_switched_event_emits_with_correct_from_to_ids`
- [ ] `unfreezing_old_identity_restores_renown_state`（worldview "待复用"语义）

---

## §4 P2 — DuguRevealedEvent consumer + 毒蛊师 baseline

### Consumer system

```rust
// server/src/identity/dugu_consumer.rs
fn consume_dugu_revealed_to_identity_tag(
    mut events: EventReader<DuguRevealedEvent>,
    mut players: Query<&mut PlayerIdentities>,
    mut delta_writer: EventWriter<SocialRenownDeltaEvent>,
) {
    for event in events.read() {
        if let Ok(mut identities) = players.get_mut(event.revealed_player) {
            let active = identities.active_mut();
            active.revealed_tags.push(RevealedTag {
                kind: RevealedTagKind::DuguRevealed,
                witnessed_at_tick: event.at_tick,
                witness_realm: event.witness_realm,
                permanent: true,
            });
            // 同步 emit social Renown delta（既有 plan-social-v1 链路）
            delta_writer.send(SocialRenownDeltaEvent {
                player: event.revealed_player,
                fame_delta: 0,
                notoriety_delta: 0,  // baseline 由 reputation_score 公式从 tag 算，不重复加 notoriety
                tags_added: vec![/* convert to RenownTagV1 if needed */],
            });
        }
    }
}
```

### `reputation_score` 公式（pure function）

```rust
pub fn reputation_score(identity: &IdentityProfile) -> i32 {
    let fame = identity.renown.fame;
    let notoriety = identity.renown.notoriety;
    let tag_penalty: i32 = identity.revealed_tags
        .iter()
        .map(|t| t.kind.baseline_penalty())
        .sum();
    fame - notoriety - tag_penalty
}
```

### 切身份消除毒蛊师 baseline

- [ ] 切身份**不删** RevealedTag（旧 identity 数据完整保留，frozen）
- [ ] 但**新 active identity** 的 `reputation_score` 从 0 开始（无 tags）
- [ ] **未来切回旧 identity** → reputation_score 重新含 -50（"我又把毒蛊师外套穿上了 NPC 又翻脸了"）

### 测试

- [ ] `dugu_revealed_event_writes_revealed_tag_to_active_identity`
- [ ] `reputation_score_default_identity_is_zero`
- [ ] `reputation_score_with_dugu_tag_is_negative_50`
- [ ] `dugu_tag_persists_after_freeze_and_unfreeze`
- [ ] `switching_to_new_identity_resets_reputation_to_zero`
- [ ] `switching_back_to_dugu_revealed_identity_restores_negative_50`

---

## §5 P3 — NPC 反应分级 + big-brain Scorer

### 4 档分级（worldview §十一）

```rust
pub enum ReactionTier {
    High,    // > 50  — NPC 主动给情报 / 折扣 / 接私活
    Normal,  // -25 ~ 50 — 正常交易
    Low,     // -75 ~ -25 — 加价 / 拒绝服务 / NPC 间传话
    Wanted,  // < -75 — 通缉，agent 主动追杀 narration
}

pub fn reaction_tier(score: i32) -> ReactionTier {
    match score {
        s if s > 50 => ReactionTier::High,
        s if s >= -25 => ReactionTier::Normal,
        s if s >= -75 => ReactionTier::Low,
        _ => ReactionTier::Wanted,
    }
}
```

### `IdentityReactionChangedEvent`

- [ ] 当 active identity 的 `reputation_score` 跨 tier 边界 → emit `IdentityReactionChangedEvent { player, from_tier, to_tier, identity_id, at_tick }`
- [ ] 推到 NPC blackboard（cached），避免每 tick query

### NPC big-brain `IdentityReactionScorer`

```rust
#[derive(Component, Clone, Debug)]
pub struct IdentityReactionScorer {
    pub target_player: Entity,
}

// Score 输出：
//   ReactionTier::Wanted  → 高优先级攻击/追杀（与 PlayerProximityScorer 加权）
//   ReactionTier::Low     → 拒交易 + 远离倾向（避免靠近）
//   ReactionTier::Normal  → 兜底
//   ReactionTier::High    → 主动接近 + 友善 narration
```

### NPC 行为分支接入

- [ ] `npc::trade::accept_trade_request` 引入 reaction_tier 检查：`Low` / `Wanted` 拒绝
- [ ] `npc::brain::ChaseAction` 在 attacker = NPC + target = Wanted 玩家时优先级提升
- [ ] **不实装传话**（留 P5）

### 测试

- [ ] `reaction_tier_boundary_high_normal_low_wanted`（4 档 boundary case）
- [ ] `identity_reaction_changed_event_emits_on_score_crossing_boundary`
- [ ] `npc_trade_rejected_for_wanted_player`
- [ ] `npc_chase_priority_boosted_for_wanted_player`
- [ ] `wanted_tier_normalized_after_identity_switch`（切身份立即降回 Normal）

---

## §6 P4 — 通用 RevealedEvent trait + 6 流派 hook

### Trait 定义

```rust
pub trait RevealedEvent: Event + Send + Sync {
    fn revealed_player(&self) -> Entity;
    fn witness(&self) -> Entity;
    fn witness_realm(&self) -> Realm;
    fn revealed_tag_kind(&self) -> RevealedTagKind;
    fn is_permanent(&self) -> bool;
    fn at_tick(&self) -> u64;
    fn at_position(&self) -> [f64; 3];
}

// dugu 实现作为参考
impl RevealedEvent for DuguRevealedEvent {
    fn revealed_player(&self) -> Entity { self.revealed_player }
    fn witness(&self) -> Entity { self.witness }
    fn witness_realm(&self) -> Realm { self.witness_realm }
    fn revealed_tag_kind(&self) -> RevealedTagKind { RevealedTagKind::DuguRevealed }
    fn is_permanent(&self) -> bool { true }
    fn at_tick(&self) -> u64 { self.at_tick }
    fn at_position(&self) -> [f64; 3] { self.at_position }
}
```

### 通用 consumer

```rust
pub fn consume_revealed_event<E: RevealedEvent>(
    mut events: EventReader<E>,
    mut players: Query<&mut PlayerIdentities>,
) {
    for event in events.read() {
        if let Ok(mut identities) = players.get_mut(event.revealed_player()) {
            identities.active_mut().revealed_tags.push(RevealedTag {
                kind: event.revealed_tag_kind(),
                witnessed_at_tick: event.at_tick(),
                witness_realm: event.witness_realm(),
                permanent: event.is_permanent(),
            });
        }
    }
}
```

### 6 流派 vN+1 接入文档

- [ ] `server/src/identity/README.md`（plan 内文档）：
  - 各流派如何 emit RevealedEvent（参考 `DuguRevealedEvent` impl）
  - 各 RevealedTagKind 的 baseline_penalty 设计建议（dugu = 50；其他流派如 anqi = 10 / zhenmai = 0 / 等）
  - 触发条件示例（anqi 暗器流：飞针被 inspect 神识识破 / tuike 蜕壳后被目击 等）
  - 测试模板：`<style>_revealed_event_writes_revealed_tag` × 6

### 测试

- [ ] `dugu_revealed_event_implements_revealed_event_trait`
- [ ] `consume_revealed_event_generic_works_for_dugu_event`
- [ ] `revealed_tag_kind_baseline_penalty_is_correct_per_kind`

---

## §7 P5 — 传话扩散 + agent + client

### NPC 间传话扩散（同 zone 概率）

- [ ] system `npc_identity_gossip_tick`：每 N tick 在每 zone 内 roll
- [ ] 已知 RevealedTag 的 NPC（witness）→ 同 zone 同派系 NPC 概率扩散（基线 5% / tick）
- [ ] 扩散后接收 NPC 把 tag 加入"已知"，触发 `IdentityReactionChangedEvent`（如果对当前 NPC 来说该 player 之前是 Normal，此扩散后变 Low/Wanted）
- [ ] 跨 zone 不扩散（worldview "口耳相传"局部性）
- [ ] **不实装"信任成本"**——同派系无条件相信传话（P5 简化）

### Agent 接入

- [ ] `IdentityReactionChangedEvent` 进入 Wanted tier 时 → emit `bong:wanted_player` Redis pub：
  ```json
  {
    "event": "wanted_player",
    "player_uuid": "...",
    "identity_display_name": "...",
    "reputation_score": -100,
    "primary_tag": "dugu_revealed",
    "tick": 12345
  }
  ```
- [ ] 修改 `agent/packages/tiandao/src/skills/calamity.md` 加一段：
  > "若收到 `wanted_player` 事件——agent 可生成"通缉令" narration（参考 §十一 "通缉" 语调），通知该 player 所在 zone 的 NPC 主动追杀。注意只对 reputation_score < -75 玩家生效，Low 档不通缉"
- [ ] schema 在 `agent/packages/schema/src/identity.ts`（新文件）加 `WantedPlayerEventV1`

### Client 身份切换面板

- [ ] **HUD 角标**（`IdentityHudCornerLabel`）：HUD 右下角小标显示当前 active identity 的 display_name + IdentityId（小字，不抢眼）
- [ ] **GUI 面板**（`IdentityPanelScreen`）：玩家在自己灵龛 5 格内按某键（建议 Y 或并入 niche UI）打开
  - 列出所有 identity（active / frozen）+ 每个的 display_name + reputation_score（可选显示，**显示**——玩家自己要知道哪个身份在被通缉）
  - 切换按钮（调 `/identity switch <id>`）
  - 创建新 identity 按钮 + 输入框
  - 改名按钮 + 输入框
- [ ] **NPC 反应分级不显示**——玩家不知道自己具体在 NPC 那里是哪一档；只能通过 NPC 反应自己悟（worldview §K 红线方向）
- [ ] CustomPayload `bong:identity_panel_state` 同步当前 identity + 列表给 client

### 测试

- [ ] `gossip_spreads_revealed_tag_to_same_zone_same_faction_npcs`
- [ ] `gossip_does_not_spread_across_zones`
- [ ] `gossip_triggers_identity_reaction_changed_for_receiving_npc`
- [ ] `wanted_player_event_emits_on_tier_drop_to_wanted`
- [ ] `wanted_player_event_does_not_emit_on_low_tier`（boundary）
- [ ] client manual smoke：开 niche UI → 看到 identity 列表 + 切换 → HUD 角标更新

---

## §8 数据契约（下游 grep 抓手）

| 契约 | 位置 |
|---|---|
| `IdentityId` newtype | `server/src/identity/mod.rs` |
| `PlayerIdentities` Component | `server/src/identity/mod.rs` |
| `IdentityProfile` struct | `server/src/identity/mod.rs` |
| `RevealedTag` struct | `server/src/identity/mod.rs` |
| `RevealedTagKind` enum | `server/src/identity/mod.rs` |
| `IdentityRegistry` Resource | `server/src/identity/mod.rs` |
| `RevealedEvent` trait | `server/src/identity/revealed.rs`（新文件）|
| `consume_revealed_event<E>` 通用 consumer | `server/src/identity/revealed.rs` |
| `consume_dugu_revealed_to_identity_tag` system | `server/src/identity/dugu_consumer.rs`（新文件）|
| `reputation_score` 函数 | `server/src/identity/mod.rs` |
| `ReactionTier` enum + `reaction_tier()` | `server/src/identity/reaction.rs`（新文件）|
| `IdentitySwitchedEvent` / `IdentityCreatedEvent` / `IdentityReactionChangedEvent` Bevy events | `server/src/identity/events.rs`（新文件）|
| `IdentityReactionScorer` big-brain Component | `server/src/npc/brain.rs` 或 `server/src/identity/scorer.rs` |
| `WithinOwnNiche` precondition | `server/src/identity/precondition.rs`（新文件） |
| `/identity` slash command handler | `server/src/identity/command.rs`（新文件） |
| `IDENTITY_SWITCH_COOLDOWN_TICKS = 24000` const | `server/src/identity/mod.rs` |
| `npc_identity_gossip_tick` system | `server/src/identity/gossip.rs`（新文件） |
| `bong:wanted_player` Redis pub + `WantedPlayerEventV1` schema | `agent/packages/schema/src/identity.ts`（新文件）+ server `redis_outbox` |
| calamity.md `wanted_player` 段 | `agent/packages/tiandao/src/skills/calamity.md` |
| `IdentityPanelScreen` Java UI | `client/src/main/java/com/bong/client/identity/IdentityPanelScreen.java`（新）|
| `IdentityHudCornerLabel` HUD | `client/src/main/java/com/bong/client/hud/IdentityHudCornerLabel.java`（新）|
| `bong:identity_panel_state` CustomPayload | `agent/packages/schema/src/identity.ts` + `server/src/network/identity_panel_emit.rs`（新）|
| persistence schema `player_identities` table | `server/src/persistence/identity.rs`（新）|

---

## §9 决议（立项时已闭环 14 项）

调研锚点：worldview §十一 全节 (commit fe00532c) + §五 末土后招原则 + plan-social-v1 ✅（Renown / Anonymity / ExposureEvent 已实装） + plan-dugu-v1 ✅（DuguRevealedEvent 已 emit + identity stub 留口） + plan-niche-defense-v1 ⏳（灵龛系统 ~5%）+ 6 流派 plan 全 ✅ finished + plan-persistence-v1 ✅。

| # | 问题 | 决议 | 落地点 |
|---|------|------|--------|
| **Q1** | 多 identity 数据模型 | ✅ A：`PlayerIdentities { identities: Vec<IdentityProfile>, active_identity_id }` 单 Component | §2 类型定义 |
| **Q2** | player vs NPC 共用系统 | ✅ B：仅玩家先做，NPC 自身 identity vN+1 | §0 设计轴心 |
| **Q3** | 初始 identity 创建时机 | ✅ B + 改名灵活：玩家首次进游戏自动创建默认 identity，display_name = MC username（可后续 `/identity rename` 改自定义） | §2 默认 identity 创建（不弹强制 UI 提示） |
| **Q4** | 身份切换交互 | ✅ A + C：`/identity` slash command **限自己灵龛 5 格内执行**（同时满足 worldview 灵龛 = 安全空间 + dev test 友好），无 op 权限分级 | §3 全节 |
| **Q5** | 洗白代价"人脉归零"含义 | ✅ B：旧身份冻结待复用（数据完整保留，frozen=true），未来切回 NPC 仍认识 | §0 + §3 切换流程 |
| **Q5b** | 切回机制 | ✅ A：可双向切换，`/identity switch <id>` 接受任意 frozen identity | §3 命令表 |
| **Q6** | 洗白代价"物资归零"含义 | ✅ **撤销**：worldview 中 "人脉/物资关系" 是社交关系层（Relationships），不指物品；切身份不动背包 | §0 设计轴心 |
| **Q6b** | 洗白代价剩什么 | ✅ B：切换冷却 1 game-day（24000 ticks，vanilla MC day），无灵龛费 + 无 fame 衰减 | §3 切换冷却 |
| **Q7** | NPC 信誉度阈值 | ✅ B 复合：`reputation_score = fame - notoriety - tag_penalty_sum`，4 档 +50 / -25 / -75 边界 | §4 公式 + §5 reaction_tier |
| **Q8** | NPC 行为反应实装路径 | ✅ B + C：`IdentityReactionChangedEvent` 推 NPC blackboard cached + big-brain `IdentityReactionScorer` | §5 全节 |
| **Q9** | 毒蛊师 -50 baseline 衰减规则 | ✅ C：仅切身份消除（旧 identity 仍带 tag，未来切回会再触发） | §4 切身份消除段 |
| **Q10** | NPC 间传话扩散模型 | ✅ A：同 zone NPC 概率扩散，留 P5 实装（ROI 较低） | §7 全节 |
| **Q11** | 6 流派 vN+1 RevealedEvent 模板 | ✅ A：本 plan P4 提供统一 `RevealedEvent` trait + 通用 consumer + 各流派接入文档；6 流派 vN+1 共用 | §6 全节 |
| **Q12** | agent 接入 | ✅ B：仅 Wanted 档 emit `bong:wanted_player` event，agent calamity skill 加 wanted_player 处理段；不全 identity 状态推送 | §7 agent 接入段 |

> **本 plan 无未拍开放问题**——P0 可立刻起。P5 的传话扩散 / agent narration / client UI 是高 ROI 但量大，可分多 PR。

---

## §10 进度日志

- **2026-05-04 立项**：骨架立项。来源：worldview §十一 commit fe00532c 已正典化"身份与信誉"系统 + plan-dugu-v1 已留 `DuguRevealedEvent` stub 等 consumer + 6 流派 vN+1 等通用 RevealedEvent trait + plan-niche-defense-v1 灵龛系统协同。**关键发现**：基础组件大半已实装（plan-social-v1 finished：Renown / Anonymity / ExposureEvent / Relationship / Relationships / FactionMembership / SocialRenownDeltaEvent + dugu DuguRevealedEvent + DuguRevealedEventV1 schema），本 plan 在其上扩多 identity 包装 + 反应分级 + 切换机制。14 决议（Q1-Q12 + Q5b + Q6b）一次性闭环。
- **2026-05-08 P0–P4 ✅ + P5 部分完成**：通过 `/consume-plan identity-v1` 8 个 atomic commit 落地。
- **2026-05-09 P5 ✅**：补齐 gossip、`identity_panel_state` server emit、client router/store/HUD/面板、server-data union，以及 trade/chase 直接接线；全量验证通过（详见 Finish Evidence）。

---

## Finish Evidence

> 本节由 `/consume-plan` 在 worktree `auto/plan-identity-v1` 更新（2026-05-09）。
> P0–P5 全部完成；非阻断后续仅保留 runClient 手验、per-NPC 视角缓存细化和 library 条目。

### 落地清单

| 阶段 | 真实模块 / 文件路径 |
|------|---------------------|
| P0 | `server/src/identity/mod.rs`（IdentityId / IdentityProfile / RevealedTag / RevealedTagKind 10 变体 / PlayerIdentities Component / IdentityRegistry Resource / reputation_score / IDENTITY_SWITCH_COOLDOWN_TICKS=24000 / attach_identity_bundle_to_joined_clients）；`server/src/persistence/identity.rs`（v17 migration + save_player_identities/load_player_identities）；`server/src/persistence/mod.rs`（CURRENT_USER_VERSION=17 + open_persistence_connection 升级 pub(crate)）|
| P1 | `server/src/identity/events.rs`（IdentityCreatedEvent / IdentitySwitchedEvent / IdentityReactionChangedEvent）；`server/src/identity/precondition.rs`（within_own_niche + check_within_own_niche + NichePreconditionError）；`server/src/identity/command.rs`（/identity slash list/new/switch/rename + apply_* 纯函数 + IdentityCmdError 7 错误分支）；`server/src/social/mod.rs`（新增 `position_is_within_own_active_spirit_niche` 公共 helper + SpiritNicheRegistry::upsert 升级 pub(crate)）|
| P2 | `server/src/identity/dugu_consumer.rs`（write_dugu_tag_if_absent + register；dedup by kind；P4 后 system 委派给 revealed::consume_revealed_event::<DuguRevealedEvent>）|
| P3 | `server/src/identity/reaction.rs`（ReactionTier 4 档 + reaction_tier()/reaction_tier_of() + scorer_value/npc_declines_trade/npc_seeks_attack helpers + IdentityReactionState Component + update_identity_reaction_state system）；`server/src/identity/scorer.rs`（IdentityReactionScorer big-brain Component + identity_reaction_scorer_system）；`server/src/social/mod.rs`（Wanted/Low active identity 拒绝 trade offer）；`server/src/npc/brain.rs`（Wanted active identity 直接拉满 ChaseTargetScorer）|
| P4 | `server/src/identity/revealed.rs`（RevealedEvent trait + DuguRevealedEvent impl + consume_revealed_event<E> 泛型 system + write_revealed_tag_if_absent helper）；`server/src/identity/README.md`（6 流派 vN+1 接入指南 + RevealedTagKind 全枚举 baseline_penalty 设计建议 + 触发条件示例 + 测试模板 + grep 抓手）|
| P5 | `agent/packages/schema/src/identity.ts`（RevealedTagKindV1 / ReactionTierV1 / WantedPlayerEventV1 / IdentityPanelEntryV1 / IdentityPanelStateV1）；`agent/packages/schema/src/server-data.ts` + `generated/server-data-v1.json` + `samples/server-data.identity-panel-state.sample.json`（`identity_panel_state` server_data union）；`agent/packages/schema/src/channels.ts` `WANTED_PLAYER`；`server/src/schema/identity.rs`（Rust serde 镜像）；`server/src/schema/channels.rs` `CH_WANTED_PLAYER`；`server/src/network/redis_bridge.rs`（RedisOutbound::WantedPlayer + match arm）；`server/src/identity/wanted_player_emit.rs`（build_wanted_player_event + emit_wanted_player_to_redis system）；`server/src/identity/gossip.rs`（record_dugu_gossip_witness + npc_identity_gossip_tick）；`server/src/network/identity_panel_emit.rs`（server_data identity_panel_state CustomPayload）；`agent/packages/tiandao/src/skills/calamity.md`（通缉令段落）；`client/src/main/java/com/bong/client/identity/{IdentityPanelEntry,IdentityPanelState,IdentityPanelStateStore,IdentityHudCornerLabel,IdentityPanelScreen,IdentityPanelScreenBootstrap}.java`；`client/src/main/java/com/bong/client/network/IdentityPanelStateHandler.java`；`client/src/main/java/com/bong/client/hud/BongHudOrchestrator.java` |

### 关键 commit

| commit | 日期 | 一句话 |
|---|---|---|
| `e78be8474` | 2026-05-08 | P0 数据模型 + 默认 identity + persistence |
| `b683ae543` | 2026-05-08 | P1 /identity slash + WithinOwnNiche + 切换冷却 |
| `70bb8ef4e` | 2026-05-08 | P2 DuguRevealedEvent consumer + 毒蛊师 -50 baseline |
| `50c58a77d` | 2026-05-08 | P3 ReactionTier + IdentityReactionScorer + 反应分级状态机 |
| `9a97374ab` | 2026-05-08 | P4 RevealedEvent trait + 通用 consumer + 6 流派 README |
| `0206a32cd` | 2026-05-08 | P5a 身份/反应分级/通缉令 IPC schema (TS + samples) |
| `dfb8633d9` | 2026-05-08 | P5b Rust schema mirror + bong:wanted_player Redis pub |
| `eb58096e6` | 2026-05-08 | P5c calamity 通缉令段落 + client HUD/state classes |
| `01533cb59` | 2026-05-09 | P5d gossip / identity_panel_state server emit / trade + chase 直接接线 |
| `1d988c117` | 2026-05-09 | P5e client 面板同步 / HUD 挂接 / server-data union + sample |
| `65876d2bc` | 2026-05-09 | Review 修复：同 tick gossip 聚合 + identity panel 冷却期刷新 |
| `b55892a97` | 2026-05-09 | Review 修复：通缉身份发起交易时拒绝，不向目标发送 offer |

### 测试结果

```bash
cd server && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
# ✅ 3175 passed

cd client && JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64 ./gradlew test build
# ✅ BUILD SUCCESSFUL

cd agent && npm run build
# ✅ @bong/schema tsc + @bong/tiandao tsc

cd agent/packages/schema && npm run generate:check
# ✅ generated schema artifacts are fresh (304 files)

cd agent/packages/schema && npm test
# ✅ 328 passed

cd agent/packages/tiandao && npm test
# ✅ 286 passed
```

server identity 相关新增/覆盖单测分布：
- `identity::tests`（mod.rs）：32
- `identity::dugu_consumer::tests`：8
- `identity::reaction::tests`：24（含 5 system-level integration test）
- `identity::revealed::tests`：9（含 2 generic consumer 集成测）
- `identity::scorer::tests`：2
- `identity::command::tests`：30
- `identity::precondition::tests`：8
- `identity::wanted_player_emit::tests`：7
- `persistence::identity::tests`：3
- `schema::identity::tests`：5
- 新增本轮 P5：`identity::gossip::tests` 3、`network::identity_panel_emit::tests` 4、`npc::brain::tests::chase_target_scorer_boosts_wanted_identity_even_outside_normal_range`、`social::tests::trade_offer_dispatch_rejects_target_with_wanted_identity`

### 跨仓库核验

- **server**：`mod identity` 注册于 `main.rs`；`identity::register` 注册 `attach_identity_bundle_to_joined_clients`、`/identity` slash、reaction state machine、IdentityReactionScorer、wanted_player Redis emit、gossip 扩散、`identity_panel_state` server_data emit、`consume_revealed_event::<DuguRevealedEvent>`；`ServerDataType::IdentityPanelState` 在 `payload_type_label()` 与 serde wire union 一线。
- **agent**：`@bong/schema` 导出 `WantedPlayerEventV1` / `IdentityPanelStateV1` / `RevealedTagKindV1` / `ReactionTierV1`；`CHANNELS.WANTED_PLAYER = "bong:wanted_player"`；`ServerDataV1` 接受 `type: "identity_panel_state"`；`tiandao/src/skills/calamity.md` 加「通缉令」段。
- **client**：`ServerDataRouter.createDefault()` 注册 `identity_panel_state`；`BongNetworkHandler.applyDispatch()` 写入 `IdentityPanelStateStore`；`BongHudOrchestrator` 挂 `IdentityHudCornerLabel`；`IdentityPanelScreenBootstrap` 按 O 打开 `IdentityPanelScreen`，按钮通过 `sendCommand("identity ...")` 交给 server 权威校验。

### 遗留 / 后续（非阻断）

- **client 真机手验**：本轮通过 `./gradlew test build`，未跑 `./gradlew runClient`。
- **per-NPC 视角缓存**：本轮 gossip 已按同 zone / 同 faction 扩散并触发 reaction event；若后续需要“每个 NPC 对同一玩家有不同 tier”，再加 per-NPC tier cache。
- **plan-niche-defense-v1 协同**：`WithinOwnNiche` 当前走 social mod 的 SpiritNicheRegistry；niche 被识破 / 摧毁后的身份耦合留给 niche-defense 后续。
- **图书馆条目**：plan 头部点名的 `peoples-XXXX 拾名录` 未写，仍由 library-curator 后续补。
