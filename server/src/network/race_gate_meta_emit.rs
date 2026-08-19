//! plan-race-system-v1 P3c — 种族门元数据表（`RaceGateMeta`）构建 + 下发。
//!
//! 静态的 per-template / per-technique 种族门数据，一次性下发（join 首帧），
//! **不**随每次 inventory snapshot 重发（既浪费又未归一化）。client 缓存两表后
//! 按当前身份离线判置灰（装备域=当前形态 / 功法域=本体，决议 §8.1 #5/#6）。
//!
//! 两表都只装 **非 `Any`** 条目——`Any` 是默认，client 缺省即 `Any`（省流量）：
//! - `item_wearer_race`：从 [`ItemRegistry`] 全模板取 `wearer_race != Any`。
//! - `technique_required_race`：从 [`TechniqueRegistry`] 当前定义动态取
//!   `required_race != Any`；Any 仍由 client 缺省语义表达，不进入表。
//!
//! 内容与玩家身份无关（静态），因此易形 / RaceChange **不需重发**——client 用
//! `PlayerRaceIdentityStore` 的最新身份对同一张表重判即可。故下发时机只需
//! join 首帧一次（`LastSentRaceGateMeta` 标记组件防重发）。

use valence::prelude::{bevy_ecs, Client, Commands, Component, Entity, Query, Res, With};

use crate::cultivation::known_techniques::TechniqueRegistry;
use crate::inventory::ItemRegistry;
use crate::network::agent_bridge::{payload_type_label, serialize_server_data_payload};
use crate::network::{log_payload_build_error, send_server_data_payload};
use crate::schema::server_data::{
    RaceGateMetaEntryV1, RaceGateMetaV1, RaceGateWireV1, ServerDataPayloadV1, ServerDataV1,
};

/// 从 [`ItemRegistry`] + [`TechniqueRegistry`] 构建种族门元数据表。
///
/// 两表都只收录**非 `Any`** 条目（`Any` 默认，client 缺省即放行）。条目按 id 升序
/// 排序，保证下发 payload 稳定（便于 diff / 测试断言 / 减少无谓字节抖动）。
pub fn build_race_gate_meta(
    registry: &ItemRegistry,
    techniques: &TechniqueRegistry,
) -> RaceGateMetaV1 {
    let mut item_wearer_race: Vec<RaceGateMetaEntryV1> = registry
        .iter_templates()
        .filter(|tpl| !matches!(tpl.wearer_race, crate::body_plan::RaceGateOwned::Any))
        .map(|tpl| RaceGateMetaEntryV1 {
            id: tpl.id.clone(),
            gate: RaceGateWireV1::from_owned(&tpl.wearer_race),
        })
        .collect();
    item_wearer_race.sort_by(|a, b| a.id.cmp(&b.id));

    let mut technique_required_race: Vec<RaceGateMetaEntryV1> = techniques
        .iter()
        .filter(|def| !matches!(def.required_race, crate::body_plan::RaceGateOwned::Any))
        .map(|def| RaceGateMetaEntryV1 {
            id: def.id.clone(),
            gate: RaceGateWireV1::from_owned(&def.required_race),
        })
        .collect();
    technique_required_race.sort_by(|a, b| a.id.cmp(&b.id));

    RaceGateMetaV1 {
        item_wearer_race,
        technique_required_race,
    }
}

/// plan-race-system-v1 P3c — 标记已给该客户端下发过 `RaceGateMeta`（join 首帧一次）。
/// 内容静态，与身份无关，故一旦下发不再重发（组件存在即跳过）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LastSentRaceGateMeta;

type RaceGateMetaEmitQueryItem<'a> = (Entity, &'a mut Client, Option<&'a LastSentRaceGateMeta>);

/// join 首帧向未收到过的客户端下发 `RaceGateMeta`。缺 [`ItemRegistry`] 资源（部分
/// 单测未插入）时跳过 item 表构建但仍下发 technique 表（`build_race_gate_meta` 对空
/// registry 也安全，此处直接 early-return 更省——technique 表来自启动期 registry，
/// 无 registry 时 item 表恒空，行为等价，选择 early-return 保持与 body_plan_layout
/// emit「缺资源优雅跳过」的一致惯例）。
pub fn emit_race_gate_meta_payloads(
    mut commands: Commands,
    registry: Option<Res<ItemRegistry>>,
    techniques: Res<TechniqueRegistry>,
    mut clients: Query<RaceGateMetaEmitQueryItem<'_>, With<Client>>,
) {
    let Some(registry) = registry.as_deref() else {
        return;
    };
    let meta = build_race_gate_meta(registry, &techniques);

    for (entity, mut client, last_sent) in &mut clients {
        if last_sent.is_some() {
            continue;
        }
        let payload = ServerDataV1::new(ServerDataPayloadV1::RaceGateMeta(meta.clone()));
        let payload_type = payload_type_label(payload.payload_type());
        match serialize_server_data_payload(&payload) {
            Ok(bytes) => {
                send_server_data_payload(&mut client, bytes.as_slice());
                commands.entity(entity).insert(LastSentRaceGateMeta);
            }
            Err(error) => log_payload_build_error(payload_type, &error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::RaceGateOwned;

    /// TechniqueRegistry 的当前投影必须完整、排序稳定且不维护一份 Rust 人口计数。
    /// 预期集合从 registry 派生；每条 wire gate 再与 owned metadata 对拍，避免只测数量。
    #[test]
    fn technique_table_projects_current_non_any_registry_entries() {
        let registry = ItemRegistry::from_map(std::collections::HashMap::new());
        let techniques = TechniqueRegistry::load_for_tests();
        let meta = build_race_gate_meta(&registry, &techniques);

        let mut expected = techniques
            .iter()
            .filter(|definition| !matches!(definition.required_race, RaceGateOwned::Any))
            .map(|definition| {
                (
                    definition.id.clone(),
                    RaceGateWireV1::from_owned(&definition.required_race),
                )
            })
            .collect::<Vec<_>>();
        expected.sort_by(|a, b| a.0.cmp(&b.0));

        let actual = meta
            .technique_required_race
            .iter()
            .map(|entry| (entry.id.clone(), entry.gate.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "technique race-gate payload must be the sorted non-Any registry projection"
        );
        assert!(
            actual.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "technique race-gate payload IDs must be strictly sorted"
        );
    }

    /// Any 档功法绝不进表——client 缺省即放行；条目数由当前 registry 派生。
    #[test]
    fn any_techniques_are_absent_from_table() {
        let registry = ItemRegistry::from_map(std::collections::HashMap::new());
        let techniques = TechniqueRegistry::load_for_tests();
        let meta = build_race_gate_meta(&registry, &techniques);

        for definition in techniques
            .iter()
            .filter(|definition| matches!(definition.required_race, RaceGateOwned::Any))
        {
            assert!(
                !meta
                    .technique_required_race
                    .iter()
                    .any(|entry| entry.id == definition.id),
                "Any 档功法 {} 不应出现在种族门表里（Any 是默认，靠 client 缺省放行）",
                definition.id
            );
        }
    }

    /// Species gate 也必须按同一 projection 接线，不能被当前 catalog 的 Humanoid 构成
    /// 假设挡住；这里通过测试覆盖构造未来合法数据扩展的 wire 形状。
    #[test]
    fn technique_species_gate_is_projected_with_wire_species() {
        let registry = ItemRegistry::from_map(std::collections::HashMap::new());
        let techniques =
            TechniqueRegistry::load_for_tests_with_override("movement.dash", |definition| {
                definition.required_race = RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale".to_string())],
                };
            });
        let meta = build_race_gate_meta(&registry, &techniques);

        let entry = meta
            .technique_required_race
            .iter()
            .find(|entry| entry.id == "movement.dash")
            .expect("overridden species technique must enter non-Any table");
        assert_eq!(entry.gate.kind, "species");
        assert_eq!(entry.gate.species, vec!["whale".to_string()]);
    }

    /// item 表当前为空（assets/items/*.toml 全部 wearer_race=Any）——P5 回填数据后
    /// 自动填充，无需再改 client / 本构建函数。
    #[test]
    fn item_table_is_empty_when_all_templates_are_any() {
        // 手工构造两个 Any 模板：表必须为空。
        let mut templates = std::collections::HashMap::new();
        templates.insert("a".to_string(), make_template("a", RaceGateOwned::Any));
        templates.insert("b".to_string(), make_template("b", RaceGateOwned::Any));
        let registry = ItemRegistry::from_map(templates);
        let meta = build_race_gate_meta(&registry, &TechniqueRegistry::load_for_tests());
        assert!(
            meta.item_wearer_race.is_empty(),
            "全 Any 模板目录应产出空 item 表，实际 {:?}",
            meta.item_wearer_race
        );
    }

    /// 非-Any item 模板（Humanoid / Species）必须进表，且 gate wire 精确。
    #[test]
    fn non_any_item_templates_enter_table_with_precise_gate() {
        let mut templates = std::collections::HashMap::new();
        templates.insert(
            "plain".to_string(),
            make_template("plain", RaceGateOwned::Any),
        );
        templates.insert(
            "mask".to_string(),
            make_template("mask", RaceGateOwned::Humanoid),
        );
        templates.insert(
            "fin".to_string(),
            make_template(
                "fin",
                RaceGateOwned::Species {
                    species: vec![crate::body_plan::RaceId::new("whale".to_string())],
                },
            ),
        );
        let registry = ItemRegistry::from_map(templates);
        let meta = build_race_gate_meta(&registry, &TechniqueRegistry::load_for_tests());

        assert_eq!(
            meta.item_wearer_race.len(),
            2,
            "两个非-Any 模板应进表（plain=Any 不进），实际 {:?}",
            meta.item_wearer_race
        );
        // 升序排序：fin < mask。
        assert_eq!(meta.item_wearer_race[0].id, "fin");
        assert_eq!(meta.item_wearer_race[0].gate.kind, "species");
        assert_eq!(
            meta.item_wearer_race[0].gate.species,
            vec!["whale".to_string()]
        );
        assert_eq!(meta.item_wearer_race[1].id, "mask");
        assert_eq!(meta.item_wearer_race[1].gate.kind, "humanoid");
        assert!(meta.item_wearer_race[1].gate.species.is_empty());
    }

    fn make_template(id: &str, wearer_race: RaceGateOwned) -> crate::inventory::ItemTemplate {
        let mut tpl = crate::inventory::ItemTemplate::minimal_for_test(id);
        tpl.wearer_race = wearer_race;
        tpl
    }
}
