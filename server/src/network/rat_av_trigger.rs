//! 噬元鼠出招 → GeckoLib 实体招式动画 emit（plan-devour-rat-model P3）。
//!
//! 鼠是 Marker 实体（无 `UniqueId`），`PlayAnim`（按玩家 UUID 寻人）走不通；招式动画走
//! `VfxEventPayloadV1::PlayEntityAnim`，按 MC 协议 `entity_id`（Valence `EntityId::get()`）
//! 定位客户端 `FaunaEntity` 播一次性招式动画（`FaunaActionBridge` → `triggerAction`）。
//!
//! 与黑武士 boss 的 `heiwushi_av_trigger` 同构：action system emit 语义事件
//! （`RatAttackAvEvent`），本模块把它翻译成 wire 上的动画名 + 时长。
//!
//! **动画名必须与 `client/.../assets/bong/animations/devour_rat.animation.json` 的 key
//! 逐字一致**，否则 GeckoLib 解析不到、实体定格绑定姿势。时长按各动画
//! `animation_length`（秒）× 20 tick/s 向上取整，播完自动回 idle。

use valence::entity::EntityId;
use valence::prelude::{EventReader, EventWriter, Query};

use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::brain_rat::{RatAttackAvEvent, RatAttackKind};
use crate::schema::vfx_event::VfxEventPayloadV1;

// ── GeckoLib 招式动画名（与 devour_rat.animation.json key 逐字对齐）────────────
const ANIM_DEVOUR_RAT_PECK: &str = "animation.bong.devour_rat.peck";
const ANIM_DEVOUR_RAT_CLAW: &str = "animation.bong.devour_rat.claw";
const ANIM_DEVOUR_RAT_POUNCE: &str = "animation.bong.devour_rat.pounce";

// ── 动画时长（tick）= animation_length(秒) × 20，向上取整 ──────────────────────
/// peck: animation_length 0.5s → 10 tick
const TICKS_DEVOUR_RAT_PECK: u16 = 10;
/// claw: animation_length 0.6s → 12 tick
const TICKS_DEVOUR_RAT_CLAW: u16 = 12;
/// pounce: animation_length 0.78s → 15.6 → 16 tick（宁可多留 0.4 tick 也不要提前截断扑击）
const TICKS_DEVOUR_RAT_POUNCE: u16 = 16;

/// 招式 → (GeckoLib 动画名, 动画占用 tick)。
///
/// 三招都有对应动画，故返回值不是 `Option`——新增招式若无动画资产，应显式在此处
/// 改成 `Option` 并补"跳过 emit"分支，而不是塞一个解析不到的名字上 wire。
pub fn rat_entity_anim_for(kind: RatAttackKind) -> (&'static str, u16) {
    match kind {
        RatAttackKind::Peck => (ANIM_DEVOUR_RAT_PECK, TICKS_DEVOUR_RAT_PECK),
        RatAttackKind::Claw => (ANIM_DEVOUR_RAT_CLAW, TICKS_DEVOUR_RAT_CLAW),
        RatAttackKind::Pounce => (ANIM_DEVOUR_RAT_POUNCE, TICKS_DEVOUR_RAT_POUNCE),
    }
}

/// 消费 `RatAttackAvEvent` → emit `PlayEntityAnim`。
///
/// 鼠无 `EntityId` 组件（尚未分配协议 id / 已 despawn）时跳过——客户端反正
/// `world.getEntityById` 定位不到，emit 出去只是一条死事件。
pub fn emit_rat_attack_entity_anim(
    mut events: EventReader<RatAttackAvEvent>,
    mut vfx_events: EventWriter<VfxEventRequest>,
    rat_ids: Query<&EntityId>,
) {
    for event in events.read() {
        let Ok(entity_id) = rat_ids.get(event.rat) else {
            continue;
        };
        let (anim, duration_ticks) = rat_entity_anim_for(event.kind);
        vfx_events.send(VfxEventRequest::new(
            event.origin,
            VfxEventPayloadV1::PlayEntityAnim {
                entity_id: entity_id.get(),
                anim: anim.to_string(),
                duration_ticks,
            },
        ));
    }
}

/// 注册 emit 系统。
///
/// 排在 `emit_vfx_event_payloads` **之前**，保证本帧 emit 的 `PlayEntityAnim` 当帧就发出去
/// （与 `heiwushi_av_trigger` 同一编排口径）。事件类型 `RatAttackAvEvent` 由
/// `npc::brain_rat::register` 注册——鼠 AI 是它的生产方。
pub fn register(app: &mut valence::prelude::App) {
    use valence::prelude::{IntoSystemConfigs, Update};
    app.add_systems(
        Update,
        emit_rat_attack_entity_anim.before(crate::network::vfx_event_emit::emit_vfx_event_payloads),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use valence::prelude::{App, DVec3, Entity, Events, Update};

    fn make_app() -> App {
        let mut app = App::new();
        app.add_event::<RatAttackAvEvent>()
            .add_event::<VfxEventRequest>()
            .add_systems(Update, emit_rat_attack_entity_anim);
        app
    }

    /// 造一只带 `EntityId` 组件的鼠（模拟已分配协议 id 的 marker 实体）。
    /// valence `EntityId` 字段私有，外部只能造 `EntityId::default()`（= -1，运行时由
    /// valence 改写为正值）；断言"emit 的 entity_id == 该实体 EntityId.get()"锁的是
    /// "读对了实体"这条接线契约，与具体数值无关。
    fn spawn_rat_with_entity_id(app: &mut App) -> Entity {
        app.world_mut().spawn(EntityId::default()).id()
    }

    fn send_attack(app: &mut App, rat: Entity, kind: RatAttackKind) {
        app.world_mut().send_event(RatAttackAvEvent {
            rat,
            kind,
            origin: DVec3::new(10.0, 64.0, -3.0),
        });
    }

    fn drain_entity_anim(app: &App) -> Vec<(i32, String, u16)> {
        app.world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .filter_map(|e| {
                if let VfxEventPayloadV1::PlayEntityAnim {
                    entity_id,
                    anim,
                    duration_ticks,
                } = &e.payload
                {
                    Some((*entity_id, anim.clone(), *duration_ticks))
                } else {
                    None
                }
            })
            .collect()
    }

    // ── 接线核验（防功能孤岛）────────────────────────────────────────────────

    /// 取源码的**生产半区**：砍掉 inline `#[cfg(test)] mod tests {` 之后的全部内容，
    /// 再剔除纯注释行。
    ///
    /// 必要性（这条 helper 本身就是一次返工的产物）：needle
    /// `add_event::<RatAttackAvEvent>()` 在 `brain_rat.rs` 里出现 9 次——生产 `register()`
    /// 1 次 + 8 个单测各自建 app 时 1 次。对全文件裸 `contains` 是**恒真**的：把生产那行删掉
    /// 照样绿，正好放过它声称要挡的失败。剔注释行则另外挡住「把调用注释掉但 pin 仍过」的绕法。
    ///
    /// 只砍 `mod tests {`（inline 模块，带花括号），不砍 `#[cfg(test)] mod xxx_test;`
    /// 这种文件级声明——`network/mod.rs` 顶部就有若干条，按前者切会把整个 `register` 函数
    /// 一起切掉，断言反而恒假。
    fn production_source(src: &str) -> String {
        let head = src
            .split_once("#[cfg(test)]\nmod tests {")
            .map(|(head, _)| head)
            // 切分串对 rustfmt 输出形态敏感。落空时直接 panic 把失败点钉在真正原因上，
            // 而不是退化成"返回全文 → pin 恒真"、让同伴元测试以误导性信息撞红。
            .unwrap_or_else(|| {
                panic!(
                    "production_source 未找到 inline 测试模块起点 `#[cfg(test)]\\nmod tests {{`；\
                     被审文件的形态变了（属性与 mod 间多了空行/注释？），切分逻辑需同步更新"
                )
            });
        head.lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 两条接线断任一条，鼠咬人就永远只播 idle，而本文件其余单测照样全绿：
    /// ① `emit_rat_attack_entity_anim` 没被 `network::register` 注册 → 事件无人消费；
    /// ② `RatAttackAvEvent` 没被 `brain_rat::register` 注册 → `EventWriter` 取不到资源。
    #[test]
    fn emit_system_and_event_are_wired_into_app() {
        let network_mod = production_source(include_str!("mod.rs"));
        assert!(
            network_mod.contains("rat_av_trigger::register(app)"),
            "network::register 必须调用 rat_av_trigger::register(app)，否则 RatAttackAvEvent \
             无人消费——鼠咬人永远只播 idle"
        );
        assert!(
            network_mod.contains("pub mod rat_av_trigger;"),
            "rat_av_trigger 必须在 network 下声明为模块"
        );

        let brain_rat = production_source(include_str!("../npc/brain_rat.rs"));
        assert!(
            brain_rat.contains("add_event::<RatAttackAvEvent>()"),
            "brain_rat::register 必须注册 RatAttackAvEvent 事件类型，否则 action system 的 \
             EventWriter<RatAttackAvEvent> 取不到 Events 资源"
        );
    }

    /// 元测试：证明上面那条 pin **不是恒真的**。
    ///
    /// 直接锁住 `production_source` 的判别力——若它退化成"返回全文"（或有人把生产半区的
    /// 切分逻辑改坏），删掉生产注册行后 needle 仍能命中，这条会撞红。
    #[test]
    fn wiring_pin_actually_fails_when_production_registration_is_removed() {
        let brain_rat = production_source(include_str!("../npc/brain_rat.rs"));
        let mutated = brain_rat.replacen("    app.add_event::<RatAttackAvEvent>();\n", "", 1);
        assert!(
            !mutated.contains("add_event::<RatAttackAvEvent>()"),
            "生产半区里 add_event::<RatAttackAvEvent>() 应当只出现一次（brain_rat::register 内）；\
             删掉后仍能命中 = 上面的接线 pin 是恒真的、挡不住孤岛。\
             （测试模块里的 8 次同名调用必须已被 production_source 切掉。）"
        );

        let network_mod = production_source(include_str!("mod.rs"));
        let mutated_net = network_mod.replacen("    rat_av_trigger::register(app);\n", "", 1);
        assert!(
            !mutated_net.contains("rat_av_trigger::register(app)"),
            "生产半区里 rat_av_trigger::register(app) 应当只出现一次"
        );
    }

    // ── 三招各自的 anim / duration / entity_id ────────────────────────────────

    #[test]
    fn each_attack_kind_emits_its_own_anim_duration_and_entity_id() {
        let cases = [
            (RatAttackKind::Peck, "animation.bong.devour_rat.peck", 10u16),
            (RatAttackKind::Claw, "animation.bong.devour_rat.claw", 12),
            (
                RatAttackKind::Pounce,
                "animation.bong.devour_rat.pounce",
                16,
            ),
        ];
        for (kind, expected_anim, expected_duration) in cases {
            let mut app = make_app();
            let rat = spawn_rat_with_entity_id(&mut app);
            let expected_id = app.world().get::<EntityId>(rat).unwrap().get();
            send_attack(&mut app, rat, kind);
            app.update();

            let emitted = drain_entity_anim(&app);
            assert_eq!(
                emitted.len(),
                1,
                "{kind:?} 应 emit 恰好 1 条 PlayEntityAnim，实际: {emitted:?}"
            );
            let (entity_id, anim, duration) = &emitted[0];
            assert_eq!(
                *entity_id, expected_id,
                "{kind:?} 的 entity_id 应取自出招鼠的 EntityId.get()={expected_id}，实际 {entity_id}\
                 —— 取错实体会让别的鼠播动画"
            );
            assert_eq!(
                anim, expected_anim,
                "{kind:?} 的 anim 应为 {expected_anim}（devour_rat.animation.json 的 key），实际 {anim}"
            );
            assert_eq!(
                *duration, expected_duration,
                "{kind:?} 的 duration_ticks 应为 {expected_duration}（animation_length×20 上取整），实际 {duration}"
            );
        }
    }

    /// 三招必须**互不相同**：招式辨识度是本阶段的目标，共用一个动画等于没做。
    #[test]
    fn all_three_kinds_map_to_distinct_animations() {
        let names: HashSet<&str> = [
            RatAttackKind::Peck,
            RatAttackKind::Claw,
            RatAttackKind::Pounce,
        ]
        .into_iter()
        .map(|kind| rat_entity_anim_for(kind).0)
        .collect();
        assert_eq!(
            names.len(),
            3,
            "peck/claw/pounce 必须映射到 3 个不同动画名，实际只有 {} 个: {names:?}",
            names.len()
        );
    }

    #[test]
    fn all_durations_are_positive_and_within_wire_limit() {
        for kind in [
            RatAttackKind::Peck,
            RatAttackKind::Claw,
            RatAttackKind::Pounce,
        ] {
            let (_, duration) = rat_entity_anim_for(kind);
            assert!(
                duration > 0
                    && duration <= crate::schema::vfx_event::VFX_ENTITY_ANIM_DURATION_TICKS_MAX,
                "{kind:?} 时长 {duration} 必须落在 1..={} —— 越界会被 \
                 VfxEventV1::to_json_bytes_checked 拒绝，动画静默不播",
                crate::schema::vfx_event::VFX_ENTITY_ANIM_DURATION_TICKS_MAX
            );
        }
    }

    #[test]
    fn all_anim_names_are_within_wire_length_limit() {
        for kind in [
            RatAttackKind::Peck,
            RatAttackKind::Claw,
            RatAttackKind::Pounce,
        ] {
            let (anim, _) = rat_entity_anim_for(kind);
            assert!(
                !anim.is_empty()
                    && anim.chars().count()
                        <= crate::schema::vfx_event::VFX_ENTITY_ANIM_NAME_MAX_CHARS,
                "{kind:?} 动画名 {anim:?} 必须非空且 ≤ {} 字符",
                crate::schema::vfx_event::VFX_ENTITY_ANIM_NAME_MAX_CHARS
            );
        }
    }

    /// emit 的 payload 必须能通过 wire 校验（不是只在内存里长得对）。
    #[test]
    fn emitted_payload_passes_wire_validation() {
        for kind in [
            RatAttackKind::Peck,
            RatAttackKind::Claw,
            RatAttackKind::Pounce,
        ] {
            let (anim, duration) = rat_entity_anim_for(kind);
            let event = crate::schema::vfx_event::VfxEventV1::play_entity_anim(1, anim, duration);
            assert!(
                event.to_json_bytes_checked().is_ok(),
                "{kind:?} 的 PlayEntityAnim payload 必须通过 schema 校验，否则 client 收不到"
            );
        }
    }

    // ── 边界：无 EntityId / 无事件 / 多事件 ──────────────────────────────────

    #[test]
    fn attack_from_rat_without_entity_id_is_skipped() {
        let mut app = make_app();
        let rat = app.world_mut().spawn_empty().id();
        send_attack(&mut app, rat, RatAttackKind::Peck);
        app.update();
        assert!(
            drain_entity_anim(&app).is_empty(),
            "鼠无 EntityId（未分配协议 id / 已 despawn）时不应 emit——客户端定位不到实体，\
             emit 出去只是死事件，实际: {:?}",
            drain_entity_anim(&app)
        );
    }

    #[test]
    fn attack_from_despawned_rat_is_skipped() {
        let mut app = make_app();
        let rat = spawn_rat_with_entity_id(&mut app);
        app.world_mut().despawn(rat);
        send_attack(&mut app, rat, RatAttackKind::Pounce);
        app.update();
        assert!(
            drain_entity_anim(&app).is_empty(),
            "已 despawn 的鼠查不到 EntityId，应安静跳过而非 panic"
        );
    }

    #[test]
    fn no_events_emits_nothing() {
        let mut app = make_app();
        spawn_rat_with_entity_id(&mut app);
        app.update();
        assert!(
            drain_entity_anim(&app).is_empty(),
            "没有咬击就不该有招式动画"
        );
    }

    #[test]
    fn multiple_rats_each_emit_their_own_anim() {
        let mut app = make_app();
        let rat_a = spawn_rat_with_entity_id(&mut app);
        let rat_b = spawn_rat_with_entity_id(&mut app);
        send_attack(&mut app, rat_a, RatAttackKind::Peck);
        send_attack(&mut app, rat_b, RatAttackKind::Pounce);
        app.update();

        let anims: Vec<String> = drain_entity_anim(&app)
            .into_iter()
            .map(|(_, anim, _)| anim)
            .collect();
        assert_eq!(
            anims,
            vec![
                "animation.bong.devour_rat.peck".to_string(),
                "animation.bong.devour_rat.pounce".to_string()
            ],
            "同一 tick 两只鼠各出各的招，不能被合并或串味，实际 {anims:?}"
        );
    }

    /// 同一只鼠同 tick 连续两次咬击（理论上被 action state 机互斥，但事件层不该吞）。
    #[test]
    fn repeated_attacks_from_same_rat_emit_once_each() {
        let mut app = make_app();
        let rat = spawn_rat_with_entity_id(&mut app);
        send_attack(&mut app, rat, RatAttackKind::Claw);
        send_attack(&mut app, rat, RatAttackKind::Claw);
        app.update();
        assert_eq!(
            drain_entity_anim(&app).len(),
            2,
            "事件层不做去重——每条 RatAttackAvEvent 都应转成一条 PlayEntityAnim"
        );
    }

    /// emit 的 `origin` 必须是出招点：`VfxEventRequest` 按它做 64 格广播过滤，
    /// 填错会让近处玩家收不到 / 远处玩家白收。
    #[test]
    fn emitted_request_origin_matches_attack_origin() {
        let mut app = make_app();
        let rat = spawn_rat_with_entity_id(&mut app);
        send_attack(&mut app, rat, RatAttackKind::Peck);
        app.update();

        let origins: Vec<DVec3> = app
            .world()
            .resource::<Events<VfxEventRequest>>()
            .iter_current_update_events()
            .map(|e| e.origin)
            .collect();
        assert_eq!(
            origins,
            vec![DVec3::new(10.0, 64.0, -3.0)],
            "VfxEventRequest.origin 应为出招坐标（广播半径过滤依据），实际 {origins:?}"
        );
    }
}
