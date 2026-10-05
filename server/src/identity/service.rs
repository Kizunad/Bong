//! Identity command 的持久化服务边界。
//!
//! 命令层只负责解析输入和组织运行时状态；稳定的玩家键与 repository 调用由此处
//! 统一处理，避免 slash command 直接依赖 SQLite 实现。

use std::io;

use crate::identity::PlayerIdentities;
use crate::persistence::{identity as identity_db, PersistenceSettings};
use crate::player::state::canonical_player_id;

/// 保存玩家 identity 切片。
pub(crate) fn save_player_identities(
    settings: &PersistenceSettings,
    username: &str,
    identities: &PlayerIdentities,
) -> io::Result<()> {
    let char_id = canonical_player_id(username);
    identity_db::save_player_identities(settings, &char_id, identities)
}
