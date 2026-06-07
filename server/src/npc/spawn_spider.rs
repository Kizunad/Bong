//! 拟态灰烬蛛 NPC 生成 — plan-fauna-mimic-spider-v1 P1
//!
//! 参照 spawn_rat.rs / spawn/beast.rs 的 bundle 组装模式，
//! 为 BeastKind::Spider 附加 MimicSpiderBlackboard + SpiderDisguiseState + NameVisible(false)。
//!
//! 生成时默认 NameVisible(false)（nameplate 隐藏），伪装为灰烬方块。
//! P2 阶段 client 伪装渲染逻辑由 SpiderDisguiseHandler.java 接管。

use bevy_transform::components::{GlobalTransform, Transform};
use valence::entity::entity::NameVisible;
use valence::entity::marker::MarkerEntityBundle;
use valence::prelude::{Commands, DVec3, Entity, EntityLayerId, Position};

use crate::fauna::components::{BeastKind, FaunaTag};
use crate::fauna::mimic_spider::{MimicSpiderBlackboard, SpiderDisguiseState};
use crate::fauna::visual::{FaunaVisualKind, ASH_SPIDER_ENTITY_KIND};
use crate::npc::brain::WanderState;
use crate::npc::brain_spider::spider_thinker;
use crate::npc::hunger::Hunger;
use crate::npc::lifecycle::{npc_runtime_bundle, NpcArchetype};
use crate::npc::lod::NpcLodTier;
use crate::npc::movement::{MovementCapabilities, MovementController, MovementCooldowns};
use crate::npc::navigator::Navigator;
use crate::npc::patrol::NpcPatrol;
use crate::npc::spawn::{NpcBlackboard, NpcCombatLoadout, NpcMarker, NpcMeleeArchetype};

/// 生成拟态灰烬蛛 NPC。
///
/// - `home_zone`：蛛归属 zone 名（Disguised qi 吸收 + Retreat 方向参考）
/// - `spawn_position`：出生世界坐标
/// - `patrol_target`：闲逛巡逻目标（Disguised 期不移动，Retreat 期使用）
///
/// # Components
///
/// | Component | 说明 |
/// |-----------|------|
/// | `NpcMarker` | NPC 标记 |
/// | `FaunaTag(Spider)` | 掉落 / 战斗 / 音效路由 |
/// | `FaunaVisualKind::AshSpider` | 视觉 shell（GeckoLib 渲染） |
/// | `SpiderDisguiseState::Disguised` | 初始三态 |
/// | `MimicSpiderBlackboard` | drained_qi / home_zone / home_pos |
/// | `NameVisible(false)` | nameplate 隐藏（伪装为灰烬方块） |
/// | `NpcBlackboard` | 玩家感知缓存（供 big-brain 使用） |
/// | `spider_thinker()` | P1 大脑（Ambush/Retreat/Chase/Melee） |
pub fn spawn_ash_spider_npc_at(
    commands: &mut Commands,
    layer: Entity,
    home_zone: &str,
    spawn_position: DVec3,
    patrol_target: DVec3,
) -> Entity {
    let loadout = NpcCombatLoadout::new(
        NpcMeleeArchetype::Brawler,
        MovementCapabilities {
            can_sprint: true,
            can_dash: false,
        },
    );

    let spider_blackboard = MimicSpiderBlackboard::new(home_zone, spawn_position);

    let entity = commands
        .spawn(MarkerEntityBundle {
            kind: ASH_SPIDER_ENTITY_KIND,
            layer: EntityLayerId(layer),
            position: Position::new([spawn_position.x, spawn_position.y, spawn_position.z]),
            // NameVisible(false) — 伪装期无 nameplate；P2 SpiderDisguiseHandler 负责 client 渲染切换
            entity_name_visible: NameVisible(false),
            ..Default::default()
        })
        .insert((
            Transform::from_xyz(
                spawn_position.x as f32,
                spawn_position.y as f32,
                spawn_position.z as f32,
            ),
            GlobalTransform::default(),
            NpcMarker,
            NpcBlackboard::default(),
            loadout.clone(),
            loadout.melee_archetype,
            loadout.melee_profile(),
            NpcArchetype::Beast,
            FaunaTag::new(BeastKind::Spider),
            FaunaVisualKind::AshSpider,
            NpcLodTier::Dormant,
            // 拟态蛛三态
            SpiderDisguiseState::default(), // Disguised
            spider_blackboard,
        ))
        .id();

    commands.entity(entity).insert((
        Navigator::new(),
        MovementController::new(),
        loadout.movement_capabilities,
        MovementCooldowns::default(),
        NpcPatrol::new(home_zone, patrol_target),
    ));

    commands.entity(entity).insert((
        Hunger::default(),
        WanderState::default(),
        spider_thinker(),
        npc_runtime_bundle(entity, NpcArchetype::Beast),
    ));

    entity
}

#[cfg(test)]
mod tests {
    use super::*;
    use valence::testing::ScenarioSingleClient;

    #[test]
    fn spawn_ash_spider_npc_attaches_fauna_tag_spider() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(5.0, 64.0, 5.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaTag>(spider).map(|t| t.beast_kind),
            Some(BeastKind::Spider),
            "生成蛛必须附加 FaunaTag(Spider)，用于掉落路由"
        );
    }

    #[test]
    fn spawn_ash_spider_npc_initial_state_is_disguised() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(5.0, 64.0, 5.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<SpiderDisguiseState>(spider),
            Some(&SpiderDisguiseState::Disguised),
            "蛛出生初始状态必须是 Disguised（伪装）"
        );
    }

    #[test]
    fn spawn_ash_spider_npc_nameplate_hidden() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(5.0, 64.0, 5.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<NameVisible>(spider),
            Some(&NameVisible(false)),
            "蛛生成时 nameplate 必须隐藏（NameVisible(false)），伪装效果依赖此"
        );
    }

    #[test]
    fn spawn_ash_spider_npc_blackboard_home_zone_set() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "test_zone",
            DVec3::new(10.0, 64.0, 10.0),
            DVec3::new(15.0, 64.0, 15.0),
        );
        app.world_mut().flush();

        let blackboard = app
            .world()
            .get::<MimicSpiderBlackboard>(spider)
            .expect("MimicSpiderBlackboard 必须附加");
        assert_eq!(
            blackboard.home_zone, "test_zone",
            "blackboard.home_zone 应为传入的 zone 名（期望 test_zone，实际 {}）",
            blackboard.home_zone
        );
    }

    #[test]
    fn spawn_ash_spider_npc_blackboard_home_pos_matches_spawn() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spawn_pos = DVec3::new(7.0, 64.0, 3.0);
        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            spawn_pos,
            DVec3::new(10.0, 64.0, 10.0),
        );
        app.world_mut().flush();

        let blackboard = app
            .world()
            .get::<MimicSpiderBlackboard>(spider)
            .expect("MimicSpiderBlackboard 必须附加");
        assert_eq!(
            blackboard.home_pos, spawn_pos,
            "home_pos 应与出生坐标一致（期望 {spawn_pos:?}，实际 {:?}）",
            blackboard.home_pos
        );
    }

    #[test]
    fn spawn_ash_spider_npc_visual_kind_is_ash_spider() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(5.0, 64.0, 5.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<FaunaVisualKind>(spider),
            Some(&FaunaVisualKind::AshSpider),
            "视觉类型必须是 AshSpider（GeckoLib 渲染使用）"
        );
    }

    #[test]
    fn spawn_ash_spider_npc_uses_ash_spider_entity_kind() {
        let scenario = ScenarioSingleClient::new();
        let layer = scenario.layer;
        let mut app = scenario.app;

        let spider = spawn_ash_spider_npc_at(
            &mut app.world_mut().commands(),
            layer,
            "spawn",
            DVec3::new(0.0, 64.0, 0.0),
            DVec3::new(5.0, 64.0, 5.0),
        );
        app.world_mut().flush();

        assert_eq!(
            app.world().get::<valence::prelude::EntityKind>(spider),
            Some(&ASH_SPIDER_ENTITY_KIND),
            "EntityKind 应为 ASH_SPIDER_ENTITY_KIND（协议层 ID 127）"
        );
    }
}
