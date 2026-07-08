//! plan-surface-stash-runtime-scatter-gap-v1 §P2 — 散修遗缴 runtime scatter
//! 生产路径存在性集成测试。
//!
//! 仿 `tsy_lifecycle_integration_test.rs` 的"构造 App + 挂系统 + app.update() +
//! 断言世界状态"集成测模式：本文件不测纯函数 `scatter_surface_stashes` 的返回值
//! （那是 `poi_novice.rs::tests` 的职责），而是验证 `poi_novice::register(app)`
//! 真实把 Startup 系统接进 `App` 调度、产出 registry site + 世界实体两者都存在
//! 且互相一致——这正是本 plan 存在的原因：此前只有纯函数层，从未有任何测试锁住
//! "纯函数被 Startup 调度调用"这一环（§8.1 #3 决议）。

#[cfg(test)]
mod tests {
    use valence::prelude::{App, EntityLayerId, Position};

    use crate::combat::CombatClock;
    use crate::world::dimension::DimensionLayers;
    use crate::world::poi_novice::{self, PoiNoviceKind, PoiNoviceRegistry};
    use crate::world::poi_respawn_tick::{self, PoiRespawnStore, SURFACE_STASH_RESPAWN_TICKS};
    use crate::world::terrain::{TerrainProvider, TerrainProviders};
    use crate::world::tsy_container::{ContainerKind, LootContainer};

    /// 构造最小 App：mock `TerrainProviders`/`DimensionLayers` + 真实
    /// `poi_novice::register(app)`（不手搓子集系统，走生产注册路径）。
    fn make_app() -> App {
        let mut app = App::new();
        let overworld = app.world_mut().spawn_empty().id();
        let tsy = app.world_mut().spawn_empty().id();
        app.insert_resource(DimensionLayers { overworld, tsy });
        app.insert_resource(TerrainProviders {
            overworld: TerrainProvider::empty_for_tests(),
            tsy: None,
        });
        poi_novice::register(&mut app);
        app
    }

    /// 首刷回归（§P2）：`poi_novice::register(app)` 后跑一次 `app.update()`，
    /// 三层断言缺一不可——只测其中一层无法排除"注册表更新了但没生成实体"或
    /// "生成了实体但没写回注册表"两种半接线退化模式（§8.1 #3 决议 3）。
    #[test]
    fn scatter_wiring_produces_12_registry_sites_matching_world_entities() {
        let mut app = make_app();
        app.update();

        // 层 1：registry 里 SurfaceStash 站点恰好 12 个。
        let registry_sites: Vec<[f32; 3]> = {
            let registry = app.world().resource::<PoiNoviceRegistry>();
            registry
                .by_kind(PoiNoviceKind::SurfaceStash)
                .map(|site| site.pos_xyz)
                .collect()
        };
        assert_eq!(
            registry_sites.len(),
            12,
            "PoiNoviceRegistry 里 SurfaceStash 站点应恰好 12 个（scatter_and_spawn_surface_stashes \
             真实接进 Startup 调度），实际 {}",
            registry_sites.len()
        );

        // 层 2：世界里存在 12 个带 LootContainer{kind: SurfaceStash} + Position +
        // EntityLayerId 的实体。
        let world = app.world_mut();
        let mut query = world.query::<(&LootContainer, &Position, &EntityLayerId)>();
        let entity_positions: Vec<[f32; 3]> = query
            .iter(world)
            .filter(|(container, _, _)| container.kind == ContainerKind::SurfaceStash)
            .map(|(_, pos, _)| [pos.0.x as f32, pos.0.y as f32, pos.0.z as f32])
            .collect();
        assert_eq!(
            entity_positions.len(),
            12,
            "世界里应有恰好 12 个 LootContainer{{kind: SurfaceStash}}+Position+EntityLayerId 实体\
             （不能只更新 registry 不 spawn 实体），实际 {}",
            entity_positions.len()
        );

        // 层 3：每个 registry site 的 pos_xyz 与对应实体 Position 一致（排序后逐一比较，
        // 因为 registry 插入顺序与 query 迭代顺序不保证一致）。
        let mut sorted_registry = registry_sites;
        sorted_registry.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut sorted_entities = entity_positions;
        sorted_entities.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            sorted_registry, sorted_entities,
            "每个 PoiNoviceSite.pos_xyz 必须与对应 LootContainer 实体的 Position 一致\
             （生产链闭环，而不是注册表/实体各说各话的半接线）"
        );
    }

    /// 复活回归（§P2）：P1 接线后，SurfaceStash 站点必须真的能被
    /// `poi_respawn_tick::respawn_tick` 读到并纳入 `PoiRespawnStore`——纯函数层
    /// （`is_server_tick_ready`）早已被 `poi_respawn_tick.rs` 自身单测锁定，本条测试
    /// 补的是"注册表里真的有 SurfaceStash 站点可供 respawn 系统读取"这一环。
    #[test]
    fn scatter_wiring_surface_stash_sites_enter_respawn_store_after_update() {
        let mut app = make_app();
        app.insert_resource(CombatClock { tick: 0 });
        poi_respawn_tick::register(&mut app);

        app.update();

        let poi_id = {
            let registry = app.world().resource::<PoiNoviceRegistry>();
            registry
                .by_kind(PoiNoviceKind::SurfaceStash)
                .next()
                .expect("scatter wiring 应至少产出一个 SurfaceStash 站点")
                .id
                .clone()
        };

        let store = app.world().resource::<PoiRespawnStore>();
        let state = store.get(&poi_id).unwrap_or_else(|| {
            panic!(
                "respawn_tick 跑过一次 Update 后，PoiRespawnStore 应该已经 ensure_site 了 \
                 SurfaceStash 站点 {poi_id}，但没找到——说明 registry 与 respawn 系统之间仍是半接线"
            )
        });
        assert_eq!(state.kind, PoiNoviceKind::SurfaceStash);
        assert!(
            !state.is_server_tick_ready(SURFACE_STASH_RESPAWN_TICKS - 1),
            "SurfaceStash 在 {} ticks 时不应 ready",
            SURFACE_STASH_RESPAWN_TICKS - 1
        );
        assert!(
            state.is_server_tick_ready(SURFACE_STASH_RESPAWN_TICKS),
            "SurfaceStash 应在 SURFACE_STASH_RESPAWN_TICKS={} ticks 后 respawn ready",
            SURFACE_STASH_RESPAWN_TICKS
        );
    }
}
