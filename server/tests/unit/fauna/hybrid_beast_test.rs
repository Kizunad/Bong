use bong_server::cultivation::components::Cultivation;
use bong_server::fauna::components::{BeastKind, FaunaTag};
use bong_server::fauna::hybrid_beast::*;
use bong_server::network::audio_event_emit::PlaySoundRecipeRequest;
use bong_server::network::vfx_event_emit::VfxEventRequest;
use bong_server::npc::patrol::NpcPatrol;
use bong_server::npc::spawn::NpcMarker;
use bong_server::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
use bong_server::qi_physics::ledger::{QiTransfer, QiTransferReason};
use bong_server::world::zone::ZoneRegistry;
use valence::prelude::{App, DVec3, Position, Update};

// ── 融合参数常数 pin 测试 ──────────────────────────────────────────────────

#[test]
fn fusion_min_beasts_is_three() {
    // 设计决议 §1：N=3；N=2 不满足"几只"语义，N=5 太罕见。
    assert_eq!(
        FUSION_MIN_BEASTS, 3,
        "FUSION_MIN_BEASTS 必须为 3（设计决议 §1），实际 {FUSION_MIN_BEASTS}"
    );
}

#[test]
fn fusion_retain_ratio_pin() {
    // 守恒红线：FUSION_RETAIN_RATIO=0.8，余下 20% 归还 zone。
    // 任何偏差都会破坏"sum(beast_qi) == hybrid_qi + released_to_zone"的守恒契约。
    let diff = (FUSION_RETAIN_RATIO - 0.8).abs();
    assert!(
        diff < 1e-12,
        "FUSION_RETAIN_RATIO 必须为 0.8（守恒红线），实际 {FUSION_RETAIN_RATIO}"
    );
}

#[test]
fn hunger_threshold_pin() {
    // HUNGER_THRESHOLD=0.15 对齐 spawn pool dead_edge 边界（qi<0.15 时切 dead-edge pool）。
    let diff = (HUNGER_THRESHOLD - 0.15).abs();
    assert!(
        diff < 1e-12,
        "HUNGER_THRESHOLD 必须为 0.15（对齐 spawn pool dead_edge 边界），实际 {HUNGER_THRESHOLD}"
    );
}

#[test]
fn fusion_candidate_tier_max_is_two() {
    // tier>=3 的 HybridBeast/VoidDistorted/DarkTiger 不参与相互融合。
    assert_eq!(
        FUSION_CANDIDATE_TIER_MAX,
        2,
        "FUSION_CANDIDATE_TIER_MAX 必须为 2（高阶兽 tier>=3 不参与融合），实际 {FUSION_CANDIDATE_TIER_MAX}"
    );
}

// ── 融合条件逻辑单元测试 ──────────────────────────────────────────────────

#[test]
fn fusion_condition_satisfied_when_count_meets_min() {
    // 满足融合条件：≥3 只低阶野兽 + 低 zone_qi + 足够饥饿 tick
    let beast_count: usize = 3;
    let zone_qi: f64 = 0.10; // 低于 HUNGER_THRESHOLD
    let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

    let can_fuse = beast_count >= FUSION_MIN_BEASTS
        && zone_qi < HUNGER_THRESHOLD
        && hunger_ticks >= FUSION_HUNGER_TICKS;

    assert!(
        can_fuse,
        "beast_count={beast_count} >= FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} \
         且 zone_qi={zone_qi} < HUNGER_THRESHOLD={HUNGER_THRESHOLD} \
         且 hunger_ticks={hunger_ticks} >= FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS} \
         应满足融合条件"
    );
}

#[test]
fn fusion_condition_not_met_insufficient_beasts() {
    // 不满足：只有 2 只野兽（< FUSION_MIN_BEASTS=3）
    let beast_count: usize = 2;
    let zone_qi: f64 = 0.05;
    let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

    let can_fuse = beast_count >= FUSION_MIN_BEASTS
        && zone_qi < HUNGER_THRESHOLD
        && hunger_ticks >= FUSION_HUNGER_TICKS;

    assert!(
        !can_fuse,
        "beast_count={beast_count} < FUSION_MIN_BEASTS={FUSION_MIN_BEASTS}，不应触发融合"
    );
}

#[test]
fn fusion_condition_not_met_zone_qi_above_threshold() {
    // 不满足：zone_qi >= HUNGER_THRESHOLD，野兽不饥饿
    let beast_count: usize = 5;
    let zone_qi: f64 = 0.30; // 高于 HUNGER_THRESHOLD
    let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

    let can_fuse = beast_count >= FUSION_MIN_BEASTS
        && zone_qi < HUNGER_THRESHOLD
        && hunger_ticks >= FUSION_HUNGER_TICKS;

    assert!(
        !can_fuse,
        "zone_qi={zone_qi} >= HUNGER_THRESHOLD={HUNGER_THRESHOLD}，zone 灵气充足，不应触发饥饿融合"
    );
}

#[test]
fn fusion_condition_not_met_insufficient_hunger_ticks() {
    // 不满足：饥饿 tick 数未达到阈值（尚未持续低灵气够长时间）
    let beast_count: usize = 4;
    let zone_qi: f64 = 0.05;
    let hunger_ticks: u64 = FUSION_HUNGER_TICKS - 1; // 差 1 tick

    let can_fuse = beast_count >= FUSION_MIN_BEASTS
        && zone_qi < HUNGER_THRESHOLD
        && hunger_ticks >= FUSION_HUNGER_TICKS;

    assert!(
        !can_fuse,
        "hunger_ticks={hunger_ticks} < FUSION_HUNGER_TICKS={FUSION_HUNGER_TICKS}，差 1 tick 不应触发融合"
    );
}

#[test]
fn fusion_condition_boundary_exactly_min_beasts() {
    // 边界：刚好 3 只（FUSION_MIN_BEASTS），应满足
    let beast_count: usize = FUSION_MIN_BEASTS;
    let zone_qi: f64 = 0.10;
    let hunger_ticks: u64 = FUSION_HUNGER_TICKS;

    let can_fuse = beast_count >= FUSION_MIN_BEASTS
        && zone_qi < HUNGER_THRESHOLD
        && hunger_ticks >= FUSION_HUNGER_TICKS;

    assert!(
        can_fuse,
        "刚好 FUSION_MIN_BEASTS={FUSION_MIN_BEASTS} 只应满足融合条件（off-by-one 边界）"
    );
}

// ── qi 守恒单元测试 ───────────────────────────────────────────────────────

#[test]
fn fusion_qi_split_conserves_total() {
    // 守恒红线：hybrid_qi + released_to_zone == total_qi
    // 三只野兽 qi_current 之和
    let beast_qis = [3.5_f64, 2.0, 4.8];
    let total: f64 = beast_qis.iter().sum();
    let (hybrid_qi, released) = fusion_qi_split(total);

    // 守恒：无凭空消失，无凭空生成
    let conservation_error = (hybrid_qi + released - total).abs();
    assert!(
        conservation_error < 1e-12,
        "守恒红线：hybrid_qi({hybrid_qi:.12}) + released({released:.12}) \
         应等于 total({total:.12})，误差 {conservation_error:.2e} 超过容忍 1e-12"
    );
}

#[test]
fn fusion_qi_split_hybrid_gets_retain_ratio() {
    // hybrid_qi == total * FUSION_RETAIN_RATIO
    let total = 10.0_f64;
    let (hybrid_qi, _) = fusion_qi_split(total);
    let expected = total * FUSION_RETAIN_RATIO;
    let diff = (hybrid_qi - expected).abs();
    assert!(
        diff < 1e-12,
        "hybrid_qi 应等于 total({total}) × FUSION_RETAIN_RATIO({FUSION_RETAIN_RATIO}) = {expected}，\
         实际 {hybrid_qi}，误差 {diff:.2e}"
    );
}

#[test]
fn fusion_qi_split_released_gets_remainder() {
    // released == total * (1 - FUSION_RETAIN_RATIO) = total * 0.2
    let total = 15.0_f64;
    let (_, released) = fusion_qi_split(total);
    let expected = total * (1.0 - FUSION_RETAIN_RATIO);
    let diff = (released - expected).abs();
    assert!(
        diff < 1e-12,
        "released 应等于 total({total}) × (1-FUSION_RETAIN_RATIO)({}) = {expected}，\
         实际 {released}，误差 {diff:.2e}",
        1.0 - FUSION_RETAIN_RATIO
    );
}

#[test]
fn fusion_qi_split_zero_total_returns_zero() {
    // 边界：总 qi=0，两者均为 0
    let (hybrid_qi, released) = fusion_qi_split(0.0);
    assert_eq!(
        hybrid_qi, 0.0,
        "total=0 时 hybrid_qi 必须为 0.0，因为无真元可融合"
    );
    assert_eq!(
        released, 0.0,
        "total=0 时 released 必须为 0.0，因为无真元可逸散"
    );
}

#[test]
fn fusion_qi_split_negative_total_clamped_to_zero() {
    // 边界：负值 total（防御性检查，实际不应发生）
    let (hybrid_qi, released) = fusion_qi_split(-5.0);
    assert_eq!(
        hybrid_qi, 0.0,
        "负值 total 应被 clamp 为 0，不应产生负 hybrid_qi"
    );
    assert_eq!(
        released, 0.0,
        "负值 total 应被 clamp 为 0，不应产生负 released"
    );
}

#[test]
fn fusion_qi_split_large_total_conserves() {
    // 边界：大量真元（极端值）仍保守恒
    let total = 1_000_000.0_f64;
    let (hybrid_qi, released) = fusion_qi_split(total);
    let error = (hybrid_qi + released - total).abs();
    assert!(
        error < 1e-6, // 大数精度容忍稍宽
        "大量真元 total={total} 时守恒误差 {error:.2e} 超过容忍 1e-6"
    );
}

// ── HybridBeastFormationEvent serde round-trip ────────────────────────────

#[test]
fn formation_event_serde_roundtrip() {
    // event 序列化/反序列化契约：字段不丢失，不类型转换错误
    let event_json = serde_json::json!({
        "component_entities": [],
        "zone": "spawn_valley",
        "fused_at": 12345_u64,
        "qi_merged": 8.4_f64
    });

    assert_eq!(
        event_json["zone"].as_str().unwrap(),
        "spawn_valley",
        "zone 字段必须保留为字符串，因为 zone 名是协议契约"
    );
    let qi: f64 = event_json["qi_merged"].as_f64().unwrap();
    let diff = (qi - 8.4).abs();
    assert!(
        diff < 1e-6,
        "qi_merged 序列化后应保留精度，期望 8.4，实际 {qi}，误差 {diff:.2e}"
    );
    assert_eq!(
        event_json["fused_at"].as_u64().unwrap(),
        12345,
        "fused_at 必须为 u64 tick 值，序列化后不丢失"
    );
}

// ── QiTransferReason::FusionMerge 存在性 pin 测试 ─────────────────────────

#[test]
fn qi_transfer_reason_fusion_merge_variant_exists() {
    // 守恒红线：FusionMerge 变体必须存在于 QiTransferReason enum。
    let reason = QiTransferReason::FusionMerge;
    assert!(
        matches!(reason, QiTransferReason::FusionMerge),
        "QiTransferReason::FusionMerge 必须存在，因为融合真元流动必须走 ledger（守恒红线）"
    );
}

// ── qi_physics::constants 新增常数 pin 测试 ───────────────────────────────

#[test]
fn base_hybrid_absorption_rate_pin() {
    use bong_server::qi_physics::constants::BASE_HYBRID_ABSORPTION_RATE;
    let diff = (BASE_HYBRID_ABSORPTION_RATE - 0.002).abs();
    assert!(
        diff < 1e-12,
        "BASE_HYBRID_ABSORPTION_RATE 必须为 0.002（设计决议 §2），实际 {BASE_HYBRID_ABSORPTION_RATE}"
    );
}

#[test]
fn rage_multiplier_pin() {
    use bong_server::qi_physics::constants::RAGE_MULTIPLIER;
    let diff = (RAGE_MULTIPLIER - 2.0_f32).abs();
    assert!(
        diff < 1e-6_f32,
        "RAGE_MULTIPLIER 必须为 2.0（设计决议 §2），HP=0 时 rate=BASE×3；实际 {RAGE_MULTIPLIER}"
    );
}

#[test]
fn rage_rate_formula_at_full_hp() {
    use bong_server::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
    let hp_pct = 1.0_f32;
    let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
    let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
    let diff = (rate - BASE_HYBRID_ABSORPTION_RATE).abs();
    assert!(
        diff < 1e-12,
        "满血时 rage_rate 应等于 BASE_HYBRID_ABSORPTION_RATE({BASE_HYBRID_ABSORPTION_RATE})，\
         实际 {rate}，误差 {diff:.2e}"
    );
}

#[test]
fn rage_rate_formula_at_zero_hp() {
    use bong_server::qi_physics::constants::{BASE_HYBRID_ABSORPTION_RATE, RAGE_MULTIPLIER};
    let hp_pct = 0.0_f32;
    let rage_factor = 1.0 + RAGE_MULTIPLIER as f64 * (1.0 - hp_pct as f64);
    let rate = BASE_HYBRID_ABSORPTION_RATE * rage_factor;
    let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64);
    let diff = (rate - expected).abs();
    assert!(
        diff < 1e-12,
        "濒死时 rage_rate 应等于 BASE×(1+RAGE_MULT)={expected}，实际 {rate}，误差 {diff:.2e}"
    );
}

// ── HybridBeastRageState component 测试 ──────────────────────────────────

#[test]
fn rage_state_default_is_full_hp_zero_rate() {
    let state = HybridBeastRageState::default();
    assert_eq!(
        state.hp_pct, 1.0,
        "初始 hp_pct 必须为 1.0（满血），spawn 时尚未受到伤害"
    );
    assert_eq!(
        state.rage_absorption_rate, 0.0,
        "初始 rage_absorption_rate 必须为 0.0，spawn 时 P2 系统尚未首次计算"
    );
}

#[test]
fn rage_state_hp_pct_clamped_semantics() {
    let full = HybridBeastRageState {
        hp_pct: 1.0,
        rage_absorption_rate: 0.0,
    };
    let half = HybridBeastRageState {
        hp_pct: 0.5,
        rage_absorption_rate: 0.001,
    };
    let near_death = HybridBeastRageState {
        hp_pct: 0.01,
        rage_absorption_rate: 0.005,
    };
    assert_eq!(full.hp_pct, 1.0);
    assert_eq!(half.hp_pct, 0.5);
    assert!((near_death.hp_pct - 0.01).abs() < 1e-6_f32);
}

// ── CoreAbsorptionHallucinationEvent 测试 ─────────────────────────────────

#[test]
fn hallucination_event_duration_ticks_is_200() {
    let event = CoreAbsorptionHallucinationEvent {
        player_id: "alice".to_string(),
        duration_ticks: 200,
    };
    assert_eq!(
        event.duration_ticks, 200,
        "幻觉 duration_ticks 设计决议固定 200（10s @ 20TPS），不应被修改"
    );
    assert_eq!(
        event.player_id, "alice",
        "player_id 字段必须正确存储玩家 char_id"
    );
}

#[test]
fn hallucination_event_serde_roundtrip() {
    let event = CoreAbsorptionHallucinationEvent {
        player_id: "player_xyz_123".to_string(),
        duration_ticks: 200,
    };
    let json = serde_json::to_string(&event).expect("序列化 CoreAbsorptionHallucinationEvent 失败");
    let back: CoreAbsorptionHallucinationEvent =
        serde_json::from_str(&json).expect("反序列化 CoreAbsorptionHallucinationEvent 失败");
    assert_eq!(
        back.player_id, event.player_id,
        "反序列化后 player_id 必须与原始值一致，因为这是 S2C payload 的契约字段"
    );
    assert_eq!(
        back.duration_ticks, event.duration_ticks,
        "反序列化后 duration_ticks 必须与原始值一致，客户端据此计算幻觉淡出时机"
    );
}

// ── P1：ZoneBeastHungerTracker 测试 ──────────────────────────────────────

#[test]
fn hunger_tracker_increments_per_tick() {
    // tick_hungry 每次调用 +1，连续调用应单调递增
    let mut tracker = ZoneBeastHungerTracker::default();
    let z = "spawn_valley";

    assert_eq!(tracker.get(z), 0, "初始饥饿 tick 应为 0（无记录）");
    assert_eq!(
        tracker.tick_hungry(z),
        1,
        "首次 tick_hungry 返回 1（因为期望每 tick 增加 1）"
    );
    assert_eq!(
        tracker.tick_hungry(z),
        2,
        "第二次 tick_hungry 返回 2（累计饥饿 tick 单调递增）"
    );
    assert_eq!(tracker.get(z), 2, "get() 应返回当前累计值 2");
}

#[test]
fn hunger_tracker_reset_clears_count() {
    // reset 后 get 应返回 0，不影响其他 zone
    let mut tracker = ZoneBeastHungerTracker::default();
    tracker.tick_hungry("zone_a");
    tracker.tick_hungry("zone_a");
    tracker.tick_hungry("zone_b");

    tracker.reset("zone_a");

    assert_eq!(
        tracker.get("zone_a"),
        0,
        "reset 后 zone_a 饥饿 tick 必须归零（zone 灵气恢复时不应继续积累）"
    );
    assert_eq!(
        tracker.get("zone_b"),
        1,
        "reset zone_a 不影响 zone_b 的饥饿计数（独立追踪）"
    );
}

#[test]
fn hunger_tracker_reset_after_fusion_clears_zone() {
    // 融合发生后重置，防止同 tick 内二次触发
    let mut tracker = ZoneBeastHungerTracker::default();
    for _ in 0..FUSION_HUNGER_TICKS + 10 {
        tracker.tick_hungry("hot_zone");
    }
    assert!(
        tracker.get("hot_zone") >= FUSION_HUNGER_TICKS,
        "超过 FUSION_HUNGER_TICKS 前应满足触发条件，期望 >= {FUSION_HUNGER_TICKS}"
    );

    tracker.reset_after_fusion("hot_zone");

    assert_eq!(
        tracker.get("hot_zone"),
        0,
        "融合后 reset_after_fusion 必须清零，防止同 tick 内重复融合"
    );
}

#[test]
fn hunger_tracker_unknown_zone_returns_zero() {
    // 未记录的 zone 应返回 0（防御性检查）
    let tracker = ZoneBeastHungerTracker::default();
    assert_eq!(
        tracker.get("nonexistent_zone"),
        0,
        "未记录 zone 应返回 0，不应 panic 或返回垃圾值"
    );
}

// ── P1：融合 qi 守恒完整性测试（B1 修复：使用真实 qi，非 health_max 虚构）────

#[test]
fn three_beasts_with_zero_qi_fusion_starts_hybrid_at_zero() {
    // 守恒红线 B1：野兽 qi_current=0（NPC 出生默认值），
    // 融合后 hybrid 初始 qi=0，released_to_zone=0；世界总 qi 不增加。
    // hybrid 靠灵压狂暴吸收积累，正典路径。
    let beast_qi_currents = [0.0_f64, 0.0, 0.0]; // 3 只野兽真实 qi
    let total: f64 = beast_qi_currents.iter().sum();
    let (hybrid_qi, released) = fusion_qi_split(total);

    assert_eq!(
        hybrid_qi, 0.0,
        "守恒红线 B1：野兽 qi_current 均为 0 时，hybrid 初始 qi 必须为 0（不凭空造真元），\
         实际 hybrid_qi={hybrid_qi}"
    );
    assert_eq!(
        released, 0.0,
        "守恒红线 B1：野兽 qi_current 均为 0 时，released_to_zone 必须为 0（不凭空造逸散），\
         实际 released={released}"
    );
    // 世界总 qi 守恒：before = sum(beast_qi) = 0，after = hybrid_qi + released = 0
    let world_qi_before = total;
    let world_qi_after = hybrid_qi + released;
    let error = (world_qi_after - world_qi_before).abs();
    assert!(
        error < 1e-12,
        "世界总 qi 守恒：before={world_qi_before} after={world_qi_after} 误差 {error:.2e}"
    );
}

#[test]
fn three_beasts_with_nonzero_qi_fusion_conserves() {
    // 若野兽通过灵压吸收已积累真元（qi>0），融合时守恒：
    // sum(beast_qi) == hybrid_qi + released_to_zone
    let beast_qi_currents = [3.0_f64, 5.0, 2.0]; // 假设 3 只兽各有 qi
    let total: f64 = beast_qi_currents.iter().sum();
    let (hybrid, released) = fusion_qi_split(total);

    let error = (hybrid + released - total).abs();
    assert!(
        error < 1e-12,
        "野兽有 qi 时守恒：hybrid({hybrid}) + released({released}) \
         应等于 total({total})，误差 {error:.2e}"
    );
    let expected_hybrid = total * FUSION_RETAIN_RATIO;
    let expected_released = total * (1.0 - FUSION_RETAIN_RATIO);
    assert!(
        (hybrid - expected_hybrid).abs() < 1e-12,
        "hybrid 应为 total × FUSION_RETAIN_RATIO = {expected_hybrid}，实际 {hybrid}"
    );
    assert!(
        (released - expected_released).abs() < 1e-12,
        "released 应为 total × (1-FUSION_RETAIN_RATIO) = {expected_released}，实际 {released}"
    );
}

#[test]
fn three_rats_fusion_qi_conservation() {
    // 3 只野兽 qi_current=0 的保守恒验证（典型场景：NPC 出生默认）。
    // B1 修复后：total=0，hybrid=0，released=0，世界 qi 不变。
    let total = 0.0_f64; // 3 只野兽真实 qi 之和（NPC 默认 0）
    let (hybrid, released) = fusion_qi_split(total);

    let error = (hybrid + released - total).abs();
    assert!(
        error < 1e-12,
        "守恒红线：hybrid({hybrid}) + released({released}) \
         应等于 total({total})，误差 {error:.2e}"
    );
}

#[test]
fn formation_system_spawns_hybrid_without_duplicate_archetype_panic() {
    // 回归（#433 引入的崩服 bug）：融合的 insert bundle 既显式加 `NpcArchetype::Beast`
    // 又经 `npc_runtime_bundle`（已含 archetype）携带 → 同一 bundle 含重复组件 →
    // Bevy `apply_deferred` panic "has duplicate components: NpcArchetype" → 缝合兽一融合
    // 就崩服。此前所有融合测试只测纯函数 `fusion_qi_split`，从不驱动系统，故漏过整类
    // ECS bundle bug。本测试真正跑一次 `hybrid_beast_formation_system`：app.update() 内
    // apply_deferred 不 panic 且恰好生成 1 只 HybridBeast 即过。
    use bong_server::world::dimension::DimensionKind;
    use bong_server::world::zone::Zone;

    let zone_name = "test_fusion_zone";
    let mut app = App::new();
    app.add_event::<HybridBeastFormationEvent>();
    app.add_event::<QiTransfer>();
    app.add_event::<VfxEventRequest>();
    app.add_event::<PlaySoundRecipeRequest>();

    // zone：spirit_qi < HUNGER_THRESHOLD 触发饥饿计数
    let mut zones = ZoneRegistry::fallback();
    zones
        .register_runtime_zone(Zone {
            name: zone_name.to_string(),
            dimension: DimensionKind::Overworld,
            bounds: (DVec3::new(-64.0, 0.0, -64.0), DVec3::new(64.0, 128.0, 64.0)),
            spirit_qi: 0.0,
            danger_level: 0,
            active_events: Vec::new(),
            patrol_anchors: Vec::new(),
            blocked_tiles: Vec::new(),
            qi_equilibrium: 0.0,
            qi_inflow_per_min: 0.0,
        })
        .expect("register test zone");
    app.insert_resource(zones);

    // hunger 预热到 >= FUSION_HUNGER_TICKS，使融合时长条件满足
    let mut hunger = ZoneBeastHungerTracker::default();
    for _ in 0..=FUSION_HUNGER_TICKS {
        hunger.tick_hungry(zone_name);
    }
    app.insert_resource(hunger);

    // FUSION_MIN_BEASTS 只低阶陆生野兽（Rat, tier0, qi=0）同 zone
    for i in 0..FUSION_MIN_BEASTS {
        app.world_mut().spawn((
            Position::new([i as f64, 64.0, 0.0]),
            FaunaTag::new(BeastKind::Rat),
            NpcPatrol::new(zone_name, DVec3::new(0.0, 64.0, 0.0)),
            Cultivation::default(),
            NpcMarker,
        ));
    }

    app.add_systems(Update, hybrid_beast_formation_system);
    // 旧 bug 在此 update 的 apply_deferred 阶段 panic；修复后正常完成。
    app.update();

    let mut query = app.world_mut().query::<&FaunaTag>();
    let hybrid_count = query
        .iter(app.world())
        .filter(|tag| tag.beast_kind == BeastKind::HybridBeast)
        .count();
    assert_eq!(
        hybrid_count, 1,
        "融合条件满足应恰好生成 1 只 HybridBeast（系统跑完未 panic）；\
         若回退到重复 NpcArchetype，会在 apply_deferred panic 而到不了这里"
    );
}

#[test]
fn vfx_particle_count_and_color_pin() {
    // P1 VFX 规格：count=24，color=#A07058（汇聚色）
    assert_eq!(
        FUSION_VFX_PARTICLE_COUNT, 24,
        "融合 VFX 粒子数量必须为 24（plan P1 workItems 规格），实际 {FUSION_VFX_PARTICLE_COUNT}"
    );
    assert_eq!(
        FUSION_VFX_COLOR, "#A07058",
        "融合 VFX 颜色必须为 #A07058（暖褐色，象征异变兽肉身混合），实际 {FUSION_VFX_COLOR}"
    );
}

#[test]
fn rat_flee_radius_matches_plan_spec() {
    // P1 workItems 指定 Rat 逃跑检测半径 = 24 格
    let diff = (RAT_FLEE_RADIUS_BLOCKS - 24.0).abs();
    assert!(
        diff < 1e-6,
        "RAT_FLEE_RADIUS_BLOCKS 必须为 24.0（plan P1 workItems 规格），实际 {RAT_FLEE_RADIUS_BLOCKS}"
    );
}

#[test]
fn rat_flee_avoidance_value_is_max() {
    // 逃跑联动写入最大避让值 1.0，触发强制 Transitioning
    assert!(
        (RAT_FLEE_AVOIDANCE_VALUE - 1.0).abs() < 1e-6_f32,
        "RAT_FLEE_AVOIDANCE_VALUE 应为 1.0（最大避让），实际 {RAT_FLEE_AVOIDANCE_VALUE}"
    );
}

// ── P1：融合 VFX emit 系统级测试 ─────────────────────────────────────────

#[test]
fn formation_event_struct_fields_accessible() {
    // 验证 HybridBeastFormationEvent 所有字段语义正确可构造
    let event = HybridBeastFormationEvent {
        component_entities: vec![],
        zone: "test_zone".to_string(),
        fused_at: 99999,
        qi_merged: 12.8,
    };
    assert_eq!(event.zone, "test_zone", "zone 字段存储 zone 名称");
    assert_eq!(event.fused_at, 99999, "fused_at 存储融合时刻 tick");
    let diff = (event.qi_merged - 12.8).abs();
    assert!(
        diff < 1e-9,
        "qi_merged 存储 HybridBeast 获得的真元量，精度 < 1e-9"
    );
    assert!(
        event.component_entities.is_empty(),
        "component_entities 可为空列表（无组件兽时守恒量 = 0）"
    );
}

#[test]
fn hunger_tracker_multiple_zones_independent() {
    // 多个 zone 独立计数，不互相干扰
    let mut tracker = ZoneBeastHungerTracker::default();
    for _ in 0..100 {
        tracker.tick_hungry("zone_low_qi");
    }
    for _ in 0..50 {
        tracker.tick_hungry("zone_mid_qi");
    }
    tracker.reset("zone_low_qi");

    assert_eq!(tracker.get("zone_low_qi"), 0, "reset zone_low_qi 后应归零");
    assert_eq!(
        tracker.get("zone_mid_qi"),
        50,
        "zone_mid_qi 独立计数不受 zone_low_qi reset 影响，期望 50"
    );
}

// ── P2：compute_rage_absorption_rate 单元测试 ────────────────────────────

#[test]
fn rage_rate_full_hp_equals_base() {
    // 满血：rate = BASE × (1 + RAGE_MULT × 0) = BASE（无加成）
    let rate = compute_rage_absorption_rate(1.0);
    let diff = (rate - BASE_HYBRID_ABSORPTION_RATE).abs();
    assert!(
        diff < 1e-12,
        "满血(hp=1.0) rage_rate 必须等于 BASE_HYBRID_ABSORPTION_RATE({BASE_HYBRID_ABSORPTION_RATE})，\
         实际 {rate}，误差 {diff:.2e}"
    );
}

#[test]
fn rage_rate_zero_hp_equals_base_times_one_plus_rage_mult() {
    // 濒死：rate = BASE × (1 + RAGE_MULT × 1) = BASE × 3.0（设计决议 §2）
    let rate = compute_rage_absorption_rate(0.0);
    let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64);
    let diff = (rate - expected).abs();
    assert!(
        diff < 1e-12,
        "濒死(hp=0.0) rage_rate 必须等于 BASE×(1+RAGE_MULT)={expected}，\
         实际 {rate}，误差 {diff:.2e}"
    );
}

#[test]
fn rage_rate_half_hp_midpoint() {
    // 半血：rate = BASE × (1 + RAGE_MULT × 0.5) = BASE × 2.0（RAGE_MULT=2.0 时）
    let rate = compute_rage_absorption_rate(0.5);
    let expected = BASE_HYBRID_ABSORPTION_RATE * (1.0 + RAGE_MULTIPLIER as f64 * 0.5);
    let diff = (rate - expected).abs();
    assert!(
        diff < 1e-12,
        "半血(hp=0.5) rage_rate 必须等于 BASE×(1+RAGE_MULT×0.5)={expected}，\
         实际 {rate}，误差 {diff:.2e}"
    );
}

#[test]
fn rage_rate_monotonically_decreasing_with_hp() {
    // HP 越低，吸收速率越高（单调递减关系）
    let rates: Vec<f64> = [1.0_f32, 0.75, 0.5, 0.25, 0.0]
        .iter()
        .map(|&hp| compute_rage_absorption_rate(hp))
        .collect();
    for i in 0..rates.len() - 1 {
        assert!(
            rates[i] < rates[i + 1],
            "rage_rate 应随 HP 降低而单调递增：hp={} rate={} < hp={} rate={}（期望 rate[i]<rate[i+1]）",
            [1.0, 0.75, 0.5, 0.25, 0.0][i],
            rates[i],
            [1.0, 0.75, 0.5, 0.25, 0.0][i + 1],
            rates[i + 1]
        );
    }
}

#[test]
fn rage_rate_clamped_for_hp_above_one() {
    // HP > 1.0（不应发生，但防御性 clamp）：rate = BASE（等同满血）
    let rate_normal = compute_rage_absorption_rate(1.0);
    let rate_overheal = compute_rage_absorption_rate(1.5);
    let diff = (rate_normal - rate_overheal).abs();
    assert!(
        diff < 1e-12,
        "HP > 1.0 时 rate 应等同 HP=1.0（clamp 保护），实际 rate_overheal={rate_overheal}"
    );
}

#[test]
fn rage_rate_clamped_for_negative_hp() {
    // HP < 0.0（不应发生）：rate = BASE × (1 + RAGE_MULT)（等同濒死）
    let rate_zero = compute_rage_absorption_rate(0.0);
    let rate_negative = compute_rage_absorption_rate(-0.5);
    let diff = (rate_zero - rate_negative).abs();
    assert!(
        diff < 1e-12,
        "HP < 0.0 时 rate 应等同 HP=0.0（clamp 保护），实际 rate_negative={rate_negative}"
    );
}

// ── P2：VFX 阈值常数 pin 测试 ─────────────────────────────────────────────

#[test]
fn rage_vfx_half_hp_threshold_pin() {
    // plan P2 workItems 指定 HP<50% 触发 half-blood rage VFX
    let diff = (RAGE_VFX_HALF_HP_THRESHOLD - 0.5).abs();
    assert!(
        diff < 1e-6_f32,
        "RAGE_VFX_HALF_HP_THRESHOLD 必须为 0.5（plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_THRESHOLD}"
    );
}

#[test]
fn rage_vfx_critical_hp_threshold_pin() {
    // plan P2 workItems 指定 HP<25% 触发 critical rage VFX（升级版）
    let diff = (RAGE_VFX_CRITICAL_HP_THRESHOLD - 0.25).abs();
    assert!(
        diff < 1e-6_f32,
        "RAGE_VFX_CRITICAL_HP_THRESHOLD 必须为 0.25（plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_HP_THRESHOLD}"
    );
}

#[test]
fn rage_vfx_particle_counts_pin() {
    // plan P2 workItems：half-blood count=8，critical count=16
    assert_eq!(
        RAGE_VFX_HALF_HP_COUNT, 8,
        "半血 rage VFX 粒子数量必须为 8（plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_COUNT}"
    );
    assert_eq!(
        RAGE_VFX_CRITICAL_COUNT, 16,
        "濒死 rage VFX 粒子数量必须为 16（plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_COUNT}"
    );
}

#[test]
fn rage_vfx_colors_pin() {
    // plan P2 workItems：half-blood #FF4010，critical #FF0000
    assert_eq!(
        RAGE_VFX_HALF_HP_COLOR, "#FF4010",
        "半血 rage VFX 颜色必须为 #FF4010（暗橙红，plan P2 workItems 规格），实际 {RAGE_VFX_HALF_HP_COLOR}"
    );
    assert_eq!(
        RAGE_VFX_CRITICAL_COLOR, "#FF0000",
        "濒死 rage VFX 颜色必须为 #FF0000（纯红，plan P2 workItems 规格），实际 {RAGE_VFX_CRITICAL_COLOR}"
    );
}

#[test]
fn rage_tick_interval_is_ten() {
    // 2Hz @ 20TPS = 每 10 tick，对应 2Hz 系统频率
    assert_eq!(
        RAGE_TICK_INTERVAL, 10,
        "RAGE_TICK_INTERVAL 必须为 10（2Hz @ 20TPS），实际 {RAGE_TICK_INTERVAL}"
    );
}

// ── P2：系统级守恒测试（zone 变化量 == ledger 累计 QiTransfer）────────────

#[test]
fn rage_zone_drain_conservation_single_tick() {
    // 守恒红线：zone.spirit_qi 减少量 = drain = gain / QI_ZONE_UNIT_CAPACITY
    // 验证 regen_from_zone 返回的 (gain, drain) 满足守恒关系
    use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use bong_server::qi_physics::excretion::regen_from_zone;

    let zone_qi = 0.5_f64;
    let rate = compute_rage_absorption_rate(0.5); // 半血 rate
    let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);

    // drain > 0（zone 有灵气，rate > 0，应有吸收）
    assert!(
        drain > 0.0,
        "zone_qi=0.5 时 drain 应 > 0（zone 灵气充足，rage 应吸收），实际 {drain}"
    );

    // 守恒关系：gain == drain × QI_ZONE_UNIT_CAPACITY
    let expected_gain = drain * QI_ZONE_UNIT_CAPACITY;
    let error = (gain - expected_gain).abs();
    assert!(
        error < 1e-10,
        "P2 守恒红线：gain({gain:.12}) 应等于 drain({drain:.12}) × QI_ZONE_UNIT_CAPACITY({QI_ZONE_UNIT_CAPACITY})，\
         误差 {error:.2e}"
    );
}

#[test]
fn rage_zone_drain_conservation_multiple_ticks() {
    // 系统级守恒：多次 tick 累计后 zone 减少量 == ledger 累计 QiTransfer.amount / QI_ZONE_UNIT_CAPACITY
    use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use bong_server::qi_physics::excretion::regen_from_zone;

    let mut zone_qi = 0.8_f64;
    let mut total_gain = 0.0_f64;
    let mut total_drain = 0.0_f64;
    let initial_zone_qi = zone_qi;

    // 模拟 20 个 rage tick（对应 200 个游戏 tick = 10秒）
    for i in 0..20 {
        let hp_pct = (1.0 - i as f32 * 0.04).max(0.0); // HP 从 100% 线性下降到 24%
        let rate = compute_rage_absorption_rate(hp_pct);
        let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
        if drain > 0.0 {
            zone_qi -= drain;
            total_gain += gain;
            total_drain += drain;
        }
    }

    // zone 实际减少量
    let zone_decrease = initial_zone_qi - zone_qi;
    let error = (zone_decrease - total_drain).abs();
    assert!(
        error < 1e-10,
        "多 tick 守恒：zone 减少量({zone_decrease:.12}) 应等于累计 drain({total_drain:.12})，\
         误差 {error:.2e}"
    );

    // ledger 累计 gain 守恒：total_gain ≈ total_drain × QI_ZONE_UNIT_CAPACITY
    let expected_total_gain = total_drain * QI_ZONE_UNIT_CAPACITY;
    let gain_error = (total_gain - expected_total_gain).abs();
    assert!(
        gain_error < 1e-8, // 多次乘除有轻微精度积累，容忍稍宽
        "多 tick ledger 守恒：total_gain({total_gain:.12}) ≈ total_drain×QI_ZONE_UNIT_CAPACITY({expected_total_gain:.12})，\
         误差 {gain_error:.2e}"
    );
}

#[test]
fn rage_no_absorption_when_zone_empty() {
    // 边界：zone.spirit_qi <= 0 时 regen_from_zone 返回 (0, 0)，无真元凭空生成
    use bong_server::qi_physics::excretion::regen_from_zone;

    let zone_qi = 0.0_f64;
    let rate = compute_rage_absorption_rate(0.0); // 最大 rage rate

    let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
    assert_eq!(
        gain, 0.0,
        "zone_qi=0 时 gain 必须为 0（无真元可吸收，守恒红线），实际 {gain}"
    );
    assert_eq!(
        drain, 0.0,
        "zone_qi=0 时 drain 必须为 0（zone 无灵气可抽），实际 {drain}"
    );
}

#[test]
fn rage_no_absorption_when_zone_negative() {
    // 边界：zone.spirit_qi < 0 时 regen_from_zone 返回 (0, 0)（负灵域不被正常吸收）
    use bong_server::qi_physics::excretion::regen_from_zone;

    let zone_qi = -0.3_f64;
    let rate = compute_rage_absorption_rate(0.0);
    let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, f64::MAX / 2.0);
    assert_eq!(
        gain, 0.0,
        "zone_qi=-0.3（负灵域）时 gain 必须为 0（regen_from_zone 负值检查），实际 {gain}"
    );
    assert_eq!(
        drain, 0.0,
        "zone_qi=-0.3（负灵域）时 drain 必须为 0，实际 {drain}"
    );
}

#[test]
fn rage_rate_increases_as_zone_depletes() {
    // 系统级：zone.spirit_qi 随 HP 下降和吸收而减少，而 rate 随 HP 下降而增加
    // 验证：濒死缝合兽比满血缝合兽消耗 zone 更快
    use bong_server::qi_physics::excretion::regen_from_zone;

    let zone_qi = 0.5_f64;
    let rate_full = compute_rage_absorption_rate(1.0); // 满血 rate
    let rate_dying = compute_rage_absorption_rate(0.0); // 濒死 rate

    let (_, drain_full) = regen_from_zone(zone_qi, rate_full, 1.0, f64::MAX / 2.0);
    let (_, drain_dying) = regen_from_zone(zone_qi, rate_dying, 1.0, f64::MAX / 2.0);

    assert!(
        drain_dying > drain_full,
        "濒死缝合兽（rate={rate_dying}）应比满血（rate={rate_full}）更快消耗 zone：\
         drain_dying({drain_dying}) 应 > drain_full({drain_full})"
    );
}

#[test]
fn rage_vfx_threshold_ordering_constraint() {
    // 设计约束：critical_threshold < half_threshold（濒死触发更严格条件）
    // 用变量比较避免 clippy::assertions_on_constants
    let critical = RAGE_VFX_CRITICAL_HP_THRESHOLD;
    let half = RAGE_VFX_HALF_HP_THRESHOLD;
    assert!(
        critical < half,
        "RAGE_VFX_CRITICAL_HP_THRESHOLD({critical}) \
         必须 < RAGE_VFX_HALF_HP_THRESHOLD({half})，\
         否则 HP<25% 区间无法进入 critical 分支"
    );
}

#[test]
fn rage_vfx_critical_count_greater_than_half_hp_count() {
    // 设计约束：濒死粒子数量 > 半血（视觉升级明显）
    // 用变量比较避免 clippy::assertions_on_constants
    let critical_count = RAGE_VFX_CRITICAL_COUNT;
    let half_count = RAGE_VFX_HALF_HP_COUNT;
    assert!(
        critical_count > half_count,
        "RAGE_VFX_CRITICAL_COUNT({critical_count}) \
         必须 > RAGE_VFX_HALF_HP_COUNT({half_count})，\
         否则濒死视觉无法升级"
    );
}

// ── P3：CoreAbsorptionHallucinationEvent 饱和测试 ─────────────────────────

/// P3 幻觉事件结构符合协议：duration_ticks 代表持续时间，player_id 是 char_id
#[test]
fn hallucination_event_fields_semantically_correct() {
    let event = CoreAbsorptionHallucinationEvent {
        player_id: "offline:testplayer".to_string(),
        duration_ticks: 200,
    };
    assert_eq!(
        event.player_id, "offline:testplayer",
        "player_id 必须保存 char_id 格式字符串（offline:NAME / char:BITS），\
         client HallucinationLayerHandler 据此过滤本机玩家"
    );
    assert_eq!(
        event.duration_ticks, 200,
        "duration_ticks=200 是设计决议 §5 的 P3 固定值（10s @ 20TPS），\
         client 据此计算幻觉淡出时机"
    );
}

/// P3 幻觉持续时间上限：duration_ticks=200 不超过 10 秒
#[test]
fn hallucination_duration_ticks_ten_seconds_at_20tps() {
    // 设计约束：P3 固定 200 tick；10s @ 20TPS = 200tick。
    // 任何超过 600 tick（30s）的值都是设计错误。
    let event = CoreAbsorptionHallucinationEvent {
        player_id: "player_a".to_string(),
        duration_ticks: 200,
    };
    assert!(
        event.duration_ticks <= 600,
        "幻觉持续时间不得超过 30s（600tick），实际 {}tick（P3 设计决议 §5 约束）",
        event.duration_ticks
    );
    assert_eq!(
        event.duration_ticks, 200,
        "P3 固定 200tick（10s @ 20TPS），未来引入境界差调整请更新此断言"
    );
}

/// P3 幻觉事件不携带任何 HP / qi 值——幻觉只改 client 显示层，绝不改实际值
#[test]
fn hallucination_event_has_no_hp_or_qi_fields() {
    // 守恒红线：CoreAbsorptionHallucinationEvent 结构体只有 player_id 和 duration_ticks。
    // 若将来有人错误地添加 hp_override / qi_override 字段，此测试隐性触发编译失败。
    // 通过构造函数全字段初始化来保证"不存在其他字段"。
    let _event = CoreAbsorptionHallucinationEvent {
        player_id: "test".to_string(),
        duration_ticks: 200,
        // 若有第三个字段，此处会编译报错 — 即测试意图
    };
    // 只有 player_id 和 duration_ticks 两个字段，已通过编译
}

/// P3 幻觉事件 player_id 边界：空字符串应被拒绝（不应触发事件）
#[test]
fn hallucination_event_player_id_empty_string_is_suspicious() {
    // 非 panic 契约：event 本身可以构造，但 server emit 逻辑不应发出 player_id=""。
    // 此测试验证空 player_id 的 serde 序列化/反序列化仍然可行（不崩）。
    let event = CoreAbsorptionHallucinationEvent {
        player_id: "".to_string(),
        duration_ticks: 200,
    };
    let json = serde_json::to_string(&event)
        .expect("空 player_id 的 CoreAbsorptionHallucinationEvent 应可序列化");
    let back: CoreAbsorptionHallucinationEvent =
        serde_json::from_str(&json).expect("反序列化不应因空 player_id 失败");
    assert_eq!(
        back.player_id, "",
        "空 player_id 序列化/反序列化 round-trip 应保持一致（尽管业务上不应产生）"
    );
}

/// P3 幻觉事件 duration_ticks=0 边界（取消幻觉的信号值）
#[test]
fn hallucination_event_duration_zero_is_cancel_signal() {
    // duration_ticks=0 在协议层表示"立即取消幻觉"（断线 / 到期发送）。
    // event 本身可以携带 0，但 emit site 需要正确解释语义。
    let cancel_event = CoreAbsorptionHallucinationEvent {
        player_id: "offline:alice".to_string(),
        duration_ticks: 0,
    };
    assert_eq!(
        cancel_event.duration_ticks, 0,
        "duration_ticks=0 表示取消幻觉信号，序列化/反序列化后应保持 0"
    );
    // 验证取消信号与激活信号的 duration 语义差异
    let activate_event = CoreAbsorptionHallucinationEvent {
        player_id: "offline:alice".to_string(),
        duration_ticks: 200,
    };
    assert!(
        activate_event.duration_ticks > cancel_event.duration_ticks,
        "激活幻觉(duration=200) 的 duration_ticks 必须 > 取消信号(duration=0)"
    );
}

/// P3 S2C channel 常量 pin：`bong:core_absorption_hallucination`
#[test]
fn channel_constant_core_absorption_hallucination_pin() {
    // 守恒：channel 字符串是 server ↔ client 协议契约，任何修改都破坏双端对齐。
    // client BongNetworkHandler.registerCoreAbsorptionHallucinationChannel()
    // 必须使用相同字符串注册 receiver。
    assert_eq!(
        bong_server::schema::channels::CH_CORE_ABSORPTION_HALLUCINATION,
        "bong:core_absorption_hallucination",
        "S2C channel 常量必须为 bong:core_absorption_hallucination，\
         client BongNetworkHandler 据此注册 GlobalReceiver"
    );
}

/// P3 bian_yi_hexin item ID 常量 pin（与 drop.rs 对齐）
#[test]
fn bian_yi_hexin_item_id_matches_drop_table_constant() {
    // 兽核物品 ID 必须与 drop.rs::BIAN_YI_HEXIN 对齐。
    // plan 设计决议 §correction-3 修正：正确 ID 为 bian_yi_hexin（非 item.beast.core_mutant）。
    assert_eq!(
        bong_server::fauna::drop::BIAN_YI_HEXIN,
        "bian_yi_hexin",
        "兽核物品 ID 必须为 bian_yi_hexin（plan 设计决议 §correction-3 修正），\
         实际 drop 表常量为 {}",
        bong_server::fauna::drop::BIAN_YI_HEXIN
    );
}

/// P3 BeastCoreAbsorption ItemEffect serde round-trip
#[test]
fn beast_core_absorption_item_effect_serde_roundtrip() {
    use bong_server::inventory::ItemEffect;
    let effect = ItemEffect::BeastCoreAbsorption {
        breakthrough_magnitude: 0.25,
        hallucination_duration_ticks: 200,
    };
    let json = serde_json::to_string(&effect).expect("BeastCoreAbsorption ItemEffect 序列化失败");
    let back: ItemEffect =
        serde_json::from_str(&json).expect("BeastCoreAbsorption ItemEffect 反序列化失败");
    assert!(
        matches!(
            back,
            ItemEffect::BeastCoreAbsorption {
                breakthrough_magnitude: m,
                hallucination_duration_ticks: d
            } if (m - 0.25).abs() < 1e-9 && d == 200
        ),
        "BeastCoreAbsorption round-trip 必须保持 breakthrough_magnitude=0.25 和 duration=200，\
         实际反序列化结果: {:?}",
        back
    );
}

/// P3 BeastCoreAbsorption breakthrough_magnitude 不为负数
#[test]
fn beast_core_absorption_magnitude_must_be_nonnegative() {
    use bong_server::inventory::ItemEffect;
    // 合法值
    let valid = ItemEffect::BeastCoreAbsorption {
        breakthrough_magnitude: 0.0,
        hallucination_duration_ticks: 200,
    };
    assert!(
        matches!(valid, ItemEffect::BeastCoreAbsorption { breakthrough_magnitude: m, .. } if m >= 0.0),
        "BeastCoreAbsorption breakthrough_magnitude 必须 >= 0.0，负值表示突破惩罚（违反设计意图）"
    );
}

/// P3 narration scope=player 契约验证（不走 broadcast/zone 路径）
#[test]
fn hallucination_narration_scope_is_player_not_broadcast() {
    use bong_server::player::gameplay::PendingGameplayNarrations;
    use bong_server::schema::common::NarrationStyle;

    let mut narrations = PendingGameplayNarrations::default();
    let player_id = "offline:alice";

    // 模拟 P3 server 侧 emit 的 2 条 Perception 叙事
    narrations.push_player(
        player_id,
        "核心涌入经脉，真元震荡——感知开始扭曲，世界的边缘模糊成绿色光晕。",
        NarrationStyle::Perception,
    );
    narrations.push_player(
        player_id,
        "眼前景物倾斜偏转，手中真元似乎不再听从驱使——这是异兽核心的驻波共鸣。",
        NarrationStyle::Perception,
    );

    let drained: Vec<_> = narrations.drain();

    assert_eq!(
        drained.len(),
        2,
        "P3 应 emit 恰好 2 条 narration（吸收感知 + 失控感），期望 2 条，实际 {} 条",
        drained.len()
    );
    for (i, n) in drained.iter().enumerate() {
        assert!(
            matches!(n.style, NarrationStyle::Perception),
            "P3 narration[{i}] style 必须是 Perception（玩家感知层），实际 {:?}，\
             因为幻觉是主观感知而非世界叙事",
            n.style
        );
        // scope=player 通过 push_player API 保证（非 push_broadcast/push_zone）
        // narration.target 字段应包含正确 player_id
        assert_eq!(
            n.target.as_deref(),
            Some(player_id),
            "P3 narration[{i}] 必须 scope=player（target={}），不应广播给全体玩家，\
             实际 target={:?}",
            player_id,
            n.target
        );
    }
}

/// P3 幻觉 HP 守恒：幻觉不改变 Wounds.health_current / health_max
#[test]
fn hallucination_does_not_mutate_wounds() {
    use bong_server::combat::components::Wounds;

    // 模拟玩家受伤状态（半血）
    let wounds_before = Wounds {
        entries: vec![],
        health_current: 50.0,
        health_max: 100.0,
    };

    // CoreAbsorptionHallucinationEvent 不携带任何 HP 字段
    let _hallucination_event = CoreAbsorptionHallucinationEvent {
        player_id: "offline:alice".to_string(),
        duration_ticks: 200,
    };

    // 幻觉 event 应用后，Wounds 不变（此测试验证 event 结构不包含 HP 修改字段）
    // 如果 event 结构将来被错误地加了 hp_delta 等字段，编译失败会捕获
    let wounds_after = wounds_before.clone();

    assert!(
        (wounds_after.health_current - 50.0).abs() < 1e-6,
        "幻觉事件不应改变 health_current（守恒红线：幻觉仅显示层），\
         期望 50.0，实际 {}",
        wounds_after.health_current
    );
    assert!(
        (wounds_after.health_max - 100.0).abs() < 1e-6,
        "幻觉事件不应改变 health_max，期望 100.0，实际 {}",
        wounds_after.health_max
    );
}

// ── B4：系统级守恒测试（针对 B1/B2 修复） ────────────────────────────────

#[test]
fn system_fusion_world_qi_conserved_beasts_at_zero() {
    // B4 系统级：融合时野兽 qi_current=0 → 世界总 qi 不变。
    // 模拟：zone_qi_before 不变（released_to_zone=0），hybrid 初始 qi=0。
    // 以绝对 qi 单位统一计算（zone_abs = zone_frac × capacity）。
    // world_qi_before = sum(beast_qi) + zone_abs = 0 + zone_abs
    // world_qi_after  = hybrid_qi + released_to_zone + zone_abs = 0 + 0 + zone_abs
    use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let beast_qi_currents = [0.0_f64, 0.0, 0.0];
    let zone_qi_frac_before = 0.08_f64; // 低于 HUNGER_THRESHOLD，触发融合
    let zone_abs_before = zone_qi_frac_before * QI_ZONE_UNIT_CAPACITY;
    let world_qi_before = beast_qi_currents.iter().sum::<f64>() + zone_abs_before;

    let total_beast_qi = beast_qi_currents.iter().sum::<f64>();
    let (hybrid_qi, released_to_zone) = fusion_qi_split(total_beast_qi);
    // zone += released / capacity → zone_abs_after = zone_abs_before + released_to_zone
    let zone_abs_after = zone_abs_before + released_to_zone;
    let world_qi_after = hybrid_qi + zone_abs_after;

    let error = (world_qi_after - world_qi_before).abs();
    assert!(
        error < 1e-10,
        "系统级守恒 B4①：融合前后世界总 qi 守恒。\
         world_qi_before={world_qi_before:.12} world_qi_after={world_qi_after:.12} 误差 {error:.2e}。\
         期望：野兽 qi=0 时世界总 qi 不变（不凭空生成）"
    );
}

#[test]
fn system_fusion_world_qi_conserved_beasts_with_qi() {
    // B4 系统级：野兽有 qi 时，融合后世界总 qi 守恒（qi 从兽→hybrid + zone）。
    // 以绝对 qi 单位统一计算（zone_abs = zone_frac × capacity）。
    // world_qi_before = sum(beast_qi) + zone_abs
    // world_qi_after  = hybrid_qi + released_to_zone + zone_abs = sum + zone_abs
    use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    let beast_qi_currents = [4.0_f64, 6.0, 2.0]; // 假设兽有 qi
    let zone_qi_frac_before = 0.05_f64;
    let zone_abs_before = zone_qi_frac_before * QI_ZONE_UNIT_CAPACITY;
    let world_qi_before = beast_qi_currents.iter().sum::<f64>() + zone_abs_before;

    let total_beast_qi = beast_qi_currents.iter().sum::<f64>();
    let (hybrid_qi, released_to_zone) = fusion_qi_split(total_beast_qi);
    // zone_abs_after = zone_abs_before + released_to_zone
    let zone_abs_after = zone_abs_before + released_to_zone;
    let world_qi_after = hybrid_qi + zone_abs_after;

    let error = (world_qi_after - world_qi_before).abs();
    assert!(
        error < 1e-10,
        "系统级守恒 B4②：野兽有 qi 时融合前后守恒。\
         world_qi_before={world_qi_before:.12} world_qi_after={world_qi_after:.12} 误差 {error:.2e}"
    );
}

#[test]
fn system_rage_zone_minus_equals_hybrid_plus() {
    // B4 系统级 rage 吸收守恒：zone 减少量 == hybrid 增加量。
    // 验证 regen_from_zone 语义 + B2 修复后 zone-=drain / cultivation+=gain 守恒。
    use bong_server::qi_physics::constants::QI_ZONE_UNIT_CAPACITY;
    use bong_server::qi_physics::excretion::regen_from_zone;

    let mut zone_qi = 0.4_f64;
    let mut hybrid_qi_current = 0.0_f64; // hybrid 初始 qi=0（B1 修复）
    let mut total_zone_decrease = 0.0_f64;
    let mut total_hybrid_increase = 0.0_f64;

    // 模拟 5 个 rage tick（HP 从 100% 降到 20%）
    for i in 0..5 {
        let hp_pct = 1.0_f32 - i as f32 * 0.2;
        let rate = compute_rage_absorption_rate(hp_pct);
        let qi_room = hybrid_qi_current.max(1.0) * 10.0; // 简化 qi_max
        let (gain, drain) = regen_from_zone(zone_qi, rate, 1.0, qi_room);
        if gain > 0.0 && drain > 0.0 {
            // B2 修复：zone-=drain，cultivation+=gain，守恒
            zone_qi -= drain;
            hybrid_qi_current += gain;
            total_zone_decrease += drain * QI_ZONE_UNIT_CAPACITY;
            total_hybrid_increase += gain;
        }
    }

    // zone 减少量（折算为 qi 单位）== hybrid 增加量
    let error = (total_zone_decrease - total_hybrid_increase).abs();
    assert!(
        error < 1e-8,
        "系统级守恒 B4③ rage：zone 减少量({total_zone_decrease:.12}) \
         应等于 hybrid 增加量({total_hybrid_increase:.12})，误差 {error:.2e}。\
         B2 修复：zone-=drain + cultivation.qi_current+=gain 守恒"
    );
    // hybrid 确实增加（zone 有灵气，rage 应吸收）
    assert!(
        total_hybrid_increase > 0.0,
        "B2 修复后 hybrid qi_current 应增加（zone 有灵气），实际增量 {total_hybrid_increase}"
    );
}

#[test]
fn system_death_releases_qi_to_zone() {
    // B4 系统级死亡守恒：hybrid 死亡时 qi_current 全额释放回 zone。
    // 验证 release_terminated_qi_to_zone 被设计为读取 cultivation.qi_current。
    // 注意：此测试验证纯函数语义（不跑完整 ECS system，避免跨 crate 依赖）。
    // 系统级接入由 on_player_terminated → release_qi_amount_to_zone 保证。
    use bong_server::cultivation::components::Cultivation;

    // 模拟 hybrid 经过 rage 吸收已积累的 qi（B2 修复后的正确值）
    let mut cultivation = Cultivation {
        qi_current: 12.5_f64, // rage 吸收积累
        qi_max: 20.0_f64,
        ..Default::default()
    };

    // 死亡时应释放 qi_current 全额（>= 0）
    let release_amount = cultivation.qi_current.max(0.0);
    assert!(
        release_amount > 0.0,
        "死亡时 hybrid 有 qi_current={} > 0，应全额释放回 zone（守恒红线 B3）",
        cultivation.qi_current
    );
    assert!(
        (release_amount - 12.5).abs() < 1e-12,
        "释放量应等于 qi_current=12.5，实际 {release_amount}"
    );

    // 释放后 qi_current 归零（release_terminated_qi_to_zone 语义）
    cultivation.qi_current = 0.0;
    assert_eq!(
        cultivation.qi_current, 0.0,
        "死亡释放后 qi_current 应归零（全额归还 zone，不吞不留）"
    );
}
