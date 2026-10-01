//! 世界分钟时钟，供天气和物品保鲜等低频系统共用。
use valence::prelude::{bevy_ecs, App, ResMut, Resource, Update};

pub const TICKS_PER_MINUTE: u32 = 1200;
pub const MINUTES_PER_DAY: u64 = 1440;

#[derive(Debug, Default, Resource)]
pub struct MinuteClock {
    pub minute: u64,
}

#[derive(Debug, Default, Resource)]
pub struct MinuteAccumulator {
    ticks: u32,
}

impl MinuteAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn step(&mut self) -> bool {
        self.ticks += 1;
        if self.ticks >= TICKS_PER_MINUTE {
            self.ticks = 0;
            true
        } else {
            false
        }
    }

    pub fn raw(&self) -> u32 {
        self.ticks
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<MinuteClock>();
    app.init_resource::<MinuteAccumulator>();
    app.add_systems(Update, advance_world_minute);
}

pub fn advance_world_minute(
    mut accumulator: ResMut<MinuteAccumulator>,
    mut clock: ResMut<MinuteClock>,
) {
    if accumulator.step() {
        clock.minute = clock.minute.saturating_add(1);
    }
}
