//! 负灵域诡影 + 噬灵藓 Narration hints（plan-neg-domain-fauna-v1 P3）。
//!
//! 两个系统各自维护 per-player per-session 首次触发门控（`Resource HashSet<Entity>`），
//! 防止同一玩家在同一服务器会话内反复收到相同提示。
//!
//! - [`ghost_contact_narration_system`]：检测首次 `GhostContact` QiTransfer 事件，
//!   向 `PendingGameplayNarrations::push_player` 发 `NarrationStyle::Perception` 提示。
//! - [`moss_drain_narration_system`]：检测玩家首次踩踏噬灵藓（挂 `ShiLingXianDrainTag`），
//!   向 `PendingGameplayNarrations::push_player` 发 `NarrationStyle::Perception` 提示。
//!
//! 两个系统均 server-only；无渲染资产（诡影粒子/噬灵藓贴图 deferred 待 VFX plan）。

use std::collections::HashSet;

use valence::prelude::{bevy_ecs, Entity, EventReader, Query, ResMut, Resource, With, Without};

use crate::botany::shiling_xian::ShiLingXianDrainTag;
use crate::cultivation::life_record::LifeRecord;
use crate::npc::spawn::common::NpcMarker;
use crate::player::gameplay::PendingGameplayNarrations;
use crate::qi_physics::ledger::{QiAccountKind, QiTransfer, QiTransferReason};
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

/// ghost_contact_narration_system：读取 `QiTransfer` 事件流，
/// 首次（per-player per-session）检测到 `GhostContact` reason 时，
/// 向 `PendingGameplayNarrations` push 一条 Perception 级提示。
///
/// 技术细节：
/// - 通过 `EventReader<QiTransfer>` 消费事件，`from`（player 账户）的 `.id` 就是 character_id。
/// - 同一 Entity 仅触发一次（`GhostNarrationSessionSeen` 门控）。
/// - 示例文案 2 条轮选（用 Entity bits 取模，稳定不随机）。
pub fn ghost_contact_narration_system(
    mut qi_events: EventReader<QiTransfer>,
    players: Query<(Entity, Option<&LifeRecord>), Without<NpcMarker>>,
    mut seen: ResMut<GhostNarrationSessionSeen>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
) {
    let Some(narrations) = narrations.as_deref_mut() else {
        // 消费事件以防积压，即使无 narrations resource
        qi_events.clear();
        return;
    };

    // 读当前帧所有 GhostContact 事件，提取涉及的 player character_id
    let ghost_contact_player_ids: Vec<String> = qi_events
        .read()
        .filter(|t| matches!(t.reason, QiTransferReason::GhostContact))
        .filter_map(|t| {
            // QiAccountId.from 是 player 侧（kind == Player），.id 就是 character_id
            if t.from.kind == QiAccountKind::Player {
                Some(t.from.id.clone())
            } else {
                None
            }
        })
        .collect();

    if ghost_contact_player_ids.is_empty() {
        return;
    }

    for (entity, life_record) in players.iter() {
        // 已经提示过：跳过（per-session 门控）
        if seen.seen.contains(&entity) {
            continue;
        }

        // 匹配 character_id
        let char_id = life_record.map(|lr| lr.character_id.as_str()).unwrap_or("");
        let matched = ghost_contact_player_ids.iter().any(|id| id == char_id);
        if !matched {
            continue;
        }

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
    use crate::cultivation::life_record::LifeRecord;
    use crate::player::gameplay::PendingGameplayNarrations;
    use crate::player::state::canonical_player_id;
    use crate::qi_physics::ledger::{QiAccountId, QiTransfer, QiTransferReason};
    use crate::schema::common::NarrationStyle;
    use valence::prelude::{App, Events, Update};

    // ── helpers ──

    fn make_app_with_ghost_narration() -> App {
        let mut app = App::new();
        app.add_event::<QiTransfer>();
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

    fn emit_ghost_contact_transfer(app: &mut App, char_id: &str) {
        let from = QiAccountId::player(char_id);
        let to = QiAccountId::zone("neg_zone");
        let transfer =
            QiTransfer::new(from, to, 1.0, QiTransferReason::GhostContact).expect("valid transfer");
        app.world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .send(transfer);
    }

    // ── T1: 首次接触诡影 → 发 narration ──

    #[test]
    fn ghost_narration_sent_on_first_contact() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Azure");

        app.world_mut()
            .spawn(LifeRecord::new(char_id.clone()))
            // Without<NpcMarker> 无需额外 component
            ;

        // Emit 一条 GhostContact QiTransfer
        emit_ghost_contact_transfer(&mut app, &char_id);

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert_eq!(
            narrations.len(),
            1,
            "首次诡影接触应发 1 条 narration（期望 1，实际 {}）",
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

    // ── T2: 第二次接触诡影 → 不重复发 narration（per-session 门控）──

    #[test]
    fn ghost_narration_not_sent_on_second_contact() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Bao");

        app.world_mut().spawn(LifeRecord::new(char_id.clone()));

        // 第一次接触
        emit_ghost_contact_transfer(&mut app, &char_id);
        app.update();
        // 清掉第一次的 narration
        app.world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();

        // 第二次接触
        emit_ghost_contact_transfer(&mut app, &char_id);
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

        app.world_mut().spawn(LifeRecord::new(char_a.clone()));
        app.world_mut().spawn(LifeRecord::new(char_b.clone()));

        emit_ghost_contact_transfer(&mut app, &char_a);
        emit_ghost_contact_transfer(&mut app, &char_b);

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

    // ── T4: 无 QiTransfer 事件时不发 narration ──

    #[test]
    fn ghost_narration_not_sent_without_qi_transfer_events() {
        let mut app = make_app_with_ghost_narration();

        app.world_mut()
            .spawn(LifeRecord::new(canonical_player_id("Empty")));

        // 不 emit 任何事件
        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "无 QiTransfer 事件时不应发 narration（期望空，实际 {} 条）",
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

    // ── T9: ShiLingXianDrain reason 不触发 ghost narration（reason 隔离）──

    #[test]
    fn ghost_narration_not_triggered_by_shiling_xian_drain_reason() {
        let mut app = make_app_with_ghost_narration();
        let char_id = canonical_player_id("Silent");

        app.world_mut().spawn(LifeRecord::new(char_id.clone()));

        // Emit ShiLingXianDrain（不是 GhostContact）
        let transfer = QiTransfer::new(
            QiAccountId::player(&char_id),
            QiAccountId::zone("neg_zone"),
            0.5,
            QiTransferReason::ShiLingXianDrain,
        )
        .expect("valid transfer");
        app.world_mut()
            .resource_mut::<Events<QiTransfer>>()
            .send(transfer);

        app.update();

        let narrations = app
            .world_mut()
            .resource_mut::<PendingGameplayNarrations>()
            .drain();
        assert!(
            narrations.is_empty(),
            "ShiLingXianDrain reason 不应触发 ghost narration（reason 隔离，期望空，实际 {} 条）",
            narrations.len()
        );
    }
}
