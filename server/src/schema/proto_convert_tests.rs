use crate::schema::proto_convert::{
    bong, cast_outcome_to_proto, inventory_item_view_to_proto, zhenfa_kind_to_proto,
};

use prost::Message;

#[test]
fn cast_outcome_reject_race_mismatch_maps_to_dedicated_proto_variant() {
    use crate::schema::combat_hud::CastOutcomeV1;
    let proto_value = cast_outcome_to_proto(&CastOutcomeV1::RejectRaceMismatch);
    assert_eq!(proto_value, bong::CastOutcome::RejectRaceMismatch as i32);
    // 不与其余已存在的 reject 变体撞值（防止手滑复制粘贴出重复映射）。
    assert_ne!(
        proto_value,
        bong::CastOutcome::RejectTechniqueInactive as i32
    );
    assert_ne!(proto_value, bong::CastOutcome::RejectNoWeapon as i32);
    assert_ne!(proto_value, bong::CastOutcome::Unspecified as i32);
}

fn alchemy_view(
    alchemy: Option<crate::inventory::AlchemyItemData>,
) -> crate::schema::inventory::InventoryItemViewV1 {
    crate::schema::inventory::InventoryItemViewV1 {
        instance_id: 7,
        item_id: "qing_xin_dan".to_string(),
        display_name: "清心丹".to_string(),
        grid_width: 1,
        grid_height: 1,
        weight: 0.1,
        rarity: crate::schema::inventory::ItemRarityV1::Uncommon,
        description: String::new(),
        stack_count: 1,
        spirit_quality: 1.0,
        durability: 1.0,
        freshness: None,
        freshness_current: None,
        mineral_id: None,
        scroll_kind: None,
        scroll_skill_id: None,
        scroll_xp_grant: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: vec![],
        forge_achieved_tier: None,
        alchemy,
        lingering_owner_qi: None,
    }
}

/// 把 InventoryItemViewV1 过真实 proto wire（encode→decode），返回 decode 后的 alchemy。

fn roundtrip_alchemy(
    view: &crate::schema::inventory::InventoryItemViewV1,
) -> Option<bong::AlchemyItemDataProto> {
    let proto = inventory_item_view_to_proto(view);
    let bytes = proto.encode_to_vec();
    let decoded = bong::InventoryItemView::decode(bytes.as_slice())
        .expect("InventoryItemView proto decode 应成功");
    decoded.alchemy
}

fn roundtrip_freshness(
    view: &crate::schema::inventory::InventoryItemViewV1,
) -> Option<bong::InventoryFreshness> {
    let proto = inventory_item_view_to_proto(view);
    let bytes = proto.encode_to_vec();
    let decoded = bong::InventoryItemView::decode(bytes.as_slice())
        .expect("InventoryItemView proto decode 应成功");
    decoded.freshness
}

#[test]
fn inventory_item_view_freshness_survives_proto_roundtrip_for_all_tracks() {
    use crate::shelflife::{DecayProfileId, DecayTrack, Freshness};

    let initial_qi = f32::from_bits(0x42c5_0001);
    for (track, expected_track) in [
        (DecayTrack::Decay, "Decay"),
        (DecayTrack::Spoil, "Spoil"),
        (DecayTrack::Age, "Age"),
    ] {
        let mut view = alchemy_view(None);
        view.freshness = Some(Freshness {
            created_at_tick: 123,
            initial_qi,
            track,
            profile: DecayProfileId::new(format!("{}_profile", expected_track.to_lowercase())),
            frozen_accumulated: 17,
            frozen_since_tick: Some(140),
        });

        let freshness = roundtrip_freshness(&view)
            .expect("freshness 必须过 InventoryItemView protobuf wire 存活");
        assert_eq!(freshness.created_at_tick, 123);
        assert_eq!(
            freshness.initial_qi.to_bits(),
            initial_qi.to_bits(),
            "Freshness.initial_qi 的权威类型是 f32，protobuf float 必须逐 bit 保真"
        );
        assert_eq!(freshness.track, expected_track);
        assert_eq!(
            freshness.profile,
            format!("{}_profile", expected_track.to_lowercase())
        );
        assert_eq!(freshness.frozen_accumulated, 17);
        assert_eq!(freshness.frozen_since_tick, Some(140));
    }
}

#[test]
fn inventory_item_view_without_freshness_keeps_proto_field_absent() {
    assert!(roundtrip_freshness(&alchemy_view(None)).is_none());
}

#[test]
fn inventory_item_view_pill_alchemy_survives_proto_roundtrip() {
    use crate::alchemy::recipe::SideEffect;
    use crate::inventory::AlchemyItemData;

    let view = alchemy_view(Some(AlchemyItemData::Pill {
        recipe_id: "qing_xin_dan".to_string(),
        quality_tier: 2,
        effect_multiplier: 0.9,
        consecrated: true,
        side_effect: Some(SideEffect {
            tag: "qi_drain_mild".to_string(),
            duration_s: 30,
            weight: 1,
            perm: false,
            color: None,
            amount: Some(1.5),
        }),
    }));
    let a = roundtrip_alchemy(&view)
        .expect("alchemy 必须过 proto wire 存活（修复前 inventory_item_view_to_proto 不搬→None）");
    assert_eq!(
            a.kind, "pill",
            "判别键 kind 必须是 \"pill\"，否则 client readAlchemyLines switch 落 default→整条 item parse 返 null"
        );
    assert_eq!(
        a.recipe_id.as_deref(),
        Some("qing_xin_dan"),
        "recipe_id 必须保真（client pillAlchemyLines readRequiredString 读它）"
    );
    assert_eq!(
        a.quality_tier,
        Some(2),
        "quality_tier 必须保真（client 渲染「丹药 2阶」），实际 {:?}",
        a.quality_tier
    );
    assert_eq!(
        a.effect_multiplier,
        Some(0.9),
        "effect_multiplier 必须保真（client 渲染「效力 90%」）"
    );
    assert_eq!(
        a.consecrated,
        Some(true),
        "consecrated 必须保真（client 渲染「开光 · 持续翻倍」）"
    );
    let se = a
        .side_effect
        .expect("side_effect 必须过 wire 存活（client 渲染「副作用 ...」）");
    assert_eq!(
        se.tag, "qi_drain_mild",
        "side_effect.tag 必须保真（client 唯一消费字段），实际 {:?}",
        se.tag
    );
    assert_eq!(se.duration_s, Some(30), "side_effect.duration_s 全镜像保真");
    assert_eq!(se.amount, Some(1.5), "side_effect.amount 全镜像保真");
}

#[test]
fn inventory_item_view_recipe_fragment_alchemy_survives_proto_roundtrip() {
    use crate::alchemy::recipe_fragment::RecipeFragment;
    use crate::inventory::AlchemyItemData;

    let view = alchemy_view(Some(AlchemyItemData::RecipeFragment {
        fragment: RecipeFragment {
            recipe_id: "huang_long_dan".to_string(),
            known_stages: vec![0, 2],
            max_quality_tier: 3,
        },
    }));
    let a = roundtrip_alchemy(&view).expect("recipe_fragment alchemy 必须过 proto wire 存活");
    assert_eq!(
        a.kind, "recipe_fragment",
        "kind 必须是 \"recipe_fragment\"，否则 client fragmentAlchemyLines 分支取不到"
    );
    let f = a
        .fragment
        .expect("fragment 子结构必须过 wire 存活（client readRequiredObject(\"fragment\")）");
    assert_eq!(
        f.recipe_id, "huang_long_dan",
        "fragment.recipe_id 必须保真（client 渲染「丹方残卷 ...」），实际 {:?}",
        f.recipe_id
    );
    assert_eq!(
        f.known_stages,
        vec![0u32, 2u32],
        "known_stages 必须保真（client 渲染「已知 N段」用 .size()），实际 {:?}",
        f.known_stages
    );
    assert_eq!(
        f.max_quality_tier, 3,
        "max_quality_tier 必须保真（client 渲染「上限M阶」，且 client 校验 1..=3）"
    );
}

#[test]
fn inventory_item_view_recipe_hint_alchemy_survives_proto_roundtrip() {
    use crate::alchemy::danxin::RecipeHint;
    use crate::inventory::AlchemyItemData;

    let view = alchemy_view(Some(AlchemyItemData::RecipeHint {
        hint: RecipeHint {
            source_pill: "qing_xin_dan".to_string(),
            recipe_id: Some("qing_xin_dan".to_string()),
            accuracy: 0.75,
            ingredients: vec!["ci_she_hao".to_string(), "ning_mai_cao".to_string()],
        },
    }));
    let a = roundtrip_alchemy(&view).expect("recipe_hint alchemy 必须过 proto wire 存活");
    assert_eq!(a.kind, "recipe_hint", "kind 必须是 \"recipe_hint\"");
    let h = a
        .hint
        .expect("hint 子结构必须过 wire 存活（client readRequiredObject(\"hint\")）");
    assert_eq!(
        h.source_pill, "qing_xin_dan",
        "hint.source_pill 必须保真（client 必填校验它），实际 {:?}",
        h.source_pill
    );
    assert_eq!(
        h.accuracy, 0.75,
        "hint.accuracy 必须保真（client 渲染「准度 75%」，校验 0..=1）"
    );
    assert_eq!(
        h.ingredients,
        vec!["ci_she_hao".to_string(), "ning_mai_cao".to_string()],
        "ingredients 必须保真（client 渲染「药痕 a/b」），实际 {:?}",
        h.ingredients
    );
}

#[test]
fn inventory_item_view_pill_residue_alchemy_survives_proto_roundtrip() {
    use crate::alchemy::residue::PillResidueKind;
    use crate::inventory::AlchemyItemData;

    let view = alchemy_view(Some(AlchemyItemData::PillResidue {
        residue_kind: PillResidueKind::FailedPill,
        produced_at_tick: 100,
        expires_at_tick: 500,
    }));
    let a = roundtrip_alchemy(&view).expect("pill_residue alchemy 必须过 proto wire 存活");
    assert_eq!(a.kind, "pill_residue", "kind 必须是 \"pill_residue\"");
    // residue_kind 字符串必须对齐 serde snake_case（FailedPill→\"failed_pill\"），
    // 否则将来接 client tooltip 时会再漂一次。
    assert_eq!(
        a.residue_kind.as_deref(),
        Some("failed_pill"),
        "residue_kind 必须是 serde snake_case \"failed_pill\"（与 JSON 路径一致），实际 {:?}",
        a.residue_kind
    );
    assert_eq!(a.produced_at_tick, Some(100), "produced_at_tick 全镜像保真");
    assert_eq!(a.expires_at_tick, Some(500), "expires_at_tick 全镜像保真");
}

#[test]
fn inventory_item_view_no_alchemy_absent_in_proto_wire() {
    // 守恒反向 case：非丹药物品 alchemy=None → proto 不设 → wire 不携带 →
    // client readAlchemyLines element==null → List.of()（无 tooltip 行），语义与 serde Option 一致。
    let view = alchemy_view(None);
    assert!(
        roundtrip_alchemy(&view).is_none(),
        "alchemy=None 时 proto wire 不应携带 alchemy（缺 presence）；\
             若 Some 说明误塞默认值→client 可能误判 kind"
    );
}

/// 整快照端到端：把带 alchemy 的 item 塞进 placed_items，过
/// ServerDataEnvelope::encode→decode 全链，确认 InventorySnapshot payload 不丢 alchemy。
/// 锁死「生产 InventorySnapshot 过线时 item.alchemy 仍在」。

#[test]
fn c2s_zhenfa_kind_proto_pins_include_trap_runtime_variants() {
    use crate::zhenfa::ZhenfaKind;
    let cases = [
        (ZhenfaKind::Trap, bong::ZhenfaKind::Trap),
        (ZhenfaKind::Ward, bong::ZhenfaKind::Ward),
        (ZhenfaKind::WarningTrap, bong::ZhenfaKind::WarningTrap),
        (ZhenfaKind::BlastTrap, bong::ZhenfaKind::BlastTrap),
        (ZhenfaKind::SlowTrap, bong::ZhenfaKind::SlowTrap),
        (ZhenfaKind::ShrineWard, bong::ZhenfaKind::ShrineWard),
        (ZhenfaKind::Lingju, bong::ZhenfaKind::Lingju),
        (ZhenfaKind::DeceiveHeaven, bong::ZhenfaKind::DeceiveHeaven),
        (ZhenfaKind::Illusion, bong::ZhenfaKind::Illusion),
        (ZhenfaKind::NetworkArray, bong::ZhenfaKind::NetworkArray),
        (ZhenfaKind::BeastTrap, bong::ZhenfaKind::BeastTrap),
        (ZhenfaKind::TripWire, bong::ZhenfaKind::TripWire),
        (ZhenfaKind::DecoyStake, bong::ZhenfaKind::DecoyStake),
    ];
    for (rust, proto) in cases {
        assert_eq!(
            zhenfa_kind_to_proto(&rust),
            proto as i32,
            "ZhenfaKind::{rust:?} must map to protobuf {proto:?}"
        );
        assert_ne!(
            zhenfa_kind_to_proto(&rust),
            bong::ZhenfaKind::Unspecified as i32,
            "ZhenfaKind::{rust:?} must not map to UNSPECIFIED"
        );
    }
    assert_eq!(
        cases.len(),
        13,
        "ZhenfaKind proto coverage must include the 13 P0 variants"
    );
}
