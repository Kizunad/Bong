import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const RiftPortalKindV1 = Type.Union([
  Type.Literal("main_rift"),
  Type.Literal("deep_rift"),
  Type.Literal("collapse_tear"),
]);
export type RiftPortalKindV1 = Static<typeof RiftPortalKindV1>;

export const RiftPortalDirectionV1 = Type.Union([
  Type.Literal("entry"),
  Type.Literal("exit"),
]);
export type RiftPortalDirectionV1 = Static<typeof RiftPortalDirectionV1>;

export const RiftPortalStateV1 = Type.Object(
  {
    entity_id: Type.Number(),
    kind: RiftPortalKindV1,
    direction: RiftPortalDirectionV1,
    family_id: Type.String({ minLength: 1 }),
    world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    trigger_radius: Type.Number({ minimum: 0 }),
    current_extract_ticks: Type.Number({ minimum: 0 }),
    activation_window_end: Type.Optional(Type.Number({ minimum: 0 })),
  },
  { additionalProperties: false },
);
export type RiftPortalStateV1 = Static<typeof RiftPortalStateV1>;

export const RiftPortalRemovedV1 = Type.Object(
  {
    entity_id: Type.Number(),
  },
  { additionalProperties: false },
);
export type RiftPortalRemovedV1 = Static<typeof RiftPortalRemovedV1>;

export const ExtractStartedV1 = Type.Object(
  {
    player_id: Type.String({ minLength: 1 }),
    portal_entity_id: Type.Number(),
    portal_kind: RiftPortalKindV1,
    required_ticks: Type.Number({ minimum: 0 }),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ExtractStartedV1 = Static<typeof ExtractStartedV1>;

export const ExtractProgressV1 = Type.Object(
  {
    player_id: Type.String({ minLength: 1 }),
    portal_entity_id: Type.Number(),
    elapsed_ticks: Type.Number({ minimum: 0 }),
    required_ticks: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ExtractProgressV1 = Static<typeof ExtractProgressV1>;

export const ExtractCompletedV1 = Type.Object(
  {
    player_id: Type.String({ minLength: 1 }),
    portal_kind: RiftPortalKindV1,
    family_id: Type.String({ minLength: 1 }),
    exit_world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    at_tick: Type.Number({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type ExtractCompletedV1 = Static<typeof ExtractCompletedV1>;

export const ExtractAbortedReasonV1 = Type.Union([
  Type.Literal("moved"),
  Type.Literal("combat"),
  Type.Literal("damaged"),
  Type.Literal("cancelled"),
  Type.Literal("portal_expired"),
  Type.Literal("out_of_range"),
  Type.Literal("not_in_tsy"),
  Type.Literal("already_busy"),
  Type.Literal("cannot_exit"),
]);
export type ExtractAbortedReasonV1 = Static<typeof ExtractAbortedReasonV1>;

export const ExtractAbortedV1 = Type.Object(
  {
    player_id: Type.String({ minLength: 1 }),
    reason: ExtractAbortedReasonV1,
  },
  { additionalProperties: false },
);
export type ExtractAbortedV1 = Static<typeof ExtractAbortedV1>;

export const ExtractFailedV1 = Type.Object(
  {
    player_id: Type.String({ minLength: 1 }),
    reason: Type.Literal("spirit_qi_drained"),
  },
  { additionalProperties: false },
);
export type ExtractFailedV1 = Static<typeof ExtractFailedV1>;

export const TsyCollapseStartedIpcV1 = Type.Object(
  {
    family_id: Type.String({ minLength: 1 }),
    at_tick: Type.Number({ minimum: 0 }),
    remaining_ticks: Type.Number({ minimum: 0 }),
    collapse_tear_entity_ids: Type.Array(Type.Number()),
  },
  { additionalProperties: false },
);
export type TsyCollapseStartedIpcV1 = Static<typeof TsyCollapseStartedIpcV1>;

export function validateExtractStartedV1Contract(data: unknown): ValidationResult {
  return validate(ExtractStartedV1, data);
}

export function validateExtractProgressV1Contract(data: unknown): ValidationResult {
  return validate(ExtractProgressV1, data);
}

export function validateExtractCompletedV1Contract(data: unknown): ValidationResult {
  return validate(ExtractCompletedV1, data);
}

export function validateExtractAbortedV1Contract(data: unknown): ValidationResult {
  return validate(ExtractAbortedV1, data);
}

export function validateExtractFailedV1Contract(data: unknown): ValidationResult {
  return validate(ExtractFailedV1, data);
}
