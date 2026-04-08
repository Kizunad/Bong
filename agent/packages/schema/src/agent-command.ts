import { Type, type Static } from "@sinclair/typebox";
import { CommandType } from "./common.js";

// ─── 指令子结构 ─────────────────────────────────────────

export const Command = Type.Object(
  {
    type: CommandType,
    target: Type.String({ description: "区域名 or NPC ID" }),
    params: Type.Record(Type.String(), Type.Any(), {
      description:
        "指令参数。spawn_event: {event, intensity, duration_ticks, target_player?}; " +
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
