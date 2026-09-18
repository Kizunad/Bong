use bong_server::cultivation::components::ColorKind;
use bong_server::network::agent_bridge::payload_type_label;
use bong_server::schema::agent_ui::{AgentUiClosePayloadV1, AgentUiRequestPayloadV1};
use bong_server::schema::botany::BotanyPlantV2RenderProfileV1;
use bong_server::schema::craft::{
    CraftOutcomeV1, CraftSessionStateV1, RecipeListV1, RecipeUnlockedV1,
};
use bong_server::schema::cultivation::{InsightOfferV1, SkillMilestoneSnapshotV1};
use bong_server::schema::movement::{
    MovementActionRequestV1, MovementActionV1, MovementStateV1, MovementZoneKindV1,
};
use bong_server::schema::poison_trait::{
    PoisonDoseEventV1, PoisonOverdoseEventV1, PoisonOverdoseSeverityV1, PoisonSideEffectTagV1,
    PoisonTraitStateV1,
};
use bong_server::schema::processing::FreshnessUpdateV1;
use bong_server::schema::realm_vision::{RealmVisionParamsV1, SpiritualSenseTargetsV1};
use bong_server::schema::server_data::*;
use bong_server::schema::social::SocialExposureEventV1;
use bong_server::schema::tuike::FalseSkinStateV1;
use bong_server::schema::woliu::VortexFieldStateV1;
use bong_server::schema::world_state::ZoneStatusV1;
use bong_server::schema::yidao::{HealerNpcAiStateV1, YidaoHudStateV1};
use bong_server::skill::config::SkillConfigSnapshot;

/// Catches wire-vs-label drift like the QuickSlotConfig "snake_case" bug
/// (would have routed `quick_slot_config` while client expected `quickslot_config`).
#[test]
fn hud_payload_wire_type_matches_label() {
    use bong_server::schema::combat_hud::*;
    let cases: Vec<ServerDataPayloadV1> =
        vec![
            ServerDataPayloadV1::CombatHudState(CombatHudStateV1 {
                hp_percent: 1.0,
                qi_percent: 1.0,
                stamina_percent: 1.0,
                combat_active: false,
                derived: DerivedAttrFlagsV1::default(),
            }),
            ServerDataPayloadV1::WoundsSnapshot(WoundsSnapshotV1 { wounds: vec![] }),
            ServerDataPayloadV1::DefenseWindow(DefenseWindowV1 {
                duration_ms: 200,
                started_at_ms: 0,
                expires_at_ms: 200,
            }),
            ServerDataPayloadV1::CastSync(CastSyncV1 {
                phase: CastPhaseV1::Idle,
                slot: 0,
                duration_ms: 0,
                started_at_ms: 0,
                outcome: CastOutcomeV1::None,
            }),
            ServerDataPayloadV1::QuickSlotConfig(QuickSlotConfigV1 {
                eligible_item_ids: vec![],
                slots: vec![None; bong_server::combat::components::QuickSlotBindings::SLOT_COUNT],
                cooldown_until_ms: vec![
                0;
                bong_server::combat::components::QuickSlotBindings::SLOT_COUNT
            ],
                ack_request_id: None,
                bind_accepted: None,
            }),
            ServerDataPayloadV1::SkillBarConfig(SkillBarConfigV1 {
                slots: vec![None; bong_server::combat::components::SkillBarBindings::SLOT_COUNT],
                cooldown_until_ms:
                    vec![0; bong_server::combat::components::SkillBarBindings::SLOT_COUNT],
            }),
            ServerDataPayloadV1::TechniquesSnapshot(TechniquesSnapshotV1 { entries: vec![] }),
            ServerDataPayloadV1::SkillConfigSnapshot(SkillConfigSnapshot {
                configs: Default::default(),
            }),
            ServerDataPayloadV1::UnlocksSync(UnlocksSyncV1::default()),
            ServerDataPayloadV1::DerivedAttrsSync(DerivedAttrsSyncV1 {
                flying: false,
                flying_qi_remaining: 0.0,
                flying_force_descent_at_ms: 0,
                phasing: false,
                phasing_until_ms: 0,
                tribulation_locked: false,
                tribulation_stage: String::new(),
                throughput_peak_norm: 0.0,
                tuike_layers: 0,
                vortex_active: false,
            }),
            ServerDataPayloadV1::EventStreamPush(EventStreamPushV1 {
                channel: EventChannelV1::Combat,
                priority: EventPriorityV1::P1Important,
                source_tag: String::new(),
                text: "x".to_string(),
                color: 0,
                created_at_ms: 0,
            }),
            ServerDataPayloadV1::VortexState(VortexFieldStateV1 {
                caster: "entity:1".to_string(),
                active: true,
                center: [0.0, 64.0, 0.0],
                radius: 1.5,
                delta: 0.25,
                env_qi_at_cast: 0.9,
                maintain_remaining_ticks: 80,
                intercepted_count: 1,
                active_skill_id: "woliu.hold".to_string(),
                charge_progress: 1.0,
                cooldown_until_ms: 0,
                backfire_level: String::new(),
                turbulence_radius: 1.0,
                turbulence_intensity: 0.5,
                turbulence_until_ms: 0,
            }),
            ServerDataPayloadV1::FalseSkinState(FalseSkinStateV1 {
                target_id: "offline:Azure".to_string(),
                kind: Some(bong_server::schema::tuike::FalseSkinKindV1::SpiderSilk),
                layers_remaining: 1,
                contam_capacity_per_layer: 10.0,
                absorbed_contam: 3.0,
                equipped_at_tick: 7,
                layers: Vec::new(),
            }),
            ServerDataPayloadV1::RiftPortalState(RiftPortalStateV1 {
                entity_id: 1,
                kind: RiftPortalKindV1::MainRift,
                direction: RiftPortalDirectionV1::Exit,
                family_id: "tsy_lingxu_01".to_string(),
                world_pos: [0.0, 64.0, 0.0],
                trigger_radius: 2.0,
                current_extract_ticks: 160,
                activation_window_end: None,
            }),
            ServerDataPayloadV1::RiftPortalRemoved(RiftPortalRemovedV1 { entity_id: 1 }),
            ServerDataPayloadV1::ExtractStarted(ExtractStartedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                portal_kind: RiftPortalKindV1::MainRift,
                required_ticks: 160,
                at_tick: 10,
            }),
            ServerDataPayloadV1::ExtractProgress(ExtractProgressV1 {
                player_id: "offline:Kiz".to_string(),
                portal_entity_id: 1,
                elapsed_ticks: 5,
                required_ticks: 160,
            }),
            ServerDataPayloadV1::ExtractCompleted(ExtractCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                portal_kind: RiftPortalKindV1::MainRift,
                family_id: "tsy_lingxu_01".to_string(),
                exit_world_pos: [0.0, 64.0, 0.0],
                at_tick: 170,
            }),
            ServerDataPayloadV1::ExtractAborted(ExtractAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractAbortedReasonV1::PortalOccupied,
            }),
            ServerDataPayloadV1::ExtractFailed(ExtractFailedV1 {
                player_id: "offline:Kiz".to_string(),
                reason: ExtractFailedReasonV1::SpiritQiDrained,
            }),
            ServerDataPayloadV1::TsyCollapseStartedIpc(TsyCollapseStartedIpcV1 {
                family_id: "tsy_lingxu_01".to_string(),
                at_tick: 100,
                remaining_ticks: 600,
                collapse_tear_entity_ids: vec![2, 3, 4],
            }),
            ServerDataPayloadV1::ContainerState(ContainerStateV1 {
                entity_id: 42,
                visual_entity_id: Some(2048),
                kind: ContainerKindV1::StoragePouch,
                family_id: "tsy_lingxu_01".to_string(),
                world_pos: [8.0, 64.0, -4.0],
                locked: None,
                depleted: false,
                searched_by_player_id: None,
            }),
            ServerDataPayloadV1::SearchStarted(SearchStartedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                required_ticks: 200,
                at_tick: 100,
            }),
            ServerDataPayloadV1::SearchProgress(SearchProgressV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                elapsed_ticks: 20,
                required_ticks: 200,
            }),
            ServerDataPayloadV1::SearchCompleted(SearchCompletedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                family_id: "tsy_lingxu_01".to_string(),
                loot_preview: vec![LootPreviewItemV1 {
                    template_id: "bone_coin".to_string(),
                    display_name: "骨币".to_string(),
                    stack_count: 3,
                }],
                at_tick: 300,
            }),
            ServerDataPayloadV1::SearchAborted(SearchAbortedV1 {
                player_id: "offline:Kiz".to_string(),
                container_entity_id: 42,
                reason: SearchAbortReasonV1::Cancelled,
                at_tick: 150,
            }),
            ServerDataPayloadV1::TribulationBroadcast(TribulationBroadcastV1::active(
                "Kiz", "warn", 12.0, -34.0, 60_000,
            )),
            ServerDataPayloadV1::TribulationState(TribulationStateV1 {
                active: true,
                char_id: "offline:Kiz".to_string(),
                actor_name: "Kiz".to_string(),
                kind: "du_xu".to_string(),
                phase: "wave".to_string(),
                world_x: 12.0,
                world_z: -34.0,
                wave_current: 2,
                wave_total: 5,
                started_tick: 120,
                phase_started_tick: 2_400,
                next_wave_tick: 2_700,
                failed: false,
                half_step_on_success: false,
                participants: vec!["offline:Kiz".to_string()],
                result: None,
            }),
            ServerDataPayloadV1::AscensionQuota(AscensionQuotaV1::new(1, 3)),
            ServerDataPayloadV1::HeartDemonOffer(HeartDemonOfferV1 {
                offer_id: "heart_demon:1:100".to_string(),
                trigger_id: "heart_demon:1:100".to_string(),
                trigger_label: "心魔劫临身".to_string(),
                realm_label: "渡虚劫 · 心魔".to_string(),
                composure: 0.5,
                quota_remaining: 1,
                quota_total: 1,
                expires_at_ms: 1_700_000_000_000,
                choices: vec![HeartDemonOfferChoiceV1 {
                    choice_id: "heart_demon_choice_0".to_string(),
                    category: "Composure".to_string(),
                    title: "守本心".to_string(),
                    effect_summary: "回复少量当前真元".to_string(),
                    flavor: "你把呼吸压回丹田。".to_string(),
                    style_hint: "稳妥".to_string(),
                }],
            }),
            ServerDataPayloadV1::BurstMeridianEvent(BurstMeridianEventV1 {
                skill: "beng_quan".to_string(),
                caster: "offline:Kiz".to_string(),
                target: Some("entity:42".to_string()),
                tick: 12,
                overload_ratio: 1.5,
                integrity_snapshot: 0.9,
            }),
            ServerDataPayloadV1::BreakthroughCinematic(BreakthroughCinematicS2cV1 {
                actor_id: "offline:Kiz".to_string(),
                phase: "apex".to_string(),
                phase_tick: 0,
                phase_duration_ticks: 80,
                realm_from: "Condense".to_string(),
                realm_to: "Solidify".to_string(),
                result: "success".to_string(),
                interrupted: false,
                world_pos: [12.0, 64.0, -8.0],
                visible_radius_blocks: 1024.0,
                global: false,
                distant_billboard: true,
                particle_density: 2.2,
                intensity: 0.78,
                season_overlay: "adaptive".to_string(),
                style: "golden_core".to_string(),
                at_tick: 2400,
            }),
            ServerDataPayloadV1::FullPowerChargingState(FullPowerChargingStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: true,
                qi_committed: 150.0,
                target_qi: 600.0,
                started_tick: 12,
            }),
            ServerDataPayloadV1::FullPowerRelease(FullPowerReleaseV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                target_uuid: Some("00000000-0000-0000-0000-000000000002".to_string()),
                qi_released: 600.0,
                tick: 24,
                hit_position: Some([8.0, 66.0, 8.0]),
            }),
            ServerDataPayloadV1::FullPowerExhaustedState(FullPowerExhaustedStateV1 {
                caster_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                active: true,
                started_tick: 24,
                recovery_at_tick: 1224,
            }),
            ServerDataPayloadV1::QiColorObserved(QiColorObservedV1 {
                observer: "offline:Kiz".to_string(),
                observed: "offline:Azure".to_string(),
                main: ColorKind::Intricate,
                secondary: Some(ColorKind::Heavy),
                is_chaotic: false,
                is_hunyuan: false,
                realm_diff: 2,
            }),
            ServerDataPayloadV1::PoisonDoseEvent(PoisonDoseEventV1 {
                v: 1,
                player_entity_id: 7,
                dose_amount: 5.0,
                side_effect_tag: PoisonSideEffectTagV1::QiFocusDrift2h,
                poison_level_after: 17.0,
                digestion_after: 50.0,
                at_tick: 100,
            }),
            ServerDataPayloadV1::PoisonOverdoseEvent(PoisonOverdoseEventV1 {
                v: 1,
                player_entity_id: 7,
                severity: PoisonOverdoseSeverityV1::Moderate,
                overflow: 30.0,
                lifespan_penalty_years: 1.0,
                micro_tear_probability: 0.1,
                at_tick: 120,
            }),
            ServerDataPayloadV1::PoisonTraitState(PoisonTraitStateV1 {
                v: 1,
                player_entity_id: 7,
                poison_toxicity: 17.0,
                digestion_current: 50.0,
                digestion_capacity: 100.0,
                toxicity_tier_unlocked: false,
            }),
            ServerDataPayloadV1::BotanyPlantV2RenderProfiles(vec![BotanyPlantV2RenderProfileV1 {
                plant_id: "ying_yuan_gu".to_string(),
                base_mesh_ref: "red_mushroom".to_string(),
                tint_rgb: 0xFFA040,
                tint_rgb_secondary: None,
                model_overlay: bong_server::schema::botany::BotanyModelOverlayV1::Emissive,
            }]),
            ServerDataPayloadV1::GatheringSession {
                session_id: "gathering:herb:offline-kiz".to_string(),
                progress_ticks: 20,
                total_ticks: 40,
                target_name: "凝脉草".to_string(),
                target_type: GatheringTargetTypeV1::Herb,
                quality_hint: GatheringQualityHintV1::FineLikely,
                tool_used: Some("hoe_iron".to_string()),
                interrupted: false,
                completed: false,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "mining:10:64:10:FanTie".to_string(),
                progress_ticks: 60,
                total_ticks: 60,
                target_name: "凡铁矿".to_string(),
                target_type: GatheringTargetTypeV1::Ore,
                quality_hint: GatheringQualityHintV1::Perfect,
                tool_used: Some("pickaxe_iron".to_string()),
                interrupted: false,
                completed: true,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "lumber:offline-kiz:1".to_string(),
                progress_ticks: 0,
                total_ticks: 50,
                target_name: "灵木".to_string(),
                target_type: GatheringTargetTypeV1::Wood,
                quality_hint: GatheringQualityHintV1::Normal,
                tool_used: None,
                interrupted: true,
                completed: false,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "gathering:herb:fine".to_string(),
                progress_ticks: 40,
                total_ticks: 40,
                target_name: "优良凝脉草".to_string(),
                target_type: GatheringTargetTypeV1::Herb,
                quality_hint: GatheringQualityHintV1::Fine,
                tool_used: Some("hoe_copper".to_string()),
                interrupted: false,
                completed: true,
            },
            ServerDataPayloadV1::GatheringSession {
                session_id: "lumber:perfect-possible".to_string(),
                progress_ticks: 45,
                total_ticks: 50,
                target_name: "灵木".to_string(),
                target_type: GatheringTargetTypeV1::Wood,
                quality_hint: GatheringQualityHintV1::PerfectPossible,
                tool_used: Some("axe_copper".to_string()),
                interrupted: false,
                completed: false,
            },
            ServerDataPayloadV1::RealmVisionParams(RealmVisionParamsV1 {
                fog_start: 30.0,
                fog_end: 60.0,
                fog_color_rgb: 0xB8B0A8,
                fog_shape: bong_server::schema::realm_vision::FogShapeV1::Cylinder,
                vignette_alpha: 0.55,
                tint_color_argb: 0x0FF0EDE8,
                particle_density: 0.0,
                transition_ticks: 100,
                server_view_distance_chunks: 4,
                post_fx_sharpen: 0.0,
            }),
            ServerDataPayloadV1::SpiritualSenseTargets(SpiritualSenseTargetsV1 {
                generation: 1,
                entries: vec![bong_server::schema::realm_vision::SenseEntryV1 {
                    kind: bong_server::schema::realm_vision::SenseKindV1::LivingQi,
                    x: 8.0,
                    y: 64.0,
                    z: -4.0,
                    intensity: 0.75,
                }],
            }),
            ServerDataPayloadV1::HealerNpcAiState(HealerNpcAiStateV1 {
                healer_id: "npc:doctor".to_string(),
                active_action: "triage".to_string(),
                queue_len: 2,
                reputation: 12,
                retreating: false,
            }),
            ServerDataPayloadV1::YidaoHudState(YidaoHudStateV1 {
                healer_id: "npc:doctor".to_string(),
                reputation: 12,
                peace_mastery: 48.0,
                karma: 3.5,
                active_skill: Some(bong_server::schema::yidao::YidaoSkillIdV1::MeridianRepair),
                patient_ids: vec!["offline:Kiz".to_string()],
                patient_hp_percent: Some(0.75),
                patient_contam_total: Some(1.25),
                severed_meridian_count: 1,
                contract_count: 2,
                mass_preview_count: 0,
            }),
            ServerDataPayloadV1::MovementState(MovementStateV1 {
                current_speed_multiplier: 0.75,
                stamina_cost_active: true,
                movement_action: MovementActionV1::Dashing,
                zone_kind: MovementZoneKindV1::Normal,
                dash_cooldown_remaining_ticks: 40,
                dash_cooldown_total_ticks: 40,
                hitbox_height_blocks: 1.8,
                stamina_current: 85.0,
                stamina_max: 100.0,
                low_stamina: false,
                last_action_tick: Some(120),
                rejected_action: Some(MovementActionRequestV1::Dash),
            }),
            ServerDataPayloadV1::CoffinState(CoffinStateV1 {
                in_coffin: true,
                lifespan_rate_multiplier: 0.9,
                coffin_grade: Some(CoffinGradeV1::Mundane),
            }),
            // ─── plan-craft-v1 P2 wire ↔ label drift guard ──────
            ServerDataPayloadV1::CraftRecipeList(Box::new(RecipeListV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipes: vec![],
                ts: 1234567,
            })),
            ServerDataPayloadV1::CraftSessionState(CraftSessionStateV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                active: false,
                recipe_id: None,
                elapsed_ticks: 0,
                total_ticks: 0,
                completed_count: 0,
                total_count: 0,
                ts: 1234567,
            }),
            ServerDataPayloadV1::CraftOutcome(CraftOutcomeV1::Completed {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example.eclipse_needle.iron".to_string(),
                output_template: "eclipse_needle_iron".to_string(),
                output_count: 3,
                completed_at_tick: 5000,
                ts: 1234567,
            }),
            ServerDataPayloadV1::RecipeUnlocked(RecipeUnlockedV1 {
                v: 1,
                player_id: "offline:Kiz".to_string(),
                recipe_id: "craft.example.fake_skin.light".to_string(),
                source: bong_server::schema::craft::UnlockEventSourceV1::Insight {
                    trigger: bong_server::schema::craft::InsightTriggerV1::NearDeath,
                },
                unlocked_at_tick: 8000,
                ts: 1234567,
            }),
            ServerDataPayloadV1::WorkbenchOpen {
                entity_id: 42,
                position: [1, 64, -2],
            },
            // F9 跨层修复：出生引导棺权威坐标广播 wire tag pin。
            ServerDataPayloadV1::TutorialCoffinPos {
                position: [0, 69, 0],
            },
            ServerDataPayloadV1::CombatEventFloater(CombatEventFloaterV1 {
                events: vec![CombatEventFloaterEntryV1 {
                    kind: "hit".to_string(),
                    amount: 5.0,
                    text: "5".to_string(),
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    outgoing: false,
                }],
            }),
            ServerDataPayloadV1::KnockbackSync(KnockbackSyncV1 {
                distance_blocks: 4.0,
                velocity_blocks_per_tick: 0.8,
                duration_ticks: 5,
                kinetic_energy: 22.4,
                collision_damage: Some(3.0),
                chain_depth: 2,
                block_broken: true,
            }),
            ServerDataPayloadV1::TechniqueProficiencyUpdate(TechniqueProficiencyUpdateV1 {
                technique_id: "sword.cleave".to_string(),
                proficiency: 0.42,
                gain: 0.008,
            }),
            ServerDataPayloadV1::PillBuffStatus(PillBuffStatusV1 {
                buff_id: "huo_xue_dan".to_string(),
                remaining_ticks: 3000,
                effect_multiplier: 1.0,
            }),
            // ─── plan-exploration-probe-return-v1 P0 ────────────────
            ServerDataPayloadV1::MineralProbeResult(MineralProbeResultV1 {
                kind: "found".to_string(),
                mineral_id: Some("chi_tong_ore".to_string()),
                remaining_units: Some(23),
                display_name_zh: Some("赤铜矿脉".to_string()),
                denial_reason: None,
            }),
            // ─── plan-exploration-probe-return-v1 P1: FreshnessUpdate wire/label guard ──
            ServerDataPayloadV1::FreshnessUpdate(FreshnessUpdateV1 {
                item_uuid: "42".to_string(),
                freshness: 0.75,
                profile_name: "test_decay".to_string(),
            }),
            // ─── plan-exploration-probe-return-v1 P2: InsightOffer wire/label guard ─────
            ServerDataPayloadV1::InsightOffer(InsightOfferV1 {
                offer_id: "insight:1:100".to_string(),
                trigger_id: "insight:1:100".to_string(),
                character_id: "offline:Kiz".to_string(),
                choices: vec![bong_server::schema::cultivation::InsightChoiceV1 {
                    category: "Qi".to_string(),
                    effect_kind: "qi_max".to_string(),
                    magnitude: 0.05,
                    flavor_text: "气海微扩张。".to_string(),
                    narrator_voice: None,
                    alignment: None,
                    cost_kind: None,
                    cost_magnitude: None,
                    cost_flavor: None,
                }],
            }),
            // ─── plan-agent-ui-data-v1 P0: Agent UI wire/label guard ─────────
            ServerDataPayloadV1::AgentUiRequest(AgentUiRequestPayloadV1 {
                request_id: "agent-ui-req".to_string(),
                target_player: "offline:Kiz".to_string(),
                xml: "<owo-ui><components><label>test</label></components></owo-ui>".to_string(),
                timeout_ticks: 600,
            }),
            ServerDataPayloadV1::AgentUiClose(AgentUiClosePayloadV1 {
                request_id: "agent-ui-req".to_string(),
                reason: Some("invalid_button_id".to_string()),
            }),
            // ─── plan-halfstep-rechallenge-integration-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::HalfStepRechallenge(HalfStepRechallengeV1 {
                active: true,
                char_id: "offline:Kiz".to_string(),
                rechallenge_window_until: 50_000,
                at_tick: 1_000,
            }),
            // ─── plan-inventory-hint-panel-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::InventoryMoveRejected(InventoryMoveRejectedV1 {
                reason: "worn_cap_full".to_string(),
                required_realm: None,
                slot: Some("chest".to_string()),
                cap: Some(3),
            }),
            // ─── plan-scroll-reading-v1 P0 wire/label guard ─────
            ServerDataPayloadV1::ScrollOpen {
                scroll_id: "scroll_meridian_primer".to_string(),
                title: "《经脉浅述·残卷》".to_string(),
                body_pages: vec!["第一页".to_string(), "第二页".to_string()],
            },
        ];

    for payload in cases {
        let label = payload_type_label(payload.payload_type());
        let envelope = ServerDataV1::new(payload);
        let bytes = serde_json::to_vec(&envelope).expect("serialize");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("decode");
        let wire_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(
            wire_type, label,
            "wire type {wire_type} does not match payload_type_label {label}"
        );
    }
}

// ─── plan-scroll-reading-v1 P0：ScrollOpen serde pin + TS↔Rust sample 对拍 ───

/// TS 端 sample（TypeBox source of truth）必须反序列化为 ScrollOpen 且字段全等。
#[test]
fn scroll_open_ts_sample_deserializes_in_rust() {
    let json = include_str!(
        "../../../../agent/packages/schema/samples/server-data.scroll-open.sample.json"
    );
    let envelope: ServerDataV1 = serde_json::from_str(json)
        .unwrap_or_else(|e| panic!("scroll-open sample should deserialize: {e}"));
    match envelope.payload {
        ServerDataPayloadV1::ScrollOpen {
            scroll_id,
            title,
            body_pages,
        } => {
            assert_eq!(scroll_id, "scroll_meridian_primer");
            assert_eq!(title, "《经脉浅述·残卷》");
            assert_eq!(
                body_pages.len(),
                3,
                "sample 应有 3 页正文，得到 {}",
                body_pages.len()
            );
        }
        other => panic!("expected ScrollOpen, got {other:?}"),
    }
}

#[test]
fn scroll_open_roundtrip() {
    let payload = ServerDataPayloadV1::ScrollOpen {
        scroll_id: "scroll_meridian_primer".to_string(),
        title: "《经脉浅述·残卷》".to_string(),
        body_pages: vec!["第一页".to_string(), "第二页".to_string()],
    };
    let envelope = ServerDataV1::new(payload);
    let bytes = serde_json::to_vec(&envelope).expect("ScrollOpen serializes");
    let decoded: ServerDataV1 =
        serde_json::from_slice(&bytes).expect("ScrollOpen round-trip deserializes");
    match decoded.payload {
        ServerDataPayloadV1::ScrollOpen {
            scroll_id,
            title,
            body_pages,
        } => {
            assert_eq!(scroll_id, "scroll_meridian_primer");
            assert_eq!(title, "《经脉浅述·残卷》");
            assert_eq!(body_pages, vec!["第一页".to_string(), "第二页".to_string()]);
        }
        other => panic!("expected ScrollOpen after round-trip, got {other:?}"),
    }
}

/// 边界：body_pages 为空数组——wire 层不校验（校验在 TOML 解析层 `parse_readable_scroll_spec`），
/// 但 serde 本身必须允许空数组反序列化（不是 wire 契约拒绝的形状）。
#[test]
fn scroll_open_wire_accepts_empty_body_pages() {
    let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","title":"t","body_pages":[]}"#;
    let envelope: ServerDataV1 =
        serde_json::from_str(json).expect("empty body_pages array should deserialize");
    match envelope.payload {
        ServerDataPayloadV1::ScrollOpen { body_pages, .. } => {
            assert!(body_pages.is_empty());
        }
        other => panic!("expected ScrollOpen, got {other:?}"),
    }
}

/// 缺失 title 字段应反序列化失败。
#[test]
fn scroll_open_rejects_missing_title() {
    let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","body_pages":["p1"]}"#;
    let result: Result<ServerDataV1, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "scroll_open without title should fail deserialization"
    );
}

/// 额外字段被拒绝（deny_unknown_fields）。
#[test]
fn scroll_open_rejects_extra_fields() {
    let json = r#"{"v":1,"type":"scroll_open","scroll_id":"x","title":"t","body_pages":["p1"],"extra":true}"#;
    let result: Result<ServerDataV1, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "scroll_open with extra field should fail deserialization (deny_unknown_fields)"
    );
}

#[test]
fn social_server_data_wire_uses_single_envelope_version() {
    let envelope = ServerDataV1::new(ServerDataPayloadV1::SocialExposure(SocialExposureEventV1 {
        v: 1,
        actor: "char:alice".to_string(),
        kind: bong_server::schema::social::ExposureKindV1::Chat,
        witnesses: vec!["char:bob".to_string()],
        tick: 42,
        zone: Some("spawn".to_string()),
    }));
    let value = serde_json::to_value(&envelope).expect("serialize social exposure");
    assert_eq!(value["v"], 1);
    assert_eq!(value["type"], "social_exposure");
    assert_eq!(value["kind"], "chat");
    assert!(
        value.get("event_v").is_none(),
        "server_data payload must not duplicate nested event version"
    );
}

#[test]
fn social_server_data_deserializes_without_nested_event_version() {
    let json = include_str!(
        "../../../../agent/packages/schema/samples/server-data.social-renown-delta.sample.json"
    );
    let payload: ServerDataV1 = serde_json::from_str(json).expect("social renown sample");

    match payload.payload {
        ServerDataPayloadV1::SocialRenownDelta(event) => {
            assert_eq!(event.v, 1);
            assert_eq!(event.char_id, "char:steve");
            assert_eq!(event.tags_added[0].tag, "kept_pact");
        }
        other => panic!("expected SocialRenownDelta, got {other:?}"),
    }
}

#[test]
fn cultivation_detail_roundtrip_and_size_budget() {
    let channel_ids: Vec<String> = bong_server::cultivation::components::MeridianId::ALL
        .iter()
        .map(|m| m.channel_id().to_string())
        .collect();
    let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
        realm: "Induce".to_string(),
        channel_ids: channel_ids.clone(),
        opened: vec![true; 20],
        flow_rate: vec![1.5; 20],
        flow_capacity: vec![10.25; 20],
        integrity: vec![0.87; 20],
        open_progress: vec![1.0; 20],
        cracks_count: vec![0; 20],
        contamination_total: 0.0,
        lifespan: Some(LifespanPreviewV1 {
            years_lived: 42.0,
            cap_by_realm: 200,
            remaining_years: 158.0,
            death_penalty_years: 10,
            tick_rate_multiplier: 1.0,
            is_wind_candle: false,
        }),
        recent_skill_milestones_summary: "t82000:skill:herbalism:lv3".to_string(),
        skill_milestones: vec![SkillMilestoneSnapshotV1 {
            skill: "herbalism".to_string(),
            new_lv: 3,
            achieved_at: 82_000,
            narration: "你摘得百草渐熟，今已识八分。".to_string(),
            total_xp_at: 550,
        }],
        qi_color_main: ColorKind::Intricate,
        qi_color_secondary: Some(ColorKind::Heavy),
        qi_color_chaotic: false,
        qi_color_hunyuan: false,
        practice_weights: vec![PracticeWeightV1 {
            color: ColorKind::Intricate,
            weight: 42.0,
            ratio: 0.7,
        }],
        target_meridian: Some(channel_ids[4].clone()),
        body_plan_id: "humanoid".to_string(),
        race_id: String::new(),
        form_race_id: String::new(),
        form_body_plan_id: String::new(),
        intrinsic_is_humanoid: false,
        form_is_humanoid: false,
    });
    let bytes = payload
        .to_json_bytes_checked()
        .expect("cultivation_detail must fit MAX_PAYLOAD_BYTES");
    assert!(
        bytes.len() <= bong_server::schema::common::MAX_PAYLOAD_BYTES,
        "over budget: {} bytes",
        bytes.len()
    );
    let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
    match back.payload {
        ServerDataPayloadV1::CultivationDetail {
            channel_ids: back_channel_ids,
            opened,
            flow_rate,
            lifespan,
            recent_skill_milestones_summary,
            skill_milestones,
            qi_color_main,
            qi_color_secondary,
            practice_weights,
            target_meridian,
            ..
        } => {
            assert_eq!(back_channel_ids, channel_ids);
            assert_eq!(opened.len(), 20);
            assert_eq!(flow_rate.len(), 20);
            assert_eq!(flow_rate[0], 1.5);
            assert_eq!(lifespan.unwrap().death_penalty_years, 10);
            assert_eq!(
                recent_skill_milestones_summary,
                "t82000:skill:herbalism:lv3"
            );
            assert_eq!(skill_milestones.len(), 1);
            assert_eq!(skill_milestones[0].skill, "herbalism");
            assert_eq!(qi_color_main, ColorKind::Intricate);
            assert_eq!(qi_color_secondary, Some(ColorKind::Heavy));
            assert_eq!(practice_weights[0].color, ColorKind::Intricate);
            assert_eq!(practice_weights[0].weight, 42.0);
            assert_eq!(target_meridian, Some(channel_ids[4].clone()));
        }
        other => panic!("expected CultivationDetail, got {other:?}"),
    }
}

/// plan-race-system-v1 P1c — `channel_ids`/其余并行数组不再假设恰好 20 条；一个
/// 合成的 6 脉非 humanoid 构型（如 P5 飞鲸草案）必须同样 round-trip 成功。
#[test]
fn cultivation_detail_non_humanoid_channel_count_roundtrips() {
    let channel_ids = vec![
        "skull_channel".to_string(),
        "spine_channel".to_string(),
        "dorsal_fin_channel".to_string(),
        "pect_fin_l_channel".to_string(),
        "pect_fin_r_channel".to_string(),
        "tail_fin_channel".to_string(),
    ];
    let n = channel_ids.len();
    let payload = ServerDataV1::new(ServerDataPayloadV1::CultivationDetail {
        realm: "Awaken".to_string(),
        channel_ids: channel_ids.clone(),
        opened: vec![false; n],
        flow_rate: vec![1.0; n],
        flow_capacity: vec![10.0; n],
        integrity: vec![1.0; n],
        open_progress: vec![0.0; n],
        cracks_count: vec![0; n],
        contamination_total: 0.0,
        lifespan: None,
        recent_skill_milestones_summary: String::new(),
        skill_milestones: Vec::new(),
        qi_color_main: ColorKind::Mellow,
        qi_color_secondary: None,
        qi_color_chaotic: false,
        qi_color_hunyuan: false,
        practice_weights: Vec::new(),
        target_meridian: Some("tail_fin_channel".to_string()),
        body_plan_id: "whale".to_string(),
        race_id: String::new(),
        form_race_id: String::new(),
        form_body_plan_id: String::new(),
        intrinsic_is_humanoid: false,
        form_is_humanoid: false,
    });
    let bytes = payload
        .to_json_bytes_checked()
        .expect("6-channel cultivation_detail must fit MAX_PAYLOAD_BYTES");
    let back: ServerDataV1 = serde_json::from_slice(&bytes).expect("roundtrip");
    match back.payload {
        ServerDataPayloadV1::CultivationDetail {
            channel_ids: back_channel_ids,
            opened,
            target_meridian,
            ..
        } => {
            assert_eq!(
                back_channel_ids.len(),
                6,
                "non-humanoid channel array length must not be forced to 20"
            );
            assert_eq!(back_channel_ids, channel_ids);
            assert_eq!(opened.len(), 6);
            assert_eq!(target_meridian, Some("tail_fin_channel".to_string()));
        }
        other => panic!("expected CultivationDetail, got {other:?}"),
    }
}

/// plan-race-system-v1 P1c — wire 直改新形状不留兼容层：`target_meridian` 旧形态
/// 是数组下标（`u8`），新形态必须是 channel id 字符串；旧数字形状必须被拒绝，
/// 不允许静默兼容解析成某个 channel。
#[test]
fn cultivation_detail_rejects_legacy_numeric_target_meridian() {
    let legacy_json = r#"{
        "v": 1,
        "type": "cultivation_detail",
        "realm": "Induce",
        "opened": [true],
        "flow_rate": [1.5],
        "flow_capacity": [10.25],
        "integrity": [0.87],
        "contamination_total": 0.0,
        "target_meridian": 4
    }"#;
    let result: Result<ServerDataV1, _> = serde_json::from_str(legacy_json);
    assert!(
        result.is_err(),
        "legacy numeric target_meridian (index-based) must be rejected after wire \
         open-up to channel id string, got {result:?}"
    );
}

/// plan-remains-suite P0 — remains_sync 双端 sample 对拍：字段值必须与
/// agent/packages/schema/samples/server-data.remains-sync.sample.json 完全一致，
/// 改 schema 必须连同 sample 一起改。
#[test]
fn remains_sync_sample_pins_wire_shape() {
    let json = include_str!(
        "../../../../agent/packages/schema/samples/server-data.remains-sync.sample.json"
    );
    let payload: ServerDataV1 =
        serde_json::from_str(json).expect("remains-sync sample should deserialize");
    match payload.payload {
        ServerDataPayloadV1::RemainsSync(remains) => {
            assert_eq!(remains.len(), 1, "sample 固定 1 条 entry");
            let entry = &remains[0];
            assert_eq!(entry.remains_id, "3fa85f64-5717-4562-b3fc-2c963f66afa6");
            assert_eq!(entry.world_pos, [8.5, 66.0, 8.5]);
            assert_eq!(entry.dimension, "minecraft:overworld");
            assert_eq!(entry.display_name, "遗骸");
            assert_eq!(entry.item_count, 3);
            assert_eq!(entry.bone_coins, 12);
        }
        other => panic!("expected RemainsSync, got {other:?}"),
    }
}

#[test]
fn remains_sync_rejects_entry_unknown_field() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "remains_sync",
        "remains": [{
            "remains_id": "x",
            "world_pos": [0.0, 64.0, 0.0],
            "dimension": "minecraft:overworld",
            "display_name": "遗骸",
            "item_count": 1,
            "bone_coins": 0,
            "unexpected": true
        }]
    });

    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "RemainsEntryV1 额外字段应被 deny_unknown_fields 拒绝"
    );
}

#[test]
fn remains_sync_rejects_entry_missing_remains_id() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "remains_sync",
        "remains": [{
            "world_pos": [0.0, 64.0, 0.0],
            "dimension": "minecraft:overworld",
            "display_name": "遗骸",
            "item_count": 1,
            "bone_coins": 0
        }]
    });

    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "RemainsEntryV1 缺 remains_id 应反序列化失败"
    );
}

/// plan-race-system-v1 P2a — body_plan_layout 双端 sample 对拍：字段值必须与
/// agent/packages/schema/samples/server-data.body-plan-layout.sample.json 完全一致，
/// 改 schema 必须连同 sample 一起改。
#[test]
fn body_plan_layout_sample_pins_wire_shape() {
    let json = include_str!(
        "../../../../agent/packages/schema/samples/server-data.body-plan-layout.sample.json"
    );
    let payload: ServerDataV1 =
        serde_json::from_str(json).expect("body-plan-layout sample should deserialize");
    match payload.payload {
        ServerDataPayloadV1::BodyPlanLayout(layout) => {
            assert_eq!(layout.body_plan_id, "humanoid");
            assert_eq!(
                layout.silhouette.len(),
                2,
                "sample 固定 head+chest 两段剪影"
            );
            assert_eq!(layout.silhouette[0].part_id, "head");
            assert_eq!(layout.silhouette[0].polygon.len(), 4);
            assert_eq!(
                layout.silhouette[0].polygon[0],
                BodyPlanPoint2V1 {
                    x: 0.434524,
                    y: 0.025424
                }
            );
            assert_eq!(layout.anchors.len(), 2);
            assert_eq!(layout.anchors[0].part_id, "head");
            assert_eq!(
                layout.anchors[0].point,
                BodyPlanPoint2V1 {
                    x: 0.5,
                    y: 0.042373
                }
            );
            assert_eq!(layout.meridian_paths.len(), 1);
            assert_eq!(layout.meridian_paths[0].channel_id, "ren");
            assert_eq!(layout.meridian_paths[0].points.len(), 2);
            assert_eq!(layout.part_display_map.len(), 2);
            assert_eq!(layout.part_display_map[0].server_part_id, "head");
            assert_eq!(layout.part_display_map[0].display_segment_id, "head");
        }
        other => panic!("expected BodyPlanLayout, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────
// plan-race-system-v1 P3a —— RaceGateWireV1 双端 sample 对拍 + fail-closed 解码。
// 三变体样本文件与 agent/packages/schema/samples/race-gate.*.sample.json 完全一致，
// 改 schema 必须连同 sample 一起改。
// ─────────────────────────────────────────────────────────────────

#[test]
fn race_gate_any_sample_pins_wire_shape_and_round_trips_to_owned() {
    let json = include_str!("../../../../agent/packages/schema/samples/race-gate.any.sample.json");
    let wire: RaceGateWireV1 = serde_json::from_str(json).expect("any sample should deserialize");
    assert_eq!(wire.kind, "any");
    assert!(wire.species.is_empty());
    assert_eq!(
        wire.try_into_owned().expect("any must decode"),
        bong_server::body_plan::RaceGateOwned::Any
    );
}

#[test]
fn race_gate_humanoid_sample_pins_wire_shape_and_round_trips_to_owned() {
    let json =
        include_str!("../../../../agent/packages/schema/samples/race-gate.humanoid.sample.json");
    let wire: RaceGateWireV1 =
        serde_json::from_str(json).expect("humanoid sample should deserialize");
    assert_eq!(wire.kind, "humanoid");
    assert!(wire.species.is_empty());
    assert_eq!(
        wire.try_into_owned().expect("humanoid must decode"),
        bong_server::body_plan::RaceGateOwned::Humanoid
    );
}

#[test]
fn race_gate_species_sample_pins_wire_shape_and_round_trips_to_owned() {
    let json =
        include_str!("../../../../agent/packages/schema/samples/race-gate.species.sample.json");
    let wire: RaceGateWireV1 =
        serde_json::from_str(json).expect("species sample should deserialize");
    assert_eq!(wire.kind, "species");
    assert_eq!(wire.species, vec!["whale".to_string()]);
    assert_eq!(
        wire.try_into_owned().expect("species must decode"),
        bong_server::body_plan::RaceGateOwned::Species {
            species: vec![bong_server::body_plan::RaceId::new("whale")]
        }
    );
}

#[test]
fn race_gate_wire_from_owned_round_trips_every_variant() {
    use bong_server::body_plan::{RaceGateOwned, RaceId};

    let cases = [
        (RaceGateOwned::Any, "any", Vec::<String>::new()),
        (RaceGateOwned::Humanoid, "humanoid", Vec::new()),
        (
            RaceGateOwned::Species {
                species: vec![RaceId::new("whale")],
            },
            "species",
            vec!["whale".to_string()],
        ),
    ];
    for (owned, expected_kind, expected_species) in cases {
        let wire = RaceGateWireV1::from_owned(&owned);
        assert_eq!(wire.kind, expected_kind);
        assert_eq!(wire.species, expected_species);
        assert_eq!(
            wire.try_into_owned().expect("round trip must decode"),
            owned
        );
    }
}

#[test]
fn race_gate_wire_species_empty_and_duplicate_preserved() {
    use bong_server::body_plan::RaceId;

    let empty = RaceGateWireV1 {
        kind: "species".to_string(),
        species: Vec::new(),
    };
    assert_eq!(
        empty.try_into_owned().expect("empty species list is valid"),
        bong_server::body_plan::RaceGateOwned::Species { species: vec![] }
    );

    let duplicate = RaceGateWireV1 {
        kind: "species".to_string(),
        species: vec!["whale".to_string(), "whale".to_string()],
    };
    match duplicate
        .try_into_owned()
        .expect("duplicate species entries are structurally valid")
    {
        bong_server::body_plan::RaceGateOwned::Species { species } => {
            assert_eq!(
                species,
                vec![RaceId::new("whale"), RaceId::new("whale")],
                "重复条目原样保留，不做去重"
            );
        }
        other => panic!("expected Species, got {other:?}"),
    }
}

#[test]
fn race_gate_wire_unknown_kind_decode_fails_closed() {
    let wire = RaceGateWireV1 {
        kind: "bogus".to_string(),
        species: Vec::new(),
    };
    let err = wire
        .try_into_owned()
        .expect_err("unknown kind must fail closed, not silently default to Any");
    assert_eq!(err.0, "bogus");
}

#[test]
fn race_gate_wire_unknown_kind_json_deserialize_succeeds_but_conversion_fails_closed() {
    // RaceGateWireV1 本身是扁平结构（deny_unknown_fields 只管字段名，不管 kind 取值语义），
    // 未知 kind 字符串本身能反序列化成 RaceGateWireV1；fail-closed 拒绝发生在
    // try_into_owned() 转换语义层——两阶段分别验证，防止把"反序列化失败"和
    // "语义拒绝"混为一谈。
    let wire: RaceGateWireV1 =
        serde_json::from_str(r#"{"kind":"bogus","species":[]}"#).expect("deserialize");
    assert!(wire.try_into_owned().is_err());
}

/// wire 往返：BodyPlanLayout 序列化 → 反序列化必须无损（含空 anchors /
/// meridian_paths 边界）。
#[test]
fn body_plan_layout_roundtrips_including_empty_optional_sections() {
    let payload = ServerDataV1::new(ServerDataPayloadV1::BodyPlanLayout(BodyPlanLayoutV1 {
        body_plan_id: "whale".to_string(),
        silhouette: vec![BodyPlanSilhouettePartV1 {
            part_id: "tail_fin".to_string(),
            polygon: vec![
                BodyPlanPoint2V1 { x: 0.1, y: 0.9 },
                BodyPlanPoint2V1 { x: 0.5, y: 0.8 },
                BodyPlanPoint2V1 { x: 0.9, y: 0.9 },
            ],
        }],
        anchors: Vec::new(),
        meridian_paths: Vec::new(),
        part_display_map: Vec::new(),
        hud_anchors: Vec::new(),
    }));
    let bytes = payload
        .to_json_bytes_checked()
        .expect("body_plan_layout must serialize");
    let back: ServerDataV1 =
        serde_json::from_slice(&bytes).expect("body_plan_layout must deserialize back");
    match back.payload {
        ServerDataPayloadV1::BodyPlanLayout(layout) => {
            assert_eq!(layout.body_plan_id, "whale");
            assert_eq!(layout.silhouette.len(), 1);
            assert_eq!(layout.silhouette[0].part_id, "tail_fin");
            assert!(layout.anchors.is_empty());
            assert!(layout.meridian_paths.is_empty());
            assert!(layout.part_display_map.is_empty());
            assert!(
                layout.hud_anchors.is_empty(),
                "hud_anchors 是可选第二锚点组，非人形/未配置构型必须留空往返"
            );
        }
        other => panic!("expected BodyPlanLayout after roundtrip, got {other:?}"),
    }
}

/// plan-race-system-v1 P2 major 修复 —— `hud_anchors` 非空往返必须逐值保留，
/// 且缺省 JSON（旧数据 / 未配置该字段的 plan）反序列化必须默认落空 `Vec`（不是
/// 反序列化失败），两条转换分支各有专属 pin。
#[test]
fn body_plan_layout_hud_anchors_roundtrip_and_missing_field_defaults_to_empty() {
    let payload = ServerDataV1::new(ServerDataPayloadV1::BodyPlanLayout(BodyPlanLayoutV1 {
        body_plan_id: "humanoid".to_string(),
        silhouette: vec![BodyPlanSilhouettePartV1 {
            part_id: "head".to_string(),
            polygon: vec![
                BodyPlanPoint2V1 { x: 0.4, y: 0.0 },
                BodyPlanPoint2V1 { x: 0.6, y: 0.0 },
                BodyPlanPoint2V1 { x: 0.5, y: 0.1 },
            ],
        }],
        anchors: Vec::new(),
        meridian_paths: Vec::new(),
        part_display_map: Vec::new(),
        hud_anchors: vec![BodyPlanPartAnchorV1 {
            part_id: "head".to_string(),
            point: BodyPlanPoint2V1 { x: 0.5, y: 0.04 },
        }],
    }));
    let bytes = payload
        .to_json_bytes_checked()
        .expect("body_plan_layout with hud_anchors must serialize");
    let back: ServerDataV1 = serde_json::from_slice(&bytes)
        .expect("body_plan_layout with hud_anchors must deserialize back");
    match back.payload {
        ServerDataPayloadV1::BodyPlanLayout(layout) => {
            assert_eq!(layout.hud_anchors.len(), 1);
            assert_eq!(layout.hud_anchors[0].part_id, "head");
            assert_eq!(layout.hud_anchors[0].point.x, 0.5);
            assert_eq!(layout.hud_anchors[0].point.y, 0.04);
        }
        other => panic!("expected BodyPlanLayout after roundtrip, got {other:?}"),
    }

    // 缺省字段（旧数据 / 未来非人 plan 不配置）反序列化必须默认空 Vec，不是 error。
    let legacy_json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "body_plan_layout",
        "body_plan_id": "whale",
        "silhouette": [{
            "part_id": "tail_fin",
            "polygon": [{"x": 0.1, "y": 0.9}, {"x": 0.5, "y": 0.8}, {"x": 0.9, "y": 0.9}],
        }],
        "anchors": [],
        "meridian_paths": [],
        "part_display_map": [],
    });
    let decoded: ServerDataV1 = serde_json::from_value(legacy_json)
        .expect("body_plan_layout missing hud_anchors field must default to empty, not fail");
    match decoded.payload {
        ServerDataPayloadV1::BodyPlanLayout(layout) => {
            assert!(
                layout.hud_anchors.is_empty(),
                "missing hud_anchors field must default to an empty Vec"
            );
        }
        other => panic!("expected BodyPlanLayout, got {other:?}"),
    }
}

#[test]
fn body_plan_layout_rejects_unknown_field_in_point() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "body_plan_layout",
        "body_plan_id": "humanoid",
        "silhouette": [{
            "part_id": "chest",
            "polygon": [
                {"x": 0.3, "y": 0.1, "z": 0.0},
                {"x": 0.7, "y": 0.1},
                {"x": 0.7, "y": 0.3}
            ]
        }],
        "anchors": [],
        "meridian_paths": [],
        "part_display_map": []
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "BodyPlanPoint2V1 额外字段（z）应被 deny_unknown_fields 拒绝——布局是 2D 归一化坐标"
    );
}

#[test]
fn body_plan_layout_rejects_missing_body_plan_id() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "body_plan_layout",
        "silhouette": [],
        "anchors": [],
        "meridian_paths": [],
        "part_display_map": []
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "BodyPlanLayoutV1 缺 body_plan_id 应反序列化失败——它是 client 寻址缓存的主键"
    );
}

#[test]
fn deserialize_server_data_samples() {
    let samples = [
        include_str!("../../../../agent/packages/schema/samples/server-data.welcome.sample.json"),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.heartbeat.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.narration.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.zone-info.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.event-alert.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.player-state.sample.json"
        ),
        include_str!("../../../../agent/packages/schema/samples/server-data.ui-open.sample.json"),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.inventory-snapshot.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.inventory-event.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.dropped-loot-sync.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.remains-sync.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.body-plan-layout.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.botany-harvest-progress.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.gathering-session.active.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.gathering-session.completed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.gathering-session.interrupted.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.botany-skill.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-furnace.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-session.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-outcome-forecast.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-outcome-resolved.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-recipe-book.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.alchemy-contamination.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.death-screen.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-xp-gain.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-lv-up.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-cap-changed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-scroll-used.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-snapshot.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skillbar-config.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.techniques-snapshot.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.skill-config-snapshot.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.rift-portal-state.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.rift-portal-removed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.extract-started.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.extract-progress.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.extract-completed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.extract-aborted.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.extract-failed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.tsy-collapse-started-ipc.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.forge-station.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.forge-session.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.forge-outcome-perfect.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.forge-outcome-flawed.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.forge-blueprint-book.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.tribulation-broadcast.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.tribulation-state.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.ascension-quota.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.heart-demon-offer.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.burst-meridian-event.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.social-anonymity.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.social-exposure.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.social-pact.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.social-feud.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.social-renown-delta.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.sparring-invite.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.trade-offer.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.realm-vision-params.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.spiritual-sense-targets.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.movement-state.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.spirit-treasure-state.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.spirit-treasure-dialogue.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.agent-ui-request.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.agent-ui-close.sample.json"
        ),
        // plan-coffin-tiers-v1 P0 charge #7：四档 + no-grade serde pin samples
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.coffin-state-mundane.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.coffin-state-jade.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.coffin-state-stone.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.coffin-state-bronze.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.coffin-state-no-grade.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.scroll-open.sample.json"
        ),
    ];

    for json in samples {
        let payload: ServerDataV1 =
            serde_json::from_str(json).expect("sample should deserialize into ServerDataV1");

        let reserialized = serde_json::to_string(&payload)
            .expect("deserialized ServerDataV1 should serialize back to JSON");
        let roundtrip: ServerDataV1 = serde_json::from_str(&reserialized)
            .expect("serialized ServerDataV1 should deserialize again");

        let payload_value =
            serde_json::to_value(&payload).expect("payload should convert to JSON value");
        let roundtrip_value =
            serde_json::to_value(&roundtrip).expect("roundtrip should convert to JSON value");

        assert_eq!(
            payload_value, roundtrip_value,
            "roundtrip must preserve typed payload content"
        );
    }
}

#[test]
fn player_state_requires_spirit_qi_max() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "player_state",
        "realm": "Solidify",
        "spirit_qi": 78.0,
        "karma": 0.2,
        "composite_power": 0.35,
        "breakdown": {
            "combat": 0.2,
            "wealth": 0.4,
            "social": 0.65,
            "karma": 0.2,
            "territory": 0.1
        },
        "zone": "blood_valley"
    });

    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "player_state 缺 spirit_qi_max 必须反序列化失败；否则 HUD 真元条会退回 100 分母"
    );
}

#[test]
fn player_state_rejects_zero_spirit_qi_max() {
    let json = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "player_state",
        "realm": "Solidify",
        "spirit_qi": 78.0,
        "spirit_qi_max": 0.0,
        "karma": 0.2,
        "composite_power": 0.35,
        "breakdown": {
            "combat": 0.2,
            "wealth": 0.4,
            "social": 0.65,
            "karma": 0.2,
            "territory": 0.1
        },
        "zone": "blood_valley"
    });

    assert!(
        serde_json::from_value::<ServerDataV1>(json).is_err(),
        "player_state spirit_qi_max=0 必须拒绝；proto3 缺 scalar tag 会退成 0，不能进 HUD fallback"
    );
}

// ─── plan-coffin-tiers-v1 P0 charge #7：CoffinGradeV1/CoffinStateV1 serde pin ────

#[test]
fn coffin_grade_v1_all_variants_serde_roundtrip() {
    // 每个 enum 变体至少一条专属正例
    let cases: &[(&str, CoffinGradeV1)] = &[
        ("\"mundane\"", CoffinGradeV1::Mundane),
        ("\"jade\"", CoffinGradeV1::Jade),
        ("\"stone\"", CoffinGradeV1::Stone),
        ("\"bronze\"", CoffinGradeV1::Bronze),
    ];
    for (json_str, expected) in cases {
        let parsed: CoffinGradeV1 = serde_json::from_str(json_str)
            .unwrap_or_else(|e| panic!("{json_str} should parse as CoffinGradeV1: {e}"));
        assert_eq!(
            parsed, *expected,
            "CoffinGradeV1 from {json_str} should equal {expected:?}"
        );
        let reserialized =
            serde_json::to_string(&parsed).expect("CoffinGradeV1 should serialize back");
        assert_eq!(
            reserialized, *json_str,
            "CoffinGradeV1::{expected:?} roundtrip should produce {json_str}"
        );
    }
}

#[test]
fn coffin_grade_v1_rejects_unknown_variant() {
    // 反例：未知 variant 必须失败
    let result = serde_json::from_str::<CoffinGradeV1>("\"diamond\"");
    assert!(
        result.is_err(),
        "unknown grade 'diamond' should fail to deserialize"
    );
}

#[test]
fn coffin_state_v1_all_grades_serde_pin() {
    // 四档 + None（出棺）serde 正例
    let cases: &[(&str, Option<CoffinGradeV1>, bool, f64)] = &[
        ("mundane", Some(CoffinGradeV1::Mundane), true, 0.9),
        ("jade", Some(CoffinGradeV1::Jade), true, 0.7),
        ("stone", Some(CoffinGradeV1::Stone), true, 0.5),
        ("bronze", Some(CoffinGradeV1::Bronze), true, 0.3),
    ];
    for (grade_str, expected_grade, in_coffin, multiplier) in cases {
        let json = serde_json::json!({
            "in_coffin": in_coffin,
            "lifespan_rate_multiplier": multiplier,
            "coffin_grade": grade_str
        });
        let state: CoffinStateV1 = serde_json::from_value(json.clone())
            .unwrap_or_else(|e| panic!("grade={grade_str} json={json} should parse: {e}"));
        assert_eq!(
            state.coffin_grade, *expected_grade,
            "grade={grade_str}: parsed coffin_grade should equal {expected_grade:?}"
        );
        assert_eq!(state.in_coffin, *in_coffin);
        assert!((state.lifespan_rate_multiplier - multiplier).abs() < 1e-9);
    }
}

#[test]
fn coffin_state_v1_none_grade_serde_pin() {
    // None（出棺）：coffin_grade 字段缺失 → None（向后兼容）
    let json = serde_json::json!({
        "in_coffin": false,
        "lifespan_rate_multiplier": 1.0
    });
    let state: CoffinStateV1 =
        serde_json::from_value(json).expect("no-grade CoffinStateV1 should parse");
    assert_eq!(
        state.coffin_grade, None,
        "missing coffin_grade should parse as None (向后兼容旧 payload)"
    );
    // 序列化时 skip_serializing_if = None → 字段不出现在 JSON
    let reserialized =
        serde_json::to_value(state).expect("CoffinStateV1 should serialize to JSON value");
    assert!(
        reserialized.get("coffin_grade").is_none(),
        "coffin_grade=None should be omitted during serialization, got {reserialized}"
    );
}

#[test]
fn coffin_state_v1_deny_unknown_fields_standalone() {
    // standalone 反序列化：deny_unknown_fields 拒绝多余字段
    let json = serde_json::json!({
        "in_coffin": true,
        "lifespan_rate_multiplier": 0.9,
        "unknown_field": "oops"
    });
    let result = serde_json::from_value::<CoffinStateV1>(json);
    assert!(
        result.is_err(),
        "CoffinStateV1 standalone deny_unknown_fields should reject extra fields"
    );
}

#[test]
fn gathering_session_rejects_invalid_enum_values() {
    let invalid_quality =
        include_str!("../../../../agent/packages/schema/samples/server-data.gathering-session.invalid-quality.sample.json");
    assert!(
        serde_json::from_str::<ServerDataV1>(invalid_quality).is_err(),
        "invalid gathering_session quality_hint sample should fail to deserialize"
    );

    let invalid_target = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "gathering_session",
        "session_id": "gathering:bad-target",
        "progress_ticks": 10,
        "total_ticks": 40,
        "target_name": "测试采集物",
        "target_type": "invalid_type",
        "quality_hint": "normal",
        "interrupted": false,
        "completed": false
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(invalid_target).is_err(),
        "invalid gathering_session target_type should fail to deserialize"
    );
}

#[test]
fn deserialize_zone_info_defaults_missing_status() {
    let value = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "zone_info",
        "zone": "blood_valley",
        "spirit_qi": -0.42,
        "danger_level": 3,
        "active_events": ["beast_tide"]
    });

    let payload: ServerDataV1 = serde_json::from_value(value).expect("deserialize zone_info");
    match payload.payload {
        ServerDataPayloadV1::ZoneInfo { status, .. } => {
            assert_eq!(status, ZoneStatusV1::Normal);
        }
        other => panic!("expected ZoneInfo, got {other:?}"),
    }
}

#[test]
fn serialize_zone_info_includes_status() {
    let payload = ServerDataV1::new(ServerDataPayloadV1::ZoneInfo {
        zone: "blood_valley".to_string(),
        spirit_qi: -0.42,
        danger_level: 3,
        status: ZoneStatusV1::Collapsed,
        active_events: Some(vec!["realm_collapse".to_string()]),
        perception_text: Some("灵气几近断绝，此地有不祥预感".to_string()),
    });

    let value: serde_json::Value = serde_json::from_slice(
        &payload
            .to_json_bytes_checked()
            .expect("zone_info should serialize"),
    )
    .expect("zone_info JSON should decode");

    assert_eq!(value["status"], "collapsed");
    assert_eq!(value["perception_text"], "灵气几近断绝，此地有不祥预感");
}

#[test]
fn ascension_quota_defaults_new_world_qi_fields_for_legacy_payloads() {
    let payload: AscensionQuotaV1 =
        serde_json::from_str(r#"{"occupied_slots":1,"quota_limit":3,"available_slots":2}"#)
            .expect("legacy ascension quota payload should deserialize");

    assert_eq!(payload, AscensionQuotaV1::new(1, 3));
}

#[test]
fn rejects_unknown_server_data_version() {
    let json = r#"{"v":99,"type":"welcome","message":"hello"}"#;
    let error = serde_json::from_str::<ServerDataV1>(json)
        .expect_err("unknown server_data version should be rejected");

    assert!(
        error.to_string().contains("ServerDataV1.v must be"),
        "unexpected server_data version error: {error}"
    );
}

#[test]
fn container_kind_v1_surface_stash_wire() {
    use bong_server::network::tsy_container_search_emit::container_kind_wire;
    use bong_server::world::tsy_container::ContainerKind;

    assert_eq!(
        container_kind_wire(ContainerKind::SurfaceStash),
        ContainerKindV1::SurfaceStash,
        "ContainerKind::SurfaceStash should map to ContainerKindV1::SurfaceStash"
    );
}

#[test]
fn container_kind_v1_serde_pin_with_surface_stash() {
    let json = serde_json::to_string(&ContainerKindV1::SurfaceStash)
        .expect("ContainerKindV1::SurfaceStash should serialize");
    assert_eq!(
        json, "\"surface_stash\"",
        "ContainerKindV1::SurfaceStash serde should produce \"surface_stash\", got {json}"
    );
    let round: ContainerKindV1 = serde_json::from_str(&json).expect("should deserialize back");
    assert_eq!(round, ContainerKindV1::SurfaceStash);
}

#[test]
fn technique_proficiency_update_rejects_missing_gain() {
    let missing_gain = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "technique_proficiency_update",
        "update": {
            "technique_id": "sword.cleave",
            "proficiency": 0.42
        }
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(missing_gain).is_err(),
        "technique_proficiency_update missing 'gain' should fail deserialization"
    );
}

#[test]
fn technique_proficiency_update_rejects_unknown_field() {
    let unknown_field = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "technique_proficiency_update",
        "update": {
            "technique_id": "sword.cleave",
            "proficiency": 0.42,
            "gain": 0.008,
            "unexpected": true
        }
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(unknown_field).is_err(),
        "technique_proficiency_update with unknown field should fail due to deny_unknown_fields"
    );
}

#[test]
fn pill_buff_status_v1_serde_pin() {
    let original = PillBuffStatusV1 {
        buff_id: "huo_xue_dan".to_string(),
        remaining_ticks: 3000,
        effect_multiplier: 1.0,
    };
    let json = serde_json::to_string(&original).expect("PillBuffStatusV1 should serialize");
    let back: PillBuffStatusV1 =
        serde_json::from_str(&json).expect("PillBuffStatusV1 should deserialize");
    assert_eq!(
        original, back,
        "PillBuffStatusV1 roundtrip must be lossless"
    );

    let envelope = ServerDataV1::new(ServerDataPayloadV1::PillBuffStatus(original.clone()));
    let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
    let round: ServerDataV1 = serde_json::from_slice(&bytes).expect("envelope should roundtrip");
    match round.payload {
        ServerDataPayloadV1::PillBuffStatus(status) => {
            assert_eq!(
                status, original,
                "envelope roundtrip must preserve PillBuffStatusV1"
            );
        }
        other => panic!("expected PillBuffStatus, got {other:?}"),
    }
}

#[test]
fn pill_buff_status_v1_rejects_unknown_field() {
    let unknown_field = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "pill_buff_status",
        "buff_id": "tie_bi_san",
        "remaining_ticks": 600,
        "effect_multiplier": 1.2,
        "unexpected": true
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(unknown_field).is_err(),
        "PillBuffStatusV1 with unknown field should fail due to deny_unknown_fields"
    );
}

#[test]
fn pill_buff_status_v1_rejects_missing_buff_id() {
    let missing = serde_json::json!({
        "v": SERVER_DATA_VERSION,
        "type": "pill_buff_status",
        "remaining_ticks": 600,
        "effect_multiplier": 1.2
    });
    assert!(
        serde_json::from_value::<ServerDataV1>(missing).is_err(),
        "PillBuffStatusV1 missing 'buff_id' should fail deserialization"
    );
}

#[test]
fn pill_buff_status_v1_zero_ticks_roundtrips() {
    let zero = PillBuffStatusV1 {
        buff_id: "expired_buff".to_string(),
        remaining_ticks: 0,
        effect_multiplier: 0.0,
    };
    let json = serde_json::to_string(&zero).expect("zero-tick PillBuffStatusV1 should serialize");
    let back: PillBuffStatusV1 = serde_json::from_str(&json).expect("zero-tick should deserialize");
    assert_eq!(zero, back);
}

// ─── plan-supply-coffin-loot-ui P1：外部容器 S2C tests ──────────

fn sample_placed_item() -> bong_server::schema::inventory::PlacedInventoryItemV1 {
    bong_server::schema::inventory::PlacedInventoryItemV1 {
        container_id: "ext_42".to_string(),
        row: 0,
        col: 1,
        item: bong_server::schema::inventory::InventoryItemViewV1 {
            instance_id: 100,
            item_id: "iron_sword".to_string(),
            display_name: "铁剑".to_string(),
            grid_width: 1,
            grid_height: 2,
            weight: 2.5,
            rarity: bong_server::schema::inventory::ItemRarityV1::Common,
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
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
            alchemy: None,
            lingering_owner_qi: None,
        },
    }
}

#[test]
fn loot_container_open_serde_roundtrip() {
    let original = LootContainerOpenV1 {
        session_id: 42,
        source_kind: LootContainerSourceKindV1::SupplyCoffin {
            grade: "common".to_string(),
        },
        rows: 3,
        cols: 4,
        placed_items: vec![sample_placed_item()],
        timeout_wall_secs: 1716872400,
    };
    let json = serde_json::to_string(&original).expect("LootContainerOpenV1 should serialize");
    let back: LootContainerOpenV1 =
        serde_json::from_str(&json).expect("LootContainerOpenV1 should deserialize");
    assert_eq!(
        original, back,
        "LootContainerOpenV1 roundtrip must be lossless"
    );
}

#[test]
fn loot_container_open_envelope_roundtrip() {
    let payload = ServerDataPayloadV1::LootContainerOpen(LootContainerOpenV1 {
        session_id: 7,
        source_kind: LootContainerSourceKindV1::SupplyCoffin {
            grade: "rare".to_string(),
        },
        rows: 4,
        cols: 5,
        placed_items: vec![],
        timeout_wall_secs: 1716872500,
    });
    let envelope = ServerDataV1::new(payload.clone());
    let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
    let round: ServerDataV1 = serde_json::from_slice(&bytes).expect("envelope should roundtrip");
    assert_eq!(
        round.payload.payload_type(),
        ServerDataType::LootContainerOpen,
        "deserialized type must be LootContainerOpen"
    );
}

#[test]
fn loot_container_open_empty_items_roundtrips() {
    let open = LootContainerOpenV1 {
        session_id: 0,
        source_kind: LootContainerSourceKindV1::SupplyCoffin {
            grade: "precious".to_string(),
        },
        rows: 5,
        cols: 6,
        placed_items: vec![],
        timeout_wall_secs: 0,
    };
    let json = serde_json::to_string(&open)
        .expect("LootContainerOpenV1 with empty items should serialize");
    let back: LootContainerOpenV1 = serde_json::from_str(&json)
        .expect("LootContainerOpenV1 with empty items should deserialize");
    assert!(
        back.placed_items.is_empty(),
        "empty placed_items must survive roundtrip"
    );
}

#[test]
fn loot_container_update_serde_roundtrip() {
    let original = LootContainerUpdateV1 {
        session_id: 42,
        placed_items: vec![sample_placed_item()],
    };
    let json = serde_json::to_string(&original).expect("LootContainerUpdateV1 should serialize");
    let back: LootContainerUpdateV1 =
        serde_json::from_str(&json).expect("LootContainerUpdateV1 should deserialize");
    assert_eq!(
        original, back,
        "LootContainerUpdateV1 roundtrip must be lossless"
    );
}

#[test]
fn loot_container_close_all_reasons_roundtrip() {
    let reasons = [
        LootContainerCloseReasonV1::Timeout,
        LootContainerCloseReasonV1::Distance,
        LootContainerCloseReasonV1::PlayerClosed,
        LootContainerCloseReasonV1::CoffinDestroyed,
        LootContainerCloseReasonV1::ContainerDestroyed,
    ];
    for reason in reasons {
        let close = LootContainerCloseV1 {
            session_id: 99,
            reason: reason.clone(),
        };
        let json = serde_json::to_string(&close).expect("LootContainerCloseV1 should serialize");
        let back: LootContainerCloseV1 =
            serde_json::from_str(&json).expect("LootContainerCloseV1 should deserialize");
        assert_eq!(
            close, back,
            "LootContainerCloseV1 roundtrip must be lossless for reason {reason:?}"
        );
    }
}

#[test]
fn loot_container_close_envelope_roundtrip() {
    let payload = ServerDataPayloadV1::LootContainerClose(LootContainerCloseV1 {
        session_id: 5,
        reason: LootContainerCloseReasonV1::Timeout,
    });
    let envelope = ServerDataV1::new(payload);
    let bytes = serde_json::to_vec(&envelope).expect("envelope should serialize");
    let round: ServerDataV1 = serde_json::from_slice(&bytes).expect("envelope should roundtrip");
    assert_eq!(
        round.payload.payload_type(),
        ServerDataType::LootContainerClose,
        "deserialized type must be LootContainerClose"
    );
}

#[test]
fn loot_container_source_kind_supply_coffin_wire_format() {
    let kind = LootContainerSourceKindV1::SupplyCoffin {
        grade: "common".to_string(),
    };
    let json = serde_json::to_string(&kind)
        .expect("LootContainerSourceKindV1::SupplyCoffin should serialize");
    assert!(
        json.contains("\"supply_coffin\""),
        "source_kind wire should use snake_case tag, got: {json}"
    );
    assert!(
        json.contains("\"grade\":\"common\""),
        "source_kind wire should contain grade field, got: {json}"
    );
}

#[test]
fn loot_container_source_kind_storage_crate_wire_format() {
    let kind = LootContainerSourceKindV1::StorageCrate { is_herb: true };
    let json = serde_json::to_string(&kind)
        .expect("LootContainerSourceKindV1::StorageCrate should serialize");
    assert!(
        json.contains("\"storage_crate\""),
        "source_kind wire should use snake_case tag, got: {json}"
    );
    assert!(
        json.contains("\"is_herb\":true"),
        "source_kind wire should contain is_herb field, got: {json}"
    );
    let back: LootContainerSourceKindV1 =
        serde_json::from_str(&json).expect("StorageCrate source_kind should deserialize");
    assert_eq!(
        kind, back,
        "StorageCrate source_kind must roundtrip without losing is_herb"
    );
}

#[test]
fn loot_container_source_kind_dead_drop_wire_format() {
    let kind = LootContainerSourceKindV1::DeadDrop;
    let json =
        serde_json::to_string(&kind).expect("LootContainerSourceKindV1::DeadDrop should serialize");
    assert_eq!(
        json, "\"dead_drop\"",
        "unit source_kind wire should be the snake_case tag"
    );
    let back: LootContainerSourceKindV1 =
        serde_json::from_str(&json).expect("DeadDrop source_kind should deserialize");
    assert_eq!(kind, back, "DeadDrop source_kind must roundtrip");
}

#[test]
fn loot_container_close_reason_wire_values() {
    let cases = [
        (LootContainerCloseReasonV1::Timeout, "\"timeout\""),
        (LootContainerCloseReasonV1::Distance, "\"distance\""),
        (
            LootContainerCloseReasonV1::PlayerClosed,
            "\"player_closed\"",
        ),
        (
            LootContainerCloseReasonV1::CoffinDestroyed,
            "\"coffin_destroyed\"",
        ),
        (
            LootContainerCloseReasonV1::ContainerDestroyed,
            "\"container_destroyed\"",
        ),
    ];
    for (reason, expected) in cases {
        let json = serde_json::to_string(&reason)
            .expect("LootContainerCloseReasonV1 variant should serialize");
        assert_eq!(
            json, expected,
            "LootContainerCloseReasonV1::{reason:?} wire value mismatch"
        );
    }
}

#[test]
fn payload_type_label_matches_for_loot_container_types() {
    assert_eq!(
        payload_type_label(ServerDataType::LootContainerOpen),
        "loot_container_open"
    );
    assert_eq!(
        payload_type_label(ServerDataType::LootContainerUpdate),
        "loot_container_update"
    );
    assert_eq!(
        payload_type_label(ServerDataType::LootContainerClose),
        "loot_container_close"
    );
}

#[test]
fn loot_container_open_rejects_missing_session_id() {
    let json = r#"{"source_kind":{"kind":"supply_coffin","grade":"common"},"rows":3,"cols":4,"placed_items":[],"timeout_wall_secs":0}"#;
    assert!(
        serde_json::from_str::<LootContainerOpenV1>(json).is_err(),
        "LootContainerOpenV1 missing session_id should fail deserialization"
    );
}

#[test]
fn loot_container_close_rejects_missing_reason() {
    let json = r#"{"session_id":1}"#;
    assert!(
        serde_json::from_str::<LootContainerCloseV1>(json).is_err(),
        "LootContainerCloseV1 missing reason should fail deserialization"
    );
}

#[test]
fn loot_container_close_rejects_unknown_reason() {
    let json = r#"{"session_id":1,"reason":"alien_abduction"}"#;
    assert!(
        serde_json::from_str::<LootContainerCloseV1>(json).is_err(),
        "LootContainerCloseV1 unknown reason should fail deserialization"
    );
}

#[test]
fn loot_container_open_rejects_missing_placed_items() {
    let json = r#"{"session_id":1,"source_kind":{"kind":"supply_coffin","grade":"rare"},"rows":5,"cols":4,"timeout_wall_secs":100}"#;
    assert!(
        serde_json::from_str::<LootContainerOpenV1>(json).is_err(),
        "LootContainerOpenV1 missing placed_items should fail deserialization"
    );
}

#[test]
fn loot_container_update_rejects_missing_session_id() {
    let json = r#"{"placed_items":[]}"#;
    assert!(
        serde_json::from_str::<LootContainerUpdateV1>(json).is_err(),
        "LootContainerUpdateV1 missing session_id should fail deserialization"
    );
}

// ─── plan-offscreen-war-v1 P9：FactionWarState payload 测试 ─────────────

#[test]
fn faction_war_state_v1_roundtrips_with_outcome() {
    // 有 winner/loser 的 Settling 阶段 payload 完整无损 roundtrip。
    let payload = FactionWarStateV1 {
        war_id: 42,
        zone: "残灰谷".to_string(),
        region_descriptor: "残灰谷一带散修".to_string(),
        phase: "settling".to_string(),
        groups: vec![0, 1],
        enlist_count: 3,
        mercenary_count: 1,
        intercept_count: 0,
        spectate_count: 2,
        winner_group: Some(0),
        loser_group: Some(1),
    };
    let json = serde_json::to_string(&payload).expect("FactionWarStateV1 should serialize");
    let back: FactionWarStateV1 =
        serde_json::from_str(&json).expect("FactionWarStateV1 should deserialize");
    assert_eq!(
        payload, back,
        "FactionWarStateV1 roundtrip must be lossless"
    );
    // winner_group/loser_group Some 时 JSON 应包含这两个字段
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        v.get("winner_group").is_some(),
        "winner_group should be in JSON when Some"
    );
    assert!(
        v.get("loser_group").is_some(),
        "loser_group should be in JSON when Some"
    );
}

#[test]
fn faction_war_state_v1_roundtrips_without_outcome() {
    // 无 winner/loser 的 Skirmish 阶段：winner_group/loser_group 字段应被 skip_serializing。
    let payload = FactionWarStateV1 {
        war_id: 7,
        zone: "残灰谷".to_string(),
        region_descriptor: "残灰谷一带散修".to_string(),
        phase: "skirmish".to_string(),
        groups: vec![0, 1],
        enlist_count: 1,
        mercenary_count: 0,
        intercept_count: 0,
        spectate_count: 0,
        winner_group: None,
        loser_group: None,
    };
    let json = serde_json::to_string(&payload).expect("serialize");
    let back: FactionWarStateV1 = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        payload, back,
        "FactionWarStateV1 no-outcome roundtrip must be lossless"
    );
    // None 时 JSON 不含 winner_group/loser_group（skip_serializing_if）
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        v.get("winner_group").is_none(),
        "winner_group should be absent when None"
    );
    assert!(
        v.get("loser_group").is_none(),
        "loser_group should be absent when None"
    );
}

#[test]
fn faction_war_state_wire_type_label_is_faction_war_state() {
    // payload_type_label → "faction_war_state"（历史 wire label 兼容）
    let label = payload_type_label(ServerDataType::FactionWarState);
    assert_eq!(
        label, "faction_war_state",
        "期望 FactionWarState 的 label 为 'faction_war_state'（历史 wire label），实际 {label}"
    );
}

#[test]
fn faction_war_state_serializes_type_field_as_faction_war_state() {
    // wire type tag "faction_war_state" 保持历史兼容。
    // ServerDataV1 用 #[serde(flatten)]，所以 type + fields 全在顶层（无 "payload" 嵌套）。
    let inner = FactionWarStateV1 {
        war_id: 1,
        zone: "血谷".to_string(),
        region_descriptor: "血谷一带散修".to_string(),
        phase: "emerging".to_string(),
        groups: vec![2, 3],
        enlist_count: 0,
        mercenary_count: 0,
        intercept_count: 0,
        spectate_count: 0,
        winner_group: None,
        loser_group: None,
    };
    let wrapper = ServerDataV1::new(ServerDataPayloadV1::FactionWarState(inner));
    let json = serde_json::to_string(&wrapper).expect("serialize wrapper");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    // payload 字段 flatten 到顶层，type 字段在顶层
    assert_eq!(
        v["type"],
        serde_json::json!("faction_war_state"),
        "期望 wire type = 'faction_war_state'（守恒：payload 零真元，reframe b 零宗门），实际 {}",
        v["type"]
    );
    // 守恒红线：不含任何真元字段名（qi 在字段名中不应出现）
    assert!(
        !json.contains("\"qi\"") && !json.contains("_qi\"") && !json.contains("\"qi_"),
        "期望 faction_war_state JSON 不含 qi 字段（零真元），实际 JSON: {json}"
    );
    // reframe b：region_descriptor 含「散修」
    assert!(
        v["region_descriptor"]
            .as_str()
            .unwrap_or("")
            .contains("散修"),
        "期望 region_descriptor 含「散修」（匿名散修描述符），实际 {}",
        v["region_descriptor"]
    );
}

// ─── plan-combat-skill-feedback-bridges-v1 P4：AnqiHud schema pin ─

#[derive(Debug, serde::Deserialize)]
struct AnqiHudWireCorpus {
    base: serde_json::Value,
    cases: Vec<AnqiHudWireCorpusCase>,
}

#[derive(Debug, serde::Deserialize)]
struct AnqiHudWireCorpusCase {
    name: String,
    accepted: bool,
    set: Option<serde_json::Map<String, serde_json::Value>>,
    remove: Option<String>,
}

fn materialize_anqi_hud_wire_case(
    base: &serde_json::Value,
    test_case: &AnqiHudWireCorpusCase,
) -> serde_json::Value {
    let mut payload = base
        .as_object()
        .expect("anqi_hud wire corpus base must be an object")
        .clone();
    let mutation_count = test_case.set.as_ref().map_or(0, serde_json::Map::len)
        + usize::from(test_case.remove.is_some());
    assert!(
        mutation_count <= 1,
        "corpus case '{}' must isolate at most one field constraint",
        test_case.name
    );

    if let Some(fields) = &test_case.set {
        for (field, value) in fields {
            payload.insert(field.clone(), value.clone());
        }
    }
    if let Some(field) = &test_case.remove {
        payload.remove(field);
    }
    serde_json::Value::Object(payload)
}

#[test]
fn anqi_hud_shared_wire_corpus_matches_rust_serde() {
    let corpus: AnqiHudWireCorpus = serde_json::from_str(include_str!(
        "../../../../agent/packages/schema/samples/server-data.anqi-hud.wire-corpus.json"
    ))
    .expect("shared anqi_hud wire corpus must be valid JSON");
    let mut names = std::collections::HashSet::new();

    for test_case in &corpus.cases {
        assert!(
            names.insert(test_case.name.as_str()),
            "duplicate corpus case '{}'",
            test_case.name
        );
        let payload = materialize_anqi_hud_wire_case(&corpus.base, test_case);
        let result = serde_json::from_value::<ServerDataV1>(payload.clone());
        assert_eq!(
            result.is_ok(),
            test_case.accepted,
            "Rust serde verdict drifted for case '{}'; payload={payload}; result={result:?}",
            test_case.name
        );

        if let Ok(wrapper) = result {
            assert_eq!(
                wrapper.payload_type(),
                ServerDataType::AnqiHud,
                "accepted corpus case '{}' must retain the anqi_hud payload type",
                test_case.name
            );
            assert!(
                matches!(&wrapper.payload, ServerDataPayloadV1::AnqiHud(_)),
                "accepted corpus case '{}' must deserialize to the AnqiHud variant; actual={:?}",
                test_case.name,
                wrapper.payload
            );
        }
    }
}

#[test]
fn anqi_hud_shared_samples_match_rust_serde_verdicts() {
    let valid_samples = [
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.echo.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.aim.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.charge.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.abrasion.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.multishot.sample.json"
        ),
    ];
    for sample in valid_samples {
        let wrapper: ServerDataV1 =
            serde_json::from_str(sample).expect("valid shared anqi_hud sample must deserialize");
        assert_eq!(
            wrapper.payload_type(),
            ServerDataType::AnqiHud,
            "valid shared sample must retain the anqi_hud payload type; sample={sample}"
        );
        assert!(
            matches!(&wrapper.payload, ServerDataPayloadV1::AnqiHud(_)),
            "valid shared sample must deserialize to the AnqiHud variant; actual={:?}; sample={sample}",
            wrapper.payload
        );
    }

    let invalid_samples = [
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-missing-field.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-extra-field.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-kind.sample.json"
        ),
        include_str!(
            "../../../../agent/packages/schema/samples/server-data.anqi-hud.invalid-tick-overflow.sample.json"
        ),
    ];
    for sample in invalid_samples {
        assert!(
            serde_json::from_str::<ServerDataV1>(sample).is_err(),
            "invalid shared anqi_hud sample must be rejected: {sample}"
        );
    }
}

#[test]
fn anqi_hud_v1_roundtrip() {
    let original = bong_server::schema::server_data::AnqiHudV1 {
        kind: AnqiHudKindV1::Abrasion,
        echo_count: 3,
        aim_progress: 0.5,
        charge_progress: 0.25,
        abrasion_container: "quiver".to_string(),
        abrasion_qi_payload: 12.5,
        tick: 999,
    };
    let json = serde_json::to_string(&original).expect("AnqiHudV1 应能序列化");
    let back: bong_server::schema::server_data::AnqiHudV1 =
        serde_json::from_str(&json).expect("AnqiHudV1 应能反序列化");
    assert_eq!(
        original, back,
        "AnqiHudV1 JSON roundtrip 必须无损；JSON={json}"
    );
}

#[test]
fn anqi_hud_payload_type_label_is_anqi_hud() {
    let label = payload_type_label(ServerDataType::AnqiHud);
    assert_eq!(
        label, "anqi_hud",
        "期望 AnqiHud 的 label 为 'anqi_hud'（client 路由键），实际 {label}"
    );
}

#[test]
fn anqi_hud_variant_type_and_complete_wire_shape_serialize_together() {
    let inner = bong_server::schema::server_data::AnqiHudV1 {
        kind: AnqiHudKindV1::Echo,
        echo_count: 5,
        aim_progress: 0.0,
        charge_progress: 0.0,
        abrasion_container: String::new(),
        abrasion_qi_payload: 0.0,
        tick: 42,
    };
    let wrapper = ServerDataV1::new(ServerDataPayloadV1::AnqiHud(inner));
    assert_eq!(
        wrapper.payload_type(),
        ServerDataType::AnqiHud,
        "AnqiHud wrapper must report the payload type used by client routing"
    );
    let value = serde_json::to_value(&wrapper).expect("serialize AnqiHud wrapper");
    assert_eq!(
        value,
        serde_json::json!({
            "v": SERVER_DATA_VERSION,
            "type": "anqi_hud",
            "kind": "echo",
            "echo_count": 5,
            "aim_progress": 0.0,
            "charge_progress": 0.0,
            "abrasion_container": "",
            "abrasion_qi_payload": 0.0,
            "tick": 42
        }),
        "AnqiHud wrapper must serialize every canonical v1 wire field together"
    );
    assert_eq!(
        payload_type_label(wrapper.payload_type()),
        value["type"].as_str().expect("wire type must be a string"),
        "payload_type_label must match the serialized wire type used by client routing"
    );
    let decoded: ServerDataV1 =
        serde_json::from_value(value).expect("serialized wrapper must deserialize");
    let ServerDataPayloadV1::AnqiHud(decoded_hud) = decoded.payload else {
        panic!("wire type anqi_hud must deserialize to the AnqiHud payload variant");
    };
    assert_eq!(
        decoded_hud.kind,
        AnqiHudKindV1::Echo,
        "complete wire shape must preserve the echo kind after deserialization"
    );
    assert_eq!(
        decoded_hud.echo_count, 5,
        "complete wire shape must preserve echo_count=5 after deserialization"
    );
    assert_eq!(
        decoded_hud.tick, 42,
        "complete wire shape must preserve tick=42 after deserialization"
    );
}

// ─── 震脉 v2 HUD S2C：schema pin（字段须与 client ZhenmaiHudServerDataHandler 逐一对齐） ─

#[test]
fn zhenmai_hud_v1_roundtrip() {
    let original = bong_server::schema::server_data::ZhenmaiHudV1 {
        skill_id: "sever_chain".to_string(),
        meridian_id: "Heart".to_string(),
        contam_removed: 0.0,
        remaining_points: 0,
        damage_reduction: 0.0,
        k_drain: 1.5,
        duration_ms: 60_000,
        tick: 999,
    };
    let json = serde_json::to_string(&original).expect("ZhenmaiHudV1 应能序列化");
    let back: bong_server::schema::server_data::ZhenmaiHudV1 =
        serde_json::from_str(&json).expect("ZhenmaiHudV1 应能反序列化");
    assert_eq!(
        original, back,
        "ZhenmaiHudV1 JSON roundtrip 必须无损；JSON={json}"
    );
}

#[test]
fn zhenmai_hud_payload_type_label_is_zhenmai_hud() {
    let label = payload_type_label(ServerDataType::ZhenmaiHud);
    assert_eq!(
        label, "zhenmai_hud",
        "期望 ZhenmaiHud 的 label 为 'zhenmai_hud'（client ServerDataRouter 路由键），实际 {label}"
    );
}

#[test]
fn zhenmai_hud_wire_emits_client_contract_fields() {
    // 字段名/类型须与 client ZhenmaiHudServerDataHandler.readString/readDouble/readDuration 对齐。
    let inner = bong_server::schema::server_data::ZhenmaiHudV1 {
        skill_id: "neutralize".to_string(),
        meridian_id: "Lung".to_string(),
        contam_removed: 2.5,
        remaining_points: 0,
        damage_reduction: 0.0,
        k_drain: 0.0,
        duration_ms: 0,
        tick: 64,
    };
    let wrapper = ServerDataV1::new(ServerDataPayloadV1::ZhenmaiHud(inner));
    let json = serde_json::to_string(&wrapper).expect("serialize ZhenmaiHud wrapper");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        v["type"],
        serde_json::json!("zhenmai_hud"),
        "wire type 须为 client 路由键 'zhenmai_hud'，实际 {}",
        v["type"]
    );
    // client switch(skill_id) 的判别键
    assert_eq!(
        v["skill_id"],
        serde_json::json!("neutralize"),
        "skill_id 须为 client switch 键 'neutralize'，实际 {}",
        v["skill_id"]
    );
    // client readString("meridian_id")
    assert_eq!(v["meridian_id"], serde_json::json!("Lung"));
    // client readDouble("contam_removed", 0.0)
    assert_eq!(v["contam_removed"], serde_json::json!(2.5));
    // 契约字段全部在场（即使为零值，flatten 不 skip → client readX 各有所依）
    for field in [
        "skill_id",
        "meridian_id",
        "contam_removed",
        "remaining_points",
        "damage_reduction",
        "k_drain",
        "duration_ms",
        "tick",
    ] {
        assert!(
            v.get(field).is_some(),
            "ZhenmaiHud wire 须含 client 契约字段 '{field}'；实际 JSON={json}"
        );
    }
}

#[test]
fn zhenmai_hud_harden_wire_damage_reduction_is_reduction_not_passthrough() {
    // 契约语义 pin：client ZhenmaiHudPlanner.appendHarden 把 damage_reduction 当作
    // 「减伤比例」渲染（value1*100 → 「减伤X%」、条形填充 = value1，1.0=全免）。
    // 因此 wire 的 damage_reduction 必须是减伤比例（reduction），而不是 server 内部
    // HardenProfile.damage_multiplier 的「伤害通过率」（passthrough）。
    // bridge 负责转换 reduction = 1 - passthrough（见 zhenmai_v2_event_bridge.rs harden 分支）；
    // 本 pin 锁住 wire 形态：harden 场景下 damage_reduction 是 [0,1] 的减伤比例。
    // 例：Spirit 境 passthrough=0.35 → wire damage_reduction=0.65（实际减伤 65%）。
    let inner = bong_server::schema::server_data::ZhenmaiHudV1 {
        skill_id: "harden".to_string(),
        meridian_id: "Heart".to_string(),
        contam_removed: 0.0,
        remaining_points: 0,
        damage_reduction: 0.65,
        k_drain: 0.0,
        duration_ms: 1_000,
        tick: 70,
    };
    let wrapper = ServerDataV1::new(ServerDataPayloadV1::ZhenmaiHud(inner));
    let json = serde_json::to_string(&wrapper).expect("serialize harden ZhenmaiHud wrapper");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["skill_id"], serde_json::json!("harden"));
    let reduction = v["damage_reduction"]
        .as_f64()
        .expect("damage_reduction 须为数值");
    assert!(
        (reduction - 0.65).abs() < 1e-4,
        "harden wire damage_reduction 须为减伤比例 0.65（client 渲染「减伤65%」），\
         不是 passthrough multiplier 0.35；实际 {reduction}（JSON={json}）"
    );
    assert!(
        (0.0..=1.0).contains(&reduction),
        "damage_reduction 须落在减伤比例区间 [0,1]，实际 {reduction}"
    );
}
