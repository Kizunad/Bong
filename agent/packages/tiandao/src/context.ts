/**
 * Context Assembler — 模块化上下文拼装引擎
 * 按 Agent 角色裁剪 world state 为结构化 prompt
 */

import type { ChatSignal, GameEvent, PlayerProfile, WorldStateV1, ZoneSnapshot } from "@bong/schema";

import { balanceAdvice, type BalanceSeverity } from "./balance.js";
import type { AgentDecision } from "./parse.js";
import {
  WORLD_MODEL_MAX_ZONE_HISTORY,
  type WorldModel,
  type ZoneTrend,
  type ZoneTrendSnapshot,
} from "./world-model.js";

const AGENT_SORT_ORDER: Record<string, number> = {
  calamity: 0,
  mutation: 1,
  era: 2,
};

export interface ContextMemory {
  worldModel?: WorldModel;
}

export interface ContextRenderInput extends ContextMemory {
  agentName: string;
  state: WorldStateV1;
}

export interface ContextBlock {
  name: string;
  priority: number; // 0=最高，越大越容易被裁剪
  required: boolean;
  render: (input: ContextRenderInput) => string;
}

export interface ContextRecipe {
  agentName: string;
  blocks: ContextBlock[];
  maxTokenEstimate: number; // 粗估 token 上限 (1 token ≈ 4 chars 中英混合按 2 chars)
}

function estimateTokens(text: string): number {
  // 粗估：中英文混合平均 2 chars/token
  return Math.ceil(text.length / 2);
}

function formatSigned(value: number, fractionDigits = 2): string {
  return `${value >= 0 ? "+" : ""}${value.toFixed(fractionDigits)}`;
}

function truncateText(text: string, maxLength: number): string {
  if (text.length <= maxLength) return text;
  return `${text.slice(0, Math.max(0, maxLength - 1))}…`;
}

function compareAgentNames(a: string, b: string): number {
  return (AGENT_SORT_ORDER[a] ?? 99) - (AGENT_SORT_ORDER[b] ?? 99) || a.localeCompare(b);
}

function agentDisplayName(name: string): string {
  switch (name) {
    case "calamity":
      return "灾劫 Agent";
    case "mutation":
      return "变化 Agent";
    case "era":
      return "时代 Agent";
    default:
      return `${name} Agent`;
  }
}

function fallbackTrendSnapshot(zone: ZoneSnapshot): ZoneTrendSnapshot {
  return {
    zone: zone.name,
    previousAverageSpiritQi: zone.spirit_qi,
    recentAverageSpiritQi: zone.spirit_qi,
    delta: 0,
    trend: "stable",
    samples: 1,
  };
}

function trendLabel(trend: ZoneTrend): string {
  switch (trend) {
    case "rising":
      return "↑上升中";
    case "falling":
      return "↓下降中";
    default:
      return "→稳定";
  }
}

function overallTrendLabel(delta: number): string {
  if (delta > 0.015) return "微升";
  if (delta < -0.015) return "微降";
  return "持平";
}

function sentimentLabel(sentiment: number): string {
  if (sentiment > 0.15) return "偏正面";
  if (sentiment < -0.15) return "偏负面";
  return "中性";
}

function balanceSeverityLabel(severity: BalanceSeverity): string {
  switch (severity) {
    case "severe":
      return "严重失衡";
    case "uneven":
      return "失衡";
    default:
      return "平衡";
  }
}

function getPlayerZoneLookup(state: WorldStateV1): Map<string, string> {
  const lookup = new Map<string, string>();

  for (const player of state.players) {
    lookup.set(player.name, player.zone);
    lookup.set(player.uuid, player.zone);
  }

  return lookup;
}

function summarizeDecision(decision: AgentDecision): string {
  if (decision.commands.length === 0) {
    return "无行动";
  }

  const summaries = decision.commands.slice(0, 2).map((command) => {
    const params = command.params && typeof command.params === "object"
      ? (command.params as Record<string, unknown>)
      : {};

    if (command.type === "spawn_event") {
      const event = typeof params.event === "string" ? params.event : "事件";
      const intensity =
        typeof params.intensity === "number" ? ` (intensity ${params.intensity.toFixed(2)})` : "";
      return `在 ${command.target} 触发 ${event}${intensity}`;
    }

    if (command.type === "modify_zone") {
      const parts: string[] = [];

      if (typeof params.spirit_qi_delta === "number") {
        parts.push(`灵气 ${formatSigned(params.spirit_qi_delta)}`);
      }

      if (typeof params.danger_level_delta === "number") {
        parts.push(`危险 ${formatSigned(params.danger_level_delta, 0)}`);
      }

      return `${command.target} ${parts.join(", ") || "调整区域参数"}`;
    }

    return `${command.target} 调整 NPC 行为`;
  });

  if (decision.commands.length > 2) {
    summaries.push(`等 ${decision.commands.length} 项`);
  }

  return summaries.join("；");
}

interface KeyPlayerEntry {
  player: PlayerProfile;
  reasons: string[];
}

function collectKeyPlayers(state: WorldStateV1): KeyPlayerEntry[] {
  if (state.players.length === 0) return [];

  const entries = new Map<string, KeyPlayerEntry>();
  const playersByPowerDesc = [...state.players].sort(
    (a, b) => b.composite_power - a.composite_power || a.name.localeCompare(b.name),
  );
  const playersByKarma = [...state.players].sort(
    (a, b) => Math.abs(b.breakdown.karma) - Math.abs(a.breakdown.karma) || a.name.localeCompare(b.name),
  );
  const recentJoinSignals = new Set(
    state.recent_events
      .filter((event) => event.type === "player_join" && typeof event.player === "string")
      .map((event) => event.player as string),
  );

  const addReason = (player: PlayerProfile | undefined, reason: string | null): void => {
    if (!player || !reason) return;

    const entry = entries.get(player.uuid) ?? {
      player,
      reasons: [],
    };

    if (!entry.reasons.includes(reason)) {
      entry.reasons.push(reason);
    }

    entries.set(player.uuid, entry);
  };

  const strongest = playersByPowerDesc[0];
  const karmaExtreme = playersByKarma[0];

  addReason(strongest, strongest ? `综合最强(${strongest.composite_power.toFixed(2)})` : null);

  if (karmaExtreme && Math.abs(karmaExtreme.breakdown.karma) >= 0.2) {
    addReason(
      karmaExtreme,
      `karma ${karmaExtreme.breakdown.karma < 0 ? "偏负" : "偏正"}(${formatSigned(karmaExtreme.breakdown.karma)})`,
    );
  }

  const newcomers = state.players
    .filter(
      (player) =>
        player.active_hours <= 2 || recentJoinSignals.has(player.uuid) || recentJoinSignals.has(player.name),
    )
    .sort((a, b) => a.active_hours - b.active_hours || a.name.localeCompare(b.name))
    .slice(0, 3);

  for (const newcomer of newcomers) {
    addReason(newcomer, `新入世(${newcomer.active_hours.toFixed(1)}h)`);
  }

  const ranked = [...entries.values()].sort((left, right) => {
    const score = (entry: KeyPlayerEntry): number => {
      const hasStrongest = entry.reasons.some((reason) => reason.startsWith("综合最强("));
      const hasKarma = entry.reasons.some((reason) => reason.startsWith("karma "));
      const newcomerSignals = entry.reasons.filter((reason) => reason.startsWith("新入世(")).length;
      return (hasStrongest ? 100 : 0) + (hasKarma ? 10 : 0) + newcomerSignals;
    };

    const scoreDelta = score(right) - score(left);
    if (scoreDelta !== 0) return scoreDelta;

    const powerDelta = right.player.composite_power - left.player.composite_power;
    if (powerDelta !== 0) return powerDelta;

    return left.player.name.localeCompare(right.player.name);
  });

  return ranked.slice(0, 3);
}

export function assembleContext(
  recipe: ContextRecipe,
  state: WorldStateV1,
  memory: ContextMemory = {},
): string {
  const rendered: { index: number; priority: number; required: boolean; text: string }[] = [];

  for (let index = 0; index < recipe.blocks.length; index++) {
    const block = recipe.blocks[index];
    const text = block.render({
      agentName: recipe.agentName,
      state,
      ...memory,
    });

    if (text) {
      rendered.push({
        index,
        priority: block.priority,
        required: block.required,
        text,
      });
    }
  }

  rendered.sort((a, b) => a.priority - b.priority || a.index - b.index);

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

// ─── 预置 Context Blocks ─────────────────────────────────

export const worldSnapshotBlock: ContextBlock = {
  name: "world_snapshot",
  priority: 1,
  required: true,
  render({ state }) {
    const zones =
      state.zones.length === 0
        ? "- 暂无 zone 数据"
        : state.zones
            .map(
              (zone: ZoneSnapshot) =>
                `- ${zone.name}: 灵气 ${zone.spirit_qi.toFixed(2)}, 危险 ${zone.danger_level}/5, 玩家 ${zone.player_count}人`,
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

    const header = "| 玩家 | 综合实力 | 战斗 | karma | 趋势 | 位置 |";
    const sep = "|------|---------|------|-------|------|------|";
    const rows = state.players.map((player: PlayerProfile) => {
      const trend = player.trend === "rising" ? "↑" : player.trend === "falling" ? "↓" : "→";
      return `| ${player.name} | ${player.composite_power.toFixed(2)} | ${player.breakdown.combat.toFixed(2)} | ${player.breakdown.karma.toFixed(2)} | ${trend} | ${player.zone} |`;
    });

    return `## 玩家画像\n${header}\n${sep}\n${rows.join("\n")}`;
  },
};

export const recentEventsBlock: ContextBlock = {
  name: "recent_events",
  priority: 2,
  required: false,
  render({ state }) {
    if (state.recent_events.length === 0) return "";

    const lines = state.recent_events.slice(-10).map((event: GameEvent) => {
      const parts = [`[tick ${event.tick}] ${event.type}`];
      if (event.player) parts.push(event.player);
      if (event.zone) parts.push(`@ ${event.zone}`);
      return parts.join(" ");
    });

    return `## 近期事件\n${lines.join("\n")}`;
  },
};

export const chatSignalsBlock: ContextBlock = {
  name: "chat_signals",
  priority: 3,
  required: false,
  render({ state, worldModel }) {
    const signals = worldModel?.chatSignals.slice(-5) ?? [];
    if (signals.length === 0) return "";

    const zoneLookup = getPlayerZoneLookup(state);
    const lines = signals.map((signal: ChatSignal) => {
      const zone = zoneLookup.get(signal.player) ?? "unknown";
      return `- ${signal.player} [${zone}]: ${truncateText(signal.raw, 24)} (sentiment: ${signal.sentiment.toFixed(2)}, intent: ${signal.intent})`;
    });

    const totalWeight = signals.reduce((sum, signal) => sum + signal.influence_weight, 0);
    const weightedSentiment =
      totalWeight === 0
        ? 0
        : signals.reduce((sum, signal) => sum + signal.sentiment * signal.influence_weight, 0) / totalWeight;

    return `## 近期民意\n${lines.join("\n")}\n民意倾向: ${sentimentLabel(weightedSentiment)} (${weightedSentiment.toFixed(2)})`;
  },
};

export const peerDecisionsBlock: ContextBlock = {
  name: "peer_decisions",
  priority: 4,
  required: false,
  render({ agentName, worldModel }) {
    if (!worldModel || worldModel.lastDecisions.size === 0) return "";

    const peerDecisions = [...worldModel.lastDecisions.entries()]
      .filter(([peerName]) => peerName !== agentName)
      .sort(([left], [right]) => compareAgentNames(left, right));

    if (peerDecisions.length === 0) return "";

    const lines = peerDecisions.map(([peerName, decision]) => {
      return `- ${agentDisplayName(peerName)} (上一轮): ${summarizeDecision(decision)}`;
    });

    return `## 其他天道意志\n${lines.join("\n")}`;
  },
};

export const worldTrendBlock: ContextBlock = {
  name: "world_trend",
  priority: 0,
  required: true,
  render({ state, worldModel }) {
    const snapshots =
      state.zones.length === 0
        ? []
        : state.zones.map((zone) => worldModel?.getZoneTrendSnapshot(zone.name, zone) ?? fallbackTrendSnapshot(zone));

    const lines =
      snapshots.length === 0
        ? "- 暂无 zone 数据"
        : snapshots
            .map((snapshot) => {
              return `- ${snapshot.zone}: 灵气 ${snapshot.previousAverageSpiritQi.toFixed(2)} → ${snapshot.recentAverageSpiritQi.toFixed(2)} (${trendLabel(snapshot.trend)})`;
            })
            .join("\n");

    const overallDelta =
      snapshots.length === 0
        ? 0
        : snapshots.reduce((sum, snapshot) => sum + snapshot.delta, 0) / snapshots.length;

    return `## 世界趋势 (最近 ${WORLD_MODEL_MAX_ZONE_HISTORY} 轮)\n${lines}\n整体灵气: ${overallTrendLabel(overallDelta)} (${formatSigned(overallDelta)})`;
  },
};

export const balanceBlock: ContextBlock = {
  name: "balance",
  priority: 1,
  required: true,
  render({ state, worldModel }) {
    const cachedSnapshot = worldModel?.getBalanceSnapshot();
    const analysis =
      cachedSnapshot && cachedSnapshot.tick === state.tick && cachedSnapshot.playerCount === state.players.length
        ? cachedSnapshot.analysis
        : balanceAdvice(state.players);
    const strongPlayers =
      analysis.strongPlayers.length === 0
        ? "无"
        : analysis.strongPlayers.map((player) => `${player.name}(${player.power.toFixed(2)}) @ ${player.zone}`).join(", ");
    const weakPlayers =
      analysis.weakPlayers.length === 0
        ? "无"
        : analysis.weakPlayers.map((player) => `${player.name}(${player.power.toFixed(2)}) @ ${player.zone}`).join(", ");
    const dominantZones =
      analysis.dominantZones.length === 0 ? "无" : analysis.dominantZones.join(", ");
    const adviceLines = analysis.recommendations
      .map((recommendation) => `- [${recommendation.kind}] ${recommendation.summary}`)
      .join("\n");

    return `## 天道平衡态\nGini 系数: ${analysis.gini.toFixed(2)} (${balanceSeverityLabel(analysis.severity)})\n强者: ${strongPlayers}\n弱者: ${weakPlayers}\n资源集中: ${dominantZones}\n建议:\n${adviceLines}`;
  },
};

export const currentEraBlock: ContextBlock = {
  name: "current_era",
  priority: 0,
  required: true,
  render({ worldModel }) {
    const currentEra = worldModel?.currentEra;
    if (!currentEra) {
      return "## 当前时代\n- 尚无时代宣告";
    }

    return `## 当前时代\n- 名称: ${currentEra.name}\n- 延续自 tick ${currentEra.sinceTick}\n- 全局影响: ${currentEra.globalEffect}`;
  },
};

export const keyPlayerBlock: ContextBlock = {
  name: "key_player",
  priority: 0,
  required: true,
  render({ state }) {
    const keyPlayers = collectKeyPlayers(state);
    if (keyPlayers.length === 0) return "";

    const lines = keyPlayers.map(({ player, reasons }) => {
      return `- ${player.name}: ${reasons.join(", ")}, 所在 ${player.zone}`;
    });

    return `## 关键人物\n${lines.join("\n")}`;
  },
};

// ─── 预置 Recipes ─────────────────────────────────────────

export const CALAMITY_RECIPE: ContextRecipe = {
  agentName: "calamity",
  maxTokenEstimate: 3000,
  blocks: [
    { ...keyPlayerBlock, priority: 0, required: true },
    { ...playerProfilesBlock, priority: 1, required: true },
    { ...recentEventsBlock, priority: 2, required: true },
    { ...chatSignalsBlock, priority: 3, required: false },
    { ...peerDecisionsBlock, priority: 4, required: false },
    { ...worldSnapshotBlock, priority: 5, required: false },
  ],
};

export const MUTATION_RECIPE: ContextRecipe = {
  agentName: "mutation",
  maxTokenEstimate: 3000,
  blocks: [
    { ...worldTrendBlock, priority: 0, required: true },
    { ...worldSnapshotBlock, priority: 1, required: true },
    { ...playerProfilesBlock, priority: 2, required: false },
    { ...chatSignalsBlock, priority: 3, required: false },
    { ...recentEventsBlock, priority: 4, required: false },
    { ...keyPlayerBlock, priority: 5, required: false },
  ],
};

export const ERA_RECIPE: ContextRecipe = {
  agentName: "era",
  maxTokenEstimate: 4000,
  blocks: [
    { ...currentEraBlock, priority: 0, required: true },
    { ...worldTrendBlock, priority: 1, required: true },
    { ...balanceBlock, priority: 2, required: true },
    { ...peerDecisionsBlock, priority: 3, required: false },
    { ...keyPlayerBlock, priority: 4, required: false },
    { ...worldSnapshotBlock, priority: 5, required: false },
    { ...playerProfilesBlock, priority: 6, required: false },
    { ...recentEventsBlock, priority: 7, required: false },
  ],
};
