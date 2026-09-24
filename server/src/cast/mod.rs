//! R9 P1 C-13：canonical cast registration contract。
//!
//! This module is deliberately inert.  It freezes the registration key space, the complete
//! player/NPC AV shapes, the one lookup, and the one AV consumer interface for the later Wave 2
//! activation.  It does not install a Bevy resource, call a producer, or replace the legacy
//! [`crate::cultivation::skill_registry::SkillRegistry`].  The latter remains the live resolver
//! registry until the full atomic activation described by the R9 plan.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::cultivation::known_techniques::{parse_required_realm, TechniqueDefinition};
use crate::cultivation::skill_registry::SkillFn;

/// The two disjoint identities accepted by the canonical registration lookup.
///
/// A quick-slot cast is represented by [`RegistrationKey::ItemCast`], never by a skill id.  The
/// owned strings make lookup safe for request-derived input while registrations themselves keep
/// their canonical declaration strings as `&'static str`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RegistrationKey {
    Skill {
        skill_id: String,
    },
    ItemCast {
        item_template_id: String,
        cast_variant: String,
    },
}

impl RegistrationKey {
    pub fn skill(skill_id: impl Into<String>) -> Self {
        Self::Skill {
            skill_id: skill_id.into(),
        }
    }

    pub fn item_cast(item_template_id: impl Into<String>, cast_variant: impl Into<String>) -> Self {
        Self::ItemCast {
            item_template_id: item_template_id.into(),
            cast_variant: cast_variant.into(),
        }
    }

    pub fn skill_id(&self) -> Option<&str> {
        match self {
            Self::Skill { skill_id } => Some(skill_id),
            Self::ItemCast { .. } => None,
        }
    }

    pub fn item_cast_parts(&self) -> Option<(&str, &str)> {
        match self {
            Self::Skill { .. } => None,
            Self::ItemCast {
                item_template_id,
                cast_variant,
            } => Some((item_template_id, cast_variant)),
        }
    }
}

impl fmt::Display for RegistrationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Skill { skill_id } => write!(f, "skill:{skill_id}"),
            Self::ItemCast {
                item_template_id,
                cast_variant,
            } => write!(f, "item-cast:{item_template_id}/{cast_variant}"),
        }
    }
}

/// Discriminator used in validation errors and contract pins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegistrationEntryKind {
    Skill,
    ItemCast,
}

/// Whether a skill entry is driven by a resolver or by one official dedicated handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillCastMode {
    Resolver,
    Dedicated { handler: DedicatedHandlerId },
}

/// Canonical identifier for a dedicated gameplay consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DedicatedHandlerId(&'static str);

impl DedicatedHandlerId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DedicatedHandlerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Registration audience.  The visual arm must agree with this discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillAudience {
    Player,
    Npc,
    Both,
}

/// The four lifecycle phase slots consumed by the canonical AV interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillAvPhase {
    Start,
    Release,
    Complete,
    Interrupt,
}

/// The five binding channels whose ownership must remain canonical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillAvChannel {
    Animation,
    Vfx,
    Sfx,
    Hud,
    Icon,
}

/// Phase-specific semantic IDs.  `None` is an explicit not-applicable phase, not a fallback
/// request to infer an effect in a resolver or router.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillAvPhaseBinding {
    pub start: Option<&'static str>,
    pub release: Option<&'static str>,
    pub complete: Option<&'static str>,
    pub interrupt: Option<&'static str>,
}

impl SkillAvPhaseBinding {
    pub const fn new(
        start: Option<&'static str>,
        release: Option<&'static str>,
        complete: Option<&'static str>,
        interrupt: Option<&'static str>,
    ) -> Self {
        Self {
            start,
            release,
            complete,
            interrupt,
        }
    }

    pub const fn empty() -> Self {
        Self::new(None, None, None, None)
    }

    pub fn value_for(self, phase: SkillAvPhase) -> Option<&'static str> {
        match phase {
            SkillAvPhase::Start => self.start,
            SkillAvPhase::Release => self.release,
            SkillAvPhase::Complete => self.complete,
            SkillAvPhase::Interrupt => self.interrupt,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.start.is_none()
            && self.release.is_none()
            && self.complete.is_none()
            && self.interrupt.is_none()
    }
}

/// Animation identity and loop ownership.  STOP/terminal semantics are intentionally not
/// represented here; the future consumer receives the full cast identity alongside this entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillAnimationBinding {
    pub start: &'static str,
    pub release: Option<&'static str>,
    pub looping: bool,
}

impl SkillAnimationBinding {
    pub const fn new(start: &'static str, release: Option<&'static str>, looping: bool) -> Self {
        Self {
            start,
            release,
            looping,
        }
    }
}

/// Capability declaration for a semantic VFX binding.  Iris remains an optional client
/// capability; this server-side contract never assumes it is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VfxCapabilityTier {
    Vanilla,
    ShaderOptional {
        iris_effect_id: &'static str,
        fallback: VfxFallback,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VfxFallback {
    VanillaParticle(&'static str),
    NoOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillVfxBinding {
    pub phases: SkillAvPhaseBinding,
    pub capability: VfxCapabilityTier,
}

impl SkillVfxBinding {
    pub const fn vanilla(phases: SkillAvPhaseBinding) -> Self {
        Self {
            phases,
            capability: VfxCapabilityTier::Vanilla,
        }
    }
}

/// The only explicit placeholder permitted by the P1 contract.  The blocker is retained in the
/// declaration so P3 can audit and remove it; animation/VFX/SFX/HUD have no placeholder variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillIconBinding {
    Asset(&'static str),
    ExplicitPlaceholder {
        asset: &'static str,
        blocker: &'static str,
    },
}

impl SkillIconBinding {
    pub const fn asset(asset: &'static str) -> Self {
        Self::Asset(asset)
    }

    pub const fn placeholder(asset: &'static str, blocker: &'static str) -> Self {
        Self::ExplicitPlaceholder { asset, blocker }
    }
}

/// The complete five-piece player-facing binding required by every player or Both skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SkillAvBinding {
    pub animation: SkillAnimationBinding,
    pub vfx: SkillVfxBinding,
    pub audio: SkillAvPhaseBinding,
    pub hud: SkillAvPhaseBinding,
    pub icon: SkillIconBinding,
}

impl SkillAvBinding {
    pub const fn new(
        animation: SkillAnimationBinding,
        vfx: SkillVfxBinding,
        audio: SkillAvPhaseBinding,
        hud: SkillAvPhaseBinding,
        icon: SkillIconBinding,
    ) -> Self {
        Self {
            animation,
            vfx,
            audio,
            hud,
            icon,
        }
    }
}

/// Explicit NPC-only animation exemption.  An NPC with no animation still carries a typed
/// `NotApplicable` arm; an empty player animation string is never accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NpcAnimationBinding {
    NotApplicable,
    Binding(SkillAnimationBinding),
}

/// NPC HUD/icon exemptions are type-level explicit.  They cannot silently borrow a player
/// skill's HUD or icon channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NpcHudBinding {
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NpcIconBinding {
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NpcVisualBinding {
    pub animation: NpcAnimationBinding,
    pub vfx: SkillVfxBinding,
    pub audio: SkillAvPhaseBinding,
    pub hud: NpcHudBinding,
    pub icon: NpcIconBinding,
}

impl NpcVisualBinding {
    pub const fn new(
        animation: NpcAnimationBinding,
        vfx: SkillVfxBinding,
        audio: SkillAvPhaseBinding,
    ) -> Self {
        Self {
            animation,
            vfx,
            audio,
            hud: NpcHudBinding::NotApplicable,
            icon: NpcIconBinding::NotApplicable,
        }
    }
}

/// Player/NPC visual arm.  Item-cast entries are constrained to `Player` by the ledger because
/// QUICK_SLOT is a player source and has no NPC arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillVisualBinding {
    Player(SkillAvBinding),
    Npc(NpcVisualBinding),
    Both {
        player: SkillAvBinding,
        npc: NpcVisualBinding,
    },
}

impl SkillVisualBinding {
    pub const fn player(binding: SkillAvBinding) -> Self {
        Self::Player(binding)
    }

    pub const fn npc(binding: NpcVisualBinding) -> Self {
        Self::Npc(binding)
    }

    pub const fn both(player: SkillAvBinding, npc: NpcVisualBinding) -> Self {
        Self::Both { player, npc }
    }

    pub fn player_binding(&self) -> Option<&SkillAvBinding> {
        match self {
            Self::Player(binding) => Some(binding),
            Self::Npc(_) => None,
            Self::Both { player, .. } => Some(player),
        }
    }

    pub fn npc_binding(&self) -> Option<&NpcVisualBinding> {
        match self {
            Self::Player(_) => None,
            Self::Npc(binding) => Some(binding),
            Self::Both { npc, .. } => Some(npc),
        }
    }
}

/// One canonical registration declaration.  The two enum arms intentionally make it impossible
/// for an item-cast key to carry a guessed/implicit `skill_id` or a resolver.
// The enum deliberately keeps the complete contract shape together.  Boxing one arm would make
// the frozen declaration less legible without changing the inert ledger's ownership rules.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum SkillRegistration {
    Skill {
        skill_id: &'static str,
        definition: TechniqueDefinition,
        resolver: Option<SkillFn>,
        cast_mode: SkillCastMode,
        audience: SkillAudience,
        av_binding_key: &'static str,
        av: SkillVisualBinding,
    },
    ItemCast {
        item_template_id: &'static str,
        cast_variant: &'static str,
        av_binding_key: &'static str,
        av: SkillVisualBinding,
    },
}

impl SkillRegistration {
    pub const fn skill(
        skill_id: &'static str,
        definition: TechniqueDefinition,
        resolver: Option<SkillFn>,
        cast_mode: SkillCastMode,
        audience: SkillAudience,
        av_binding_key: &'static str,
        av: SkillVisualBinding,
    ) -> Self {
        Self::Skill {
            skill_id,
            definition,
            resolver,
            cast_mode,
            audience,
            av_binding_key,
            av,
        }
    }

    pub const fn item_cast(
        item_template_id: &'static str,
        cast_variant: &'static str,
        av_binding_key: &'static str,
        av: SkillVisualBinding,
    ) -> Self {
        Self::ItemCast {
            item_template_id,
            cast_variant,
            av_binding_key,
            av,
        }
    }

    pub fn key(&self) -> RegistrationKey {
        match self {
            Self::Skill { skill_id, .. } => RegistrationKey::skill(*skill_id),
            Self::ItemCast {
                item_template_id,
                cast_variant,
                ..
            } => RegistrationKey::item_cast(*item_template_id, *cast_variant),
        }
    }

    pub const fn entry_kind(&self) -> RegistrationEntryKind {
        match self {
            Self::Skill { .. } => RegistrationEntryKind::Skill,
            Self::ItemCast { .. } => RegistrationEntryKind::ItemCast,
        }
    }

    /// `None` is a contract guarantee for item-cast/QUICK_SLOT entries.
    pub const fn skill_id(&self) -> Option<&'static str> {
        match self {
            Self::Skill { skill_id, .. } => Some(*skill_id),
            Self::ItemCast { .. } => None,
        }
    }

    pub const fn av_binding_key(&self) -> &'static str {
        match self {
            Self::Skill { av_binding_key, .. } | Self::ItemCast { av_binding_key, .. } => {
                av_binding_key
            }
        }
    }

    pub const fn av(&self) -> &SkillVisualBinding {
        match self {
            Self::Skill { av, .. } | Self::ItemCast { av, .. } => av,
        }
    }

    pub fn definition(&self) -> Option<&TechniqueDefinition> {
        match self {
            Self::Skill { definition, .. } => Some(definition),
            Self::ItemCast { .. } => None,
        }
    }

    pub const fn resolver(&self) -> Option<SkillFn> {
        match self {
            Self::Skill { resolver, .. } => *resolver,
            Self::ItemCast { .. } => None,
        }
    }

    pub const fn cast_mode(&self) -> Option<SkillCastMode> {
        match self {
            Self::Skill { cast_mode, .. } => Some(*cast_mode),
            Self::ItemCast { .. } => None,
        }
    }

    pub const fn audience(&self) -> Option<SkillAudience> {
        match self {
            Self::Skill { audience, .. } => Some(*audience),
            Self::ItemCast { .. } => None,
        }
    }
}

/// Inert ledger state.  There is intentionally no `Live` variant in the P1 contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InertEntryState {
    Declared,
    Unwired,
    TestOnly,
}

#[derive(Debug, Clone)]
pub struct InertRegistrationEntry {
    pub registration: SkillRegistration,
    pub state: InertEntryState,
}

impl InertRegistrationEntry {
    pub fn key(&self) -> RegistrationKey {
        self.registration.key()
    }
}

/// A request handed to the one future AV consumer interface.  The request carries the resolved
/// canonical key and binding together; a consumer must not infer either from `skill_id`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillAvRequest<'a> {
    pub key: &'a RegistrationKey,
    pub phase: SkillAvPhase,
    pub binding: &'a SkillVisualBinding,
}

/// The sole AV consumer seam frozen by C-13.  P1 intentionally provides no implementation and
/// no producer call site; Wave 2 installs exactly one concrete consumer behind this interface.
pub trait SkillAvConsumer {
    fn consume(&mut self, request: SkillAvRequest<'_>);
}

/// Fail-fast validation errors.  Errors are returned before any ledger map/vector is mutated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    EmptyField {
        entry: RegistrationEntryKind,
        field: &'static str,
    },
    InvalidField {
        entry: RegistrationEntryKind,
        field: &'static str,
        reason: String,
    },
    DuplicateKey {
        key: RegistrationKey,
    },
    DuplicateAvBindingKey {
        key: String,
        existing: RegistrationKey,
        duplicate: RegistrationKey,
    },
    DuplicateAvChannel {
        channel: SkillAvChannel,
        existing: RegistrationKey,
        duplicate: RegistrationKey,
    },
    DefinitionKeyMismatch {
        skill_id: String,
        definition_id: String,
    },
    ResolverMissing {
        skill_id: String,
    },
    ResolverUnexpected {
        skill_id: String,
    },
    DedicatedHandlerMissing {
        skill_id: String,
    },
    AudienceVisualMismatch {
        skill_id: String,
        audience: SkillAudience,
        visual: RegistrationEntryKind,
    },
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { entry, field } => {
                write!(f, "empty {entry:?} registration field {field}")
            }
            Self::InvalidField {
                entry,
                field,
                reason,
            } => write!(f, "invalid {entry:?} registration field {field}: {reason}"),
            Self::DuplicateKey { key } => write!(f, "duplicate registration key {key}"),
            Self::DuplicateAvBindingKey {
                key,
                existing,
                duplicate,
            } => write!(
                f,
                "duplicate AV binding key {key:?}: {existing} and {duplicate}"
            ),
            Self::DuplicateAvChannel {
                channel,
                existing,
                duplicate,
            } => write!(
                f,
                "duplicate player AV {channel:?} channel: {existing} and {duplicate}"
            ),
            Self::DefinitionKeyMismatch {
                skill_id,
                definition_id,
            } => write!(
                f,
                "skill registration key {skill_id:?} does not match definition id {definition_id:?}"
            ),
            Self::ResolverMissing { skill_id } => {
                write!(f, "resolver mode requires a resolver for {skill_id:?}")
            }
            Self::ResolverUnexpected { skill_id } => write!(
                f,
                "dedicated mode must not carry a resolver for {skill_id:?}"
            ),
            Self::DedicatedHandlerMissing { skill_id } => write!(
                f,
                "dedicated mode requires exactly one non-empty handler for {skill_id:?}"
            ),
            Self::AudienceVisualMismatch {
                skill_id,
                audience,
                visual,
            } => write!(
                f,
                "audience {audience:?} does not match visual arm {visual:?} for {skill_id:?}"
            ),
        }
    }
}

impl std::error::Error for RegistrationError {}

/// Canonical C-13 inert registration lookup and entry ledger.
///
/// `by_key` is the only lookup index.  Item casts do not have a second item→skill map, and this
/// ledger is not installed as a production resource in P1.
#[derive(Debug, Default)]
pub struct SkillRegistrationLedger {
    entries: Vec<InertRegistrationEntry>,
    by_key: HashMap<RegistrationKey, usize>,
    by_av_binding_key: HashMap<&'static str, usize>,
}

impl SkillRegistrationLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a declared inert entry.  This convenience method never activates it.
    pub fn register(&mut self, registration: SkillRegistration) -> Result<(), RegistrationError> {
        self.register_inert(registration, InertEntryState::Declared)
    }

    pub fn register_inert(
        &mut self,
        registration: SkillRegistration,
        state: InertEntryState,
    ) -> Result<(), RegistrationError> {
        validate_registration(&registration)?;
        let key = registration.key();
        if self.by_key.contains_key(&key) {
            return Err(RegistrationError::DuplicateKey { key });
        }

        if let Some(index) = self
            .by_av_binding_key
            .get(registration.av_binding_key())
            .copied()
        {
            return Err(RegistrationError::DuplicateAvBindingKey {
                key: registration.av_binding_key().to_string(),
                existing: self.entries[index].key(),
                duplicate: key,
            });
        }

        if let Some(new_player_binding) = registration.av().player_binding() {
            for existing in &self.entries {
                let Some(existing_player_binding) = existing.registration.av().player_binding()
                else {
                    continue;
                };
                for (channel, same) in [
                    (
                        SkillAvChannel::Animation,
                        existing_player_binding.animation == new_player_binding.animation,
                    ),
                    (
                        SkillAvChannel::Vfx,
                        existing_player_binding.vfx == new_player_binding.vfx,
                    ),
                    (
                        SkillAvChannel::Sfx,
                        existing_player_binding.audio == new_player_binding.audio,
                    ),
                    (
                        SkillAvChannel::Hud,
                        existing_player_binding.hud == new_player_binding.hud,
                    ),
                    (
                        SkillAvChannel::Icon,
                        existing_player_binding.icon == new_player_binding.icon,
                    ),
                ] {
                    if same {
                        return Err(RegistrationError::DuplicateAvChannel {
                            channel,
                            existing: existing.key(),
                            duplicate: key,
                        });
                    }
                }
            }
        }

        let index = self.entries.len();
        self.by_key.insert(key, index);
        self.by_av_binding_key
            .insert(registration.av_binding_key(), index);
        self.entries.push(InertRegistrationEntry {
            registration,
            state,
        });
        Ok(())
    }

    pub fn register_unwired(
        &mut self,
        registration: SkillRegistration,
    ) -> Result<(), RegistrationError> {
        self.register_inert(registration, InertEntryState::Unwired)
    }

    pub fn register_test_only(
        &mut self,
        registration: SkillRegistration,
    ) -> Result<(), RegistrationError> {
        self.register_inert(registration, InertEntryState::TestOnly)
    }

    /// The only canonical key lookup.  The convenience lookups below construct this same key and
    /// delegate here; they are not parallel registries.
    pub fn lookup(&self, key: &RegistrationKey) -> Option<&SkillRegistration> {
        self.by_key
            .get(key)
            .and_then(|index| self.entries.get(*index))
            .map(|entry| &entry.registration)
    }

    pub fn lookup_entry(&self, key: &RegistrationKey) -> Option<&InertRegistrationEntry> {
        self.by_key
            .get(key)
            .and_then(|index| self.entries.get(*index))
    }

    pub fn lookup_skill(&self, skill_id: &str) -> Option<&SkillRegistration> {
        self.lookup(&RegistrationKey::skill(skill_id))
    }

    pub fn lookup_item_cast(
        &self,
        item_template_id: &str,
        cast_variant: &str,
    ) -> Option<&SkillRegistration> {
        self.lookup(&RegistrationKey::item_cast(item_template_id, cast_variant))
    }

    pub fn av_request<'a>(
        &'a self,
        key: &'a RegistrationKey,
        phase: SkillAvPhase,
    ) -> Option<SkillAvRequest<'a>> {
        let entry = self.lookup_entry(key)?;
        Some(SkillAvRequest {
            key,
            phase,
            binding: entry.registration.av(),
        })
    }

    pub fn entries(&self) -> &[InertRegistrationEntry] {
        &self.entries
    }

    pub fn iter(&self) -> impl Iterator<Item = &InertRegistrationEntry> {
        self.entries.iter()
    }

    pub fn count_state(&self, state: InertEntryState) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state == state)
            .count()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

fn validate_registration(registration: &SkillRegistration) -> Result<(), RegistrationError> {
    match registration {
        SkillRegistration::Skill {
            skill_id,
            definition,
            resolver,
            cast_mode,
            audience,
            av_binding_key,
            av,
        } => {
            validate_token(RegistrationEntryKind::Skill, "skill_id", skill_id)?;
            validate_token(
                RegistrationEntryKind::Skill,
                "av_binding_key",
                av_binding_key,
            )?;
            validate_definition(definition)?;
            if definition.id != *skill_id {
                return Err(RegistrationError::DefinitionKeyMismatch {
                    skill_id: (*skill_id).to_string(),
                    definition_id: definition.id.clone(),
                });
            }
            match cast_mode {
                SkillCastMode::Resolver if resolver.is_none() => {
                    return Err(RegistrationError::ResolverMissing {
                        skill_id: (*skill_id).to_string(),
                    });
                }
                SkillCastMode::Dedicated { handler } => {
                    if resolver.is_some() {
                        return Err(RegistrationError::ResolverUnexpected {
                            skill_id: (*skill_id).to_string(),
                        });
                    }
                    if handler.as_str().trim().is_empty() {
                        return Err(RegistrationError::DedicatedHandlerMissing {
                            skill_id: (*skill_id).to_string(),
                        });
                    }
                    validate_token(
                        RegistrationEntryKind::Skill,
                        "cast_mode.handler",
                        handler.as_str(),
                    )?;
                }
                SkillCastMode::Resolver => {}
            }
            validate_visual_for_audience(skill_id, *audience, av)
        }
        SkillRegistration::ItemCast {
            item_template_id,
            cast_variant,
            av_binding_key,
            av,
        } => {
            validate_token(
                RegistrationEntryKind::ItemCast,
                "item_template_id",
                item_template_id,
            )?;
            validate_token(
                RegistrationEntryKind::ItemCast,
                "cast_variant",
                cast_variant,
            )?;
            validate_token(
                RegistrationEntryKind::ItemCast,
                "av_binding_key",
                av_binding_key,
            )?;
            if !matches!(av, SkillVisualBinding::Player(_)) {
                return Err(RegistrationError::AudienceVisualMismatch {
                    skill_id: format!("{item_template_id}/{cast_variant}"),
                    audience: SkillAudience::Player,
                    visual: RegistrationEntryKind::ItemCast,
                });
            }
            validate_player_binding(av.player_binding().expect("Player arm checked"))
        }
    }
}

fn validate_visual_for_audience(
    skill_id: &'static str,
    audience: SkillAudience,
    av: &SkillVisualBinding,
) -> Result<(), RegistrationError> {
    let matches = match (audience, av) {
        (SkillAudience::Player, SkillVisualBinding::Player(binding)) => {
            validate_player_binding(binding)?;
            true
        }
        (SkillAudience::Npc, SkillVisualBinding::Npc(binding)) => {
            validate_npc_binding(binding)?;
            true
        }
        (SkillAudience::Both, SkillVisualBinding::Both { player, npc }) => {
            validate_player_binding(player)?;
            validate_npc_binding(npc)?;
            true
        }
        _ => false,
    };
    if matches {
        Ok(())
    } else {
        Err(RegistrationError::AudienceVisualMismatch {
            skill_id: skill_id.to_string(),
            audience,
            visual: RegistrationEntryKind::Skill,
        })
    }
}

fn validate_definition(definition: &TechniqueDefinition) -> Result<(), RegistrationError> {
    for (field, value) in [
        ("definition.id", definition.id.as_str()),
        ("definition.grade", definition.grade.as_str()),
        (
            "definition.required_realm",
            definition.required_realm.as_str(),
        ),
    ] {
        validate_token(RegistrationEntryKind::Skill, field, value)?;
    }
    for (field, value) in [
        ("definition.display_name", definition.display_name.as_str()),
        ("definition.description", definition.description.as_str()),
    ] {
        validate_text(RegistrationEntryKind::Skill, field, value)?;
    }
    if parse_required_realm(&definition.required_realm).is_none() {
        return Err(RegistrationError::InvalidField {
            entry: RegistrationEntryKind::Skill,
            field: "definition.required_realm",
            reason: format!("unknown realm {:?}", definition.required_realm),
        });
    }
    if !matches!(
        definition.grade.as_str(),
        "common" | "yellow" | "profound" | "earth" | "rare"
    ) {
        return Err(RegistrationError::InvalidField {
            entry: RegistrationEntryKind::Skill,
            field: "definition.grade",
            reason: format!("unknown grade {:?}", definition.grade),
        });
    }
    for (field, value) in [
        ("definition.qi_cost", definition.qi_cost),
        (
            "definition.stamina_cost",
            f64::from(definition.stamina_cost),
        ),
        ("definition.range", f64::from(definition.range)),
    ] {
        if !value.is_finite() || value < 0.0 {
            return Err(RegistrationError::InvalidField {
                entry: RegistrationEntryKind::Skill,
                field,
                reason: format!("must be finite and non-negative, got {value}"),
            });
        }
    }
    let mut seen_meridians = HashSet::new();
    for meridian in &definition.required_meridians {
        validate_token(
            RegistrationEntryKind::Skill,
            "definition.required_meridians[].channel",
            &meridian.channel,
        )?;
        let Some(parsed_channel) =
            crate::cultivation::technique_scroll::parse_meridian_id(&meridian.channel)
        else {
            return Err(RegistrationError::InvalidField {
                entry: RegistrationEntryKind::Skill,
                field: "definition.required_meridians[].channel",
                reason: format!("unknown meridian {:?}", meridian.channel),
            });
        };
        if !seen_meridians.insert(parsed_channel) {
            return Err(RegistrationError::InvalidField {
                entry: RegistrationEntryKind::Skill,
                field: "definition.required_meridians[].channel",
                reason: format!("duplicate meridian {:?}", meridian.channel),
            });
        }
        if !meridian.min_health.is_finite()
            || meridian.min_health <= 0.0
            || meridian.min_health > 1.0
        {
            return Err(RegistrationError::InvalidField {
                entry: RegistrationEntryKind::Skill,
                field: "definition.required_meridians[].min_health",
                reason: format!("must be finite and in (0, 1], got {}", meridian.min_health),
            });
        }
    }
    Ok(())
}

fn validate_player_binding(binding: &SkillAvBinding) -> Result<(), RegistrationError> {
    validate_animation(&binding.animation, RegistrationEntryKind::Skill)?;
    validate_phases(
        &binding.vfx.phases,
        RegistrationEntryKind::Skill,
        "vfx.phases",
        true,
    )?;
    validate_vfx_capability(&binding.vfx.capability)?;
    validate_phases(&binding.audio, RegistrationEntryKind::Skill, "audio", true)?;
    validate_phases(&binding.hud, RegistrationEntryKind::Skill, "hud", true)?;
    validate_icon(&binding.icon, RegistrationEntryKind::Skill)
}

fn validate_npc_binding(binding: &NpcVisualBinding) -> Result<(), RegistrationError> {
    if let NpcAnimationBinding::Binding(animation) = binding.animation {
        validate_animation(&animation, RegistrationEntryKind::Skill)?;
    }
    validate_phases(
        &binding.vfx.phases,
        RegistrationEntryKind::Skill,
        "npc.vfx.phases",
        true,
    )?;
    validate_vfx_capability(&binding.vfx.capability)?;
    validate_phases(
        &binding.audio,
        RegistrationEntryKind::Skill,
        "npc.audio",
        true,
    )?;
    Ok(())
}

fn validate_animation(
    animation: &SkillAnimationBinding,
    entry: RegistrationEntryKind,
) -> Result<(), RegistrationError> {
    validate_token(entry, "animation.start", animation.start)?;
    if let Some(release) = animation.release {
        validate_token(entry, "animation.release", release)?;
    }
    Ok(())
}

fn validate_phases(
    phases: &SkillAvPhaseBinding,
    entry: RegistrationEntryKind,
    field: &'static str,
    require_one: bool,
) -> Result<(), RegistrationError> {
    let values = [
        phases.start,
        phases.release,
        phases.complete,
        phases.interrupt,
    ];
    for value in values.into_iter().flatten() {
        validate_token(entry, field, value)?;
    }
    if require_one && phases.is_empty() {
        return Err(RegistrationError::EmptyField { entry, field });
    }
    Ok(())
}

fn validate_vfx_capability(capability: &VfxCapabilityTier) -> Result<(), RegistrationError> {
    match capability {
        VfxCapabilityTier::Vanilla => Ok(()),
        VfxCapabilityTier::ShaderOptional {
            iris_effect_id,
            fallback,
        } => {
            validate_token(
                RegistrationEntryKind::Skill,
                "vfx.capability.iris_effect_id",
                iris_effect_id,
            )?;
            if let VfxFallback::VanillaParticle(particle) = fallback {
                validate_token(
                    RegistrationEntryKind::Skill,
                    "vfx.capability.fallback",
                    particle,
                )?;
            }
            Ok(())
        }
    }
}

fn validate_icon(
    icon: &SkillIconBinding,
    entry: RegistrationEntryKind,
) -> Result<(), RegistrationError> {
    match icon {
        SkillIconBinding::Asset(asset) => validate_token(entry, "icon.asset", asset),
        SkillIconBinding::ExplicitPlaceholder { asset, blocker } => {
            validate_token(entry, "icon.asset", asset)?;
            validate_token(entry, "icon.blocker", blocker)?;
            if !blocker.starts_with("[BLOCKED:") {
                return Err(RegistrationError::InvalidField {
                    entry,
                    field: "icon.blocker",
                    reason: "placeholder blocker must start with [BLOCKED:".to_string(),
                });
            }
            Ok(())
        }
    }
}

fn validate_token(
    entry: RegistrationEntryKind,
    field: &'static str,
    value: &str,
) -> Result<(), RegistrationError> {
    if value.trim().is_empty() {
        return Err(RegistrationError::EmptyField { entry, field });
    }
    if value
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(RegistrationError::InvalidField {
            entry,
            field,
            reason: "must not contain whitespace or control characters".to_string(),
        });
    }
    Ok(())
}

fn validate_text(
    entry: RegistrationEntryKind,
    field: &'static str,
    value: &str,
) -> Result<(), RegistrationError> {
    if value.trim().is_empty() {
        return Err(RegistrationError::EmptyField { entry, field });
    }
    if value.chars().any(char::is_control) {
        return Err(RegistrationError::InvalidField {
            entry,
            field,
            reason: "must not contain control characters".to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body_plan::RaceGateOwned;
    use crate::cultivation::known_techniques::{
        SkillCategory, TechniqueDispatch, TechniqueRequiredMeridian,
    };

    fn definition(id: &str) -> TechniqueDefinition {
        TechniqueDefinition {
            id: id.to_string(),
            display_name: format!("{id} display"),
            grade: "common".to_string(),
            description: format!("{id} description"),
            required_realm: "Awaken".to_string(),
            required_meridians: Vec::<TechniqueRequiredMeridian>::new(),
            required_race: RaceGateOwned::Any,
            qi_cost: 1.0,
            stamina_cost: 1.0,
            cast_ticks: 1,
            cooldown_ticks: 1,
            range: 1.0,
            // Legacy metadata remains outside this contract; the canonical icon is in `av`.
            icon_texture: "legacy:ignored.png".to_string(),
            category: SkillCategory::Attack,
            dispatch: TechniqueDispatch::MetadataBacked,
        }
    }

    fn resolver(
        _world: &mut valence::prelude::bevy_ecs::world::World,
        _caster: valence::prelude::Entity,
        _slot: u8,
        _target: Option<valence::prelude::Entity>,
    ) -> crate::cultivation::skill_registry::CastResult {
        crate::cultivation::skill_registry::CastResult::Interrupted
    }

    fn player_binding(suffix: &'static str) -> SkillVisualBinding {
        SkillVisualBinding::player(SkillAvBinding::new(
            SkillAnimationBinding::new(suffix, None, false),
            SkillVfxBinding::vanilla(SkillAvPhaseBinding::new(
                Some(suffix),
                None,
                Some(suffix),
                Some(suffix),
            )),
            SkillAvPhaseBinding::new(Some(suffix), None, Some(suffix), Some(suffix)),
            SkillAvPhaseBinding::new(Some(suffix), None, Some(suffix), Some(suffix)),
            SkillIconBinding::asset(suffix),
        ))
    }

    fn skill(id: &'static str, suffix: &'static str) -> SkillRegistration {
        SkillRegistration::skill(
            id,
            definition(id),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            suffix,
            player_binding(suffix),
        )
    }

    fn item(suffix: &'static str) -> SkillRegistration {
        SkillRegistration::item_cast("item.scroll", suffix, suffix, player_binding(suffix))
    }

    #[test]
    fn unified_lookup_resolves_skill_and_item_cast_from_one_ledger() {
        let mut ledger = SkillRegistrationLedger::new();
        ledger.register(skill("contract.alpha", "alpha")).unwrap();
        ledger.register(item("quick")).unwrap();

        let skill_key = RegistrationKey::skill("contract.alpha");
        let item_key = RegistrationKey::item_cast("item.scroll", "quick");
        assert_eq!(
            ledger.lookup(&skill_key).unwrap().skill_id(),
            Some("contract.alpha")
        );
        assert_eq!(
            ledger.lookup_skill("contract.alpha").unwrap().key(),
            skill_key
        );
        assert_eq!(
            ledger
                .lookup_item_cast("item.scroll", "quick")
                .unwrap()
                .key(),
            item_key
        );
        assert_eq!(ledger.len(), 2, "skill 与 item-cast 必须共存于唯一 lookup");
    }

    #[test]
    fn key_identity_and_quick_slot_skill_id_are_discriminated() {
        let skill_key = RegistrationKey::skill("item.scroll");
        let item_key = RegistrationKey::item_cast("item.scroll", "quick");
        assert_ne!(skill_key, item_key, "skill key 与 item-cast tuple 不能碰撞");
        assert_eq!(skill_key.skill_id(), Some("item.scroll"));
        assert_eq!(item_key.skill_id(), None);

        let item_registration = item("quick");
        assert_eq!(
            item_registration.skill_id(),
            None,
            "QUICK_SLOT 不得猜测 skill_id"
        );
        assert_eq!(
            item_registration.key().item_cast_parts(),
            Some(("item.scroll", "quick"))
        );
    }

    #[test]
    fn duplicate_key_fails_before_mutating_ledger() {
        let mut ledger = SkillRegistrationLedger::new();
        ledger.register(skill("contract.alpha", "alpha")).unwrap();
        let error = ledger
            .register(skill("contract.alpha", "alpha-2"))
            .unwrap_err();
        assert!(matches!(error, RegistrationError::DuplicateKey { .. }));
        assert_eq!(ledger.len(), 1, "duplicate 拒绝后不能留下半条 entry");
    }

    #[test]
    fn duplicate_av_binding_key_is_rejected_without_parallel_item_registry() {
        let mut ledger = SkillRegistrationLedger::new();
        ledger.register(skill("contract.alpha", "shared")).unwrap();
        let error = ledger.register(item("shared")).unwrap_err();
        assert!(matches!(
            error,
            RegistrationError::DuplicateAvBindingKey { .. }
        ));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn each_player_av_channel_must_have_unique_binding_value() {
        let mut ledger = SkillRegistrationLedger::new();
        ledger.register(skill("contract.alpha", "alpha")).unwrap();
        let duplicate_animation = SkillRegistration::skill(
            "contract.beta",
            definition("contract.beta"),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "beta-key",
            player_binding("alpha"),
        );
        let error = ledger.register(duplicate_animation).unwrap_err();
        assert!(matches!(
            error,
            RegistrationError::DuplicateAvChannel {
                channel: SkillAvChannel::Animation,
                ..
            }
        ));
    }

    #[test]
    fn resolver_and_dedicated_constraints_are_fail_fast() {
        let missing_resolver = SkillRegistration::skill(
            "contract.missing-resolver",
            definition("contract.missing-resolver"),
            None,
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "missing-resolver",
            player_binding("missing-resolver"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(missing_resolver),
            Err(RegistrationError::ResolverMissing { .. })
        ));

        let unexpected_resolver = SkillRegistration::skill(
            "contract.unexpected-resolver",
            definition("contract.unexpected-resolver"),
            Some(resolver),
            SkillCastMode::Dedicated {
                handler: DedicatedHandlerId::new("official.handler"),
            },
            SkillAudience::Player,
            "unexpected-resolver",
            player_binding("unexpected-resolver"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(unexpected_resolver),
            Err(RegistrationError::ResolverUnexpected { .. })
        ));

        let missing_handler = SkillRegistration::skill(
            "contract.missing-handler",
            definition("contract.missing-handler"),
            None,
            SkillCastMode::Dedicated {
                handler: DedicatedHandlerId::new(""),
            },
            SkillAudience::Player,
            "missing-handler",
            player_binding("missing-handler"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(missing_handler),
            Err(RegistrationError::DedicatedHandlerMissing { .. })
        ));
    }

    #[test]
    fn definition_key_and_invalid_fields_fail_fast() {
        let mismatch = SkillRegistration::skill(
            "contract.key",
            definition("contract.other"),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "mismatch",
            player_binding("mismatch"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(mismatch),
            Err(RegistrationError::DefinitionKeyMismatch { .. })
        ));

        let empty_id = SkillRegistration::skill(
            "",
            definition(""),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "empty-id",
            player_binding("empty-id"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(empty_id),
            Err(RegistrationError::EmptyField {
                field: "skill_id",
                ..
            })
        ));

        let invalid_phase = SkillRegistration::skill(
            "contract.invalid-phase",
            definition("contract.invalid-phase"),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "invalid-phase",
            SkillVisualBinding::player(SkillAvBinding::new(
                SkillAnimationBinding::new("invalid-phase", None, false),
                SkillVfxBinding::vanilla(SkillAvPhaseBinding::empty()),
                SkillAvPhaseBinding::new(Some("invalid-phase"), None, None, None),
                SkillAvPhaseBinding::new(Some("invalid-phase"), None, None, None),
                SkillIconBinding::asset("invalid-phase"),
            )),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(invalid_phase),
            Err(RegistrationError::EmptyField {
                field: "vfx.phases",
                ..
            })
        ));

        let mut invalid_meridian_definition = definition("contract.invalid-meridian");
        invalid_meridian_definition
            .required_meridians
            .push(TechniqueRequiredMeridian {
                channel: "unknown_meridian".to_string(),
                min_health: 1.0,
            });
        let invalid_meridian = SkillRegistration::skill(
            "contract.invalid-meridian",
            invalid_meridian_definition,
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Player,
            "invalid-meridian",
            player_binding("invalid-meridian"),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(invalid_meridian),
            Err(RegistrationError::InvalidField {
                field: "definition.required_meridians[].channel",
                ..
            })
        ));
    }

    #[test]
    fn item_cast_and_placeholder_inputs_fail_closed() {
        let empty_item =
            SkillRegistration::item_cast("", "quick", "empty-item", player_binding("empty-item"));
        assert!(matches!(
            SkillRegistrationLedger::new().register(empty_item),
            Err(RegistrationError::EmptyField {
                entry: RegistrationEntryKind::ItemCast,
                field: "item_template_id",
            })
        ));

        let invalid_placeholder = SkillRegistration::item_cast(
            "item.scroll",
            "placeholder",
            "invalid-placeholder",
            SkillVisualBinding::player(SkillAvBinding::new(
                SkillAnimationBinding::new("placeholder-animation", None, false),
                SkillVfxBinding::vanilla(SkillAvPhaseBinding::new(
                    Some("placeholder-vfx"),
                    None,
                    Some("placeholder-vfx"),
                    Some("placeholder-vfx"),
                )),
                SkillAvPhaseBinding::new(
                    Some("placeholder-sfx"),
                    None,
                    Some("placeholder-sfx"),
                    Some("placeholder-sfx"),
                ),
                SkillAvPhaseBinding::new(
                    Some("placeholder-hud"),
                    None,
                    Some("placeholder-hud"),
                    Some("placeholder-hud"),
                ),
                SkillIconBinding::placeholder("placeholder-icon", "missing blocker marker"),
            )),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(invalid_placeholder),
            Err(RegistrationError::InvalidField {
                field: "icon.blocker",
                ..
            })
        ));

        let item_with_npc_arm = SkillRegistration::item_cast(
            "item.scroll",
            "npc",
            "item-npc-arm",
            SkillVisualBinding::npc(NpcVisualBinding::new(
                NpcAnimationBinding::NotApplicable,
                SkillVfxBinding::vanilla(SkillAvPhaseBinding::new(
                    Some("item-npc-vfx"),
                    None,
                    Some("item-npc-vfx"),
                    Some("item-npc-vfx"),
                )),
                SkillAvPhaseBinding::new(
                    Some("item-npc-sfx"),
                    None,
                    Some("item-npc-sfx"),
                    Some("item-npc-sfx"),
                ),
            )),
        );
        assert!(matches!(
            SkillRegistrationLedger::new().register(item_with_npc_arm),
            Err(RegistrationError::AudienceVisualMismatch {
                audience: SkillAudience::Player,
                ..
            })
        ));
    }

    #[test]
    fn audience_visual_arms_and_npc_exemptions_are_explicit() {
        let npc = NpcVisualBinding::new(
            NpcAnimationBinding::NotApplicable,
            SkillVfxBinding::vanilla(SkillAvPhaseBinding::new(
                Some("npc-vfx"),
                None,
                Some("npc-vfx"),
                Some("npc-vfx"),
            )),
            SkillAvPhaseBinding::new(Some("npc-sfx"), None, Some("npc-sfx"), Some("npc-sfx")),
        );
        let registration = SkillRegistration::skill(
            "contract.npc",
            definition("contract.npc"),
            Some(resolver),
            SkillCastMode::Resolver,
            SkillAudience::Npc,
            "npc-key",
            SkillVisualBinding::npc(npc),
        );
        let mut ledger = SkillRegistrationLedger::new();
        ledger.register(registration).unwrap();
        let entry = ledger.lookup_skill("contract.npc").unwrap();
        let SkillVisualBinding::Npc(npc) = entry.av() else {
            panic!("NPC audience must select typed NPC visual arm");
        };
        assert_eq!(npc.hud, NpcHudBinding::NotApplicable);
        assert_eq!(npc.icon, NpcIconBinding::NotApplicable);
        assert_eq!(npc.animation, NpcAnimationBinding::NotApplicable);
    }

    #[test]
    fn inert_states_are_one_ledger_and_av_request_uses_canonical_key() {
        let mut ledger = SkillRegistrationLedger::new();
        ledger
            .register_inert(
                skill("contract.declared", "declared"),
                InertEntryState::Declared,
            )
            .unwrap();
        ledger
            .register_unwired(skill("contract.unwired", "unwired"))
            .unwrap();
        ledger.register_test_only(item("test-only")).unwrap();

        assert_eq!(ledger.count_state(InertEntryState::Declared), 1);
        assert_eq!(ledger.count_state(InertEntryState::Unwired), 1);
        assert_eq!(ledger.count_state(InertEntryState::TestOnly), 1);
        let key = RegistrationKey::skill("contract.unwired");
        let request = ledger.av_request(&key, SkillAvPhase::Interrupt).unwrap();
        assert_eq!(request.key, &key);
        assert_eq!(request.phase, SkillAvPhase::Interrupt);
        assert_eq!(request.binding, ledger.lookup(&key).unwrap().av());
    }
}
