//! skillbar_config 发射契约测试。
//!
//! plan-skill-av-relink-v1 P3（图标链）追加：Skill 槽下发的 icon_texture 必须严格
//! 等于 `TechniqueRegistry` 中该 id 的 icon_texture 且非空；Item 槽 icon_texture 恒空串
//! （P0 显式契约：client 按 template_id 走 ItemIconRegistry 富解析）；未知
//! technique id / 解析不到的 Item instance / 无 inventory 一律下发 None 槽。

use std::collections::HashMap;

use valence::prelude::{App, Update};
use valence::protocol::packets::play::CustomPayloadS2c;
use valence::testing::create_mock_client;

use super::skillbar_config_emit::emit_skillbar_config_payloads;
use crate::combat::components::{SkillBarBindings, SkillSlot};
use crate::combat::CombatClock;
use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::inventory::{
    ContainerState, InventoryRevision, ItemCategory, ItemInstance, ItemRarity, ItemRegistry,
    ItemTemplate, PlacedItemState, PlayerInventory,
};
use crate::network::agent_bridge::SERVER_DATA_CHANNEL;
use crate::schema::combat_hud::{SkillBarConfigV1, SkillBarEntryV1};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

fn empty_inventory() -> PlayerInventory {
    PlayerInventory {
        triggered_treasures: Vec::new(),
        revision: InventoryRevision(0),
        containers: vec![ContainerState {
            quick_access: false,
            id: "main_pack".to_string(),
            name: "main_pack".to_string(),
            rows: 5,
            cols: 7,
            items: Vec::new(),
            owner_instance_id: None,
        }],
        equipped: Default::default(),
        hotbar: Default::default(),
        bone_coins: 0,
        max_weight: 50.0,
    }
}

fn template(id: &str, display_name: &str) -> ItemTemplate {
    ItemTemplate {
        id: id.to_string(),
        display_name: display_name.to_string(),
        category: ItemCategory::Pill,
        placeable: None,
        max_stack_count: 1,
        grid_w: 1,
        grid_h: 1,
        base_weight: 0.1,
        rarity: ItemRarity::Common,
        spirit_quality_initial: 1.0,
        description: String::new(),
        effect: None,
        cast_duration_ms: 250,
        cooldown_ms: 1250,
        weapon_spec: None,
        forge_station_spec: None,
        blueprint_scroll_spec: None,
        inscription_scroll_spec: None,
        technique_scroll_spec: None,
        readable_scroll_spec: None,
        recipe_fragment_spec: None,
        container_spec: None,
        shelflife_profile: None,
        shield_spec: None,
        shelflife_track: None,
        wearer_race: crate::body_plan::types::RaceGateOwned::default(),
    }
}

fn item_instance(instance_id: u64, template_id: &str) -> ItemInstance {
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

/// 跑一轮 emit 系统并收取该 client 收到的 skillbar_config payload。
/// `inventory` 传 `None` 时不挂 `PlayerInventory` component（覆盖 Option 兜底分支）。
fn emit_and_collect_skillbar_config(
    registry: ItemRegistry,
    tick: u64,
    bindings: SkillBarBindings,
    inventory: Option<PlayerInventory>,
) -> SkillBarConfigV1 {
    let mut app = App::new();
    app.insert_resource(TechniqueRegistry::load_for_tests());
    app.insert_resource(CombatClock { tick });
    app.insert_resource(registry);
    app.add_systems(Update, emit_skillbar_config_payloads);

    let (client_bundle, mut helper) = create_mock_client("Azure");
    match inventory {
        Some(inventory) => {
            app.world_mut().spawn((client_bundle, bindings, inventory));
        }
        None => {
            app.world_mut().spawn((client_bundle, bindings));
        }
    }

    app.update();
    let mut client_query = app.world_mut().query::<&mut valence::prelude::Client>();
    for mut client in client_query.iter_mut(app.world_mut()) {
        client
            .flush_packets()
            .expect("mock client packets should flush");
    }

    helper
        .collect_received()
        .0
        .into_iter()
        .find_map(|frame| {
            let packet = frame.decode::<CustomPayloadS2c>().ok()?;
            if packet.channel.as_str() != SERVER_DATA_CHANNEL {
                return None;
            }
            let payload: ServerDataV1 = serde_json::from_slice(packet.data.0 .0)
                .expect("server_data payload should decode");
            match payload.payload {
                ServerDataPayloadV1::SkillBarConfig(config) => Some(config),
                _ => None,
            }
        })
        .expect("skillbar_config should be emitted")
}

#[test]
fn skillbar_config_emit_serializes_skill_item_and_cooldown() {
    let mut inventory = empty_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_instance(42, "tea"),
    });
    let mut bindings = SkillBarBindings::default();
    assert!(bindings.set(
        0,
        SkillSlot::Skill {
            skill_id: "burst_meridian.beng_quan".to_string(),
        },
    ));
    assert!(bindings.set(1, SkillSlot::Item { instance_id: 42 }));
    bindings.set_cooldown("burst_meridian.beng_quan", 70);

    let skillbar = emit_and_collect_skillbar_config(
        ItemRegistry::from_map(HashMap::from([(
            "tea".to_string(),
            template("tea", "清茶"),
        )])),
        10,
        bindings,
        Some(inventory),
    );

    let technique_registry = TechniqueRegistry::load_for_tests();
    let expected_icon = technique_registry
        .get("burst_meridian.beng_quan")
        .expect("burst_meridian.beng_quan must be registered")
        .icon_texture
        .as_str();
    assert_eq!(skillbar.slots.len(), 9);
    assert!(matches!(
        &skillbar.slots[0],
        Some(SkillBarEntryV1::Skill { skill_id, display_name, cast_duration_ms, cooldown_ms, icon_texture })
            if skill_id == "burst_meridian.beng_quan"
                && display_name == "崩拳"
                && *cast_duration_ms == 400
                && *cooldown_ms == 3000
                && icon_texture == expected_icon
    ));
    assert!(matches!(
        &skillbar.slots[1],
        Some(SkillBarEntryV1::Item { template_id, display_name, cast_duration_ms, cooldown_ms, icon_texture })
            if template_id == "tea"
                && display_name == "清茶"
                && *cast_duration_ms == 250
                && *cooldown_ms == 1250
                && icon_texture.is_empty()
    ));
    assert!(skillbar.cooldown_until_ms[0] > 0);
}

/// bughunt skillbar-rebind-cooldown-reset —— 冷却按 skill_id 记账后，同一招式绑在
/// 两个槽位必须在 wire payload 里显示**相同**的非零冷却（旧的按槽位数组实现下，
/// 只有真正 cast 过的那个槽会显示冷却，另一个槽恒 0——那正是"绑多槽绕过冷却"的
/// 可利用信号）。同时验证 Item 槽即便与某个恰好同名的 skill_id 冷却 entry 共存，
/// 也恒报 0（Item 槽从不查 cooldowns map）。
#[test]
fn skillbar_config_emit_same_skill_bound_to_two_slots_reports_identical_cooldown() {
    let mut bindings = SkillBarBindings::default();
    assert!(bindings.set(
        0,
        SkillSlot::Skill {
            skill_id: "burst_meridian.beng_quan".to_string(),
        },
    ));
    assert!(bindings.set(
        4,
        SkillSlot::Skill {
            skill_id: "burst_meridian.beng_quan".to_string(),
        },
    ));
    assert!(bindings.set(8, SkillSlot::Item { instance_id: 42 }));
    bindings.set_cooldown("burst_meridian.beng_quan", 70);

    let mut inventory = empty_inventory();
    inventory.containers[0].items.push(PlacedItemState {
        row: 0,
        col: 0,
        instance: item_instance(42, "tea"),
    });

    let skillbar = emit_and_collect_skillbar_config(
        ItemRegistry::from_map(HashMap::from([(
            "tea".to_string(),
            template("tea", "清茶"),
        )])),
        10,
        bindings,
        Some(inventory),
    );

    assert!(
        skillbar.cooldown_until_ms[0] > 0,
        "槽 0（beng_quan）应显示冷却"
    );
    assert_eq!(
        skillbar.cooldown_until_ms[0], skillbar.cooldown_until_ms[4],
        "同一 skill_id 绑在槽 0 和槽 4，两槽下发的 cooldown_until_ms 必须完全一致——\
         按 slot 记账的旧实现下这里会是 0（漏了另一槽的冷却），恰是「绑多槽绕过冷却」的信号"
    );
    assert_eq!(
        skillbar.cooldown_until_ms[8], 0,
        "Item 槽恒报 0，不查 cooldowns map（即便有同名 skill_id 冷却 entry 存在）"
    );
}

/// plan-skill-av-relink-v1 P3 —— 全部 technique 逐条对拍：Skill 槽下发的
/// icon_texture 必须严格等于 `TechniqueRegistry` 对应定义且非空。
/// SkillBar 只有 9 槽，按 9 条一批分批绑定跑 emit，保证 49 条全覆盖。
#[test]
fn skillbar_skill_slots_emit_definition_icon_texture_for_every_technique() {
    let technique_registry = TechniqueRegistry::load_for_tests();
    let definitions: Vec<_> = technique_registry.iter().collect();
    let mut covered = 0usize;
    for chunk in definitions.chunks(SkillBarBindings::SLOT_COUNT) {
        let mut bindings = SkillBarBindings::default();
        for (slot, def) in chunk.iter().enumerate() {
            assert!(bindings.set(
                slot as u8,
                SkillSlot::Skill {
                    skill_id: def.id.to_string(),
                },
            ));
        }
        let config = emit_and_collect_skillbar_config(
            ItemRegistry::from_map(HashMap::new()),
            0,
            bindings,
            Some(empty_inventory()),
        );
        assert_eq!(
            config.slots.len(),
            SkillBarBindings::SLOT_COUNT,
            "skillbar_config.slots 长度必须恒为 {}（client 按 idx 取）",
            SkillBarBindings::SLOT_COUNT
        );
        for (slot, def) in chunk.iter().enumerate() {
            match &config.slots[slot] {
                Some(SkillBarEntryV1::Skill {
                    skill_id,
                    icon_texture,
                    ..
                }) => {
                    assert_eq!(
                        skill_id.as_str(),
                        def.id,
                        "槽 {slot} 应回发绑定的 technique id"
                    );
                    assert_eq!(
                        icon_texture.as_str(),
                        def.icon_texture,
                        "Skill 槽 `{}` 下发的 icon_texture 必须严格等于 \
                         TechniqueRegistry 定义值 `{}`——漂移意味着 emit 路径没走 registry",
                        def.id,
                        def.icon_texture
                    );
                    assert!(
                        !icon_texture.is_empty(),
                        "Skill 槽 `{}` 的 icon_texture 不得为空串（HUD 无图标可渲染）",
                        def.id
                    );
                }
                other => panic!(
                    "槽 {slot} 绑定了已注册 technique `{}`，应下发 Skill 条目，实际 {other:?}",
                    def.id
                ),
            }
        }
        for slot in chunk.len()..SkillBarBindings::SLOT_COUNT {
            assert!(
                config.slots[slot].is_none(),
                "未绑定的槽 {slot} 应下发 None，实际 {:?}",
                config.slots[slot]
            );
        }
        covered += chunk.len();
    }
    assert_eq!(
        covered,
        definitions.len(),
        "分批循环必须遍历全部 technique 定义"
    );
}

/// 未知 technique id / 背包解析不到的 Item instance → 槽位下发 None（不发条目、
/// 不 panic、不发占位）。
#[test]
fn skillbar_unknown_skill_id_and_unresolvable_item_emit_none() {
    let mut bindings = SkillBarBindings::default();
    assert!(bindings.set(
        0,
        SkillSlot::Skill {
            skill_id: "no.such_technique".to_string(),
        },
    ));
    assert!(bindings.set(1, SkillSlot::Item { instance_id: 9999 }));

    let config = emit_and_collect_skillbar_config(
        ItemRegistry::from_map(HashMap::new()),
        0,
        bindings,
        Some(empty_inventory()),
    );

    assert!(
        config.slots[0].is_none(),
        "未注册的 technique id 不应下发条目（TechniqueRegistry 返回 None），实际 {:?}",
        config.slots[0]
    );
    assert!(
        config.slots[1].is_none(),
        "背包/装备/hotbar 均解析不到的 Item instance 不应下发条目，实际 {:?}",
        config.slots[1]
    );
    assert!(
        config.slots.iter().all(Option::is_none),
        "其余空槽也应全部为 None"
    );
}

/// Item 槽绑定存在但玩家实体没挂 `PlayerInventory` component → 槽位下发 None
/// （emit 系统对 Option<&PlayerInventory> 的兜底分支）。
#[test]
fn skillbar_item_slot_without_inventory_component_emits_none() {
    let mut bindings = SkillBarBindings::default();
    assert!(bindings.set(0, SkillSlot::Item { instance_id: 42 }));

    let config = emit_and_collect_skillbar_config(
        ItemRegistry::from_map(HashMap::from([(
            "tea".to_string(),
            template("tea", "清茶"),
        )])),
        0,
        bindings,
        None,
    );

    assert!(
        config.slots[0].is_none(),
        "无 PlayerInventory component 时 Item 槽应下发 None，实际 {:?}",
        config.slots[0]
    );
}
