//! plan-lingtian-v1 fix-spec-1901-v2 §4.1 — 灵田 C2S 请求的持久 ingress 队列。
//!
//! v2 架构：network producer 不再直接读取 `Position` / `CurrentDimension`，
//! 也不再直接写六类 `Start*Request` event。它只把已解析的 C2S 字段按到达顺序
//! push 进本队列；本 tick 所有权威位置/维度写入完成后的唯一
//! `validate_and_dispatch_lingtian_requests` 验证点才读取最终状态并转为
//! `Start*Request`。
//!
//! 队列保存 `Entity`、目标 `BlockPos` 和 action-specific 参数，不保存客户端
//! 声称的当前位置或维度；不查询 plot / inventory / terrain。
//!
//! 边界（fix-spec §10 OPEN-2，owner 已收口）：`QUEUE_CAP` 之上 fail-closed 丢弃
//! 新请求并限频 warn，绝不丢弃旧请求去执行后来的请求——恶意客户端不能以无限
//! payload 令 server 内存无界增长。

use valence::prelude::bevy_ecs;
use valence::prelude::bevy_ecs::system::SystemParam;
use valence::prelude::{BlockPos, Entity, Resource};

use crate::botany::PlantId;

use super::session::{ReplenishSource, SessionMode};

/// 队列长度上限（OPEN-2 安全默认）。到达上限后 fail-closed 丢弃新请求。
pub const QUEUE_CAP: usize = 256;

/// 已解析但尚未过 post-transfer gate 的灵田请求。
#[derive(Debug, Clone)]
pub(crate) enum PendingLingtianRequest {
    Till {
        actor: Entity,
        pos: BlockPos,
        hoe_instance_id: u64,
        mode: SessionMode,
    },
    Renew {
        actor: Entity,
        pos: BlockPos,
        hoe_instance_id: u64,
    },
    Planting {
        actor: Entity,
        pos: BlockPos,
        plant_id: PlantId,
    },
    Harvest {
        actor: Entity,
        pos: BlockPos,
        mode: SessionMode,
    },
    Replenish {
        actor: Entity,
        pos: BlockPos,
        source: ReplenishSource,
    },
    DrainQi {
        actor: Entity,
        pos: BlockPos,
    },
}

impl PendingLingtianRequest {
    pub(crate) fn actor_and_pos(&self) -> (Entity, BlockPos) {
        match self {
            PendingLingtianRequest::Till { actor, pos, .. }
            | PendingLingtianRequest::Renew { actor, pos, .. }
            | PendingLingtianRequest::Planting { actor, pos, .. }
            | PendingLingtianRequest::Harvest { actor, pos, .. }
            | PendingLingtianRequest::Replenish { actor, pos, .. }
            | PendingLingtianRequest::DrainQi { actor, pos } => (*actor, *pos),
        }
    }
}

/// v2 单点 post-transfer validation 的 ingress 队列（Resource，非 Events——
/// 请求必须跨越 validation point 保留）。
///
/// `validator` 开始时 `std::mem::take(&mut inbox)` 取出当前快照：validator 运行
/// 期间新 push 的请求留在下一批，不会在已完成的位置快照上被偷跑复用。
#[derive(Debug, Default, Resource)]
pub(crate) struct PendingLingtianRequests {
    pub(crate) inbox: std::collections::VecDeque<PendingLingtianRequest>,
}

impl PendingLingtianRequests {
    pub(crate) fn len(&self) -> usize {
        self.inbox.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inbox.is_empty()
    }

    /// 按到达顺序入队。满队列时 fail-closed：丢弃新请求、限频 warn，
    /// 保留既有请求（不允许"丢旧保新"绕过 gate）。
    pub(crate) fn push(&mut self, request: PendingLingtianRequest) {
        if self.inbox.len() >= QUEUE_CAP {
            // 限频：同一 actor 的 flood 不必每包都刷 warn。
            let actor = match &request {
                PendingLingtianRequest::Till { actor, .. }
                | PendingLingtianRequest::Renew { actor, .. }
                | PendingLingtianRequest::Planting { actor, .. }
                | PendingLingtianRequest::Harvest { actor, .. }
                | PendingLingtianRequest::Replenish { actor, .. }
                | PendingLingtianRequest::DrainQi { actor, .. } => *actor,
            };
            // 用 entity 低 bit 粗限频：同 actor 连续 flood 只 warn 第一次，
            // 不同 actor 轮流 flood 仍可观测（上限 256 条/批，日志量可控）。
            if (actor.index() % 64) == 0 {
                tracing::warn!(
                    "[bong][lingtian] PendingLingtianRequests full ({QUEUE_CAP}); \
                     dropping request from actor={actor:?} (fail-closed)"
                );
            }
            return;
        }
        self.inbox.push_back(request);
    }

    /// 把本批暂缓的旧请求放回队首。旧请求必须排在 validator 取走快照后才到达的
    /// 新请求之前；若合并后触顶，仍按 drop-new 保留更早的请求。
    pub(crate) fn prepend_batch(
        &mut self,
        mut requests: std::collections::VecDeque<PendingLingtianRequest>,
    ) {
        requests.append(&mut self.inbox);
        requests.truncate(QUEUE_CAP);
        self.inbox = requests;
    }

    /// 取走当前批次快照；本 tick 后续 push 留到下一批。
    pub(crate) fn take_batch(&mut self) -> std::collections::VecDeque<PendingLingtianRequest> {
        std::mem::take(&mut self.inbox)
    }
}

/// post-transfer validator 的 SystemParam 包（避开 Bevy 0.14 tuple-arity 上限，
/// 与 `LingtianRequestParams` 同一风格）。六个 writer 只在 gate 成功后写事件。
#[derive(SystemParam)]
pub(crate) struct LingtianDispatchWriters<'w> {
    pub(crate) till: bevy_ecs::event::EventWriter<'w, super::events::StartTillRequest>,
    pub(crate) renew: bevy_ecs::event::EventWriter<'w, super::events::StartRenewRequest>,
    pub(crate) planting: bevy_ecs::event::EventWriter<'w, super::events::StartPlantingRequest>,
    pub(crate) harvest: bevy_ecs::event::EventWriter<'w, super::events::StartHarvestRequest>,
    pub(crate) replenish: bevy_ecs::event::EventWriter<'w, super::events::StartReplenishRequest>,
    pub(crate) drain_qi: bevy_ecs::event::EventWriter<'w, super::events::StartDrainQiRequest>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(actor: Entity, x: i32) -> PendingLingtianRequest {
        PendingLingtianRequest::DrainQi {
            actor,
            pos: BlockPos::new(x, 64, 0),
        }
    }

    fn positions(queue: &PendingLingtianRequests) -> Vec<i32> {
        queue
            .inbox
            .iter()
            .map(|request| request.actor_and_pos().1.x)
            .collect()
    }

    #[test]
    fn empty_queue_has_snapshot_and_length_zero() {
        let mut queue = PendingLingtianRequests::default();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert!(queue.take_batch().is_empty());
        assert!(queue.is_empty());
    }

    #[test]
    fn queue_is_fifo_and_drops_only_new_request_at_capacity() {
        let actor = Entity::from_raw(1);
        let mut queue = PendingLingtianRequests::default();
        for x in 0..QUEUE_CAP as i32 {
            queue.push(request(actor, x));
        }
        queue.push(request(actor, QUEUE_CAP as i32));

        assert_eq!(queue.len(), QUEUE_CAP);
        assert_eq!(positions(&queue).first(), Some(&0));
        assert_eq!(positions(&queue).last(), Some(&((QUEUE_CAP - 1) as i32)));
    }

    #[test]
    fn take_batch_is_snapshot_and_deferred_batch_precedes_new_arrivals() {
        let actor = Entity::from_raw(1);
        let mut queue = PendingLingtianRequests::default();
        queue.push(request(actor, 1));
        queue.push(request(actor, 2));

        let batch = queue.take_batch();
        queue.push(request(actor, 3));
        queue.prepend_batch(batch);

        assert_eq!(positions(&queue), vec![1, 2, 3]);
    }

    #[test]
    fn prepend_batch_preserves_old_requests_when_capacity_is_reached() {
        let actor = Entity::from_raw(1);
        let mut queue = PendingLingtianRequests::default();
        for x in 100..(100 + QUEUE_CAP as i32) {
            queue.push(request(actor, x));
        }
        let deferred = std::collections::VecDeque::from([request(actor, 1), request(actor, 2)]);
        queue.prepend_batch(deferred);

        assert_eq!(queue.len(), QUEUE_CAP);
        assert_eq!(positions(&queue).first(), Some(&1));
        assert_eq!(positions(&queue)[1], 2);
        assert_eq!(
            positions(&queue).last(),
            Some(&((100 + QUEUE_CAP - 3) as i32))
        );
    }
}
