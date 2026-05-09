//! 飞鲸沉浸感 chat narration —— spawn / death 时附近玩家广播。
//!
//! 用 marker component（`WhaleSpawnNarrationPending`）+ system 读取的方式，
//! 任何 spawn 路径（dev cmd / 自然刷新 / agent 触发）都自动 narration，
//! 不必每个 spawn callsite 重复写 chat 逻辑。

use valence::message::SendMessage;
use valence::prelude::{
    bevy_ecs, App, Client, Commands, Component, Entity, EventReader, Position, Query, Update, With,
    Without,
};

use crate::combat::events::DeathEvent;
use crate::fauna::components::{BeastKind, FaunaTag};

/// 玩家听到鲸鸣 / 看到鲸殒落的最大距离。鲸体型大、飘高，200 块半径合理。
const NARRATION_HEARING_RADIUS_BLOCKS: f64 = 200.0;

/// 鲸 spawn 时的叙事池。splitmix64 选一句。
const SPAWN_NARRATIONS: [&str; 3] = [
    "[天音] 远处云海翻涌，一头巨鲸自鸿蒙浮现，悠远鲸鸣回荡天际……",
    "[天音] 天外传来低沉鲸鸣，似自亘古之外，万物皆为之静听。",
    "[天音] 长鲸破云而来，鳞光照彻苍穹，灵识漫照人间。",
];

/// 鲸殒落时的叙事池。
const DEATH_NARRATIONS: [&str; 3] = [
    "[天音] 巨鲸殒落，骨血洒落人间。神兽陨命，灵气逸散。",
    "[天音] 长鲸坠地，灵识归虚。叹其生时何辉，殒时何寂。",
    "[天音] 一声长吟震九霄，鲸魂自此散尽，唯余骨殖留世。",
];

/// 标记新 spawn 的鲸：narration system 下一 tick 读取并广播，然后清掉标记。
#[derive(Debug, Clone, Copy, Component)]
pub struct WhaleSpawnNarrationPending;

pub fn register(app: &mut App) {
    app.add_systems(
        Update,
        (whale_spawn_narration_system, whale_death_narration_system),
    );
}

/// 选一句叙事。seed 通常是 entity index 或 tick，保证不同鲸 / 不同时刻不撞句。
fn pick_narration<'a>(pool: &'a [&'a str], seed: u64) -> &'a str {
    let idx = (splitmix64(seed) as usize) % pool.len();
    pool[idx]
}

/// 任何挂着 `WhaleSpawnNarrationPending` 的鲸 → 广播 spawn 叙事 → 清标记。
pub fn whale_spawn_narration_system(
    mut commands: Commands,
    pending: Query<(Entity, &Position), With<WhaleSpawnNarrationPending>>,
    mut clients: Query<(&Position, &mut Client), Without<WhaleSpawnNarrationPending>>,
) {
    for (whale, whale_pos) in &pending {
        let line = pick_narration(&SPAWN_NARRATIONS, whale.to_bits());
        let origin = whale_pos.get();
        for (client_pos, mut client) in clients.iter_mut() {
            if client_pos.get().distance(origin) <= NARRATION_HEARING_RADIUS_BLOCKS {
                client.send_chat_message(line.to_string());
            }
        }
        commands
            .entity(whale)
            .remove::<WhaleSpawnNarrationPending>();
    }
}

/// DeathEvent → 若死的是 FaunaTag::Whale → 广播 death 叙事。
pub fn whale_death_narration_system(
    mut deaths: EventReader<DeathEvent>,
    whales: Query<(&FaunaTag, &Position)>,
    mut clients: Query<(&Position, &mut Client)>,
) {
    for event in deaths.read() {
        let Ok((tag, whale_pos)) = whales.get(event.target) else {
            continue;
        };
        if tag.beast_kind != BeastKind::Whale {
            continue;
        }
        let line = pick_narration(&DEATH_NARRATIONS, event.at_tick);
        let origin = whale_pos.get();
        for (client_pos, mut client) in clients.iter_mut() {
            if client_pos.get().distance(origin) <= NARRATION_HEARING_RADIUS_BLOCKS {
                client.send_chat_message(line.to_string());
            }
        }
    }
}

fn splitmix64(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_narration_returns_one_of_pool() {
        for seed in 0..1000u64 {
            let pick = pick_narration(&SPAWN_NARRATIONS, seed);
            assert!(SPAWN_NARRATIONS.contains(&pick));
        }
    }

    #[test]
    fn pick_narration_deterministic_given_seed() {
        let a = pick_narration(&SPAWN_NARRATIONS, 42);
        let b = pick_narration(&SPAWN_NARRATIONS, 42);
        assert_eq!(a, b, "same seed must yield same line");
    }

    #[test]
    fn pick_narration_changes_with_seed() {
        // 饱和：1000 个连续 seed 必须命中至少 2 句不同
        let lines: std::collections::HashSet<_> = (0..1000u64)
            .map(|s| pick_narration(&SPAWN_NARRATIONS, s))
            .collect();
        assert!(lines.len() >= 2, "splitmix should spread across pool");
    }

    #[test]
    fn narration_pool_lengths_are_pinned() {
        // pin：spawn 3 句、death 3 句（防意外删句）
        assert_eq!(SPAWN_NARRATIONS.len(), 3);
        assert_eq!(DEATH_NARRATIONS.len(), 3);
    }

    #[test]
    fn hearing_radius_pinned_at_200_blocks() {
        assert_eq!(NARRATION_HEARING_RADIUS_BLOCKS, 200.0);
    }
}
