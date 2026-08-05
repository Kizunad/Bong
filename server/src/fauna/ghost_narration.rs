//! 负灵域诡影 + 噬灵藓 Narration hints（plan-neg-domain-fauna-v1 P3）。
//!
//! 两个系统各自维护 per-player per-session 首次触发门控（`Resource HashSet<Entity>`），
//! 防止同一玩家在同一服务器会话内反复收到相同提示。
//!
//! - [`ghost_contact_narration_system`]：检测首次诡影接触（`GhostContactCooldown` 组件被 insert/刷新），
//!   向 `PendingGameplayNarrations::push_player` 发 `NarrationStyle::Perception` 提示。
//!   信号来源：`ghost_contact_system` 每次接触都 insert `GhostContactCooldown`，本系统仅在
//!   per-session 首次看到该 component 时触发——不依赖 `QiTransferReason`（helper 统一走 ReleaseToZone）。
//! - [`moss_drain_narration_system`]：检测玩家首次踩踏噬灵藓（挂 `ShiLingXianDrainTag`），
//!   向 `PendingGameplayNarrations::push_player` 发 `NarrationStyle::Perception` 提示。
//!
//! 两个系统均 server-only；无渲染资产（诡影粒子/噬灵藓贴图 deferred 待 VFX plan）。

use std::collections::HashSet;

use valence::prelude::{bevy_ecs, Entity, Query, ResMut, Resource, With, Without};

use crate::botany::shiling_xian::ShiLingXianDrainTag;
use crate::cultivation::life_record::LifeRecord;
use crate::fauna::ghost::GhostContactCooldown;
use crate::npc::spawn::common::NpcMarker;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;

// ── 诡影接触提示文案（2 条；Perception 风格，从内感描写出发）──

/// 诡影接触 narration 模板（从中随机/轮选取 1 条）。
pub const GHOST_CONTACT_HINTS: &[&str] = &[
    "真元骤然一空，周身似有无形漩涡划过——这里的空气本身就在吞噬你的真气",
    "诡影触碰的一瞬，五脏真元剧烈震荡。负灵域的东西，无形却索命",
];

// ── 噬灵藓踩踏提示文案（2 条；Perception 风格，脚底感觉为主）──

/// 噬灵藓踩踏 narration 模板（从中随机/轮选取 1 条）。
pub const MOSS_DRAIN_HINTS: &[&str] = &[
    "脚下的黑苔正在吸食你的真元，每踏一步，气海都在萎缩",
    "噬灵藓——听说了，踩着它比闯坍缩渊还费真气",
];

// ── per-session 首次触发门控 Resource ──

/// 本会话已收到诡影接触 narration 的玩家 Entity 集合。
/// server 重启后自动重置；不持久化——设计意图是仅在本次会话内提示一次。
#[derive(Debug, Default, Resource)]
pub struct GhostNarrationSessionSeen {
    pub seen: HashSet<Entity>,
}

/// 本会话已收到噬灵藓踩踏 narration 的玩家 Entity 集合。
#[derive(Debug, Default, Resource)]
pub struct MossNarrationSessionSeen {
    pub seen: HashSet<Entity>,
}

// ── 系统 ──

/// ghost_contact_narration_system：检测首次诡影接触（`GhostContactCooldown` 组件存在），
/// 首次（per-player per-session）向 `PendingGameplayNarrations` push 一条 Perception 级提示。
///
/// 设计原因：`ghost_contact_system` 每次接触都 `commands.entity(entity).insert(GhostContactCooldown{..})`，
/// 本系统直接查询有 `GhostContactCooldown` 的玩家 Entity——无需依赖 `QiTransferReason`
///（`release_qi_amount_to_zone` helper 统一硬编码为 `ReleaseToZone`，不再 emit `GhostContact` reason）。
///
/// 技术细节：
/// - 直接查询有 `GhostContactCooldown` 的玩家 Entity（`With<GhostContactCooldown>` 过滤）。
/// - 同一 Entity 仅触发一次（`GhostNarrationSessionSeen` 门控）。
/// - 示例文案 2 条轮选（用 Entity bits 取模，稳定不随机）。
/// - 不依赖 `ReleaseToZone`（death/craft/moss 共用，会误触发）。
#[allow(clippy::type_complexity)]
pub fn ghost_contact_narration_system(
    players: Query<(Entity, Option<&LifeRecord>), (Without<NpcMarker>, With<GhostContactCooldown>)>,
    mut seen: ResMut<GhostNarrationSessionSeen>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let Some(narrations) = narrations.as_deref_mut() else {
        return;
    };

    for (entity, life_record) in players.iter() {
        // 已经提示过：跳过（per-session 门控）
        if seen.seen.contains(&entity) {
            continue;
        }

        let char_id = life_record.map(|lr| lr.character_id.as_str()).unwrap_or("");

        // 选一条文案（用 entity.to_bits() 取模，稳定不随机）
        let hint_idx = (entity.to_bits() as usize) % GHOST_CONTACT_HINTS.len();
        let hint_text = GHOST_CONTACT_HINTS[hint_idx];

        narrations.push_player(char_id, hint_text, NarrationStyle::Perception);
        seen.seen.insert(entity);
    }
}

/// moss_drain_narration_system：检测玩家首次挂 `ShiLingXianDrainTag`（即首次踩踏噬灵藓），
/// 向 `PendingGameplayNarrations` push 一条 Perception 级提示。
///
/// 技术细节：
/// - 直接查询有 `ShiLingXianDrainTag` 的玩家 Entity，无需读 QiTransfer 事件流。
/// - 同一 Entity 仅触发一次（`MossNarrationSessionSeen` 门控）。
#[allow(clippy::type_complexity)]
pub fn moss_drain_narration_system(
    players: Query<(Entity, Option<&LifeRecord>), (Without<NpcMarker>, With<ShiLingXianDrainTag>)>,
    mut seen: ResMut<MossNarrationSessionSeen>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let Some(narrations) = narrations.as_deref_mut() else {
        return;
    };

    for (entity, life_record) in players.iter() {
        if seen.seen.contains(&entity) {
            continue;
        }

        let char_id = life_record.map(|lr| lr.character_id.as_str()).unwrap_or("");

        let hint_idx = (entity.to_bits() as usize) % MOSS_DRAIN_HINTS.len();
        let hint_text = MOSS_DRAIN_HINTS[hint_idx];

        narrations.push_player(char_id, hint_text, NarrationStyle::Perception);
        seen.seen.insert(entity);
    }
}

// ── 单测 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::botany::shiling_xian::ShiLingXianDrainTag;
    use crate::cultivation::components::Cultivation;
    use crate::cultivation::life_record::LifeRecord;
    use crate::fauna::ghost::{GhostContactCooldown, GhostEntity, GhostZoneRegistry};
    use crate::player::gameplay::PendingGameplayNarrations;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::ledger::QiTransfer;
    use crate::schema::common::NarrationStyle;
    use crate::world::dimension::{CurrentDimension, DimensionKind};
    use crate::world::zone::ZoneRegistry;
    use valence::prelude::{App, DVec3, IntoSystemConfigs, Position, Update};

    // ── helpers ──

    fn make_app_with_ghost_narration() -> App {
        let mut app = App::new();
        app.insert_resource(PendingGameplayNarrations::default());
        app.init_resource::<GhostNarrationSessionSeen>();
        app.add_systems(Update, ghost_contact_narration_system);
        app
    }

    fn make_app_with_moss_narration() -> App {
        let mut app = App::new();
        app.insert_resource(PendingGameplayNarrations::default());
        app.init_resource::<MossNarrationSessionSeen>();
        app.add_systems(Update, moss_drain_narration_system);
        app
    }

    // ── T1: 首次接触诡影（GhostContactCooldown 存在）→ 发 narration ──

    #[test]
    fn ghost_narration_sent_on_first_contact() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Azure");

        // 模拟 ghost_contact_system 已 insert GhostContactCooldown
        app.world_mut().spawn((
            LifeRecord::new(char_id.clone()),
            GhostContactCooldown {
                last_contact_tick: 1,
            },
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "首次诡影接触（GhostContactCooldown 存在）应发 1 条 narration（期望 1，实际 {}）",
            narrations.len()
        );
        assert!(
            matches!(narrations[0].style, NarrationStyle::Perception),
            "narration style 应为 Perception（期望），实际 {:?}",
            narrations[0].style
        );
        assert!(!narrations[0].text.is_empty(), "narration 文案不应为空");
        assert_eq!(
            narrations[0].target.as_deref(),
            Some(char_id.as_str()),
            "narration target 应为 player character_id（期望 {char_id}，实际 {:?}）",
            narrations[0].target
        );
    }

    // ── T2: 第二次接触诡影 → per-session 门控，不重复发 narration ──

    #[test]
    fn ghost_narration_not_sent_on_second_contact() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Bao");

        let player = app
            .world_mut()
            .spawn((
                LifeRecord::new(char_id.clone()),
                GhostContactCooldown {
                    last_contact_tick: 1,
                },
            ))
            .id();

        // 第一次 update（触发 narration）
        app.update();
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        // 模拟第二次接触（cooldown 刷新）
        app.world_mut()
            .entity_mut(player)
            .insert(GhostContactCooldown {
                last_contact_tick: 25,
            });
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "per-session 门控：第二次诡影接触不应重复发 narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }

    // ── T3: 不同玩家各自首次接触各发一条 ──

    #[test]
    fn ghost_narration_sends_one_per_player_on_first_contact() {
        let mut app = make_app_with_ghost_narration();
        let char_a = canonical_player_id("Alice");
        let char_b = canonical_player_id("Bob");

        app.world_mut().spawn((
            LifeRecord::new(char_a.clone()),
            GhostContactCooldown {
                last_contact_tick: 1,
            },
        ));
        app.world_mut().spawn((
            LifeRecord::new(char_b.clone()),
            GhostContactCooldown {
                last_contact_tick: 1,
            },
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            2,
            "两个不同玩家首次接触各发一条 narration（期望 2，实际 {}）",
            narrations.len()
        );
        let targets: Vec<_> = narrations
            .iter()
            .filter_map(|n| n.target.as_deref())
            .collect();
        assert!(
            targets.contains(&char_a.as_str()),
            "narration 应包含 player Alice（实际 {:?}）",
            targets
        );
        assert!(
            targets.contains(&char_b.as_str()),
            "narration 应包含 player Bob（实际 {:?}）",
            targets
        );
    }

    // ── T4: 无 GhostContactCooldown 时不发 narration ──

    #[test]
    fn ghost_narration_not_sent_without_cooldown_component() {
        let mut app = make_app_with_ghost_narration();

        // 玩家没有 GhostContactCooldown（未被诡影接触过）
        app.world_mut()
            .spawn(LifeRecord::new(canonical_player_id("Empty")));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "无 GhostContactCooldown 时不应发 narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }

    // ── T5: 首次踩踏噬灵藓 → 发 moss narration ──

    #[test]
    fn moss_narration_sent_on_first_step() {
        let mut app = make_app_with_moss_narration();
        let char_id = canonical_player_id("Cyan");

        let player = app
            .world_mut()
            .spawn((
                LifeRecord::new(char_id.clone()),
                ShiLingXianDrainTag {
                    drain_per_tick: 0.2,
                    zone_name: "neg_zone".to_string(),
                },
            ))
            .id();

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "首次踩踏噬灵藓应发 1 条 narration（期望 1，实际 {}）",
            narrations.len()
        );
        assert!(
            matches!(narrations[0].style, NarrationStyle::Perception),
            "narration style 应为 Perception，实际 {:?}",
            narrations[0].style
        );
        assert_eq!(
            narrations[0].target.as_deref(),
            Some(char_id.as_str()),
            "narration target 应为 player（期望 {char_id}，实际 {:?}）",
            narrations[0].target
        );

        // 确认玩家被加入 seen
        assert!(
            app.world()
                .resource::<MossNarrationSessionSeen>()
                .seen
                .contains(&player),
            "首次踩踏后玩家应加入 MossNarrationSessionSeen.seen（期望：contain player entity）"
        );
    }

    // ── T6: 持续踩踏（多 tick）不重复发 moss narration ──

    #[test]
    fn moss_narration_not_sent_on_repeated_steps() {
        let mut app = make_app_with_moss_narration();
        let char_id = canonical_player_id("Dragon");

        app.world_mut().spawn((
            LifeRecord::new(char_id.clone()),
            ShiLingXianDrainTag {
                drain_per_tick: 0.2,
                zone_name: "neg_zone".to_string(),
            },
        ));

        // 第一次 update（发 narration）
        app.update();
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        // 多次 update 模拟持续踩踏
        for _ in 0..5 {
            app.update();
        }

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "持续踩踏噬灵藓不应重复发 narration（per-session 门控，期望空，实际 {} 条）",
            narrations.len()
        );
    }

    // ── T7: 无 ShiLingXianDrainTag 时不发 moss narration ──

    #[test]
    fn moss_narration_not_sent_when_no_drain_tag() {
        let mut app = make_app_with_moss_narration();

        // 玩家没有 ShiLingXianDrainTag
        app.world_mut()
            .spawn(LifeRecord::new(canonical_player_id("Safe")));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "无 ShiLingXianDrainTag 时不应发 moss narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }

    // ── T8: 文案内容 pin 测试——确保模板非空且每个变体都有值 ──

    #[test]
    fn ghost_contact_hints_are_non_empty() {
        // 静态检查：当前共 2 条（plan-neg-domain-fauna-v1 §P3 设计决议）
        assert_eq!(
            GHOST_CONTACT_HINTS.len(),
            2,
            "GHOST_CONTACT_HINTS 应有 2 条文案（期望 2，实际 {}）",
            GHOST_CONTACT_HINTS.len()
        );
        for (idx, hint) in GHOST_CONTACT_HINTS.iter().enumerate() {
            assert!(
                !hint.is_empty(),
                "GHOST_CONTACT_HINTS[{idx}] 不应为空字符串"
            );
        }
    }

    #[test]
    fn moss_drain_hints_are_non_empty() {
        // 静态检查：当前共 2 条（plan-neg-domain-fauna-v1 §P3 设计决议）
        assert_eq!(
            MOSS_DRAIN_HINTS.len(),
            2,
            "MOSS_DRAIN_HINTS 应有 2 条文案（期望 2，实际 {}）",
            MOSS_DRAIN_HINTS.len()
        );
        for (idx, hint) in MOSS_DRAIN_HINTS.iter().enumerate() {
            assert!(!hint.is_empty(), "MOSS_DRAIN_HINTS[{idx}] 不应为空字符串");
        }
    }

    // ── T9: 无 GhostContactCooldown 的玩家（仅挂 ShiLingXianDrainTag）不触发 ghost narration ──
    // 隔离：moss 踩踏不应产生 ghost narration，两个系统各司其职。

    #[test]
    fn ghost_narration_not_triggered_by_moss_drain_tag() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Silent");

        // 只有 ShiLingXianDrainTag，没有 GhostContactCooldown
        app.world_mut().spawn((
            LifeRecord::new(char_id.clone()),
            ShiLingXianDrainTag {
                drain_per_tick: 0.2,
                zone_name: "neg_zone".to_string(),
            },
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "仅有 ShiLingXianDrainTag 时不应触发 ghost narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }

    // ── E2E: ghost_contact_system × ghost_contact_narration_system 端到端链路 ──
    // 这是锁住"玩家进负灵域 → 诡影接触 → narration 触发"完整链路的集成测试。
    // 替代旧的"手塞 GhostContact reason"假绿测试，验证真实 system 串联路径。

    use crate::fauna::ghost::{ghost_contact_system, GHOST_SIPHON_RADIUS};

    fn make_e2e_app(spirit_qi: f64) -> App {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
        let mut zones = ZoneRegistry::fallback();
        zones.find_zone_mut("spawn").unwrap().spirit_qi = spirit_qi;
        app.insert_resource(zones);
        app.init_resource::<GhostZoneRegistry>();
        app.insert_resource(PendingGameplayNarrations::default());
        app.init_resource::<GhostNarrationSessionSeen>();
        // ghost_contact_system 先于 narration system 运行（insert cooldown → narration 本 tick 感知）
        app.add_systems(
            Update,
            (ghost_contact_system, ghost_contact_narration_system).chain(),
        );
        app
    }

    /// E2E-T1: 玩家在负灵域 + 诡影在接触半径内 → 首次接触触发 perception narration。
    #[test]
    fn e2e_ghost_contact_system_triggers_narration_on_first_contact() {
        let mut app = make_e2e_app(-0.5);
        let char_id = canonical_player_id("E2eAzure");

        // 诡影在玩家附近（距离 < GHOST_SIPHON_RADIUS=2.0）
        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.1),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            LifeRecord::new(char_id.clone()),
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "E2E：玩家首次进入负灵域诡影半径应触发 1 条 perception narration（期望 1，实际 {}）",
            narrations.len()
        );
        assert!(
            matches!(narrations[0].style, NarrationStyle::Perception),
            "E2E：narration style 应为 Perception（诡影首次接触 perception 叙事），实际 {:?}",
            narrations[0].style
        );
        assert_eq!(
            narrations[0].target.as_deref(),
            Some(char_id.as_str()),
            "E2E：narration target 应为 player（期望 {char_id}，实际 {:?}）",
            narrations[0].target
        );
    }

    /// E2E-T2: 同一玩家第二次接触（cooldown 内/已提示）→ narration 不重复触发。
    #[test]
    fn e2e_ghost_narration_not_repeated_on_second_contact() {
        let mut app = make_e2e_app(-0.5);
        let char_id = canonical_player_id("E2eBao");

        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.1),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            LifeRecord::new(char_id.clone()),
        ));

        // 第一次 update → 触发接触 + narration
        app.update();
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        // 多次 update（cooldown 内 ghost_contact_system 不 re-trigger，但即使触发 narration 也走 seen 门控）
        for _ in 0..5 {
            app.update();
        }

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "E2E：per-session 门控——同一玩家不应重复收到诡影 narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }

    /// E2E-T3: 正灵域（spirit_qi >= 0）中不触发诡影接触，narration 不发。
    #[test]
    fn e2e_ghost_narration_not_triggered_in_positive_zone() {
        let mut app = make_e2e_app(0.5); // 正灵域
        let char_id = canonical_player_id("E2eSafe");

        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(8.0, 66.0, 8.0),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            LifeRecord::new(char_id.clone()),
        ));

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "E2E：正灵域中不应触发诡影 narration（ghost_contact_system 不接触 → 无 cooldown → 无 narration，期望空，实际 {} 条）",
            narrations.len()
        );
    }

    /// E2E-T4: 诡影超出 siphon 半径 → 不接触 → narration 不发。
    #[test]
    fn e2e_ghost_narration_not_triggered_when_ghost_out_of_radius() {
        let mut app = make_e2e_app(-0.5);
        let char_id = canonical_player_id("E2eFar");

        // 诡影在 100 格外（远超 GHOST_SIPHON_RADIUS=2.0）
        app.world_mut().spawn(GhostEntity {
            position: DVec3::new(108.0, 66.0, 8.0),
            drift_velocity: DVec3::ZERO,
            zone_name: "spawn".to_string(),
            tick_counter: 0,
        });

        app.world_mut().spawn((
            Position::new([8.0, 66.0, 8.0]),
            CurrentDimension(DimensionKind::Overworld),
            Cultivation {
                qi_current: 50.0,
                qi_max: 100.0,
                ..Default::default()
            },
            LifeRecord::new(char_id.clone()),
        ));

        // 校验 ghost 确实超出半径（文档化测试前提）
        let dist = (DVec3::new(108.0, 66.0, 8.0) - DVec3::new(8.0, 66.0, 8.0)).length();
        assert!(
            dist >= GHOST_SIPHON_RADIUS,
            "测试前提：诡影距离 {dist} 应 >= GHOST_SIPHON_RADIUS {GHOST_SIPHON_RADIUS}"
        );

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "E2E：诡影超出 siphon 半径时不应触发 narration（期望空，实际 {} 条）",
            narrations.len()
        );
    }
}
