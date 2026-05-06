use serde::{Deserialize, Serialize};

use super::world_state::{SeasonStateV1, SeasonV1};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeasonChangedV1 {
    pub v: u8,
    pub from: SeasonV1,
    pub to: SeasonV1,
    pub tick: u64,
    pub state: SeasonStateV1,
}

impl SeasonChangedV1 {
    pub fn new(
        event: crate::world::season::SeasonChangedEvent,
        state: crate::world::season::SeasonState,
    ) -> Self {
        Self {
            v: 1,
            from: event.from.into(),
            to: event.to.into(),
            tick: event.tick,
            state: state.into(),
        }
    }
}
