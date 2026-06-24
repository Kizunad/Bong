use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{bevy_ecs, Commands, Component, Entity, With};

use crate::combat::components::{Lifecycle, LifecycleState, StatusEffects};
use crate::combat::events::StatusEffectKind;
use crate::combat::status::has_active_status;
use crate::cultivation::components::{Cultivation, Realm};
use crate::identity::{reaction::npc_should_seek_attack, PlayerIdentities};
use crate::npc::lod::{
    lod_gated_score, lod_gated_score_by_kind, NpcLodConfig, NpcLodTick, NpcLodTier, ScorerKind,
};
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::spawn::{NpcBlackboard, NpcMarker, NpcMeleeProfile};

use super::{
    NpcBehaviorConfig, CHASE_RANGE, DASH_MAX_DISTANCE, DASH_MIN_DISTANCE, DEFAULT_FLEE_THRESHOLD,
};

// ---------------------------------------------------------------------------
// PlayerProximityScorer — flee trigger
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct PlayerProximityScorer;

impl ScorerBuilder for PlayerProximityScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("PlayerProximityScorer")
    }
}

pub(crate) fn player_proximity_scorer_system(
    npcs: Query<(&NpcBlackboard, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<PlayerProximityScorer>>,
    npc_behavior: Option<valence::prelude::Res<NpcBehaviorConfig>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let flee_threshold = npc_behavior
            .as_deref()
            .map(|behavior| behavior.threshold_for_npc(*actor))
            .unwrap_or(DEFAULT_FLEE_THRESHOLD)
            .clamp(0.0, 1.0);

        let value = if let Ok((blackboard, tier)) = npcs.get(*actor) {
            match lod_gated_score(tier, tick, &cfg, || {
                score_for_flee_threshold(
                    proximity_score(blackboard.player_distance),
                    flee_threshold,
                )
            }) {
                Some(value) => value,
                None => continue,
            }
        } else {
            0.0
        };

        score.set(value);
    }
}

pub(crate) fn score_for_flee_threshold(score: f32, flee_threshold: f32) -> f32 {
    if score >= flee_threshold {
        1.0
    } else {
        0.0
    }
}

pub(crate) fn proximity_score(distance: f32) -> f32 {
    if !distance.is_finite() {
        return 0.0;
    }

    ((8.0 - distance) / 8.0).clamp(0.0, 1.0)
}

#[cfg(test)]
pub(crate) fn should_flee_from_score(score: f32) -> bool {
    score >= super::PROXIMITY_THRESHOLD
}

// ---------------------------------------------------------------------------
// ChaseTargetScorer
// ---------------------------------------------------------------------------

/// Scores high when a player is within [`CHASE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct ChaseTargetScorer;

impl ScorerBuilder for ChaseTargetScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("ChaseTargetScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn chase_target_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &NpcMeleeProfile,
            Option<&NpcLodTier>,
            Option<&Lifecycle>,
        ),
        With<NpcMarker>,
    >,
    players: Query<&PlayerIdentities, With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ChaseTargetScorer>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile, tier, lifecycle)) = npcs.get(*actor) {
            // NPC is not alive — suppress all chase scoring.
            if lifecycle.is_some_and(|lc| lc.state != LifecycleState::Alive) {
                score.set(0.0);
                continue;
            }
            match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Cosmetic, || {
                if bb.retaliation_target.is_some() {
                    return 1.0;
                }
                if bb
                    .nearest_player
                    .and_then(|player| players.get(player).ok())
                    .and_then(PlayerIdentities::active)
                    .is_some_and(npc_should_seek_attack)
                {
                    return 1.0;
                }
                chase_score(bb.player_distance, profile)
            }) {
                Some(value) => value,
                None => continue,
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

pub(crate) fn chase_score(distance: f32, profile: &NpcMeleeProfile) -> f32 {
    if !distance.is_finite() || distance > CHASE_RANGE {
        return 0.0;
    }
    if distance <= profile.preferred_distance {
        return 0.0;
    }
    ((CHASE_RANGE - distance) / CHASE_RANGE).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// MeleeRangeScorer
// ---------------------------------------------------------------------------

/// Scores high (1.0) when a player is within [`MELEE_RANGE`] blocks.
#[derive(Clone, Copy, Debug, Component)]
pub struct MeleeRangeScorer;

impl ScorerBuilder for MeleeRangeScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("MeleeRangeScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn melee_range_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &NpcMeleeProfile,
            Option<&NpcLodTier>,
            Option<&Lifecycle>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<MeleeRangeScorer>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile, tier, lifecycle)) = npcs.get(*actor) {
            // NPC is not alive — suppress melee scoring.
            if lifecycle.is_some_and(|lc| lc.state != LifecycleState::Alive) {
                score.set(0.0);
                continue;
            }
            match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Cosmetic, || {
                if bb.player_distance <= profile.reach.max {
                    1.0
                } else {
                    0.0
                }
            }) {
                Some(value) => value,
                None => continue,
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

// ---------------------------------------------------------------------------
// DashScorer
// ---------------------------------------------------------------------------

/// Scores high when the player is within dash range and dash is off cooldown.
#[derive(Clone, Copy, Debug, Component)]
pub struct DashScorer;

impl ScorerBuilder for DashScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("DashScorer")
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn dash_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &MovementCapabilities,
            &MovementCooldowns,
            &MovementController,
            Option<&NpcLodTier>,
            Option<&Lifecycle>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<DashScorer>>,
    game_tick: Option<valence::prelude::Res<crate::npc::movement::GameTick>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let tick = game_tick.map(|t| t.0).unwrap_or(0);
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let lod_tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);

    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, caps, cooldowns, ctrl, tier, lifecycle)) = npcs.get(*actor) {
            // NPC is not alive — suppress dash scoring.
            if lifecycle.is_some_and(|lc| lc.state != LifecycleState::Alive) {
                score.set(0.0);
                continue;
            }
            match lod_gated_score_by_kind(tier, lod_tick, &cfg, ScorerKind::Cosmetic, || {
                dash_score(bb, caps, cooldowns, ctrl, tick)
            }) {
                Some(value) => value,
                None => continue,
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

pub(crate) fn dash_score(
    bb: &NpcBlackboard,
    caps: &MovementCapabilities,
    cooldowns: &MovementCooldowns,
    ctrl: &MovementController,
    current_tick: u32,
) -> f32 {
    if !caps.can_dash {
        return 0.0;
    }
    if current_tick < cooldowns.dash_ready_at {
        return 0.0;
    }
    if ctrl.navigator_should_yield() {
        return 0.0; // already in an override
    }
    if !bb.player_distance.is_finite() {
        return 0.0;
    }
    if bb.player_distance < DASH_MIN_DISTANCE || bb.player_distance > DASH_MAX_DISTANCE {
        return 0.0;
    }

    // Score high -- dash should take priority over regular chase when available.
    0.9
}

// ---------------------------------------------------------------------------
// NpcDefenseScorer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Component)]
pub struct NpcDefenseScorer;

impl ScorerBuilder for NpcDefenseScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }

    fn label(&self) -> Option<&str> {
        Some("NpcDefenseScorer")
    }
}

pub(crate) fn npc_defense_score_for_realm(realm: Realm) -> f32 {
    match realm {
        Realm::Awaken => 0.0,
        Realm::Induce => 0.5,
        Realm::Condense => 0.65,
        Realm::Solidify | Realm::Spirit | Realm::Void => 0.7,
    }
}

fn has_blocking_status(statuses: &StatusEffects) -> bool {
    has_active_status(statuses, StatusEffectKind::ParryRecovery)
        || has_active_status(statuses, StatusEffectKind::SwordParrying)
        || has_active_status(statuses, StatusEffectKind::Staggered)
}

#[allow(clippy::type_complexity)]
pub(crate) fn npc_defense_scorer_system(
    npcs: Query<
        (
            &NpcBlackboard,
            &Cultivation,
            Option<&StatusEffects>,
            Option<&NpcLodTier>,
            Option<&Lifecycle>,
        ),
        With<NpcMarker>,
    >,
    mut scorers: Query<(&Actor, &mut Score), With<NpcDefenseScorer>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, cultivation, statuses_opt, tier, lifecycle)) = npcs.get(*actor) {
            // NPC is not alive — suppress all defense scoring.
            if lifecycle.is_some_and(|lc| lc.state != LifecycleState::Alive) {
                score.set(0.0);
                continue;
            }
            match lod_gated_score_by_kind(tier, tick, &cfg, ScorerKind::Cosmetic, || {
                if bb.nearest_player.is_none() {
                    return 0.0;
                }
                if let Some(statuses) = statuses_opt {
                    if has_blocking_status(statuses) {
                        return 0.0;
                    }
                }
                npc_defense_score_for_realm(cultivation.realm)
            }) {
                Some(value) => value,
                None => continue,
            }
        } else {
            0.0
        };
        score.set(value);
    }
}

use valence::prelude::Query;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::Realm;
    use crate::identity::{IdentityId, IdentityProfile, RevealedTag, RevealedTagKind};
    use crate::npc::brain::PROXIMITY_THRESHOLD;
    use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
    use crate::npc::spawn::NpcMeleeProfile;
    use big_brain::prelude::{BigBrainSet, FirstToScore, Thinker};
    use valence::prelude::{App, IntoSystemConfigs, PreUpdate};

    #[test]
    fn player_proximity_scorer_thresholds() {
        let score_at_just_inside_threshold_distance = proximity_score(3.2);
        let score_at_exact_threshold_distance = proximity_score(3.2);
        let score_just_outside_threshold_distance = proximity_score(3.3);
        let score_out_of_range = proximity_score(8.0);

        assert!(
            should_flee_from_score(score_at_just_inside_threshold_distance),
            "3.2 blocks should meet threshold"
        );
        assert!(
            should_flee_from_score(score_at_exact_threshold_distance),
            "exact threshold score should trigger flee"
        );
        assert!(
            !should_flee_from_score(score_just_outside_threshold_distance),
            "3.3 blocks should fall under threshold"
        );
        assert_eq!(score_out_of_range, 0.0, "8+ blocks should score 0");

        let thinker = Thinker::build()
            .picker(FirstToScore {
                threshold: PROXIMITY_THRESHOLD,
            })
            .when(
                PlayerProximityScorer,
                super::super::actions_combat::FleeAction,
            );
        let mut app = App::new();
        app.world_mut().spawn(thinker);
        assert_eq!(PROXIMITY_THRESHOLD, 0.6);
        assert!((proximity_score(3.2) - 0.6).abs() < 1e-6);
    }

    #[test]
    fn chase_score_within_range() {
        let profile = NpcMeleeProfile::fist();
        assert!(chase_score(10.0, &profile) > 0.0);
        assert!(chase_score(32.0, &profile) > -f32::EPSILON);
        assert_eq!(chase_score(33.0, &profile), 0.0);
        assert_eq!(chase_score(f32::INFINITY, &profile), 0.0);
        assert_eq!(chase_score(0.8, &profile), 0.0);
    }

    #[test]
    fn chase_target_scorer_boosts_wanted_identity_even_outside_normal_range() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            chase_target_scorer_system.in_set(BigBrainSet::Scorers),
        );
        let mut profile = IdentityProfile::new(IdentityId::DEFAULT, "test", 0);
        profile.renown.notoriety = 30;
        profile.revealed_tags.push(RevealedTag {
            kind: RevealedTagKind::DuguRevealed,
            witnessed_at_tick: 20,
            witness_realm: Realm::Spirit,
            permanent: true,
        });
        let target = app
            .world_mut()
            .spawn((
                ClientMarker,
                PlayerIdentities {
                    identities: vec![profile],
                    active_identity_id: IdentityId::DEFAULT,
                    last_switch_tick: 0,
                },
            ))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(target),
                    player_distance: 80.0,
                    target_position: Some(valence::prelude::DVec3::new(80.0, 66.0, 0.0)),
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), ChaseTargetScorer))
            .id();

        app.update();

        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn melee_range_scorer_respects_profile_reach_max() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            melee_range_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let short_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 2.8,
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
            ))
            .id();
        let long_npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 2.8,
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
            ))
            .id();

        let short_scorer = app
            .world_mut()
            .spawn((Actor(short_npc), Score::default(), MeleeRangeScorer))
            .id();
        let long_scorer = app
            .world_mut()
            .spawn((Actor(long_npc), Score::default(), MeleeRangeScorer))
            .id();

        app.update();

        assert_eq!(app.world().get::<Score>(short_scorer).unwrap().get(), 0.0);
        assert_eq!(app.world().get::<Score>(long_scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn dash_score_zero_without_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: false,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 0), 0.0);
    }

    #[test]
    fn dash_score_positive_in_range_with_capability() {
        let bb = NpcBlackboard {
            player_distance: 8.0, // within DASH_MIN..DASH_MAX
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert!(dash_score(&bb, &caps, &cd, &ctrl, 0) > 0.0);
    }

    #[test]
    fn dash_score_zero_on_cooldown() {
        let bb = NpcBlackboard {
            player_distance: 8.0,
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns {
            sprint_ready_at: 0,
            dash_ready_at: 100, // cooldown active
        };
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb, &caps, &cd, &ctrl, 50), 0.0);
    }

    #[test]
    fn dash_score_zero_outside_range() {
        let bb_too_close = NpcBlackboard {
            player_distance: 3.0, // < DASH_MIN_DISTANCE
            ..Default::default()
        };
        let bb_too_far = NpcBlackboard {
            player_distance: 20.0, // > DASH_MAX_DISTANCE
            ..Default::default()
        };
        let caps = MovementCapabilities {
            can_sprint: true,
            can_dash: true,
        };
        let cd = MovementCooldowns::default();
        let ctrl = MovementController::new();

        assert_eq!(dash_score(&bb_too_close, &caps, &cd, &ctrl, 0), 0.0);
        assert_eq!(dash_score(&bb_too_far, &caps, &cd, &ctrl, 0), 0.0);
    }

    // ─── NpcDefenseScorer tests ─────────────────────────────────────────────

    #[test]
    fn defense_score_awaken_is_zero() {
        assert_eq!(
            npc_defense_score_for_realm(Realm::Awaken),
            0.0,
            "Awaken NPCs should never attempt defense"
        );
    }

    #[test]
    fn defense_score_induce_is_half() {
        assert!(
            (npc_defense_score_for_realm(Realm::Induce) - 0.5).abs() < f32::EPSILON,
            "Induce NPCs should score 0.5 for defense"
        );
    }

    #[test]
    fn defense_score_condense_is_0_65() {
        assert!(
            (npc_defense_score_for_realm(Realm::Condense) - 0.65).abs() < f32::EPSILON,
            "Condense NPCs should score 0.65 for defense"
        );
    }

    #[test]
    fn defense_score_solidify_and_above_is_0_7() {
        for realm in [Realm::Solidify, Realm::Spirit, Realm::Void] {
            assert!(
                (npc_defense_score_for_realm(realm) - 0.7).abs() < f32::EPSILON,
                "realm {realm:?} should score 0.7 for defense"
            );
        }
    }

    #[test]
    fn defense_score_all_realms_exhaustive() {
        let expected = [
            (Realm::Awaken, 0.0),
            (Realm::Induce, 0.5),
            (Realm::Condense, 0.65),
            (Realm::Solidify, 0.7),
            (Realm::Spirit, 0.7),
            (Realm::Void, 0.7),
        ];
        for (realm, expected_score) in expected {
            let actual = npc_defense_score_for_realm(realm);
            assert!(
                (actual - expected_score).abs() < f32::EPSILON,
                "realm {realm:?}: expected {expected_score}, got {actual}"
            );
        }
    }

    #[test]
    fn defense_score_and_jiemai_gate_agree_for_every_realm() {
        // 本 bug 的本质回归锁：防御 scorer 评 >0 的境界，npc_defense_action 的施放 gate
        // (jiemai_qi_cost_for_realm) 必须返回 Some，否则 picker 选中 NpcDefenseAction 后
        // Requested 阶段必败 → NPC 战斗中反复选防御死循环空转。反向亦然（有成本却评 0 = 浪费）。
        // 化虚此前 scorer=0.7 而 gate=None，恰好违反，故曾死循环。
        use crate::combat::jiemai::jiemai_qi_cost_for_realm;
        for realm in [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ] {
            let scores = npc_defense_score_for_realm(realm) > 0.0;
            let castable = jiemai_qi_cost_for_realm(realm).is_some();
            assert_eq!(
                scores, castable,
                "realm {realm:?}: 防御 scorer 评分(>0)={scores} 但施放 gate 可用={castable}，\
                 二者必须一致，否则 Void 类 scorer/action 不匹配会让 NPC 防御死循环"
            );
        }
    }

    #[test]
    fn defense_scorer_blocked_by_parry_recovery() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        let statuses = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::ParryRecovery,
                magnitude: 1.0,
                remaining_ticks: 20,
                source_pill: None,
            }],
        };
        assert!(
            has_blocking_status(&statuses),
            "ParryRecovery should block defense scorer"
        );
    }

    #[test]
    fn defense_scorer_blocked_by_sword_parrying() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        let statuses = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::SwordParrying,
                magnitude: 1.0,
                remaining_ticks: 20,
                source_pill: None,
            }],
        };
        assert!(
            has_blocking_status(&statuses),
            "SwordParrying should block defense scorer"
        );
    }

    #[test]
    fn defense_scorer_blocked_by_staggered() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        let statuses = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::Staggered,
                magnitude: 1.0,
                remaining_ticks: 20,
                source_pill: None,
            }],
        };
        assert!(
            has_blocking_status(&statuses),
            "Staggered should block defense scorer"
        );
    }

    #[test]
    fn defense_scorer_not_blocked_by_unrelated_status() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        let statuses = StatusEffects {
            active: vec![ActiveStatusEffect {
                kind: StatusEffectKind::Bleeding,
                magnitude: 1.0,
                remaining_ticks: 20,
                source_pill: None,
            }],
        };
        assert!(
            !has_blocking_status(&statuses),
            "Bleeding should not block defense scorer"
        );
    }

    #[test]
    fn defense_scorer_not_blocked_with_empty_statuses() {
        use crate::combat::components::StatusEffects;
        let statuses = StatusEffects { active: vec![] };
        assert!(
            !has_blocking_status(&statuses),
            "empty statuses should not block defense scorer"
        );
    }

    #[test]
    fn defense_scorer_system_returns_zero_when_no_player_nearby() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: None,
                    player_distance: f32::INFINITY,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Solidify,
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "defense scorer should return 0 with no player nearby"
        );
    }

    #[test]
    fn defense_scorer_system_returns_realm_score_with_player_nearby() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (actual - 0.65).abs() < f32::EPSILON,
            "Condense NPC with player nearby should score 0.65, got {actual}"
        );
    }

    #[test]
    fn defense_scorer_system_returns_zero_with_blocking_status() {
        use crate::combat::components::{ActiveStatusEffect, StatusEffects};
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Solidify,
                    ..Default::default()
                },
                StatusEffects {
                    active: vec![ActiveStatusEffect {
                        kind: StatusEffectKind::ParryRecovery,
                        magnitude: 1.0,
                        remaining_ticks: 20,
                        source_pill: None,
                    }],
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "defense scorer should return 0 during ParryRecovery"
        );
    }

    // ─── Lifecycle guard tests for NpcDefenseScorer ────────────────────────

    #[test]
    fn defense_scorer_alive_npc_scores_normally() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                Lifecycle {
                    character_id: "npc_alive".to_string(),
                    ..Default::default()
                },
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (actual - 0.65).abs() < f32::EPSILON,
            "Alive Condense NPC with player nearby should score 0.65, got {actual}"
        );
    }

    #[test]
    fn defense_scorer_near_death_npc_scores_zero() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let mut lifecycle = Lifecycle {
            character_id: "npc_near_death".to_string(),
            ..Default::default()
        };
        lifecycle.enter_near_death(10);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                lifecycle,
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "NearDeath NPC should have defense score suppressed to 0.0 — dead NPCs must not defend"
        );
    }

    #[test]
    fn defense_scorer_terminated_state_npc_scores_zero() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let lifecycle = Lifecycle {
            character_id: "npc_terminated".to_string(),
            state: LifecycleState::Terminated,
            ..Default::default()
        };

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Solidify,
                    ..Default::default()
                },
                lifecycle,
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "Terminated NPC should have defense score suppressed to 0.0"
        );
    }

    #[test]
    fn defense_scorer_awaiting_revival_npc_scores_zero() {
        // 补齐 LifecycleState::AwaitingRevival 变体覆盖（CodeRabbit #697 Major）：
        // 「非 Alive 一律抑制」契约对每个变体都成立，防回归。
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let lifecycle = Lifecycle {
            character_id: "npc_awaiting_revival".to_string(),
            state: LifecycleState::AwaitingRevival,
            ..Default::default()
        };

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                lifecycle,
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "AwaitingRevival NPC should have defense score suppressed to 0.0 (非 Alive 一律抑制)"
        );
    }

    #[test]
    fn defense_scorer_no_lifecycle_component_scores_normally() {
        use crate::cultivation::components::Cultivation;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Condense,
                    ..Default::default()
                },
                // No Lifecycle component — fallback should score normally
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (actual - 0.65).abs() < f32::EPSILON,
            "NPC without Lifecycle component should score normally for defense (not suppressed), got {actual}"
        );
    }

    // ─── Lifecycle guard tests for chase/melee/dash scorers ────────────────

    #[test]
    fn chase_target_scorer_alive_npc_scores_normally() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            chase_target_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 10.0,
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
                Lifecycle {
                    character_id: "npc_alive".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), ChaseTargetScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            actual > 0.0,
            "Alive NPC within chase range should score > 0, got {actual}"
        );
    }

    #[test]
    fn chase_target_scorer_near_death_npc_scores_zero() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            chase_target_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let mut lifecycle = Lifecycle {
            character_id: "npc_near_death".to_string(),
            ..Default::default()
        };
        lifecycle.enter_near_death(10);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 10.0,
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
                lifecycle,
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), ChaseTargetScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "NearDeath NPC should have chase score suppressed to 0.0"
        );
    }

    #[test]
    fn chase_target_scorer_no_lifecycle_component_fallback() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            chase_target_scorer_system.in_set(BigBrainSet::Scorers),
        );

        // NPC without Lifecycle component — should score normally (not suppressed).
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 10.0,
                    ..Default::default()
                },
                NpcMeleeProfile::fist(),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), ChaseTargetScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            actual > 0.0,
            "NPC without Lifecycle component should score normally (not suppressed), got {actual}"
        );
    }

    #[test]
    fn melee_range_scorer_alive_npc_scores_normally() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            melee_range_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 2.8,
                    ..Default::default()
                },
                NpcMeleeProfile::spear(), // reach.max = 3.0
                Lifecycle {
                    character_id: "npc_alive".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MeleeRangeScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            actual, 1.0,
            "Alive NPC within melee reach should score 1.0, got {actual}"
        );
    }

    #[test]
    fn melee_range_scorer_near_death_npc_scores_zero() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            melee_range_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let mut lifecycle = Lifecycle {
            character_id: "npc_near_death".to_string(),
            ..Default::default()
        };
        lifecycle.enter_near_death(10);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 2.8,
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
                lifecycle,
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MeleeRangeScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "NearDeath NPC should have melee score suppressed to 0.0"
        );
    }

    #[test]
    fn melee_range_scorer_no_lifecycle_component_fallback() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            melee_range_scorer_system.in_set(BigBrainSet::Scorers),
        );

        // NPC without Lifecycle — should score normally.
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 2.8,
                    ..Default::default()
                },
                NpcMeleeProfile::spear(),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), MeleeRangeScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            actual, 1.0,
            "NPC without Lifecycle component should score normally for melee (not suppressed), got {actual}"
        );
    }

    #[test]
    fn dash_scorer_alive_npc_scores_normally() {
        let mut app = App::new();
        app.insert_resource(crate::npc::movement::GameTick(0));
        app.add_systems(PreUpdate, dash_scorer_system.in_set(BigBrainSet::Scorers));

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 8.0, // within DASH_MIN..DASH_MAX
                    ..Default::default()
                },
                MovementCapabilities {
                    can_sprint: true,
                    can_dash: true,
                },
                MovementCooldowns::default(),
                MovementController::new(),
                Lifecycle {
                    character_id: "npc_alive".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), DashScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            actual > 0.0,
            "Alive NPC with dash capability in range should score > 0, got {actual}"
        );
    }

    #[test]
    fn dash_scorer_near_death_npc_scores_zero() {
        let mut app = App::new();
        app.insert_resource(crate::npc::movement::GameTick(0));
        app.add_systems(PreUpdate, dash_scorer_system.in_set(BigBrainSet::Scorers));

        let mut lifecycle = Lifecycle {
            character_id: "npc_near_death".to_string(),
            ..Default::default()
        };
        lifecycle.enter_near_death(10);

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 8.0,
                    ..Default::default()
                },
                MovementCapabilities {
                    can_sprint: true,
                    can_dash: true,
                },
                MovementCooldowns::default(),
                MovementController::new(),
                lifecycle,
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), DashScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "NearDeath NPC should have dash score suppressed to 0.0"
        );
    }

    #[test]
    fn dash_scorer_no_lifecycle_component_fallback() {
        let mut app = App::new();
        app.insert_resource(crate::npc::movement::GameTick(0));
        app.add_systems(PreUpdate, dash_scorer_system.in_set(BigBrainSet::Scorers));

        // NPC without Lifecycle — should score normally.
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    player_distance: 8.0,
                    ..Default::default()
                },
                MovementCapabilities {
                    can_sprint: true,
                    can_dash: true,
                },
                MovementCooldowns::default(),
                MovementController::new(),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), DashScorer))
            .id();

        app.update();

        let actual = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            actual > 0.0,
            "NPC without Lifecycle component should score normally for dash (not suppressed), got {actual}"
        );
    }

    #[test]
    fn defense_scorer_system_lod_far_returns_zero() {
        use crate::cultivation::components::Cultivation;
        use crate::npc::lod::NpcLodTier;
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            npc_defense_scorer_system.in_set(BigBrainSet::Scorers),
        );

        let player = app.world_mut().spawn_empty().id();

        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                NpcBlackboard {
                    nearest_player: Some(player),
                    player_distance: 3.0,
                    ..Default::default()
                },
                Cultivation {
                    realm: Realm::Spirit,
                    ..Default::default()
                },
                NpcLodTier::Far,
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), NpcDefenseScorer))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "defense scorer should return 0 for Far LOD tier (ScorerKind::Cosmetic)"
        );
    }
}
