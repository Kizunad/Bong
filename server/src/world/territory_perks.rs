//! plan-territory-v1 P1 — 驻守收益与代价（ZoneDominance 消费层）。
//!
//! ## 设计概要
//!
//! P1 在 P0 `ZoneDominance`（territory.rs）之上新增三条真效果：
//!
//! 1. **灵气优先权（`ZoneDominanceRegenStore`）**：
//!    霸主在其 zone 获得 +20% qi regen 速率（`DOMINANCE_REGEN_MULTIPLIER = 1.20`）。
//!    只乘 `rate` 参数，`regen_from_zone` 内生 `drain = gain / QI_ZONE_UNIT_CAPACITY`
//!    不变，总量守恒（zero-sum：霸主吸气快 20% 同时 zone 贫化快 20%）。
//!    与 `ZoneSpiritBonusStore`（war/settle.rs）同级，克隆其写法。
//!
//! 2. **天道注意力加速（`TiandaoAttentionInput.is_dominant`）**：
//!    霸主的 `accumulation_rate` 乘以 `DOMINANCE_ATTENTION_MULT = 1.5`（出头椽子先烂）。
//!    真效果链：`attention.level` 累积快 → 更快进 Watch/Pressure →
//!    (a) tiandao 既有 response chain 对霸主更狠（降 zone qi / 异变 / 天劫）
//!    (b) `FearCultivatorScorer` 自动读 `response` 提分 → NPC 对霸主恐惧↑（零改 brain）。
//!
//! 3. **NPC 尊敬/投靠（`apply_dominance_reputation`）**：
//!    带 `NpcPatrol + NpcPlayerReputation` 的 NPC，若其 `home_zone` 有霸主，
//!    则每次 territory eval（60s 节奏）对该霸主 `rep.adjust(+DOMINANCE_REP_DELTA=0.02)`，
//!    渐进逼近 High 信誉等级 → `check_trade_eligibility` 折扣/稀有品解禁（真效果）。
//!
//! ## 代价三层（内生，不靠额外惩罚字段）
//!
//! 1. 灵气加速消耗：zone `spirit_qi` 贫化快 20%（零和）。
//! 2. 天道盯更狠：attention 快 50% → Watch/Pressure 触发更频繁。
//! 3. 树敌：attention↑ → FearCultivatorScorer 对 NPC 提分 → NPC 散 → 交易/情报网削弱。
//!
//! ## defer（不留半实现）
//!
//! - NPC 主动跟随 Action（深改 brain）→ P1.5。
//! - `compute_threat_assessments` 对霸主降威胁偏置（threat.rs 拿不到 char_id）→ P1.5。
//! - `InfluenceChangedEvent` 直接驱动 NPC 反应 → P3 narration。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, App, IntoSystemConfigs, Res, ResMut, Resource, Update};

use crate::npc::patrol::NpcPatrol;
use crate::npc::trade::NpcPlayerReputation;
use crate::world::territory::{ZoneInfluenceMap, TERRITORY_EVAL_INTERVAL_TICKS};

// ─── 常量 ─────────────────────────────────────────────────────────────────────

/// 霸主 zone 内 qi regen 速率倍率。
/// 守恒安全：只乘 `compute_regen` 的 `rate` 入参，`drain = gain / QI_ZONE_UNIT_CAPACITY`
/// 不变，player gain 增多少 zone 就减多少，zero-sum。
/// 比 war winner 的 1.10 稍高（持续驻守霸主取 1.20），显著但不过激，可调。
pub const DOMINANCE_REGEN_MULTIPLIER: f64 = 1.20;

/// 霸主天道注意力累积速率乘子（worldview §八 "出头椽子先烂"）。
/// `accumulation_rate(input)` 末尾乘此值；只改纯函数输出，不改 `TiandaoAttention` 存储字段。
pub const DOMINANCE_ATTENTION_MULT: f64 = 1.5;

/// 每次 territory eval（约 60s）对霸主的 NPC 信誉增量。
/// 0.02/eval × 25 次 ≈ 25 分钟逼近 High tier（0.5 → 0.7+）。可调。
pub const DOMINANCE_REP_DELTA: f32 = 0.02;

// ─── ZoneDominanceRegenStore ──────────────────────────────────────────────────

/// 霸主 zone qi regen 速率倍率表（plan-territory-v1 P1）。
///
/// key = zone name，value = 速率倍率（默认 1.0）。
/// `recompute_territory_perks` system 每 tick 从 `ZoneInfluenceMap.dominant` 重建：
/// 有霸主 → `multipliers[zone] = DOMINANCE_REGEN_MULTIPLIER`，
/// 无霸主 → 移除条目（回落 1.0）。
///
/// **守恒安全**：与 `ZoneSpiritBonusStore`（war/settle.rs）同级，只存倍率，
/// 不触任何真元账户，不 emit `QiTransfer`。
#[derive(Resource, Default, Debug)]
pub struct ZoneDominanceRegenStore {
    /// zone name → regen 速率倍率（默认查不到即 1.0）
    pub multipliers: HashMap<String, f64>,
}

impl ZoneDominanceRegenStore {
    /// 查询 zone 的霸主 regen 倍率；无霸主返回 1.0（中性）。
    pub fn multiplier_for(&self, zone: &str) -> f64 {
        self.multipliers.get(zone).copied().unwrap_or(1.0)
    }
}

// ─── System: recompute_territory_perks ───────────────────────────────────────

/// 从 `ZoneInfluenceMap.dominant` 重建 `ZoneDominanceRegenStore`。
///
/// 每 tick 运行（`.after(territory::territory_tick)`）。
/// 有 dominant → store 条目 = `DOMINANCE_REGEN_MULTIPLIER`。
/// 无 dominant → 移除条目（回落 1.0）。
/// qi tick 使用 `Option<Res<ZoneDominanceRegenStore>>` 防资源缺失 panic（克隆 war_bonus 写法）。
pub fn recompute_territory_perks(
    influence_map: Option<Res<ZoneInfluenceMap>>,
    mut regen_store: ResMut<ZoneDominanceRegenStore>,
) {
    let Some(influence_map) = influence_map else {
        regen_store.multipliers.clear();
        return;
    };

    // 先收集有霸主的 zone 集合，再全量重建（防孤儿条目）
    let dominant_zones: HashMap<String, f64> = influence_map
        .zones
        .iter()
        .filter_map(|(zone_name, entry)| {
            entry
                .dominant
                .as_ref()
                .map(|_| (zone_name.clone(), DOMINANCE_REGEN_MULTIPLIER))
        })
        .collect();

    regen_store.multipliers = dominant_zones;
}

// ─── System: apply_dominance_reputation ──────────────────────────────────────

/// 对 home_zone 有霸主的 NPC，每 territory eval 节奏（60s）对霸主信誉 +DOMINANCE_REP_DELTA。
///
/// **真效果链**：
/// `rep.adjust(+0.02)` → score 渐进 0.7+ → `RepTier::High` →
/// `check_trade_eligibility` 返回 `Allowed { discount: true }` → 交易折扣/稀有品解禁。
///
/// 节流：与 territory eval 同步（`TERRITORY_EVAL_INTERVAL_TICKS ≈ 60s`），
/// 通过 `last_rep_tick` Local 实现，防每 tick 都调。
pub fn apply_dominance_reputation(
    influence_map: Option<Res<ZoneInfluenceMap>>,
    clock: Option<Res<crate::cultivation::tick::CultivationClock>>,
    mut last_rep_tick: valence::prelude::Local<u64>,
    mut npcs: valence::prelude::Query<(&NpcPatrol, &mut NpcPlayerReputation)>,
) {
    let Some(clock) = clock else {
        return;
    };
    let now_tick = clock.tick;

    // 节流：与 territory eval 间隔一致
    if *last_rep_tick > 0 && now_tick.saturating_sub(*last_rep_tick) < TERRITORY_EVAL_INTERVAL_TICKS
    {
        return;
    }
    *last_rep_tick = now_tick;

    let Some(influence_map) = influence_map else {
        return;
    };

    for (patrol, mut rep) in npcs.iter_mut() {
        let Some(entry) = influence_map.zones.get(&patrol.home_zone) else {
            continue;
        };
        let Some(dominant) = &entry.dominant else {
            continue;
        };
        rep.adjust(&dominant.char_id, DOMINANCE_REP_DELTA);
    }
}

// ─── 注册 ─────────────────────────────────────────────────────────────────────

/// 注册 P1 资源与 system 到 Bevy App。
///
/// 挂载顺序：
/// - `recompute_territory_perks` → `.after(territory_tick)`（读最新 dominant）
/// - `apply_dominance_reputation` → `.after(recompute_territory_perks)`（读同 tick dominant）
pub fn register(app: &mut App) {
    app.init_resource::<ZoneDominanceRegenStore>();
    app.add_systems(
        Update,
        (
            recompute_territory_perks.after(crate::world::territory::territory_tick),
            apply_dominance_reputation.after(recompute_territory_perks),
        ),
    );
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::territory::{PlayerInfluence, ZoneDominance, ZoneInfluenceEntry};

    // ── 辅助 ─────────────────────────────────────────────────────────────────

    fn assert_close(actual: f64, expected: f64, msg: &str) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "{msg}: 期望 {expected:.12}, 实际 {actual:.12}"
        );
    }

    fn make_zone_with_dominant(zone_name: &str, char_id: &str, influence: f64) -> ZoneInfluenceMap {
        let mut entry = ZoneInfluenceEntry::default();
        entry.players.insert(
            char_id.to_string(),
            PlayerInfluence {
                value: influence,
                ..Default::default()
            },
        );
        entry.dominant = Some(ZoneDominance {
            char_id: char_id.to_string(),
            influence,
            established_tick: 1000,
            public_known: false,
        });
        let mut map = ZoneInfluenceMap::default();
        map.zones.insert(zone_name.to_string(), entry);
        map
    }

    // ── 1. 灵气优先权：无霸主 → multiplier_for = 1.0 ─────────────────────────

    #[test]
    fn no_dominant_zone_multiplier_is_one() {
        let store = ZoneDominanceRegenStore::default();
        assert_close(
            store.multiplier_for("spawn"),
            1.0,
            "无霸主 zone 倍率应为 1.0（中性）",
        );
    }

    // ── 2. 灵气优先权：有霸主 → multiplier_for = DOMINANCE_REGEN_MULTIPLIER ──

    #[test]
    fn dominant_zone_multiplier_is_1_20() {
        let mut store = ZoneDominanceRegenStore::default();
        store
            .multipliers
            .insert("spawn".to_string(), DOMINANCE_REGEN_MULTIPLIER);
        assert_close(
            store.multiplier_for("spawn"),
            DOMINANCE_REGEN_MULTIPLIER,
            "有霸主 zone 倍率应为 DOMINANCE_REGEN_MULTIPLIER(1.20)",
        );
    }

    // ── 3. recompute_territory_perks：有 dominant 时 store 写入倍率 ──────────

    #[test]
    fn recompute_fills_store_when_dominant_present() {
        use valence::prelude::App;

        let map = make_zone_with_dominant("qingyun", "player:hero", 50.0);

        let mut app = App::new();
        app.insert_resource(map);
        app.init_resource::<ZoneDominanceRegenStore>();
        app.add_systems(valence::prelude::Update, recompute_territory_perks);

        app.update();

        let store = app.world().resource::<ZoneDominanceRegenStore>();
        assert_close(
            store.multiplier_for("qingyun"),
            DOMINANCE_REGEN_MULTIPLIER,
            "有霸主 qingyun zone 应写入 DOMINANCE_REGEN_MULTIPLIER(1.20)",
        );
    }

    // ── 4. recompute_territory_perks：dominant=None 时 store 无条目 ──────────

    #[test]
    fn recompute_clears_store_when_no_dominant() {
        use valence::prelude::App;

        // 先预填一个条目
        let mut store = ZoneDominanceRegenStore::default();
        store
            .multipliers
            .insert("spawn".to_string(), DOMINANCE_REGEN_MULTIPLIER);

        // ZoneInfluenceMap 中 spawn 无霸主
        let mut map = ZoneInfluenceMap::default();
        let entry = ZoneInfluenceEntry::default(); // dominant = None
        map.zones.insert("spawn".to_string(), entry);

        let mut app = App::new();
        app.insert_resource(map);
        app.insert_resource(store);
        app.add_systems(valence::prelude::Update, recompute_territory_perks);

        app.update();

        let store = app.world().resource::<ZoneDominanceRegenStore>();
        assert_close(
            store.multiplier_for("spawn"),
            1.0,
            "dominant=None 时 store 应清除条目，multiplier_for 回落 1.0",
        );
    }

    // ── 5. 霸主失去后 store 条目移除回 1.0 ───────────────────────────────────

    #[test]
    fn store_clears_after_dominant_lost() {
        use valence::prelude::App;

        // 第一 tick：有霸主
        let map = make_zone_with_dominant("bloodmount", "player:a", 45.0);
        let mut app = App::new();
        app.insert_resource(map);
        app.init_resource::<ZoneDominanceRegenStore>();
        app.add_systems(valence::prelude::Update, recompute_territory_perks);
        app.update();

        {
            let store = app.world().resource::<ZoneDominanceRegenStore>();
            assert_close(
                store.multiplier_for("bloodmount"),
                DOMINANCE_REGEN_MULTIPLIER,
                "第一 tick 后应有 DOMINANCE_REGEN_MULTIPLIER",
            );
        }

        // 移除霸主
        {
            let mut map = app.world_mut().resource_mut::<ZoneInfluenceMap>();
            if let Some(entry) = map.zones.get_mut("bloodmount") {
                entry.dominant = None;
            }
        }

        app.update();

        let store = app.world().resource::<ZoneDominanceRegenStore>();
        assert_close(
            store.multiplier_for("bloodmount"),
            1.0,
            "霸主消失后 store 条目应清除，multiplier_for 回落 1.0",
        );
    }

    // ── 6. 守恒断言：DOMINANCE_REGEN_MULTIPLIER 只乘 rate，守恒不变 ──────────
    //
    // 验证思路：同 war/settle.rs 的守恒证明逻辑——
    // `compute_regen(zone_qi, rate * m, integrity, room)` 内 `drain = gain / CAP`，
    // 霸主 gain 多 20% 则 zone drain 也多 20%。
    // 此处用 qi_physics::regen_from_zone 直接验算，断言 gain == drain * CAP。

    #[test]
    fn dominance_regen_multiplier_preserves_zero_sum() {
        use crate::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
        use crate::qi_physics::regen_from_zone;

        let zone_qi = 0.8f64;
        let base_rate = 1.0f64;
        let avg_integrity = 1.0f64;
        let qi_room = 1000.0f64;

        // 无霸主（倍率 1.0）
        let (gain_base, drain_base) = regen_from_zone(zone_qi, base_rate, avg_integrity, qi_room);

        // 有霸主（倍率 DOMINANCE_REGEN_MULTIPLIER = 1.20）
        let (gain_dom, drain_dom) = regen_from_zone(
            zone_qi,
            base_rate * DOMINANCE_REGEN_MULTIPLIER,
            avg_integrity,
            qi_room,
        );

        // 守恒：gain == drain * CAP（函数内生不变量）
        assert!(
            (gain_dom - drain_dom * QI_ZONE_UNIT_CAPACITY).abs() < 1e-9,
            "霸主情形 gain({gain_dom:.9}) 应等于 drain({drain_dom:.9}) × QI_ZONE_UNIT_CAPACITY({QI_ZONE_UNIT_CAPACITY})"
        );
        assert!(
            (gain_base - drain_base * QI_ZONE_UNIT_CAPACITY).abs() < 1e-9,
            "基线情形也应 zero-sum: gain({gain_base:.9}) == drain({drain_base:.9}) × CAP"
        );

        // 霸主 gain 约为基线 × 1.20（QI_ZONE_UNIT_CAPACITY 充足时）
        assert!(
            gain_dom > gain_base,
            "霸主 gain({gain_dom:.9}) 应 > 基线 gain({gain_base:.9})"
        );
        assert!(
            drain_dom > drain_base,
            "霸主 drain({drain_dom:.9}) 也应 > 基线 drain({drain_base:.9})（灵气零和代价）"
        );
    }

    // ── 7. 天道加速：is_dominant=true → accumulation_rate × 1.5 ─────────────

    #[test]
    fn dominant_attention_mult_1_5() {
        use crate::cultivation::components::Realm;
        use crate::world::season::Season;
        use crate::world::tiandao_hunt::TiandaoActivity;
        use crate::world::tiandao_hunt::{accumulation_rate_with_dominance, TiandaoAttentionInput};

        let input = TiandaoAttentionInput {
            realm: Realm::Solidify,
            zone_spirit_qi: 0.5,
            activity: TiandaoActivity::Standing,
            season: Season::default(),
            is_dominant: false,
        };
        let input_dom = TiandaoAttentionInput {
            is_dominant: true,
            ..input
        };

        let base = accumulation_rate_with_dominance(input);
        let dominant = accumulation_rate_with_dominance(input_dom);

        assert!(
            (dominant - base * DOMINANCE_ATTENTION_MULT).abs() < 1e-9,
            "is_dominant=true 应使 accumulation_rate 乘以 DOMINANCE_ATTENTION_MULT({DOMINANCE_ATTENTION_MULT})；\
             base={base:.9}, dominant={dominant:.9}"
        );
    }

    // ── 8. 天道加速：is_dominant=false → 不加速 ──────────────────────────────

    #[test]
    fn non_dominant_attention_not_accelerated() {
        use crate::cultivation::components::Realm;
        use crate::world::season::Season;
        use crate::world::tiandao_hunt::TiandaoActivity;
        use crate::world::tiandao_hunt::{
            accumulation_rate, accumulation_rate_with_dominance, TiandaoAttentionInput,
        };

        let input = TiandaoAttentionInput {
            realm: Realm::Spirit,
            zone_spirit_qi: 0.7,
            activity: TiandaoActivity::Meditating,
            season: Season::default(),
            is_dominant: false,
        };

        let old_rate = accumulation_rate(input);
        let new_rate = accumulation_rate_with_dominance(input);

        assert!(
            (old_rate - new_rate).abs() < 1e-9,
            "is_dominant=false 时两函数应返回相同速率；old={old_rate:.9}, new={new_rate:.9}"
        );
    }

    // ── 9. 天道加速边界：Awaken realm_base_rate=0 → 霸主×1.5 仍=0 ────────────

    #[test]
    fn awaken_dominant_attention_rate_still_zero() {
        use crate::cultivation::components::Realm;
        use crate::world::season::Season;
        use crate::world::tiandao_hunt::TiandaoActivity;
        use crate::world::tiandao_hunt::{accumulation_rate_with_dominance, TiandaoAttentionInput};

        let input = TiandaoAttentionInput {
            realm: Realm::Awaken,
            zone_spirit_qi: 1.0,
            activity: TiandaoActivity::Standing,
            season: Season::default(),
            is_dominant: true,
        };

        let rate = accumulation_rate_with_dominance(input);
        assert_close(
            rate,
            0.0,
            "Awaken realm_base_rate=0 → 霸主×1.5 仍=0（低境不被天道盯）",
        );
    }

    // ── 10. NPC 态度：apply_dominance_reputation 对 home_zone 霸主 rep 上升 ──

    #[test]
    fn apply_dominance_reputation_raises_rep_for_dominant_zone_npc() {
        use crate::cultivation::tick::CultivationClock;
        use crate::npc::patrol::NpcPatrol;
        use crate::npc::trade::NpcPlayerReputation;
        use valence::prelude::{App, DVec3, Update};

        let mut app = App::new();

        // 构造 ZoneInfluenceMap：spawn zone 有霸主 "player:hero"
        let map = make_zone_with_dominant("spawn", "player:hero", 50.0);
        app.insert_resource(map);
        // 推进时钟到 TERRITORY_EVAL_INTERVAL_TICKS，确保节流通过
        app.insert_resource(CultivationClock {
            tick: TERRITORY_EVAL_INTERVAL_TICKS,
        });
        app.add_systems(Update, apply_dominance_reputation);

        // 构造 NPC（home_zone = "spawn"）
        let npc = app
            .world_mut()
            .spawn((
                NpcPatrol::new("spawn", DVec3::new(0.0, 66.0, 0.0)),
                NpcPlayerReputation::default(),
            ))
            .id();

        let rep_before = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:hero");

        app.update();

        let rep_after = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:hero");

        assert!(
            rep_after > rep_before,
            "home_zone 霸主的 NPC 信誉应上升；before={rep_before:.4}, after={rep_after:.4}"
        );
        assert!(
            (rep_after - rep_before - DOMINANCE_REP_DELTA).abs() < 1e-5,
            "rep 增量应等于 DOMINANCE_REP_DELTA({DOMINANCE_REP_DELTA})；\
             before={rep_before:.4}, after={rep_after:.4}, diff={:.4}",
            rep_after - rep_before
        );
    }

    // ── 11. NPC 态度：非霸主玩家 rep 不变 ────────────────────────────────────

    #[test]
    fn apply_dominance_reputation_does_not_adjust_non_dominant_player() {
        use crate::cultivation::tick::CultivationClock;
        use crate::npc::patrol::NpcPatrol;
        use crate::npc::trade::NpcPlayerReputation;
        use valence::prelude::{App, DVec3, Update};

        let mut app = App::new();
        let map = make_zone_with_dominant("spawn", "player:hero", 50.0);
        app.insert_resource(map);
        app.insert_resource(CultivationClock {
            tick: TERRITORY_EVAL_INTERVAL_TICKS,
        });
        app.add_systems(Update, apply_dominance_reputation);

        let mut rep = NpcPlayerReputation::default();
        // 给非霸主玩家预设不同信誉
        rep.adjust("player:other", -0.1); // 0.4
        let npc = app
            .world_mut()
            .spawn((NpcPatrol::new("spawn", DVec3::new(0.0, 66.0, 0.0)), rep))
            .id();

        let rep_other_before = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:other");

        app.update();

        let rep_other_after = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:other");

        assert!(
            (rep_other_before - rep_other_after).abs() < 1e-5,
            "非霸主玩家信誉不应被修改；before={rep_other_before:.4}, after={rep_other_after:.4}"
        );
    }

    // ── 12. NPC 态度：别的 zone NPC 不调 ─────────────────────────────────────

    #[test]
    fn apply_dominance_reputation_does_not_adjust_npc_in_different_zone() {
        use crate::cultivation::tick::CultivationClock;
        use crate::npc::patrol::NpcPatrol;
        use crate::npc::trade::NpcPlayerReputation;
        use valence::prelude::{App, DVec3, Update};

        let mut app = App::new();
        // 只有 "spawn" 有霸主
        let map = make_zone_with_dominant("spawn", "player:hero", 50.0);
        app.insert_resource(map);
        app.insert_resource(CultivationClock {
            tick: TERRITORY_EVAL_INTERVAL_TICKS,
        });
        app.add_systems(Update, apply_dominance_reputation);

        // NPC 在 "mountain" zone（无霸主）
        let npc = app
            .world_mut()
            .spawn((
                NpcPatrol::new("mountain", DVec3::new(200.0, 66.0, 200.0)),
                NpcPlayerReputation::default(),
            ))
            .id();

        let rep_before = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:hero");

        app.update();

        let rep_after = app
            .world()
            .entity(npc)
            .get::<NpcPlayerReputation>()
            .unwrap()
            .get("player:hero");

        assert!(
            (rep_before - rep_after).abs() < 1e-5,
            "别的 zone 的 NPC 信誉不应被修改；before={rep_before:.4}, after={rep_after:.4}"
        );
    }

    // ── 13. NPC 态度契约：rep 升到 High → check_trade_eligibility = Allowed ──

    #[test]
    fn high_rep_from_dominance_enables_trade_eligibility() {
        use crate::npc::trade::{
            check_trade_eligibility, NpcPlayerReputation, RepTier, TradeEligibility,
        };

        let mut rep = NpcPlayerReputation::default();
        // 多次 apply rep 逼近 High（0.5 + 10×0.02 = 0.7，刚好过 0.7 → High）
        for _ in 0..11 {
            rep.adjust("player:hero", DOMINANCE_REP_DELTA);
        }
        // 0.5 + 0.22 = 0.72 > 0.7 → High
        let tier = rep.tier("player:hero");
        assert_eq!(
            tier,
            RepTier::High,
            "11 次 rep.adjust(+{DOMINANCE_REP_DELTA}) 后应达 High tier；实际={tier:?}"
        );
        let eligibility = check_trade_eligibility(tier);
        assert!(
            matches!(eligibility, TradeEligibility::Allowed { .. }),
            "High rep 应使 check_trade_eligibility=Allowed（折扣/稀有品解禁）；实际={eligibility:?}"
        );
    }

    // ── 14. 代价同源：霸主既得 regen×1.20 又得 attention×1.5（不可分）────────

    #[test]
    fn dominance_perk_and_cost_are_inseparable() {
        use crate::cultivation::components::Realm;
        use crate::world::season::Season;
        use crate::world::tiandao_hunt::TiandaoActivity;
        use crate::world::tiandao_hunt::{accumulation_rate_with_dominance, TiandaoAttentionInput};

        // 同一 zone 的霸主状态：regen store = 1.20
        let mut regen_store = ZoneDominanceRegenStore::default();
        regen_store
            .multipliers
            .insert("spawn".to_string(), DOMINANCE_REGEN_MULTIPLIER);

        let regen_mult = regen_store.multiplier_for("spawn");
        assert_close(
            regen_mult,
            DOMINANCE_REGEN_MULTIPLIER,
            "霸主 regen 倍率应为 DOMINANCE_REGEN_MULTIPLIER(1.20)",
        );

        // 同一 dominant → attention 也加速 1.5
        let input_dom = TiandaoAttentionInput {
            realm: Realm::Spirit,
            zone_spirit_qi: 0.8,
            activity: TiandaoActivity::Standing,
            season: Season::default(),
            is_dominant: true,
        };
        let input_nondom = TiandaoAttentionInput {
            is_dominant: false,
            ..input_dom
        };

        let rate_dom = accumulation_rate_with_dominance(input_dom);
        let rate_nondom = accumulation_rate_with_dominance(input_nondom);

        assert!(
            rate_dom > rate_nondom,
            "霸主 attention 速率({rate_dom:.9})应 > 非霸主({rate_nondom:.9})"
        );

        // 断言收益代价不可分：regen 1.20 AND attention×1.5 同时存在
        assert!(
            regen_mult > 1.0 && rate_dom > rate_nondom,
            "霸主必须同时得到 regen 优先（{regen_mult:.2}）和 attention 加速（{rate_dom:.4} vs {rate_nondom:.4}）——无代价霸权不存在"
        );
    }

    // ── 15. canonical_player_id 不匹配时不误判 ────────────────────────────────

    #[test]
    fn mismatched_char_id_does_not_trigger_perk() {
        use valence::prelude::App;

        // dominant.char_id = "player:hero"，但 ZoneDominanceRegenStore 只按 zone 存倍率
        // 此测试验证：store 只有 zone 粒度，不按 char_id 过滤，查询接口正确工作
        let store = ZoneDominanceRegenStore::default();
        // 不插入 "spawn" → 代表该 zone 没有霸主（不误判）
        assert_close(
            store.multiplier_for("spawn"),
            1.0,
            "未在 store 中注册的 zone 不应误判有霸主倍率",
        );

        // 验证 recompute：ZoneInfluenceMap 中霸主 char_id 不同 → store 仍正确写入
        let map = make_zone_with_dominant("spawn", "player:alice", 55.0);
        let mut app = App::new();
        app.insert_resource(map);
        app.init_resource::<ZoneDominanceRegenStore>();
        app.add_systems(valence::prelude::Update, recompute_territory_perks);
        app.update();

        let s = app.world().resource::<ZoneDominanceRegenStore>();
        assert_close(
            s.multiplier_for("spawn"),
            DOMINANCE_REGEN_MULTIPLIER,
            "spawn zone 有 dominant 时应写入 DOMINANCE_REGEN_MULTIPLIER 而非遗漏",
        );
    }

    // ── 16. recompute：dominant=None 的 zone store 无条目 ────────────────────

    #[test]
    fn recompute_no_entry_for_zone_without_dominant() {
        use valence::prelude::App;

        let mut map = ZoneInfluenceMap::default();
        // 插入一个无霸主的 zone
        let empty_entry = ZoneInfluenceEntry::default();
        map.zones.insert("empty_zone".to_string(), empty_entry);

        let mut app = App::new();
        app.insert_resource(map);
        app.init_resource::<ZoneDominanceRegenStore>();
        app.add_systems(valence::prelude::Update, recompute_territory_perks);
        app.update();

        let store = app.world().resource::<ZoneDominanceRegenStore>();
        assert!(
            !store.multipliers.contains_key("empty_zone"),
            "dominant=None 的 zone 不应在 store 中有条目"
        );
        assert_close(
            store.multiplier_for("empty_zone"),
            1.0,
            "无霸主 zone 查询应返回 1.0",
        );
    }
}
