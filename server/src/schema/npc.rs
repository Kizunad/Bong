use serde::{Deserialize, Serialize};

use crate::npc::faction::{FactionStatus, GroupStatus};
use crate::npc::war::WarPhase;

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

/// plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry（`bong:faction_state`，reframe b）。
///
/// **纯观测 payload**——末法残土无具名宗门，离屏散修在某 zone 自发聚成匿名涌现集体；本结构记录
/// 一个涌现群体周期性的人口盘面：匿名 `group_id`（裸数字，无「青云猎盟」式专名）+ `region_descriptor`
/// （`"{zone}一带散修"` 式区域描述符）+ `population`（存活成员计数）+ `status`（消长三态）+ 该群体
/// 当前的**涌现强者**（`strongest_*`，最高境界活体；Q2 派生焦点，**无号令权**——不是宗主 / 掌门，
/// 只是恰好境界最高的散修）。
///
/// **守恒红线（§10.1 #5）**：census 全只读 dormant store + faction store，不触碰 `WorldQiAccount`
/// / ledger；强者陨落不在此特殊处理真元（仍走 P2 的 `release_dormant_qi_to_zone`）。本结构与
/// `DormantCombatOutcomeV1` 同为观测旁路，绝不携带也不触发任何真元流动。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionStateV1 {
    pub v: u8,
    pub kind: String,
    /// 匿名涌现群体 id（== `EmergentGroupId.0`，裸数字，无具名宗门）。
    pub group_id: u16,
    /// 区域描述符（`"{zone}一带散修"`，dominant_zone 派生）——群体的可读匿名标识。
    pub region_descriptor: String,
    /// 该群体存活成员数（不含已离屏战死待释放的 `combat_dead_pending_release` 快照）。
    pub population: u32,
    /// 消长三态：相对上轮 census 的人口变化（rising / stable / waning）。
    pub status: GroupStatus,
    /// 该群体成员最集中的 zone（众数；平票取字典序最小，确定性）。
    pub dominant_zone: String,
    /// 涌现强者境界（最高 realm 活体，`realm_to_string` 形式，如 `"Solidify"`）。
    pub strongest_realm: String,
    /// 涌现强者 char_id（最高 realm 活体；平局取 char_id 字典序最小）。
    pub strongest_char_id: String,
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

/// plan-offscreen-war-v1 P6：涌现区域冲突生命周期 telemetry（`bong:faction/war`，reframe b）。
///
/// **纯观测、零真元**——末法残土无宣战 / 无具名宗门。离屏 dormant 群体在某 zone 累积互殴越阈值
/// → 自发升级成「战事」（Emerging→Skirmish→Settling→Aftermath）。本结构记录一场涌现冲突的
/// war_id / zone / 匿名区域描述符 / 当前阶段 / 关联裸 group_id / 玩家立场计数 / 结算。
/// 守恒红线：本 payload **不含任何真元字段**；真元流动仍唯一走 P2 release_dormant_qi_to_zone。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionWarEventV1 {
    pub v: u8,
    pub kind: String,
    /// war 唯一 id（单调递增，per zone 重置）。
    pub war_id: u64,
    pub zone: String,
    /// `"{zone}一带散修"`——禁具名宗门。
    pub region_descriptor: String,
    /// 涌现冲突阶段（snake_case：emerging / skirmish / settling / aftermath）。
    pub phase: WarPhase,
    /// 参与群体裸 id（去重升序，无专名）。
    pub groups: Vec<u16>,
    /// 投靠玩家计数。
    pub enlist_count: u32,
    /// 佣兵玩家计数。
    pub mercenary_count: u32,
    /// 截胡玩家计数。
    pub intercept_count: u32,
    /// 旁观玩家计数。
    pub spectate_count: u32,
    /// 胜方 group_id（Settling/Aftermath 才 Some）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub winner_group: Option<u16>,
    /// 败方 group_id（Settling/Aftermath 才 Some）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub loser_group: Option<u16>,
    /// 累积战死计数（Settling/Aftermath 才 Some）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total_casualties: Option<u32>,
    pub at_tick: u64,
}

// ── plan-faction-expansion-v1 P0：具名势力状态 schema ────────────────────────────
//
// 命名避撞：既有 FactionStateV1（schema/npc.rs:100，emergent group census）+
// bong:faction_state（CH_FACTION_STATE，plan-offscreen-war-v1 P5 正被使用）。
// 本 plan 专用 NamedFactionStateV1 + bong:named_faction_state，绝不复用既有名。
//
// TS source of truth：agent/packages/schema/src/npc.ts（NamedFactionStateV1）。
// 字段对齐 TypeBox additionalProperties:false ↔ serde 默认拒未知（无 deny_unknown_fields
// 宏但 TypeBox 侧已有反样本守卫）。

/// plan-faction-expansion-v1 P0：具名势力注册表条目 schema（`bong:named_faction_state`）。
///
/// id 序列化为 snake_case（如 "qingyun_hunters"，对齐 NamedFactionId::as_str()）。
/// status 复用 npc::faction::FactionStatus（snake_case：active/headless/decayed）。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NamedFactionEntryV1 {
    /// 具名势力 id（snake_case，如 "qingyun_hunters"）。
    pub id: String,
    pub display_name: String,
    /// zone 锚点字符串（对齐 world/zone.rs 体系，如 "qingyun_peaks"）。
    pub zone_anchor: String,
    pub current_npc_count: u32,
    /// 领袖存活态（active / headless / decayed）。
    pub status: FactionStatus,
}

/// plan-faction-expansion-v1 P0：两具名势力关系矩阵条目（`bong:named_faction_state` relation_matrix）。
///
/// hostile 由 FactionStore::faction_id_for_war→is_hostile_pair 派生（P0 兼容层）。
/// P1 faction-wars 接入后可扩展 relation_kind 字段；P0 只留 bool。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactionRelationEntryV1 {
    /// 势力 a id（snake_case）。
    pub a: String,
    /// 势力 b id（snake_case）。
    pub b: String,
    /// true=敌对，由 faction_id_for_war→is_hostile_pair 派生。
    pub hostile: bool,
}

/// plan-faction-expansion-v1 P0：具名势力注册表快照（`bong:named_faction_state`）。
///
/// 与 FactionStateV1（emergent group census，bong:faction_state）完全独立——
/// 不同 kind（"named_faction_state"），不同 channel（bong:named_faction_state）。
///
/// P0 publish stub：publish_named_faction_state system 真发一帧到 CH_NAMED_FACTION_STATE
/// （参见 schema/channels.rs）。下游消费契约：
/// - social-v2 WarReputation 按 (NamedFactionId, NamedFactionId) 累积
/// - faction-wars FactionWarEventV1 携 NamedFactionId 发起/防守方
///
/// P1: faction-wars consumes NamedFactionId via faction_id_for_war
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NamedFactionStateV1 {
    pub v: u8,
    pub kind: String,
    pub named_factions: Vec<NamedFactionEntryV1>,
    pub relation_matrix: Vec<FactionRelationEntryV1>,
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

    fn sample_pending_relic() -> PendingDormantRelicV1 {
        PendingDormantRelicV1 {
            v: 1,
            kind: "pending_dormant_relic".to_string(),
            char_id: "dormant:fallen:disciple".to_string(),
            zone: "rift_valley".to_string(),
            pos: [12.0, 64.0, -8.0],
            archetype: "disciple".to_string(),
            loot_seed: 0xFFFF_FFFF_0000_0001,
            created_tick: 42,
            at_tick: 4321,
        }
    }

    #[test]
    fn pending_dormant_relic_v1_roundtrips() {
        // plan-offscreen-war-v1 P3（CodeRabbit）：战场遗物 telemetry wire 必须无损 roundtrip——
        // e2e 据此把 char_id 与 bong:npc/death 对账、读 zone/pos/loot_seed 校验物化契约。
        // loot_seed 取含 high-bit 的 u64 边界值，确认 u64 不被截断。
        let payload = sample_pending_relic();
        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: PendingDormantRelicV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed, payload,
            "PendingDormantRelicV1 must round-trip losslessly, otherwise external relic observation reads wrong values"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["char_id"],
            serde_json::json!("dormant:fallen:disciple"),
            "char_id must serialize as the fallen NPC's char_id string (== matching NpcDeathV1.npc_id); got {}",
            value["char_id"]
        );
        assert_eq!(
            value["pos"],
            serde_json::json!([12.0, 64.0, -8.0]),
            "pos must serialize as a [f64;3] array (hydrate spawns loot there); got {}",
            value["pos"]
        );
        assert_eq!(
            value["loot_seed"].as_u64(),
            Some(0xFFFF_FFFF_0000_0001),
            "loot_seed must serialize as a lossless u64 including the high bit (deterministic loot depends on it); got {}",
            value["loot_seed"]
        );
    }

    #[test]
    fn pending_dormant_relic_v1_rejects_pos_with_wrong_arity() {
        // 反：pos 元数错（2 元而非 [f64;3]）必须解析失败——锁住落点 3 元契约。
        let mut bad = serde_json::to_value(sample_pending_relic()).unwrap();
        bad["pos"] = serde_json::json!([12.0, 64.0]);
        let parsed = serde_json::from_value::<PendingDormantRelicV1>(bad);
        assert!(
            parsed.is_err(),
            "a 2-element pos must be rejected because pos is a fixed [f64;3] landing point; serde accepted it: {parsed:?}"
        );
    }

    #[test]
    fn pending_dormant_relic_v1_rejects_non_string_char_id() {
        // 反：char_id 类型错（number 非 string）必须解析失败——锁住 char_id 字符串契约。
        let mut bad = serde_json::to_value(sample_pending_relic()).unwrap();
        bad["char_id"] = serde_json::json!(7);
        let parsed = serde_json::from_value::<PendingDormantRelicV1>(bad);
        assert!(
            parsed.is_err(),
            "a numeric char_id must be rejected (the field is a String char_id == NpcDeathV1.npc_id); serde accepted it: {parsed:?}"
        );
    }

    #[test]
    fn pending_dormant_relic_v1_rejects_non_u64_loot_seed() {
        // 反：loot_seed 类型错（string 非 u64）必须解析失败——锁住 deterministic 种子的 u64 契约。
        let mut bad = serde_json::to_value(sample_pending_relic()).unwrap();
        bad["loot_seed"] = serde_json::json!("not-a-number");
        let parsed = serde_json::from_value::<PendingDormantRelicV1>(bad);
        assert!(
            parsed.is_err(),
            "a string loot_seed must be rejected because the field is a u64 deterministic seed; serde accepted it: {parsed:?}"
        );
    }

    #[test]
    fn pending_dormant_relic_v1_rejects_non_u64_created_tick() {
        // 反：created_tick 类型错（负数无法解析成 u64）必须解析失败——锁住结算 tick 的 u64 契约。
        let mut bad = serde_json::to_value(sample_pending_relic()).unwrap();
        bad["created_tick"] = serde_json::json!(-1);
        let parsed = serde_json::from_value::<PendingDormantRelicV1>(bad);
        assert!(
            parsed.is_err(),
            "a negative created_tick must be rejected because the field is a u64 settlement tick; serde accepted it: {parsed:?}"
        );
    }

    fn sample_faction_state() -> FactionStateV1 {
        FactionStateV1 {
            v: 1,
            kind: "faction_state".to_string(),
            group_id: 2,
            region_descriptor: "rift_valley一带散修".to_string(),
            population: 7,
            status: GroupStatus::Rising,
            dominant_zone: "rift_valley".to_string(),
            strongest_realm: "Solidify".to_string(),
            strongest_char_id: "dormant:rogue:3".to_string(),
            at_tick: 4321,
        }
    }

    #[test]
    fn faction_state_v1_roundtrips() {
        // plan-offscreen-war-v1 P5：散修群体消长盘面 telemetry wire 必须无损 roundtrip——
        // e2e / 调试脚本据此读 group_id / population / status / strongest_* 观测群体此消彼长。
        // status 取 Rising 这条变体，确认 GroupStatus 嵌入字段也无损 roundtrip。
        let payload = sample_faction_state();
        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: FactionStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed, payload,
            "FactionStateV1 must round-trip losslessly, otherwise群体消长观测会读到错值"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            value["group_id"].as_u64(),
            Some(2),
            "group_id must serialize as a bare u16 number (anonymous emergent group id, no named sect); got {}",
            value["group_id"]
        );
        assert_eq!(
            value["status"],
            serde_json::json!("rising"),
            "status must serialize as the snake_case GroupStatus string (rising/stable/waning); got {}",
            value["status"]
        );
        assert_eq!(
            value["region_descriptor"],
            serde_json::json!("rift_valley一带散修"),
            "region_descriptor must serialize as the anonymous \"{{zone}}一带散修\" descriptor (no named sect); got {}",
            value["region_descriptor"]
        );
        assert_eq!(
            value["strongest_realm"],
            serde_json::json!("Solidify"),
            "strongest_realm must serialize as the realm_to_string label of the emergent strongest (highest-realm living member); got {}",
            value["strongest_realm"]
        );
    }

    #[test]
    fn faction_state_v1_each_status_variant_roundtrips() {
        // 状态转换饱和：三个 GroupStatus 变体各一条专属 case，确认嵌入 FactionStateV1 后
        // 每个变体都无损 roundtrip（不仅 Rising，Stable / Waning 同样锁住）。
        for (status, wire) in [
            (GroupStatus::Rising, "rising"),
            (GroupStatus::Stable, "stable"),
            (GroupStatus::Waning, "waning"),
        ] {
            let mut payload = sample_faction_state();
            payload.status = status;
            let json = serde_json::to_string(&payload).expect("serialize");
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(
                value["status"],
                serde_json::json!(wire),
                "FactionStateV1 with status {status:?} must serialize status to {wire:?}, got {}",
                value["status"]
            );
            let parsed: FactionStateV1 = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(
                parsed.status, status,
                "FactionStateV1 status must round-trip back to {status:?}, got {:?}",
                parsed.status
            );
        }
    }

    #[test]
    fn faction_state_v1_rejects_unknown_status() {
        // 反：非法 status 字符串（不属于 rising/stable/waning）必须解析失败——锁住 GroupStatus
        // 的 enum 契约，agent 端 TypeBox 同步拒。
        let mut bad = serde_json::to_value(sample_faction_state()).unwrap();
        bad["status"] = serde_json::json!("ascending");
        let parsed = serde_json::from_value::<FactionStateV1>(bad);
        assert!(
            parsed.is_err(),
            "an unknown status string \"ascending\" must be rejected because status is a GroupStatus \
             enum (rising/stable/waning only); serde accepted it: {parsed:?}"
        );
    }

    #[test]
    fn faction_state_v1_rejects_missing_field() {
        // 反：缺必填字段（这里删掉 population）必须解析失败——FactionStateV1 无 serde default，
        // 每个字段都是观测契约的一部分，缺字段不能静默归零。
        let mut bad = serde_json::to_value(sample_faction_state()).unwrap();
        bad.as_object_mut().unwrap().remove("population");
        let parsed = serde_json::from_value::<FactionStateV1>(bad);
        assert!(
            parsed.is_err(),
            "a payload missing the required `population` field must be rejected (no serde default); \
             serde accepted it: {parsed:?}"
        );
    }

    #[test]
    fn faction_state_v1_rejects_non_u16_group_id() {
        // 反：group_id 超出 u16 范围（这里 70000 > u16::MAX）必须解析失败——锁住 group_id 的
        // u16 契约（== EmergentGroupId.0），防 wire 端塞进越界 id。
        let mut bad = serde_json::to_value(sample_faction_state()).unwrap();
        bad["group_id"] = serde_json::json!(70000);
        let parsed = serde_json::from_value::<FactionStateV1>(bad);
        assert!(
            parsed.is_err(),
            "a group_id of 70000 must be rejected because the field is a u16 (== EmergentGroupId.0, \
             max 65535); serde accepted it: {parsed:?}"
        );
    }

    // ─────────────── FactionWarEventV1 (plan-offscreen-war-v1 P6) ───────────────

    fn sample_faction_war(phase: WarPhase, with_outcome: bool) -> FactionWarEventV1 {
        FactionWarEventV1 {
            v: 1,
            kind: "faction_war_event".to_string(),
            war_id: 7,
            zone: "残灰谷".to_string(),
            region_descriptor: "残灰谷一带散修".to_string(),
            phase,
            groups: vec![0, 1],
            enlist_count: 2,
            mercenary_count: 1,
            intercept_count: 0,
            spectate_count: 3,
            winner_group: if with_outcome { Some(0) } else { None },
            loser_group: if with_outcome { Some(1) } else { None },
            total_casualties: if with_outcome { Some(6) } else { None },
            at_tick: 999,
        }
    }

    #[test]
    fn faction_war_v1_roundtrips() {
        // plan-offscreen-war-v1 P6：FactionWarEventV1 含 outcome 字段的完整无损 roundtrip。
        let payload = sample_faction_war(WarPhase::Settling, true);
        let json = serde_json::to_string(&payload).expect("serialize");
        let parsed: FactionWarEventV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            parsed, payload,
            "FactionWarEventV1 must roundtrip losslessly"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["v"], serde_json::json!(1));
        assert_eq!(value["kind"], serde_json::json!("faction_war_event"));
        assert_eq!(value["war_id"], serde_json::json!(7));
        assert_eq!(value["zone"], serde_json::json!("残灰谷"));
        assert_eq!(
            value["region_descriptor"],
            serde_json::json!("残灰谷一带散修")
        );
        assert_eq!(value["phase"], serde_json::json!("settling"));
        assert_eq!(value["groups"], serde_json::json!([0, 1]));
        assert_eq!(value["enlist_count"], serde_json::json!(2));
        assert_eq!(value["mercenary_count"], serde_json::json!(1));
        assert_eq!(value["winner_group"], serde_json::json!(0));
        assert_eq!(value["loser_group"], serde_json::json!(1));
        assert_eq!(value["total_casualties"], serde_json::json!(6));
    }

    #[test]
    fn faction_war_v1_each_phase_variant_roundtrips() {
        // plan-offscreen-war-v1 P6：四 WarPhase 变体各一条正向 roundtrip。
        for (phase, name) in [
            (WarPhase::Emerging, "emerging"),
            (WarPhase::Skirmish, "skirmish"),
            (WarPhase::Settling, "settling"),
            (WarPhase::Aftermath, "aftermath"),
        ] {
            let payload = sample_faction_war(
                phase,
                matches!(phase, WarPhase::Settling | WarPhase::Aftermath),
            );
            let json = serde_json::to_string(&payload).unwrap();
            let parsed: FactionWarEventV1 = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.phase, phase, "phase={name} must survive roundtrip");

            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(
                value["phase"],
                serde_json::json!(name),
                "期望 phase={name} 序列化为 snake_case 字符串，实际 {}",
                value["phase"]
            );
        }
    }

    #[test]
    fn faction_war_v1_outcome_none_omits_fields() {
        // plan-offscreen-war-v1 P6：outcome=None 时 winner/loser/casualties 不出现在 wire。
        let payload = sample_faction_war(WarPhase::Emerging, false);
        let value = serde_json::to_value(&payload).unwrap();
        assert!(
            value.get("winner_group").is_none(),
            "期望 winner_group=None 时不出现在 wire JSON（skip_serializing_if），实际 key 存在"
        );
        assert!(
            value.get("loser_group").is_none(),
            "期望 loser_group=None 时不出现在 wire JSON（skip_serializing_if），实际 key 存在"
        );
        assert!(
            value.get("total_casualties").is_none(),
            "期望 total_casualties=None 时不出现在 wire JSON（skip_serializing_if），实际 key 存在"
        );
    }

    #[test]
    fn faction_war_v1_rejects_non_u16_group_id() {
        // 反：groups 数组中含 > u16::MAX 的值，serde 应拒绝（Vec<u16> 不接受越界 number）。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "faction_war_event",
            "war_id": 7,
            "zone": "残灰谷",
            "region_descriptor": "残灰谷一带散修",
            "phase": "emerging",
            "groups": [0, 70000],
            "enlist_count": 0,
            "mercenary_count": 0,
            "intercept_count": 0,
            "spectate_count": 0,
            "at_tick": 1
        });
        let parsed = serde_json::from_value::<FactionWarEventV1>(bad);
        assert!(
            parsed.is_err(),
            "期望 group_id=70000 > u16::MAX 被 serde 拒绝（因 groups 是 Vec<u16>，裸匿名 id 上限 65535），\
             实际解析成功: {parsed:?}"
        );
    }

    #[test]
    fn faction_war_v1_rejects_missing_war_id() {
        // 反：缺必填字段 war_id 时必须解析失败。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "faction_war_event",
            "zone": "残灰谷",
            "region_descriptor": "残灰谷一带散修",
            "phase": "emerging",
            "groups": [0, 1],
            "enlist_count": 0,
            "mercenary_count": 0,
            "intercept_count": 0,
            "spectate_count": 0,
            "at_tick": 1
        });
        let parsed = serde_json::from_value::<FactionWarEventV1>(bad);
        assert!(
            parsed.is_err(),
            "期望缺 war_id 字段时 serde 报错（必填），实际解析成功: {parsed:?}"
        );
    }

    // ─── plan-faction-expansion-v1 P0：NamedFactionStateV1 schema 双端 ─────────

    fn sample_named_faction_state() -> NamedFactionStateV1 {
        NamedFactionStateV1 {
            v: 1,
            kind: "named_faction_state".to_string(),
            named_factions: vec![
                NamedFactionEntryV1 {
                    id: "qingyun_hunters".to_string(),
                    display_name: "青云猎盟".to_string(),
                    zone_anchor: "qingyun_peaks".to_string(),
                    current_npc_count: 0,
                    status: FactionStatus::Active,
                },
                NamedFactionEntryV1 {
                    id: "cangyuan_merchants".to_string(),
                    display_name: "沧渊商会".to_string(),
                    zone_anchor: "blood_valley".to_string(),
                    current_npc_count: 0,
                    status: FactionStatus::Active,
                },
                NamedFactionEntryV1 {
                    id: "north_waste_drifters".to_string(),
                    display_name: "北荒漂流者".to_string(),
                    zone_anchor: "north_wastes".to_string(),
                    current_npc_count: 0,
                    status: FactionStatus::Headless,
                },
            ],
            relation_matrix: vec![FactionRelationEntryV1 {
                a: "qingyun_hunters".to_string(),
                b: "cangyuan_merchants".to_string(),
                hostile: true,
            }],
            at_tick: 100,
        }
    }

    #[test]
    fn test_named_faction_state_v1_roundtrip() {
        // NamedFactionStateV1（含 Headless 北荒）serde_json 往返无损 + status 序列化为 snake_case。
        let payload = sample_named_faction_state();
        let json = serde_json::to_string(&payload).expect("NamedFactionStateV1 序列化必须成功");
        let parsed: NamedFactionStateV1 =
            serde_json::from_str(&json).expect("NamedFactionStateV1 反序列化必须成功");
        assert_eq!(
            parsed, payload,
            "NamedFactionStateV1 必须无损 roundtrip（含 Headless 北荒 status）"
        );

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["v"], serde_json::json!(1), "v 字段必须序列化为 1");
        assert_eq!(
            value["kind"],
            serde_json::json!("named_faction_state"),
            "kind 字段必须是 \"named_faction_state\"（与既有 FactionStateV1 的 \"faction_state\" 区分）"
        );
        // 北荒漂流者 status 序列化为 snake_case "headless"。
        let north_status = &value["named_factions"][2]["status"];
        assert_eq!(
            *north_status,
            serde_json::json!("headless"),
            "NorthWasteDrifters status 必须序列化为 \"headless\"（FactionStatus snake_case），实际 {north_status}"
        );
    }

    #[test]
    fn test_named_faction_state_v1_invalid_status_rejected() {
        // 反：非法 status 串（"alive"）反序列化失败——锁住 FactionStatus 三变体契约。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "named_faction_state",
            "named_factions": [{
                "id": "qingyun_hunters",
                "display_name": "青云猎盟",
                "zone_anchor": "qingyun_peaks",
                "current_npc_count": 0,
                "status": "alive"
            }],
            "relation_matrix": [],
            "at_tick": 1
        });
        let parsed = serde_json::from_value::<NamedFactionStateV1>(bad);
        assert!(
            parsed.is_err(),
            "非法 status 字符串 \"alive\" 必须被 serde 拒绝（FactionStatus 只有 active/headless/decayed），\
             实际解析成功: {parsed:?}"
        );
    }

    #[test]
    fn test_named_faction_state_v1_missing_field_rejected() {
        // 反：缺必填字段（at_tick）反序列化失败（无 serde default）。
        let bad = serde_json::json!({
            "v": 1,
            "kind": "named_faction_state",
            "named_factions": [],
            "relation_matrix": []
        });
        let parsed = serde_json::from_value::<NamedFactionStateV1>(bad);
        assert!(
            parsed.is_err(),
            "缺必填 at_tick 字段必须被 serde 拒绝（无 serde default），实际解析成功: {parsed:?}"
        );
    }

    #[test]
    fn test_named_faction_state_v1_faction_status_three_variants() {
        // FactionStatus 三变体（active/headless/decayed）各有专属 pin 测试，嵌入 NamedFactionEntryV1。
        for (status, wire) in [
            (FactionStatus::Active, "active"),
            (FactionStatus::Headless, "headless"),
            (FactionStatus::Decayed, "decayed"),
        ] {
            let entry = NamedFactionEntryV1 {
                id: "qingyun_hunters".to_string(),
                display_name: "青云猎盟".to_string(),
                zone_anchor: "qingyun_peaks".to_string(),
                current_npc_count: 0,
                status,
            };
            let value = serde_json::to_value(&entry).expect("NamedFactionEntryV1 序列化");
            assert_eq!(
                value["status"],
                serde_json::json!(wire),
                "FactionStatus::{status:?} 嵌入 NamedFactionEntryV1 必须序列化为 {wire:?}，实际 {}",
                value["status"]
            );
            let back: NamedFactionEntryV1 =
                serde_json::from_value(value).expect("NamedFactionEntryV1 反序列化");
            assert_eq!(
                back.status, status,
                "FactionStatus::{status:?} 必须 roundtrip 还原，实际 {:?}",
                back.status
            );
        }
    }
}
