//! 玩家身份系统（plan-identity-v1）。
//!
//! 玩家可在自己灵龛 5 格内通过 `/identity` 维护多 identity（list / new / switch / rename）。
//! 每个 identity 独立持有 [`Renown`] 与 [`RevealedTag`] 列表，切换 identity 即"换一套 NPC
//! 信誉度账本"。worldview §十一 把这套机制定为末法残土的洗白通道：旧 identity 冻结待复用、
//! 新 identity 信誉从 0 开始；毒蛊师等 permanent tag 仅在切回旧 identity 时再触发。
//!
//! 子模块：
//! - [`events`]：P1 `IdentityCreatedEvent` / `IdentitySwitchedEvent` Bevy 事件
//! - [`precondition`]：P1 `WithinOwnNiche` precondition（玩家须在自己灵龛 5 格内）
//! - [`command`]：P1 `/identity list / new / switch / rename` slash command
//! - `DuguRevealedEvent` consumer 在 P2 (`dugu_consumer.rs`)；NPC 反应分级在 P3
//! - 通用 `RevealedEvent` trait 在 P4 (`revealed.rs`)；gossip + agent + client UI 在 P5

pub mod command;
pub mod dugu_consumer;
pub mod events;
pub mod precondition;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use valence::prelude::{
    bevy_ecs, Added, App, Component, Entity, IntoSystemConfigs, Query, Res, ResMut, Resource,
    Update, Username, With,
};

use crate::cultivation::components::Realm;
use crate::npc::movement::GameTick;
use crate::persistence::{identity as identity_db, PersistenceSettings};
use crate::player::state::canonical_player_id;
use crate::social::components::Renown;

/// 切换冷却 = vanilla 1 game-day（24000 ticks）。
///
/// worldview §十一 "切换冷却 1 game-day" + plan-jiezeq-v1 沿用 vanilla MC day 长度。
/// rename 不消耗冷却（仅 new / switch 占用）。
pub const IDENTITY_SWITCH_COOLDOWN_TICKS: u64 = 24_000;

/// 玩家本地 identity id（per-player local，0 = 首次进游戏自动创建的默认 identity）。
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct IdentityId(pub u32);

impl IdentityId {
    pub const DEFAULT: Self = Self(0);

    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// 玩家持有的所有 identity 集合 + 当前激活 id + 上次切换 tick。
#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq)]
pub struct PlayerIdentities {
    pub identities: Vec<IdentityProfile>,
    pub active_identity_id: IdentityId,
    pub last_switch_tick: u64,
}

impl Default for PlayerIdentities {
    fn default() -> Self {
        Self {
            identities: Vec::new(),
            active_identity_id: IdentityId::DEFAULT,
            last_switch_tick: 0,
        }
    }
}

impl PlayerIdentities {
    /// 用 MC username 创建首次进游戏的默认 identity（id=0，display_name=username，frozen=false）。
    pub fn with_default(display_name: impl Into<String>, created_at_tick: u64) -> Self {
        let profile = IdentityProfile::new(IdentityId::DEFAULT, display_name, created_at_tick);
        Self {
            identities: vec![profile],
            active_identity_id: IdentityId::DEFAULT,
            last_switch_tick: 0,
        }
    }

    pub fn active(&self) -> Option<&IdentityProfile> {
        self.identities
            .iter()
            .find(|p| p.id == self.active_identity_id)
    }

    pub fn active_mut(&mut self) -> Option<&mut IdentityProfile> {
        let active_id = self.active_identity_id;
        self.identities.iter_mut().find(|p| p.id == active_id)
    }

    pub fn get(&self, id: IdentityId) -> Option<&IdentityProfile> {
        self.identities.iter().find(|p| p.id == id)
    }

    pub fn get_mut(&mut self, id: IdentityId) -> Option<&mut IdentityProfile> {
        self.identities.iter_mut().find(|p| p.id == id)
    }

    /// 计算下一个可用的 IdentityId（在已有列表上 +1）。
    pub fn next_id(&self) -> IdentityId {
        self.identities
            .iter()
            .map(|p| p.id.0)
            .max()
            .map(|max| IdentityId(max.saturating_add(1)))
            .unwrap_or(IdentityId::DEFAULT)
    }

    /// 检查切换冷却：`last_switch_tick + IDENTITY_SWITCH_COOLDOWN_TICKS <= now`。
    pub fn cooldown_passed(&self, now_tick: u64) -> bool {
        if self.last_switch_tick == 0 {
            return true; // 从未切换过
        }
        now_tick.saturating_sub(self.last_switch_tick) >= IDENTITY_SWITCH_COOLDOWN_TICKS
    }

    /// 剩余冷却 ticks（已过则 0）。
    pub fn cooldown_remaining(&self, now_tick: u64) -> u64 {
        if self.last_switch_tick == 0 {
            return 0;
        }
        let elapsed = now_tick.saturating_sub(self.last_switch_tick);
        IDENTITY_SWITCH_COOLDOWN_TICKS.saturating_sub(elapsed)
    }
}

/// 单个 identity 档案（display_name + Renown + revealed_tags + frozen 标志）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdentityProfile {
    pub id: IdentityId,
    pub display_name: String,
    pub created_at_tick: u64,
    pub renown: Renown,
    pub revealed_tags: Vec<RevealedTag>,
    pub frozen: bool,
}

impl IdentityProfile {
    pub fn new(id: IdentityId, display_name: impl Into<String>, created_at_tick: u64) -> Self {
        Self {
            id,
            display_name: display_name.into(),
            created_at_tick,
            renown: Renown::default(),
            revealed_tags: Vec::new(),
            frozen: false,
        }
    }

    pub fn rename(&mut self, new_name: impl Into<String>) {
        self.display_name = new_name.into();
    }

    /// 当前 identity 的 reputation score（fame - notoriety - sum(tag_baseline_penalty)）。
    pub fn reputation_score(&self) -> i32 {
        reputation_score(self)
    }

    /// 是否已带某个 RevealedTagKind（用于幂等去重写入）。
    pub fn has_tag(&self, kind: RevealedTagKind) -> bool {
        self.revealed_tags.iter().any(|t| t.kind == kind)
    }
}

/// "被识破"事件落地后写入的 tag，绑定到具体 identity。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RevealedTag {
    pub kind: RevealedTagKind,
    pub witnessed_at_tick: u64,
    pub witness_realm: Realm,
    /// 是否永久（true 不衰减；毒蛊师 = true，其他流派 vN+1 自定）。
    pub permanent: bool,
}

/// 招式 / 流派识别 tag 全枚举（worldview §五 "招式 tag 可被识破"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevealedTagKind {
    /// 毒蛊师暴露（worldview §十一 -50 baseline 永久）。
    DuguRevealed,
    /// 暗器流（plan-anqi-v1 vN+1 hook，本 plan 仅声明 enum 变体）。
    AnqiMaster,
    /// 阵法流（plan-zhenfa-v1 vN+1 hook）。
    ZhenfaMaster,
    /// 爆脉流（plan-baomai-v1 vN+1 hook）。
    BaomaiUser,
    /// 替尸流（plan-tuike-v1 vN+1 hook）。
    TuikeUser,
    /// 涡流流（plan-woliu-v1 vN+1 hook）。
    WoliuMaster,
    /// 截脉流（plan-zhenmai-v1 vN+1 hook）。
    ZhenmaiUser,
    /// 通用招式 tag（非流派，预留 sword/forge/alchemy 类纯名声）。
    SwordMaster,
    /// 炼器名声。
    ForgeMaster,
    /// 炼丹名声。
    AlchemyMaster,
}

impl RevealedTagKind {
    /// 该 tag 落地后的 baseline penalty（reputation_score 累减项）。
    ///
    /// worldview §十一 仅明示 dugu = -50 baseline；其他流派 vN+1 自定 penalty，
    /// 当前阶段返回 0 做 hook（避免破坏现有 reputation_score 公式）。
    pub const fn baseline_penalty(self) -> i32 {
        match self {
            Self::DuguRevealed => 50,
            _ => 0,
        }
    }

    /// 是否永久 tag（不允许 vN+1 衰减）。worldview §十一 仅 dugu 永久。
    pub const fn is_permanent_default(self) -> bool {
        matches!(self, Self::DuguRevealed)
    }
}

/// 全局 identity 索引：用于 NPC 间传话扩散（P5）+ agent wanted_player 查询。
///
/// 当前 P0 阶段空骨架：仅声明结构 + register；P5 实装 gossip 时填充。
#[derive(Debug, Default, Resource)]
pub struct IdentityRegistry {
    pub by_player_uuid: HashMap<Uuid, IdentityRegistryEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct IdentityRegistryEntry {
    pub char_id: String,
    pub active_display_name: String,
    pub active_identity_id: IdentityId,
    pub reputation_score: i32,
}

/// reputation_score = fame - notoriety - sum(tag_baseline_penalty)。
///
/// worldview §十一 "毒蛊师 -50 baseline" + 4 档分级（High > 50 / Normal -25..50 / Low
/// -75..-25 / Wanted < -75）由 P3 [`crate::identity::reaction::reaction_tier`] 消费。
pub fn reputation_score(identity: &IdentityProfile) -> i32 {
    let fame = identity.renown.fame;
    let notoriety = identity.renown.notoriety;
    let tag_penalty: i32 = identity
        .revealed_tags
        .iter()
        .map(|t| t.kind.baseline_penalty())
        .sum();
    fame.saturating_sub(notoriety).saturating_sub(tag_penalty)
}

/// 注册 identity 模块（events / resource / systems）。
pub fn register(app: &mut App) {
    app.init_resource::<IdentityRegistry>();
    app.add_systems(
        Update,
        attach_identity_bundle_to_joined_clients
            .after(crate::player::attach_player_state_to_joined_clients),
    );
    command::register(app);
    dugu_consumer::register(app);
}

/// 玩家加入时附挂 [`PlayerIdentities`] Component。
///
/// 已存在持久化记录 → 从 SQLite 读取；否则 → 创建默认 identity（id=0,
/// display_name=MC username）。**不弹强制 UI 提示**——玩家可后续 `/identity rename` 自定义。
type JoinedClientIdentityQuery<'world, 'state> = Query<
    'world,
    'state,
    (Entity, &'static Username),
    (Added<Username>, With<valence::prelude::Client>),
>;

fn attach_identity_bundle_to_joined_clients(
    mut commands: valence::prelude::Commands,
    new_clients: JoinedClientIdentityQuery<'_, '_>,
    persistence: Option<Res<PersistenceSettings>>,
    game_tick: Option<Res<GameTick>>,
    mut registry: ResMut<IdentityRegistry>,
) {
    let now_tick = game_tick.map(|t| t.0 as u64).unwrap_or(0);
    for (entity, username) in new_clients.iter() {
        let username_str = username.as_str();
        let char_id = canonical_player_id(username_str);

        let loaded = persistence.as_deref().and_then(|settings| {
            identity_db::load_player_identities(settings, &char_id)
                .ok()
                .flatten()
        });

        let identities = loaded.unwrap_or_else(|| {
            // 首次进入：用 MC username 作为默认 display_name。
            PlayerIdentities::with_default(username_str.to_string(), now_tick)
        });

        if let Some(active) = identities.active() {
            registry.by_player_uuid.insert(
                Uuid::new_v5(&Uuid::NAMESPACE_OID, char_id.as_bytes()),
                IdentityRegistryEntry {
                    char_id: char_id.clone(),
                    active_display_name: active.display_name.clone(),
                    active_identity_id: active.id,
                    reputation_score: reputation_score(active),
                },
            );
        }

        commands.entity(entity).insert(identities);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::social::RenownTagV1;

    #[test]
    fn identity_id_default_is_zero() {
        assert_eq!(IdentityId::DEFAULT, IdentityId(0));
    }

    #[test]
    fn identity_id_next_increments() {
        assert_eq!(IdentityId(0).next(), IdentityId(1));
        assert_eq!(IdentityId(5).next(), IdentityId(6));
    }

    #[test]
    fn identity_id_next_saturates_at_u32_max() {
        assert_eq!(IdentityId(u32::MAX).next(), IdentityId(u32::MAX));
    }

    #[test]
    fn player_identities_default_is_empty() {
        let pid = PlayerIdentities::default();
        assert!(pid.identities.is_empty());
        assert_eq!(pid.active_identity_id, IdentityId::DEFAULT);
        assert_eq!(pid.last_switch_tick, 0);
        assert!(pid.active().is_none(), "default 集合无 active profile");
    }

    #[test]
    fn with_default_uses_mc_username() {
        let pid = PlayerIdentities::with_default("kiz", 100);
        assert_eq!(pid.identities.len(), 1);
        let profile = pid.active().expect("default identity 必须 active");
        assert_eq!(profile.display_name, "kiz");
        assert_eq!(profile.id, IdentityId::DEFAULT);
        assert_eq!(profile.created_at_tick, 100);
        assert!(!profile.frozen);
        assert!(profile.revealed_tags.is_empty());
    }

    #[test]
    fn identity_profile_default_renown_is_zero() {
        let profile = IdentityProfile::new(IdentityId(0), "test", 0);
        assert_eq!(profile.renown.fame, 0);
        assert_eq!(profile.renown.notoriety, 0);
        assert!(profile.renown.tags.is_empty());
    }

    #[test]
    fn next_id_returns_default_for_empty_set() {
        let pid = PlayerIdentities::default();
        assert_eq!(pid.next_id(), IdentityId::DEFAULT);
    }

    #[test]
    fn next_id_returns_max_plus_one() {
        let mut pid = PlayerIdentities::with_default("a", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(3), "b", 0));
        pid.identities
            .push(IdentityProfile::new(IdentityId(7), "c", 0));
        assert_eq!(pid.next_id(), IdentityId(8));
    }

    #[test]
    fn cooldown_passed_true_when_never_switched() {
        let pid = PlayerIdentities::with_default("a", 0);
        assert!(pid.cooldown_passed(0));
        assert!(pid.cooldown_passed(1_000_000));
    }

    #[test]
    fn cooldown_passed_respects_24000_tick_window() {
        let mut pid = PlayerIdentities::with_default("a", 0);
        pid.last_switch_tick = 100;
        // 100 + 24000 = 24100；< 24100 → 未过；>= 24100 → 过
        assert!(!pid.cooldown_passed(24_099));
        assert!(pid.cooldown_passed(24_100));
        assert!(pid.cooldown_passed(50_000));
    }

    #[test]
    fn cooldown_remaining_decrements() {
        let mut pid = PlayerIdentities::with_default("a", 0);
        pid.last_switch_tick = 100;
        assert_eq!(pid.cooldown_remaining(100), IDENTITY_SWITCH_COOLDOWN_TICKS);
        assert_eq!(pid.cooldown_remaining(1100), 23_000);
        assert_eq!(pid.cooldown_remaining(24_100), 0);
        assert_eq!(pid.cooldown_remaining(50_000), 0); // saturates at 0
    }

    #[test]
    fn cooldown_remaining_zero_when_never_switched() {
        let pid = PlayerIdentities::with_default("a", 0);
        assert_eq!(pid.cooldown_remaining(123_456), 0);
    }

    #[test]
    fn revealed_tag_kind_dugu_baseline_penalty_is_50() {
        assert_eq!(RevealedTagKind::DuguRevealed.baseline_penalty(), 50);
    }

    #[test]
    fn revealed_tag_kind_other_baseline_penalty_is_zero() {
        assert_eq!(RevealedTagKind::AnqiMaster.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::ZhenfaMaster.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::BaomaiUser.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::TuikeUser.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::WoliuMaster.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::ZhenmaiUser.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::SwordMaster.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::ForgeMaster.baseline_penalty(), 0);
        assert_eq!(RevealedTagKind::AlchemyMaster.baseline_penalty(), 0);
    }

    #[test]
    fn revealed_tag_kind_only_dugu_is_permanent_default() {
        assert!(RevealedTagKind::DuguRevealed.is_permanent_default());
        for kind in [
            RevealedTagKind::AnqiMaster,
            RevealedTagKind::ZhenfaMaster,
            RevealedTagKind::BaomaiUser,
            RevealedTagKind::TuikeUser,
            RevealedTagKind::WoliuMaster,
            RevealedTagKind::ZhenmaiUser,
            RevealedTagKind::SwordMaster,
            RevealedTagKind::ForgeMaster,
            RevealedTagKind::AlchemyMaster,
        ] {
            assert!(!kind.is_permanent_default(), "{kind:?} 不应为永久默认");
        }
    }

    #[test]
    fn reputation_score_no_tags_no_renown_is_zero() {
        let profile = IdentityProfile::new(IdentityId(0), "test", 0);
        assert_eq!(reputation_score(&profile), 0);
    }

    #[test]
    fn reputation_score_with_dugu_tag_is_negative_50() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 100,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        assert_eq!(reputation_score(&profile), -50);
    }

    #[test]
    fn reputation_score_combines_fame_notoriety_tags() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        profile.renown.fame = 100;
        profile.renown.notoriety = 30;
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 100,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        // 100 - 30 - 50 = 20
        assert_eq!(reputation_score(&profile), 20);
    }

    #[test]
    fn reputation_score_non_dugu_tag_does_not_penalize() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::AnqiMaster,
            witnessed_at_tick: 100,
            witness_realm: Realm::Spirit,
            permanent: false,
        });
        assert_eq!(reputation_score(&profile), 0);
    }

    #[test]
    fn reputation_score_sums_multiple_dugu_tags() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        for _ in 0..3 {
            profile.revealed_tags.push(RevealedTag {
                kind: RevealedTagKind::DuguRevealed,
                witnessed_at_tick: 100,
                witness_realm: Realm::Spirit,
                permanent: true,
            });
        }
        // 三次 dugu tag 累计 -150（同 kind 多次写入是允许的，去重靠 has_tag 调用方判断）
        assert_eq!(reputation_score(&profile), -150);
    }

    #[test]
    fn identity_profile_rename_changes_display_name() {
        let mut profile = IdentityProfile::new(IdentityId(0), "old", 0);
        profile.rename("new");
        assert_eq!(profile.display_name, "new");
    }

    #[test]
    fn identity_profile_has_tag_detects_existing() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        assert!(!profile.has_tag(RevealedTagKind::DuguRevealed));
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 0,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        assert!(profile.has_tag(RevealedTagKind::DuguRevealed));
        assert!(!profile.has_tag(RevealedTagKind::AnqiMaster));
    }

    #[test]
    fn active_returns_currently_active_profile() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 0));
        // 当前 active = 0
        assert_eq!(pid.active().unwrap().display_name, "kiz");
        pid.active_identity_id = IdentityId(1);
        assert_eq!(pid.active().unwrap().display_name, "alt");
    }

    #[test]
    fn active_mut_allows_in_place_modification() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.active_mut().unwrap().rename("new");
        assert_eq!(pid.active().unwrap().display_name, "new");
    }

    #[test]
    fn get_returns_profile_by_id() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(7), "alt", 100));
        assert_eq!(pid.get(IdentityId(7)).unwrap().display_name, "alt");
        assert!(pid.get(IdentityId(99)).is_none());
    }

    #[test]
    fn player_identities_serializes_round_trip() {
        let mut pid = PlayerIdentities::with_default("kiz", 100);
        pid.identities
            .push(IdentityProfile::new(IdentityId(1), "alt", 200));
        pid.identities[1].renown.fame = 30;
        pid.identities[1].renown.notoriety = 10;
        pid.identities[1].renown.tags.push(RenownTagV1 {
            tag: "test_tag".to_string(),
            weight: 5.0,
            last_seen_tick: 200,
            permanent: false,
        });
        pid.identities[1].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 300,
            witness_realm: Realm::Solidify,
            permanent: true,
        });
        pid.identities[1].frozen = true;
        pid.last_switch_tick = 12_345;
        pid.active_identity_id = IdentityId(1);

        let json = serde_json::to_string(&pid).expect("serialize");
        let recovered: PlayerIdentities = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(pid, recovered);
    }

    #[test]
    fn revealed_tag_kind_serde_snake_case() {
        let kind = RevealedTagKind::DuguRevealed;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, r#""dugu_revealed""#);

        let parsed: RevealedTagKind = serde_json::from_str(r#""anqi_master""#).unwrap();
        assert_eq!(parsed, RevealedTagKind::AnqiMaster);
    }

    #[test]
    fn identity_registry_entry_is_default_constructible() {
        let entry = IdentityRegistryEntry::default();
        assert_eq!(entry.active_identity_id, IdentityId::DEFAULT);
        assert_eq!(entry.reputation_score, 0);
        assert!(entry.char_id.is_empty());
        assert!(entry.active_display_name.is_empty());
    }

    #[test]
    fn identity_registry_entry_round_trip_via_attach_path() {
        // 模拟 attach_identity_bundle_to_joined_clients 写入 registry 的字段填充
        let mut pid = PlayerIdentities::with_default("kiz", 100);
        pid.identities[0].renown.fame = 80;
        pid.identities[0].renown.notoriety = 20;
        let active = pid.active().expect("active");
        let entry = IdentityRegistryEntry {
            char_id: "offline:kiz".to_string(),
            active_display_name: active.display_name.clone(),
            active_identity_id: active.id,
            reputation_score: reputation_score(active),
        };
        assert_eq!(entry.char_id, "offline:kiz");
        assert_eq!(entry.active_display_name, "kiz");
        assert_eq!(entry.active_identity_id, IdentityId::DEFAULT);
        assert_eq!(entry.reputation_score, 60);
    }

    #[test]
    fn identity_profile_reputation_score_method_matches_free_function() {
        let mut profile = IdentityProfile::new(IdentityId(0), "test", 0);
        profile.renown.fame = 42;
        profile.renown.notoriety = 7;
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 0,
            witness_realm: Realm::Solidify,
            permanent: true,
        });
        assert_eq!(profile.reputation_score(), reputation_score(&profile));
        assert_eq!(profile.reputation_score(), -15);
    }

    #[test]
    fn get_mut_returns_mutable_profile_by_id() {
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities
            .push(IdentityProfile::new(IdentityId(3), "alt", 0));
        let alt = pid.get_mut(IdentityId(3)).expect("alt by id");
        alt.renown.fame = 99;
        assert_eq!(pid.get(IdentityId(3)).unwrap().renown.fame, 99);
        assert!(pid.get_mut(IdentityId(99)).is_none());
    }

    #[test]
    fn cooldown_constant_is_24000_ticks() {
        assert_eq!(IDENTITY_SWITCH_COOLDOWN_TICKS, 24_000);
    }
}
