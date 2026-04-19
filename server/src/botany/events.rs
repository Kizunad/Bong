use valence::prelude::{Commands, EventReader, Position, Query, Res, Resource};

use crate::combat::events::DeathEvent;
use crate::world::zone::ZoneRegistry;

use super::components::{BotanyVariantRoll, Plant, PlantLifecycleClock};
use super::lifecycle::roll_variant_for_zone;
use super::registry::{BotanyKindRegistry, BotanyPlantId, BotanySpawnMode};

/// plan §1.2.3 异变兽死亡触发 kong_shou_hen 生成的概率（`chance_inverse = 5` ⇒ 1/5 = 20%）。
/// 测试可覆盖为 1（100% 必触发）或 0（从不触发）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BotanyEventSpawnRoll {
    pub chance_inverse: u32,
}

impl Default for BotanyEventSpawnRoll {
    fn default() -> Self {
        Self { chance_inverse: 5 }
    }
}

impl Resource for BotanyEventSpawnRoll {}

/// 订阅 `DeathEvent`：NPC / 玩家死亡时，按概率在所在 zone 生成一株 kong_shou_hen（空兽痕）。
/// plan §1.2.3 特殊路径：不扣 zone.spirit_qi（植物本就生于灵气极薄之地）。
pub fn spawn_event_triggered_plants_on_death(
    mut commands: Commands,
    mut death_events: EventReader<DeathEvent>,
    zone_registry: Option<Res<ZoneRegistry>>,
    clock: Res<PlantLifecycleClock>,
    registry: Res<BotanyKindRegistry>,
    roll: Res<BotanyEventSpawnRoll>,
    variant_roll: Res<BotanyVariantRoll>,
    positions: Query<&Position>,
) {
    let Some(zones) = zone_registry.as_deref() else {
        return;
    };
    let Some(kind) = registry.get(BotanyPlantId::KongShouHen) else {
        return;
    };
    if kind.spawn_mode != BotanySpawnMode::EventTriggered {
        return;
    }
    if roll.chance_inverse == 0 {
        // 吞事件避免二次触发
        death_events.read().for_each(drop);
        return;
    }

    let now = clock.tick;
    for ev in death_events.read() {
        let Ok(position) = positions.get(ev.target) else {
            continue;
        };
        let Some(zone) = zones.find_zone(position.get()) else {
            continue;
        };
        let seed = event_spawn_seed(now, ev.target.to_bits());
        if !event_should_spawn(seed, roll.chance_inverse) {
            continue;
        }

        let pos = position.get();
        let variant = roll_variant_for_zone(zone, seed, variant_roll.as_ref());
        commands.spawn(Plant {
            id: BotanyPlantId::KongShouHen,
            zone_name: zone.name.clone(),
            position: [pos.x, pos.y, pos.z],
            planted_at_tick: now,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant,
        });
    }
}

fn event_spawn_seed(now_tick: u64, target_bits: u64) -> u64 {
    now_tick
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ target_bits.wrapping_mul(0xBF58_476D_1CE4_E5B9)
}

fn event_should_spawn(seed: u64, chance_inverse: u32) -> bool {
    if chance_inverse == 0 {
        return false;
    }
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z % u64::from(chance_inverse) == 0
}

#[cfg(test)]
mod tests {
    use valence::prelude::{App, Events, Position, Update};

    use super::*;
    use crate::botany::components::PlantLifecycleClock;
    use crate::world::zone::{Zone, ZoneRegistry};

    fn make_app() -> App {
        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantLifecycleClock::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(BotanyEventSpawnRoll { chance_inverse: 1 }); // 100%
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "spawn".to_string(),
                bounds: (
                    Position::new([-10.0, 0.0, -10.0]).get(),
                    Position::new([10.0, 128.0, 10.0]).get(),
                ),
                spirit_qi: 0.5,
                danger_level: 1,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });
        app.add_event::<DeathEvent>();
        app.add_systems(Update, spawn_event_triggered_plants_on_death);
        app
    }

    #[test]
    fn death_event_spawns_kong_shou_hen_when_roll_hits() {
        let mut app = make_app();
        let victim = app
            .world_mut()
            .spawn(Position::new([0.0, 64.0, 0.0]))
            .id();

        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim,
                cause: "test".to_string(),
                at_tick: 1,
            });

        app.update();

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        let kongs: Vec<_> = plants
            .iter(world)
            .filter(|p| p.id == BotanyPlantId::KongShouHen)
            .collect();
        assert_eq!(
            kongs.len(),
            1,
            "death with chance_inverse=1 should spawn one kong_shou_hen"
        );
        assert_eq!(kongs[0].zone_name, "spawn");
    }

    #[test]
    fn death_event_outside_zones_does_not_spawn() {
        let mut app = make_app();
        let victim = app
            .world_mut()
            .spawn(Position::new([9999.0, 64.0, 9999.0]))
            .id();

        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim,
                cause: "test".to_string(),
                at_tick: 1,
            });

        app.update();

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        assert_eq!(plants.iter(world).count(), 0);
    }

    #[test]
    fn zero_chance_never_spawns() {
        let mut app = make_app();
        app.insert_resource(BotanyEventSpawnRoll { chance_inverse: 0 });
        let victim = app
            .world_mut()
            .spawn(Position::new([0.0, 64.0, 0.0]))
            .id();

        app.world_mut()
            .resource_mut::<Events<DeathEvent>>()
            .send(DeathEvent {
                target: victim,
                cause: "test".to_string(),
                at_tick: 1,
            });

        app.update();

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        assert_eq!(plants.iter(world).count(), 0);
    }

    #[test]
    fn kong_shou_hen_survives_lifecycle_tick_in_any_zone() {
        // 放到没有 Marsh/Plains 标签的 zone，确认 event-kind 豁免 unsupported 检查
        use crate::botany::components::{PlantStaticPointStore, PlantLifecycleClock};
        use crate::botany::lifecycle::run_botany_lifecycle_tick;

        let mut app = App::new();
        app.insert_resource(BotanyKindRegistry::default());
        app.insert_resource(PlantStaticPointStore::default());
        app.insert_resource(BotanyVariantRoll::default());
        app.insert_resource(PlantLifecycleClock { tick: 99 }); // 下 tick 触发 interval
        app.insert_resource(ZoneRegistry {
            zones: vec![Zone {
                name: "blood_valley".to_string(),
                bounds: (
                    Position::new([0.0, 0.0, 0.0]).get(),
                    Position::new([1.0, 1.0, 1.0]).get(),
                ),
                spirit_qi: -0.5, // 极端低灵气，普通植物必 wither
                danger_level: 9,
                active_events: vec![],
                patrol_anchors: vec![],
                blocked_tiles: vec![],
            }],
        });

        app.world_mut().spawn(Plant {
            id: BotanyPlantId::KongShouHen,
            zone_name: "blood_valley".to_string(),
            position: [0.5, 1.0, 0.5],
            planted_at_tick: 0,
            wither_progress: 0,
            source_point: None,
            harvested: false,
            trampled: false,
            variant: crate::botany::registry::PlantVariant::None,
        });

        app.add_systems(Update, run_botany_lifecycle_tick);
        app.update();

        let world = app.world_mut();
        let mut plants = world.query::<&Plant>();
        assert_eq!(
            plants.iter(world).count(),
            1,
            "EventTriggered plant should survive regardless of biome / spirit_qi"
        );
    }
}
