//! 客户端可控请求触发的一次性 guard 日志去重（跨系统共享资源）。
//!
//! 空手/拒收护栏的 guard info! 日志（carrier.rs 的 `throw_carrier guard`、
//! client_request_handler.rs 的 `container_switch guard`）按 (carrier wire id,
//! reason) 去重：任意连接反复发送同一请求只记一条，防止把 info 日志喂成无界
//! 输出（review finding [major]：客户端可控请求制造无界 info 日志路径）。
//!
//! 去重键带 tick 窗口过期：超过 GUARD_LOG_DEDUP_WINDOW_TICKS 未再触发的
//! (carrier, reason) 被剪除——历史玩家 ID 不随服务器寿命永久驻留，内存上界
//! = 窗口内活跃身份数 × 2（review finding [major]：dedup 表随历史玩家无限
//! 增长）。时间源用 CombatClock.tick（server 每 tick 前进一次）。

use std::collections::HashMap;

use valence::prelude::{bevy_ecs, Resource};

/// 去重/过期窗口：窗口内同 (carrier, reason) 至多发一次；窗口外重新触发可再发。
/// 20tps 下 1200 tick ≈ 60s——e2e 护栏场景的断言窗口是 8s 级，首发必然落在窗口
/// 内；旧身份 60s 后自动过期，不会永久占住内存。
pub const GUARD_LOG_DEDUP_WINDOW_TICKS: u64 = 1200;

#[derive(Debug, Default, Resource)]
pub struct GuardLogDedup {
    last_emit_tick: HashMap<(String, &'static str), u64>,
}

impl GuardLogDedup {
    /// 返回该 (carrier, reason) 在 tick 时刻是否应再发一次 guard 日志：
    /// 窗口内已发过为 false；否则记录本次并返回 true。
    pub fn should_emit(&mut self, carrier: &str, reason: &'static str, tick: u64) -> bool {
        self.gc(tick);
        let key = (carrier.to_string(), reason);
        if self
            .last_emit_tick
            .get(&key)
            .is_some_and(|last| tick.saturating_sub(*last) < GUARD_LOG_DEDUP_WINDOW_TICKS)
        {
            return false;
        }
        self.last_emit_tick.insert(key, tick);
        true
    }

    fn gc(&mut self, tick: u64) {
        self.last_emit_tick
            .retain(|_, last| tick.saturating_sub(*last) < GUARD_LOG_DEDUP_WINDOW_TICKS);
    }
}
