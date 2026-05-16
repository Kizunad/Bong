//! 丹毒累计追踪系统。
//!
//! P0 scope: 定义 event + 当 DandaoStyle 被 insert 时自动初始化。
//! 通用服药路径的自动追踪（hook 进 handle_alchemy_take_pill）需要 handler
//! 签名变更，留给后续 PR。当前丹道招式内部直接调 advance_toxin。

use valence::prelude::*;

use super::components::DandaoStyle;
use crate::cultivation::color::{record_style_practice, PracticeLog};
use crate::cultivation::components::ColorKind;

/// 某次丹道操作（服药/炼丹/招式）导致丹毒累积的事件通知。
/// 丹道招式 resolver 在消耗丹药时主动 send。
#[derive(Event, Debug, Clone)]
pub struct PillIntakeTracked {
    pub entity: Entity,
    pub toxin_amount: f64,
    /// 若本次推进触发了变异阶段提升，记录新阶段。
    pub new_stage: Option<u8>,
}

/// 每帧读取 PillIntakeTracked，累计 PracticeLog Mellow 权重。
/// DandaoStyle.advance_toxin() 由招式 resolver 直接调用（P0 scope），
/// 本系统只负责 PracticeLog side-effect。
pub fn track_pill_intake_system(
    mut events: EventReader<PillIntakeTracked>,
    mut players: Query<&mut PracticeLog>,
) {
    for ev in events.read() {
        if ev.toxin_amount <= 0.0 {
            continue;
        }
        if let Ok(mut log) = players.get_mut(ev.entity) {
            record_style_practice(&mut log, ColorKind::Mellow);
        }
    }
}

/// Lazy-insert DandaoStyle 的辅助函数。
/// 如果 entity 没有 DandaoStyle，插入默认值。
pub fn ensure_dandao_style(commands: &mut Commands, entity: Entity, has: bool) {
    if !has {
        commands.entity(entity).insert(DandaoStyle::default());
    }
}
