use valence::prelude::{
    bevy_ecs, App, Component, Despawned, IntoSystemConfigs, PreUpdate, Query, Res, Resource, With,
    Without,
};

use crate::npc::spawn::NpcMarker;

/// Commoner/Beast 饱食度。`value ∈ [0, 1]`：1 = 饱腹，0 = 饥饿。
/// 每 tick 自然衰减；`FarmAction` 成功时回补。
#[derive(Clone, Copy, Debug, Component)]
pub struct Hunger {
    pub value: f64,
}

impl Default for Hunger {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

impl Hunger {
    #[allow(dead_code)]
    pub const fn new(value: f64) -> Self {
        Self { value }
    }

    pub fn set(&mut self, value: f64) {
        self.value = value.clamp(0.0, 1.0);
    }

    pub fn replenish(&mut self, amount: f64) {
        self.set(self.value + amount.max(0.0));
    }

    pub fn consume(&mut self, amount: f64) {
        self.set(self.value - amount.max(0.0));
    }

    /// Scorer 用：饥饿度 → 0..1。value=1 → 0，value=0 → 1。
    pub fn hunger_pressure(self) -> f32 {
        (1.0 - self.value.clamp(0.0, 1.0)) as f32
    }
}

#[derive(Clone, Copy, Debug, Resource)]
pub struct HungerConfig {
    pub decay_per_tick: f64,
    pub farm_restore_per_tick: f64,
}

impl Default for HungerConfig {
    fn default() -> Self {
        Self {
            decay_per_tick: 1.0 / 4_800.0,
            farm_restore_per_tick: 1.0 / 120.0,
        }
    }
}

pub fn register(app: &mut App) {
    app.insert_resource(HungerConfig::default()).add_systems(
        PreUpdate,
        decay_hunger_system.before(big_brain::prelude::BigBrainSet::Scorers),
    );
}

type HungerQuery<'w, 's> =
    Query<'w, 's, &'static mut Hunger, (With<NpcMarker>, Without<Despawned>)>;

fn decay_hunger_system(config: Res<HungerConfig>, mut npcs: HungerQuery<'_, '_>) {
    if config.decay_per_tick <= 0.0 {
        return;
    }
    for mut hunger in &mut npcs {
        hunger.consume(config.decay_per_tick);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::prelude::{App, PreUpdate};

    #[test]
    fn hunger_clamps_to_unit_range() {
        let mut h = Hunger::default();
        h.consume(2.0);
        assert_eq!(h.value, 0.0);
        h.replenish(0.3);
        assert!((h.value - 0.3).abs() < f64::EPSILON);
        h.replenish(5.0);
        assert_eq!(h.value, 1.0);
    }

    #[test]
    fn hunger_pressure_inverts_value() {
        assert_eq!(Hunger::new(1.0).hunger_pressure(), 0.0);
        assert_eq!(Hunger::new(0.0).hunger_pressure(), 1.0);
        assert!((Hunger::new(0.25).hunger_pressure() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn decay_system_drops_value_each_tick() {
        let mut app = App::new();
        app.insert_resource(HungerConfig {
            decay_per_tick: 0.1,
            farm_restore_per_tick: 0.0,
        });
        app.add_systems(PreUpdate, decay_hunger_system);

        let npc = app.world_mut().spawn((NpcMarker, Hunger::default())).id();
        app.update();
        app.update();

        let hunger = *app.world().get::<Hunger>(npc).unwrap();
        assert!((hunger.value - 0.8).abs() < 1e-6);
    }

    #[test]
    fn decay_system_ignores_entities_without_npc_marker() {
        let mut app = App::new();
        app.insert_resource(HungerConfig {
            decay_per_tick: 0.5,
            farm_restore_per_tick: 0.0,
        });
        app.add_systems(PreUpdate, decay_hunger_system);

        let player = app.world_mut().spawn(Hunger::default()).id();
        app.update();

        let hunger = *app.world().get::<Hunger>(player).unwrap();
        assert_eq!(hunger.value, 1.0);
    }
}
