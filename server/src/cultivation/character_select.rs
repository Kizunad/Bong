//! 多周目新角色生成 spec（plan-multi-life-v1 §3 数据契约 + Q-ML4 决议）。
//!
//! plan §2 流程明确："新角色生成: Realm = Awaken, 运数 = 3, 寿元 = 醒灵 cap,
//! spawn 位置 = spawn_plain（Q-ML4，与新玩家相同）, 物品/真元/境界 = 0"。
//!
//! 本模块把这 5 个字段集中在 [`NewCharacterSpec`] 一个 struct 里——避免散落
//! 在 `combat::lifecycle::reset_for_new_character` 内再硬编码各处常数（曾经
//! 的 bug：reset 用 `LifespanCapTable::MORTAL` 而 attach 用 `for_realm(Awaken)`
//! 即 `LifespanCapTable::AWAKEN`，两套路径数值漂移）。
//!
//! `next_character_spec()` 是唯一入口，所有"开新角色"调用点必须从这里取数据。

use crate::cultivation::components::Realm;
use crate::cultivation::lifespan::LifespanCapTable;
use crate::cultivation::luck_pool::INITIAL_FORTUNE_PER_LIFE;

/// 新角色生成参数（plan-multi-life-v1 §2 / §3）。
///
/// 字段说明：
/// - `spawn_pos`：出生坐标，必须等于 [`crate::player::spawn_position`] 即 spawn_plain
///   （Q-ML4 决议：第二世新角色出生位置 = 与新玩家相同）
/// - `realm`：起始境界。plan §0 第 4 条："不允许跳过教学，新角色必须从醒灵走"
/// - `initial_fortune`：起始运数。引 [`INITIAL_FORTUNE_PER_LIFE`]，不重复定义
/// - `lifespan_cap`：寿元上限。来自 lifespan-v1 §2 `LifespanCapTable::AWAKEN`
///   （plan-multi-life-v1 Q-ML0 reframe：寿元数据全部引 lifespan-v1）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NewCharacterSpec {
    pub spawn_pos: [f64; 3],
    pub realm: Realm,
    pub initial_fortune: u8,
    pub lifespan_cap: u32,
}

/// 返回 plan-multi-life-v1 §2 规定的新角色 spec。
///
/// 此函数是 reset_for_new_character / character_lifecycle 路径的唯一参数源。
/// 调用方不应再硬编码 `Realm::Awaken` / `LifespanCapTable::AWAKEN` / `3` 等数值。
pub fn next_character_spec() -> NewCharacterSpec {
    NewCharacterSpec {
        spawn_pos: crate::player::spawn_position(),
        realm: Realm::Awaken,
        initial_fortune: INITIAL_FORTUNE_PER_LIFE,
        lifespan_cap: LifespanCapTable::AWAKEN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_realm_is_awaken() {
        // plan §0 第 4 条 + Q-ML4：不允许跳过教学，新角色必须从醒灵走
        assert_eq!(next_character_spec().realm, Realm::Awaken);
    }

    #[test]
    fn spec_initial_fortune_matches_per_life_constant() {
        // plan §0 O.4：每角色独立 3 次（引 luck_pool::INITIAL_FORTUNE_PER_LIFE）
        assert_eq!(
            next_character_spec().initial_fortune,
            INITIAL_FORTUNE_PER_LIFE,
        );
    }

    #[test]
    fn spec_lifespan_cap_is_awaken_per_lifespan_v1() {
        // Q-ML0 reframe：寿元上限引 plan-lifespan-v1 §2 LifespanCapTable
        // plan §2："寿元 = 醒灵 cap"
        assert_eq!(next_character_spec().lifespan_cap, LifespanCapTable::AWAKEN);
        // 防数值漂移：AWAKEN 应仍是 120（plan-lifespan-v1 §2 表）
        assert_eq!(LifespanCapTable::AWAKEN, 120);
    }

    #[test]
    fn spec_spawn_pos_matches_player_spawn_plain() {
        // Q-ML4：第二世新角色 spawn 位置 = spawn_plain（与新玩家相同）
        // spawn_plain 由 plan-spawn-tutorial-v1 实装为 crate::player::spawn_position()
        assert_eq!(
            next_character_spec().spawn_pos,
            crate::player::spawn_position(),
            "新角色 spawn_pos 必须直接引用 player::spawn_position()，否则两条路径数值漂移",
        );
    }

    #[test]
    fn spec_is_idempotent() {
        // 多次调用返回相同 spec —— 否则同一 reset 流程的不同字段读取可能拿到不同值
        let a = next_character_spec();
        let b = next_character_spec();
        assert_eq!(a, b);
    }

    #[test]
    fn spec_lifespan_cap_distinct_from_mortal() {
        // 防回归：`reset_for_new_character` 历史 bug 是给新角色 MORTAL=80 cap，
        // 与 attach 路径的 AWAKEN=120 数值漂移。spec 必须返回 AWAKEN，不是 MORTAL。
        assert_ne!(
            next_character_spec().lifespan_cap,
            LifespanCapTable::MORTAL,
            "新角色寿元 cap 必须 = AWAKEN，不可退回 MORTAL（plan §2 规定）",
        );
    }
}
