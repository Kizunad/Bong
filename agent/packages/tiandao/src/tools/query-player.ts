import { NEWBIE_POWER_THRESHOLD } from "@bong/schema";
import type { AgentTool } from "./types.js";
import { toolSchema } from "./types.js";

const QUERY_KEYS = ["uuid", "name"] as const;

interface QueryPlayerArgs {
  uuid?: string;
  name?: string;
}

type QuerySelector =
  | { ok: true; by: "uuid" | "name"; value: string }
  | { ok: false; code: "INVALID_QUERY"; message: string };

function parseSelector(args: QueryPlayerArgs): QuerySelector {
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

export const queryPlayerTool: AgentTool<QueryPlayerArgs, unknown> = {
  name: "query-player",
  description:
    "Lookup one player by uuid or name with power breakdown, position and newcomer/newbie protection signals",
  readonly: true,
  parameters: toolSchema.object(
    {
      uuid: toolSchema.string(),
      name: toolSchema.string(),
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
      player: toolSchema.object(
        {
          uuid: toolSchema.string(),
          name: toolSchema.string(),
          realm: toolSchema.string(),
          trend: toolSchema.string(),
          zone: toolSchema.string(),
          pos: toolSchema.array(toolSchema.number()),
          compositePower: toolSchema.number(),
          recentKills: toolSchema.number(),
          recentDeaths: toolSchema.number(),
          breakdown: toolSchema.object(
            {
              combat: toolSchema.number(),
              wealth: toolSchema.number(),
              social: toolSchema.number(),
              karma: toolSchema.number(),
              territory: toolSchema.number(),
            },
            { additionalProperties: false },
          ),
          lifeRecord: toolSchema.object(
            {
              recentBiographySummary: toolSchema.string(),
              recentSkillMilestonesSummary: toolSchema.string(),
              recentSkillMilestones: toolSchema.array(
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
            },
            { additionalProperties: false },
          ),
          social: toolSchema.anyOf(
            toolSchema.null(),
            toolSchema.object(
              {
                renown: toolSchema.object(
                  {
                    fame: toolSchema.number(),
                    notoriety: toolSchema.number(),
                    topTags: toolSchema.array(toolSchema.string()),
                  },
                  { additionalProperties: false },
                ),
                relationships: toolSchema.array(
                  toolSchema.object(
                    {
                      kind: toolSchema.string(),
                      peer: toolSchema.string(),
                      sinceTick: toolSchema.number(),
                      metadata: toolSchema.unknown(),
                    },
                    { additionalProperties: false },
                  ),
                ),
                exposedToCount: toolSchema.number(),
                factionMembership: toolSchema.anyOf(
                  toolSchema.null(),
                  toolSchema.object(
                    {
                      faction: toolSchema.string(),
                      rank: toolSchema.number(),
                      loyalty: toolSchema.number(),
                      betrayalCount: toolSchema.number(),
                      permanentlyRefused: toolSchema.boolean(),
                    },
                    { additionalProperties: false },
                  ),
                ),
              },
              {
                required: ["renown", "relationships", "exposedToCount"],
                additionalProperties: false,
              },
            ),
          ),
        },
        { additionalProperties: false },
      ),
      protection: toolSchema.object(
        {
          newbieThreshold: toolSchema.number(),
          newbieProtected: toolSchema.boolean(),
          newcomerDetected: toolSchema.boolean(),
          protected: toolSchema.boolean(),
          reasons: toolSchema.array(toolSchema.string()),
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
        error: {
          code: "PLAYER_NOT_FOUND",
          message: `player not found by ${selector.by}: ${selector.value}`,
        },
      };
    }

    const newcomerDetected = ctx.latestState.recent_events.some(
      (event) =>
        event.type === "player_join" &&
        (event.player === player.uuid ||
          event.player === player.name ||
          event.target === player.uuid ||
          event.target === player.name),
    );
    const newbieProtected = player.composite_power < NEWBIE_POWER_THRESHOLD;
    const lifeRecord = player.life_record as {
      recent_biography_summary?: string;
      recent_skill_milestones_summary?: string;
      skill_milestones?: Array<{
        skill: string;
        new_lv: number;
        achieved_at: number;
        narration: string;
        total_xp_at: number;
      }>;
    } | undefined;
    const recentSkillMilestones = lifeRecord?.skill_milestones ?? [];
    const latestSkillMilestone = recentSkillMilestones.at(-1);
    const protectionReasons: string[] = [];
    if (newbieProtected) {
      protectionReasons.push(`composite_power < ${NEWBIE_POWER_THRESHOLD}`);
    }
    if (newcomerDetected) {
      protectionReasons.push("recent player_join signal");
    }

    return {
      ok: true,
      query: {
        by: selector.by,
        value: selector.value,
      },
      player: {
        uuid: player.uuid,
        name: player.name,
        realm: player.realm,
        trend: player.trend,
        zone: player.zone,
        pos: [player.pos[0], player.pos[1], player.pos[2]],
        compositePower: player.composite_power,
        recentKills: player.recent_kills,
        recentDeaths: player.recent_deaths,
        breakdown: {
          combat: player.breakdown.combat,
          wealth: player.breakdown.wealth,
          social: player.breakdown.social,
          karma: player.breakdown.karma,
          territory: player.breakdown.territory,
        },
        lifeRecord: {
          recentBiographySummary: lifeRecord?.recent_biography_summary ?? "",
          recentSkillMilestonesSummary:
            lifeRecord?.recent_skill_milestones_summary ?? "",
          recentSkillMilestones:
            recentSkillMilestones.map((milestone) => ({
              skill: milestone.skill,
              newLv: milestone.new_lv,
              achievedAt: milestone.achieved_at,
              narration: milestone.narration,
              totalXpAt: milestone.total_xp_at,
            })) ?? [],
        },
        social: player.social
          ? {
              renown: {
                fame: player.social.renown.fame,
                notoriety: player.social.renown.notoriety,
                topTags: player.social.renown.top_tags.map((tag) => tag.tag),
              },
              relationships: player.social.relationships.map((relationship) => ({
                kind: relationship.kind,
                peer: relationship.peer,
                sinceTick: relationship.since_tick,
                metadata: relationship.metadata,
              })),
              exposedToCount: player.social.exposed_to_count,
              factionMembership: player.social.faction_membership
                ? {
                    faction: player.social.faction_membership.faction,
                    rank: player.social.faction_membership.rank,
                    loyalty: player.social.faction_membership.loyalty,
                    betrayalCount: player.social.faction_membership.betrayal_count,
                    permanentlyRefused:
                      player.social.faction_membership.permanently_refused,
                  }
                : null,
            }
          : null,
      },
      protection: {
        newbieThreshold: NEWBIE_POWER_THRESHOLD,
        newbieProtected,
        newcomerDetected,
        protected: newbieProtected || newcomerDetected,
        reasons: protectionReasons,
      },
      summary: `${player.name}@${player.zone} power ${player.composite_power.toFixed(2)}, kills ${player.recent_kills}, deaths ${player.recent_deaths}, ${socialSummary(player)}, ${latestSkillMilestone ? `latest skill ${latestSkillMilestone.skill} Lv.${latestSkillMilestone.new_lv}` : `skill milestones ${recentSkillMilestones.length}`}`,
    };
  },
};

function socialSummary(player: {
  social?: {
    renown: { fame: number; notoriety: number; top_tags: Array<{ tag: string }> };
    relationships: unknown[];
  };
}): string {
  if (!player.social) return "social unknown";
  const tags = player.social.renown.top_tags.map((tag) => tag.tag).join("/") || "no tags";
  return `renown ${player.social.renown.fame}/${player.social.renown.notoriety} ${tags}, relationships ${player.social.relationships.length}`;
}
