use valence::prelude::{Added, Changed, Client, Entity, Query, Res, Username, With};

use crate::combat::sword_basics::sword_proficiency_label;
use crate::cultivation::known_techniques::{KnownTechniques, TechniqueRegistry};
use crate::network::agent_bridge::{
    payload_type_label, serialize_server_data_payload, SERVER_DATA_CHANNEL,
};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::combat_hud::{
    TechniqueEntryV1, TechniqueRequiredMeridianV1, TechniquesSnapshotV1,
};
use crate::schema::server_data::{ServerDataPayloadV1, ServerDataV1};

type TechniquesSnapshotFilter = (With<Client>, Changed<KnownTechniques>);
type JoinTechniquesSnapshotFilter = (With<Client>, Added<KnownTechniques>);
type TechniquesSnapshotQueryItem<'a> = (Entity, &'a mut Client, &'a Username, &'a KnownTechniques);

pub fn emit_techniques_snapshot_payloads(
    registry: Res<TechniqueRegistry>,
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, TechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(
            &registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

pub fn emit_join_techniques_snapshot_payloads(
    registry: Res<TechniqueRegistry>,
    mut clients: Query<TechniquesSnapshotQueryItem<'_>, JoinTechniquesSnapshotFilter>,
) {
    for (entity, mut client, username, known) in &mut clients {
        send_techniques_snapshot_to_client(
            &registry,
            entity,
            &mut client,
            username.0.as_str(),
            known,
        );
    }
}

fn build_techniques_snapshot(
    registry: &TechniqueRegistry,
    known: &KnownTechniques,
) -> TechniquesSnapshotV1 {
    TechniquesSnapshotV1 {
        entries: known
            .entries
            .iter()
            .filter_map(|known| {
                let definition = registry.get(&known.id)?;
                Some(TechniqueEntryV1 {
                    id: definition.id.to_string(),
                    display_name: definition.display_name.to_string(),
                    grade: definition.grade.to_string(),
                    proficiency: known.proficiency.clamp(0.0, 1.0),
                    proficiency_label: sword_proficiency_label(known.proficiency).to_string(),
                    active: known.active,
                    description: definition.description.to_string(),
                    required_realm: definition.required_realm.to_string(),
                    required_meridians: definition
                        .required_meridians
                        .iter()
                        .map(|required| TechniqueRequiredMeridianV1 {
                            channel: required.channel.to_string(),
                            min_health: required.min_health,
                        })
                        .collect(),
                    qi_cost: definition.qi_cost,
                    stamina_cost: definition.stamina_cost,
                    cast_ticks: definition.cast_ticks,
                    cooldown_ticks: definition.cooldown_ticks,
                    range: definition.range,
                })
            })
            .collect(),
    }
}

pub fn send_techniques_snapshot_to_client(
    registry: &TechniqueRegistry,
    entity: Entity,
    client: &mut Client,
    username: &str,
    known: &KnownTechniques,
) {
    let snapshot = build_techniques_snapshot(registry, known);
    let payload = ServerDataV1::new(ServerDataPayloadV1::TechniquesSnapshot(snapshot));
    let payload_type = payload_type_label(payload.payload_type());
    let payload_bytes = match serialize_server_data_payload(&payload) {
        Ok(bytes) => bytes,
        Err(error) => {
            log_payload_build_error(payload_type, &error);
            return;
        }
    };
    send_server_data_payload(client, payload_bytes.as_slice());
    tracing::debug!(
        "[bong][network] sent {} {} payload to entity {entity:?} for `{username}`",
        SERVER_DATA_CHANNEL,
        payload_type
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cultivation::known_techniques::{KnownTechnique, KnownTechniques};

    #[test]
    fn snapshot_uses_injected_registry_and_omits_unknown_known_ids() {
        let registry =
            TechniqueRegistry::load_for_tests_with_override("sword.cleave", |definition| {
                definition.display_name = "运行时覆写劈".to_string();
                definition.qi_cost = 7.25;
                definition.range = 4.5;
            });
        let known = KnownTechniques {
            entries: vec![
                KnownTechnique {
                    id: "unknown.removed".to_string(),
                    proficiency: 0.4,
                    active: true,
                },
                KnownTechnique {
                    id: "sword.cleave".to_string(),
                    proficiency: 1.5,
                    active: true,
                },
            ],
        };

        let snapshot = build_techniques_snapshot(&registry, &known);

        assert_eq!(snapshot.entries.len(), 1);
        let entry = &snapshot.entries[0];
        assert_eq!(entry.id, "sword.cleave");
        assert_eq!(entry.display_name, "运行时覆写劈");
        assert_eq!(entry.proficiency, 1.0);
        assert_eq!(entry.qi_cost, 7.25);
        assert_eq!(entry.range, 4.5);
    }

    #[test]
    fn snapshot_qi_cost_preserves_f64_precision_boundary() {
        // registry 的 f64 成本必须原样进入快照，避免服务端扣费与客户端展示不一致。
        let registry =
            TechniqueRegistry::load_for_tests_with_override("sword.cleave", |definition| {
                definition.qi_cost = 16_777_217.0;
            });
        let known = KnownTechniques {
            entries: vec![KnownTechnique {
                id: "sword.cleave".to_string(),
                proficiency: 0.5,
                active: true,
            }],
        };
        let snapshot = build_techniques_snapshot(&registry, &known);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(
            snapshot.entries[0].qi_cost, 16_777_217.0,
            "快照必须保留 registry 的 f64 真元成本，不能窄化成 16777216"
        );
    }

    #[test]
    fn aggregate_snapshot_bound_accepts_checked_in_catalog() {
        // M18：catalog 被接受 ⇒ 学会全部条目的玩家必能收到完整快照。checked-in catalog
        // 49 条、最长 description 41 字节，最坏聚合大小必须显著低于 32 KiB wire 上限。
        let registry = TechniqueRegistry::load_for_tests();
        let aggregate = registry.aggregate_snapshot_size();
        assert!(
            aggregate <= crate::schema::common::MAX_PAYLOAD_BYTES,
            "checked-in catalog worst-case snapshot ~{aggregate} bytes must fit MAX_PAYLOAD_BYTES = {}",
            crate::schema::common::MAX_PAYLOAD_BYTES
        );
        // 保守估计的实际余量：即使再翻 10 倍也仍在限制内（与 loader 的
        // MAX_CATALOG_ENTRIES = 512 相对照，填满 512 条就应突破）。
        let extrapolated = aggregate * 10;
        assert!(
            extrapolated > crate::schema::common::MAX_PAYLOAD_BYTES,
            "10x catalog scale should exceed the wire limit (aggregate={aggregate}, extrapolated={extrapolated})"
        );
    }

    #[test]
    fn oversize_description_is_rejected_at_loader_not_dropped_at_send() {
        // M18 负向用例：超长 description 必须在 loader 拒绝（单条 1024 字节上限，
        // checked-in 最长 41 字节），不能等发送端 `PayloadBuildError::Oversize`
        // 整包丢弃让快照消失。
        let too_long = "刀".repeat(2_000);
        let error = TechniqueRegistry::load_from_contents_for_tests(&format!(
            r#"
[[techniques]]
id = "sword.cleave"
display_name = "劈"
grade = "common"
description = "{too_long}"
required_realm = "Awaken"
required_meridians = []
required_race = {{ kind = "humanoid" }}
qi_cost = 1.0
stamina_cost = 0.0
cast_ticks = 10
cooldown_ticks = 30
range = 3.0
icon_texture = "bong-client:textures/gui/items/skill_scroll_sword_cleave.png"
category = "attack"
dispatch = "metadata_backed"
"#
        ))
        .expect_err("2000-byte description must be rejected by the loader (M18)");
        assert!(
            format!("{error}").contains("description"),
            "rejection message should name the description field, got {error}"
        );
    }

    #[test]
    fn control_character_snapshot_is_rejected_before_json_send() {
        let encoded_control_bytes = r"\u0001".repeat(1_000);
        let mut catalog = String::new();
        for index in 0..8 {
            catalog.push_str(&format!(
                r#"
[[techniques]]
id = "bulk.{index}"
display_name = "批"
grade = "common"
description = "{encoded_control_bytes}"
required_realm = "Awaken"
required_meridians = []
required_race = {{ kind = "any" }}
qi_cost = 1.0
stamina_cost = 0.0
cast_ticks = 10
cooldown_ticks = 30
range = 3.0
icon_texture = "bong-client:textures/gui/items/skill_scroll_sword_cleave.png"
category = "attack"
dispatch = "direct_generic"
"#
            ));
        }

        let registry = TechniqueRegistry::load_from_contents_for_tests(&catalog)
            .expect("control-character descriptions under 1024 bytes must load");
        let known = KnownTechniques {
            entries: registry
                .iter()
                .map(|definition| KnownTechnique {
                    id: definition.id.clone(),
                    proficiency: 0.5,
                    active: true,
                })
                .collect(),
        };
        let payload = ServerDataV1::new(ServerDataPayloadV1::TechniquesSnapshot(
            build_techniques_snapshot(&registry, &known),
        ));
        let actual_json = payload
            .to_json_bytes_checked()
            .expect_err("control-character snapshot must exceed the JSON wire limit");
        assert!(
            matches!(
                actual_json,
                crate::schema::server_data::ServerDataBuildError::Oversize { .. }
            ),
            "the real JSON serializer must report Oversize, got {actual_json:?}"
        );
        assert!(
            registry.aggregate_snapshot_size() > crate::schema::common::MAX_PAYLOAD_BYTES,
            "startup estimate must cover the six-byte control-character escapes"
        );

        let error = crate::cultivation::known_techniques::validate_startup_wiring(
            &registry,
            &crate::cultivation::skill_registry::SkillRegistry::default(),
            &crate::cultivation::meridian::severed::SkillMeridianDependencies::default(),
        )
        .expect_err("a registry whose real JSON snapshot is oversized must fail startup wiring");
        assert!(
            error.to_string().contains("MAX_PAYLOAD_BYTES"),
            "startup rejection must identify the payload limit, got {error}"
        );
    }
}
