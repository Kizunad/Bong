//! plan-poi-novice-v1 — 散修聚居点 scenario stub。

use valence::prelude::{bevy_ecs, Component};

pub const ROGUE_VILLAGE_MIN_NPCS: u8 = 2;
pub const ROGUE_VILLAGE_MAX_NPCS: u8 = 3;

#[derive(Debug, Clone, Component, PartialEq, Eq)]
pub struct PoiRogueVillageSpec {
    pub village_id: String,
    pub min_rogues: u8,
    pub max_rogues: u8,
    pub uses_dead_letter_mailbox: bool,
}

impl PoiRogueVillageSpec {
    pub fn new(village_id: impl Into<String>) -> Self {
        Self {
            village_id: village_id.into(),
            min_rogues: ROGUE_VILLAGE_MIN_NPCS,
            max_rogues: ROGUE_VILLAGE_MAX_NPCS,
            uses_dead_letter_mailbox: true,
        }
    }

    pub fn spawn_count_for_seed(&self, seed: u64) -> u8 {
        if self.min_rogues == self.max_rogues {
            return self.min_rogues;
        }
        let span = self.max_rogues.saturating_sub(self.min_rogues) + 1;
        self.min_rogues + (seed % u64::from(span)) as u8
    }
}

pub fn log_rogue_village_contract() {
    let spec = PoiRogueVillageSpec::new("spawn:rogue_village");
    tracing::debug!(
        "[bong][poi-novice] rogue village contract id={} spawn_range={}..={} seed0={} dead_letter_mailbox={}",
        spec.village_id,
        spec.min_rogues,
        spec.max_rogues,
        spec.spawn_count_for_seed(0),
        spec.uses_dead_letter_mailbox
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rogue_village_uses_two_to_three_rogues_and_dead_letter_mailbox() {
        let spec = PoiRogueVillageSpec::new("spawn:rogue_village");
        assert_eq!(spec.min_rogues, 2);
        assert_eq!(spec.max_rogues, 3);
        assert!(spec.uses_dead_letter_mailbox);
        assert_eq!(spec.spawn_count_for_seed(0), 2);
        assert_eq!(spec.spawn_count_for_seed(1), 3);
    }
}
