use crate::schema::realm_vision::{SenseEntryV1, SenseKindV1};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpiritualSenseEventKind {
    Breakthrough,
    Tribulation,
    EraDecree,
    VoidPresence,
}

pub fn event_to_sense_entry(
    kind: SpiritualSenseEventKind,
    position: [f64; 3],
    intensity: f64,
) -> SenseEntryV1 {
    let sense_kind = match kind {
        SpiritualSenseEventKind::Breakthrough => SenseKindV1::LivingQi,
        SpiritualSenseEventKind::Tribulation => SenseKindV1::CrisisPremonition,
        SpiritualSenseEventKind::EraDecree => SenseKindV1::HeavenlyGaze,
        SpiritualSenseEventKind::VoidPresence => SenseKindV1::CultivatorRealm,
    };
    SenseEntryV1 {
        kind: sense_kind,
        x: position[0],
        y: position[1],
        z: position[2],
        intensity: intensity.clamp(0.0, 1.0),
    }
}
