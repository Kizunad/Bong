use valence::command::graph::CommandGraphBuilder;
use valence::command::handler::CommandResultEvent;
use valence::command::{AddCommand, Command};
use valence::message::SendMessage;
use valence::prelude::{App, Client, Commands, EventReader, IntoSystemConfigs, Query, Update};

use crate::combat::anticheat::AntiCheatCounter;
use crate::combat::body_mass::{BodyMass, Stance};
use crate::combat::carrier::{CarrierCharging, CarrierStore};
use crate::combat::components::{
    Casting, CombatState, DerivedAttrs, Lifecycle, QuickSlotBindings, SkillBarBindings, Stamina,
    StatusEffects, UnlockedStyles, Wounds,
};
use crate::combat::{anqi_v2, player_attack};
use crate::craft::session::CraftSession;
use crate::cultivation::breakthrough_cinematic::BreakthroughCinematic;
use crate::cultivation::color::PracticeLog;
use crate::cultivation::components::{
    Contamination, Cultivation, Karma, MeridianSystem, QiColor, Realm,
};
use crate::cultivation::dugu::{
    DuguObfuscationDisrupted, DuguPoisonState, DuguPractice, PendingDuguInfusion,
};
use crate::cultivation::full_power_strike::{
    ChargingState, Exhausted, FullPowerChargeRateOverride,
};
use crate::cultivation::insight::InsightQuota;
use crate::cultivation::insight_apply::{InsightModifiers, UnlockedPerceptions};
use crate::cultivation::insight_flow::PendingInsightOffer;
use crate::cultivation::known_techniques::KnownTechniques;
use crate::cultivation::life_record::LifeRecord;
use crate::cultivation::lifespan::{DeathRegistry, LifespanComponent, LifespanExtensionLedger};
use crate::cultivation::meridian::severed::MeridianSeveredPermanent;
use crate::cultivation::meridian_open::MeridianTarget;
use crate::cultivation::poison_trait::components::{DigestionLoad, PoisonToxicity};
use crate::cultivation::tribulation::{
    HeartDemonResolution, JueBiRuntimeContext, PendingHeartDemonOffer, TribulationOriginDimension,
    TribulationState,
};
use crate::inventory::{clear_player_inventory, ClearScope, OverloadedMarker, PlayerInventory};
use crate::movement::{player_knockback::ActivePlayerKnockback, MovementState};
use crate::player::state::PlayerState;
use crate::skill::components::SkillSet;

/// [dev-only] `/reset` 仅用于开发调试。
///
/// 该命令会直接重置玩家运行态，绕过世界观自然修炼流程与 qi_physics 分类账守恒；
/// 不得把这里的状态改写逻辑复用到正式玩法路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetCmd {
    Self_,
}

impl Command for ResetCmd {
    fn assemble_graph(graph: &mut CommandGraphBuilder<Self>) {
        graph
            .root()
            .literal("reset")
            .with_executable(|_| ResetCmd::Self_);
    }
}

/// 注册开发重置命令；生产玩法入口不应依赖这个模块。
pub fn register(app: &mut App) {
    app.add_command::<ResetCmd>();
    register_systems(app);
}

fn register_systems(app: &mut App) {
    app.add_systems(
        Update,
        (
            reset_cultivation_state,
            reset_life_state,
            reset_combat_state,
            reset_progression_state,
            reset_inventory_and_ui_state,
            remove_runtime_state,
            send_reset_feedback,
        )
            .chain(),
    );
}

type CultivationResetItem<'a> = (
    Option<&'a mut Cultivation>,
    Option<&'a mut MeridianSystem>,
    Option<&'a mut QiColor>,
    Option<&'a mut Karma>,
    Option<&'a mut PracticeLog>,
    Option<&'a mut Contamination>,
);

fn reset_cultivation_state(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut players: Query<CultivationResetItem<'_>>,
) {
    for event in events.read() {
        let Ok((cultivation, meridians, qi_color, karma, practice_log, contamination)) =
            players.get_mut(event.executor)
        else {
            continue;
        };

        if let Some(mut cultivation) = cultivation {
            *cultivation = Cultivation::default();
        }
        if let Some(mut meridians) = meridians {
            *meridians = MeridianSystem::default();
        }
        if let Some(mut qi_color) = qi_color {
            *qi_color = QiColor::default();
        }
        if let Some(mut karma) = karma {
            *karma = Karma::default();
        }
        if let Some(mut practice_log) = practice_log {
            *practice_log = PracticeLog::default();
        }
        if let Some(mut contamination) = contamination {
            *contamination = Contamination::default();
        }
    }
}

type LifeResetItem<'a> = (
    Option<&'a mut LifeRecord>,
    Option<&'a mut Lifecycle>,
    Option<&'a mut LifespanComponent>,
    Option<&'a mut DeathRegistry>,
    Option<&'a mut LifespanExtensionLedger>,
);

fn reset_life_state(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut players: Query<LifeResetItem<'_>>,
) {
    for event in events.read() {
        let Ok((life_record, lifecycle, lifespan, death_registry, lifespan_ledger)) =
            players.get_mut(event.executor)
        else {
            continue;
        };

        if let Some(mut life_record) = life_record {
            reset_life_record(&mut life_record);
        }
        if let Some(mut lifecycle) = lifecycle {
            reset_lifecycle(&mut lifecycle);
        }
        if let Some(mut lifespan) = lifespan {
            *lifespan = LifespanComponent::for_realm(Realm::Awaken);
        }
        if let Some(mut death_registry) = death_registry {
            let char_id = death_registry.char_id.clone();
            *death_registry = DeathRegistry::new(char_id);
        }
        if let Some(mut lifespan_ledger) = lifespan_ledger {
            *lifespan_ledger = LifespanExtensionLedger::default();
        }
    }
}

type CombatResetItem<'a> = (
    Option<&'a mut Wounds>,
    Option<&'a mut Stamina>,
    Option<&'a mut CombatState>,
    Option<&'a mut StatusEffects>,
    Option<&'a mut DerivedAttrs>,
    Option<&'a mut BodyMass>,
    Option<&'a mut Stance>,
    Option<&'a mut AntiCheatCounter>,
    Option<&'a mut CarrierStore>,
    Option<&'a mut anqi_v2::ContainerSlot>,
    Option<&'a mut player_attack::PlayerAttackCooldown>,
);

fn reset_combat_state(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut players: Query<CombatResetItem<'_>>,
) {
    for event in events.read() {
        let Ok((
            wounds,
            stamina,
            combat_state,
            status_effects,
            derived_attrs,
            body_mass,
            stance,
            anticheat,
            carrier_store,
            container_slot,
            attack_cooldown,
        )) = players.get_mut(event.executor)
        else {
            continue;
        };

        if let Some(mut wounds) = wounds {
            *wounds = Wounds::default();
        }
        if let Some(mut stamina) = stamina {
            *stamina = Stamina::default();
        }
        if let Some(mut combat_state) = combat_state {
            *combat_state = CombatState::default();
        }
        if let Some(mut status_effects) = status_effects {
            *status_effects = StatusEffects::default();
        }
        if let Some(mut derived_attrs) = derived_attrs {
            *derived_attrs = DerivedAttrs::default();
        }
        if let Some(mut body_mass) = body_mass {
            *body_mass = BodyMass::default();
        }
        if let Some(mut stance) = stance {
            *stance = Stance::default();
        }
        if let Some(mut anticheat) = anticheat {
            *anticheat = AntiCheatCounter::default();
        }
        if let Some(mut carrier_store) = carrier_store {
            *carrier_store = CarrierStore::default();
        }
        if let Some(mut container_slot) = container_slot {
            *container_slot = anqi_v2::ContainerSlot::default();
        }
        if let Some(mut attack_cooldown) = attack_cooldown {
            *attack_cooldown = player_attack::PlayerAttackCooldown::default();
        }
    }
}

type ProgressionResetItem<'a> = (
    Option<&'a mut InsightQuota>,
    Option<&'a mut UnlockedPerceptions>,
    Option<&'a mut InsightModifiers>,
    Option<&'a mut MeridianSeveredPermanent>,
    Option<&'a mut PoisonToxicity>,
    Option<&'a mut DigestionLoad>,
    Option<&'a mut DuguPractice>,
    Option<&'a mut KnownTechniques>,
    Option<&'a mut SkillSet>,
);

fn reset_progression_state(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut players: Query<ProgressionResetItem<'_>>,
) {
    for event in events.read() {
        let Ok((
            insight_quota,
            perceptions,
            insight_modifiers,
            severed,
            poison,
            digestion,
            dugu,
            techniques,
            skill_set,
        )) = players.get_mut(event.executor)
        else {
            continue;
        };

        if let Some(mut insight_quota) = insight_quota {
            *insight_quota = InsightQuota::default();
        }
        if let Some(mut perceptions) = perceptions {
            *perceptions = UnlockedPerceptions::default();
        }
        if let Some(mut insight_modifiers) = insight_modifiers {
            *insight_modifiers = InsightModifiers::new();
        }
        if let Some(mut severed) = severed {
            *severed = MeridianSeveredPermanent::default();
        }
        if let Some(mut poison) = poison {
            *poison = PoisonToxicity::default();
        }
        if let Some(mut digestion) = digestion {
            *digestion = DigestionLoad::for_realm(Realm::Awaken);
        }
        if let Some(mut dugu) = dugu {
            *dugu = DuguPractice::default();
        }
        if let Some(mut techniques) = techniques {
            *techniques = KnownTechniques::default();
        }
        if let Some(mut skill_set) = skill_set {
            *skill_set = SkillSet::default();
        }
    }
}

type InventoryResetItem<'a> = (
    Option<&'a mut PlayerInventory>,
    Option<&'a mut PlayerState>,
    Option<&'a mut MovementState>,
    Option<&'a mut QuickSlotBindings>,
    Option<&'a mut SkillBarBindings>,
    Option<&'a mut UnlockedStyles>,
);

fn reset_inventory_and_ui_state(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut players: Query<InventoryResetItem<'_>>,
) {
    for event in events.read() {
        let Ok((inventory, player_state, movement, quick_slots, skill_bar, styles)) =
            players.get_mut(event.executor)
        else {
            continue;
        };

        if let Some(mut inventory) = inventory {
            clear_player_inventory(&mut inventory, ClearScope::All);
        }
        if let Some(mut player_state) = player_state {
            *player_state = PlayerState::default();
        }
        if let Some(mut movement) = movement {
            *movement = MovementState::default();
        }
        if let Some(mut quick_slots) = quick_slots {
            *quick_slots = QuickSlotBindings::default();
        }
        if let Some(mut skill_bar) = skill_bar {
            *skill_bar = SkillBarBindings::default();
        }
        if let Some(mut styles) = styles {
            *styles = UnlockedStyles::default();
        }
    }
}

fn remove_runtime_state(
    mut commands: Commands,
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
) {
    for event in events.read() {
        commands
            .entity(event.executor)
            .remove::<MeridianTarget>()
            .remove::<Casting>()
            .remove::<PendingInsightOffer>()
            .remove::<BreakthroughCinematic>()
            .remove::<ChargingState>()
            .remove::<FullPowerChargeRateOverride>()
            .remove::<Exhausted>()
            .remove::<TribulationState>()
            .remove::<TribulationOriginDimension>()
            .remove::<JueBiRuntimeContext>()
            .remove::<PendingHeartDemonOffer>()
            .remove::<HeartDemonResolution>()
            .remove::<ActivePlayerKnockback>()
            .remove::<CraftSession>()
            .remove::<OverloadedMarker>()
            .remove::<CarrierCharging>()
            .remove::<PendingDuguInfusion>()
            .remove::<DuguObfuscationDisrupted>()
            .remove::<DuguPoisonState>();
    }
}

fn send_reset_feedback(
    mut events: EventReader<CommandResultEvent<ResetCmd>>,
    mut clients: Query<&mut Client>,
) {
    for event in events.read() {
        let Ok(mut client) = clients.get_mut(event.executor) else {
            continue;
        };
        tracing::warn!(
            "[dev-cmd] reset self: reset online player state while preserving identity/position"
        );
        client.send_chat_message(
            "[dev] reset self: state restored; identity, position and spawn anchor preserved",
        );
    }
}

fn reset_life_record(life_record: &mut LifeRecord) {
    let character_id = life_record.character_id.clone();
    let created_at = life_record.created_at;
    *life_record = LifeRecord::new(character_id);
    life_record.created_at = created_at;
}

fn reset_lifecycle(lifecycle: &mut Lifecycle) {
    let character_id = lifecycle.character_id.clone();
    let spawn_anchor = lifecycle.spawn_anchor;
    *lifecycle = Lifecycle {
        character_id,
        spawn_anchor,
        ..Default::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::dev::test_support::{run_update, spawn_test_client};
    use crate::combat::components::{LifecycleState, SkillSlot, StaminaState};
    use crate::craft::recipe::RecipeId;
    use crate::cultivation::components::{ColorKind, ContamSource, MeridianId};
    use crate::inventory::{
        ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState,
        MAIN_PACK_CONTAINER_ID,
    };
    use std::collections::{HashMap, HashSet};
    use valence::prelude::{DVec3, Events};

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_event::<CommandResultEvent<ResetCmd>>();
        register_systems(&mut app);
        app
    }

    fn send(app: &mut App, player: valence::prelude::Entity) {
        app.world_mut()
            .resource_mut::<Events<CommandResultEvent<ResetCmd>>>()
            .send(CommandResultEvent {
                result: ResetCmd::Self_,
                executor: player,
                modifiers: Default::default(),
            });
    }

    fn item(id: u64, template_id: &str) -> ItemInstance {
        ItemInstance {
            instance_id: id,
            template_id: template_id.to_string(),
            display_name: template_id.to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.1,
            rarity: ItemRarity::Common,
            description: "test item".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 1.0,
            freshness: None,
            mineral_id: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: Vec::new(),
            forge_achieved_tier: None,
            alchemy: None,
            lingering_owner_qi: None,
        }
    }

    fn dirty_inventory() -> PlayerInventory {
        let mut hotbar: [Option<ItemInstance>; 9] = Default::default();
        hotbar[0] = Some(item(2, "hotbar_item"));
        let mut equipped = HashMap::new();
        equipped.insert("weapon".to_string(), item(3, "sword"));

        PlayerInventory {
            revision: InventoryRevision(7),
            containers: vec![ContainerState {
                id: MAIN_PACK_CONTAINER_ID.to_string(),
                name: "主背包".to_string(),
                rows: 2,
                cols: 2,
                items: vec![PlacedItemState {
                    row: 0,
                    col: 0,
                    instance: item(1, "main_item"),
                }],
            }],
            equipped,
            hotbar,
            bone_coins: 0,
            max_weight: 99.0,
        }
    }

    fn spawn_dirty_player(app: &mut App) -> valence::prelude::Entity {
        let player = spawn_test_client(app, "Alice", [0.0, 0.0, 0.0]);

        let mut meridians = MeridianSystem::default();
        meridians.get_mut(MeridianId::Lung).opened = true;

        let mut practice_log = PracticeLog::default();
        practice_log.add(ColorKind::Sharp, 12.0);

        let mut life_record = LifeRecord::new("offline:Alice");
        life_record.created_at = 11;
        life_record.push(
            crate::cultivation::life_record::BiographyEntry::MeridianOpened {
                id: MeridianId::Lung,
                tick: 9,
            },
        );
        life_record.spirit_root_first = Some(MeridianId::Lung);

        let mut quick_slots = QuickSlotBindings::default();
        quick_slots.set(0, Some(2));
        let mut skill_bar = SkillBarBindings::default();
        skill_bar.set(
            0,
            SkillSlot::Skill {
                skill_id: "burst_meridian.beng_quan".to_string(),
            },
        );

        app.world_mut().entity_mut(player).insert((
            Cultivation {
                realm: Realm::Void,
                qi_current: 88.0,
                qi_max: 200.0,
                qi_max_frozen: Some(20.0),
                last_qi_zero_at: Some(3),
                pending_material_bonus: 5.0,
                composure: 0.2,
                composure_recover_rate: 0.4,
            },
            meridians,
            QiColor {
                main: ColorKind::Violent,
                secondary: Some(ColorKind::Sharp),
                is_chaotic: true,
                is_hunyuan: false,
                permanent_lock_mask: HashSet::from([ColorKind::Violent]),
            },
            Karma { weight: 0.7 },
            practice_log,
            Contamination {
                entries: vec![ContamSource {
                    amount: 3.0,
                    color: ColorKind::Turbid,
                    meridian_id: Some(MeridianId::Lung),
                    attacker_id: Some("npc:bad".to_string()),
                    introduced_at: 5,
                }],
            },
            life_record,
            Lifecycle {
                character_id: "offline:Alice".to_string(),
                death_count: 2,
                spawn_anchor: Some([1.0, 2.0, 3.0]),
                state: LifecycleState::Terminated,
                ..Default::default()
            },
            LifespanComponent {
                years_lived: 60.0,
                ..LifespanComponent::for_realm(Realm::Void)
            },
            DeathRegistry {
                char_id: "offline:Alice".to_string(),
                death_count: 4,
                last_death_tick: Some(55),
                prev_death_tick: Some(44),
                last_death_zone: None,
            },
            LifespanExtensionLedger {
                accumulated_years: 30.0,
                enlightenment_used: true,
            },
        ));
        app.world_mut().entity_mut(player).insert((
            Wounds {
                health_current: 1.0,
                ..Default::default()
            },
            Stamina {
                current: 0.0,
                state: StaminaState::Exhausted,
                ..Default::default()
            },
            CombatState {
                in_combat_until_tick: Some(99),
                ..Default::default()
            },
            DerivedAttrs {
                attack_power: 9.0,
                ..Default::default()
            },
            MovementState {
                action: crate::movement::MovementAction::Dashing,
                active_until_tick: 99,
                ..Default::default()
            },
            PlayerState {
                karma: 0.8,
                inventory_score: 0.9,
            },
            dirty_inventory(),
            quick_slots,
            skill_bar,
        ));
        player
    }

    #[test]
    fn reset_restores_core_player_state_but_keeps_identity() {
        let mut app = setup_app();
        let player = spawn_dirty_player(&mut app);

        send(&mut app, player);
        run_update(&mut app);

        let cultivation = app.world().get::<Cultivation>(player).unwrap();
        assert_eq!(cultivation.realm, Realm::Awaken);
        assert_eq!(cultivation.qi_current, 0.0);
        assert_eq!(cultivation.qi_max, 10.0);
        assert_eq!(
            app.world()
                .get::<MeridianSystem>(player)
                .unwrap()
                .opened_count(),
            0
        );
        assert_eq!(
            app.world().get::<QiColor>(player).unwrap().main,
            ColorKind::Mellow
        );
        assert!(app
            .world()
            .get::<QiColor>(player)
            .unwrap()
            .secondary
            .is_none());
        assert!(app
            .world()
            .get::<PracticeLog>(player)
            .unwrap()
            .weights
            .is_empty());
        assert!(app
            .world()
            .get::<Contamination>(player)
            .unwrap()
            .entries
            .is_empty());

        let life_record = app.world().get::<LifeRecord>(player).unwrap();
        assert_eq!(life_record.character_id, "offline:Alice");
        assert_eq!(life_record.created_at, 11);
        assert!(life_record.biography.is_empty());
        assert_eq!(life_record.spirit_root_first, None);

        let lifecycle = app.world().get::<Lifecycle>(player).unwrap();
        assert_eq!(lifecycle.character_id, "offline:Alice");
        assert_eq!(lifecycle.spawn_anchor, Some([1.0, 2.0, 3.0]));
        assert_eq!(lifecycle.death_count, 0);
        assert_eq!(lifecycle.state, LifecycleState::Alive);
        assert_eq!(
            app.world()
                .get::<LifespanComponent>(player)
                .unwrap()
                .cap_by_realm,
            crate::cultivation::lifespan::LifespanCapTable::AWAKEN
        );
        assert_eq!(
            app.world()
                .get::<DeathRegistry>(player)
                .unwrap()
                .death_count,
            0
        );

        assert_eq!(
            app.world().get::<Wounds>(player).unwrap().health_current,
            app.world().get::<Wounds>(player).unwrap().health_max
        );
        assert_eq!(app.world().get::<Stamina>(player).unwrap().current, 100.0);
        assert_eq!(
            app.world()
                .get::<CombatState>(player)
                .unwrap()
                .in_combat_until_tick,
            None
        );
        assert_eq!(
            app.world().get::<MovementState>(player).unwrap().action,
            crate::movement::MovementAction::None
        );
        assert_eq!(app.world().get::<PlayerState>(player).unwrap().karma, 0.0);

        let inventory = app.world().get::<PlayerInventory>(player).unwrap();
        assert_eq!(inventory.revision, InventoryRevision(8));
        assert!(inventory
            .containers
            .iter()
            .all(|container| container.items.is_empty()));
        assert!(inventory.hotbar.iter().all(Option::is_none));
        assert!(inventory.equipped.is_empty());
        assert!(app
            .world()
            .get::<QuickSlotBindings>(player)
            .unwrap()
            .slots
            .iter()
            .all(Option::is_none));
        assert!(app
            .world()
            .get::<SkillBarBindings>(player)
            .unwrap()
            .slots
            .iter()
            .all(|slot| matches!(slot, SkillSlot::Empty)));
    }

    #[test]
    fn reset_uses_safe_modifier_defaults_and_removes_runtime_components() {
        let mut app = setup_app();
        let player = spawn_test_client(&mut app, "Alice", [0.0, 0.0, 0.0]);
        app.world_mut().entity_mut(player).insert((
            InsightModifiers::default(),
            MeridianTarget(MeridianId::Chong),
            ActivePlayerKnockback {
                velocity: DVec3::new(1.0, 0.0, 0.0),
                remaining_ticks: 3,
                recovery_ticks: 2,
                source_entity: None,
            },
            CraftSession {
                recipe_id: RecipeId::from("craft.example.test"),
                started_at_tick: 1,
                remaining_ticks: 10,
                total_ticks: 10,
                owner_player_id: "offline:Alice".to_string(),
                qi_paid: 1.0,
                quantity_total: 1,
                completed_count: 0,
            },
        ));

        send(&mut app, player);
        run_update(&mut app);

        let modifiers = app.world().get::<InsightModifiers>(player).unwrap();
        assert_eq!(modifiers.qi_regen_mul, 1.0);
        assert_eq!(modifiers.vortex_backfire_resist_mul, 1.0);
        assert_eq!(modifiers.vortex_flow_speed_mul, 1.0);
        assert_eq!(modifiers.breakthrough_failure_penalty_mul, 1.0);
        assert_eq!(modifiers.meridian_heal_slowdown_mul, 1.0);
        assert!(app.world().get::<MeridianTarget>(player).is_none());
        assert!(app.world().get::<ActivePlayerKnockback>(player).is_none());
        assert!(app.world().get::<CraftSession>(player).is_none());
    }
}
