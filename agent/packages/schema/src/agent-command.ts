import { Type, type Static } from "@sinclair/typebox";

import { CommandType } from "./common.js";
import { validate, type ValidationResult } from "./validate.js";

const ALLOWED_NPC_ARCHETYPES = [
  "zombie",
  "commoner",
  "rogue",
  "beast",
  "disciple",
  "guardian_relic",
  "daoxiang",
  "zhinian",
  "fuya",
  "skull_fiend",
] as const;
const ALLOWED_FACTION_IDS = ["attack", "defend", "neutral"] as const;
const ALLOWED_FACTION_EVENT_KINDS = [
  "set_leader",
  "clear_leader",
  "set_leader_lineage",
  "adjust_loyalty_bias",
  "enqueue_mission",
  "pop_mission",
] as const;

// ─── 指令子结构 ─────────────────────────────────────────

export const Command = Type.Object(
  {
    type: CommandType,
    target: Type.String({ description: "区域名 or NPC ID" }),
    params: Type.Record(Type.String(), Type.Any(), {
      description:
        "指令参数。spawn_event: {event, intensity, duration_ticks, target_player?}; " +
        "spawn_npc: {archetype, initial_age_ticks?, count?, reason?}; " +
        "despawn_npc: {}; " +
        "faction_event: {kind, faction_id, subject_id?, mission_id?, loyalty_delta?}; " +
        "modify_zone: {spirit_qi_delta, danger_level_delta?}; " +
        "npc_behavior: {flee_threshold}; " +
        "heartbeat_override: {action, event_type, target_zone?, duration_ticks?, intensity_override?}",
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
        if (command?.type === "npc_behavior") {
          const fleeThreshold = command?.params?.flee_threshold;
          if (typeof fleeThreshold !== "number") {
            semanticErrors.push(`/commands/${index}/params/flee_threshold: Expected number`);
          }
        }
        if (command?.type === "heartbeat_override") {
          const action = command?.params?.action;
          const eventType = command?.params?.event_type;
          const targetZone = command?.params?.target_zone;
          const durationTicks = command?.params?.duration_ticks;
          const intensityOverride = command?.params?.intensity_override;
          if (!isAllowedLiteral(["suppress", "accelerate", "force"], action)) {
            semanticErrors.push(`/commands/${index}/params/action: Expected suppress|accelerate|force`);
          }
          if (!isAllowedLiteral(["pseudo_vein", "beast_tide", "realm_collapse", "karma_backlash"], eventType)) {
            semanticErrors.push(`/commands/${index}/params/event_type: Expected supported heartbeat event type`);
          }
          if (
            targetZone !== undefined
            && (typeof targetZone !== "string" || targetZone.trim().length === 0)
          ) {
            semanticErrors.push(`/commands/${index}/params/target_zone: Expected non-empty string`);
          }
          if (
            durationTicks !== undefined
            && (typeof durationTicks !== "number" || !Number.isInteger(durationTicks) || durationTicks < 1)
          ) {
            semanticErrors.push(`/commands/${index}/params/duration_ticks: Expected positive integer`);
          }
          if (
            intensityOverride !== undefined
            && (typeof intensityOverride !== "number" || intensityOverride < 0 || intensityOverride > 1)
          ) {
            semanticErrors.push(`/commands/${index}/params/intensity_override: Expected number in [0, 1]`);
          }
        }
        continue;
      }

      const kind = command?.params?.kind;
      const factionId = command?.params?.faction_id;
      if (!isAllowedLiteral(ALLOWED_FACTION_EVENT_KINDS, kind)) {
        semanticErrors.push(`/commands/${index}/params/kind: Expected supported faction event kind`);
      }
      if (!isAllowedLiteral(ALLOWED_FACTION_IDS, factionId)) {
        semanticErrors.push(`/commands/${index}/params/faction_id: Expected supported faction id`);
      }
      continue;
    }

    const archetype = command?.params?.archetype;
    if (!isAllowedLiteral(ALLOWED_NPC_ARCHETYPES, archetype)) {
      semanticErrors.push(`/commands/${index}/params/archetype: Expected supported NPC archetype`);
    }

    const count = command?.params?.count;
    if (count !== undefined && (typeof count !== "number" || !Number.isInteger(count) || count < 1 || count > 5)) {
      semanticErrors.push(`/commands/${index}/params/count: Expected integer in [1, 5]`);
    }
  }

  if (semanticErrors.length > 0) {
    return { ok: false, errors: semanticErrors };
  }

  return result;
}

function isAllowedLiteral(values: readonly string[], candidate: unknown): boolean {
  return typeof candidate === "string" && values.includes(candidate);
}
