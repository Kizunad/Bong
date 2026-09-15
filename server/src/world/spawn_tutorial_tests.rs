use super::*;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemTemplate,
    PlacedItemState, MAIN_PACK_CONTAINER_ID,
};
use std::collections::HashMap;
use valence::testing::create_mock_client;

/// plan-scroll-reading-v1 P0 — registry 仅含 `MERIDIAN_PRIMER_TEMPLATE_ID`（挂了
/// `readable_scroll_spec`），供 grant 幂等测试使用。
fn registry_with_meridian_primer() -> ItemRegistry {
    let mut templates = HashMap::new();
    templates.insert(
        MERIDIAN_PRIMER_TEMPLATE_ID.to_string(),
        ItemTemplate {
            id: MERIDIAN_PRIMER_TEMPLATE_ID.to_string(),
            display_name: "《经脉浅述·残卷》".to_string(),
            category: ItemCategory::Scroll,
            placeable: None,
            max_stack_count: 1,
            grid_w: 1,
            grid_h: 2,
            base_weight: 0.05,
            rarity: ItemRarity::Common,
            spirit_quality_initial: 0.3,
            description: "残破的入门讲义，记述经脉通行之序。翻开可读。".to_string(),
            effect: None,
            cast_duration_ms: 1500,
            cooldown_ms: 1500,
            weapon_spec: None,
            forge_station_spec: None,
            blueprint_scroll_spec: None,
            inscription_scroll_spec: None,
            technique_scroll_spec: None,
            readable_scroll_spec: Some(crate::inventory::ReadableScrollSpec {
                title: "《经脉浅述·残卷》".to_string(),
                body_pages: vec!["第一页".to_string(), "第二页".to_string()],
                anim_id: Some("bong:read_scroll".to_string()),
            }),
            recipe_fragment_spec: None,
            container_spec: None,
            shelflife_profile: None,
            shield_spec: None,
            shelflife_track: None,
            wearer_race: crate::body_plan::types::RaceGateOwned::default(),
        },
    );
    ItemRegistry::from_map(templates)
}

/// registry 缺 `MERIDIAN_PRIMER_TEMPLATE_ID`（配置错误场景）——只含无关模板。
fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(1),
        containers: vec![ContainerState {
            quick_access: false,
            id: MAIN_PACK_CONTAINER_ID.to_string(),
            name: "主背包".to_string(),
            rows: 3,
            cols: 3,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: HashMap::new(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

// ── plan-scroll-reading-v1 P0 §8.1 #1：grant_meridian_primer_once ──────────

#[test]
fn inventory_has_template_scans_containers_equipped_and_hotbar() {
    // containers
    let with_container_item = inventory_with_items(vec![(MERIDIAN_PRIMER_TEMPLATE_ID, 1)]);
    assert!(inventory_has_template(
        &with_container_item,
        MERIDIAN_PRIMER_TEMPLATE_ID
    ));

    // empty inventory: not present
    let empty = empty_inventory();
    assert!(!inventory_has_template(&empty, MERIDIAN_PRIMER_TEMPLATE_ID));

    // equipped slot
    let mut equipped_inv = empty_inventory();
    equipped_inv.equipped.insert(
        "main_hand".to_string(),
        crate::inventory::SlotContents {
            worn: vec![test_item(2, MERIDIAN_PRIMER_TEMPLATE_ID)],
            held: None,
        },
    );
    assert!(inventory_has_template(
        &equipped_inv,
        MERIDIAN_PRIMER_TEMPLATE_ID
    ));

    // hotbar slot
    let mut hotbar_inv = empty_inventory();
    hotbar_inv.hotbar[0] = Some(test_item(3, MERIDIAN_PRIMER_TEMPLATE_ID));
    assert!(inventory_has_template(
        &hotbar_inv,
        MERIDIAN_PRIMER_TEMPLATE_ID
    ));
}

#[test]
fn restored_tutorial_state_does_not_increment_started_telemetry() {
    let restored = TutorialState::new(12);
    let mut telemetry = TutorialTelemetry::default();
    let state = tutorial_state_for_join(Some(restored.clone()), 200, &mut telemetry);

    assert_eq!(state, restored);
    assert_eq!(telemetry.started, 0);

    let fresh = tutorial_state_for_join(None, 220, &mut telemetry);
    assert_eq!(fresh.entered_at_tick, 220);
    assert_eq!(telemetry.started, 1);
}

#[test]
fn client_with_handoff_gets_tutorial_state_and_removes_handoff() {
    let mut app = App::new();
    app.init_resource::<TutorialTelemetry>();
    app.add_systems(Update, attach_tutorial_state_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(CultivationBundleTutorialHandoff {
            accepted_bundle: None,
        });

    app.update();

    let state = app
        .world()
        .get::<TutorialState>(entity)
        .expect("a joined client with a tutorial handoff must receive TutorialState");
    assert_eq!(state.entered_at_tick, 0, "no CombatClock means tick 0");
    assert!(
        app.world()
            .get::<CultivationBundleTutorialHandoff>(entity)
            .is_none(),
        "the tutorial handoff must be consumed after attachment"
    );
}

#[test]
fn reconnect_blocked_client_keeps_handoff_pending() {
    let mut app = App::new();
    app.init_resource::<TutorialTelemetry>();
    app.add_systems(Update, attach_tutorial_state_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        CultivationBundleTutorialHandoff {
            accepted_bundle: None,
        },
        crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked,
    ));

    app.update();

    assert!(
        app.world().get::<TutorialState>(entity).is_none(),
        "blocked reconnect targets must not receive TutorialState"
    );
    assert!(
        app.world()
            .get::<CultivationBundleTutorialHandoff>(entity)
            .is_some(),
        "the tutorial handoff must remain pending for the later ReconnectReady frame"
    );
}

#[test]
fn coffin_open_requires_player_proximity() {
    let coffin = DVec3::new(0.0, 69.0, 0.0);

    assert!(coffin_open_in_range(DVec3::new(2.0, 69.0, 2.0), coffin));
    assert!(!coffin_open_in_range(DVec3::new(12.0, 69.0, 0.0), coffin));
}

// ── test helper ──────────────────────────────────────────────

fn test_item(instance_id: u64, template_id: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: template_id.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: ItemRarity::Common,
        description: String::new(),
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

fn inventory_with_items(items: Vec<(&str, u64)>) -> PlayerInventory {
    let mut inv = empty_inventory();
    for (idx, (template_id, instance_id)) in items.iter().enumerate() {
        inv.containers[0].items.push(PlacedItemState {
            row: idx as u8,
            col: 0,
            instance: test_item(*instance_id, template_id),
        });
    }
    inv
}

// ── P2.1: CraftHintShown ─────────────────────────────────────

// ── P2.2: recipe fragment learning flow (client_request plumbing) ──

// ── P2.3: blueprint & recipe asset verification ──

// ── P2.4: first alchemy / forge hint logic ──

// ── F9 跨层修复：send_tutorial_coffin_pos_on_join ──────────────

mod tutorial_coffin_pos_broadcast {
    use super::*;
    use valence::protocol::packets::play::CustomPayloadS2c;
    use valence::testing::{create_mock_client, MockClientHelper};

    fn app_with_system() -> App {
        let mut app = App::new();
        app.add_systems(Update, send_tutorial_coffin_pos_on_join);
        app
    }

    fn spawn_mock_client(app: &mut App, username: &str) -> (Entity, MockClientHelper) {
        let (client_bundle, helper) = create_mock_client(username);
        let entity = app.world_mut().spawn(client_bundle).id();
        (entity, helper)
    }

    fn flush_all_client_packets(app: &mut App) {
        let world = app.world_mut();
        let mut query = world.query::<&mut Client>();
        for mut client in query.iter_mut(world) {
            client
                .flush_packets()
                .expect("mock client packets should flush successfully");
        }
    }

    /// Decodes every `bong:server_data` frame the mock client received into
    /// `ServerDataV1` and keeps only `TutorialCoffinPos` payloads.
    fn collect_tutorial_coffin_pos_payloads(helper: &mut MockClientHelper) -> Vec<[i32; 3]> {
        let mut positions = Vec::new();
        for frame in helper.collect_received().0 {
            let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
                continue;
            };
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                continue;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("typed payload should decode as ServerDataV1");
            if let ServerDataPayloadV1::TutorialCoffinPos { position } = payload.payload {
                positions.push(position);
            }
        }
        positions
    }

    #[test]
    fn sends_authoritative_pos_to_client_joined_after_coffin_marker_exists() {
        let mut app = app_with_system();
        app.world_mut().spawn(TutorialCoffin { pos: [12, 71, -33] });
        let (_entity, mut helper) = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);

        let positions = collect_tutorial_coffin_pos_payloads(&mut helper);
        assert_eq!(
            positions,
            vec![[12, 71, -33]],
            "newly joined client should receive exactly one tutorial_coffin_pos payload \
                 carrying the TutorialCoffin's authoritative pos, not a hardcoded box"
        );
    }

    #[test]
    fn does_not_resend_once_client_is_marked_as_sent() {
        let mut app = app_with_system();
        app.world_mut().spawn(TutorialCoffin { pos: [0, 69, 0] });
        let (entity, mut helper) = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);
        assert_eq!(
            collect_tutorial_coffin_pos_payloads(&mut helper).len(),
            1,
            "first tick after join should send exactly one payload"
        );

        // Second tick: same client, no new join. Must not resend.
        app.update();
        flush_all_client_packets(&mut app);
        assert!(
            collect_tutorial_coffin_pos_payloads(&mut helper).is_empty(),
            "already-marked client must not receive a duplicate tutorial_coffin_pos payload"
        );
        assert!(
            app.world()
                .entity(entity)
                .contains::<TutorialCoffinPosSent>(),
            "client entity should be tagged TutorialCoffinPosSent after the first send"
        );
    }

    #[test]
    fn retries_until_coffin_marker_spawns_late() {
        // Player joins before spawn_tutorial_poi_markers has produced the TutorialCoffin
        // entity (e.g. still waiting on skin_pool readiness at server boot).
        let mut app = app_with_system();
        let (_entity, mut helper) = spawn_mock_client(&mut app, "Azure");

        app.update();
        flush_all_client_packets(&mut app);
        assert!(
            collect_tutorial_coffin_pos_payloads(&mut helper).is_empty(),
            "no coffin marker yet -> no payload should be sent this tick"
        );

        // The POI marker becomes available on a later tick.
        app.world_mut().spawn(TutorialCoffin { pos: [4, 65, 4] });
        app.update();
        flush_all_client_packets(&mut app);

        assert_eq!(
            collect_tutorial_coffin_pos_payloads(&mut helper),
            vec![[4, 65, 4]],
            "once the coffin marker exists, the previously-unsent client must receive it \
                 on the next tick instead of being permanently skipped"
        );
    }

    #[test]
    fn no_clients_and_no_coffin_is_a_noop() {
        // Regression guard: an empty world (no players, no coffin marker yet) must not
        // panic when `coffins.iter().next()` is `None` and `clients` is empty.
        let mut app = app_with_system();
        app.update();
    }
}

// ── plan-scroll-reading-v1 P0 §8.1 #1：grant_meridian_primer_on_join tick-poll ──

mod meridian_primer_join_grant {
    use super::*;
    use valence::testing::create_mock_client;

    fn app_with_system() -> App {
        let mut app = App::new();
        app.insert_resource(registry_with_meridian_primer());
        app.insert_resource(InventoryInstanceIdAllocator::new(300));
        app.add_systems(Update, grant_meridian_primer_on_join);
        app
    }

    fn spawn_client_with_state(
        app: &mut App,
        username: &str,
        state: TutorialState,
        inventory: PlayerInventory,
    ) -> Entity {
        let (client_bundle, _helper) = create_mock_client(username);
        app.world_mut()
            .spawn(client_bundle)
            .insert(state)
            .insert(inventory)
            .id()
    }

    fn scroll_count(app: &App, entity: Entity) -> usize {
        app.world()
            .entity(entity)
            .get::<PlayerInventory>()
            .expect("PlayerInventory must be attached")
            .containers
            .iter()
            .flat_map(|c| c.items.iter())
            .filter(|p| p.instance.template_id == MERIDIAN_PRIMER_TEMPLATE_ID)
            .count()
    }

    fn has_hook(app: &App, entity: Entity) -> bool {
        app.world()
            .entity(entity)
            .get::<TutorialState>()
            .expect("TutorialState must be attached")
            .has(TutorialHook::MeridianPrimerGranted)
    }

    #[test]
    fn grants_to_freshly_joined_client_with_no_prior_hook() {
        let mut app = app_with_system();
        let entity =
            spawn_client_with_state(&mut app, "Azure", TutorialState::new(0), empty_inventory());

        app.update();

        assert_eq!(
            scroll_count(&app, entity),
            1,
            "fresh client should receive exactly one meridian primer scroll after one tick"
        );
        assert!(has_hook(&app, entity));
        assert!(
            app.world()
                .entity(entity)
                .contains::<MeridianPrimerJoinChecked>(),
            "entity should be marked checked after the system processes it"
        );
    }

    #[test]
    fn does_not_regrant_on_reconnect_simulation_second_tick() {
        let mut app = app_with_system();
        let entity =
            spawn_client_with_state(&mut app, "Azure", TutorialState::new(0), empty_inventory());

        app.update();
        assert_eq!(
            scroll_count(&app, entity),
            1,
            "first tick should grant once"
        );

        // Second tick simulates the same connected client being polled again
        // (e.g. system runs every tick, not just on join).
        app.update();
        assert_eq!(
            scroll_count(&app, entity),
            1,
            "second tick (reconnect-equivalent poll) must not add a duplicate copy"
        );
    }

    #[test]
    fn backfills_old_player_whose_restored_state_predates_this_hook() {
        // Simulates a pre-existing player: TutorialState restored from persistence with
        // unrelated hooks already triggered, but never MeridianPrimerGranted (this hook
        // didn't exist yet when they last played).
        let mut restored_state = TutorialState::new(12_000);
        restored_state.trigger(TutorialHook::CoffinOpened);
        restored_state.trigger(TutorialHook::RealmAdvancedToInduce);
        let mut app = app_with_system();
        let entity =
            spawn_client_with_state(&mut app, "Veteran", restored_state, empty_inventory());

        app.update();

        assert_eq!(
            scroll_count(&app, entity),
            1,
            "old player with restored state must be backfilled exactly once"
        );
        assert!(has_hook(&app, entity));
    }

    #[test]
    fn crash_window_item_present_hook_missing_does_not_duplicate_and_converges() {
        let mut inventory_with_survivor = empty_inventory();
        inventory_with_survivor.containers[0]
            .items
            .push(PlacedItemState {
                row: 0,
                col: 0,
                instance: test_item(999, MERIDIAN_PRIMER_TEMPLATE_ID),
            });
        let mut app = app_with_system();
        let entity = spawn_client_with_state(
            &mut app,
            "Azure",
            TutorialState::new(0), // hook NOT set -- simulates the crash window
            inventory_with_survivor,
        );

        app.update();

        assert_eq!(
            scroll_count(&app, entity),
            1,
            "crash-window survivor instance must not be duplicated"
        );
        assert!(
            has_hook(&app, entity),
            "hook must converge to true even though it started false"
        );
    }

    #[test]
    fn client_without_tutorial_state_yet_is_skipped_until_attached() {
        // Simulates the ordering hazard this system is designed to tolerate: a client
        // joins before `attach_tutorial_state_to_joined_clients` (or the inventory attach
        // system) has run this tick. Because the query requires both components, this
        // entity simply won't match yet -- no panic, no premature grant.
        let mut app = app_with_system();
        let (client_bundle, _helper) = create_mock_client("Azure");
        let entity = app.world_mut().spawn(client_bundle).id();

        app.update();
        assert!(
            !app.world()
                .entity(entity)
                .contains::<MeridianPrimerJoinChecked>(),
            "entity without TutorialState/PlayerInventory must not be processed yet"
        );

        // The attach systems (simulated here by directly inserting the components)
        // catch up on a later tick.
        app.world_mut()
            .entity_mut(entity)
            .insert(TutorialState::new(0))
            .insert(empty_inventory());
        app.update();

        assert_eq!(
            scroll_count(&app, entity),
            1,
            "once TutorialState/PlayerInventory attach, the next tick must grant \
                 (mirrors send_tutorial_coffin_pos_on_join's late-marker-arrival tolerance)"
        );
    }

    #[test]
    fn no_clients_is_a_noop() {
        let mut app = app_with_system();
        app.update();
    }
}
