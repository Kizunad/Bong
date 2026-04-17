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

/// plan-HUD-v1 §3.4 / §11.4 当前防御姿态 + 替尸伪皮层数 + 绝灵涡流冷却。
/// 仅在玩家解锁了对应流派时才有意义（client 端配合 `unlocks_sync` 做条件渲染）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefenseStanceV1 {
    None,
    Jiemai,
    Tishi,
    Jueling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefenseSyncV1 {
    pub stance: DefenseStanceV1,
    /// 替尸流剩余伪皮层数（§3.4）。0 = 不渲染。
    pub fake_skin_layers: u32,
    /// 绝灵流涡流是否激活中（§3.4 蓝色转圈）。
    pub vortex_active: bool,
    /// 涡流冷却结束的 unix ms；0 = 无冷却。
    pub vortex_ready_at_ms: u64,
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
pub struct SoulBondV1 {
    pub character_id: String,
    pub bond_level: u8,
    pub bond_progress: f32,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soul_bond: Option<SoulBondV1>,
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
    fn weapon_equipped_roundtrip_with_bond() {
        let original = WeaponEquippedV1 {
            slot: "main_hand".to_string(),
            weapon: Some(WeaponViewV1 {
                instance_id: 42,
                template_id: "iron_sword".to_string(),
                weapon_kind: "sword".to_string(),
                durability_current: 185.0,
                durability_max: 200.0,
                quality_tier: 1,
                soul_bond: Some(SoulBondV1 {
                    character_id: "char_a".to_string(),
                    bond_level: 2,
                    bond_progress: 0.4,
                }),
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
    fn defense_sync_roundtrip_preserves_content() {
        let original = DefenseSyncV1 {
            stance: DefenseStanceV1::Tishi,
            fake_skin_layers: 3,
            vortex_active: false,
            vortex_ready_at_ms: 1_700_000_005_000,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: DefenseSyncV1 = serde_json::from_str(&json).expect("deserialize");
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
