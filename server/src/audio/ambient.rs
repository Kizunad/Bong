//! Zone-aware ambient and music-state packets for audio-world-v1.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{
    bevy_ecs, ident, App, Client, DVec3, Entity, IntoSystemConfigs, Position, Query, Res, ResMut,
    Resource, Update, With,
};

use crate::audio::SoundRecipeRegistry;
use crate::combat::components::CombatState;
use crate::combat::CombatClock;
use crate::cultivation::tick::{CultivationClock, CultivationSessionPracticeAccumulator};
use crate::cultivation::tribulation::TribulationState;
use crate::schema::audio::{
    validate_recipe_id, AmbientZoneS2c, LoopConfig, SoundLayer, SoundRecipe,
};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::season::{Season, WorldSeasonState, VANILLA_DAY_TICKS};
use crate::world::tsy::TsyPresence;
use crate::world::zone::{TsyDepth, Zone, ZoneRegistry, DEFAULT_ZONES_PATH};

#[cfg(test)]
const AUDIO_AMBIENT_ZONE_CHANNEL: &str = "bong:audio/ambient_zone";
pub const DEFAULT_TSY_ZONES_PATH: &str = "zones.tsy.json";

const AUDIO_WORLD_LOOP_FLAG: &str = "audio_world";
const AMBIENT_CROSSFADE_TICKS: u32 = 60;
const NIGHT_START_TICK: u64 = 13_000;
const NIGHT_END_TICK: u64 = 23_000;
const PITCH_SHIFT_SUMMER: f32 = 0.10;
const PITCH_SHIFT_WINTER: f32 = -0.10;
const WILDERNESS_ZONE_NAME: &str = "wilderness";

#[derive(Debug, Default)]
pub struct AmbientAudioState {
    last_by_entity: HashMap<Entity, AudioWorldKey>,
}

impl Resource for AmbientAudioState {}

#[derive(Debug, Default)]
pub struct AmbientZoneRecipes {
    by_zone: HashMap<String, String>,
}

impl Resource for AmbientZoneRecipes {}

impl AmbientZoneRecipes {
    pub fn load_default() -> Self {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut recipes = Self::default();
        recipes.merge_path(manifest_dir.join(DEFAULT_ZONES_PATH));
        recipes.merge_path(manifest_dir.join(DEFAULT_TSY_ZONES_PATH));
        recipes
    }

    #[cfg(test)]
    fn from_pairs(pairs: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
        Self {
            by_zone: pairs
                .into_iter()
                .map(|(zone, recipe)| (zone.to_string(), recipe.to_string()))
                .collect(),
        }
    }

    fn get(&self, zone_name: &str) -> Option<&str> {
        self.by_zone.get(zone_name).map(String::as_str)
    }

    fn merge_path(&mut self, path: PathBuf) {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
            Err(error) => {
                tracing::warn!(
                    "[bong][audio] failed to read ambient zone config {}: {error}",
                    path.display()
                );
                return;
            }
        };
        let config = match serde_json::from_str::<AmbientZoneConfigFile>(&contents) {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(
                    "[bong][audio] failed to parse ambient zone config {}: {error}",
                    path.display()
                );
                return;
            }
        };

        for zone in config.zones {
            let name = zone.name.trim();
            if name.is_empty() {
                continue;
            }
            let Some(recipe_id) = zone
                .ambient_recipe_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if let Err(error) = validate_recipe_id(recipe_id) {
                tracing::warn!(
                    "[bong][audio] ignored ambient recipe `{}` for zone `{}` in {}: {error}",
                    recipe_id,
                    name,
                    path.display()
                );
                continue;
            }
            self.by_zone.insert(name.to_string(), recipe_id.to_string());
        }
    }
}

#[derive(Debug, Deserialize)]
struct AmbientZoneConfigFile {
    zones: Vec<AmbientZoneConfig>,
}

#[derive(Debug, Deserialize)]
struct AmbientZoneConfig {
    name: String,
    #[serde(default)]
    ambient_recipe_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AudioWorldKey {
    zone_name: String,
    recipe_id: String,
    music_state: AudioMusicState,
    is_night: bool,
    season: Season,
    tsy_depth: Option<TsyDepth>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AudioMusicState {
    Ambient,
    Combat,
    Cultivation,
    Tsy,
    Tribulation,
}

impl AudioMusicState {
    const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Ambient => "AMBIENT",
            Self::Combat => "COMBAT",
            Self::Cultivation => "CULTIVATION",
            Self::Tsy => "TSY",
            Self::Tribulation => "TRIBULATION",
        }
    }
}

type AmbientClientItem<'a> = (
    Entity,
    &'a mut Client,
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a CombatState>,
    Option<&'a TsyPresence>,
    Option<&'a TribulationState>,
);

#[derive(SystemParam)]
pub struct AmbientZoneChangeParams<'w> {
    registry: Option<Res<'w, SoundRecipeRegistry>>,
    zone_registry: Option<Res<'w, ZoneRegistry>>,
    zone_recipes: Option<Res<'w, AmbientZoneRecipes>>,
    clock: Option<Res<'w, CombatClock>>,
    cultivation_clock: Option<Res<'w, CultivationClock>>,
    practice_accumulator: Option<Res<'w, CultivationSessionPracticeAccumulator>>,
    season_state: Option<Res<'w, WorldSeasonState>>,
}

pub fn register(app: &mut App) {
    app.init_resource::<AmbientAudioState>()
        .insert_resource(AmbientZoneRecipes::load_default())
        .add_systems(
            Update,
            ambient_zone_change_system
                .after(crate::cultivation::tick::qi_regen_and_zone_drain_tick),
        );
}

pub fn ambient_zone_change_system(
    params: AmbientZoneChangeParams,
    mut state: ResMut<AmbientAudioState>,
    mut clients: Query<AmbientClientItem<'_>, With<Client>>,
) {
    let Some(registry) = params.registry.as_deref() else {
        return;
    };
    let zone_registry = params.zone_registry.as_deref();
    let tick = params
        .clock
        .as_deref()
        .map(|clock| clock.tick)
        .unwrap_or_default();
    let cultivation_tick = params
        .cultivation_clock
        .as_deref()
        .map(|clock| clock.tick)
        .unwrap_or(tick);
    let season = params
        .season_state
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_else(|| crate::world::season::query_season("", tick).season);
    let mut live_entities = std::collections::HashSet::new();

    for (entity, mut client, position, dimension, combat, tsy_presence, tribulation) in &mut clients
    {
        live_entities.insert(entity);
        let dim = dimension
            .map(|dim| dim.0)
            .unwrap_or(DimensionKind::Overworld);
        let zone = zone_registry.and_then(|registry| registry.find_zone(dim, position.get()));
        let zone_name = zone
            .map(|zone| zone.name.clone())
            .or_else(|| tsy_presence.map(|presence| presence.family_id.clone()))
            .unwrap_or_else(|| WILDERNESS_ZONE_NAME.to_string());
        let tsy_depth = zone.and_then(Zone::tsy_depth);
        let is_tsy = dim == DimensionKind::Tsy || tsy_presence.is_some() || tsy_depth.is_some();
        let is_cultivating = params
            .practice_accumulator
            .as_deref()
            .is_some_and(|accumulator| {
                accumulator.is_recently_practicing(entity, cultivation_tick)
            });
        let music_state = resolve_music_state(tick, combat, is_cultivating, is_tsy, tribulation);
        let recipe_id = recipe_for_state(
            music_state,
            params.zone_recipes.as_deref(),
            zone.as_ref(),
            zone_name.as_str(),
        );
        let is_night = matches!(music_state, AudioMusicState::Ambient) && is_night_tick(tick);
        let key = AudioWorldKey {
            zone_name: zone_name.clone(),
            recipe_id: recipe_id.to_string(),
            music_state,
            is_night,
            season,
            tsy_depth,
        };

        if state.last_by_entity.get(&entity) == Some(&key) {
            continue;
        }

        let Some(base_recipe) = registry.get(recipe_id) else {
            tracing::warn!("[bong][audio] ambient recipe `{recipe_id}` is missing");
            state.last_by_entity.insert(entity, key);
            continue;
        };
        let recipe = recipe_with_context(base_recipe, music_state, is_night, season, tsy_depth);
        let packet = AmbientZoneS2c {
            v: crate::schema::audio::AUDIO_EVENT_VERSION,
            zone_name: zone_name.clone(),
            ambient_recipe_id: recipe_id.to_string(),
            music_state: music_state.as_wire_str().to_string(),
            is_night,
            season: season.as_wire_str().to_string(),
            tsy_depth: tsy_depth.map(tsy_depth_wire).map(str::to_string),
            fade_ticks: AMBIENT_CROSSFADE_TICKS,
            pos: Some(block_pos(position.get())),
            volume_mul: volume_mul_for(music_state, is_night),
            pitch_shift: pitch_shift_for(music_state, season),
            recipe,
        };
        let bytes = match packet.to_json_bytes_checked() {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("[bong][audio] dropping ambient packet for {entity:?}: {error:?}");
                state.last_by_entity.insert(entity, key);
                continue;
            }
        };
        client.send_custom_payload(ident!("bong:audio/ambient_zone"), &bytes);
        state.last_by_entity.insert(entity, key);
    }

    state
        .last_by_entity
        .retain(|entity, _| live_entities.contains(entity));
}

fn resolve_music_state(
    tick: u64,
    combat: Option<&CombatState>,
    is_cultivating: bool,
    is_tsy: bool,
    tribulation: Option<&TribulationState>,
) -> AudioMusicState {
    if tribulation.is_some_and(|state| !state.failed) {
        return AudioMusicState::Tribulation;
    }
    if combat.is_some_and(|state| state.in_combat_until_tick.is_some_and(|until| until > tick)) {
        return AudioMusicState::Combat;
    }
    if is_tsy {
        return AudioMusicState::Tsy;
    }
    if is_cultivating {
        return AudioMusicState::Cultivation;
    }
    AudioMusicState::Ambient
}

fn recipe_for_state<'a>(
    state: AudioMusicState,
    zone_recipes: Option<&'a AmbientZoneRecipes>,
    zone: Option<&&Zone>,
    zone_name: &'a str,
) -> &'a str {
    match state {
        AudioMusicState::Ambient => zone_recipes
            .and_then(|recipes| {
                recipes.get(zone.map(|zone| zone.name.as_str()).unwrap_or(zone_name))
            })
            .unwrap_or_else(|| {
                ambient_recipe_for_zone(zone.map(|zone| zone.name.as_str()).unwrap_or(zone_name))
            }),
        AudioMusicState::Combat => "combat_music",
        AudioMusicState::Cultivation => "cultivation_meditate",
        AudioMusicState::Tsy => "ambient_tsy",
        AudioMusicState::Tribulation => "tribulation_atmosphere",
    }
}

fn ambient_recipe_for_zone(zone_name: &str) -> &'static str {
    match zone_name {
        "spawn" | "spawn_plain" => "ambient_spawn_plain",
        "qingyun_peaks" => "ambient_qingyun_peaks",
        "lingquan_marsh" | "spring_marsh" => "ambient_spring_marsh",
        "rift_valley" | "blood_valley" => "ambient_rift_valley",
        "north_wastes" => "ambient_north_wastes",
        _ => "ambient_wilderness",
    }
}

fn recipe_with_context(
    base: &SoundRecipe,
    state: AudioMusicState,
    is_night: bool,
    season: Season,
    tsy_depth: Option<TsyDepth>,
) -> SoundRecipe {
    let mut recipe = base.clone();
    if recipe.loop_cfg.is_none() {
        recipe.loop_cfg = Some(LoopConfig {
            interval_ticks: 120,
            while_flag: AUDIO_WORLD_LOOP_FLAG.to_string(),
        });
    }

    if matches!(state, AudioMusicState::Ambient) && is_night {
        recipe
            .layers
            .push(layer("minecraft:entity.bat.ambient", 0.02, 0.3, 10));
    }
    match base.id.as_str() {
        "ambient_spring_marsh" => {
            recipe
                .layers
                .push(layer("minecraft:entity.frog.ambient", 0.04, 0.6, 20));
        }
        "ambient_rift_valley" => {
            recipe
                .layers
                .push(layer("minecraft:block.stone.break", 0.05, 0.3, 30));
        }
        "ambient_north_wastes" => {
            recipe
                .layers
                .push(layer("minecraft:entity.wolf.growl", 0.04, 0.4, 40));
        }
        "ambient_tsy" => {
            recipe
                .layers
                .push(layer("minecraft:block.anvil.land", 0.03, 0.2, 15));
        }
        _ => {}
    }
    if matches!(state, AudioMusicState::Ambient | AudioMusicState::Tsy) {
        match season {
            Season::Summer => {
                recipe
                    .layers
                    .push(layer("minecraft:block.fire.ambient", 0.02, 1.0, 0))
            }
            Season::Winter => {
                recipe
                    .layers
                    .push(layer("minecraft:block.powder_snow.step", 0.02, 0.8, 0));
            }
            Season::SummerToWinter | Season::WinterToSummer => {}
        }
    }
    if matches!(state, AudioMusicState::Tsy) && tsy_depth == Some(TsyDepth::Deep) {
        recipe.layers.push(layer(
            "minecraft:block.respawn_anchor.deplete",
            0.06,
            0.4,
            40,
        ));
    }
    recipe
}

fn layer(sound: &str, volume: f32, pitch: f32, delay_ticks: u32) -> SoundLayer {
    SoundLayer {
        sound: sound.to_string(),
        volume,
        pitch,
        delay_ticks,
    }
}

fn volume_mul_for(state: AudioMusicState, is_night: bool) -> f32 {
    if matches!(state, AudioMusicState::Ambient) && is_night {
        1.5
    } else {
        1.0
    }
}

fn pitch_shift_for(state: AudioMusicState, season: Season) -> f32 {
    if !matches!(state, AudioMusicState::Ambient | AudioMusicState::Tsy) {
        return 0.0;
    }
    match season {
        Season::Summer => PITCH_SHIFT_SUMMER,
        Season::Winter => PITCH_SHIFT_WINTER,
        Season::SummerToWinter | Season::WinterToSummer => 0.0,
    }
}

fn is_night_tick(tick: u64) -> bool {
    let day_tick = tick % VANILLA_DAY_TICKS;
    (NIGHT_START_TICK..NIGHT_END_TICK).contains(&day_tick)
}

fn tsy_depth_wire(depth: TsyDepth) -> &'static str {
    match depth {
        TsyDepth::Shallow => "shallow",
        TsyDepth::Mid => "mid",
        TsyDepth::Deep => "deep",
    }
}

fn block_pos(origin: DVec3) -> [i32; 3] {
    [
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, Update};
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    use crate::combat::components::CombatState;
    use crate::cultivation::tribulation::{TribulationKind, TribulationPhase, TribulationState};
    use crate::world::zone::Zone;

    fn setup_app(zones: ZoneRegistry) -> App {
        let mut app = App::new();
        app.insert_resource(SoundRecipeRegistry::load_default().expect("audio recipes load"));
        app.insert_resource(AmbientZoneRecipes::load_default());
        app.insert_resource(zones);
        app.insert_resource(CombatClock::default());
        app.insert_resource(CultivationClock::default());
        app.insert_resource(CultivationSessionPracticeAccumulator::default());
        app.init_resource::<AmbientAudioState>();
        app.add_systems(Update, ambient_zone_change_system);
        app
    }

    fn spawn_client(app: &mut App, name: &str, pos: [f64; 3]) -> (Entity, MockClientHelper) {
        let (mut bundle, helper) = create_mock_client(name);
        bundle.player.position = Position::new(pos);
        let entity = app.world_mut().spawn(bundle).id();
        (entity, helper)
    }

    fn flush_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client.flush_packets().expect("mock client flushes packets");
        }
    }

    fn collect_ambient(helper: &mut MockClientHelper) -> Vec<AmbientZoneS2c> {
        let mut payloads = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != AUDIO_AMBIENT_ZONE_CHANNEL {
                continue;
            }
            payloads
                .push(serde_json::from_slice(packet.data.0 .0).expect("ambient packet decodes"));
        }
        payloads
    }

    #[test]
    fn default_zone_recipe_config_loads_world_and_tsy() {
        let recipes = AmbientZoneRecipes::load_default();

        assert_eq!(recipes.get("spawn"), Some("ambient_spawn_plain"));
        assert_eq!(recipes.get("lingquan_marsh"), Some("ambient_spring_marsh"));
        assert_eq!(recipes.get("tsy_lingxu_01_deep"), Some("ambient_tsy"));
    }

    #[test]
    fn recipe_context_adds_ambient_detail_layers() {
        let cases = [
            ("ambient_spring_marsh", "minecraft:entity.frog.ambient"),
            ("ambient_rift_valley", "minecraft:block.stone.break"),
            ("ambient_north_wastes", "minecraft:entity.wolf.growl"),
            ("ambient_tsy", "minecraft:block.anvil.land"),
        ];

        for (recipe_id, expected_sound) in cases {
            let recipe = recipe_with_context(
                &test_recipe(recipe_id),
                if recipe_id == "ambient_tsy" {
                    AudioMusicState::Tsy
                } else {
                    AudioMusicState::Ambient
                },
                false,
                Season::SummerToWinter,
                None,
            );

            assert!(
                recipe
                    .layers
                    .iter()
                    .any(|layer| layer.sound == expected_sound),
                "{recipe_id} should include {expected_sound}"
            );
        }
    }

    #[test]
    fn configured_zone_recipe_overrides_name_fallback() {
        let mut app = setup_app(ZoneRegistry {
            zones: vec![test_zone(
                "custom_audio_zone",
                [0.0, 60.0, 0.0],
                [16.0, 90.0, 16.0],
                DimensionKind::Overworld,
            )],
        });
        app.insert_resource(AmbientZoneRecipes::from_pairs([(
            "custom_audio_zone",
            "ambient_qingyun_peaks",
        )]));
        let (_entity, mut helper) = spawn_client(&mut app, "listener", [8.0, 64.0, 8.0]);

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].zone_name, "custom_audio_zone");
        assert_eq!(payloads[0].ambient_recipe_id, "ambient_qingyun_peaks");
    }

    #[test]
    fn zone_change_emits_ambient() {
        let mut app = setup_app(ZoneRegistry {
            zones: vec![
                test_zone(
                    "spawn",
                    [0.0, 60.0, 0.0],
                    [16.0, 90.0, 16.0],
                    DimensionKind::Overworld,
                ),
                test_zone(
                    "lingquan_marsh",
                    [32.0, 60.0, 0.0],
                    [64.0, 90.0, 16.0],
                    DimensionKind::Overworld,
                ),
            ],
        });
        let (entity, mut helper) = spawn_client(&mut app, "near", [8.0, 64.0, 8.0]);

        app.update();
        flush_packets(&mut app);
        let first = collect_ambient(&mut helper);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].zone_name, "spawn");
        assert_eq!(first[0].ambient_recipe_id, "ambient_spawn_plain");

        app.world_mut()
            .entity_mut(entity)
            .insert(Position::new([40.0, 64.0, 8.0]));
        app.update();
        flush_packets(&mut app);
        let second = collect_ambient(&mut helper);
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].zone_name, "lingquan_marsh");
        assert_eq!(second[0].ambient_recipe_id, "ambient_spring_marsh");
    }

    #[test]
    fn unknown_zone_uses_wilderness_not_spawn_recipe() {
        let mut app = setup_app(ZoneRegistry {
            zones: vec![test_zone(
                "spawn",
                [0.0, 60.0, 0.0],
                [16.0, 90.0, 16.0],
                DimensionKind::Overworld,
            )],
        });
        let (_entity, mut helper) = spawn_client(&mut app, "wanderer", [128.0, 64.0, 128.0]);

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);

        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].zone_name, WILDERNESS_ZONE_NAME);
        assert_eq!(payloads[0].ambient_recipe_id, "ambient_wilderness");
    }

    #[test]
    fn missing_recipe_records_failed_key_until_state_changes() {
        let mut app = setup_app(ZoneRegistry::fallback());
        app.insert_resource(AmbientZoneRecipes::from_pairs([(
            "spawn",
            "missing_recipe",
        )]));
        let (entity, mut helper) = spawn_client(&mut app, "listener", [8.0, 64.0, 8.0]);

        app.update();
        flush_packets(&mut app);
        assert!(collect_ambient(&mut helper).is_empty());
        assert!(app
            .world()
            .resource::<AmbientAudioState>()
            .last_by_entity
            .contains_key(&entity));

        app.update();
        flush_packets(&mut app);
        assert!(collect_ambient(&mut helper).is_empty());
    }

    #[test]
    fn combat_state_triggers_music() {
        let mut app = setup_app(ZoneRegistry::fallback());
        app.world_mut().resource_mut::<CombatClock>().tick = 10;
        let (entity, mut helper) = spawn_client(&mut app, "fighter", [8.0, 64.0, 8.0]);
        app.world_mut().entity_mut(entity).insert(CombatState {
            in_combat_until_tick: Some(200),
            last_attack_at_tick: Some(10),
            incoming_window: None,
        });

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].music_state, "COMBAT");
        assert_eq!(payloads[0].ambient_recipe_id, "combat_music");
    }

    #[test]
    fn meditation_triggers_ambient() {
        let mut app = setup_app(ZoneRegistry::fallback());
        let (entity, mut helper) = spawn_client(&mut app, "cultivator", [8.0, 64.0, 8.0]);
        app.world_mut().resource_mut::<CultivationClock>().tick = 20;
        app.world_mut()
            .resource_mut::<CultivationSessionPracticeAccumulator>()
            .note_practice_tick_for_tests(entity, 20);

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].music_state, "CULTIVATION");
        assert_eq!(payloads[0].ambient_recipe_id, "cultivation_meditate");
    }

    #[test]
    fn tsy_dimension_triggers_ambient() {
        let mut app = setup_app(ZoneRegistry {
            zones: vec![test_zone(
                "tsy_lingxu_01_deep",
                [0.0, 60.0, 0.0],
                [16.0, 90.0, 16.0],
                DimensionKind::Tsy,
            )],
        });
        let (entity, mut helper) = spawn_client(&mut app, "delver", [8.0, 64.0, 8.0]);
        app.world_mut()
            .entity_mut(entity)
            .insert(CurrentDimension(DimensionKind::Tsy));

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].music_state, "TSY");
        assert_eq!(payloads[0].ambient_recipe_id, "ambient_tsy");
        assert_eq!(payloads[0].tsy_depth.as_deref(), Some("deep"));
        assert!(payloads[0]
            .recipe
            .layers
            .iter()
            .any(|layer| layer.sound == "minecraft:block.respawn_anchor.deplete"));
    }

    #[test]
    fn tribulation_overrides_combat() {
        let mut app = setup_app(ZoneRegistry::fallback());
        app.world_mut().resource_mut::<CombatClock>().tick = 10;
        let (entity, mut helper) = spawn_client(&mut app, "tribulator", [8.0, 64.0, 8.0]);
        app.world_mut()
            .entity_mut(entity)
            .insert(CombatState {
                in_combat_until_tick: Some(200),
                last_attack_at_tick: Some(10),
                incoming_window: None,
            })
            .insert(TribulationState {
                kind: TribulationKind::DuXu,
                phase: TribulationPhase::Omen,
                epicenter: [8.0, 64.0, 8.0],
                wave_current: 0,
                waves_total: 3,
                started_tick: 10,
                phase_started_tick: 10,
                next_wave_tick: 100,
                participants: vec![],
                failed: false,
            });

        app.update();
        flush_packets(&mut app);
        let payloads = collect_ambient(&mut helper);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].music_state, "TRIBULATION");
        assert_eq!(payloads[0].ambient_recipe_id, "tribulation_atmosphere");
    }

    fn test_zone(name: &str, min: [f64; 3], max: [f64; 3], dimension: DimensionKind) -> Zone {
        Zone {
            name: name.to_string(),
            dimension,
            bounds: (
                DVec3::new(min[0], min[1], min[2]),
                DVec3::new(max[0], max[1], max[2]),
            ),
            spirit_qi: 0.5,
            danger_level: 1,
            active_events: vec![],
            patrol_anchors: vec![DVec3::new(min[0], min[1], min[2])],
            blocked_tiles: vec![],
        }
    }

    fn test_recipe(id: &str) -> SoundRecipe {
        SoundRecipe {
            id: id.to_string(),
            layers: vec![layer("minecraft:ambient.cave", 0.1, 1.0, 0)],
            loop_cfg: None,
            priority: 10,
            attenuation: crate::schema::audio::AudioAttenuation::ZoneBroadcast,
            category: crate::schema::audio::AudioSoundCategory::Ambient,
            bus: None,
        }
    }
}
