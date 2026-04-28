//! plan-HUD-v1 §2.1 / §11.1 客户端 `CombatHudState` 推送 schema。
//!
//! 三个 percent + DerivedAttrFlags（§3.3 飞行/虚化/渡劫锁定）。
//! 服务端按 `Changed<Cultivation> | Changed<Stamina>` 节流推送。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedAttrFlagsV1 {
    pub flying: bool,
    pub phasing: bool,
    pub tribulation_locked: bool,
}

/// plan-HUD-v1 §2.1 mini body 伤口红点的数据源。
/// `WoundsSnapshotHandler` 客户端期望字段：part / kind / severity / state / infection / scar / updated_at_ms。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WoundEntryV1 {
    pub part: String,
    pub kind: String,
    pub severity: f32,
    /// "stable" | "bleeding" | "healing" | "scarred"
    pub state: String,
    pub infection: f32,
    pub scar: bool,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WoundsSnapshotV1 {
    pub wounds: Vec<WoundEntryV1>,
}

/// plan-HUD-v1 §3.2 截脉弹反窗口推送（屏幕中心红环收缩）。
/// 当前 Bong server 走「反应模式」(C) ——玩家按 V → 立即开 200ms 窗口 →
/// server push 这个 payload，client 渲染红环 + 期间内被打中算弹反成功。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefenseWindowV1 {
    pub duration_ms: u32,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

/// plan-HUD-v1 §4 cast 状态机推送。
/// `phase` 与客户端 `CastState.Phase` 1:1：idle / casting / complete / interrupt。
/// `outcome` 仅 interrupt/complete 有意义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastPhaseV1 {
    Idle,
    Casting,
    Complete,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastOutcomeV1 {
    None,
    Completed,
    InterruptMovement,
    InterruptContam,
    InterruptControl,
    UserCancel,
    Death,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastSyncV1 {
    pub phase: CastPhaseV1,
    /// 0..=8 表示 F1..F9；idle 时 client 忽略。
    pub slot: u8,
    pub duration_ms: u32,
    pub started_at_ms: u64,
    pub outcome: CastOutcomeV1,
}

/// plan-HUD-v1 §10.4 / §11.4 F1-F9 槽位完整配置 + 当前 cooldown。
/// server 在 `QuickSlotBindings` 变化时推（绑定 / cast 完成 / 中断 → cooldown 写入）。
/// `slots` / `cooldown_until_ms` 永远长度 9（client 用 idx 取）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuickSlotConfigV1 {
    pub slots: Vec<Option<QuickSlotEntryV1>>,
    /// 0 表示无冷却；否则为 unix ms 截止时间。
    pub cooldown_until_ms: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuickSlotEntryV1 {
    pub item_id: String,
    pub display_name: String,
    pub cast_duration_ms: u32,
    pub cooldown_ms: u32,
    pub icon_texture: String,
}

/// plan-hotbar-modify-v1 §5.3 1-9 技能栏完整配置 + 当前 cooldown。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillBarConfigV1 {
    pub slots: Vec<Option<SkillBarEntryV1>>,
    /// 0 表示无冷却；否则为 unix ms 截止时间。
    pub cooldown_until_ms: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "snake_case")]
pub enum SkillBarEntryV1 {
    Item {
        template_id: String,
        display_name: String,
        cast_duration_ms: u32,
        cooldown_ms: u32,
        icon_texture: String,
    },
    Skill {
        skill_id: String,
        display_name: String,
        cast_duration_ms: u32,
        cooldown_ms: u32,
        icon_texture: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniquesSnapshotV1 {
    pub entries: Vec<TechniqueEntryV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueEntryV1 {
    pub id: String,
    pub display_name: String,
    pub grade: String,
    pub proficiency: f32,
    pub active: bool,
    pub description: String,
    pub required_realm: String,
    pub required_meridians: Vec<TechniqueRequiredMeridianV1>,
    pub qi_cost: f32,
    pub cast_ticks: u32,
    pub cooldown_ticks: u32,
    pub range: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TechniqueRequiredMeridianV1 {
    pub channel: String,
    pub min_health: f32,
}

/// plan-HUD-v1 §1.3 / §11.4 玩家解锁的防御流派（截脉/替尸/绝灵）。
/// 客户端用于条件渲染门禁——未解锁的指示器完全不显示（§1.4）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnlocksSyncV1 {
    pub jiemai: bool,
    pub tishi: bool,
    pub jueling: bool,
}

/// plan-HUD-v1 §2.3 / §11.4 右侧统一事件流单条推送。
/// `channel` 与 client `UnifiedEvent.Channel` 1:1：combat / cultivation / world / social / system。
/// `priority` 与 client `Priority` 1:1，决定事件存活时间。
/// `source_tag` 用作折叠 key（同 channel + tag + text 在 1.5s 内 fold ×N）。
/// `color` 0 表示 client 用 channel default 色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventChannelV1 {
    Combat,
    Cultivation,
    World,
    Social,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventPriorityV1 {
    P0Critical,
    P1Important,
    P2Normal,
    P3Verbose,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventStreamPushV1 {
    pub channel: EventChannelV1,
    pub priority: EventPriorityV1,
    pub source_tag: String,
    pub text: String,
    pub color: u32,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatHudStateV1 {
    /// HP 百分比 [0.0, 1.0]。当前从 Wounds 总伤推导（v1 未接，固定 1.0）。
    pub hp_percent: f32,
    /// 真元百分比 [0.0, 1.0]。
    pub qi_percent: f32,
    /// 体力百分比 [0.0, 1.0]。
    pub stamina_percent: f32,
    pub derived: DerivedAttrFlagsV1,
}

// ─── plan-weapon-v1 §8.2：装备槽推送 ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponViewV1 {
    pub instance_id: u64,
    pub template_id: String,
    /// 小写 snake_case: `sword` / `saber` / `staff` / `fist` / `spear` / `dagger` / `bow`。
    pub weapon_kind: String,
    pub durability_current: f32,
    pub durability_max: f32,
    pub quality_tier: u8,
}

/// 装备槽变更推送（plan-weapon-v1 §8.2）。
///
/// `weapon: None` 表示该槽位被清空（卸下 / 死亡 drop / 武器 broken 后自动移除）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponEquippedV1 {
    /// `main_hand` / `off_hand` / `two_hand`。
    pub slot: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weapon: Option<WeaponViewV1>,
}

/// 武器损坏通知（plan-weapon-v1 §6.3）。`Weapon` component 已被 server 移除，
/// 但 ItemInstance（durability=0）仍在背包里等待修复。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponBrokenV1 {
    pub instance_id: u64,
    pub template_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreasureViewV1 {
    pub instance_id: u64,
    pub template_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreasureEquippedV1 {
    pub slot: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub treasure: Option<TreasureViewV1>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_content() {
        let original = CombatHudStateV1 {
            hp_percent: 0.85,
            qi_percent: 0.42,
            stamina_percent: 0.91,
            derived: DerivedAttrFlagsV1 {
                flying: true,
                phasing: false,
                tribulation_locked: false,
            },
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: CombatHudStateV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn weapon_equipped_roundtrip_basic() {
        let original = WeaponEquippedV1 {
            slot: "main_hand".to_string(),
            weapon: Some(WeaponViewV1 {
                instance_id: 42,
                template_id: "iron_sword".to_string(),
                weapon_kind: "sword".to_string(),
                durability_current: 185.0,
                durability_max: 200.0,
                quality_tier: 1,
            }),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: WeaponEquippedV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn weapon_equipped_roundtrip_without_weapon_omits_field() {
        // plan §8.2：weapon=None 时 omit 字段(skip_serializing_if)而非 `"weapon":null`
        let original = WeaponEquippedV1 {
            slot: "main_hand".to_string(),
            weapon: None,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        assert!(
            !json.contains("weapon"),
            "weapon field should be omitted: {json}"
        );
        let parsed: WeaponEquippedV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn weapon_broken_roundtrip() {
        let original = WeaponBrokenV1 {
            instance_id: 77,
            template_id: "bone_dagger".to_string(),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: WeaponBrokenV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn treasure_equipped_roundtrip_basic() {
        let original = TreasureEquippedV1 {
            slot: "treasure_belt_0".to_string(),
            treasure: Some(TreasureViewV1 {
                instance_id: 88,
                template_id: "starter_talisman".to_string(),
                display_name: "启程护符".to_string(),
            }),
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: TreasureEquippedV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let json = r#"{"hp_percent":1.0,"qi_percent":1.0,"stamina_percent":1.0,"derived":{"flying":false,"phasing":false,"tribulation_locked":false},"extra":1}"#;
        assert!(serde_json::from_str::<CombatHudStateV1>(json).is_err());
    }

    #[test]
    fn wounds_snapshot_roundtrip_preserves_content() {
        let original = WoundsSnapshotV1 {
            wounds: vec![
                WoundEntryV1 {
                    part: "chest".to_string(),
                    kind: "cut".to_string(),
                    severity: 0.6,
                    state: "bleeding".to_string(),
                    infection: 0.1,
                    scar: false,
                    updated_at_ms: 123_456,
                },
                WoundEntryV1 {
                    part: "head".to_string(),
                    kind: "concussion".to_string(),
                    severity: 0.3,
                    state: "stable".to_string(),
                    infection: 0.0,
                    scar: false,
                    updated_at_ms: 123_457,
                },
            ],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: WoundsSnapshotV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn wounds_snapshot_rejects_unknown_field() {
        let json = r#"{"wounds":[],"extra":1}"#;
        assert!(serde_json::from_str::<WoundsSnapshotV1>(json).is_err());
    }

    #[test]
    fn cast_sync_roundtrip_preserves_content() {
        let original = CastSyncV1 {
            phase: CastPhaseV1::Casting,
            slot: 3,
            duration_ms: 1500,
            started_at_ms: 1_700_000_000_000,
            outcome: CastOutcomeV1::None,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: CastSyncV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn quickslot_config_roundtrip_preserves_content() {
        let original = QuickSlotConfigV1 {
            slots: vec![
                Some(QuickSlotEntryV1 {
                    item_id: "kai_mai_pill".to_string(),
                    display_name: "开脉丹".to_string(),
                    cast_duration_ms: 1500,
                    cooldown_ms: 1500,
                    icon_texture: String::new(),
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            cooldown_until_ms: vec![1_700_000_001_500, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: QuickSlotConfigV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn skillbar_config_roundtrip_preserves_item_skill_and_empty_slots() {
        let original = SkillBarConfigV1 {
            slots: vec![
                Some(SkillBarEntryV1::Skill {
                    skill_id: "burst_meridian.beng_quan".to_string(),
                    display_name: "崩拳".to_string(),
                    cast_duration_ms: 400,
                    cooldown_ms: 3000,
                    icon_texture: "bong:textures/gui/skill/beng_quan.png".to_string(),
                }),
                Some(SkillBarEntryV1::Item {
                    template_id: "iron_sword".to_string(),
                    display_name: "铁剑".to_string(),
                    cast_duration_ms: 0,
                    cooldown_ms: 0,
                    icon_texture: "bong:textures/item/iron_sword.png".to_string(),
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
            cooldown_until_ms: vec![1_700_000_003_000, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: SkillBarConfigV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn techniques_snapshot_roundtrip_preserves_detail_fields() {
        let original = TechniquesSnapshotV1 {
            entries: vec![TechniqueEntryV1 {
                id: "burst_meridian.beng_quan".to_string(),
                display_name: "崩拳".to_string(),
                grade: "yellow".to_string(),
                proficiency: 0.5,
                active: true,
                description: "以臂经爆发短劲，近身破防。".to_string(),
                required_realm: "Induce".to_string(),
                required_meridians: vec![TechniqueRequiredMeridianV1 {
                    channel: "LargeIntestine".to_string(),
                    min_health: 0.01,
                }],
                qi_cost: 0.4,
                cast_ticks: 8,
                cooldown_ticks: 60,
                range: 1.3,
            }],
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: TechniquesSnapshotV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn unlocks_sync_roundtrip_preserves_content() {
        let original = UnlocksSyncV1 {
            jiemai: true,
            tishi: false,
            jueling: true,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: UnlocksSyncV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn event_stream_push_roundtrip_preserves_content() {
        let original = EventStreamPushV1 {
            channel: EventChannelV1::Combat,
            priority: EventPriorityV1::P1Important,
            source_tag: "Chest-Cut".to_string(),
            text: "受 Chest Cut 伤 -12".to_string(),
            color: 0,
            created_at_ms: 1_700_000_000_000,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: EventStreamPushV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    #[test]
    fn defense_window_roundtrip_preserves_content() {
        let original = DefenseWindowV1 {
            duration_ms: 200,
            started_at_ms: 1_700_000_000_000,
            expires_at_ms: 1_700_000_000_200,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: DefenseWindowV1 = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }
}
