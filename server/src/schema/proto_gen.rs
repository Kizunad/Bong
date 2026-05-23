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
}
