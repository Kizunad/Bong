use big_brain::prelude::{FirstToScore, Thinker, ThinkerBuilder};
use valence::prelude::{Commands, DVec3, Entity};

use crate::cultivation::components::Realm;
use crate::npc::brain::{
    AgeingScorer, FarmAction, FearCultivatorScorer, FleeCultivatorAction, GoToPoiAction,
    HungerScorer, RetireAction, ReturnHomeAction, ReturnHomeScorer, StallAction, TradeStallScorer,
    WanderScorer, WanderState,
};
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle_with_age, NpcArchetype};
use crate::npc::trade::NpcPlayerReputation;
use crate::skin::{initial_age_ratio, select_npc_visual_profile};

use super::common::{
    attach_player_skin, draw_npc_skin, spawn_rogue_commoner_base, NpcCombatLoadout,
    NpcSkinSpawnContext,
};

// ---------------------------------------------------------------------------
// Thinker
// ---------------------------------------------------------------------------

pub(crate) fn commoner_npc_thinker() -> ThinkerBuilder {
    Thinker::build()
        .picker(FirstToScore { threshold: 0.05 })
        .when(AgeingScorer, RetireAction)
        .when(FearCultivatorScorer, FleeCultivatorAction)
        .when(ReturnHomeScorer, ReturnHomeAction)
        .when(TradeStallScorer, StallAction)
        .when(HungerScorer, FarmAction)
        .when(WanderScorer, GoToPoiAction::default())
}

// ---------------------------------------------------------------------------
// Spawn
// ---------------------------------------------------------------------------

/// Spawn a Commoner NPC. MineSkin 池可用时走假玩家 skin；否则退回 vanilla villager。
/// Starting age is controlled by
/// `initial_age_ticks` — newborns pass `0.0`, agent-spawned adults can pass
/// any value below the Commoner default max age.
#[allow(clippy::too_many_arguments)]
pub fn spawn_commoner_npc_at(
    commands: &mut Commands,
    skin_context: NpcSkinSpawnContext<'_>,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
    realm: Realm,
    initial_age_ticks: f64,
) -> Entity {
    let loadout = NpcCombatLoadout::civilian();
    let profile = select_npc_visual_profile(
        NpcArchetype::Commoner,
        realm,
        None,
        None,
        initial_age_ratio(NpcArchetype::Commoner, initial_age_ticks),
    );
    let skin = draw_npc_skin(skin_context, profile, spawn_position);
    let entity = spawn_rogue_commoner_base(
        commands,
        layer,
        spawn_position,
        &skin,
        profile,
        loadout.clone(),
        NpcArchetype::Commoner,
        home_zone,
        patrol_target,
    );

    if let Some(skin) = skin.filter(|skin| !skin.is_fallback()) {
        attach_player_skin(commands, entity, NpcArchetype::Commoner, skin);
    }

    commands.entity(entity).insert((
        Hunger::default(),
        WanderState::default(),
        commoner_npc_thinker(),
        NpcPlayerReputation::default(),
    ));

    let runtime = npc_runtime_bundle_with_age(entity, NpcArchetype::Commoner, initial_age_ticks);
    commands.entity(entity).insert(runtime);

    entity
}
