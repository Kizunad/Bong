import type { PlayerProfile } from "@bong/schema";

export type BalanceSeverity = "balanced" | "uneven" | "severe";

export type BalanceRecommendationKind =
  | "pressure_strongest"
  | "support_weaker_players"
  | "watch_dominant_zones"
  | "maintain_balance";

export interface BalancePlayerSummary {
  name: string;
  power: number;
  zone: string;
}

export interface BalanceRecommendation {
  kind: BalanceRecommendationKind;
  targets: string[];
  summary: string;
}

export interface BalanceAnalysis {
  gini: number;
  severity: BalanceSeverity;
  strongPlayers: BalancePlayerSummary[];
  weakPlayers: BalancePlayerSummary[];
  dominantZones: string[];
  recommendations: BalanceRecommendation[];
}

const STRONG_POWER_THRESHOLD = 0.7;
const WEAK_POWER_THRESHOLD = 0.3;
const BALANCED_GINI_THRESHOLD = 0.25;
const SEVERE_GINI_THRESHOLD = 0.4;
const MAX_PLAYERS_PER_GROUP = 2;
const MAX_DOMINANT_ZONES = 2;

function compareByPowerDesc(a: PlayerProfile, b: PlayerProfile): number {
  return b.composite_power - a.composite_power || a.name.localeCompare(b.name);
}

function compareByPowerAsc(a: PlayerProfile, b: PlayerProfile): number {
  return a.composite_power - b.composite_power || a.name.localeCompare(b.name);
}

function summarizePlayer(player: PlayerProfile): BalancePlayerSummary {
  return {
    name: player.name,
    power: player.composite_power,
    zone: player.zone,
  };
}

export function giniCoefficient(powers: number[]): number {
  const sorted = [...powers].sort((a, b) => a - b);
  const count = sorted.length;

  if (count === 0) return 0;

  const total = sorted.reduce((sum, power) => sum + power, 0);
  if (total === 0) return 0;

  let numerator = 0;
  for (let index = 0; index < count; index++) {
    numerator += (2 * (index + 1) - count - 1) * sorted[index];
  }

  return numerator / (count * total);
}

export function balanceAdvice(players: PlayerProfile[]): BalanceAnalysis {
  const gini = giniCoefficient(players.map((player) => player.composite_power));

  let severity: BalanceSeverity = "balanced";
  if (gini >= SEVERE_GINI_THRESHOLD) {
    severity = "severe";
  } else if (gini >= BALANCED_GINI_THRESHOLD) {
    severity = "uneven";
  }

  const strongPlayers = [...players]
    .filter((player) => player.composite_power >= STRONG_POWER_THRESHOLD)
    .sort(compareByPowerDesc)
    .slice(0, MAX_PLAYERS_PER_GROUP)
    .map(summarizePlayer);

  const weakPlayers = [...players]
    .filter((player) => player.composite_power <= WEAK_POWER_THRESHOLD)
    .sort(compareByPowerAsc)
    .slice(0, MAX_PLAYERS_PER_GROUP)
    .map(summarizePlayer);

  const zoneTotals = new Map<string, number>();
  for (const player of players) {
    zoneTotals.set(player.zone, (zoneTotals.get(player.zone) ?? 0) + player.composite_power);
  }

  const averageZonePower =
    zoneTotals.size === 0
      ? 0
      : [...zoneTotals.values()].reduce((sum, power) => sum + power, 0) / zoneTotals.size;

  const dominantZones =
    severity === "balanced"
      ? []
      : [...zoneTotals.entries()]
          .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
          .filter(([, power]) => power > 0 && power >= averageZonePower)
          .slice(0, MAX_DOMINANT_ZONES)
          .map(([zone]) => zone);

  const recommendations: BalanceRecommendation[] = [];

  if (severity === "severe" && strongPlayers.length > 0) {
    const targets = strongPlayers.map((player) => player.name);
    recommendations.push({
      kind: "pressure_strongest",
      targets,
      summary: `对 ${targets.join("、")} 施压`,
    });
  }

  if (weakPlayers.length > 0) {
    const targets = [...new Set(weakPlayers.map((player) => player.zone))];
    recommendations.push({
      kind: "support_weaker_players",
      targets,
      summary: `在 ${targets.join("、")} 增加机缘密度`,
    });
  }

  if (dominantZones.length > 0) {
    recommendations.push({
      kind: "watch_dominant_zones",
      targets: dominantZones,
      summary: `关注 ${dominantZones.join("、")} 的资源集中`,
    });
  }

  if (recommendations.length === 0) {
    recommendations.push({
      kind: "maintain_balance",
      targets: [],
      summary: "维持当前平衡",
    });
  }

  return {
    gini,
    severity,
    strongPlayers,
    weakPlayers,
    dominantZones,
    recommendations,
  };
}
