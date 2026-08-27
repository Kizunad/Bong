use std::collections::HashMap;

use bong_server::body_plan::race_registry::RaceRegistry;
use bong_server::combat::events::ApplyStatusEffectIntent;
use bong_server::cultivation::breakthrough::BreakthroughRequest;
use bong_server::cultivation::forging::ForgeRequest as ForgeGameplayRequest;
use bong_server::cultivation::insight::InsightChosen;
use bong_server::cultivation::known_techniques::TechniqueRegistry;
use bong_server::forge::blueprint::BlueprintRegistry;
use bong_server::forge::blueprint::TemperBeat;
use bong_server::forge::events::{
    ConsecrationInject, InscriptionScrollSubmit, StartForgeRequest, StepAdvance, TemperingHit,
};
use bong_server::forge::session::{ForgeSession, ForgeSessionId, ForgeSessions, ForgeStep};
use bong_server::forge::steps::next_step_after;
use bong_server::inventory::{
    InscriptionScrollSpec, InventoryRevision, ItemInstance, ItemRegistry, ItemTemplate,
    PlayerInventory,
};
use bong_server::network::agent_ui::AgentUiResponseEvent;
use bong_server::network::client_request::forge_contract::{
    dispatch_forge_request, try_into_forge_request, ForgeRequest,
};
use bong_server::network::client_request_handler::{
    ClientRequestDispatchParams, SkillScrollRequestParams,
};
use bong_server::player::state::PlayerState;
use bong_server::schema::client_request::ClientRequestV1;
use valence::prelude::{
    App, Client, Commands, Entity, Events, Query, ResMut, Resource, Update, Username,
};

struct PendingForgeBatch(Vec<(Entity, ForgeRequest)>);

impl Resource for PendingForgeBatch {}

fn dispatch_pending_batch(
    mut pending: ResMut<PendingForgeBatch>,
    mut dispatch: ClientRequestDispatchParams,
    mut skill_scroll: SkillScrollRequestParams,
    mut commands: Commands,
    mut clients: Query<(&Username, &mut Client)>,
    mut inventories: Query<&mut PlayerInventory>,
    player_states: Query<&PlayerState>,
) {
    let requests = std::mem::take(&mut pending.0);
    let mut projected_steps = HashMap::new();
    for (player, request) in requests {
        dispatch_forge_request(
            request,
            player,
            &mut projected_steps,
            &mut dispatch,
            &mut skill_scroll,
            &mut commands,
            &mut clients,
            &mut inventories,
            &player_states,
        );
    }
}

fn forge_dispatch_app() -> App {
    let mut app = App::new();
    app.insert_resource(
        TechniqueRegistry::load_from_path(
            format!(
                "{}/assets/cultivation/techniques.toml",
                env!("CARGO_MANIFEST_DIR")
            ),
            &RaceRegistry::default(),
        )
        .expect("the checked-in technique catalog must be readable"),
    );
    app.insert_resource(ItemRegistry::from_map(HashMap::new()));
    app.insert_resource(
        BlueprintRegistry::load_dir(format!(
            "{}/assets/forge/blueprints",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("the Forge dispatcher contract test needs the checked-in blueprint registry"),
    );
    app.add_event::<ApplyStatusEffectIntent>();
    app.add_event::<BreakthroughRequest>();
    app.add_event::<ForgeGameplayRequest>();
    app.add_event::<InsightChosen>();
    app.add_event::<AgentUiResponseEvent>();
    app.add_event::<StartForgeRequest>();
    app.add_event::<StepAdvance>();
    app.add_event::<TemperingHit>();
    app.add_event::<InscriptionScrollSubmit>();
    app.add_event::<ConsecrationInject>();
    app.insert_resource(PendingForgeBatch(Vec::new()));
    app.add_systems(Update, dispatch_pending_batch);
    app
}

fn inscription_inventory() -> PlayerInventory {
    let mut hotbar: [Option<ItemInstance>; 9] = std::array::from_fn(|_| None);
    hotbar[0] = Some(ItemInstance {
        instance_id: 7001,
        template_id: "inscription_scroll_sharp".to_owned(),
        display_name: "锋锐残卷".to_owned(),
        grid_w: 1,
        grid_h: 1,
        weight: 0.1,
        rarity: bong_server::inventory::ItemRarity::Common,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 0.0,
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
    });
    PlayerInventory {
        revision: InventoryRevision(0),
        containers: Vec::new(),
        equipped: HashMap::new(),
        hotbar,
        bone_coins: 0,
        max_weight: 99.0,
        triggered_treasures: Vec::new(),
    }
}

fn session(id: u64, caster: Entity, step: ForgeStep, step_index: usize) -> ForgeSession {
    let mut session = ForgeSession::new(
        ForgeSessionId(id),
        "ling_feng_v0".to_owned(),
        Entity::from_raw(10_000 + id as u32),
        caster,
    );
    session.current_step = step;
    session.step_index = step_index;
    session
}

#[test]
fn forge_extractor_accepts_exactly_eight_variants_and_preserves_all_fields() {
    let cases = [
        (
            ClientRequestV1::ForgeStationPlace {
                v: 1,
                x: i32::MIN,
                y: 64,
                z: i32::MAX,
                item_instance_id: u64::MAX,
                station_tier: u8::MAX,
            },
            ForgeRequest::StationPlace {
                x: i32::MIN,
                y: 64,
                z: i32::MAX,
                item_instance_id: u64::MAX,
                station_tier: u8::MAX,
            },
        ),
        (
            ClientRequestV1::ForgeInscriptionScroll {
                v: 1,
                session_id: u64::MAX,
                inscription_id: "sharp_v0".to_owned(),
            },
            ForgeRequest::InscriptionScroll {
                session_id: u64::MAX,
                inscription_id: "sharp_v0".to_owned(),
            },
        ),
        (
            ClientRequestV1::ForgeTemperingHit {
                v: 1,
                session_id: 9,
                beat: "F".to_owned(),
                ticks_remaining: u32::MAX,
            },
            ForgeRequest::TemperingHit {
                session_id: 9,
                beat: "F".to_owned(),
                ticks_remaining: u32::MAX,
            },
        ),
        (
            ClientRequestV1::ForgeConsecrationInject {
                v: 1,
                session_id: 10,
                qi_amount: 12.5,
            },
            ForgeRequest::ConsecrationInject {
                session_id: 10,
                qi_amount: 12.5,
            },
        ),
        (
            ClientRequestV1::ForgeStepAdvance {
                v: 1,
                session_id: 11,
            },
            ForgeRequest::StepAdvance { session_id: 11 },
        ),
        (
            ClientRequestV1::ForgeLearnBlueprint {
                v: 1,
                blueprint_id: "ling_feng_v0".to_owned(),
            },
            ForgeRequest::LearnBlueprint {
                blueprint_id: "ling_feng_v0".to_owned(),
            },
        ),
        (
            ClientRequestV1::ForgeStartSession {
                v: 1,
                station_pos: (i32::MIN, 0, i32::MAX),
                blueprint_id: "ling_feng_v0".to_owned(),
                materials: vec![("fan_tie".to_owned(), u32::MAX)],
            },
            ForgeRequest::StartSession {
                station_pos: (i32::MIN, 0, i32::MAX),
                blueprint_id: "ling_feng_v0".to_owned(),
                materials: vec![("fan_tie".to_owned(), u32::MAX)],
            },
        ),
        (
            ClientRequestV1::ForgeBlueprintTurnPage {
                v: 1,
                delta: i32::MIN,
            },
            ForgeRequest::BlueprintTurnPage { delta: i32::MIN },
        ),
    ];

    for (wire, expected) in cases {
        assert_eq!(
            try_into_forge_request(wire).ok(),
            Some(expected),
            "Forge extractor must preserve the complete payload for every Forge variant"
        );
    }
}

#[test]
fn non_forge_request_is_returned_unchanged() {
    let request = ClientRequestV1::BreakthroughRequest { v: 1 };
    assert!(matches!(
        try_into_forge_request(request),
        Err(ClientRequestV1::BreakthroughRequest { v: 1 })
    ));
}

#[test]
fn one_batch_step_advance_projection_allows_each_dependent_request() {
    let mut app = forge_dispatch_app();
    let player = app.world_mut().spawn(inscription_inventory()).id();

    let mut scroll_template = ItemTemplate::minimal_for_test("inscription_scroll_sharp");
    scroll_template.inscription_scroll_spec = Some(InscriptionScrollSpec {
        inscription_id: "sharp_v0".to_owned(),
    });
    let item_registry = ItemRegistry::from_map(HashMap::from([(
        "inscription_scroll_sharp".to_owned(),
        scroll_template,
    )]));
    app.insert_resource(item_registry);

    let mut sessions = ForgeSessions::new();
    sessions.insert(session(1, player, ForgeStep::Billet, 0));
    sessions.insert(session(2, player, ForgeStep::Tempering, 1));
    sessions.insert(session(3, player, ForgeStep::Inscription, 2));
    app.insert_resource(sessions);

    let mut batch = app.world_mut().resource_mut::<PendingForgeBatch>();
    batch.0 = vec![
        (player, ForgeRequest::StepAdvance { session_id: 1 }),
        (
            player,
            ForgeRequest::TemperingHit {
                session_id: 1,
                beat: "L".to_owned(),
                ticks_remaining: 1,
            },
        ),
        (player, ForgeRequest::StepAdvance { session_id: 2 }),
        (
            player,
            ForgeRequest::InscriptionScroll {
                session_id: 2,
                inscription_id: "sharp_v0".to_owned(),
            },
        ),
        (player, ForgeRequest::StepAdvance { session_id: 3 }),
        (
            player,
            ForgeRequest::ConsecrationInject {
                session_id: 3,
                qi_amount: 2.5,
            },
        ),
    ];
    app.update();

    let steps: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<StepAdvance>>()
        .drain()
        .collect();
    assert_eq!(
        steps.len(),
        3,
        "each valid advance must emit exactly one StepAdvance event"
    );
    assert_eq!(steps[0].session, ForgeSessionId(1));
    assert_eq!(steps[0].from_step, ForgeStep::Billet);
    assert_eq!(steps[1].session, ForgeSessionId(2));
    assert_eq!(steps[1].from_step, ForgeStep::Tempering);
    assert_eq!(steps[2].session, ForgeSessionId(3));
    assert_eq!(steps[2].from_step, ForgeStep::Inscription);

    let hits: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<TemperingHit>>()
        .drain()
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "projected Tempering state must admit the following hit"
    );
    assert_eq!(hits[0].session, ForgeSessionId(1));
    assert_eq!(hits[0].beat, TemperBeat::Light);
    assert_eq!(hits[0].ticks_remaining, 1);

    let inscriptions: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<InscriptionScrollSubmit>>()
        .drain()
        .collect();
    assert_eq!(
        inscriptions.len(),
        1,
        "projected Inscription state must admit the following exact scroll"
    );
    assert_eq!(inscriptions[0].session, ForgeSessionId(2));
    assert_eq!(inscriptions[0].caster, player);
    assert_eq!(inscriptions[0].item_instance_id, 7001);
    assert_eq!(inscriptions[0].inscription_id, "sharp_v0");

    let injections: Vec<_> = app
        .world_mut()
        .resource_mut::<Events<ConsecrationInject>>()
        .drain()
        .collect();
    assert_eq!(
        injections.len(),
        1,
        "projected Consecration state must admit the following injection"
    );
    assert_eq!(injections[0].session, ForgeSessionId(3));
    assert_eq!(injections[0].qi_amount, 2.5);
}

#[test]
fn next_step_projection_uses_checked_blueprint_order() {
    let registry = BlueprintRegistry::load_dir(format!(
        "{}/assets/forge/blueprints",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("the checked-in Forge blueprint registry must be readable");
    let blueprint = registry
        .get("ling_feng_v0")
        .expect("the projected-state contract uses the four-step ling_feng blueprint");
    assert_eq!(
        next_step_after(blueprint, 0),
        ForgeStep::Tempering,
        "StepAdvance projection must follow the blueprint's next typed step"
    );
    assert_eq!(
        next_step_after(blueprint, 1),
        ForgeStep::Inscription,
        "StepAdvance projection must preserve the inscription successor"
    );
    assert_eq!(
        next_step_after(blueprint, 2),
        ForgeStep::Consecration,
        "StepAdvance projection must preserve the consecration successor"
    );
}
