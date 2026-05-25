use big_brain::prelude::{Actor, Score, ScorerBuilder};
use valence::client::ClientMarker;
use valence::prelude::{bevy_ecs, Commands, Component, Entity, With};

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

pub(crate) fn chase_target_scorer_system(
    npcs: Query<(&NpcBlackboard, &NpcMeleeProfile, Option<&NpcLodTier>), With<NpcMarker>>,
    players: Query<&PlayerIdentities, With<ClientMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<ChaseTargetScorer>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile, tier)) = npcs.get(*actor) {
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

pub(crate) fn melee_range_scorer_system(
    npcs: Query<(&NpcBlackboard, &NpcMeleeProfile, Option<&NpcLodTier>), With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<MeleeRangeScorer>>,
    lod_config: Option<valence::prelude::Res<NpcLodConfig>>,
    lod_tick: Option<valence::prelude::Res<NpcLodTick>>,
) {
    let cfg = lod_config.as_deref().cloned().unwrap_or_default();
    let tick = lod_tick.as_deref().map(|t| t.0).unwrap_or(0);
    for (Actor(actor), mut score) in &mut scorers {
        let value = if let Ok((bb, profile, tier)) = npcs.get(*actor) {
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
        let value = if let Ok((bb, caps, cooldowns, ctrl, tier)) = npcs.get(*actor) {
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
}
