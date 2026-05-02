use valence::prelude::{bevy_ecs, Entity, Event, EventReader, EventWriter, Query};

use crate::combat::components::{BodyPart, Wound, WoundKind, Wounds};
use crate::cultivation::components::{ColorKind, ContamSource, Contamination};
use crate::inventory::{InventoryDurabilityChangedEvent, PlayerInventory};
use crate::tools::ToolKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButcherDropKind {
    Bone,
    Meat,
    Hide,
}

impl ButcherDropKind {
    pub fn item_id(self) -> &'static str {
        match self {
            Self::Bone => "yi_shou_gu",
            Self::Meat => "raw_beast_meat",
            Self::Hide => "raw_beast_hide",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherSession {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButcherOutcome {
    pub drop: Option<ButcherDropKind>,
    pub wound: bool,
    pub contamination: bool,
    pub tool_durability_cost: Option<ToolDurabilityCost>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolDurabilityCost {
    pub tool: ToolKind,
    pub cost_basis_points: u16,
}

impl ToolDurabilityCost {
    pub fn cost_ratio(self) -> f64 {
        f64::from(self.cost_basis_points) / 10_000.0
    }
}

#[derive(Debug, Clone, Event)]
pub struct ButcherRequest {
    pub player: Entity,
    pub corpse: Entity,
    pub tool: Option<ToolKind>,
}

pub fn start_butcher_session(
    player: Entity,
    corpse: Entity,
    tool: Option<ToolKind>,
) -> ButcherSession {
    ButcherSession {
        player,
        corpse,
        tool,
    }
}

pub fn resolve_butcher_session(session: &ButcherSession) -> ButcherOutcome {
    match session.tool {
        Some(ToolKind::GuHaiQian) => ButcherOutcome {
            drop: Some(ButcherDropKind::Bone),
            wound: false,
            contamination: false,
            tool_durability_cost: Some(ToolDurabilityCost {
                tool: ToolKind::GuHaiQian,
                cost_basis_points: ToolKind::GuHaiQian.durability_cost_basis_points_per_use(),
            }),
        },
        Some(ToolKind::CaiYaoDao) => ButcherOutcome {
            drop: Some(ButcherDropKind::Meat),
            wound: false,
            contamination: false,
            tool_durability_cost: Some(ToolDurabilityCost {
                tool: ToolKind::CaiYaoDao,
                cost_basis_points: ToolKind::CaiYaoDao.durability_cost_basis_points_per_use(),
            }),
        },
        Some(ToolKind::GuaDao) => ButcherOutcome {
            drop: Some(ButcherDropKind::Hide),
            wound: false,
            contamination: false,
            tool_durability_cost: Some(ToolDurabilityCost {
                tool: ToolKind::GuaDao,
                cost_basis_points: ToolKind::GuaDao.durability_cost_basis_points_per_use(),
            }),
        },
        _ => ButcherOutcome {
            drop: None,
            wound: true,
            contamination: true,
            tool_durability_cost: None,
        },
    }
}

pub fn apply_butcher_tool_durability_cost(
    player: Entity,
    inventory: &mut PlayerInventory,
    outcome: &ButcherOutcome,
    durability_events: &mut EventWriter<InventoryDurabilityChangedEvent>,
) -> Option<crate::tools::ToolDurabilityUseOutcome> {
    let cost = outcome.tool_durability_cost?;
    if crate::tools::main_hand_tool_in_inventory(inventory) != Some(cost.tool) {
        return None;
    }
    crate::tools::damage_main_hand_tool(player, inventory, durability_events, cost.cost_ratio())
}

#[allow(clippy::type_complexity)]
pub fn handle_butcher_requests(
    mut requests: EventReader<ButcherRequest>,
    mut inventories: Query<&mut PlayerInventory>,
    mut hazards: Query<(Option<&mut Wounds>, Option<&mut Contamination>)>,
    mut durability_events: EventWriter<InventoryDurabilityChangedEvent>,
) {
    for request in requests.read() {
        let actual_tool = inventories
            .get(request.player)
            .ok()
            .and_then(crate::tools::main_hand_tool_in_inventory);
        let session = start_butcher_session(request.player, request.corpse, actual_tool);
        let outcome = resolve_butcher_session(&session);

        if let Ok(mut inventory) = inventories.get_mut(request.player) {
            let _ = apply_butcher_tool_durability_cost(
                request.player,
                &mut inventory,
                &outcome,
                &mut durability_events,
            );
        }

        if outcome.wound || outcome.contamination {
            if let Ok((Some(mut wounds), Some(mut contamination))) = hazards.get_mut(request.player)
            {
                apply_bare_hand_butcher_hazard(&mut wounds, &mut contamination, 0);
            }
        }
    }
}

pub fn apply_bare_hand_butcher_hazard(
    wounds: &mut Wounds,
    contamination: &mut Contamination,
    now_tick: u64,
) {
    wounds.entries.push(Wound {
        location: BodyPart::ArmR,
        kind: WoundKind::Cut,
        severity: 0.35,
        bleeding_per_sec: 0.0,
        created_at_tick: now_tick,
        inflicted_by: Some("fauna_butcher_hazard".to_string()),
    });
    contamination.entries.push(ContamSource {
        amount: 0.4,
        color: ColorKind::Turbid,
        attacker_id: Some("fauna_butcher_hazard".to_string()),
        introduced_at: now_tick,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlayerInventory,
        EQUIP_SLOT_MAIN_HAND, MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::HashMap;
    use valence::prelude::{App, Events, Update};

    #[derive(valence::prelude::Resource)]
    struct TestButcherDurabilityState {
        player: Entity,
        inventory: PlayerInventory,
        outcome: ButcherOutcome,
        result: Option<crate::tools::ToolDurabilityUseOutcome>,
    }

    fn inventory_with_main_hand_tool(template_id: &str, durability: f64) -> PlayerInventory {
        PlayerInventory {
            revision: InventoryRevision(0),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "main".to_string(),
                rows: 5,
                cols: 7,
                items: vec![],
            }],
            equipped: HashMap::from([(
                EQUIP_SLOT_MAIN_HAND.to_string(),
                ItemInstance {
                    instance_id: 7_001,
                    template_id: template_id.to_string(),
                    display_name: template_id.to_string(),
                    grid_w: 1,
                    grid_h: 1,
                    weight: 0.1,
                    rarity: ItemRarity::Common,
                    description: String::new(),
                    stack_count: 1,
                    spirit_quality: 0.0,
                    durability,
                    freshness: None,
                    mineral_id: None,
                    charges: None,
                    forge_quality: None,
                    forge_color: None,
                    forge_side_effects: Vec::new(),
                    forge_achieved_tier: None,
                },
            )]),
            hotbar: Default::default(),
            bone_coins: 0,
            max_weight: 50.0,
        }
    }

    fn apply_test_butcher_tool_cost_system(
        mut state: valence::prelude::ResMut<TestButcherDurabilityState>,
        mut events: EventWriter<InventoryDurabilityChangedEvent>,
    ) {
        let player = state.player;
        let outcome = state.outcome;
        let result =
            apply_butcher_tool_durability_cost(player, &mut state.inventory, &outcome, &mut events);
        state.result = result;
    }

    #[test]
    fn bone_pliers_extract_beast_bone() {
        let session = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::GuHaiQian),
        );

        assert_eq!(
            resolve_butcher_session(&session),
            ButcherOutcome {
                drop: Some(ButcherDropKind::Bone),
                wound: false,
                contamination: false,
                tool_durability_cost: Some(ToolDurabilityCost {
                    tool: ToolKind::GuHaiQian,
                    cost_basis_points: 100,
                }),
            }
        );
        assert_eq!(ButcherDropKind::Bone.item_id(), "yi_shou_gu");
    }

    #[test]
    fn cutting_tool_extracts_meat_and_scraper_extracts_hide() {
        let knife = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::CaiYaoDao),
        );
        let scraper = start_butcher_session(
            Entity::from_raw(1),
            Entity::from_raw(2),
            Some(ToolKind::GuaDao),
        );

        assert_eq!(
            resolve_butcher_session(&knife).drop,
            Some(ButcherDropKind::Meat)
        );
        assert_eq!(
            resolve_butcher_session(&scraper).drop,
            Some(ButcherDropKind::Hide)
        );
        assert_eq!(
            resolve_butcher_session(&knife).tool_durability_cost,
            Some(ToolDurabilityCost {
                tool: ToolKind::CaiYaoDao,
                cost_basis_points: 100,
            })
        );
        assert_eq!(
            resolve_butcher_session(&scraper).tool_durability_cost,
            Some(ToolDurabilityCost {
                tool: ToolKind::GuaDao,
                cost_basis_points: 100,
            })
        );
    }

    #[test]
    fn bare_hand_butchery_causes_wound_and_contamination() {
        let session = start_butcher_session(Entity::from_raw(1), Entity::from_raw(2), None);

        assert_eq!(
            resolve_butcher_session(&session),
            ButcherOutcome {
                drop: None,
                wound: true,
                contamination: true,
                tool_durability_cost: None,
            }
        );

        let mut wounds = Wounds::default();
        let mut contamination = Contamination::default();
        apply_bare_hand_butcher_hazard(&mut wounds, &mut contamination, 77);

        assert_eq!(wounds.entries.len(), 1);
        assert_eq!(wounds.entries[0].kind, WoundKind::Cut);
        assert_eq!(contamination.entries.len(), 1);
        assert_eq!(contamination.entries[0].amount, 0.4);
        assert_eq!(contamination.entries[0].introduced_at, 77);
    }

    #[test]
    fn successful_butcher_outcome_ticks_main_hand_tool_durability() {
        let mut app = App::new();
        app.add_event::<InventoryDurabilityChangedEvent>();
        let player = Entity::from_raw(1);
        let session = start_butcher_session(player, Entity::from_raw(2), Some(ToolKind::GuHaiQian));
        let outcome = resolve_butcher_session(&session);
        app.insert_resource(TestButcherDurabilityState {
            player,
            inventory: inventory_with_main_hand_tool("gu_hai_qian", 1.0),
            outcome,
            result: None,
        });
        app.add_systems(
            valence::prelude::Update,
            apply_test_butcher_tool_cost_system,
        );

        app.update();

        let state = app.world().resource::<TestButcherDurabilityState>();
        let result = state
            .result
            .as_ref()
            .expect("successful butcher should damage the tool");

        assert_eq!(result.kind, ToolKind::GuHaiQian);
        assert_eq!(result.instance_id, 7_001);
        assert_eq!(
            state.inventory.equipped[EQUIP_SLOT_MAIN_HAND].durability,
            0.99
        );

        let events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        let sent: Vec<_> = events.iter_current_update_events().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].entity, player);
        assert_eq!(sent[0].instance_id, 7_001);
        assert_eq!(sent[0].durability, 0.99);
    }

    #[test]
    fn bare_hand_butcher_outcome_does_not_tick_tool_durability() {
        let mut app = App::new();
        app.add_event::<InventoryDurabilityChangedEvent>();
        let player = Entity::from_raw(1);
        let session = start_butcher_session(player, Entity::from_raw(2), None);
        let outcome = resolve_butcher_session(&session);
        app.insert_resource(TestButcherDurabilityState {
            player,
            inventory: inventory_with_main_hand_tool("gu_hai_qian", 1.0),
            outcome,
            result: None,
        });
        app.add_systems(
            valence::prelude::Update,
            apply_test_butcher_tool_cost_system,
        );

        app.update();

        let state = app.world().resource::<TestButcherDurabilityState>();
        assert!(state.result.is_none());
        assert_eq!(
            state.inventory.equipped[EQUIP_SLOT_MAIN_HAND].durability,
            1.0
        );
        let events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        assert_eq!(events.iter_current_update_events().count(), 0);
    }

    #[test]
    fn butcher_request_uses_actual_main_hand_tool_and_ticks_durability() {
        let mut app = App::new();
        app.add_event::<ButcherRequest>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, handle_butcher_requests);
        let player = app
            .world_mut()
            .spawn((inventory_with_main_hand_tool("gu_hai_qian", 1.0),))
            .id();
        let corpse = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(ButcherRequest {
            player,
            corpse,
            tool: Some(ToolKind::CaiYaoDao),
        });
        app.update();

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.equipped[EQUIP_SLOT_MAIN_HAND].durability, 0.99);
        let events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        let sent: Vec<_> = events.iter_current_update_events().collect();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].entity, player);
        assert_eq!(sent[0].instance_id, 7_001);
        assert_eq!(sent[0].durability, 0.99);
    }

    #[test]
    fn butcher_request_with_broken_tool_applies_bare_hand_hazard() {
        let mut app = App::new();
        app.add_event::<ButcherRequest>();
        app.add_event::<InventoryDurabilityChangedEvent>();
        app.add_systems(Update, handle_butcher_requests);
        let player = app
            .world_mut()
            .spawn((
                inventory_with_main_hand_tool("gu_hai_qian", 0.0),
                Wounds::default(),
                Contamination::default(),
            ))
            .id();
        let corpse = app.world_mut().spawn_empty().id();

        app.world_mut().send_event(ButcherRequest {
            player,
            corpse,
            tool: Some(ToolKind::GuHaiQian),
        });
        app.update();

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.equipped[EQUIP_SLOT_MAIN_HAND].durability, 0.0);
        assert_eq!(app.world().get::<Wounds>(player).unwrap().entries.len(), 1);
        assert_eq!(
            app.world()
                .get::<Contamination>(player)
                .unwrap()
                .entries
                .len(),
            1
        );
        let events = app
            .world()
            .resource::<Events<InventoryDurabilityChangedEvent>>();
        assert_eq!(events.iter_current_update_events().count(), 0);
    }
}
