import type { AgentTool } from "./types.js";
import { toolSchema } from "./types.js";

const MAX_ZONES = 10;
const MAX_EVENTS_PER_ZONE = 5;
const MAX_SUMMARY_EVENTS = 8;

interface ListActiveEventsArgs {
  zone?: string;
}

export const listActiveEventsTool: AgentTool<ListActiveEventsArgs, unknown> = {
  name: "list-active-events",
  description: "List per-zone active events with a compact deduplicated summary",
  readonly: true,
  parameters: toolSchema.object(
    {
      zone: toolSchema.string(),
    },
    {
      required: [],
      additionalProperties: false,
    },
  ),
  result: toolSchema.object(
    {
      ok: toolSchema.boolean(),
      zones: toolSchema.array(
        toolSchema.object(
          {
            name: toolSchema.string(),
            dangerLevel: toolSchema.number(),
            eventCount: toolSchema.number(),
            events: toolSchema.array(toolSchema.string()),
          },
          { additionalProperties: false },
        ),
      ),
      dedupedEvents: toolSchema.array(toolSchema.string()),
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
    const zoneFilter = typeof args.zone === "string" ? args.zone.trim() : "";
    if (typeof args.zone === "string" && zoneFilter.length === 0) {
      return {
        ok: false,
        error: {
          code: "INVALID_QUERY",
          message: "'zone' must be a non-empty string when provided",
        },
      };
    }

    const selectedZones = ctx.latestState.zones
      .filter((zone) => (zoneFilter.length > 0 ? zone.name === zoneFilter : true))
      .sort((left, right) => left.name.localeCompare(right.name))
      .slice(0, MAX_ZONES);

    if (zoneFilter.length > 0 && selectedZones.length === 0) {
      return {
        ok: false,
        error: {
          code: "ZONE_NOT_FOUND",
          message: `zone '${zoneFilter}' not found`,
        },
      };
    }

    const zones = selectedZones.map((zone) => {
      const events = [...zone.active_events]
        .filter((entry): entry is string => typeof entry === "string" && entry.length > 0)
        .sort((left, right) => left.localeCompare(right))
        .slice(0, MAX_EVENTS_PER_ZONE);

      return {
        name: zone.name,
        dangerLevel: zone.danger_level,
        eventCount: zone.active_events.length,
        events,
      };
    });

    const dedupedEvents = Array.from(
      new Set(zones.flatMap((zone) => zone.events)),
    )
      .sort((left, right) => left.localeCompare(right))
      .slice(0, MAX_SUMMARY_EVENTS);

    const zoneSummary = zones
      .map((zone) => `${zone.name}(${zone.eventCount})`)
      .join(", ");
    const summary = zones.length === 0
      ? "no active events"
      : `active zones ${zones.length}: ${zoneSummary}; deduped events: ${dedupedEvents.join(", ") || "none"}`;

    return {
      ok: true,
      zones,
      dedupedEvents,
      summary,
    };
  },
};
