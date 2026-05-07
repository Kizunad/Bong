import type { AgentTool } from "./types.js";
import { toolSchema } from "./types.js";

interface QueryRatDensityArgs {
  zone: string;
}

function dominantPhase(snapshot: {
  solitary: number;
  transitioning: number;
  gregarious: number;
}): "solitary" | "transitioning" | "gregarious" {
  const entries: Array<["solitary" | "transitioning" | "gregarious", number]> = [
    ["solitary", snapshot.solitary],
    ["transitioning", snapshot.transitioning],
    ["gregarious", snapshot.gregarious],
  ];
  return entries.sort((left, right) => right[1] - left[1])[0]?.[0] ?? "solitary";
}

export const queryRatDensityTool: AgentTool<QueryRatDensityArgs, unknown> = {
  name: "query-rat-density",
  description: "Read current rat density and phase counts for one zone",
  readonly: true,
  parameters: toolSchema.object(
    {
      zone: toolSchema.string(),
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
      total: toolSchema.number(),
      phases: toolSchema.object(
        {
          solitary: toolSchema.number(),
          transitioning: toolSchema.number(),
          gregarious: toolSchema.number(),
        },
        { additionalProperties: false },
      ),
      dominantPhase: toolSchema.string({
        enum: ["solitary", "transitioning", "gregarious"],
      }),
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

    const snapshot = ctx.latestState.rat_density_heatmap.zones[zone];
    if (!snapshot) {
      return {
        ok: false,
        zone,
        error: {
          code: "ZONE_NOT_FOUND",
          message: `rat density for zone '${zone}' not found`,
        },
      };
    }

    const phases = {
      solitary: snapshot.solitary,
      transitioning: snapshot.transitioning,
      gregarious: snapshot.gregarious,
    };
    const phase = dominantPhase(phases);

    return {
      ok: true,
      zone,
      total: snapshot.total,
      phases,
      dominantPhase: phase,
      summary: `${zone}: rats=${snapshot.total}, solitary=${snapshot.solitary}, transitioning=${snapshot.transitioning}, gregarious=${snapshot.gregarious}, dominant=${phase}`,
    };
  },
};
