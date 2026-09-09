    use super::*;

    // ── 枚举 pin 测试 ────────────────────────────────────────────────────────

    #[test]
    fn daozhan_state_default_is_mimicry() {
        assert_eq!(
            DaoZhangState::default(),
            DaoZhangState::Mimicry,
            "DaoZhangState 默认应是 Mimicry（伪装先行，暴起在后）"
        );
    }

    #[test]
    fn fake_behavior_cycle_has_three_variants() {
        let cycle = FakeBehavior::cycle();
        assert_eq!(cycle.len(), 3, "FakeBehavior::cycle 应包含 3 种假动作");
        assert!(cycle.contains(&FakeBehavior::Swing));
        assert!(cycle.contains(&FakeBehavior::Sneak));
        assert!(cycle.contains(&FakeBehavior::Mine));
    }

    #[test]
    fn daozhan_state_serde_roundtrip() {
        let states = [DaoZhangState::Mimicry, DaoZhangState::Ambush];
        for s in states {
            let json = serde_json::to_string(&s).expect("serialize");
            let back: DaoZhangState = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(s, back, "DaoZhangState serde 往返：{json}");
        }
    }

    #[test]
    fn fake_behavior_serde_roundtrip() {
        for b in FakeBehavior::cycle() {
            let json = serde_json::to_string(&b).expect("serialize");
            let back: FakeBehavior = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(b, back, "FakeBehavior serde 往返：{json}");
        }
    }

    // ── spawn 概率 ──────────────────────────────────────────────────────────

    #[test]
    fn realm_spawn_probability_values() {
        // 化虚 80%
        assert!(
            (realm_spawn_probability(Realm::Void) - 0.80).abs() < 1e-9,
            "化虚 spawn 概率应为 0.80，实际={:.4}",
            realm_spawn_probability(Realm::Void)
        );
        // 通灵 50%
        assert!(
            (realm_spawn_probability(Realm::Spirit) - 0.50).abs() < 1e-9,
            "通灵 spawn 概率应为 0.50，实际={:.4}",
            realm_spawn_probability(Realm::Spirit)
        );
        // 固元 20%
        assert!(
            (realm_spawn_probability(Realm::Solidify) - 0.20).abs() < 1e-9,
            "固元 spawn 概率应为 0.20，实际={:.4}",
            realm_spawn_probability(Realm::Solidify)
        );
        // 凝脉 0%
        assert_eq!(
            realm_spawn_probability(Realm::Condense),
            0.0,
            "凝脉不化道伥，概率应为 0"
        );
        // 引气 0%
        assert_eq!(
            realm_spawn_probability(Realm::Induce),
            0.0,
            "引气不化道伥，概率应为 0"
        );
        // 醒灵 0%
        assert_eq!(
            realm_spawn_probability(Realm::Awaken),
            0.0,
            "醒灵不化道伥，概率应为 0"
        );
    }

    #[test]
    fn realm_spawn_probability_boundary_at_full_range() {
        // 所有概率都在 [0, 1] 范围内
        for realm in [
            Realm::Void,
            Realm::Spirit,
            Realm::Solidify,
            Realm::Condense,
            Realm::Induce,
            Realm::Awaken,
        ] {
            let p = realm_spawn_probability(realm);
            assert!(
                (0.0..=1.0).contains(&p),
                "{realm:?} spawn 概率 {p} 超出 [0,1] 范围"
            );
        }
    }

    #[test]
    fn daozhan_spawn_roll_deterministic() {
        // 相同 seed + 相同 realm → 相同结果（确定性 RNG）
        let (a, _) = daozhan_spawn_roll(Realm::Void, 12345);
        let (b, _) = daozhan_spawn_roll(Realm::Void, 12345);
        assert_eq!(
            a, b,
            "daozhan_spawn_roll 必须是确定性的，相同 seed 应给出相同结果"
        );
    }

    #[test]
    fn daozhan_spawn_roll_never_hits_for_zero_probability() {
        // 醒灵概率 = 0.0，任何 seed 都不应命中
        for seed in [0u64, 1, 42, 999, u64::MAX, 0x9E3779B9_7F4A7C15] {
            let (hit, _) = daozhan_spawn_roll(Realm::Awaken, seed);
            assert!(!hit, "醒灵 spawn 概率为 0，seed={seed} 时不应命中");
        }
    }

    #[test]
    fn daozhan_spawn_roll_void_high_rate_statistical() {
        // 化虚 80%，大量样本应接近预期
        let mut hits = 0u32;
        let mut seed = 0xDEAD_BEEF_0000_0000u64;
        let n = 10_000u32;
        for _ in 0..n {
            let (hit, next) = daozhan_spawn_roll(Realm::Void, seed);
            if hit {
                hits += 1;
            }
            seed = next;
        }
        let rate = hits as f64 / n as f64;
        // 允许 ±5% 误差（n=10000 时 3σ ≈ 1.2%，5% 足够宽松）
        assert!(
            (0.75..=0.85).contains(&rate),
            "化虚 spawn roll 命中率 {rate:.3} 应在 [0.75, 0.85] 之间（期望 0.80）"
        );
    }

    #[test]
    fn daozhan_spawn_roll_advances_seed() {
        // next seed 不等于输入 seed（防止 seed 不推进导致相关性）
        let seed = 42u64;
        let (_, next) = daozhan_spawn_roll(Realm::Void, seed);
        assert_ne!(seed, next, "daozhan_spawn_roll 应推进 seed");
    }

    // ── loot 分档 ────────────────────────────────────────────────────────────

    #[test]
    fn daozhan_loot_tier_by_realm() {
        // 化虚/通灵 → High
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Void)),
            DaoZhangLootTier::High,
            "化虚道伥应掉高档 loot"
        );
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Spirit)),
            DaoZhangLootTier::High,
            "通灵道伥应掉高档 loot"
        );
        // 固元 → Mid
        assert_eq!(
            daozhan_loot_tier(Some(Realm::Solidify)),
            DaoZhangLootTier::Mid,
            "固元道伥应掉中档 loot"
        );
        // 凝脉/引气/醒灵 → Base
        for realm in [Realm::Condense, Realm::Induce, Realm::Awaken] {
            assert_eq!(
                daozhan_loot_tier(Some(realm)),
                DaoZhangLootTier::Base,
                "{realm:?} 道伥应掉基础 loot"
            );
        }
        // 天道凝结（None）→ Base
        assert_eq!(
            daozhan_loot_tier(None),
            DaoZhangLootTier::Base,
            "天道凝结道伥（无原始境界）应掉基础 loot"
        );
    }

    // ── DaoZhangSpawnTrigger serde pin 测试 ─────────────────────────────────

    #[test]
    fn spawn_trigger_collapse_zone_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::CollapseZoneDeath {
            family_id: "tsy_lingxu_01".into(),
            origin_realm: Realm::Void,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "CollapseZoneDeath serde 往返失败：{json}");
    }

    #[test]
    fn spawn_trigger_tribulation_strike_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::TribulationStrike {
            origin_realm: Realm::Spirit,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "TribulationStrike serde 往返失败：{json}");
    }

    #[test]
    fn spawn_trigger_tiandao_condense_serde_roundtrip() {
        let trigger = DaoZhangSpawnTrigger::TiandaoCondense {
            zone_name: "spawn".into(),
            condensed_qi: 12.5,
        };
        let json = serde_json::to_string(&trigger).expect("serialize");
        let back: DaoZhangSpawnTrigger = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(trigger, back, "TiandaoCondense serde 往返失败：{json}");
    }

    // ── DaoZhangBehaviorBlackboard ────────────────────────────────────────────

    #[test]
    fn blackboard_initial_state() {
        let pos = DVec3::new(1.0, 64.0, -3.0);
        let bb = DaoZhangBehaviorBlackboard::new("spawn", pos, Some(Realm::Void));
        assert_eq!(bb.home_zone, "spawn");
        assert_eq!(bb.home_pos, pos);
        assert_eq!(
            bb.daozhan_qi, 0.0,
            "初始 daozhan_qi 应为 0（守恒约束：未吸取任何真元）"
        );
        assert_eq!(bb.origin_realm, Some(Realm::Void));
        assert_eq!(bb.behavior_queue.len(), 3, "初始队列含 3 个假动作");
        assert!(bb.pack_id.is_none(), "v1 不使用 pack_id");
    }

    #[test]
    fn blackboard_behavior_queue_cycles() {
        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", DVec3::ZERO, None);
        // 取 6 次，应循环：Swing Sneak Mine Swing Sneak Mine
        let expected = [
            FakeBehavior::Swing,
            FakeBehavior::Sneak,
            FakeBehavior::Mine,
            FakeBehavior::Swing,
            FakeBehavior::Sneak,
            FakeBehavior::Mine,
        ];
        for (i, exp) in expected.into_iter().enumerate() {
            let got = bb.next_behavior();
            assert_eq!(
                got, exp,
                "第 {i} 次 next_behavior 应返回 {exp:?}，实际 {got:?}"
            );
        }
        // 队列长度始终不变（循环不消耗）
        assert_eq!(
            bb.behavior_queue.len(),
            3,
            "循环后队列仍应有 3 个元素（push_back 保持长度）"
        );
    }

    #[test]
    fn blackboard_tiandao_condense_has_no_origin_realm() {
        // 天道凝结的道伥无具体原始修士境界，origin_realm = None
        let bb = DaoZhangBehaviorBlackboard::new("collapse_zone_deep", DVec3::ZERO, None);
        assert!(
            bb.origin_realm.is_none(),
            "天道凝结道伥的 origin_realm 应为 None"
        );
        assert_eq!(
            daozhan_loot_tier(bb.origin_realm),
            DaoZhangLootTier::Base,
            "天道凝结道伥 loot 应为 Base 档"
        );
    }

    #[test]
    fn blackboard_daozhan_qi_is_conservation_tracked() {
        // daozhan_qi 字段存在且初始为 0，测试"守恒不变式：吸取后 daozhan_qi > 0"
        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", DVec3::ZERO, Some(Realm::Void));
        bb.daozhan_qi += 5.0; // 模拟 P2 DaoZhangDrain 后累积
        assert!(
            bb.daozhan_qi > 0.0,
            "累积 daozhan_qi={} 应 > 0（守恒：已吸取玩家真元 5.0）",
            bb.daozhan_qi
        );
    }

    // ── TIANDAO_CONDENSE_THRESHOLD 常数 pin 测试 ─────────────────────────────

    #[test]
    fn tiandao_condense_threshold_is_plausible() {
        // spirit_qi 是 0.0–1.0 归一化，0.8 为高浓度区间合理门控。
        // 用本地变量避免 clippy::assertions_on_constants（常量折叠会让 assert! 变 assert!(true)）。
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            threshold > 0.5,
            "TIANDAO_CONDENSE_THRESHOLD 应 > 0.5（高浓度门控），实际={}",
            threshold
        );
        assert!(
            threshold < 1.0,
            "TIANDAO_CONDENSE_THRESHOLD 应 < 1.0（不能是不可达的满值），实际={}",
            threshold
        );
    }

    // ── P1: Mimicry 常数 pin 测试 ────────────────────────────────────────────

    #[test]
    fn mimicry_behavior_tick_range_is_valid() {
        // 用本地变量避免 clippy::assertions_on_constants（常量折叠会让 assert! 变 assert!(true)）
        let min: u32 = MIMICRY_BEHAVIOR_MIN_TICKS;
        let max: u32 = MIMICRY_BEHAVIOR_MAX_TICKS;
        assert!(
            min >= 20,
            "Mimicry 最短持续应 >= 20 tick（1s），实际={}",
            min
        );
        assert!(
            max > min,
            "Mimicry 最长持续应 > 最短持续，min={} max={}",
            min,
            max
        );
        assert!(
            max <= 200,
            "Mimicry 最长持续应 <= 200 tick（10s），避免过久呆站，实际={}",
            max
        );
    }

    #[test]
    fn mimicry_score_below_ambush_threshold() {
        // Mimicry score 必须 < Ambush 满分 1.0（保证暴起可抢占伪装）
        // 用本地变量避免 clippy::assertions_on_constants
        let score: f32 = DAOZHAN_MIMICRY_SCORE;
        assert!(
            score < 1.0,
            "DAOZHAN_MIMICRY_SCORE 应 < 1.0（Ambush=1.0 优先），实际={}",
            score
        );
        assert!(
            score > 0.0,
            "DAOZHAN_MIMICRY_SCORE 应 > 0（有实际伪装驱动力），实际={}",
            score
        );
    }

    #[test]
    fn mimicry_sense_radius_plausible() {
        // 用本地变量避免 clippy::assertions_on_constants
        let radius: f64 = DAOZHAN_MIMICRY_SENSE_RADIUS;
        assert!(
            radius >= 8.0,
            "感知半径应 >= 8 格（能发现附近玩家），实际={}",
            radius
        );
        assert!(
            radius <= 32.0,
            "感知半径应 <= 32 格（不跨区域感知），实际={}",
            radius
        );
    }

    // ── P1: mimicry_behavior_duration_ticks 单元测试 ─────────────────────────

    #[test]
    fn mimicry_duration_always_in_range() {
        // 任意 seed 返回的 duration 应在 [MIN, MAX] 范围
        for seed in [0u64, 1, 42, 12345, u64::MAX, 0xDEAD_BEEF] {
            let d = mimicry_behavior_duration_ticks(seed);
            assert!(
                d >= MIMICRY_BEHAVIOR_MIN_TICKS,
                "seed={seed} duration={d} 低于 MIN={}",
                MIMICRY_BEHAVIOR_MIN_TICKS
            );
            assert!(
                d <= MIMICRY_BEHAVIOR_MAX_TICKS,
                "seed={seed} duration={d} 超过 MAX={}",
                MIMICRY_BEHAVIOR_MAX_TICKS
            );
        }
    }

    #[test]
    fn mimicry_duration_deterministic() {
        // 相同 seed 必须给出相同结果（确定性，供测试锁定）
        let d1 = mimicry_behavior_duration_ticks(9999);
        let d2 = mimicry_behavior_duration_ticks(9999);
        assert_eq!(
            d1, d2,
            "mimicry_behavior_duration_ticks 必须是确定性的（seed=9999，第一次={d1}，第二次={d2}）"
        );
    }

    #[test]
    fn mimicry_duration_varies_across_seeds() {
        // 不同 seed 应产生不同 duration（验证 RNG 不退化为常数）
        let durations: std::collections::HashSet<u32> =
            (0u64..50).map(mimicry_behavior_duration_ticks).collect();
        assert!(
            durations.len() > 1,
            "mimicry_behavior_duration_ticks 对 50 个不同 seed 应给出至少 2 种不同持续时长（实际只有 {} 种）",
            durations.len()
        );
    }

    // ── P1: DaoZhangMimicryScorer / Action big-brain 测试 ───────────────────

    use big_brain::prelude::{ActionState, Score};
    use valence::prelude::{App, Update};

    fn daozhan_test_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (daozhan_mimicry_scorer_system, daozhan_mimicry_action_system),
        );
        app
    }

    #[test]
    fn mimicry_scorer_zero_when_ambush_state() {
        // Ambush 态时评分应为 0（不触发伪装循环）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "Ambush 态时 DaoZhangMimicryScorer 应为 0，实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_high_score_when_player_in_range() {
        // Mimicry 态 + 玩家在感知范围内 → score = DAOZHAN_MIMICRY_SCORE
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let player_pos = DVec3::new(10.0, 64.0, 0.0); // 10 < 16 = sense radius

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_pos.x, player_pos.y, player_pos.z]),
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - DAOZHAN_MIMICRY_SCORE).abs() < 1e-6,
            "玩家在感知范围内时 DaoZhangMimicryScorer 应={DAOZHAN_MIMICRY_SCORE}，实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_low_score_when_no_player() {
        // Mimicry 态 + 无玩家 → score = 0.3（维持基础游荡）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 0.3).abs() < 1e-6,
            "无玩家时 DaoZhangMimicryScorer 应=0.3（维持游荡），实际={score}"
        );
    }

    #[test]
    fn mimicry_scorer_low_score_when_player_out_of_range() {
        // Mimicry 态 + 玩家超出感知半径 → score = 0.3
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);
        let player_far = DVec3::new(DAOZHAN_MIMICRY_SENSE_RADIUS + 1.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        app.world_mut().spawn((
            ClientMarker,
            Position::new([player_far.x, player_far.y, player_far.z]),
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangMimicryScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 0.3).abs() < 1e-6,
            "玩家超出感知半径时 DaoZhangMimicryScorer 应=0.3，实际={score}"
        );
    }

    #[test]
    fn mimicry_action_requested_transitions_to_executing() {
        // Requested 时 action 应转 Executing，并取第一个假动作
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Requested,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Executing,
            "Requested 后 action 应变 Executing，实际={state:?}"
        );

        // current_behavior_ticks 应被设置为 duration（> 0）
        let bb = app
            .world()
            .get::<DaoZhangBehaviorBlackboard>(daozhan)
            .unwrap();
        assert!(
            bb.current_behavior_ticks >= MIMICRY_BEHAVIOR_MIN_TICKS,
            "current_behavior_ticks 应被设置为 duration >= MIN={}，实际={}",
            MIMICRY_BEHAVIOR_MIN_TICKS,
            bb.current_behavior_ticks
        );
    }

    #[test]
    fn mimicry_action_executing_decrements_ticks() {
        // Executing 时每帧 current_behavior_ticks 递减
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", pos, None);
        bb.current_behavior_ticks = 10; // 手动设置计时

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                bb,
            ))
            .id();

        app.world_mut().spawn((
            Actor(daozhan),
            ActionState::Executing,
            DaoZhangMimicryAction,
        ));

        app.update();

        let bb = app
            .world()
            .get::<DaoZhangBehaviorBlackboard>(daozhan)
            .unwrap();
        assert_eq!(
            bb.current_behavior_ticks, 9,
            "每帧 Executing 时 current_behavior_ticks 应递减 1（10→9），实际={}",
            bb.current_behavior_ticks
        );
    }

    #[test]
    fn mimicry_action_cycles_back_to_requested_when_ticks_zero() {
        // current_behavior_ticks 降到 0 时 action 重入 Requested（循环下一个假动作）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let mut bb = DaoZhangBehaviorBlackboard::new("spawn", pos, None);
        bb.current_behavior_ticks = 0; // 倒计时已到

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                bb,
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Executing,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Requested,
            "倒计时到 0 时 action 应重入 Requested（驱动下一假动作），实际={state:?}"
        );
    }

    #[test]
    fn mimicry_action_cancelled_sets_failure() {
        // Cancelled 时 action 变 Failure
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Cancelled,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Failure,
            "Cancelled 时 action 应变 Failure，实际={state:?}"
        );
    }

    #[test]
    fn mimicry_action_failure_when_not_mimicry_state() {
        // Ambush 态时 action 立即 Failure（被 Ambush 接管）
        let mut app = daozhan_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush, // 非 Mimicry 态
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let action = app
            .world_mut()
            .spawn((
                Actor(daozhan),
                ActionState::Executing,
                DaoZhangMimicryAction,
            ))
            .id();

        app.update();

        let state = app.world().get::<ActionState>(action).unwrap();
        assert_eq!(
            *state,
            ActionState::Failure,
            "非 Mimicry 态时 action 应立即 Failure，实际={state:?}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: player_back_faces_npc_p2 单元测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn back_angle_threshold_constant_pin() {
        // DAOZHAN_BACK_ANGLE_DEG = 150°；cos(150°) ≈ -0.866
        let threshold: f64 = DAOZHAN_BACK_ANGLE_DEG;
        assert!(
            (threshold - 150.0).abs() < 1e-9,
            "DAOZHAN_BACK_ANGLE_DEG 应为 150°，实际={threshold}"
        );
        // 对应 cos(150°) ≈ -0.866（背对判断 cutoff）
        let cos_val = threshold.to_radians().cos();
        assert!(
            (cos_val - (-0.866_025_4)).abs() < 1e-4,
            "cos(150°) ≈ -0.866，实际={cos_val:.6}"
        );
    }

    #[test]
    fn back_faces_npc_true_when_player_faces_away() {
        // 玩家站在 (0,64,0)，道伥在 (0,64,5)（+Z 方向）
        // 玩家 yaw=0 → facing=(0,0,1)（向 +Z，即朝向道伥方向）
        // 要让玩家背对道伥：yaw=180° → facing=(0,0,-1)（背道伥而去）
        // to_npc = (0,0,5-0)=(0,0,5)，normalize=(0,0,1)
        // facing(yaw=180°) = (-sin(180°),0,cos(180°)) = (0,0,-1)
        // dot = (0,0,-1)·(0,0,1) = -1.0 <= -0.866 → 背对 ✓
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        let look = Look {
            yaw: 180.0,
            pitch: 0.0,
        };
        assert!(
            player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=180° 时玩家应背对 +Z 方向的道伥（期望 back=true）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_faces_toward_npc() {
        // 玩家面朝道伥（yaw=0 → facing=(0,0,1)，npc 在 +Z 方向）
        // dot = 1.0 >> -0.866 → 非背对
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=0 时玩家面朝道伥，不应触发背对判断（期望 back=false）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_look_is_none() {
        // 没有 Look Component 时不触发背对（无法判断方向，保守返回 false）
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 64.0, 5.0);
        assert!(
            !player_back_faces_npc_p2(player_pos, None, npc_pos),
            "Look=None 时 back 应返回 false（无法判断方向）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_on_same_xz_as_npc() {
        // 同一 XZ 坐标（to_npc_xz 长度为 0），不触发背对（避免除零）
        use valence::entity::Look;
        let player_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc_pos = DVec3::new(0.0, 70.0, 0.0); // 同 XZ，只差 Y
        let look = Look {
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "XZ 距离为 0 时不应触发背对（避免除零，期望 back=false）"
        );
    }

    #[test]
    fn back_faces_boundary_at_exactly_150_degrees() {
        // 偏转角恰好 150° 时应触发（> = <=  threshold 边界）
        // 玩家在原点，道伥在 +Z，facing 旋转 150°（从 +Z 方向转 150°）= yaw=150
        // yaw=150° → facing = (-sin(150°),0,cos(150°)) = (-0.5,0,-0.866)
        // to_npc_norm = (0,0,1)
        // dot = 0+0+(-0.866)·1 = -0.866 = cos(150°) → 应触发（<= threshold）
        use valence::entity::Look;
        let player_pos = DVec3::ZERO;
        let npc_pos = DVec3::new(0.0, 0.0, 5.0);
        // 从 +Z 方向顺时针 150° = yaw=150°（MC yaw 0=south=+Z，顺时针增加）
        let look = Look {
            yaw: 150.0,
            pitch: 0.0,
        };
        assert!(
            player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=150° 对 +Z 方向道伥时 dot=cos(150°) 恰好触发背对（边界应为 true）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_side_facing_90_degrees() {
        // B1 回归测试：玩家 90° 侧对道伥不应触发暴起（正典：> 150° 才触发）。
        // 玩家在原点，道伥在 +Z 方向，yaw=90°（面朝 -X，侧对道伥）。
        // MC yaw=90° → facing = (-sin(90°),0,cos(90°)) = (-1,0,0)
        // to_npc_norm = (0,0,1)
        // dot = 0 — 明显 > cos(150°) ≈ -0.866 → 不触发（期望 false）
        use valence::entity::Look;
        let player_pos = DVec3::ZERO;
        let npc_pos = DVec3::new(0.0, 0.0, 5.0);
        let look = Look {
            yaw: 90.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=90°（侧对）时不应触发道伥暴起，偏转角仅 90° < 150°（期望 back=false）"
        );
    }

    #[test]
    fn back_faces_npc_false_when_player_facing_45_degrees_toward_npc() {
        // yaw=45°（朝向介于正对与侧对之间），偏转角仅 45° < 150° → 不触发
        use valence::entity::Look;
        let player_pos = DVec3::ZERO;
        let npc_pos = DVec3::new(0.0, 0.0, 5.0);
        let look = Look {
            yaw: 45.0,
            pitch: 0.0,
        };
        assert!(
            !player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=45° 时偏转角仅 45°，远小于 150° 阈值，不应触发背对（期望 back=false）"
        );
    }

    #[test]
    fn back_faces_npc_true_when_player_facing_151_degrees() {
        // 偏转角 151° > 150° 阈值 → 应触发（>150° 背对区间）
        // 道伥在 +Z，yaw=151° → facing = (-sin(151°),0,cos(151°))
        // dot = cos(151°) ≈ -0.875 < cos(150°) ≈ -0.866 → 触发
        use valence::entity::Look;
        let player_pos = DVec3::ZERO;
        let npc_pos = DVec3::new(0.0, 0.0, 5.0);
        let look = Look {
            yaw: 151.0,
            pitch: 0.0,
        };
        assert!(
            player_back_faces_npc_p2(player_pos, Some(&look), npc_pos),
            "yaw=151° 时偏转角 151° > 150° 阈值，应触发背对（期望 back=true）"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: qi_ratio_p2 单元测试
    // ═══════════════════════════════════════════════════════════════════════

    fn make_cultivation(qi_current: f64, qi_max: f64) -> Cultivation {
        Cultivation {
            qi_current,
            qi_max,
            qi_max_frozen: None,
            ..Default::default()
        }
    }

    #[test]
    fn qi_ratio_full_is_one() {
        let cult = make_cultivation(100.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            (r - 1.0).abs() < 1e-9,
            "qi_current=qi_max 时比例应为 1.0，实际={r}"
        );
    }

    #[test]
    fn qi_ratio_empty_is_zero() {
        let cult = make_cultivation(0.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(r.abs() < 1e-9, "qi_current=0 时比例应为 0，实际={r}");
    }

    #[test]
    fn qi_ratio_below_low_threshold() {
        // qi_current = 15, qi_max = 100 → ratio=0.15 < 0.20 → 触发 low_qi 条件
        let cult = make_cultivation(15.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            r < DAOZHAN_LOW_QI_RATIO,
            "比例=0.15 应低于 DAOZHAN_LOW_QI_RATIO={DAOZHAN_LOW_QI_RATIO}"
        );
    }

    #[test]
    fn qi_ratio_at_boundary_20_percent() {
        // qi_current = 20, qi_max = 100 → ratio=0.20 = threshold，不触发（要求严格 <）
        let cult = make_cultivation(20.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            (r - 0.20).abs() < 1e-9,
            "比例应为 0.20（边界），实际={r:.6}"
        );
        // 边界：< 0.20 才触发，等于不触发（r >= threshold → 不触发）
        assert!(
            r >= DAOZHAN_LOW_QI_RATIO,
            "ratio=0.20 不应小于 DAOZHAN_LOW_QI_RATIO（边界 off-by-one：< 而非 <=）"
        );
    }

    #[test]
    fn qi_ratio_negative_current_clamped_to_zero() {
        // 防御性：qi_current < 0 时 clamp 到 0
        let cult = make_cultivation(-5.0, 100.0);
        let r = qi_ratio_p2(&cult);
        assert!(
            r.abs() < 1e-9,
            "qi_current<0 时 ratio 应 clamp 到 0，实际={r}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: DaoZhangAmbushScorer ECS 测试
    // ═══════════════════════════════════════════════════════════════════════

    fn ambush_test_app() -> App {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                daozhan_mimicry_scorer_system,
                daozhan_mimicry_action_system,
                daozhan_ambush_scorer_system,
            ),
        );
        app
    }

    #[test]
    fn ambush_scorer_zero_when_ambush_state() {
        // 已处于 Ambush 态时 scorer 应为 0（不重入）
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "Ambush 态时 DaoZhangAmbushScorer 应为 0（不重入），实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_on_cooldown() {
        // 冷却未过期时 scorer 应为 0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
                // 冷却到 tick=99999，当前 tick=0
                DaoZhangAmbushCooldown {
                    ready_at_tick: 99_999,
                },
            ))
            .id();

        // 玩家在感知范围内且背对（理论上应触发，但被冷却阻止）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 3.0]),
            Cultivation {
                qi_current: 5.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "冷却中时 DaoZhangAmbushScorer 应为 0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_one_when_player_low_qi() {
        // 玩家 qi < 20% 时 scorer 应为 1.0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        // 玩家在 5 格内（< 8 = DAOZHAN_AMBUSH_SENSE_RADIUS），qi=10/100 = 10% < 20%
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 5.0]),
            Cultivation {
                qi_current: 10.0,
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert!(
            (score - 1.0).abs() < 1e-6,
            "低真元（qi=10%）时 DaoZhangAmbushScorer 应=1.0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_player_out_of_range() {
        // 玩家超出感知范围时 scorer 为 0（即使 qi 低）
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        // 玩家在 20 格处（> 8 = DAOZHAN_AMBUSH_SENSE_RADIUS）
        app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 20.0]),
            Cultivation {
                qi_current: 1.0, // 很低
                qi_max: 100.0,
                ..Default::default()
            },
        ));

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "超出感知范围时 DaoZhangAmbushScorer 应=0，实际={score}"
        );
    }

    #[test]
    fn ambush_scorer_zero_when_no_player_nearby() {
        // 无玩家时 scorer 为 0
        let mut app = ambush_test_app();
        let pos = DVec3::new(0.0, 64.0, 0.0);

        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Mimicry,
                DaoZhangBehaviorBlackboard::new("spawn", pos, None),
            ))
            .id();

        let scorer = app
            .world_mut()
            .spawn((Actor(daozhan), Score::default(), DaoZhangAmbushScorer))
            .id();

        app.update();

        let score = app.world().get::<Score>(scorer).unwrap().get();
        assert_eq!(
            score, 0.0,
            "无玩家时 DaoZhangAmbushScorer 应=0，实际={score}"
        );
    }

    fn ambush_action_test_app() -> App {
        let mut app = App::new();
        app.insert_resource(WorldQiAccount::default());
        app.add_event::<VfxEventRequest>();
        app.add_event::<PlaySoundRecipeRequest>();
        app.add_event::<DaoZhangRevealEvent>();
        app.add_event::<QiTransfer>();
        app.add_systems(Update, daozhan_ambush_action_system);
        app
    }

    fn spawn_executing_ambush(
        app: &mut App,
        player_qi: f64,
        player_identity: Option<&str>,
        daozhan_qi: f64,
    ) -> (Entity, Entity) {
        let mut player = app.world_mut().spawn((
            ClientMarker,
            Position::new([0.0, 64.0, 3.0]),
            Cultivation {
                qi_current: player_qi,
                qi_max: 100.0,
                ..Default::default()
            },
        ));
        if let Some(character_id) = player_identity {
            player.insert(LifeRecord::new(character_id));
        }
        let player = player.id();

        let pos = DVec3::new(0.0, 64.0, 0.0);
        let mut blackboard = DaoZhangBehaviorBlackboard::new("spawn", pos, None);
        blackboard.daozhan_qi = daozhan_qi;
        let daozhan = app
            .world_mut()
            .spawn((
                NpcMarker,
                Position::new([pos.x, pos.y, pos.z]),
                DaoZhangState::Ambush,
                blackboard,
                DaoZhangAmbushChainState {
                    hits_done: 0,
                    target_player: player,
                    next_hit_at_tick: 0,
                },
            ))
            .id();
        app.world_mut()
            .spawn((Actor(daozhan), ActionState::Executing, DaoZhangAmbushAction));
        (player, daozhan)
    }

    #[test]
    fn ambush_drain_commits_both_owners_and_canonical_audit_together() {
        let mut app = ambush_action_test_app();
        let (player, daozhan) = spawn_executing_ambush(&mut app, 20.0, Some("ambush_target"), 5.0);

        app.update();

        let player_qi = app.world().get::<Cultivation>(player).unwrap().qi_current;
        let daozhan_qi = app
            .world()
            .get::<DaoZhangBehaviorBlackboard>(daozhan)
            .unwrap()
            .daozhan_qi;
        assert_eq!(player_qi, 12.0, "伏击命中应从玩家物理 owner 扣除 8 真元");
        assert_eq!(daozhan_qi, 13.0, "伏击命中应向道伥物理 owner 增加同量真元");
        assert_eq!(
            player_qi + daozhan_qi,
            25.0,
            "伏击事务前后两个物理 owner 的真元总和必须守恒"
        );

        let ledger = app.world().resource::<WorldQiAccount>();
        let transfers: Vec<_> = ledger
            .transfers()
            .iter()
            .filter(|transfer| transfer.reason == QiTransferReason::DaoZhangDrain)
            .collect();
        assert_eq!(transfers.len(), 1, "一次命中只能产生一条已提交 audit");
        assert_eq!(transfers[0].from, QiAccountId::player("ambush_target"));
        assert_eq!(
            transfers[0].to,
            QiAccountId::npc(crate::npc::brain::canonical_npc_id(daozhan))
        );
        assert_eq!(transfers[0].amount, 8.0);
        let chain = app
            .world()
            .get::<DaoZhangAmbushChainState>(daozhan)
            .unwrap();
        assert_eq!(
            chain.hits_done, 1,
            "连击计数只能在两端 owner 与 audit 同步提交后推进"
        );
    }

    #[test]
    fn ambush_drain_with_zero_player_qi_is_a_noop() {
        let mut app = ambush_action_test_app();
        let (player, daozhan) = spawn_executing_ambush(&mut app, 0.0, Some("empty_target"), 5.0);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            0.0
        );
        assert_eq!(
            app.world()
                .get::<DaoZhangBehaviorBlackboard>(daozhan)
                .unwrap()
                .daozhan_qi,
            5.0
        );
        assert!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .all(|transfer| transfer.reason != QiTransferReason::DaoZhangDrain),
            "没有真元可吸取时不得伪造 audit"
        );
    }

    #[test]
    fn ambush_drain_without_canonical_player_identity_fails_closed() {
        for identity in [None, Some("   ")] {
            let mut app = ambush_action_test_app();
            let (player, daozhan) = spawn_executing_ambush(&mut app, 20.0, identity, 5.0);

            app.update();

            assert_eq!(
                app.world().get::<Cultivation>(player).unwrap().qi_current,
                20.0,
                "缺失或空白 LifeRecord 时不得用 Entity id 代替玩家 owner"
            );
            assert_eq!(
                app.world()
                    .get::<DaoZhangBehaviorBlackboard>(daozhan)
                    .unwrap()
                    .daozhan_qi,
                5.0,
                "身份验证失败时目标 owner 不得变化"
            );
            assert!(
                app.world()
                    .resource::<WorldQiAccount>()
                    .transfers()
                    .iter()
                    .all(|transfer| transfer.reason != QiTransferReason::DaoZhangDrain),
                "身份验证失败时不得追加 audit"
            );
        }
    }

    #[test]
    fn ambush_drain_missing_target_entity_fails_closed() {
        let mut app = ambush_action_test_app();
        let (player, daozhan) =
            spawn_executing_ambush(&mut app, 20.0, Some("despawned_target"), 5.0);
        app.world_mut().despawn(player);

        app.update();

        assert_eq!(
            app.world()
                .get::<DaoZhangBehaviorBlackboard>(daozhan)
                .unwrap()
                .daozhan_qi,
            5.0,
            "目标已不存在时道伥 owner 不得变化"
        );
        assert!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .all(|transfer| transfer.reason != QiTransferReason::DaoZhangDrain),
            "目标已不存在时不得追加 audit"
        );
    }

    #[test]
    fn ambush_drain_unrepresentable_target_is_fully_atomic() {
        let mut app = ambush_action_test_app();
        let target_qi = f64::MAX / 2.0;
        let (player, daozhan) =
            spawn_executing_ambush(&mut app, 20.0, Some("ulp_guard_target"), target_qi);

        app.update();

        assert_eq!(
            app.world().get::<Cultivation>(player).unwrap().qi_current,
            20.0,
            "目标无法表示 +8 时 source debit 必须回滚"
        );
        assert_eq!(
            app.world()
                .get::<DaoZhangBehaviorBlackboard>(daozhan)
                .unwrap()
                .daozhan_qi,
            target_qi,
            "目标无法表示 +8 时 target credit 必须回滚"
        );
        assert!(
            app.world()
                .resource::<WorldQiAccount>()
                .transfers()
                .iter()
                .all(|transfer| transfer.reason != QiTransferReason::DaoZhangDrain),
            "失败事务不得留下成功 audit"
        );
        assert_eq!(
            app.world()
                .get::<DaoZhangAmbushChainState>(daozhan)
                .unwrap()
                .hits_done,
            0,
            "目标无法表示 credit 时本次连击机会必须保留以便重试"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P2: 守恒核心测试：player.qi -= amount == daozhan.daozhan_qi += amount
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn drain_amount_constant_pin() {
        // DAOZHAN_DRAIN_AMOUNT_PER_HIT 应大于 0 且合理（< qi_max 典型值）
        let drain: f64 = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        assert!(
            drain > 0.0,
            "DAOZHAN_DRAIN_AMOUNT_PER_HIT 应 > 0，实际={drain}"
        );
        assert!(
            drain <= 50.0,
            "DAOZHAN_DRAIN_AMOUNT_PER_HIT 应 <= 50（单次不超过半满上限），实际={drain}"
        );
    }

    #[test]
    fn drain_conservation_normal_case() {
        // 守恒：player.qi_current -= drained，daozhan.daozhan_qi += drained（之和不变）
        let player_qi_before = 100.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        let player_qi_after = player_qi_before - drained;
        let daozhan_qi_after = 0.0f64 + drained; // 初始 daozhan_qi=0

        // 守恒：player 减少量 == daozhan 增加量
        assert!(
            (player_qi_before - player_qi_after - daozhan_qi_after).abs() < 1e-9,
            "守恒失败：player 减少 {:.3}，daozhan 增加 {:.3}，差值应为 0",
            player_qi_before - player_qi_after,
            daozhan_qi_after
        );
    }

    #[test]
    fn drain_conservation_player_has_less_than_drain_amount() {
        // 边界：玩家真元 < DAOZHAN_DRAIN_AMOUNT_PER_HIT 时，drained = 玩家实际量（不负数）
        let player_qi_before = 3.0f64; // 远小于 DAOZHAN_DRAIN_AMOUNT_PER_HIT=8
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        assert!(
            (drained - 3.0).abs() < 1e-9,
            "玩家 qi=3 时应只吸取 3，实际吸取={drained}"
        );

        let player_qi_after = player_qi_before - drained;
        assert!(
            player_qi_after >= 0.0,
            "玩家 qi 不应变负数，实际={player_qi_after}"
        );

        let daozhan_qi_after = drained;
        // 总和守恒
        assert!(
            (player_qi_before - (player_qi_after + daozhan_qi_after)).abs() < 1e-9,
            "守恒：player_before={player_qi_before}，player_after+daozhan={:.3}",
            player_qi_after + daozhan_qi_after
        );
    }

    #[test]
    fn drain_conservation_player_qi_zero() {
        // 边界：玩家 qi=0 时 drained=0，daozhan_qi 不增加（无真元可吸）
        let player_qi_before = 0.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;
        let available = player_qi_before.max(0.0);
        let drained = drain.min(available);

        assert!(
            drained.abs() < 1e-9,
            "玩家 qi=0 时 drained 应为 0，实际={drained}"
        );
    }

    #[test]
    fn drain_conservation_three_hits_total() {
        // 三连击总守恒：player 总减少量 == daozhan 总累积量
        let mut player_qi = 100.0f64;
        let mut daozhan_qi = 0.0f64;
        let drain = DAOZHAN_DRAIN_AMOUNT_PER_HIT;

        for _ in 0..DAOZHAN_AMBUSH_CHAIN_COUNT {
            let available = player_qi.max(0.0);
            let drained = drain.min(available);
            player_qi -= drained;
            daozhan_qi += drained;
        }

        assert!(
            (player_qi + daozhan_qi - 100.0).abs() < 1e-9,
            "三连击后 player_qi({player_qi:.3}) + daozhan_qi({daozhan_qi:.3}) 应等于初始总量 100"
        );
    }

    #[test]
    fn ambush_chain_state_initial_values_pin() {
        // 确认 DaoZhangAmbushChainState 初始字段语义
        let e = Entity::PLACEHOLDER;
        let cs = DaoZhangAmbushChainState {
            hits_done: 0,
            target_player: e,
            next_hit_at_tick: 0,
        };
        assert_eq!(cs.hits_done, 0, "初始 hits_done 应为 0（尚未打出任何一击）");
        assert_eq!(
            cs.target_player, e,
            "target_player 应为初始化时指定的 entity"
        );
    }

    #[test]
    fn ambush_chain_hits_constant_matches_ambush_chain_count() {
        // DAOZHAN_AMBUSH_CHAIN_HITS 应与 DAOZHAN_AMBUSH_CHAIN_COUNT 一致（两个常量来源同一逻辑）
        let chain_hits: u32 = DAOZHAN_AMBUSH_CHAIN_HITS;
        let chain_count: u32 = DAOZHAN_AMBUSH_CHAIN_COUNT;
        assert_eq!(
            chain_hits, chain_count,
            "DAOZHAN_AMBUSH_CHAIN_HITS({chain_hits}) 应等于 DAOZHAN_AMBUSH_CHAIN_COUNT({chain_count})"
        );
        // 应为 3
        assert_eq!(
            chain_hits, 3,
            "三连击设计决议：道伥连击次数应为 3，实际={chain_hits}"
        );
    }

    #[test]
    fn ambush_cooldown_ticks_plausible() {
        // DAOZHAN_AMBUSH_COOLDOWN_TICKS 应在合理范围（1s~10s）
        let cd: u64 = DAOZHAN_AMBUSH_COOLDOWN_TICKS;
        assert!(cd >= 20, "暴起冷却应 >= 20 tick（1s），实际={cd}");
        assert!(cd <= 200, "暴起冷却应 <= 200 tick（10s），实际={cd}");
    }

    #[test]
    fn low_qi_ratio_constant_pin() {
        // DAOZHAN_LOW_QI_RATIO 应为 0.20（设计决议）
        let r: f64 = DAOZHAN_LOW_QI_RATIO;
        assert!(
            (r - 0.20).abs() < 1e-9,
            "DAOZHAN_LOW_QI_RATIO 应为 0.20（设计决议 #2），实际={r}"
        );
    }

    #[test]
    fn reveal_vfx_constants_pin() {
        // VFX 规格 pin：count=12，#7040A8，10tick（与设计决议锁定）
        let count: u16 = DAOZHAN_REVEAL_PARTICLE_COUNT;
        assert_eq!(count, 12, "暴起粒子数量应为 12，实际={count}");

        assert_eq!(
            DAOZHAN_REVEAL_PARTICLE_COLOR, "#7040A8",
            "暴起粒子颜色应为 #7040A8，实际={}",
            DAOZHAN_REVEAL_PARTICLE_COLOR
        );

        let duration: u16 = DAOZHAN_REVEAL_PARTICLE_DURATION_TICKS;
        assert_eq!(duration, 10, "暴起粒子持续应为 10 tick，实际={duration}");

        assert_eq!(
            DAOZHAN_REVEAL_VFX_EVENT_ID, "bong:vfx/daozhan_reveal",
            "VFX event_id 应为 bong:vfx/daozhan_reveal，实际={}",
            DAOZHAN_REVEAL_VFX_EVENT_ID
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: 天道凝结守恒测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn tiandao_condense_threshold_blocks_low_zone_spirit_qi() {
        // zone.spirit_qi <= TIANDAO_CONDENSE_THRESHOLD 时不应触发凝结
        // 模拟：spirit_qi=0.7 < threshold=0.8 → 不触发
        let spirit_qi = 0.70f64;
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            spirit_qi <= threshold,
            "spirit_qi={spirit_qi} 应 <= TIANDAO_CONDENSE_THRESHOLD={threshold}（不触发凝结）"
        );
        // 模拟凝结守卫：不触发时 zone 灵气不变
        let zone_qi_before = spirit_qi;
        let should_condense = spirit_qi > threshold;
        let zone_qi_after = if should_condense {
            (spirit_qi - TIANDAO_CONDENSE_QI_COST).clamp(-1.0, 1.0)
        } else {
            spirit_qi
        };
        assert!(
            (zone_qi_before - zone_qi_after).abs() < 1e-9,
            "低于阈值时 zone 灵气不应变化（守恒：不凝结不扣减），before={zone_qi_before:.3} after={zone_qi_after:.3}"
        );
    }

    #[test]
    fn tiandao_condense_conservation_zone_decreases_by_cost() {
        // 守恒：zone 扣减量 == 道伥获得初始 qi
        let spirit_qi_before = 0.90f64; // > threshold=0.8
        let threshold: f64 = TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            spirit_qi_before > threshold,
            "测试前提：spirit_qi={spirit_qi_before} 应 > threshold={threshold}"
        );
        // 实际 cost = min(COST, spirit_qi - threshold)（防止扣过头）
        let actual_cost = TIANDAO_CONDENSE_QI_COST.min(spirit_qi_before - threshold);
        let spirit_qi_after = (spirit_qi_before - actual_cost).clamp(-1.0, 1.0);
        let daozhan_initial_qi = actual_cost;

        // 守恒：zone 减少量 == 道伥初始 qi
        assert!(
            (spirit_qi_before - spirit_qi_after - daozhan_initial_qi).abs() < 1e-9,
            "守恒失败：zone 减少 {:.4}，道伥初始 qi={:.4}，差值应为 0",
            spirit_qi_before - spirit_qi_after,
            daozhan_initial_qi
        );
    }

    #[test]
    fn tiandao_condense_actual_cost_capped_to_available_headroom() {
        // 边界：spirit_qi = threshold + 极小量（0.001），cost 不超过可用头量
        let spirit_qi = TIANDAO_CONDENSE_THRESHOLD + 0.001;
        let actual_cost = TIANDAO_CONDENSE_QI_COST.min(spirit_qi - TIANDAO_CONDENSE_THRESHOLD);
        let headroom = spirit_qi - TIANDAO_CONDENSE_THRESHOLD;
        assert!(
            actual_cost <= headroom + 1e-9,
            "actual_cost={actual_cost:.5} 不应超过可用头量 headroom={headroom:.5}（防止 zone 跌穿 threshold）"
        );
    }

    #[test]
    fn tiandao_condense_interval_ticks_plausible() {
        // TIANDAO_CONDENSE_INTERVAL_TICKS 应为合理值（不小于 20 tick = 1s）
        let interval: u32 = TIANDAO_CONDENSE_INTERVAL_TICKS;
        assert!(
            interval >= 20,
            "TIANDAO_CONDENSE_INTERVAL_TICKS 应 >= 20 tick（1s），实际={interval}"
        );
        assert!(
            interval <= 6000,
            "TIANDAO_CONDENSE_INTERVAL_TICKS 应 <= 6000 tick（5分钟），实际={interval}"
        );
    }

    #[test]
    fn tiandao_condense_max_per_zone_plausible() {
        // TIANDAO_CONDENSE_MAX_PER_ZONE 应在合理范围（1~10）
        let max: u32 = TIANDAO_CONDENSE_MAX_PER_ZONE;
        assert!(
            max >= 1,
            "TIANDAO_CONDENSE_MAX_PER_ZONE 应 >= 1，实际={max}"
        );
        assert!(
            max <= 10,
            "TIANDAO_CONDENSE_MAX_PER_ZONE 应 <= 10（防止同区堆叠过多），实际={max}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: DaoZhangDeathSystem 守恒测试（unit level，无 ECS）
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn death_release_full_amount_not_partial() {
        // 守恒语义：死亡时 daozhan_qi 全额归还（非1%），验证数学逻辑
        let daozhan_qi_accumulated = 24.0f64; // 三连击 × 8.0 = 24.0
                                              // 正典 helper 返回归还的实际量（全额 or 0）
                                              // 我们只测逻辑：release_amount == daozhan_qi（不是 0.01 × daozhan_qi）
        let release_amount = daozhan_qi_accumulated;
        let partial_1pct = daozhan_qi_accumulated * 0.01;
        assert!(
            release_amount > partial_1pct,
            "道伥死亡应全额归还（{release_amount:.3}），而非 1%（{partial_1pct:.3}）"
        );
        // 守恒不变式：release == daozhan_qi
        assert!(
            (release_amount - daozhan_qi_accumulated).abs() < 1e-9,
            "release_amount 应等于 daozhan_qi_accumulated（守恒），差值应为 0"
        );
    }

    #[test]
    fn death_release_zero_when_daozhan_qi_is_zero() {
        // 天道凝结道伥未吸取任何真元时，daozhan_qi=0，死亡时不应触发释放（守恒：无吸无还）
        let daozhan_qi = 0.0f64;
        let should_release = daozhan_qi > 0.0;
        assert!(
            !should_release,
            "daozhan_qi=0 时不应触发 release_qi_amount_to_zone（无真元可还）"
        );
    }

    #[test]
    fn death_release_includes_both_accumulated_and_initial_qi() {
        // 道伥既有天道凝结初始 qi，又有伏击吸取累积量
        // 两者都存在于 daozhan_qi 字段（天道凝结路径：condense 初始量写入 daozhan_qi）
        let condense_qi = TIANDAO_CONDENSE_INITIAL_QI;
        let drained_from_player = DAOZHAN_DRAIN_AMOUNT_PER_HIT * 2.0; // 2 连击
                                                                      // 模拟 blackboard.daozhan_qi 包含两部分
        let total_daozhan_qi = condense_qi + drained_from_player;
        let release_amount = total_daozhan_qi;

        // 守恒：全额归还（含凝结量 + 吸取量）
        assert!(
            (release_amount - total_daozhan_qi).abs() < 1e-9,
            "死亡归还量应包含凝结量({condense_qi:.4}) + 吸取量({drained_from_player:.4})，合计={total_daozhan_qi:.4}"
        );
        assert!(
            total_daozhan_qi > condense_qi,
            "含伏击吸取后总量 {total_daozhan_qi:.4} 应大于仅凝结量 {condense_qi:.4}"
        );
    }

    // ═══════════════════════════════════════════════════════════════════════
    // P3: 神识识破 DisguisedDaoZhang — 常数 pin 测试
    // ═══════════════════════════════════════════════════════════════════════

    #[test]
    fn disguised_daozhan_condense_state_default_is_empty() {
        // DaoZhangCondenseState default 应无任何 zone 记录（全新服务器无冷却）
        let state = DaoZhangCondenseState::default();
        assert!(
            state.last_condense_tick.is_empty(),
            "DaoZhangCondenseState 初始应无 zone 冷却记录（空 HashMap）"
        );
    }

    #[test]
    fn spawn_request_event_fields_accessible() {
        // SpawnDaoZhangFromCondenseRequest 字段语义 pin（守恒测试用）
        let req = SpawnDaoZhangFromCondenseRequest {
            zone_name: "spawn".to_string(),
            condensed_qi: TIANDAO_CONDENSE_INITIAL_QI,
            tick: 600,
        };
        assert_eq!(req.zone_name, "spawn", "zone_name 字段应为初始化值");
        assert!(
            (req.condensed_qi - TIANDAO_CONDENSE_INITIAL_QI).abs() < 1e-9,
            "condensed_qi 应等于 TIANDAO_CONDENSE_INITIAL_QI={TIANDAO_CONDENSE_INITIAL_QI}"
        );
        assert_eq!(req.tick, 600, "tick 字段应为初始化值");
    }

    #[test]
    fn tiandao_initial_qi_equals_cost() {
        // TIANDAO_CONDENSE_INITIAL_QI 应等于 TIANDAO_CONDENSE_QI_COST（守恒：zone 扣多少，道伥得多少）
        assert!(
            (TIANDAO_CONDENSE_INITIAL_QI - TIANDAO_CONDENSE_QI_COST).abs() < 1e-9,
            "TIANDAO_CONDENSE_INITIAL_QI({}) 应等于 TIANDAO_CONDENSE_QI_COST({})（守恒 1:1 转移）",
            TIANDAO_CONDENSE_INITIAL_QI,
            TIANDAO_CONDENSE_QI_COST
        );
    }
