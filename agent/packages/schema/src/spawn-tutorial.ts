import { Type, type Static } from "@sinclair/typebox";

import { BlockPosV1 } from "./alchemy.js";
import { validate, type ValidationResult } from "./validate.js";

const tickField = Type.Integer({ minimum: 0 });

export const TutorialHookV1 = Type.Union([
  Type.Literal("spawn_entered"),
  Type.Literal("coffin_opened"),
  Type.Literal("moved200_blocks"),
  Type.Literal("first_sit_meditate"),
  Type.Literal("first_meridian_opened"),
  Type.Literal("rat_swarm_encounter"),
  Type.Literal("lingquan_reached"),
  Type.Literal("breakthrough_window"),
  Type.Literal("realm_advanced_to_induce"),
]);
export type TutorialHookV1 = Static<typeof TutorialHookV1>;

export const TutorialHookEventV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("tutorial_hook_event"),
    player_id: Type.String({ minLength: 1, maxLength: 128 }),
    hook: TutorialHookV1,
    tick: tickField,
  },
  { additionalProperties: false },
);
export type TutorialHookEventV1 = Static<typeof TutorialHookEventV1>;

export const CoffinOpenedV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("coffin_opened"),
    player_id: Type.String({ minLength: 1, maxLength: 128 }),
    coffin_pos: BlockPosV1,
    granted_item_id: Type.Literal("spirit_niche_stone"),
    tick: tickField,
  },
  { additionalProperties: false },
);
export type CoffinOpenedV1 = Static<typeof CoffinOpenedV1>;

export function validateTutorialHookEventV1Contract(data: unknown): ValidationResult {
  return validate(TutorialHookEventV1, data);
}

export function validateCoffinOpenedV1Contract(data: unknown): ValidationResult {
  return validate(CoffinOpenedV1, data);
}
