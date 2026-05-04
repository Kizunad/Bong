use serde::{Deserialize, Serialize};
use valence::prelude::{bevy_ecs, App, Entity, Event, EventReader, EventWriter, Update};

use crate::social::events::NicheIntrusionAttempt;

#[derive(Debug, Clone, Event, Serialize, Deserialize)]
pub struct FoodScavengerSellsCoord {
    pub scavenger: Entity,
    pub buyer: Entity,
    pub niche_owner: String,
    pub niche_pos: [i32; 3],
    pub price_bone_coins: u64,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntrusionNpcSpawnPlan {
    pub niche_owner: String,
    pub niche_pos: [i32; 3],
    pub hostile_reason: String,
}

pub fn register(app: &mut App) {
    app.add_event::<FoodScavengerSellsCoord>();
    app.add_systems(Update, food_scavenger_sales_emit_intrusion_attempts);
}

pub fn food_scavenger_sale_to_intrusion_plan(
    sale: &FoodScavengerSellsCoord,
) -> IntrusionNpcSpawnPlan {
    IntrusionNpcSpawnPlan {
        niche_owner: sale.niche_owner.clone(),
        niche_pos: sale.niche_pos,
        hostile_reason: "food_scavenger_sold_niche_coord".to_string(),
    }
}

fn food_scavenger_sales_emit_intrusion_attempts(
    mut sales: EventReader<FoodScavengerSellsCoord>,
    mut attempts: EventWriter<NicheIntrusionAttempt>,
) {
    for sale in sales.read() {
        let plan = food_scavenger_sale_to_intrusion_plan(sale);
        attempts.send(NicheIntrusionAttempt {
            intruder: sale.buyer,
            intruder_char_id: format!("npc:{:?}", sale.buyer),
            niche_owner: plan.niche_owner.clone(),
            niche_pos: plan.niche_pos,
            items_taken: Vec::new(),
            intruder_qi_fraction: 1.0,
            intruder_back_turned: false,
            tick: sale.tick,
        });
        tracing::info!(
            "[bong][npc][niche-defense] {} buyer={:?} target_owner={} pos={:?}",
            plan.hostile_reason,
            sale.buyer,
            sale.niche_owner,
            sale.niche_pos
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn food_scavenger_sale_preserves_niche_target() {
        let sale = FoodScavengerSellsCoord {
            scavenger: Entity::from_raw(1),
            buyer: Entity::from_raw(2),
            niche_owner: "char:owner".to_string(),
            niche_pos: [8, 64, 9],
            price_bone_coins: 15,
            tick: 100,
        };
        let plan = food_scavenger_sale_to_intrusion_plan(&sale);
        assert_eq!(plan.niche_owner, "char:owner");
        assert_eq!(plan.niche_pos, [8, 64, 9]);
    }
}
