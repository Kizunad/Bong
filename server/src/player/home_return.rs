use std::collections::{HashMap, HashSet};

use valence::prelude::{App, Client, Position, Query, Res, ResMut, Resource, Update, With};

use crate::combat::components::{Lifecycle, LifecycleState};
use crate::combat::CombatClock;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;
use crate::social::{position_is_within_own_active_spirit_niche, SpiritNicheRegistry};
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

const TICKS_PER_MINUTE: u64 = 20 * 60;

#[derive(Debug, Clone, Copy, Default)]
struct HomePresence {
    inside_home: bool,
    away_since_tick: Option<u64>,
}

#[derive(Debug, Default)]
struct HomeReturnState {
    players: HashMap<String, HomePresence>,
}

impl Resource for HomeReturnState {}

pub fn register(app: &mut App) {
    app.init_resource::<HomeReturnState>();
    app.add_systems(Update, detect_home_returns);
}

fn detect_home_returns(
    registry: Option<Res<SpiritNicheRegistry>>,
    clock: Option<Res<CombatClock>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    mut state: ResMut<HomeReturnState>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    players: Query<(&Lifecycle, &Position, Option<&CurrentDimension>), With<Client>>,
) {
    let registry = registry.as_deref();
    let tick = clock.as_deref().map(|clock| clock.tick).unwrap_or_default();
    let mut active_character_ids = HashSet::new();
    for (lifecycle, position, dimension) in &players {
        if lifecycle.state == LifecycleState::Terminated || !is_overworld(dimension) {
            continue;
        }
        active_character_ids.insert(lifecycle.character_id.clone());
        let Some(registry) = registry else {
            continue;
        };
        let inside_home = position_is_within_own_active_spirit_niche(
            lifecycle.character_id.as_str(),
            position.get(),
            registry,
        );

        let entry = state
            .players
            .entry(lifecycle.character_id.clone())
            .or_insert_with(|| HomePresence {
                inside_home,
                away_since_tick: (!inside_home).then_some(tick),
            });
        if entry.inside_home == inside_home {
            continue;
        }

        if inside_home {
            let away_ticks = entry
                .away_since_tick
                .map(|start| tick.saturating_sub(start))
                .unwrap_or_default();
            entry.inside_home = true;
            entry.away_since_tick = None;
            if let Some(narrations) = narrations.as_deref_mut() {
                let zone_context = zone_context(zone_registry.as_deref(), position);
                for line in home_return_lines(away_ticks, &zone_context) {
                    narrations.push_player(
                        lifecycle.character_id.as_str(),
                        line,
                        NarrationStyle::Perception,
                    );
                }
            }
        } else {
            entry.inside_home = false;
            entry.away_since_tick = Some(tick);
        }
    }
    state
        .players
        .retain(|character_id, _| active_character_ids.contains(character_id));
}

fn is_overworld(dimension: Option<&CurrentDimension>) -> bool {
    dimension.map(|dimension| dimension.0).unwrap_or_default() == DimensionKind::Overworld
}

fn zone_context(zone_registry: Option<&ZoneRegistry>, position: &Position) -> ZoneContext {
    let Some(zone) = zone_registry
        .and_then(|registry| registry.find_zone(DimensionKind::Overworld, position.get()))
    else {
        return ZoneContext {
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: 0.0,
        };
    };
    ZoneContext {
        zone_name: zone.name.clone(),
        spirit_qi: zone.spirit_qi,
    }
}

fn home_return_lines(away_ticks: u64, zone: &ZoneContext) -> Vec<String> {
    let minutes = away_ticks / TICKS_PER_MINUTE;
    let away_label = if minutes == 0 {
        "短短一阵".to_string()
    } else {
        format!("{minutes} 分钟")
    };
    vec![
        format!(
            "你在外 {away_label}。灵龛冷石把脚步声吞下，{}。",
            qi_change_line(zone)
        ),
        format!(
            "龛内镜面起雾：{}；{}。",
            target_hint(zone, 0),
            target_hint(zone, 1)
        ),
        "散修的闲话只给方向，不替你接任务；下一趟去哪，仍由你自己定。".to_string(),
    ]
}

fn qi_change_line(zone: &ZoneContext) -> &'static str {
    if zone.spirit_qi <= 0.1 {
        "附近的灵气又薄了些，荒地比你离开时更安静"
    } else if zone.spirit_qi >= 0.55 {
        "远处馈赠区还在发亮，抢草的人多半也闻到了"
    } else {
        "风里有新踩出的灰，说明有人刚从附近绕过去"
    }
}

fn target_hint(zone: &ZoneContext, offset: usize) -> &'static str {
    let seed = zone
        .zone_name
        .bytes()
        .fold(offset, |acc, byte| acc.wrapping_add(byte as usize));
    const HINTS: [&str; 4] = [
        "灵泉湿地东边有草疯长，但回来的脚印很少",
        "血谷边上有人埋了东西，味道像赤髓草",
        "青云残峰西侧多了焦土，那里也许刚落过雷",
        "北荒传来低吼，听着不像普通野兽",
    ];
    HINTS[seed % HINTS.len()]
}

#[derive(Debug, Clone)]
struct ZoneContext {
    zone_name: String,
    spirit_qi: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::social::components::SpiritNiche;
    use valence::prelude::DVec3;
    use valence::testing::create_mock_client;

    fn app_with_home_return() -> App {
        let mut app = App::new();
        app.insert_resource(SpiritNicheRegistry::default());
        app.insert_resource(PendingGameplayNarrations::default());
        app.insert_resource(CombatClock { tick: 0 });
        register(&mut app);
        app
    }

    fn add_niche(app: &mut App, owner: &str, pos: [i32; 3]) {
        app.world_mut()
            .resource_mut::<SpiritNicheRegistry>()
            .upsert(SpiritNiche {
                owner: owner.to_string(),
                pos,
                placed_at_tick: 1,
                revealed: false,
                revealed_by: None,
                is_damaged: false,
                guardians: Vec::new(),
            });
    }

    fn spawn_player(
        app: &mut App,
        username: &str,
        character_id: &str,
        pos: [f64; 3],
    ) -> valence::prelude::Entity {
        let (mut bundle, _helper) = create_mock_client(username);
        bundle.player.position = Position::new(DVec3::new(pos[0], pos[1], pos[2]));
        let entity = app.world_mut().spawn(bundle).id();
        app.world_mut().entity_mut(entity).insert((
            Lifecycle {
                character_id: character_id.to_string(),
                ..Default::default()
            },
            CurrentDimension(DimensionKind::Overworld),
        ));
        entity
    }

    fn set_pos(app: &mut App, entity: valence::prelude::Entity, pos: [f64; 3]) {
        *app.world_mut().get_mut::<Position>(entity).unwrap() =
            Position::new(DVec3::new(pos[0], pos[1], pos[2]));
    }

    fn set_tick(app: &mut App, tick: u64) {
        app.world_mut().resource_mut::<CombatClock>().tick = tick;
    }

    fn drain(app: &mut App) -> Vec<crate::schema::narration::Narration> {
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain()
    }

    #[test]
    fn home_return_narration_by_changes() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:azure", [20, 64, 20]);
        let player = spawn_player(&mut app, "Azure", "char:azure", [40.0, 64.0, 40.0]);

        app.update();
        set_tick(&mut app, TICKS_PER_MINUTE * 42);
        set_pos(&mut app, player, [20.5, 64.5, 20.5]);
        app.update();

        let narrations = drain(&mut app);
        assert_eq!(
            3,
            narrations.len(),
            "expected 3 home return narration lines because the loop emits summary, target hints, and agency reminder"
        );
        assert!(
            narrations[0].text.contains("42 分钟"),
            "expected first narration to include away duration because player stayed out 42 minutes, actual {:?}",
            narrations[0].text
        );
        assert!(
            narrations[1].text.contains("龛内镜面"),
            "expected second narration to include mirror target hints, actual {:?}",
            narrations[1].text
        );
        assert!(
            narrations[2].text.contains("下一趟"),
            "expected third narration to preserve next-run agency, actual {:?}",
            narrations[2].text
        );
        assert!(
            narrations
                .iter()
                .all(|line| line.target.as_deref() == Some("char:azure")),
            "expected all narrations to target returning player char:azure, actual {:?}",
            narrations
        );
        assert!(narrations
            .iter()
            .all(|line| line.style == NarrationStyle::Perception),
            "expected all narrations to use Perception style because home return is sensed world state, actual {:?}",
            narrations
        );
    }

    #[test]
    fn home_return_does_not_repeat_while_staying_inside() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:azure", [20, 64, 20]);
        let player = spawn_player(&mut app, "Azure", "char:azure", [40.0, 64.0, 40.0]);

        app.update();
        set_pos(&mut app, player, [20.5, 64.5, 20.5]);
        app.update();
        assert_eq!(
            3,
            drain(&mut app).len(),
            "expected first return to emit 3 narration lines because player crossed away→home"
        );

        app.update();
        assert!(
            drain(&mut app).is_empty(),
            "expected no repeat narration because player remained inside home, actual narration queued"
        );

        set_pos(&mut app, player, [40.0, 64.0, 40.0]);
        app.update();
        set_pos(&mut app, player, [20.5, 64.5, 20.5]);
        app.update();
        assert_eq!(
            3,
            drain(&mut app).len(),
            "expected second return to emit again because player left and re-entered"
        );
    }

    #[test]
    fn home_return_ignores_other_players_niche() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:owner", [20, 64, 20]);
        spawn_player(&mut app, "Azure", "char:azure", [20.5, 64.5, 20.5]);

        app.update();

        assert!(
            drain(&mut app).is_empty(),
            "expected no narration because player entered another owner's spirit niche, actual narration queued"
        );
    }

    #[test]
    fn home_return_ignores_tsy_dimension() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:azure", [20, 64, 20]);
        let player = spawn_player(&mut app, "Azure", "char:azure", [40.0, 64.0, 40.0]);
        app.world_mut()
            .entity_mut(player)
            .insert(CurrentDimension(DimensionKind::Tsy));

        app.update();
        set_pos(&mut app, player, [20.5, 64.5, 20.5]);
        app.update();

        assert!(
            drain(&mut app).is_empty(),
            "expected no narration because home return ignores Tsy dimension, actual narration queued"
        );
    }

    #[test]
    fn home_return_missing_optional_resources_does_not_panic() {
        let mut app = App::new();
        app.insert_resource(HomeReturnState::default());
        app.add_systems(Update, detect_home_returns);
        spawn_player(&mut app, "Azure", "char:azure", [20.5, 64.5, 20.5]);

        app.update();
    }

    #[test]
    fn home_return_zero_minutes_uses_short_away_label() {
        let lines = home_return_lines(
            0,
            &ZoneContext {
                zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
                spirit_qi: 0.3,
            },
        );

        assert!(
            lines[0].contains("短短一阵"),
            "expected zero-minute return to use short away label, actual {:?}",
            lines[0]
        );
    }

    #[test]
    fn home_return_qi_change_lines_cover_thresholds() {
        let dead_zone = ZoneContext {
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: 0.1,
        };
        let middle_zone = ZoneContext {
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: 0.3,
        };
        let bright_zone = ZoneContext {
            zone_name: DEFAULT_SPAWN_ZONE_NAME.to_string(),
            spirit_qi: 0.55,
        };

        assert_eq!(
            "附近的灵气又薄了些，荒地比你离开时更安静",
            qi_change_line(&dead_zone),
            "expected <=0.1 spirit_qi to use dead-zone thinning line"
        );
        assert_eq!(
            "风里有新踩出的灰，说明有人刚从附近绕过去",
            qi_change_line(&middle_zone),
            "expected middle spirit_qi to use trail line"
        );
        assert_eq!(
            "远处馈赠区还在发亮，抢草的人多半也闻到了",
            qi_change_line(&bright_zone),
            "expected >=0.55 spirit_qi to use bright resource competition line"
        );
    }

    #[test]
    fn home_return_state_removes_disconnected_players() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:azure", [20, 64, 20]);
        let player = spawn_player(&mut app, "Azure", "char:azure", [40.0, 64.0, 40.0]);

        app.update();
        assert!(
            app.world()
                .resource::<HomeReturnState>()
                .players
                .contains_key("char:azure"),
            "expected away player to be tracked before disconnect, actual missing state"
        );

        app.world_mut().despawn(player);
        app.update();

        assert!(
            !app.world()
                .resource::<HomeReturnState>()
                .players
                .contains_key("char:azure"),
            "expected disconnected player state to be removed, actual stale state remained"
        );
    }

    #[test]
    fn home_return_state_removes_terminated_players() {
        let mut app = app_with_home_return();
        add_niche(&mut app, "char:azure", [20, 64, 20]);
        let player = spawn_player(&mut app, "Azure", "char:azure", [40.0, 64.0, 40.0]);

        app.update();
        app.world_mut()
            .entity_mut(player)
            .get_mut::<Lifecycle>()
            .unwrap()
            .state = LifecycleState::Terminated;
        app.update();

        assert!(
            !app.world()
                .resource::<HomeReturnState>()
                .players
                .contains_key("char:azure"),
            "expected terminated player state to be removed, actual stale state remained"
        );
    }
}
