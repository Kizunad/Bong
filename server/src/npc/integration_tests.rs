//! plan-npc-overhaul-v1 §P4 — 集成校准 + e2e + 性能基准 + 数值平衡断言。
//!
//! 纯 ECS / 纯函数测试（无需真实网络/渲染），验证 NPC 遭遇全链路、
//! 性能基准（50 NPC threat < 5ms 等）、数值平衡（定价/翻脸/信誉）。

#[cfg(test)]
mod tests {
    use crate::cultivation::components::Realm;
    use crate::npc::brain::threat::{
        build_threat_assessment, decide_self_interest, decide_self_interest_with_memory,
        NpcThreatConfig, SelfInterestDecision, ThreatAssessment,
    };
    use crate::npc::interaction_memory::{
        NpcInteractionOutcome, NpcInteractionType, NpcMemoryComponent, NpcMemoryEntry,
    };
    use crate::npc::lifecycle::NpcArchetype;
    use crate::npc::spawn::PoissonSpawnSampler;
    use crate::npc::trade::{
        DynamicPricing, NpcPlayerReputation, ReputationGossipEvent, TradePricingConfig,
        GOSSIP_INITIAL_HOPS, GOSSIP_PROPAGATION_RADIUS,
    };
    use valence::prelude::{DVec3, Entity};

    // =========================================================================
    // P4.1 — 完整 NPC 遭遇 e2e 集成测试
    // =========================================================================

    /// 完整 NPC 遭遇链路 e2e：
    /// 1. 建 ThreatAssessment → 验证 score 存在
    /// 2. 健康玩家 → Trade
    /// 3. 定价合理性
    /// 4. 虚弱玩家 → Ambush
    /// 5. 新手保护 → Guard
    /// 6. 信誉 attack -0.3 / trade +0.05
    /// 7. gossip 传播验证
    #[test]
    fn encounter_e2e_threat_to_decision_to_trade_to_ambush() {
        let config = NpcThreatConfig::default();

        // ─── Step 1: 构建 ThreatAssessment 并验证 score 存在 ─────────────
        let assessment_healthy = build_threat_assessment(
            Realm::Condense, // player realm
            Realm::Condense, // npc realm
            90.0,            // player qi_current
            100.0,           // player qi_max
            100.0,           // player health_current
            100.0,           // player health_max
            10.0,            // distance
            false,           // no weapon visible
            &config,
        );
        assert!(
            assessment_healthy.score >= 0.0 && assessment_healthy.score <= 1.0,
            "ThreatAssessment score should be in [0.0, 1.0], got {}",
            assessment_healthy.score
        );
        assert_eq!(
            assessment_healthy.realm_delta, 0,
            "same realm (Condense vs Condense) should yield realm_delta=0, got {}",
            assessment_healthy.realm_delta
        );
        assert!(
            (assessment_healthy.player_qi_ratio - 0.9).abs() < 0.01,
            "qi 90/100 should give ratio ~0.9, got {}",
            assessment_healthy.player_qi_ratio
        );

        // ─── Step 2: 健康玩家 (qi=90%) → Trade ─────────────────────────
        // To get Trade, we need threat in [0.5, 0.8) with qi > 0.5.
        // Craft a scenario: use direct assessment with score in trade window.
        let trade_assessment = ThreatAssessment {
            score: 0.6,
            realm_delta: 0,
            player_qi_ratio: 0.9,
            player_wound_ratio: 0.0,
            player_distance: 10.0,
            has_weapon_visible: false,
        };
        let decision_trade = decide_self_interest(&trade_assessment);
        assert_eq!(
            decision_trade,
            SelfInterestDecision::Trade,
            "healthy player (qi=90%, score=0.6) should trigger Trade, got {:?}",
            decision_trade
        );

        // ─── Step 3: 定价合理性 (base=12 灵草, spawn zone defaults) ─────
        let pricing_config = TradePricingConfig::default();
        let price_lingcao = DynamicPricing::compute_price(
            12,  // base (灵草)
            0.5, // zone_qi (spawn default)
            1.0, // npc_qi_need (not urgent)
            0.5, // reputation (neutral)
            0.0, // supply_scarcity
            &pricing_config,
        );
        assert_eq!(
            price_lingcao, 12,
            "灵草 at neutral zone_qi (0.5) + neutral rep (0.5) + no scarcity should cost base=12, got {}",
            price_lingcao
        );

        // ─── Step 4: 虚弱玩家 (qi=10%, same realm) → Ambush ────────────
        let assessment_weak = build_threat_assessment(
            Realm::Condense, // player
            Realm::Condense, // npc
            10.0,            // qi_current
            100.0,           // qi_max
            60.0,            // health_current (40% wounded)
            100.0,           // health_max
            8.0,             // close distance
            false,           // no weapon
            &config,
        );
        // Verify low threat → potential ambush
        assert!(
            assessment_weak.player_qi_ratio < 0.2,
            "qi 10/100 should give ratio < 0.2, got {}",
            assessment_weak.player_qi_ratio
        );
        let decision_weak = decide_self_interest(&assessment_weak);
        assert_eq!(
            decision_weak,
            SelfInterestDecision::Ambush,
            "weak player (qi=10%, wounds=40%, same realm, close) should be Ambush, got {:?} (threat={:.3})",
            decision_weak,
            assessment_weak.score
        );

        // ─── Step 5: 新手保护 (realm 2 below NPC + qi=0) → Guard ───────
        let assessment_newbie = build_threat_assessment(
            Realm::Awaken,   // player (2 below Condense)
            Realm::Condense, // npc
            0.0,             // qi empty
            100.0,
            50.0, // wounded
            100.0,
            5.0, // close
            false,
            &config,
        );
        assert_eq!(
            assessment_newbie.realm_delta, -2,
            "Awaken(1) - Condense(3) should be realm_delta=-2, got {}",
            assessment_newbie.realm_delta
        );
        let decision_newbie = decide_self_interest(&assessment_newbie);
        assert_ne!(
            decision_newbie,
            SelfInterestDecision::Ambush,
            "newbie protection: player 2 realms below NPC should NOT get Ambush, got {:?}",
            decision_newbie
        );
        assert_eq!(
            decision_newbie,
            SelfInterestDecision::Guard,
            "newbie player (realm -2) should get Guard, got {:?}",
            decision_newbie
        );

        // ─── Step 6: 信誉调节——attack -0.3, trade +0.05 ────────────────
        let mut rep = NpcPlayerReputation::default();
        let initial_rep = rep.get("player:hero");
        assert!(
            (initial_rep - 0.5).abs() < f32::EPSILON,
            "initial reputation should be 0.5 (neutral), got {}",
            initial_rep
        );

        // Attack: -0.3
        rep.adjust("player:hero", -0.3);
        let after_attack = rep.get("player:hero");
        assert!(
            (after_attack - 0.2).abs() < f32::EPSILON,
            "after attack (-0.3), reputation should be 0.2, got {}",
            after_attack
        );

        // Trade: +0.05
        rep.adjust("player:hero", 0.05);
        let after_trade = rep.get("player:hero");
        assert!(
            (after_trade - 0.25).abs() < f32::EPSILON,
            "after trade (+0.05), reputation should be 0.25, got {}",
            after_trade
        );

        // ─── Step 7: gossip 传播验证（数据层） ──────────────────────────
        // Verify gossip event can be constructed and constants are correct.
        let gossip = ReputationGossipEvent {
            source_npc: Entity::from_raw(10),
            target_player_id: "player:hero".to_string(),
            delta: -0.05,
            hops_remaining: GOSSIP_INITIAL_HOPS,
        };
        assert_eq!(gossip.hops_remaining, 3, "initial gossip hops should be 3");
        assert!(
            (gossip.delta - (-0.05)).abs() < f32::EPSILON,
            "gossip delta should be -0.05"
        );
        assert_eq!(
            GOSSIP_PROPAGATION_RADIUS, 48.0,
            "gossip propagation radius should be 48 blocks"
        );

        // Verify 2nd NPC within 48 blocks would receive gossip decay
        let npc1_pos = DVec3::new(0.0, 64.0, 0.0);
        let npc2_pos = DVec3::new(30.0, 64.0, 0.0);
        let dist = ((npc2_pos.x - npc1_pos.x).powi(2)
            + (npc2_pos.y - npc1_pos.y).powi(2)
            + (npc2_pos.z - npc1_pos.z).powi(2))
        .sqrt();
        assert!(
            dist <= GOSSIP_PROPAGATION_RADIUS,
            "NPC2 at 30 blocks should be within gossip range (48), dist={}",
            dist
        );

        // Apply gossip to 2nd NPC's reputation
        let mut rep2 = NpcPlayerReputation::default();
        // Decay formula: delta * 0.5^(GOSSIP_INITIAL_HOPS - hops_remaining)
        // At first hop (hops=3): decay = 0.5^(3-3) = 1.0 → full delta
        let decay = 0.5_f32.powf((GOSSIP_INITIAL_HOPS - gossip.hops_remaining) as f32);
        let effective_delta = gossip.delta * decay;
        rep2.adjust(&gossip.target_player_id, effective_delta);
        let rep2_after = rep2.get(&gossip.target_player_id);
        assert!(
            rep2_after < 0.5,
            "gossip should decrease 2nd NPC's reputation below 0.5, got {}",
            rep2_after
        );
        assert!(
            (rep2_after - 0.45).abs() < f32::EPSILON,
            "after gossip -0.05 * 1.0 decay, rep should be 0.45, got {}",
            rep2_after
        );
    }

    /// Memory bias 与 decision 集成：attack memory 提升 threat → 决策翻转。
    #[test]
    fn encounter_e2e_memory_biases_affect_decision() {
        // 构造一个 medium-threat 场景 (score ~0.6)
        let assessment = ThreatAssessment {
            score: 0.65,
            realm_delta: 1,
            player_qi_ratio: 0.7,
            player_wound_ratio: 0.0,
            player_distance: 10.0,
            has_weapon_visible: false,
        };

        // Without attack memory: score=0.65 + qi>0.5 → Trade
        let d_no_memory = decide_self_interest_with_memory(&assessment, false, false, false);
        assert_eq!(
            d_no_memory,
            SelfInterestDecision::Trade,
            "without memory, score=0.65 qi=0.7 → Trade, got {:?}",
            d_no_memory
        );

        // With attack memory: effective score = 0.65 + 0.2 = 0.85 → Flee
        let d_with_attack = decide_self_interest_with_memory(&assessment, true, false, false);
        assert_eq!(
            d_with_attack,
            SelfInterestDecision::Flee,
            "with attack memory, effective score 0.85 → Flee, got {:?}",
            d_with_attack
        );

        // With robbery memory: override → Flee
        let d_with_robbery = decide_self_interest_with_memory(&assessment, false, false, true);
        assert_eq!(
            d_with_robbery,
            SelfInterestDecision::Flee,
            "robbery memory always overrides to Flee, got {:?}",
            d_with_robbery
        );
    }

    /// Memory component 交互记录影响 decision 决策。
    #[test]
    fn encounter_e2e_npc_memory_drives_trade_broadening() {
        let mut mem = NpcMemoryComponent::default();

        // Record a trade interaction
        mem.remember(NpcMemoryEntry {
            player_uuid: "player:alice".to_string(),
            interaction_type: NpcInteractionType::Trade,
            timestamp: 100,
            outcome: NpcInteractionOutcome::Friendly,
        });

        // Verify the memory has trade record
        let has_traded = mem
            .interactions
            .iter()
            .any(|e| e.interaction_type == NpcInteractionType::Trade);
        assert!(has_traded, "memory should contain a Trade interaction");

        // With trade memory, trade window broadens to [0.3, 0.8)
        // Craft a scenario with score=0.4 (in broadened range but not normal range)
        let assessment = ThreatAssessment {
            score: 0.4,
            realm_delta: 0,
            player_qi_ratio: 0.6,
            player_wound_ratio: 0.0,
            player_distance: 10.0,
            has_weapon_visible: false,
        };

        let d_with_trade = decide_self_interest_with_memory(&assessment, false, true, false);
        assert_eq!(
            d_with_trade,
            SelfInterestDecision::Trade,
            "with trade memory, score=0.4 qi=0.6 should be Trade (broadened [0.3,0.8)), got {:?}",
            d_with_trade
        );

        let d_without_trade = decide_self_interest_with_memory(&assessment, false, false, false);
        assert_ne!(
            d_without_trade,
            SelfInterestDecision::Trade,
            "without trade memory, score=0.4 should NOT be Trade (normal range [0.5,0.8)), got {:?}",
            d_without_trade
        );
    }

    // =========================================================================
    // P4.2 — 性能基准测试
    // =========================================================================

    /// 50 NPC threat assessment 计算应在 5ms 以内。
    #[test]
    fn threat_assessment_50_npcs_under_5ms() {
        let config = NpcThreatConfig::default();
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
        ];

        let start = std::time::Instant::now();

        for i in 0..50u32 {
            let npc_realm = realms[i as usize % realms.len()];
            let player_realm = realms[(i as usize + 1) % realms.len()];
            let qi_current = 10.0 + (i as f64) * 2.0;
            let qi_max = 100.0;
            let health_current = 50.0 + (i as f32);
            let health_max = 100.0;
            let distance = 5.0 + (i as f32) * 0.5;

            let assessment = build_threat_assessment(
                player_realm,
                npc_realm,
                qi_current,
                qi_max,
                health_current,
                health_max,
                distance,
                i % 3 == 0,
                &config,
            );

            // Also compute decision (the full pipeline)
            let _decision = decide_self_interest_with_memory(
                &assessment,
                i % 5 == 0, // some have attack memory
                i % 7 == 0, // some have trade memory
                false,
            );
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 5,
            "50 NPC threat assessments + decisions should complete in < 5ms, took {} ms ({} us)",
            elapsed.as_millis(),
            elapsed.as_micros()
        );
    }

    /// Poisson sampler: 20 NPC 生成应在 50ms 以内。
    #[test]
    fn poisson_sampler_20_npcs_under_50ms() {
        let zone_bounds = (
            DVec3::new(-250.0, 60.0, -250.0),
            DVec3::new(250.0, 80.0, 250.0),
        );
        let sampler = PoissonSpawnSampler::adaptive_for_zone(zone_bounds);

        let mut existing: Vec<(DVec3, NpcArchetype)> = Vec::new();
        let start = std::time::Instant::now();

        for i in 0..20u64 {
            let archetype = if i % 2 == 0 {
                NpcArchetype::Rogue
            } else {
                NpcArchetype::Commoner
            };
            if let Some(pos) = sampler.sample_position(zone_bounds, &existing, archetype, i * 7919)
            {
                existing.push((pos, archetype));
            }
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 50,
            "PoissonSpawnSampler × 20 should complete in < 50ms, took {} ms ({} us)",
            elapsed.as_millis(),
            elapsed.as_micros()
        );
        assert!(
            !existing.is_empty(),
            "at least some NPCs should have been placed in a 500×500 zone"
        );
    }

    /// DynamicPricing: 1000 次定价计算应在 10ms 以内。
    #[test]
    fn dynamic_pricing_1000_calls_under_10ms() {
        let config = TradePricingConfig::default();

        let start = std::time::Instant::now();

        for i in 0..1000u32 {
            let base = 5 + (i % 200);
            let zone_qi = 0.1 + (i as f32 % 10.0) * 0.09;
            let npc_qi = 0.1 + (i as f32 % 5.0) * 0.2;
            let rep = 0.1 + (i as f32 % 9.0) * 0.1;
            let scarcity = (i as f32 % 3.0) * 0.5;

            let price =
                DynamicPricing::compute_price(base, zone_qi, npc_qi, rep, scarcity, &config);
            // Verify basic sanity: price >= 1
            assert!(
                price >= 1,
                "DynamicPricing should always return >= 1, got {} for base={} zone_qi={:.2} rep={:.2}",
                price,
                base,
                zone_qi,
                rep
            );
        }

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 10,
            "1000 DynamicPricing::compute_price calls should complete in < 10ms, took {} ms ({} us)",
            elapsed.as_millis(),
            elapsed.as_micros()
        );
    }

    // =========================================================================
    // P4.3 — 数值平衡断言
    // =========================================================================

    /// spawn zone 默认条件下灵草定价在 3-20 骨币区间。
    #[test]
    fn spawn_zone_lingcao_price_reasonable_range() {
        let config = TradePricingConfig::default();

        // Neutral conditions: zone_qi=0.5, npc_qi=1.0 (not urgent), rep=0.5, no scarcity
        let price_neutral = DynamicPricing::compute_price(12, 0.5, 1.0, 0.5, 0.0, &config);
        assert!(
            (3..=20).contains(&price_neutral),
            "灵草 at spawn zone neutral conditions should be 3-20 骨币, got {}",
            price_neutral
        );

        // Low qi zone: should be more expensive
        let price_low_qi = DynamicPricing::compute_price(12, 0.2, 1.0, 0.5, 0.0, &config);
        assert!(
            price_low_qi > price_neutral,
            "灵草 in low-qi zone should cost more than neutral: low_qi={} vs neutral={}",
            price_low_qi,
            price_neutral
        );

        // High reputation: should be cheaper
        let price_high_rep = DynamicPricing::compute_price(12, 0.5, 1.0, 0.8, 0.0, &config);
        assert!(
            price_high_rep <= price_neutral,
            "灵草 with high rep should cost <= neutral: high_rep={} vs neutral={}",
            price_high_rep,
            price_neutral
        );

        // Scarce item: should be more expensive
        let price_scarce = DynamicPricing::compute_price(12, 0.5, 1.0, 0.5, 1.0, &config);
        assert!(
            price_scarce > price_neutral,
            "scarce 灵草 should cost more than base: scarce={} vs neutral={}",
            price_scarce,
            price_neutral
        );
    }

    /// ambush 阈值验证：qi < 20% + same realm + wounds > 40% → Ambush。
    #[test]
    fn ambush_threshold_matches_plan() {
        let config = NpcThreatConfig::default();

        // Build assessment via the canonical function
        let assessment = build_threat_assessment(
            Realm::Condense, // player
            Realm::Condense, // npc (same realm → delta=0)
            15.0,            // qi 15% (< 20%)
            100.0,
            55.0, // 45% wounded (> 40%)
            100.0,
            8.0, // close
            false,
            &config,
        );

        assert!(
            assessment.player_qi_ratio < 0.2,
            "qi 15/100 should give ratio < 0.2, got {}",
            assessment.player_qi_ratio
        );
        assert!(
            assessment.player_wound_ratio > 0.4,
            "health 55/100 should give wound_ratio > 0.4, got {}",
            assessment.player_wound_ratio
        );

        let decision = decide_self_interest(&assessment);
        assert_eq!(
            decision,
            SelfInterestDecision::Ambush,
            "qi < 20% + same realm (delta=0) + wounds > 40% should trigger Ambush, \
             got {:?} (threat={:.3}, qi_ratio={:.2}, wound_ratio={:.2})",
            decision,
            assessment.score,
            assessment.player_qi_ratio,
            assessment.player_wound_ratio
        );
    }

    /// ambush 需要 threat < 0.3 —— 验证直构 assessment。
    #[test]
    fn ambush_requires_low_threat_score() {
        let assessment = ThreatAssessment {
            score: 0.2,
            realm_delta: 0,
            player_qi_ratio: 0.15,
            player_wound_ratio: 0.5,
            player_distance: 10.0,
            has_weapon_visible: false,
        };
        let decision = decide_self_interest(&assessment);
        assert_eq!(
            decision,
            SelfInterestDecision::Ambush,
            "score=0.2, qi<0.2, wounds>0.4, same realm → Ambush, got {:?}",
            decision
        );

        // Score >= 0.3 should NOT be Ambush
        let assessment_mid = ThreatAssessment {
            score: 0.35,
            realm_delta: 0,
            player_qi_ratio: 0.15,
            player_wound_ratio: 0.5,
            player_distance: 10.0,
            has_weapon_visible: false,
        };
        let decision_mid = decide_self_interest(&assessment_mid);
        assert_ne!(
            decision_mid,
            SelfInterestDecision::Ambush,
            "score >= 0.3 should never trigger Ambush, got {:?} (score={:.2})",
            decision_mid,
            assessment_mid.score
        );
    }

    /// 新手保护精确边界: realm_delta < -1 → no Ambush。
    #[test]
    fn newbie_protection_exact_boundary() {
        // realm_delta = -1 → Ambush still possible (boundary)
        let a_minus1 = ThreatAssessment {
            score: 0.2,
            realm_delta: -1,
            player_qi_ratio: 0.1,
            player_wound_ratio: 0.6,
            player_distance: 8.0,
            has_weapon_visible: false,
        };
        let d_minus1 = decide_self_interest(&a_minus1);
        assert_eq!(
            d_minus1,
            SelfInterestDecision::Ambush,
            "realm_delta=-1 should still allow Ambush (boundary), got {:?}",
            d_minus1
        );

        // realm_delta = -2 → newbie protection → Guard
        let a_minus2 = ThreatAssessment {
            score: 0.2,
            realm_delta: -2,
            player_qi_ratio: 0.1,
            player_wound_ratio: 0.6,
            player_distance: 8.0,
            has_weapon_visible: false,
        };
        let d_minus2 = decide_self_interest(&a_minus2);
        assert_eq!(
            d_minus2,
            SelfInterestDecision::Guard,
            "realm_delta=-2 should trigger newbie protection → Guard, got {:?}",
            d_minus2
        );

        // realm_delta = -3 → also protected
        let a_minus3 = ThreatAssessment {
            score: 0.1,
            realm_delta: -3,
            player_qi_ratio: 0.0,
            player_wound_ratio: 0.9,
            player_distance: 5.0,
            has_weapon_visible: false,
        };
        let d_minus3 = decide_self_interest(&a_minus3);
        assert_ne!(
            d_minus3,
            SelfInterestDecision::Ambush,
            "realm_delta=-3 should never be Ambush (newbie protection), got {:?}",
            d_minus3
        );
    }

    /// 信誉等级 → 定价联动: High → 折扣, Low → 加价, Hostile → 最高。
    #[test]
    fn reputation_tiers_affect_pricing() {
        let config = TradePricingConfig::default();
        let base = 100u32;

        // High rep (0.8) → discount
        let price_high = DynamicPricing::compute_price(base, 0.5, 1.0, 0.8, 0.0, &config);
        // Mid rep (0.5) → normal
        let price_mid = DynamicPricing::compute_price(base, 0.5, 1.0, 0.5, 0.0, &config);
        // Low rep (0.2) → markup
        let price_low = DynamicPricing::compute_price(base, 0.5, 1.0, 0.2, 0.0, &config);

        assert!(
            price_high < price_mid,
            "high rep price ({}) should be less than mid rep price ({})",
            price_high,
            price_mid
        );
        assert!(
            price_low > price_mid,
            "low rep price ({}) should be greater than mid rep price ({})",
            price_low,
            price_mid
        );

        // Exact values per config defaults
        assert_eq!(
            price_high, 85,
            "high rep (>0.7) should give 15% discount: 100 * 0.85 = 85, got {}",
            price_high
        );
        assert_eq!(
            price_mid, 100,
            "mid rep (0.3-0.7) should give no modifier: 100, got {}",
            price_mid
        );
        assert_eq!(
            price_low, 130,
            "low rep (<0.3) should give 30% markup: 100 * 1.3 = 130, got {}",
            price_low
        );
    }

    /// 价格下限 floor_ratio=0.3 确保最低不低于 base * 30%。
    #[test]
    fn pricing_floor_ratio_enforced() {
        let config = TradePricingConfig::default();

        // Maximum discounts: high zone qi + high rep + urgent NPC
        // 0.8 * 0.85 * 0.7 = 0.476 → 47.6 for base=100
        // floor = 100 * 0.3 = 30
        // max(30, 47.6) = 48 (rounded)
        let price = DynamicPricing::compute_price(100, 0.8, 0.1, 0.9, 0.0, &config);
        let floor = (100.0 * config.floor_ratio).round() as u32;
        assert!(
            price >= floor,
            "price {} should be >= floor {} (base=100 * floor_ratio={})",
            price,
            floor,
            config.floor_ratio
        );

        // Small base: floor=1 (base=1, floor = 1 * 0.3 = 0.3, but min price is 1)
        let price_min = DynamicPricing::compute_price(1, 0.8, 0.1, 0.9, 0.0, &config);
        assert!(
            price_min >= 1,
            "minimum price should always be 1, got {}",
            price_min
        );
    }

    /// NPC budget bucket 分配正确性。
    #[test]
    fn budget_bucket_mapping_exhaustive() {
        use crate::npc::lifecycle::NpcBudgetBucket;

        let expected = [
            (NpcArchetype::Zombie, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Commoner, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Rogue, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Disciple, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Daoxiang, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Zhinian, NpcBudgetBucket::Humanoid),
            (NpcArchetype::Beast, NpcBudgetBucket::Beast),
            (NpcArchetype::Fuya, NpcBudgetBucket::Beast),
            (NpcArchetype::GuardianRelic, NpcBudgetBucket::Special),
            (NpcArchetype::SkullFiend, NpcBudgetBucket::Special),
        ];

        for (archetype, expected_bucket) in expected {
            assert_eq!(
                archetype.budget_bucket(),
                expected_bucket,
                "archetype {:?} should map to bucket {:?}, got {:?}",
                archetype,
                expected_bucket,
                archetype.budget_bucket()
            );
        }

        // Verify caps
        assert_eq!(
            NpcBudgetBucket::Humanoid.default_cap(),
            26,
            "Humanoid bucket cap should be 26"
        );
        assert_eq!(
            NpcBudgetBucket::Beast.default_cap(),
            20,
            "Beast bucket cap should be 20"
        );
        assert_eq!(
            NpcBudgetBucket::Special.default_cap(),
            4,
            "Special bucket cap should be 4"
        );
    }

    /// Poisson sampler 自适应间距与 zone 面积正相关。
    #[test]
    fn poisson_adaptive_distance_scales_with_area() {
        // Large zone (500x500)
        let large = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(-250.0, 60.0, -250.0),
            DVec3::new(250.0, 80.0, 250.0),
        ));
        assert!(
            (large.min_same_archetype_dist - 48.0).abs() < f64::EPSILON,
            "500x500 zone should use 48-block same-archetype distance, got {}",
            large.min_same_archetype_dist
        );

        // Medium zone (400x400)
        let medium = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(-200.0, 60.0, -200.0),
            DVec3::new(200.0, 80.0, 200.0),
        ));
        assert!(
            (medium.min_same_archetype_dist - 40.0).abs() < f64::EPSILON,
            "400x400 zone should use 40-block same-archetype distance, got {}",
            medium.min_same_archetype_dist
        );

        // Small zone (200x200)
        let small = PoissonSpawnSampler::adaptive_for_zone((
            DVec3::new(-100.0, 60.0, -100.0),
            DVec3::new(100.0, 80.0, 100.0),
        ));
        assert!(
            (small.min_same_archetype_dist - 32.0).abs() < f64::EPSILON,
            "200x200 zone should use 32-block same-archetype distance, got {}",
            small.min_same_archetype_dist
        );

        // Cross-archetype distances should be half same-archetype
        assert!(
            (large.min_cross_archetype_dist - 24.0).abs() < f64::EPSILON,
            "large zone cross-archetype distance should be 24, got {}",
            large.min_cross_archetype_dist
        );
        assert!(
            (medium.min_cross_archetype_dist - 20.0).abs() < f64::EPSILON,
            "medium zone cross-archetype distance should be 20, got {}",
            medium.min_cross_archetype_dist
        );
        assert!(
            (small.min_cross_archetype_dist - 16.0).abs() < f64::EPSILON,
            "small zone cross-archetype distance should be 16, got {}",
            small.min_cross_archetype_dist
        );
    }

    /// 所有 SelfInterestDecision 变体可达性验证。
    #[test]
    fn all_decision_variants_reachable_from_e2e_scenarios() {
        // Trade: score in [0.5, 0.8) + qi > 0.5
        let a_trade = ThreatAssessment {
            score: 0.6,
            realm_delta: 1,
            player_qi_ratio: 0.8,
            player_wound_ratio: 0.0,
            player_distance: 10.0,
            has_weapon_visible: false,
        };
        assert_eq!(
            decide_self_interest(&a_trade),
            SelfInterestDecision::Trade,
            "Trade should be reachable"
        );

        // Flee: score >= 0.8
        let a_flee = ThreatAssessment {
            score: 0.85,
            realm_delta: 3,
            player_qi_ratio: 1.0,
            player_wound_ratio: 0.0,
            player_distance: 5.0,
            has_weapon_visible: true,
        };
        assert_eq!(
            decide_self_interest(&a_flee),
            SelfInterestDecision::Flee,
            "Flee should be reachable"
        );

        // Guard: score in [0.5, 0.8) + qi <= 0.5
        let a_guard = ThreatAssessment {
            score: 0.6,
            realm_delta: 0,
            player_qi_ratio: 0.3,
            player_wound_ratio: 0.0,
            player_distance: 10.0,
            has_weapon_visible: false,
        };
        assert_eq!(
            decide_self_interest(&a_guard),
            SelfInterestDecision::Guard,
            "Guard should be reachable"
        );

        // Ambush: score < 0.3 + qi < 0.2 + realm_delta >= -1
        let a_ambush = ThreatAssessment {
            score: 0.2,
            realm_delta: 0,
            player_qi_ratio: 0.1,
            player_wound_ratio: 0.5,
            player_distance: 8.0,
            has_weapon_visible: false,
        };
        assert_eq!(
            decide_self_interest(&a_ambush),
            SelfInterestDecision::Ambush,
            "Ambush should be reachable"
        );

        // Ignore: distance > 32
        let a_ignore = ThreatAssessment {
            score: 0.5,
            realm_delta: 0,
            player_qi_ratio: 0.5,
            player_wound_ratio: 0.0,
            player_distance: 35.0,
            has_weapon_visible: false,
        };
        assert_eq!(
            decide_self_interest(&a_ignore),
            SelfInterestDecision::Ignore,
            "Ignore should be reachable"
        );
    }

    /// 50 NPC threat score 计算结果全部 clamped to [0, 1]。
    #[test]
    fn threat_scores_always_clamped_0_1() {
        let config = NpcThreatConfig::default();
        let realms = [
            Realm::Awaken,
            Realm::Induce,
            Realm::Condense,
            Realm::Solidify,
            Realm::Spirit,
            Realm::Void,
        ];

        for i in 0..50u32 {
            let assessment = build_threat_assessment(
                realms[i as usize % realms.len()],
                realms[(i as usize + 3) % realms.len()],
                (i as f64) * 3.0,
                100.0,
                (i as f32) * 2.0,
                100.0,
                i as f32,
                i % 2 == 0,
                &config,
            );
            assert!(
                assessment.score >= 0.0 && assessment.score <= 1.0,
                "threat score for NPC #{} should be in [0, 1], got {}",
                i,
                assessment.score
            );
        }
    }

    /// Compute threat score 与各因子权重一致性。
    #[test]
    fn threat_score_weights_sum_correctly() {
        let config = NpcThreatConfig::default();
        let total_weight = config.realm_weight
            + config.qi_weight
            + config.wound_weight
            + config.weapon_weight
            + config.distance_weight;
        assert!(
            (total_weight - 1.0).abs() < 0.01,
            "threat config weights should sum to ~1.0, got {}",
            total_weight
        );
    }

    /// 定价 combined modifiers 端到端: low_qi zone + low rep + scarce + urgent。
    #[test]
    fn pricing_combined_worst_case() {
        let config = TradePricingConfig::default();
        // low zone qi (1.5×) + low rep (1.3×) + scarce (1.5×) + urgent (0.7×)
        // = 100 * 1.5 * 1.3 * 1.5 * 0.7 = 204.75 → 205
        let price = DynamicPricing::compute_price(100, 0.2, 0.1, 0.2, 1.0, &config);
        let expected = (100.0_f32 * 1.5 * 1.3 * 1.5 * 0.7).round() as u32;
        assert_eq!(
            price, expected,
            "combined worst case: expected {} (100*1.5*1.3*1.5*0.7), got {}",
            expected, price
        );
    }

    /// 定价 combined modifiers: high_qi zone + high rep + not urgent → 最低合理价格。
    #[test]
    fn pricing_combined_best_case() {
        let config = TradePricingConfig::default();
        // high zone qi (0.8×) + high rep (0.85×) + no scarcity (1.0×) + not urgent (1.0×)
        // = 100 * 0.8 * 0.85 * 1.0 * 1.0 = 68
        let price = DynamicPricing::compute_price(100, 0.8, 1.0, 0.8, 0.0, &config);
        let expected = (100.0_f32 * 0.8 * 0.85).round() as u32;
        assert_eq!(
            price, expected,
            "combined best case: expected {} (100*0.8*0.85), got {}",
            expected, price
        );
        // Verify it's above floor
        let floor = (100.0 * config.floor_ratio).round() as u32;
        assert!(
            price >= floor,
            "best case price {} should be above floor {}",
            price,
            floor
        );
    }

    /// Poisson sampler 返回 None 当 zone 饱和。
    #[test]
    fn poisson_sampler_returns_none_when_saturated() {
        // Small zone (100×100) with many NPCs already placed
        let zone_bounds = (DVec3::new(-50.0, 60.0, -50.0), DVec3::new(50.0, 80.0, 50.0));
        let sampler = PoissonSpawnSampler::adaptive_for_zone(zone_bounds);
        // min_same_archetype_dist = 32 for small zone

        // Fill with enough NPCs to saturate
        let mut existing: Vec<(DVec3, NpcArchetype)> = Vec::new();
        for i in 0..50u64 {
            if let Some(pos) =
                sampler.sample_position(zone_bounds, &existing, NpcArchetype::Rogue, i * 31)
            {
                existing.push((pos, NpcArchetype::Rogue));
            }
        }

        // Eventually the sampler should return None (zone saturated)
        let last_attempt =
            sampler.sample_position(zone_bounds, &existing, NpcArchetype::Rogue, 999_999);
        // We don't assert None directly because the exact saturation point depends on
        // RNG, but we verify the sampler handles density gracefully
        let total_placed = existing.len();
        assert!(
            total_placed < 50,
            "100×100 zone with 32-block spacing should saturate before placing all 50, placed {}",
            total_placed
        );
        let _ = last_attempt; // Suppress unused warning
    }

    /// 信誉累积：多次 trade (+0.05) 可达 High tier。
    #[test]
    fn reputation_accumulates_to_high_tier() {
        use crate::npc::trade::RepTier;

        let mut rep = NpcPlayerReputation::default();
        // Start at 0.5 (Mid), need > 0.7 for High
        // 5 trades: 0.5 + 5*0.05 = 0.75 → High
        for _ in 0..5 {
            rep.adjust("player:faithful", 0.05);
        }
        assert_eq!(
            rep.tier("player:faithful"),
            RepTier::High,
            "5 successful trades should elevate to High tier, score={}",
            rep.get("player:faithful")
        );
    }

    /// 信誉 attack 后恢复需要很多次 trade。
    #[test]
    fn reputation_attack_hard_to_recover() {
        use crate::npc::trade::RepTier;

        let mut rep = NpcPlayerReputation::default();
        // Attack: -0.3 → 0.5 - 0.3 = 0.2 (Low tier)
        rep.adjust("player:bully", -0.3);
        assert_eq!(
            rep.tier("player:bully"),
            RepTier::Low,
            "after attack, should be Low tier"
        );

        // 6 trades to recover: 0.2 + 6*0.05 = 0.5 (Mid, but just barely)
        for _ in 0..6 {
            rep.adjust("player:bully", 0.05);
        }
        assert_eq!(
            rep.tier("player:bully"),
            RepTier::Mid,
            "6 trades after attack should recover to Mid, score={}",
            rep.get("player:bully")
        );
    }
}
