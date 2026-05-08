# `crate::identity` — 身份与信誉系统接入指南

本目录实装 plan-identity-v1 全部 P0–P5 范围。流派 vN+1 想接入"被识破"事件链路（让某次招式 / 神识扫描 / 战斗暴露写入玩家 active identity 的 `RevealedTag` 列表，改变 NPC 反应分级），按本指南操作。

## 1. 数据形状（必看）

```
PlayerIdentities (Component)
├─ identities: Vec<IdentityProfile>
│   └─ IdentityProfile { id, display_name, renown, revealed_tags, frozen, ... }
├─ active_identity_id: IdentityId
└─ last_switch_tick: u64

RevealedTag {
    kind: RevealedTagKind,    // 关键 enum
    witnessed_at_tick: u64,
    witness_realm: Realm,
    permanent: bool,           // 是否永远不衰减
}

RevealedTagKind 全枚举：
- DuguRevealed     (毒蛊师)            permanent=true,  baseline_penalty=50
- AnqiMaster       (暗器流)            permanent=false, baseline_penalty=0
- ZhenfaMaster     (阵法流)            permanent=false, baseline_penalty=0
- BaomaiUser       (爆脉流)            permanent=false, baseline_penalty=0
- TuikeUser        (替尸流)            permanent=false, baseline_penalty=0
- WoliuMaster      (涡流流)            permanent=false, baseline_penalty=0
- ZhenmaiUser      (截脉流)            permanent=false, baseline_penalty=0
- SwordMaster      (通用招式)          permanent=false, baseline_penalty=0
- ForgeMaster      (炼器名声)          permanent=false, baseline_penalty=0
- AlchemyMaster    (炼丹名声)          permanent=false, baseline_penalty=0
```

`baseline_penalty` 由 `RevealedTagKind::baseline_penalty()` 提供（const fn，编译期决定）。**当前正典只有 dugu = 50；6 流派 vN+1 需要 penalty 时各自扩 enum 实现 + 在 `revealed.rs` 单测里更新 `revealed_tag_kind_baseline_penalty_is_correct_per_kind`。**

## 2. RevealedEvent trait 接入流程

每个流派 vN+1 按下面三步走：

### 2.1 实现 trait

```rust
use crate::identity::{RevealedEvent, RevealedTagKind};
use crate::cultivation::components::Realm;
use valence::prelude::{Entity, Event};

#[derive(Debug, Clone, Event)]
pub struct AnqiRevealedEvent {
    pub revealed_player: Entity,
    pub witness: Entity,
    pub witness_realm: Realm,
    pub at_position: [f64; 3],
    pub at_tick: u64,
}

impl RevealedEvent for AnqiRevealedEvent {
    fn revealed_player(&self) -> Entity { self.revealed_player }
    fn witness(&self) -> Entity { self.witness }
    fn witness_realm(&self) -> Realm { self.witness_realm }
    fn revealed_tag_kind(&self) -> RevealedTagKind { RevealedTagKind::AnqiMaster }
    fn is_permanent(&self) -> bool { false }   // 暗器流可衰减；只有 dugu 永久
    fn at_tick(&self) -> u64 { self.at_tick }
    fn at_position(&self) -> [f64; 3] { self.at_position }
}
```

### 2.2 注册 generic consumer

```rust
use crate::identity::revealed::consume_revealed_event;
use valence::prelude::{App, Update};

pub fn register(app: &mut App) {
    app.add_event::<AnqiRevealedEvent>()
        .add_systems(Update, consume_revealed_event::<AnqiRevealedEvent>);
}
```

注册后所有 `AnqiRevealedEvent` 会自动写入对应玩家 active identity 的 `RevealedTag`，并触发 SQLite 持久化。Dedup by kind 自动生效——同一 active identity 内同 kind 只保留首次写入。

### 2.3 触发 emit

招式结算 / 神识扫描 / 战斗暴露的代码处 emit 即可：

```rust
events.send(AnqiRevealedEvent {
    revealed_player: player_entity,
    witness: npc_entity,
    witness_realm: cultivation.realm,
    at_position: position.get().to_array(),
    at_tick: now_tick,
});
```

## 3. 触发条件示例（设计建议）

| 流派 | 触发示例 | permanent |
|---|---|---|
| AnqiMaster | 飞针 / 暗器命中 + 被神识识破 | false |
| ZhenfaMaster | 阵法触发被同境界以上修士看到 | false |
| BaomaiUser | 爆脉招式释放被目击 | false |
| TuikeUser | 替尸 / 蜕壳后被原 zone NPC 目击 | false |
| WoliuMaster | 涡流招式被高境神识看穿 | false |
| ZhenmaiUser | 截脉招式残留被识破 | false |
| DuguRevealed | 毒蛊师身份暴露（plan-dugu-v1 既有）| **true** |

`permanent=false` 表示 RevealedTag 在 vN+1 引入的衰减系统下会被裁掉；`permanent=true` 永久，唯一消除路径 = 切身份（旧 identity frozen 仍带 tag，未来切回再触发 `baseline_penalty`）。

## 4. baseline_penalty 设计建议

只有 dugu 在 worldview §十一 明文 `-50 baseline`。若流派要加 penalty：

```rust
// in identity/mod.rs
impl RevealedTagKind {
    pub const fn baseline_penalty(self) -> i32 {
        match self {
            Self::DuguRevealed => 50,
            Self::AnqiMaster   => 10,    // 推荐：暗器流引人忌但不到拒交易
            Self::ZhenfaMaster => 0,
            // ...
            _ => 0,
        }
    }
}
```

**改 baseline_penalty 必须同步更新 `revealed.rs::tests::revealed_tag_kind_baseline_penalty_is_correct_per_kind`**——这条 pin 测试就是为了防 silent regression。

## 5. NPC 反应：不需要额外接入

写入 `RevealedTag` 后：
1. `reputation_score()` 自动累减 `sum(tag.baseline_penalty)`
2. `update_identity_reaction_state` 系统（每 tick）检测 tier 跨边界 → emit `IdentityReactionChangedEvent`
3. 持有 `IdentityReactionScorer` 的 NPC 自动给目标玩家打分（Wanted=1.0 / Low=0.6 / 其他=0）

NPC AI 若想 opt-in，spawn 时挂 scorer：
```rust
commands.spawn((Actor(npc), Score::default(), IdentityReactionScorer));
```

P3 不修改 npc/trade.rs / npc/brain.rs ChaseAction 既有逻辑——`npc_should_decline_trade()` / `npc_should_seek_attack()` 是 helper，需要时各 NPC 子系统自己 query 调用。

## 6. 测试模板

vN+1 流派测试套件按下面骨架补：

```rust
#[test]
fn anqi_revealed_event_implements_revealed_event_trait() {
    let event = AnqiRevealedEvent { /* ... */ };
    assert_eq!(event.revealed_tag_kind(), RevealedTagKind::AnqiMaster);
    assert!(!event.is_permanent());
}

#[test]
fn anqi_revealed_event_writes_revealed_tag_to_active_identity() {
    use valence::prelude::{App, Update};
    let mut app = App::new();
    app.add_event::<AnqiRevealedEvent>();
    app.add_systems(Update, consume_revealed_event::<AnqiRevealedEvent>);
    // ... spawn player, send event, app.update(), assert tag present
}

#[test]
fn anqi_tag_dedup_by_kind() {
    // 同 active identity 内同 kind 只保留一份
}

#[test]
fn anqi_tag_only_writes_to_active_identity() {
    // 多 identity 时仅当前 active 受污
}
```

## 7. 命中 grep 抓手

```bash
grep -rn 'RevealedEvent\|RevealedTagKind\|consume_revealed_event' server/src --include='*.rs'
```

应至少命中：
- `server/src/identity/revealed.rs`（trait + 泛型 consumer）
- `server/src/identity/mod.rs`（RevealedTagKind enum + baseline_penalty）
- `server/src/cultivation/dugu.rs`（DuguRevealedEvent 已 emit）
- `server/src/identity/dugu_consumer.rs`（dugu 单态 helper）
- 各流派 vN+1 自己的模块

---

> worldview §十一 锚点：身份与信誉是末法残土核心生存机制。"信息差"通过 RevealedTag 把"招式 / 流派识别 = 事件"（§五 末土后招原则）落地到 reputation_score 的累减项里。
