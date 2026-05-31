//! plan-offscreen-war-v1 P3 交付物 3+4：克制式战场遗物的 **deferred-on-hydrate 物化**。
//!
//! 战死结算（combat.rs / mod.rs 交付物 1）emit 的 `PendingDormantRelicCreated` 已由
//! `persistence::persist_pending_dormant_relics_system`（交付物 2）落盘进 sqlite
//! `pending_dormant_relics`。本模块负责**玩家靠近时**把这些待物化遗物变成可拾取的地面
//! loot：
//!
//! 1. 检测哪些 zone 当前有玩家在场（[`zones_with_players`]）；
//! 2. 从 sqlite 读出这些 zone 的 pending relic（`load_pending_dormant_relics_for_zone`）；
//! 3. 用 deterministic 种子 `roll_loot(default_loot_for_archetype(archetype), loot_seed)`
//!    roll 出战利品（同一遗物每次掉同一批，可复现）；
//! 4. **零真元物化**（[`materialize_relic_loot`]）：每件 loot 的 `ItemInstance` 显式
//!    `spirit_quality = 0.0`，落进 `DroppedLootRegistry`（registry 一变 changed，
//!    `dropped_loot_sync_emit::emit_changed_dropped_loot_syncs` 自动下发给所有 client）；
//! 5. 物化完 `delete_pending_dormant_relic` 删掉该行，保证不二次物化；
//! 6. 发 VFX（`offscreen_relic_reveal` 地表贴花）+ audio（`offscreen_relic_reveal` 低沉
//!    levelup + 骨裂）+ zone narration（perception style，玩家感知"此地曾有厮杀"）。
//!
//! **为何不挂在 `hydrate_dormant_near_players_system` 里**（plan §141 原写的 hydrate 入口）：
//! 那个 system 的触发守卫在**无玩家在场时不触发**（player_positions 为空），纯 headless 真服
//! e2e（无 client）测不到。这里把"消费 pending relic → 物化地面 loot"做成独立 system +
//! 纯函数 [`materialize_relic_loot`]，既能在真服里随玩家靠近触发，又能用 Bevy App 单测直接
//! 驱动纯函数锁住零真元 / deterministic 契约（dossier 侦察B 推荐路径）。
//!
//! **防无 chunk 穿地丢失**：地面 loot 走 `DroppedLootRegistry` 纯数据（非 Bevy entity / 非
//! ItemEntity），不依赖 chunk 加载，对齐 dossier 的 `pending_daoxiang_spawns` deferred 范式
//! 与 memory `project_infinite_fall_chunk`——玩家来了才物化，物化即可见，不会穿地。
//!
//! **守恒红线（§10.1 #5 ④）**：物化路径**完全不碰** `WorldQiAccount` / ledger；每件 loot
//! `spirit_quality = 0.0`（骨堆 / 残卷只作叙事 / 材料，无真元）。败者残余真元早在战死结算时
//! 已由 `release_dormant_qi_to_zone` 守恒还给 zone，遗物是那之后用零真元快照创建的。

use valence::client::ClientMarker;
use valence::prelude::{App, DVec3, Events, Position, Query, Res, ResMut, Update, With};

use crate::inventory::{
    DroppedLootEntry, DroppedLootRegistry, InventoryInstanceIdAllocator, ItemInstance, ItemRegistry,
};
use crate::network::audio_event_emit::{AudioRecipient, PlaySoundRecipeRequest};
use crate::network::gameplay_vfx::{send_spawn, spawn_request};
use crate::network::vfx_event_emit::VfxEventRequest;
use crate::npc::lifecycle::NpcArchetype;
use crate::npc::loot::{default_loot_for_archetype, roll_loot};
use crate::persistence::{
    delete_pending_dormant_relic, load_pending_dormant_relics_for_zone, PendingDormantRelicRecord,
    PersistenceSettings,
};
use crate::player::gameplay::PendingGameplayNarrations;
use crate::schema::common::NarrationStyle;
use crate::world::dimension::{CurrentDimension, DimensionKind};
use crate::world::zone::ZoneRegistry;

/// `bong:vfx_event` ID：战场遗物被玩家发现时的地表贴花揭示（plan §145）。
/// client `OffscreenRelicRevealPlayer` 消费它，用 `BongGroundDecalParticle` 贴地。
pub const OFFSCREEN_RELIC_REVEAL_VFX: &str = "offscreen_relic_reveal";

/// `audio_recipe` ID：战场遗物揭示音效（plan §146，两层：低沉 levelup + 骨裂）。
pub const OFFSCREEN_RELIC_REVEAL_AUDIO: &str = "offscreen_relic_reveal";

/// 骨堆遗骸贴花色（灰白，plan §145）——对应"宗门信物"叙事变体（战死者遗骸为主）。
pub const RELIC_DECAL_COLOR_BONE: &str = "#B8AFA0";

/// 残卷遗骸贴花色（暗黄，plan §145）——对应"散修残经"叙事变体（半卷残经为主）。
pub const RELIC_DECAL_COLOR_SCROLL: &str = "#7A6A3C";

/// 按 `loot_seed` 奇偶选贴花色，**与 [`relic_reveal_narration`] 同源**——同一遗物的贴花颜色
/// 与叙事文案一致（偶=骨堆灰白 + 宗门信物；奇=残卷暗黄 + 散修残经），视觉与文字讲同一个故事。
fn relic_decal_color(loot_seed: u64) -> &'static str {
    if loot_seed & 1 == 0 {
        RELIC_DECAL_COLOR_BONE
    } else {
        RELIC_DECAL_COLOR_SCROLL
    }
}

/// 战场遗物 loot 的**零真元**常量——物化每件 `ItemInstance` 时显式写入 `spirit_quality`。
///
/// 这是 §10.1 #5 ④守恒红线的代码锚点：遗物绝不携带真元（`roll_loot` 产出的 `RolledLoot`
/// 不带 spirit_quality，这里**不读 template.spirit_quality_initial**，强制 0.0），骨堆 / 残卷
/// 仅作叙事 / 材料。提取为具名 const 让任何"误用 template 初始品质"的回归一眼可辨。
pub const RELIC_LOOT_SPIRIT_QUALITY: f64 = 0.0;

/// 单遗物物化出的地面 loot 件数上限——克制基调（末法遍地尸体违和），一具遗骸最多散落几件，
/// 不淹没世界 / 不刷爆 registry。`default_loot_for_archetype` 的表本就短，这里再兜底 clamp。
const MAX_LOOT_ENTRIES_PER_RELIC: usize = 6;

/// 把 loot 件按确定性微偏移散开在遗骸点周围（同 `tsy_hostile::jittered_drop_pos` 思路），
/// 避免多件 loot 完全重叠在一个坐标。纯整数 hash → [-0.5, 0.5] 偏移，确定性可复现。
fn jittered_relic_loot_pos(base: [f64; 3], loot_seed: u64, idx: u64) -> [f64; 3] {
    let mix = loot_seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(idx.wrapping_mul(0xBF58_476D_1CE4_E5B9));
    // 取两段位做 x / z 偏移，y 不抖（贴地）。
    let dx = ((mix >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5;
    let dz = ((mix >> 32) & 0xFFFF) as f64 / 65535.0 - 0.5;
    [base[0] + dx, base[1], base[2] + dz]
}

/// **纯函数**：把一条 sqlite pending relic 记录物化成一批**零真元**地面 loot 条目
/// （plan §141/§142/§143 交付物 3+4 核心）。
///
/// - 用 `record.loot_seed` deterministic roll `default_loot_for_archetype(archetype)`——同一遗骸
///   每次掉同一批战利品（可复现，真服 e2e 锁得住）；
/// - 每件 loot 的 `ItemInstance.spirit_quality` 强制 [`RELIC_LOOT_SPIRIT_QUALITY`]（=0.0，
///   **不读 template 初始品质**，守恒红线 §10.1 #5 ④）；
/// - `dimension` 由调用方从 zone 解析后传入（记录只存 zone + position）；
/// - 脏 archetype 串（旧 schema / 手写 sqlite）→ `NpcArchetype::from_str` 返 None → 返回空 vec
///   （显式跳过，绝不静默掉僵尸 loot）。
///
/// 返回 `Vec<DroppedLootEntry>`，由调用方 `registry.entries.insert` 落进 `DroppedLootRegistry`。
/// 纯函数零副作用（除 allocator 取 id），可被单测完全锁住。
pub fn materialize_relic_loot(
    record: &PendingDormantRelicRecord,
    dimension: DimensionKind,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Vec<DroppedLootEntry> {
    let Some(archetype) = NpcArchetype::from_str(&record.archetype) else {
        tracing::warn!(
            "[bong][npc] pending relic {} has unknown archetype `{}`; skipping materialization",
            record.relic_id,
            record.archetype
        );
        return Vec::new();
    };

    let table = default_loot_for_archetype(archetype);
    let rolled = roll_loot(&table, record.loot_seed);
    let base = [record.pos_x, record.pos_y, record.pos_z];
    let source_container_id = format!("offscreen_relic:{}:{}", record.zone, record.char_id);

    let mut entries = Vec::new();
    for (idx, roll) in rolled
        .into_iter()
        .take(MAX_LOOT_ENTRIES_PER_RELIC)
        .enumerate()
    {
        let Some(item) =
            relic_item_from_template(&roll.template_id, roll.stack, item_registry, allocator)
        else {
            // 模板缺失（roll 出一个未注册 template_id）→ 跳过该件，不中断其它件。
            continue;
        };
        let world_pos = jittered_relic_loot_pos(base, record.loot_seed, idx as u64);
        entries.push(DroppedLootEntry {
            instance_id: item.instance_id,
            source_container_id: source_container_id.clone(),
            source_row: 0,
            source_col: 0,
            world_pos,
            dimension,
            item,
        });
    }
    entries
}

/// 从 template 物化一件**零真元** `ItemInstance`（仿 `tsy_hostile::template_as_item`，但
/// `spirit_quality` 强制 [`RELIC_LOOT_SPIRIT_QUALITY`] 而非 `template.spirit_quality_initial`）。
/// 这是遗物零真元的物化点（§10.1 #5 ④ verify 修正——`RolledLoot` 不带 spirit_quality，必须在此
/// 显式覆写 0.0，否则会误继承 template 初始品质把真元凭空带进世界）。
fn relic_item_from_template(
    template_id: &str,
    count: u32,
    item_registry: &ItemRegistry,
    allocator: &mut InventoryInstanceIdAllocator,
) -> Option<ItemInstance> {
    let template = item_registry.get(template_id)?;
    let instance_id = allocator.next_id().ok()?;
    Some(ItemInstance {
        instance_id,
        template_id: template.id.clone(),
        display_name: template.display_name.clone(),
        grid_w: template.grid_w,
        grid_h: template.grid_h,
        weight: template.base_weight,
        rarity: template.rarity,
        description: template.description.clone(),
        stack_count: count,
        // 守恒红线：遗物零真元，**不读** template.spirit_quality_initial。
        spirit_quality: RELIC_LOOT_SPIRIT_QUALITY,
        durability: 1.0,
        freshness: None,
        mineral_id: None,
        charges: None,
        forge_quality: None,
        forge_color: None,
        forge_side_effects: Vec::new(),
        forge_achieved_tier: None,
        alchemy: None,
        lingering_owner_qi: None,
    })
}

/// 收集当前**有玩家在场**的 zone 名集合（去重）。deferred-on-hydrate 只物化这些 zone 的遗物。
/// 无玩家 → 空集 → 不物化任何遗物（遗物静静躺在 sqlite 等人来，由 TTL sweep 兜底过期清理）。
fn zones_with_players(
    zone_registry: &ZoneRegistry,
    players: &Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
) -> Vec<String> {
    let mut zones = Vec::new();
    for (pos, dim) in players.iter() {
        let dimension = dim.map(|d| d.0).unwrap_or(DimensionKind::Overworld);
        if let Some(zone) = zone_registry.find_zone(dimension, pos.get()) {
            if !zones.contains(&zone.name) {
                zones.push(zone.name.clone());
            }
        }
    }
    zones
}

/// plan-offscreen-war-v1 P3 交付物 3+4：deferred-on-hydrate 遗物物化 system。
///
/// 每帧（限频由 [`crate::npc::dormant::NpcVirtualizationConfig::transition_interval_ticks`] 节流，
/// 复用 hydrate 同款 cadence）：玩家所在 zone → 读 sqlite pending relic → 物化零真元地面 loot →
/// 删行 → 发 VFX/audio/narration。无玩家 → 直接返回（不物化）。
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn hydrate_pending_dormant_relics_system(
    game_tick: Option<Res<crate::npc::movement::GameTick>>,
    config: Res<crate::npc::dormant::NpcVirtualizationConfig>,
    settings: Option<Res<PersistenceSettings>>,
    zone_registry: Option<Res<ZoneRegistry>>,
    item_registry: Option<Res<ItemRegistry>>,
    mut allocator: Option<ResMut<InventoryInstanceIdAllocator>>,
    mut loot_registry: Option<ResMut<DroppedLootRegistry>>,
    mut vfx_events: Option<ResMut<Events<VfxEventRequest>>>,
    mut audio_events: Option<ResMut<Events<PlaySoundRecipeRequest>>>,
    mut narrations: Option<ResMut<PendingGameplayNarrations>>,
    players: Query<(&Position, Option<&CurrentDimension>), With<ClientMarker>>,
) {
    let tick = crate::npc::dormant::current_tick(game_tick.as_deref());
    if !crate::npc::dormant::should_run_interval(tick, config.transition_interval_ticks) {
        return;
    }

    let (
        Some(settings),
        Some(zone_registry),
        Some(item_registry),
        Some(allocator),
        Some(loot_registry),
    ) = (
        settings.as_deref(),
        zone_registry.as_deref(),
        item_registry.as_deref(),
        allocator.as_deref_mut(),
        loot_registry.as_deref_mut(),
    )
    else {
        return;
    };

    let player_zones = zones_with_players(zone_registry, &players);
    if player_zones.is_empty() {
        return;
    }

    for zone_name in player_zones {
        let relics = match load_pending_dormant_relics_for_zone(settings, &zone_name) {
            Ok(relics) => relics,
            Err(error) => {
                tracing::warn!(
                    "[bong][npc] failed to load pending relics for zone {zone_name}: {error}"
                );
                continue;
            }
        };
        if relics.is_empty() {
            continue;
        }

        let dimension = zone_registry
            .find_zone_by_name(&zone_name)
            .map(|zone| zone.dimension)
            .unwrap_or(DimensionKind::Overworld);

        for record in relics {
            let entries = materialize_relic_loot(&record, dimension, item_registry, allocator);

            // 即便 entries 为空（脏 archetype / 全部模板缺失），也删掉该 pending 行——它无法
            // 物化成任何东西，留着只会每次玩家来都重试失败。删后发 narration 仍合理（"曾有厮杀"）。
            let reveal_pos = DVec3::new(record.pos_x, record.pos_y, record.pos_z);
            for entry in entries {
                loot_registry.entries.insert(entry.instance_id, entry);
            }

            if let Err(error) = delete_pending_dormant_relic(settings, &record.relic_id) {
                tracing::warn!(
                    "[bong][npc] failed to delete consumed pending relic {}: {error}",
                    record.relic_id
                );
                // 删失败仍继续——下次玩家来会重新物化（registry 用 instance_id 去重不会爆，
                // 但会多发一份 loot；删失败是 IO 异常路径，记日志即可，不阻塞）。
            }

            // 视听：发现遗物的地表贴花 + 低沉揭示音效 + zone 感知叙事。
            if let Some(vfx_events) = vfx_events.as_deref_mut() {
                send_spawn(
                    vfx_events,
                    spawn_request(
                        OFFSCREEN_RELIC_REVEAL_VFX,
                        reveal_pos,
                        None,
                        // 贴花色与 narration 同源（loot_seed 奇偶）：骨堆灰白 / 残卷暗黄，
                        // 视觉与文字讲同一个故事。
                        relic_decal_color(record.loot_seed),
                        0.7,
                        1,
                        // lifetime 拉满（200 tick，BongGroundDecalParticle 上限）——遗物贴花"持续
                        // 到拾取"的感知近似（client decal maxAge clamp 到 200，足够长）。
                        200,
                    ),
                );
            }
            if let Some(audio_events) = audio_events.as_deref_mut() {
                // 低沉骨裂揭示音（offscreen_relic_reveal recipe），半径内玩家可闻（贴 hydrate
                // 半径 64 同款感知尺度）。pos 取遗骸方块坐标（floor）。
                audio_events.send(PlaySoundRecipeRequest {
                    recipe_id: OFFSCREEN_RELIC_REVEAL_AUDIO.to_string(),
                    instance_id: 0,
                    pos: Some([
                        record.pos_x.floor() as i32,
                        record.pos_y.floor() as i32,
                        record.pos_z.floor() as i32,
                    ]),
                    flag: None,
                    volume_mul: 1.0,
                    pitch_shift: 0.0,
                    recipient: AudioRecipient::Radius {
                        origin: reveal_pos,
                        radius: config.hydrate_radius_blocks,
                    },
                });
            }
            if let Some(narrations) = narrations.as_deref_mut() {
                narrations.push_zone(
                    &zone_name,
                    relic_reveal_narration(record.loot_seed),
                    NarrationStyle::Perception,
                );
            }
            tracing::debug!(
                "[bong][npc] hydrated battlefield relic {} in zone {} (char={})",
                record.relic_id,
                zone_name,
                record.char_id
            );
        }
    }
}

/// 两条 zone-scope perception narration（plan §148），按 loot_seed 奇偶确定性二选一——
/// 让"宗门信物"和"散修残经"两种叙事都出现，且同一遗物每次描述一致。
fn relic_reveal_narration(loot_seed: u64) -> &'static str {
    if loot_seed & 1 == 0 {
        "此地残留厮杀气息，骨片间散落一枚断裂的宗门信物。"
    } else {
        "不知名的散修曾在此陨落，唯余一捧灰烬与半卷残经。"
    }
}

pub fn register(app: &mut App) {
    tracing::info!("[bong][npc] registering deferred-on-hydrate battlefield relic materialization");
    app.add_systems(Update, hydrate_pending_dormant_relics_system);
}

#[cfg(test)]
mod tests;
