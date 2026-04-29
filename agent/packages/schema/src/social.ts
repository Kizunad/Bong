import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const ExposureKindV1 = Type.Union([
  Type.Literal("chat"),
  Type.Literal("trade"),
  Type.Literal("divine"),
  Type.Literal("death"),
]);
export type ExposureKindV1 = Static<typeof ExposureKindV1>;

export const RelationshipKindV1 = Type.Union([
  Type.Literal("master"),
  Type.Literal("disciple"),
  Type.Literal("companion"),
  Type.Literal("pact"),
  Type.Literal("feud"),
]);
export type RelationshipKindV1 = Static<typeof RelationshipKindV1>;

export const RenownTagV1 = Type.Object(
  {
    tag: Type.String(),
    weight: Type.Number(),
    last_seen_tick: Type.Integer({ minimum: 0 }),
    permanent: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type RenownTagV1 = Static<typeof RenownTagV1>;

export const RelationshipSnapshotV1 = Type.Object(
  {
    kind: RelationshipKindV1,
    peer: Type.String(),
    since_tick: Type.Integer({ minimum: 0 }),
    metadata: Type.Any(),
  },
  { additionalProperties: false },
);
export type RelationshipSnapshotV1 = Static<typeof RelationshipSnapshotV1>;

export const RenownSnapshotV1 = Type.Object(
  {
    fame: Type.Integer(),
    notoriety: Type.Integer(),
    top_tags: Type.Array(RenownTagV1),
  },
  { additionalProperties: false },
);
export type RenownSnapshotV1 = Static<typeof RenownSnapshotV1>;

export const FactionMembershipSnapshotV1 = Type.Object(
  {
    faction: Type.String(),
    rank: Type.Integer({ minimum: 0 }),
    loyalty: Type.Integer(),
    betrayal_count: Type.Integer({ minimum: 0 }),
    invite_block_until_tick: Type.Optional(Type.Integer({ minimum: 0 })),
    permanently_refused: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type FactionMembershipSnapshotV1 = Static<typeof FactionMembershipSnapshotV1>;

export const PlayerSocialSnapshotV1 = Type.Object(
  {
    renown: RenownSnapshotV1,
    relationships: Type.Array(RelationshipSnapshotV1),
    exposed_to_count: Type.Integer({ minimum: 0 }),
    faction_membership: Type.Optional(FactionMembershipSnapshotV1),
  },
  { additionalProperties: false },
);
export type PlayerSocialSnapshotV1 = Static<typeof PlayerSocialSnapshotV1>;

export const SocialRemoteIdentityV1 = Type.Object(
  {
    player_uuid: Type.String(),
    anonymous: Type.Boolean(),
    display_name: Type.Optional(Type.String()),
    realm_band: Type.Optional(Type.String()),
    breath_hint: Type.Optional(Type.String()),
    renown_tags: Type.Array(Type.String()),
  },
  { additionalProperties: false },
);
export type SocialRemoteIdentityV1 = Static<typeof SocialRemoteIdentityV1>;

export const SocialAnonymityPayloadV1 = Type.Object(
  {
    viewer: Type.String(),
    remotes: Type.Array(SocialRemoteIdentityV1),
  },
  { additionalProperties: false },
);
export type SocialAnonymityPayloadV1 = Static<typeof SocialAnonymityPayloadV1>;

export const SocialExposureEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    actor: Type.String(),
    kind: ExposureKindV1,
    witnesses: Type.Array(Type.String()),
    tick: Type.Integer({ minimum: 0 }),
    zone: Type.Optional(Type.String()),
  },
  { additionalProperties: false },
);
export type SocialExposureEventV1 = Static<typeof SocialExposureEventV1>;

export const SocialPactEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    left: Type.String(),
    right: Type.String(),
    terms: Type.String(),
    tick: Type.Integer({ minimum: 0 }),
    broken: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type SocialPactEventV1 = Static<typeof SocialPactEventV1>;

export const SocialFeudEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    left: Type.String(),
    right: Type.String(),
    tick: Type.Integer({ minimum: 0 }),
    place: Type.Optional(Type.String()),
  },
  { additionalProperties: false },
);
export type SocialFeudEventV1 = Static<typeof SocialFeudEventV1>;

export const SocialRenownDeltaV1 = Type.Object(
  {
    v: Type.Literal(1),
    char_id: Type.String(),
    fame_delta: Type.Integer(),
    notoriety_delta: Type.Integer(),
    tags_added: Type.Array(RenownTagV1),
    tick: Type.Integer({ minimum: 0 }),
    reason: Type.String(),
  },
  { additionalProperties: false },
);
export type SocialRenownDeltaV1 = Static<typeof SocialRenownDeltaV1>;

export const SparringInvitePayloadV1 = Type.Object(
  {
    invite_id: Type.String(),
    initiator: Type.String(),
    target: Type.String(),
    realm_band: Type.String(),
    breath_hint: Type.String(),
    terms: Type.String(),
    expires_at_ms: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SparringInvitePayloadV1 = Static<typeof SparringInvitePayloadV1>;

export function validateSocialExposureEventV1Contract(data: unknown): ValidationResult {
  return validate(SocialExposureEventV1, data);
}

export function validateSocialPactEventV1Contract(data: unknown): ValidationResult {
  return validate(SocialPactEventV1, data);
}

export function validateSocialFeudEventV1Contract(data: unknown): ValidationResult {
  return validate(SocialFeudEventV1, data);
}

export function validateSocialRenownDeltaV1Contract(data: unknown): ValidationResult {
  return validate(SocialRenownDeltaV1, data);
}
