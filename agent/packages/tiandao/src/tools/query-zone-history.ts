import { WorldModel } from "../world-model.js";
import type { AgentTool } from "./types.js";
import { toolSchema } from "./types.js";

const MAX_HISTORY_LIMIT = 10;

interface QueryZoneHistoryArgs {
  zone: string;
  limit?: number;
}

function normalizeLimit(limit: number | undefined): number {
  if (typeof limit !== "number" || !Number.isFinite(limit)) {
    return 5;
  }

  const normalized = Math.floor(limit);
  if (normalized <= 0) {
    return 1;
  }

  return Math.min(normalized, MAX_HISTORY_LIMIT);
}

function formatSigned(value: number): string {
  const normalized = Math.abs(value) < 0.005 ? 0 : value;
  return `${normalized >= 0 ? "+" : ""}${normalized.toFixed(2)}`;
}

export const queryZoneHistoryTool: AgentTool<QueryZoneHistoryArgs, unknown> = {
  name: "query-zone-history",
  description:
    "Read bounded spirit_qi and danger history for one zone with trend metadata and concise summary",
  readonly: true,
  parameters: toolSchema.object(
    {
      zone: toolSchema.string(),
      limit: toolSchema.number(),
    },
    {
      required: ["zone"],
      additionalProperties: false,
    },
  ),
  result: toolSchema.object(
    {
      ok: toolSchema.boolean(),
      zone: toolSchema.string(),
      limit: toolSchema.number(),
      history: toolSchema.array(
        toolSchema.object(
          {
            index: toolSchema.number(),
            spiritQi: toolSchema.number(),
            dangerLevel: toolSchema.number(),
            activeEventCount: toolSchema.number(),
            playerCount: toolSchema.number(),
          },
          { additionalProperties: false },
        ),
      ),
      trend: toolSchema.object(
        {
          direction: toolSchema.string({ enum: ["rising", "stable", "falling"] }),
          delta: toolSchema.number(),
          previousSpiritQi: toolSchema.number(),
          currentSpiritQi: toolSchema.number(),
        },
        { additionalProperties: false },
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
    const zone = args.zone.trim();
    if (zone.length === 0) {
      return {
        ok: false,
        error: {
          code: "INVALID_QUERY",
          message: "'zone' must be a non-empty string",
        },
      };
    }

    const model = WorldModel.fromJSON(ctx.worldModel);
    const fullHistory = model.getZoneHistory(zone);
    if (fullHistory.length === 0) {
      return {
        ok: false,
        zone,
        limit: normalizeLimit(args.limit),
        error: {
          code: "ZONE_NOT_FOUND",
          message: `zone '${zone}' has no history`,
        },
      };
    }

    const limit = normalizeLimit(args.limit);
    const boundedHistory = fullHistory.slice(-limit);
    const history = boundedHistory.map((entry, index) => ({
      index: index + 1,
      spiritQi: entry.spirit_qi,
      dangerLevel: entry.danger_level,
      activeEventCount: entry.active_events.length,
      playerCount: entry.player_count,
    }));

    const trend = model.getZoneTrendSummary(zone);
    const first = boundedHistory[0];
    const last = boundedHistory[boundedHistory.length - 1];
    const localDelta = first && last ? last.spirit_qi - first.spirit_qi : 0;
    const trendDirection = trend?.trend ?? "stable";
    const trendDelta = trend?.delta ?? localDelta;
    const summary = `${zone}: ${history.length} snapshots, spirit_qi ${first?.spirit_qi.toFixed(2) ?? "0.00"}→${last?.spirit_qi.toFixed(2) ?? "0.00"} (${formatSigned(localDelta)}), trend ${trendDirection} (${formatSigned(trendDelta)})`;

    return {
      ok: true,
      zone,
      limit,
      history,
      trend: {
        direction: trendDirection,
        delta: trendDelta,
        previousSpiritQi: trend?.previousSpiritQi ?? (first?.spirit_qi ?? 0),
        currentSpiritQi: trend?.currentSpiritQi ?? (last?.spirit_qi ?? 0),
      },
      summary,
    };
  },
};
