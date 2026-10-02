//! R9 P1 的唯一 cast reducer。
//!
//! 这不是网络接线，也不是功法 resolver。它是一个可被测试逐消息驱动的、惰性的
//! contract-first 状态机。所有副作用都作为 `CastEffect` 回执返回，后续 Wave 2 才会
//! 把这些回执接到 transport/AV consumer。

use std::collections::BTreeMap;

use super::identity::{CastCaster, CastIdentity, CastSessionId};

pub const TERMINAL_RECORD_CAPACITY: usize = 256;
pub const AV_TOMBSTONE_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastSource {
    QuickSlot,
    SkillBar,
    Dedicated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CastPhase {
    Idle,
    Casting,
    Complete,
    Interrupt,
}

/// P-09 的完整 outcome 集合（reject/meridian outcome 只允许出现在 Idle attempt feedback）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CastOutcome {
    None,
    Completed,
    InterruptMovement,
    InterruptContam,
    InterruptControl,
    UserCancel,
    Death,
    MeridianGated,
    RejectQiInsufficient,
    RejectOnCooldown,
    RejectInvalidTarget,
    RejectInRecovery,
    RejectRealmTooLow,
    RejectNoWeapon,
    RejectTechniqueInactive,
    RejectRaceMismatch,
    RejectSkillConfigInvalid,
}

impl CastOutcome {
    pub const fn is_rejection(self) -> bool {
        matches!(
            self,
            Self::MeridianGated
                | Self::RejectQiInsufficient
                | Self::RejectOnCooldown
                | Self::RejectInvalidTarget
                | Self::RejectInRecovery
                | Self::RejectRealmTooLow
                | Self::RejectNoWeapon
                | Self::RejectTechniqueInactive
                | Self::RejectRaceMismatch
                | Self::RejectSkillConfigInvalid
        )
    }

    pub const fn is_interrupt(self) -> bool {
        matches!(
            self,
            Self::InterruptMovement
                | Self::InterruptContam
                | Self::InterruptControl
                | Self::UserCancel
                | Self::Death
        )
    }

    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed) || self.is_interrupt()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastBegin {
    pub caster: CastCaster,
    pub session: CastSessionId,
    pub allocator_exhausted: bool,
    pub active_cast_instance_id: Option<u64>,
    pub minimum_cast_instance_id: Option<u64>,
}

impl CastBegin {
    pub fn open(caster: CastCaster, session: CastSessionId) -> Self {
        Self {
            caster,
            session,
            allocator_exhausted: false,
            active_cast_instance_id: None,
            minimum_cast_instance_id: None,
        }
    }

    pub fn advertised_active(
        caster: CastCaster,
        session: CastSessionId,
        active_cast_instance_id: u64,
        allocator_exhausted: bool,
    ) -> Option<Self> {
        if active_cast_instance_id == 0 {
            return None;
        }
        Some(Self {
            caster,
            session,
            allocator_exhausted,
            active_cast_instance_id: Some(active_cast_instance_id),
            minimum_cast_instance_id: Some(active_cast_instance_id),
        })
    }

    pub fn is_well_formed(&self) -> bool {
        match (self.active_cast_instance_id, self.minimum_cast_instance_id) {
            (None, None) => true,
            (Some(active), Some(floor)) => active != 0 && active == floor,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastMessage {
    Begin(CastBegin),
    Reject {
        identity: CastIdentity,
        outcome: CastOutcome,
    },
    Casting {
        identity: CastIdentity,
    },
    Complete {
        identity: CastIdentity,
    },
    Interrupt {
        identity: CastIdentity,
        outcome: CastOutcome,
    },
    Play {
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    },
    Stop {
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    },
    Unload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceDisposition {
    Accepted,
    AcceptedIdempotent,
    Ignored,
}

impl ReduceDisposition {
    pub const fn is_accepted(self) -> bool {
        !matches!(self, Self::Ignored)
    }

    pub const fn is_idempotent(self) -> bool {
        matches!(self, Self::AcceptedIdempotent)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastEffect {
    AuthoritativeCasting(CastIdentity),
    AuthoritativeInterrupt {
        identity: CastIdentity,
        outcome: CastOutcome,
    },
    AuthoritativeOutcome {
        identity: CastIdentity,
        outcome: CastOutcome,
    },
    RejectFeedback {
        identity: CastIdentity,
        outcome: CastOutcome,
    },
    PlayAv {
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    },
    StopAv {
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReduceResult {
    pub disposition: ReduceDisposition,
    pub rule: &'static str,
    pub effects: Vec<CastEffect>,
}

impl ReduceResult {
    fn accepted(rule: &'static str, effects: Vec<CastEffect>) -> Self {
        Self {
            disposition: ReduceDisposition::Accepted,
            rule,
            effects,
        }
    }

    fn idempotent(rule: &'static str) -> Self {
        Self {
            disposition: ReduceDisposition::AcceptedIdempotent,
            rule,
            effects: Vec::new(),
        }
    }

    fn ignored(rule: &'static str) -> Self {
        Self {
            disposition: ReduceDisposition::Ignored,
            rule,
            effects: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastLifecycle {
    Empty,
    Open,
    Reserved,
    Active,
    Exhausted,
    ExhaustedReserved,
    ExhaustedActive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionGate {
    pub caster: CastCaster,
    pub session: CastSessionId,
    pub exhausted: bool,
    /// BEGIN 广播的 floor。没有 advertised active 时为 None。
    pub advertised_floor: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvOwner {
    pub identity: CastIdentity,
    pub animation_id_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvTombstone {
    pub identity: CastIdentity,
    pub av_binding_key: String,
    pub animation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptFeedback {
    pub identity: CastIdentity,
    pub outcome: CastOutcome,
}

/// 唯一状态容器。四个分区（reject、authoritative、AV、session gate）不互相误清。
#[derive(Debug, Clone, Default)]
pub struct CastState {
    pub gate: Option<SessionGate>,
    pub reserved: Option<CastIdentity>,
    pub pending_identity: Option<CastIdentity>,
    pub active: Option<CastIdentity>,
    pub attempt_high_water: u64,
    pub terminal_high_water: u64,
    pub latest_feedback: Option<AttemptFeedback>,
    pub rejected_attempts: BTreeMap<CastIdentity, CastOutcome>,
    pub terminal_records: BTreeMap<CastIdentity, CastOutcome>,
    pub av_owner: Option<AvOwner>,
    pub av_owner_binding_key: Option<String>,
    pub av_owner_animation_id: Option<String>,
    pub av_tombstones: BTreeMap<(CastIdentity, String, String), AvTombstone>,
    pub av_supersession_floor: Option<CastIdentity>,
    pub pending_pre_admission_stop: Option<CastIdentity>,
}

impl CastState {
    pub fn lifecycle(&self) -> CastLifecycle {
        match (self.gate, self.reserved, self.active) {
            (None, _, _) => CastLifecycle::Empty,
            (Some(gate), Some(_), _) if gate.exhausted => CastLifecycle::ExhaustedReserved,
            (Some(gate), _, Some(_)) if gate.exhausted => CastLifecycle::ExhaustedActive,
            (Some(gate), Some(_), _) if !gate.exhausted => CastLifecycle::Reserved,
            (Some(gate), _, Some(_)) if !gate.exhausted => CastLifecycle::Active,
            (Some(gate), None, None) if gate.exhausted => CastLifecycle::Exhausted,
            (Some(_), None, None) => CastLifecycle::Open,
            (Some(gate), _, _) if gate.exhausted => CastLifecycle::Exhausted,
            (Some(_), _, _) => CastLifecycle::Open,
        }
    }

    pub fn reduce(&mut self, message: CastMessage) -> ReduceResult {
        match message {
            CastMessage::Begin(begin) => self.reduce_begin(begin),
            CastMessage::Reject { identity, outcome } => self.reduce_reject(identity, outcome),
            CastMessage::Casting { identity } => self.reduce_casting(identity),
            CastMessage::Complete { identity } => {
                self.reduce_terminal(identity, CastOutcome::Completed)
            }
            CastMessage::Interrupt { identity, outcome } => self.reduce_terminal(identity, outcome),
            CastMessage::Play {
                identity,
                av_binding_key,
                animation_id,
            } => self.reduce_play(identity, av_binding_key, animation_id),
            CastMessage::Stop {
                identity,
                av_binding_key,
                animation_id,
            } => self.reduce_stop(identity, av_binding_key, animation_id),
            CastMessage::Unload => self.reduce_unload(),
        }
    }

    fn reduce_begin(&mut self, begin: CastBegin) -> ReduceResult {
        if !begin.is_well_formed() {
            return ReduceResult::ignored("R-03 malformed BEGIN");
        }
        let Some(current) = self.gate else {
            self.install_begin(begin);
            return ReduceResult::accepted("R-01", Vec::new());
        };
        if begin.session.session_id == current.session.session_id
            && begin.session.session_generation < current.session.session_generation
        {
            return ReduceResult::ignored("R-03 old generation");
        }
        if begin.session.session_generation < current.session.session_generation
            || (begin.session.session_generation == current.session.session_generation
                && begin.session.session_id != current.session.session_id)
        {
            return ReduceResult::ignored("R-03 session mismatch");
        }
        if begin.session.session_generation > current.session.session_generation
            || begin.session.session_id != current.session.session_id
        {
            self.install_begin(begin);
            return ReduceResult::accepted("R-01", Vec::new());
        }

        let Some(new_floor) = begin.minimum_cast_instance_id else {
            if begin.active_cast_instance_id.is_some() {
                return ReduceResult::ignored("R-03 active/floor mismatch");
            }
            if begin.allocator_exhausted && !current.exhausted {
                self.gate.as_mut().expect("gate present").exhausted = true;
                return ReduceResult::accepted("R-02 exhaustion upgrade", Vec::new());
            }
            return ReduceResult::idempotent("R-02");
        };
        if current.advertised_floor != Some(new_floor) {
            return ReduceResult::ignored("R-03 advertised active changed");
        }
        if begin.allocator_exhausted && !current.exhausted {
            self.gate.as_mut().expect("gate present").exhausted = true;
            return ReduceResult::accepted("R-02 exhaustion upgrade", Vec::new());
        }
        ReduceResult::idempotent("R-02")
    }

    fn install_begin(&mut self, begin: CastBegin) {
        self.gate = Some(SessionGate {
            caster: begin.caster,
            session: begin.session,
            exhausted: begin.allocator_exhausted,
            advertised_floor: begin.minimum_cast_instance_id,
        });
        self.reserved = begin.active_cast_instance_id.map(|instance| CastIdentity {
            session_id: begin.session.session_id,
            session_generation: begin.session.session_generation,
            cast_instance_id: instance,
        });
        self.pending_identity = self.reserved;
        self.active = None;
        self.attempt_high_water = 0;
        self.terminal_high_water = 0;
        self.latest_feedback = None;
        self.rejected_attempts.clear();
        self.terminal_records.clear();
        self.clear_av_state();
    }

    fn reduce_reject(&mut self, identity: CastIdentity, outcome: CastOutcome) -> ReduceResult {
        if !self.identity_admitted(identity) || !outcome.is_rejection() {
            return ReduceResult::ignored("R-14 invalid reject");
        }
        if let Some(previous) = self.rejected_attempts.get(&identity).copied() {
            return if previous == outcome {
                ReduceResult::idempotent("R-05")
            } else {
                ReduceResult::ignored("R-14 reject outcome conflict")
            };
        }
        if self.terminal_records.contains_key(&identity) {
            return ReduceResult::ignored("R-14 reject after terminal");
        }
        if self.admission_exhausted_for(identity) {
            return ReduceResult::ignored("R-14 exhausted admission");
        }
        if identity.cast_instance_id <= self.attempt_high_water {
            return ReduceResult::ignored("R-14 stale reject");
        }
        self.attempt_high_water = identity.cast_instance_id;
        self.rejected_attempts.insert(identity, outcome);
        if self
            .latest_feedback
            .is_none_or(|feedback| identity.cast_instance_id >= feedback.identity.cast_instance_id)
        {
            self.latest_feedback = Some(AttemptFeedback { identity, outcome });
        }
        self.mark_exhausted_if_max(identity.cast_instance_id);
        ReduceResult::accepted(
            "R-04",
            vec![CastEffect::RejectFeedback { identity, outcome }],
        )
    }

    fn reduce_casting(&mut self, identity: CastIdentity) -> ReduceResult {
        if !self.identity_admitted(identity) {
            return ReduceResult::ignored("R-14 invalid CASTING");
        }
        if self.rejected_attempts.contains_key(&identity)
            || self.terminal_records.contains_key(&identity)
        {
            return ReduceResult::ignored("R-14 CASTING after disposition");
        }
        if self.active == Some(identity) {
            return ReduceResult::idempotent("R-07");
        }
        if self.admission_exhausted_for(identity) {
            return ReduceResult::ignored("R-14 exhausted admission");
        }
        if identity.cast_instance_id <= self.attempt_high_water
            && self.pending_identity != Some(identity)
        {
            return ReduceResult::ignored("R-14 stale CASTING");
        }

        let mut effects = Vec::new();
        if let Some(old) = self.active {
            if old == identity {
                return ReduceResult::idempotent("R-07");
            }
            self.close_active_as_interrupt(old, &mut effects);
        }
        let consumed_pending_stop = self.pending_identity == Some(identity);
        self.reserved = None;
        self.pending_identity = None;
        if consumed_pending_stop {
            self.pending_pre_admission_stop = None;
        }
        self.active = Some(identity);
        self.attempt_high_water = self.attempt_high_water.max(identity.cast_instance_id);
        self.mark_exhausted_if_max(identity.cast_instance_id);
        effects.push(CastEffect::AuthoritativeCasting(identity));
        ReduceResult::accepted("R-06", effects)
    }

    fn reduce_terminal(&mut self, identity: CastIdentity, outcome: CastOutcome) -> ReduceResult {
        if !self.identity_admitted(identity) || !outcome.is_terminal() {
            return ReduceResult::ignored("R-14 invalid terminal");
        }
        if let Some(previous) = self.terminal_records.get(&identity).copied() {
            return if previous == outcome {
                ReduceResult::idempotent("R-09")
            } else {
                ReduceResult::ignored("R-14 terminal outcome conflict")
            };
        }
        if self.rejected_attempts.contains_key(&identity) {
            return ReduceResult::ignored("R-14 terminal for rejected attempt");
        }
        if self.admission_exhausted_for(identity) {
            return ReduceResult::ignored("R-14 exhausted admission");
        }
        if self.active != Some(identity)
            && self.reserved != Some(identity)
            && identity.cast_instance_id <= self.terminal_high_water
        {
            return ReduceResult::idempotent("R-08 evicted terminal replay");
        }
        if let Some(old) = self.active {
            if old != identity {
                if identity.cast_instance_id <= old.cast_instance_id {
                    return ReduceResult::ignored("R-14 stale terminal");
                }
                let mut effects = Vec::new();
                self.close_active_as_interrupt(old, &mut effects);
                self.accept_terminal(identity, outcome, &mut effects);
                return ReduceResult::accepted("R-08 supersession", effects);
            }
        }
        let mut effects = Vec::new();
        self.accept_terminal(identity, outcome, &mut effects);
        ReduceResult::accepted("R-08", effects)
    }

    fn accept_terminal(
        &mut self,
        identity: CastIdentity,
        outcome: CastOutcome,
        effects: &mut Vec<CastEffect>,
    ) {
        self.stop_owner_for_identity(identity, effects);
        self.terminal_records.insert(identity, outcome);
        self.terminal_high_water = self.terminal_high_water.max(identity.cast_instance_id);
        self.attempt_high_water = self.attempt_high_water.max(identity.cast_instance_id);
        if self
            .latest_feedback
            .is_none_or(|feedback| identity.cast_instance_id >= feedback.identity.cast_instance_id)
        {
            self.latest_feedback = None;
        }
        if self.active == Some(identity) {
            self.active = None;
        }
        if self.reserved == Some(identity) {
            self.reserved = None;
        }
        if self.pending_identity == Some(identity) {
            self.pending_identity = None;
        }
        if self.pending_pre_admission_stop == Some(identity) {
            self.pending_pre_admission_stop = None;
        }
        self.mark_exhausted_if_max(identity.cast_instance_id);
        self.evict_terminal_records_if_needed();
        effects.push(CastEffect::AuthoritativeOutcome { identity, outcome });
    }

    fn close_active_as_interrupt(&mut self, identity: CastIdentity, effects: &mut Vec<CastEffect>) {
        self.stop_owner_for_identity(identity, effects);
        let outcome = CastOutcome::InterruptControl;
        self.terminal_records.insert(identity, outcome);
        self.terminal_high_water = self.terminal_high_water.max(identity.cast_instance_id);
        self.attempt_high_water = self.attempt_high_water.max(identity.cast_instance_id);
        if self.active == Some(identity) {
            self.active = None;
        }
        self.reserved = self.reserved.filter(|value| *value != identity);
        self.pending_identity = self.pending_identity.filter(|value| *value != identity);
        self.pending_pre_admission_stop = self
            .pending_pre_admission_stop
            .filter(|value| *value != identity);
        self.evict_terminal_records_if_needed();
        effects.push(CastEffect::AuthoritativeInterrupt { identity, outcome });
    }

    fn reduce_play(
        &mut self,
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    ) -> ReduceResult {
        if !self.identity_admitted(identity)
            || av_binding_key.trim().is_empty()
            || animation_id.trim().is_empty()
            || self.active != Some(identity)
            || self.terminal_records.contains_key(&identity)
            || self.av_tombstone_exists(&identity, &av_binding_key, &animation_id)
            || self.floor_rejects(identity)
        {
            return ReduceResult::ignored("R-14 PLAY gate");
        }
        if let Some(owner) = self.av_owner {
            if owner.identity == identity
                && self.av_owner_binding_key.as_deref() == Some(av_binding_key.as_str())
            {
                return if self.av_owner_animation_id.as_deref() == Some(animation_id.as_str()) {
                    ReduceResult::idempotent("R-10")
                } else {
                    ReduceResult::ignored("R-14 PLAY animation conflict")
                };
            }
            return ReduceResult::ignored("R-14 AV owner already held");
        }
        self.av_owner = Some(AvOwner {
            identity,
            animation_id_hash: stable_text_hash(&animation_id),
        });
        self.av_owner_binding_key = Some(av_binding_key.clone());
        self.av_owner_animation_id = Some(animation_id.clone());
        ReduceResult::accepted(
            "R-10",
            vec![CastEffect::PlayAv {
                identity,
                av_binding_key,
                animation_id,
            }],
        )
    }

    fn reduce_stop(
        &mut self,
        identity: CastIdentity,
        av_binding_key: String,
        animation_id: String,
    ) -> ReduceResult {
        if !self.identity_admitted(identity)
            || av_binding_key.trim().is_empty()
            || animation_id.trim().is_empty()
        {
            return ReduceResult::ignored("R-14 invalid STOP");
        }
        let exact_owner = self.av_owner.is_some_and(|owner| {
            owner.identity == identity
                && self.av_owner_binding_key.as_deref() == Some(av_binding_key.as_str())
                && self.av_owner_animation_id.as_deref() == Some(animation_id.as_str())
        });
        if exact_owner {
            self.clear_av_owner();
            self.insert_tombstone(identity, av_binding_key.clone(), animation_id.clone());
            self.raise_floor_for_active(identity);
            return ReduceResult::accepted(
                "R-11",
                vec![CastEffect::StopAv {
                    identity,
                    av_binding_key,
                    animation_id,
                }],
            );
        }
        if self.av_owner.is_some_and(|owner| {
            owner.identity == identity
                && self.av_owner_binding_key.as_deref() == Some(av_binding_key.as_str())
        }) {
            return ReduceResult::ignored("R-14 STOP animation conflict");
        }
        if self.pending_identity == Some(identity) && self.active != Some(identity) {
            if self.pending_pre_admission_stop == Some(identity) {
                return ReduceResult::idempotent("R-12 pending STOP");
            }
            self.pending_pre_admission_stop = Some(identity);
            return ReduceResult::accepted("R-12 pending STOP", Vec::new());
        }
        if self.av_tombstone_exists(&identity, &av_binding_key, &animation_id) {
            return ReduceResult::idempotent("R-13");
        }
        self.insert_tombstone(identity, av_binding_key, animation_id);
        self.raise_floor_for_active(identity);
        ReduceResult::accepted("R-12 non-owner STOP", Vec::new())
    }

    fn reduce_unload(&mut self) -> ReduceResult {
        if self.gate.is_none() {
            return ReduceResult::ignored("R-15 no session");
        }
        self.gate = None;
        self.reserved = None;
        self.pending_identity = None;
        self.active = None;
        self.attempt_high_water = 0;
        self.terminal_high_water = 0;
        self.latest_feedback = None;
        self.rejected_attempts.clear();
        self.terminal_records.clear();
        self.clear_av_state();
        ReduceResult::accepted("R-15", Vec::new())
    }

    fn identity_admitted(&self, identity: CastIdentity) -> bool {
        self.gate.is_some_and(|gate| {
            gate.session.session_id == identity.session_id
                && gate.session.session_generation == identity.session_generation
                && identity.cast_instance_id != 0
                && gate
                    .advertised_floor
                    .is_none_or(|floor| identity.cast_instance_id >= floor)
        })
    }

    fn admission_exhausted_for(&self, identity: CastIdentity) -> bool {
        self.gate.is_some_and(|gate| {
            gate.exhausted
                && self.reserved != Some(identity)
                && self.pending_identity != Some(identity)
                && self.active != Some(identity)
        })
    }

    fn floor_rejects(&self, identity: CastIdentity) -> bool {
        self.av_supersession_floor.is_some_and(|floor| {
            floor.session() == identity.session()
                && identity.cast_instance_id <= floor.cast_instance_id
        })
    }

    fn mark_exhausted_if_max(&mut self, instance: u64) {
        if instance == u64::MAX {
            if let Some(gate) = &mut self.gate {
                gate.exhausted = true;
            }
        }
    }

    fn raise_floor_for_active(&mut self, identity: CastIdentity) {
        if self.active == Some(identity) {
            self.av_supersession_floor = Some(match self.av_supersession_floor {
                Some(current) if current.session() == identity.session() => {
                    if current.cast_instance_id >= identity.cast_instance_id {
                        current
                    } else {
                        identity
                    }
                }
                _ => identity,
            });
        }
    }

    fn stop_owner_for_identity(&mut self, identity: CastIdentity, effects: &mut Vec<CastEffect>) {
        let Some(owner) = self.av_owner else {
            return;
        };
        if owner.identity != identity {
            return;
        }
        let binding = self.av_owner_binding_key.take().unwrap_or_default();
        let animation = self.av_owner_animation_id.take().unwrap_or_default();
        self.av_owner = None;
        self.insert_tombstone(identity, binding.clone(), animation.clone());
        self.raise_floor_for_active(identity);
        effects.push(CastEffect::StopAv {
            identity,
            av_binding_key: binding,
            animation_id: animation,
        });
    }

    fn clear_av_owner(&mut self) {
        self.av_owner = None;
        self.av_owner_binding_key = None;
        self.av_owner_animation_id = None;
    }

    fn clear_av_state(&mut self) {
        self.clear_av_owner();
        self.av_tombstones.clear();
        self.av_supersession_floor = None;
        self.pending_pre_admission_stop = None;
    }

    fn av_tombstone_exists(&self, identity: &CastIdentity, binding: &str, animation: &str) -> bool {
        self.av_tombstones
            .contains_key(&(*identity, binding.to_owned(), animation.to_owned()))
    }

    fn insert_tombstone(&mut self, identity: CastIdentity, binding: String, animation: String) {
        self.av_tombstones.insert(
            (identity, binding.clone(), animation.clone()),
            AvTombstone {
                identity,
                av_binding_key: binding,
                animation_id: animation,
            },
        );
        while self.av_tombstones.len() > AV_TOMBSTONE_CAPACITY {
            let Some(oldest) = self.av_tombstones.keys().next().cloned() else {
                break;
            };
            self.av_tombstones.remove(&oldest);
        }
    }

    fn evict_terminal_records_if_needed(&mut self) {
        while self.terminal_records.len() > TERMINAL_RECORD_CAPACITY {
            let Some(oldest) = self.terminal_records.keys().next().copied() else {
                break;
            };
            self.terminal_records.remove(&oldest);
        }
    }
}

fn stable_text_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn ids() -> (CastCaster, CastSessionId) {
        (
            CastCaster::Player(Uuid::from_u128(1)),
            CastSessionId::new(Uuid::from_u128(2), 1).expect("valid session"),
        )
    }

    fn identity(instance: u64) -> CastIdentity {
        CastIdentity::new(Uuid::from_u128(2), 1, instance).expect("valid identity")
    }

    fn begin_open(state: &mut CastState) {
        let (caster, session) = ids();
        assert!(state
            .reduce(CastMessage::Begin(CastBegin::open(caster, session)))
            .disposition
            .is_accepted());
    }

    #[test]
    fn begin_advertised_active_waits_for_casting() {
        let (caster, session) = ids();
        let mut state = CastState::default();
        state.reduce(CastMessage::Begin(
            CastBegin::advertised_active(caster, session, 7, false).unwrap(),
        ));
        assert_eq!(state.lifecycle(), CastLifecycle::Reserved);
        assert_eq!(state.active, None);
        let result = state.reduce(CastMessage::Casting {
            identity: identity(7),
        });
        assert_eq!(result.rule, "R-06");
        assert_eq!(state.active, Some(identity(7)));
        assert_eq!(state.reserved, None);
    }

    #[test]
    fn reject_does_not_clear_unrelated_active_cast() {
        let mut state = CastState::default();
        begin_open(&mut state);
        state.reduce(CastMessage::Casting {
            identity: identity(1),
        });
        let result = state.reduce(CastMessage::Reject {
            identity: identity(2),
            outcome: CastOutcome::RejectOnCooldown,
        });
        assert_eq!(result.rule, "R-04");
        assert_eq!(state.active, Some(identity(1)));
        assert_eq!(state.latest_feedback.unwrap().identity, identity(2));
    }

    #[test]
    fn stop_never_creates_terminal_and_pending_stop_is_consumed_by_casting() {
        let (caster, session) = ids();
        let mut state = CastState::default();
        state.reduce(CastMessage::Begin(
            CastBegin::advertised_active(caster, session, 9, false).unwrap(),
        ));
        state.reduce(CastMessage::Stop {
            identity: identity(9),
            av_binding_key: "contract/test".to_string(),
            animation_id: "cast".to_string(),
        });
        assert!(state.terminal_records.is_empty());
        assert_eq!(state.pending_pre_admission_stop, Some(identity(9)));
        state.reduce(CastMessage::Casting {
            identity: identity(9),
        });
        assert_eq!(state.pending_pre_admission_stop, None);
        let result = state.reduce(CastMessage::Play {
            identity: identity(9),
            av_binding_key: "contract/test".to_string(),
            animation_id: "cast".to_string(),
        });
        assert!(result.disposition.is_accepted());
    }

    #[test]
    fn terminal_supersession_closes_old_active_atomically() {
        let mut state = CastState::default();
        begin_open(&mut state);
        state.reduce(CastMessage::Casting {
            identity: identity(7),
        });
        let result = state.reduce(CastMessage::Complete {
            identity: identity(8),
        });
        assert_eq!(result.rule, "R-08 supersession");
        assert_eq!(state.active, None);
        assert_eq!(
            state.terminal_records.get(&identity(7)),
            Some(&CastOutcome::InterruptControl)
        );
        assert_eq!(
            state.terminal_records.get(&identity(8)),
            Some(&CastOutcome::Completed)
        );
        assert_eq!(
            result.effects.len(),
            2,
            "旧 active interrupt + 新 terminal 必须同一回执"
        );
    }

    #[test]
    fn terminal_replay_is_idempotent_and_stop_does_not_replay_outcome() {
        let mut state = CastState::default();
        begin_open(&mut state);
        state.reduce(CastMessage::Casting {
            identity: identity(1),
        });
        state.reduce(CastMessage::Complete {
            identity: identity(1),
        });
        let replay = state.reduce(CastMessage::Complete {
            identity: identity(1),
        });
        assert!(replay.disposition.is_idempotent());
        let stop = state.reduce(CastMessage::Stop {
            identity: identity(1),
            av_binding_key: "contract/test".to_string(),
            animation_id: "cast".to_string(),
        });
        assert!(stop.disposition.is_accepted());
        assert_eq!(state.terminal_records.len(), 1);
    }

    #[test]
    fn bounded_terminal_and_tombstone_churn_preserves_high_water() {
        let mut state = CastState::default();
        begin_open(&mut state);
        for instance in 1..=(TERMINAL_RECORD_CAPACITY as u64 + 4) {
            state.reduce(CastMessage::Complete {
                identity: identity(instance),
            });
        }
        assert_eq!(state.terminal_records.len(), TERMINAL_RECORD_CAPACITY);
        assert_eq!(
            state.terminal_high_water,
            TERMINAL_RECORD_CAPACITY as u64 + 4
        );
        assert!(state
            .reduce(CastMessage::Complete {
                identity: identity(1)
            })
            .disposition
            .is_idempotent());

        for instance in 1..=(AV_TOMBSTONE_CAPACITY as u64 + 4) {
            state.reduce(CastMessage::Stop {
                identity: identity(instance),
                av_binding_key: format!("contract/{instance}"),
                animation_id: "cast".to_string(),
            });
        }
        assert_eq!(state.av_tombstones.len(), AV_TOMBSTONE_CAPACITY);
    }

    #[test]
    fn unload_clears_all_four_partitions_without_terminal_outcome() {
        let mut state = CastState::default();
        begin_open(&mut state);
        state.reduce(CastMessage::Casting {
            identity: identity(1),
        });
        state.reduce(CastMessage::Reject {
            identity: identity(2),
            outcome: CastOutcome::RejectNoWeapon,
        });
        let result = state.reduce(CastMessage::Unload);
        assert_eq!(result.rule, "R-15");
        assert!(state.gate.is_none());
        assert!(state.rejected_attempts.is_empty());
        assert!(state.terminal_records.is_empty());
        assert!(state.av_tombstones.is_empty());
    }

    #[test]
    fn exhausted_begin_allows_only_advertised_identity_and_no_new_attempt() {
        let (caster, session) = ids();
        let mut state = CastState::default();
        state.reduce(CastMessage::Begin(
            CastBegin::advertised_active(caster, session, 7, true).unwrap(),
        ));
        let reject = state.reduce(CastMessage::Reject {
            identity: identity(8),
            outcome: CastOutcome::RejectOnCooldown,
        });
        assert_eq!(reject.rule, "R-14 exhausted admission");
        let casting = state.reduce(CastMessage::Casting {
            identity: identity(7),
        });
        assert_eq!(casting.rule, "R-06");
        assert_eq!(state.active, Some(identity(7)));
    }
}
