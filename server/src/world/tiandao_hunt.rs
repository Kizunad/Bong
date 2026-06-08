use crate::cultivation::components::{Cultivation, Realm};
use crate::cultivation::tick::CultivationClock;
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::player::state::canonical_player_id;
use crate::schema::agent_command::Command;
use crate::schema::common::CommandType;
use crate::schema::vfx_event::VfxEventPayloadV1;
use crate::world::calamity::{EVENT_BEAST_TIDE, EVENT_REALM_COLLAPSE, EVENT_THUNDER_TRIBULATION};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::events::ActiveEventsResource;
use crate::world::season::{Season, WorldSeasonState};
use crate::world::zone::ZoneRegistry;
use serde_json::json;
use std::collections::HashMap;
use valence::prelude::{
    bevy_ecs, ident, App, Client, Commands, Component, DVec3, Entity, Events, Position, Query, Res,
    ResMut, Update, Username, With, Without,
};

const ATTENTION_MIN: f64 = 0.0;
const ATTENTION_MAX: f64 = 100.0;
pub const TIANDAO_HUNT_EVAL_INTERVAL_TICKS: u64 = 10 * 20;
const TIANDAO_PRESENCE_CHANNEL: &str = "bong:tiandao_presence";
const TIANDAO_PRESENCE_SPIRIT_REALM_MIN_RANK: u8 = 4;
const TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS: u64 = 3 * 60 * 20;
const TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS: u64 = 5 * 60 * 20;
const TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS: u64 = 2 * 60 * 20;
const TIANDAO_AUDIO_INSTANCE_BASE: u64 = 710_000;

#[derive(Debug, Clone, Component, PartialEq)]
pub struct TiandaoAttention {
    pub level: f64,
    pub response: TiandaoResponseLevel,
    pub last_eval_tick: u64,
    pub accumulation_rate: f64,
    pub peak_level: f64,
    pub last_response_tick: u64,
    pub last_emitted_response: TiandaoResponseLevel,
    pub last_presence_tick: u64,
}

impl Default for TiandaoAttention {
    fn default() -> Self {
        Self {
            level: 0.0,
            response: TiandaoResponseLevel::None,
            last_eval_tick: 0,
            accumulation_rate: 0.0,
            peak_level: 0.0,
            last_response_tick: 0,
            last_emitted_response: TiandaoResponseLevel::None,
            last_presence_tick: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TiandaoResponseLevel {
    None,
    Watch,
    Pressure,
    Tribulation,
    Annihilate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TiandaoActivity {
    #[cfg(test)]
    Meditating,
    #[cfg(test)]
    Combat,
    #[cfg(test)]
    Moving,
    Standing,
    #[cfg(test)]
    InNiche,
}

type AttentionAttachFilter = (With<Client>, With<Cultivation>, Without<TiandaoAttention>);
type TiandaoPlayerQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Cultivation,
        &'static Position,
        &'static Username,
        &'static mut Client,
        Option<&'static CurrentDimension>,
        &'static mut TiandaoAttention,
    ),
    With<Client>,
>;

struct TiandaoResponseSinks<'a> {
    zones: Option<&'a mut ZoneRegistry>,
    active_events: Option<&'a mut ActiveEventsResource>,
    vfx_events: Option<&'a mut Events<VfxEventRequest>>,
    audio_events: Option<&'a mut Events<PlaySoundRecipeRequest>>,
}

impl Default for TiandaoActivity {
    fn default() -> Self {
        Self::Standing
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoAttentionInput {
    pub realm: Realm,
    pub zone_spirit_qi: f64,
    pub activity: TiandaoActivity,
    pub season: Season,
}

pub fn register(app: &mut App) {
    app.add_systems(Update, (attach_tiandao_attention, tiandao_hunt_tick));
}

pub fn attach_tiandao_attention(
    mut commands: Commands,
    players: Query<Entity, AttentionAttachFilter>,
) {
    for entity in players.iter() {
        commands.entity(entity).insert(TiandaoAttention::default());
    }
}

pub fn tiandao_hunt_tick(
    clock: Option<Res<CultivationClock>>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    season: Option<Res<WorldSeasonState>>,
    mut active_events: Option<ResMut<ActiveEventsResource>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    mut players: TiandaoPlayerQuery<'_, '_>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;
    let season = season
        .as_deref()
        .map(|state| state.current.season)
        .unwrap_or_default();
    for (player_entity, cultivation, position, username, mut client, dimension, mut attention) in
        players.iter_mut()
    {
        let eval = apply_attention_eval(
            cultivation,
            position,
            dimension,
            &mut attention,
            zones.as_deref(),
            season,
            now_tick,
        );
        let Some(eval) = eval else {
            continue;
        };

        emit_tiandao_presence_payload(
            &mut client,
            cultivation.realm,
            &attention,
            eval.zone_name.as_deref().unwrap_or("unknown"),
            eval.zone_spirit_qi,
            now_tick,
        );
        attention.last_presence_tick = now_tick;

        apply_tiandao_response_chain(
            &mut attention,
            eval,
            player_entity,
            username.0.as_str(),
            TiandaoResponseSinks {
                zones: zones.as_deref_mut(),
                active_events: active_events.as_deref_mut(),
                vfx_events: vfx_events.as_deref_mut(),
                audio_events: audio_events.as_deref_mut(),
            },
            now_tick,
        );
    }
}

pub fn apply_attention_eval(
    cultivation: &Cultivation,
    position: &Position,
    dimension: Option<&CurrentDimension>,
    attention: &mut TiandaoAttention,
    zones: Option<&ZoneRegistry>,
    season: Season,
    now_tick: u64,
) -> Option<TiandaoEvalSnapshot> {
    let dimension = dimension
        .map(|dim| dim.0)
        .unwrap_or(DimensionKind::Overworld);
    let zone = zones
        .and_then(|registry| registry.find_zone(dimension, position.0))
        .map(|zone| (zone.name.clone(), zone.spirit_qi));
    let zone_spirit_qi = zone
        .as_ref()
        .map(|(_, spirit_qi)| *spirit_qi)
        .unwrap_or_default();
    let input = TiandaoAttentionInput {
        realm: cultivation.realm,
        zone_spirit_qi,
        activity: TiandaoActivity::Standing,
        season,
    };
    if should_evaluate_attention(now_tick, attention.last_eval_tick) {
        advance_attention(attention, input, now_tick);
        Some(TiandaoEvalSnapshot {
            position: position.0,
            zone_name: zone.map(|(name, _)| name),
            zone_spirit_qi,
            response: attention.response,
            level: attention.level,
        })
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TiandaoEvalSnapshot {
    pub position: DVec3,
    pub zone_name: Option<String>,
    pub zone_spirit_qi: f64,
    pub response: TiandaoResponseLevel,
    pub level: f64,
}

pub fn should_evaluate_attention(now_tick: u64, last_eval_tick: u64) -> bool {
    now_tick >= last_eval_tick
        && now_tick.saturating_sub(last_eval_tick) >= TIANDAO_HUNT_EVAL_INTERVAL_TICKS
}

pub fn advance_attention(
    attention: &mut TiandaoAttention,
    input: TiandaoAttentionInput,
    eval_tick: u64,
) {
    let previous_response = attention.response;
    let accumulation_rate = accumulation_rate(input);
    let decay = decay_rate(previous_response, input.zone_spirit_qi);
    let next_level =
        (attention.level + accumulation_rate - decay).clamp(ATTENTION_MIN, ATTENTION_MAX);

    attention.level = next_level;
    attention.accumulation_rate = accumulation_rate;
    attention.peak_level = attention.peak_level.max(next_level);
    attention.response = response_for_level(previous_response, next_level);
    attention.last_eval_tick = eval_tick;
}

pub fn accumulation_rate(input: TiandaoAttentionInput) -> f64 {
    realm_base_rate(input.realm)
        * zone_qi_factor(input.zone_spirit_qi)
        * activity_factor(input.activity)
        * season_factor(input.season)
}

pub const fn realm_base_rate(realm: Realm) -> f64 {
    match realm {
        Realm::Awaken | Realm::Induce => 0.0,
        Realm::Condense => 0.01,
        Realm::Solidify => 0.05,
        Realm::Spirit => 0.15,
        Realm::Void => 0.40,
    }
}

pub fn zone_qi_factor(spirit_qi: f64) -> f64 {
    if spirit_qi <= 0.1 {
        0.3
    } else if spirit_qi <= 0.3 {
        0.6
    } else if spirit_qi <= 0.6 {
        1.0
    } else {
        1.8
    }
}

pub const fn activity_factor(activity: TiandaoActivity) -> f64 {
    match activity {
        #[cfg(test)]
        TiandaoActivity::Meditating => 1.5,
        #[cfg(test)]
        TiandaoActivity::Combat => 1.2,
        #[cfg(test)]
        TiandaoActivity::Moving => 0.8,
        TiandaoActivity::Standing => 1.0,
        #[cfg(test)]
        TiandaoActivity::InNiche => 0.5,
    }
}

pub const fn season_factor(season: Season) -> f64 {
    if season.is_xizhuan() {
        1.5
    } else {
        1.0
    }
}

pub fn decay_rate(response: TiandaoResponseLevel, zone_spirit_qi: f64) -> f64 {
    let base = match response {
        TiandaoResponseLevel::None => 0.08,
        TiandaoResponseLevel::Watch => 0.05,
        TiandaoResponseLevel::Pressure => 0.03,
        TiandaoResponseLevel::Tribulation => 0.01,
        TiandaoResponseLevel::Annihilate => 0.0,
    };
    base * zone_decay_multiplier(zone_spirit_qi)
}

pub fn zone_decay_multiplier(spirit_qi: f64) -> f64 {
    if spirit_qi < 0.0 {
        5.0
    } else if spirit_qi == 0.0 {
        3.0
    } else {
        1.0
    }
}

pub fn response_for_level(previous: TiandaoResponseLevel, level: f64) -> TiandaoResponseLevel {
    match previous {
        TiandaoResponseLevel::None => {
            if level >= 15.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::None
            }
        }
        TiandaoResponseLevel::Watch => {
            if level >= 40.0 {
                TiandaoResponseLevel::Pressure
            } else if level < 10.0 {
                TiandaoResponseLevel::None
            } else {
                TiandaoResponseLevel::Watch
            }
        }
        TiandaoResponseLevel::Pressure => {
            if level >= 70.0 {
                TiandaoResponseLevel::Tribulation
            } else if level < 30.0 {
                TiandaoResponseLevel::Watch
            } else {
                TiandaoResponseLevel::Pressure
            }
        }
        TiandaoResponseLevel::Tribulation => {
            if level >= 90.0 {
                TiandaoResponseLevel::Annihilate
            } else if level < 60.0 {
                TiandaoResponseLevel::Pressure
            } else {
                TiandaoResponseLevel::Tribulation
            }
        }
        TiandaoResponseLevel::Annihilate => {
            if level < 80.0 {
                TiandaoResponseLevel::Tribulation
            } else {
                TiandaoResponseLevel::Annihilate
            }
        }
    }
}

fn apply_tiandao_response_chain(
    attention: &mut TiandaoAttention,
    eval: TiandaoEvalSnapshot,
    player_entity: Entity,
    username: &str,
    sinks: TiandaoResponseSinks<'_>,
    now_tick: u64,
) {
    let Some(profile) = tiandao_response_profile(eval.response) else {
        return;
    };

    if attention.last_emitted_response == eval.response
        && attention.last_response_tick != 0
        && now_tick.saturating_sub(attention.last_response_tick) < profile.interval_ticks
    {
        return;
    }
    attention.last_response_tick = now_tick;
    attention.last_emitted_response = eval.response;

    if let Some(audio_events) = sinks.audio_events {
        audio_events.send(tiandao_audio_request(&eval, profile, player_entity));
    }
    if let (Some(vfx_events), Some(request)) =
        (sinks.vfx_events, tiandao_vfx_request(&eval, profile))
    {
        vfx_events.send(request);
    }

    let Some(event_name) = profile.event_name else {
        return;
    };
    let (Some(zone_name), Some(active_events), Some(zones)) =
        (eval.zone_name.as_deref(), sinks.active_events, sinks.zones)
    else {
        return;
    };
    let target_player = canonical_player_id(username);
    let command = tiandao_spawn_event_command(
        zone_name,
        event_name,
        profile.duration_ticks,
        profile.intensity,
        target_player.as_str(),
        eval.response,
    );
    active_events.enqueue_from_spawn_command_with_karma(&command, Some(zones), None, None);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TiandaoResponseProfile {
    pub vignette_rgb: u32,
    pub vignette_alpha: f64,
    pub shake_intensity: f64,
    pub saturation: f64,
    pub audio_recipe_id: &'static str,
    pub vfx_event_id: Option<&'static str>,
    pub vfx_color: &'static str,
    pub event_name: Option<&'static str>,
    pub interval_ticks: u64,
    pub duration_ticks: u64,
    pub intensity: f64,
}

pub fn tiandao_response_profile(response: TiandaoResponseLevel) -> Option<TiandaoResponseProfile> {
    match response {
        TiandaoResponseLevel::None => None,
        TiandaoResponseLevel::Watch => Some(TiandaoResponseProfile {
            vignette_rgb: 0x400800,
            vignette_alpha: 0.03,
            shake_intensity: 0.0,
            saturation: 1.0,
            audio_recipe_id: "tiandao_watch_ambient",
            vfx_event_id: None,
            vfx_color: "#400800",
            event_name: None,
            interval_ticks: 5 * 60 * 20,
            duration_ticks: 60 * 20,
            intensity: 0.15,
        }),
        TiandaoResponseLevel::Pressure => Some(TiandaoResponseProfile {
            vignette_rgb: 0x400800,
            vignette_alpha: 0.08,
            shake_intensity: 0.35,
            saturation: 0.95,
            audio_recipe_id: "tiandao_pressure_ambient",
            vfx_event_id: Some("bong:tiandao_beast_spawn"),
            vfx_color: "#604020",
            event_name: Some(EVENT_BEAST_TIDE),
            interval_ticks: TIANDAO_PRESSURE_EVENT_INTERVAL_TICKS,
            duration_ticks: 60 * 20,
            intensity: 0.35,
        }),
        TiandaoResponseLevel::Tribulation => Some(TiandaoResponseProfile {
            vignette_rgb: 0x601000,
            vignette_alpha: 0.15,
            shake_intensity: 0.65,
            saturation: 0.85,
            audio_recipe_id: "tiandao_tribulation_ambient",
            vfx_event_id: Some("bong:tiandao_directed_thunder"),
            vfx_color: "#E0E8FF",
            event_name: Some(EVENT_THUNDER_TRIBULATION),
            interval_ticks: TIANDAO_TRIBULATION_EVENT_INTERVAL_TICKS,
            duration_ticks: 60 * 20,
            intensity: 0.75,
        }),
        TiandaoResponseLevel::Annihilate => Some(TiandaoResponseProfile {
            vignette_rgb: 0x801000,
            vignette_alpha: 0.25,
            shake_intensity: 1.0,
            saturation: 0.7,
            audio_recipe_id: "tiandao_annihilate_ambient",
            vfx_event_id: Some("bong:realm_collapse_boundary"),
            vfx_color: "#601000",
            event_name: Some(EVENT_REALM_COLLAPSE),
            interval_ticks: TIANDAO_ANNIHILATE_EVENT_INTERVAL_TICKS,
            duration_ticks: 30 * 20,
            intensity: 1.0,
        }),
    }
}

fn tiandao_audio_request(
    eval: &TiandaoEvalSnapshot,
    profile: TiandaoResponseProfile,
    player_entity: Entity,
) -> PlaySoundRecipeRequest {
    let pos = eval.position;
    PlaySoundRecipeRequest {
        recipe_id: profile.audio_recipe_id.to_string(),
        instance_id: TIANDAO_AUDIO_INSTANCE_BASE + u64::from(eval.response.rank()),
        pos: Some([
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        ]),
        flag: Some(format!("tiandao:{}", eval.response.as_wire())),
        volume_mul: 1.0,
        pitch_shift: 0.0,
        recipient: AudioRecipient::Single(player_entity),
    }
}

fn tiandao_vfx_request(
    eval: &TiandaoEvalSnapshot,
    profile: TiandaoResponseProfile,
) -> Option<VfxEventRequest> {
    let event_id = profile.vfx_event_id?;
    Some(VfxEventRequest::new(
        eval.position,
        VfxEventPayloadV1::SpawnParticle {
            event_id: event_id.to_string(),
            origin: [eval.position.x, eval.position.y, eval.position.z],
            direction: Some([6.0, 1.0, 6.0]),
            color: Some(profile.vfx_color.to_string()),
            strength: Some(profile.intensity.clamp(0.0, 1.0) as f32),
            count: Some(match eval.response {
                TiandaoResponseLevel::Watch => 1,
                TiandaoResponseLevel::Pressure => 6,
                TiandaoResponseLevel::Tribulation => 18,
                TiandaoResponseLevel::Annihilate => 32,
                TiandaoResponseLevel::None => 1,
            }),
            duration_ticks: Some(match eval.response {
                TiandaoResponseLevel::Watch => 80,
                TiandaoResponseLevel::Pressure => 100,
                TiandaoResponseLevel::Tribulation => 80,
                TiandaoResponseLevel::Annihilate => 160,
                TiandaoResponseLevel::None => 20,
            }),
        },
    ))
}

fn tiandao_spawn_event_command(
    zone_name: &str,
    event_name: &str,
    duration_ticks: u64,
    intensity: f64,
    target_player: &str,
    response: TiandaoResponseLevel,
) -> Command {
    Command {
        command_type: CommandType::SpawnEvent,
        target: zone_name.to_string(),
        params: HashMap::from([
            ("event".to_string(), json!(event_name)),
            ("duration_ticks".to_string(), json!(duration_ticks)),
            ("intensity".to_string(), json!(intensity.clamp(0.0, 1.0))),
            ("target_player".to_string(), json!(target_player)),
            ("attention_level".to_string(), json!(response.as_wire())),
            ("reason".to_string(), json!("tiandao_hunt_p1")),
        ]),
    }
}

fn emit_tiandao_presence_payload(
    client: &mut Client,
    realm: Realm,
    attention: &TiandaoAttention,
    zone_name: &str,
    zone_spirit_qi: f64,
    now_tick: u64,
) {
    let profile = (realm_rank(realm) >= TIANDAO_PRESENCE_SPIRIT_REALM_MIN_RANK)
        .then(|| tiandao_response_profile(attention.response))
        .flatten();
    let payload = json!({
        "v": 1,
        "type": "tiandao_presence",
        "level": attention.level,
        "response": attention.response.as_wire(),
        "zone": zone_name,
        "zone_spirit_qi": zone_spirit_qi,
        "vignette_rgb": profile.map(|profile| profile.vignette_rgb).unwrap_or(0),
        "vignette_alpha": profile.map(|profile| profile.vignette_alpha).unwrap_or(0.0),
        "shake_intensity": profile.map(|profile| profile.shake_intensity).unwrap_or(0.0),
        "saturation": profile.map(|profile| profile.saturation).unwrap_or(1.0),
        "audio_recipe": profile.map(|profile| profile.audio_recipe_id).unwrap_or(""),
        "tick": now_tick,
    });
    match serde_json::to_vec(&payload) {
        Ok(bytes) => {
            let _ = TIANDAO_PRESENCE_CHANNEL;
            client.send_custom_payload(ident!("bong:tiandao_presence"), &bytes);
        }
        Err(error) => {
            tracing::warn!("[bong][tiandao_hunt] failed to serialize presence payload: {error}");
        }
    }
}

pub const fn realm_rank(realm: Realm) -> u8 {
    match realm {
        Realm::Awaken => 0,
        Realm::Induce => 1,
        Realm::Condense => 2,
        Realm::Solidify => 3,
        Realm::Spirit => 4,
        Realm::Void => 5,
    }
}

impl TiandaoResponseLevel {
    pub const fn as_wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Watch => "watch",
            Self::Pressure => "pressure",
            Self::Tribulation => "tribulation",
            Self::Annihilate => "annihilate",
        }
    }

    pub const fn rank(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Watch => 1,
            Self::Pressure => 2,
            Self::Tribulation => 3,
            Self::Annihilate => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::components::{Cultivation, Realm};
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::season::Season;
    use valence::prelude::DVec3;

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    fn input(realm: Realm) -> TiandaoAttentionInput {
        TiandaoAttentionInput {
            realm,
            zone_spirit_qi: 0.6,
            activity: TiandaoActivity::Standing,
            season: Season::Summer,
        }
    }

    #[test]
    fn realm_base_rates_match_plan_scale() {
        assert_eq!(realm_base_rate(Realm::Awaken), 0.0);
        assert_eq!(realm_base_rate(Realm::Induce), 0.0);
        assert_eq!(realm_base_rate(Realm::Condense), 0.01);
        assert_eq!(realm_base_rate(Realm::Solidify), 0.05);
        assert_eq!(realm_base_rate(Realm::Spirit), 0.15);
        assert_eq!(realm_base_rate(Realm::Void), 0.40);
    }

    #[test]
    fn low_realms_never_accumulate_attention() {
        for realm in [Realm::Awaken, Realm::Induce] {
            let mut attention = TiandaoAttention {
                level: 10.0,
                ..TiandaoAttention::default()
            };
            advance_attention(&mut attention, input(realm), 200);
            assert!(attention.level < 10.0);
            assert_eq!(attention.accumulation_rate, 0.0);
            assert_eq!(attention.response, TiandaoResponseLevel::None);
        }
    }

    #[test]
    fn zone_qi_factor_has_all_plan_thresholds() {
        assert_eq!(zone_qi_factor(-0.5), 0.3);
        assert_eq!(zone_qi_factor(0.1), 0.3);
        assert_eq!(zone_qi_factor(0.1001), 0.6);
        assert_eq!(zone_qi_factor(0.3), 0.6);
        assert_eq!(zone_qi_factor(0.3001), 1.0);
        assert_eq!(zone_qi_factor(0.6), 1.0);
        assert_eq!(zone_qi_factor(0.6001), 1.8);
    }

    #[test]
    fn activity_factor_matches_plan_actions() {
        assert_eq!(activity_factor(TiandaoActivity::Meditating), 1.5);
        assert_eq!(activity_factor(TiandaoActivity::Combat), 1.2);
        assert_eq!(activity_factor(TiandaoActivity::Moving), 0.8);
        assert_eq!(activity_factor(TiandaoActivity::Standing), 1.0);
        assert_eq!(activity_factor(TiandaoActivity::InNiche), 0.5);
    }

    #[test]
    fn xizhuan_season_increases_accumulation() {
        let normal = accumulation_rate(TiandaoAttentionInput {
            season: Season::Summer,
            ..input(Realm::Void)
        });
        let xizhuan = accumulation_rate(TiandaoAttentionInput {
            season: Season::SummerToWinter,
            ..input(Realm::Void)
        });
        assert!((xizhuan - normal * 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn response_upgrade_thresholds_are_inclusive() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::None, 15.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 40.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 70.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 90.0),
            TiandaoResponseLevel::Annihilate
        );
    }

    #[test]
    fn response_downgrade_uses_hysteresis() {
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 10.0),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Watch, 9.99),
            TiandaoResponseLevel::None
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 30.0),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Pressure, 29.99),
            TiandaoResponseLevel::Watch
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 60.0),
            TiandaoResponseLevel::Tribulation
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Tribulation, 59.99),
            TiandaoResponseLevel::Pressure
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 80.0),
            TiandaoResponseLevel::Annihilate
        );
        assert_eq!(
            response_for_level(TiandaoResponseLevel::Annihilate, 79.99),
            TiandaoResponseLevel::Tribulation
        );
    }

    #[test]
    fn advance_attention_clamps_and_tracks_peak() {
        let mut attention = TiandaoAttention {
            level: 99.9,
            response: TiandaoResponseLevel::Annihilate,
            last_eval_tick: u64::MAX,
            accumulation_rate: 0.0,
            peak_level: 50.0,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Void,
                zone_spirit_qi: 1.0,
                activity: TiandaoActivity::Meditating,
                season: Season::SummerToWinter,
            },
            u64::MAX,
        );
        assert_eq!(attention.level, 100.0);
        assert_eq!(attention.peak_level, 100.0);
        assert_eq!(attention.last_eval_tick, u64::MAX);
        assert_eq!(attention.response, TiandaoResponseLevel::Annihilate);
    }

    #[test]
    fn decay_never_underflows_attention() {
        let mut attention = TiandaoAttention {
            level: 0.01,
            response: TiandaoResponseLevel::None,
            ..TiandaoAttention::default()
        };
        advance_attention(
            &mut attention,
            TiandaoAttentionInput {
                realm: Realm::Awaken,
                zone_spirit_qi: -0.5,
                activity: TiandaoActivity::Standing,
                season: Season::Summer,
            },
            200,
        );
        assert_eq!(attention.level, 0.0);
    }

    #[test]
    fn dead_and_negative_zones_accelerate_decay_without_qi_transfer() {
        assert_eq!(zone_decay_multiplier(0.0), 3.0);
        assert_eq!(zone_decay_multiplier(-0.01), 5.0);
        assert_eq!(zone_decay_multiplier(0.01), 1.0);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, 0.0), 0.15);
        assert_close(decay_rate(TiandaoResponseLevel::Watch, -0.1), 0.25);
    }

    #[test]
    fn void_high_qi_meditation_reaches_watch_in_plan_timeframe() {
        let mut attention = TiandaoAttention::default();
        let input = TiandaoAttentionInput {
            realm: Realm::Void,
            zone_spirit_qi: 0.9,
            activity: TiandaoActivity::Meditating,
            season: Season::Summer,
        };
        for _ in 0..28 {
            advance_attention(&mut attention, input, 200);
        }
        assert_eq!(attention.response, TiandaoResponseLevel::Watch);
        assert!(attention.level >= 15.0);
    }

    #[test]
    fn evaluation_respects_ten_second_interval() {
        let cultivation = Cultivation {
            realm: Realm::Void,
            ..Cultivation::default()
        };
        let position = Position(DVec3::new(0.0, 65.0, 0.0));
        let dimension = CurrentDimension(DimensionKind::Overworld);
        let zones = ZoneRegistry::fallback();
        let mut attention = TiandaoAttention::default();

        apply_attention_eval(
            &cultivation,
            &position,
            Some(&dimension),
            &mut attention,
            Some(&zones),
            Season::Summer,
            199,
        );
        assert_eq!(attention.last_eval_tick, 0);
        assert_eq!(attention.level, 0.0);

        apply_attention_eval(
            &cultivation,
            &position,
            Some(&dimension),
            &mut attention,
            Some(&zones),
            Season::Summer,
            200,
        );
        assert_eq!(attention.last_eval_tick, 200);
        assert!(attention.level > 0.0);
    }

    #[test]
    fn response_profile_matches_four_levels() {
        assert!(tiandao_response_profile(TiandaoResponseLevel::None).is_none());

        let watch = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();
        assert_eq!(watch.audio_recipe_id, "tiandao_watch_ambient");
        assert_eq!(watch.vfx_event_id, None);
        assert_eq!(watch.event_name, None);

        let pressure = tiandao_response_profile(TiandaoResponseLevel::Pressure).unwrap();
        assert_eq!(pressure.audio_recipe_id, "tiandao_pressure_ambient");
        assert_eq!(pressure.vfx_event_id, Some("bong:tiandao_beast_spawn"));
        assert_eq!(pressure.event_name, Some(EVENT_BEAST_TIDE));

        let tribulation = tiandao_response_profile(TiandaoResponseLevel::Tribulation).unwrap();
        assert_eq!(tribulation.audio_recipe_id, "tiandao_tribulation_ambient");
        assert_eq!(
            tribulation.vfx_event_id,
            Some("bong:tiandao_directed_thunder")
        );
        assert_eq!(tribulation.event_name, Some(EVENT_THUNDER_TRIBULATION));

        let annihilate = tiandao_response_profile(TiandaoResponseLevel::Annihilate).unwrap();
        assert_eq!(annihilate.audio_recipe_id, "tiandao_annihilate_ambient");
        assert_eq!(
            annihilate.vfx_event_id,
            Some("bong:realm_collapse_boundary")
        );
        assert_eq!(annihilate.event_name, Some(EVENT_REALM_COLLAPSE));
    }

    #[test]
    fn realm_rank_gates_presence_payload_to_spirit_and_above() {
        assert_eq!(realm_rank(Realm::Awaken), 0);
        assert_eq!(realm_rank(Realm::Induce), 1);
        assert_eq!(realm_rank(Realm::Condense), 2);
        assert_eq!(realm_rank(Realm::Solidify), 3);
        assert_eq!(realm_rank(Realm::Spirit), 4);
        assert_eq!(realm_rank(Realm::Void), 5);
    }

    #[test]
    fn tiandao_spawn_command_uses_target_and_attention_wire_value() {
        let command = tiandao_spawn_event_command(
            "spawn",
            EVENT_THUNDER_TRIBULATION,
            600,
            1.2,
            "player-1",
            TiandaoResponseLevel::Tribulation,
        );
        assert_eq!(command.command_type, CommandType::SpawnEvent);
        assert_eq!(command.target, "spawn");
        assert_eq!(
            command.params.get("event").and_then(|value| value.as_str()),
            Some(EVENT_THUNDER_TRIBULATION)
        );
        assert_eq!(
            command
                .params
                .get("target_player")
                .and_then(|value| value.as_str()),
            Some("player-1")
        );
        assert_eq!(
            command
                .params
                .get("attention_level")
                .and_then(|value| value.as_str()),
            Some("tribulation")
        );
        assert_eq!(
            command
                .params
                .get("reason")
                .and_then(|value| value.as_str()),
            Some("tiandao_hunt_p1")
        );
    }

    #[test]
    fn watch_profile_has_audio_but_no_particle_event() {
        let eval = TiandaoEvalSnapshot {
            position: DVec3::new(1.2, 64.0, -3.4),
            zone_name: Some("spawn".to_string()),
            zone_spirit_qi: 0.6,
            response: TiandaoResponseLevel::Watch,
            level: 20.0,
        };
        let profile = tiandao_response_profile(TiandaoResponseLevel::Watch).unwrap();

        let player = Entity::from_raw(42);
        let audio = tiandao_audio_request(&eval, profile, player);

        assert_eq!(audio.recipe_id, "tiandao_watch_ambient");
        assert_eq!(audio.instance_id, TIANDAO_AUDIO_INSTANCE_BASE + 1);
        assert_eq!(audio.flag.as_deref(), Some("tiandao:watch"));
        assert!(matches!(audio.recipient, AudioRecipient::Single(entity) if entity == player));
        assert!(
            tiandao_vfx_request(&eval, profile).is_none(),
            "Watch 级按 plan 只给 HUD/音效氛围，不产生可见粒子"
        );
    }
}
