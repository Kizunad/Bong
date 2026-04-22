import { Type, type Static } from "@sinclair/typebox";

import { AgentCommandV1, Command } from "./agent-command.js";
import { Narration } from "./narration.js";
import { validate, type ValidationResult } from "./validate.js";
import { WorldStateV1 } from "./world-state.js";

export const AgentWorldModelSnapshotV1 = Type.Object(
  {
    currentEra: Type.Union([
      Type.Object(
        {
          name: Type.String(),
          sinceTick: Type.Integer(),
          globalEffect: Type.String(),
        },
        { additionalProperties: false },
      ),
      Type.Null(),
    ]),
    zoneHistory: Type.Record(Type.String(), Type.Array(WorldStateV1.properties.zones.items)),
    lastDecisions: Type.Record(
      Type.String(),
      Type.Object(
        {
          commands: Type.Array(Command),
          narrations: Type.Array(Narration),
          reasoning: Type.String(),
        },
        { additionalProperties: false },
      ),
    ),
    playerFirstSeenTick: Type.Record(Type.String(), Type.Integer()),
    lastTick: Type.Union([Type.Integer(), Type.Null()]),
    lastStateTs: Type.Union([Type.Integer(), Type.Null()]),
  },
  { additionalProperties: false },
);
export type AgentWorldModelSnapshotV1 = Static<typeof AgentWorldModelSnapshotV1>;

export const AgentWorldModelEnvelopeV1 = Type.Object(
  {
    v: Type.Literal(1),
    id: Type.String({ description: "Unique world-model publish id" }),
    source: Type.Optional(AgentCommandV1.properties.source),
    snapshot: AgentWorldModelSnapshotV1,
  },
  { additionalProperties: false },
);
export type AgentWorldModelEnvelopeV1 = Static<typeof AgentWorldModelEnvelopeV1>;

export function validateAgentWorldModelEnvelopeV1Contract(data: unknown): ValidationResult {
  return validate(AgentWorldModelEnvelopeV1, data);
}
