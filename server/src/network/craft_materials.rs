//! 材料暂存的库存事务：先持久化，成功后发布库存与掉落。

use valence::message::SendMessage;
use valence::prelude::{Client, Entity, EventReader, Position, Query, Res, ResMut, Username};

use crate::craft::{
    events::MaterialMoveIntent, preparation, CraftRegistry, CraftSession, RecipeUnlockState,
};
use crate::cultivation::components::Cultivation;
use crate::forge::{
    blueprint::BlueprintRegistry, learned::LearnedBlueprints, station::WeaponForgeStation,
};
use crate::inventory::{DroppedLootRegistry, ItemRegistry, PlayerInventory};
use crate::player::state::{
    canonical_player_id, save_player_craft_checkpoint, PlayerState, PlayerStatePersistence,
};
use crate::world::dimension::CurrentDimension;

type MaterialPlayer<'a> = (
    Entity,
    &'a Username,
    &'a mut Client,
    &'a mut PlayerInventory,
    &'a PlayerState,
    &'a Cultivation,
    &'a Position,
    Option<&'a CurrentDimension>,
    Option<&'a CraftSession>,
);

#[allow(clippy::too_many_arguments)]
pub fn apply_craft_material_intents(
    mut intents: EventReader<MaterialMoveIntent>,
    recipes: Res<CraftRegistry>,
    unlocks: Res<RecipeUnlockState>,
    items: Res<ItemRegistry>,
    persistence: Option<Res<PlayerStatePersistence>>,
    mut dropped: ResMut<DroppedLootRegistry>,
    mut players: Query<MaterialPlayer<'_>>,
    blueprints: Option<Res<BlueprintRegistry>>,
    learned: Query<&LearnedBlueprints>,
    stations: Query<&WeaponForgeStation>,
) {
    for intent in intents.read() {
        let Ok((
            entity,
            username,
            mut client,
            mut inventory,
            state,
            cultivation,
            position,
            dimension,
            session,
        )) = players.get_mut(intent.caster)
        else {
            continue;
        };
        let mut staged = inventory.clone();
        let result = (|| {
            // 整批返还（instance_id == None）是 ForgeWindows.close 的关窗路径。
            // 这里有意不因 expected_revision 漂移拒绝它：关窗时若只是无关的背包版本
            // 变化就拒绝，暂存材料会卡在炉里，代价高于一次同配方同工位的陈旧整批返还。
            // 单件移动（instance_id 有值）仍必须匹配版本；紧随其后的 recipe_id / station_pos
            // 校验也会挡住换配方或换工位的陈旧请求。未覆盖的仅是同配方同工位的陈旧关窗，
            // 而请求按连接串行处理，这种交错实际难以发生。
            if session.is_some()
                || (intent.instance_id.is_some()
                    && intent.expected_revision != inventory.revision.0)
            {
                return Err("制作进行中或背包已变化，请重试".to_string());
            }
            if intent.returning
                && (staged.material_preparation.recipe_id.as_deref()
                    != Some(intent.recipe_id.as_str())
                    || staged.material_preparation.station_pos != intent.station_pos)
            {
                return Err("材料不属于当前工序，或已经投入炉次".into());
            }
            let drops = if intent.returning {
                preparation::return_materials(
                    &mut staged,
                    &items,
                    intent.instance_id,
                    [position.0.x, position.0.y, position.0.z],
                    dimension.map(|d| d.0).unwrap_or_default(),
                )?
            } else {
                let instance = intent.instance_id.ok_or("未指定材料实例")?;
                if let Some(pos) = intent.station_pos {
                    let p = position.get();
                    if !p.is_finite()
                        || (p.x - f64::from(pos.0)).abs() > 3.0
                        || (p.y - f64::from(pos.1)).abs() > 3.0
                        || (p.z - f64::from(pos.2)).abs() > 3.0
                        || dimension.map(|value| value.0)
                            != Some(crate::world::dimension::DimensionKind::Overworld)
                    {
                        return Err("请靠近炼器砧后投料".into());
                    }
                    let station = stations
                        .iter()
                        .find(|station| station.pos == Some(pos))
                        .ok_or("工位不存在")?;
                    if station.owner.is_some_and(|owner| owner != entity)
                        || station.session.is_some()
                        || station.integrity <= 0.0
                    {
                        return Err("工位不可投料或已开炉".into());
                    }
                    if !learned
                        .get(entity)
                        .is_ok_and(|book| book.knows(intent.recipe_id.as_str()))
                    {
                        return Err("尚未学会此锻造图谱".into());
                    }
                    let blueprint = blueprints
                        .as_deref()
                        .and_then(|registry| registry.get(intent.recipe_id.as_str()))
                        .ok_or("图谱不存在")?;
                    crate::forge::preparation::stage(&mut staged, blueprint, pos, instance)?;
                } else {
                    let recipe = recipes.get(&intent.recipe_id).ok_or("配方不存在")?;
                    if !unlocks
                        .is_unlocked(&canonical_player_id(username.0.as_str()), &intent.recipe_id)
                    {
                        return Err("配方未解锁".into());
                    }
                    preparation::stage_material(&mut staged, recipe, instance)?;
                }
                Vec::new()
            };
            if let Some(persistence) = persistence.as_deref() {
                save_player_craft_checkpoint(
                    persistence,
                    username.0.as_str(),
                    Some(&staged),
                    session,
                    None,
                    None,
                    &drops,
                )
                .map_err(|error| format!("保存材料失败：{error}"))?;
            }
            Ok(drops)
        })();
        match result {
            Ok(drops) => {
                *inventory = staged;
                dropped
                    .entries
                    .extend(drops.into_iter().map(|entry| (entry.instance_id, entry)));
            }
            Err(error) => {
                tracing::warn!(
                    "[bong][craft] material move rejected player={}: {error}",
                    username.0
                );
                client.send_chat_message(error);
            }
        }
        super::inventory_snapshot_emit::send_inventory_snapshot_to_client(
            entity,
            &mut client,
            username.0.as_str(),
            &inventory,
            state,
            cultivation,
            "material_move",
        );
    }
}
