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
                season_name: "spring".to_string(),
                day_in_season: 15,
                total_days: 90,
                qi_multiplier: 1.2,
            }),
            social: Some(PlayerSocialSnapshot {
                fame: 100,
                notoriety: -5,
                reputation_tag: "righteous".to_string(),
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
        assert_eq!(ss.season_name, "spring");
        assert_eq!(ss.day_in_season, 15);
        let soc = decoded.social.expect("social 应存在");
        assert_eq!(soc.fame, 100);
        assert_eq!(soc.notoriety, -5);
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
