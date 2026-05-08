import { Type, type Static } from "@sinclair/typebox";

export const SkillConfigV1 = Type.Record(
  Type.String({ minLength: 1 }),
  Type.Unknown(),
);
export type SkillConfigV1 = Static<typeof SkillConfigV1>;

export const SkillConfigSnapshotV1 = Type.Object(
  {
    configs: Type.Record(Type.String({ minLength: 1 }), SkillConfigV1),
  },
  { additionalProperties: false },
);
export type SkillConfigSnapshotV1 = Static<typeof SkillConfigSnapshotV1>;
