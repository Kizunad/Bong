//! R9 P1 施法身份与会话分配。
//!
//! 这些类型只描述 contract-first 阶段的领域边界，尚未接入网络或任何真实功法。
//! `CastIdentity` 的完整三元组在后续 reducer、AV 事件和 wire mirror 中都必须原样传递。

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use uuid::Uuid;

/// 施法者身份。player 与 NPC 共用同一个会话门，不能用 player-only sentinel 代替 NPC。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CastCaster {
    Player(Uuid),
    Npc(Uuid),
}

impl CastCaster {
    pub const fn is_npc(self) -> bool {
        matches!(self, Self::Npc(_))
    }

    pub const fn uuid(self) -> Uuid {
        match self {
            Self::Player(uuid) | Self::Npc(uuid) => uuid,
        }
    }
}

/// 一次 transport 连接内的施法命名空间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CastSessionId {
    pub session_id: Uuid,
    pub session_generation: u64,
}

impl CastSessionId {
    pub fn new(session_id: Uuid, session_generation: u64) -> Result<Self, IdentityError> {
        if session_generation == 0 {
            return Err(IdentityError::ZeroGeneration);
        }
        Ok(Self {
            session_id,
            session_generation,
        })
    }
}

/// 完整施法身份。除 BEGIN 外的每条消息都必须携带三个字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CastIdentity {
    pub session_id: Uuid,
    pub session_generation: u64,
    pub cast_instance_id: u64,
}

impl CastIdentity {
    pub fn new(
        session_id: Uuid,
        session_generation: u64,
        cast_instance_id: u64,
    ) -> Result<Self, IdentityError> {
        if session_generation == 0 {
            return Err(IdentityError::ZeroGeneration);
        }
        if cast_instance_id == 0 {
            return Err(IdentityError::ZeroInstance);
        }
        Ok(Self {
            session_id,
            session_generation,
            cast_instance_id,
        })
    }

    pub const fn session(self) -> CastSessionId {
        CastSessionId {
            session_id: self.session_id,
            session_generation: self.session_generation,
        }
    }
}

impl fmt::Display for CastIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.session_id, self.session_generation, self.cast_instance_id
        )
    }
}

/// 一次进入 admission 的尝试。拒绝也会消费一个 instance id，保证 attempt high-water 可审计。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastAttempt {
    pub identity: CastIdentity,
}

/// 单调分配器耗尽后的错误；耗尽后不再产生任何 identity 或副作用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    Exhausted,
}

/// 进程级 session generation 分配器。实例放在 server 生命周期根部即可保证全进程单调。
#[derive(Debug)]
pub struct CastGenerationAllocator {
    next_generation: AtomicU64,
}

impl Default for CastGenerationAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl CastGenerationAllocator {
    pub const fn new(first_generation: u64) -> Self {
        Self {
            next_generation: AtomicU64::new(first_generation),
        }
    }

    pub fn allocate(&self) -> Result<u64, AllocationError> {
        self.next_generation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current != 0 && current != u64::MAX).then_some(current + 1)
            })
            .map_err(|_| AllocationError::Exhausted)
    }
}

/// 身份字段的 fail-closed 错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    ZeroGeneration,
    ZeroInstance,
}

/// CastSession 只负责 session gate 与 attempt identity 分配，不负责 gameplay 或 AV。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CastSession {
    pub caster: CastCaster,
    pub id: CastSessionId,
    next_instance_id: u64,
    exhausted: bool,
}

impl CastSession {
    pub fn new(
        caster: CastCaster,
        session_id: Uuid,
        session_generation: u64,
    ) -> Result<Self, IdentityError> {
        Ok(Self {
            caster,
            id: CastSessionId::new(session_id, session_generation)?,
            next_instance_id: 1,
            exhausted: false,
        })
    }

    pub const fn exhausted(self) -> bool {
        self.exhausted
    }

    pub const fn next_instance_id(self) -> u64 {
        self.next_instance_id
    }

    /// 为 accepted 或 rejected attempt 分配身份。
    pub fn allocate_attempt(&mut self) -> Result<CastAttempt, AllocationError> {
        if self.exhausted {
            return Err(AllocationError::Exhausted);
        }
        let cast_instance_id = self.next_instance_id;
        let identity = CastIdentity {
            session_id: self.id.session_id,
            session_generation: self.id.session_generation,
            cast_instance_id,
        };
        if cast_instance_id == u64::MAX {
            self.exhausted = true;
        } else {
            self.next_instance_id += 1;
        }
        Ok(CastAttempt { identity })
    }

    /// 测试或协议回放可把分配器置于 max-1 边界；生产方仍只能使用 `allocate_attempt`。
    #[cfg(test)]
    fn set_next_instance_id_for_test(&mut self, next: u64) {
        self.next_instance_id = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_requires_non_zero_generation_and_instance() {
        let session = Uuid::from_u128(1);
        assert_eq!(
            CastIdentity::new(session, 0, 1),
            Err(IdentityError::ZeroGeneration)
        );
        assert_eq!(
            CastIdentity::new(session, 1, 0),
            Err(IdentityError::ZeroInstance)
        );
    }

    #[test]
    fn every_attempt_uses_full_identity_and_max_exhausts_gate() {
        let mut session = CastSession::new(
            CastCaster::Player(Uuid::from_u128(2)),
            Uuid::from_u128(3),
            1,
        )
        .expect("valid session");
        session.set_next_instance_id_for_test(u64::MAX - 1);
        let penultimate = session.allocate_attempt().expect("penultimate identity");
        let final_attempt = session.allocate_attempt().expect("max identity");
        assert_eq!(penultimate.identity.cast_instance_id, u64::MAX - 1);
        assert_eq!(final_attempt.identity.cast_instance_id, u64::MAX);
        assert!(session.exhausted());
        assert_eq!(session.allocate_attempt(), Err(AllocationError::Exhausted));
    }

    #[test]
    fn generation_allocator_is_monotonic_and_fails_closed_at_exhaustion() {
        let allocator = CastGenerationAllocator::default();
        assert_eq!(allocator.allocate(), Ok(1));
        assert_eq!(allocator.allocate(), Ok(2));

        let exhausted = CastGenerationAllocator::new(u64::MAX);
        assert_eq!(exhausted.allocate(), Err(AllocationError::Exhausted));
        assert_eq!(exhausted.allocate(), Err(AllocationError::Exhausted));
    }
}
