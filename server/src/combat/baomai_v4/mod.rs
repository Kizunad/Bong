//! plan-baomai-v4 — 爆脉肉搏 · 疤纹深度扩展。
//!
//! 将体修从"按钮式输出"升级为"战斗史塑角色"的被动成长/战术博弈体系。

pub mod adjacency;
pub mod constants;
pub mod crack_reading;
pub mod dead_armor;
pub mod events;
pub mod iron_cocoon;
pub mod resonance_lock;
pub mod scar_circuit;
pub mod scar_history;

#[cfg(test)]
mod tests;

use valence::prelude::{App, IntoSystemConfigs, Update};

use crate::combat::CombatSystemSet;

pub fn register(app: &mut App) {
    // P0-P4 events.
    app.add_event::<events::ScarCircuitFormedEvent>();
    app.add_event::<events::ScarCircuitBrokenEvent>();
    app.add_event::<events::IronCocoonStageUpEvent>();
    app.add_event::<events::CrackReadingResultEvent>();
    app.add_event::<events::VoluntarySeverEvent>();
    // P3 voluntary_sever_apply_system emits MeridianSeveredEvent. Ensure it's registered
    // (no-op if already registered by cultivation::register in the full app).
    app.add_event::<crate::cultivation::meridian::severed::MeridianSeveredEvent>();
    // P5 resonance lock events.
    app.add_event::<events::ResonanceLockEvent>();
    app.add_event::<events::ResonanceLockEndEvent>();

    // Systems.
    app.add_systems(
        Update,
        (
            // P0: ScarHistory tracking — runs in Physics to catch overload events.
            scar_history::scar_history_track_system.in_set(CombatSystemSet::Physics),
            // P1: Scar circuit 40-tick check — runs in Physics after scar_history.
            scar_circuit::scar_circuit_check_system
                .in_set(CombatSystemSet::Physics)
                .after(scar_history::scar_history_track_system),
            // P1: Scar circuit -> DerivedAttrs — runs in Physics after circuit check.
            scar_circuit::scar_circuit_derive_system
                .in_set(CombatSystemSet::Physics)
                .after(scar_circuit::scar_circuit_check_system),
            // P2: Iron cocoon stage check — runs in Physics after scar_history.
            iron_cocoon::iron_cocoon_check_system
                .in_set(CombatSystemSet::Physics)
                .after(scar_history::scar_history_track_system),
            // P2: Iron cocoon passives -> DerivedAttrs — runs in Physics after cocoon check.
            iron_cocoon::iron_cocoon_passive_system
                .in_set(CombatSystemSet::Physics)
                .after(iron_cocoon::iron_cocoon_check_system),
            // P3: VoluntarySever apply — runs in Resolve (after Physics).
            dead_armor::voluntary_sever_apply_system.in_set(CombatSystemSet::Resolve),
            // P4: Crack reading — runs in Emit (after Resolve) to read final CombatEvent results.
            crack_reading::crack_reading_system.in_set(CombatSystemSet::Emit),
            // P5: Resonance meter — runs in Emit (reads CombatEvent results).
            resonance_lock::resonance_meter_system.in_set(CombatSystemSet::Emit),
            // P5: Resonance lock tick — runs in Physics (per-tick drain + termination check).
            resonance_lock::resonance_lock_tick_system.in_set(CombatSystemSet::Physics),
            // P5: Resonance cascade — runs in Resolve (after MeridianSeveredEvent).
            resonance_lock::resonance_cascade_system.in_set(CombatSystemSet::Resolve),
        ),
    );
}
