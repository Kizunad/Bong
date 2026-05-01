import type {
  WorldStateV1,
  PlayerProfile,
  ZoneSnapshot,
  GameEvent,
  ChatSignal,
} from "@bong/schema";
import { buildChatSignalsBlock } from "./chat-processor.js";
import type { WorldModel, TrendDirection } from "./world-model.js";

export interface ContextInput {
  state: WorldStateV1;
  chatSignals: ChatSignal[];
  nowSeconds?: number;
  agentName?: string;
  worldModel?: WorldModel;
}

export interface ContextBlock {
  name: string;
  priority: number;
  required: boolean;
  render: (input: ContextInput) => string;
}

export interface ContextRecipe {
  agentName: string;
  blocks: ContextBlock[];
  maxTokenEstimate: number;
}

function estimateTokens(text: string): number {
  return Math.ceil(text.length / 2);
}

export function createContextInput(
  state: WorldStateV1,
  chatSignals: ChatSignal[] = [],
  nowSeconds?: number,
  options: { agentName?: string; worldModel?: WorldModel } = {},
): ContextInput {
  return {
    state,
    chatSignals,
    nowSeconds,
    agentName: options.agentName,
    worldModel: options.worldModel,
  };
}

export function assembleContext(
  recipe: ContextRecipe,
  inputOrState: ContextInput | WorldStateV1,
  options?: { worldModel?: WorldModel; chatSignals?: ChatSignal[] },
): string {
  let input: ContextInput;
  if ("v" in inputOrState && "tick" in inputOrState) {
    const worldModel = options?.worldModel;
    worldModel?.updateState(inputOrState);
    input = createContextInput(inputOrState, options?.chatSignals ?? [], undefined, {
      worldModel,
    });
  } else {
    input = inputOrState as ContextInput;
  }

  const rendered: { priority: number; required: boolean; text: string }[] = [];

  for (const block of recipe.blocks) {
    const text = block.render(input);
    if (text) {
      rendered.push({ priority: block.priority, required: block.required, text });
    }
  }

  rendered.sort((a, b) => a.priority - b.priority);

  let total = 0;
  const included: string[] = [];

  for (const item of rendered) {
    const tokens = estimateTokens(item.text);
    if (total + tokens > recipe.maxTokenEstimate && !item.required) {
      continue;
    }
    included.push(item.text);
    total += tokens;
  }

  return included.join("\n\n---\n\n");
}

export const worldSnapshotBlock: ContextBlock = {
  name: "world_snapshot",
  priority: 1,
  required: true,
  render({ state }) {
    const zones = state.zones
      .map(
        (z: ZoneSnapshot) =>
          `- ${z.name}: 灵气 ${z.spirit_qi.toFixed(2)}, 危险 ${z.danger_level}/5, 玩家 ${z.player_count}人`,
      )
      .join("\n");
    return `## 世界快照\nTick: ${state.tick}, 在线: ${state.players.length}人\n\n${zones}`;
  },
};

export const playerProfilesBlock: ContextBlock = {
  name: "player_profiles",
  priority: 1,
  required: true,
  render({ state }) {
    if (state.players.length === 0) return "";
    const header = "| 玩家 | 综合实力 | 战斗 | karma | 声名 | 关系 | 趋势 | 位置 |";
    const sep = "|------|---------|------|-------|------|------|------|------|";
    const rows = state.players.map((p: PlayerProfile) => {
      const trend = p.trend === "rising" ? "↑" : p.trend === "falling" ? "↓" : "→";
      const renown = p.social
        ? `${p.social.renown.fame}/${p.social.renown.notoriety} ${formatRenownTags(
            p.social.renown.top_tags,
          )}`
        : "-";
      const relationships = p.social ? `${p.social.relationships.length}` : "-";
      return `| ${p.name} | ${p.composite_power.toFixed(2)} | ${p.breakdown.combat.toFixed(2)} | ${p.breakdown.karma.toFixed(2)} | ${renown} | ${relationships} | ${trend} | ${p.zone} |`;
    });
    return `## 玩家画像\n${header}\n${sep}\n${rows.join("\n")}`;
  },
};

export const perceptionEnvelopeBlock: ContextBlock = {
  name: "perception_envelope",
  priority: 1,
  required: true,
  render({ state }) {
    if (state.players.length === 0) return "";

    const lines = state.players.map((player) => {
      const profile = describePerceptionProfile(player.realm);
      const [x, y, z] = player.pos;
      return `- ${player.uuid}: ${player.realm} @ ${player.zone} (${x.toFixed(0)},${y.toFixed(0)},${z.toFixed(0)})；${profile}`;
    });

    return [
      "## 玩家可感知边界",
      "叙事只能写玩家肉眼或神识可感之事；超出范围只能写作远方异象、传闻或 NPC 口述。",
      "匿名：正文不要主动写玩家名，除非渡虚劫点名或死亡遗念。",
      ...lines,
    ].join("\n");
  },
};

function formatRenownTags(tags: NonNullable<PlayerProfile["social"]>["renown"]["top_tags"]): string {
  if (tags.length === 0) return "";
  return `(${tags.map((tag) => tag.tag).join("/")})`;
}

function describePerceptionProfile(realm: string): string {
  switch (realm) {
    case "Awaken":
      return "凡眼近距，只能感到灵气浓淡；不可直叙墙后/远处细节";
    case "Induce":
      return "神识 50m，可感 LivingQi；远处战斗只能作风声/兽鸣";
    case "Condense":
      return "神识约 200m，可感 LivingQi + AmbientLeyline；可写区域灵气精确波动";
    case "Solidify":
      return "神识约 500m，可感 LivingQi + AmbientLeyline + CultivatorRealm；只写境界段，不写具体数值";
    case "Spirit":
      return "神识约 1000m，可感天道注意力与危机预警；可写危机方位，不剧透全貌";
    case "Void":
      return "神识三圈：500m 明察，2000m 只感修士/战斗/灵气波动，外圈仅感渡虚劫/时代法旨等大事";
    default:
      return "境界未知，按凡眼近距处理；不可直叙远处细节";
  }
}

export const recentEventsBlock: ContextBlock = {
  name: "recent_events",
  priority: 2,
  required: false,
  render({ state }) {
    if (state.recent_events.length === 0) return "";
    const lines = state.recent_events.slice(-10).map((e: GameEvent) => {
      const parts = [`[tick ${e.tick}] ${e.type}`];
      if (e.player) parts.push(e.player);
      if (e.zone) parts.push(`@ ${e.zone}`);
      return parts.join(" ");
    });
    return `## 近期事件\n${lines.join("\n")}`;
  },
};

export const chatSignalsBlock: ContextBlock = {
  name: "chat_signals",
  priority: 2,
  required: false,
  render({ chatSignals, nowSeconds }) {
    if (chatSignals.length === 0) {
      return "";
    }

    return buildChatSignalsBlock({
      signals: chatSignals,
      nowSeconds: nowSeconds ?? Math.floor(Date.now() / 1000),
    });
  },
};

export const peerDecisionsBlock: ContextBlock = {
  name: "peer_decisions",
  priority: 3,
  required: false,
  render({ worldModel, agentName }) {
    const peerDecisions = worldModel?.getPeerDecisions(agentName) ?? [];
    if (peerDecisions.length === 0) {
      return "";
    }

    const lines = peerDecisions.map(
      (decision) => `- ${decision.displayName} (上一轮): ${decision.summary}`,
    );
    return `## 其他天道意志\n${lines.join("\n")}`;
  },
};

export const recentNarrationsBlock: ContextBlock = {
  name: "recent_narrations",
  priority: 3,
  required: false,
  render({ worldModel }) {
    const narrations = worldModel?.getRecentNarrations(6) ?? [];
    if (narrations.length === 0) {
      return "";
    }

    const lines = narrations.map((narration) => {
      const target = narration.target ? ` target=${narration.target}` : "";
      return `- ${narration.displayName}: ${narration.scope}${target} ${narration.style}「${narration.text}」`;
    });
    return `## 近轮天道叙事\n${lines.join("\n")}\n要求：本轮若要叙事，必须换物象、换句式，不要复用上述文本或同义近句。`;
  },
};

export const worldTrendBlock: ContextBlock = {
  name: "world_trend",
  priority: 3,
  required: false,
  render({ worldModel }) {
    const summary = worldModel?.getWorldTrendSummary() ?? null;
    const currentEra = worldModel?.currentEra ?? null;
    if (!summary && !currentEra) {
      return "";
    }

    const sections: string[] = [];

    if (currentEra) {
      sections.push(
        [
          "## 当前时代",
          `- ${currentEra.name}（始于 tick ${currentEra.sinceTick}）`,
          `- 律令: ${currentEra.globalEffect}`,
        ].join("\n"),
      );
    }

    if (!summary) {
      return sections.join("\n\n");
    }

    const zoneLines = summary.zones.map(
      (zone) =>
        `- ${zone.name}: 灵气 ${zone.previousSpiritQi.toFixed(2)} → ${zone.currentSpiritQi.toFixed(2)} (${formatTrendArrow(zone.trend)}${formatTrendLabel(zone.trend)})`,
    );

    sections.push(
      `## 世界趋势 (最近 10 轮)\n${zoneLines.join("\n")}\n整体灵气: ${describeWorldTrend(summary.trend)} (${formatSigned(summary.delta)})`,
    );

    return sections.join("\n\n");
  },
};

export const balanceBlock: ContextBlock = {
  name: "balance",
  priority: 3,
  required: false,
  render({ worldModel }) {
    if (!worldModel) {
      return "";
    }

    const balance = worldModel.getBalanceSummary();
    const strongPlayers =
      balance.strongPlayers.length > 0
        ? balance.strongPlayers
            .map((player) => `${player.name}(${player.compositePower.toFixed(2)})`)
            .join(", ") +
          (balance.dominantStrongZone ? ` — 集中在 ${balance.dominantStrongZone}` : "")
        : "无";

    const weakPlayers =
      balance.weakPlayers.length > 0
        ? balance.weakPlayers
            .map((player) => `${player.name}(${player.compositePower.toFixed(2)})`)
            .join(", ") +
          (balance.weakestZone ? ` — ${balance.weakestZone}` : "")
        : "无";

    return [
      "## 天道平衡态",
      `Gini 系数: ${balance.gini.toFixed(2)} (${balance.severityLabel})`,
      `强者: ${strongPlayers}`,
      `弱者: ${weakPlayers}`,
      `建议: ${balance.advice}`,
    ].join("\n");
  },
};

export const keyPlayerBlock: ContextBlock = {
  name: "key_players",
  priority: 3,
  required: false,
  render({ worldModel }) {
    const keyPlayers = worldModel?.getKeyPlayers() ?? [];
    if (keyPlayers.length === 0) {
      return "";
    }

    const lines = keyPlayers.map(
      (player) => `- ${player.name}: ${player.reasons.join("，")} — ${player.note}`,
    );
    return `## 关键人物\n${lines.join("\n")}`;
  },
};

export const CALAMITY_RECIPE: ContextRecipe = {
  agentName: "calamity",
  maxTokenEstimate: 3000,
  blocks: [
    { ...keyPlayerBlock, priority: 0, required: true },
    { ...playerProfilesBlock, priority: 1, required: true },
    { ...perceptionEnvelopeBlock, priority: 2, required: true },
    { ...recentEventsBlock, priority: 3, required: true },
    { ...balanceBlock, priority: 4, required: false },
    { ...recentNarrationsBlock, priority: 5, required: false },
    { ...peerDecisionsBlock, priority: 6, required: false },
    { ...chatSignalsBlock, priority: 7, required: false },
    { ...worldSnapshotBlock, priority: 8, required: false },
  ],
};

export const MUTATION_RECIPE: ContextRecipe = {
  agentName: "mutation",
  maxTokenEstimate: 3000,
  blocks: [
    { ...worldSnapshotBlock, priority: 0, required: true },
    { ...playerProfilesBlock, priority: 1, required: true },
    { ...perceptionEnvelopeBlock, priority: 2, required: true },
    { ...balanceBlock, priority: 3, required: false },
    { ...recentNarrationsBlock, priority: 4, required: false },
    { ...peerDecisionsBlock, priority: 5, required: false },
    { ...chatSignalsBlock, priority: 6, required: false },
    { ...recentEventsBlock, priority: 7, required: false },
  ],
};

export const ERA_RECIPE: ContextRecipe = {
  agentName: "era",
  maxTokenEstimate: 4000,
  blocks: [
    { ...worldSnapshotBlock, priority: 0, required: true },
    { ...peerDecisionsBlock, priority: 1, required: true },
    { ...worldTrendBlock, priority: 2, required: true },
    { ...perceptionEnvelopeBlock, priority: 3, required: true },
    { ...balanceBlock, priority: 4, required: false },
    { ...recentNarrationsBlock, priority: 5, required: false },
    { ...playerProfilesBlock, priority: 6, required: true },
    { ...recentEventsBlock, priority: 7, required: true },
    { ...keyPlayerBlock, priority: 8, required: false },
    { ...chatSignalsBlock, priority: 9, required: false },
  ],
};

function formatTrendArrow(trend: TrendDirection): string {
  switch (trend) {
    case "rising":
      return "↑";
    case "falling":
      return "↓";
    case "stable":
      return "→";
  }
}

function formatTrendLabel(trend: TrendDirection): string {
  switch (trend) {
    case "rising":
      return "上升中";
    case "falling":
      return "下降中";
    case "stable":
      return "稳定";
  }
}

function describeWorldTrend(trend: TrendDirection): string {
  switch (trend) {
    case "rising":
      return "微升";
    case "falling":
      return "微降";
    case "stable":
      return "平稳";
  }
}

function formatSigned(value: number): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}`;
}
