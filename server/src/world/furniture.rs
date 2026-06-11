use std::collections::HashMap;

use valence::prelude::BlockState;
use valence::prelude::{bevy_ecs, Resource};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FurnitureKind {
    SimpleBed,
    MeditationMat,
    MoistureBase,
    SpiritStoneRack,
}

impl FurnitureKind {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [
        Self::SimpleBed,
        Self::MeditationMat,
        Self::MoistureBase,
        Self::SpiritStoneRack,
    ];

    #[cfg(test)]
    pub fn template_id(self) -> &'static str {
        match self {
            Self::SimpleBed => "simple_bed",
            Self::MeditationMat => "meditation_mat",
            Self::MoistureBase => "moisture_base",
            Self::SpiritStoneRack => "spirit_stone_rack",
        }
    }

    #[cfg(test)]
    pub fn block_state(self) -> BlockState {
        match self {
            Self::SimpleBed => BlockState::BONG_SIMPLE_BED,
            Self::MeditationMat => BlockState::BONG_MEDITATION_MAT,
            Self::MoistureBase => BlockState::BONG_MOISTURE_BASE,
            Self::SpiritStoneRack => BlockState::BONG_SPIRIT_STONE_RACK,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct FurnitureRegistry {
    by_pos: HashMap<[i32; 3], FurnitureKind>,
}

impl FurnitureRegistry {
    pub fn register(&mut self, pos: [i32; 3], kind: FurnitureKind) -> Option<FurnitureKind> {
        self.by_pos.insert(pos, kind)
    }

    pub fn remove(&mut self, pos: [i32; 3]) -> Option<FurnitureKind> {
        self.by_pos.remove(&pos)
    }

    #[cfg(test)]
    pub fn kind_at(&self, pos: [i32; 3]) -> Option<FurnitureKind> {
        self.by_pos.get(&pos).copied()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.by_pos.len()
    }

    #[allow(dead_code)]
    pub fn rebuild_from_blocks<I>(&mut self, blocks: I)
    where
        I: IntoIterator<Item = ([i32; 3], BlockState)>,
    {
        self.by_pos.clear();
        for (pos, state) in blocks {
            if let Some(kind) = furniture_kind_for_block_state(state) {
                self.register(pos, kind);
            }
        }
    }

    #[allow(dead_code)]
    pub fn kinds_in_range(
        &self,
        center: [i32; 3],
        radius: i32,
    ) -> impl Iterator<Item = ([i32; 3], FurnitureKind)> + '_ {
        self.by_pos.iter().filter_map(move |(pos, kind)| {
            let distance = chebyshev_distance(*pos, center);
            (radius >= 0 && distance <= radius).then_some((*pos, *kind))
        })
    }
}

pub fn furniture_kind_for_template_id(template_id: &str) -> Option<FurnitureKind> {
    match template_id {
        "simple_bed" => Some(FurnitureKind::SimpleBed),
        "meditation_mat" => Some(FurnitureKind::MeditationMat),
        "moisture_base" => Some(FurnitureKind::MoistureBase),
        "spirit_stone_rack" => Some(FurnitureKind::SpiritStoneRack),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn furniture_kind_for_block_state(state: BlockState) -> Option<FurnitureKind> {
    match state {
        BlockState::BONG_SIMPLE_BED => Some(FurnitureKind::SimpleBed),
        BlockState::BONG_MEDITATION_MAT => Some(FurnitureKind::MeditationMat),
        BlockState::BONG_MOISTURE_BASE => Some(FurnitureKind::MoistureBase),
        BlockState::BONG_SPIRIT_STONE_RACK => Some(FurnitureKind::SpiritStoneRack),
        _ => None,
    }
}

fn chebyshev_distance(left: [i32; 3], right: [i32; 3]) -> i32 {
    (left[0] - right[0])
        .abs()
        .max((left[1] - right[1]).abs())
        .max((left[2] - right[2]).abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_id_mapping_pins_all_furniture_variants() {
        for kind in FurnitureKind::ALL {
            assert_eq!(
                furniture_kind_for_template_id(kind.template_id()),
                Some(kind),
                "template_id `{}` must map to {kind:?}",
                kind.template_id()
            );
        }
        assert_eq!(furniture_kind_for_template_id("torch_item"), None);
        assert_eq!(furniture_kind_for_template_id("missing_furniture"), None);
    }

    #[test]
    fn block_state_mapping_pins_all_furniture_variants() {
        for kind in FurnitureKind::ALL {
            assert_eq!(
                furniture_kind_for_block_state(kind.block_state()),
                Some(kind),
                "block state {:?} must map to {kind:?}",
                kind.block_state()
            );
        }
        assert_eq!(furniture_kind_for_block_state(BlockState::DIRT), None);
        assert_eq!(
            furniture_kind_for_block_state(BlockState::BONG_ZHENFA_NODE),
            None
        );
    }

    #[test]
    fn register_query_and_remove_are_consistent() {
        let mut registry = FurnitureRegistry::default();
        let pos = [10, 64, -3];

        assert_eq!(registry.register(pos, FurnitureKind::SimpleBed), None);
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::SimpleBed));
        assert_eq!(
            registry.kinds_in_range([11, 64, -4], 1).collect::<Vec<_>>(),
            vec![(pos, FurnitureKind::SimpleBed)]
        );
        assert_eq!(registry.remove(pos), Some(FurnitureKind::SimpleBed));
        assert_eq!(registry.kind_at(pos), None);
        assert!(registry.kinds_in_range([11, 64, -4], 1).next().is_none());
    }

    #[test]
    fn same_position_register_replaces_then_can_re_register() {
        let mut registry = FurnitureRegistry::default();
        let pos = [0, 65, 0];

        assert_eq!(registry.register(pos, FurnitureKind::SimpleBed), None);
        assert_eq!(
            registry.register(pos, FurnitureKind::MeditationMat),
            Some(FurnitureKind::SimpleBed)
        );
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::MeditationMat));
        assert_eq!(registry.remove(pos), Some(FurnitureKind::MeditationMat));
        assert_eq!(registry.register(pos, FurnitureKind::MoistureBase), None);
        assert_eq!(registry.kind_at(pos), Some(FurnitureKind::MoistureBase));
    }

    #[test]
    fn range_query_uses_chebyshev_boundary() {
        let mut registry = FurnitureRegistry::default();
        registry.register([2, 64, 0], FurnitureKind::SimpleBed);
        registry.register([3, 64, 0], FurnitureKind::MeditationMat);
        registry.register([0, 67, 0], FurnitureKind::MoistureBase);
        registry.register([0, 64, -2], FurnitureKind::SpiritStoneRack);

        let mut hits = registry
            .kinds_in_range([0, 64, 0], 2)
            .map(|(_, kind)| kind)
            .collect::<Vec<_>>();
        hits.sort_by_key(|kind| *kind as u8);

        assert_eq!(
            hits,
            vec![FurnitureKind::SimpleBed, FurnitureKind::SpiritStoneRack],
            "radius=2 should include boundary distance 2 and exclude distance 3"
        );
        assert!(registry.kinds_in_range([0, 64, 0], -1).next().is_none());
    }

    #[test]
    fn rebuild_from_blocks_keeps_only_furniture_states() {
        let mut registry = FurnitureRegistry::default();
        registry.register([99, 1, 99], FurnitureKind::SimpleBed);

        registry.rebuild_from_blocks([
            ([1, 64, 1], BlockState::BONG_SIMPLE_BED),
            ([2, 64, 1], BlockState::DIRT),
            ([3, 64, 1], BlockState::BONG_ZHENFA_NODE),
            ([4, 64, 1], BlockState::BONG_MEDITATION_MAT),
        ]);

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.kind_at([1, 64, 1]), Some(FurnitureKind::SimpleBed));
        assert_eq!(
            registry.kind_at([4, 64, 1]),
            Some(FurnitureKind::MeditationMat)
        );
        assert_eq!(registry.kind_at([99, 1, 99]), None);
    }
}
