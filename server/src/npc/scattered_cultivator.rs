use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use valence::prelude::{
    bevy_ecs, App, BlockPos, Component, Entity, EventWriter, Position, Query, Res, ResMut,
    Resource, Update, With,
};

use crate::combat::components::Lifecycle;
use crate::cultivation::components::Realm;
use crate::npc::movement::GameTick;
use crate::social::components::SpiritNiche;
use crate::social::events::NicheIntrusionAttempt;

use super::spawn::NpcMarker;

pub type PlotId = BlockPos;

pub const PLAYER_PLOT_REACTION_RADIUS: f64 = 5.0;
pub const PLAYER_PLOT_LINGER_TICKS: u32 = 600;
pub const MIGRATION_COOLDOWN_TICKS: u64 = 12_000;

type CultivatorPlotQueryItem<'a> = (Entity, &'a ScatteredCultivator);
type CultivatorPlotQueryFilter = (With<NpcMarker>, With<Position>);

#[derive(Debug, Clone, Component)]
pub struct ScatteredCultivator {
    pub home_plot: Option<PlotId>,
    pub temperament: FarmingTemperament,
    pub fail_streak: u8,
    pub last_replenish_tick: u64,
    pub migration_cooldown_until: u64,
}

impl ScatteredCultivator {
    pub const fn new(temperament: FarmingTemperament) -> Self {
        Self {
            home_plot: None,
            temperament,
            fail_streak: 0,
            last_replenish_tick: 0,
            migration_cooldown_until: 0,
        }
    }

    pub fn migration_ready(&self, now_tick: u64) -> bool {
        now_tick >= self.migration_cooldown_until
    }

    pub fn mark_migrated(&mut self, now_tick: u64) {
        self.home_plot = None;
        self.fail_streak = 0;
        self.migration_cooldown_until = now_tick.saturating_add(MIGRATION_COOLDOWN_TICKS);
    }

    pub fn record_farming_failure(&mut self) {
        self.fail_streak = self.fail_streak.saturating_add(1);
    }

    pub fn record_farming_success(&mut self) {
        self.fail_streak = 0;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FarmingTemperament {
    #[default]
    Patient,
    Greedy,
    Anxious,
    Aggressive,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemperamentWeights {
    pub soil: f32,
    pub qi_density: f32,
    pub own_qi: f32,
    pub season: f32,
    pub threat: f32,
    pub tool: f32,
}

impl FarmingTemperament {
    pub fn deterministic(seed: u64) -> Self {
        match seed % 100 {
            0..=29 => Self::Patient,
            30..=59 => Self::Greedy,
            60..=84 => Self::Anxious,
            _ => Self::Aggressive,
        }
    }

    pub const fn weights(self) -> TemperamentWeights {
        match self {
            Self::Patient => TemperamentWeights {
                soil: 1.0,
                qi_density: 0.8,
                own_qi: 0.8,
                season: 1.0,
                threat: 1.1,
                tool: 0.7,
            },
            Self::Greedy => TemperamentWeights {
                soil: 0.8,
                qi_density: 1.4,
                own_qi: 0.9,
                season: 0.8,
                threat: 0.7,
                tool: 0.8,
            },
            Self::Anxious => TemperamentWeights {
                soil: 0.9,
                qi_density: 0.9,
                own_qi: 0.8,
                season: 0.8,
                threat: 1.5,
                tool: 0.7,
            },
            Self::Aggressive => TemperamentWeights {
                soil: 0.9,
                qi_density: 1.1,
                own_qi: 1.0,
                season: 0.7,
                threat: 0.5,
                tool: 1.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CultivatorPlayerReaction {
    Flee,
    RespectfulTrade,
    RobPlayer,
    StealPlot,
}

pub fn choose_player_reaction(
    temperament: FarmingTemperament,
    player_realm: Realm,
    cultivator_realm: Realm,
    player_qi_fraction: f64,
    player_bloodied: bool,
    linger_ticks: u32,
) -> CultivatorPlayerReaction {
    if linger_ticks >= PLAYER_PLOT_LINGER_TICKS
        && matches!(temperament, FarmingTemperament::Aggressive)
    {
        return CultivatorPlayerReaction::StealPlot;
    }
    if player_bloodied && player_qi_fraction <= 0.25 {
        return CultivatorPlayerReaction::RobPlayer;
    }
    if realm_rank(player_realm) > realm_rank(cultivator_realm) && player_qi_fraction >= 0.8 {
        return CultivatorPlayerReaction::Flee;
    }
    CultivatorPlayerReaction::RespectfulTrade
}

fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 1,
        Realm::Induce => 2,
        Realm::Condense => 3,
        Realm::Solidify => 4,
        Realm::Spirit => 5,
        Realm::Void => 6,
    }
}

#[derive(Debug, Default, Resource)]
pub struct ScatteredCultivatorSocialMemory {
    linger_started_at: HashMap<(Entity, Entity), u32>,
    intrusion_sent_at: HashMap<(Entity, Entity), u32>,
}

pub fn detect_scattered_cultivator_plot_trespass(
    cultivators: Query<CultivatorPlotQueryItem, CultivatorPlotQueryFilter>,
    players: Query<(Entity, &Position, &Lifecycle), WithoutNpcMarker>,
    niches: Query<&SpiritNiche>,
    tick: Option<Res<GameTick>>,
    mut memory: ResMut<ScatteredCultivatorSocialMemory>,
    mut attempts: EventWriter<NicheIntrusionAttempt>,
) {
    let now = tick.as_deref().map(|tick| tick.0).unwrap_or(0);
    for (cultivator, scattered) in &cultivators {
        let Some(plot) = scattered.home_plot else {
            continue;
        };
        let plot_center = [plot.x as f64 + 0.5, plot.y as f64, plot.z as f64 + 0.5];
        for (player, player_pos, lifecycle) in &players {
            let pos = player_pos.get();
            let dx = pos.x - plot_center[0];
            let dy = pos.y - plot_center[1];
            let dz = pos.z - plot_center[2];
            if (dx * dx + dy * dy + dz * dz).sqrt() > PLAYER_PLOT_REACTION_RADIUS {
                memory.linger_started_at.remove(&(cultivator, player));
                memory.intrusion_sent_at.remove(&(cultivator, player));
                continue;
            }

            let started = *memory
                .linger_started_at
                .entry((cultivator, player))
                .or_insert(now);
            let linger_ticks = now.saturating_sub(started);
            if choose_player_reaction(
                scattered.temperament,
                Realm::Awaken,
                Realm::Awaken,
                1.0,
                false,
                linger_ticks,
            ) != CultivatorPlayerReaction::StealPlot
            {
                continue;
            }
            if memory
                .intrusion_sent_at
                .get(&(cultivator, player))
                .is_some_and(|sent| now.saturating_sub(*sent) < PLAYER_PLOT_LINGER_TICKS)
            {
                continue;
            }
            let Some(niche) = niches
                .iter()
                .find(|niche| niche.owner == lifecycle.character_id)
            else {
                continue;
            };
            attempts.send(NicheIntrusionAttempt {
                intruder: cultivator,
                intruder_char_id: format!("npc:{cultivator:?}"),
                niche_owner: niche.owner.clone(),
                niche_pos: niche.pos,
                items_taken: Vec::new(),
                intruder_qi_fraction: 1.0,
                intruder_back_turned: false,
                tick: u64::from(now),
            });
            memory.intrusion_sent_at.insert((cultivator, player), now);
        }
    }
}

type WithoutNpcMarker = valence::prelude::Without<NpcMarker>;

pub fn register(app: &mut App) {
    app.insert_resource(ScatteredCultivatorSocialMemory::default())
        .add_systems(Update, detect_scattered_cultivator_plot_trespass);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temperament_distribution_is_stable() {
        let counts = (0u64..100).fold([0usize; 4], |mut counts, seed| {
            match FarmingTemperament::deterministic(seed) {
                FarmingTemperament::Patient => counts[0] += 1,
                FarmingTemperament::Greedy => counts[1] += 1,
                FarmingTemperament::Anxious => counts[2] += 1,
                FarmingTemperament::Aggressive => counts[3] += 1,
            }
            counts
        });
        assert_eq!(counts, [30, 30, 25, 15]);
    }

    #[test]
    fn aggressive_lingering_player_triggers_steal_plot_reaction() {
        let reaction = choose_player_reaction(
            FarmingTemperament::Aggressive,
            Realm::Awaken,
            Realm::Awaken,
            1.0,
            false,
            PLAYER_PLOT_LINGER_TICKS,
        );
        assert_eq!(reaction, CultivatorPlayerReaction::StealPlot);
    }

    #[test]
    fn weak_bloodied_player_is_robbed_before_trade() {
        let reaction = choose_player_reaction(
            FarmingTemperament::Patient,
            Realm::Awaken,
            Realm::Awaken,
            0.1,
            true,
            0,
        );
        assert_eq!(reaction, CultivatorPlayerReaction::RobPlayer);
    }

    #[test]
    fn aggressive_plot_linger_emits_niche_intrusion_attempt() {
        let mut app = App::new();
        app.insert_resource(ScatteredCultivatorSocialMemory::default())
            .insert_resource(GameTick(0))
            .add_event::<NicheIntrusionAttempt>()
            .add_systems(Update, detect_scattered_cultivator_plot_trespass);

        let plot = BlockPos::new(4, 64, 4);
        app.world_mut().spawn((
            NpcMarker,
            Position::new([4.5, 65.0, 4.5]),
            ScatteredCultivator::new(FarmingTemperament::Aggressive).with_home_for_test(plot),
        ));
        app.world_mut().spawn((
            Position::new([4.5, 64.0, 4.5]),
            Lifecycle {
                character_id: "char:owner".to_string(),
                ..Default::default()
            },
        ));
        app.world_mut().spawn(SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [8, 64, 8],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            guardians: Vec::new(),
        });

        app.update();
        app.world_mut().resource_mut::<GameTick>().0 = PLAYER_PLOT_LINGER_TICKS;
        app.update();

        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<NicheIntrusionAttempt>>();
        let mut reader = events.get_reader();
        let attempts = reader.read(events).collect::<Vec<_>>();
        assert_eq!(attempts.len(), 1);
        assert_eq!(attempts[0].niche_owner, "char:owner");
        assert_eq!(attempts[0].niche_pos, [8, 64, 8]);
        assert_eq!(attempts[0].tick, u64::from(PLAYER_PLOT_LINGER_TICKS));
    }

    #[test]
    fn leaving_plot_radius_clears_linger_and_intrusion_cooldown() {
        let mut app = App::new();
        app.insert_resource(ScatteredCultivatorSocialMemory::default())
            .insert_resource(GameTick(0))
            .add_event::<NicheIntrusionAttempt>()
            .add_systems(Update, detect_scattered_cultivator_plot_trespass);

        let plot = BlockPos::new(4, 64, 4);
        let cultivator = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([4.5, 65.0, 4.5]),
                ScatteredCultivator::new(FarmingTemperament::Aggressive).with_home_for_test(plot),
            ))
            .id();
        let player = app
            .world_mut()
            .spawn((
                Position::new([4.5, 64.0, 4.5]),
                Lifecycle {
                    character_id: "char:owner".to_string(),
                    ..Default::default()
                },
            ))
            .id();
        app.world_mut().spawn(SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [8, 64, 8],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            guardians: Vec::new(),
        });

        app.update();
        app.world_mut().resource_mut::<GameTick>().0 = PLAYER_PLOT_LINGER_TICKS;
        app.update();
        {
            let memory = app.world().resource::<ScatteredCultivatorSocialMemory>();
            assert!(memory.intrusion_sent_at.contains_key(&(cultivator, player)));
        }

        app.world_mut()
            .entity_mut(player)
            .insert(Position::new([100.0, 64.0, 100.0]));
        app.update();

        let memory = app.world().resource::<ScatteredCultivatorSocialMemory>();
        assert!(!memory.linger_started_at.contains_key(&(cultivator, player)));
        assert!(!memory.intrusion_sent_at.contains_key(&(cultivator, player)));
    }

    trait ScatteredCultivatorTestExt {
        fn with_home_for_test(self, plot: PlotId) -> Self;
    }

    impl ScatteredCultivatorTestExt for ScatteredCultivator {
        fn with_home_for_test(mut self, plot: PlotId) -> Self {
            self.home_plot = Some(plot);
            self
        }
    }
}
