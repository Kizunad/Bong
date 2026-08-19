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

/// 每 actor 准入配额（QUEUE_CAP / 8）。全局 QUEUE_CAP 只挡内存无界增长，挡不住
/// 单客户端 flood 挤占其他玩家的队列空间；per-actor 配额把任一 actor 可占用的
/// 槽位上界收口到 1/8，剩余槽位始终留给其他玩家（central review 1984-31332727941
/// finding [3]+[6]）。
pub const PER_ACTOR_CAP: usize = QUEUE_CAP / 8;

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
    /// 当前 `inbox` 内每个 actor 的占用数。准入时做 per-actor 配额检查（见
    /// `PER_ACTOR_CAP`）；`take_batch` 清空后置零，`prepend_batch` 重建，
    /// 保证计数始终反映 `inbox` 当前内容。
    actor_counts: std::collections::HashMap<Entity, usize>,
}

impl PendingLingtianRequests {
    pub(crate) fn len(&self) -> usize {
        self.inbox.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inbox.is_empty()
    }

    /// 按到达顺序入队。全局满队列或该 actor 已占满 per-actor 配额时 fail-closed：
    /// 丢弃新请求、限频 warn，保留既有请求（不允许"丢旧保新"绕过 gate）。
    pub(crate) fn push(&mut self, request: PendingLingtianRequest) {
        // 用 entity 低 bit 粗限频：同 actor 连续 flood 只 warn 第一次，
        // 不同 actor 轮流 flood 仍可观测（上限 256 条/批，日志量可控）。
        let actor = match &request {
            PendingLingtianRequest::Till { actor, .. }
            | PendingLingtianRequest::Renew { actor, .. }
            | PendingLingtianRequest::Planting { actor, .. }
            | PendingLingtianRequest::Harvest { actor, .. }
            | PendingLingtianRequest::Replenish { actor, .. }
            | PendingLingtianRequest::DrainQi { actor, .. } => *actor,
        };
        let actor_occupancy = self.actor_counts.get(&actor).copied().unwrap_or(0);
        if self.inbox.len() >= QUEUE_CAP || actor_occupancy >= PER_ACTOR_CAP {
            if (actor.index() % 64) == 0 {
                tracing::warn!(
                    "[bong][lingtian] PendingLingtianRequests full (global {}/{QUEUE_CAP} \
                     or per-actor {actor_occupancy}/{PER_ACTOR_CAP}); dropping request \
                     from actor={actor:?} (fail-closed)",
                    self.inbox.len()
                );
            }
            return;
        }
        *self.actor_counts.entry(actor).or_insert(0) += 1;
        self.inbox.push_back(request);
    }

    /// 把本批暂缓的旧请求放回队首。旧请求必须排在 validator 取走快照后才到达的
    /// 新请求之前；若合并后触顶，仍按 drop-new 保留更早的请求。
    pub(crate) fn prepend_batch(
        &mut self,
        mut requests: std::collections::VecDeque<PendingLingtianRequest>,
    ) {
        requests.append(&mut self.inbox);
        let mut merged = std::collections::VecDeque::with_capacity(requests.len().min(QUEUE_CAP));
        let mut actor_counts = std::collections::HashMap::new();
        for request in requests {
            if merged.len() >= QUEUE_CAP {
                break;
            }
            let actor = request.actor_and_pos().0;
            let actor_occupancy = actor_counts.entry(actor).or_insert(0);
            if *actor_occupancy >= PER_ACTOR_CAP {
                continue;
            }
            *actor_occupancy += 1;
            merged.push_back(request);
        }
        self.inbox = merged;
        self.actor_counts = actor_counts;
    }

    /// 取走当前批次快照；本 tick 后续 push 留到下一批。
    pub(crate) fn take_batch(&mut self) -> std::collections::VecDeque<PendingLingtianRequest> {
        self.actor_counts.clear();
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
    fn global_cap_drops_only_new_request_across_distinct_actors() {
        // 全局 cap 测试用不同 actor 填满（每 actor 1 条），避免单 actor 触发
        // per-actor 配额：这条测试验证的是全局 drop-new 保 FIFO，而非配额。
        let mut queue = PendingLingtianRequests::default();
        for x in 0..QUEUE_CAP as i32 {
            queue.push(request(Entity::from_raw((x + 1) as u32), x));
        }
        queue.push(request(
            Entity::from_raw((QUEUE_CAP + 1) as u32),
            QUEUE_CAP as i32,
        ));

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
        // 同上：全局 cap 场景用不同 actor，per-actor 配额不应成为干扰因素。
        let mut queue = PendingLingtianRequests::default();
        for x in 100..(100 + QUEUE_CAP as i32) {
            queue.push(request(Entity::from_raw((x - 100 + 1) as u32), x));
        }
        let deferred = std::collections::VecDeque::from([
            request(Entity::from_raw(300), 1),
            request(Entity::from_raw(301), 2),
        ]);
        queue.prepend_batch(deferred);

        assert_eq!(queue.len(), QUEUE_CAP);
        assert_eq!(positions(&queue).first(), Some(&1));
        assert_eq!(positions(&queue)[1], 2);
        assert_eq!(
            positions(&queue).last(),
            Some(&((100 + QUEUE_CAP - 3) as i32))
        );
    }

    #[test]
    fn per_actor_cap_bounds_one_actor_without_discarding_others() {
        let mut queue = PendingLingtianRequests::default();
        let attacker = Entity::from_raw(1);

        // attacker 填满自己的 per-actor 配额。
        for x in 0..PER_ACTOR_CAP as i32 {
            queue.push(request(attacker, x));
        }
        assert_eq!(queue.len(), PER_ACTOR_CAP);

        // attacker 的第 PER_ACTOR_CAP+1 条被 drop-new 丢弃（fail-closed，保旧）。
        queue.push(request(attacker, PER_ACTOR_CAP as i32));
        assert_eq!(queue.len(), PER_ACTOR_CAP);
        assert_eq!(positions(&queue).first(), Some(&0));
        assert_eq!(
            positions(&queue).last(),
            Some(&((PER_ACTOR_CAP - 1) as i32))
        );

        // 其他 actor 的请求仍可准入——配额不能构成对其他玩家的饿死。
        queue.push(request(Entity::from_raw(2), 9000));
        assert_eq!(queue.len(), PER_ACTOR_CAP + 1);
        assert_eq!(positions(&queue).last(), Some(&9000));
    }

    #[test]
    fn per_actor_quota_survives_take_and_prepend_snapshot_round_trip() {
        // 验证快照往返（take_batch 清计数 + prepend_batch 重建计数）后 per-actor
        // 配额仍然生效：validator 的 defer/prepend 不能让一个 actor 借机绕过配额。
        let mut queue = PendingLingtianRequests::default();
        let actor = Entity::from_raw(1);
        for x in 0..PER_ACTOR_CAP as i32 {
            queue.push(request(actor, x));
        }

        let batch = queue.take_batch();
        assert!(queue.is_empty());
        queue.prepend_batch(batch);
        assert_eq!(queue.len(), PER_ACTOR_CAP);

        queue.push(request(actor, PER_ACTOR_CAP as i32));
        assert_eq!(queue.len(), PER_ACTOR_CAP);
        assert_eq!(
            positions(&queue).last(),
            Some(&((PER_ACTOR_CAP - 1) as i32)),
            "prepend 后配额仍应挡住该 actor 的下一跳"
        );
    }

    #[test]
    fn prepend_batch_reapplies_per_actor_quota_without_starving_later_actors() {
        let attacker = Entity::from_raw(1);
        let victim = Entity::from_raw(2);
        let mut queue = PendingLingtianRequests::default();
        for x in 0..PER_ACTOR_CAP as i32 {
            queue.push(request(attacker, x));
        }

        let deferred = queue.take_batch();
        for x in 100..(100 + PER_ACTOR_CAP as i32) {
            queue.push(request(attacker, x));
        }
        queue.push(request(victim, 9000));
        queue.prepend_batch(deferred);

        assert_eq!(
            positions(&queue),
            (0..PER_ACTOR_CAP as i32)
                .chain(std::iter::once(9000))
                .collect::<Vec<_>>(),
            "prepend 必须保留攻击者较早的 deferred FIFO、丢弃其超额新请求，并继续准入后续其他 actor"
        );

        queue.push(request(attacker, 9998));
        queue.push(request(victim, 9999));
        assert_eq!(
            positions(&queue).last(),
            Some(&9999),
            "合并后计数必须只包含实际保留项：攻击者仍受 cap 限制，victim 可继续入队"
        );
        assert_eq!(queue.len(), PER_ACTOR_CAP + 2);
    }
}
