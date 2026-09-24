use super::*;

fn qi_test_app() -> App {
    let mut app = App::new();
    app.insert_resource(WorldQiAccount::default());
    app
}

use crate::combat::components::Lifecycle;
use crate::combat::CombatClock;
use crate::identity::events::IdentityReactionChangedEvent;
use crate::identity::reaction::{IdentityReactionState, ReactionTier};
use crate::identity::wanted_player_emit::build_wanted_player_event;
use crate::identity::{IdentityId, IdentityProfile, RevealedTag, RevealedTagKind};
use crate::inventory::{
    ContainerState, InventoryRevision, ItemInstance, ItemRarity, PlacedItemState, SlotContents,
    EQUIP_SLOT_MAIN_HAND,
};
use crate::network::identity_panel_emit::build_identity_panel_state;
use crate::persistence::{bootstrap_sqlite, identity as identity_db};
use crate::schema::server_data::ServerDataType;
use crate::schema::social::RenownTagV1;
use crate::social::events::PlayerChatCollected;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use valence::prelude::{App, Events, Position, Update};
use valence::protocol::packets::play::{CustomPayloadS2c, GameMessageS2c};
use valence::testing::{create_mock_client, MockClientHelper};

fn item_instance(instance_id: u64, template_id: &str, display_name: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: template_id.to_string(),
        display_name: display_name.to_string(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.4,
        rarity: ItemRarity::Rare,
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

fn spirit_niche_test_item(instance_id: u64) -> ItemInstance {
    item_instance(instance_id, SPIRIT_NICHE_ITEM_TEMPLATE_ID, "灵龛基座")
}

fn niche_repair_kit_test_item(instance_id: u64) -> ItemInstance {
    item_instance(
        instance_id,
        SPIRIT_NICHE_REPAIR_ITEM_TEMPLATE_ID,
        "灵龛修补料",
    )
}

fn spirit_niche_stone_test_item(instance_id: u64) -> ItemInstance {
    item_instance(instance_id, "spirit_niche_stone", "龛石")
}

fn wood_plank_test_item(instance_id: u64) -> ItemInstance {
    item_instance(instance_id, "wood_plank", "木板")
}

fn trade_test_item(instance_id: u64, name: &str) -> ItemInstance {
    ItemInstance {
        instance_id,
        template_id: format!("trade_item_{instance_id}"),
        display_name: name.to_string(),
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

fn inventory_with_item(item: ItemInstance) -> PlayerInventory {
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: vec![PlacedItemState {
                row: 0,
                col: 0,
                instance: item,
            }],

            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn empty_trade_inventory() -> PlayerInventory {
    PlayerInventory {
        material_preparation: Default::default(),
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "主背包".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 45.0,
    }
}

fn trade_inventory(instance_id: u64, name: &str) -> PlayerInventory {
    inventory_with_item(trade_test_item(instance_id, name))
}

fn move_first_trade_item_to_main_hand(inventory: &mut PlayerInventory) {
    let item = inventory.containers[0].items.remove(0).instance;
    inventory.equipped.insert(
        EQUIP_SLOT_MAIN_HAND.to_string(),
        SlotContents::held_single(item),
    );
}

fn spawn_trade_player_with_helper(
    app: &mut App,
    name: &str,
    character_id: &str,
    x: f64,
) -> (Entity, MockClientHelper) {
    let (mut bundle, helper) = create_mock_client(name);
    bundle.player.position = Position::new([x, 64.0, 0.0]);
    let entity = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: character_id.to_string(),
            ..Default::default()
        },
        trade_inventory(if x == 0.0 { 1001 } else { 2002 }, name),
        PlayerState::default(),
        Cultivation::default(),
        LifeRecord::new(character_id),
    ));
    (entity, helper)
}

fn spawn_trade_player(app: &mut App, name: &str, character_id: &str, x: f64) -> Entity {
    spawn_trade_player_with_helper(app, name, character_id, x).0
}

fn setup_trade_app() -> App {
    let mut app = qi_test_app();
    app.init_resource::<TradeOfferRegistry>();
    app.add_event::<TradeOfferRequest>();
    app.add_event::<TradeOfferResponseEvent>();
    app.add_event::<SocialExposureEvent>();
    app.add_systems(
        Update,
        (
            dispatch_trade_offers,
            handle_trade_offer_responses.after(dispatch_trade_offers),
            apply_social_exposures.after(handle_trade_offer_responses),
            crate::network::inventory_snapshot_emit::emit_changed_inventory_snapshots
                .after(handle_trade_offer_responses),
        ),
    );
    app
}

fn unique_temp_dir(test_name: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "bong-social-{test_name}-{}-{unique_suffix}",
        std::process::id()
    ))
}

fn social_persistence(test_name: &str) -> (PersistenceSettings, PathBuf) {
    let data_dir = unique_temp_dir(test_name);
    let db_path = data_dir.join("bong.db");
    bootstrap_sqlite(&db_path, &format!("social-{test_name}"))
        .expect("sqlite bootstrap should succeed");
    (
        PersistenceSettings::with_db_path(db_path, format!("social-{test_name}")),
        data_dir,
    )
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

fn collect_server_data_payloads(helper: &mut MockClientHelper) -> Vec<ServerDataV1> {
    let mut payloads = Vec::new();
    for frame in helper.collect_received().0 {
        let Ok(packet) = frame.decode::<CustomPayloadS2c>() else {
            continue;
        };
        if packet.channel.as_str() != "bong:server_data" {
            continue;
        }
        payloads.push(
            serde_json::from_slice(packet.data.0 .0).expect("server_data payload should decode"),
        );
    }
    payloads
}

fn collect_chat_messages(helper: &mut MockClientHelper) -> Vec<String> {
    helper
        .collect_received()
        .0
        .into_iter()
        .filter_map(|frame| {
            frame
                .decode::<GameMessageS2c>()
                .ok()
                .map(|packet| packet.chat.to_legacy_lossy())
        })
        .collect()
}

fn spawn_social_payload_client(
    app: &mut App,
    name: &str,
    character_id: &str,
    x: f64,
) -> (Entity, MockClientHelper) {
    let (mut bundle, helper) = create_mock_client(name);
    bundle.player.position = Position::new([x, 64.0, 0.0]);
    let entity = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: character_id.to_string(),
            ..Default::default()
        },
        Anonymity::default(),
        Renown::default(),
        ExposureLog::default(),
    ));
    (entity, helper)
}

#[test]
fn joined_client_gets_default_social_bundle() {
    let mut app = qi_test_app();
    app.add_systems(Update, attach_social_bundle_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();

    app.update();

    let entity_ref = app.world().entity(entity);
    assert!(entity_ref.contains::<Anonymity>());
    assert!(entity_ref.contains::<Renown>());
    assert!(entity_ref.contains::<FactionReputation>());
    assert!(entity_ref.contains::<Relationships>());
    assert!(entity_ref.contains::<ExposureLog>());
}

#[test]
fn reconnect_blocked_client_is_excluded_from_social_bundle() {
    let mut app = qi_test_app();
    app.add_systems(Update, attach_social_bundle_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked);

    app.update();

    let entity_ref = app.world().entity(entity);
    assert!(
        !entity_ref.contains::<Anonymity>(),
        "blocked reconnect targets must not receive the social bundle on the Added<Client> edge"
    );
    assert!(!entity_ref.contains::<Renown>());
    assert!(!entity_ref.contains::<FactionReputation>());
    assert!(!entity_ref.contains::<Relationships>());
    assert!(!entity_ref.contains::<ExposureLog>());
}

#[test]
fn reconnect_ready_client_gets_default_social_bundle() {
    let mut app = qi_test_app();
    app.add_systems(Update, attach_social_bundle_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut()
        .entity_mut(entity)
        .insert(crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked);
    app.update();
    assert!(
        !app.world().entity(entity).contains::<Anonymity>(),
        "the blocked frame must not attach the social bundle"
    );

    app.world_mut()
        .entity_mut(entity)
        .remove::<crate::cultivation::known_techniques::KnownTechniquesReconnectBlocked>()
        .insert(crate::cultivation::known_techniques::KnownTechniquesReconnectReady);
    app.update();

    let entity_ref = app.world().entity(entity);
    assert!(
        entity_ref.contains::<Anonymity>(),
        "the ReconnectReady edge alone must attach the social bundle after the Added<Client> edge is consumed"
    );
    assert!(entity_ref.contains::<Renown>());
    assert!(entity_ref.contains::<FactionReputation>());
    assert!(entity_ref.contains::<Relationships>());
    assert!(entity_ref.contains::<ExposureLog>());
}

#[test]
fn joined_client_hydrates_persisted_faction_reputation() {
    let (persistence, data_dir) = social_persistence("faction-reputation-hydrate");
    let mut persisted = FactionReputation::default();
    persisted.apply_delta(crate::npc::faction::NamedFactionId::QingyunHunters, 42);
    persist_social_faction_reputation(&persistence, "char:azure", &persisted)
        .expect("faction reputation should persist before join");

    let mut app = qi_test_app();
    app.insert_resource(persistence);
    app.add_systems(Update, attach_social_bundle_to_joined_clients);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert(Lifecycle {
        character_id: "char:azure".to_string(),
        ..Default::default()
    });

    app.update();

    let reputation = app
        .world()
        .get::<FactionReputation>(entity)
        .expect("joined client should receive FactionReputation component");
    let score = reputation.score(crate::npc::faction::NamedFactionId::QingyunHunters);
    assert_eq!(
        score, 42,
        "expected persisted QingyunHunters reputation to hydrate on join, actual {score}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn chat_exposure_records_nearby_witness_only_after_collected_chat() {
    let mut app = qi_test_app();
    app.add_event::<PlayerChatCollected>();
    app.add_event::<SocialExposureEvent>();
    app.add_systems(Update, expose_chat_speakers);
    let (mut alice_bundle, _alice_helper) = create_mock_client("Alice");
    alice_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let alice = app.world_mut().spawn(alice_bundle).id();
    let (mut bob_bundle, _bob_helper) = create_mock_client("Bob");
    bob_bundle.player.position = Position::new([30.0, 64.0, 0.0]);
    let bob = app.world_mut().spawn(bob_bundle).id();
    let (mut far_bundle, _far_helper) = create_mock_client("Far");
    far_bundle.player.position = Position::new([80.0, 64.0, 0.0]);
    let far = app.world_mut().spawn(far_bundle).id();
    app.world_mut().entity_mut(alice).insert(Lifecycle {
        character_id: "char:alice".to_string(),
        ..Default::default()
    });
    app.world_mut().entity_mut(bob).insert(Lifecycle {
        character_id: "char:bob".to_string(),
        ..Default::default()
    });
    app.world_mut().entity_mut(far).insert(Lifecycle {
        character_id: "char:far".to_string(),
        ..Default::default()
    });
    app.world_mut().send_event(PlayerChatCollected {
        entity: alice,
        username: "Alice".to_string(),
        char_id: "char:alice".to_string(),
        zone: "spawn".to_string(),
        raw: "我在此处".to_string(),
        timestamp: 99,
    });

    app.update();

    let events = app.world().resource::<Events<SocialExposureEvent>>();
    let mut reader = events.get_reader();
    let collected = reader.read(events).cloned().collect::<Vec<_>>();
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].actor, "char:alice");
    assert_eq!(collected[0].witnesses, vec!["char:bob"]);
}

#[test]
fn social_events_persist_and_reload_by_character_id() {
    let (persistence, data_dir) = social_persistence("event-roundtrip");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(
        Update,
        (
            apply_social_exposures,
            apply_social_relationships,
            apply_social_renown_deltas,
        ),
    );

    let (client_bundle, _helper) = create_mock_client("Azure");
    let azure = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(azure).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        Anonymity::default(),
        Renown::default(),
        Relationships::default(),
        ExposureLog::default(),
    ));

    app.world_mut().send_event(SocialExposureEvent {
        actor: "char:azure".to_string(),
        kind: ExposureKindV1::Chat,
        witnesses: vec!["char:bob".to_string()],
        tick: 42,
        zone: Some("spawn".to_string()),
    });
    app.world_mut().send_event(SocialRelationshipEvent {
        left: "char:azure".to_string(),
        right: "char:rival".to_string(),
        left_kind: RelationshipKindV1::Feud,
        right_kind: RelationshipKindV1::Feud,
        tick: 43,
        metadata: serde_json::json!({ "place": "spawn" }),
    });
    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:azure".to_string(),
        identity_id: None,
        fame_delta: 3,
        notoriety_delta: 5,
        tags_added: vec![RenownTagV1 {
            tag: "戮道者".to_string(),
            weight: 9.0,
            last_seen_tick: 44,
            permanent: false,
        }],
        tick: 44,
        reason: "test".to_string(),
    });

    app.update();

    let loaded = load_social_components(&persistence, "char:azure")
        .expect("persisted social components should reload");

    assert!(loaded.anonymity.is_exposed_to("char:bob"));
    assert_eq!(loaded.exposure_log.0.len(), 1);
    assert_eq!(loaded.exposure_log.0[0].kind, ExposureKindV1::Chat);
    assert_eq!(loaded.relationships.edges.len(), 1);
    assert_eq!(loaded.relationships.edges[0].kind, RelationshipKindV1::Feud);
    assert_eq!(loaded.relationships.edges[0].peer, "char:rival");
    assert_eq!(loaded.relationships.edges[0].metadata["place"], "spawn");
    let loaded_peer = load_social_components(&persistence, "char:rival")
        .expect("reverse relationship should persist for offline peer");
    assert_eq!(loaded_peer.relationships.edges.len(), 1);
    assert_eq!(
        loaded_peer.relationships.edges[0].kind,
        RelationshipKindV1::Feud
    );
    assert_eq!(loaded_peer.relationships.edges[0].peer, "char:azure");
    assert_eq!(loaded.renown.fame, 3);
    assert_eq!(loaded.renown.notoriety, 5);
    assert_eq!(loaded.renown.tags[0].tag, "戮道者");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn social_exposure_live_refreshes_anonymity_for_actor_and_witnesses_only() {
    for kind in [
        ExposureKindV1::Chat,
        ExposureKindV1::Trade,
        ExposureKindV1::Death,
    ] {
        let mut app = qi_test_app();
        app.add_event::<SocialExposureEvent>();
        app.add_systems(Update, apply_social_exposures);
        let (_actor, mut actor_helper) =
            spawn_social_payload_client(&mut app, "Actor", "char:actor", 0.0);
        let (_witness, mut witness_helper) =
            spawn_social_payload_client(&mut app, "Witness", "char:witness", 10.0);
        let (_bystander, mut bystander_helper) =
            spawn_social_payload_client(&mut app, "Bystander", "char:bystander", 20.0);

        app.world_mut().send_event(SocialExposureEvent {
            actor: "char:actor".to_string(),
            kind,
            witnesses: vec!["char:witness".to_string()],
            tick: 42,
            zone: Some("spawn".to_string()),
        });

        app.update();
        flush_all_client_packets(&mut app);

        let actor_payloads = collect_server_data_payloads(&mut actor_helper);
        assert_eq!(
            actor_payloads
                .iter()
                .filter(|payload| payload.payload_type() == ServerDataType::SocialExposure)
                .count(),
            1,
            "actor should still receive social_exposure event for {kind:?}"
        );
        assert_eq!(
            actor_payloads
                .iter()
                .filter(|payload| payload.payload_type() == ServerDataType::SocialAnonymity)
                .count(),
            1,
            "actor should receive one live anonymity refresh for {kind:?}"
        );

        let witness_payloads = collect_server_data_payloads(&mut witness_helper);
        assert_eq!(
            witness_payloads
                .iter()
                .filter(|payload| payload.payload_type() == ServerDataType::SocialExposure)
                .count(),
            1,
            "witness should receive social_exposure event for {kind:?}"
        );
        let witness_anonymity = witness_payloads
            .iter()
            .find_map(|payload| match &payload.payload {
                ServerDataPayloadV1::SocialAnonymity(anonymity) => Some(anonymity),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!("witness should receive live SocialAnonymity refresh for {kind:?}")
            });
        assert_eq!(witness_anonymity.viewer, "char:witness");
        let actor_remote = witness_anonymity
            .remotes
            .iter()
            .find(|remote| remote.player_uuid == "char:actor")
            .expect("witness anonymity refresh should contain exposed actor remote");
        assert!(
            !actor_remote.anonymous,
            "witness should see actor name tag immediately after {kind:?} exposure"
        );
        assert_eq!(actor_remote.display_name.as_deref(), Some("char:actor"));

        let bystander_payloads = collect_server_data_payloads(&mut bystander_helper);
        assert!(
            bystander_payloads.is_empty(),
            "non-witness must not receive exposure or anonymity refresh for {kind:?}"
        );
    }
}

#[test]
fn faction_reputation_delta_updates_matching_client_only() {
    let mut app = qi_test_app();
    app.add_event::<FactionReputationDeltaEvent>();
    app.add_systems(Update, apply_faction_reputation_deltas);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let azure = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(azure).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        FactionReputation::default(),
        Renown::default(),
    ));
    let (other_bundle, _other_helper) = create_mock_client("Other");
    let other = app.world_mut().spawn(other_bundle).id();
    app.world_mut().entity_mut(other).insert((
        Lifecycle {
            character_id: "char:other".to_string(),
            ..Default::default()
        },
        FactionReputation::default(),
        Renown::default(),
    ));
    app.world_mut().send_event(FactionReputationDeltaEvent {
        char_id: "char:azure".to_string(),
        faction: crate::npc::faction::NamedFactionId::QingyunHunters,
        delta: 25,
        tick: 7,
        reason: "test".to_string(),
    });

    app.update();

    let reputation = app.world().get::<FactionReputation>(azure).unwrap();
    let azure_score = reputation.score(crate::npc::faction::NamedFactionId::QingyunHunters);
    assert_eq!(
        azure_score, 25,
        "expected char:azure QingyunHunters reputation to increase by 25 because event char_id matched, actual {azure_score}"
    );
    let other_reputation = app.world().get::<FactionReputation>(other).unwrap();
    let other_score = other_reputation.score(crate::npc::faction::NamedFactionId::QingyunHunters);
    assert_eq!(
        other_score, 0,
        "expected char:other QingyunHunters reputation to stay 0 because event targets char:azure, actual {other_score}"
    );
    let renown = app.world().get::<Renown>(azure).unwrap();
    assert_eq!(
        (renown.fame, renown.notoriety),
        (0, 0),
        "FactionReputation 与全局 Renown 必须并行，不能互相污染"
    );
}

#[test]
fn faction_reputation_delta_is_frozen_for_decayed_faction() {
    let mut registry = crate::npc::faction::NamedFactionRegistry::startup_default();
    registry
        .get_mut(crate::npc::faction::NamedFactionId::QingyunHunters)
        .unwrap()
        .set_status(crate::npc::faction::FactionStatus::Decayed);

    let mut app = qi_test_app();
    app.insert_resource(registry);
    app.add_event::<FactionReputationDeltaEvent>();
    app.add_systems(Update, apply_faction_reputation_deltas);
    let (client_bundle, _helper) = create_mock_client("Azure");
    let azure = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(azure).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        FactionReputation::default(),
    ));
    app.world_mut().send_event(FactionReputationDeltaEvent {
        char_id: "char:azure".to_string(),
        faction: crate::npc::faction::NamedFactionId::QingyunHunters,
        delta: 50,
        tick: 7,
        reason: "decayed".to_string(),
    });

    app.update();

    let reputation = app.world().get::<FactionReputation>(azure).unwrap();
    assert_eq!(
        reputation.score(crate::npc::faction::NamedFactionId::QingyunHunters),
        0,
        "Decayed 势力的 per_faction 信誉必须冻结"
    );
}

#[test]
fn faction_reputation_delta_persists_for_offline_character() {
    let (persistence, data_dir) = social_persistence("offline-faction-reputation-delta");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<FactionReputationDeltaEvent>();
    app.add_systems(Update, apply_faction_reputation_deltas);

    app.world_mut().send_event(FactionReputationDeltaEvent {
        char_id: "char:offline".to_string(),
        faction: crate::npc::faction::NamedFactionId::CangyuanMerchants,
        delta: 33,
        tick: 55,
        reason: "test_offline".to_string(),
    });

    app.update();

    let loaded = load_social_components(&persistence, "char:offline")
        .expect("offline faction reputation should persist");
    let score = loaded
        .faction_reputation
        .score(crate::npc::faction::NamedFactionId::CangyuanMerchants);
    assert_eq!(
        score, 33,
        "expected offline CangyuanMerchants reputation delta to persist, actual {score}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn pact_relationship_emits_link_vfx_for_online_participants() {
    let mut app = qi_test_app();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, apply_social_relationships);

    let (alice_bundle, _alice_helper) = create_mock_client("Alice");
    let alice = app.world_mut().spawn(alice_bundle).id();
    app.world_mut().entity_mut(alice).insert((
        Lifecycle {
            character_id: "char:alice".to_string(),
            ..Default::default()
        },
        Position::new([0.0, 64.0, 0.0]),
        Relationships::default(),
    ));
    let (bob_bundle, _bob_helper) = create_mock_client("Bob");
    let bob = app.world_mut().spawn(bob_bundle).id();
    app.world_mut().entity_mut(bob).insert((
        Lifecycle {
            character_id: "char:bob".to_string(),
            ..Default::default()
        },
        Position::new([2.0, 64.0, 0.0]),
        Relationships::default(),
    ));
    app.world_mut().send_event(SocialRelationshipEvent {
        left: "char:alice".to_string(),
        right: "char:bob".to_string(),
        left_kind: RelationshipKindV1::Pact,
        right_kind: RelationshipKindV1::Pact,
        tick: 81,
        metadata: serde_json::json!({
            "terms": "同渡此劫",
            "broken": false,
        }),
    });

    app.update();

    let events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = events
        .iter_current_update_events()
        .next()
        .expect("online pact should emit link vfx");
    match &emitted.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::SOCIAL_PACT_LINK);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }
}

#[test]
fn pact_events_create_relationship_exposure_and_betrayer_renown() {
    let (persistence, data_dir) = social_persistence("pact-event");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(
        Update,
        (
            handle_social_pacts,
            apply_social_exposures.after(handle_social_pacts),
            apply_social_relationships.after(handle_social_pacts),
            apply_social_renown_deltas.after(handle_social_pacts),
        ),
    );

    app.world_mut().send_event(SocialPactEvent {
        left: "char:alice".to_string(),
        right: "char:bob".to_string(),
        terms: "同渡此劫".to_string(),
        tick: 81,
        broken: false,
        breaker: None,
        witnesses: vec!["char:witness".to_string()],
    });

    app.update();

    let alice = load_social_components(&persistence, "char:alice")
        .expect("alice pact state should persist");
    assert!(alice.anonymity.is_exposed_to("char:bob"));
    assert!(alice.anonymity.is_exposed_to("char:witness"));
    assert_eq!(alice.relationships.edges.len(), 1);
    assert_eq!(alice.relationships.edges[0].kind, RelationshipKindV1::Pact);
    assert_eq!(alice.relationships.edges[0].peer, "char:bob");
    assert_eq!(alice.relationships.edges[0].metadata["terms"], "同渡此劫");
    assert_eq!(alice.relationships.edges[0].metadata["broken"], false);
    let bob =
        load_social_components(&persistence, "char:bob").expect("bob pact state should persist");
    assert!(bob.anonymity.is_exposed_to("char:alice"));
    assert!(bob.anonymity.is_exposed_to("char:witness"));
    assert_eq!(bob.relationships.edges[0].peer, "char:alice");

    app.world_mut().send_event(SocialPactEvent {
        left: "char:alice".to_string(),
        right: "char:bob".to_string(),
        terms: "同渡此劫".to_string(),
        tick: 99,
        broken: true,
        breaker: Some("char:bob".to_string()),
        witnesses: Vec::new(),
    });

    app.update();

    let alice = load_social_components(&persistence, "char:alice")
        .expect("alice broken pact state should persist");
    assert_eq!(alice.relationships.edges.len(), 1);
    assert_eq!(alice.relationships.edges[0].since_tick, 99);
    assert_eq!(alice.relationships.edges[0].metadata["broken"], true);
    assert_eq!(alice.relationships.edges[0].metadata["breaker"], "char:bob");
    let bob = load_social_components(&persistence, "char:bob")
        .expect("bob broken pact state should persist");
    assert_eq!(bob.renown.notoriety, 50);
    assert_eq!(bob.renown.tags.len(), 1);
    assert_eq!(bob.renown.tags[0].tag, "背盟者");
    assert!(bob.renown.tags[0].permanent);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn pact_broken_producer_bridges_to_active_identity_and_panel_score() {
    let mut app = qi_test_app();
    app.add_event::<SocialPactEvent>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(
        Update,
        (
            handle_social_pacts,
            apply_social_renown_deltas.after(handle_social_pacts),
        ),
    );

    let (client_bundle, _helper) = create_mock_client("Breaker");
    let player = app.world_mut().spawn(client_bundle).id();
    let mut identities = PlayerIdentities::with_default("锈锋", 0);
    identities
        .identities
        .push(IdentityProfile::new(IdentityId(1), "碎影", 20));
    identities.active_identity_id = IdentityId(1);
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:breaker".to_string(),
            ..Default::default()
        },
        Renown::default(),
        identities,
    ));

    app.world_mut().send_event(SocialPactEvent {
        left: "char:alice".to_string(),
        right: "char:breaker".to_string(),
        terms: "同渡此劫".to_string(),
        tick: 99,
        broken: true,
        breaker: Some("char:breaker".to_string()),
        witnesses: Vec::new(),
    });

    app.update();

    let social_renown = app.world().get::<Renown>(player).unwrap();
    assert_eq!((social_renown.fame, social_renown.notoriety), (0, 50));
    assert_eq!(social_renown.tags[0].tag, "背盟者");

    let identities = app.world().get::<PlayerIdentities>(player).unwrap();
    let active = identities.active().unwrap();
    assert_eq!(active.id, IdentityId(1));
    assert_eq!((active.renown.fame, active.renown.notoriety), (0, 50));
    assert_eq!(active.reputation_score(), -50);
    assert_eq!(active.renown.tags[0].tag, "背盟者");
    let inactive = identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!((inactive.renown.fame, inactive.renown.notoriety), (0, 0));
    assert!(inactive.renown.tags.is_empty());

    let panel = build_identity_panel_state(identities, 99);
    let active_panel_entry = panel
        .identities
        .iter()
        .find(|entry| entry.identity_id == 1)
        .expect("active identity panel entry");
    assert_eq!(
        active_panel_entry.reputation_score, -50,
        "identity_panel_state must read the same active identity renown as social delta bridge"
    );
}

#[test]
fn renown_delta_crosses_wanted_reaction_from_active_identity_bridge() {
    let mut app = qi_test_app();
    app.insert_resource(crate::npc::movement::GameTick(120));
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_event::<IdentityReactionChangedEvent>();
    app.add_systems(
        Update,
        (
            apply_social_renown_deltas,
            crate::identity::reaction::update_identity_reaction_state
                .after(apply_social_renown_deltas),
        ),
    );

    let (client_bundle, _helper) = create_mock_client("Wanted");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:wanted".to_string(),
            ..Default::default()
        },
        Renown::default(),
        PlayerIdentities::with_default("锈名", 0),
        IdentityReactionState {
            tier: ReactionTier::Normal,
        },
    ));

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:wanted".to_string(),
        identity_id: None,
        fame_delta: 0,
        notoriety_delta: 80,
        tags_added: vec![RenownTagV1 {
            tag: "背盟者".to_string(),
            weight: 80.0,
            last_seen_tick: 120,
            permanent: true,
        }],
        tick: 120,
        reason: "test_wanted_identity_bridge".to_string(),
    });

    app.update();

    let identities = app.world().get::<PlayerIdentities>(player).unwrap();
    let active = identities.active().unwrap();
    assert_eq!(active.reputation_score(), -80);

    let reaction_state = app.world().get::<IdentityReactionState>(player).unwrap();
    assert_eq!(reaction_state.tier, ReactionTier::Wanted);

    let events = app
        .world()
        .resource::<Events<IdentityReactionChangedEvent>>();
    let changed = events
        .iter_current_update_events()
        .next()
        .expect("renown bridge should drive identity reaction boundary");
    assert_eq!(changed.from_tier, ReactionTier::Normal);
    assert_eq!(changed.to_tier, ReactionTier::Wanted);
    assert_eq!(changed.identity_id, IdentityId::DEFAULT);

    let wanted_payload = build_wanted_player_event("char:wanted", identities, changed)
        .expect("wanted payload should build from active identity");
    assert_eq!(wanted_payload.identity_display_name, "锈名");
    assert_eq!(wanted_payload.reputation_score, -80);
}

#[test]
fn mentorship_event_writes_directional_master_disciple_edges() {
    let (persistence, data_dir) = social_persistence("mentorship-event");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialMentorshipEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_systems(
        Update,
        (
            handle_social_mentorships,
            apply_social_relationships.after(handle_social_mentorships),
        ),
    );

    app.world_mut().send_event(SocialMentorshipEvent {
        master: "char:npc_hermit".to_string(),
        disciple: "char:alice".to_string(),
        tick: 313,
        technique_hint: Some("残拳一式".to_string()),
        source: "encounter_event".to_string(),
    });

    app.update();

    let master = load_social_components(&persistence, "char:npc_hermit")
        .expect("master relationship state should reload");
    assert_eq!(master.relationships.edges.len(), 1);
    assert_eq!(
        master.relationships.edges[0].kind,
        RelationshipKindV1::Master
    );
    assert_eq!(master.relationships.edges[0].peer, "char:alice");
    assert_eq!(
        master.relationships.edges[0].metadata["source"],
        "encounter_event"
    );
    assert_eq!(
        master.relationships.edges[0].metadata["technique_hint"],
        "残拳一式"
    );
    let disciple = load_social_components(&persistence, "char:alice")
        .expect("disciple relationship state should reload");
    assert_eq!(disciple.relationships.edges.len(), 1);
    assert_eq!(
        disciple.relationships.edges[0].kind,
        RelationshipKindV1::Disciple
    );
    assert_eq!(disciple.relationships.edges[0].peer, "char:npc_hermit");

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn sparring_invite_dispatches_payload_only_to_target() {
    let mut app = qi_test_app();
    app.init_resource::<SparringInviteRegistry>();
    app.add_event::<SparringInviteRequest>();
    app.add_systems(Update, dispatch_sparring_invites);
    let (initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
    let initiator = app.world_mut().spawn(initiator_bundle).id();
    app.world_mut().entity_mut(initiator).insert((
        Lifecycle {
            character_id: "char:initiator".to_string(),
            ..Default::default()
        },
        Cultivation {
            realm: Realm::Condense,
            ..Default::default()
        },
    ));
    let (target_bundle, mut target_helper) = create_mock_client("Target");
    let target = app.world_mut().spawn(target_bundle).id();
    app.world_mut().entity_mut(target).insert(Lifecycle {
        character_id: "char:target".to_string(),
        ..Default::default()
    });

    app.world_mut().send_event(SparringInviteRequest {
        initiator,
        target,
        terms: "点到为止".to_string(),
        tick: 84000,
    });

    app.update();
    flush_all_client_packets(&mut app);

    assert!(collect_server_data_payloads(&mut initiator_helper).is_empty());
    let payloads = collect_server_data_payloads(&mut target_helper);
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].payload_type(), ServerDataType::SparringInvite);
    match &payloads[0].payload {
        ServerDataPayloadV1::SparringInvite(invite) => {
            assert!(invite.invite_id.starts_with("sparring:"));
            assert!(!invite.invite_id.contains("char:initiator"));
            assert!(!invite.invite_id.contains("char:target"));
            assert_eq!(invite.initiator, "char:initiator");
            assert_eq!(invite.target, "char:target");
            assert_eq!(invite.realm_band, "condense_solidify");
            assert_eq!(invite.breath_hint, "气息相试");
            assert_eq!(invite.terms, "点到为止");
            assert!(invite.expires_at_ms > 0);
        }
        other => panic!("expected sparring invite payload, got {other:?}"),
    }
}

#[test]
fn sparring_acceptance_creates_runtime_session() {
    let mut app = qi_test_app();
    app.init_resource::<SparringInviteRegistry>();
    app.add_event::<SparringInviteRequest>();
    app.add_event::<SparringInviteResponseEvent>();
    app.add_systems(
        Update,
        (
            dispatch_sparring_invites,
            handle_sparring_invite_responses.after(dispatch_sparring_invites),
        ),
    );
    let (initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
    let initiator = app.world_mut().spawn(initiator_bundle).id();
    app.world_mut().entity_mut(initiator).insert((
        Lifecycle {
            character_id: "char:initiator".to_string(),
            ..Default::default()
        },
        Cultivation::default(),
    ));
    let (target_bundle, mut target_helper) = create_mock_client("Target");
    let target = app.world_mut().spawn(target_bundle).id();
    app.world_mut().entity_mut(target).insert(Lifecycle {
        character_id: "char:target".to_string(),
        ..Default::default()
    });

    app.world_mut().send_event(SparringInviteRequest {
        initiator,
        target,
        terms: "点到为止".to_string(),
        tick: 100,
    });
    app.update();
    let invite_id = app
        .world()
        .resource::<SparringInviteRegistry>()
        .pending
        .keys()
        .next()
        .expect("sparring invite should be pending")
        .clone();

    app.world_mut().send_event(SparringInviteResponseEvent {
        player: target,
        invite_id,
        kind: SparringInviteResponseKind::Accept,
        tick: 110,
    });
    app.update();
    flush_all_client_packets(&mut app);
    let _ = collect_server_data_payloads(&mut initiator_helper);
    let _ = collect_server_data_payloads(&mut target_helper);

    let initiator_state = app.world().get::<SparringState>(initiator).unwrap();
    let target_state = app.world().get::<SparringState>(target).unwrap();
    assert_eq!(initiator_state.partner, target);
    assert_eq!(target_state.partner, initiator);
    assert_eq!(initiator_state.invite_id, target_state.invite_id);
}

fn spawn_sparring_player(app: &mut App, name: &str, character_id: &str) -> Entity {
    let (bundle, _helper) = create_mock_client(name);
    let entity = app.world_mut().spawn(bundle).id();
    app.world_mut().entity_mut(entity).insert(Lifecycle {
        character_id: character_id.to_string(),
        ..Default::default()
    });
    entity
}

fn setup_pending_sparring_app(created_at_tick: u64, clock_tick: u64) -> (App, Entity, Entity) {
    let mut app = qi_test_app();
    app.init_resource::<SparringInviteRegistry>();
    app.insert_resource(CombatClock { tick: clock_tick });
    app.add_event::<SparringInviteResponseEvent>();
    app.add_systems(
        Update,
        (
            handle_sparring_invite_responses,
            expire_sparring_sessions.after(handle_sparring_invite_responses),
        ),
    );
    let initiator = spawn_sparring_player(&mut app, "Initiator", "char:initiator");
    let target = spawn_sparring_player(&mut app, "Target", "char:target");
    app.world_mut()
        .resource_mut::<SparringInviteRegistry>()
        .pending
        .insert(
            "sparring:test".to_string(),
            PendingSparringInvite {
                initiator,
                target,
                created_at_tick,
            },
        );
    (app, initiator, target)
}

#[test]
fn sparring_accepts_at_exact_expiry_boundary_and_expires_one_tick_later() {
    let created_at_tick = 100;
    let expiry_tick = created_at_tick + SPARRING_INVITE_TIMEOUT_TICKS;
    let (mut boundary_app, initiator, target) =
        setup_pending_sparring_app(created_at_tick, expiry_tick);
    boundary_app
        .world_mut()
        .send_event(SparringInviteResponseEvent {
            player: target,
            invite_id: "sparring:test".to_string(),
            kind: SparringInviteResponseKind::Accept,
            tick: expiry_tick,
        });

    boundary_app.update();

    assert!(boundary_app
        .world()
        .get::<SparringState>(initiator)
        .is_some());
    assert!(boundary_app.world().get::<SparringState>(target).is_some());
    assert!(boundary_app
        .world()
        .resource::<SparringInviteRegistry>()
        .pending
        .is_empty());

    let (mut expired_app, _initiator, _target) =
        setup_pending_sparring_app(created_at_tick, expiry_tick + 1);
    expired_app.update();
    assert!(
        expired_app
            .world()
            .resource::<SparringInviteRegistry>()
            .pending
            .is_empty(),
        "invite must be garbage-collected immediately after the inclusive expiry boundary"
    );
}

#[test]
fn unauthorized_sparring_responses_leave_invite_consumable_by_target() {
    for kind in [
        SparringInviteResponseKind::Accept,
        SparringInviteResponseKind::Decline,
        SparringInviteResponseKind::Timeout,
    ] {
        let (mut app, initiator, target) = setup_pending_sparring_app(100, 110);
        let intruder = spawn_sparring_player(&mut app, "Intruder", "char:intruder");
        app.world_mut().send_event(SparringInviteResponseEvent {
            player: intruder,
            invite_id: "sparring:test".to_string(),
            kind,
            tick: 110,
        });

        app.update();

        assert!(
            app.world()
                .resource::<SparringInviteRegistry>()
                .pending
                .contains_key("sparring:test"),
            "third-party {kind:?} must not consume the target's pending invite"
        );
        assert!(app.world().get::<SparringState>(intruder).is_none());

        app.world_mut().send_event(SparringInviteResponseEvent {
            player: target,
            invite_id: "sparring:test".to_string(),
            kind: SparringInviteResponseKind::Accept,
            tick: 111,
        });
        app.update();

        let initiator_state = app
            .world()
            .get::<SparringState>(initiator)
            .expect("real target must still be able to accept after an unauthorized response");
        let target_state = app
            .world()
            .get::<SparringState>(target)
            .expect("real target acceptance must create its sparring state");
        assert_eq!(initiator_state.partner, target);
        assert_eq!(target_state.partner, initiator);
        assert!(app
            .world()
            .resource::<SparringInviteRegistry>()
            .pending
            .is_empty());
    }
}

#[test]
fn trade_offer_dispatches_payload_only_to_target_and_hides_ids() {
    let mut app = qi_test_app();
    app.init_resource::<TradeOfferRegistry>();
    app.add_event::<TradeOfferRequest>();
    app.add_systems(Update, dispatch_trade_offers);
    let (mut initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
    initiator_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let initiator = app.world_mut().spawn(initiator_bundle).id();
    app.world_mut().entity_mut(initiator).insert((
        Lifecycle {
            character_id: "char:initiator".to_string(),
            ..Default::default()
        },
        trade_inventory(1001, "出物"),
    ));
    let (mut target_bundle, mut target_helper) = create_mock_client("Target");
    target_bundle.player.position = Position::new([10.0, 64.0, 0.0]);
    let target = app.world_mut().spawn(target_bundle).id();
    app.world_mut().entity_mut(target).insert((
        Lifecycle {
            character_id: "char:target".to_string(),
            ..Default::default()
        },
        trade_inventory(2002, "回物"),
    ));

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    flush_all_client_packets(&mut app);

    assert!(collect_server_data_payloads(&mut initiator_helper).is_empty());
    let payloads = collect_server_data_payloads(&mut target_helper);
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].payload_type(), ServerDataType::TradeOffer);
    match &payloads[0].payload {
        ServerDataPayloadV1::TradeOffer(offer) => {
            assert!(offer.offer_id.starts_with("trade:"));
            assert!(!offer.offer_id.contains("char:initiator"));
            assert!(!offer.offer_id.contains("char:target"));
            assert_eq!(offer.initiator, "char:initiator");
            assert_eq!(offer.target, "char:target");
            assert_eq!(offer.offered_item.instance_id, 1001);
            assert_eq!(offer.requested_items[0].instance_id, 2002);
        }
        other => panic!("expected trade offer payload, got {other:?}"),
    }
    assert_eq!(
        app.world().resource::<TradeOfferRegistry>().pending.len(),
        1
    );
}

#[test]
fn trade_offer_dispatch_allows_low_and_wanted_initiators_between_players() {
    for (tier, notoriety) in [("Low", 0), ("Wanted", 30)] {
        let mut app = qi_test_app();
        app.init_resource::<TradeOfferRegistry>();
        app.add_event::<TradeOfferRequest>();
        app.add_systems(Update, dispatch_trade_offers);
        let (mut initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
        initiator_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let initiator = app.world_mut().spawn(initiator_bundle).id();
        let mut initiator_identities = PlayerIdentities::with_default("毒蛊师", 0);
        initiator_identities.identities[0].renown.notoriety = notoriety;
        initiator_identities.identities[0]
            .revealed_tags
            .push(RevealedTag {
                kind: RevealedTagKind::DuguRevealed,
                witnessed_at_tick: 20,
                witness_realm: Realm::Spirit,
                permanent: true,
            });
        app.world_mut().entity_mut(initiator).insert((
            Lifecycle {
                character_id: "char:initiator".to_string(),
                ..Default::default()
            },
            trade_inventory(1001, "出物"),
            initiator_identities,
        ));
        let (mut target_bundle, mut target_helper) = create_mock_client("Target");
        target_bundle.player.position = Position::new([10.0, 64.0, 0.0]);
        let target = app.world_mut().spawn(target_bundle).id();
        app.world_mut().entity_mut(target).insert((
            Lifecycle {
                character_id: "char:target".to_string(),
                ..Default::default()
            },
            trade_inventory(2002, "回物"),
        ));

        app.world_mut().send_event(TradeOfferRequest {
            initiator,
            target,
            offered_instance_id: 1001,
            tick: 42,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(
            collect_chat_messages(&mut initiator_helper).is_empty(),
            "{tier} 玩家发起玩家交易时不得收到伪 NPC 拒绝文案"
        );
        let target_payloads = collect_server_data_payloads(&mut target_helper);
        assert_eq!(target_payloads.len(), 1, "{tier} 玩家 offer 应送达目标");
        match &target_payloads[0].payload {
            ServerDataPayloadV1::TradeOffer(offer) => {
                assert_eq!(offer.initiator, "char:initiator");
                assert_eq!(offer.target, "char:target");
                assert_eq!(offer.offered_item.instance_id, 1001);
                assert_eq!(offer.requested_items[0].instance_id, 2002);
            }
            other => panic!("expected trade offer payload for {tier}, got {other:?}"),
        }
        assert_eq!(
            app.world().resource::<TradeOfferRegistry>().pending.len(),
            1,
            "{tier} 只约束 NPC 反应，不得阻断玩家对玩家交易"
        );
    }
}

#[test]
fn trade_offer_dispatch_rejects_invalid_requests() {
    let cases = [
        "self_trade",
        "far_target",
        "terminated_initiator",
        "missing_offered_item",
        "empty_target_inventory",
    ];
    for case in cases {
        let mut app = qi_test_app();
        app.init_resource::<TradeOfferRegistry>();
        app.add_event::<TradeOfferRequest>();
        app.add_systems(Update, dispatch_trade_offers);
        let (mut initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
        initiator_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
        let initiator = app.world_mut().spawn(initiator_bundle).id();
        app.world_mut().entity_mut(initiator).insert((
            Lifecycle {
                character_id: "char:initiator".to_string(),
                state: if case == "terminated_initiator" {
                    LifecycleState::Terminated
                } else {
                    LifecycleState::Alive
                },
                ..Default::default()
            },
            trade_inventory(1001, "出物"),
        ));
        let (mut target_bundle, mut target_helper) = create_mock_client("Target");
        target_bundle.player.position = Position::new(if case == "far_target" {
            [80.0, 64.0, 0.0]
        } else {
            [10.0, 64.0, 0.0]
        });
        let target = app.world_mut().spawn(target_bundle).id();
        app.world_mut().entity_mut(target).insert((
            Lifecycle {
                character_id: "char:target".to_string(),
                ..Default::default()
            },
            if case == "empty_target_inventory" {
                empty_trade_inventory()
            } else {
                trade_inventory(2002, "回物")
            },
        ));

        app.world_mut().send_event(TradeOfferRequest {
            initiator,
            target: if case == "self_trade" {
                initiator
            } else {
                target
            },
            offered_instance_id: if case == "missing_offered_item" {
                9999
            } else {
                1001
            },
            tick: 42,
        });
        app.update();
        flush_all_client_packets(&mut app);

        assert!(collect_server_data_payloads(&mut initiator_helper).is_empty());
        assert!(collect_server_data_payloads(&mut target_helper).is_empty());
        assert!(app
            .world()
            .resource::<TradeOfferRegistry>()
            .pending
            .is_empty());
    }
}

#[test]
fn trade_offer_dispatch_rejects_equipped_offered_item() {
    let mut app = qi_test_app();
    app.init_resource::<TradeOfferRegistry>();
    app.add_event::<TradeOfferRequest>();
    app.add_systems(Update, dispatch_trade_offers);
    let (mut initiator_bundle, mut initiator_helper) = create_mock_client("Initiator");
    initiator_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let initiator = app.world_mut().spawn(initiator_bundle).id();
    let mut initiator_inventory = trade_inventory(1001, "出物");
    move_first_trade_item_to_main_hand(&mut initiator_inventory);
    app.world_mut().entity_mut(initiator).insert((
        Lifecycle {
            character_id: "char:initiator".to_string(),
            ..Default::default()
        },
        initiator_inventory,
    ));
    let (mut target_bundle, mut target_helper) = create_mock_client("Target");
    target_bundle.player.position = Position::new([10.0, 64.0, 0.0]);
    let target = app.world_mut().spawn(target_bundle).id();
    app.world_mut().entity_mut(target).insert((
        Lifecycle {
            character_id: "char:target".to_string(),
            ..Default::default()
        },
        trade_inventory(2002, "回物"),
    ));

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    flush_all_client_packets(&mut app);

    assert!(collect_server_data_payloads(&mut initiator_helper).is_empty());
    assert!(
        collect_server_data_payloads(&mut target_helper).is_empty(),
        "equipped offered item must not create a trade offer payload"
    );
    assert!(
        app.world()
            .resource::<TradeOfferRegistry>()
            .pending
            .is_empty(),
        "equipped offered item must not create pending trade state"
    );
    let inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
    assert_eq!(
        inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|slot| slot.held.as_ref())
            .map(|item| item.instance_id),
        Some(1001),
        "rejected trade offer must leave equipped item in main hand"
    );
}

#[test]
fn trade_acceptance_exchanges_items_records_life_and_exposure() {
    let mut app = setup_trade_app();
    let initiator = spawn_trade_player(&mut app, "Initiator", "char:initiator", 0.0);
    let target = spawn_trade_player(&mut app, "Target", "char:target", 10.0);

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();
    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });
    app.update();

    let initiator_inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
    let target_inventory = app.world().get::<PlayerInventory>(target).unwrap();
    assert!(inventory_item_by_instance(initiator_inventory, 1001).is_none());
    assert!(inventory_item_by_instance(target_inventory, 2002).is_none());
    assert!(inventory_item_by_instance(initiator_inventory, 2002).is_some());
    assert!(inventory_item_by_instance(target_inventory, 1001).is_some());
    assert_eq!(initiator_inventory.revision, InventoryRevision(1));
    assert_eq!(target_inventory.revision, InventoryRevision(1));

    let initiator_life = app.world().get::<LifeRecord>(initiator).unwrap();
    let target_life = app.world().get::<LifeRecord>(target).unwrap();
    match initiator_life.biography.as_slice() {
        [BiographyEntry::TradeCompleted {
            counterparty_id,
            offered_item,
            received_item,
            tick,
        }] => {
            assert_eq!(counterparty_id, "char:target");
            assert_eq!(offered_item, "Initiator");
            assert_eq!(received_item, "Target");
            assert_eq!(*tick, 50);
        }
        other => panic!("expected initiator trade biography, got {other:?}"),
    }
    match target_life.biography.as_slice() {
        [BiographyEntry::TradeCompleted {
            counterparty_id,
            offered_item,
            received_item,
            tick,
        }] => {
            assert_eq!(counterparty_id, "char:initiator");
            assert_eq!(offered_item, "Target");
            assert_eq!(received_item, "Initiator");
            assert_eq!(*tick, 50);
        }
        other => panic!("expected target trade biography, got {other:?}"),
    }

    let events = app.world().resource::<Events<SocialExposureEvent>>();
    let mut reader = events.get_reader();
    let collected = reader.read(events).cloned().collect::<Vec<_>>();
    assert_eq!(collected.len(), 2);
    assert!(collected.iter().any(|event| {
        event.actor == "char:initiator"
            && event.kind == ExposureKindV1::Trade
            && event.witnesses == vec!["char:target"]
    }));
    assert!(collected.iter().any(|event| {
        event.actor == "char:target"
            && event.kind == ExposureKindV1::Trade
            && event.witnesses == vec!["char:initiator"]
    }));
    assert!(app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .is_empty());
}

#[test]
fn non_target_trade_response_does_not_consume_offer_before_target_accepts() {
    let mut app = setup_trade_app();
    let initiator = spawn_trade_player(&mut app, "Initiator", "char:initiator", 0.0);
    let target = spawn_trade_player(&mut app, "Target", "char:target", 10.0);
    let intruder = spawn_trade_player(&mut app, "Intruder", "char:intruder", 20.0);

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();

    app.world_mut().send_event(TradeOfferResponseEvent {
        player: intruder,
        offer_id: offer_id.clone(),
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });
    app.update();

    assert!(
        app.world()
            .resource::<TradeOfferRegistry>()
            .pending
            .contains_key(&offer_id),
        "非目标 responder 不得消费 pending offer，否则真实目标无法随后响应"
    );
    assert!(inventory_item_by_instance(
        app.world().get::<PlayerInventory>(initiator).unwrap(),
        1001
    )
    .is_some());
    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(target).unwrap(), 2002)
            .is_some()
    );

    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 51,
    });
    app.update();

    assert!(inventory_item_by_instance(
        app.world().get::<PlayerInventory>(initiator).unwrap(),
        2002
    )
    .is_some());
    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(target).unwrap(), 1001)
            .is_some()
    );
    assert!(app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .is_empty());
}

#[test]
fn successful_trade_emits_one_final_snapshot_per_player_over_bounded_window() {
    let mut app = setup_trade_app();
    let (initiator, mut initiator_helper) =
        spawn_trade_player_with_helper(&mut app, "Initiator", "char:initiator", 0.0);
    let (target, mut target_helper) =
        spawn_trade_player_with_helper(&mut app, "Target", "char:target", 10.0);

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    flush_all_client_packets(&mut app);
    let _ = collect_server_data_payloads(&mut initiator_helper);
    let _ = collect_server_data_payloads(&mut target_helper);
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();
    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });

    for _ in 0..4 {
        app.update();
        flush_all_client_packets(&mut app);
    }

    for (label, helper, received_id, absent_id) in [
        ("initiator", &mut initiator_helper, 2002, 1001),
        ("target", &mut target_helper, 1001, 2002),
    ] {
        let snapshots = collect_server_data_payloads(helper)
            .into_iter()
            .filter_map(|payload| match payload.payload {
                ServerDataPayloadV1::InventorySnapshot(snapshot) => Some(snapshot),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            snapshots.len(),
            1,
            "{label} must receive exactly one final inventory snapshot across the full post-response window"
        );
        let snapshot = &snapshots[0];
        assert_eq!(snapshot.revision, 1);
        assert!(snapshot
            .placed_items
            .iter()
            .any(|placed| placed.item.instance_id == received_id));
        assert!(!snapshot
            .placed_items
            .iter()
            .any(|placed| placed.item.instance_id == absent_id));
    }
}

#[test]
fn capacity_rejected_trade_does_not_emit_unchanged_inventory_snapshots() {
    let mut app = setup_trade_app();
    let (initiator, mut initiator_helper) =
        spawn_trade_player_with_helper(&mut app, "Initiator", "char:initiator", 0.0);
    let (target, mut target_helper) =
        spawn_trade_player_with_helper(&mut app, "Target", "char:target", 10.0);
    {
        let mut initiator_inventory = app
            .world_mut()
            .get_mut::<PlayerInventory>(initiator)
            .unwrap();
        initiator_inventory.containers[0].rows = 1;
        initiator_inventory.containers[0].cols = 1;
    }
    app.world_mut()
        .get_mut::<PlayerInventory>(target)
        .unwrap()
        .containers[0]
        .items[0]
        .instance
        .grid_w = 2;

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    flush_all_client_packets(&mut app);
    let _ = collect_server_data_payloads(&mut initiator_helper);
    let _ = collect_server_data_payloads(&mut target_helper);
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();

    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });
    for _ in 0..4 {
        app.update();
        flush_all_client_packets(&mut app);
    }

    for (label, helper) in [
        ("initiator", &mut initiator_helper),
        ("target", &mut target_helper),
    ] {
        let snapshots = collect_server_data_payloads(helper)
            .into_iter()
            .filter(|payload| matches!(payload.payload, ServerDataPayloadV1::InventorySnapshot(_)))
            .count();
        assert_eq!(
            snapshots, 0,
            "{label} 在容量拒绝后不得收到内容与 revision 均未变化的 inventory_snapshot"
        );
    }
    let initiator_inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
    let target_inventory = app.world().get::<PlayerInventory>(target).unwrap();
    assert_eq!(initiator_inventory.revision, InventoryRevision(0));
    assert_eq!(target_inventory.revision, InventoryRevision(0));
    assert!(inventory_item_by_instance(initiator_inventory, 1001).is_some());
    assert!(inventory_item_by_instance(target_inventory, 2002).is_some());
}

#[test]
fn trade_response_rejects_equipped_requested_item() {
    let mut app = setup_trade_app();
    let initiator = spawn_trade_player(&mut app, "Initiator", "char:initiator", 0.0);
    let target = spawn_trade_player(&mut app, "Target", "char:target", 10.0);

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();
    move_first_trade_item_to_main_hand(
        &mut app.world_mut().get_mut::<PlayerInventory>(target).unwrap(),
    );

    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });
    app.update();

    let initiator_inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
    let target_inventory = app.world().get::<PlayerInventory>(target).unwrap();
    assert!(
        inventory_item_by_instance(initiator_inventory, 1001).is_some(),
        "rejected trade response must leave initiator offered item untouched"
    );
    assert_eq!(
        target_inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|slot| slot.held.as_ref())
            .map(|item| item.instance_id),
        Some(2002),
        "equipped requested item must remain equipped and untraded"
    );
    assert!(
        app.world()
            .get::<LifeRecord>(initiator)
            .unwrap()
            .biography
            .is_empty(),
        "rejected equipped-item trade must not record a completed trade"
    );
    assert!(
        app.world()
            .get::<LifeRecord>(target)
            .unwrap()
            .biography
            .is_empty(),
        "rejected equipped-item trade must not record a completed trade"
    );
}

#[test]
fn trade_response_rejects_equipped_offered_item() {
    let mut app = setup_trade_app();
    let initiator = spawn_trade_player(&mut app, "Initiator", "char:initiator", 0.0);
    let target = spawn_trade_player(&mut app, "Target", "char:target", 10.0);

    app.world_mut().send_event(TradeOfferRequest {
        initiator,
        target,
        offered_instance_id: 1001,
        tick: 42,
    });
    app.update();
    let offer_id = app
        .world()
        .resource::<TradeOfferRegistry>()
        .pending
        .keys()
        .next()
        .expect("trade offer should be pending")
        .clone();
    move_first_trade_item_to_main_hand(
        &mut app
            .world_mut()
            .get_mut::<PlayerInventory>(initiator)
            .unwrap(),
    );

    app.world_mut().send_event(TradeOfferResponseEvent {
        player: target,
        offer_id,
        accepted: true,
        requested_instance_id: Some(2002),
        tick: 50,
    });
    app.update();

    let initiator_inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
    let target_inventory = app.world().get::<PlayerInventory>(target).unwrap();
    assert_eq!(
        initiator_inventory
            .equipped
            .get(EQUIP_SLOT_MAIN_HAND)
            .and_then(|slot| slot.held.as_ref())
            .map(|item| item.instance_id),
        Some(1001),
        "equipped offered item must remain equipped and untraded"
    );
    assert!(
        inventory_item_by_instance(target_inventory, 2002).is_some(),
        "rejected trade response must leave target requested item untouched"
    );
    assert!(
        inventory_item_by_instance(target_inventory, 1001).is_none(),
        "target must not receive initiator equipped offered item"
    );
    assert!(
        app.world()
            .get::<LifeRecord>(initiator)
            .unwrap()
            .biography
            .is_empty(),
        "rejected equipped-item trade must not record a completed trade"
    );
    assert!(
        app.world()
            .get::<LifeRecord>(target)
            .unwrap()
            .biography
            .is_empty(),
        "rejected equipped-item trade must not record a completed trade"
    );
}

#[test]
fn trade_response_rejects_terminal_expired_or_missing_items() {
    let cases = [
        "declined",
        "missing_response_item",
        "expired",
        "terminated_target",
        "far_at_response",
        "offered_item_removed",
        "requested_item_removed",
    ];
    for case in cases {
        let mut app = setup_trade_app();
        let initiator = spawn_trade_player(&mut app, "Initiator", "char:initiator", 0.0);
        let target = spawn_trade_player(&mut app, "Target", "char:target", 10.0);
        app.world_mut().send_event(TradeOfferRequest {
            initiator,
            target,
            offered_instance_id: 1001,
            tick: 42,
        });
        app.update();
        let offer_id = app
            .world()
            .resource::<TradeOfferRegistry>()
            .pending
            .keys()
            .next()
            .expect("trade offer should be pending")
            .clone();

        match case {
            "terminated_target" => {
                app.world_mut().get_mut::<Lifecycle>(target).unwrap().state =
                    LifecycleState::Terminated;
            }
            "far_at_response" => {
                *app.world_mut().get_mut::<Position>(target).unwrap() =
                    Position::new([80.0, 64.0, 0.0]);
            }
            "offered_item_removed" => {
                app.world_mut()
                    .get_mut::<PlayerInventory>(initiator)
                    .unwrap()
                    .containers[0]
                    .items
                    .clear();
            }
            "requested_item_removed" => {
                app.world_mut()
                    .get_mut::<PlayerInventory>(target)
                    .unwrap()
                    .containers[0]
                    .items
                    .clear();
            }
            _ => {}
        }

        app.world_mut().send_event(TradeOfferResponseEvent {
            player: target,
            offer_id,
            accepted: case != "declined",
            requested_instance_id: if case == "missing_response_item" {
                None
            } else {
                Some(2002)
            },
            tick: if case == "expired" {
                42 + TRADE_OFFER_TIMEOUT_TICKS + 1
            } else {
                50
            },
        });
        app.update();

        let initiator_inventory = app.world().get::<PlayerInventory>(initiator).unwrap();
        let target_inventory = app.world().get::<PlayerInventory>(target).unwrap();
        assert!(inventory_item_by_instance(initiator_inventory, 2002).is_none());
        assert!(inventory_item_by_instance(target_inventory, 1001).is_none());
        assert!(app
            .world()
            .resource::<TradeOfferRegistry>()
            .pending
            .is_empty());
    }
}

#[test]
fn expire_trade_offers_garbage_collects_timed_out_pending_offers() {
    let mut app = qi_test_app();
    let initiator = app.world_mut().spawn_empty().id();
    let target = app.world_mut().spawn_empty().id();
    let mut registry = TradeOfferRegistry::default();
    registry.pending.insert(
        "trade:old".to_string(),
        PendingTradeOffer {
            initiator,
            target,
            initiator_char_id: "char:initiator".to_string(),
            target_char_id: "char:target".to_string(),
            offered_instance_id: 1001,
            offered_item: trade_item_summary(&trade_test_item(1001, "出物")),
            expires_at_tick: 10,
        },
    );
    registry.pending.insert(
        "trade:fresh".to_string(),
        PendingTradeOffer {
            initiator,
            target,
            initiator_char_id: "char:initiator".to_string(),
            target_char_id: "char:target".to_string(),
            offered_instance_id: 1001,
            offered_item: trade_item_summary(&trade_test_item(1001, "出物")),
            expires_at_tick: 30,
        },
    );
    app.insert_resource(registry);
    app.insert_resource(CombatClock { tick: 20 });
    app.add_systems(Update, expire_trade_offers);

    app.update();

    let registry = app.world().resource::<TradeOfferRegistry>();
    assert!(!registry.pending.contains_key("trade:old"));
    assert!(registry.pending.contains_key("trade:fresh"));
}

#[test]
fn faction_membership_decisions_apply_cooldown_and_betrayal_tags() {
    let (persistence, data_dir) = social_persistence("faction-membership");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<FactionMembershipDecisionEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(
        Update,
        (
            apply_faction_membership_decisions,
            apply_social_renown_deltas.after(apply_faction_membership_decisions),
        ),
    );
    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        Karma::default(),
    ));

    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::AcceptInvite,
        tick: 10,
    });
    app.update();

    let membership = app.world().get::<FactionMembership>(player).unwrap();
    assert_eq!(membership.faction, FactionId::Attack);
    assert_eq!(membership.rank, 0);
    assert_eq!(membership.loyalty, 10);

    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::Resign,
        tick: 20,
    });
    app.update();

    assert!(app.world().get::<FactionMembership>(player).is_none());
    let loaded = load_social_components(&persistence, "char:azure")
        .expect("resigned membership should persist");
    assert_eq!(loaded.faction_membership.unwrap().loyalty, -10);
    assert_eq!(loaded.renown.notoriety, 0);

    app.world_mut()
        .entity_mut(player)
        .insert(FactionMembership {
            faction: FactionId::Attack,
            named_faction: None,
            rank: 0,
            loyalty: 10,
            betrayal_count: 0,
            invite_block_until_tick: None,
            permanently_refused: false,
        });
    for tick in [30_u64, 40, 50] {
        app.world_mut().send_event(FactionMembershipDecisionEvent {
            player,
            faction: FactionId::Attack,
            kind: FactionMembershipDecisionKind::Betray,
            tick,
        });
        app.update();
    }

    assert!(app.world().get::<FactionMembership>(player).is_none());
    let loaded = load_social_components(&persistence, "char:azure")
        .expect("betrayal membership should persist");
    let membership = loaded
        .faction_membership
        .expect("membership memory remains");
    assert_eq!(membership.betrayal_count, 3);
    assert_eq!(
        membership.invite_block_until_tick,
        Some(50 + FACTION_BETRAYAL_BLOCK_TICKS)
    );
    assert!(membership.permanently_refused);
    assert_eq!(loaded.renown.notoriety, 150);
    assert!(loaded
        .renown
        .tags
        .iter()
        .any(|tag| tag.tag == "三叛之人" && tag.permanent));
    let karma = app.world().get::<Karma>(player).unwrap();
    assert_eq!(karma.weight, 1.0);

    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Defend,
        kind: FactionMembershipDecisionKind::AcceptInvite,
        tick: 60,
    });
    app.update();

    assert!(app.world().get::<FactionMembership>(player).is_none());

    let _ = std::fs::remove_dir_all(data_dir);
}

/// Resign must persist faction=Neutral so reconnect does not restore original faction.
#[test]
fn resign_persists_neutral_faction_prevents_reconnect_readmission() {
    let (persistence, data_dir) = social_persistence("resign-neutral");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<FactionMembershipDecisionEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_faction_membership_decisions);

    let (client_bundle, _helper) = create_mock_client("Azure");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert(Lifecycle {
        character_id: "char:resign-neutral".to_string(),
        ..Default::default()
    });

    // Join Attack faction first.
    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::AcceptInvite,
        tick: 10,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<FactionMembership>(player)
            .unwrap()
            .faction,
        FactionId::Attack,
        "expected Attack membership after AcceptInvite"
    );

    // Resign — should write Neutral to DB, not Attack.
    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::Resign,
        tick: 20,
    });
    app.update();

    assert!(
        app.world().get::<FactionMembership>(player).is_none(),
        "expected FactionMembership component to be removed after Resign"
    );

    // Simulated reconnect: load persisted membership from DB.
    let loaded = load_social_components(&persistence, "char:resign-neutral")
        .expect("load after resign should succeed");
    let persisted_membership = loaded
        .faction_membership
        .expect("persisted membership record must exist to hold betrayal state for invite gating");
    assert_eq!(
        persisted_membership.faction,
        FactionId::Neutral,
        "expected persisted faction=Neutral after Resign — \
         was {:?}, which would have caused auto-readmission on reconnect",
        persisted_membership.faction
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

/// Expel must persist faction=Neutral so reconnect does not restore original faction.
#[test]
fn expel_persists_neutral_faction_prevents_reconnect_readmission() {
    let (persistence, data_dir) = social_persistence("expel-neutral");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<FactionMembershipDecisionEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_faction_membership_decisions);

    let (client_bundle, _helper) = create_mock_client("Defender");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:expel-neutral".to_string(),
            ..Default::default()
        },
        Karma::default(),
    ));

    // Join Defend faction first.
    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Defend,
        kind: FactionMembershipDecisionKind::AcceptInvite,
        tick: 5,
    });
    app.update();
    assert_eq!(
        app.world()
            .get::<FactionMembership>(player)
            .unwrap()
            .faction,
        FactionId::Defend,
        "expected Defend membership after AcceptInvite"
    );

    // Expel — should write Neutral to DB.
    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Defend,
        kind: FactionMembershipDecisionKind::Expel,
        tick: 15,
    });
    app.update();

    assert!(
        app.world().get::<FactionMembership>(player).is_none(),
        "expected FactionMembership component to be removed after Expel"
    );

    let loaded = load_social_components(&persistence, "char:expel-neutral")
        .expect("load after expel should succeed");
    let persisted_membership = loaded
        .faction_membership
        .expect("persisted membership record must exist to hold betrayal/block state");
    assert_eq!(
        persisted_membership.faction,
        FactionId::Neutral,
        "expected persisted faction=Neutral after Expel — \
         was {:?}, which would have caused auto-readmission on reconnect",
        persisted_membership.faction
    );
    // Betrayal count and block tick must still be recorded (used by invite gating).
    assert_eq!(
        persisted_membership.betrayal_count, 1,
        "expected betrayal_count=1 after first Expel"
    );
    assert!(
        persisted_membership.invite_block_until_tick.is_some(),
        "expected invite_block_until_tick to be set after Expel"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

/// Betray must persist faction=Neutral so reconnect does not restore original faction.
/// Also verifies that betrayal_count and permanently_refused are preserved alongside Neutral.
#[test]
fn betray_persists_neutral_faction_and_preserves_refusal_state() {
    let (persistence, data_dir) = social_persistence("betray-neutral");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<FactionMembershipDecisionEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(
        Update,
        (
            apply_faction_membership_decisions,
            apply_social_renown_deltas.after(apply_faction_membership_decisions),
        ),
    );

    let (client_bundle, _helper) = create_mock_client("Traitor");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:betray-neutral".to_string(),
            ..Default::default()
        },
        Karma::default(),
    ));

    // Join Attack, then immediately betray.
    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::AcceptInvite,
        tick: 1,
    });
    app.update();

    app.world_mut().send_event(FactionMembershipDecisionEvent {
        player,
        faction: FactionId::Attack,
        kind: FactionMembershipDecisionKind::Betray,
        tick: 10,
    });
    app.update();

    assert!(
        app.world().get::<FactionMembership>(player).is_none(),
        "expected FactionMembership component to be removed after Betray"
    );

    let loaded = load_social_components(&persistence, "char:betray-neutral")
        .expect("load after betray should succeed");
    let persisted_membership = loaded
        .faction_membership
        .expect("persisted membership must exist to hold betrayal/block state");

    assert_eq!(
        persisted_membership.faction,
        FactionId::Neutral,
        "expected persisted faction=Neutral after Betray — \
         was {:?}, which would have caused auto-readmission on reconnect (the bug)",
        persisted_membership.faction
    );
    // Side-effect fields must survive so invite gating works on reconnect.
    assert_eq!(
        persisted_membership.betrayal_count, 1,
        "expected betrayal_count=1 to survive in persisted row"
    );
    assert!(
        persisted_membership.invite_block_until_tick.is_some(),
        "expected invite block to be persisted alongside Neutral faction"
    );
    assert_eq!(
        persisted_membership.loyalty, 0,
        "expected loyalty=0 after Betray"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn renown_delta_persists_for_offline_character() {
    let (persistence, data_dir) = social_persistence("offline-renown-delta");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_social_renown_deltas);

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:offline".to_string(),
        identity_id: None,
        fame_delta: 2,
        notoriety_delta: 7,
        tags_added: vec![RenownTagV1 {
            tag: "背盟者".to_string(),
            weight: 10.0,
            last_seen_tick: 55,
            permanent: true,
        }],
        tick: 55,
        reason: "test_offline".to_string(),
    });

    app.update();

    let loaded = load_social_components(&persistence, "char:offline")
        .expect("offline renown should persist");
    assert_eq!(loaded.renown.fame, 2);
    assert_eq!(loaded.renown.notoriety, 7);
    assert_eq!(loaded.renown.tags.len(), 1);
    assert_eq!(loaded.renown.tags[0].tag, "背盟者");
    assert!(loaded.renown.tags[0].permanent);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn renown_delta_bridges_to_active_identity_only() {
    let mut app = qi_test_app();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_social_renown_deltas);

    let (client_bundle, _helper) = create_mock_client("Bob");
    let player = app.world_mut().spawn(client_bundle).id();
    let mut identities = PlayerIdentities::with_default("锈锋", 0);
    identities
        .identities
        .push(IdentityProfile::new(IdentityId(1), "碎影", 20));
    identities.active_identity_id = IdentityId(1);
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:bob".to_string(),
            ..Default::default()
        },
        Renown::default(),
        identities,
    ));

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:bob".to_string(),
        identity_id: None,
        fame_delta: 4,
        notoriety_delta: 9,
        tags_added: vec![RenownTagV1 {
            tag: "背信者".to_string(),
            weight: 9.0,
            last_seen_tick: 88,
            permanent: false,
        }],
        tick: 88,
        reason: "test_identity_bridge".to_string(),
    });

    app.update();

    let social_renown = app.world().get::<Renown>(player).unwrap();
    assert_eq!((social_renown.fame, social_renown.notoriety), (4, 9));
    let identities = app.world().get::<PlayerIdentities>(player).unwrap();
    let active = identities.active().unwrap();
    assert_eq!(active.id, IdentityId(1));
    assert_eq!((active.renown.fame, active.renown.notoriety), (4, 9));
    assert_eq!(active.renown.tags[0].tag, "背信者");
    let inactive = identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!(
        (inactive.renown.fame, inactive.renown.notoriety),
        (0, 0),
        "renown bridge must not leak anonymous identity reputation into inactive identities"
    );
    assert!(inactive.renown.tags.is_empty());
}

#[test]
fn renown_delta_with_identity_id_does_not_follow_later_active_switch() {
    let mut app = qi_test_app();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_social_renown_deltas);

    let (client_bundle, _helper) = create_mock_client("AliasRace");
    let player = app.world_mut().spawn(client_bundle).id();
    let mut identities = PlayerIdentities::with_default("锈锋", 0);
    identities
        .identities
        .push(IdentityProfile::new(IdentityId(1), "碎影", 20));
    identities.active_identity_id = IdentityId(1);
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:alias-race".to_string(),
            ..Default::default()
        },
        Renown::default(),
        identities,
    ));

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:alias-race".to_string(),
        identity_id: Some(IdentityId::DEFAULT),
        fame_delta: 0,
        notoriety_delta: 11,
        tags_added: vec![RenownTagV1 {
            tag: "背信者".to_string(),
            weight: 11.0,
            last_seen_tick: 90,
            permanent: false,
        }],
        tick: 90,
        reason: "test_identity_id_pins_alias".to_string(),
    });

    app.update();

    let identities = app.world().get::<PlayerIdentities>(player).unwrap();
    let current_active = identities.active().unwrap();
    assert_eq!(current_active.id, IdentityId(1));
    assert_eq!(
        (current_active.renown.fame, current_active.renown.notoriety),
        (0, 0),
        "delta carrying identity_id=0 must not follow the later active alias"
    );
    let original_alias = identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!(
        (original_alias.renown.fame, original_alias.renown.notoriety),
        (0, 11)
    );
    assert_eq!(original_alias.renown.tags[0].tag, "背信者");
}

#[test]
fn online_renown_delta_persists_active_identity_bridge() {
    let (persistence, data_dir) = social_persistence("online-identity-renown-delta");
    let mut identities = PlayerIdentities::with_default("锈锋", 0);
    identities
        .identities
        .push(IdentityProfile::new(IdentityId(1), "碎影", 20));
    identities.active_identity_id = IdentityId(1);
    identity_db::save_player_identities(&persistence, "char:online", &identities)
        .expect("identity row should persist before online renown event");

    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_social_renown_deltas);

    let (client_bundle, _helper) = create_mock_client("Online");
    let player = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(player).insert((
        Lifecycle {
            character_id: "char:online".to_string(),
            ..Default::default()
        },
        Renown::default(),
        identities,
    ));

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:online".to_string(),
        identity_id: None,
        fame_delta: 5,
        notoriety_delta: 8,
        tags_added: vec![RenownTagV1 {
            tag: "背盟者".to_string(),
            weight: 10.0,
            last_seen_tick: 66,
            permanent: true,
        }],
        tick: 66,
        reason: "test_online_identity_bridge".to_string(),
    });

    app.update();

    let identities = app.world().get::<PlayerIdentities>(player).unwrap();
    let active = identities.active().unwrap();
    assert_eq!(active.id, IdentityId(1));
    assert_eq!((active.renown.fame, active.renown.notoriety), (5, 8));
    assert_eq!(active.renown.tags[0].tag, "背盟者");
    let inactive = identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!((inactive.renown.fame, inactive.renown.notoriety), (0, 0));
    assert!(inactive.renown.tags.is_empty());

    let loaded_identities = identity_db::load_player_identities(&persistence, "char:online")
        .expect("online identity row should load")
        .expect("online identity row should remain");
    let persisted_active = loaded_identities.active().unwrap();
    assert_eq!(persisted_active.id, IdentityId(1));
    assert_eq!(
        (
            persisted_active.renown.fame,
            persisted_active.renown.notoriety
        ),
        (5, 8)
    );
    assert_eq!(persisted_active.renown.tags[0].tag, "背盟者");
    let persisted_inactive = loaded_identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!(
        (
            persisted_inactive.renown.fame,
            persisted_inactive.renown.notoriety
        ),
        (0, 0)
    );
    assert!(persisted_inactive.renown.tags.is_empty());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn renown_delta_bridges_to_offline_persisted_active_identity() {
    let (persistence, data_dir) = social_persistence("offline-identity-renown-delta");
    let mut identities = PlayerIdentities::with_default("锈锋", 0);
    identities
        .identities
        .push(IdentityProfile::new(IdentityId(1), "碎影", 20));
    identities.active_identity_id = IdentityId(1);
    identity_db::save_player_identities(&persistence, "char:offline", &identities)
        .expect("identity row should persist before offline renown event");

    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, apply_social_renown_deltas);

    app.world_mut().send_event(SocialRenownDeltaEvent {
        char_id: "char:offline".to_string(),
        identity_id: None,
        fame_delta: 2,
        notoriety_delta: 7,
        tags_added: vec![RenownTagV1 {
            tag: "背盟者".to_string(),
            weight: 10.0,
            last_seen_tick: 55,
            permanent: true,
        }],
        tick: 55,
        reason: "test_offline_identity_bridge".to_string(),
    });

    app.update();

    let loaded_social = load_social_components(&persistence, "char:offline")
        .expect("offline social renown should persist");
    assert_eq!(
        (loaded_social.renown.fame, loaded_social.renown.notoriety),
        (2, 7)
    );

    let loaded_identities = identity_db::load_player_identities(&persistence, "char:offline")
        .expect("offline identities should load")
        .expect("offline identity row should remain");
    let active = loaded_identities.active().unwrap();
    assert_eq!(active.id, IdentityId(1));
    assert_eq!((active.renown.fame, active.renown.notoriety), (2, 7));
    assert_eq!(active.renown.tags[0].tag, "背盟者");
    let inactive = loaded_identities.get(IdentityId::DEFAULT).unwrap();
    assert_eq!((inactive.renown.fame, inactive.renown.notoriety), (0, 0));
    assert!(inactive.renown.tags.is_empty());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn companion_relationship_emits_after_five_hours_nearby() {
    let mut app = qi_test_app();
    app.insert_resource(CombatClock {
        tick: COMPANION_SCAN_INTERVAL_TICKS,
    });
    let mut progress = CompanionProgress::default();
    progress.pair_seconds.insert(
        companion_pair_key("char:alice", "char:bob"),
        COMPANION_REQUIRED_SECONDS - 1,
    );
    app.insert_resource(progress);
    app.add_event::<SocialRelationshipEvent>();
    app.add_systems(Update, update_companion_relationships);

    let (mut alice_bundle, _alice_helper) = create_mock_client("Alice");
    alice_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let alice = app.world_mut().spawn(alice_bundle).id();
    let (mut bob_bundle, _bob_helper) = create_mock_client("Bob");
    bob_bundle.player.position = Position::new([30.0, 64.0, 0.0]);
    let bob = app.world_mut().spawn(bob_bundle).id();
    app.world_mut().entity_mut(alice).insert((
        Lifecycle {
            character_id: "char:alice".to_string(),
            ..Default::default()
        },
        Relationships::default(),
    ));
    app.world_mut().entity_mut(bob).insert((
        Lifecycle {
            character_id: "char:bob".to_string(),
            ..Default::default()
        },
        Relationships::default(),
    ));

    app.update();

    let events = app.world().resource::<Events<SocialRelationshipEvent>>();
    let mut reader = events.get_reader();
    let collected = reader.read(events).cloned().collect::<Vec<_>>();
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].left, "char:alice");
    assert_eq!(collected[0].right, "char:bob");
    assert_eq!(collected[0].left_kind, RelationshipKindV1::Companion);
    assert_eq!(collected[0].right_kind, RelationshipKindV1::Companion);
    assert_eq!(collected[0].metadata["source"], "co_presence");
    assert_eq!(collected[0].metadata["last_interaction_tick"], 20);
}

#[test]
fn stale_companion_relationships_expire_and_delete_persisted_edges() {
    let (persistence, data_dir) = social_persistence("companion-expire");
    persist_social_relationship(
        &persistence,
        "char:alice",
        &Relationship {
            kind: RelationshipKindV1::Companion,
            peer: "char:bob".to_string(),
            since_tick: 10,
            metadata: serde_json::json!({ "last_interaction_tick": 10 }),
        },
    )
    .expect("left companion edge should persist");
    persist_social_relationship(
        &persistence,
        "char:bob",
        &Relationship {
            kind: RelationshipKindV1::Companion,
            peer: "char:alice".to_string(),
            since_tick: 10,
            metadata: serde_json::json!({ "last_interaction_tick": 10 }),
        },
    )
    .expect("right companion edge should persist");

    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.insert_resource(CombatClock {
        tick: 10 + COMPANION_EXPIRE_TICKS,
    });
    app.add_systems(Update, expire_companion_relationships);
    let (alice_bundle, _alice_helper) = create_mock_client("Alice");
    let alice = app.world_mut().spawn(alice_bundle).id();
    app.world_mut().entity_mut(alice).insert((
        Lifecycle {
            character_id: "char:alice".to_string(),
            ..Default::default()
        },
        Relationships {
            edges: vec![Relationship {
                kind: RelationshipKindV1::Companion,
                peer: "char:bob".to_string(),
                since_tick: 10,
                metadata: serde_json::json!({ "last_interaction_tick": 10 }),
            }],
        },
    ));

    app.update();

    let relationships = app.world().get::<Relationships>(alice).unwrap();
    assert!(relationships.edges.is_empty());
    let alice = load_social_components(&persistence, "char:alice")
        .expect("alice relationship state should reload");
    assert!(alice.relationships.edges.is_empty());
    let bob = load_social_components(&persistence, "char:bob")
        .expect("bob relationship state should reload");
    assert!(bob.relationships.edges.is_empty());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn pk_notoriety_delta_requires_higher_fame_victim() {
    let mut app = qi_test_app();
    app.add_event::<DeathEvent>();
    app.add_event::<SocialExposureEvent>();
    app.add_event::<SocialRelationshipEvent>();
    app.add_event::<SocialRenownDeltaEvent>();
    app.add_systems(Update, handle_death_social_effects);

    let (mut killer_bundle, _killer_helper) = create_mock_client("Killer");
    killer_bundle.player.position = Position::new([0.0, 64.0, 0.0]);
    let killer = app.world_mut().spawn(killer_bundle).id();
    let (mut low_fame_bundle, _low_helper) = create_mock_client("LowFame");
    low_fame_bundle.player.position = Position::new([1.0, 64.0, 0.0]);
    let low_fame_victim = app.world_mut().spawn(low_fame_bundle).id();
    let (mut high_fame_bundle, _high_helper) = create_mock_client("HighFame");
    high_fame_bundle.player.position = Position::new([2.0, 64.0, 0.0]);
    let high_fame_victim = app.world_mut().spawn(high_fame_bundle).id();

    app.world_mut().entity_mut(killer).insert((
        Lifecycle {
            character_id: "char:killer".to_string(),
            ..Default::default()
        },
        Renown {
            fame: 5,
            ..Default::default()
        },
    ));
    app.world_mut().entity_mut(low_fame_victim).insert((
        Lifecycle {
            character_id: "char:low".to_string(),
            ..Default::default()
        },
        Renown {
            fame: 4,
            ..Default::default()
        },
    ));
    app.world_mut().entity_mut(high_fame_victim).insert((
        Lifecycle {
            character_id: "char:high".to_string(),
            ..Default::default()
        },
        Renown {
            fame: 9,
            ..Default::default()
        },
    ));
    app.world_mut().send_event(DeathEvent {
        target: low_fame_victim,
        cause: "pvp".to_string(),
        attacker: Some(killer),
        attacker_player_id: Some("char:killer".to_string()),
        at_tick: 10,
    });
    app.world_mut().send_event(DeathEvent {
        target: high_fame_victim,
        cause: "pvp".to_string(),
        attacker: Some(killer),
        attacker_player_id: Some("char:killer".to_string()),
        at_tick: 11,
    });

    app.update();

    let events = app.world().resource::<Events<SocialRenownDeltaEvent>>();
    let mut reader = events.get_reader();
    let collected = reader.read(events).cloned().collect::<Vec<_>>();
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].char_id, "char:killer");
    assert_eq!(collected[0].notoriety_delta, 10);
    assert_eq!(collected[0].tick, 11);
    assert_eq!(collected[0].reason, "pk_death_higher_fame_victim");
}

#[test]
fn spirit_niche_place_consumes_base_sets_anchor_and_persists() {
    let (persistence, data_dir) = social_persistence("spirit-niche-place");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_test_item(4242)),
        Cultivation {
            qi_current: 10.0,
            ..Default::default()
        },
    ));
    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(4242),
        tick: 77,
    });

    app.update();

    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.spawn_anchor, Some([11.5, 65.0, 10.5]));
    assert!(app.world().get::<SpiritNiche>(entity).is_some());
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(inventory_item_by_instance(inventory, 4242).is_none());
    let loaded = load_social_components(&persistence, "char:azure")
        .expect("spirit niche should persist")
        .spirit_niche
        .expect("persisted niche should load");
    assert_eq!(loaded.pos, [11, 64, 10]);
    assert!(!loaded.revealed);
    let registry = app.world().resource::<SpiritNicheRegistry>();
    assert!(block_break_is_protected_by_registered_spirit_niche(
        Some("char:other"),
        [11, 64, 10],
        registry
    ));
    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted = vfx_events
        .iter_current_update_events()
        .next()
        .expect("spirit niche placement should emit vfx");
    match &emitted.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::SOCIAL_NICHE_ESTABLISH);
        }
        other => panic!("expected SpawnParticle, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn spirit_niche_repair_consumes_kit_clears_damage_and_emits_feedback() {
    let (persistence, data_dir) = social_persistence("spirit-niche-repair");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    let mut registry = SpiritNicheRegistry::default();
    registry.upsert(SpiritNiche {
        owner: "char:azure".to_string(),
        pos: [11, 64, 10],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: true,
        guardians: Vec::new(),
    });
    app.insert_resource(registry);
    app.insert_resource(PendingGameplayNarrations::default());
    app.add_event::<SpiritNicheRepairRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();
    app.add_systems(Update, handle_spirit_niche_repair_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            spawn_anchor: Some([11.5, 65.0, 10.5]),
            spawn_anchor_damaged: true,
            ..Default::default()
        },
        inventory_with_item(niche_repair_kit_test_item(9001)),
        SpiritNiche {
            owner: "char:azure".to_string(),
            pos: [11, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: true,
            guardians: Vec::new(),
        },
    ));
    app.world_mut().send_event(SpiritNicheRepairRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(9001),
        tick: 77,
    });

    app.update();

    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert!(
        !lifecycle.spawn_anchor_damaged,
        "repair must clear the revive-weakened damage marker"
    );
    let niche = app.world().get::<SpiritNiche>(entity).unwrap();
    assert!(!niche.is_damaged, "component damage flag should be cleared");
    let registry = app.world().resource::<SpiritNicheRegistry>();
    assert!(
        !registry.niches.get("char:azure").unwrap().is_damaged,
        "registry damage flag should stay in sync with component"
    );
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(
        inventory_item_by_instance(inventory, 9001).is_none(),
        "successful repair must consume exactly the repair kit instance"
    );
    let loaded = load_social_components(&persistence, "char:azure")
        .expect("repaired niche should persist")
        .spirit_niche
        .expect("persisted niche should load");
    assert!(!loaded.is_damaged, "persisted damage flag should be false");

    let vfx_events = app.world().resource::<Events<VfxEventRequest>>();
    let emitted_vfx = vfx_events
        .iter_current_update_events()
        .next()
        .expect("repair should emit vfx");
    match &emitted_vfx.payload {
        crate::schema::vfx_event::VfxEventPayloadV1::SpawnParticle { event_id, .. } => {
            assert_eq!(event_id, gameplay_vfx::SOCIAL_NICHE_REPAIR);
        }
        other => panic!("expected repair SpawnParticle, got {other:?}"),
    }
    let audio_events = app.world().resource::<Events<PlaySoundRecipeRequest>>();
    let emitted_audio = audio_events
        .iter_current_update_events()
        .next()
        .expect("repair should emit audio");
    assert_eq!(emitted_audio.recipe_id, SPIRIT_NICHE_REPAIR_AUDIO_RECIPE_ID);
    let narrations = app
        .world_mut()
        .resource_mut::<PendingGameplayNarrations>()
        .drain();
    assert_eq!(narrations.len(), 1);
    assert_eq!(narrations[0].target.as_deref(), Some("char:azure"));
    assert_eq!(narrations[0].style, NarrationStyle::Perception);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn spirit_niche_repair_rejects_intact_or_wrong_item_without_consuming() {
    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNicheRepairRequest>();
    app.add_systems(Update, handle_spirit_niche_repair_requests);

    let (mut intact_bundle, _helper) = create_mock_client("Intact");
    intact_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let intact = app.world_mut().spawn(intact_bundle).id();
    app.world_mut().entity_mut(intact).insert((
        Lifecycle {
            character_id: "char:intact".to_string(),
            spawn_anchor: Some([11.5, 65.0, 10.5]),
            ..Default::default()
        },
        inventory_with_item(niche_repair_kit_test_item(9001)),
        SpiritNiche {
            owner: "char:intact".to_string(),
            pos: [11, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: false,
            guardians: Vec::new(),
        },
    ));

    let (mut wrong_bundle, _helper) = create_mock_client("Wrong");
    wrong_bundle.player.position = Position::new([12.0, 64.0, 10.0]);
    let wrong = app.world_mut().spawn(wrong_bundle).id();
    app.world_mut().entity_mut(wrong).insert((
        Lifecycle {
            character_id: "char:wrong".to_string(),
            spawn_anchor: Some([12.5, 65.0, 10.5]),
            spawn_anchor_damaged: true,
            ..Default::default()
        },
        inventory_with_item(wood_plank_test_item(9002)),
        SpiritNiche {
            owner: "char:wrong".to_string(),
            pos: [12, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: true,
            guardians: Vec::new(),
        },
    ));

    app.world_mut().send_event(SpiritNicheRepairRequest {
        player: intact,
        pos: [11, 64, 10],
        item_instance_id: Some(9001),
        tick: 77,
    });
    app.world_mut().send_event(SpiritNicheRepairRequest {
        player: wrong,
        pos: [12, 64, 10],
        item_instance_id: Some(9002),
        tick: 78,
    });

    app.update();

    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(intact).unwrap(), 9001)
            .is_some(),
        "intact niche rejection must not consume repair kit"
    );
    assert!(
        !app.world().get::<SpiritNiche>(intact).unwrap().is_damaged,
        "intact niche should remain intact"
    );
    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(wrong).unwrap(), 9002)
            .is_some(),
        "wrong item rejection must not consume inventory item"
    );
    assert!(
        app.world().get::<SpiritNiche>(wrong).unwrap().is_damaged,
        "wrong item rejection should keep damage flag"
    );
    assert!(
        app.world()
            .get::<Lifecycle>(wrong)
            .unwrap()
            .spawn_anchor_damaged,
        "wrong item rejection should keep revive damage marker"
    );
}

#[test]
fn spirit_niche_repair_rejects_non_owner_and_allows_repeated_damage_repair_cycle() {
    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNicheRepairRequest>();
    app.add_systems(Update, handle_spirit_niche_repair_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            spawn_anchor: Some([11.5, 65.0, 10.5]),
            spawn_anchor_damaged: true,
            ..Default::default()
        },
        inventory_with_item(niche_repair_kit_test_item(9001)),
        SpiritNiche {
            owner: "char:other".to_string(),
            pos: [11, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: true,
            guardians: Vec::new(),
        },
    ));

    app.world_mut().send_event(SpiritNicheRepairRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(9001),
        tick: 77,
    });
    app.update();

    assert!(
        app.world().get::<SpiritNiche>(entity).unwrap().is_damaged,
        "non-owner rejection should keep damage flag"
    );
    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(entity).unwrap(), 9001)
            .is_some(),
        "non-owner rejection must not consume repair kit"
    );

    app.world_mut().entity_mut(entity).insert((
        inventory_with_item(niche_repair_kit_test_item(9002)),
        SpiritNiche {
            owner: "char:azure".to_string(),
            pos: [11, 64, 10],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: true,
            guardians: Vec::new(),
        },
    ));
    app.world_mut().send_event(SpiritNicheRepairRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(9002),
        tick: 78,
    });
    app.update();

    assert!(
        !app.world().get::<SpiritNiche>(entity).unwrap().is_damaged,
        "same niche should be repairable after becoming damaged again"
    );
    assert!(
        inventory_item_by_instance(app.world().get::<PlayerInventory>(entity).unwrap(), 9002)
            .is_none(),
        "valid second repair should consume the new repair kit"
    );
}

#[test]
fn niche_base_template_id_is_recipe_output() {
    let mut registry = crate::craft::CraftRegistry::new();
    crate::craft::register_workbench_recipes(&mut registry)
        .expect("workbench recipes should register");
    let recipe = registry
        .get(&crate::craft::RecipeId::new("workbench.shelter.niche_base"))
        .expect("niche base recipe should exist");

    assert_eq!(
        recipe.output.0, SPIRIT_NICHE_ITEM_TEMPLATE_ID,
        "spirit niche placement must accept the workbench recipe output"
    );
}

#[test]
fn spirit_niche_place_rejects_old_stone_material() {
    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_stone_test_item(4242)),
    ));
    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(4242),
        tick: 77,
    });

    app.update();

    assert!(app.world().get::<SpiritNiche>(entity).is_none());
    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.spawn_anchor, None);
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(
        inventory_item_by_instance(inventory, 4242).is_some(),
        "old spirit_niche_stone is crafting material only and must not be consumed"
    );
}

#[test]
fn spirit_niche_place_rejects_missing_or_wrong_item_without_consuming() {
    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        inventory_with_item(wood_plank_test_item(4242)),
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: None,
        tick: 77,
    });
    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(4242),
        tick: 78,
    });

    app.update();

    assert!(app.world().get::<SpiritNiche>(entity).is_none());
    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.spawn_anchor, None);
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(
        inventory_item_by_instance(inventory, 4242).is_some(),
        "wrong template must not be consumed"
    );
}

#[test]
fn spirit_niche_place_rejects_remote_target_without_consuming() {
    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_test_item(4242)),
    ));
    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [128, 64, 128],
        item_instance_id: Some(4242),
        tick: 77,
    });

    app.update();

    assert!(app.world().get::<SpiritNiche>(entity).is_none());
    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.spawn_anchor, None);
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(
        inventory_item_by_instance(inventory, 4242).is_some(),
        "remote target rejection must happen before item consumption"
    );
}

#[test]
fn spirit_niche_place_rejects_occupied_active_coordinates() {
    let mut app = qi_test_app();
    let mut registry = SpiritNicheRegistry::default();
    registry.upsert(SpiritNiche {
        owner: "char:owner".to_string(),
        pos: [11, 64, 10],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: false,
        guardians: Vec::new(),
    });
    app.insert_resource(registry);
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Azure");
    client_bundle.player.position = Position::new([10.0, 64.0, 10.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:azure".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_test_item(4242)),
    ));
    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [11, 64, 10],
        item_instance_id: Some(4242),
        tick: 77,
    });

    app.update();

    assert!(app.world().get::<SpiritNiche>(entity).is_none());
    let lifecycle = app.world().get::<Lifecycle>(entity).unwrap();
    assert_eq!(lifecycle.spawn_anchor, None);
    let inventory = app.world().get::<PlayerInventory>(entity).unwrap();
    assert!(inventory_item_by_instance(inventory, 4242).is_some());
    let registry = app.world().resource::<SpiritNicheRegistry>();
    assert_eq!(registry.active_niches().count(), 1);
}

#[test]
fn load_social_spirit_niche_rejects_invalid_guardians_json() {
    let (persistence, data_dir) = social_persistence("spirit-niche-invalid-guardians");
    let connection =
        open_social_connection(&persistence).expect("social sqlite should open for test");
    connection
        .execute(
            "
            INSERT INTO social_spirit_niches (
                owner, pos_x, pos_y, pos_z, placed_at_tick, revealed, revealed_by,
                guardians_json, schema_version, last_updated_wall
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9)
            ",
            rusqlite::params![
                "char:owner",
                20_i64,
                64_i64,
                20_i64,
                1_i64,
                0_i64,
                Option::<String>::None,
                "{not-valid-json",
                100_i64,
            ],
        )
        .expect("invalid guardians fixture row should insert");

    let error = load_social_components(&persistence, "char:owner")
        .expect_err("invalid guardians_json must not silently drop guardians");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        error.to_string().contains("invalid guardians_json"),
        "unexpected error: {error}"
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn spirit_niche_break_attempt_reveals_and_disables_anchor() {
    let (persistence, data_dir) = social_persistence("spirit-niche-reveal");
    let mut app = qi_test_app();
    app.insert_resource(persistence.clone());
    let mut registry = SpiritNicheRegistry::default();
    registry.upsert(SpiritNiche {
        owner: "char:owner".to_string(),
        pos: [20, 64, 20],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: false,
        guardians: Vec::new(),
    });
    app.insert_resource(registry);
    app.add_event::<DiggingEvent>();
    app.add_event::<SpiritNicheRevealRequest>();
    app.add_event::<NicheIntrusionAttempt>();
    app.add_systems(
        Update,
        (
            detect_spirit_niche_break_attempts,
            apply_spirit_niche_reveals.after(detect_spirit_niche_break_attempts),
        ),
    );

    let (owner_bundle, _owner_helper) = create_mock_client("Owner");
    let owner = app.world_mut().spawn(owner_bundle).id();
    app.world_mut().entity_mut(owner).insert((
        Lifecycle {
            character_id: "char:owner".to_string(),
            spawn_anchor: Some([20.5, 65.0, 20.5]),
            ..Default::default()
        },
        SpiritNiche {
            owner: "char:owner".to_string(),
            pos: [20, 64, 20],
            placed_at_tick: 1,
            revealed: false,
            revealed_by: None,
            is_damaged: false,
            guardians: Vec::new(),
        },
    ));
    let (mut observer_bundle, _observer_helper) = create_mock_client("Observer");
    observer_bundle.player.position = Position::new([20.0, 64.0, 20.0]);
    let observer = app.world_mut().spawn(observer_bundle).id();
    app.world_mut().entity_mut(observer).insert((
        Lifecycle {
            character_id: "char:observer".to_string(),
            ..Default::default()
        },
        Cultivation {
            qi_current: 2.0,
            qi_max: 10.0,
            ..Default::default()
        },
    ));
    app.world_mut().send_event(DiggingEvent {
        client: observer,
        position: BlockPos::new(20, 64, 20),
        direction: valence::protocol::Direction::Up,
        state: DiggingState::Start,
    });

    app.update();

    let lifecycle = app.world().get::<Lifecycle>(owner).unwrap();
    assert_eq!(lifecycle.spawn_anchor, None);
    let niche = app.world().get::<SpiritNiche>(owner).unwrap();
    assert!(niche.revealed);
    assert_eq!(niche.revealed_by.as_deref(), Some("char:observer"));
    assert!(!block_break_is_protected_by_registered_spirit_niche(
        Some("char:other"),
        [20, 64, 20],
        app.world().resource::<SpiritNicheRegistry>()
    ));
    let loaded = load_social_components(&persistence, "char:owner")
        .expect("revealed spirit niche should persist")
        .spirit_niche
        .expect("persisted niche should load");
    assert!(loaded.revealed);
    assert_eq!(loaded.revealed_by.as_deref(), Some("char:observer"));

    let mut attempts = app
        .world_mut()
        .resource_mut::<Events<NicheIntrusionAttempt>>();
    let attempts = attempts.drain().collect::<Vec<_>>();
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].intruder, observer);
    assert_eq!(attempts[0].intruder_char_id, "char:observer");
    assert_eq!(attempts[0].niche_owner, "char:owner");
    assert_eq!(attempts[0].niche_pos, [20, 64, 20]);
    assert_eq!(attempts[0].items_taken, Vec::<u64>::new());
    assert_eq!(attempts[0].intruder_qi_fraction, 0.2);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn spirit_niche_coordinate_reveal_emits_owner_reveal_only_for_exact_active_hit() {
    let mut app = qi_test_app();
    let mut registry = SpiritNicheRegistry::default();
    registry.upsert(SpiritNiche {
        owner: "char:owner".to_string(),
        pos: [20, 64, 20],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: false,
        guardians: Vec::new(),
    });
    app.insert_resource(registry);
    app.add_event::<SpiritNicheCoordinateRevealRequest>();
    app.add_event::<SpiritNicheRevealRequest>();
    app.add_systems(Update, handle_spirit_niche_coordinate_reveals);

    let (mut observer_bundle, _observer_helper) = create_mock_client("Observer");
    observer_bundle.player.position = Position::new([20.0, 64.0, 20.0]);
    let observer = app.world_mut().spawn(observer_bundle).id();
    app.world_mut().entity_mut(observer).insert(Lifecycle {
        character_id: "char:observer".to_string(),
        ..Default::default()
    });
    app.world_mut()
        .send_event(SpiritNicheCoordinateRevealRequest {
            observer,
            pos: [20, 64, 20],
            source: SpiritNicheRevealSource::Gaze,
            tick: 99,
        });
    app.world_mut()
        .send_event(SpiritNicheCoordinateRevealRequest {
            observer,
            pos: [21, 64, 20],
            source: SpiritNicheRevealSource::MarkCoordinate,
            tick: 100,
        });

    app.update();

    let mut events = app
        .world_mut()
        .resource_mut::<Events<SpiritNicheRevealRequest>>();
    let collected = events.drain().collect::<Vec<_>>();
    assert_eq!(collected.len(), 1);
    assert_eq!(collected[0].observer, Some(observer));
    assert_eq!(collected[0].owner, "char:owner");
    assert_eq!(collected[0].source, SpiritNicheRevealSource::Gaze);
    assert_eq!(collected[0].tick, 99);
}

#[test]
fn spirit_niche_coordinate_reveal_rejects_remote_coordinate_hits() {
    let mut app = qi_test_app();
    let mut registry = SpiritNicheRegistry::default();
    registry.upsert(SpiritNiche {
        owner: "char:owner".to_string(),
        pos: [20, 64, 20],
        placed_at_tick: 1,
        revealed: false,
        revealed_by: None,
        is_damaged: false,
        guardians: Vec::new(),
    });
    app.insert_resource(registry);
    app.add_event::<SpiritNicheCoordinateRevealRequest>();
    app.add_event::<SpiritNicheRevealRequest>();
    app.add_systems(Update, handle_spirit_niche_coordinate_reveals);

    let (mut observer_bundle, _observer_helper) = create_mock_client("Observer");
    observer_bundle.player.position = Position::new([80.0, 64.0, 80.0]);
    let observer = app.world_mut().spawn(observer_bundle).id();
    app.world_mut().entity_mut(observer).insert(Lifecycle {
        character_id: "char:observer".to_string(),
        ..Default::default()
    });
    app.world_mut()
        .send_event(SpiritNicheCoordinateRevealRequest {
            observer,
            pos: [20, 64, 20],
            source: SpiritNicheRevealSource::Gaze,
            tick: 99,
        });

    app.update();

    let mut events = app
        .world_mut()
        .resource_mut::<Events<SpiritNicheRevealRequest>>();
    assert!(events.drain().next().is_none());
}

// ── QS-03 fix: spirit_niche_penalty qi conservation ─────────────────────

/// 在负灵域放置灵龛时，真元损耗应从玩家扣除 *并* 通过 QiTransfer 回灌至负灵域，
/// 守恒不变式：zone.spirit_qi 在扣减后升高（向 0 靠近），且 QiTransfer 事件被 emit。
///
/// 测试构造细节：
/// - zone 初始 spirit_qi = -0.5 (负灵域, room = (50-(-25)).max(0)=75, 全额接收)
/// - 玩家 qi_max=100, qi_current=100, Realm::Awaken (realm_factor=1.0)
/// - 预期 damage = 100 * 0.1 * 1.0 = 10.0
/// - 玩家 qi_current 后 = 90.0
/// - zone spirit_qi 后 = (-25 + 10) / 50 = -0.3
/// - QiTransfer 事件 amount = 10.0, reason = ReleaseToZone
/// - entity 需插 CurrentDimension(Overworld) 否则 find_zone 返回 None 走 Overflow 而不更新 zone
#[test]
fn spirit_niche_place_negative_zone_debits_qi_and_credits_zone() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();

    // 建立带负灵气的 zone registry
    let mut zone_registry = ZoneRegistry::fallback();
    zone_registry
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist in fallback registry")
        .spirit_qi = -0.5;
    app.insert_resource(zone_registry);

    app.add_systems(Update, handle_spirit_niche_place_requests);

    // 玩家位置在 spawn zone 内（bounds: [-128,64,-128] → [128,80,128]）
    let (mut client_bundle, _helper) = create_mock_client("Negative");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:negative".to_string(),
            ..Default::default()
        },
        LifeRecord::new("char:negative"),
        inventory_with_item(spirit_niche_test_item(9001)),
        Cultivation {
            realm: Realm::Awaken,
            qi_max: 100.0,
            qi_current: 100.0,
            ..Default::default()
        },
        // 必须插入 CurrentDimension 否则 find_zone 返回 None 走 Overflow 而不更新 zone (pitfall b)
        CurrentDimension(DimensionKind::Overworld),
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [9, 66, 8],
        item_instance_id: Some(9001),
        tick: 1,
    });
    app.update();

    // 1. 玩家 qi_current 应扣减 10.0
    let cultivation = app.world().get::<Cultivation>(entity).expect("Cultivation");
    assert!(
        (cultivation.qi_current - 90.0).abs() < 1e-9,
        "spirit_niche_penalty: 预期 qi_current=90.0 (扣减10.0 damage), 实际={}",
        cultivation.qi_current
    );

    // 2. zone spirit_qi 应升高（damage 10 = 10/50=0.2 → spirit_qi 从 -0.5 升至 -0.3）
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone must exist after update")
        .spirit_qi;
    assert!(
        (zone_after - (-0.3)).abs() < 1e-9,
        "spirit_niche_penalty: 预期 zone spirit_qi=-0.3 (从-0.5升高0.2), 实际={}",
        zone_after
    );

    // 3. QiTransfer 事件应 emit，amount=10.0, reason=ReleaseToZone
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let matching: Vec<_> = transfers
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "spirit_niche_penalty: 应恰好 emit 1 个 ReleaseToZone QiTransfer，实际 {}",
        matching.len()
    );
    assert!(
        (matching[0].amount - 10.0).abs() < 1e-9,
        "spirit_niche_penalty: QiTransfer amount 应=10.0，实际={}",
        matching[0].amount
    );
}

/// 守恒 Critical（CodeRabbit #699）：qi_current < damage 时只回灌实际扣减量，不凭空创生真元。
/// qi_current=5, damage=10（qi_max=100, Awaken）→ 实扣 5、qi_current→0、zone 仅 +5（-0.5→-0.4），
/// 而非 bug 的 +10（-0.5→-0.3 凭空多 5 真元）。
#[test]
fn spirit_niche_place_negative_zone_credits_only_actual_deducted_qi() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();

    let mut zone_registry = ZoneRegistry::fallback();
    zone_registry
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist in fallback registry")
        .spirit_qi = -0.5;
    app.insert_resource(zone_registry);
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("LowQi");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:lowqi".to_string(),
            ..Default::default()
        },
        LifeRecord::new("char:lowqi"),
        inventory_with_item(spirit_niche_test_item(9002)),
        Cultivation {
            realm: Realm::Awaken,
            qi_max: 100.0,
            qi_current: 5.0, // < damage(10) → 实际只能扣 5
            ..Default::default()
        },
        CurrentDimension(DimensionKind::Overworld),
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [9, 66, 8],
        item_instance_id: Some(9002),
        tick: 1,
    });
    app.update();

    // qi_current clamp 到 0（扣 5）
    let qi_after = app
        .world()
        .get::<Cultivation>(entity)
        .expect("Cultivation")
        .qi_current;
    assert!(
        qi_after.abs() < 1e-9,
        "qi_current 应被扣到 0（5 - damage10 clamp），实际={qi_after}"
    );

    // zone 只 +5（实际扣减量）：-25+5=-20 → spirit_qi=-0.4（非 bug 的 -0.3）
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone")
        .spirit_qi;
    assert!(
        (zone_after - (-0.4)).abs() < 1e-9,
        "zone 应只回灌实际扣减 5（-0.5→-0.4），实际={zone_after}；若 ≈-0.3 说明回灌了完整 damage 凭空创生真元"
    );

    // QiTransfer amount == 实际扣减 5（非请求的 10）
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let matching: Vec<_> = transfers
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .collect();
    assert_eq!(matching.len(), 1, "应恰好 1 个 ReleaseToZone");
    assert!(
        (matching[0].amount - 5.0).abs() < 1e-9,
        "QiTransfer amount 应=实际扣减 5，实际={}",
        matching[0].amount
    );
}

/// 边界：正灵域放置灵龛不触发真元损耗，zone 不变，无 QiTransfer emit。
#[test]
fn spirit_niche_place_positive_zone_no_qi_cost_no_transfer() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::ZoneRegistry;

    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();

    // 默认 spawn zone spirit_qi=0.9 (正灵域)
    app.insert_resource(ZoneRegistry::fallback());
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("Positive");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:positive".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_test_item(9002)),
        Cultivation {
            realm: Realm::Awaken,
            qi_max: 100.0,
            qi_current: 80.0,
            ..Default::default()
        },
        CurrentDimension(DimensionKind::Overworld),
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [9, 66, 8],
        item_instance_id: Some(9002),
        tick: 2,
    });
    app.update();

    // 正灵域：qi_current 不变
    let cultivation = app.world().get::<Cultivation>(entity).expect("Cultivation");
    assert!(
        (cultivation.qi_current - 80.0).abs() < 1e-9,
        "正灵域放置灵龛不应扣减 qi_current，预期 80.0，实际={}",
        cultivation.qi_current
    );

    // 无 ReleaseToZone QiTransfer
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let release_count = transfers
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .count();
    assert_eq!(
        release_count, 0,
        "正灵域放置灵龛不应 emit ReleaseToZone QiTransfer，实际 {}",
        release_count
    );
}

/// 边界：玩家在负灵域放置时如果没有 Cultivation 组件，不崩溃也不产生任何 QiTransfer。
#[test]
fn spirit_niche_place_negative_zone_no_cultivation_no_qi_event() {
    use crate::qi_physics::ledger::QiTransferReason;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();

    // 负灵域
    let mut zone_registry = ZoneRegistry::fallback();
    zone_registry
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = -0.5;
    app.insert_resource(zone_registry);
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("NoCultivation");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:nocultivation".to_string(),
            ..Default::default()
        },
        inventory_with_item(spirit_niche_test_item(9003)),
        CurrentDimension(DimensionKind::Overworld),
        // 故意不插 Cultivation
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [9, 66, 8],
        item_instance_id: Some(9003),
        tick: 3,
    });
    app.update();

    // 灵龛放置成功（即使没有 Cultivation）
    assert!(
        app.world().get::<SpiritNiche>(entity).is_some(),
        "无 Cultivation 时仍应成功放置灵龛"
    );

    // 无 ReleaseToZone QiTransfer（没有真元可扣）
    let transfers = app.world().resource::<Events<QiTransfer>>();
    let release_count = transfers
        .iter_current_update_events()
        .filter(|t| t.reason == QiTransferReason::ReleaseToZone)
        .count();
    assert_eq!(
        release_count, 0,
        "无 Cultivation 时不应产生 ReleaseToZone QiTransfer，实际 {}",
        release_count
    );
}

/// 边界：所有六个境界的 realm_factor 覆盖——负灵域 realm::Void 扣最多 (2.5x)。
#[test]
fn spirit_niche_place_negative_zone_void_realm_max_damage() {
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};

    let mut app = qi_test_app();
    app.insert_resource(SpiritNicheRegistry::default());
    app.add_event::<SpiritNichePlaceRequest>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<QiTransfer>();

    let mut zone_registry = ZoneRegistry::fallback();
    zone_registry
        .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone should exist")
        .spirit_qi = -1.0; // 完全负灵域，room 最大
    app.insert_resource(zone_registry);
    app.add_systems(Update, handle_spirit_niche_place_requests);

    let (mut client_bundle, _helper) = create_mock_client("VoidRealm");
    client_bundle.player.position = Position::new([8.0, 66.0, 8.0]);
    let entity = app.world_mut().spawn(client_bundle).id();
    app.world_mut().entity_mut(entity).insert((
        Lifecycle {
            character_id: "char:voidrealm".to_string(),
            ..Default::default()
        },
        LifeRecord::new("char:voidrealm"),
        inventory_with_item(spirit_niche_test_item(9004)),
        Cultivation {
            realm: Realm::Void,
            qi_max: 100.0,
            qi_current: 100.0,
            ..Default::default()
        },
        CurrentDimension(DimensionKind::Overworld),
    ));

    app.world_mut().send_event(SpiritNichePlaceRequest {
        player: entity,
        pos: [9, 66, 8],
        item_instance_id: Some(9004),
        tick: 4,
    });
    app.update();

    // Void realm_factor = 2.5; damage = 100 * 0.1 * 2.5 = 25.0
    let expected_damage = 25.0_f64;
    let cultivation = app.world().get::<Cultivation>(entity).expect("Cultivation");
    assert!(
        (cultivation.qi_current - (100.0 - expected_damage)).abs() < 1e-9,
        "Void 境界 spirit_niche_penalty: 预期 qi_current={}, 实际={}",
        100.0 - expected_damage,
        cultivation.qi_current
    );

    // zone 应升高 25/50 = 0.5 → 从 -1.0 升至 -0.5
    let zone_after = app
        .world()
        .resource::<ZoneRegistry>()
        .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
        .expect("spawn zone must exist")
        .spirit_qi;
    assert!(
        (zone_after - (-0.5)).abs() < 1e-9,
        "Void 境界 spirit_niche_penalty: 预期 zone spirit_qi=-0.5, 实际={}",
        zone_after
    );
}
