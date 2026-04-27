import { type Static, Type } from "@sinclair/typebox";

import { EquipSlotV1 } from "./inventory.js";
import { type ValidationResult, validate } from "./validate.js";

export const ArmorDurabilityChangedV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: Type.String({ minLength: 1 }),
    slot: EquipSlotV1,
    instance_id: Type.Integer({ minimum: 0 }),
    template_id: Type.String({ minLength: 1 }),
    cur: Type.Number({ minimum: 0 }),
    max: Type.Number({ minimum: 0 }),
    durability_ratio: Type.Number({ minimum: 0, maximum: 1 }),
    broken: Type.Boolean(),
  },
  { additionalProperties: false },
);
export type ArmorDurabilityChangedV1 = Static<typeof ArmorDurabilityChangedV1>;

export function validateArmorDurabilityChangedV1Contract(
  data: unknown,
): ValidationResult {
  return validate(ArmorDurabilityChangedV1, data);
}
