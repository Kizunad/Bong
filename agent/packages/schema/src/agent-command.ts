import { Type, type Static } from "@sinclair/typebox";

import { CommandType } from "./common.js";
import { validate, type ValidationResult } from "./validate.js";

// ─── 指令子结构 ─────────────────────────────────────────

export const Command = Type.Object(
  {
    type: CommandType,
    target: Type.String({ description: "区域名 or NPC ID" }),
    params: Type.Record(Type.String(), Type.Any(), {
      description:
        "指令参数。spawn_event: {event, intensity, duration_ticks, target_player?}; " +
        "spawn_npc: {archetype}; " +
        "despawn_npc: {}; " +
        "faction_event: {kind, faction_id, subject_id?, mission_id?, loyalty_delta?}; " +
        "modify_zone: {spirit_qi_delta, danger_level_delta?}; " +
        "npc_behavior: {aggression?, flee_threshold?, patrol_radius?}",
    }),
  },
  { additionalProperties: false },
);
export type Command = Static<typeof Command>;

// ─── 顶层消息 ──────────────────────────────────────────

export const AgentCommandV1 = Type.Object(
  {
    v: Type.Literal(1),
    id: Type.String({ description: "Unique command batch ID, e.g. cmd_1712345678_001" }),
    source: Type.Optional(
      Type.Union([
        Type.Literal("arbiter"),
        Type.Literal("calamity"),
        Type.Literal("mutation"),
        Type.Literal("era"),
      ]),
    ),
    commands: Type.Array(Command, { maxItems: 5 }),
  },
  { additionalProperties: false },
);
export type AgentCommandV1 = Static<typeof AgentCommandV1>;

export function validateAgentCommandV1Contract(data: unknown): ValidationResult {
  const result = validate(AgentCommandV1, data);
  if (!result.ok) {
    return result;
  }

  const candidate = data as { commands?: Array<{ type?: unknown; params?: Record<string, unknown> }> };
  const commands = candidate.commands ?? [];

  const semanticErrors: string[] = [];
  for (const [index, command] of commands.entries()) {
    if (command?.type !== "spawn_npc") {
      if (command?.type !== "faction_event") {
        continue;
      }

      const kind = command?.params?.kind;
      const factionId = command?.params?.faction_id;
      if (typeof kind !== "string") {
        semanticErrors.push(`/commands/${index}/params/kind: Expected string`);
      }
      if (typeof factionId !== "string") {
        semanticErrors.push(`/commands/${index}/params/faction_id: Expected string`);
      }
      continue;
    }

    const archetype = command?.params?.archetype;
    if (typeof archetype !== "string") {
      semanticErrors.push(`/commands/${index}/params/archetype: Expected string`);
    }
  }

  if (semanticErrors.length > 0) {
    return { ok: false, errors: semanticErrors };
  }

  return result;
}
