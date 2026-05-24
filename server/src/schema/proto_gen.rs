//! Protobuf 生成代码入口 — prost codegen 输出。
//!
//! `build.rs` 编译 `proto/bong/*.proto`，prost 默认按 proto package 名
//! 输出到 `$OUT_DIR/bong.rs`。此文件通过 `include!` 引入。

/// Proto-generated types for `package bong;`.
#[allow(clippy::enum_variant_names, clippy::large_enum_variant)]
pub mod bong {
    include!(concat!(env!("OUT_DIR"), "/bong.rs"));
}

#[cfg(test)]
mod tests {
    use super::bong::*;
    use prost::Message;

    // ─── common.proto 枚举 pin 测试 ──────────────────────────────

    #[test]
    fn realm_enum_has_all_6_stages_plus_unspecified() {
        let expected = [
            (Realm::Unspecified, 0, "REALM_UNSPECIFIED"),
            (Realm::Awaken, 1, "REALM_AWAKEN"),
            (Realm::Induce, 2, "REALM_INDUCE"),
            (Realm::Condense, 3, "REALM_CONDENSE"),
            (Realm::Solidify, 4, "REALM_SOLIDIFY"),
            (Realm::Spirit, 5, "REALM_SPIRIT"),
            (Realm::Void, 6, "REALM_VOID"),
        ];
        for (variant, wire, name) in expected {
            assert_eq!(
                variant as i32, wire,
                "Realm::{name} 的 wire value 应为 {wire}"
            );
            assert_eq!(
                variant.as_str_name(),
                name,
                "Realm wire {wire} 的 str_name 应为 {name}"
            );
            assert_eq!(
                Realm::from_str_name(name),
                Some(variant),
                "Realm::from_str_name({name}) 应返回 Some({variant:?})"
            );
        }
        // 确保 7 个值涵盖了所有变体（6 境界 + unspecified）。
        assert_eq!(expected.len(), 7, "Realm 应有 7 个变体（含 UNSPECIFIED）");
    }

    #[test]
    fn meridian_id_enum_has_20_meridians_plus_unspecified() {
        // 12 正经 + 8 奇经 + 1 unspecified = 21
        let all = [
            MeridianId::Unspecified,
            MeridianId::Lung,
            MeridianId::LargeIntestine,
            MeridianId::Stomach,
            MeridianId::Spleen,
            MeridianId::Heart,
            MeridianId::SmallIntestine,
            MeridianId::Bladder,
            MeridianId::Kidney,
            MeridianId::Pericardium,
            MeridianId::TripleEnergizer,
            MeridianId::Gallbladder,
            MeridianId::Liver,
            MeridianId::Ren,
            MeridianId::Du,
            MeridianId::Chong,
            MeridianId::Dai,
            MeridianId::YinQiao,
            MeridianId::YangQiao,
            MeridianId::YinWei,
            MeridianId::YangWei,
        ];
        assert_eq!(
            all.len(),
            21,
            "MeridianId 应有 21 个变体（20 经脉 + UNSPECIFIED）"
        );
        // Wire values 应连续 0..=20。
        for (i, m) in all.iter().enumerate() {
            assert_eq!(
                *m as i32, i as i32,
                "MeridianId 变体 {i} 的 wire value 应为 {i}"
            );
        }
    }

    #[test]
    fn skill_id_enum_has_6_skills_plus_unspecified() {
        let all = [
            SkillId::Unspecified,
            SkillId::Herbalism,
            SkillId::Alchemy,
            SkillId::Forging,
            SkillId::Combat,
            SkillId::Mineral,
            SkillId::Cultivation,
        ];
        assert_eq!(
            all.len(),
            7,
            "SkillId 应有 7 个变体（6 技能 + UNSPECIFIED）"
        );
    }

    #[test]
    fn color_kind_enum_has_10_colors_plus_unspecified() {
        let all = [
            ColorKind::Unspecified,
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
            ColorKind::Intricate,
            ColorKind::Gentle,
            ColorKind::Insidious,
            ColorKind::Violent,
            ColorKind::Turbid,
        ];
        assert_eq!(
            all.len(),
            11,
            "ColorKind 应有 11 个变体（10 真元色 + UNSPECIFIED）"
        );
    }

    // ─── 消息 roundtrip 测试 ─────────────────────────────────────

    #[test]
    fn vec3_roundtrip() {
        let original = Vec3 {
            x: 1.5,
            y: -42.0,
            z: 999.999,
        };
        let bytes = original.encode_to_vec();
        let decoded = Vec3::decode(bytes.as_slice()).expect("Vec3 decode 失败");
        assert_eq!(decoded, original, "Vec3 roundtrip 不一致");
    }

    #[test]
    fn item_slot_roundtrip_with_optional_fields() {
        let with_options = ItemSlot {
            slot_index: 5,
            template_id: "iron_sword".to_string(),
            count: 1,
            forge_color: Some(ColorKind::Sharp as i32),
            durability: Some(100),
        };
        let bytes = with_options.encode_to_vec();
        let decoded = ItemSlot::decode(bytes.as_slice()).expect("ItemSlot decode 失败");
        assert_eq!(
            decoded, with_options,
            "ItemSlot roundtrip（含 optional）不一致"
        );

        let without_options = ItemSlot {
            slot_index: 0,
            template_id: "herb".to_string(),
            count: 64,
            forge_color: None,
            durability: None,
        };
        let bytes = without_options.encode_to_vec();
        let decoded = ItemSlot::decode(bytes.as_slice()).expect("ItemSlot decode 失败");
        assert_eq!(
            decoded, without_options,
            "ItemSlot roundtrip（无 optional）不一致"
        );
    }

    #[test]
    fn welcome_message_roundtrip() {
        let msg = Welcome {
            message: "欢迎来到末法残土".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = Welcome::decode(bytes.as_slice()).expect("Welcome decode 失败");
        assert_eq!(decoded.message, "欢迎来到末法残土");
    }

    // ─── envelope oneof roundtrip ────────────────────────────────

    #[test]
    fn server_data_envelope_welcome_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::Welcome(Welcome {
                message: "hello".to_string(),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ServerDataEnvelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::Welcome(w)) => {
                assert_eq!(w.message, "hello", "envelope welcome message 不匹配");
            }
            other => panic!("期望 Welcome payload，实际是 {other:?}"),
        }
    }

    #[test]
    fn server_data_envelope_cultivation_detail_roundtrip() {
        let detail = CultivationDetail {
            realm: Realm::Condense as i32,
            meridians: vec![
                MeridianState {
                    id: MeridianId::Lung as i32,
                    opened: true,
                    flow_rate: 0.5,
                    flow_capacity: 1.0,
                    integrity: 0.95,
                    open_progress: 1.0,
                    cracks_count: 0,
                },
                MeridianState {
                    id: MeridianId::Heart as i32,
                    opened: false,
                    flow_rate: 0.0,
                    flow_capacity: 0.0,
                    integrity: 1.0,
                    open_progress: 0.3,
                    cracks_count: 2,
                },
            ],
            target_meridian: Some(MeridianId::Heart as i32),
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: ColorKind::Mellow as i32,
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: vec![],
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CultivationDetail(detail)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("CultivationDetail envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::CultivationDetail(d)) => {
                assert_eq!(d.realm, Realm::Condense as i32, "realm 不匹配");
                assert_eq!(d.meridians.len(), 2, "经脉数量不匹配");
                assert_eq!(
                    d.meridians[0].id,
                    MeridianId::Lung as i32,
                    "第一条经脉应为 Lung"
                );
                assert!(d.meridians[0].opened, "Lung 应已打通");
                assert!(!d.meridians[1].opened, "Heart 应未打通");
                assert_eq!(
                    d.target_meridian,
                    Some(MeridianId::Heart as i32),
                    "target_meridian 应为 Heart ({})，实际是 {:?}",
                    MeridianId::Heart as i32,
                    d.target_meridian
                );
            }
            other => panic!("期望 CultivationDetail payload，实际是 {other:?}"),
        }
    }

    #[test]
    fn client_request_envelope_set_meridian_target_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SetMeridianTarget(
                SetMeridianTarget {
                    meridian: MeridianId::Pericardium as i32,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("ClientRequestEnvelope decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SetMeridianTarget(t)) => {
                assert_eq!(
                    t.meridian,
                    MeridianId::Pericardium as i32,
                    "SetMeridianTarget 经脉应为 Pericardium"
                );
            }
            other => panic!("期望 SetMeridianTarget payload，实际是 {other:?}"),
        }
    }

    #[test]
    fn client_request_envelope_breakthrough_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::BreakthroughRequest(
                BreakthroughRequest {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("ClientRequestEnvelope decode 失败");
        assert!(
            matches!(
                decoded.payload,
                Some(client_request_envelope::Payload::BreakthroughRequest(_))
            ),
            "期望 BreakthroughRequest payload"
        );
    }

    // ─── 边界测试 ────────────────────────────────────────────────

    #[test]
    fn empty_envelope_roundtrip() {
        // payload = None 的空信封应能正常 roundtrip（0 字节 wire）。
        let envelope = ServerDataEnvelope { payload: None };
        let bytes = envelope.encode_to_vec();
        assert!(bytes.is_empty(), "空 envelope 应编码为 0 字节");
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("空 envelope decode 不应失败");
        assert!(
            decoded.payload.is_none(),
            "空 envelope 解码后 payload 应为 None"
        );
    }

    #[test]
    fn narration_batch_empty_and_nonempty() {
        // 空 narration 列表
        let empty = NarrationBatch { narrations: vec![] };
        let bytes = empty.encode_to_vec();
        let decoded = NarrationBatch::decode(bytes.as_slice()).unwrap();
        assert!(decoded.narrations.is_empty(), "空 narrations 应为 []");

        // 非空
        let batch = NarrationBatch {
            narrations: vec![
                NarrationEntry {
                    text: "天雷将至".to_string(),
                    scope: "broadcast".to_string(),
                    style: "perception".to_string(),
                    target: None,
                    kind: None,
                },
                NarrationEntry {
                    text: "你感到一阵寒意".to_string(),
                    scope: "player".to_string(),
                    style: "narration".to_string(),
                    target: Some("Steve".to_string()),
                    kind: None,
                },
            ],
        };
        let bytes = batch.encode_to_vec();
        let decoded = NarrationBatch::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.narrations.len(), 2, "应有 2 条 narration");
        assert_eq!(decoded.narrations[0].text, "天雷将至");
        assert_eq!(decoded.narrations[1].scope, "player");
    }

    #[test]
    fn zone_info_optional_fields() {
        // perception_text = None
        let info = ZoneInfo {
            zone: "qingyun_peaks".to_string(),
            spirit_qi: 0.85,
            danger_level: 3,
            status: "stable".to_string(),
            active_events: vec!["beast_tide".to_string()],
            perception_text: None,
        };
        let bytes = info.encode_to_vec();
        let decoded = ZoneInfo::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.zone, "qingyun_peaks");
        assert_eq!(decoded.spirit_qi, 0.85);
        assert!(
            decoded.perception_text.is_none(),
            "perception_text 应为 None"
        );
        assert_eq!(decoded.active_events.len(), 1);

        // perception_text = Some
        let info2 = ZoneInfo {
            perception_text: Some("灵气充沛，草木繁茂".to_string()),
            ..info.clone()
        };
        let bytes = info2.encode_to_vec();
        let decoded = ZoneInfo::decode(bytes.as_slice()).unwrap();
        assert_eq!(
            decoded.perception_text.as_deref(),
            Some("灵气充沛，草木繁茂"),
            "perception_text 应为 Some"
        );
    }

    #[test]
    fn player_state_optional_fields() {
        let state = PlayerState {
            player: Some("test_player".to_string()),
            realm: Realm::Spirit as i32,
            spirit_qi: 42.5,
            karma: 0.8,
            composite_power: 100.0,
            zone: "spawn".to_string(),
            local_neg_pressure: Some(0.3),
            breakdown: None,
            season_state: None,
            social: None,
        };
        let bytes = state.encode_to_vec();
        let decoded = PlayerState::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.player.as_deref(), Some("test_player"));
        assert_eq!(decoded.realm, Realm::Spirit as i32);
        assert_eq!(decoded.local_neg_pressure, Some(0.3));

        // 无 optional
        let state2 = PlayerState {
            player: None,
            realm: Realm::Awaken as i32,
            spirit_qi: 0.0,
            karma: 0.0,
            composite_power: 0.0,
            zone: "spawn".to_string(),
            local_neg_pressure: None,
            breakdown: None,
            season_state: None,
            social: None,
        };
        let bytes = state2.encode_to_vec();
        let decoded = PlayerState::decode(bytes.as_slice()).unwrap();
        assert!(decoded.player.is_none());
        assert!(decoded.local_neg_pressure.is_none());
    }

    #[test]
    fn unknown_enum_value_handled_gracefully() {
        // Proto3 允许未知 enum 值——prost 保留原始 i32。
        let slot = ItemSlot {
            slot_index: 0,
            template_id: "test".to_string(),
            count: 1,
            forge_color: Some(999), // 不存在的 ColorKind 值
            durability: None,
        };
        let bytes = slot.encode_to_vec();
        let decoded = ItemSlot::decode(bytes.as_slice()).unwrap();
        assert_eq!(decoded.forge_color, Some(999), "未知 enum 值应保留原始 i32");
    }

    // ─── 错误分支 / malformed decode 测试 ────────────────────────

    #[test]
    fn server_data_envelope_rejects_malformed_bytes() {
        let bad = vec![0x0a, 0xff, 0xff, 0xff];
        let result = ServerDataEnvelope::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "truncated/malformed bytes 应导致 decode 失败，实际成功: {result:?}"
        );
    }

    #[test]
    fn client_request_envelope_rejects_malformed_bytes() {
        let bad = vec![0x0a, 0xff, 0xff, 0xff];
        let result = ClientRequestEnvelope::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "truncated/malformed bytes 应导致 decode 失败，实际成功: {result:?}"
        );
    }

    #[test]
    fn item_slot_rejects_truncated_bytes() {
        let valid = ItemSlot {
            slot_index: 1,
            template_id: "sword".to_string(),
            count: 1,
            forge_color: None,
            durability: None,
        };
        let mut bytes = valid.encode_to_vec();
        bytes.truncate(bytes.len() / 2);
        let result = ItemSlot::decode(bytes.as_slice());
        assert!(
            result.is_err(),
            "截断后的 ItemSlot bytes 应导致 decode 失败，实际成功: {result:?}"
        );
    }

    #[test]
    fn narration_batch_rejects_truncated_length_delimited() {
        // length-delimited field 声明 100 字节但实际只有 2 字节
        let bad = vec![0x0a, 0x64, 0x0a, 0x02];
        let result = NarrationBatch::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "截断的 length-delimited field 应导致 decode 失败，实际成功: {result:?}"
        );
    }

    // ═══════════════════════════════════════════════════════════
    // P1 先锋迁移：10 payload 补齐 roundtrip + 边界 + 错误分支
    // ═══════════════════════════════════════════════════════════

    // ─── 3. NarrationEntry 补齐 optional target/kind ────────────

    #[test]
    fn narration_entry_with_optional_target_and_kind() {
        let entry = NarrationEntry {
            text: "你感到一阵寒意".to_string(),
            scope: "player".to_string(),
            style: "perception".to_string(),
            target: Some("offline:Steve".to_string()),
            kind: Some("death_insight".to_string()),
        };
        let bytes = entry.encode_to_vec();
        let decoded = NarrationEntry::decode(bytes.as_slice()).expect("NarrationEntry decode 失败");
        assert_eq!(
            decoded.target.as_deref(),
            Some("offline:Steve"),
            "NarrationEntry.target 应为 Some(\"offline:Steve\")，实际 {:?}",
            decoded.target
        );
        assert_eq!(
            decoded.kind.as_deref(),
            Some("death_insight"),
            "NarrationEntry.kind 应为 Some(\"death_insight\")，实际 {:?}",
            decoded.kind
        );
        assert_eq!(decoded.text, "你感到一阵寒意");
        assert_eq!(decoded.scope, "player");
        assert_eq!(decoded.style, "perception");
    }

    #[test]
    fn narration_entry_without_optional_fields() {
        let entry = NarrationEntry {
            text: "天雷将至".to_string(),
            scope: "broadcast".to_string(),
            style: "system_warning".to_string(),
            target: None,
            kind: None,
        };
        let bytes = entry.encode_to_vec();
        let decoded = NarrationEntry::decode(bytes.as_slice()).expect("NarrationEntry decode 失败");
        assert!(
            decoded.target.is_none(),
            "broadcast 的 target 应为 None，实际 {:?}",
            decoded.target
        );
        assert!(
            decoded.kind.is_none(),
            "broadcast 的 kind 应为 None，实际 {:?}",
            decoded.kind
        );
    }

    #[test]
    fn narration_batch_all_styles_roundtrip() {
        let styles = [
            "system_warning",
            "perception",
            "narration",
            "era_decree",
            "political_jianghu",
        ];
        for style in styles {
            let batch = NarrationBatch {
                narrations: vec![NarrationEntry {
                    text: format!("style={style}"),
                    scope: "broadcast".to_string(),
                    style: style.to_string(),
                    target: None,
                    kind: None,
                }],
            };
            let bytes = batch.encode_to_vec();
            let decoded = NarrationBatch::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("NarrationBatch style={style} decode 失败: {e}"));
            assert_eq!(
                decoded.narrations[0].style, style,
                "style roundtrip 不匹配: 期望 {style}"
            );
        }
    }

    #[test]
    fn narration_batch_all_scopes_roundtrip() {
        let scopes = ["broadcast", "zone", "player"];
        for scope in scopes {
            let target = if scope == "broadcast" {
                None
            } else {
                Some("target_player".to_string())
            };
            let entry = NarrationEntry {
                text: "test".to_string(),
                scope: scope.to_string(),
                style: "narration".to_string(),
                target,
                kind: None,
            };
            let bytes = entry.encode_to_vec();
            let decoded = NarrationEntry::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("NarrationEntry scope={scope} decode 失败: {e}"));
            assert_eq!(decoded.scope, scope);
        }
    }

    #[test]
    fn narration_entry_all_kind_variants() {
        let kinds = [
            "death_insight",
            "niche_intrusion",
            "niche_intrusion_by_npc",
            "npc_farm_pressure",
            "scattered_cultivator",
            "political_jianghu",
        ];
        for kind in kinds {
            let entry = NarrationEntry {
                text: "test".to_string(),
                scope: "broadcast".to_string(),
                style: "narration".to_string(),
                target: None,
                kind: Some(kind.to_string()),
            };
            let bytes = entry.encode_to_vec();
            let decoded = NarrationEntry::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("NarrationEntry kind={kind} decode 失败: {e}"));
            assert_eq!(
                decoded.kind.as_deref(),
                Some(kind),
                "kind roundtrip 不匹配: 期望 Some(\"{kind}\")"
            );
        }
    }

    // ─── 5. PlayerState 补齐 breakdown/season_state/social ──────

    #[test]
    fn player_state_full_fields_roundtrip() {
        let state = PlayerState {
            player: Some("test_player".to_string()),
            realm: Realm::Condense as i32,
            spirit_qi: 42.5,
            karma: 0.8,
            composite_power: 100.0,
            zone: "qingyun_peaks".to_string(),
            local_neg_pressure: Some(0.3),
            breakdown: Some(PlayerPowerBreakdown {
                combat: 50.0,
                wealth: 20.0,
                social: 10.0,
                karma: 15.0,
                territory: 5.0,
            }),
            season_state: Some(SeasonState {
                season: Season::Summer as i32,
                tick_into_phase: 1500,
                phase_total_ticks: 72000,
                year_index: 3,
            }),
            social: Some(PlayerSocialSnapshot {
                renown: Some(RenownSnapshot {
                    fame: 100,
                    notoriety: -5,
                    top_tags: vec![RenownTag {
                        tag: "righteous".to_string(),
                        weight: 0.8,
                        last_seen_tick: 5000,
                        permanent: false,
                    }],
                }),
                relationships: vec![],
                exposed_to_count: 3,
                faction_membership: None,
            }),
        };
        let bytes = state.encode_to_vec();
        let decoded = PlayerState::decode(bytes.as_slice()).expect("PlayerState full decode 失败");
        assert_eq!(decoded.player.as_deref(), Some("test_player"));
        assert_eq!(decoded.realm, Realm::Condense as i32);
        assert_eq!(decoded.spirit_qi, 42.5);
        let bd = decoded.breakdown.expect("breakdown 应存在");
        assert_eq!(bd.combat, 50.0, "breakdown.combat 不匹配");
        assert_eq!(bd.territory, 5.0, "breakdown.territory 不匹配");
        let ss = decoded.season_state.expect("season_state 应存在");
        assert_eq!(ss.season, Season::Summer as i32, "season 应为 Summer");
        assert_eq!(ss.tick_into_phase, 1500, "tick_into_phase 不匹配");
        assert_eq!(ss.year_index, 3, "year_index 不匹配");
        let soc = decoded.social.expect("social 应存在");
        let renown = soc.renown.expect("renown 应存在");
        assert_eq!(renown.fame, 100, "fame 不匹配");
        assert_eq!(renown.notoriety, -5, "notoriety 不匹配");
        assert_eq!(renown.top_tags.len(), 1, "top_tags 应有 1 条");
        assert_eq!(soc.exposed_to_count, 3, "exposed_to_count 不匹配");
    }

    #[test]
    fn player_state_without_optional_nested_messages() {
        let state = PlayerState {
            player: None,
            realm: Realm::Awaken as i32,
            spirit_qi: 0.0,
            karma: 0.0,
            composite_power: 0.0,
            zone: "spawn".to_string(),
            local_neg_pressure: None,
            breakdown: None,
            season_state: None,
            social: None,
        };
        let bytes = state.encode_to_vec();
        let decoded =
            PlayerState::decode(bytes.as_slice()).expect("PlayerState minimal decode 失败");
        assert!(decoded.player.is_none());
        assert!(decoded.breakdown.is_none(), "breakdown 应为 None");
        assert!(decoded.season_state.is_none(), "season_state 应为 None");
        assert!(decoded.social.is_none(), "social 应为 None");
    }

    #[test]
    fn player_state_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::PlayerState(PlayerState {
                player: Some("Steve".to_string()),
                realm: Realm::Spirit as i32,
                spirit_qi: 999.9,
                karma: 0.0,
                composite_power: 500.0,
                zone: "rift_valley".to_string(),
                local_neg_pressure: Some(0.7),
                breakdown: Some(PlayerPowerBreakdown {
                    combat: 200.0,
                    wealth: 100.0,
                    social: 100.0,
                    karma: 50.0,
                    territory: 50.0,
                }),
                season_state: None,
                social: None,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("PlayerState envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::PlayerState(ps)) => {
                assert_eq!(ps.realm, Realm::Spirit as i32);
                assert_eq!(ps.breakdown.unwrap().combat, 200.0);
            }
            other => panic!("期望 PlayerState payload，实际 {other:?}"),
        }
    }

    // ─── 6. CultivationDetail 补齐所有额外字段 ─────────────────

    #[test]
    fn cultivation_detail_full_fields_roundtrip() {
        let detail = CultivationDetail {
            realm: Realm::Solidify as i32,
            meridians: vec![MeridianState {
                id: MeridianId::Lung as i32,
                opened: true,
                flow_rate: 0.8,
                flow_capacity: 1.0,
                integrity: 0.95,
                open_progress: 1.0,
                cracks_count: 0,
            }],
            target_meridian: Some(MeridianId::Heart as i32),
            contamination_total: 12.5,
            lifespan: Some(LifespanPreview {
                years_lived: 35.0,
                cap_by_realm: 200,
                remaining_years: 165.0,
                death_penalty_years: 5,
                tick_rate_multiplier: 1.0,
                is_wind_candle: false,
            }),
            recent_skill_milestones_summary: "Alchemy Lv.3".to_string(),
            skill_milestones: vec![SkillMilestoneSnapshot {
                skill: "alchemy".to_string(),
                new_lv: 3,
                achieved_at: 100_000,
                narration: "丹术精进".to_string(),
                total_xp_at: 5000,
            }],
            qi_color_main: ColorKind::Sharp as i32,
            qi_color_secondary: Some(ColorKind::Heavy as i32),
            qi_color_chaotic: false,
            qi_color_hunyuan: true,
            practice_weights: vec![PracticeWeight {
                color: ColorKind::Sharp as i32,
                weight: 0.7,
                ratio: 0.7,
            }],
        };
        let bytes = detail.encode_to_vec();
        let decoded = CultivationDetail::decode(bytes.as_slice())
            .expect("CultivationDetail full decode 失败");
        assert_eq!(decoded.realm, Realm::Solidify as i32);
        assert_eq!(
            decoded.contamination_total, 12.5,
            "contamination_total 不匹配"
        );
        let ls = decoded.lifespan.expect("lifespan 应存在");
        assert_eq!(ls.years_lived, 35.0);
        assert_eq!(ls.cap_by_realm, 200);
        assert!(!ls.is_wind_candle);
        assert_eq!(decoded.skill_milestones.len(), 1);
        assert_eq!(decoded.skill_milestones[0].skill, "alchemy");
        assert_eq!(decoded.skill_milestones[0].new_lv, 3);
        assert_eq!(decoded.qi_color_main, ColorKind::Sharp as i32);
        assert_eq!(decoded.qi_color_secondary, Some(ColorKind::Heavy as i32));
        assert!(!decoded.qi_color_chaotic);
        assert!(decoded.qi_color_hunyuan);
        assert_eq!(decoded.practice_weights.len(), 1);
        assert_eq!(decoded.practice_weights[0].ratio, 0.7);
    }

    #[test]
    fn cultivation_detail_empty_arrays_roundtrip() {
        let detail = CultivationDetail {
            realm: Realm::Awaken as i32,
            meridians: vec![],
            target_meridian: None,
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: ColorKind::Mellow as i32,
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: vec![],
        };
        let bytes = detail.encode_to_vec();
        let decoded = CultivationDetail::decode(bytes.as_slice())
            .expect("CultivationDetail empty arrays decode 失败");
        assert!(decoded.meridians.is_empty(), "empty meridians roundtrip");
        assert!(
            decoded.skill_milestones.is_empty(),
            "empty skill_milestones roundtrip"
        );
        assert!(
            decoded.practice_weights.is_empty(),
            "empty practice_weights roundtrip"
        );
        assert!(decoded.target_meridian.is_none(), "target_meridian 应 None");
        assert!(decoded.lifespan.is_none(), "lifespan 应 None");
        assert!(
            decoded.qi_color_secondary.is_none(),
            "qi_color_secondary 应 None"
        );
    }

    #[test]
    fn cultivation_detail_all_20_meridians() {
        let all_meridians: Vec<MeridianState> = (1..=20)
            .map(|i| MeridianState {
                id: i,
                opened: i <= 12,
                flow_rate: i as f64 * 0.05,
                flow_capacity: 1.0,
                integrity: 1.0 - (i as f64 * 0.01),
                open_progress: if i <= 12 { 1.0 } else { i as f64 * 0.05 },
                cracks_count: if i > 15 { i as u32 - 15 } else { 0 },
            })
            .collect();
        let detail = CultivationDetail {
            realm: Realm::Void as i32,
            meridians: all_meridians,
            target_meridian: Some(MeridianId::YangWei as i32),
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: ColorKind::Turbid as i32,
            qi_color_secondary: None,
            qi_color_chaotic: true,
            qi_color_hunyuan: false,
            practice_weights: vec![],
        };
        let bytes = detail.encode_to_vec();
        let decoded = CultivationDetail::decode(bytes.as_slice())
            .expect("CultivationDetail 20 meridians decode 失败");
        assert_eq!(
            decoded.meridians.len(),
            20,
            "应有 20 条经脉，实际 {}",
            decoded.meridians.len()
        );
        assert_eq!(decoded.target_meridian, Some(MeridianId::YangWei as i32));
        assert!(decoded.qi_color_chaotic);
    }

    #[test]
    fn cultivation_detail_all_10_color_kinds_in_practice_weights() {
        let weights: Vec<PracticeWeight> = [
            ColorKind::Sharp,
            ColorKind::Heavy,
            ColorKind::Mellow,
            ColorKind::Solid,
            ColorKind::Light,
            ColorKind::Intricate,
            ColorKind::Gentle,
            ColorKind::Insidious,
            ColorKind::Violent,
            ColorKind::Turbid,
        ]
        .iter()
        .enumerate()
        .map(|(i, c)| PracticeWeight {
            color: *c as i32,
            weight: (i + 1) as f64 * 0.1,
            ratio: 0.1,
        })
        .collect();
        let detail = CultivationDetail {
            realm: Realm::Condense as i32,
            meridians: vec![],
            target_meridian: None,
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: ColorKind::Mellow as i32,
            qi_color_secondary: None,
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: weights,
        };
        let bytes = detail.encode_to_vec();
        let decoded = CultivationDetail::decode(bytes.as_slice())
            .expect("CultivationDetail 10 colors decode 失败");
        assert_eq!(
            decoded.practice_weights.len(),
            10,
            "应有 10 种真元色权重，实际 {}",
            decoded.practice_weights.len()
        );
        assert_eq!(decoded.practice_weights[0].color, ColorKind::Sharp as i32);
        assert_eq!(decoded.practice_weights[9].color, ColorKind::Turbid as i32);
    }

    #[test]
    fn cultivation_detail_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CultivationDetail(
                CultivationDetail {
                    realm: Realm::Solidify as i32,
                    meridians: vec![MeridianState {
                        id: MeridianId::Lung as i32,
                        opened: true,
                        flow_rate: 0.5,
                        flow_capacity: 1.0,
                        integrity: 0.95,
                        open_progress: 1.0,
                        cracks_count: 0,
                    }],
                    target_meridian: None,
                    contamination_total: 1.5,
                    lifespan: Some(LifespanPreview {
                        years_lived: 10.0,
                        cap_by_realm: 100,
                        remaining_years: 90.0,
                        death_penalty_years: 0,
                        tick_rate_multiplier: 1.0,
                        is_wind_candle: false,
                    }),
                    recent_skill_milestones_summary: String::new(),
                    skill_milestones: vec![],
                    qi_color_main: ColorKind::Mellow as i32,
                    qi_color_secondary: None,
                    qi_color_chaotic: false,
                    qi_color_hunyuan: false,
                    practice_weights: vec![],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("CultivationDetail envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::CultivationDetail(d)) => {
                assert_eq!(d.contamination_total, 1.5);
                assert!(d.lifespan.is_some());
            }
            other => panic!("期望 CultivationDetail payload，实际 {other:?}"),
        }
    }

    // ─── 7. SkillXpGain — tagged union oneof ────────────────────

    #[test]
    fn skill_xp_gain_source_action_roundtrip() {
        let msg = SkillXpGain {
            char_id: 1001,
            skill: SkillId::Herbalism as i32,
            amount: 50,
            source: Some(skill_xp_gain::Source::ActionSource(XpGainSourceAction {
                plan_id: "lingtian".to_string(),
                action: "harvest_auto".to_string(),
            })),
            source_realm_breakthrough: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SkillXpGain::decode(bytes.as_slice()).expect("SkillXpGain action decode 失败");
        assert_eq!(decoded.char_id, 1001);
        assert_eq!(decoded.skill, SkillId::Herbalism as i32);
        assert_eq!(decoded.amount, 50);
        match decoded.source {
            Some(skill_xp_gain::Source::ActionSource(a)) => {
                assert_eq!(a.plan_id, "lingtian", "plan_id 不匹配");
                assert_eq!(a.action, "harvest_auto", "action 不匹配");
            }
            other => panic!("期望 SourceAction，实际 {other:?}"),
        }
        assert!(!decoded.source_realm_breakthrough);
    }

    #[test]
    fn skill_xp_gain_source_scroll_roundtrip() {
        let msg = SkillXpGain {
            char_id: 2002,
            skill: SkillId::Alchemy as i32,
            amount: 500,
            source: Some(skill_xp_gain::Source::ScrollSource(XpGainSourceScroll {
                scroll_id: "scroll:bai_cao_tu_kao_can".to_string(),
                xp_grant: 500,
            })),
            source_realm_breakthrough: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SkillXpGain::decode(bytes.as_slice()).expect("SkillXpGain scroll decode 失败");
        match decoded.source {
            Some(skill_xp_gain::Source::ScrollSource(s)) => {
                assert_eq!(s.scroll_id, "scroll:bai_cao_tu_kao_can");
                assert_eq!(s.xp_grant, 500);
            }
            other => panic!("期望 SourceScroll，实际 {other:?}"),
        }
    }

    #[test]
    fn skill_xp_gain_source_realm_breakthrough_roundtrip() {
        let msg = SkillXpGain {
            char_id: 3003,
            skill: SkillId::Cultivation as i32,
            amount: 0,
            source: None,
            source_realm_breakthrough: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = SkillXpGain::decode(bytes.as_slice())
            .expect("SkillXpGain realm_breakthrough decode 失败");
        assert!(
            decoded.source_realm_breakthrough,
            "source_realm_breakthrough 应为 true"
        );
        assert!(
            decoded.source.is_none(),
            "realm_breakthrough 时 source oneof 应为 None"
        );
    }

    #[test]
    fn skill_xp_gain_source_mentor_roundtrip() {
        let msg = SkillXpGain {
            char_id: 4004,
            skill: SkillId::Combat as i32,
            amount: 100,
            source: Some(skill_xp_gain::Source::MentorSource(XpGainSourceMentor {
                mentor_char: 42,
            })),
            source_realm_breakthrough: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SkillXpGain::decode(bytes.as_slice()).expect("SkillXpGain mentor decode 失败");
        match decoded.source {
            Some(skill_xp_gain::Source::MentorSource(m)) => {
                assert_eq!(m.mentor_char, 42, "mentor_char 不匹配");
            }
            other => panic!("期望 SourceMentor，实际 {other:?}"),
        }
    }

    #[test]
    fn skill_xp_gain_all_skill_ids() {
        let skills = [
            SkillId::Herbalism,
            SkillId::Alchemy,
            SkillId::Forging,
            SkillId::Combat,
            SkillId::Mineral,
            SkillId::Cultivation,
        ];
        for skill in skills {
            let msg = SkillXpGain {
                char_id: 1,
                skill: skill as i32,
                amount: 1,
                source: None,
                source_realm_breakthrough: false,
            };
            let bytes = msg.encode_to_vec();
            let decoded = SkillXpGain::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("SkillXpGain skill={skill:?} decode 失败: {e}"));
            assert_eq!(
                decoded.skill, skill as i32,
                "skill roundtrip 不匹配: 期望 {skill:?}"
            );
        }
    }

    #[test]
    fn skill_xp_gain_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SkillXpGain(SkillXpGain {
                char_id: 99,
                skill: SkillId::Forging as i32,
                amount: 25,
                source: Some(skill_xp_gain::Source::ActionSource(XpGainSourceAction {
                    plan_id: "forge".to_string(),
                    action: "hammer".to_string(),
                })),
                source_realm_breakthrough: false,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SkillXpGain envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SkillXpGain(s)) => {
                assert_eq!(s.skill, SkillId::Forging as i32);
                assert_eq!(s.amount, 25);
            }
            other => panic!("期望 SkillXpGain payload，实际 {other:?}"),
        }
    }

    // ─── 8. InventorySnapshot — 重型嵌套 ────────────────────────

    #[test]
    fn inventory_item_view_full_roundtrip() {
        let item = InventoryItemView {
            instance_id: 42,
            item_id: "qing_feng_sword".to_string(),
            display_name: "青锋剑".to_string(),
            grid_width: 1,
            grid_height: 2,
            weight: 2.5,
            rarity: "rare".to_string(),
            description: "炼成之剑".to_string(),
            stack_count: 1,
            spirit_quality: 1.0,
            durability: 0.95,
            mineral_id: Some("qing_gang".to_string()),
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
            charges: Some(3),
            forge_quality: Some(0.98),
            forge_color: Some(ColorKind::Sharp as i32),
            forge_side_effects: vec!["brittle_edge".to_string()],
            forge_achieved_tier: Some(2),
        };
        let bytes = item.encode_to_vec();
        let decoded = InventoryItemView::decode(bytes.as_slice())
            .expect("InventoryItemView full decode 失败");
        assert_eq!(decoded.instance_id, 42);
        assert_eq!(decoded.item_id, "qing_feng_sword");
        assert_eq!(decoded.grid_width, 1);
        assert_eq!(decoded.grid_height, 2);
        assert_eq!(decoded.weight, 2.5);
        assert_eq!(decoded.rarity, "rare");
        assert_eq!(decoded.stack_count, 1);
        assert_eq!(decoded.spirit_quality, 1.0);
        assert_eq!(decoded.durability, 0.95);
        assert_eq!(decoded.mineral_id.as_deref(), Some("qing_gang"));
        assert_eq!(decoded.charges, Some(3));
        assert_eq!(decoded.forge_quality, Some(0.98));
        assert_eq!(decoded.forge_color, Some(ColorKind::Sharp as i32));
        assert_eq!(decoded.forge_side_effects, vec!["brittle_edge"]);
        assert_eq!(decoded.forge_achieved_tier, Some(2));
    }

    #[test]
    fn inventory_item_view_minimal_roundtrip() {
        let item = InventoryItemView {
            instance_id: 1,
            item_id: "herb".to_string(),
            display_name: "草药".to_string(),
            grid_width: 1,
            grid_height: 1,
            weight: 0.1,
            rarity: "common".to_string(),
            description: String::new(),
            stack_count: 64,
            spirit_quality: 0.0,
            durability: 1.0,
            mineral_id: None,
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: vec![],
            forge_achieved_tier: None,
        };
        let bytes = item.encode_to_vec();
        let decoded = InventoryItemView::decode(bytes.as_slice())
            .expect("InventoryItemView minimal decode 失败");
        assert_eq!(decoded.stack_count, 64);
        assert!(decoded.mineral_id.is_none());
        assert!(decoded.forge_quality.is_none());
        assert!(decoded.forge_color.is_none());
        assert!(decoded.forge_side_effects.is_empty());
    }

    #[test]
    fn inventory_snapshot_empty_roundtrip() {
        let snapshot = InventorySnapshot {
            revision: 0,
            containers: vec![ContainerSnapshot {
                id: "body_pocket".to_string(),
                name: "贴身口袋".to_string(),
                rows: 2,
                cols: 3,
            }],
            placed_items: vec![],
            equipped: Some(EquippedInventorySnapshot {
                head: None,
                chest: None,
                legs: None,
                feet: None,
                false_skin: None,
                main_hand: None,
                off_hand: None,
                two_hand: None,
                treasure_belt_0: None,
                treasure_belt_1: None,
                treasure_belt_2: None,
                treasure_belt_3: None,
                back_pack: None,
                waist_pouch: None,
                chest_satchel: None,
                extra_hand_0: None,
                extra_hand_1: None,
            }),
            hotbar: vec![], // 空 hotbar 边界
            bone_coins: 0,
            weight: Some(InventoryWeight {
                current: 0.0,
                max: 30.0,
            }),
            realm: "Awaken".to_string(),
            qi_current: 0.0,
            qi_max: 10.0,
            body_level: 1.0,
        };
        let bytes = snapshot.encode_to_vec();
        let decoded = InventorySnapshot::decode(bytes.as_slice())
            .expect("InventorySnapshot empty decode 失败");
        assert_eq!(decoded.revision, 0);
        assert_eq!(decoded.containers.len(), 1);
        assert_eq!(decoded.containers[0].id, "body_pocket");
        assert!(decoded.placed_items.is_empty());
        assert!(decoded.hotbar.is_empty());
        assert_eq!(decoded.bone_coins, 0);
        assert_eq!(decoded.realm, "Awaken");
    }

    #[test]
    fn inventory_snapshot_with_items_roundtrip() {
        let item = InventoryItemView {
            instance_id: 1001,
            item_id: "starter_talisman".to_string(),
            display_name: "启程护符".to_string(),
            grid_width: 1,
            grid_height: 1,
            weight: 0.2,
            rarity: "common".to_string(),
            description: String::new(),
            stack_count: 1,
            spirit_quality: 0.0,
            durability: 1.0,
            mineral_id: None,
            scroll_kind: None,
            scroll_skill_id: None,
            scroll_xp_grant: None,
            charges: None,
            forge_quality: None,
            forge_color: None,
            forge_side_effects: vec![],
            forge_achieved_tier: None,
        };

        let snapshot = InventorySnapshot {
            revision: 12,
            containers: vec![
                ContainerSnapshot {
                    id: "body_pocket".to_string(),
                    name: "贴身口袋".to_string(),
                    rows: 2,
                    cols: 3,
                },
                ContainerSnapshot {
                    id: "back_pack".to_string(),
                    name: "背包".to_string(),
                    rows: 4,
                    cols: 6,
                },
            ],
            placed_items: vec![PlacedInventoryItem {
                container_id: "body_pocket".to_string(),
                row: 0,
                col: 0,
                item: Some(item.clone()),
            }],
            equipped: Some(EquippedInventorySnapshot {
                head: None,
                chest: None,
                legs: None,
                feet: None,
                false_skin: None,
                main_hand: Some(item.clone()),
                off_hand: None,
                two_hand: None,
                treasure_belt_0: None,
                treasure_belt_1: None,
                treasure_belt_2: None,
                treasure_belt_3: None,
                back_pack: None,
                waist_pouch: None,
                chest_satchel: None,
                extra_hand_0: None,
                extra_hand_1: None,
            }),
            hotbar: (0..9)
                .map(|i| HotbarSlot {
                    item: if i == 0 { Some(item.clone()) } else { None },
                })
                .collect(),
            bone_coins: 57,
            weight: Some(InventoryWeight {
                current: 0.2,
                max: 30.0,
            }),
            realm: "Awaken".to_string(),
            qi_current: 5.0,
            qi_max: 10.0,
            body_level: 1.0,
        };
        let bytes = snapshot.encode_to_vec();
        let decoded = InventorySnapshot::decode(bytes.as_slice())
            .expect("InventorySnapshot with items decode 失败");
        assert_eq!(decoded.revision, 12);
        assert_eq!(decoded.containers.len(), 2);
        assert_eq!(decoded.placed_items.len(), 1);
        assert_eq!(
            decoded.placed_items[0].item.as_ref().unwrap().item_id,
            "starter_talisman"
        );
        assert_eq!(decoded.hotbar.len(), 9, "hotbar 应有 9 槽");
        assert!(decoded.hotbar[0].item.is_some(), "hotbar[0] 应有物品");
        assert!(decoded.hotbar[1].item.is_none(), "hotbar[1] 应为空");
        assert_eq!(decoded.bone_coins, 57);
        assert_eq!(decoded.weight.as_ref().unwrap().current, 0.2);
        let equipped = decoded.equipped.expect("equipped 应存在");
        assert!(equipped.main_hand.is_some(), "main_hand 应有物品");
        assert!(equipped.head.is_none(), "head 应为 None");
    }

    #[test]
    fn inventory_snapshot_envelope_roundtrip() {
        let snapshot = InventorySnapshot {
            revision: 1,
            containers: vec![ContainerSnapshot {
                id: "body_pocket".to_string(),
                name: "贴身口袋".to_string(),
                rows: 2,
                cols: 3,
            }],
            placed_items: vec![],
            equipped: None,
            hotbar: vec![],
            bone_coins: 100,
            weight: None,
            realm: "Induce".to_string(),
            qi_current: 0.0,
            qi_max: 0.0,
            body_level: 0.0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::InventorySnapshot(snapshot)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("InventorySnapshot envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::InventorySnapshot(s)) => {
                assert_eq!(s.bone_coins, 100);
                assert_eq!(s.realm, "Induce");
            }
            other => panic!("期望 InventorySnapshot payload，实际 {other:?}"),
        }
    }

    #[test]
    fn inventory_item_view_all_rarity_values() {
        let rarities = ["common", "uncommon", "rare", "epic", "legendary", "ancient"];
        for rarity in rarities {
            let item = InventoryItemView {
                instance_id: 1,
                item_id: "x".to_string(),
                display_name: "x".to_string(),
                grid_width: 1,
                grid_height: 1,
                weight: 0.0,
                rarity: rarity.to_string(),
                description: String::new(),
                stack_count: 1,
                spirit_quality: 0.0,
                durability: 1.0,
                mineral_id: None,
                scroll_kind: None,
                scroll_skill_id: None,
                scroll_xp_grant: None,
                charges: None,
                forge_quality: None,
                forge_color: None,
                forge_side_effects: vec![],
                forge_achieved_tier: None,
            };
            let bytes = item.encode_to_vec();
            let decoded = InventoryItemView::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("rarity={rarity} decode 失败: {e}"));
            assert_eq!(
                decoded.rarity, rarity,
                "rarity roundtrip 不匹配: 期望 {rarity}"
            );
        }
    }

    // ─── 9. CombatHudState ──────────────────────────────────────

    #[test]
    fn combat_hud_state_full_roundtrip() {
        let state = CombatHudState {
            hp_percent: 0.85,
            qi_percent: 0.42,
            stamina_percent: 0.91,
            derived: Some(DerivedAttrFlags {
                flying: true,
                phasing: false,
                tribulation_locked: false,
            }),
        };
        let bytes = state.encode_to_vec();
        let decoded = CombatHudState::decode(bytes.as_slice()).expect("CombatHudState decode 失败");
        assert_eq!(decoded.hp_percent, 0.85, "hp_percent 不匹配");
        assert_eq!(decoded.qi_percent, 0.42, "qi_percent 不匹配");
        assert_eq!(decoded.stamina_percent, 0.91, "stamina_percent 不匹配");
        let derived = decoded.derived.expect("derived 应存在");
        assert!(derived.flying, "flying 应为 true");
        assert!(!derived.phasing, "phasing 应为 false");
        assert!(!derived.tribulation_locked, "tribulation_locked 应为 false");
    }

    #[test]
    fn combat_hud_state_zeros_roundtrip() {
        let state = CombatHudState {
            hp_percent: 0.0,
            qi_percent: 0.0,
            stamina_percent: 0.0,
            derived: Some(DerivedAttrFlags {
                flying: false,
                phasing: false,
                tribulation_locked: false,
            }),
        };
        let bytes = state.encode_to_vec();
        let decoded =
            CombatHudState::decode(bytes.as_slice()).expect("CombatHudState zeros decode 失败");
        // proto3 default: float 0.0, bool false 都是默认值，不写入 wire。
        assert_eq!(decoded.hp_percent, 0.0);
        assert_eq!(decoded.qi_percent, 0.0);
        assert_eq!(decoded.stamina_percent, 0.0);
    }

    #[test]
    fn combat_hud_state_max_values() {
        let state = CombatHudState {
            hp_percent: 1.0,
            qi_percent: 1.0,
            stamina_percent: 1.0,
            derived: Some(DerivedAttrFlags {
                flying: true,
                phasing: true,
                tribulation_locked: true,
            }),
        };
        let bytes = state.encode_to_vec();
        let decoded =
            CombatHudState::decode(bytes.as_slice()).expect("CombatHudState max decode 失败");
        assert_eq!(decoded.hp_percent, 1.0);
        let derived = decoded.derived.unwrap();
        assert!(derived.flying);
        assert!(derived.phasing);
        assert!(derived.tribulation_locked);
    }

    #[test]
    fn combat_hud_state_without_derived() {
        let state = CombatHudState {
            hp_percent: 0.5,
            qi_percent: 0.5,
            stamina_percent: 0.5,
            derived: None,
        };
        let bytes = state.encode_to_vec();
        let decoded = CombatHudState::decode(bytes.as_slice())
            .expect("CombatHudState without derived decode 失败");
        assert!(decoded.derived.is_none(), "derived 应为 None");
    }

    #[test]
    fn combat_hud_state_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CombatHudState(
                CombatHudState {
                    hp_percent: 0.75,
                    qi_percent: 0.50,
                    stamina_percent: 1.0,
                    derived: Some(DerivedAttrFlags {
                        flying: false,
                        phasing: true,
                        tribulation_locked: false,
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("CombatHudState envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::CombatHudState(c)) => {
                assert_eq!(c.hp_percent, 0.75);
                assert!(c.derived.unwrap().phasing);
            }
            other => panic!("期望 CombatHudState payload，实际 {other:?}"),
        }
    }

    // ─── 10. KnockbackSync ──────────────────────────────────────

    #[test]
    fn knockback_sync_with_collision_damage() {
        let sync = KnockbackSync {
            distance_blocks: 3.5,
            velocity_blocks_per_tick: 0.175,
            duration_ticks: 20,
            kinetic_energy: 245.0,
            collision_damage: Some(12.5),
            chain_depth: 2,
            block_broken: true,
        };
        let bytes = sync.encode_to_vec();
        let decoded = KnockbackSync::decode(bytes.as_slice())
            .expect("KnockbackSync with collision decode 失败");
        assert_eq!(decoded.distance_blocks, 3.5, "distance_blocks 不匹配");
        assert_eq!(decoded.velocity_blocks_per_tick, 0.175, "velocity 不匹配");
        assert_eq!(decoded.duration_ticks, 20, "duration_ticks 不匹配");
        assert_eq!(decoded.kinetic_energy, 245.0, "kinetic_energy 不匹配");
        assert_eq!(
            decoded.collision_damage,
            Some(12.5),
            "collision_damage 应为 Some(12.5)，实际 {:?}",
            decoded.collision_damage
        );
        assert_eq!(decoded.chain_depth, 2, "chain_depth 不匹配");
        assert!(decoded.block_broken, "block_broken 应为 true");
    }

    #[test]
    fn knockback_sync_without_collision_damage() {
        let sync = KnockbackSync {
            distance_blocks: 1.0,
            velocity_blocks_per_tick: 0.05,
            duration_ticks: 10,
            kinetic_energy: 20.0,
            collision_damage: None,
            chain_depth: 0,
            block_broken: false,
        };
        let bytes = sync.encode_to_vec();
        let decoded = KnockbackSync::decode(bytes.as_slice())
            .expect("KnockbackSync without collision decode 失败");
        assert!(
            decoded.collision_damage.is_none(),
            "collision_damage 应为 None，实际 {:?}",
            decoded.collision_damage
        );
        assert_eq!(decoded.chain_depth, 0);
        assert!(!decoded.block_broken);
    }

    #[test]
    fn knockback_sync_boundary_values() {
        // 极端值：最大距离、最大能量、chain_depth max for u8
        let sync = KnockbackSync {
            distance_blocks: f64::MAX,
            velocity_blocks_per_tick: 0.0,
            duration_ticks: u32::MAX,
            kinetic_energy: f64::MAX,
            collision_damage: Some(f32::MAX),
            chain_depth: 255, // u8 max, fits in uint32
            block_broken: false,
        };
        let bytes = sync.encode_to_vec();
        let decoded =
            KnockbackSync::decode(bytes.as_slice()).expect("KnockbackSync boundary decode 失败");
        assert_eq!(decoded.distance_blocks, f64::MAX);
        assert_eq!(decoded.duration_ticks, u32::MAX);
        assert_eq!(decoded.chain_depth, 255);
    }

    #[test]
    fn knockback_sync_zero_values() {
        let sync = KnockbackSync {
            distance_blocks: 0.0,
            velocity_blocks_per_tick: 0.0,
            duration_ticks: 0,
            kinetic_energy: 0.0,
            collision_damage: None,
            chain_depth: 0,
            block_broken: false,
        };
        let bytes = sync.encode_to_vec();
        // Proto3: 全默认值 → wire bytes 极少（但仍可 roundtrip）
        let decoded =
            KnockbackSync::decode(bytes.as_slice()).expect("KnockbackSync zeros decode 失败");
        assert_eq!(decoded.distance_blocks, 0.0);
        assert_eq!(decoded.kinetic_energy, 0.0);
    }

    #[test]
    fn knockback_sync_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::KnockbackSync(
                KnockbackSync {
                    distance_blocks: 5.0,
                    velocity_blocks_per_tick: 0.25,
                    duration_ticks: 20,
                    kinetic_energy: 500.0,
                    collision_damage: Some(25.0),
                    chain_depth: 1,
                    block_broken: true,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("KnockbackSync envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::KnockbackSync(k)) => {
                assert_eq!(k.distance_blocks, 5.0);
                assert_eq!(k.collision_damage, Some(25.0));
                assert!(k.block_broken);
            }
            other => panic!("期望 KnockbackSync payload，实际 {other:?}"),
        }
    }

    // ─── 错误分支 / malformed decode for new messages ───────────

    #[test]
    fn skill_xp_gain_rejects_malformed_bytes() {
        let bad = vec![0x0a, 0xff, 0xff, 0xff];
        let result = SkillXpGain::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "malformed SkillXpGain 应 decode 失败，实际 {result:?}"
        );
    }

    #[test]
    fn inventory_snapshot_rejects_malformed_bytes() {
        let bad = vec![0x0a, 0xff, 0xff, 0xff];
        let result = InventorySnapshot::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "malformed InventorySnapshot 应 decode 失败，实际 {result:?}"
        );
    }

    #[test]
    fn combat_hud_state_rejects_malformed_bytes() {
        let bad = vec![0x0a, 0xff, 0xff, 0xff];
        let result = CombatHudState::decode(bad.as_slice());
        assert!(
            result.is_err(),
            "malformed CombatHudState 应 decode 失败，实际 {result:?}"
        );
    }

    #[test]
    fn knockback_sync_rejects_truncated_bytes() {
        let valid = KnockbackSync {
            distance_blocks: 3.0,
            velocity_blocks_per_tick: 0.15,
            duration_ticks: 20,
            kinetic_energy: 100.0,
            collision_damage: Some(10.0),
            chain_depth: 1,
            block_broken: true,
        };
        let mut bytes = valid.encode_to_vec();
        bytes.truncate(bytes.len() / 2);
        let result = KnockbackSync::decode(bytes.as_slice());
        assert!(
            result.is_err(),
            "截断的 KnockbackSync bytes 应 decode 失败，实际 {result:?}"
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 炼丹 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn alchemy_furnace_roundtrip() {
        let msg = AlchemyFurnace {
            pos_x: Some(-12),
            pos_y: Some(64),
            pos_z: Some(38),
            tier: 2,
            integrity: 0.95,
            integrity_max: 1.0,
            owner_name: "Azure".to_string(),
            has_session: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyFurnace::decode(bytes.as_slice()).expect("AlchemyFurnace decode 失败");
        assert_eq!(decoded.pos_x, Some(-12), "pos_x 应为 -12");
        assert_eq!(decoded.pos_y, Some(64), "pos_y 应为 64");
        assert_eq!(decoded.pos_z, Some(38), "pos_z 应为 38");
        assert_eq!(decoded.tier, 2, "tier 应为 2");
        assert!(
            (decoded.integrity - 0.95).abs() < 1e-9,
            "integrity 应为 0.95"
        );
        assert!(
            (decoded.integrity_max - 1.0).abs() < 1e-9,
            "integrity_max 应为 1.0"
        );
        assert_eq!(decoded.owner_name, "Azure");
        assert!(decoded.has_session, "has_session 应为 true");
    }

    #[test]
    fn alchemy_furnace_no_pos_roundtrip() {
        let msg = AlchemyFurnace {
            pos_x: None,
            pos_y: None,
            pos_z: None,
            tier: 1,
            integrity: 0.5,
            integrity_max: 1.0,
            owner_name: "".to_string(),
            has_session: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyFurnace::decode(bytes.as_slice()).expect("AlchemyFurnace (no pos) decode 失败");
        assert!(decoded.pos_x.is_none(), "pos_x 应为 None");
        assert!(decoded.pos_y.is_none(), "pos_y 应为 None");
        assert!(decoded.pos_z.is_none(), "pos_z 应为 None");
    }

    #[test]
    fn alchemy_session_roundtrip() {
        let msg = AlchemySession {
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            active: true,
            elapsed_ticks: 80,
            target_ticks: 200,
            temp_current: 320.0,
            temp_target: 350.0,
            temp_band: 30.0,
            qi_injected: 5.0,
            qi_target: 10.0,
            status_label: "heating".to_string(),
            stages: vec![AlchemyStageHint {
                at_tick: 80,
                window: 20,
                summary: "hui_yuan_zhi x1".to_string(),
                completed: false,
                missed: false,
            }],
            interventions_recent: vec!["inject_qi".to_string()],
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemySession::decode(bytes.as_slice()).expect("AlchemySession decode 失败");
        assert_eq!(decoded.recipe_id.as_deref(), Some("kai_mai_pill_v0"));
        assert!(decoded.active);
        assert_eq!(decoded.elapsed_ticks, 80);
        assert_eq!(decoded.stages.len(), 1, "应有 1 个 stage hint");
        assert_eq!(decoded.stages[0].at_tick, 80);
        assert_eq!(decoded.interventions_recent.len(), 1);
    }

    #[test]
    fn alchemy_session_empty_stages_roundtrip() {
        let msg = AlchemySession {
            recipe_id: None,
            active: false,
            elapsed_ticks: 0,
            target_ticks: 0,
            temp_current: 0.0,
            temp_target: 0.0,
            temp_band: 0.0,
            qi_injected: 0.0,
            qi_target: 0.0,
            status_label: "".to_string(),
            stages: vec![],
            interventions_recent: vec![],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemySession::decode(bytes.as_slice()).expect("AlchemySession (empty) decode 失败");
        assert!(decoded.recipe_id.is_none());
        assert!(decoded.stages.is_empty());
        assert!(decoded.interventions_recent.is_empty());
    }

    #[test]
    fn alchemy_outcome_forecast_roundtrip() {
        let msg = AlchemyOutcomeForecast {
            perfect_pct: 0.1,
            good_pct: 0.3,
            flawed_pct: 0.3,
            waste_pct: 0.2,
            explode_pct: 0.1,
            perfect_note: "温度精准".to_string(),
            good_note: "偏差可控".to_string(),
            flawed_note: "杂质较多".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyOutcomeForecast::decode(bytes.as_slice())
            .expect("AlchemyOutcomeForecast decode 失败");
        assert!((decoded.perfect_pct - 0.1).abs() < 1e-9);
        assert_eq!(decoded.perfect_note, "温度精准");
    }

    #[test]
    fn alchemy_outcome_bucket_enum_pin() {
        let expected = [
            (AlchemyOutcomeBucket::Unspecified, 0),
            (AlchemyOutcomeBucket::Perfect, 1),
            (AlchemyOutcomeBucket::Good, 2),
            (AlchemyOutcomeBucket::Flawed, 3),
            (AlchemyOutcomeBucket::Waste, 4),
            (AlchemyOutcomeBucket::Explode, 5),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "AlchemyOutcomeBucket::{:?} wire 值应为 {wire}",
                variant
            );
        }
    }

    #[test]
    fn alchemy_outcome_resolved_full_roundtrip() {
        let msg = AlchemyOutcomeResolved {
            bucket: AlchemyOutcomeBucket::Perfect as i32,
            recipe_id: Some("kai_mai_pill_v0".to_string()),
            pill: Some("kai_mai_pill".to_string()),
            quality: Some(0.95),
            toxin_amount: Some(0.02),
            toxin_color: Some(ColorKind::Insidious as i32),
            qi_gain: Some(5.0),
            side_effect_tag: Some("minor_burn".to_string()),
            flawed_path: false,
            damage: None,
            meridian_crack: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyOutcomeResolved::decode(bytes.as_slice())
            .expect("AlchemyOutcomeResolved decode 失败");
        assert_eq!(decoded.bucket, AlchemyOutcomeBucket::Perfect as i32);
        assert_eq!(decoded.pill.as_deref(), Some("kai_mai_pill"));
        assert_eq!(decoded.toxin_color, Some(ColorKind::Insidious as i32));
        assert!(!decoded.flawed_path);
        assert!(decoded.damage.is_none(), "damage 应为 None");
        assert!(decoded.meridian_crack.is_none(), "meridian_crack 应为 None");
    }

    #[test]
    fn alchemy_outcome_resolved_explode_with_damage_roundtrip() {
        let msg = AlchemyOutcomeResolved {
            bucket: AlchemyOutcomeBucket::Explode as i32,
            recipe_id: None,
            pill: None,
            quality: None,
            toxin_amount: None,
            toxin_color: None,
            qi_gain: None,
            side_effect_tag: None,
            flawed_path: true,
            damage: Some(12.0),
            meridian_crack: Some(0.3),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyOutcomeResolved::decode(bytes.as_slice())
            .expect("AlchemyOutcomeResolved (explode) decode 失败");
        assert_eq!(decoded.bucket, AlchemyOutcomeBucket::Explode as i32);
        assert!(decoded.recipe_id.is_none());
        assert!(decoded.flawed_path);
        assert_eq!(decoded.damage, Some(12.0));
        assert!((decoded.meridian_crack.unwrap() - 0.3).abs() < 1e-9);
    }

    #[test]
    fn alchemy_recipe_book_roundtrip() {
        let msg = AlchemyRecipeBook {
            learned: vec![AlchemyRecipeEntry {
                id: "kai_mai_pill_v0".to_string(),
                display_name: "开脉丹方".to_string(),
                body_text: "...".to_string(),
                author: "散修 刘三".to_string(),
                era: "末法 十二年".to_string(),
                max_known: 8,
            }],
            current_index: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyRecipeBook::decode(bytes.as_slice()).expect("AlchemyRecipeBook decode 失败");
        assert_eq!(decoded.learned.len(), 1);
        assert_eq!(decoded.learned[0].id, "kai_mai_pill_v0");
        assert_eq!(decoded.learned[0].max_known, 8);
        assert_eq!(decoded.current_index, 0);
    }

    #[test]
    fn alchemy_recipe_book_empty_roundtrip() {
        let msg = AlchemyRecipeBook {
            learned: vec![],
            current_index: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyRecipeBook::decode(bytes.as_slice())
            .expect("AlchemyRecipeBook (empty) decode 失败");
        assert!(decoded.learned.is_empty());
    }

    #[test]
    fn alchemy_contamination_roundtrip() {
        let msg = AlchemyContamination {
            levels: vec![AlchemyContaminationLevel {
                color: ColorKind::Mellow as i32,
                current: 0.18,
                max: 0.6,
                ok: true,
            }],
            metabolism_note: "代谢正常".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyContamination::decode(bytes.as_slice())
            .expect("AlchemyContamination decode 失败");
        assert_eq!(decoded.levels.len(), 1);
        assert_eq!(decoded.levels[0].color, ColorKind::Mellow as i32);
        assert!(decoded.levels[0].ok);
        assert_eq!(decoded.metabolism_note, "代谢正常");
    }

    // ─── 炼丹 C2S roundtrip ─────────────────────────────────────

    #[test]
    fn alchemy_open_furnace_roundtrip() {
        let msg = AlchemyOpenFurnace {
            furnace_pos_x: -12,
            furnace_pos_y: 64,
            furnace_pos_z: 38,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyOpenFurnace::decode(bytes.as_slice()).expect("AlchemyOpenFurnace decode 失败");
        assert_eq!(decoded.furnace_pos_x, -12);
        assert_eq!(decoded.furnace_pos_y, 64);
        assert_eq!(decoded.furnace_pos_z, 38);
    }

    #[test]
    fn alchemy_feed_slot_roundtrip() {
        let msg = AlchemyFeedSlot {
            furnace_pos_x: -12,
            furnace_pos_y: 64,
            furnace_pos_z: 38,
            slot_idx: 0,
            material: "ci_she_hao".to_string(),
            count: 3,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyFeedSlot::decode(bytes.as_slice()).expect("AlchemyFeedSlot decode 失败");
        assert_eq!(decoded.slot_idx, 0);
        assert_eq!(decoded.material, "ci_she_hao");
        assert_eq!(decoded.count, 3);
    }

    #[test]
    fn alchemy_take_back_roundtrip() {
        let msg = AlchemyTakeBack {
            furnace_pos_x: -12,
            furnace_pos_y: 64,
            furnace_pos_z: 38,
            slot_idx: 2,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyTakeBack::decode(bytes.as_slice()).expect("AlchemyTakeBack decode 失败");
        assert_eq!(decoded.slot_idx, 2);
    }

    #[test]
    fn alchemy_ignite_roundtrip() {
        let msg = AlchemyIgnite {
            furnace_pos_x: -12,
            furnace_pos_y: 64,
            furnace_pos_z: 38,
            recipe_id: "kai_mai_pill_v0".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyIgnite::decode(bytes.as_slice()).expect("AlchemyIgnite decode 失败");
        assert_eq!(decoded.recipe_id, "kai_mai_pill_v0");
    }

    #[test]
    fn alchemy_intervention_adjust_temp_roundtrip() {
        let msg = AlchemyIntervention {
            furnace_pos_x: -12,
            furnace_pos_y: 64,
            furnace_pos_z: 38,
            intervention: Some(AlchemyInterventionProto {
                kind: Some(alchemy_intervention_proto::Kind::AdjustTemp(0.6)),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyIntervention::decode(bytes.as_slice())
            .expect("AlchemyIntervention (adjust_temp) decode 失败");
        match decoded.intervention.unwrap().kind.unwrap() {
            alchemy_intervention_proto::Kind::AdjustTemp(t) => {
                assert!((t - 0.6).abs() < 1e-9, "temp 应为 0.6，实际 {t}");
            }
            other => panic!("期望 AdjustTemp，实际 {other:?}"),
        }
    }

    #[test]
    fn alchemy_intervention_inject_qi_proto_roundtrip() {
        let msg = AlchemyIntervention {
            furnace_pos_x: 0,
            furnace_pos_y: 0,
            furnace_pos_z: 0,
            intervention: Some(AlchemyInterventionProto {
                kind: Some(alchemy_intervention_proto::Kind::InjectQi(1.5)),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyIntervention::decode(bytes.as_slice())
            .expect("AlchemyIntervention (inject_qi) decode 失败");
        match decoded.intervention.unwrap().kind.unwrap() {
            alchemy_intervention_proto::Kind::InjectQi(q) => {
                assert!((q - 1.5).abs() < 1e-9, "qi 应为 1.5，实际 {q}");
            }
            other => panic!("期望 InjectQi，实际 {other:?}"),
        }
    }

    #[test]
    fn alchemy_intervention_auto_profile_proto_roundtrip() {
        let msg = AlchemyIntervention {
            furnace_pos_x: 0,
            furnace_pos_y: 0,
            furnace_pos_z: 0,
            intervention: Some(AlchemyInterventionProto {
                kind: Some(alchemy_intervention_proto::Kind::AutoProfileId(
                    "kai_mai_safe".to_string(),
                )),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyIntervention::decode(bytes.as_slice())
            .expect("AlchemyIntervention (auto_profile) decode 失败");
        match decoded.intervention.unwrap().kind.unwrap() {
            alchemy_intervention_proto::Kind::AutoProfileId(id) => {
                assert_eq!(id, "kai_mai_safe");
            }
            other => panic!("期望 AutoProfileId，实际 {other:?}"),
        }
    }

    #[test]
    fn alchemy_turn_page_roundtrip() {
        let msg = AlchemyTurnPage { delta: -1 };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyTurnPage::decode(bytes.as_slice()).expect("AlchemyTurnPage decode 失败");
        assert_eq!(decoded.delta, -1);
    }

    #[test]
    fn alchemy_learn_recipe_roundtrip() {
        let msg = AlchemyLearnRecipe {
            recipe_id: "kai_mai_pill_v0".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyLearnRecipe::decode(bytes.as_slice()).expect("AlchemyLearnRecipe decode 失败");
        assert_eq!(decoded.recipe_id, "kai_mai_pill_v0");
    }

    #[test]
    fn alchemy_learn_recipe_fragment_roundtrip() {
        let msg = AlchemyLearnRecipeFragment {
            item_instance_id: 4242,
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyLearnRecipeFragment::decode(bytes.as_slice())
            .expect("AlchemyLearnRecipeFragment decode 失败");
        assert_eq!(decoded.item_instance_id, 4242);
    }

    #[test]
    fn alchemy_take_pill_roundtrip() {
        let msg = AlchemyTakePill {
            pill_item_id: "kai_mai_pill".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            AlchemyTakePill::decode(bytes.as_slice()).expect("AlchemyTakePill decode 失败");
        assert_eq!(decoded.pill_item_id, "kai_mai_pill");
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 锻造 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn forge_station_roundtrip() {
        let msg = ForgeStation {
            station_id: "s1".to_string(),
            tier: 1,
            integrity: 0.95,
            owner_name: "test".to_string(),
            has_session: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeStation::decode(bytes.as_slice()).expect("ForgeStation decode 失败");
        assert_eq!(decoded.station_id, "s1");
        assert_eq!(decoded.tier, 1);
        assert!(!decoded.has_session);
    }

    #[test]
    fn forge_step_enum_pin() {
        let expected = [
            (ForgeStep::Unspecified, 0),
            (ForgeStep::Billet, 1),
            (ForgeStep::Tempering, 2),
            (ForgeStep::Inscription, 3),
            (ForgeStep::Consecration, 4),
            (ForgeStep::Done, 5),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "ForgeStep::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn temper_beat_enum_pin() {
        let expected = [
            (TemperBeat::Unspecified, 0),
            (TemperBeat::Light, 1),
            (TemperBeat::Heavy, 2),
            (TemperBeat::Fold, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "TemperBeat::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn forge_outcome_bucket_enum_pin() {
        let expected = [
            (ForgeOutcomeBucket::Unspecified, 0),
            (ForgeOutcomeBucket::Perfect, 1),
            (ForgeOutcomeBucket::Good, 2),
            (ForgeOutcomeBucket::Flawed, 3),
            (ForgeOutcomeBucket::Waste, 4),
            (ForgeOutcomeBucket::Explode, 5),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "ForgeOutcomeBucket::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn forge_session_data_billet_roundtrip() {
        let msg = ForgeSessionData {
            session_id: 42,
            blueprint_id: "iron_sword_v0".to_string(),
            blueprint_name: "铁剑".to_string(),
            active: true,
            current_step: ForgeStep::Billet as i32,
            step_index: 0,
            achieved_tier: 1,
            step_state: Some(ForgeStepState {
                state: Some(forge_step_state::State::Billet(ForgeStepStateBillet {
                    materials_in: vec![ForgeMaterialPair {
                        material: "iron_ingot".to_string(),
                        count: 3,
                    }],
                    active_carrier: None,
                    resolved_tier_cap: 1,
                })),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeSessionData::decode(bytes.as_slice())
            .expect("ForgeSessionData (billet) decode 失败");
        assert_eq!(decoded.session_id, 42);
        assert_eq!(decoded.blueprint_id, "iron_sword_v0");
        assert_eq!(decoded.current_step, ForgeStep::Billet as i32);
        match decoded.step_state.unwrap().state.unwrap() {
            forge_step_state::State::Billet(b) => {
                assert_eq!(b.materials_in.len(), 1);
                assert_eq!(b.materials_in[0].material, "iron_ingot");
                assert_eq!(b.materials_in[0].count, 3);
                assert!(b.active_carrier.is_none());
                assert_eq!(b.resolved_tier_cap, 1);
            }
            other => panic!("期望 Billet state，实际 {other:?}"),
        }
    }

    #[test]
    fn forge_session_data_tempering_roundtrip() {
        let msg = ForgeSessionData {
            session_id: 43,
            blueprint_id: "qing_feng_v0".to_string(),
            blueprint_name: "清风".to_string(),
            active: true,
            current_step: ForgeStep::Tempering as i32,
            step_index: 1,
            achieved_tier: 2,
            step_state: Some(ForgeStepState {
                state: Some(forge_step_state::State::Tempering(
                    ForgeStepStateTempering {
                        pattern: vec![
                            TemperBeat::Light as i32,
                            TemperBeat::Heavy as i32,
                            TemperBeat::Fold as i32,
                        ],
                        beat_cursor: 0,
                        hits: 0,
                        misses: 0,
                        deviation: 0,
                        qi_spent: 0.0,
                    },
                )),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeSessionData::decode(bytes.as_slice())
            .expect("ForgeSessionData (tempering) decode 失败");
        match decoded.step_state.unwrap().state.unwrap() {
            forge_step_state::State::Tempering(t) => {
                assert_eq!(t.pattern.len(), 3);
                assert_eq!(t.pattern[0], TemperBeat::Light as i32);
                assert_eq!(t.pattern[1], TemperBeat::Heavy as i32);
                assert_eq!(t.pattern[2], TemperBeat::Fold as i32);
            }
            other => panic!("期望 Tempering state，实际 {other:?}"),
        }
    }

    #[test]
    fn forge_session_data_consecration_with_color_roundtrip() {
        let msg = ForgeSessionData {
            session_id: 44,
            blueprint_id: "x".to_string(),
            blueprint_name: "x".to_string(),
            active: true,
            current_step: ForgeStep::Consecration as i32,
            step_index: 3,
            achieved_tier: 3,
            step_state: Some(ForgeStepState {
                state: Some(forge_step_state::State::Consecration(
                    ForgeStepStateConsecration {
                        qi_injected: 50.0,
                        qi_required: 100.0,
                        color_imprint: Some(ColorKind::Sharp as i32),
                    },
                )),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeSessionData::decode(bytes.as_slice())
            .expect("ForgeSessionData (consecration) decode 失败");
        match decoded.step_state.unwrap().state.unwrap() {
            forge_step_state::State::Consecration(c) => {
                assert!((c.qi_injected - 50.0).abs() < 1e-9);
                assert_eq!(c.color_imprint, Some(ColorKind::Sharp as i32));
            }
            other => panic!("期望 Consecration state，实际 {other:?}"),
        }
    }

    #[test]
    fn forge_session_data_none_state_roundtrip() {
        let msg = ForgeSessionData {
            session_id: 45,
            blueprint_id: "x".to_string(),
            blueprint_name: "x".to_string(),
            active: false,
            current_step: ForgeStep::Done as i32,
            step_index: 4,
            achieved_tier: 1,
            step_state: Some(ForgeStepState {
                state: Some(forge_step_state::State::NoneState(true)),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeSessionData::decode(bytes.as_slice())
            .expect("ForgeSessionData (none) decode 失败");
        match decoded.step_state.unwrap().state.unwrap() {
            forge_step_state::State::NoneState(v) => assert!(v),
            other => panic!("期望 NoneState，实际 {other:?}"),
        }
    }

    #[test]
    fn forge_session_data_inscription_step_roundtrip() {
        let msg = ForgeSessionData {
            session_id: 50,
            blueprint_id: "dao_v0".to_string(),
            blueprint_name: "铸刀".to_string(),
            active: true,
            current_step: ForgeStep::Inscription as i32,
            step_index: 2,
            achieved_tier: 0,
            step_state: Some(ForgeStepState {
                state: Some(forge_step_state::State::Inscription(
                    ForgeStepStateInscription {
                        filled_slots: 3,
                        max_slots: 5,
                        failed: false,
                    },
                )),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeSessionData::decode(bytes.as_slice())
            .expect("ForgeSessionData (inscription) decode 失败");
        match decoded.step_state.unwrap().state.unwrap() {
            forge_step_state::State::Inscription(i) => {
                assert_eq!(i.filled_slots, 3, "filled_slots 应为 3");
                assert_eq!(i.max_slots, 5, "max_slots 应为 5");
                assert!(!i.failed, "failed 应为 false");
            }
            other => panic!("期望 Inscription，实际 {other:?}"),
        }
    }

    #[test]
    fn forge_outcome_roundtrip() {
        let msg = ForgeOutcome {
            session_id: 1,
            blueprint_id: "iron_sword_v0".to_string(),
            bucket: ForgeOutcomeBucket::Perfect as i32,
            weapon_item: Some("iron_sword".to_string()),
            quality: 1.0,
            color: None,
            side_effects: vec![],
            achieved_tier: 1,
            flawed_path: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeOutcome::decode(bytes.as_slice()).expect("ForgeOutcome decode 失败");
        assert_eq!(decoded.session_id, 1);
        assert_eq!(decoded.bucket, ForgeOutcomeBucket::Perfect as i32);
        assert_eq!(decoded.weapon_item.as_deref(), Some("iron_sword"));
        assert!(!decoded.flawed_path);
        assert!(decoded.color.is_none());
    }

    #[test]
    fn forge_outcome_flawed_with_color_roundtrip() {
        let msg = ForgeOutcome {
            session_id: 2,
            blueprint_id: "qing_feng_v0".to_string(),
            bucket: ForgeOutcomeBucket::Flawed as i32,
            weapon_item: None,
            quality: 0.3,
            color: Some(ColorKind::Heavy as i32),
            side_effects: vec!["brittle".to_string()],
            achieved_tier: 1,
            flawed_path: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeOutcome::decode(bytes.as_slice()).expect("ForgeOutcome (flawed) decode 失败");
        assert_eq!(decoded.bucket, ForgeOutcomeBucket::Flawed as i32);
        assert!(decoded.weapon_item.is_none());
        assert_eq!(decoded.color, Some(ColorKind::Heavy as i32));
        assert_eq!(decoded.side_effects, vec!["brittle"]);
        assert!(decoded.flawed_path);
    }

    #[test]
    fn forge_blueprint_book_roundtrip() {
        let msg = ForgeBlueprintBook {
            learned: vec![ForgeBlueprintEntry {
                id: "iron_sword_v0".to_string(),
                display_name: "铁剑（测试）".to_string(),
                tier_cap: 1,
                step_count: 1,
            }],
            current_index: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeBlueprintBook::decode(bytes.as_slice()).expect("ForgeBlueprintBook decode 失败");
        assert_eq!(decoded.learned.len(), 1);
        assert_eq!(decoded.learned[0].id, "iron_sword_v0");
        assert_eq!(decoded.learned[0].tier_cap, 1);
    }

    // ─── 锻造 C2S roundtrip ─────────────────────────────────────

    #[test]
    fn forge_start_session_roundtrip() {
        let msg = ForgeStartSession {
            station_id: "s1".to_string(),
            blueprint_id: "iron_sword_v0".to_string(),
            materials: vec![ForgeMaterialPair {
                material: "iron_ingot".to_string(),
                count: 3,
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeStartSession::decode(bytes.as_slice()).expect("ForgeStartSession decode 失败");
        assert_eq!(decoded.station_id, "s1");
        assert_eq!(decoded.materials.len(), 1);
        assert_eq!(decoded.materials[0].material, "iron_ingot");
    }

    #[test]
    fn forge_tempering_hit_roundtrip() {
        let msg = ForgeTemperingHit {
            session_id: 42,
            beat: "L".to_string(),
            ticks_remaining: 100,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeTemperingHit::decode(bytes.as_slice()).expect("ForgeTemperingHit decode 失败");
        assert_eq!(decoded.session_id, 42);
        assert_eq!(decoded.beat, "L");
        assert_eq!(decoded.ticks_remaining, 100);
    }

    #[test]
    fn forge_inscription_scroll_roundtrip() {
        let msg = ForgeInscriptionScroll {
            session_id: 42,
            inscription_id: "rune_fire_v0".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeInscriptionScroll::decode(bytes.as_slice())
            .expect("ForgeInscriptionScroll decode 失败");
        assert_eq!(decoded.inscription_id, "rune_fire_v0");
    }

    #[test]
    fn forge_consecration_inject_roundtrip() {
        let msg = ForgeConsecrationInject {
            session_id: 42,
            qi_amount: 50.0,
        };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeConsecrationInject::decode(bytes.as_slice())
            .expect("ForgeConsecrationInject decode 失败");
        assert!((decoded.qi_amount - 50.0).abs() < 1e-9);
    }

    #[test]
    fn forge_step_advance_roundtrip() {
        let msg = ForgeStepAdvance { session_id: 42 };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeStepAdvance::decode(bytes.as_slice()).expect("ForgeStepAdvance decode 失败");
        assert_eq!(decoded.session_id, 42);
    }

    #[test]
    fn forge_blueprint_turn_page_roundtrip() {
        let msg = ForgeBlueprintTurnPage { delta: -1 };
        let bytes = msg.encode_to_vec();
        let decoded = ForgeBlueprintTurnPage::decode(bytes.as_slice())
            .expect("ForgeBlueprintTurnPage decode 失败");
        assert_eq!(decoded.delta, -1);
    }

    #[test]
    fn forge_learn_blueprint_roundtrip() {
        let msg = ForgeLearnBlueprint {
            blueprint_id: "qing_feng_v0".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            ForgeLearnBlueprint::decode(bytes.as_slice()).expect("ForgeLearnBlueprint decode 失败");
        assert_eq!(decoded.blueprint_id, "qing_feng_v0");
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 工坊合成 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn craft_category_enum_pin() {
        let expected = [
            (CraftCategory::Unspecified, 0),
            (CraftCategory::AnqiCarrier, 1),
            (CraftCategory::DuguPotion, 2),
            (CraftCategory::TuikeSkin, 3),
            (CraftCategory::ZhenfaTrap, 4),
            (CraftCategory::Tool, 5),
            (CraftCategory::ArmorCraft, 6),
            (CraftCategory::Container, 7),
            (CraftCategory::PoisonPowder, 8),
            (CraftCategory::Misc, 9),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "CraftCategory::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn craft_failure_reason_enum_pin() {
        let expected = [
            (CraftFailureReason::Unspecified, 0),
            (CraftFailureReason::PlayerCancelled, 1),
            (CraftFailureReason::PlayerDied, 2),
            (CraftFailureReason::InternalError, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "CraftFailureReason::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn insight_trigger_enum_pin() {
        let expected = [
            (InsightTrigger::Unspecified, 0),
            (InsightTrigger::Breakthrough, 1),
            (InsightTrigger::NearDeath, 2),
            (InsightTrigger::DefeatStronger, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "InsightTrigger::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn craft_recipe_list_roundtrip() {
        let msg = CraftRecipeList {
            v: 1,
            player_id: "offline:Alice".to_string(),
            recipes: vec![CraftRecipeEntry {
                id: "craft.example.herb_knife.iron".to_string(),
                category: CraftCategory::Tool as i32,
                display_name: "采药刀（凡铁）".to_string(),
                materials: vec![
                    CraftMaterialPair {
                        template_id: "iron_ingot".to_string(),
                        count: 1,
                    },
                    CraftMaterialPair {
                        template_id: "wood_handle".to_string(),
                        count: 1,
                    },
                ],
                qi_cost: 0.0,
                time_ticks: 600,
                output: Some(CraftOutputPair {
                    template_id: "herb_knife_iron".to_string(),
                    count: 1,
                }),
                requirements: Some(CraftRequirements {
                    realm_min: None,
                    qi_color_min: None,
                    skill_lv_min: None,
                }),
                unlocked: false,
            }],
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CraftRecipeList::decode(bytes.as_slice()).expect("CraftRecipeList decode 失败");
        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.player_id, "offline:Alice");
        assert_eq!(decoded.recipes.len(), 1);
        assert_eq!(decoded.recipes[0].id, "craft.example.herb_knife.iron");
        assert_eq!(decoded.recipes[0].category, CraftCategory::Tool as i32);
        assert_eq!(decoded.recipes[0].materials.len(), 2);
        assert_eq!(
            decoded.recipes[0].output.as_ref().unwrap().template_id,
            "herb_knife_iron"
        );
        assert!(!decoded.recipes[0].unlocked);
    }

    #[test]
    fn craft_recipe_list_empty_roundtrip() {
        let msg = CraftRecipeList {
            v: 1,
            player_id: "offline:Alice".to_string(),
            recipes: vec![],
            ts: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CraftRecipeList::decode(bytes.as_slice()).expect("CraftRecipeList (empty) decode 失败");
        assert!(decoded.recipes.is_empty());
    }

    #[test]
    fn craft_recipe_entry_with_requirements_roundtrip() {
        let entry = CraftRecipeEntry {
            id: "craft.example.foo".to_string(),
            category: CraftCategory::ArmorCraft as i32,
            display_name: "测试".to_string(),
            materials: vec![],
            qi_cost: 10.0,
            time_ticks: 1200,
            output: Some(CraftOutputPair {
                template_id: "foo".to_string(),
                count: 1,
            }),
            requirements: Some(CraftRequirements {
                realm_min: Some(Realm::Awaken as i32),
                qi_color_min: Some(QiColorMinPair {
                    color: ColorKind::Insidious as i32,
                    min_share: 0.05,
                }),
                skill_lv_min: Some(2),
            }),
            unlocked: true,
        };
        let bytes = entry.encode_to_vec();
        let decoded = CraftRecipeEntry::decode(bytes.as_slice())
            .expect("CraftRecipeEntry (with requirements) decode 失败");
        let req = decoded.requirements.unwrap();
        assert_eq!(req.realm_min, Some(Realm::Awaken as i32));
        let qc = req.qi_color_min.unwrap();
        assert_eq!(qc.color, ColorKind::Insidious as i32);
        assert!((qc.min_share - 0.05).abs() < 1e-6);
        assert_eq!(req.skill_lv_min, Some(2));
        assert!(decoded.unlocked);
    }

    #[test]
    fn craft_session_state_active_roundtrip() {
        let msg = CraftSessionState {
            v: 1,
            player_id: "offline:Alice".to_string(),
            active: true,
            recipe_id: Some("craft.example.eclipse_needle.iron".to_string()),
            elapsed_ticks: 30,
            total_ticks: 100,
            completed_count: 1,
            total_count: 3,
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CraftSessionState::decode(bytes.as_slice()).expect("CraftSessionState decode 失败");
        assert!(decoded.active);
        assert_eq!(
            decoded.recipe_id.as_deref(),
            Some("craft.example.eclipse_needle.iron")
        );
        assert_eq!(decoded.completed_count, 1);
        assert_eq!(decoded.total_count, 3);
    }

    #[test]
    fn craft_session_state_inactive_roundtrip() {
        let msg = CraftSessionState {
            v: 1,
            player_id: "offline:Alice".to_string(),
            active: false,
            recipe_id: None,
            elapsed_ticks: 0,
            total_ticks: 0,
            completed_count: 0,
            total_count: 0,
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded = CraftSessionState::decode(bytes.as_slice())
            .expect("CraftSessionState (inactive) decode 失败");
        assert!(!decoded.active);
        assert!(decoded.recipe_id.is_none());
    }

    #[test]
    fn craft_outcome_completed_roundtrip() {
        let msg = CraftOutcome {
            outcome: Some(craft_outcome::Outcome::Completed(CraftOutcomeCompleted {
                v: 1,
                player_id: "offline:Alice".to_string(),
                recipe_id: "craft.example.eclipse_needle.iron".to_string(),
                output_template: "eclipse_needle_iron".to_string(),
                output_count: 3,
                completed_at_tick: 5000,
                ts: 1234567,
            })),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CraftOutcome::decode(bytes.as_slice()).expect("CraftOutcome (completed) decode 失败");
        match decoded.outcome.unwrap() {
            craft_outcome::Outcome::Completed(c) => {
                assert_eq!(c.output_template, "eclipse_needle_iron");
                assert_eq!(c.output_count, 3);
            }
            other => panic!("期望 Completed，实际 {other:?}"),
        }
    }

    #[test]
    fn craft_outcome_failed_roundtrip() {
        let msg = CraftOutcome {
            outcome: Some(craft_outcome::Outcome::Failed(CraftOutcomeFailed {
                v: 1,
                player_id: "offline:Alice".to_string(),
                recipe_id: "craft.example.eclipse_needle.iron".to_string(),
                reason: CraftFailureReason::PlayerCancelled as i32,
                material_returned: 3,
                qi_refunded: 0.0,
                ts: 1234567,
            })),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CraftOutcome::decode(bytes.as_slice()).expect("CraftOutcome (failed) decode 失败");
        match decoded.outcome.unwrap() {
            craft_outcome::Outcome::Failed(f) => {
                assert_eq!(f.reason, CraftFailureReason::PlayerCancelled as i32);
                assert_eq!(f.material_returned, 3);
            }
            other => panic!("期望 Failed，实际 {other:?}"),
        }
    }

    #[test]
    fn recipe_unlocked_scroll_source_roundtrip() {
        let msg = RecipeUnlocked {
            v: 1,
            player_id: "offline:Alice".to_string(),
            recipe_id: "craft.example.foo".to_string(),
            source: Some(UnlockEventSource {
                source: Some(unlock_event_source::Source::ScrollItemTemplate(
                    "scroll_eclipse".to_string(),
                )),
            }),
            unlocked_at_tick: 10000,
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            RecipeUnlocked::decode(bytes.as_slice()).expect("RecipeUnlocked (scroll) decode 失败");
        match decoded.source.unwrap().source.unwrap() {
            unlock_event_source::Source::ScrollItemTemplate(s) => {
                assert_eq!(s, "scroll_eclipse");
            }
            other => panic!("期望 ScrollItemTemplate，实际 {other:?}"),
        }
    }

    #[test]
    fn recipe_unlocked_mentor_source_roundtrip() {
        let msg = RecipeUnlocked {
            v: 1,
            player_id: "offline:Alice".to_string(),
            recipe_id: "craft.example.foo".to_string(),
            source: Some(UnlockEventSource {
                source: Some(unlock_event_source::Source::MentorNpcArchetype(
                    "poison_master".to_string(),
                )),
            }),
            unlocked_at_tick: 10000,
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            RecipeUnlocked::decode(bytes.as_slice()).expect("RecipeUnlocked (mentor) decode 失败");
        match decoded.source.unwrap().source.unwrap() {
            unlock_event_source::Source::MentorNpcArchetype(s) => {
                assert_eq!(s, "poison_master");
            }
            other => panic!("期望 MentorNpcArchetype，实际 {other:?}"),
        }
    }

    #[test]
    fn recipe_unlocked_insight_source_roundtrip() {
        let msg = RecipeUnlocked {
            v: 1,
            player_id: "offline:Alice".to_string(),
            recipe_id: "craft.example.foo".to_string(),
            source: Some(UnlockEventSource {
                source: Some(unlock_event_source::Source::InsightTrigger(
                    InsightTrigger::NearDeath as i32,
                )),
            }),
            unlocked_at_tick: 10000,
            ts: 1234567,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            RecipeUnlocked::decode(bytes.as_slice()).expect("RecipeUnlocked (insight) decode 失败");
        match decoded.source.unwrap().source.unwrap() {
            unlock_event_source::Source::InsightTrigger(t) => {
                assert_eq!(t, InsightTrigger::NearDeath as i32);
            }
            other => panic!("期望 InsightTrigger，实际 {other:?}"),
        }
    }

    // ─── 工坊 C2S roundtrip ─────────────────────────────────────

    #[test]
    fn craft_start_roundtrip() {
        let msg = CraftStart {
            recipe_id: "craft.example.herb_knife.iron".to_string(),
            quantity: 3,
        };
        let bytes = msg.encode_to_vec();
        let decoded = CraftStart::decode(bytes.as_slice()).expect("CraftStart decode 失败");
        assert_eq!(decoded.recipe_id, "craft.example.herb_knife.iron");
        assert_eq!(decoded.quantity, 3);
    }

    #[test]
    fn craft_cancel_roundtrip() {
        let msg = CraftCancel {};
        let bytes = msg.encode_to_vec();
        let decoded = CraftCancel::decode(bytes.as_slice()).expect("CraftCancel decode 失败");
        let _ = decoded; // 无字段，decode 成功即通过
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 采集 / 药草 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn botany_harvest_mode_enum_pin() {
        let expected = [
            (BotanyHarvestMode::Unspecified, 0),
            (BotanyHarvestMode::Manual, 1),
            (BotanyHarvestMode::Auto, 2),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "BotanyHarvestMode::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn botany_model_overlay_enum_pin() {
        let expected = [
            (BotanyModelOverlay::Unspecified, 0),
            (BotanyModelOverlay::None, 1),
            (BotanyModelOverlay::Emissive, 2),
            (BotanyModelOverlay::DualPhase, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "BotanyModelOverlay::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn botany_harvest_progress_full_roundtrip() {
        let msg = BotanyHarvestProgress {
            session_id: "session-botany-01".to_string(),
            target_id: "plant-1".to_string(),
            target_name: "开脉草".to_string(),
            plant_kind: "ning_mai_cao".to_string(),
            mode: "manual".to_string(),
            progress: 0.5,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: false,
            detail: "晨露未散".to_string(),
            hazard_hints: vec!["靠近 -0.4 真元/s 叠加".to_string()],
            target_pos_x: Some(10.5),
            target_pos_y: Some(64.0),
            target_pos_z: Some(10.5),
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyHarvestProgress::decode(bytes.as_slice())
            .expect("BotanyHarvestProgress decode 失败");
        assert_eq!(decoded.session_id, "session-botany-01");
        assert_eq!(decoded.mode, "manual");
        assert!((decoded.progress - 0.5).abs() < 1e-9);
        assert!(decoded.auto_selectable);
        assert_eq!(decoded.hazard_hints.len(), 1);
        assert_eq!(decoded.target_pos_x, Some(10.5));
        assert_eq!(decoded.target_pos_y, Some(64.0));
        assert_eq!(decoded.target_pos_z, Some(10.5));
    }

    #[test]
    fn botany_harvest_progress_no_pos_roundtrip() {
        let msg = BotanyHarvestProgress {
            session_id: "session-botany-02".to_string(),
            target_id: "plant-2".to_string(),
            target_name: "赤髓草".to_string(),
            plant_kind: "chi_sui_cao".to_string(),
            mode: "auto".to_string(),
            progress: 1.0,
            auto_selectable: true,
            request_pending: false,
            interrupted: false,
            completed: true,
            detail: "".to_string(),
            hazard_hints: vec![],
            target_pos_x: None,
            target_pos_y: None,
            target_pos_z: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyHarvestProgress::decode(bytes.as_slice())
            .expect("BotanyHarvestProgress (no pos) decode 失败");
        assert!(decoded.target_pos_x.is_none());
        assert!(decoded.target_pos_y.is_none());
        assert!(decoded.target_pos_z.is_none());
        assert!(decoded.completed);
        assert!(decoded.hazard_hints.is_empty());
    }

    #[test]
    fn botany_plant_v2_render_profiles_roundtrip() {
        let msg = BotanyPlantV2RenderProfiles {
            profiles: vec![BotanyPlantV2RenderProfile {
                plant_id: "ying_yuan_gu".to_string(),
                base_mesh_ref: "red_mushroom".to_string(),
                tint_rgb: 0xFFA040,
                tint_rgb_secondary: None,
                model_overlay: BotanyModelOverlay::Emissive as i32,
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyPlantV2RenderProfiles::decode(bytes.as_slice())
            .expect("BotanyPlantV2RenderProfiles decode 失败");
        assert_eq!(decoded.profiles.len(), 1);
        assert_eq!(decoded.profiles[0].plant_id, "ying_yuan_gu");
        assert_eq!(decoded.profiles[0].tint_rgb, 0xFFA040);
        assert!(decoded.profiles[0].tint_rgb_secondary.is_none());
        assert_eq!(
            decoded.profiles[0].model_overlay,
            BotanyModelOverlay::Emissive as i32
        );
    }

    #[test]
    fn botany_plant_v2_render_profile_with_secondary_roundtrip() {
        let msg = BotanyPlantV2RenderProfile {
            plant_id: "ci_she_hao".to_string(),
            base_mesh_ref: "sweet_berries".to_string(),
            tint_rgb: 0x40FF80,
            tint_rgb_secondary: Some(0xFF2020),
            model_overlay: BotanyModelOverlay::DualPhase as i32,
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyPlantV2RenderProfile::decode(bytes.as_slice())
            .expect("BotanyPlantV2RenderProfile (secondary) decode 失败");
        assert_eq!(decoded.tint_rgb_secondary, Some(0xFF2020));
        assert_eq!(decoded.model_overlay, BotanyModelOverlay::DualPhase as i32);
    }

    #[test]
    fn botany_skill_roundtrip() {
        let msg = BotanySkill {
            level: 3,
            xp: 240,
            xp_to_next_level: 400,
            auto_unlock_level: 3,
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanySkill::decode(bytes.as_slice()).expect("BotanySkill decode 失败");
        assert_eq!(decoded.level, 3);
        assert_eq!(decoded.xp, 240);
        assert_eq!(decoded.xp_to_next_level, 400);
        assert_eq!(decoded.auto_unlock_level, 3);
    }

    // ─── 采集 C2S roundtrip ─────────────────────────────────────

    #[test]
    fn botany_harvest_request_manual_roundtrip() {
        let msg = BotanyHarvestRequest {
            session_id: "session-botany-01".to_string(),
            mode: BotanyHarvestMode::Manual as i32,
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyHarvestRequest::decode(bytes.as_slice())
            .expect("BotanyHarvestRequest decode 失败");
        assert_eq!(decoded.session_id, "session-botany-01");
        assert_eq!(decoded.mode, BotanyHarvestMode::Manual as i32);
    }

    #[test]
    fn botany_harvest_request_auto_roundtrip() {
        let msg = BotanyHarvestRequest {
            session_id: "session-botany-02".to_string(),
            mode: BotanyHarvestMode::Auto as i32,
        };
        let bytes = msg.encode_to_vec();
        let decoded = BotanyHarvestRequest::decode(bytes.as_slice())
            .expect("BotanyHarvestRequest (auto) decode 失败");
        assert_eq!(decoded.mode, BotanyHarvestMode::Auto as i32);
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 采矿 / 伐木 / 通用采集 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn mining_progress_roundtrip() {
        let msg = MiningProgress {
            session_id: "mining-01".to_string(),
            ore_pos_x: 10,
            ore_pos_y: 32,
            ore_pos_z: -5,
            progress: 0.75,
            interrupted: false,
            completed: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = MiningProgress::decode(bytes.as_slice()).expect("MiningProgress decode 失败");
        assert_eq!(decoded.session_id, "mining-01");
        assert_eq!(decoded.ore_pos_x, 10);
        assert_eq!(decoded.ore_pos_y, 32);
        assert_eq!(decoded.ore_pos_z, -5);
        assert!((decoded.progress - 0.75).abs() < 1e-9);
        assert!(!decoded.interrupted);
        assert!(!decoded.completed);
    }

    #[test]
    fn mining_progress_completed_roundtrip() {
        let msg = MiningProgress {
            session_id: "mining-02".to_string(),
            ore_pos_x: 0,
            ore_pos_y: 0,
            ore_pos_z: 0,
            progress: 1.0,
            interrupted: false,
            completed: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = MiningProgress::decode(bytes.as_slice())
            .expect("MiningProgress (completed) decode 失败");
        assert!(decoded.completed);
    }

    #[test]
    fn lumber_progress_roundtrip() {
        let msg = LumberProgress {
            session_id: "lumber-01".to_string(),
            log_pos_x: 5,
            log_pos_y: 64,
            log_pos_z: 5,
            progress: 0.5,
            interrupted: true,
            completed: false,
            detail: "树干太硬".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = LumberProgress::decode(bytes.as_slice()).expect("LumberProgress decode 失败");
        assert_eq!(decoded.session_id, "lumber-01");
        assert!(decoded.interrupted);
        assert_eq!(decoded.detail, "树干太硬");
    }

    #[test]
    fn gathering_target_type_enum_pin() {
        let expected = [
            (GatheringTargetType::Unspecified, 0),
            (GatheringTargetType::Herb, 1),
            (GatheringTargetType::Ore, 2),
            (GatheringTargetType::Wood, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "GatheringTargetType::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn gathering_quality_hint_enum_pin() {
        let expected = [
            (GatheringQualityHint::Unspecified, 0),
            (GatheringQualityHint::Normal, 1),
            (GatheringQualityHint::FineLikely, 2),
            (GatheringQualityHint::PerfectPossible, 3),
            (GatheringQualityHint::Fine, 4),
            (GatheringQualityHint::Perfect, 5),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "GatheringQualityHint::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn gathering_session_roundtrip() {
        let msg = GatheringSession {
            session_id: "gather-01".to_string(),
            progress_ticks: 30,
            total_ticks: 100,
            target_name: "开脉草".to_string(),
            target_type: GatheringTargetType::Herb as i32,
            quality_hint: GatheringQualityHint::FineLikely as i32,
            tool_used: Some("herb_knife_iron".to_string()),
            interrupted: false,
            completed: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            GatheringSession::decode(bytes.as_slice()).expect("GatheringSession decode 失败");
        assert_eq!(decoded.session_id, "gather-01");
        assert_eq!(decoded.progress_ticks, 30);
        assert_eq!(decoded.total_ticks, 100);
        assert_eq!(decoded.target_type, GatheringTargetType::Herb as i32);
        assert_eq!(
            decoded.quality_hint,
            GatheringQualityHint::FineLikely as i32
        );
        assert_eq!(decoded.tool_used.as_deref(), Some("herb_knife_iron"));
    }

    #[test]
    fn gathering_session_no_tool_roundtrip() {
        let msg = GatheringSession {
            session_id: "gather-02".to_string(),
            progress_ticks: 0,
            total_ticks: 200,
            target_name: "铁矿".to_string(),
            target_type: GatheringTargetType::Ore as i32,
            quality_hint: GatheringQualityHint::Normal as i32,
            tool_used: None,
            interrupted: false,
            completed: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = GatheringSession::decode(bytes.as_slice())
            .expect("GatheringSession (no tool) decode 失败");
        assert!(decoded.tool_used.is_none());
        assert_eq!(decoded.target_type, GatheringTargetType::Ore as i32);
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 灵田 roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn lingtian_session_kind_enum_pin() {
        let expected = [
            (LingtianSessionKind::Unspecified, 0),
            (LingtianSessionKind::Till, 1),
            (LingtianSessionKind::Renew, 2),
            (LingtianSessionKind::Planting, 3),
            (LingtianSessionKind::Harvest, 4),
            (LingtianSessionKind::Replenish, 5),
            (LingtianSessionKind::DrainQi, 6),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "LingtianSessionKind::{variant:?} wire 值应为 {wire}"
            );
        }
    }

    #[test]
    fn lingtian_session_data_active_roundtrip() {
        let msg = LingtianSessionData {
            active: true,
            kind: LingtianSessionKind::Planting as i32,
            pos_x: 10,
            pos_y: 64,
            pos_z: 20,
            elapsed_ticks: 30,
            target_ticks: 100,
            plant_id: Some("ning_mai_cao".to_string()),
            source: None,
            dye_contamination: Some(0.15),
            dye_contamination_warning: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianSessionData::decode(bytes.as_slice())
            .expect("LingtianSessionData (planting) decode 失败");
        assert!(decoded.active);
        assert_eq!(decoded.kind, LingtianSessionKind::Planting as i32);
        assert_eq!(decoded.pos_x, 10);
        assert_eq!(decoded.pos_y, 64);
        assert_eq!(decoded.pos_z, 20);
        assert_eq!(decoded.plant_id.as_deref(), Some("ning_mai_cao"));
        assert!(decoded.source.is_none());
        assert!((decoded.dye_contamination.unwrap() - 0.15).abs() < 1e-6);
        assert!(!decoded.dye_contamination_warning);
    }

    #[test]
    fn lingtian_session_data_replenish_roundtrip() {
        let msg = LingtianSessionData {
            active: true,
            kind: LingtianSessionKind::Replenish as i32,
            pos_x: 0,
            pos_y: 64,
            pos_z: 0,
            elapsed_ticks: 0,
            target_ticks: 200,
            plant_id: None,
            source: Some("bone_coin".to_string()),
            dye_contamination: None,
            dye_contamination_warning: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianSessionData::decode(bytes.as_slice())
            .expect("LingtianSessionData (replenish) decode 失败");
        assert_eq!(decoded.source.as_deref(), Some("bone_coin"));
        assert!(decoded.plant_id.is_none());
        assert!(decoded.dye_contamination.is_none());
    }

    #[test]
    fn lingtian_session_data_inactive_roundtrip() {
        let msg = LingtianSessionData {
            active: false,
            kind: LingtianSessionKind::Till as i32,
            pos_x: 0,
            pos_y: 0,
            pos_z: 0,
            elapsed_ticks: 0,
            target_ticks: 0,
            plant_id: None,
            source: None,
            dye_contamination: None,
            dye_contamination_warning: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianSessionData::decode(bytes.as_slice())
            .expect("LingtianSessionData (inactive) decode 失败");
        assert!(!decoded.active);
    }

    #[test]
    fn lingtian_session_dye_contamination_warning_roundtrip() {
        let msg = LingtianSessionData {
            active: true,
            kind: LingtianSessionKind::Harvest as i32,
            pos_x: 5,
            pos_y: 65,
            pos_z: 5,
            elapsed_ticks: 50,
            target_ticks: 60,
            plant_id: Some("ci_she_hao".to_string()),
            source: None,
            dye_contamination: Some(0.35),
            dye_contamination_warning: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianSessionData::decode(bytes.as_slice())
            .expect("LingtianSessionData (dye warning) decode 失败");
        assert!(decoded.dye_contamination_warning);
        assert!((decoded.dye_contamination.unwrap() - 0.35).abs() < 1e-6);
    }

    // ─── 灵田 C2S roundtrip ─────────────────────────────────────

    #[test]
    fn lingtian_start_till_roundtrip() {
        let msg = LingtianStartTill {
            x: 10,
            y: 64,
            z: 20,
            hoe_instance_id: 4242,
            mode: "manual".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            LingtianStartTill::decode(bytes.as_slice()).expect("LingtianStartTill decode 失败");
        assert_eq!(decoded.x, 10);
        assert_eq!(decoded.hoe_instance_id, 4242);
        assert_eq!(decoded.mode, "manual");
    }

    #[test]
    fn lingtian_start_renew_roundtrip() {
        let msg = LingtianStartRenew {
            x: 10,
            y: 64,
            z: 20,
            hoe_instance_id: 4242,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            LingtianStartRenew::decode(bytes.as_slice()).expect("LingtianStartRenew decode 失败");
        assert_eq!(decoded.hoe_instance_id, 4242);
    }

    #[test]
    fn lingtian_start_planting_roundtrip() {
        let msg = LingtianStartPlanting {
            x: 10,
            y: 64,
            z: 20,
            plant_id: "ning_mai_cao".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianStartPlanting::decode(bytes.as_slice())
            .expect("LingtianStartPlanting decode 失败");
        assert_eq!(decoded.plant_id, "ning_mai_cao");
    }

    #[test]
    fn lingtian_start_harvest_roundtrip() {
        let msg = LingtianStartHarvest {
            x: 10,
            y: 64,
            z: 20,
            mode: "auto".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianStartHarvest::decode(bytes.as_slice())
            .expect("LingtianStartHarvest decode 失败");
        assert_eq!(decoded.mode, "auto");
    }

    #[test]
    fn lingtian_start_replenish_roundtrip() {
        let msg = LingtianStartReplenish {
            x: 10,
            y: 64,
            z: 20,
            source: "bone_coin".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianStartReplenish::decode(bytes.as_slice())
            .expect("LingtianStartReplenish decode 失败");
        assert_eq!(decoded.source, "bone_coin");
    }

    #[test]
    fn lingtian_start_drain_qi_roundtrip() {
        let msg = LingtianStartDrainQi {
            x: 10,
            y: 64,
            z: 20,
        };
        let bytes = msg.encode_to_vec();
        let decoded = LingtianStartDrainQi::decode(bytes.as_slice())
            .expect("LingtianStartDrainQi decode 失败");
        assert_eq!(decoded.x, 10);
        assert_eq!(decoded.y, 64);
        assert_eq!(decoded.z, 20);
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — 矿石 C2S roundtrip
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn mineral_probe_roundtrip() {
        let msg = MineralProbe { x: 8, y: 32, z: 8 };
        let bytes = msg.encode_to_vec();
        let decoded = MineralProbe::decode(bytes.as_slice()).expect("MineralProbe decode 失败");
        assert_eq!(decoded.x, 8);
        assert_eq!(decoded.y, 32);
        assert_eq!(decoded.z, 8);
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — envelope S2C oneof 可区分性
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn server_data_envelope_b1_s2c_variants_distinguishable() {
        // 验证所有 B1 S2C oneof variant 能正确 encode → decode 且不互相串。
        let variant_payloads: Vec<(&str, server_data_envelope::Payload)> = vec![
            (
                "AlchemyFurnace",
                server_data_envelope::Payload::AlchemyFurnace(AlchemyFurnace {
                    pos_x: Some(0),
                    pos_y: Some(64),
                    pos_z: Some(0),
                    tier: 1,
                    integrity: 1.0,
                    integrity_max: 1.0,
                    owner_name: "t".to_string(),
                    has_session: false,
                }),
            ),
            (
                "AlchemySession",
                server_data_envelope::Payload::AlchemySession(AlchemySession {
                    recipe_id: None,
                    active: false,
                    elapsed_ticks: 0,
                    target_ticks: 0,
                    temp_current: 0.0,
                    temp_target: 0.0,
                    temp_band: 0.0,
                    qi_injected: 0.0,
                    qi_target: 0.0,
                    status_label: "".to_string(),
                    stages: vec![],
                    interventions_recent: vec![],
                }),
            ),
            (
                "AlchemyOutcomeForecast",
                server_data_envelope::Payload::AlchemyOutcomeForecast(AlchemyOutcomeForecast {
                    perfect_pct: 0.0,
                    good_pct: 0.0,
                    flawed_pct: 0.0,
                    waste_pct: 0.0,
                    explode_pct: 0.0,
                    perfect_note: "".to_string(),
                    good_note: "".to_string(),
                    flawed_note: "".to_string(),
                }),
            ),
            (
                "AlchemyOutcomeResolved",
                server_data_envelope::Payload::AlchemyOutcomeResolved(AlchemyOutcomeResolved {
                    bucket: 0,
                    recipe_id: None,
                    pill: None,
                    quality: None,
                    toxin_amount: None,
                    toxin_color: None,
                    qi_gain: None,
                    side_effect_tag: None,
                    flawed_path: false,
                    damage: None,
                    meridian_crack: None,
                }),
            ),
            (
                "AlchemyRecipeBook",
                server_data_envelope::Payload::AlchemyRecipeBook(AlchemyRecipeBook {
                    learned: vec![],
                    current_index: 0,
                }),
            ),
            (
                "AlchemyContamination",
                server_data_envelope::Payload::AlchemyContamination(AlchemyContamination {
                    levels: vec![],
                    metabolism_note: "".to_string(),
                }),
            ),
            (
                "ForgeStation",
                server_data_envelope::Payload::ForgeStation(ForgeStation {
                    station_id: "s".to_string(),
                    tier: 1,
                    integrity: 1.0,
                    owner_name: "t".to_string(),
                    has_session: false,
                }),
            ),
            (
                "ForgeSession",
                server_data_envelope::Payload::ForgeSession(ForgeSessionData {
                    session_id: 1,
                    blueprint_id: "x".to_string(),
                    blueprint_name: "x".to_string(),
                    active: false,
                    current_step: 0,
                    step_index: 0,
                    achieved_tier: 0,
                    step_state: None,
                }),
            ),
            (
                "ForgeOutcome",
                server_data_envelope::Payload::ForgeOutcome(ForgeOutcome {
                    session_id: 1,
                    blueprint_id: "x".to_string(),
                    bucket: 0,
                    weapon_item: None,
                    quality: 0.0,
                    color: None,
                    side_effects: vec![],
                    achieved_tier: 0,
                    flawed_path: false,
                }),
            ),
            (
                "ForgeBlueprintBook",
                server_data_envelope::Payload::ForgeBlueprintBook(ForgeBlueprintBook {
                    learned: vec![],
                    current_index: 0,
                }),
            ),
            (
                "CraftRecipeList",
                server_data_envelope::Payload::CraftRecipeList(CraftRecipeList {
                    v: 1,
                    player_id: "x".to_string(),
                    recipes: vec![],
                    ts: 0,
                }),
            ),
            (
                "CraftSessionState",
                server_data_envelope::Payload::CraftSessionState(CraftSessionState {
                    v: 1,
                    player_id: "x".to_string(),
                    active: false,
                    recipe_id: None,
                    elapsed_ticks: 0,
                    total_ticks: 0,
                    completed_count: 0,
                    total_count: 0,
                    ts: 0,
                }),
            ),
            (
                "CraftOutcome",
                server_data_envelope::Payload::CraftOutcome(CraftOutcome { outcome: None }),
            ),
            (
                "RecipeUnlocked",
                server_data_envelope::Payload::RecipeUnlocked(RecipeUnlocked {
                    v: 1,
                    player_id: "x".to_string(),
                    recipe_id: "y".to_string(),
                    source: None,
                    unlocked_at_tick: 0,
                    ts: 0,
                }),
            ),
            (
                "BotanyHarvestProgress",
                server_data_envelope::Payload::BotanyHarvestProgress(BotanyHarvestProgress {
                    session_id: "s".to_string(),
                    target_id: "t".to_string(),
                    target_name: "n".to_string(),
                    plant_kind: "k".to_string(),
                    mode: "manual".to_string(),
                    progress: 0.0,
                    auto_selectable: false,
                    request_pending: false,
                    interrupted: false,
                    completed: false,
                    detail: "".to_string(),
                    hazard_hints: vec![],
                    target_pos_x: None,
                    target_pos_y: None,
                    target_pos_z: None,
                }),
            ),
            (
                "BotanyPlantV2RenderProfiles",
                server_data_envelope::Payload::BotanyPlantV2RenderProfiles(
                    BotanyPlantV2RenderProfiles { profiles: vec![] },
                ),
            ),
            (
                "BotanySkill",
                server_data_envelope::Payload::BotanySkill(BotanySkill {
                    level: 0,
                    xp: 0,
                    xp_to_next_level: 0,
                    auto_unlock_level: 0,
                }),
            ),
            (
                "MiningProgress",
                server_data_envelope::Payload::MiningProgress(MiningProgress {
                    session_id: "m".to_string(),
                    ore_pos_x: 0,
                    ore_pos_y: 0,
                    ore_pos_z: 0,
                    progress: 0.0,
                    interrupted: false,
                    completed: false,
                }),
            ),
            (
                "LumberProgress",
                server_data_envelope::Payload::LumberProgress(LumberProgress {
                    session_id: "l".to_string(),
                    log_pos_x: 0,
                    log_pos_y: 0,
                    log_pos_z: 0,
                    progress: 0.0,
                    interrupted: false,
                    completed: false,
                    detail: "".to_string(),
                }),
            ),
            (
                "GatheringSession",
                server_data_envelope::Payload::GatheringSession(GatheringSession {
                    session_id: "g".to_string(),
                    progress_ticks: 0,
                    total_ticks: 0,
                    target_name: "n".to_string(),
                    target_type: 0,
                    quality_hint: 0,
                    tool_used: None,
                    interrupted: false,
                    completed: false,
                }),
            ),
            (
                "LingtianSession",
                server_data_envelope::Payload::LingtianSession(LingtianSessionData {
                    active: false,
                    kind: 0,
                    pos_x: 0,
                    pos_y: 0,
                    pos_z: 0,
                    elapsed_ticks: 0,
                    target_ticks: 0,
                    plant_id: None,
                    source: None,
                    dye_contamination: None,
                    dye_contamination_warning: false,
                }),
            ),
        ];

        for (name, payload) in variant_payloads {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} envelope roundtrip 后 payload 不应为 None"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B1 — envelope C2S oneof 可区分性
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn client_request_envelope_b1_c2s_variants_distinguishable() {
        let variant_payloads: Vec<(&str, client_request_envelope::Payload)> = vec![
            (
                "AlchemyOpenFurnace",
                client_request_envelope::Payload::AlchemyOpenFurnace(AlchemyOpenFurnace {
                    furnace_pos_x: 0,
                    furnace_pos_y: 64,
                    furnace_pos_z: 0,
                }),
            ),
            (
                "AlchemyFeedSlot",
                client_request_envelope::Payload::AlchemyFeedSlot(AlchemyFeedSlot {
                    furnace_pos_x: 0,
                    furnace_pos_y: 64,
                    furnace_pos_z: 0,
                    slot_idx: 0,
                    material: "x".to_string(),
                    count: 1,
                }),
            ),
            (
                "AlchemyTakeBack",
                client_request_envelope::Payload::AlchemyTakeBack(AlchemyTakeBack {
                    furnace_pos_x: 0,
                    furnace_pos_y: 64,
                    furnace_pos_z: 0,
                    slot_idx: 0,
                }),
            ),
            (
                "AlchemyIgnite",
                client_request_envelope::Payload::AlchemyIgnite(AlchemyIgnite {
                    furnace_pos_x: 0,
                    furnace_pos_y: 64,
                    furnace_pos_z: 0,
                    recipe_id: "x".to_string(),
                }),
            ),
            (
                "AlchemyIntervention",
                client_request_envelope::Payload::AlchemyIntervention(AlchemyIntervention {
                    furnace_pos_x: 0,
                    furnace_pos_y: 64,
                    furnace_pos_z: 0,
                    intervention: None,
                }),
            ),
            (
                "AlchemyTurnPage",
                client_request_envelope::Payload::AlchemyTurnPage(AlchemyTurnPage { delta: 1 }),
            ),
            (
                "AlchemyLearnRecipe",
                client_request_envelope::Payload::AlchemyLearnRecipe(AlchemyLearnRecipe {
                    recipe_id: "x".to_string(),
                }),
            ),
            (
                "AlchemyLearnRecipeFragment",
                client_request_envelope::Payload::AlchemyLearnRecipeFragment(
                    AlchemyLearnRecipeFragment {
                        item_instance_id: 1,
                    },
                ),
            ),
            (
                "AlchemyTakePill",
                client_request_envelope::Payload::AlchemyTakePill(AlchemyTakePill {
                    pill_item_id: "x".to_string(),
                }),
            ),
            (
                "ForgeStartSession",
                client_request_envelope::Payload::ForgeStartSession(ForgeStartSession {
                    station_id: "s".to_string(),
                    blueprint_id: "b".to_string(),
                    materials: vec![],
                }),
            ),
            (
                "ForgeTemperingHit",
                client_request_envelope::Payload::ForgeTemperingHit(ForgeTemperingHit {
                    session_id: 1,
                    beat: "L".to_string(),
                    ticks_remaining: 0,
                }),
            ),
            (
                "ForgeInscriptionScroll",
                client_request_envelope::Payload::ForgeInscriptionScroll(ForgeInscriptionScroll {
                    session_id: 1,
                    inscription_id: "x".to_string(),
                }),
            ),
            (
                "ForgeConsecrationInject",
                client_request_envelope::Payload::ForgeConsecrationInject(
                    ForgeConsecrationInject {
                        session_id: 1,
                        qi_amount: 0.0,
                    },
                ),
            ),
            (
                "ForgeStepAdvance",
                client_request_envelope::Payload::ForgeStepAdvance(ForgeStepAdvance {
                    session_id: 1,
                }),
            ),
            (
                "ForgeBlueprintTurnPage",
                client_request_envelope::Payload::ForgeBlueprintTurnPage(ForgeBlueprintTurnPage {
                    delta: 1,
                }),
            ),
            (
                "ForgeLearnBlueprint",
                client_request_envelope::Payload::ForgeLearnBlueprint(ForgeLearnBlueprint {
                    blueprint_id: "x".to_string(),
                }),
            ),
            (
                "CraftStart",
                client_request_envelope::Payload::CraftStart(CraftStart {
                    recipe_id: "x".to_string(),
                    quantity: 1,
                }),
            ),
            (
                "CraftCancel",
                client_request_envelope::Payload::CraftCancel(CraftCancel {}),
            ),
            (
                "BotanyHarvestRequest",
                client_request_envelope::Payload::BotanyHarvestRequest(BotanyHarvestRequest {
                    session_id: "s".to_string(),
                    mode: BotanyHarvestMode::Manual as i32,
                }),
            ),
            (
                "LingtianStartTill",
                client_request_envelope::Payload::LingtianStartTill(LingtianStartTill {
                    x: 0,
                    y: 64,
                    z: 0,
                    hoe_instance_id: 1,
                    mode: "manual".to_string(),
                }),
            ),
            (
                "LingtianStartRenew",
                client_request_envelope::Payload::LingtianStartRenew(LingtianStartRenew {
                    x: 0,
                    y: 64,
                    z: 0,
                    hoe_instance_id: 1,
                }),
            ),
            (
                "LingtianStartPlanting",
                client_request_envelope::Payload::LingtianStartPlanting(LingtianStartPlanting {
                    x: 0,
                    y: 64,
                    z: 0,
                    plant_id: "x".to_string(),
                }),
            ),
            (
                "LingtianStartHarvest",
                client_request_envelope::Payload::LingtianStartHarvest(LingtianStartHarvest {
                    x: 0,
                    y: 64,
                    z: 0,
                    mode: "manual".to_string(),
                }),
            ),
            (
                "LingtianStartReplenish",
                client_request_envelope::Payload::LingtianStartReplenish(LingtianStartReplenish {
                    x: 0,
                    y: 64,
                    z: 0,
                    source: "bone_coin".to_string(),
                }),
            ),
            (
                "LingtianStartDrainQi",
                client_request_envelope::Payload::LingtianStartDrainQi(LingtianStartDrainQi {
                    x: 0,
                    y: 64,
                    z: 0,
                }),
            ),
            (
                "MineralProbe",
                client_request_envelope::Payload::MineralProbe(MineralProbe { x: 8, y: 32, z: 8 }),
            ),
        ];

        for (name, payload) in variant_payloads {
            let envelope = ClientRequestEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} C2S envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} C2S envelope roundtrip 后 payload 不应为 None"
            );
        }
    }

    // ─── envelope 完整性：新增 oneof variant 全覆盖 ──────────────

    #[test]
    fn server_data_envelope_all_new_variants_distinguishable() {
        // 确保 oneof 7..10 能正确区分。
        let payloads: Vec<server_data_envelope::Payload> = vec![
            server_data_envelope::Payload::SkillXpGain(SkillXpGain {
                char_id: 1,
                skill: SkillId::Herbalism as i32,
                amount: 10,
                source: None,
                source_realm_breakthrough: false,
            }),
            server_data_envelope::Payload::InventorySnapshot(InventorySnapshot {
                revision: 1,
                containers: vec![],
                placed_items: vec![],
                equipped: None,
                hotbar: vec![],
                bone_coins: 0,
                weight: None,
                realm: "Awaken".to_string(),
                qi_current: 0.0,
                qi_max: 0.0,
                body_level: 0.0,
            }),
            server_data_envelope::Payload::CombatHudState(CombatHudState {
                hp_percent: 1.0,
                qi_percent: 1.0,
                stamina_percent: 1.0,
                derived: None,
            }),
            server_data_envelope::Payload::KnockbackSync(KnockbackSync {
                distance_blocks: 2.0,
                velocity_blocks_per_tick: 0.1,
                duration_ticks: 10,
                kinetic_energy: 50.0,
                collision_damage: None,
                chain_depth: 0,
                block_broken: false,
            }),
        ];

        let variant_names = [
            "SkillXpGain",
            "InventorySnapshot",
            "CombatHudState",
            "KnockbackSync",
        ];

        for (i, payload) in payloads.into_iter().enumerate() {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{} envelope decode 失败: {e}", variant_names[i]));
            // 验证是对应 variant 而非其他
            let name = variant_names[i];
            match (&decoded.payload, i) {
                (Some(server_data_envelope::Payload::SkillXpGain(_)), 0) => {}
                (Some(server_data_envelope::Payload::InventorySnapshot(_)), 1) => {}
                (Some(server_data_envelope::Payload::CombatHudState(_)), 2) => {}
                (Some(server_data_envelope::Payload::KnockbackSync(_)), 3) => {}
                (other, _) => {
                    panic!("envelope variant {name} roundtrip 得到错误 variant: {other:?}")
                }
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    // P2 B2：战斗 / 伤口 / 防御 / 施法 / 毒 / 载体 / 暗器
    // ═══════════════════════════════════════════════════════════

    // ─── WoundsSnapshot ─────────────────────────────────────────

    #[test]
    fn wounds_snapshot_roundtrip_nonempty() {
        let msg = WoundsSnapshot {
            wounds: vec![
                WoundEntry {
                    part: "chest".to_string(),
                    kind: "cut".to_string(),
                    severity: 0.6,
                    state: "bleeding".to_string(),
                    infection: 0.1,
                    scar: false,
                    updated_at_ms: 123_456,
                },
                WoundEntry {
                    part: "head".to_string(),
                    kind: "concussion".to_string(),
                    severity: 0.3,
                    state: "stable".to_string(),
                    infection: 0.0,
                    scar: true,
                    updated_at_ms: 123_457,
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded = WoundsSnapshot::decode(bytes.as_slice()).expect("WoundsSnapshot decode 失败");
        assert_eq!(decoded.wounds.len(), 2, "应有 2 条伤口记录");
        assert_eq!(decoded.wounds[0].part, "chest");
        assert_eq!(decoded.wounds[0].severity, 0.6_f32);
        assert!(decoded.wounds[1].scar, "head 伤口应有疤痕");
    }

    #[test]
    fn wounds_snapshot_roundtrip_empty() {
        let msg = WoundsSnapshot { wounds: vec![] };
        let bytes = msg.encode_to_vec();
        let decoded =
            WoundsSnapshot::decode(bytes.as_slice()).expect("空 WoundsSnapshot decode 失败");
        assert!(decoded.wounds.is_empty(), "空伤口快照应为空 vec");
    }

    #[test]
    fn wounds_snapshot_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::WoundsSnapshot(
                WoundsSnapshot {
                    wounds: vec![WoundEntry {
                        part: "arm_l".to_string(),
                        kind: "burn".to_string(),
                        severity: 0.9,
                        state: "healing".to_string(),
                        infection: 0.0,
                        scar: false,
                        updated_at_ms: 999,
                    }],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("WoundsSnapshot envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::WoundsSnapshot(s)) => {
                assert_eq!(s.wounds.len(), 1);
                assert_eq!(s.wounds[0].part, "arm_l");
            }
            other => panic!("期望 WoundsSnapshot payload，实际是 {other:?}"),
        }
    }

    // ─── DefenseWindow ──────────────────────────────────────────

    #[test]
    fn defense_window_roundtrip() {
        let msg = DefenseWindow {
            duration_ms: 200,
            started_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_000_200,
        };
        let bytes = msg.encode_to_vec();
        let decoded = DefenseWindow::decode(bytes.as_slice()).expect("DefenseWindow decode 失败");
        assert_eq!(decoded.duration_ms, 200, "duration_ms 应为 200");
        assert_eq!(decoded.started_at_ms, 1_700_000_000_000);
        assert_eq!(decoded.expires_at_ms, 1_700_000_000_200);
    }

    #[test]
    fn defense_window_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DefenseWindow(
                DefenseWindow {
                    duration_ms: 1000,
                    started_at_ms: 100,
                    expires_at_ms: 1100,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("DefenseWindow envelope decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(server_data_envelope::Payload::DefenseWindow(_))
        ));
    }

    // ─── CastSync ───────────────────────────────────────────────

    #[test]
    fn cast_sync_roundtrip_all_phases() {
        let phases = [
            (CastPhase::Idle, CastOutcome::None),
            (CastPhase::Casting, CastOutcome::None),
            (CastPhase::Complete, CastOutcome::Completed),
            (CastPhase::Interrupt, CastOutcome::InterruptMovement),
            (CastPhase::Interrupt, CastOutcome::InterruptContam),
            (CastPhase::Interrupt, CastOutcome::InterruptControl),
            (CastPhase::Interrupt, CastOutcome::UserCancel),
            (CastPhase::Interrupt, CastOutcome::Death),
        ];
        for (phase, outcome) in phases {
            let msg = CastSync {
                phase: phase as i32,
                slot: 3,
                duration_ms: 1500,
                started_at_ms: 1_700_000_000_000,
                outcome: outcome as i32,
            };
            let bytes = msg.encode_to_vec();
            let decoded = CastSync::decode(bytes.as_slice()).unwrap_or_else(|e| {
                panic!("CastSync phase={phase:?} outcome={outcome:?} decode 失败: {e}")
            });
            assert_eq!(
                decoded.phase, phase as i32,
                "CastSync phase 应为 {phase:?}，实际 {}",
                decoded.phase
            );
            assert_eq!(
                decoded.outcome, outcome as i32,
                "CastSync outcome 应为 {outcome:?}，实际 {}",
                decoded.outcome
            );
            assert_eq!(decoded.slot, 3);
        }
    }

    #[test]
    fn cast_sync_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CastSync(CastSync {
                phase: CastPhase::Casting as i32,
                slot: 5,
                duration_ms: 800,
                started_at_ms: 42,
                outcome: CastOutcome::None as i32,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("CastSync envelope decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::CastSync(c)) => {
                assert_eq!(c.slot, 5);
                assert_eq!(c.phase, CastPhase::Casting as i32);
            }
            other => panic!("期望 CastSync payload，实际是 {other:?}"),
        }
    }

    // ─── QuickSlotConfig ────────────────────────────────────────

    #[test]
    fn quick_slot_config_roundtrip_mixed_slots() {
        let msg = QuickSlotConfig {
            slots: vec![
                OptionalQuickSlotEntry {
                    entry: Some(QuickSlotEntry {
                        item_id: "kai_mai_pill".to_string(),
                        display_name: "开脉丹".to_string(),
                        cast_duration_ms: 1500,
                        cooldown_ms: 1500,
                        icon_texture: "bong:pill.png".to_string(),
                    }),
                },
                OptionalQuickSlotEntry { entry: None },
                OptionalQuickSlotEntry { entry: None },
            ],
            cooldown_until_ms: vec![1_700_000_001_500, 0, 0],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            QuickSlotConfig::decode(bytes.as_slice()).expect("QuickSlotConfig decode 失败");
        assert_eq!(decoded.slots.len(), 3);
        assert!(decoded.slots[0].entry.is_some(), "槽 0 应有条目");
        assert!(decoded.slots[1].entry.is_none(), "槽 1 应为空");
        assert_eq!(decoded.cooldown_until_ms[0], 1_700_000_001_500);
        assert_eq!(
            decoded.slots[0].entry.as_ref().unwrap().item_id,
            "kai_mai_pill"
        );
    }

    // ─── SkillBarConfig ─────────────────────────────────────────

    #[test]
    fn skill_bar_config_roundtrip_item_and_skill() {
        let msg = SkillBarConfig {
            slots: vec![
                OptionalSkillBarEntry {
                    entry: Some(SkillBarEntry {
                        kind: Some(skill_bar_entry::Kind::Skill(SkillBarEntrySkill {
                            skill_id: "burst_meridian.beng_quan".to_string(),
                            display_name: "崩拳".to_string(),
                            cast_duration_ms: 400,
                            cooldown_ms: 3000,
                            icon_texture: "bong:beng_quan.png".to_string(),
                        })),
                    }),
                },
                OptionalSkillBarEntry {
                    entry: Some(SkillBarEntry {
                        kind: Some(skill_bar_entry::Kind::Item(SkillBarEntryItem {
                            template_id: "iron_sword".to_string(),
                            display_name: "铁剑".to_string(),
                            cast_duration_ms: 0,
                            cooldown_ms: 0,
                            icon_texture: "bong:iron_sword.png".to_string(),
                        })),
                    }),
                },
                OptionalSkillBarEntry { entry: None },
            ],
            cooldown_until_ms: vec![3000, 0, 0],
        };
        let bytes = msg.encode_to_vec();
        let decoded = SkillBarConfig::decode(bytes.as_slice()).expect("SkillBarConfig decode 失败");
        assert_eq!(decoded.slots.len(), 3);
        match &decoded.slots[0].entry.as_ref().unwrap().kind {
            Some(skill_bar_entry::Kind::Skill(s)) => {
                assert_eq!(s.skill_id, "burst_meridian.beng_quan");
            }
            other => panic!("槽 0 应为 Skill variant，实际 {other:?}"),
        }
        match &decoded.slots[1].entry.as_ref().unwrap().kind {
            Some(skill_bar_entry::Kind::Item(i)) => {
                assert_eq!(i.template_id, "iron_sword");
            }
            other => panic!("槽 1 应为 Item variant，实际 {other:?}"),
        }
        assert!(decoded.slots[2].entry.is_none(), "槽 2 应为空");
    }

    // ─── TechniquesSnapshot ─────────────────────────────────────

    #[test]
    fn techniques_snapshot_roundtrip() {
        let msg = TechniquesSnapshot {
            entries: vec![TechniqueEntry {
                id: "burst_meridian.beng_quan".to_string(),
                display_name: "崩拳".to_string(),
                grade: "yellow".to_string(),
                proficiency: 0.5,
                proficiency_label: "熟练".to_string(),
                active: true,
                description: "以臂经爆发短劲".to_string(),
                required_realm: "Induce".to_string(),
                required_meridians: vec![TechniqueRequiredMeridian {
                    channel: "LargeIntestine".to_string(),
                    min_health: 0.01,
                }],
                qi_cost: 0.4,
                stamina_cost: 0.0,
                cast_ticks: 8,
                cooldown_ticks: 60,
                range: 1.3,
            }],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            TechniquesSnapshot::decode(bytes.as_slice()).expect("TechniquesSnapshot decode 失败");
        assert_eq!(decoded.entries.len(), 1);
        let e = &decoded.entries[0];
        assert_eq!(e.id, "burst_meridian.beng_quan");
        assert_eq!(e.required_meridians.len(), 1);
        assert_eq!(e.required_meridians[0].channel, "LargeIntestine");
        assert_eq!(e.cast_ticks, 8);
        assert_eq!(e.range, 1.3_f32);
    }

    #[test]
    fn techniques_snapshot_empty() {
        let msg = TechniquesSnapshot { entries: vec![] };
        let bytes = msg.encode_to_vec();
        let decoded = TechniquesSnapshot::decode(bytes.as_slice())
            .expect("空 TechniquesSnapshot decode 失败");
        assert!(decoded.entries.is_empty());
    }

    // ─── UnlocksSync ────────────────────────────────────────────

    #[test]
    fn unlocks_sync_roundtrip() {
        let msg = UnlocksSync {
            jiemai: true,
            tishi: false,
            jueling: true,
        };
        let bytes = msg.encode_to_vec();
        let decoded = UnlocksSync::decode(bytes.as_slice()).expect("UnlocksSync decode 失败");
        assert!(decoded.jiemai, "jiemai 应为 true");
        assert!(!decoded.tishi, "tishi 应为 false");
        assert!(decoded.jueling, "jueling 应为 true");
    }

    // ─── DerivedAttrsSync ───────────────────────────────────────

    #[test]
    fn derived_attrs_sync_roundtrip() {
        let msg = DerivedAttrsSync {
            flying: true,
            flying_qi_remaining: 42.5,
            flying_force_descent_at_ms: 1_700_000_000_000,
            phasing: false,
            phasing_until_ms: 0,
            tribulation_locked: true,
            tribulation_stage: "gather".to_string(),
            throughput_peak_norm: 0.8,
            tuike_layers: 3,
            vortex_active: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            DerivedAttrsSync::decode(bytes.as_slice()).expect("DerivedAttrsSync decode 失败");
        assert!(decoded.flying, "flying 应为 true");
        assert_eq!(decoded.flying_qi_remaining, 42.5_f32);
        assert!(decoded.tribulation_locked);
        assert_eq!(decoded.tribulation_stage, "gather");
        assert_eq!(decoded.tuike_layers, 3);
    }

    // ─── EventStreamPush ────────────────────────────────────────

    #[test]
    fn event_stream_push_roundtrip_all_channels() {
        let channels = [
            EventChannel::Combat,
            EventChannel::Cultivation,
            EventChannel::World,
            EventChannel::Social,
            EventChannel::System,
        ];
        for channel in channels {
            let msg = EventStreamPush {
                channel: channel as i32,
                priority: EventPriority::P1Important as i32,
                source_tag: "test".to_string(),
                text: "事件文本".to_string(),
                color: 0xFF0000,
                created_at_ms: 42,
            };
            let bytes = msg.encode_to_vec();
            let decoded = EventStreamPush::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("EventStreamPush channel={channel:?} decode 失败: {e}"));
            assert_eq!(decoded.channel, channel as i32, "channel 应为 {channel:?}");
        }
    }

    #[test]
    fn event_stream_push_all_priorities() {
        let priorities = [
            EventPriority::P0Critical,
            EventPriority::P1Important,
            EventPriority::P2Normal,
            EventPriority::P3Verbose,
        ];
        for prio in priorities {
            let msg = EventStreamPush {
                channel: EventChannel::Combat as i32,
                priority: prio as i32,
                source_tag: "test".to_string(),
                text: "t".to_string(),
                color: 0,
                created_at_ms: 0,
            };
            let bytes = msg.encode_to_vec();
            let decoded = EventStreamPush::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("EventStreamPush prio={prio:?} decode 失败: {e}"));
            assert_eq!(decoded.priority, prio as i32);
        }
    }

    // ─── WeaponEquipped ─────────────────────────────────────────

    #[test]
    fn weapon_equipped_roundtrip_with_weapon() {
        let msg = WeaponEquipped {
            slot: "main_hand".to_string(),
            weapon: Some(WeaponView {
                instance_id: 42,
                template_id: "iron_sword".to_string(),
                weapon_kind: "sword".to_string(),
                durability_current: 185.0,
                durability_max: 200.0,
                quality_tier: 1,
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded = WeaponEquipped::decode(bytes.as_slice()).expect("WeaponEquipped decode 失败");
        assert_eq!(decoded.slot, "main_hand");
        let w = decoded.weapon.expect("weapon 应为 Some");
        assert_eq!(w.instance_id, 42);
        assert_eq!(w.template_id, "iron_sword");
        assert_eq!(w.quality_tier, 1);
    }

    #[test]
    fn weapon_equipped_roundtrip_without_weapon() {
        let msg = WeaponEquipped {
            slot: "off_hand".to_string(),
            weapon: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            WeaponEquipped::decode(bytes.as_slice()).expect("WeaponEquipped 无武器 decode 失败");
        assert_eq!(decoded.slot, "off_hand");
        assert!(decoded.weapon.is_none(), "weapon 应为 None");
    }

    // ─── WeaponBroken ───────────────────────────────────────────

    #[test]
    fn weapon_broken_roundtrip() {
        let msg = WeaponBroken {
            instance_id: 77,
            template_id: "bone_dagger".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = WeaponBroken::decode(bytes.as_slice()).expect("WeaponBroken decode 失败");
        assert_eq!(decoded.instance_id, 77);
        assert_eq!(decoded.template_id, "bone_dagger");
    }

    // ─── TreasureEquipped ───────────────────────────────────────

    #[test]
    fn treasure_equipped_roundtrip_with_treasure() {
        let msg = TreasureEquipped {
            slot: "treasure_belt_0".to_string(),
            treasure: Some(TreasureView {
                instance_id: 88,
                template_id: "starter_talisman".to_string(),
                display_name: "启程护符".to_string(),
            }),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            TreasureEquipped::decode(bytes.as_slice()).expect("TreasureEquipped decode 失败");
        assert_eq!(decoded.slot, "treasure_belt_0");
        let t = decoded.treasure.expect("treasure 应为 Some");
        assert_eq!(t.display_name, "启程护符");
    }

    #[test]
    fn treasure_equipped_roundtrip_without_treasure() {
        let msg = TreasureEquipped {
            slot: "treasure_belt_1".to_string(),
            treasure: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded = TreasureEquipped::decode(bytes.as_slice())
            .expect("TreasureEquipped 无法宝 decode 失败");
        assert!(decoded.treasure.is_none());
    }

    // ─── VortexFieldState ───────────────────────────────────────

    #[test]
    fn vortex_field_state_roundtrip() {
        let msg = VortexFieldState {
            caster: "player:a".to_string(),
            active: true,
            center_x: 100.0,
            center_y: 64.0,
            center_z: -200.0,
            radius: 5.0,
            delta: 0.3,
            env_qi_at_cast: 0.8,
            maintain_remaining_ticks: 100,
            intercepted_count: 2,
            active_skill_id: "woliu.hold".to_string(),
            charge_progress: 0.5,
            cooldown_until_ms: 0,
            backfire_level: "safe".to_string(),
            turbulence_radius: 10.0,
            turbulence_intensity: 0.7,
            turbulence_until_ms: 1_700_000_000_000,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            VortexFieldState::decode(bytes.as_slice()).expect("VortexFieldState decode 失败");
        assert_eq!(decoded.caster, "player:a");
        assert!(decoded.active);
        assert_eq!(decoded.center_x, 100.0);
        assert_eq!(decoded.center_z, -200.0);
        assert_eq!(decoded.maintain_remaining_ticks, 100);
        assert_eq!(decoded.active_skill_id, "woliu.hold");
    }

    // ─── DuguPoisonState ────────────────────────────────────────

    #[test]
    fn dugu_poison_state_roundtrip_active() {
        let msg = DuguPoisonState {
            target: "player:alice".to_string(),
            active: true,
            meridian_id: "Heart".to_string(),
            attacker: "player:bob".to_string(),
            attached_at_tick: 100,
            poisoner_realm_tier: 3,
            loss_per_tick: 0.01,
            flow_capacity_after: 0.5,
            qi_max_after: 80.0,
            server_tick: 200,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            DuguPoisonState::decode(bytes.as_slice()).expect("DuguPoisonState decode 失败");
        assert_eq!(decoded.target, "player:alice");
        assert!(decoded.active);
        assert_eq!(decoded.poisoner_realm_tier, 3);
        assert_eq!(decoded.loss_per_tick, 0.01);
    }

    #[test]
    fn dugu_poison_state_roundtrip_cleared() {
        let msg = DuguPoisonState {
            target: "player:alice".to_string(),
            active: false,
            meridian_id: String::new(),
            attacker: String::new(),
            attached_at_tick: 0,
            poisoner_realm_tier: 0,
            loss_per_tick: 0.0,
            flow_capacity_after: 0.0,
            qi_max_after: 0.0,
            server_tick: 88,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            DuguPoisonState::decode(bytes.as_slice()).expect("cleared DuguPoisonState decode 失败");
        assert!(!decoded.active);
        assert_eq!(decoded.server_tick, 88);
    }

    // ─── PoisonDoseEvent ────────────────────────────────────────

    #[test]
    fn poison_dose_event_roundtrip_all_tags() {
        let tags = [
            PoisonSideEffectTag::QiFocusDrift2h,
            PoisonSideEffectTag::RageBurst30m,
            PoisonSideEffectTag::HallucinTint6h,
            PoisonSideEffectTag::DigestLock6h,
            PoisonSideEffectTag::ToxicityTierUnlock,
        ];
        for tag in tags {
            let msg = PoisonDoseEvent {
                player_entity_id: 7,
                dose_amount: 5.0,
                side_effect_tag: tag as i32,
                poison_level_after: 5.0,
                digestion_after: 20.0,
                at_tick: 10,
            };
            let bytes = msg.encode_to_vec();
            let decoded = PoisonDoseEvent::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("PoisonDoseEvent tag={tag:?} decode 失败: {e}"));
            assert_eq!(
                decoded.side_effect_tag, tag as i32,
                "side_effect_tag 应为 {tag:?}"
            );
            assert_eq!(decoded.dose_amount, 5.0_f32);
        }
    }

    // ─── PoisonOverdoseEvent ────────────────────────────────────

    #[test]
    fn poison_overdose_event_roundtrip_all_severities() {
        let severities = [
            PoisonOverdoseSeverity::Mild,
            PoisonOverdoseSeverity::Moderate,
            PoisonOverdoseSeverity::Severe,
        ];
        for sev in severities {
            let msg = PoisonOverdoseEvent {
                player_entity_id: 7,
                severity: sev as i32,
                overflow: 1.5,
                lifespan_penalty_years: 0.1,
                micro_tear_probability: 0.05,
                at_tick: 10,
            };
            let bytes = msg.encode_to_vec();
            let decoded = PoisonOverdoseEvent::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("PoisonOverdoseEvent sev={sev:?} decode 失败: {e}"));
            assert_eq!(decoded.severity, sev as i32, "severity 应为 {sev:?}");
            assert_eq!(decoded.overflow, 1.5_f32);
        }
    }

    // ─── PoisonTraitState ───────────────────────────────────────

    #[test]
    fn poison_trait_state_roundtrip() {
        let msg = PoisonTraitState {
            player_entity_id: 7,
            poison_toxicity: 5.0,
            digestion_current: 20.0,
            digestion_capacity: 100.0,
            toxicity_tier_unlocked: false,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            PoisonTraitState::decode(bytes.as_slice()).expect("PoisonTraitState decode 失败");
        assert_eq!(decoded.player_entity_id, 7);
        assert_eq!(decoded.poison_toxicity, 5.0_f32);
        assert_eq!(decoded.digestion_capacity, 100.0_f32);
        assert!(!decoded.toxicity_tier_unlocked);
    }

    // ─── CarrierState ───────────────────────────────────────────

    #[test]
    fn carrier_state_roundtrip_with_instance_id() {
        let msg = CarrierState {
            carrier: "bone_chip".to_string(),
            phase: CarrierChargePhase::Charged as i32,
            progress: 1.0,
            sealed_qi: 50.0,
            sealed_qi_initial: 50.0,
            half_life_remaining_ticks: 200,
            item_instance_id: Some(42),
        };
        let bytes = msg.encode_to_vec();
        let decoded = CarrierState::decode(bytes.as_slice()).expect("CarrierState decode 失败");
        assert_eq!(decoded.carrier, "bone_chip");
        assert_eq!(decoded.phase, CarrierChargePhase::Charged as i32);
        assert_eq!(decoded.item_instance_id, Some(42));
    }

    #[test]
    fn carrier_state_roundtrip_without_instance_id() {
        let msg = CarrierState {
            carrier: "lingmu_arrow".to_string(),
            phase: CarrierChargePhase::Idle as i32,
            progress: 0.0,
            sealed_qi: 0.0,
            sealed_qi_initial: 0.0,
            half_life_remaining_ticks: 0,
            item_instance_id: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CarrierState::decode(bytes.as_slice()).expect("CarrierState 无 instance decode 失败");
        assert!(decoded.item_instance_id.is_none());
        assert_eq!(decoded.phase, CarrierChargePhase::Idle as i32);
    }

    #[test]
    fn carrier_charge_phase_all_variants() {
        let phases = [
            CarrierChargePhase::Idle,
            CarrierChargePhase::Charging,
            CarrierChargePhase::Charged,
        ];
        for phase in phases {
            let msg = CarrierState {
                carrier: "test".to_string(),
                phase: phase as i32,
                progress: 0.0,
                sealed_qi: 0.0,
                sealed_qi_initial: 0.0,
                half_life_remaining_ticks: 0,
                item_instance_id: None,
            };
            let bytes = msg.encode_to_vec();
            let decoded = CarrierState::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("CarrierState phase={phase:?} decode 失败: {e}"));
            assert_eq!(decoded.phase, phase as i32);
        }
    }

    // ─── FalseSkinState ─────────────────────────────────────────

    #[test]
    fn false_skin_state_roundtrip_with_layers() {
        let msg = FalseSkinState {
            target_id: "offline:Azure".to_string(),
            kind: Some(FalseSkinKind::SpiderSilk as i32),
            layers_remaining: 2,
            contam_capacity_per_layer: 30.0,
            absorbed_contam: 10.0,
            equipped_at_tick: 100,
            layers: vec![
                FalseSkinLayerState {
                    tier: FalseSkinTier::Light as i32,
                    spirit_quality: 0.5,
                    damage_capacity: 20.0,
                    contam_load: 5.0,
                    permanent_taint_load: 0.1,
                },
                FalseSkinLayerState {
                    tier: FalseSkinTier::Ancient as i32,
                    spirit_quality: 0.9,
                    damage_capacity: 50.0,
                    contam_load: 0.0,
                    permanent_taint_load: 0.0,
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded = FalseSkinState::decode(bytes.as_slice()).expect("FalseSkinState decode 失败");
        assert_eq!(decoded.target_id, "offline:Azure");
        assert_eq!(decoded.kind, Some(FalseSkinKind::SpiderSilk as i32));
        assert_eq!(decoded.layers.len(), 2);
        assert_eq!(decoded.layers[0].tier, FalseSkinTier::Light as i32);
        assert_eq!(decoded.layers[1].tier, FalseSkinTier::Ancient as i32);
    }

    #[test]
    fn false_skin_state_roundtrip_empty() {
        let msg = FalseSkinState {
            target_id: "test".to_string(),
            kind: None,
            layers_remaining: 0,
            contam_capacity_per_layer: 0.0,
            absorbed_contam: 0.0,
            equipped_at_tick: 0,
            layers: vec![],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            FalseSkinState::decode(bytes.as_slice()).expect("空 FalseSkinState decode 失败");
        assert!(decoded.kind.is_none());
        assert!(decoded.layers.is_empty());
    }

    #[test]
    fn false_skin_kind_all_variants() {
        let kinds = [FalseSkinKind::SpiderSilk, FalseSkinKind::RottenWoodArmor];
        for kind in kinds {
            assert_ne!(kind as i32, 0, "{kind:?} 不应为 UNSPECIFIED (0)");
        }
    }

    #[test]
    fn false_skin_tier_all_variants() {
        let tiers = [
            FalseSkinTier::Fan,
            FalseSkinTier::Light,
            FalseSkinTier::Mid,
            FalseSkinTier::Heavy,
            FalseSkinTier::Ancient,
        ];
        for tier in tiers {
            assert_ne!(tier as i32, 0, "{tier:?} 不应为 UNSPECIFIED (0)");
        }
    }

    // ─── CombatEventFloater ─────────────────────────────────────

    #[test]
    fn combat_event_floater_roundtrip() {
        let msg = CombatEventFloater {
            events: vec![
                CombatEventFloaterEntry {
                    kind: "damage".to_string(),
                    amount: 25.0,
                    text: "-25".to_string(),
                    x: 100.0,
                    y: 65.0,
                    z: -50.0,
                },
                CombatEventFloaterEntry {
                    kind: "heal".to_string(),
                    amount: 10.0,
                    text: "+10".to_string(),
                    x: 101.0,
                    y: 66.0,
                    z: -49.0,
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            CombatEventFloater::decode(bytes.as_slice()).expect("CombatEventFloater decode 失败");
        assert_eq!(decoded.events.len(), 2);
        assert_eq!(decoded.events[0].kind, "damage");
        assert_eq!(decoded.events[0].amount, 25.0_f32);
        assert_eq!(decoded.events[1].text, "+10");
    }

    #[test]
    fn combat_event_floater_empty() {
        let msg = CombatEventFloater { events: vec![] };
        let bytes = msg.encode_to_vec();
        let decoded = CombatEventFloater::decode(bytes.as_slice())
            .expect("空 CombatEventFloater decode 失败");
        assert!(decoded.events.is_empty());
    }

    // ─── TechniqueProficiencyUpdate ─────────────────────────────

    #[test]
    fn technique_proficiency_update_roundtrip() {
        let msg = TechniqueProficiencyUpdate {
            technique_id: "burst_meridian.beng_quan".to_string(),
            proficiency: 0.75,
            gain: 0.05,
        };
        let bytes = msg.encode_to_vec();
        let decoded = TechniqueProficiencyUpdate::decode(bytes.as_slice())
            .expect("TechniqueProficiencyUpdate decode 失败");
        assert_eq!(decoded.technique_id, "burst_meridian.beng_quan");
        assert_eq!(decoded.proficiency, 0.75_f32);
        assert_eq!(decoded.gain, 0.05_f32);
    }

    // ─── PillBuffStatus ─────────────────────────────────────────

    #[test]
    fn pill_buff_status_roundtrip() {
        let msg = PillBuffStatus {
            buff_id: "kai_mai_buff".to_string(),
            remaining_ticks: 1200,
            effect_multiplier: 1.5,
        };
        let bytes = msg.encode_to_vec();
        let decoded = PillBuffStatus::decode(bytes.as_slice()).expect("PillBuffStatus decode 失败");
        assert_eq!(decoded.buff_id, "kai_mai_buff");
        assert_eq!(decoded.remaining_ticks, 1200);
        assert_eq!(decoded.effect_multiplier, 1.5);
    }

    // ─── SkillConfigSnapshotProto ───────────────────────────────

    #[test]
    fn skill_config_snapshot_proto_roundtrip() {
        let msg = SkillConfigSnapshotProto {
            configs: vec![
                SkillConfigEntry {
                    skill_id: "zhenmai.sever_chain".to_string(),
                    json_config: r#"{"meridian_id":"Pericardium"}"#.to_string(),
                },
                SkillConfigEntry {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                    json_config: r#"{}"#.to_string(),
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded = SkillConfigSnapshotProto::decode(bytes.as_slice())
            .expect("SkillConfigSnapshotProto decode 失败");
        assert_eq!(decoded.configs.len(), 2);
        assert_eq!(decoded.configs[0].skill_id, "zhenmai.sever_chain");
        assert!(decoded.configs[0].json_config.contains("Pericardium"));
    }

    #[test]
    fn skill_config_snapshot_proto_empty() {
        let msg = SkillConfigSnapshotProto { configs: vec![] };
        let bytes = msg.encode_to_vec();
        let decoded = SkillConfigSnapshotProto::decode(bytes.as_slice())
            .expect("空 SkillConfigSnapshotProto decode 失败");
        assert!(decoded.configs.is_empty());
    }

    // ═══════════════════════════════════════════════════════════
    // P2 B2：C2S 战斗 / 暗器 / 技能栏 / 死亡
    // ═══════════════════════════════════════════════════════════

    // ─── Jiemai ─────────────────────────────────────────────────

    #[test]
    fn jiemai_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::Jiemai(Jiemai {})),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("Jiemai C2S envelope decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::Jiemai(_))
        ));
    }

    // ─── ChargeCarrier ──────────────────────────────────────────

    #[test]
    fn charge_carrier_roundtrip_with_slot() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ChargeCarrier(
                ChargeCarrier {
                    slot: Some(AnqiCarrierSlot::MainHand as i32),
                    qi_target: 50.0,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ChargeCarrier C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ChargeCarrier(c)) => {
                assert_eq!(c.slot, Some(AnqiCarrierSlot::MainHand as i32));
                assert_eq!(c.qi_target, 50.0_f32);
            }
            other => panic!("期望 ChargeCarrier payload，实际 {other:?}"),
        }
    }

    #[test]
    fn charge_carrier_roundtrip_without_slot() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ChargeCarrier(
                ChargeCarrier {
                    slot: None,
                    qi_target: 30.0,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("ChargeCarrier 无 slot decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ChargeCarrier(c)) => {
                assert!(c.slot.is_none(), "slot 应为 None");
                assert_eq!(c.qi_target, 30.0_f32);
            }
            other => panic!("期望 ChargeCarrier payload，实际 {other:?}"),
        }
    }

    // ─── ThrowCarrier ───────────────────────────────────────────

    #[test]
    fn throw_carrier_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ThrowCarrier(
                ThrowCarrier {
                    slot: AnqiCarrierSlot::OffHand as i32,
                    dir_x: 0.0,
                    dir_y: 1.0,
                    dir_z: 0.0,
                    power: 0.8,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ThrowCarrier C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ThrowCarrier(t)) => {
                assert_eq!(t.slot, AnqiCarrierSlot::OffHand as i32);
                assert_eq!(t.dir_y, 1.0_f32);
                assert_eq!(t.power, 0.8_f32);
            }
            other => panic!("期望 ThrowCarrier payload，实际 {other:?}"),
        }
    }

    // ─── AnqiContainerSwitch ────────────────────────────────────

    #[test]
    fn anqi_container_switch_roundtrip_cycle() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::AnqiContainerSwitch(
                AnqiContainerSwitch { to: None },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("AnqiContainerSwitch cycle decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::AnqiContainerSwitch(s)) => {
                assert!(s.to.is_none(), "to 应为 None（循环切换）");
            }
            other => panic!("期望 AnqiContainerSwitch payload，实际 {other:?}"),
        }
    }

    #[test]
    fn anqi_container_switch_roundtrip_direct() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::AnqiContainerSwitch(
                AnqiContainerSwitch {
                    to: Some(AnqiContainerKind::Quiver as i32),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("AnqiContainerSwitch direct decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::AnqiContainerSwitch(s)) => {
                assert_eq!(s.to, Some(AnqiContainerKind::Quiver as i32));
            }
            other => panic!("期望 AnqiContainerSwitch payload，实际 {other:?}"),
        }
    }

    #[test]
    fn anqi_container_kind_all_variants() {
        let kinds = [
            AnqiContainerKind::HandSlot,
            AnqiContainerKind::Quiver,
            AnqiContainerKind::PocketPouch,
            AnqiContainerKind::Fenglinghe,
        ];
        for kind in kinds {
            assert_ne!(kind as i32, 0, "{kind:?} 不应为 UNSPECIFIED (0)");
        }
    }

    // ─── UseQuickSlot ───────────────────────────────────────────

    #[test]
    fn use_quick_slot_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::UseQuickSlot(
                UseQuickSlot { slot: 3 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("UseQuickSlot C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::UseQuickSlot(u)) => {
                assert_eq!(u.slot, 3);
            }
            other => panic!("期望 UseQuickSlot payload，实际 {other:?}"),
        }
    }

    // ─── QuickSlotBind ──────────────────────────────────────────

    #[test]
    fn quick_slot_bind_roundtrip_bind() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::QuickSlotBind(
                QuickSlotBind {
                    slot: 1,
                    item_id: Some("kai_mai_pill".to_string()),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("QuickSlotBind bind decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::QuickSlotBind(b)) => {
                assert_eq!(b.slot, 1);
                assert_eq!(b.item_id.as_deref(), Some("kai_mai_pill"));
            }
            other => panic!("期望 QuickSlotBind payload，实际 {other:?}"),
        }
    }

    #[test]
    fn quick_slot_bind_roundtrip_clear() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::QuickSlotBind(
                QuickSlotBind {
                    slot: 5,
                    item_id: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("QuickSlotBind clear decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::QuickSlotBind(b)) => {
                assert_eq!(b.slot, 5);
                assert!(b.item_id.is_none(), "清空槽位 item_id 应为 None");
            }
            other => panic!("期望 QuickSlotBind payload，实际 {other:?}"),
        }
    }

    // ─── SkillBarCast ───────────────────────────────────────────

    #[test]
    fn skill_bar_cast_roundtrip_with_target() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillBarCast(
                SkillBarCast {
                    slot: 0,
                    target: Some("entity:42".to_string()),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillBarCast with target decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillBarCast(c)) => {
                assert_eq!(c.slot, 0);
                assert_eq!(c.target.as_deref(), Some("entity:42"));
            }
            other => panic!("期望 SkillBarCast payload，实际 {other:?}"),
        }
    }

    #[test]
    fn skill_bar_cast_roundtrip_without_target() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillBarCast(
                SkillBarCast {
                    slot: 2,
                    target: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillBarCast no target decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillBarCast(c)) => {
                assert_eq!(c.slot, 2);
                assert!(c.target.is_none());
            }
            other => panic!("期望 SkillBarCast payload，实际 {other:?}"),
        }
    }

    // ─── SkillBarBind ───────────────────────────────────────────

    #[test]
    fn skill_bar_bind_roundtrip_item() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillBarBind(
                SkillBarBind {
                    slot: 1,
                    binding: Some(SkillBarBinding {
                        kind: Some(skill_bar_binding::Kind::Item(SkillBarBindingItem {
                            template_id: "iron_sword".to_string(),
                        })),
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("SkillBarBind item decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillBarBind(b)) => {
                assert_eq!(b.slot, 1);
                match b.binding.as_ref().unwrap().kind.as_ref() {
                    Some(skill_bar_binding::Kind::Item(i)) => {
                        assert_eq!(i.template_id, "iron_sword");
                    }
                    other => panic!("期望 Item binding，实际 {other:?}"),
                }
            }
            other => panic!("期望 SkillBarBind payload，实际 {other:?}"),
        }
    }

    #[test]
    fn skill_bar_bind_roundtrip_skill() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillBarBind(
                SkillBarBind {
                    slot: 2,
                    binding: Some(SkillBarBinding {
                        kind: Some(skill_bar_binding::Kind::Skill(SkillBarBindingSkill {
                            skill_id: "burst_meridian.beng_quan".to_string(),
                        })),
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillBarBind skill decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillBarBind(b)) => {
                assert_eq!(b.slot, 2);
                match b.binding.as_ref().unwrap().kind.as_ref() {
                    Some(skill_bar_binding::Kind::Skill(s)) => {
                        assert_eq!(s.skill_id, "burst_meridian.beng_quan");
                    }
                    other => panic!("期望 Skill binding，实际 {other:?}"),
                }
            }
            other => panic!("期望 SkillBarBind payload，实际 {other:?}"),
        }
    }

    #[test]
    fn skill_bar_bind_roundtrip_clear() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillBarBind(
                SkillBarBind {
                    slot: 0,
                    binding: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillBarBind clear decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillBarBind(b)) => {
                assert_eq!(b.slot, 0);
                assert!(b.binding.is_none(), "清空槽位 binding 应为 None");
            }
            other => panic!("期望 SkillBarBind payload，实际 {other:?}"),
        }
    }

    // ─── SkillConfigIntent ──────────────────────────────────────

    #[test]
    fn skill_config_intent_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SkillConfigIntent(
                SkillConfigIntent {
                    skill_id: "zhenmai.sever_chain".to_string(),
                    json_config: r#"{"meridian_id":"Pericardium","backfire_kind":"tainted_yuan"}"#
                        .to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillConfigIntent C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SkillConfigIntent(s)) => {
                assert_eq!(s.skill_id, "zhenmai.sever_chain");
                assert!(s.json_config.contains("Pericardium"));
            }
            other => panic!("期望 SkillConfigIntent payload，实际 {other:?}"),
        }
    }

    // ─── CombatReincarnate / CombatTerminate / CombatCreateNewCharacter ──

    #[test]
    fn combat_reincarnate_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CombatReincarnate(
                CombatReincarnate {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("CombatReincarnate C2S decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CombatReincarnate(_))
        ));
    }

    #[test]
    fn combat_terminate_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CombatTerminate(
                CombatTerminate {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("CombatTerminate C2S decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CombatTerminate(_))
        ));
    }

    #[test]
    fn combat_create_new_character_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CombatCreateNewCharacter(
                CombatCreateNewCharacter {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("CombatCreateNewCharacter C2S decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CombatCreateNewCharacter(
                _
            ))
        ));
    }

    // ─── B2 全部 S2C envelope 多路复用 roundtrip ─────────────────

    #[test]
    fn b2_s2c_all_envelope_variants_roundtrip() {
        let payloads: Vec<(server_data_envelope::Payload, &str)> = vec![
            (
                server_data_envelope::Payload::WoundsSnapshot(WoundsSnapshot { wounds: vec![] }),
                "WoundsSnapshot",
            ),
            (
                server_data_envelope::Payload::DefenseWindow(DefenseWindow {
                    duration_ms: 100,
                    started_at_ms: 0,
                    expires_at_ms: 100,
                }),
                "DefenseWindow",
            ),
            (
                server_data_envelope::Payload::CastSync(CastSync {
                    phase: CastPhase::Idle as i32,
                    slot: 0,
                    duration_ms: 0,
                    started_at_ms: 0,
                    outcome: CastOutcome::None as i32,
                }),
                "CastSync",
            ),
            (
                server_data_envelope::Payload::QuickSlotConfig(QuickSlotConfig {
                    slots: vec![],
                    cooldown_until_ms: vec![],
                }),
                "QuickSlotConfig",
            ),
            (
                server_data_envelope::Payload::SkillBarConfig(SkillBarConfig {
                    slots: vec![],
                    cooldown_until_ms: vec![],
                }),
                "SkillBarConfig",
            ),
            (
                server_data_envelope::Payload::TechniquesSnapshot(TechniquesSnapshot {
                    entries: vec![],
                }),
                "TechniquesSnapshot",
            ),
            (
                server_data_envelope::Payload::UnlocksSync(UnlocksSync {
                    jiemai: false,
                    tishi: false,
                    jueling: false,
                }),
                "UnlocksSync",
            ),
            (
                server_data_envelope::Payload::DerivedAttrsSync(DerivedAttrsSync {
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
                "DerivedAttrsSync",
            ),
            (
                server_data_envelope::Payload::EventStreamPush(EventStreamPush {
                    channel: EventChannel::Combat as i32,
                    priority: EventPriority::P2Normal as i32,
                    source_tag: "test".to_string(),
                    text: "t".to_string(),
                    color: 0,
                    created_at_ms: 0,
                }),
                "EventStreamPush",
            ),
            (
                server_data_envelope::Payload::WeaponEquipped(WeaponEquipped {
                    slot: "main_hand".to_string(),
                    weapon: None,
                }),
                "WeaponEquipped",
            ),
            (
                server_data_envelope::Payload::WeaponBroken(WeaponBroken {
                    instance_id: 1,
                    template_id: "t".to_string(),
                }),
                "WeaponBroken",
            ),
            (
                server_data_envelope::Payload::TreasureEquipped(TreasureEquipped {
                    slot: "treasure_belt_0".to_string(),
                    treasure: None,
                }),
                "TreasureEquipped",
            ),
            (
                server_data_envelope::Payload::VortexState(VortexFieldState {
                    caster: "a".to_string(),
                    active: false,
                    center_x: 0.0,
                    center_y: 0.0,
                    center_z: 0.0,
                    radius: 0.0,
                    delta: 0.0,
                    env_qi_at_cast: 0.0,
                    maintain_remaining_ticks: 0,
                    intercepted_count: 0,
                    active_skill_id: String::new(),
                    charge_progress: 0.0,
                    cooldown_until_ms: 0,
                    backfire_level: String::new(),
                    turbulence_radius: 0.0,
                    turbulence_intensity: 0.0,
                    turbulence_until_ms: 0,
                }),
                "VortexState",
            ),
            (
                server_data_envelope::Payload::DuguPoisonState(DuguPoisonState {
                    target: "t".to_string(),
                    active: false,
                    meridian_id: String::new(),
                    attacker: String::new(),
                    attached_at_tick: 0,
                    poisoner_realm_tier: 0,
                    loss_per_tick: 0.0,
                    flow_capacity_after: 0.0,
                    qi_max_after: 0.0,
                    server_tick: 0,
                }),
                "DuguPoisonState",
            ),
            (
                server_data_envelope::Payload::PoisonDoseEvent(PoisonDoseEvent {
                    player_entity_id: 1,
                    dose_amount: 0.0,
                    side_effect_tag: PoisonSideEffectTag::QiFocusDrift2h as i32,
                    poison_level_after: 0.0,
                    digestion_after: 0.0,
                    at_tick: 0,
                }),
                "PoisonDoseEvent",
            ),
            (
                server_data_envelope::Payload::PoisonOverdoseEvent(PoisonOverdoseEvent {
                    player_entity_id: 1,
                    severity: PoisonOverdoseSeverity::Mild as i32,
                    overflow: 0.0,
                    lifespan_penalty_years: 0.0,
                    micro_tear_probability: 0.0,
                    at_tick: 0,
                }),
                "PoisonOverdoseEvent",
            ),
            (
                server_data_envelope::Payload::PoisonTraitState(PoisonTraitState {
                    player_entity_id: 1,
                    poison_toxicity: 0.0,
                    digestion_current: 0.0,
                    digestion_capacity: 0.0,
                    toxicity_tier_unlocked: false,
                }),
                "PoisonTraitState",
            ),
            (
                server_data_envelope::Payload::CarrierState(CarrierState {
                    carrier: "bone_chip".to_string(),
                    phase: CarrierChargePhase::Idle as i32,
                    progress: 0.0,
                    sealed_qi: 0.0,
                    sealed_qi_initial: 0.0,
                    half_life_remaining_ticks: 0,
                    item_instance_id: None,
                }),
                "CarrierState",
            ),
            (
                server_data_envelope::Payload::FalseSkinState(FalseSkinState {
                    target_id: "t".to_string(),
                    kind: None,
                    layers_remaining: 0,
                    contam_capacity_per_layer: 0.0,
                    absorbed_contam: 0.0,
                    equipped_at_tick: 0,
                    layers: vec![],
                }),
                "FalseSkinState",
            ),
            (
                server_data_envelope::Payload::CombatEventFloater(CombatEventFloater {
                    events: vec![],
                }),
                "CombatEventFloater",
            ),
            (
                server_data_envelope::Payload::TechniqueProficiencyUpdate(
                    TechniqueProficiencyUpdate {
                        technique_id: "t".to_string(),
                        proficiency: 0.0,
                        gain: 0.0,
                    },
                ),
                "TechniqueProficiencyUpdate",
            ),
            (
                server_data_envelope::Payload::PillBuffStatus(PillBuffStatus {
                    buff_id: "b".to_string(),
                    remaining_ticks: 0,
                    effect_multiplier: 0.0,
                }),
                "PillBuffStatus",
            ),
            (
                server_data_envelope::Payload::SkillConfigSnapshot(SkillConfigSnapshotProto {
                    configs: vec![],
                }),
                "SkillConfigSnapshot",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ─── B2 全部 C2S envelope 多路复用 roundtrip ─────────────────

    #[test]
    fn b2_c2s_all_envelope_variants_roundtrip() {
        let payloads: Vec<(client_request_envelope::Payload, &str)> = vec![
            (
                client_request_envelope::Payload::Jiemai(Jiemai {}),
                "Jiemai",
            ),
            (
                client_request_envelope::Payload::ChargeCarrier(ChargeCarrier {
                    slot: None,
                    qi_target: 0.0,
                }),
                "ChargeCarrier",
            ),
            (
                client_request_envelope::Payload::ThrowCarrier(ThrowCarrier {
                    slot: AnqiCarrierSlot::MainHand as i32,
                    dir_x: 1.0,
                    dir_y: 0.0,
                    dir_z: 0.0,
                    power: 1.0,
                }),
                "ThrowCarrier",
            ),
            (
                client_request_envelope::Payload::AnqiContainerSwitch(AnqiContainerSwitch {
                    to: None,
                }),
                "AnqiContainerSwitch",
            ),
            (
                client_request_envelope::Payload::UseQuickSlot(UseQuickSlot { slot: 0 }),
                "UseQuickSlot",
            ),
            (
                client_request_envelope::Payload::QuickSlotBind(QuickSlotBind {
                    slot: 0,
                    item_id: None,
                }),
                "QuickSlotBind",
            ),
            (
                client_request_envelope::Payload::SkillBarCast(SkillBarCast {
                    slot: 0,
                    target: None,
                }),
                "SkillBarCast",
            ),
            (
                client_request_envelope::Payload::SkillBarBind(SkillBarBind {
                    slot: 0,
                    binding: None,
                }),
                "SkillBarBind",
            ),
            (
                client_request_envelope::Payload::SkillConfigIntent(SkillConfigIntent {
                    skill_id: "x".to_string(),
                    json_config: "{}".to_string(),
                }),
                "SkillConfigIntent",
            ),
            (
                client_request_envelope::Payload::CombatReincarnate(CombatReincarnate {}),
                "CombatReincarnate",
            ),
            (
                client_request_envelope::Payload::CombatTerminate(CombatTerminate {}),
                "CombatTerminate",
            ),
            (
                client_request_envelope::Payload::CombatCreateNewCharacter(
                    CombatCreateNewCharacter {},
                ),
                "CombatCreateNewCharacter",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ClientRequestEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} C2S envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} C2S envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B3 — 社交 / NPC / 身份 / 交易 / 领地 roundtrip 测试
    // ═══════════════════════════════════════════════════════════════

    // ─── ExposureKind enum pin ──────────────────────────────────

    #[test]
    fn exposure_kind_enum_has_all_4_variants_plus_unspecified() {
        let expected = [
            (ExposureKind::Unspecified, 0),
            (ExposureKind::Chat, 1),
            (ExposureKind::Trade, 2),
            (ExposureKind::Divine, 3),
            (ExposureKind::Death, 4),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "ExposureKind::{variant:?} wire value 应为 {wire}"
            );
        }
    }

    // ─── GuardianKind enum pin ──────────────────────────────────

    #[test]
    fn guardian_kind_enum_has_all_3_variants_plus_unspecified() {
        let expected = [
            (GuardianKind::Unspecified, 0),
            (GuardianKind::Puppet, 1),
            (GuardianKind::ZhenfaTrap, 2),
            (GuardianKind::BondedDaoxiang, 3),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "GuardianKind::{variant:?} wire value 应为 {wire}"
            );
        }
    }

    // ─── RevealedTagKind enum pin ───────────────────────────────

    #[test]
    fn revealed_tag_kind_enum_has_all_10_variants_plus_unspecified() {
        let expected = [
            (RevealedTagKind::Unspecified, 0),
            (RevealedTagKind::DuguRevealed, 1),
            (RevealedTagKind::AnqiMaster, 2),
            (RevealedTagKind::ZhenfaMaster, 3),
            (RevealedTagKind::BaomaiUser, 4),
            (RevealedTagKind::TuikeUser, 5),
            (RevealedTagKind::WoliuMaster, 6),
            (RevealedTagKind::ZhenmaiUser, 7),
            (RevealedTagKind::SwordMaster, 8),
            (RevealedTagKind::ForgeMaster, 9),
            (RevealedTagKind::AlchemyMaster, 10),
        ];
        for (variant, wire) in expected {
            assert_eq!(
                variant as i32, wire,
                "RevealedTagKind::{variant:?} wire value 应为 {wire}"
            );
        }
        assert_eq!(
            expected.len(),
            11,
            "RevealedTagKind 应有 11 个变体（含 UNSPECIFIED）"
        );
    }

    // ─── SocialAnonymity S2C ────────────────────────────────────

    #[test]
    fn social_anonymity_roundtrip_with_remotes() {
        let msg = SocialAnonymity {
            viewer: "offline:kiz".to_string(),
            remotes: vec![
                SocialRemoteIdentity {
                    player_uuid: "11111111-1111-1111-1111-111111111111".to_string(),
                    anonymous: false,
                    display_name: Some("玄锋".to_string()),
                    realm_band: Some("condense".to_string()),
                    breath_hint: Some("cold".to_string()),
                    renown_tags: vec!["sword_master".to_string()],
                },
                SocialRemoteIdentity {
                    player_uuid: "22222222-2222-2222-2222-222222222222".to_string(),
                    anonymous: true,
                    display_name: None,
                    realm_band: None,
                    breath_hint: None,
                    renown_tags: vec![],
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SocialAnonymity::decode(bytes.as_slice()).expect("SocialAnonymity decode 失败");
        assert_eq!(decoded.viewer, "offline:kiz");
        assert_eq!(decoded.remotes.len(), 2);
        assert_eq!(decoded.remotes[0].display_name, Some("玄锋".to_string()));
        assert!(!decoded.remotes[0].anonymous);
        assert!(decoded.remotes[1].anonymous);
        assert_eq!(decoded.remotes[1].display_name, None);
    }

    #[test]
    fn social_anonymity_roundtrip_empty_remotes() {
        let msg = SocialAnonymity {
            viewer: "test".to_string(),
            remotes: vec![],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SocialAnonymity::decode(bytes.as_slice()).expect("SocialAnonymity empty decode 失败");
        assert_eq!(decoded.viewer, "test");
        assert!(decoded.remotes.is_empty());
    }

    // ─── SocialExposure S2C ─────────────────────────────────────

    #[test]
    fn social_exposure_roundtrip_all_exposure_kinds() {
        for kind in [
            ExposureKind::Chat,
            ExposureKind::Trade,
            ExposureKind::Divine,
            ExposureKind::Death,
        ] {
            let msg = SocialExposure {
                actor: "char:alice".to_string(),
                kind: kind as i32,
                witnesses: vec!["char:bob".to_string()],
                tick: 42,
                zone: Some("spawn".to_string()),
            };
            let bytes = msg.encode_to_vec();
            let decoded = SocialExposure::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("SocialExposure kind={kind:?} decode 失败: {e}"));
            assert_eq!(decoded.kind, kind as i32);
            assert_eq!(decoded.actor, "char:alice");
            assert_eq!(decoded.witnesses, vec!["char:bob".to_string()]);
        }
    }

    #[test]
    fn social_exposure_roundtrip_no_zone() {
        let msg = SocialExposure {
            actor: "a".to_string(),
            kind: ExposureKind::Chat as i32,
            witnesses: vec![],
            tick: 0,
            zone: None,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SocialExposure::decode(bytes.as_slice()).expect("SocialExposure no zone decode 失败");
        assert_eq!(decoded.zone, None);
        assert!(decoded.witnesses.is_empty());
    }

    // ─── SocialPact S2C ─────────────────────────────────────────

    #[test]
    fn social_pact_roundtrip_broken_and_intact() {
        for broken in [true, false] {
            let msg = SocialPact {
                left: "char:alice".to_string(),
                right: "char:bob".to_string(),
                terms: "non-aggression".to_string(),
                tick: 1000,
                broken,
            };
            let bytes = msg.encode_to_vec();
            let decoded = SocialPact::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("SocialPact broken={broken} decode 失败: {e}"));
            assert_eq!(decoded.broken, broken);
            assert_eq!(decoded.terms, "non-aggression");
        }
    }

    // ─── SocialFeud S2C ─────────────────────────────────────────

    #[test]
    fn social_feud_roundtrip_with_and_without_place() {
        let with_place = SocialFeud {
            left: "a".to_string(),
            right: "b".to_string(),
            tick: 500,
            place: Some("blood_valley".to_string()),
        };
        let bytes = with_place.encode_to_vec();
        let decoded =
            SocialFeud::decode(bytes.as_slice()).expect("SocialFeud with place decode 失败");
        assert_eq!(decoded.place, Some("blood_valley".to_string()));

        let no_place = SocialFeud {
            left: "a".to_string(),
            right: "b".to_string(),
            tick: 500,
            place: None,
        };
        let bytes = no_place.encode_to_vec();
        let decoded =
            SocialFeud::decode(bytes.as_slice()).expect("SocialFeud no place decode 失败");
        assert_eq!(decoded.place, None);
    }

    // ─── SocialRenownDelta S2C ──────────────────────────────────

    #[test]
    fn social_renown_delta_roundtrip_with_tags() {
        let msg = SocialRenownDelta {
            char_id: "char:kiz".to_string(),
            fame_delta: 100,
            notoriety_delta: -50,
            tags_added: vec![RenownTag {
                tag: "sword_master".to_string(),
                weight: 1.5,
                last_seen_tick: 9999,
                permanent: true,
            }],
            tick: 42000,
            reason: "defeated_elite".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            SocialRenownDelta::decode(bytes.as_slice()).expect("SocialRenownDelta decode 失败");
        assert_eq!(decoded.fame_delta, 100);
        assert_eq!(decoded.notoriety_delta, -50);
        assert_eq!(decoded.tags_added.len(), 1);
        assert_eq!(decoded.tags_added[0].tag, "sword_master");
        assert!(decoded.tags_added[0].permanent);
    }

    #[test]
    fn social_renown_delta_roundtrip_no_tags() {
        let msg = SocialRenownDelta {
            char_id: "char:test".to_string(),
            fame_delta: 0,
            notoriety_delta: 0,
            tags_added: vec![],
            tick: 0,
            reason: "none".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = SocialRenownDelta::decode(bytes.as_slice())
            .expect("SocialRenownDelta empty tags decode 失败");
        assert!(decoded.tags_added.is_empty());
    }

    // ─── IdentityPanelState S2C ─────────────────────────────────

    #[test]
    fn identity_panel_state_roundtrip_with_entries() {
        let msg = IdentityPanelState {
            active_identity_id: 1,
            last_switch_tick: 12000,
            cooldown_remaining_ticks: 12000,
            identities: vec![
                IdentityPanelEntry {
                    identity_id: 0,
                    display_name: "kiz".to_string(),
                    reputation_score: -50,
                    frozen: true,
                    revealed_tag_kinds: vec![RevealedTagKind::DuguRevealed as i32],
                },
                IdentityPanelEntry {
                    identity_id: 1,
                    display_name: "alt".to_string(),
                    reputation_score: 0,
                    frozen: false,
                    revealed_tag_kinds: vec![],
                },
            ],
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            IdentityPanelState::decode(bytes.as_slice()).expect("IdentityPanelState decode 失败");
        assert_eq!(decoded.active_identity_id, 1);
        assert_eq!(decoded.identities.len(), 2);
        assert_eq!(decoded.identities[0].display_name, "kiz");
        assert!(decoded.identities[0].frozen);
        assert_eq!(
            decoded.identities[0].revealed_tag_kinds,
            vec![RevealedTagKind::DuguRevealed as i32]
        );
        assert!(!decoded.identities[1].frozen);
        assert!(decoded.identities[1].revealed_tag_kinds.is_empty());
    }

    #[test]
    fn identity_panel_state_roundtrip_empty() {
        let msg = IdentityPanelState {
            active_identity_id: 0,
            last_switch_tick: 0,
            cooldown_remaining_ticks: 0,
            identities: vec![],
        };
        let bytes = msg.encode_to_vec();
        let decoded = IdentityPanelState::decode(bytes.as_slice())
            .expect("IdentityPanelState empty decode 失败");
        assert!(decoded.identities.is_empty());
    }

    // ─── NicheIntrusion S2C ─────────────────────────────────────

    #[test]
    fn niche_intrusion_roundtrip_with_items() {
        let msg = NicheIntrusion {
            niche_pos_x: 10,
            niche_pos_y: 64,
            niche_pos_z: -20,
            intruder_id: "char:thief".to_string(),
            items_taken: vec![1001, 1002, 1003],
            taint_delta: 0.25,
        };
        let bytes = msg.encode_to_vec();
        let decoded = NicheIntrusion::decode(bytes.as_slice()).expect("NicheIntrusion decode 失败");
        assert_eq!(decoded.niche_pos_x, 10);
        assert_eq!(decoded.niche_pos_y, 64);
        assert_eq!(decoded.niche_pos_z, -20);
        assert_eq!(decoded.intruder_id, "char:thief");
        assert_eq!(decoded.items_taken, vec![1001, 1002, 1003]);
        assert!((decoded.taint_delta - 0.25).abs() < 1e-6);
    }

    #[test]
    fn niche_intrusion_roundtrip_no_items() {
        let msg = NicheIntrusion {
            niche_pos_x: 0,
            niche_pos_y: 0,
            niche_pos_z: 0,
            intruder_id: "t".to_string(),
            items_taken: vec![],
            taint_delta: 0.0,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            NicheIntrusion::decode(bytes.as_slice()).expect("NicheIntrusion empty decode 失败");
        assert!(decoded.items_taken.is_empty());
    }

    // ─── NicheGuardianFatigue S2C ───────────────────────────────

    #[test]
    fn niche_guardian_fatigue_roundtrip_all_guardian_kinds() {
        for kind in [
            GuardianKind::Puppet,
            GuardianKind::ZhenfaTrap,
            GuardianKind::BondedDaoxiang,
        ] {
            let msg = NicheGuardianFatigue {
                guardian_kind: kind as i32,
                charges_remaining: 3,
            };
            let bytes = msg.encode_to_vec();
            let decoded = NicheGuardianFatigue::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("NicheGuardianFatigue kind={kind:?} decode 失败: {e}"));
            assert_eq!(decoded.guardian_kind, kind as i32);
            assert_eq!(decoded.charges_remaining, 3);
        }
    }

    // ─── NicheGuardianBroken S2C ────────────────────────────────

    #[test]
    fn niche_guardian_broken_roundtrip() {
        let msg = NicheGuardianBroken {
            guardian_kind: GuardianKind::Puppet as i32,
            intruder_id: "char:raider".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            NicheGuardianBroken::decode(bytes.as_slice()).expect("NicheGuardianBroken decode 失败");
        assert_eq!(decoded.guardian_kind, GuardianKind::Puppet as i32);
        assert_eq!(decoded.intruder_id, "char:raider");
    }

    // ─── SparringInvite S2C ─────────────────────────────────────

    #[test]
    fn sparring_invite_roundtrip() {
        let msg = SparringInvite {
            invite_id: "sparring:1:a:b".to_string(),
            initiator: "char:alice".to_string(),
            target: "char:bob".to_string(),
            realm_band: "condense".to_string(),
            breath_hint: "fire".to_string(),
            terms: "first_blood".to_string(),
            expires_at_ms: 1700000000000,
        };
        let bytes = msg.encode_to_vec();
        let decoded = SparringInvite::decode(bytes.as_slice()).expect("SparringInvite decode 失败");
        assert_eq!(decoded.invite_id, "sparring:1:a:b");
        assert_eq!(decoded.initiator, "char:alice");
        assert_eq!(decoded.target, "char:bob");
        assert_eq!(decoded.expires_at_ms, 1700000000000);
    }

    // ─── TradeOffer S2C ─────────────────────────────────────────

    #[test]
    fn trade_offer_roundtrip_with_requested_items() {
        let msg = TradeOffer {
            offer_id: "trade:abc".to_string(),
            initiator: "char:alice".to_string(),
            target: "char:bob".to_string(),
            offered_item: Some(TradeItemSummary {
                instance_id: 1001,
                item_id: "iron_sword".to_string(),
                display_name: "铁剑".to_string(),
                stack_count: 1,
            }),
            requested_items: vec![TradeItemSummary {
                instance_id: 2002,
                item_id: "spirit_grass".to_string(),
                display_name: "灵草".to_string(),
                stack_count: 5,
            }],
            expires_at_ms: 1700000000000,
        };
        let bytes = msg.encode_to_vec();
        let decoded = TradeOffer::decode(bytes.as_slice()).expect("TradeOffer decode 失败");
        assert_eq!(decoded.offer_id, "trade:abc");
        assert!(decoded.offered_item.is_some());
        let offered = decoded.offered_item.unwrap();
        assert_eq!(offered.item_id, "iron_sword");
        assert_eq!(offered.display_name, "铁剑");
        assert_eq!(decoded.requested_items.len(), 1);
        assert_eq!(decoded.requested_items[0].stack_count, 5);
    }

    #[test]
    fn trade_offer_roundtrip_no_requested_items() {
        let msg = TradeOffer {
            offer_id: "trade:def".to_string(),
            initiator: "a".to_string(),
            target: "b".to_string(),
            offered_item: Some(TradeItemSummary {
                instance_id: 1,
                item_id: "x".to_string(),
                display_name: "x".to_string(),
                stack_count: 1,
            }),
            requested_items: vec![],
            expires_at_ms: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded =
            TradeOffer::decode(bytes.as_slice()).expect("TradeOffer empty requested decode 失败");
        assert!(decoded.requested_items.is_empty());
    }

    // ─── SpiritNichePlace C2S ───────────────────────────────────

    #[test]
    fn spirit_niche_place_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SpiritNichePlace(
                SpiritNichePlace {
                    x: 11,
                    y: 64,
                    z: 10,
                    item_instance_id: 4242,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SpiritNichePlace C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SpiritNichePlace(p)) => {
                assert_eq!((p.x, p.y, p.z), (11, 64, 10));
                assert_eq!(p.item_instance_id, 4242);
            }
            other => panic!("expected SpiritNichePlace, got {other:?}"),
        }
    }

    // ─── SpiritNicheGaze C2S ────────────────────────────────────

    #[test]
    fn spirit_niche_gaze_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SpiritNicheGaze(
                SpiritNicheGaze {
                    x: 11,
                    y: 64,
                    z: 10,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SpiritNicheGaze C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SpiritNicheGaze(p)) => {
                assert_eq!((p.x, p.y, p.z), (11, 64, 10));
            }
            other => panic!("expected SpiritNicheGaze, got {other:?}"),
        }
    }

    // ─── SpiritNicheMarkCoordinate C2S ──────────────────────────

    #[test]
    fn spirit_niche_mark_coordinate_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SpiritNicheMarkCoordinate(
                SpiritNicheMarkCoordinate {
                    x: 11,
                    y: 64,
                    z: 10,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SpiritNicheMarkCoordinate C2S decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::SpiritNicheMarkCoordinate(
                _
            ))
        ));
    }

    // ─── SpiritNicheActivateGuardian C2S ────────────────────────

    #[test]
    fn spirit_niche_activate_guardian_c2s_roundtrip_all_kinds() {
        for kind in [
            GuardianKind::Puppet,
            GuardianKind::ZhenfaTrap,
            GuardianKind::BondedDaoxiang,
        ] {
            let envelope = ClientRequestEnvelope {
                payload: Some(
                    client_request_envelope::Payload::SpiritNicheActivateGuardian(
                        SpiritNicheActivateGuardian {
                            niche_pos_x: 1,
                            niche_pos_y: 64,
                            niche_pos_z: -2,
                            guardian_kind: kind as i32,
                            materials: vec!["bone_powder".to_string()],
                        },
                    ),
                ),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice()).unwrap_or_else(|e| {
                panic!("SpiritNicheActivateGuardian kind={kind:?} C2S decode 失败: {e}")
            });
            match decoded.payload {
                Some(client_request_envelope::Payload::SpiritNicheActivateGuardian(p)) => {
                    assert_eq!(p.guardian_kind, kind as i32);
                    assert_eq!(p.materials, vec!["bone_powder".to_string()]);
                }
                other => panic!("expected SpiritNicheActivateGuardian, got {other:?}"),
            }
        }
    }

    #[test]
    fn spirit_niche_activate_guardian_c2s_empty_materials() {
        let envelope = ClientRequestEnvelope {
            payload: Some(
                client_request_envelope::Payload::SpiritNicheActivateGuardian(
                    SpiritNicheActivateGuardian {
                        niche_pos_x: 0,
                        niche_pos_y: 0,
                        niche_pos_z: 0,
                        guardian_kind: GuardianKind::Puppet as i32,
                        materials: vec![],
                    },
                ),
            ),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SpiritNicheActivateGuardian empty materials C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SpiritNicheActivateGuardian(p)) => {
                assert!(p.materials.is_empty());
            }
            other => panic!("expected SpiritNicheActivateGuardian, got {other:?}"),
        }
    }

    // ─── SparringInviteResponse C2S ─────────────────────────────

    #[test]
    fn sparring_invite_response_c2s_roundtrip_accept_and_decline() {
        for (accepted, timed_out) in [(true, false), (false, false), (false, true)] {
            let envelope = ClientRequestEnvelope {
                payload: Some(client_request_envelope::Payload::SparringInviteResponse(
                    SparringInviteResponse {
                        invite_id: "sparring:1:a:b".to_string(),
                        accepted,
                        timed_out,
                    },
                )),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice()).unwrap_or_else(|e| {
                panic!("SparringInviteResponse accepted={accepted} timed_out={timed_out} C2S decode 失败: {e}")
            });
            match decoded.payload {
                Some(client_request_envelope::Payload::SparringInviteResponse(p)) => {
                    assert_eq!(p.accepted, accepted);
                    assert_eq!(p.timed_out, timed_out);
                    assert_eq!(p.invite_id, "sparring:1:a:b");
                }
                other => panic!("expected SparringInviteResponse, got {other:?}"),
            }
        }
    }

    // ─── TradeOfferRequest C2S ──────────────────────────────────

    #[test]
    fn trade_offer_request_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::TradeOfferRequest(
                TradeOfferRequest {
                    target: "entity:42".to_string(),
                    offered_instance_id: 1001,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("TradeOfferRequest C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::TradeOfferRequest(p)) => {
                assert_eq!(p.target, "entity:42");
                assert_eq!(p.offered_instance_id, 1001);
            }
            other => panic!("expected TradeOfferRequest, got {other:?}"),
        }
    }

    // ─── TradeOfferResponse C2S ─────────────────────────────────

    #[test]
    fn trade_offer_response_c2s_roundtrip_accepted_and_declined() {
        // Accepted with requested item
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::TradeOfferResponse(
                TradeOfferResponse {
                    offer_id: "trade:abc".to_string(),
                    accepted: true,
                    requested_instance_id: Some(2002),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("TradeOfferResponse accepted C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::TradeOfferResponse(p)) => {
                assert!(p.accepted);
                assert_eq!(p.requested_instance_id, Some(2002));
            }
            other => panic!("expected TradeOfferResponse, got {other:?}"),
        }

        // Declined (no requested item)
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::TradeOfferResponse(
                TradeOfferResponse {
                    offer_id: "trade:abc".to_string(),
                    accepted: false,
                    requested_instance_id: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("TradeOfferResponse declined C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::TradeOfferResponse(p)) => {
                assert!(!p.accepted);
                assert_eq!(p.requested_instance_id, None);
            }
            other => panic!("expected TradeOfferResponse, got {other:?}"),
        }
    }

    // ─── NpcInspectRequest C2S ──────────────────────────────────

    #[test]
    fn npc_inspect_request_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::NpcInspectRequest(
                NpcInspectRequest { npc_entity_id: 42 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("NpcInspectRequest C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::NpcInspectRequest(p)) => {
                assert_eq!(p.npc_entity_id, 42);
            }
            other => panic!("expected NpcInspectRequest, got {other:?}"),
        }
    }

    // ─── NpcDialogueChoice C2S ──────────────────────────────────

    #[test]
    fn npc_dialogue_choice_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::NpcDialogueChoice(
                NpcDialogueChoice {
                    npc_entity_id: 42,
                    option_id: "trade".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("NpcDialogueChoice C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::NpcDialogueChoice(p)) => {
                assert_eq!(p.npc_entity_id, 42);
                assert_eq!(p.option_id, "trade");
            }
            other => panic!("expected NpcDialogueChoice, got {other:?}"),
        }
    }

    // ─── NpcTradeRequest C2S ────────────────────────────────────

    #[test]
    fn npc_trade_request_c2s_roundtrip_with_and_without_offers() {
        // With offered items
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::NpcTradeRequest(
                NpcTradeRequest {
                    npc_entity_id: 42,
                    offered_items: vec![1001, 1002],
                    requested_item_id: "spirit_grass".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("NpcTradeRequest with offers C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::NpcTradeRequest(p)) => {
                assert_eq!(p.npc_entity_id, 42);
                assert_eq!(p.offered_items, vec![1001, 1002]);
                assert_eq!(p.requested_item_id, "spirit_grass");
            }
            other => panic!("expected NpcTradeRequest, got {other:?}"),
        }

        // Without offered items
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::NpcTradeRequest(
                NpcTradeRequest {
                    npc_entity_id: 42,
                    offered_items: vec![],
                    requested_item_id: "spirit_grass".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("NpcTradeRequest no offers C2S decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::NpcTradeRequest(p)) => {
                assert!(p.offered_items.is_empty());
            }
            other => panic!("expected NpcTradeRequest, got {other:?}"),
        }
    }

    // ─── B3 全部 S2C envelope 多路复用 roundtrip ─────────────────

    #[test]
    fn b3_s2c_all_envelope_variants_roundtrip() {
        let payloads: Vec<(server_data_envelope::Payload, &str)> = vec![
            (
                server_data_envelope::Payload::SocialAnonymity(SocialAnonymity {
                    viewer: "test".to_string(),
                    remotes: vec![],
                }),
                "SocialAnonymity",
            ),
            (
                server_data_envelope::Payload::SocialExposure(SocialExposure {
                    actor: "a".to_string(),
                    kind: ExposureKind::Chat as i32,
                    witnesses: vec![],
                    tick: 0,
                    zone: None,
                }),
                "SocialExposure",
            ),
            (
                server_data_envelope::Payload::SocialPact(SocialPact {
                    left: "a".to_string(),
                    right: "b".to_string(),
                    terms: "t".to_string(),
                    tick: 0,
                    broken: false,
                }),
                "SocialPact",
            ),
            (
                server_data_envelope::Payload::SocialFeud(SocialFeud {
                    left: "a".to_string(),
                    right: "b".to_string(),
                    tick: 0,
                    place: None,
                }),
                "SocialFeud",
            ),
            (
                server_data_envelope::Payload::SocialRenownDelta(SocialRenownDelta {
                    char_id: "c".to_string(),
                    fame_delta: 0,
                    notoriety_delta: 0,
                    tags_added: vec![],
                    tick: 0,
                    reason: "r".to_string(),
                }),
                "SocialRenownDelta",
            ),
            (
                server_data_envelope::Payload::IdentityPanelState(IdentityPanelState {
                    active_identity_id: 0,
                    last_switch_tick: 0,
                    cooldown_remaining_ticks: 0,
                    identities: vec![],
                }),
                "IdentityPanelState",
            ),
            (
                server_data_envelope::Payload::NicheIntrusion(NicheIntrusion {
                    niche_pos_x: 0,
                    niche_pos_y: 0,
                    niche_pos_z: 0,
                    intruder_id: "t".to_string(),
                    items_taken: vec![],
                    taint_delta: 0.0,
                }),
                "NicheIntrusion",
            ),
            (
                server_data_envelope::Payload::NicheGuardianFatigue(NicheGuardianFatigue {
                    guardian_kind: GuardianKind::Puppet as i32,
                    charges_remaining: 0,
                }),
                "NicheGuardianFatigue",
            ),
            (
                server_data_envelope::Payload::NicheGuardianBroken(NicheGuardianBroken {
                    guardian_kind: GuardianKind::Puppet as i32,
                    intruder_id: "t".to_string(),
                }),
                "NicheGuardianBroken",
            ),
            (
                server_data_envelope::Payload::SparringInvite(SparringInvite {
                    invite_id: "i".to_string(),
                    initiator: "a".to_string(),
                    target: "b".to_string(),
                    realm_band: "r".to_string(),
                    breath_hint: "h".to_string(),
                    terms: "t".to_string(),
                    expires_at_ms: 0,
                }),
                "SparringInvite",
            ),
            (
                server_data_envelope::Payload::TradeOffer(TradeOffer {
                    offer_id: "o".to_string(),
                    initiator: "a".to_string(),
                    target: "b".to_string(),
                    offered_item: Some(TradeItemSummary {
                        instance_id: 1,
                        item_id: "x".to_string(),
                        display_name: "x".to_string(),
                        stack_count: 1,
                    }),
                    requested_items: vec![],
                    expires_at_ms: 0,
                }),
                "TradeOffer",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B3 S2C envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B3 S2C envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ─── B3 全部 C2S envelope 多路复用 roundtrip ─────────────────

    #[test]
    fn b3_c2s_all_envelope_variants_roundtrip() {
        let payloads: Vec<(client_request_envelope::Payload, &str)> = vec![
            (
                client_request_envelope::Payload::SpiritNichePlace(SpiritNichePlace {
                    x: 0,
                    y: 0,
                    z: 0,
                    item_instance_id: 0,
                }),
                "SpiritNichePlace",
            ),
            (
                client_request_envelope::Payload::SpiritNicheGaze(SpiritNicheGaze {
                    x: 0,
                    y: 0,
                    z: 0,
                }),
                "SpiritNicheGaze",
            ),
            (
                client_request_envelope::Payload::SpiritNicheMarkCoordinate(
                    SpiritNicheMarkCoordinate { x: 0, y: 0, z: 0 },
                ),
                "SpiritNicheMarkCoordinate",
            ),
            (
                client_request_envelope::Payload::SpiritNicheActivateGuardian(
                    SpiritNicheActivateGuardian {
                        niche_pos_x: 0,
                        niche_pos_y: 0,
                        niche_pos_z: 0,
                        guardian_kind: GuardianKind::Puppet as i32,
                        materials: vec![],
                    },
                ),
                "SpiritNicheActivateGuardian",
            ),
            (
                client_request_envelope::Payload::SparringInviteResponse(SparringInviteResponse {
                    invite_id: "i".to_string(),
                    accepted: false,
                    timed_out: false,
                }),
                "SparringInviteResponse",
            ),
            (
                client_request_envelope::Payload::TradeOfferRequest(TradeOfferRequest {
                    target: "t".to_string(),
                    offered_instance_id: 0,
                }),
                "TradeOfferRequest",
            ),
            (
                client_request_envelope::Payload::TradeOfferResponse(TradeOfferResponse {
                    offer_id: "o".to_string(),
                    accepted: false,
                    requested_instance_id: None,
                }),
                "TradeOfferResponse",
            ),
            (
                client_request_envelope::Payload::NpcInspectRequest(NpcInspectRequest {
                    npc_entity_id: 0,
                }),
                "NpcInspectRequest",
            ),
            (
                client_request_envelope::Payload::NpcDialogueChoice(NpcDialogueChoice {
                    npc_entity_id: 0,
                    option_id: "o".to_string(),
                }),
                "NpcDialogueChoice",
            ),
            (
                client_request_envelope::Payload::NpcTradeRequest(NpcTradeRequest {
                    npc_entity_id: 0,
                    offered_items: vec![],
                    requested_item_id: "r".to_string(),
                }),
                "NpcTradeRequest",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ClientRequestEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B3 C2S envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B3 C2S envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B4 — 天劫 / 死亡 / 复活 / 突破 / 境界视觉
    // ═══════════════════════════════════════════════════════════════

    // ─── TribulationState roundtrip ────────────────────────────────

    #[test]
    fn tribulation_state_envelope_roundtrip() {
        let state = TribulationState {
            active: true,
            char_id: "offline:Azure".to_string(),
            actor_name: "Azure".to_string(),
            kind: "du_xu".to_string(),
            phase: "wave".to_string(),
            world_x: 100.5,
            world_z: -200.3,
            wave_current: 2,
            wave_total: 5,
            started_tick: 10000,
            phase_started_tick: 10500,
            next_wave_tick: 11000,
            failed: false,
            half_step_on_success: true,
            participants: vec!["Azure".to_string(), "Observer1".to_string()],
            result: None,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TribulationState(
                state.clone(),
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TribulationState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TribulationState(s)) => {
                assert!(s.active, "active should be true");
                assert_eq!(s.char_id, "offline:Azure");
                assert_eq!(s.actor_name, "Azure");
                assert_eq!(s.kind, "du_xu");
                assert_eq!(s.phase, "wave");
                assert!((s.world_x - 100.5).abs() < 1e-9);
                assert!((s.world_z - (-200.3)).abs() < 1e-9);
                assert_eq!(s.wave_current, 2);
                assert_eq!(s.wave_total, 5);
                assert_eq!(s.started_tick, 10000);
                assert_eq!(s.phase_started_tick, 10500);
                assert_eq!(s.next_wave_tick, 11000);
                assert!(!s.failed);
                assert!(s.half_step_on_success);
                assert_eq!(s.participants.len(), 2);
                assert_eq!(s.result, None, "result should be None");
            }
            other => panic!("expected TribulationState, got {other:?}"),
        }
    }

    #[test]
    fn tribulation_state_cleared_roundtrip() {
        let state = TribulationState {
            active: false,
            char_id: String::new(),
            actor_name: String::new(),
            kind: "du_xu".to_string(),
            phase: "settle".to_string(),
            world_x: 0.0,
            world_z: 0.0,
            wave_current: 0,
            wave_total: 0,
            started_tick: 0,
            phase_started_tick: 0,
            next_wave_tick: 0,
            failed: false,
            half_step_on_success: false,
            participants: vec![],
            result: Some("ascended".to_string()),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TribulationState(state)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("cleared TribulationState decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::TribulationState(s)) => {
                assert!(!s.active);
                assert_eq!(s.result, Some("ascended".to_string()));
            }
            other => panic!("expected cleared TribulationState, got {other:?}"),
        }
    }

    // ─── TribulationBroadcast roundtrip ────────────────────────────

    #[test]
    fn tribulation_broadcast_active_roundtrip() {
        let bc = TribulationBroadcast {
            active: true,
            actor_name: "Elder".to_string(),
            stage: "wave".to_string(),
            world_x: 50.0,
            world_z: 60.0,
            expires_at_ms: 1700000000000,
            spectate_invite: true,
            spectate_distance: 128.0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TribulationBroadcast(
                bc.clone(),
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TribulationBroadcast decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TribulationBroadcast(b)) => {
                assert!(b.active);
                assert_eq!(b.actor_name, "Elder");
                assert_eq!(b.stage, "wave");
                assert!((b.world_x - 50.0).abs() < 1e-9);
                assert!((b.world_z - 60.0).abs() < 1e-9);
                assert_eq!(b.expires_at_ms, 1700000000000);
                assert!(b.spectate_invite);
                assert!((b.spectate_distance - 128.0).abs() < 1e-9);
            }
            other => panic!("expected TribulationBroadcast, got {other:?}"),
        }
    }

    #[test]
    fn tribulation_broadcast_clear_roundtrip() {
        let bc = TribulationBroadcast {
            active: false,
            actor_name: String::new(),
            stage: "done".to_string(),
            world_x: 0.0,
            world_z: 0.0,
            expires_at_ms: 0,
            spectate_invite: false,
            spectate_distance: 0.0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TribulationBroadcast(bc)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("cleared TribulationBroadcast");
        match decoded.payload {
            Some(server_data_envelope::Payload::TribulationBroadcast(b)) => {
                assert!(!b.active);
                assert_eq!(b.stage, "done");
            }
            other => panic!("expected cleared TribulationBroadcast, got {other:?}"),
        }
    }

    // ─── AscensionQuota roundtrip ──────────────────────────────────

    #[test]
    fn ascension_quota_envelope_roundtrip() {
        let quota = AscensionQuota {
            occupied_slots: 3,
            quota_limit: 10,
            available_slots: 7,
            total_world_qi: 50000.0,
            quota_k: 0.0001,
            quota_basis: "natural".to_string(),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::AscensionQuota(quota)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("AscensionQuota decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::AscensionQuota(q)) => {
                assert_eq!(q.occupied_slots, 3);
                assert_eq!(q.quota_limit, 10);
                assert_eq!(q.available_slots, 7);
                assert!((q.total_world_qi - 50000.0).abs() < 1e-3);
                assert!((q.quota_k - 0.0001).abs() < 1e-9);
                assert_eq!(q.quota_basis, "natural");
            }
            other => panic!("expected AscensionQuota, got {other:?}"),
        }
    }

    #[test]
    fn ascension_quota_zero_values_roundtrip() {
        let quota = AscensionQuota {
            occupied_slots: 0,
            quota_limit: 0,
            available_slots: 0,
            total_world_qi: 0.0,
            quota_k: 0.0,
            quota_basis: String::new(),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::AscensionQuota(quota)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("zero AscensionQuota decode 失败");
        assert!(
            decoded.payload.is_some(),
            "zero AscensionQuota should have payload"
        );
    }

    // ─── HeartDemonOffer roundtrip ─────────────────────────────────

    #[test]
    fn heart_demon_offer_envelope_roundtrip() {
        let offer = HeartDemonOffer {
            offer_id: "hd_offer:1".to_string(),
            trigger_id: "du_xu_wave_3".to_string(),
            trigger_label: "渡虚第三波".to_string(),
            realm_label: "凝脉".to_string(),
            composure: 0.75,
            quota_remaining: 2,
            quota_total: 3,
            expires_at_ms: 1700000060000,
            choices: vec![
                HeartDemonOfferChoice {
                    choice_id: "resist".to_string(),
                    category: "willpower".to_string(),
                    title: "坚守本心".to_string(),
                    effect_summary: "composure +0.1".to_string(),
                    flavor: "你的意志如山".to_string(),
                    style_hint: "positive".to_string(),
                },
                HeartDemonOfferChoice {
                    choice_id: "submit".to_string(),
                    category: "corruption".to_string(),
                    title: "顺从心魔".to_string(),
                    effect_summary: "composure -0.3".to_string(),
                    flavor: "黑暗涌来".to_string(),
                    style_hint: "negative".to_string(),
                },
            ],
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::HeartDemonOffer(offer)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("HeartDemonOffer decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::HeartDemonOffer(o)) => {
                assert_eq!(o.offer_id, "hd_offer:1");
                assert_eq!(o.trigger_id, "du_xu_wave_3");
                assert!((o.composure - 0.75).abs() < 1e-9);
                assert_eq!(o.quota_remaining, 2);
                assert_eq!(o.quota_total, 3);
                assert_eq!(o.choices.len(), 2);
                assert_eq!(o.choices[0].choice_id, "resist");
                assert_eq!(o.choices[1].choice_id, "submit");
                assert_eq!(o.choices[0].category, "willpower");
                assert_eq!(o.choices[1].style_hint, "negative");
            }
            other => panic!("expected HeartDemonOffer, got {other:?}"),
        }
    }

    #[test]
    fn heart_demon_offer_empty_choices_roundtrip() {
        let offer = HeartDemonOffer {
            offer_id: "hd_offer:empty".to_string(),
            trigger_id: "t".to_string(),
            trigger_label: String::new(),
            realm_label: String::new(),
            composure: 1.0,
            quota_remaining: 0,
            quota_total: 0,
            expires_at_ms: 0,
            choices: vec![],
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::HeartDemonOffer(offer)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("empty HeartDemonOffer decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::HeartDemonOffer(o)) => {
                assert_eq!(o.choices.len(), 0);
            }
            other => panic!("expected empty HeartDemonOffer, got {other:?}"),
        }
    }

    // ─── BurstMeridianEvent roundtrip ──────────────────────────────

    #[test]
    fn burst_meridian_event_envelope_roundtrip() {
        let event = BurstMeridianEvent {
            skill: "beng_quan".to_string(),
            caster: "offline:Azure".to_string(),
            target: Some("npc_1v0".to_string()),
            tick: 42000,
            overload_ratio: 1.5,
            integrity_snapshot: 0.3,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::BurstMeridianEvent(event)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("BurstMeridianEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::BurstMeridianEvent(e)) => {
                assert_eq!(e.skill, "beng_quan");
                assert_eq!(e.caster, "offline:Azure");
                assert_eq!(e.target, Some("npc_1v0".to_string()));
                assert_eq!(e.tick, 42000);
                assert!((e.overload_ratio - 1.5).abs() < 1e-9);
                assert!((e.integrity_snapshot - 0.3).abs() < 1e-9);
            }
            other => panic!("expected BurstMeridianEvent, got {other:?}"),
        }
    }

    #[test]
    fn burst_meridian_event_no_target_roundtrip() {
        let event = BurstMeridianEvent {
            skill: "vortex_overload".to_string(),
            caster: "offline:Azure".to_string(),
            target: None,
            tick: 50000,
            overload_ratio: 2.0,
            integrity_snapshot: 0.0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::BurstMeridianEvent(event)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("BurstMeridianEvent no target decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::BurstMeridianEvent(e)) => {
                assert_eq!(e.target, None, "target should be None");
            }
            other => panic!("expected BurstMeridianEvent, got {other:?}"),
        }
    }

    // ─── BreakthroughCinematic roundtrip ───────────────────────────

    #[test]
    fn breakthrough_cinematic_envelope_roundtrip() {
        let cin = BreakthroughCinematic {
            actor_id: "offline:Azure".to_string(),
            phase: "ascending".to_string(),
            phase_tick: 100,
            phase_duration_ticks: 300,
            realm_from: "Condense".to_string(),
            realm_to: "Solidify".to_string(),
            result: "success".to_string(),
            interrupted: false,
            world_pos_x: 8.0,
            world_pos_y: 150.0,
            world_pos_z: 8.0,
            visible_radius_blocks: 64.0,
            global: true,
            distant_billboard: true,
            particle_density: 4.0,
            intensity: 0.8,
            season_overlay: "summer".to_string(),
            style: "standard".to_string(),
            at_tick: 90000,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::BreakthroughCinematic(cin)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("BreakthroughCinematic decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::BreakthroughCinematic(c)) => {
                assert_eq!(c.actor_id, "offline:Azure");
                assert_eq!(c.phase, "ascending");
                assert_eq!(c.phase_tick, 100);
                assert_eq!(c.phase_duration_ticks, 300);
                assert_eq!(c.realm_from, "Condense");
                assert_eq!(c.realm_to, "Solidify");
                assert_eq!(c.result, "success");
                assert!(!c.interrupted);
                assert!((c.world_pos_x - 8.0).abs() < 1e-9);
                assert!((c.world_pos_y - 150.0).abs() < 1e-9);
                assert!((c.world_pos_z - 8.0).abs() < 1e-9);
                assert!((c.visible_radius_blocks - 64.0).abs() < 1e-9);
                assert!(c.global);
                assert!(c.distant_billboard);
                assert!((c.particle_density - 4.0).abs() < 1e-5);
                assert!((c.intensity - 0.8).abs() < 1e-5);
                assert_eq!(c.season_overlay, "summer");
                assert_eq!(c.style, "standard");
                assert_eq!(c.at_tick, 90000);
            }
            other => panic!("expected BreakthroughCinematic, got {other:?}"),
        }
    }

    #[test]
    fn breakthrough_cinematic_interrupted_roundtrip() {
        let cin = BreakthroughCinematic {
            actor_id: "npc_1v0".to_string(),
            phase: "disrupted".to_string(),
            phase_tick: 0,
            phase_duration_ticks: 0,
            realm_from: "Induce".to_string(),
            realm_to: "Condense".to_string(),
            result: "failed".to_string(),
            interrupted: true,
            world_pos_x: -50.0,
            world_pos_y: 64.0,
            world_pos_z: 100.0,
            visible_radius_blocks: 32.0,
            global: false,
            distant_billboard: false,
            particle_density: 1.0,
            intensity: 0.2,
            season_overlay: "winter".to_string(),
            style: "minimal".to_string(),
            at_tick: 0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::BreakthroughCinematic(cin)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("interrupted BreakthroughCinematic decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::BreakthroughCinematic(c)) => {
                assert!(c.interrupted);
                assert_eq!(c.result, "failed");
                assert!(!c.global);
            }
            other => panic!("expected interrupted BreakthroughCinematic, got {other:?}"),
        }
    }

    // ─── DeathScreen roundtrip ─────────────────────────────────────

    #[test]
    fn death_screen_full_roundtrip() {
        let ds = DeathScreen {
            visible: true,
            cause: "combat:bleed_out".to_string(),
            luck_remaining: 0.3,
            final_words: vec!["你的修为到此为止".to_string(), "但愿来生...".to_string()],
            countdown_until_ms: 1700000030000,
            can_reincarnate: true,
            can_terminate: false,
            stage: Some(DeathScreenStage::Fortune as i32),
            death_number: Some(3),
            zone_kind: Some(DeathScreenZoneKind::Death as i32),
            lifespan: Some(LifespanPreview {
                years_lived: 85.5,
                cap_by_realm: 120,
                remaining_years: 34.5,
                death_penalty_years: 10,
                tick_rate_multiplier: 1.0,
                is_wind_candle: false,
            }),
            cinematic: Some(DeathCinematicData {
                v: 1,
                character_id: "offline:Azure".to_string(),
                phase: DeathCinematicPhase::Roll as i32,
                phase_tick: 40,
                phase_duration_ticks: 100,
                total_elapsed_ticks: 200,
                total_duration_ticks: 500,
                roll: Some(DeathCinematicRoll {
                    probability: 0.7,
                    threshold: 0.5,
                    luck_value: 0.6,
                    result: DeathRollResult::Survive as i32,
                }),
                insight_text: vec!["一念之间".to_string()],
                is_final: false,
                death_number: 3,
                zone_kind: DeathCinematicZoneKind::Death as i32,
                tsy_death: false,
                rebirth_weakened_ticks: 600,
                skip_predeath: false,
            }),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DeathScreen(ds)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("DeathScreen decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::DeathScreen(d)) => {
                assert!(d.visible);
                assert_eq!(d.cause, "combat:bleed_out");
                assert!((d.luck_remaining - 0.3).abs() < 1e-9);
                assert_eq!(d.final_words.len(), 2);
                assert_eq!(d.countdown_until_ms, 1700000030000);
                assert!(d.can_reincarnate);
                assert!(!d.can_terminate);
                assert_eq!(d.stage, Some(DeathScreenStage::Fortune as i32));
                assert_eq!(d.death_number, Some(3));
                assert_eq!(d.zone_kind, Some(DeathScreenZoneKind::Death as i32));
                // lifespan
                let ls = d.lifespan.expect("lifespan should be Some");
                assert!((ls.years_lived - 85.5).abs() < 1e-9);
                assert_eq!(ls.cap_by_realm, 120);
                assert!((ls.remaining_years - 34.5).abs() < 1e-9);
                assert_eq!(ls.death_penalty_years, 10);
                assert!(!ls.is_wind_candle);
                // cinematic
                let cin = d.cinematic.expect("cinematic should be Some");
                assert_eq!(cin.v, 1);
                assert_eq!(cin.character_id, "offline:Azure");
                assert_eq!(cin.phase, DeathCinematicPhase::Roll as i32);
                let roll = cin.roll.expect("roll should be Some");
                assert!((roll.probability - 0.7).abs() < 1e-9);
                assert_eq!(roll.result, DeathRollResult::Survive as i32);
                assert_eq!(cin.insight_text.len(), 1);
                assert!(!cin.is_final);
                assert!(!cin.tsy_death);
                assert_eq!(cin.rebirth_weakened_ticks, 600);
            }
            other => panic!("expected DeathScreen, got {other:?}"),
        }
    }

    #[test]
    fn death_screen_minimal_roundtrip() {
        let ds = DeathScreen {
            visible: false,
            cause: String::new(),
            luck_remaining: 0.0,
            final_words: vec![],
            countdown_until_ms: 0,
            can_reincarnate: false,
            can_terminate: false,
            stage: None,
            death_number: None,
            zone_kind: None,
            lifespan: None,
            cinematic: None,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DeathScreen(ds)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("minimal DeathScreen decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::DeathScreen(d)) => {
                assert!(!d.visible);
                assert_eq!(d.stage, None);
                assert_eq!(d.death_number, None);
                assert_eq!(d.zone_kind, None);
                assert!(d.lifespan.is_none());
                assert!(d.cinematic.is_none());
            }
            other => panic!("expected minimal DeathScreen, got {other:?}"),
        }
    }

    #[test]
    fn death_screen_tribulation_stage_roundtrip() {
        let ds = DeathScreen {
            visible: true,
            cause: "tribulation:lightning".to_string(),
            luck_remaining: 0.0,
            final_words: vec![],
            countdown_until_ms: 0,
            can_reincarnate: false,
            can_terminate: true,
            stage: Some(DeathScreenStage::Tribulation as i32),
            death_number: Some(1),
            zone_kind: Some(DeathScreenZoneKind::Negative as i32),
            lifespan: None,
            cinematic: None,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DeathScreen(ds)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("tribulation DeathScreen decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::DeathScreen(d)) => {
                assert_eq!(d.stage, Some(DeathScreenStage::Tribulation as i32));
                assert_eq!(d.zone_kind, Some(DeathScreenZoneKind::Negative as i32));
                assert!(d.can_terminate);
            }
            other => panic!("expected tribulation DeathScreen, got {other:?}"),
        }
    }

    // ─── DeathCinematic enum pin tests ─────────────────────────────

    #[test]
    fn death_cinematic_phase_enum_pin() {
        assert_eq!(DeathCinematicPhase::Unspecified as i32, 0);
        assert_eq!(DeathCinematicPhase::Predeath as i32, 1);
        assert_eq!(DeathCinematicPhase::DeathMoment as i32, 2);
        assert_eq!(DeathCinematicPhase::Roll as i32, 3);
        assert_eq!(DeathCinematicPhase::InsightOverlay as i32, 4);
        assert_eq!(DeathCinematicPhase::Darkness as i32, 5);
        assert_eq!(DeathCinematicPhase::Rebirth as i32, 6);
    }

    #[test]
    fn death_roll_result_enum_pin() {
        assert_eq!(DeathRollResult::Unspecified as i32, 0);
        assert_eq!(DeathRollResult::Pending as i32, 1);
        assert_eq!(DeathRollResult::Survive as i32, 2);
        assert_eq!(DeathRollResult::Fall as i32, 3);
        assert_eq!(DeathRollResult::Final as i32, 4);
    }

    #[test]
    fn death_screen_stage_enum_pin() {
        assert_eq!(DeathScreenStage::Unspecified as i32, 0);
        assert_eq!(DeathScreenStage::Fortune as i32, 1);
        assert_eq!(DeathScreenStage::Tribulation as i32, 2);
    }

    #[test]
    fn death_screen_zone_kind_enum_pin() {
        assert_eq!(DeathScreenZoneKind::Unspecified as i32, 0);
        assert_eq!(DeathScreenZoneKind::Ordinary as i32, 1);
        assert_eq!(DeathScreenZoneKind::Death as i32, 2);
        assert_eq!(DeathScreenZoneKind::Negative as i32, 3);
    }

    #[test]
    fn death_cinematic_zone_kind_enum_pin() {
        assert_eq!(DeathCinematicZoneKind::Unspecified as i32, 0);
        assert_eq!(DeathCinematicZoneKind::Ordinary as i32, 1);
        assert_eq!(DeathCinematicZoneKind::Death as i32, 2);
        assert_eq!(DeathCinematicZoneKind::Negative as i32, 3);
    }

    // ─── DeathCinematicData all phases roundtrip ───────────────────

    #[test]
    fn death_cinematic_all_phases_roundtrip() {
        let phases = [
            DeathCinematicPhase::Predeath,
            DeathCinematicPhase::DeathMoment,
            DeathCinematicPhase::Roll,
            DeathCinematicPhase::InsightOverlay,
            DeathCinematicPhase::Darkness,
            DeathCinematicPhase::Rebirth,
        ];
        for phase in phases {
            let cin = DeathCinematicData {
                v: 1,
                character_id: "c".to_string(),
                phase: phase as i32,
                phase_tick: 0,
                phase_duration_ticks: 100,
                total_elapsed_ticks: 0,
                total_duration_ticks: 500,
                roll: Some(DeathCinematicRoll {
                    probability: 0.5,
                    threshold: 0.5,
                    luck_value: 0.5,
                    result: DeathRollResult::Pending as i32,
                }),
                insight_text: vec![],
                is_final: false,
                death_number: 1,
                zone_kind: DeathCinematicZoneKind::Ordinary as i32,
                tsy_death: false,
                rebirth_weakened_ticks: 0,
                skip_predeath: false,
            };
            let ds = DeathScreen {
                visible: true,
                cause: "test".to_string(),
                luck_remaining: 0.0,
                final_words: vec![],
                countdown_until_ms: 0,
                can_reincarnate: false,
                can_terminate: false,
                stage: None,
                death_number: None,
                zone_kind: None,
                lifespan: None,
                cinematic: Some(cin),
            };
            let envelope = ServerDataEnvelope {
                payload: Some(server_data_envelope::Payload::DeathScreen(ds)),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("DeathCinematic phase {phase:?} decode: {e}"));
            match decoded.payload {
                Some(server_data_envelope::Payload::DeathScreen(d)) => {
                    let c = d.cinematic.unwrap();
                    assert_eq!(
                        c.phase, phase as i32,
                        "phase should roundtrip for {phase:?}"
                    );
                }
                other => panic!("expected DeathScreen for phase {phase:?}, got {other:?}"),
            }
        }
    }

    // ─── DeathRollResult all variants roundtrip ────────────────────

    #[test]
    fn death_roll_all_results_roundtrip() {
        let results = [
            DeathRollResult::Pending,
            DeathRollResult::Survive,
            DeathRollResult::Fall,
            DeathRollResult::Final,
        ];
        for result in results {
            let roll = DeathCinematicRoll {
                probability: 0.5,
                threshold: 0.5,
                luck_value: 0.5,
                result: result as i32,
            };
            let cin = DeathCinematicData {
                v: 1,
                character_id: "c".to_string(),
                phase: DeathCinematicPhase::Roll as i32,
                phase_tick: 0,
                phase_duration_ticks: 100,
                total_elapsed_ticks: 0,
                total_duration_ticks: 500,
                roll: Some(roll),
                insight_text: vec![],
                is_final: result == DeathRollResult::Final,
                death_number: 1,
                zone_kind: DeathCinematicZoneKind::Ordinary as i32,
                tsy_death: false,
                rebirth_weakened_ticks: 0,
                skip_predeath: false,
            };
            let ds = DeathScreen {
                visible: true,
                cause: "test".to_string(),
                luck_remaining: 0.0,
                final_words: vec![],
                countdown_until_ms: 0,
                can_reincarnate: false,
                can_terminate: false,
                stage: None,
                death_number: None,
                zone_kind: None,
                lifespan: None,
                cinematic: Some(cin),
            };
            let envelope = ServerDataEnvelope {
                payload: Some(server_data_envelope::Payload::DeathScreen(ds)),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("DeathRoll result {result:?} decode: {e}"));
            match decoded.payload {
                Some(server_data_envelope::Payload::DeathScreen(d)) => {
                    let r = d.cinematic.unwrap().roll.unwrap();
                    assert_eq!(
                        r.result, result as i32,
                        "roll result should roundtrip for {result:?}"
                    );
                }
                other => panic!("expected DeathScreen for result {result:?}, got {other:?}"),
            }
        }
    }

    // ─── TerminateScreen roundtrip ─────────────────────────────────

    #[test]
    fn terminate_screen_envelope_roundtrip() {
        let ts = TerminateScreen {
            visible: true,
            final_words: "归于虚无".to_string(),
            epilogue: "角色终结".to_string(),
            archetype_suggestion: "warrior".to_string(),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TerminateScreen(ts)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TerminateScreen decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TerminateScreen(t)) => {
                assert!(t.visible);
                assert_eq!(t.final_words, "归于虚无");
                assert_eq!(t.epilogue, "角色终结");
                assert_eq!(t.archetype_suggestion, "warrior");
            }
            other => panic!("expected TerminateScreen, got {other:?}"),
        }
    }

    #[test]
    fn terminate_screen_hidden_roundtrip() {
        let ts = TerminateScreen {
            visible: false,
            final_words: String::new(),
            epilogue: String::new(),
            archetype_suggestion: String::new(),
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TerminateScreen(ts)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("hidden TerminateScreen decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::TerminateScreen(t)) => {
                assert!(!t.visible);
            }
            other => panic!("expected hidden TerminateScreen, got {other:?}"),
        }
    }

    // ─── QiColorObserved roundtrip ─────────────────────────────────

    #[test]
    fn qi_color_observed_envelope_roundtrip() {
        let obs = QiColorObserved {
            observer: "offline:Azure".to_string(),
            observed: "npc_1v0".to_string(),
            main: ColorKind::Sharp as i32,
            secondary: Some(ColorKind::Heavy as i32),
            is_chaotic: true,
            is_hunyuan: false,
            realm_diff: -2,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::QiColorObserved(obs)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("QiColorObserved decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::QiColorObserved(o)) => {
                assert_eq!(o.observer, "offline:Azure");
                assert_eq!(o.observed, "npc_1v0");
                assert_eq!(o.main, ColorKind::Sharp as i32);
                assert_eq!(o.secondary, Some(ColorKind::Heavy as i32));
                assert!(o.is_chaotic);
                assert!(!o.is_hunyuan);
                assert_eq!(o.realm_diff, -2);
            }
            other => panic!("expected QiColorObserved, got {other:?}"),
        }
    }

    #[test]
    fn qi_color_observed_no_secondary_roundtrip() {
        let obs = QiColorObserved {
            observer: "a".to_string(),
            observed: "b".to_string(),
            main: ColorKind::Mellow as i32,
            secondary: None,
            is_chaotic: false,
            is_hunyuan: true,
            realm_diff: 3,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::QiColorObserved(obs)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("QiColorObserved no secondary decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::QiColorObserved(o)) => {
                assert_eq!(o.secondary, None);
                assert!(o.is_hunyuan);
                assert_eq!(o.realm_diff, 3);
            }
            other => panic!("expected QiColorObserved, got {other:?}"),
        }
    }

    // ─── RealmVisionParams roundtrip ───────────────────────────────

    #[test]
    fn realm_vision_params_envelope_roundtrip() {
        let params = RealmVisionParams {
            fog_start: 10.0,
            fog_end: 80.0,
            fog_color_rgb: 0x4488CC,
            fog_shape: FogShape::Cylinder as i32,
            vignette_alpha: 0.3,
            tint_color_argb: 0x80FF0000,
            particle_density: 2.5,
            transition_ticks: 40,
            server_view_distance_chunks: 8,
            post_fx_sharpen: 0.1,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::RealmVisionParams(params)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("RealmVisionParams decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::RealmVisionParams(p)) => {
                assert!((p.fog_start - 10.0).abs() < 1e-9);
                assert!((p.fog_end - 80.0).abs() < 1e-9);
                assert_eq!(p.fog_color_rgb, 0x4488CC);
                assert_eq!(p.fog_shape, FogShape::Cylinder as i32);
                assert!((p.vignette_alpha - 0.3).abs() < 1e-9);
                assert_eq!(p.tint_color_argb, 0x80FF0000);
                assert!((p.particle_density - 2.5).abs() < 1e-9);
                assert_eq!(p.transition_ticks, 40);
                assert_eq!(p.server_view_distance_chunks, 8);
                assert!((p.post_fx_sharpen - 0.1).abs() < 1e-9);
            }
            other => panic!("expected RealmVisionParams, got {other:?}"),
        }
    }

    #[test]
    fn realm_vision_params_sphere_fog_roundtrip() {
        let params = RealmVisionParams {
            fog_start: 5.0,
            fog_end: 50.0,
            fog_color_rgb: 0,
            fog_shape: FogShape::Sphere as i32,
            vignette_alpha: 0.0,
            tint_color_argb: 0,
            particle_density: 0.0,
            transition_ticks: 0,
            server_view_distance_chunks: 32,
            post_fx_sharpen: 0.0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::RealmVisionParams(params)),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("sphere RealmVisionParams decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::RealmVisionParams(p)) => {
                assert_eq!(p.fog_shape, FogShape::Sphere as i32);
                assert_eq!(p.server_view_distance_chunks, 32);
            }
            other => panic!("expected sphere RealmVisionParams, got {other:?}"),
        }
    }

    // ─── FogShape enum pin ─────────────────────────────────────────

    #[test]
    fn fog_shape_enum_pin() {
        assert_eq!(FogShape::Unspecified as i32, 0);
        assert_eq!(FogShape::Cylinder as i32, 1);
        assert_eq!(FogShape::Sphere as i32, 2);
    }

    // ─── SpiritualSenseTargets roundtrip ───────────────────────────

    #[test]
    fn spiritual_sense_targets_envelope_roundtrip() {
        let targets = SpiritualSenseTargets {
            entries: vec![
                SenseEntry {
                    kind: SenseKind::LivingQi as i32,
                    x: 100.0,
                    y: 64.0,
                    z: -50.0,
                    intensity: 0.8,
                },
                SenseEntry {
                    kind: SenseKind::CultivatorRealm as i32,
                    x: 200.0,
                    y: 70.0,
                    z: 30.0,
                    intensity: 0.5,
                },
                SenseEntry {
                    kind: SenseKind::ZhenfaArray as i32,
                    x: -10.0,
                    y: 55.0,
                    z: 80.0,
                    intensity: 1.0,
                },
            ],
            generation: 42,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SpiritualSenseTargets(
                targets,
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("SpiritualSenseTargets decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SpiritualSenseTargets(t)) => {
                assert_eq!(t.entries.len(), 3);
                assert_eq!(t.generation, 42);
                assert_eq!(t.entries[0].kind, SenseKind::LivingQi as i32);
                assert!((t.entries[0].x - 100.0).abs() < 1e-9);
                assert!((t.entries[0].intensity - 0.8).abs() < 1e-9);
                assert_eq!(t.entries[1].kind, SenseKind::CultivatorRealm as i32);
                assert_eq!(t.entries[2].kind, SenseKind::ZhenfaArray as i32);
            }
            other => panic!("expected SpiritualSenseTargets, got {other:?}"),
        }
    }

    #[test]
    fn spiritual_sense_targets_empty_roundtrip() {
        let targets = SpiritualSenseTargets {
            entries: vec![],
            generation: 0,
        };
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SpiritualSenseTargets(
                targets,
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("empty SpiritualSenseTargets decode");
        match decoded.payload {
            Some(server_data_envelope::Payload::SpiritualSenseTargets(t)) => {
                assert_eq!(t.entries.len(), 0);
                assert_eq!(t.generation, 0);
            }
            other => panic!("expected empty SpiritualSenseTargets, got {other:?}"),
        }
    }

    // ─── SenseKind enum pin ────────────────────────────────────────

    #[test]
    fn sense_kind_enum_pin() {
        assert_eq!(SenseKind::Unspecified as i32, 0);
        assert_eq!(SenseKind::LivingQi as i32, 1);
        assert_eq!(SenseKind::AmbientLeyline as i32, 2);
        assert_eq!(SenseKind::CultivatorRealm as i32, 3);
        assert_eq!(SenseKind::HeavenlyGaze as i32, 4);
        assert_eq!(SenseKind::CrisisPremonition as i32, 5);
        assert_eq!(SenseKind::ZhenfaArray as i32, 6);
        assert_eq!(SenseKind::ZhenfaWardAlert as i32, 7);
        assert_eq!(SenseKind::SpiritEye as i32, 8);
        assert_eq!(SenseKind::NicheIntrusionTrace as i32, 9);
    }

    // ─── SenseKind all variants roundtrip ──────────────────────────

    #[test]
    fn sense_kind_all_variants_roundtrip() {
        let kinds = [
            SenseKind::LivingQi,
            SenseKind::AmbientLeyline,
            SenseKind::CultivatorRealm,
            SenseKind::HeavenlyGaze,
            SenseKind::CrisisPremonition,
            SenseKind::ZhenfaArray,
            SenseKind::ZhenfaWardAlert,
            SenseKind::SpiritEye,
            SenseKind::NicheIntrusionTrace,
        ];
        for kind in kinds {
            let targets = SpiritualSenseTargets {
                entries: vec![SenseEntry {
                    kind: kind as i32,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    intensity: 0.5,
                }],
                generation: 1,
            };
            let envelope = ServerDataEnvelope {
                payload: Some(server_data_envelope::Payload::SpiritualSenseTargets(
                    targets,
                )),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("SenseKind {kind:?} decode: {e}"));
            match decoded.payload {
                Some(server_data_envelope::Payload::SpiritualSenseTargets(t)) => {
                    assert_eq!(
                        t.entries[0].kind, kind as i32,
                        "kind should roundtrip for {kind:?}"
                    );
                }
                other => panic!("expected SpiritualSenseTargets for {kind:?}, got {other:?}"),
            }
        }
    }

    // ─── B4 S2C all variants distinguishable ───────────────────────

    #[test]
    fn b4_s2c_all_envelope_variants_roundtrip() {
        let payloads: Vec<(server_data_envelope::Payload, &str)> = vec![
            (
                server_data_envelope::Payload::TribulationState(TribulationState {
                    active: true,
                    char_id: "c".to_string(),
                    actor_name: "a".to_string(),
                    kind: "du_xu".to_string(),
                    phase: "omen".to_string(),
                    world_x: 0.0,
                    world_z: 0.0,
                    wave_current: 0,
                    wave_total: 3,
                    started_tick: 0,
                    phase_started_tick: 0,
                    next_wave_tick: 0,
                    failed: false,
                    half_step_on_success: false,
                    participants: vec![],
                    result: None,
                }),
                "TribulationState",
            ),
            (
                server_data_envelope::Payload::TribulationBroadcast(TribulationBroadcast {
                    active: true,
                    actor_name: "a".to_string(),
                    stage: "wave".to_string(),
                    world_x: 0.0,
                    world_z: 0.0,
                    expires_at_ms: 0,
                    spectate_invite: false,
                    spectate_distance: 0.0,
                }),
                "TribulationBroadcast",
            ),
            (
                server_data_envelope::Payload::AscensionQuota(AscensionQuota {
                    occupied_slots: 1,
                    quota_limit: 5,
                    available_slots: 4,
                    total_world_qi: 0.0,
                    quota_k: 0.0,
                    quota_basis: String::new(),
                }),
                "AscensionQuota",
            ),
            (
                server_data_envelope::Payload::HeartDemonOffer(HeartDemonOffer {
                    offer_id: "o".to_string(),
                    trigger_id: "t".to_string(),
                    trigger_label: String::new(),
                    realm_label: String::new(),
                    composure: 1.0,
                    quota_remaining: 0,
                    quota_total: 0,
                    expires_at_ms: 0,
                    choices: vec![],
                }),
                "HeartDemonOffer",
            ),
            (
                server_data_envelope::Payload::BurstMeridianEvent(BurstMeridianEvent {
                    skill: "s".to_string(),
                    caster: "c".to_string(),
                    target: None,
                    tick: 0,
                    overload_ratio: 0.0,
                    integrity_snapshot: 0.0,
                }),
                "BurstMeridianEvent",
            ),
            (
                server_data_envelope::Payload::BreakthroughCinematic(BreakthroughCinematic {
                    actor_id: "a".to_string(),
                    phase: "p".to_string(),
                    phase_tick: 0,
                    phase_duration_ticks: 1,
                    realm_from: "Awaken".to_string(),
                    realm_to: "Induce".to_string(),
                    result: "success".to_string(),
                    interrupted: false,
                    world_pos_x: 0.0,
                    world_pos_y: 0.0,
                    world_pos_z: 0.0,
                    visible_radius_blocks: 1.0,
                    global: false,
                    distant_billboard: false,
                    particle_density: 1.0,
                    intensity: 0.5,
                    season_overlay: String::new(),
                    style: String::new(),
                    at_tick: 0,
                }),
                "BreakthroughCinematic",
            ),
            (
                server_data_envelope::Payload::DeathScreen(DeathScreen {
                    visible: true,
                    cause: "c".to_string(),
                    luck_remaining: 0.0,
                    final_words: vec![],
                    countdown_until_ms: 0,
                    can_reincarnate: false,
                    can_terminate: false,
                    stage: None,
                    death_number: None,
                    zone_kind: None,
                    lifespan: None,
                    cinematic: None,
                }),
                "DeathScreen",
            ),
            (
                server_data_envelope::Payload::TerminateScreen(TerminateScreen {
                    visible: true,
                    final_words: "f".to_string(),
                    epilogue: "e".to_string(),
                    archetype_suggestion: "a".to_string(),
                }),
                "TerminateScreen",
            ),
            (
                server_data_envelope::Payload::QiColorObserved(QiColorObserved {
                    observer: "o".to_string(),
                    observed: "t".to_string(),
                    main: ColorKind::Mellow as i32,
                    secondary: None,
                    is_chaotic: false,
                    is_hunyuan: false,
                    realm_diff: 0,
                }),
                "QiColorObserved",
            ),
            (
                server_data_envelope::Payload::RealmVisionParams(RealmVisionParams {
                    fog_start: 0.0,
                    fog_end: 100.0,
                    fog_color_rgb: 0,
                    fog_shape: FogShape::Cylinder as i32,
                    vignette_alpha: 0.0,
                    tint_color_argb: 0,
                    particle_density: 0.0,
                    transition_ticks: 0,
                    server_view_distance_chunks: 8,
                    post_fx_sharpen: 0.0,
                }),
                "RealmVisionParams",
            ),
            (
                server_data_envelope::Payload::SpiritualSenseTargets(SpiritualSenseTargets {
                    entries: vec![],
                    generation: 0,
                }),
                "SpiritualSenseTargets",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B4 S2C envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B4 S2C envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ─── B4 C2S roundtrip tests ────────────────────────────────────

    #[test]
    fn start_du_xu_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::StartDuXu(StartDuXu {})),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("StartDuXu decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::StartDuXu(_)) => {}
            other => panic!("expected StartDuXu, got {other:?}"),
        }
    }

    #[test]
    fn abort_tribulation_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::AbortTribulation(
                AbortTribulation {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("AbortTribulation decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::AbortTribulation(_)) => {}
            other => panic!("expected AbortTribulation, got {other:?}"),
        }
    }

    #[test]
    fn heart_demon_decision_chosen_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::HeartDemonDecision(
                HeartDemonDecision {
                    choice_idx: Some(2),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("HeartDemonDecision chosen decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::HeartDemonDecision(d)) => {
                assert_eq!(d.choice_idx, Some(2));
            }
            other => panic!("expected HeartDemonDecision, got {other:?}"),
        }
    }

    #[test]
    fn heart_demon_decision_timeout_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::HeartDemonDecision(
                HeartDemonDecision { choice_idx: None },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("HeartDemonDecision timeout decode");
        match decoded.payload {
            Some(client_request_envelope::Payload::HeartDemonDecision(d)) => {
                assert_eq!(d.choice_idx, None, "timeout should yield None");
            }
            other => panic!("expected HeartDemonDecision timeout, got {other:?}"),
        }
    }

    #[test]
    fn duo_she_request_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::DuoSheRequest(
                DuoSheRequest {
                    target_id: "npc_12v0".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("DuoSheRequest decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::DuoSheRequest(d)) => {
                assert_eq!(d.target_id, "npc_12v0");
            }
            other => panic!("expected DuoSheRequest, got {other:?}"),
        }
    }

    #[test]
    fn qi_color_inspect_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::QiColorInspect(
                QiColorInspect {
                    observed: "entity_bits:42".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("QiColorInspect decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::QiColorInspect(q)) => {
                assert_eq!(q.observed, "entity_bits:42");
            }
            other => panic!("expected QiColorInspect, got {other:?}"),
        }
    }

    #[test]
    fn use_life_core_envelope_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::UseLifeCore(UseLifeCore {
                instance_id: 4242,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("UseLifeCore decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::UseLifeCore(u)) => {
                assert_eq!(u.instance_id, 4242);
            }
            other => panic!("expected UseLifeCore, got {other:?}"),
        }
    }

    // ─── B4 C2S all variants distinguishable ───────────────────────

    #[test]
    fn b4_c2s_all_envelope_variants_roundtrip() {
        let payloads: Vec<(client_request_envelope::Payload, &str)> = vec![
            (
                client_request_envelope::Payload::StartDuXu(StartDuXu {}),
                "StartDuXu",
            ),
            (
                client_request_envelope::Payload::AbortTribulation(AbortTribulation {}),
                "AbortTribulation",
            ),
            (
                client_request_envelope::Payload::HeartDemonDecision(HeartDemonDecision {
                    choice_idx: Some(0),
                }),
                "HeartDemonDecision",
            ),
            (
                client_request_envelope::Payload::DuoSheRequest(DuoSheRequest {
                    target_id: "t".to_string(),
                }),
                "DuoSheRequest",
            ),
            (
                client_request_envelope::Payload::QiColorInspect(QiColorInspect {
                    observed: "o".to_string(),
                }),
                "QiColorInspect",
            ),
            (
                client_request_envelope::Payload::UseLifeCore(UseLifeCore { instance_id: 1 }),
                "UseLifeCore",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ClientRequestEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B4 C2S envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B4 C2S envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B5 — S2C roundtrip tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn event_alert_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::EventAlert(EventAlert {
                event: EventKind::ThunderTribulation.into(),
                message: "雷劫降临".to_string(),
                zone: Some("spawn".to_string()),
                duration_ticks: Some(200),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice()).expect("EventAlert decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::EventAlert(e)) => {
                assert_eq!(e.event, EventKind::ThunderTribulation as i32);
                assert_eq!(e.message, "雷劫降临");
                assert_eq!(e.zone, Some("spawn".to_string()));
                assert_eq!(e.duration_ticks, Some(200));
            }
            other => panic!("expected EventAlert, got {other:?}"),
        }
    }

    #[test]
    fn coffin_state_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CoffinState(CoffinState {
                in_coffin: true,
                lifespan_rate_multiplier: 0.5,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("CoffinState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::CoffinState(c)) => {
                assert!(c.in_coffin);
                assert!((c.lifespan_rate_multiplier - 0.5).abs() < 1e-9);
            }
            other => panic!("expected CoffinState, got {other:?}"),
        }
    }

    #[test]
    fn ui_open_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::UiOpen(UiOpen {
                ui: Some("inspect".to_string()),
                xml: "<root/>".to_string(),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice()).expect("UiOpen decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::UiOpen(u)) => {
                assert_eq!(u.ui, Some("inspect".to_string()));
                assert_eq!(u.xml, "<root/>");
            }
            other => panic!("expected UiOpen, got {other:?}"),
        }
    }

    #[test]
    fn inventory_event_moved_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::InventoryEvent(
                InventoryEvent {
                    event: Some(inventory_event::Event::Moved(InventoryEventMoved {
                        revision: 1,
                        instance_id: 42,
                        from: Some(InventoryLocation {
                            location: Some(inventory_location::Location::Container(
                                InventoryLocationContainer {
                                    container_id: "pack_main".to_string(),
                                    row: 0,
                                    col: 1,
                                },
                            )),
                        }),
                        to: Some(InventoryLocation {
                            location: Some(inventory_location::Location::Equip(
                                InventoryLocationEquip {
                                    slot: EquipSlot::MainHand.into(),
                                },
                            )),
                        }),
                    })),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("InventoryEvent Moved decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::InventoryEvent(ie)) => match ie.event {
                Some(inventory_event::Event::Moved(m)) => {
                    assert_eq!(m.revision, 1);
                    assert_eq!(m.instance_id, 42);
                }
                other => panic!("expected Moved, got {other:?}"),
            },
            other => panic!("expected InventoryEvent, got {other:?}"),
        }
    }

    #[test]
    fn inventory_event_stack_changed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::InventoryEvent(
                InventoryEvent {
                    event: Some(inventory_event::Event::StackChanged(
                        InventoryEventStackChanged {
                            revision: 5,
                            instance_id: 99,
                            stack_count: 64,
                        },
                    )),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("InventoryEvent StackChanged decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::InventoryEvent(ie)) => match ie.event {
                Some(inventory_event::Event::StackChanged(s)) => {
                    assert_eq!(s.stack_count, 64);
                }
                other => panic!("expected StackChanged, got {other:?}"),
            },
            other => panic!("expected InventoryEvent, got {other:?}"),
        }
    }

    #[test]
    fn inventory_event_durability_changed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::InventoryEvent(
                InventoryEvent {
                    event: Some(inventory_event::Event::DurabilityChanged(
                        InventoryEventDurabilityChanged {
                            revision: 3,
                            instance_id: 7,
                            durability: 0.75,
                        },
                    )),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("InventoryEvent DurabilityChanged decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::InventoryEvent(ie)) => match ie.event {
                Some(inventory_event::Event::DurabilityChanged(d)) => {
                    assert!((d.durability - 0.75).abs() < 1e-9);
                }
                other => panic!("expected DurabilityChanged, got {other:?}"),
            },
            other => panic!("expected InventoryEvent, got {other:?}"),
        }
    }

    #[test]
    fn dropped_loot_sync_envelope_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DroppedLootSync(
                DroppedLootSync {
                    drops: vec![DroppedLootEntry {
                        instance_id: 1,
                        source_container_id: "pack".to_string(),
                        source_row: 0,
                        source_col: 0,
                        world_pos_x: 10.0,
                        world_pos_y: 64.0,
                        world_pos_z: -5.0,
                        item: Some(InventoryItemView {
                            instance_id: 1,
                            item_id: "bone_coin".to_string(),
                            display_name: "骨币".to_string(),
                            ..Default::default()
                        }),
                    }],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("DroppedLootSync decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::DroppedLootSync(d)) => {
                assert_eq!(d.drops.len(), 1);
                assert_eq!(d.drops[0].instance_id, 1);
            }
            other => panic!("expected DroppedLootSync, got {other:?}"),
        }
    }

    #[test]
    fn rift_portal_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::RiftPortalState(
                RiftPortalState {
                    entity_id: 42,
                    kind: RiftPortalKind::MainRift.into(),
                    direction: RiftPortalDirection::Entry.into(),
                    family_id: "tsy_01".to_string(),
                    world_pos_x: 1.0,
                    world_pos_y: 64.0,
                    world_pos_z: -1.0,
                    trigger_radius: 3.0,
                    current_extract_ticks: 0,
                    activation_window_end: Some(999),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("RiftPortalState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::RiftPortalState(r)) => {
                assert_eq!(r.entity_id, 42);
                assert_eq!(r.kind, RiftPortalKind::MainRift as i32);
                assert_eq!(r.activation_window_end, Some(999));
            }
            other => panic!("expected RiftPortalState, got {other:?}"),
        }
    }

    #[test]
    fn rift_portal_removed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::RiftPortalRemoved(
                RiftPortalRemoved { entity_id: 7 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("RiftPortalRemoved decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::RiftPortalRemoved(r)) => {
                assert_eq!(r.entity_id, 7);
            }
            other => panic!("expected RiftPortalRemoved, got {other:?}"),
        }
    }

    #[test]
    fn extract_started_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ExtractStarted(
                ExtractStarted {
                    player_id: "kiz".to_string(),
                    portal_entity_id: 42,
                    portal_kind: RiftPortalKind::DeepRift.into(),
                    required_ticks: 100,
                    at_tick: 5000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ExtractStarted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ExtractStarted(e)) => {
                assert_eq!(e.player_id, "kiz");
                assert_eq!(e.portal_kind, RiftPortalKind::DeepRift as i32);
            }
            other => panic!("expected ExtractStarted, got {other:?}"),
        }
    }

    #[test]
    fn extract_progress_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ExtractProgress(
                ExtractProgress {
                    player_id: "kiz".to_string(),
                    portal_entity_id: 42,
                    elapsed_ticks: 50,
                    required_ticks: 100,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ExtractProgress decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ExtractProgress(e)) => {
                assert_eq!(e.elapsed_ticks, 50);
            }
            other => panic!("expected ExtractProgress, got {other:?}"),
        }
    }

    #[test]
    fn extract_completed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ExtractCompleted(
                ExtractCompleted {
                    player_id: "kiz".to_string(),
                    portal_kind: RiftPortalKind::CollapseTear.into(),
                    family_id: "tsy_01".to_string(),
                    exit_pos_x: 1.0,
                    exit_pos_y: 65.0,
                    exit_pos_z: 2.0,
                    at_tick: 6000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ExtractCompleted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ExtractCompleted(e)) => {
                assert_eq!(e.portal_kind, RiftPortalKind::CollapseTear as i32);
            }
            other => panic!("expected ExtractCompleted, got {other:?}"),
        }
    }

    #[test]
    fn extract_aborted_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ExtractAborted(
                ExtractAborted {
                    player_id: "kiz".to_string(),
                    reason: ExtractAbortedReason::Moved.into(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ExtractAborted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ExtractAborted(e)) => {
                assert_eq!(e.reason, ExtractAbortedReason::Moved as i32);
            }
            other => panic!("expected ExtractAborted, got {other:?}"),
        }
    }

    #[test]
    fn extract_failed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ExtractFailed(
                ExtractFailed {
                    player_id: "kiz".to_string(),
                    reason: ExtractFailedReason::SpiritQiDrained.into(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ExtractFailed decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ExtractFailed(e)) => {
                assert_eq!(e.reason, ExtractFailedReason::SpiritQiDrained as i32);
            }
            other => panic!("expected ExtractFailed, got {other:?}"),
        }
    }

    #[test]
    fn tsy_collapse_started_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TsyCollapseStarted(
                TsyCollapseStartedIpc {
                    family_id: "tsy_01".to_string(),
                    at_tick: 10000,
                    remaining_ticks: 200,
                    collapse_tear_entity_ids: vec![1, 2, 3],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("TsyCollapseStartedIpc decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TsyCollapseStarted(t)) => {
                assert_eq!(t.collapse_tear_entity_ids.len(), 3);
            }
            other => panic!("expected TsyCollapseStarted, got {other:?}"),
        }
    }

    #[test]
    fn container_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ContainerState(
                ContainerStateProto {
                    entity_id: 99,
                    kind: ContainerKind::RelicCore.into(),
                    family_id: "tsy_01".to_string(),
                    world_pos_x: 0.0,
                    world_pos_y: 60.0,
                    world_pos_z: 0.0,
                    locked: Some(KeyKind::JadeCoffinSeal.into()),
                    depleted: false,
                    searched_by_player_id: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ContainerState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ContainerState(c)) => {
                assert_eq!(c.kind, ContainerKind::RelicCore as i32);
                assert_eq!(c.locked, Some(KeyKind::JadeCoffinSeal as i32));
            }
            other => panic!("expected ContainerState, got {other:?}"),
        }
    }

    #[test]
    fn search_started_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SearchStarted(
                SearchStarted {
                    player_id: "kiz".to_string(),
                    container_entity_id: 42,
                    required_ticks: 60,
                    at_tick: 1000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SearchStarted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SearchStarted(s)) => {
                assert_eq!(s.required_ticks, 60);
            }
            other => panic!("expected SearchStarted, got {other:?}"),
        }
    }

    #[test]
    fn search_progress_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SearchProgress(
                SearchProgress {
                    player_id: "kiz".to_string(),
                    container_entity_id: 42,
                    elapsed_ticks: 30,
                    required_ticks: 60,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SearchProgress decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SearchProgress(s)) => {
                assert_eq!(s.elapsed_ticks, 30);
            }
            other => panic!("expected SearchProgress, got {other:?}"),
        }
    }

    #[test]
    fn search_completed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SearchCompleted(
                SearchCompleted {
                    player_id: "kiz".to_string(),
                    container_entity_id: 42,
                    family_id: "tsy_01".to_string(),
                    loot_preview: vec![LootPreviewItem {
                        template_id: "bone_coin".to_string(),
                        display_name: "骨币".to_string(),
                        stack_count: 10,
                    }],
                    at_tick: 2000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SearchCompleted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SearchCompleted(s)) => {
                assert_eq!(s.loot_preview.len(), 1);
                assert_eq!(s.loot_preview[0].stack_count, 10);
            }
            other => panic!("expected SearchCompleted, got {other:?}"),
        }
    }

    #[test]
    fn search_aborted_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SearchAborted(
                SearchAborted {
                    player_id: "kiz".to_string(),
                    container_entity_id: 42,
                    reason: SearchAbortReason::Combat.into(),
                    at_tick: 1500,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SearchAborted decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SearchAborted(s)) => {
                assert_eq!(s.reason, SearchAbortReason::Combat as i32);
            }
            other => panic!("expected SearchAborted, got {other:?}"),
        }
    }

    #[test]
    fn skill_lv_up_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SkillLvUp(SkillLvUp {
                char_id: 1,
                skill: SkillId::Herbalism.into(),
                new_lv: 3,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice()).expect("SkillLvUp decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SkillLvUp(s)) => {
                assert_eq!(s.new_lv, 3);
                assert_eq!(s.skill, SkillId::Herbalism as i32);
            }
            other => panic!("expected SkillLvUp, got {other:?}"),
        }
    }

    #[test]
    fn skill_cap_changed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SkillCapChanged(
                SkillCapChanged {
                    char_id: 1,
                    skill: SkillId::Alchemy.into(),
                    new_cap: 5,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SkillCapChanged decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SkillCapChanged(s)) => {
                assert_eq!(s.new_cap, 5);
            }
            other => panic!("expected SkillCapChanged, got {other:?}"),
        }
    }

    #[test]
    fn skill_scroll_used_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SkillScrollUsed(
                SkillScrollUsed {
                    char_id: 1,
                    scroll_id: "scroll_herb_01".to_string(),
                    skill: SkillId::Herbalism.into(),
                    xp_granted: 100,
                    was_duplicate: false,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SkillScrollUsed decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SkillScrollUsed(s)) => {
                assert_eq!(s.xp_granted, 100);
                assert!(!s.was_duplicate);
            }
            other => panic!("expected SkillScrollUsed, got {other:?}"),
        }
    }

    #[test]
    fn skill_snapshot_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SkillSnapshot(
                SkillSnapshotProto {
                    char_id: 1,
                    skills: vec![SkillSnapshotEntry {
                        skill_name: "herbalism".to_string(),
                        entry: Some(SkillEntrySnapshot {
                            lv: 2,
                            xp: 50,
                            xp_to_next: 100,
                            total_xp: 150,
                            cap: 5,
                            recent_gain_xp: 10,
                        }),
                    }],
                    consumed_scrolls: vec!["scroll_01".to_string()],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SkillSnapshot decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SkillSnapshot(s)) => {
                assert_eq!(s.skills.len(), 1);
                assert_eq!(s.consumed_scrolls.len(), 1);
            }
            other => panic!("expected SkillSnapshot, got {other:?}"),
        }
    }

    #[test]
    fn full_power_charging_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::FullPowerCharging(
                FullPowerChargingState {
                    caster_uuid: "uuid-1".to_string(),
                    active: true,
                    qi_committed: 100.0,
                    target_qi: 200.0,
                    started_tick: 5000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("FullPowerChargingState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::FullPowerCharging(f)) => {
                assert!(f.active);
                assert!((f.qi_committed - 100.0).abs() < 1e-9);
            }
            other => panic!("expected FullPowerCharging, got {other:?}"),
        }
    }

    #[test]
    fn full_power_release_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::FullPowerRelease(
                FullPowerRelease {
                    caster_uuid: "uuid-1".to_string(),
                    target_uuid: Some("uuid-2".to_string()),
                    qi_released: 150.0,
                    tick: 5100,
                    hit_pos_x: Some(10.0),
                    hit_pos_y: Some(64.0),
                    hit_pos_z: Some(-5.0),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("FullPowerRelease decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::FullPowerRelease(f)) => {
                assert_eq!(f.target_uuid, Some("uuid-2".to_string()));
                assert_eq!(f.hit_pos_x, Some(10.0));
            }
            other => panic!("expected FullPowerRelease, got {other:?}"),
        }
    }

    #[test]
    fn full_power_exhausted_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::FullPowerExhausted(
                FullPowerExhaustedState {
                    caster_uuid: "uuid-1".to_string(),
                    active: true,
                    started_tick: 5100,
                    recovery_at_tick: 5300,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("FullPowerExhaustedState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::FullPowerExhausted(f)) => {
                assert_eq!(f.recovery_at_tick, 5300);
            }
            other => panic!("expected FullPowerExhausted, got {other:?}"),
        }
    }

    #[test]
    fn healer_npc_ai_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::HealerNpcAiState(
                HealerNpcAiState {
                    healer_id: "npc:doc".to_string(),
                    active_action: "idle".to_string(),
                    queue_len: 3,
                    reputation: 12,
                    retreating: false,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("HealerNpcAiState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::HealerNpcAiState(h)) => {
                assert_eq!(h.queue_len, 3);
                assert!(!h.retreating);
            }
            other => panic!("expected HealerNpcAiState, got {other:?}"),
        }
    }

    #[test]
    fn yidao_hud_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::YidaoHudState(
                YidaoHudState {
                    healer_id: "npc:doc".to_string(),
                    reputation: 10,
                    peace_mastery: 50.0,
                    karma: 3.5,
                    active_skill: Some(YidaoSkillId::MeridianRepair.into()),
                    patient_ids: vec!["p1".to_string()],
                    patient_hp_percent: Some(0.5),
                    patient_contam_total: Some(1.0),
                    severed_meridian_count: 1,
                    contract_count: 2,
                    mass_preview_count: 0,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("YidaoHudState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::YidaoHudState(y)) => {
                assert_eq!(y.active_skill, Some(YidaoSkillId::MeridianRepair as i32));
                assert_eq!(y.patient_ids.len(), 1);
            }
            other => panic!("expected YidaoHudState, got {other:?}"),
        }
    }

    #[test]
    fn movement_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::MovementState(
                MovementStateProto {
                    current_speed_multiplier: 1.2,
                    stamina_cost_active: true,
                    movement_action: MovementAction::Dashing.into(),
                    zone_kind: MovementZoneKind::Normal.into(),
                    dash_cooldown_remaining_ticks: 20,
                    hitbox_height_blocks: 1.8,
                    stamina_current: 80.0,
                    stamina_max: 100.0,
                    low_stamina: false,
                    last_action_tick: Some(999),
                    rejected_action: None,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("MovementState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::MovementState(m)) => {
                assert_eq!(m.movement_action, MovementAction::Dashing as i32);
                assert_eq!(m.last_action_tick, Some(999));
            }
            other => panic!("expected MovementState, got {other:?}"),
        }
    }

    #[test]
    fn spirit_treasure_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SpiritTreasureState(
                SpiritTreasureStateProto {
                    treasures: vec![SpiritTreasureClientState {
                        template_id: "jade_sword".to_string(),
                        display_name: "玉剑".to_string(),
                        instance_id: 42,
                        equipped: true,
                        passive_active: true,
                        affinity: 0.8,
                        sleeping: false,
                        source_sect: Some("qingyun".to_string()),
                        icon_texture: "items/jade_sword".to_string(),
                        passive_effects: vec![SpiritTreasurePassive {
                            kind: "qi_boost".to_string(),
                            value: 0.1,
                            description: "+10% qi".to_string(),
                        }],
                    }],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("SpiritTreasureState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SpiritTreasureState(s)) => {
                assert_eq!(s.treasures.len(), 1);
                assert!(s.treasures[0].equipped);
            }
            other => panic!("expected SpiritTreasureState, got {other:?}"),
        }
    }

    #[test]
    fn spirit_treasure_dialogue_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::SpiritTreasureDialogue(
                SpiritTreasureDialogueProto {
                    dialogue: Some(SpiritTreasureDialogueData {
                        request_id: "r1".to_string(),
                        character_id: "c1".to_string(),
                        treasure_id: "t1".to_string(),
                        text: "你好".to_string(),
                        tone: SpiritTreasureDialogueTone::Curious.into(),
                        affinity_delta: 0.05,
                    }),
                    display_name: "玉剑".to_string(),
                    zone: "spawn".to_string(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("SpiritTreasureDialogue decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::SpiritTreasureDialogue(s)) => {
                assert_eq!(s.display_name, "玉剑");
                let d = s.dialogue.unwrap();
                assert_eq!(d.tone, SpiritTreasureDialogueTone::Curious as i32);
            }
            other => panic!("expected SpiritTreasureDialogue, got {other:?}"),
        }
    }

    // ─── 专用 channel S2C roundtrip ────────────────────────────────

    #[test]
    fn vfx_event_play_anim_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::VfxEvent(VfxEvent {
                payload: Some(vfx_event::Payload::PlayAnim(VfxPlayAnim {
                    target_player: "uuid".to_string(),
                    anim_id: "bong:slash".to_string(),
                    priority: 1000,
                    fade_in_ticks: Some(3),
                })),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("VfxEvent PlayAnim decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::VfxEvent(v)) => match v.payload {
                Some(vfx_event::Payload::PlayAnim(a)) => {
                    assert_eq!(a.priority, 1000);
                    assert_eq!(a.fade_in_ticks, Some(3));
                }
                other => panic!("expected PlayAnim, got {other:?}"),
            },
            other => panic!("expected VfxEvent, got {other:?}"),
        }
    }

    #[test]
    fn vfx_event_spawn_particle_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::VfxEvent(VfxEvent {
                payload: Some(vfx_event::Payload::SpawnParticle(VfxSpawnParticle {
                    event_id: "bong:slash".to_string(),
                    origin_x: 10.0,
                    origin_y: 64.0,
                    origin_z: -5.0,
                    direction_x: Some(1.0),
                    direction_y: Some(0.0),
                    direction_z: Some(0.0),
                    color: Some("#88ccff".to_string()),
                    strength: Some(0.75),
                    count: Some(4),
                    duration_ticks: Some(20),
                })),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("VfxEvent SpawnParticle decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::VfxEvent(v)) => match v.payload {
                Some(vfx_event::Payload::SpawnParticle(p)) => {
                    assert_eq!(p.color, Some("#88ccff".to_string()));
                    assert_eq!(p.count, Some(4));
                }
                other => panic!("expected SpawnParticle, got {other:?}"),
            },
            other => panic!("expected VfxEvent, got {other:?}"),
        }
    }

    #[test]
    fn audio_play_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::AudioPlayEvent(
                AudioPlayEvent {
                    recipe_id: "pill_consume".to_string(),
                    instance_id: 7,
                    pos_x: Some(1),
                    pos_y: Some(64),
                    pos_z: Some(-2),
                    flag: None,
                    volume_mul: 0.8,
                    pitch_shift: 0.0,
                    recipe: Some(SoundRecipeProto {
                        id: "pill_consume".to_string(),
                        layers: vec![SoundLayerProto {
                            sound: "minecraft:entity.generic.drink".to_string(),
                            volume: 0.4,
                            pitch: 1.0,
                            delay_ticks: 0,
                        }],
                        loop_config: None,
                        priority: 40,
                        attenuation: AudioAttenuation::PlayerLocal.into(),
                        category: AudioSoundCategory::Voice.into(),
                        bus: None,
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("AudioPlayEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::AudioPlayEvent(a)) => {
                assert_eq!(a.recipe_id, "pill_consume");
                assert_eq!(a.recipe.unwrap().layers.len(), 1);
            }
            other => panic!("expected AudioPlayEvent, got {other:?}"),
        }
    }

    #[test]
    fn audio_stop_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::AudioStopEvent(
                AudioStopEvent {
                    instance_id: 7,
                    fade_out_ticks: 10,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("AudioStopEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::AudioStopEvent(a)) => {
                assert_eq!(a.instance_id, 7);
                assert_eq!(a.fade_out_ticks, 10);
            }
            other => panic!("expected AudioStopEvent, got {other:?}"),
        }
    }

    #[test]
    fn ambient_zone_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::AmbientZoneEvent(
                AmbientZoneEvent {
                    zone_name: "spawn".to_string(),
                    ambient_recipe_id: "ambient_spawn".to_string(),
                    music_state: "AMBIENT".to_string(),
                    is_night: false,
                    season: "summer".to_string(),
                    tsy_depth: None,
                    fade_ticks: 60,
                    pos_x: None,
                    pos_y: None,
                    pos_z: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipe: Some(SoundRecipeProto {
                        id: "ambient_spawn".to_string(),
                        layers: vec![],
                        loop_config: None,
                        priority: 10,
                        attenuation: AudioAttenuation::ZoneBroadcast.into(),
                        category: AudioSoundCategory::Ambient.into(),
                        bus: Some(AudioBus::Environment.into()),
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("AmbientZoneEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::AmbientZoneEvent(a)) => {
                assert_eq!(a.zone_name, "spawn");
                assert_eq!(a.music_state, "AMBIENT");
            }
            other => panic!("expected AmbientZoneEvent, got {other:?}"),
        }
    }

    #[test]
    fn zone_environment_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::ZoneEnvironmentState(
                ZoneEnvironmentState {
                    dimension: "minecraft:overworld".to_string(),
                    zone_id: "spawn".to_string(),
                    effects: vec![
                        EnvironmentEffectProto {
                            effect: Some(environment_effect_proto::Effect::TornadoColumn(
                                EffectTornadoColumn {
                                    center_x: 1.0,
                                    center_y: 70.0,
                                    center_z: 2.0,
                                    radius: 9.0,
                                    height: 48.0,
                                    particle_density: 0.6,
                                },
                            )),
                        },
                        EnvironmentEffectProto {
                            effect: Some(environment_effect_proto::Effect::FogVeil(
                                EffectFogVeil {
                                    aabb_min_x: 0.0,
                                    aabb_min_y: 60.0,
                                    aabb_min_z: 0.0,
                                    aabb_max_x: 32.0,
                                    aabb_max_y: 95.0,
                                    aabb_max_z: 32.0,
                                    tint_r: 120,
                                    tint_g: 132,
                                    tint_b: 148,
                                    density: 0.32,
                                },
                            )),
                        },
                    ],
                    generation: 7,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("ZoneEnvironmentState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::ZoneEnvironmentState(z)) => {
                assert_eq!(z.effects.len(), 2);
                assert_eq!(z.generation, 7);
            }
            other => panic!("expected ZoneEnvironmentState, got {other:?}"),
        }
    }

    #[test]
    fn mutation_state_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::MutationState(
                MutationStateProto {
                    entity: "player_123".to_string(),
                    stage: MutationStage::Heavy.into(),
                    slots: vec![ActiveMutation {
                        kind: MutationKind::Horns.into(),
                        body_slot: "head".to_string(),
                        level: 2,
                        acquired_tick: 12345,
                    }],
                    meridian_penalty: 0.15,
                    cumulative_toxin: 280.0,
                    social_penalty: -50,
                    server_tick: 99999,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("MutationState decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::MutationState(m)) => {
                assert_eq!(m.stage, MutationStage::Heavy as i32);
                assert_eq!(m.slots.len(), 1);
            }
            other => panic!("expected MutationState, got {other:?}"),
        }
    }

    #[test]
    fn mutation_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::MutationEvent(
                MutationEventProto {
                    entity: "p".to_string(),
                    from_stage: MutationStage::Subtle.into(),
                    to_stage: MutationStage::Visible.into(),
                    cumulative_toxin: 105.0,
                    new_meridian_penalty: 0.08,
                    server_tick: 50000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("MutationEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::MutationEvent(m)) => {
                assert_eq!(m.from_stage, MutationStage::Subtle as i32);
                assert_eq!(m.to_stage, MutationStage::Visible as i32);
            }
            other => panic!("expected MutationEvent, got {other:?}"),
        }
    }

    #[test]
    fn dandao_style_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::DandaoStyle(
                DandaoStyleProto {
                    entity: "p".to_string(),
                    brew_count: 42,
                    pill_intake_count: 100,
                    cumulative_toxin: 123.5,
                    mutation_stage: MutationStage::Visible.into(),
                    mastery_ticks: 99999,
                    server_tick: 10000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("DandaoStyle decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::DandaoStyle(d)) => {
                assert_eq!(d.brew_count, 42);
            }
            other => panic!("expected DandaoStyle, got {other:?}"),
        }
    }

    #[test]
    fn tsy_enter_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TsyEnterEvent(
                TsyEnterEvent {
                    kind: "tsy_enter".to_string(),
                    tick: 12345,
                    player_id: "kiz".to_string(),
                    family_id: "tsy_01".to_string(),
                    return_to: Some(TsyDimensionAnchor {
                        dimension: "minecraft:overworld".to_string(),
                        pos_x: 0.0,
                        pos_y: 65.0,
                        pos_z: 0.0,
                    }),
                    filtered_items: vec![TsyFilteredItem {
                        instance_id: 7,
                        template_id: "bone_coin".to_string(),
                        reason: "spirit_quality_too_high".to_string(),
                    }],
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TsyEnterEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TsyEnterEvent(t)) => {
                assert_eq!(t.filtered_items.len(), 1);
                assert_eq!(t.family_id, "tsy_01");
            }
            other => panic!("expected TsyEnterEvent, got {other:?}"),
        }
    }

    #[test]
    fn tsy_exit_event_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TsyExitEvent(TsyExitEvent {
                kind: "tsy_exit".to_string(),
                tick: 99999,
                player_id: "kiz".to_string(),
                family_id: "tsy_01".to_string(),
                duration_ticks: 12000,
                qi_drained_total: 350.5,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TsyExitEvent decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TsyExitEvent(t)) => {
                assert_eq!(t.duration_ticks, 12000);
            }
            other => panic!("expected TsyExitEvent, got {other:?}"),
        }
    }

    #[test]
    fn tsy_npc_spawned_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TsyNpcSpawned(
                TsyNpcSpawned {
                    kind: "tsy_npc_spawned".to_string(),
                    family_id: "tsy_01".to_string(),
                    archetype: TsyHostileArchetype::GuardianRelicSentinel.into(),
                    count: 3,
                    at_tick: 12000,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ServerDataEnvelope::decode(bytes.as_slice()).expect("TsyNpcSpawned decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TsyNpcSpawned(t)) => {
                assert_eq!(
                    t.archetype,
                    TsyHostileArchetype::GuardianRelicSentinel as i32
                );
                assert_eq!(t.count, 3);
            }
            other => panic!("expected TsyNpcSpawned, got {other:?}"),
        }
    }

    #[test]
    fn tsy_sentinel_phase_changed_roundtrip() {
        let envelope = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::TsySentinelPhaseChanged(
                TsySentinelPhaseChanged {
                    kind: "tsy_sentinel_phase_changed".to_string(),
                    family_id: "tsy_01".to_string(),
                    container_entity_id: 42,
                    phase: 1,
                    max_phase: 3,
                    at_tick: 12345,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("TsySentinelPhaseChanged decode 失败");
        match decoded.payload {
            Some(server_data_envelope::Payload::TsySentinelPhaseChanged(t)) => {
                assert_eq!(t.phase, 1);
                assert_eq!(t.max_phase, 3);
            }
            other => panic!("expected TsySentinelPhaseChanged, got {other:?}"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P2 B5 — C2S roundtrip tests
    // ═══════════════════════════════════════════════════════════════

    #[test]
    fn void_action_suppress_tsy_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::VoidAction(VoidAction {
                request: Some(void_action::Request::SuppressTsy(VoidActionSuppressTsy {
                    zone_id: "tsy_01".to_string(),
                })),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("VoidAction SuppressTsy decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::VoidAction(v)) => match v.request {
                Some(void_action::Request::SuppressTsy(s)) => {
                    assert_eq!(s.zone_id, "tsy_01");
                }
                other => panic!("expected SuppressTsy, got {other:?}"),
            },
            other => panic!("expected VoidAction, got {other:?}"),
        }
    }

    #[test]
    fn movement_action_request_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::MovementAction(
                MovementActionRequest {
                    action: MovementActionRequestKind::Dash.into(),
                    yaw_degrees: Some(90.5),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("MovementAction decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::MovementAction(m)) => {
                assert_eq!(m.action, MovementActionRequestKind::Dash as i32);
                assert!((m.yaw_degrees.unwrap() - 90.5).abs() < 0.01);
            }
            other => panic!("expected MovementAction, got {other:?}"),
        }
    }

    #[test]
    fn forge_request_c2s_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ForgeRequest(
                ForgeRequestC2s {
                    meridian: MeridianId::Lung.into(),
                    axis: ForgeAxis::Rate.into(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ForgeRequest decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ForgeRequest(f)) => {
                assert_eq!(f.meridian, MeridianId::Lung as i32);
                assert_eq!(f.axis, ForgeAxis::Rate as i32);
            }
            other => panic!("expected ForgeRequest, got {other:?}"),
        }
    }

    #[test]
    fn insight_decision_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::InsightDecision(
                InsightDecision {
                    trigger_id: "trigger_01".to_string(),
                    choice_idx: Some(2),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("InsightDecision decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::InsightDecision(i)) => {
                assert_eq!(i.trigger_id, "trigger_01");
                assert_eq!(i.choice_idx, Some(2));
            }
            other => panic!("expected InsightDecision, got {other:?}"),
        }
    }

    #[test]
    fn alchemy_furnace_place_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::AlchemyFurnacePlace(
                AlchemyFurnacePlace {
                    x: 10,
                    y: 64,
                    z: -5,
                    item_instance_id: 99,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("AlchemyFurnacePlace decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::AlchemyFurnacePlace(a)) => {
                assert_eq!(a.item_instance_id, 99);
            }
            other => panic!("expected AlchemyFurnacePlace, got {other:?}"),
        }
    }

    #[test]
    fn coffin_open_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CoffinOpen(CoffinOpen {
                x: 1,
                y: 64,
                z: -1,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("CoffinOpen decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::CoffinOpen(c)) => {
                assert_eq!(c.y, 64);
            }
            other => panic!("expected CoffinOpen, got {other:?}"),
        }
    }

    #[test]
    fn coffin_leave_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CoffinLeave(
                CoffinLeave {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("CoffinLeave decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CoffinLeave(_))
        ));
    }

    #[test]
    fn zhenfa_place_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ZhenfaPlace(ZhenfaPlace {
                x: 10,
                y: 64,
                z: -5,
                kind: ZhenfaKind::Trap.into(),
                carrier: Some(ZhenfaCarrierKind::CommonStone.into()),
                qi_invest_ratio: 0.5,
                trigger: Some("proximity".to_string()),
                item_instance_id: Some(42),
                target_face: Some(TrapTargetFace::Top.into()),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ZhenfaPlace decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ZhenfaPlace(z)) => {
                assert_eq!(z.kind, ZhenfaKind::Trap as i32);
                assert_eq!(z.carrier, Some(ZhenfaCarrierKind::CommonStone as i32));
                assert_eq!(z.target_face, Some(TrapTargetFace::Top as i32));
            }
            other => panic!("expected ZhenfaPlace, got {other:?}"),
        }
    }

    #[test]
    fn zhenfa_trigger_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ZhenfaTrigger(
                ZhenfaTrigger {
                    instance_id: Some(42),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ZhenfaTrigger decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ZhenfaTrigger(z)) => {
                assert_eq!(z.instance_id, Some(42));
            }
            other => panic!("expected ZhenfaTrigger, got {other:?}"),
        }
    }

    #[test]
    fn zhenfa_disarm_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ZhenfaDisarm(
                ZhenfaDisarm {
                    x: 1,
                    y: 64,
                    z: -1,
                    mode: ZhenfaDisarmMode::ForceBreak.into(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ZhenfaDisarm decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ZhenfaDisarm(z)) => {
                assert_eq!(z.mode, ZhenfaDisarmMode::ForceBreak as i32);
            }
            other => panic!("expected ZhenfaDisarm, got {other:?}"),
        }
    }

    #[test]
    fn learn_skill_scroll_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::LearnSkillScroll(
                LearnSkillScroll { instance_id: 42 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("LearnSkillScroll decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::LearnSkillScroll(l)) => {
                assert_eq!(l.instance_id, 42);
            }
            other => panic!("expected LearnSkillScroll, got {other:?}"),
        }
    }

    #[test]
    fn inventory_move_intent_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::InventoryMoveIntent(
                InventoryMoveIntent {
                    instance_id: 42,
                    from: Some(InventoryLocation {
                        location: Some(inventory_location::Location::Hotbar(
                            InventoryLocationHotbar { index: 0 },
                        )),
                    }),
                    to: Some(InventoryLocation {
                        location: Some(inventory_location::Location::Equip(
                            InventoryLocationEquip {
                                slot: EquipSlot::MainHand.into(),
                            },
                        )),
                    }),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("InventoryMoveIntent decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::InventoryMoveIntent(m)) => {
                assert_eq!(m.instance_id, 42);
            }
            other => panic!("expected InventoryMoveIntent, got {other:?}"),
        }
    }

    #[test]
    fn equip_false_skin_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::EquipFalseSkin(
                EquipFalseSkin {
                    slot: EquipSlot::FalseSkin.into(),
                    item_instance_id: 42,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("EquipFalseSkin decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::EquipFalseSkin(e)) => {
                assert_eq!(e.slot, EquipSlot::FalseSkin as i32);
            }
            other => panic!("expected EquipFalseSkin, got {other:?}"),
        }
    }

    #[test]
    fn forge_false_skin_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ForgeFalseSkin(
                ForgeFalseSkinC2s {
                    kind: FalseSkinKind::SpiderSilk.into(),
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ForgeFalseSkin decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ForgeFalseSkin(f)) => {
                assert_eq!(f.kind, FalseSkinKind::SpiderSilk as i32);
            }
            other => panic!("expected ForgeFalseSkin, got {other:?}"),
        }
    }

    #[test]
    fn apply_pill_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ApplyPill(ApplyPill {
                instance_id: 42,
                target_kind: ApplyPillTargetKind::Meridian.into(),
                meridian_id: Some(MeridianId::Heart.into()),
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ApplyPill decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ApplyPill(a)) => {
                assert_eq!(a.target_kind, ApplyPillTargetKind::Meridian as i32);
                assert_eq!(a.meridian_id, Some(MeridianId::Heart as i32));
            }
            other => panic!("expected ApplyPill, got {other:?}"),
        }
    }

    #[test]
    fn self_antidote_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::SelfAntidote(
                SelfAntidote { instance_id: 42 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("SelfAntidote decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::SelfAntidote(s)) => {
                assert_eq!(s.instance_id, 42);
            }
            other => panic!("expected SelfAntidote, got {other:?}"),
        }
    }

    #[test]
    fn start_extract_request_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::StartExtractRequest(
                StartExtractRequest {
                    portal_entity_id: 42,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("StartExtractRequest decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::StartExtractRequest(s)) => {
                assert_eq!(s.portal_entity_id, 42);
            }
            other => panic!("expected StartExtractRequest, got {other:?}"),
        }
    }

    #[test]
    fn cancel_extract_request_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CancelExtractRequest(
                CancelExtractRequest {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("CancelExtractRequest decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CancelExtractRequest(_))
        ));
    }

    #[test]
    fn start_search_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::StartSearch(StartSearch {
                container_entity_id: 42,
            })),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("StartSearch decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::StartSearch(s)) => {
                assert_eq!(s.container_entity_id, 42);
            }
            other => panic!("expected StartSearch, got {other:?}"),
        }
    }

    #[test]
    fn cancel_search_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::CancelSearch(
                CancelSearch {},
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("CancelSearch decode 失败");
        assert!(matches!(
            decoded.payload,
            Some(client_request_envelope::Payload::CancelSearch(_))
        ));
    }

    #[test]
    fn forge_station_place_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::ForgeStationPlace(
                ForgeStationPlace {
                    x: 10,
                    y: 64,
                    z: -5,
                    item_instance_id: 99,
                    station_tier: 2,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("ForgeStationPlace decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::ForgeStationPlace(f)) => {
                assert_eq!(f.station_tier, 2);
                assert_eq!(f.item_instance_id, 99);
            }
            other => panic!("expected ForgeStationPlace, got {other:?}"),
        }
    }

    #[test]
    fn repair_weapon_intent_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::RepairWeaponIntent(
                RepairWeaponIntent {
                    instance_id: 42,
                    station_pos_x: 10,
                    station_pos_y: 64,
                    station_pos_z: -5,
                },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("RepairWeaponIntent decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::RepairWeaponIntent(r)) => {
                assert_eq!(r.instance_id, 42);
                assert_eq!(r.station_pos_y, 64);
            }
            other => panic!("expected RepairWeaponIntent, got {other:?}"),
        }
    }

    #[test]
    fn pickup_dropped_item_roundtrip() {
        let envelope = ClientRequestEnvelope {
            payload: Some(client_request_envelope::Payload::PickupDroppedItem(
                PickupDroppedItem { instance_id: 42 },
            )),
        };
        let bytes = envelope.encode_to_vec();
        let decoded =
            ClientRequestEnvelope::decode(bytes.as_slice()).expect("PickupDroppedItem decode 失败");
        match decoded.payload {
            Some(client_request_envelope::Payload::PickupDroppedItem(p)) => {
                assert_eq!(p.instance_id, 42);
            }
            other => panic!("expected PickupDroppedItem, got {other:?}"),
        }
    }

    // ─── B5 batch all S2C variants distinguishable ──────────────────

    #[test]
    fn b5_s2c_all_envelope_variants_roundtrip() {
        let payloads: Vec<(server_data_envelope::Payload, &str)> = vec![
            (
                server_data_envelope::Payload::EventAlert(EventAlert {
                    event: EventKind::BeastTide.into(),
                    message: "m".to_string(),
                    zone: None,
                    duration_ticks: None,
                }),
                "EventAlert",
            ),
            (
                server_data_envelope::Payload::CoffinState(CoffinState {
                    in_coffin: false,
                    lifespan_rate_multiplier: 1.0,
                }),
                "CoffinState",
            ),
            (
                server_data_envelope::Payload::UiOpen(UiOpen {
                    ui: None,
                    xml: "x".to_string(),
                }),
                "UiOpen",
            ),
            (
                server_data_envelope::Payload::InventoryEvent(InventoryEvent {
                    event: Some(inventory_event::Event::StackChanged(
                        InventoryEventStackChanged {
                            revision: 1,
                            instance_id: 1,
                            stack_count: 1,
                        },
                    )),
                }),
                "InventoryEvent",
            ),
            (
                server_data_envelope::Payload::DroppedLootSync(DroppedLootSync { drops: vec![] }),
                "DroppedLootSync",
            ),
            (
                server_data_envelope::Payload::RiftPortalState(RiftPortalState {
                    entity_id: 1,
                    kind: 1,
                    direction: 1,
                    family_id: "f".to_string(),
                    world_pos_x: 0.0,
                    world_pos_y: 0.0,
                    world_pos_z: 0.0,
                    trigger_radius: 1.0,
                    current_extract_ticks: 0,
                    activation_window_end: None,
                }),
                "RiftPortalState",
            ),
            (
                server_data_envelope::Payload::RiftPortalRemoved(RiftPortalRemoved {
                    entity_id: 1,
                }),
                "RiftPortalRemoved",
            ),
            (
                server_data_envelope::Payload::ExtractStarted(ExtractStarted {
                    player_id: "p".to_string(),
                    portal_entity_id: 1,
                    portal_kind: 1,
                    required_ticks: 1,
                    at_tick: 1,
                }),
                "ExtractStarted",
            ),
            (
                server_data_envelope::Payload::ExtractProgress(ExtractProgress {
                    player_id: "p".to_string(),
                    portal_entity_id: 1,
                    elapsed_ticks: 1,
                    required_ticks: 1,
                }),
                "ExtractProgress",
            ),
            (
                server_data_envelope::Payload::ExtractCompleted(ExtractCompleted {
                    player_id: "p".to_string(),
                    portal_kind: 1,
                    family_id: "f".to_string(),
                    exit_pos_x: 0.0,
                    exit_pos_y: 0.0,
                    exit_pos_z: 0.0,
                    at_tick: 1,
                }),
                "ExtractCompleted",
            ),
            (
                server_data_envelope::Payload::ExtractAborted(ExtractAborted {
                    player_id: "p".to_string(),
                    reason: 1,
                }),
                "ExtractAborted",
            ),
            (
                server_data_envelope::Payload::ExtractFailed(ExtractFailed {
                    player_id: "p".to_string(),
                    reason: 1,
                }),
                "ExtractFailed",
            ),
            (
                server_data_envelope::Payload::TsyCollapseStarted(TsyCollapseStartedIpc {
                    family_id: "f".to_string(),
                    at_tick: 1,
                    remaining_ticks: 1,
                    collapse_tear_entity_ids: vec![],
                }),
                "TsyCollapseStarted",
            ),
            (
                server_data_envelope::Payload::ContainerState(ContainerStateProto {
                    entity_id: 1,
                    kind: 1,
                    family_id: "f".to_string(),
                    world_pos_x: 0.0,
                    world_pos_y: 0.0,
                    world_pos_z: 0.0,
                    locked: None,
                    depleted: false,
                    searched_by_player_id: None,
                }),
                "ContainerState",
            ),
            (
                server_data_envelope::Payload::SearchStarted(SearchStarted {
                    player_id: "p".to_string(),
                    container_entity_id: 1,
                    required_ticks: 1,
                    at_tick: 1,
                }),
                "SearchStarted",
            ),
            (
                server_data_envelope::Payload::SearchProgress(SearchProgress {
                    player_id: "p".to_string(),
                    container_entity_id: 1,
                    elapsed_ticks: 1,
                    required_ticks: 1,
                }),
                "SearchProgress",
            ),
            (
                server_data_envelope::Payload::SearchCompleted(SearchCompleted {
                    player_id: "p".to_string(),
                    container_entity_id: 1,
                    family_id: "f".to_string(),
                    loot_preview: vec![],
                    at_tick: 1,
                }),
                "SearchCompleted",
            ),
            (
                server_data_envelope::Payload::SearchAborted(SearchAborted {
                    player_id: "p".to_string(),
                    container_entity_id: 1,
                    reason: 1,
                    at_tick: 1,
                }),
                "SearchAborted",
            ),
            (
                server_data_envelope::Payload::SkillLvUp(SkillLvUp {
                    char_id: 1,
                    skill: 1,
                    new_lv: 1,
                }),
                "SkillLvUp",
            ),
            (
                server_data_envelope::Payload::SkillCapChanged(SkillCapChanged {
                    char_id: 1,
                    skill: 1,
                    new_cap: 1,
                }),
                "SkillCapChanged",
            ),
            (
                server_data_envelope::Payload::SkillScrollUsed(SkillScrollUsed {
                    char_id: 1,
                    scroll_id: "s".to_string(),
                    skill: 1,
                    xp_granted: 1,
                    was_duplicate: false,
                }),
                "SkillScrollUsed",
            ),
            (
                server_data_envelope::Payload::SkillSnapshot(SkillSnapshotProto {
                    char_id: 1,
                    skills: vec![],
                    consumed_scrolls: vec![],
                }),
                "SkillSnapshot",
            ),
            (
                server_data_envelope::Payload::FullPowerCharging(FullPowerChargingState {
                    caster_uuid: "u".to_string(),
                    active: true,
                    qi_committed: 0.0,
                    target_qi: 0.0,
                    started_tick: 0,
                }),
                "FullPowerCharging",
            ),
            (
                server_data_envelope::Payload::FullPowerRelease(FullPowerRelease {
                    caster_uuid: "u".to_string(),
                    target_uuid: None,
                    qi_released: 0.0,
                    tick: 0,
                    hit_pos_x: None,
                    hit_pos_y: None,
                    hit_pos_z: None,
                }),
                "FullPowerRelease",
            ),
            (
                server_data_envelope::Payload::FullPowerExhausted(FullPowerExhaustedState {
                    caster_uuid: "u".to_string(),
                    active: true,
                    started_tick: 0,
                    recovery_at_tick: 0,
                }),
                "FullPowerExhausted",
            ),
            (
                server_data_envelope::Payload::HealerNpcAiState(HealerNpcAiState {
                    healer_id: "h".to_string(),
                    active_action: "a".to_string(),
                    queue_len: 0,
                    reputation: 0,
                    retreating: false,
                }),
                "HealerNpcAiState",
            ),
            (
                server_data_envelope::Payload::YidaoHudState(YidaoHudState {
                    healer_id: "h".to_string(),
                    reputation: 0,
                    peace_mastery: 0.0,
                    karma: 0.0,
                    active_skill: None,
                    patient_ids: vec![],
                    patient_hp_percent: None,
                    patient_contam_total: None,
                    severed_meridian_count: 0,
                    contract_count: 0,
                    mass_preview_count: 0,
                }),
                "YidaoHudState",
            ),
            (
                server_data_envelope::Payload::MovementState(MovementStateProto {
                    current_speed_multiplier: 1.0,
                    stamina_cost_active: false,
                    movement_action: 1,
                    zone_kind: 1,
                    dash_cooldown_remaining_ticks: 0,
                    hitbox_height_blocks: 1.8,
                    stamina_current: 100.0,
                    stamina_max: 100.0,
                    low_stamina: false,
                    last_action_tick: None,
                    rejected_action: None,
                }),
                "MovementState",
            ),
            (
                server_data_envelope::Payload::SpiritTreasureState(SpiritTreasureStateProto {
                    treasures: vec![],
                }),
                "SpiritTreasureState",
            ),
            (
                server_data_envelope::Payload::SpiritTreasureDialogue(
                    SpiritTreasureDialogueProto {
                        dialogue: None,
                        display_name: "n".to_string(),
                        zone: "z".to_string(),
                    },
                ),
                "SpiritTreasureDialogue",
            ),
            (
                server_data_envelope::Payload::VfxEvent(VfxEvent {
                    payload: Some(vfx_event::Payload::StopAnim(VfxStopAnim {
                        target_player: "u".to_string(),
                        anim_id: "a".to_string(),
                        fade_out_ticks: None,
                    })),
                }),
                "VfxEvent",
            ),
            (
                server_data_envelope::Payload::AudioPlayEvent(AudioPlayEvent {
                    recipe_id: "r".to_string(),
                    instance_id: 1,
                    pos_x: None,
                    pos_y: None,
                    pos_z: None,
                    flag: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipe: None,
                }),
                "AudioPlayEvent",
            ),
            (
                server_data_envelope::Payload::AudioStopEvent(AudioStopEvent {
                    instance_id: 1,
                    fade_out_ticks: 0,
                }),
                "AudioStopEvent",
            ),
            (
                server_data_envelope::Payload::AmbientZoneEvent(AmbientZoneEvent {
                    zone_name: "z".to_string(),
                    ambient_recipe_id: "r".to_string(),
                    music_state: "AMBIENT".to_string(),
                    is_night: false,
                    season: "summer".to_string(),
                    tsy_depth: None,
                    fade_ticks: 0,
                    pos_x: None,
                    pos_y: None,
                    pos_z: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipe: None,
                }),
                "AmbientZoneEvent",
            ),
            (
                server_data_envelope::Payload::ZoneEnvironmentState(ZoneEnvironmentState {
                    dimension: "d".to_string(),
                    zone_id: "z".to_string(),
                    effects: vec![],
                    generation: 0,
                }),
                "ZoneEnvironmentState",
            ),
            (
                server_data_envelope::Payload::MutationState(MutationStateProto {
                    entity: "e".to_string(),
                    stage: 1,
                    slots: vec![],
                    meridian_penalty: 0.0,
                    cumulative_toxin: 0.0,
                    social_penalty: 0,
                    server_tick: 0,
                }),
                "MutationState",
            ),
            (
                server_data_envelope::Payload::MutationEvent(MutationEventProto {
                    entity: "e".to_string(),
                    from_stage: 1,
                    to_stage: 2,
                    cumulative_toxin: 0.0,
                    new_meridian_penalty: 0.0,
                    server_tick: 0,
                }),
                "MutationEvent",
            ),
            (
                server_data_envelope::Payload::DandaoStyle(DandaoStyleProto {
                    entity: "e".to_string(),
                    brew_count: 0,
                    pill_intake_count: 0,
                    cumulative_toxin: 0.0,
                    mutation_stage: 1,
                    mastery_ticks: 0,
                    server_tick: 0,
                }),
                "DandaoStyle",
            ),
            (
                server_data_envelope::Payload::TsyEnterEvent(TsyEnterEvent {
                    kind: "k".to_string(),
                    tick: 0,
                    player_id: "p".to_string(),
                    family_id: "f".to_string(),
                    return_to: None,
                    filtered_items: vec![],
                }),
                "TsyEnterEvent",
            ),
            (
                server_data_envelope::Payload::TsyExitEvent(TsyExitEvent {
                    kind: "k".to_string(),
                    tick: 0,
                    player_id: "p".to_string(),
                    family_id: "f".to_string(),
                    duration_ticks: 0,
                    qi_drained_total: 0.0,
                }),
                "TsyExitEvent",
            ),
            (
                server_data_envelope::Payload::TsyNpcSpawned(TsyNpcSpawned {
                    kind: "k".to_string(),
                    family_id: "f".to_string(),
                    archetype: 1,
                    count: 1,
                    at_tick: 0,
                }),
                "TsyNpcSpawned",
            ),
            (
                server_data_envelope::Payload::TsySentinelPhaseChanged(TsySentinelPhaseChanged {
                    kind: "k".to_string(),
                    family_id: "f".to_string(),
                    container_entity_id: 1,
                    phase: 1,
                    max_phase: 3,
                    at_tick: 0,
                }),
                "TsySentinelPhaseChanged",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ServerDataEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ServerDataEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B5 S2C envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B5 S2C envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ─── B5 batch all C2S variants distinguishable ──────────────────

    #[test]
    fn b5_c2s_all_envelope_variants_roundtrip() {
        let payloads: Vec<(client_request_envelope::Payload, &str)> = vec![
            (
                client_request_envelope::Payload::VoidAction(VoidAction {
                    request: Some(void_action::Request::ExplodeZone(VoidActionExplodeZone {
                        zone_id: "z".to_string(),
                    })),
                }),
                "VoidAction",
            ),
            (
                client_request_envelope::Payload::MovementAction(MovementActionRequest {
                    action: 1,
                    yaw_degrees: None,
                }),
                "MovementAction",
            ),
            (
                client_request_envelope::Payload::ForgeRequest(ForgeRequestC2s {
                    meridian: 1,
                    axis: 1,
                }),
                "ForgeRequest",
            ),
            (
                client_request_envelope::Payload::InsightDecision(InsightDecision {
                    trigger_id: "t".to_string(),
                    choice_idx: None,
                }),
                "InsightDecision",
            ),
            (
                client_request_envelope::Payload::AlchemyFurnacePlace(AlchemyFurnacePlace {
                    x: 0,
                    y: 0,
                    z: 0,
                    item_instance_id: 1,
                }),
                "AlchemyFurnacePlace",
            ),
            (
                client_request_envelope::Payload::CoffinOpen(CoffinOpen { x: 0, y: 0, z: 0 }),
                "CoffinOpen",
            ),
            (
                client_request_envelope::Payload::CoffinPlace(CoffinPlace {
                    x: 0,
                    y: 0,
                    z: 0,
                    item_instance_id: 1,
                }),
                "CoffinPlace",
            ),
            (
                client_request_envelope::Payload::CoffinEnter(CoffinEnter { x: 0, y: 0, z: 0 }),
                "CoffinEnter",
            ),
            (
                client_request_envelope::Payload::CoffinLeave(CoffinLeave {}),
                "CoffinLeave",
            ),
            (
                client_request_envelope::Payload::ZhenfaPlace(ZhenfaPlace {
                    x: 0,
                    y: 0,
                    z: 0,
                    kind: 1,
                    carrier: None,
                    qi_invest_ratio: 0.5,
                    trigger: None,
                    item_instance_id: None,
                    target_face: None,
                }),
                "ZhenfaPlace",
            ),
            (
                client_request_envelope::Payload::ZhenfaTrigger(ZhenfaTrigger {
                    instance_id: None,
                }),
                "ZhenfaTrigger",
            ),
            (
                client_request_envelope::Payload::ZhenfaDisarm(ZhenfaDisarm {
                    x: 0,
                    y: 0,
                    z: 0,
                    mode: 1,
                }),
                "ZhenfaDisarm",
            ),
            (
                client_request_envelope::Payload::LearnSkillScroll(LearnSkillScroll {
                    instance_id: 1,
                }),
                "LearnSkillScroll",
            ),
            (
                client_request_envelope::Payload::TechniqueScrollUse(TechniqueScrollUse {
                    instance_id: 1,
                }),
                "TechniqueScrollUse",
            ),
            (
                client_request_envelope::Payload::InventoryMoveIntent(InventoryMoveIntent {
                    instance_id: 1,
                    from: None,
                    to: None,
                }),
                "InventoryMoveIntent",
            ),
            (
                client_request_envelope::Payload::EquipFalseSkin(EquipFalseSkin {
                    slot: 1,
                    item_instance_id: 1,
                }),
                "EquipFalseSkin",
            ),
            (
                client_request_envelope::Payload::ForgeFalseSkin(ForgeFalseSkinC2s { kind: 1 }),
                "ForgeFalseSkin",
            ),
            (
                client_request_envelope::Payload::InventoryDiscardItem(InventoryDiscardItem {
                    instance_id: 1,
                    from: None,
                }),
                "InventoryDiscardItem",
            ),
            (
                client_request_envelope::Payload::DropWeaponIntent(DropWeaponIntent {
                    instance_id: 1,
                    from: None,
                }),
                "DropWeaponIntent",
            ),
            (
                client_request_envelope::Payload::RepairWeaponIntent(RepairWeaponIntent {
                    instance_id: 1,
                    station_pos_x: 0,
                    station_pos_y: 0,
                    station_pos_z: 0,
                }),
                "RepairWeaponIntent",
            ),
            (
                client_request_envelope::Payload::PickupDroppedItem(PickupDroppedItem {
                    instance_id: 1,
                }),
                "PickupDroppedItem",
            ),
            (
                client_request_envelope::Payload::ApplyPill(ApplyPill {
                    instance_id: 1,
                    target_kind: 1,
                    meridian_id: None,
                }),
                "ApplyPill",
            ),
            (
                client_request_envelope::Payload::SelfAntidote(SelfAntidote { instance_id: 1 }),
                "SelfAntidote",
            ),
            (
                client_request_envelope::Payload::StartExtractRequest(StartExtractRequest {
                    portal_entity_id: 1,
                }),
                "StartExtractRequest",
            ),
            (
                client_request_envelope::Payload::CancelExtractRequest(CancelExtractRequest {}),
                "CancelExtractRequest",
            ),
            (
                client_request_envelope::Payload::StartSearch(StartSearch {
                    container_entity_id: 1,
                }),
                "StartSearch",
            ),
            (
                client_request_envelope::Payload::CancelSearch(CancelSearch {}),
                "CancelSearch",
            ),
            (
                client_request_envelope::Payload::ForgeStationPlace(ForgeStationPlace {
                    x: 0,
                    y: 0,
                    z: 0,
                    item_instance_id: 1,
                    station_tier: 1,
                }),
                "ForgeStationPlace",
            ),
        ];

        for (payload, name) in payloads {
            let envelope = ClientRequestEnvelope {
                payload: Some(payload),
            };
            let bytes = envelope.encode_to_vec();
            let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("{name} B5 C2S envelope decode 失败: {e}"));
            assert!(
                decoded.payload.is_some(),
                "{name} B5 C2S envelope roundtrip 后 payload 应为 Some"
            );
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // P4.2 — Schema evolution 测试
    // ═══════════════════════════════════════════════════════════════

    /// 验证 prost decode 忽略末尾多余字节（模拟新 schema 增加字段）。
    /// proto wire format 天然跳过未知 field tag，所以追加有效 field bytes
    /// 到一个已编码消息末尾，decode 旧 schema 时应静默忽略。
    #[test]
    fn schema_evolution_extra_trailing_fields_are_silently_ignored() {
        let original = Welcome {
            message: "hello".to_string(),
        };
        let mut bytes = original.encode_to_vec();

        // 追加一个合法但 Welcome 未定义的 field (tag=99, varint 42)。
        // tag = (99 << 3) | 0 = 792 → varint 编码 [0xF8, 0x06]
        // value = 42 → varint [0x2A]
        bytes.extend_from_slice(&[0xF8, 0x06, 0x2A]);

        let decoded =
            Welcome::decode(bytes.as_slice()).expect("Welcome decode 应忽略未知 field，不应失败");
        assert_eq!(
            decoded.message, "hello",
            "decode 后已知 field 'message' 应保持原值 'hello'，实际 '{}'",
            decoded.message
        );
    }

    /// 验证追加多个未知 field（length-delimited + varint）后仍能正确 decode。
    #[test]
    fn schema_evolution_multiple_unknown_fields_ignored() {
        let state = PlayerState {
            player: Some("tester".to_string()),
            realm: Realm::Condense as i32,
            spirit_qi: 100.0,
            karma: 0.5,
            composite_power: 50.0,
            zone: "spawn".to_string(),
            local_neg_pressure: None,
            breakdown: None,
            season_state: None,
            social: None,
        };
        let mut bytes = state.encode_to_vec();

        // 追加未知 field 100 (varint): tag = (100 << 3) | 0 = 800 → [0xA0, 0x06], value=99 → [0x63]
        bytes.extend_from_slice(&[0xA0, 0x06, 0x63]);
        // 追加未知 field 101 (length-delimited string): tag = (101 << 3) | 2 = 810 → [0xAA, 0x06],
        // len=5 → [0x05], data = "extra"
        bytes.extend_from_slice(&[0xAA, 0x06, 0x05, b'e', b'x', b't', b'r', b'a']);

        let decoded = PlayerState::decode(bytes.as_slice())
            .expect("PlayerState decode 应忽略 2 个未知 field，不应失败");
        assert_eq!(
            decoded.player.as_deref(),
            Some("tester"),
            "追加未知 field 后 player 应保持 'tester'，实际 {:?}",
            decoded.player
        );
        assert_eq!(
            decoded.realm,
            Realm::Condense as i32,
            "追加未知 field 后 realm 应保持 Condense ({}), 实际 {}",
            Realm::Condense as i32,
            decoded.realm
        );
        assert_eq!(
            decoded.zone, "spawn",
            "追加未知 field 后 zone 应保持 'spawn'，实际 '{}'",
            decoded.zone
        );
    }

    /// 验证 minimal message（仅 proto3 默认值）decode 后各 field 拿到正确默认值。
    /// 空 bytes → 全 default（空串、0、false、None for optional）。
    #[test]
    fn schema_evolution_missing_fields_get_defaults() {
        // 空 bytes 在 proto3 中完全合法——所有 field 都是默认值。
        let decoded = CultivationDetail::decode(&[] as &[u8])
            .expect("空 bytes decode 为 CultivationDetail 应成功");
        assert_eq!(
            decoded.realm,
            Realm::Unspecified as i32,
            "缺失 realm 应默认为 UNSPECIFIED (0)，实际 {}",
            decoded.realm
        );
        assert!(
            decoded.meridians.is_empty(),
            "缺失 meridians 应为空 vec，实际 len={}",
            decoded.meridians.len()
        );
        assert_eq!(
            decoded.target_meridian, None,
            "缺失 optional target_meridian 应为 None，实际 {:?}",
            decoded.target_meridian
        );
        assert_eq!(
            decoded.contamination_total, 0.0,
            "缺失 contamination_total 应为 0.0，实际 {}",
            decoded.contamination_total
        );
        assert!(
            decoded.lifespan.is_none(),
            "缺失 optional lifespan 应为 None"
        );
        assert!(
            decoded.recent_skill_milestones_summary.is_empty(),
            "缺失 string 应为空串，实际 '{}'",
            decoded.recent_skill_milestones_summary
        );
        assert!(
            decoded.skill_milestones.is_empty(),
            "缺失 repeated 应为空 vec"
        );
        assert_eq!(
            decoded.qi_color_main,
            ColorKind::Unspecified as i32,
            "缺失 enum 应为 UNSPECIFIED (0)"
        );
        assert_eq!(
            decoded.qi_color_secondary, None,
            "缺失 optional enum 应为 None"
        );
        assert!(!decoded.qi_color_chaotic, "缺失 bool 应为 false");
        assert!(!decoded.qi_color_hunyuan, "缺失 bool 应为 false");
        assert!(
            decoded.practice_weights.is_empty(),
            "缺失 repeated 应为空 vec"
        );
    }

    /// 验证 enum field 设为未定义的 i32 值时 prost 保留原始值不 panic。
    #[test]
    fn schema_evolution_enum_unrecognized_i32_preserved() {
        // CultivationDetail.realm 是 i32（proto Realm enum），设为 999。
        let detail = CultivationDetail {
            realm: 999, // 不存在的 Realm 值
            meridians: vec![],
            target_meridian: None,
            contamination_total: 0.0,
            lifespan: None,
            recent_skill_milestones_summary: String::new(),
            skill_milestones: vec![],
            qi_color_main: 888, // 不存在的 ColorKind 值
            qi_color_secondary: Some(777),
            qi_color_chaotic: false,
            qi_color_hunyuan: false,
            practice_weights: vec![],
        };
        let bytes = detail.encode_to_vec();
        let decoded = CultivationDetail::decode(bytes.as_slice())
            .expect("含未知 enum 值的 CultivationDetail decode 不应 panic");
        assert_eq!(
            decoded.realm, 999,
            "未知 Realm 值 999 应原样保留，实际 {}",
            decoded.realm
        );
        assert_eq!(
            decoded.qi_color_main, 888,
            "未知 ColorKind 值 888 应原样保留，实际 {}",
            decoded.qi_color_main
        );
        assert_eq!(
            decoded.qi_color_secondary,
            Some(777),
            "未知 optional ColorKind 值 777 应原样保留，实际 {:?}",
            decoded.qi_color_secondary
        );

        // 验证 Realm::try_from(999) 返回 Err（prost 的 enum try_from）。
        let try_realm = Realm::try_from(999);
        assert!(
            try_realm.is_err(),
            "Realm::try_from(999) 应返回 Err，因为 999 不是有效变体"
        );
    }

    /// 验证 oneof envelope 在新增 variant 时旧 decoder 仍能 decode（payload = None）。
    /// 构造一个 ServerDataEnvelope 的 bytes，oneof field tag 超出已知范围，
    /// prost 应将 payload 解码为 None（因为没有匹配的 variant）。
    #[test]
    fn schema_evolution_unknown_oneof_variant_decodes_as_none() {
        // ServerDataEnvelope 的 oneof 最高 field 号是 118 (tsy_sentinel_phase_changed)。
        // 模拟 field 200 (length-delimited): tag = (200 << 3) | 2 = 1602 → varint [0xC2, 0x0C]
        // len = 3, data = [0x0a, 0x01, 0x42]（内嵌一条 string field tag=1 value="B"）
        let fake_bytes: &[u8] = &[0xC2, 0x0C, 0x03, 0x0a, 0x01, 0x42];
        let decoded = ServerDataEnvelope::decode(fake_bytes)
            .expect("含未知 oneof variant 的 ServerDataEnvelope decode 不应失败");
        assert!(
            decoded.payload.is_none(),
            "未知 oneof field 200 应导致 payload = None（旧 decoder 不识别），实际 {:?}",
            decoded.payload
        );
    }

    // ═══════════════════════════════════════════════════════════════
    // P4.3 — Proto vs JSON 性能 benchmark
    // ═══════════════════════════════════════════════════════════════

    /// 对比 proto (prost) 与 JSON (serde_json) 的编解码性能和体积。
    ///
    /// 运行：`cargo test proto_vs_json_benchmark -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn proto_vs_json_benchmark() {
        use crate::schema::server_data::ServerDataV1;
        use std::time::Instant;

        const ITERS: u32 = 1000;

        // ── 构造 proto 测试载荷 ──────────────────────────────────

        let proto_welcome = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::Welcome(Welcome {
                message: "欢迎来到末法残土！灵气衰退的时代，修仙之路危机四伏。".to_string(),
            })),
        };

        let proto_player_state = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::PlayerState(PlayerState {
                player: Some("散修·张三".to_string()),
                realm: Realm::Condense as i32,
                spirit_qi: 1234.567,
                karma: 0.42,
                composite_power: 8765.4321,
                zone: "qingyun_peaks".to_string(),
                local_neg_pressure: Some(0.15),
                breakdown: Some(PlayerPowerBreakdown {
                    combat: 3000.0,
                    wealth: 1500.0,
                    social: 800.0,
                    karma: 420.0,
                    territory: 200.0,
                }),
                season_state: Some(SeasonState {
                    season: Season::Winter as i32,
                    tick_into_phase: 5000,
                    phase_total_ticks: 24000,
                    year_index: 3,
                }),
                social: None,
            })),
        };

        let proto_cultivation = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CultivationDetail(
                CultivationDetail {
                    realm: Realm::Solidify as i32,
                    meridians: (1..=20)
                        .map(|i| MeridianState {
                            id: i,
                            opened: i <= 12,
                            flow_rate: if i <= 12 { 0.8 } else { 0.0 },
                            flow_capacity: if i <= 12 { 1.0 } else { 0.5 },
                            integrity: 0.95 - (i as f64 * 0.01),
                            open_progress: if i <= 12 { 1.0 } else { 0.3 + i as f64 * 0.02 },
                            cracks_count: if i > 15 { 1 } else { 0 },
                        })
                        .collect(),
                    target_meridian: Some(MeridianId::Ren as i32),
                    contamination_total: 12.5,
                    lifespan: Some(LifespanPreview {
                        years_lived: 45.3,
                        cap_by_realm: 200,
                        remaining_years: 154.7,
                        death_penalty_years: 5,
                        tick_rate_multiplier: 1.0,
                        is_wind_candle: false,
                    }),
                    recent_skill_milestones_summary: "采药 Lv.5, 战斗 Lv.3".to_string(),
                    skill_milestones: vec![
                        SkillMilestoneSnapshot {
                            skill: "herbalism".to_string(),
                            new_lv: 5,
                            achieved_at: 120000,
                            narration: "你的采药技艺更加纯熟".to_string(),
                            total_xp_at: 15000,
                        },
                        SkillMilestoneSnapshot {
                            skill: "combat".to_string(),
                            new_lv: 3,
                            achieved_at: 80000,
                            narration: "你的战斗本能有所觉醒".to_string(),
                            total_xp_at: 5000,
                        },
                    ],
                    qi_color_main: ColorKind::Sharp as i32,
                    qi_color_secondary: Some(ColorKind::Heavy as i32),
                    qi_color_chaotic: false,
                    qi_color_hunyuan: false,
                    practice_weights: vec![
                        PracticeWeight {
                            color: ColorKind::Sharp as i32,
                            weight: 0.6,
                            ratio: 0.6,
                        },
                        PracticeWeight {
                            color: ColorKind::Heavy as i32,
                            weight: 0.4,
                            ratio: 0.4,
                        },
                    ],
                },
            )),
        };

        let proto_inventory = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::InventorySnapshot(
                InventorySnapshot {
                    revision: 42,
                    containers: vec![
                        ContainerSnapshot {
                            id: "main_pack".to_string(),
                            name: "行囊".to_string(),
                            rows: 4,
                            cols: 6,
                        },
                        ContainerSnapshot {
                            id: "waist_pouch".to_string(),
                            name: "腰包".to_string(),
                            rows: 2,
                            cols: 3,
                        },
                    ],
                    placed_items: (0..8)
                        .map(|i| PlacedInventoryItem {
                            container_id: "main_pack".to_string(),
                            row: i / 6,
                            col: i % 6,
                            item: Some(InventoryItemView {
                                instance_id: 1000 + i,
                                item_id: format!("item_{i}"),
                                display_name: format!("物品{i}"),
                                grid_width: 1,
                                grid_height: 1,
                                weight: 0.5 + i as f64 * 0.1,
                                rarity: "common".to_string(),
                                description: format!("一件普通的物品，编号{i}"),
                                stack_count: 1 + i,
                                spirit_quality: 0.0,
                                durability: 100.0,
                                mineral_id: None,
                                scroll_kind: None,
                                scroll_skill_id: None,
                                scroll_xp_grant: None,
                                charges: None,
                                forge_quality: None,
                                forge_color: None,
                                forge_side_effects: vec![],
                                forge_achieved_tier: None,
                            }),
                        })
                        .collect(),
                    equipped: Some(EquippedInventorySnapshot {
                        head: None,
                        chest: None,
                        legs: None,
                        feet: None,
                        false_skin: None,
                        main_hand: Some(InventoryItemView {
                            instance_id: 999,
                            item_id: "iron_sword".to_string(),
                            display_name: "铁剑".to_string(),
                            grid_width: 1,
                            grid_height: 3,
                            weight: 2.5,
                            rarity: "uncommon".to_string(),
                            description: "一柄略有灵气的铁剑".to_string(),
                            stack_count: 1,
                            spirit_quality: 0.3,
                            durability: 85.0,
                            mineral_id: None,
                            scroll_kind: None,
                            scroll_skill_id: None,
                            scroll_xp_grant: None,
                            charges: None,
                            forge_quality: Some(0.7),
                            forge_color: Some(ColorKind::Sharp as i32),
                            forge_side_effects: vec!["锋锐".to_string()],
                            forge_achieved_tier: Some(2),
                        }),
                        off_hand: None,
                        two_hand: None,
                        treasure_belt_0: None,
                        treasure_belt_1: None,
                        treasure_belt_2: None,
                        treasure_belt_3: None,
                        back_pack: None,
                        waist_pouch: None,
                        chest_satchel: None,
                        extra_hand_0: None,
                        extra_hand_1: None,
                    }),
                    hotbar: (0..9)
                        .map(|i| HotbarSlot {
                            item: if i < 3 {
                                Some(InventoryItemView {
                                    instance_id: 2000 + i as u64,
                                    item_id: format!("hotbar_{i}"),
                                    display_name: format!("快捷栏{i}"),
                                    grid_width: 1,
                                    grid_height: 1,
                                    weight: 0.1,
                                    rarity: "common".to_string(),
                                    description: String::new(),
                                    stack_count: 1,
                                    spirit_quality: 0.0,
                                    durability: 100.0,
                                    mineral_id: None,
                                    scroll_kind: None,
                                    scroll_skill_id: None,
                                    scroll_xp_grant: None,
                                    charges: None,
                                    forge_quality: None,
                                    forge_color: None,
                                    forge_side_effects: vec![],
                                    forge_achieved_tier: None,
                                })
                            } else {
                                None
                            },
                        })
                        .collect(),
                    bone_coins: 12345,
                    weight: Some(InventoryWeight {
                        current: 15.3,
                        max: 50.0,
                    }),
                    realm: "Condense".to_string(),
                    qi_current: 800.0,
                    qi_max: 1200.0,
                    body_level: 3.5,
                },
            )),
        };

        let proto_combat_hud = ServerDataEnvelope {
            payload: Some(server_data_envelope::Payload::CombatHudState(
                CombatHudState {
                    hp_percent: 0.85,
                    qi_percent: 0.67,
                    stamina_percent: 0.92,
                    derived: Some(DerivedAttrFlags {
                        flying: false,
                        phasing: false,
                        tribulation_locked: true,
                    }),
                },
            )),
        };

        // ── 构造对应的 JSON 载荷 ─────────────────────────────────

        let json_welcome =
            ServerDataV1::welcome("欢迎来到末法残土！灵气衰退的时代，修仙之路危机四伏。");

        let json_combat_hud = serde_json::json!({
            "v": 1,
            "type": "CombatHudState",
            "hp_percent": 0.85,
            "qi_percent": 0.67,
            "stamina_percent": 0.92,
            "derived": {
                "flying": false,
                "phasing": false,
                "tribulation_locked": true
            }
        });

        // ── 跑 benchmark ────────────────────────────────────────

        struct BenchResult {
            name: &'static str,
            proto_encode_ns: u128,
            proto_decode_ns: u128,
            proto_bytes: usize,
            json_encode_ns: u128,
            json_bytes: usize,
        }

        let mut results = Vec::new();

        // Helper: benchmark one proto payload
        macro_rules! bench_proto {
            ($name:expr, $payload:expr) => {{
                let payload = &$payload;
                // warm up
                let _ = payload.encode_to_vec();

                // encode
                let start = Instant::now();
                let mut encoded = Vec::new();
                for _ in 0..ITERS {
                    encoded = payload.encode_to_vec();
                }
                let proto_encode_ns = start.elapsed().as_nanos() / ITERS as u128;
                let proto_bytes = encoded.len();

                // decode
                let start = Instant::now();
                for _ in 0..ITERS {
                    let _ = ServerDataEnvelope::decode(encoded.as_slice()).unwrap();
                }
                let proto_decode_ns = start.elapsed().as_nanos() / ITERS as u128;

                (proto_encode_ns, proto_decode_ns, proto_bytes)
            }};
        }

        // 1. Welcome
        {
            let (pe, pd, pb) = bench_proto!("Welcome", proto_welcome);

            let start = Instant::now();
            let mut jb = Vec::new();
            for _ in 0..ITERS {
                jb = serde_json::to_vec(&json_welcome).unwrap();
            }
            let je = start.elapsed().as_nanos() / ITERS as u128;

            results.push(BenchResult {
                name: "Welcome",
                proto_encode_ns: pe,
                proto_decode_ns: pd,
                proto_bytes: pb,
                json_encode_ns: je,
                json_bytes: jb.len(),
            });
        }

        // 2. PlayerState
        {
            let (pe, pd, pb) = bench_proto!("PlayerState", proto_player_state);
            // JSON: use raw json! for comparable payload
            let json_ps = serde_json::json!({
                "v": 1,
                "type": "PlayerState",
                "player": "散修·张三",
                "realm": "Condense",
                "spirit_qi": 1234.567,
                "karma": 0.42,
                "composite_power": 8765.4321,
                "zone": "qingyun_peaks",
                "local_neg_pressure": 0.15,
                "breakdown": { "combat": 3000.0, "wealth": 1500.0, "social": 800.0, "karma": 420.0, "territory": 200.0 },
                "season_state": { "season": "winter", "tick_into_phase": 5000, "phase_total_ticks": 24000, "year_index": 3 }
            });
            let start = Instant::now();
            let mut jb = Vec::new();
            for _ in 0..ITERS {
                jb = serde_json::to_vec(&json_ps).unwrap();
            }
            let je = start.elapsed().as_nanos() / ITERS as u128;
            results.push(BenchResult {
                name: "PlayerState",
                proto_encode_ns: pe,
                proto_decode_ns: pd,
                proto_bytes: pb,
                json_encode_ns: je,
                json_bytes: jb.len(),
            });
        }

        // 3. CultivationDetail (heavy payload)
        {
            let (pe, pd, pb) = bench_proto!("CultivationDetail", proto_cultivation);
            // JSON: construct comparable
            let json_cd = serde_json::json!({
                "v": 1,
                "type": "CultivationDetail",
                "realm": "Solidify",
                "meridians": (1..=20).map(|i| serde_json::json!({
                    "id": i,
                    "opened": i <= 12,
                    "flow_rate": if i <= 12 { 0.8 } else { 0.0 },
                    "flow_capacity": if i <= 12 { 1.0 } else { 0.5 },
                    "integrity": 0.95 - (i as f64 * 0.01),
                    "open_progress": if i <= 12 { 1.0 } else { 0.3 + i as f64 * 0.02 },
                    "cracks_count": if i > 15 { 1 } else { 0 }
                })).collect::<Vec<_>>(),
                "target_meridian": 13,
                "contamination_total": 12.5,
                "lifespan": {
                    "years_lived": 45.3,
                    "cap_by_realm": 200,
                    "remaining_years": 154.7,
                    "death_penalty_years": 5,
                    "tick_rate_multiplier": 1.0,
                    "is_wind_candle": false
                },
                "recent_skill_milestones_summary": "采药 Lv.5, 战斗 Lv.3",
                "skill_milestones": [
                    { "skill": "herbalism", "new_lv": 5, "achieved_at": 120000, "narration": "你的采药技艺更加纯熟", "total_xp_at": 15000 },
                    { "skill": "combat", "new_lv": 3, "achieved_at": 80000, "narration": "你的战斗本能有所觉醒", "total_xp_at": 5000 }
                ],
                "qi_color_main": "Sharp",
                "qi_color_secondary": "Heavy",
                "qi_color_chaotic": false,
                "qi_color_hunyuan": false,
                "practice_weights": [
                    { "color": "Sharp", "weight": 0.6, "ratio": 0.6 },
                    { "color": "Heavy", "weight": 0.4, "ratio": 0.4 }
                ]
            });
            let start = Instant::now();
            let mut jb = Vec::new();
            for _ in 0..ITERS {
                jb = serde_json::to_vec(&json_cd).unwrap();
            }
            let je = start.elapsed().as_nanos() / ITERS as u128;
            results.push(BenchResult {
                name: "CultivationDetail",
                proto_encode_ns: pe,
                proto_decode_ns: pd,
                proto_bytes: pb,
                json_encode_ns: je,
                json_bytes: jb.len(),
            });
        }

        // 4. InventorySnapshot (heaviest payload)
        {
            let (pe, pd, pb) = bench_proto!("InventorySnapshot", proto_inventory);
            // JSON: simplified comparable
            let json_inv = serde_json::json!({
                "v": 1,
                "type": "InventorySnapshot",
                "revision": 42,
                "containers": [
                    { "id": "main_pack", "name": "行囊", "rows": 4, "cols": 6 },
                    { "id": "waist_pouch", "name": "腰包", "rows": 2, "cols": 3 }
                ],
                "placed_items": (0..8).map(|i| serde_json::json!({
                    "container_id": "main_pack",
                    "row": i / 6,
                    "col": i % 6,
                    "item": {
                        "instance_id": 1000 + i,
                        "item_id": format!("item_{i}"),
                        "display_name": format!("物品{i}"),
                        "grid_width": 1,
                        "grid_height": 1,
                        "weight": 0.5 + i as f64 * 0.1,
                        "rarity": "common",
                        "description": format!("一件普通的物品，编号{i}"),
                        "stack_count": 1 + i,
                        "spirit_quality": 0.0,
                        "durability": 100.0
                    }
                })).collect::<Vec<_>>(),
                "bone_coins": 12345,
                "realm": "Condense",
                "qi_current": 800.0,
                "qi_max": 1200.0,
                "body_level": 3.5
            });
            let start = Instant::now();
            let mut jb = Vec::new();
            for _ in 0..ITERS {
                jb = serde_json::to_vec(&json_inv).unwrap();
            }
            let je = start.elapsed().as_nanos() / ITERS as u128;
            results.push(BenchResult {
                name: "InventorySnapshot",
                proto_encode_ns: pe,
                proto_decode_ns: pd,
                proto_bytes: pb,
                json_encode_ns: je,
                json_bytes: jb.len(),
            });
        }

        // 5. CombatHudState (small, high-frequency)
        {
            let (pe, pd, pb) = bench_proto!("CombatHudState", proto_combat_hud);
            let start = Instant::now();
            let mut jb = Vec::new();
            for _ in 0..ITERS {
                jb = serde_json::to_vec(&json_combat_hud).unwrap();
            }
            let je = start.elapsed().as_nanos() / ITERS as u128;
            results.push(BenchResult {
                name: "CombatHudState",
                proto_encode_ns: pe,
                proto_decode_ns: pd,
                proto_bytes: pb,
                json_encode_ns: je,
                json_bytes: jb.len(),
            });
        }

        // ── 输出比较表 ──────────────────────────────────────────

        println!();
        println!("╔══════════════════════╦═══════════════╦═══════════════╦══════════════╦═══════════════╦══════════════╦══════════╗");
        println!("║ Payload              ║ Proto Enc(ns) ║ Proto Dec(ns) ║ Proto Bytes  ║ JSON Enc(ns)  ║ JSON Bytes   ║ Size Δ   ║");
        println!("╠══════════════════════╬═══════════════╬═══════════════╬══════════════╬═══════════════╬══════════════╬══════════╣");
        for r in &results {
            let ratio = if r.json_bytes > 0 {
                format!("{:.1}x", r.json_bytes as f64 / r.proto_bytes as f64)
            } else {
                "N/A".to_string()
            };
            println!(
                "║ {:<20} ║ {:>13} ║ {:>13} ║ {:>12} ║ {:>13} ║ {:>12} ║ {:>8} ║",
                r.name,
                r.proto_encode_ns,
                r.proto_decode_ns,
                r.proto_bytes,
                r.json_encode_ns,
                r.json_bytes,
                ratio
            );
        }
        println!("╚══════════════════════╩═══════════════╩═══════════════╩══════════════╩═══════════════╩══════════════╩══════════╝");
        println!();
        println!("Proto 优势: 编码通常更快，体积更小（尤其重型嵌套消息如 InventorySnapshot）。");
        println!("JSON 优势: 人类可读，调试方便。");
        println!("（{ITERS} 次迭代取平均值）");
    }

    // ═══════════════════════════════════════════════════════════════
    // P4.4 — buf breaking CI 配置 pin 测试
    // ═══════════════════════════════════════════════════════════════

    /// 验证 buf breaking CI 配置存在且内容正确。
    /// 防止 CI 配置被意外移除或修改导致 proto 兼容性保护失效。
    #[test]
    fn buf_breaking_ci_is_configured() {
        // 1. 验证 e2e.yml 包含 buf breaking 步骤
        let e2e_yml = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join(".github/workflows/e2e.yml"),
        )
        .expect("应能读取 .github/workflows/e2e.yml — CI 配置缺失");

        assert!(
            e2e_yml.contains("buf breaking"),
            "e2e.yml 应包含 'buf breaking' 步骤 — proto 兼容性 CI 保护缺失"
        );
        assert!(
            e2e_yml.contains("Install buf") || e2e_yml.contains("bufbuild/buf-action"),
            "e2e.yml 应包含 buf 安装步骤（'Install buf' 或 'bufbuild/buf-action'）"
        );
        assert!(
            e2e_yml.contains("buf lint"),
            "e2e.yml 应包含 'buf lint' 步骤 — proto lint CI 保护缺失"
        );

        // 2. 验证 proto/buf.yaml 配置
        let buf_yaml = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("proto/buf.yaml"),
        )
        .expect("应能读取 proto/buf.yaml — buf 配置文件缺失");

        assert!(
            buf_yaml.contains("breaking:"),
            "buf.yaml 应包含 'breaking:' 配置块"
        );
        assert!(
            buf_yaml.contains("- FILE"),
            "buf.yaml breaking.use 应包含 '- FILE'（文件级兼容性策略），实际内容中未找到"
        );
        assert!(buf_yaml.contains("lint:"), "buf.yaml 应包含 'lint:' 配置块");
        assert!(
            buf_yaml.contains("- STANDARD"),
            "buf.yaml lint.use 应包含 '- STANDARD'"
        );
    }
}
