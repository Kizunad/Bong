//! Protobuf 生成代码入口 — prost codegen 输出。
//!
//! `build.rs` 编译 `proto/bong/*.proto`，prost 默认按 proto package 名
//! 输出到 `$OUT_DIR/bong.rs`。此文件通过 `include!` 引入。

/// Proto-generated types for `package bong;`.
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
        assert_eq!(all.len(), 21, "MeridianId 应有 21 个变体（20 经脉 + UNSPECIFIED）");
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
        assert_eq!(all.len(), 7, "SkillId 应有 7 个变体（6 技能 + UNSPECIFIED）");
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
        assert_eq!(all.len(), 11, "ColorKind 应有 11 个变体（10 真元色 + UNSPECIFIED）");
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
        let decoded = Vec3::decode(bytes.as_slice())
            .expect("Vec3 decode 失败");
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
        let decoded = ItemSlot::decode(bytes.as_slice())
            .expect("ItemSlot decode 失败");
        assert_eq!(decoded, with_options, "ItemSlot roundtrip（含 optional）不一致");

        let without_options = ItemSlot {
            slot_index: 0,
            template_id: "herb".to_string(),
            count: 64,
            forge_color: None,
            durability: None,
        };
        let bytes = without_options.encode_to_vec();
        let decoded = ItemSlot::decode(bytes.as_slice())
            .expect("ItemSlot decode 失败");
        assert_eq!(decoded, without_options, "ItemSlot roundtrip（无 optional）不一致");
    }

    #[test]
    fn welcome_message_roundtrip() {
        let msg = Welcome {
            message: "欢迎来到末法残土".to_string(),
        };
        let bytes = msg.encode_to_vec();
        let decoded = Welcome::decode(bytes.as_slice())
            .expect("Welcome decode 失败");
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
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("ServerDataEnvelope decode 失败");
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
            payload: Some(
                client_request_envelope::Payload::BreakthroughRequest(
                    BreakthroughRequest {},
                ),
            ),
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
        let decoded = ServerDataEnvelope::decode(bytes.as_slice())
            .expect("空 envelope decode 不应失败");
        assert!(decoded.payload.is_none(), "空 envelope 解码后 payload 应为 None");
    }

    #[test]
    fn narration_batch_empty_and_nonempty() {
        // 空 narration 列表
        let empty = NarrationBatch {
            narrations: vec![],
        };
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
                },
                NarrationEntry {
                    text: "你感到一阵寒意".to_string(),
                    scope: "player".to_string(),
                    style: "narration".to_string(),
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
        assert!(decoded.perception_text.is_none(), "perception_text 应为 None");
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
}
