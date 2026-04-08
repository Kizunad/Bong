import type { PlayerProfile } from "@bong/schema";

const STRONG_POWER_THRESHOLD = 0.7;
const WEAK_POWER_THRESHOLD = 0.3;
const GINI_WARNING_THRESHOLD = 0.35;
const GINI_CRITICAL_THRESHOLD = 0.55;

export interface BalancePlayerSummary {
  name: string;
  zone: string;
  compositePower: number;
}

export interface BalanceSummary {
  gini: number;
  severity: "healthy" | "warning" | "critical";
  severityLabel: string;
  strongPlayers: BalancePlayerSummary[];
  weakPlayers: BalancePlayerSummary[];
  dominantStrongZone: string | null;
  weakestZone: string | null;
  advice: string;
}

export function giniCoefficient(powers: number[]): number {
  const sorted = powers
    .filter((value) => Number.isFinite(value) && value >= 0)
    .sort((a, b) => a - b);

  const count = sorted.length;
  if (count === 0) {
    return 0;
  }

  const sum = sorted.reduce((acc, value) => acc + value, 0);
  if (sum === 0) {
    return 0;
  }

  let numerator = 0;
  for (let i = 0; i < count; i++) {
    numerator += (2 * (i + 1) - count - 1) * sorted[i];
  }

  return numerator / (count * sum);
}

export function summarizeBalance(players: PlayerProfile[]): BalanceSummary {
  const strongPlayers = [...players]
    .filter((player) => player.composite_power > STRONG_POWER_THRESHOLD)
    .sort((a, b) => b.composite_power - a.composite_power)
    .map(toBalancePlayerSummary);

  const weakPlayers = [...players]
    .filter((player) => player.composite_power < WEAK_POWER_THRESHOLD)
    .sort((a, b) => a.composite_power - b.composite_power)
    .map(toBalancePlayerSummary);

  const gini = giniCoefficient(players.map((player) => player.composite_power));
  const severity = classifyBalanceSeverity(gini);
  const dominantStrongZone = mostCommonZone(strongPlayers.map((player) => player.zone));
  const weakestZone = mostCommonZone(weakPlayers.map((player) => player.zone));

  return {
    gini,
    severity,
    severityLabel: formatSeverityLabel(severity),
    strongPlayers,
    weakPlayers,
    dominantStrongZone,
    weakestZone,
    advice: buildBalanceAdvice({
      playerCount: players.length,
      severity,
      strongPlayers,
      weakPlayers,
      dominantStrongZone,
      weakestZone,
    }),
  };
}

function toBalancePlayerSummary(player: PlayerProfile): BalancePlayerSummary {
  return {
    name: player.name,
    zone: player.zone,
    compositePower: player.composite_power,
  };
}

function classifyBalanceSeverity(
  gini: number,
): BalanceSummary["severity"] {
  if (gini >= GINI_CRITICAL_THRESHOLD) {
    return "critical";
  }

  if (gini >= GINI_WARNING_THRESHOLD) {
    return "warning";
  }

  return "healthy";
}

function formatSeverityLabel(severity: BalanceSummary["severity"]): string {
  switch (severity) {
    case "critical":
      return "严重失衡";
    case "warning":
      return "偏离平衡";
    case "healthy":
      return "大体均衡";
  }
}

function mostCommonZone(zones: string[]): string | null {
  if (zones.length === 0) {
    return null;
  }

  const counts = new Map<string, number>();
  for (const zone of zones) {
    counts.set(zone, (counts.get(zone) ?? 0) + 1);
  }

  let winner: string | null = null;
  let winnerCount = 0;
  for (const [zone, count] of counts) {
    if (count > winnerCount) {
      winner = zone;
      winnerCount = count;
    }
  }

  return winner;
}

function buildBalanceAdvice(args: {
  playerCount: number;
  severity: BalanceSummary["severity"];
  strongPlayers: BalancePlayerSummary[];
  weakPlayers: BalancePlayerSummary[];
  dominantStrongZone: string | null;
  weakestZone: string | null;
}): string {
  const {
    playerCount,
    severity,
    strongPlayers,
    weakPlayers,
    dominantStrongZone,
    weakestZone,
  } = args;

  if (playerCount === 0) {
    return "暂无玩家，保持静观。";
  }

  if (severity === "critical") {
    const actions: string[] = [];
    if (strongPlayers[0]) {
      actions.push(`对 ${strongPlayers[0].name} 施压`);
    }
    if (weakestZone) {
      actions.push(`${weakestZone} 增加机缘密度`);
    }
    if (dominantStrongZone && strongPlayers.length > 1) {
      actions.push(`${dominantStrongZone} 降低资源堆积`);
    }
    return actions.length > 0 ? actions.join("，") : "拆解强弱分层，避免单区垄断。";
  }

  if (severity === "warning") {
    const actions: string[] = [];
    if (strongPlayers[0]) {
      actions.push(`轻压 ${strongPlayers[0].name}`);
    }
    if (weakestZone) {
      actions.push(`${weakestZone} 补益低阶资源`);
    }
    return actions.length > 0 ? actions.join("，") : "缓慢拉平强弱差距。";
  }

  if (weakPlayers.length > 0) {
    return "保持总体均衡，同时关注弱者成长窗口。";
  }

  return "维持当前张弛，观察下一轮。";
}
