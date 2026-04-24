//! NPC 社交：`SocializeAction`（同派系寒暄）、`FactionDuelScorer`（敌对
//! 相遇即追击）+ 交易估价函数（骨币换算，plan §5.1 简化版）。
//!
//! 敌对派系实际相遇侦测已由 `faction::assign_hostile_encounters` 实装
//! （写入 `DuelTarget` component）。本模块提供对接 big-brain 的 Scorer
//! 让 Beast/Disciple thinker 能感知 DuelTarget 并切换到战斗行为。

use big_brain::prelude::{ActionBuilder, ActionState, Actor, BigBrainSet, Score, ScorerBuilder};
use valence::prelude::{
    bevy_ecs, App, Commands, Component, Entity, IntoSystemConfigs, Position, PreUpdate, Query, With,
};

use crate::inventory::{ItemInstance, ItemRarity};
use crate::npc::faction::FactionMembership;
use crate::npc::navigator::Navigator;
use crate::npc::spawn::{DuelTarget, NpcMarker};

/// SocializeAction 单次持续 tick 上限（到时 Success 让 picker 重选）。
pub const SOCIALIZE_MAX_TICKS: u32 = 120;
/// 同派系相遇的判定距离（格）。
pub const SOCIALIZE_RANGE: f64 = 6.0;
/// 社交 baseline 分数（低于大部分高优先级行为，高于 Wander baseline）。
pub const SOCIALIZE_BASELINE_SCORE: f32 = 0.1;

/// 附近有同派系 NPC（在 SOCIALIZE_RANGE 内）且自身无 DuelTarget → 返回社交分。
#[derive(Clone, Copy, Debug, Component)]
pub struct SocializeScorer;

/// 敌对派系已被 `assign_hostile_encounters` 标为 `DuelTarget` → 分数 1.0。
/// Thinker 用此触发 Chase/Attack 链（与 ChaseTargetScorer 叠加）。
#[derive(Clone, Copy, Debug, Component)]
pub struct FactionDuelScorer;

/// 寒暄：停 Navigator、倒计时，到期 Success。
#[derive(Clone, Copy, Debug, Component)]
pub struct SocializeAction;

/// Socialize 运行态。
#[derive(Clone, Copy, Debug, Default, Component)]
pub struct SocializeState {
    pub partner: Option<Entity>,
    pub elapsed_ticks: u32,
}

impl ScorerBuilder for SocializeScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("SocializeScorer")
    }
}

impl ScorerBuilder for FactionDuelScorer {
    fn build(&self, cmd: &mut Commands, scorer: Entity, _actor: Entity) {
        cmd.entity(scorer).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("FactionDuelScorer")
    }
}

impl ActionBuilder for SocializeAction {
    fn build(&self, cmd: &mut Commands, action: Entity, _actor: Entity) {
        cmd.entity(action).insert(*self);
    }
    fn label(&self) -> Option<&str> {
        Some("SocializeAction")
    }
}

pub fn register(app: &mut App) {
    app.add_systems(
        PreUpdate,
        (socialize_scorer_system, faction_duel_scorer_system).in_set(BigBrainSet::Scorers),
    )
    .add_systems(
        PreUpdate,
        socialize_action_system.in_set(BigBrainSet::Actions),
    );
}

type SocializeNpcQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Position, &'static FactionMembership),
    With<NpcMarker>,
>;

type SocializeSelfQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Position, &'static FactionMembership, Option<&'static DuelTarget>),
    With<NpcMarker>,
>;

fn socialize_scorer_system(
    self_q: SocializeSelfQuery<'_, '_>,
    peers: SocializeNpcQuery<'_, '_>,
    mut scorers: Query<(&Actor, &mut Score), With<SocializeScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = match self_q.get(*actor) {
            Ok((pos, membership, duel)) => {
                if duel.is_some() {
                    // 有敌对目标就不寒暄
                    0.0
                } else {
                    let p = pos.get();
                    let has_same_faction_peer = peers.iter().any(|(ent, ppos, pmem)| {
                        ent != *actor
                            && pmem.faction_id == membership.faction_id
                            && p.distance(ppos.get()) <= SOCIALIZE_RANGE
                    });
                    if has_same_faction_peer {
                        SOCIALIZE_BASELINE_SCORE
                    } else {
                        0.0
                    }
                }
            }
            Err(_) => 0.0,
        };
        score.set(value);
    }
}

fn faction_duel_scorer_system(
    duelists: Query<Option<&DuelTarget>, With<NpcMarker>>,
    mut scorers: Query<(&Actor, &mut Score), With<FactionDuelScorer>>,
) {
    for (Actor(actor), mut score) in &mut scorers {
        let value = duelists
            .get(*actor)
            .ok()
            .flatten()
            .map(|_| 1.0)
            .unwrap_or(0.0);
        score.set(value);
    }
}

fn socialize_action_system(
    self_q: SocializeSelfQuery<'_, '_>,
    peers: SocializeNpcQuery<'_, '_>,
    mut mutables: Query<(&mut Navigator, &mut SocializeState), With<NpcMarker>>,
    mut actions: Query<(&Actor, &mut ActionState), With<SocializeAction>>,
) {
    for (Actor(actor), mut state) in &mut actions {
        let Ok((pos, membership, _)) = self_q.get(*actor) else {
            *state = ActionState::Failure;
            continue;
        };
        let Ok((mut navigator, mut sstate)) = mutables.get_mut(*actor) else {
            *state = ActionState::Failure;
            continue;
        };

        match *state {
            ActionState::Requested => {
                let partner = peers
                    .iter()
                    .filter_map(|(ent, ppos, pmem)| {
                        if ent == *actor {
                            return None;
                        }
                        if pmem.faction_id != membership.faction_id {
                            return None;
                        }
                        let d = pos.get().distance(ppos.get());
                        if d <= SOCIALIZE_RANGE {
                            Some((ent, d))
                        } else {
                            None
                        }
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                match partner {
                    Some((ent, _)) => {
                        navigator.stop();
                        sstate.partner = Some(ent);
                        sstate.elapsed_ticks = 0;
                        *state = ActionState::Executing;
                    }
                    None => {
                        *state = ActionState::Success;
                    }
                }
            }
            ActionState::Executing => {
                sstate.elapsed_ticks = sstate.elapsed_ticks.saturating_add(1);
                if sstate.elapsed_ticks >= SOCIALIZE_MAX_TICKS {
                    sstate.partner = None;
                    *state = ActionState::Success;
                }
            }
            ActionState::Cancelled => {
                sstate.partner = None;
                *state = ActionState::Failure;
            }
            ActionState::Init | ActionState::Success | ActionState::Failure => {}
        }
    }
}

// ---------------------------------------------------------------------------
// 交易估价（plan §5.1 简化版）
// ---------------------------------------------------------------------------

/// 稀有度 → 基础骨币价格表。用于 NPC 与 NPC、NPC 与玩家交易的 baseline。
/// 真实经济（rarity 乘数 / 灵力品阶 / 鲜度折扣）由 plan-economy 或后续
/// commit 细化；本表仅提供"存在但不精致"的估价。
pub const fn rarity_base_price(rarity: ItemRarity) -> u64 {
    match rarity {
        ItemRarity::Common => 4,
        ItemRarity::Uncommon => 12,
        ItemRarity::Rare => 40,
        ItemRarity::Epic => 150,
        ItemRarity::Legendary => 600,
    }
}

/// 估价单个 `ItemInstance`（骨币）。考虑：
/// - rarity 基础价
/// - stack_count 倍率
/// - spirit_quality（0..=1）+50% 加成
/// - durability（0..=1）≤0.2 时打 5 折
pub fn estimate_item_price(item: &ItemInstance) -> u64 {
    let base = rarity_base_price(item.rarity) as f64;
    let quality_mult = 1.0 + item.spirit_quality.clamp(0.0, 1.0) * 0.5;
    let durability_mult = if item.durability.clamp(0.0, 1.0) <= 0.2 {
        0.5
    } else {
        1.0
    };
    let per = base * quality_mult * durability_mult;
    (per * item.stack_count.max(1) as f64).round().max(1.0) as u64
}

/// 估价一批 `ItemInstance` 的总骨币值。
pub fn estimate_trade_value(items: &[ItemInstance]) -> u64 {
    items.iter().map(estimate_item_price).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemInstance;
    use crate::npc::faction::{FactionId, FactionRank, MissionQueue, Reputation};
    use valence::prelude::{App, DVec3, PreUpdate};

    fn make_item(rarity: ItemRarity, stack: u32, quality: f64, durability: f64) -> ItemInstance {
        ItemInstance {
            instance_id: 1,
            template_id: "x".to_string(),
            display_name: "X".to_string(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.0,
            rarity,
            description: String::new(),
            stack_count: stack,
            spirit_quality: quality,
            durability,
            freshness: None,
        }
    }

    fn make_membership(faction: FactionId) -> FactionMembership {
        FactionMembership {
            faction_id: faction,
            rank: FactionRank::Disciple,
            reputation: Reputation::default(),
            lineage: None,
            mission_queue: MissionQueue::default(),
        }
    }

    // --- 估价 ---

    #[test]
    fn rarity_base_price_monotonically_increases() {
        let commons = rarity_base_price(ItemRarity::Common);
        let uncommon = rarity_base_price(ItemRarity::Uncommon);
        let rare = rarity_base_price(ItemRarity::Rare);
        let epic = rarity_base_price(ItemRarity::Epic);
        let legend = rarity_base_price(ItemRarity::Legendary);
        assert!(commons < uncommon);
        assert!(uncommon < rare);
        assert!(rare < epic);
        assert!(epic < legend);
    }

    #[test]
    fn estimate_item_price_respects_stack_count() {
        let one = make_item(ItemRarity::Common, 1, 0.0, 1.0);
        let stack_ten = make_item(ItemRarity::Common, 10, 0.0, 1.0);
        assert_eq!(
            estimate_item_price(&stack_ten),
            estimate_item_price(&one) * 10
        );
    }

    #[test]
    fn estimate_item_price_quality_bonus() {
        let plain = make_item(ItemRarity::Rare, 1, 0.0, 1.0);
        let premium = make_item(ItemRarity::Rare, 1, 1.0, 1.0);
        assert!(estimate_item_price(&premium) > estimate_item_price(&plain));
    }

    #[test]
    fn estimate_item_price_durability_discount() {
        let fine = make_item(ItemRarity::Epic, 1, 0.0, 1.0);
        let broken = make_item(ItemRarity::Epic, 1, 0.0, 0.1);
        // Epic base = 150; broken 0.5x = 75
        assert_eq!(estimate_item_price(&broken) * 2, estimate_item_price(&fine));
    }

    #[test]
    fn estimate_item_price_never_returns_zero() {
        let weird = ItemInstance {
            instance_id: 0,
            template_id: "".into(),
            display_name: "".into(),
            grid_w: 1,
            grid_h: 1,
            weight: 0.0,
            rarity: ItemRarity::Common,
            description: String::new(),
            stack_count: 0,
            spirit_quality: 0.0,
            durability: 0.0,
            freshness: None,
        };
        assert!(estimate_item_price(&weird) >= 1);
    }

    #[test]
    fn estimate_trade_value_sums_items() {
        let bundle = [
            make_item(ItemRarity::Common, 5, 0.0, 1.0),
            make_item(ItemRarity::Rare, 1, 0.5, 1.0),
        ];
        let sum = estimate_item_price(&bundle[0]) + estimate_item_price(&bundle[1]);
        assert_eq!(estimate_trade_value(&bundle), sum);
    }

    #[test]
    fn estimate_trade_value_empty_list_zero() {
        let empty: [ItemInstance; 0] = [];
        assert_eq!(estimate_trade_value(&empty), 0);
    }

    // --- SocializeScorer ---

    fn build_socialize_app() -> App {
        let mut app = App::new();
        app.add_systems(PreUpdate, socialize_scorer_system);
        app
    }

    #[test]
    fn socialize_scorer_zero_when_alone() {
        let mut app = build_socialize_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), SocializeScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn socialize_scorer_nonzero_with_same_faction_peer_in_range() {
        let mut app = build_socialize_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let _peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), SocializeScorer))
            .id();
        app.update();
        let got = app.world().get::<Score>(scorer).unwrap().get();
        assert!(got > 0.0);
    }

    #[test]
    fn socialize_scorer_zero_when_peer_different_faction() {
        let mut app = build_socialize_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let _peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                make_membership(FactionId::Defend),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), SocializeScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn socialize_scorer_zero_when_peer_too_far() {
        let mut app = build_socialize_app();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let _peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([50.0, 64.0, 50.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), SocializeScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    #[test]
    fn socialize_scorer_zero_when_duel_target_assigned() {
        let mut app = build_socialize_app();
        let dummy = app.world_mut().spawn(NpcMarker).id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
                DuelTarget(dummy),
            ))
            .id();
        let _peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([3.0, 64.0, 3.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), SocializeScorer))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<Score>(scorer).unwrap().get(),
            0.0,
            "敌对时不社交"
        );
    }

    // --- FactionDuelScorer ---

    #[test]
    fn faction_duel_scorer_one_with_duel_target() {
        let mut app = App::new();
        app.add_systems(PreUpdate, faction_duel_scorer_system);
        let dummy = app.world_mut().spawn(NpcMarker).id();
        let npc = app
            .world_mut()
            .spawn((NpcMarker, DuelTarget(dummy)))
            .id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), FactionDuelScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 1.0);
    }

    #[test]
    fn faction_duel_scorer_zero_without_duel_target() {
        let mut app = App::new();
        app.add_systems(PreUpdate, faction_duel_scorer_system);
        let npc = app.world_mut().spawn(NpcMarker).id();
        let scorer = app
            .world_mut()
            .spawn((Actor(npc), Score::default(), FactionDuelScorer))
            .id();
        app.update();
        assert_eq!(app.world().get::<Score>(scorer).unwrap().get(), 0.0);
    }

    // --- SocializeAction ---

    #[test]
    fn socialize_action_success_when_no_same_faction_peer() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            socialize_action_system.in_set(BigBrainSet::Actions),
        );
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
                Navigator::new(),
                SocializeState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), SocializeAction, ActionState::Requested))
            .id();
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn socialize_action_records_partner_and_succeeds_on_timeout() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            socialize_action_system.in_set(BigBrainSet::Actions),
        );
        let peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 2.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
                Navigator::new(),
                SocializeState::default(),
            ))
            .id();
        let action = app
            .world_mut()
            .spawn((Actor(npc), SocializeAction, ActionState::Requested))
            .id();
        app.update();
        let s = *app.world().get::<SocializeState>(npc).unwrap();
        assert_eq!(s.partner, Some(peer));
        {
            let mut st = app.world_mut().get_mut::<SocializeState>(npc).unwrap();
            st.elapsed_ticks = SOCIALIZE_MAX_TICKS - 1;
        }
        app.update();
        assert_eq!(
            *app.world().get::<ActionState>(action).unwrap(),
            ActionState::Success
        );
    }

    #[test]
    fn socialize_action_stops_navigator_on_partner_found() {
        let mut app = App::new();
        app.add_systems(
            PreUpdate,
            socialize_action_system.in_set(BigBrainSet::Actions),
        );
        let mut nav = Navigator::new();
        nav.set_goal(DVec3::new(100.0, 64.0, 100.0), 1.0);
        let _peer = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([2.0, 64.0, 2.0]),
                make_membership(FactionId::Attack),
            ))
            .id();
        let npc = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([0.0, 64.0, 0.0]),
                make_membership(FactionId::Attack),
                nav,
                SocializeState::default(),
            ))
            .id();
        let _action = app
            .world_mut()
            .spawn((Actor(npc), SocializeAction, ActionState::Requested))
            .id();
        app.update();
        assert!(app.world().get::<Navigator>(npc).unwrap().is_idle());
    }

}
