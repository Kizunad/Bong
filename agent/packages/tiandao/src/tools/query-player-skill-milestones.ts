import type { AgentTool } from "./types.js";
import { toolSchema } from "./types.js";

const QUERY_KEYS = ["uuid", "name"] as const;
const DEFAULT_LIMIT = 3;
const MAX_LIMIT = 10;

interface QueryPlayerSkillMilestonesArgs {
  uuid?: string;
  name?: string;
  limit?: number;
}

type QuerySelector =
  | { ok: true; by: "uuid" | "name"; value: string }
  | { ok: false; code: "INVALID_QUERY"; message: string };

function parseSelector(args: QueryPlayerSkillMilestonesArgs): QuerySelector {
  const uuid = typeof args.uuid === "string" ? args.uuid.trim() : "";
  const name = typeof args.name === "string" ? args.name.trim() : "";
  const hasUuid = uuid.length > 0;
  const hasName = name.length > 0;

  if (hasUuid && hasName) {
    return {
      ok: false,
      code: "INVALID_QUERY",
      message: "'uuid' and 'name' are mutually exclusive",
    };
  }

  if (!hasUuid && !hasName) {
    return {
      ok: false,
      code: "INVALID_QUERY",
      message: "provide exactly one of 'uuid' or 'name'",
    };
  }

  return hasUuid
    ? { ok: true, by: "uuid", value: uuid }
    : { ok: true, by: "name", value: name };
}

function normalizeLimit(limit: unknown): number {
  if (typeof limit !== "number" || !Number.isFinite(limit)) {
    return DEFAULT_LIMIT;
  }
  return Math.max(1, Math.min(MAX_LIMIT, Math.trunc(limit)));
}

export const queryPlayerSkillMilestonesTool: AgentTool<QueryPlayerSkillMilestonesArgs, unknown> = {
  name: "query-player-skill-milestones",
  description:
    "Lookup one player by uuid or name and return recent structured skill milestones with narration text",
  readonly: true,
  parameters: toolSchema.object(
    {
      uuid: toolSchema.string(),
      name: toolSchema.string(),
      limit: toolSchema.number(),
    },
    {
      required: [],
      additionalProperties: false,
    },
  ),
  result: toolSchema.object(
    {
      ok: toolSchema.boolean(),
      query: toolSchema.object(
        {
          by: toolSchema.string({ enum: QUERY_KEYS }),
          value: toolSchema.string(),
        },
        { additionalProperties: false },
      ),
      limit: toolSchema.number(),
      player: toolSchema.object(
        {
          uuid: toolSchema.string(),
          name: toolSchema.string(),
          zone: toolSchema.string(),
        },
        { additionalProperties: false },
      ),
      milestones: toolSchema.array(
        toolSchema.object(
          {
            skill: toolSchema.string(),
            newLv: toolSchema.number(),
            achievedAt: toolSchema.number(),
            narration: toolSchema.string(),
            totalXpAt: toolSchema.number(),
          },
          { additionalProperties: false },
        ),
      ),
      summary: toolSchema.string(),
      error: toolSchema.object(
        {
          code: toolSchema.string(),
          message: toolSchema.string(),
        },
        { additionalProperties: false },
      ),
    },
    {
      required: ["ok"],
      additionalProperties: false,
    },
  ),
  async execute(args, ctx) {
    const selector = parseSelector(args);
    if (!selector.ok) {
      return {
        ok: false,
        error: {
          code: selector.code,
          message: selector.message,
        },
      };
    }

    const limit = normalizeLimit(args.limit);
    const player = ctx.latestState.players.find((candidate) =>
      selector.by === "uuid" ? candidate.uuid === selector.value : candidate.name === selector.value,
    );

    if (!player) {
      return {
        ok: false,
        query: {
          by: selector.by,
          value: selector.value,
        },
        limit,
        error: {
          code: "PLAYER_NOT_FOUND",
          message: `player not found by ${selector.by}: ${selector.value}`,
        },
      };
    }

    const milestones = (player.life_record?.skill_milestones ?? [])
      .slice(-limit)
      .map((milestone) => ({
        skill: milestone.skill,
        newLv: milestone.new_lv,
        achievedAt: milestone.achieved_at,
        narration: milestone.narration,
        totalXpAt: milestone.total_xp_at,
      }));

    return {
      ok: true,
      query: {
        by: selector.by,
        value: selector.value,
      },
      limit,
      player: {
        uuid: player.uuid,
        name: player.name,
        zone: player.zone,
      },
      milestones,
      summary: `${player.name}@${player.zone} recent skill milestones ${milestones.length}`,
    };
  },
};
