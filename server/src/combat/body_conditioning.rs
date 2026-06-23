//! 广播体操功法（`body.guangbo_ticao`）：proficiency 0→1 线性缩放三类增益
//! （+5% 移速 / +5% 跳跃 / +0.5% 四肢防御），每次施放递减增长 proficiency。
//! `body_conditioning_aggregate` 在 Physics 阶段读 `KnownTechniques` 写入 `DerivedAttrs`。

use valence::prelude::{bevy_ecs, Entity, Event, EventReader, Events, Position, Query, ResMut};

use crate::combat::armor::ARMOR_MITIGATION_CAP;
use crate::combat::components::{BodyPart, DerivedAttrs, WoundKind};
use crate::cultivation::components::Cultivation;
use crate::cultivation::death_hooks::release_qi_amount_to_zone;
use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};
use crate::cultivation::life_record::LifeRecord;
use crate::qi_physics::QiTransfer;
use crate::world::dimension::CurrentDimension;
use crate::world::zone::ZoneRegistry;

pub const GUANGBO_TICAO_ID: &str = "body.guangbo_ticao";

/// 每次成功练习消耗的真元（守恒：proficiency 增长不得凭空——每 rep 付真元代价）。
/// 与 `known_techniques.body.guangbo_ticao.qi_cost` 保持一致；改一处必须改另一处，
/// `qi_cost_matches_known_technique_definition` 测试会撞红。
pub const GUANGBO_TICAO_QI_COST: f64 = 1.0;

const MOVE_SPEED_BONUS_MAX: f32 = 0.05;
const JUMP_HEIGHT_BONUS_MAX: f32 = 0.05;
const LIMB_DEFENSE_BONUS_MAX: f32 = 0.005;

const LIMB_PARTS: [BodyPart; 4] = [
    BodyPart::ArmL,
    BodyPart::ArmR,
    BodyPart::LegL,
    BodyPart::LegR,
];

const ALL_WOUND_KINDS: [WoundKind; 5] = [
    WoundKind::Cut,
    WoundKind::Blunt,
    WoundKind::Pierce,
    WoundKind::Burn,
    WoundKind::Concussion,
];

pub fn guangbo_ticao_move_speed(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * MOVE_SPEED_BONUS_MAX
}

pub fn guangbo_ticao_jump_height(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * JUMP_HEIGHT_BONUS_MAX
}

pub fn guangbo_ticao_limb_defense(proficiency: f32) -> f32 {
    proficiency.clamp(0.0, 1.0) * LIMB_DEFENSE_BONUS_MAX
}

#[derive(Debug, Clone, Event)]
pub struct GuangboTicaoPracticeEvent {
    pub entity: Entity,
}

pub fn guangbo_proficiency_gain(current: f32) -> f32 {
    if current < 0.5 {
        0.01
    } else {
        0.005
    }
}

pub fn record_guangbo_practice(known: &mut KnownTechniques) -> f32 {
    let entry = ensure_entry(known);
    let gain = guangbo_proficiency_gain(entry.proficiency);
    entry.proficiency = (entry.proficiency + gain).clamp(0.0, 1.0);
    entry.proficiency
}

/// 一次练习的结算结果——`tick_casts_or_interrupt` 据此决定是否发 AV / 日志。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PracticeOutcome {
    /// 付清真元代价，proficiency 已递增；携带递增后的 proficiency 值。
    Trained { proficiency: f32 },
    /// 真元不足，本次练习无效——不扣真元、不涨 proficiency（守恒：进度必须付费）。
    TooTiredNoQi,
}

/// 守恒入口：真元充足时扣 `GUANGBO_TICAO_QI_COST` 并递增 proficiency；
/// 不足则拒绝（既不扣费也不涨熟练度）。proficiency 仍受 `record_guangbo_practice`
/// 的 clamp(0,1) + 递减增长上限约束，故总收益有界、增长需持续付费。
pub fn try_record_guangbo_practice(
    cultivation: &mut Cultivation,
    known: &mut KnownTechniques,
) -> PracticeOutcome {
    if cultivation.qi_current < GUANGBO_TICAO_QI_COST {
        return PracticeOutcome::TooTiredNoQi;
    }
    // 扣费后绝不允许为负（防御性 clamp，理论上前置判定已保证 >= cost）。
    cultivation.qi_current = (cultivation.qi_current - GUANGBO_TICAO_QI_COST).max(0.0);
    let proficiency = record_guangbo_practice(known);
    PracticeOutcome::Trained { proficiency }
}

fn ensure_entry(known: &mut KnownTechniques) -> &mut KnownTechnique {
    if let Some(idx) = known.entries.iter().position(|e| e.id == GUANGBO_TICAO_ID) {
        return &mut known.entries[idx];
    }
    known.entries.push(KnownTechnique {
        id: GUANGBO_TICAO_ID.to_string(),
        proficiency: 0.0,
        active: true,
    });
    known.entries.last_mut().expect("entry was just inserted")
}

/// 消费 `GuangboTicaoPracticeEvent`：每次成功练习扣真元 + 递增 proficiency，
/// 并将消耗的真元回灌至当前区域（守恒：player_qi 减少必须对应 zone_qi 增加）。
///
/// 守恒：通过 [`try_record_guangbo_practice`] 走真元门——真元不足则本次练习无效
/// （不扣费、不涨熟练度）。无 `Cultivation` 组件的实体（理论上玩家恒有）按"无真元
/// 可付"处理，同样不涨 proficiency，避免凭空增益。
#[allow(clippy::too_many_arguments)]
pub fn consume_guangbo_practice_events(
    mut events: EventReader<GuangboTicaoPracticeEvent>,
    mut q: Query<(&mut KnownTechniques, Option<&mut Cultivation>)>,
    locations: Query<(
        Option<&Position>,
        Option<&CurrentDimension>,
        Option<&LifeRecord>,
    )>,
    mut zones: Option<ResMut<ZoneRegistry>>,
    mut qi_transfers: Option<ResMut<Events<QiTransfer>>>,
) {
    for event in events.read() {
        let Ok((mut known, cultivation)) = q.get_mut(event.entity) else {
            continue;
        };
        let Some(mut cultivation) = cultivation else {
            // 无修为组件 → 无真元可付 → 守恒下不给 proficiency。
            continue;
        };
        let outcome = try_record_guangbo_practice(&mut cultivation, &mut known);
        if matches!(outcome, PracticeOutcome::Trained { .. }) {
            // 守恒：扣除的真元必须回灌区域，否则 world qi ledger 产生永久漏洞。
            let (position, current_dimension, life_record) = locations
                .get(event.entity)
                .map(|(p, d, l)| (p, d, l))
                .unwrap_or((None, None, None));
            release_qi_amount_to_zone(
                event.entity,
                GUANGBO_TICAO_QI_COST,
                position,
                current_dimension,
                life_record,
                zones.as_deref_mut(),
                qi_transfers.as_deref_mut(),
                "guangbo_ticao",
            );
        }
    }
}

pub fn apply_guangbo_ticao_bonuses(attrs: &mut DerivedAttrs, known: &KnownTechniques) {
    let Some(entry) = known.entries.iter().find(|e| e.id == GUANGBO_TICAO_ID) else {
        return;
    };
    if !entry.active {
        return;
    }

    let prof = entry.proficiency;
    attrs.move_speed_multiplier *= 1.0 + guangbo_ticao_move_speed(prof);
    attrs.jump_height_multiplier *= 1.0 + guangbo_ticao_jump_height(prof);

    let limb_def = guangbo_ticao_limb_defense(prof);
    if limb_def > 0.0 {
        for &part in &LIMB_PARTS {
            for &kind in &ALL_WOUND_KINDS {
                attrs
                    .defense_profile
                    .entry((part, kind))
                    .and_modify(|v| *v = (*v + limb_def).min(ARMOR_MITIGATION_CAP))
                    .or_insert(limb_def);
            }
        }
    }
}

pub fn body_conditioning_aggregate(mut q: Query<(&mut DerivedAttrs, Option<&KnownTechniques>)>) {
    for (mut attrs, known) in &mut q {
        let Some(known) = known else { continue };
        apply_guangbo_ticao_bonuses(&mut attrs, known);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_near(actual: f32, expected: f32, label: &str) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "{label}: expected {expected}, actual {actual} — check {label} scaling"
        );
    }

    fn make_known(proficiency: f32, active: bool) -> KnownTechniques {
        KnownTechniques {
            entries: vec![KnownTechnique {
                id: GUANGBO_TICAO_ID.to_string(),
                proficiency,
                active,
            }],
        }
    }

    #[test]
    fn zero_proficiency_gives_no_bonus() {
        assert_near(
            guangbo_ticao_move_speed(0.0),
            0.0,
            "guangbo_ticao_move_speed(0)",
        );
        assert_near(
            guangbo_ticao_jump_height(0.0),
            0.0,
            "guangbo_ticao_jump_height(0)",
        );
        assert_near(
            guangbo_ticao_limb_defense(0.0),
            0.0,
            "guangbo_ticao_limb_defense(0)",
        );
    }

    #[test]
    fn half_proficiency_gives_half_bonus() {
        assert_near(
            guangbo_ticao_move_speed(0.5),
            0.025,
            "guangbo_ticao_move_speed(0.5)",
        );
        assert_near(
            guangbo_ticao_jump_height(0.5),
            0.025,
            "guangbo_ticao_jump_height(0.5)",
        );
        assert_near(
            guangbo_ticao_limb_defense(0.5),
            0.0025,
            "guangbo_ticao_limb_defense(0.5)",
        );
    }

    #[test]
    fn full_proficiency_gives_max_bonus() {
        assert_near(
            guangbo_ticao_move_speed(1.0),
            MOVE_SPEED_BONUS_MAX,
            "guangbo_ticao_move_speed(1.0) vs MOVE_SPEED_BONUS_MAX",
        );
        assert_near(
            guangbo_ticao_jump_height(1.0),
            JUMP_HEIGHT_BONUS_MAX,
            "guangbo_ticao_jump_height(1.0) vs JUMP_HEIGHT_BONUS_MAX",
        );
        assert_near(
            guangbo_ticao_limb_defense(1.0),
            LIMB_DEFENSE_BONUS_MAX,
            "guangbo_ticao_limb_defense(1.0) vs LIMB_DEFENSE_BONUS_MAX",
        );
    }

    #[test]
    fn proficiency_clamped_above_one() {
        assert_eq!(
            guangbo_ticao_move_speed(1.5),
            MOVE_SPEED_BONUS_MAX,
            "proficiency >1 should clamp to MOVE_SPEED_BONUS_MAX"
        );
        assert_eq!(
            guangbo_ticao_jump_height(2.0),
            JUMP_HEIGHT_BONUS_MAX,
            "proficiency >1 should clamp to JUMP_HEIGHT_BONUS_MAX"
        );
    }

    #[test]
    fn proficiency_clamped_below_zero() {
        assert_eq!(
            guangbo_ticao_move_speed(-0.5),
            0.0,
            "negative proficiency should clamp to 0"
        );
    }

    #[test]
    fn aggregate_applies_all_bonuses_at_full_proficiency() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_near(
            attrs.move_speed_multiplier,
            1.0 + MOVE_SPEED_BONUS_MAX,
            "move_speed after full-prof aggregate",
        );
        assert_near(
            attrs.jump_height_multiplier,
            1.0 + JUMP_HEIGHT_BONUS_MAX,
            "jump_height after full-prof aggregate",
        );
        assert_eq!(
            attrs.defense_profile.len(),
            20,
            "expected 4 limbs × 5 wound kinds = 20 defense_profile entries"
        );
        for &part in &LIMB_PARTS {
            for &kind in &ALL_WOUND_KINDS {
                let v = attrs.defense_profile[&(part, kind)];
                assert_near(v, LIMB_DEFENSE_BONUS_MAX, "defense_profile limb entry");
            }
        }
        assert!(
            !attrs
                .defense_profile
                .contains_key(&(BodyPart::Head, WoundKind::Cut)),
            "Head should not get limb defense"
        );
        assert!(
            !attrs
                .defense_profile
                .contains_key(&(BodyPart::Chest, WoundKind::Blunt)),
            "Chest should not get limb defense"
        );
        assert!(
            !attrs
                .defense_profile
                .contains_key(&(BodyPart::Abdomen, WoundKind::Pierce)),
            "Abdomen should not get limb defense"
        );
    }

    #[test]
    fn aggregate_stacks_with_existing_armor() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        attrs
            .defense_profile
            .insert((BodyPart::ArmL, WoundKind::Cut), 0.3);
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        let expected = 0.3 + LIMB_DEFENSE_BONUS_MAX;
        assert_near(
            attrs.defense_profile[&(BodyPart::ArmL, WoundKind::Cut)],
            expected,
            "armor 0.3 + LIMB_DEFENSE_BONUS_MAX stacking",
        );
    }

    #[test]
    fn aggregate_respects_mitigation_cap() {
        let known = make_known(1.0, true);
        let mut attrs = DerivedAttrs::default();
        attrs
            .defense_profile
            .insert((BodyPart::LegR, WoundKind::Blunt), ARMOR_MITIGATION_CAP);
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.defense_profile[&(BodyPart::LegR, WoundKind::Blunt)],
            ARMOR_MITIGATION_CAP,
            "defense_profile should not exceed ARMOR_MITIGATION_CAP ({ARMOR_MITIGATION_CAP})"
        );
    }

    #[test]
    fn inactive_technique_leaves_attrs_unchanged() {
        let known = make_known(1.0, false);
        let mut attrs = DerivedAttrs::default();
        let before_speed = attrs.move_speed_multiplier;
        let before_jump = attrs.jump_height_multiplier;
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.move_speed_multiplier, before_speed,
            "inactive technique should not change move_speed_multiplier"
        );
        assert_eq!(
            attrs.jump_height_multiplier, before_jump,
            "inactive technique should not change jump_height_multiplier"
        );
        assert!(
            attrs.defense_profile.is_empty(),
            "inactive technique should not add defense_profile entries"
        );
    }

    #[test]
    fn unknown_technique_leaves_attrs_unchanged() {
        let known = KnownTechniques { entries: vec![] };
        let mut attrs = DerivedAttrs::default();
        let before_speed = attrs.move_speed_multiplier;
        let before_jump = attrs.jump_height_multiplier;
        apply_guangbo_ticao_bonuses(&mut attrs, &known);

        assert_eq!(
            attrs.move_speed_multiplier, before_speed,
            "empty KnownTechniques should not change move_speed_multiplier"
        );
        assert_eq!(
            attrs.jump_height_multiplier, before_jump,
            "empty KnownTechniques should not change jump_height_multiplier"
        );
        assert!(
            attrs.defense_profile.is_empty(),
            "empty KnownTechniques should not add defense_profile entries"
        );
    }

    #[test]
    fn record_practice_increases_proficiency() {
        let mut known = KnownTechniques { entries: vec![] };
        let p1 = record_guangbo_practice(&mut known);
        assert!(
            p1 > 0.0,
            "first record_guangbo_practice should yield positive proficiency, got {p1}"
        );
        assert_eq!(
            known.entries.len(),
            1,
            "record_guangbo_practice should auto-create entry"
        );
        assert_eq!(
            known.entries[0].id, GUANGBO_TICAO_ID,
            "auto-created entry should have id={GUANGBO_TICAO_ID}"
        );

        let p2 = record_guangbo_practice(&mut known);
        assert!(
            p2 > p1,
            "second practice should increase proficiency: {p2} > {p1}"
        );
    }

    #[test]
    fn proficiency_gain_diminishes_past_half() {
        let low_gain = guangbo_proficiency_gain(0.3);
        let high_gain = guangbo_proficiency_gain(0.7);
        assert!(
            low_gain > high_gain,
            "gain at 0.3 ({low_gain}) should exceed gain at 0.7 ({high_gain}) — diminishing returns"
        );
    }

    #[test]
    fn proficiency_never_exceeds_one() {
        let mut known = make_known(0.999, true);
        let p = record_guangbo_practice(&mut known);
        assert!(
            p <= 1.0,
            "proficiency should never exceed 1.0 after practice, got {p}"
        );
    }

    // ── 守恒：真元门 try_record_guangbo_practice ─────────────────────────────

    fn cultivation_with_qi(qi: f64) -> Cultivation {
        Cultivation {
            qi_current: qi,
            qi_max: 100.0,
            ..Cultivation::default()
        }
    }

    /// 守恒锚点：每 rep 扣的真元必须等于 known_techniques 声明的 qi_cost，
    /// 否则代价与定义脱节。改 TechniqueDefinition.qi_cost 必须同步本常量。
    #[test]
    fn qi_cost_matches_known_technique_definition() {
        use crate::cultivation::known_techniques::technique_definition;
        let def = technique_definition(GUANGBO_TICAO_ID)
            .expect("body.guangbo_ticao must exist in TECHNIQUE_DEFINITIONS");
        assert_eq!(
            f64::from(def.qi_cost),
            GUANGBO_TICAO_QI_COST,
            "GUANGBO_TICAO_QI_COST ({GUANGBO_TICAO_QI_COST}) must equal known_techniques \
             qi_cost ({}); 守恒：练习代价不得偏离技能定义",
            def.qi_cost
        );
    }

    #[test]
    fn practice_with_sufficient_qi_charges_cost_and_grants_proficiency() {
        let mut cultivation = cultivation_with_qi(10.0);
        let mut known = KnownTechniques { entries: vec![] };

        let outcome = try_record_guangbo_practice(&mut cultivation, &mut known);

        match outcome {
            PracticeOutcome::Trained { proficiency } => {
                assert!(
                    proficiency > 0.0,
                    "付费练习应递增 proficiency，got {proficiency}"
                );
            }
            other => panic!("expected Trained with qi available, got {other:?}"),
        }
        assert_eq!(
            cultivation.qi_current,
            10.0 - GUANGBO_TICAO_QI_COST,
            "成功练习应恰好扣 GUANGBO_TICAO_QI_COST 真元（守恒：进度有代价）"
        );
    }

    #[test]
    fn practice_with_insufficient_qi_is_rejected_no_charge_no_gain() {
        // qi 严格小于 cost → 拒绝。用 cost 的一半做边界外侧锚点。
        let mut cultivation = cultivation_with_qi(GUANGBO_TICAO_QI_COST - 0.5);
        let mut known = KnownTechniques { entries: vec![] };
        let qi_before = cultivation.qi_current;

        let outcome = try_record_guangbo_practice(&mut cultivation, &mut known);

        assert_eq!(
            outcome,
            PracticeOutcome::TooTiredNoQi,
            "真元不足时练习必须被拒（守恒：不得凭空涨熟练度）"
        );
        assert_eq!(
            cultivation.qi_current, qi_before,
            "被拒练习不得扣真元，期望 {qi_before} 实际 {}",
            cultivation.qi_current
        );
        assert!(
            known.entries.is_empty(),
            "被拒练习不得创建 / 递增 proficiency entry，但 entries={:?}",
            known.entries
        );
    }

    #[test]
    fn practice_at_exact_qi_cost_boundary_is_accepted() {
        // qi == cost（off-by-one 边界）：>= 成立应放行，扣后归零。
        let mut cultivation = cultivation_with_qi(GUANGBO_TICAO_QI_COST);
        let mut known = KnownTechniques { entries: vec![] };

        let outcome = try_record_guangbo_practice(&mut cultivation, &mut known);

        assert!(
            matches!(outcome, PracticeOutcome::Trained { .. }),
            "qi 恰好等于 cost 应放行（边界 >=），got {outcome:?}"
        );
        assert_eq!(
            cultivation.qi_current, 0.0,
            "扣费后真元应归零（恰好 cost），实际 {}",
            cultivation.qi_current
        );
    }

    #[test]
    fn repeated_practice_drains_qi_and_eventually_starves() {
        // 守恒压力测试：有限真元只能买有限次练习；耗尽后练习无效，proficiency 停涨。
        let mut cultivation = cultivation_with_qi(3.0 * GUANGBO_TICAO_QI_COST);
        let mut known = KnownTechniques { entries: vec![] };

        let mut trained_count = 0;
        let mut last_prof = 0.0;
        for _ in 0..10 {
            match try_record_guangbo_practice(&mut cultivation, &mut known) {
                PracticeOutcome::Trained { proficiency } => {
                    assert!(
                        proficiency >= last_prof,
                        "proficiency 应单调不减：{proficiency} vs {last_prof}"
                    );
                    last_prof = proficiency;
                    trained_count += 1;
                }
                PracticeOutcome::TooTiredNoQi => {}
            }
        }
        assert_eq!(
            trained_count, 3,
            "3×cost 真元应恰好买到 3 次练习（守恒：每次付费），实际 {trained_count} 次"
        );
        assert!(
            cultivation.qi_current < GUANGBO_TICAO_QI_COST,
            "练习耗尽真元后余量应 < cost，实际 {}",
            cultivation.qi_current
        );
    }

    // ── consume_guangbo_practice_events 系统级链路（event → 扣真元 → 涨 proficiency）──

    mod system {
        use super::*;
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::{QiAccountId, QiTransfer, QiTransferReason};
        use crate::world::dimension::{CurrentDimension, DimensionKind};
        use crate::world::zone::{ZoneRegistry, DEFAULT_SPAWN_ZONE_NAME};
        use valence::prelude::{App, Events, Position, Update};

        fn build_app() -> App {
            let mut app = App::new();
            app.add_event::<GuangboTicaoPracticeEvent>();
            app.add_systems(Update, consume_guangbo_practice_events);
            app
        }

        fn build_app_with_zone() -> App {
            let mut app = App::new();
            app.add_event::<GuangboTicaoPracticeEvent>();
            app.add_event::<QiTransfer>();
            app.insert_resource(ZoneRegistry::fallback());
            app.add_systems(Update, consume_guangbo_practice_events);
            app
        }

        #[test]
        fn event_with_qi_charges_and_increments_proficiency() {
            let mut app = build_app();
            let entity = app
                .world_mut()
                .spawn((
                    KnownTechniques { entries: vec![] },
                    cultivation_with_qi(5.0),
                ))
                .id();
            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(
                cultivation.qi_current,
                5.0 - GUANGBO_TICAO_QI_COST,
                "消费练习事件应扣 GUANGBO_TICAO_QI_COST 真元"
            );
            let known = app.world().get::<KnownTechniques>(entity).unwrap();
            let entry = known
                .entries
                .iter()
                .find(|e| e.id == GUANGBO_TICAO_ID)
                .expect("练习应创建 guangbo entry");
            assert!(
                entry.proficiency > 0.0,
                "付费练习应递增 proficiency，got {}",
                entry.proficiency
            );
        }

        #[test]
        fn event_without_qi_does_not_grant_proficiency() {
            let mut app = build_app();
            let entity = app
                .world_mut()
                .spawn((
                    KnownTechniques { entries: vec![] },
                    cultivation_with_qi(0.0),
                ))
                .id();
            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(
                cultivation.qi_current, 0.0,
                "真元为 0 的练习不得扣费（守恒）"
            );
            let known = app.world().get::<KnownTechniques>(entity).unwrap();
            assert!(
                known.entries.iter().all(|e| e.proficiency == 0.0),
                "真元不足时 proficiency 不得凭空增长，entries={:?}",
                known.entries
            );
        }

        #[test]
        fn event_without_cultivation_component_is_noop_no_panic() {
            // 守恒边界：无 Cultivation 组件 → 无真元可付 → 不涨 proficiency（也不 panic）。
            let mut app = build_app();
            let entity = app
                .world_mut()
                .spawn(KnownTechniques { entries: vec![] })
                .id();
            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            let known = app.world().get::<KnownTechniques>(entity).unwrap();
            assert!(
                known.entries.is_empty(),
                "无 Cultivation 实体不应获得 proficiency（守恒：进度必须付真元），entries={:?}",
                known.entries
            );
        }

        #[test]
        fn event_for_unknown_entity_is_noop() {
            // 事件指向不存在 / 无 KnownTechniques 的实体 → consumer get_mut 失败 → 静默跳过。
            let mut app = build_app();
            // spawn 一个无 KnownTechniques 的实体，事件却指向它。
            let entity = app.world_mut().spawn(cultivation_with_qi(5.0)).id();
            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            // 无 KnownTechniques → 不应被扣真元（consumer 在拿到 known 之前就 continue）。
            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(
                cultivation.qi_current, 5.0,
                "无 KnownTechniques 的实体不应被扣真元"
            );
        }

        // ── 守恒：zone credit 链路（扣真元 → 回灌区域）────────────────────────────

        /// 成功练习应将 GUANGBO_TICAO_QI_COST 回灌到当前区域，emit QiTransfer 事件，
        /// 且 zone.spirit_qi 应增加对应量。
        ///
        /// pitfall (a)：将 zone 清空（spirit_qi=0.0）避免接近满容量时发生溢出分割导致
        ///              total ≠ cost 的断言失败。
        /// pitfall (b)：施放者必须挂 CurrentDimension，否则 find_zone 返回 None，
        ///              qi 路由到 Overflow 账户而非 zone。
        #[test]
        fn event_credits_qi_to_zone_and_emits_qi_transfer() {
            let mut app = build_app_with_zone();

            // pitfall (a)：清空 zone，确保 1.0 能整体被 zone 接受，无溢出分割。
            app.world_mut()
                .resource_mut::<ZoneRegistry>()
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .spirit_qi = 0.0;

            let zone_before = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;

            // pitfall (b)：施放者必须有 CurrentDimension 组件。
            let entity = app
                .world_mut()
                .spawn((
                    KnownTechniques { entries: vec![] },
                    cultivation_with_qi(5.0),
                    Position::new([0.0, 64.0, 0.0]),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();

            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            // 真元应被扣除。
            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(
                cultivation.qi_current,
                5.0 - GUANGBO_TICAO_QI_COST,
                "守恒：练习应扣 GUANGBO_TICAO_QI_COST 真元，实际 {}",
                cultivation.qi_current
            );

            // zone.spirit_qi 应增加（单位：spirit_qi 比例）。
            let zone_after = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;
            let zone_delta_raw = (zone_after - zone_before) * QI_ZONE_UNIT_CAPACITY;
            assert!(
                (zone_delta_raw - GUANGBO_TICAO_QI_COST).abs() < 1e-6,
                "守恒：扣除的 qi_cost 必须进入当前 zone；期望 delta={}, 实际 {zone_delta_raw}",
                GUANGBO_TICAO_QI_COST
            );

            // QiTransfer 事件应被 emit（守恒审计链路）。
            let transfers: Vec<_> = app
                .world()
                .resource::<Events<QiTransfer>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            assert_eq!(
                transfers.len(),
                1,
                "广播体操扣 {GUANGBO_TICAO_QI_COST} 真元时必须 emit 1 条 QiTransfer，实际 {} 条",
                transfers.len()
            );
            let transfer = &transfers[0];
            assert_eq!(
                transfer.to,
                QiAccountId::zone(DEFAULT_SPAWN_ZONE_NAME),
                "QiTransfer.to 应回灌当前 spawn zone，实际 {:?}",
                transfer.to
            );
            assert!(
                (transfer.amount - GUANGBO_TICAO_QI_COST).abs() < 1e-6,
                "QiTransfer.amount 应等于 GUANGBO_TICAO_QI_COST={GUANGBO_TICAO_QI_COST}，实际 {}",
                transfer.amount
            );
            assert_eq!(
                transfer.reason,
                QiTransferReason::ReleaseToZone,
                "广播体操释放真元应使用 ReleaseToZone，实际 {:?}",
                transfer.reason
            );
        }

        /// 真元不足时练习被拒——zone 不应得到任何 credit，QiTransfer 不应被 emit。
        #[test]
        fn insufficient_qi_no_zone_credit() {
            let mut app = build_app_with_zone();

            // 清空 zone 以便精确断言 delta == 0。
            app.world_mut()
                .resource_mut::<ZoneRegistry>()
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .spirit_qi = 0.0;

            let zone_before = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;

            let entity = app
                .world_mut()
                .spawn((
                    KnownTechniques { entries: vec![] },
                    cultivation_with_qi(0.0),
                    Position::new([0.0, 64.0, 0.0]),
                    CurrentDimension(DimensionKind::Overworld),
                ))
                .id();

            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            // zone 不应变化。
            let zone_after = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;
            assert!(
                (zone_after - zone_before).abs() < 1e-12,
                "真元不足时 zone 不应得到任何 credit：before={zone_before}, after={zone_after}"
            );

            // QiTransfer 不应被 emit。
            let transfers: Vec<_> = app
                .world()
                .resource::<Events<QiTransfer>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            assert!(
                transfers.is_empty(),
                "真元不足练习不得 emit QiTransfer，实际 {} 条",
                transfers.len()
            );
        }

        /// 无 CurrentDimension 时 qi 路由到 Overflow（不泄漏到 void），zone 不变，
        /// 但 QiTransfer 仍应 emit（到 Overflow 账户）——守恒规则：qi 必有去处。
        #[test]
        fn missing_current_dimension_routes_to_overflow_not_void() {
            let mut app = build_app_with_zone();

            // 清空 zone 以便精确判断 zone 未变。
            app.world_mut()
                .resource_mut::<ZoneRegistry>()
                .find_zone_mut(DEFAULT_SPAWN_ZONE_NAME)
                .expect("spawn zone should exist")
                .spirit_qi = 0.0;

            let zone_before = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;

            // 无 CurrentDimension 组件（pitfall b 的反面验证）。
            let entity = app
                .world_mut()
                .spawn((
                    KnownTechniques { entries: vec![] },
                    cultivation_with_qi(5.0),
                    Position::new([0.0, 64.0, 0.0]),
                    // 故意不挂 CurrentDimension
                ))
                .id();

            app.world_mut()
                .resource_mut::<Events<GuangboTicaoPracticeEvent>>()
                .send(GuangboTicaoPracticeEvent { entity });

            app.update();

            // 真元仍应被扣（守恒：qi_current 已减少）。
            let cultivation = app.world().get::<Cultivation>(entity).unwrap();
            assert_eq!(
                cultivation.qi_current,
                5.0 - GUANGBO_TICAO_QI_COST,
                "无 CurrentDimension 时真元仍应被扣，实际 {}",
                cultivation.qi_current
            );

            // zone 不应变化（qi 路由到 Overflow 而非 zone）。
            let zone_after = app
                .world()
                .resource::<ZoneRegistry>()
                .find_zone_by_name(DEFAULT_SPAWN_ZONE_NAME)
                .unwrap()
                .spirit_qi;
            assert!(
                (zone_after - zone_before).abs() < 1e-12,
                "无 CurrentDimension 时 zone 不应变化（qi 走 overflow）：before={zone_before}, after={zone_after}"
            );

            // QiTransfer 仍应被 emit（到 Overflow 账户——守恒：qi 有去处）。
            let transfers: Vec<_> = app
                .world()
                .resource::<Events<QiTransfer>>()
                .iter_current_update_events()
                .cloned()
                .collect();
            assert_eq!(
                transfers.len(),
                1,
                "无 CurrentDimension 时 qi 走 Overflow，仍应 emit 1 条 QiTransfer，实际 {} 条",
                transfers.len()
            );
            assert!(
                matches!(
                    transfers[0].to.kind,
                    crate::qi_physics::QiAccountKind::Overflow
                ),
                "无 CurrentDimension 的 QiTransfer 应路由到 Overflow，实际 {:?}",
                transfers[0].to
            );
        }
    }
}
