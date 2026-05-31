use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcSpawnedV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub source: String,
    pub zone: String,
    pub pos: [f64; 3],
    pub initial_age_ticks: f64,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NpcDeathV1 {
    pub v: u8,
    pub kind: String,
    pub npc_id: String,
    pub archetype: String,
    pub cause: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub life_record_snapshot: Option<String>,
    pub age_ticks: f64,
    pub max_age_ticks: f64,
    pub at_tick: u64,
    /// plan-offscreen-war-v1 P0：是否离屏 dormant 派系互殴所致。
    /// serde default=false 让旧 payload（无此字段）仍可反序列化，向后兼容。
    #[serde(default)]
    pub from_dormant_combat: bool,
    /// plan-offscreen-war-v1 P0：死亡坐标 [x,y,z]，无则 None 不上线。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pos: Option<[f64; 3]>,
}

/// plan-offscreen-war-v1 P2：离屏 dormant 派系互殴战果 telemetry（`bong:npc/combat`）。
///
/// **纯观测 payload**——记录一场离屏战死的胜者 / 败者 / 所在 zone / 守恒回灌给 zone 的
/// 真元量。真元流动本身走 `release_dormant_qi_to_zone` → `ledger.transfer`，本结构不参与
/// 任何 balance 变动；外部 e2e 用它把战果与 `bong:npc/death` 对账（loser == death.npc_id）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DormantCombatOutcomeV1 {
    pub v: u8,
    pub kind: String,
    /// 胜者 char_id（真元不变，dormant 简化未流动即未失衡）。
    pub winner: String,
    /// 败者 char_id（== 对应 `NpcDeathV1.npc_id`，cause=combat）。
    pub loser: String,
    /// 战斗发生的 zone 名（败者残余真元守恒回灌此 zone）。
    pub zone: String,
    /// 本场战死实际守恒回灌给 zone 的真元量（== `release_dormant_qi_to_zone` 的
    /// `transfer.amount`；zone 满则可能 < 败者全部真元，余量本轮留败者账户等下轮重试）。
    pub qi_released: f64,
    pub at_tick: u64,
}

/// plan-offscreen-war-v1 P3：克制式战场遗物创建 telemetry（`bong:npc/relic`）。
///
/// **纯观测 payload**——一名克制判定通过的离屏战死者在战场留下了一处待物化遗物（已落盘进
/// sqlite `pending_dormant_relics`）。**零真元**：遗物不携带任何真元（loot 物化时 spirit_quality=0），
/// 本结构同样不含真元字段。真服 e2e 据此 headless 断言"知名战死 → 遗物创建"（不便直接读 sqlite
/// 时的 redis 可观测面，§11）。`char_id` == 对应 `NpcDeathV1.npc_id`（cause=combat）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PendingDormantRelicV1 {
    pub v: u8,
    pub kind: String,
    /// 战死者 char_id（== 对应 `NpcDeathV1.npc_id`）。
    pub char_id: String,
    /// 遗物所在 zone（玩家靠近此 zone 时物化成地面 loot）。
    pub zone: String,
    /// 遗物落点 [x,y,z]。
    pub pos: [f64; 3],
    /// 战死者 archetype（as_str()）——hydrate 时按它 roll loot 表。
    pub archetype: String,
    /// deterministic loot 种子（hydrate 用它复现 loot）。
    pub loot_seed: u64,
    /// 逻辑结算 tick（deferred-on-hydrate 时序校验用）。
    pub created_tick: u64,
    pub at_tick: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionEventV1 {
    pub v: u8,
    pub kind: String,
    pub faction_id: String,
    pub event_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader_id: Option<String>,
    pub loyalty_bias: f64,
    pub mission_queue_size: u32,
    pub at_tick: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_death_omits_absent_optional_fields() {
        let payload = NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "npc_1v1".to_string(),
            archetype: "commoner".to_string(),
            cause: "natural_aging".to_string(),
            faction_id: None,
            life_record_snapshot: None,
            age_ticks: 10.0,
            max_age_ticks: 20.0,
            at_tick: 3,
            from_dormant_combat: false,
            pos: None,
        };

        let value = serde_json::to_value(payload).expect("serialize");
        assert!(value.get("faction_id").is_none());
        assert!(value.get("life_record_snapshot").is_none());
        // pos 为 None 时不上线（skip_serializing_if）。
        assert!(
            value.get("pos").is_none(),
            "pos=None 应被 skip，不出现在 wire JSON"
        );
        // from_dormant_combat 是 bool（非 Option），始终上线，默认场景为 false。
        assert_eq!(
            value.get("from_dormant_combat"),
            Some(&serde_json::json!(false)),
            "from_dormant_combat 始终序列化，默认 false"
        );
    }

    #[test]
    fn npc_death_v1_roundtrip_with_from_dormant_combat() {
        // plan-offscreen-war-v1 P0：离屏 dormant 互殴死亡 wire 必须完整 roundtrip
        // 新字段（from_dormant_combat=true + pos），让 agent 能区分战死 vs 老死。
        let payload = NpcDeathV1 {
            v: 1,
            kind: "npc_death".to_string(),
            npc_id: "dormant:rogue:7".to_string(),
            archetype: "rogue".to_string(),
            cause: "combat".to_string(),
            faction_id: Some("attack".to_string()),
            life_record_snapshot: Some("残灰谷争脉".to_string()),
            age_ticks: 50.0,
            max_age_ticks: 200.0,
            at_tick: 1234,
            from_dormant_combat: true,
            pos: Some([12.5, 64.0, -8.0]),
        };

        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: NpcDeathV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, payload, "新字段必须无损 roundtrip");

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["from_dormant_combat"],
            serde_json::json!(true),
            "from_dormant_combat 应序列化为 wire bool true（因为本 payload 是离屏战死），实际 {}",
            value["from_dormant_combat"]
        );
        assert_eq!(
            value["pos"],
            serde_json::json!([12.5, 64.0, -8.0]),
            "pos 应序列化为 3 元 [x,y,z] 数组（agent 据此定位战场），实际 {}",
            value["pos"]
        );
        assert_eq!(
            value["cause"],
            serde_json::json!("combat"),
            "cause 应为 \"combat\"（区别于 natural_aging，让 agent 分辨战死 vs 老死），实际 {}",
            value["cause"]
        );
    }

    #[test]
    fn npc_death_v1_rejects_non_bool_from_dormant_combat() {
        // 反：from_dormant_combat 类型错（string 非 bool）必须解析失败。
        // serde 对 bool 字段拒绝错误类型——锁住 wire 类型契约，agent 端 TypeBox 同步拒。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "npc_death",
            "npc_id": "dormant:rogue:9",
            "archetype": "rogue",
            "cause": "combat",
            "age_ticks": 50.0,
            "max_age_ticks": 200.0,
            "at_tick": 1234,
            "from_dormant_combat": "yes"
        });
        let parsed = serde_json::from_value::<NpcDeathV1>(bad);
        assert!(
            parsed.is_err(),
            "from_dormant_combat 为 string \"yes\" 应被 serde 拒绝（字段是 bool），\
             实际解析成功得到 {parsed:?}"
        );
    }

    #[test]
    fn npc_death_v1_rejects_pos_with_wrong_arity() {
        // 反：pos 非 3 元数组（这里 2 元，缺 z）必须解析失败。
        // [f64; 3] 是定长 tuple，serde 对元数不符拒绝——锁住坐标形状契约。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "npc_death",
            "npc_id": "dormant:rogue:9",
            "archetype": "rogue",
            "cause": "combat",
            "age_ticks": 50.0,
            "max_age_ticks": 200.0,
            "at_tick": 1234,
            "from_dormant_combat": true,
            "pos": [12.5, 64.0]
        });
        let parsed = serde_json::from_value::<NpcDeathV1>(bad);
        assert!(
            parsed.is_err(),
            "pos 为 2 元数组（缺 z）应被 serde 拒绝（pos 是定长 [f64; 3]），\
             实际解析成功得到 {parsed:?}"
        );
    }

    #[test]
    fn npc_death_v1_deserializes_legacy_payload_without_new_fields() {
        // 向后兼容：旧 server 发的、不带 from_dormant_combat/pos 的 payload 仍能解析。
        // serde default 让 from_dormant_combat 回退 false、pos 回退 None。
        let legacy = serde_json::json!({
            "v": 1,
            "kind": "npc_death",
            "npc_id": "npc_old",
            "archetype": "commoner",
            "cause": "natural_aging",
            "age_ticks": 10.0,
            "max_age_ticks": 20.0,
            "at_tick": 3
        });
        let parsed: NpcDeathV1 =
            serde_json::from_value(legacy).expect("legacy payload without new fields must parse");
        assert!(
            !parsed.from_dormant_combat,
            "缺字段时 from_dormant_combat 必须 default 为 false（向后兼容）"
        );
        assert_eq!(parsed.pos, None, "缺字段时 pos 必须 default 为 None");
    }

    #[test]
    fn dormant_combat_outcome_v1_roundtrips() {
        // plan-offscreen-war-v1 P2：离屏战果 telemetry wire 必须无损 roundtrip，
        // 外部 e2e 据此把 loser 与 bong:npc/death 对账、读 qi_released 校验还灵气。
        let payload = DormantCombatOutcomeV1 {
            v: 1,
            kind: "dormant_combat_outcome".to_string(),
            winner: "dormant:rogue:3".to_string(),
            loser: "dormant:rogue:7".to_string(),
            zone: "spawn".to_string(),
            qi_released: 0.4,
            at_tick: 1234,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: DormantCombatOutcomeV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed, payload,
            "DormantCombatOutcomeV1 必须无损 roundtrip，否则外部观测对账会读到错值"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["loser"],
            serde_json::json!("dormant:rogue:7"),
            "loser 必须序列化为 char_id 字符串（== 对应 NpcDeathV1.npc_id），实际 {}",
            value["loser"]
        );
        assert_eq!(
            value["winner"],
            serde_json::json!("dormant:rogue:3"),
            "winner 必须序列化为 char_id 字符串，实际 {}",
            value["winner"]
        );
        assert_eq!(
            value["qi_released"],
            serde_json::json!(0.4),
            "qi_released 必须序列化为守恒回灌量 f64（外部据此校验 zone spirit_qi 上升），实际 {}",
            value["qi_released"]
        );
    }

    #[test]
    fn dormant_combat_outcome_v1_rejects_non_string_loser() {
        // 反：loser 类型错（number 非 string）必须解析失败——锁住 char_id 字符串契约。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "dormant_combat_outcome",
            "winner": "dormant:rogue:3",
            "loser": 7,
            "zone": "spawn",
            "qi_released": 0.4,
            "at_tick": 1234
        });
        let parsed = serde_json::from_value::<DormantCombatOutcomeV1>(bad);
        assert!(
            parsed.is_err(),
            "loser 为 number 7 应被 serde 拒绝（字段是 String char_id），实际解析成功得到 {parsed:?}"
        );
    }
}
