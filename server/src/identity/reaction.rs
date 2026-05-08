//! NPC 反应分级 4 档（plan-identity-v1 P3）。
//!
//! worldview §十一 锚点：
//! - High > 50 → 主动给情报 / 折扣 / 接私活
//! - Normal -25..=50 → 正常交易
//! - Low -75..=-26 → 加价 / 拒绝服务 / NPC 间传话扩散
//! - Wanted < -75 → 通缉，agent 主动追杀 narration（agent 接入在 P5）
//!
//! 边界规则（exhaustive）：
//! - 51 → High（s > 50）
//! - 50 → Normal（s == 50 落 Normal 桶）
//! - -25 → Normal（s == -25 仍是 Normal）
//! - -26 → Low
//! - -75 → Low（s == -75 仍是 Low）
//! - -76 → Wanted

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, Entity, EventWriter, Query, Res, Update, With,
};

use super::events::IdentityReactionChangedEvent;
use super::{reputation_score, IdentityProfile, PlayerIdentities};
use crate::npc::movement::GameTick;

/// 4 档反应分级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionTier {
    High,
    Normal,
    Low,
    Wanted,
}

impl ReactionTier {
    /// big-brain Scorer 用的归一化分数（0.0 ~ 1.0），用于 IdentityReactionScorer
    /// 决定 NPC 行为优先级。
    ///
    /// - Wanted → 1.0（最高优先级，NPC 倾向主动追杀 / 拒绝交易）
    /// - Low → 0.6（拒交易但非追杀）
    /// - Normal → 0.0（不触发任何 identity-driven 行为）
    /// - High → 0.0（friendly NPC，identity scorer 不输出，由其他 friendly scorer 决定）
    pub fn scorer_value(self) -> f32 {
        match self {
            Self::Wanted => 1.0,
            Self::Low => 0.6,
            Self::Normal | Self::High => 0.0,
        }
    }

    /// NPC 是否应拒绝交易。worldview §十一 Low / Wanted 档拒交易。
    pub fn npc_declines_trade(self) -> bool {
        matches!(self, Self::Low | Self::Wanted)
    }

    /// NPC 是否应主动追杀。仅 Wanted 档触发（worldview §十一 通缉）。
    pub fn npc_seeks_attack(self) -> bool {
        matches!(self, Self::Wanted)
    }
}

/// 主公式：把 reputation_score 映射到 4 档。
pub fn reaction_tier(score: i32) -> ReactionTier {
    match score {
        s if s > 50 => ReactionTier::High,
        s if s >= -25 => ReactionTier::Normal,
        s if s >= -75 => ReactionTier::Low,
        _ => ReactionTier::Wanted,
    }
}

/// 给定 IdentityProfile，直接计算其反应分级。
pub fn reaction_tier_of(profile: &IdentityProfile) -> ReactionTier {
    reaction_tier(reputation_score(profile))
}

/// NPC 是否应拒绝某玩家 active identity 的交易（plan §5 trade 拒绝）。
pub fn npc_should_decline_trade(profile: &IdentityProfile) -> bool {
    reaction_tier_of(profile).npc_declines_trade()
}

/// NPC 是否应主动追杀某玩家 active identity（plan §5 chase 优先级，仅 Wanted）。
pub fn npc_should_seek_attack(profile: &IdentityProfile) -> bool {
    reaction_tier_of(profile).npc_seeks_attack()
}

/// 缓存玩家当前的反应分级，供 NPC AI / 下游订阅 blackboard 查询。
///
/// 由 [`update_identity_reaction_state`] 维护：每 tick 检查 active identity 的
/// reputation_score → 计算新 tier；若与上次不同则更新自身 + emit
/// [`IdentityReactionChangedEvent`]。
#[derive(Debug, Clone, Component, Serialize, Deserialize, PartialEq, Eq)]
pub struct IdentityReactionState {
    pub tier: ReactionTier,
}

impl Default for IdentityReactionState {
    fn default() -> Self {
        Self {
            tier: ReactionTier::Normal,
        }
    }
}

/// 注册反应分级系统。
pub fn register(app: &mut App) {
    app.add_event::<IdentityReactionChangedEvent>()
        .add_systems(Update, update_identity_reaction_state);
}

/// 玩家 tick 维护反应分级 + 边界发射。
///
/// 行为：
/// - 玩家持有 [`PlayerIdentities`] 但无 [`IdentityReactionState`] → 立即 attach 当前 tier
///   （**不发射事件**——首次出生不算"跨边界"）
/// - 玩家有 state，新 tier 与旧 tier 不同 → 更新 state + emit 事件
/// - 同 tier → no-op
pub fn update_identity_reaction_state(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &PlayerIdentities,
            Option<&mut IdentityReactionState>,
        ),
        With<valence::prelude::Client>,
    >,
    game_tick: Option<Res<GameTick>>,
    mut writer: EventWriter<IdentityReactionChangedEvent>,
) {
    let now_tick = game_tick.map(|t| t.0 as u64).unwrap_or(0);
    for (entity, identities, state) in players.iter_mut() {
        let Some(active) = identities.active() else {
            continue;
        };
        let current_tier = reaction_tier_of(active);
        match state {
            None => {
                commands
                    .entity(entity)
                    .insert(IdentityReactionState { tier: current_tier });
            }
            Some(mut existing) => {
                if existing.tier != current_tier {
                    let from_tier = existing.tier;
                    existing.tier = current_tier;
                    writer.send(IdentityReactionChangedEvent {
                        player: entity,
                        identity_id: identities.active_identity_id,
                        from_tier,
                        to_tier: current_tier,
                        at_tick: now_tick,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{IdentityId, IdentityProfile, RevealedTag, RevealedTagKind};

    #[test]
    fn reaction_tier_high_strict_above_50() {
        assert_eq!(reaction_tier(51), ReactionTier::High);
        assert_eq!(reaction_tier(100), ReactionTier::High);
        assert_eq!(reaction_tier(i32::MAX), ReactionTier::High);
    }

    #[test]
    fn reaction_tier_normal_50_inclusive_to_neg_25_inclusive() {
        assert_eq!(reaction_tier(50), ReactionTier::Normal);
        assert_eq!(reaction_tier(0), ReactionTier::Normal);
        assert_eq!(reaction_tier(-25), ReactionTier::Normal);
    }

    #[test]
    fn reaction_tier_low_neg_26_to_neg_75() {
        assert_eq!(reaction_tier(-26), ReactionTier::Low);
        assert_eq!(reaction_tier(-50), ReactionTier::Low);
        assert_eq!(reaction_tier(-75), ReactionTier::Low);
    }

    #[test]
    fn reaction_tier_wanted_below_neg_75() {
        assert_eq!(reaction_tier(-76), ReactionTier::Wanted);
        assert_eq!(reaction_tier(-200), ReactionTier::Wanted);
        assert_eq!(reaction_tier(i32::MIN), ReactionTier::Wanted);
    }

    #[test]
    fn reaction_tier_boundary_high_normal_low_wanted_full_sweep() {
        // 完整 boundary sweep：51,50, -25,-26, -75,-76
        let cases = [
            (51, ReactionTier::High),
            (50, ReactionTier::Normal),
            (-25, ReactionTier::Normal),
            (-26, ReactionTier::Low),
            (-75, ReactionTier::Low),
            (-76, ReactionTier::Wanted),
        ];
        for (score, expected) in cases {
            assert_eq!(
                reaction_tier(score),
                expected,
                "score={score} expected={expected:?}"
            );
        }
    }

    #[test]
    fn scorer_value_wanted_max() {
        assert_eq!(ReactionTier::Wanted.scorer_value(), 1.0);
    }

    #[test]
    fn scorer_value_low_partial() {
        assert_eq!(ReactionTier::Low.scorer_value(), 0.6);
    }

    #[test]
    fn scorer_value_normal_high_zero() {
        assert_eq!(ReactionTier::Normal.scorer_value(), 0.0);
        assert_eq!(ReactionTier::High.scorer_value(), 0.0);
    }

    #[test]
    fn npc_declines_trade_only_low_and_wanted() {
        assert!(!ReactionTier::High.npc_declines_trade());
        assert!(!ReactionTier::Normal.npc_declines_trade());
        assert!(ReactionTier::Low.npc_declines_trade());
        assert!(ReactionTier::Wanted.npc_declines_trade());
    }

    #[test]
    fn npc_seeks_attack_only_wanted() {
        assert!(!ReactionTier::High.npc_seeks_attack());
        assert!(!ReactionTier::Normal.npc_seeks_attack());
        assert!(!ReactionTier::Low.npc_seeks_attack());
        assert!(ReactionTier::Wanted.npc_seeks_attack());
    }

    fn dugu_profile(extra_notoriety: i32) -> IdentityProfile {
        let mut p = IdentityProfile::new(IdentityId(0), "test", 0);
        p.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 100,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        p.renown.notoriety = extra_notoriety;
        p
    }

    #[test]
    fn dugu_only_with_no_renown_falls_in_low_tier() {
        // dugu = -50 baseline → Low (since -50 ∈ [-75, -25])
        let profile = dugu_profile(0);
        assert_eq!(reaction_tier_of(&profile), ReactionTier::Low);
    }

    #[test]
    fn dugu_with_high_notoriety_drops_to_wanted() {
        // dugu -50 + notoriety -30 (notoriety=30, fame=0, -50 - 30 = -80) → Wanted
        let profile = dugu_profile(30);
        assert_eq!(reaction_tier_of(&profile), ReactionTier::Wanted);
    }

    #[test]
    fn high_fame_overcomes_dugu_baseline() {
        let mut profile = dugu_profile(0);
        profile.renown.fame = 100;
        // 100 - 0 - 50 = 50 → Normal
        assert_eq!(reaction_tier_of(&profile), ReactionTier::Normal);
    }

    #[test]
    fn very_high_fame_pushes_to_high_tier() {
        let mut profile = dugu_profile(0);
        profile.renown.fame = 200;
        // 200 - 0 - 50 = 150 → High
        assert_eq!(reaction_tier_of(&profile), ReactionTier::High);
    }

    #[test]
    fn npc_should_decline_trade_for_low_and_wanted_tiers() {
        // -50 (Low) → decline
        assert!(npc_should_decline_trade(&dugu_profile(0)));
        // -80 (Wanted) → decline
        assert!(npc_should_decline_trade(&dugu_profile(30)));
    }

    #[test]
    fn npc_should_decline_trade_false_for_normal_and_high() {
        let mut high = IdentityProfile::new(IdentityId(0), "test", 0);
        high.renown.fame = 100;
        // 100 → High
        assert!(!npc_should_decline_trade(&high));
        let normal = IdentityProfile::new(IdentityId(0), "test", 0);
        // 0 → Normal
        assert!(!npc_should_decline_trade(&normal));
    }

    #[test]
    fn npc_should_seek_attack_only_for_wanted_tier() {
        // -50 (Low) → no attack
        assert!(!npc_should_seek_attack(&dugu_profile(0)));
        // -80 (Wanted) → attack
        assert!(npc_should_seek_attack(&dugu_profile(30)));
    }

    #[test]
    fn reaction_tier_serde_round_trip() {
        for tier in [
            ReactionTier::High,
            ReactionTier::Normal,
            ReactionTier::Low,
            ReactionTier::Wanted,
        ] {
            let json = serde_json::to_string(&tier).unwrap();
            let parsed: ReactionTier = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, tier);
        }
    }

    #[test]
    fn reaction_tier_serde_uses_snake_case() {
        assert_eq!(
            serde_json::to_string(&ReactionTier::Wanted).unwrap(),
            r#""wanted""#
        );
        assert_eq!(
            serde_json::to_string(&ReactionTier::Normal).unwrap(),
            r#""normal""#
        );
    }

    // ---- system-level integration ----

    use crate::identity::PlayerIdentities;
    use valence::prelude::{App, Entity, Events};
    use valence::testing::create_mock_client;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<IdentityReactionChangedEvent>();
        app.add_systems(Update, update_identity_reaction_state);
        app.finish();
        app.cleanup();
        app
    }

    fn spawn_player(app: &mut App, identities: PlayerIdentities) -> Entity {
        let (client_bundle, _helper) = create_mock_client("ReactionTester");
        let entity = app.world_mut().spawn(client_bundle).id();
        app.world_mut().entity_mut(entity).insert(identities);
        entity
    }

    fn collect_events(app: &App) -> Vec<IdentityReactionChangedEvent> {
        let events = app
            .world()
            .resource::<Events<IdentityReactionChangedEvent>>();
        let mut reader = events.get_reader();
        reader.read(events).cloned().collect()
    }

    #[test]
    fn first_tick_attaches_state_without_emitting_event() {
        let mut app = setup_app();
        let pid = PlayerIdentities::with_default("kiz", 0);
        let entity = spawn_player(&mut app, pid);
        app.update();

        let state = app
            .world()
            .entity(entity)
            .get::<IdentityReactionState>()
            .expect("state attached");
        assert_eq!(state.tier, ReactionTier::Normal);
        assert_eq!(collect_events(&app).len(), 0, "首次 attach 不应发射事件");
    }

    #[test]
    fn identity_reaction_changed_event_emits_on_score_crossing_boundary() {
        let mut app = setup_app();
        let pid = PlayerIdentities::with_default("kiz", 0);
        let entity = spawn_player(&mut app, pid);
        app.update();
        // 现在 tier = Normal
        // 写入 dugu tag → tier 变 Low
        {
            let mut player = app.world_mut().entity_mut(entity);
            let mut identities = player
                .get_mut::<PlayerIdentities>()
                .expect("PlayerIdentities");
            identities
                .active_mut()
                .unwrap()
                .revealed_tags
                .push(RevealedTag {
                    kind: RevealedTagKind::DuguRevealed,
                    witnessed_at_tick: 100,
                    witness_realm: crate::cultivation::components::Realm::Spirit,
                    permanent: true,
                });
        }
        app.update();

        let events = collect_events(&app);
        assert_eq!(events.len(), 1, "应发射 1 条事件，实际 {events:?}");
        assert_eq!(events[0].player, entity);
        assert_eq!(events[0].from_tier, ReactionTier::Normal);
        assert_eq!(events[0].to_tier, ReactionTier::Low);

        let state = app
            .world()
            .entity(entity)
            .get::<IdentityReactionState>()
            .unwrap();
        assert_eq!(state.tier, ReactionTier::Low);
    }

    #[test]
    fn no_event_when_tier_unchanged() {
        let mut app = setup_app();
        let pid = PlayerIdentities::with_default("kiz", 0);
        let entity = spawn_player(&mut app, pid);
        app.update();
        // 同 tier 范围内的 score 漂移：从 0 到 50（仍是 Normal）
        {
            let mut player = app.world_mut().entity_mut(entity);
            let mut identities = player
                .get_mut::<PlayerIdentities>()
                .expect("PlayerIdentities");
            identities.active_mut().unwrap().renown.fame = 50;
        }
        app.update();

        let events = collect_events(&app);
        assert!(events.is_empty(), "tier 不变不应发射事件");
    }

    #[test]
    fn wanted_tier_normalized_after_identity_switch() {
        let mut app = setup_app();
        // 初始 identity 已是 Wanted（fame=0, notoriety=30, dugu tag → -80）
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].renown.notoriety = 30;
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 50,
            witness_realm: crate::cultivation::components::Realm::Spirit,
            permanent: true,
        });
        let entity = spawn_player(&mut app, pid);
        app.update();
        let state = app
            .world()
            .entity(entity)
            .get::<IdentityReactionState>()
            .unwrap();
        assert_eq!(state.tier, ReactionTier::Wanted);

        // 切到全新 identity → tier 应回 Normal
        {
            let mut player = app.world_mut().entity_mut(entity);
            let mut identities = player
                .get_mut::<PlayerIdentities>()
                .expect("PlayerIdentities");
            identities.identities[0].frozen = true;
            identities.identities.push(IdentityProfile::new(
                crate::identity::IdentityId(1),
                "fresh",
                100,
            ));
            identities.active_identity_id = crate::identity::IdentityId(1);
            identities.last_switch_tick = 100;
        }
        app.update();

        let events = collect_events(&app);
        assert_eq!(events.len(), 1, "应发射切换后 tier 跨边界事件");
        assert_eq!(events[0].from_tier, ReactionTier::Wanted);
        assert_eq!(events[0].to_tier, ReactionTier::Normal);
        let state = app
            .world()
            .entity(entity)
            .get::<IdentityReactionState>()
            .unwrap();
        assert_eq!(state.tier, ReactionTier::Normal);
    }

    #[test]
    fn switching_back_to_dugu_identity_emits_normal_to_low_event() {
        // 切回 dugu identity → tier 从 Normal 跨回 Low
        let mut app = setup_app();
        let mut pid = PlayerIdentities::with_default("kiz", 0);
        pid.identities[0].revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 50,
            witness_realm: crate::cultivation::components::Realm::Spirit,
            permanent: true,
        });
        pid.identities.push(IdentityProfile::new(
            crate::identity::IdentityId(1),
            "fresh",
            100,
        ));
        // 当前 active = fresh (IdentityId(1))
        pid.active_identity_id = crate::identity::IdentityId(1);
        pid.identities[0].frozen = true;
        let entity = spawn_player(&mut app, pid);
        app.update();
        // 当前 active 是 fresh，无 tag → Normal
        let state = app
            .world()
            .entity(entity)
            .get::<IdentityReactionState>()
            .unwrap();
        assert_eq!(state.tier, ReactionTier::Normal);

        // 切回 dugu identity (id=0)
        {
            let mut player = app.world_mut().entity_mut(entity);
            let mut identities = player
                .get_mut::<PlayerIdentities>()
                .expect("PlayerIdentities");
            identities.active_identity_id = crate::identity::IdentityId(0);
            identities.identities[0].frozen = false;
            identities.identities[1].frozen = true;
        }
        app.update();

        let events = collect_events(&app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].from_tier, ReactionTier::Normal);
        assert_eq!(events[0].to_tier, ReactionTier::Low);
        assert_eq!(events[0].identity_id, crate::identity::IdentityId(0));
    }
}
