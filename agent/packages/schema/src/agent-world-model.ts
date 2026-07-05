import { Type, type Static } from "@sinclair/typebox";

import { AgentCommandV1, Command } from "./agent-command.js";
import { Narration } from "./narration.js";
import { validate, type ValidationResult } from "./validate.js";
import { WorldStateV1 } from "./world-state.js";

export const AgentWorldModelSnapshotV1 = Type.Object(
  {
    current_era: Type.Union([
      Type.Object(
        {
          name: Type.String(),
          since_tick: Type.Integer(),
          global_effect: Type.String(),
        },
        { additionalProperties: false },
      ),
      Type.Null(),
    ]),
    zone_history: Type.Record(Type.String(), Type.Array(WorldStateV1.properties.zones.items)),
    last_decisions: Type.Record(
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
    player_first_seen_tick: Type.Record(Type.String(), Type.Integer()),
    neg_domain_pending_tribulations: Type.Record(
      Type.String(),
      Type.Object(
        {
          player_uuid: Type.String(),
          player_name: Type.String(),
          zone: Type.String(),
          entered_at_tick: Type.Integer(),
          last_suppressed_tick: Type.Integer(),
          reason: Type.Literal("negative_domain_tribulation_exempt"),
        },
        { additionalProperties: false },
      ),
    ),
    neg_domain_escape_telemetry: Type.Object(
      {
        escape_entry_count: Type.Integer({ minimum: 0 }),
        post_escape_realm_drop_count: Type.Integer({ minimum: 0 }),
        successful_tribulation_avoidance_count: Type.Integer({ minimum: 0 }),
        active_escape_session_count: Type.Integer({ minimum: 0 }),
        post_escape_realm_drop_rate: Type.Number({ minimum: 0 }),
      },
      { additionalProperties: false },
    ),
    neg_domain_escape_sessions: Type.Record(
      Type.String(),
      Type.Object(
        {
          player_uuid: Type.String(),
          player_name: Type.String(),
          zone: Type.String(),
          entered_at_tick: Type.Integer(),
          entry_realm_rank: Type.Number(),
        },
        { additionalProperties: false },
      ),
    ),
    last_tick: Type.Union([Type.Integer(), Type.Null()]),
    last_state_ts: Type.Union([Type.Integer(), Type.Null()]),
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
