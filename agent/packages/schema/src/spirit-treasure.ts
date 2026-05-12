import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

export const SpiritTreasureDialogueTriggerV1 = Type.Union([
  Type.Literal("player"),
  Type.Literal("random"),
  Type.Literal("event"),
]);
export type SpiritTreasureDialogueTriggerV1 = Static<
  typeof SpiritTreasureDialogueTriggerV1
>;

export const SpiritTreasureDialogueToneV1 = Type.Union([
  Type.Literal("cold"),
  Type.Literal("curious"),
  Type.Literal("warning"),
  Type.Literal("amused"),
  Type.Literal("silent"),
]);
export type SpiritTreasureDialogueToneV1 = Static<typeof SpiritTreasureDialogueToneV1>;

export const SpiritTreasureDialogueHistoryEntryV1 = Type.Object(
  {
    speaker: Type.String({ minLength: 1, maxLength: 32 }),
    content: Type.String({ maxLength: 512 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasureDialogueHistoryEntryV1 = Static<
  typeof SpiritTreasureDialogueHistoryEntryV1
>;

export const SpiritTreasureDialogueContextV1 = Type.Object(
  {
    realm: Type.String({ minLength: 1, maxLength: 64 }),
    qi_percent: Type.Number({ minimum: 0, maximum: 1 }),
    zone: Type.String({ minLength: 1, maxLength: 128 }),
    recent_events: Type.Array(Type.String({ maxLength: 256 }), { maxItems: 16 }),
    affinity: Type.Number({ minimum: 0, maximum: 1 }),
    dialogue_history: Type.Array(SpiritTreasureDialogueHistoryEntryV1, {
      maxItems: 16,
    }),
    equipped: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type SpiritTreasureDialogueContextV1 = Static<
  typeof SpiritTreasureDialogueContextV1
>;

export const SpiritTreasureDialogueRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    request_id: Type.String({ minLength: 1, maxLength: 160 }),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    treasure_id: Type.String({ minLength: 1, maxLength: 128 }),
    trigger: SpiritTreasureDialogueTriggerV1,
    player_message: Type.Optional(Type.String({ maxLength: 512 })),
    context: SpiritTreasureDialogueContextV1,
  },
  { additionalProperties: false },
);
export type SpiritTreasureDialogueRequestV1 = Static<
  typeof SpiritTreasureDialogueRequestV1
>;

export const SpiritTreasureDialogueV1 = Type.Object(
  {
    v: Type.Literal(1),
    request_id: Type.String({ minLength: 1, maxLength: 160 }),
    character_id: Type.String({ minLength: 1, maxLength: 160 }),
    treasure_id: Type.String({ minLength: 1, maxLength: 128 }),
    text: Type.String({ minLength: 1, maxLength: 512 }),
    tone: SpiritTreasureDialogueToneV1,
    affinity_delta: Type.Number({ minimum: -1, maximum: 1 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasureDialogueV1 = Static<typeof SpiritTreasureDialogueV1>;

export const SpiritTreasurePassiveV1 = Type.Object(
  {
    kind: Type.String({ minLength: 1, maxLength: 96 }),
    value: Type.Number(),
    description: Type.String({ minLength: 1, maxLength: 160 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasurePassiveV1 = Static<typeof SpiritTreasurePassiveV1>;

export const SpiritTreasureClientStateV1 = Type.Object(
  {
    template_id: Type.String({ minLength: 1, maxLength: 128 }),
    display_name: Type.String({ minLength: 1, maxLength: 64 }),
    instance_id: Type.Integer({ minimum: 0 }),
    equipped: Type.Boolean(),
    passive_active: Type.Boolean(),
    affinity: Type.Number({ minimum: 0, maximum: 1 }),
    sleeping: Type.Boolean(),
    source_sect: Type.Union([Type.String({ minLength: 1, maxLength: 64 }), Type.Null()]),
    icon_texture: Type.String({ minLength: 1, maxLength: 160 }),
    passive_effects: Type.Array(SpiritTreasurePassiveV1, { maxItems: 8 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasureClientStateV1 = Static<typeof SpiritTreasureClientStateV1>;

export const SpiritTreasureStatePayloadV1 = Type.Object(
  {
    treasures: Type.Array(SpiritTreasureClientStateV1, { maxItems: 8 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasureStatePayloadV1 = Static<typeof SpiritTreasureStatePayloadV1>;

export const SpiritTreasureDialoguePayloadV1 = Type.Object(
  {
    dialogue: SpiritTreasureDialogueV1,
    display_name: Type.String({ minLength: 1, maxLength: 64 }),
    zone: Type.String({ minLength: 1, maxLength: 128 }),
  },
  { additionalProperties: false },
);
export type SpiritTreasureDialoguePayloadV1 = Static<
  typeof SpiritTreasureDialoguePayloadV1
>;

export function validateSpiritTreasureDialogueRequestV1Contract(
  data: unknown,
): ValidationResult {
  return validate(SpiritTreasureDialogueRequestV1, data);
}

export function validateSpiritTreasureDialogueV1Contract(data: unknown): ValidationResult {
  return validate(SpiritTreasureDialogueV1, data);
}

export function validateSpiritTreasureStatePayloadV1Contract(
  data: unknown,
): ValidationResult {
  return validate(SpiritTreasureStatePayloadV1, data);
}

export function validateSpiritTreasureDialoguePayloadV1Contract(
  data: unknown,
): ValidationResult {
  return validate(SpiritTreasureDialoguePayloadV1, data);
}
