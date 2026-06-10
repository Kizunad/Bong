//! plan-entity-model-v1 — server ↔ Fabric custom entity model contract.
//!
//! The client registers `bong:*` entity types after whale (`raw_id=125`) and
//! fauna (`raw_id=126..=145`). This module is the server-side mirror: existing
//! gameplay components keep owning logic, while this layer spawns lightweight
//! marker entities with the custom `EntityKind` ids (`146..=159`) that the
//! Fabric renderer consumes.
//!
//! `146..=148`: plan-supply-coffin-v1 — 巨剑沧海 物资棺 (Common/Rare/Precious).
//! `149..=159`: plan-entity-model-v1 / tsy-container entries.
//! `160..=163`: plan-coffin-tiers-v1 — 延寿棺四档 (Mundane/Jade/Stone/Bronze)，
//!              紧随 STONE_CASKET=159 连号（与客户端 BongEntityModelKind 同一注册循环）。
//! `164`:       Baolongwang BOSS（见 `fauna::visual::BAOLONGWANG_ENTITY_KIND`），
//!              客户端独立 bootstrap 在四档之后注册，故取最高号 164。
//! `165`:       Workbench。

use std::collections::{HashMap, HashSet};

use bevy_transform::components::{GlobalTransform, Transform};
use valence::entity::entity::NoGravity;
use valence::entity::marker::MarkerEntityBundle;
use valence::entity::tracked_data::TrackedData;
use valence::prelude::{
    apply_deferred, bevy_ecs, App, Commands, Component, DVec3, DetectChanges, Entity, EntityKind,
    EntityLayerId, IntoSystemConfigs, Look, Position, Query, Ref, Res, ResMut, Resource, Update,
};
use valence::protocol::VarInt;

use crate::alchemy::furnace::AlchemyFurnace;
use crate::forge::station::WeaponForgeStation;
use crate::lingtian::plot::LingtianPlot;
use crate::social::components::SpiritNiche;
use crate::world::dimension::DimensionLayers;
use crate::world::rift_portal::{RiftKind, RiftPortal};
use crate::world::spirit_eye::SpiritEyeRegistry;
use crate::world::tsy_container::{ContainerKind, LootContainer};
use crate::zhenfa::{ZhenfaAnchor, ZhenfaRegistry};

// plan-supply-coffin-v1 — 巨剑沧海三档物资棺。客户端 BongEntityModelKind 同步注册。
pub const COFFIN_COMMON_ENTITY_KIND: EntityKind = EntityKind::new(146);
pub const COFFIN_RARE_ENTITY_KIND: EntityKind = EntityKind::new(147);
pub const COFFIN_PRECIOUS_ENTITY_KIND: EntityKind = EntityKind::new(148);
pub const SPIRIT_NICHE_ENTITY_KIND: EntityKind = EntityKind::new(149);
pub const SPIRIT_EYE_ENTITY_KIND: EntityKind = EntityKind::new(150);
pub const RIFT_PORTAL_ENTITY_KIND: EntityKind = EntityKind::new(151);
pub const FORGE_STATION_ENTITY_KIND: EntityKind = EntityKind::new(152);
pub const ALCHEMY_FURNACE_ENTITY_KIND: EntityKind = EntityKind::new(153);
pub const FORMATION_CORE_ENTITY_KIND: EntityKind = EntityKind::new(154);
pub const LINGTIAN_PLOT_ENTITY_KIND: EntityKind = EntityKind::new(155);
pub const DRY_CORPSE_ENTITY_KIND: EntityKind = EntityKind::new(156);
pub const BONE_SKELETON_ENTITY_KIND: EntityKind = EntityKind::new(157);
pub const STORAGE_POUCH_ENTITY_KIND: EntityKind = EntityKind::new(158);
pub const STONE_CASKET_ENTITY_KIND: EntityKind = EntityKind::new(159);

// plan-coffin-tiers-v1 P2 — 延寿棺四档建模实体。客户端 BongEntityModelKind 同步注册
// （raw_id 1:1 对齐）。紧随 STONE_CASKET=159 连号 160..=163；Baolongwang BOSS 在四档
// 之后注册，取 164。（修正：早期文档把 160 留给 boss，但客户端把四档并入同一注册
// 循环，无法在中间为 boss 插空，故四档连号、boss 取最高号。）
pub const COFFIN_MUNDANE_ENTITY_KIND: EntityKind = EntityKind::new(160);
pub const COFFIN_JADE_ENTITY_KIND: EntityKind = EntityKind::new(161);
pub const COFFIN_STONE_ENTITY_KIND: EntityKind = EntityKind::new(162);
pub const COFFIN_BRONZE_ENTITY_KIND: EntityKind = EntityKind::new(163);
pub const WORKBENCH_ENTITY_KIND: EntityKind = EntityKind::new(165);

const BONG_VISUAL_STATE_INDEX: u8 = 8;
const TRACKED_DATA_TYPE_INTEGER: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BongVisualKind {
    SpiritNiche,
    SpiritEye,
    RiftPortal,
    ForgeStation,
    AlchemyFurnace,
    FormationCore,
    LingtianPlot,
    DryCorpse,
    BoneSkeleton,
    StoragePouch,
    StoneCasket,
    // plan-supply-coffin-v1 — 三档物资棺，作为渲染壳。
    // server 端的 spawn / despawn 由 supply_coffin::refresh 驱动；视觉 state
    // 当前只用 0 (intact)，opening 状态贴图待后续视觉 polish PR 接入。
    CoffinCommon,
    CoffinRare,
    CoffinPrecious,
    // plan-coffin-tiers-v1 P2 — 延寿棺四档建模实体（替换旧双 CHEST 占位）。
    // spawn / despawn 由 coffin::handle_coffin_place_requests / breaks / menu_reclaim
    // 命令式驱动（CoffinRegistry 是 source-of-truth，非 ECS 组件同步）；
    // 视觉 state 暂用 0 (intact)，掀盖/在棺 state 待 P3 接入。
    CoffinMundane,
    CoffinJade,
    CoffinStone,
    CoffinBronze,
    Workbench,
}

impl BongVisualKind {
    pub const fn entity_kind(self) -> EntityKind {
        match self {
            Self::SpiritNiche => SPIRIT_NICHE_ENTITY_KIND,
            Self::SpiritEye => SPIRIT_EYE_ENTITY_KIND,
            Self::RiftPortal => RIFT_PORTAL_ENTITY_KIND,
            Self::ForgeStation => FORGE_STATION_ENTITY_KIND,
            Self::AlchemyFurnace => ALCHEMY_FURNACE_ENTITY_KIND,
            Self::FormationCore => FORMATION_CORE_ENTITY_KIND,
            Self::LingtianPlot => LINGTIAN_PLOT_ENTITY_KIND,
            Self::DryCorpse => DRY_CORPSE_ENTITY_KIND,
            Self::BoneSkeleton => BONE_SKELETON_ENTITY_KIND,
            Self::StoragePouch => STORAGE_POUCH_ENTITY_KIND,
            Self::StoneCasket => STONE_CASKET_ENTITY_KIND,
            Self::CoffinCommon => COFFIN_COMMON_ENTITY_KIND,
            Self::CoffinRare => COFFIN_RARE_ENTITY_KIND,
            Self::CoffinPrecious => COFFIN_PRECIOUS_ENTITY_KIND,
            Self::CoffinMundane => COFFIN_MUNDANE_ENTITY_KIND,
            Self::CoffinJade => COFFIN_JADE_ENTITY_KIND,
            Self::CoffinStone => COFFIN_STONE_ENTITY_KIND,
            Self::CoffinBronze => COFFIN_BRONZE_ENTITY_KIND,
            Self::Workbench => WORKBENCH_ENTITY_KIND,
        }
    }
}

impl crate::coffin::CoffinGrade {
    /// 延寿棺档级 → `BongVisualKind` 渲染壳映射（plan-coffin-tiers-v1 P2）。
    /// 仿 `SupplyCoffinGrade::visual_kind`，让 coffin 模块放置时按档选 marker。
    pub const fn visual_kind(self) -> BongVisualKind {
        match self {
            Self::Mundane => BongVisualKind::CoffinMundane,
            Self::Jade => BongVisualKind::CoffinJade,
            Self::Stone => BongVisualKind::CoffinStone,
            Self::Bronze => BongVisualKind::CoffinBronze,
        }
    }
}

impl crate::supply_coffin::SupplyCoffinGrade {
    /// Maps `SupplyCoffinGrade` → `BongVisualKind` for entity spawning.
    /// plan-supply-coffin-v1 P1.1 — server-side render kind selection.
    pub const fn visual_kind(self) -> BongVisualKind {
        match self {
            Self::Common => BongVisualKind::CoffinCommon,
            Self::Rare => BongVisualKind::CoffinRare,
            Self::Precious => BongVisualKind::CoffinPrecious,
        }
    }
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct BongVisualState(pub u8);

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct BongVisualAttachment {
    pub visual: Entity,
}

#[derive(Debug, Clone, Copy, Component, PartialEq, Eq)]
pub struct BongVisualEntity {
    pub kind: BongVisualKind,
    pub source: Option<Entity>,
}

#[derive(Debug, Default, Resource)]
struct SpiritEyeVisuals {
    by_eye_id: HashMap<String, Entity>,
}

type RiftPortalVisualQueryItem<'a> = (
    Entity,
    Ref<'a, RiftPortal>,
    Ref<'a, Position>,
    &'a EntityLayerId,
    Option<&'a BongVisualAttachment>,
);
type RiftPortalVisualQuery<'w, 's> = Query<'w, 's, RiftPortalVisualQueryItem<'static>>;

type ZhenfaAnchorVisualQueryItem<'a> = (
    Entity,
    Ref<'a, ZhenfaAnchor>,
    Ref<'a, Position>,
    Option<&'a BongVisualAttachment>,
);
type ZhenfaAnchorVisualQuery<'w, 's> = Query<'w, 's, ZhenfaAnchorVisualQueryItem<'static>>;

type TsyContainerVisualQueryItem<'a> = (
    Entity,
    Ref<'a, LootContainer>,
    Ref<'a, Position>,
    &'a EntityLayerId,
    Option<&'a BongVisualAttachment>,
);
type TsyContainerVisualQuery<'w, 's> = Query<'w, 's, TsyContainerVisualQueryItem<'static>>;

pub fn register(app: &mut App) {
    tracing::info!("[bong][entity-model] registering server visual entity bridge");
    app.init_resource::<SpiritEyeVisuals>();
    app.add_systems(
        Update,
        (
            (
                sync_spirit_eye_visuals,
                sync_spirit_niche_visuals,
                sync_rift_portal_visuals,
                sync_forge_station_visuals,
                sync_alchemy_furnace_visuals,
                sync_zhenfa_anchor_visuals,
                sync_lingtian_plot_visuals,
                sync_tsy_container_visuals,
                cleanup_orphan_visual_entities,
            ),
            apply_deferred,
            sync_bong_visual_state_metadata,
        )
            .chain(),
    );
}

/// 在指定 layer 上 spawn 一个自定义 marker 渲染实体，返回其 `Entity` id。
///
/// 这是 server 视觉桥的底层 primitive：内部 sync 系统通过 `upsert_attached_visual`
/// 复用它，外部模块（如 plan-coffin-tiers-v1 的延寿棺，`CoffinRegistry` 命令式驱动）
/// 直接调用并自行记录返回的 `Entity` id 用于后续 despawn / 重建。
///
/// `source` = 关联的 gameplay 源实体（用于 `cleanup_orphan_visual_entities` 孤儿回收）；
/// `CoffinRegistry` 非 ECS 组件，故传 `None`，despawn 由 coffin 模块自管。
pub fn spawn_visual_marker(
    commands: &mut Commands,
    layer: Entity,
    source: Option<Entity>,
    kind: BongVisualKind,
    pos: DVec3,
    visual_state: u8,
) -> Entity {
    commands
        .spawn((
            MarkerEntityBundle {
                kind: kind.entity_kind(),
                layer: EntityLayerId(layer),
                position: Position::new([pos.x, pos.y, pos.z]),
                entity_no_gravity: NoGravity(true),
                look: Look::new(0.0, 0.0),
                ..Default::default()
            },
            Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
            GlobalTransform::default(),
            BongVisualEntity { kind, source },
            BongVisualState(visual_state),
        ))
        .id()
}

fn upsert_attached_visual(
    commands: &mut Commands,
    source: Entity,
    attachment: Option<&BongVisualAttachment>,
    layer: Entity,
    kind: BongVisualKind,
    pos: DVec3,
    visual_state: u8,
) {
    if let Some(attachment) = attachment {
        commands.entity(attachment.visual).insert((
            EntityLayerId(layer),
            Position::new([pos.x, pos.y, pos.z]),
            Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
            BongVisualState(visual_state),
        ));
        return;
    }

    let visual = spawn_visual_marker(commands, layer, Some(source), kind, pos, visual_state);
    commands
        .entity(source)
        .insert(BongVisualAttachment { visual });
}

fn sync_spirit_eye_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    registry: Option<Res<SpiritEyeRegistry>>,
    mut visuals: ResMut<SpiritEyeVisuals>,
) {
    let (Some(layers), Some(registry)) = (layers, registry) else {
        return;
    };

    let mut live_ids = HashSet::with_capacity(registry.eyes.len());
    for eye in &registry.eyes {
        live_ids.insert(eye.id.0.clone());
        let layer = layers.entity_for(eye.dimension);
        let pos = DVec3::new(eye.pos[0], eye.pos[1], eye.pos[2]);
        let state = spirit_eye_visual_state(eye.qi_concentration);
        if let Some(visual) = visuals.by_eye_id.get(&eye.id.0).copied() {
            commands.entity(visual).insert((
                EntityLayerId(layer),
                Position::new([pos.x, pos.y, pos.z]),
                Transform::from_xyz(pos.x as f32, pos.y as f32, pos.z as f32),
                BongVisualState(state),
            ));
        } else {
            let visual = spawn_visual_marker(
                &mut commands,
                layer,
                None,
                BongVisualKind::SpiritEye,
                pos,
                state,
            );
            visuals.by_eye_id.insert(eye.id.0.clone(), visual);
        }
    }

    let mut stale_visuals = Vec::new();
    visuals.by_eye_id.retain(|eye_id, visual| {
        if live_ids.contains(eye_id) {
            true
        } else {
            stale_visuals.push(*visual);
            false
        }
    });
    for visual in stale_visuals {
        commands.entity(visual).despawn();
    }
}

fn sync_spirit_niche_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    niches: Query<(Entity, Ref<SpiritNiche>, Option<&BongVisualAttachment>)>,
) {
    let Some(layers) = layers else {
        return;
    };
    for (source, niche, attachment) in &niches {
        if attachment.is_some() && !niche.is_changed() {
            continue;
        }
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layers.overworld,
            BongVisualKind::SpiritNiche,
            block_center(niche.pos, 0.0),
            spirit_niche_visual_state(&niche),
        );
    }
}

fn sync_rift_portal_visuals(mut commands: Commands, portals: RiftPortalVisualQuery<'_, '_>) {
    for (source, portal, pos, layer, attachment) in &portals {
        if attachment.is_some() && !portal.is_changed() && !pos.is_changed() {
            continue;
        }
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layer.0,
            BongVisualKind::RiftPortal,
            pos.get(),
            rift_portal_visual_state(portal.kind),
        );
    }
}

fn sync_forge_station_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    stations: Query<(
        Entity,
        Ref<WeaponForgeStation>,
        Option<&BongVisualAttachment>,
    )>,
) {
    let Some(layers) = layers else {
        return;
    };
    for (source, station, attachment) in &stations {
        if attachment.is_some() && !station.is_changed() {
            continue;
        }
        let Some(pos) = station.block_pos() else {
            continue;
        };
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layers.overworld,
            BongVisualKind::ForgeStation,
            block_pos_center(pos, 0.0),
            u8::from(station.session.is_some()),
        );
    }
}

fn sync_alchemy_furnace_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    furnaces: Query<(Entity, Ref<AlchemyFurnace>, Option<&BongVisualAttachment>)>,
) {
    let Some(layers) = layers else {
        return;
    };
    for (source, furnace, attachment) in &furnaces {
        if attachment.is_some() && !furnace.is_changed() {
            continue;
        }
        let Some(pos) = furnace.block_pos() else {
            continue;
        };
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layers.overworld,
            BongVisualKind::AlchemyFurnace,
            block_pos_center(pos, 0.0),
            u8::from(furnace.is_busy()),
        );
    }
}

fn sync_zhenfa_anchor_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    registry: Option<Res<ZhenfaRegistry>>,
    anchors: ZhenfaAnchorVisualQuery<'_, '_>,
) {
    let Some(layers) = layers else {
        return;
    };
    let Some(registry) = registry else {
        return;
    };
    for (source, anchor, pos, attachment) in &anchors {
        if attachment.is_some() && !anchor.is_changed() && !pos.is_changed() {
            continue;
        }
        let visual_state = registry.anchor_visual_state(&anchor);
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layers.overworld,
            BongVisualKind::FormationCore,
            pos.get(),
            visual_state,
        );
    }
}

fn sync_lingtian_plot_visuals(
    mut commands: Commands,
    layers: Option<Res<DimensionLayers>>,
    plots: Query<(Entity, Ref<LingtianPlot>, Option<&BongVisualAttachment>)>,
) {
    let Some(layers) = layers else {
        return;
    };
    for (source, plot, attachment) in &plots {
        if attachment.is_some() && !plot.is_changed() {
            continue;
        }
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layers.overworld,
            BongVisualKind::LingtianPlot,
            block_pos_center(plot.pos, 0.0),
            lingtian_plot_visual_state(&plot),
        );
    }
}

fn sync_tsy_container_visuals(mut commands: Commands, containers: TsyContainerVisualQuery<'_, '_>) {
    for (source, container, pos, layer, attachment) in &containers {
        if attachment.is_some() && !container.is_changed() && !pos.is_changed() {
            continue;
        }
        upsert_attached_visual(
            &mut commands,
            source,
            attachment,
            layer.0,
            container_visual_kind(container.kind),
            pos.get(),
            container_visual_state(&container),
        );
    }
}

fn cleanup_orphan_visual_entities(
    mut commands: Commands,
    visuals: Query<(Entity, &BongVisualEntity)>,
    sources: Query<()>,
) {
    for (visual, marker) in &visuals {
        let Some(source) = marker.source else {
            continue;
        };
        if sources.get(source).is_err() {
            commands.entity(visual).despawn();
        }
    }
}

fn sync_bong_visual_state_metadata(
    mut visuals: Query<
        (&BongVisualState, &mut TrackedData),
        bevy_ecs::query::Changed<BongVisualState>,
    >,
) {
    for (state, mut tracked_data) in &mut visuals {
        let value = VarInt(i32::from(state.0));
        tracked_data.insert_init_value(BONG_VISUAL_STATE_INDEX, TRACKED_DATA_TYPE_INTEGER, value);
        tracked_data.append_update_value(BONG_VISUAL_STATE_INDEX, TRACKED_DATA_TYPE_INTEGER, value);
    }
}

fn spirit_niche_visual_state(niche: &SpiritNiche) -> u8 {
    if niche.revealed {
        2
    } else {
        1
    }
}

fn spirit_eye_visual_state(qi_concentration: f64) -> u8 {
    if qi_concentration >= 0.95 {
        2
    } else if qi_concentration >= 0.70 {
        1
    } else {
        0
    }
}

fn rift_portal_visual_state(kind: RiftKind) -> u8 {
    match kind {
        RiftKind::MainRift => 0,
        RiftKind::DeepRift => 1,
        RiftKind::CollapseTear => 2,
    }
}

fn lingtian_plot_visual_state(plot: &LingtianPlot) -> u8 {
    if let Some(crop) = &plot.crop {
        if crop.is_ripe() {
            3
        } else {
            2
        }
    } else if plot.is_barren() {
        0
    } else {
        1
    }
}

fn container_visual_kind(kind: ContainerKind) -> BongVisualKind {
    match kind {
        ContainerKind::DryCorpse => BongVisualKind::DryCorpse,
        ContainerKind::Skeleton => BongVisualKind::BoneSkeleton,
        ContainerKind::StoragePouch => BongVisualKind::StoragePouch,
        ContainerKind::StoneCasket | ContainerKind::RelicCore => BongVisualKind::StoneCasket,
        // SurfaceStash 用 DryCorpse 视觉（plan-onboarding P0.1 fallback）
        ContainerKind::SurfaceStash => BongVisualKind::DryCorpse,
    }
}

fn container_visual_state(container: &LootContainer) -> u8 {
    if container.depleted {
        2
    } else if container.searched_by.is_some() {
        1
    } else {
        0
    }
}

fn block_center(pos: [i32; 3], y_offset: f64) -> DVec3 {
    DVec3::new(
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + y_offset,
        f64::from(pos[2]) + 0.5,
    )
}

fn block_pos_center(pos: valence::prelude::BlockPos, y_offset: f64) -> DVec3 {
    DVec3::new(
        f64::from(pos.x) + 0.5,
        f64::from(pos.y) + y_offset,
        f64::from(pos.z) + 0.5,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::dimension::DimensionKind;
    use crate::world::rift_portal::{PortalDirection, RiftPortal};
    use crate::world::tsy::DimensionAnchor;
    use valence::prelude::{App, BlockPos};

    #[test]
    fn entity_kind_ids_stay_aligned_with_client_raw_ids() {
        let ids = [
            COFFIN_COMMON_ENTITY_KIND,
            COFFIN_RARE_ENTITY_KIND,
            COFFIN_PRECIOUS_ENTITY_KIND,
            SPIRIT_NICHE_ENTITY_KIND,
            SPIRIT_EYE_ENTITY_KIND,
            RIFT_PORTAL_ENTITY_KIND,
            FORGE_STATION_ENTITY_KIND,
            ALCHEMY_FURNACE_ENTITY_KIND,
            FORMATION_CORE_ENTITY_KIND,
            LINGTIAN_PLOT_ENTITY_KIND,
            DRY_CORPSE_ENTITY_KIND,
            BONE_SKELETON_ENTITY_KIND,
            STORAGE_POUCH_ENTITY_KIND,
            STONE_CASKET_ENTITY_KIND,
            // plan-coffin-tiers-v1 P2 — 延寿棺四档，连号 160..=163（Baolongwang BOSS=164 在其后）。
            COFFIN_MUNDANE_ENTITY_KIND,
            COFFIN_JADE_ENTITY_KIND,
            COFFIN_STONE_ENTITY_KIND,
            COFFIN_BRONZE_ENTITY_KIND,
            WORKBENCH_ENTITY_KIND,
        ]
        .map(|kind| kind.get());

        assert_eq!(
            ids,
            [
                146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161,
                162, 163, 165
            ],
            "entity raw_id contract drifted: client BongEntityModelKind must stay 1:1; \
             延寿棺四档连号 160-163，Baolongwang=164，Workbench=165"
        );
    }

    #[test]
    fn workbench_visual_kind_maps_to_raw_id_165() {
        assert_eq!(
            BongVisualKind::Workbench.entity_kind(),
            WORKBENCH_ENTITY_KIND,
            "Workbench visual entity must keep raw_id 165 for client renderer parity"
        );
    }

    #[test]
    fn coffin_grade_maps_to_visual_kind() {
        use crate::coffin::CoffinGrade;
        // 四档延寿棺 grade → BongVisualKind → EntityKind 全链对齐（client P3 渲染绑定依赖）。
        assert_eq!(
            CoffinGrade::Mundane.visual_kind().entity_kind(),
            COFFIN_MUNDANE_ENTITY_KIND,
            "Mundane 应映射到 raw_id 160"
        );
        assert_eq!(
            CoffinGrade::Jade.visual_kind().entity_kind(),
            COFFIN_JADE_ENTITY_KIND,
            "Jade 应映射到 raw_id 161"
        );
        assert_eq!(
            CoffinGrade::Stone.visual_kind().entity_kind(),
            COFFIN_STONE_ENTITY_KIND,
            "Stone 应映射到 raw_id 162"
        );
        assert_eq!(
            CoffinGrade::Bronze.visual_kind().entity_kind(),
            COFFIN_BRONZE_ENTITY_KIND,
            "Bronze 应映射到 raw_id 163"
        );
    }

    #[test]
    fn supply_coffin_grade_maps_to_visual_kind() {
        use crate::supply_coffin::SupplyCoffinGrade;
        assert_eq!(
            SupplyCoffinGrade::Common.visual_kind().entity_kind(),
            COFFIN_COMMON_ENTITY_KIND
        );
        assert_eq!(
            SupplyCoffinGrade::Rare.visual_kind().entity_kind(),
            COFFIN_RARE_ENTITY_KIND
        );
        assert_eq!(
            SupplyCoffinGrade::Precious.visual_kind().entity_kind(),
            COFFIN_PRECIOUS_ENTITY_KIND
        );
    }

    #[test]
    fn rift_portal_spawns_custom_visual_marker_with_metadata_state() {
        let mut app = App::new();
        app.add_systems(
            Update,
            (
                sync_rift_portal_visuals,
                apply_deferred,
                sync_bong_visual_state_metadata,
            )
                .chain(),
        );
        let layer = app.world_mut().spawn_empty().id();
        let source = app
            .world_mut()
            .spawn((
                RiftPortal::new(
                    "family".to_string(),
                    DimensionAnchor {
                        dimension: DimensionKind::Tsy,
                        pos: DVec3::ZERO,
                    },
                    2.0,
                    PortalDirection::Entry,
                    RiftKind::DeepRift,
                ),
                Position::new([10.0, 64.0, -4.0]),
                EntityLayerId(layer),
            ))
            .id();

        app.update();

        let attachment = *app
            .world()
            .get::<BongVisualAttachment>(source)
            .expect("source should point at visual entity");
        let visual = attachment.visual;
        assert_eq!(
            app.world().get::<EntityKind>(visual).copied(),
            Some(RIFT_PORTAL_ENTITY_KIND)
        );
        assert_eq!(
            app.world().get::<BongVisualState>(visual).copied(),
            Some(BongVisualState(1))
        );
        assert_eq!(
            app.world()
                .get::<TrackedData>(visual)
                .and_then(TrackedData::init_data),
            Some([BONG_VISUAL_STATE_INDEX, TRACKED_DATA_TYPE_INTEGER, 1, 0xff].as_slice()),
            "custom VisualState metadata must use DataTracker index 8, INTEGER type, and VarInt value for Fabric renderer"
        );
    }

    #[test]
    fn source_component_state_maps_to_visual_state() {
        assert_eq!(spirit_eye_visual_state(0.5), 0);
        assert_eq!(spirit_eye_visual_state(0.7), 1);
        assert_eq!(spirit_eye_visual_state(1.0), 2);
        assert_eq!(rift_portal_visual_state(RiftKind::CollapseTear), 2);

        let mut plot = LingtianPlot::new(BlockPos::new(1, 64, 1), None);
        assert_eq!(lingtian_plot_visual_state(&plot), 1);
        plot.harvest_count = crate::lingtian::plot::N_RENEW;
        assert_eq!(lingtian_plot_visual_state(&plot), 0);
    }
}
