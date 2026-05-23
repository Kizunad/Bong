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
        let decoded = AlchemyFurnace::decode(bytes.as_slice())
            .expect("AlchemyFurnace decode 失败");
        assert_eq!(decoded.pos_x, Some(-12), "pos_x 应为 -12");
        assert_eq!(decoded.pos_y, Some(64), "pos_y 应为 64");
        assert_eq!(decoded.pos_z, Some(38), "pos_z 应为 38");
        assert_eq!(decoded.tier, 2, "tier 应为 2");
        assert!((decoded.integrity - 0.95).abs() < 1e-9, "integrity 应为 0.95");
        assert!((decoded.integrity_max - 1.0).abs() < 1e-9, "integrity_max 应为 1.0");
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
        let decoded = AlchemyFurnace::decode(bytes.as_slice())
            .expect("AlchemyFurnace (no pos) decode 失败");
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
        let decoded = AlchemySession::decode(bytes.as_slice())
            .expect("AlchemySession decode 失败");
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
        let decoded = AlchemySession::decode(bytes.as_slice())
            .expect("AlchemySession (empty) decode 失败");
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
            learned: vec![
                AlchemyRecipeEntry {
                    id: "kai_mai_pill_v0".to_string(),
                    display_name: "开脉丹方".to_string(),
                    body_text: "...".to_string(),
                    author: "散修 刘三".to_string(),
                    era: "末法 十二年".to_string(),
                    max_known: 8,
                },
            ],
            current_index: 0,
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyRecipeBook::decode(bytes.as_slice())
            .expect("AlchemyRecipeBook decode 失败");
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
        let decoded = AlchemyOpenFurnace::decode(bytes.as_slice())
            .expect("AlchemyOpenFurnace decode 失败");
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
        let decoded = AlchemyFeedSlot::decode(bytes.as_slice())
            .expect("AlchemyFeedSlot decode 失败");
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
        let decoded = AlchemyTakeBack::decode(bytes.as_slice())
            .expect("AlchemyTakeBack decode 失败");
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
        let decoded = AlchemyIgnite::decode(bytes.as_slice())
            .expect("AlchemyIgnite decode 失败");
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
        let decoded = AlchemyTurnPage::decode(bytes.as_slice())
            .expect("AlchemyTurnPage decode 失败");
        assert_eq!(decoded.delta, -1);
    }

    #[test]
    fn alchemy_learn_recipe_roundtrip() {
        let msg = AlchemyLearnRecipe {
            recipe_id: "kai_mai_pill_v0".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = AlchemyLearnRecipe::decode(bytes.as_slice())
            .expect("AlchemyLearnRecipe decode 失败");
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
        let decoded = AlchemyTakePill::decode(bytes.as_slice())
            .expect("AlchemyTakePill decode 失败");
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
        let decoded = ForgeStation::decode(bytes.as_slice())
            .expect("ForgeStation decode 失败");
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
            assert_eq!(variant as i32, wire, "ForgeStep::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "TemperBeat::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "ForgeOutcomeBucket::{variant:?} wire 值应为 {wire}");
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
                state: Some(forge_step_state::State::Tempering(ForgeStepStateTempering {
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
                })),
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
        let decoded = ForgeOutcome::decode(bytes.as_slice())
            .expect("ForgeOutcome decode 失败");
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
        let decoded = ForgeOutcome::decode(bytes.as_slice())
            .expect("ForgeOutcome (flawed) decode 失败");
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
        let decoded = ForgeBlueprintBook::decode(bytes.as_slice())
            .expect("ForgeBlueprintBook decode 失败");
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
        let decoded = ForgeStartSession::decode(bytes.as_slice())
            .expect("ForgeStartSession decode 失败");
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
        let decoded = ForgeTemperingHit::decode(bytes.as_slice())
            .expect("ForgeTemperingHit decode 失败");
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
        let decoded = ForgeStepAdvance::decode(bytes.as_slice())
            .expect("ForgeStepAdvance decode 失败");
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
        let decoded = ForgeLearnBlueprint::decode(bytes.as_slice())
            .expect("ForgeLearnBlueprint decode 失败");
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
            assert_eq!(variant as i32, wire, "CraftCategory::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "CraftFailureReason::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "InsightTrigger::{variant:?} wire 值应为 {wire}");
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
                    CraftMaterialPair { template_id: "iron_ingot".to_string(), count: 1 },
                    CraftMaterialPair { template_id: "wood_handle".to_string(), count: 1 },
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
        let decoded = CraftRecipeList::decode(bytes.as_slice())
            .expect("CraftRecipeList decode 失败");
        assert_eq!(decoded.v, 1);
        assert_eq!(decoded.player_id, "offline:Alice");
        assert_eq!(decoded.recipes.len(), 1);
        assert_eq!(decoded.recipes[0].id, "craft.example.herb_knife.iron");
        assert_eq!(decoded.recipes[0].category, CraftCategory::Tool as i32);
        assert_eq!(decoded.recipes[0].materials.len(), 2);
        assert_eq!(decoded.recipes[0].output.as_ref().unwrap().template_id, "herb_knife_iron");
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
        let decoded = CraftRecipeList::decode(bytes.as_slice())
            .expect("CraftRecipeList (empty) decode 失败");
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
        let decoded = CraftSessionState::decode(bytes.as_slice())
            .expect("CraftSessionState decode 失败");
        assert!(decoded.active);
        assert_eq!(decoded.recipe_id.as_deref(), Some("craft.example.eclipse_needle.iron"));
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
        let decoded = CraftOutcome::decode(bytes.as_slice())
            .expect("CraftOutcome (completed) decode 失败");
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
        let decoded = CraftOutcome::decode(bytes.as_slice())
            .expect("CraftOutcome (failed) decode 失败");
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
        let decoded = RecipeUnlocked::decode(bytes.as_slice())
            .expect("RecipeUnlocked (scroll) decode 失败");
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
        let decoded = RecipeUnlocked::decode(bytes.as_slice())
            .expect("RecipeUnlocked (mentor) decode 失败");
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
        let decoded = RecipeUnlocked::decode(bytes.as_slice())
            .expect("RecipeUnlocked (insight) decode 失败");
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
        let decoded = CraftStart::decode(bytes.as_slice())
            .expect("CraftStart decode 失败");
        assert_eq!(decoded.recipe_id, "craft.example.herb_knife.iron");
        assert_eq!(decoded.quantity, 3);
    }

    #[test]
    fn craft_cancel_roundtrip() {
        let msg = CraftCancel {};
        let bytes = msg.encode_to_vec();
        let decoded = CraftCancel::decode(bytes.as_slice())
            .expect("CraftCancel decode 失败");
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
            assert_eq!(variant as i32, wire, "BotanyHarvestMode::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "BotanyModelOverlay::{variant:?} wire 值应为 {wire}");
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
        assert_eq!(decoded.profiles[0].model_overlay, BotanyModelOverlay::Emissive as i32);
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
        let decoded = BotanySkill::decode(bytes.as_slice())
            .expect("BotanySkill decode 失败");
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
        let decoded = MiningProgress::decode(bytes.as_slice())
            .expect("MiningProgress decode 失败");
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
        let decoded = LumberProgress::decode(bytes.as_slice())
            .expect("LumberProgress decode 失败");
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
            assert_eq!(variant as i32, wire, "GatheringTargetType::{variant:?} wire 值应为 {wire}");
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
            assert_eq!(variant as i32, wire, "GatheringQualityHint::{variant:?} wire 值应为 {wire}");
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
        let decoded = GatheringSession::decode(bytes.as_slice())
            .expect("GatheringSession decode 失败");
        assert_eq!(decoded.session_id, "gather-01");
        assert_eq!(decoded.progress_ticks, 30);
        assert_eq!(decoded.total_ticks, 100);
        assert_eq!(decoded.target_type, GatheringTargetType::Herb as i32);
        assert_eq!(decoded.quality_hint, GatheringQualityHint::FineLikely as i32);
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
            assert_eq!(variant as i32, wire, "LingtianSessionKind::{variant:?} wire 值应为 {wire}");
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
        let decoded = LingtianStartTill::decode(bytes.as_slice())
            .expect("LingtianStartTill decode 失败");
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
        let decoded = LingtianStartRenew::decode(bytes.as_slice())
            .expect("LingtianStartRenew decode 失败");
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
        let msg = MineralProbe {
            x: 8,
            y: 32,
            z: 8,
        };
        let bytes = msg.encode_to_vec();
        let decoded = MineralProbe::decode(bytes.as_slice())
            .expect("MineralProbe decode 失败");
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
            ("AlchemyFurnace", server_data_envelope::Payload::AlchemyFurnace(AlchemyFurnace {
                pos_x: Some(0), pos_y: Some(64), pos_z: Some(0),
                tier: 1, integrity: 1.0, integrity_max: 1.0,
                owner_name: "t".to_string(), has_session: false,
            })),
            ("AlchemySession", server_data_envelope::Payload::AlchemySession(AlchemySession {
                recipe_id: None, active: false, elapsed_ticks: 0, target_ticks: 0,
                temp_current: 0.0, temp_target: 0.0, temp_band: 0.0,
                qi_injected: 0.0, qi_target: 0.0, status_label: "".to_string(),
                stages: vec![], interventions_recent: vec![],
            })),
            ("AlchemyOutcomeForecast", server_data_envelope::Payload::AlchemyOutcomeForecast(AlchemyOutcomeForecast {
                perfect_pct: 0.0, good_pct: 0.0, flawed_pct: 0.0,
                waste_pct: 0.0, explode_pct: 0.0, perfect_note: "".to_string(),
                good_note: "".to_string(), flawed_note: "".to_string(),
            })),
            ("AlchemyOutcomeResolved", server_data_envelope::Payload::AlchemyOutcomeResolved(AlchemyOutcomeResolved {
                bucket: 0, recipe_id: None, pill: None, quality: None,
                toxin_amount: None, toxin_color: None, qi_gain: None,
                side_effect_tag: None, flawed_path: false, damage: None, meridian_crack: None,
            })),
            ("AlchemyRecipeBook", server_data_envelope::Payload::AlchemyRecipeBook(AlchemyRecipeBook {
                learned: vec![], current_index: 0,
            })),
            ("AlchemyContamination", server_data_envelope::Payload::AlchemyContamination(AlchemyContamination {
                levels: vec![], metabolism_note: "".to_string(),
            })),
            ("ForgeStation", server_data_envelope::Payload::ForgeStation(ForgeStation {
                station_id: "s".to_string(), tier: 1, integrity: 1.0,
                owner_name: "t".to_string(), has_session: false,
            })),
            ("ForgeSession", server_data_envelope::Payload::ForgeSession(ForgeSessionData {
                session_id: 1, blueprint_id: "x".to_string(), blueprint_name: "x".to_string(),
                active: false, current_step: 0, step_index: 0, achieved_tier: 0,
                step_state: None,
            })),
            ("ForgeOutcome", server_data_envelope::Payload::ForgeOutcome(ForgeOutcome {
                session_id: 1, blueprint_id: "x".to_string(),
                bucket: 0, weapon_item: None, quality: 0.0,
                color: None, side_effects: vec![], achieved_tier: 0, flawed_path: false,
            })),
            ("ForgeBlueprintBook", server_data_envelope::Payload::ForgeBlueprintBook(ForgeBlueprintBook {
                learned: vec![], current_index: 0,
            })),
            ("CraftRecipeList", server_data_envelope::Payload::CraftRecipeList(CraftRecipeList {
                v: 1, player_id: "x".to_string(), recipes: vec![], ts: 0,
            })),
            ("CraftSessionState", server_data_envelope::Payload::CraftSessionState(CraftSessionState {
                v: 1, player_id: "x".to_string(), active: false, recipe_id: None,
                elapsed_ticks: 0, total_ticks: 0, completed_count: 0, total_count: 0, ts: 0,
            })),
            ("CraftOutcome", server_data_envelope::Payload::CraftOutcome(CraftOutcome {
                outcome: None,
            })),
            ("RecipeUnlocked", server_data_envelope::Payload::RecipeUnlocked(RecipeUnlocked {
                v: 1, player_id: "x".to_string(), recipe_id: "y".to_string(),
                source: None, unlocked_at_tick: 0, ts: 0,
            })),
            ("BotanyHarvestProgress", server_data_envelope::Payload::BotanyHarvestProgress(BotanyHarvestProgress {
                session_id: "s".to_string(), target_id: "t".to_string(),
                target_name: "n".to_string(), plant_kind: "k".to_string(),
                mode: "manual".to_string(), progress: 0.0,
                auto_selectable: false, request_pending: false,
                interrupted: false, completed: false, detail: "".to_string(),
                hazard_hints: vec![], target_pos_x: None, target_pos_y: None, target_pos_z: None,
            })),
            ("BotanyPlantV2RenderProfiles", server_data_envelope::Payload::BotanyPlantV2RenderProfiles(BotanyPlantV2RenderProfiles {
                profiles: vec![],
            })),
            ("BotanySkill", server_data_envelope::Payload::BotanySkill(BotanySkill {
                level: 0, xp: 0, xp_to_next_level: 0, auto_unlock_level: 0,
            })),
            ("MiningProgress", server_data_envelope::Payload::MiningProgress(MiningProgress {
                session_id: "m".to_string(), ore_pos_x: 0, ore_pos_y: 0, ore_pos_z: 0,
                progress: 0.0, interrupted: false, completed: false,
            })),
            ("LumberProgress", server_data_envelope::Payload::LumberProgress(LumberProgress {
                session_id: "l".to_string(), log_pos_x: 0, log_pos_y: 0, log_pos_z: 0,
                progress: 0.0, interrupted: false, completed: false, detail: "".to_string(),
            })),
            ("GatheringSession", server_data_envelope::Payload::GatheringSession(GatheringSession {
                session_id: "g".to_string(), progress_ticks: 0, total_ticks: 0,
                target_name: "n".to_string(), target_type: 0, quality_hint: 0,
                tool_used: None, interrupted: false, completed: false,
            })),
            ("LingtianSession", server_data_envelope::Payload::LingtianSession(LingtianSessionData {
                active: false, kind: 0, pos_x: 0, pos_y: 0, pos_z: 0,
                elapsed_ticks: 0, target_ticks: 0, plant_id: None, source: None,
                dye_contamination: None, dye_contamination_warning: false,
            })),
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
            ("AlchemyOpenFurnace", client_request_envelope::Payload::AlchemyOpenFurnace(AlchemyOpenFurnace {
                furnace_pos_x: 0, furnace_pos_y: 64, furnace_pos_z: 0,
            })),
            ("AlchemyFeedSlot", client_request_envelope::Payload::AlchemyFeedSlot(AlchemyFeedSlot {
                furnace_pos_x: 0, furnace_pos_y: 64, furnace_pos_z: 0,
                slot_idx: 0, material: "x".to_string(), count: 1,
            })),
            ("AlchemyTakeBack", client_request_envelope::Payload::AlchemyTakeBack(AlchemyTakeBack {
                furnace_pos_x: 0, furnace_pos_y: 64, furnace_pos_z: 0, slot_idx: 0,
            })),
            ("AlchemyIgnite", client_request_envelope::Payload::AlchemyIgnite(AlchemyIgnite {
                furnace_pos_x: 0, furnace_pos_y: 64, furnace_pos_z: 0,
                recipe_id: "x".to_string(),
            })),
            ("AlchemyIntervention", client_request_envelope::Payload::AlchemyIntervention(AlchemyIntervention {
                furnace_pos_x: 0, furnace_pos_y: 64, furnace_pos_z: 0,
                intervention: None,
            })),
            ("AlchemyTurnPage", client_request_envelope::Payload::AlchemyTurnPage(AlchemyTurnPage {
                delta: 1,
            })),
            ("AlchemyLearnRecipe", client_request_envelope::Payload::AlchemyLearnRecipe(AlchemyLearnRecipe {
                recipe_id: "x".to_string(),
            })),
            ("AlchemyLearnRecipeFragment", client_request_envelope::Payload::AlchemyLearnRecipeFragment(AlchemyLearnRecipeFragment {
                item_instance_id: 1,
            })),
            ("AlchemyTakePill", client_request_envelope::Payload::AlchemyTakePill(AlchemyTakePill {
                pill_item_id: "x".to_string(),
            })),
            ("ForgeStartSession", client_request_envelope::Payload::ForgeStartSession(ForgeStartSession {
                station_id: "s".to_string(), blueprint_id: "b".to_string(), materials: vec![],
            })),
            ("ForgeTemperingHit", client_request_envelope::Payload::ForgeTemperingHit(ForgeTemperingHit {
                session_id: 1, beat: "L".to_string(), ticks_remaining: 0,
            })),
            ("ForgeInscriptionScroll", client_request_envelope::Payload::ForgeInscriptionScroll(ForgeInscriptionScroll {
                session_id: 1, inscription_id: "x".to_string(),
            })),
            ("ForgeConsecrationInject", client_request_envelope::Payload::ForgeConsecrationInject(ForgeConsecrationInject {
                session_id: 1, qi_amount: 0.0,
            })),
            ("ForgeStepAdvance", client_request_envelope::Payload::ForgeStepAdvance(ForgeStepAdvance {
                session_id: 1,
            })),
            ("ForgeBlueprintTurnPage", client_request_envelope::Payload::ForgeBlueprintTurnPage(ForgeBlueprintTurnPage {
                delta: 1,
            })),
            ("ForgeLearnBlueprint", client_request_envelope::Payload::ForgeLearnBlueprint(ForgeLearnBlueprint {
                blueprint_id: "x".to_string(),
            })),
            ("CraftStart", client_request_envelope::Payload::CraftStart(CraftStart {
                recipe_id: "x".to_string(), quantity: 1,
            })),
            ("CraftCancel", client_request_envelope::Payload::CraftCancel(CraftCancel {})),
            ("BotanyHarvestRequest", client_request_envelope::Payload::BotanyHarvestRequest(BotanyHarvestRequest {
                session_id: "s".to_string(), mode: BotanyHarvestMode::Manual as i32,
            })),
            ("LingtianStartTill", client_request_envelope::Payload::LingtianStartTill(LingtianStartTill {
                x: 0, y: 64, z: 0, hoe_instance_id: 1, mode: "manual".to_string(),
            })),
            ("LingtianStartRenew", client_request_envelope::Payload::LingtianStartRenew(LingtianStartRenew {
                x: 0, y: 64, z: 0, hoe_instance_id: 1,
            })),
            ("LingtianStartPlanting", client_request_envelope::Payload::LingtianStartPlanting(LingtianStartPlanting {
                x: 0, y: 64, z: 0, plant_id: "x".to_string(),
            })),
            ("LingtianStartHarvest", client_request_envelope::Payload::LingtianStartHarvest(LingtianStartHarvest {
                x: 0, y: 64, z: 0, mode: "manual".to_string(),
            })),
            ("LingtianStartReplenish", client_request_envelope::Payload::LingtianStartReplenish(LingtianStartReplenish {
                x: 0, y: 64, z: 0, source: "bone_coin".to_string(),
            })),
            ("LingtianStartDrainQi", client_request_envelope::Payload::LingtianStartDrainQi(LingtianStartDrainQi {
                x: 0, y: 64, z: 0,
            })),
            ("MineralProbe", client_request_envelope::Payload::MineralProbe(MineralProbe {
                x: 8, y: 32, z: 8,
            })),
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
        let decoded = WoundsSnapshot::decode(bytes.as_slice())
            .expect("WoundsSnapshot decode 失败");
        assert_eq!(decoded.wounds.len(), 2, "应有 2 条伤口记录");
        assert_eq!(decoded.wounds[0].part, "chest");
        assert_eq!(decoded.wounds[0].severity, 0.6_f32);
        assert!(decoded.wounds[1].scar, "head 伤口应有疤痕");
    }

    #[test]
    fn wounds_snapshot_roundtrip_empty() {
        let msg = WoundsSnapshot { wounds: vec![] };
        let bytes = msg.encode_to_vec();
        let decoded = WoundsSnapshot::decode(bytes.as_slice())
            .expect("空 WoundsSnapshot decode 失败");
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
        let decoded = DefenseWindow::decode(bytes.as_slice())
            .expect("DefenseWindow decode 失败");
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
            let decoded = CastSync::decode(bytes.as_slice())
                .unwrap_or_else(|e| panic!("CastSync phase={phase:?} outcome={outcome:?} decode 失败: {e}"));
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
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("CastSync envelope decode 失败");
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
        let decoded = QuickSlotConfig::decode(bytes.as_slice())
            .expect("QuickSlotConfig decode 失败");
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
        let decoded = SkillBarConfig::decode(bytes.as_slice())
            .expect("SkillBarConfig decode 失败");
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
        let decoded = TechniquesSnapshot::decode(bytes.as_slice())
            .expect("TechniquesSnapshot decode 失败");
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
        let decoded = UnlocksSync::decode(bytes.as_slice())
            .expect("UnlocksSync decode 失败");
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
        let decoded = DerivedAttrsSync::decode(bytes.as_slice())
            .expect("DerivedAttrsSync decode 失败");
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
            assert_eq!(
                decoded.channel, channel as i32,
                "channel 应为 {channel:?}"
            );
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
        let decoded = WeaponEquipped::decode(bytes.as_slice())
            .expect("WeaponEquipped decode 失败");
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
        let decoded = WeaponEquipped::decode(bytes.as_slice())
            .expect("WeaponEquipped 无武器 decode 失败");
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
        let decoded = WeaponBroken::decode(bytes.as_slice())
            .expect("WeaponBroken decode 失败");
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
        let decoded = TreasureEquipped::decode(bytes.as_slice())
            .expect("TreasureEquipped decode 失败");
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
        let decoded = VortexFieldState::decode(bytes.as_slice())
            .expect("VortexFieldState decode 失败");
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
        let decoded = DuguPoisonState::decode(bytes.as_slice())
            .expect("DuguPoisonState decode 失败");
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
        let decoded = DuguPoisonState::decode(bytes.as_slice())
            .expect("cleared DuguPoisonState decode 失败");
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
            assert_eq!(
                decoded.severity, sev as i32,
                "severity 应为 {sev:?}"
            );
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
        let decoded = PoisonTraitState::decode(bytes.as_slice())
            .expect("PoisonTraitState decode 失败");
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
        let decoded = CarrierState::decode(bytes.as_slice())
            .expect("CarrierState decode 失败");
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
        let decoded = CarrierState::decode(bytes.as_slice())
            .expect("CarrierState 无 instance decode 失败");
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
        let decoded = FalseSkinState::decode(bytes.as_slice())
            .expect("FalseSkinState decode 失败");
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
        let decoded = FalseSkinState::decode(bytes.as_slice())
            .expect("空 FalseSkinState decode 失败");
        assert!(decoded.kind.is_none());
        assert!(decoded.layers.is_empty());
    }

    #[test]
    fn false_skin_kind_all_variants() {
        let kinds = [
            FalseSkinKind::SpiderSilk,
            FalseSkinKind::RottenWoodArmor,
        ];
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
        let decoded = CombatEventFloater::decode(bytes.as_slice())
            .expect("CombatEventFloater decode 失败");
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
        let decoded = PillBuffStatus::decode(bytes.as_slice())
            .expect("PillBuffStatus decode 失败");
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
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("ChargeCarrier C2S decode 失败");
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
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("ThrowCarrier C2S decode 失败");
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
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("UseQuickSlot C2S decode 失败");
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
        let decoded = ClientRequestEnvelope::decode(bytes.as_slice())
            .expect("SkillBarBind item decode 失败");
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
            Some(client_request_envelope::Payload::CombatCreateNewCharacter(_))
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
}
