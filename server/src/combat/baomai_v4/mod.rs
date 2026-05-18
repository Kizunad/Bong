//! plan-baomai-v4 — 爆脉肉搏 · 疤纹深度扩展。
//!
//! 将体修从"按钮式输出"升级为"战斗史塑角色"的被动成长/战术博弈体系。

pub mod adjacency;
pub mod constants;
pub mod events;
pub mod iron_cocoon;
pub mod scar_circuit;
pub mod scar_history;

#[cfg(test)]
mod tests;

use valence::prelude::{App, IntoSystemConfigs, Update};

use crate::combat::CombatSystemSet;

pub fn register(app: &mut App) {
    // P0-P2 events.
    app.add_event::<events::ScarCircuitFormedEvent>();
    app.add_event::<events::ScarCircuitBrokenEvent>();
    app.add_event::<events::IronCocoonStageUpEvent>();

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
        ),
    );
}
