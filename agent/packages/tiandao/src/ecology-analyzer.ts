import type {
  BotanyEcologySnapshotV1,
  BotanyZoneEcologyV1,
  LingtianZonePressureV1,
  Narration,
} from "@bong/schema";
import type { WorldModel } from "./world-model.js";

const REALLOCATION_STREAK = 5;
const REALLOCATION_LOW_QI = 0.15;
const REALLOCATION_RICH_QI = 0.85;
const HIGH_PLANT_COUNT_THRESHOLD = 10;
const TAINTED_THRESHOLD = 3;
const THUNDER_THRESHOLD = 5;
const MULTI_ZONE_TAINTED_STREAK = 3;
const THUNDER_SPIKE_RATIO = 3;
const NARRATION_COOLDOWN_TICKS = 12_000;

export class EcologyAnalyzer {
  private readonly lastNarrationTickByKey = new Map<string, number>();

  ingestBotanyEcology(worldModel: WorldModel, snapshot: BotanyEcologySnapshotV1): Narration[] {
    worldModel.ingestBotanyEcology(snapshot);

    const narrations: Narration[] = [];
    const reallocation = this.maybeNarrateQiReallocation(worldModel, snapshot);
    if (reallocation) {
      narrations.push(reallocation);
    }

    const tainted = this.maybeNarrateMultiZoneTainted(worldModel, snapshot.tick);
    if (tainted) {
      narrations.push(tainted);
    }

    for (const zone of snapshot.zones) {
      const thunder = this.maybeNarrateThunderSpike(worldModel, zone, snapshot.tick);
      if (thunder) {
        narrations.push(thunder);
      }
    }

    return narrations;
  }

  ingestLingtianZonePressure(
    worldModel: WorldModel,
    event: LingtianZonePressureV1,
  ): Narration[] {
    worldModel.ingestLingtianZonePressure(event);
    if (event.level !== "high") {
      return [];
    }

    const zone = worldModel.botany_ecology?.zones.find((entry) => entry.zone === event.zone);
    if (!zone || zone.spirit_qi >= 0.2 || variantCount(zone, "tainted") <= 2) {
      return [];
    }

    if (!this.canNarrate(`joint:${event.zone}`, event.tick)) {
      return [];
    }

    return [
      {
        scope: "zone",
        target: event.zone,
        style: "narration",
        text: "此地灵田压过土息，草木紫斑仍不肯退。天道只记账，不问是谁先伸手。",
      },
    ];
  }

  private maybeNarrateQiReallocation(
    worldModel: WorldModel,
    snapshot: BotanyEcologySnapshotV1,
  ): Narration | null {
    const depletedZone = snapshot.zones.find((zone) => {
      const history = worldModel.getBotanyEcologyHistory(zone.zone).slice(-REALLOCATION_STREAK);
      return (
        history.length >= REALLOCATION_STREAK &&
        history.every(
          (entry) =>
            entry.spirit_qi < REALLOCATION_LOW_QI &&
            totalPlantCount(entry) >= HIGH_PLANT_COUNT_THRESHOLD,
        )
      );
    });

    if (!depletedZone) {
      return null;
    }

    const richZone = snapshot.zones.find((zone) => zone.spirit_qi > REALLOCATION_RICH_QI);
    if (!richZone || !this.canNarrate("qi_reallocation", snapshot.tick)) {
      return null;
    }

    return {
      scope: "broadcast",
      style: "narration",
      text: "某处灵脉已瘦，无人应。另一处灵气渐聚，犹无人知。",
    };
  }

  private maybeNarrateMultiZoneTainted(
    worldModel: WorldModel,
    tick: number,
  ): Narration | null {
    const recent = worldModel.getRecentBotanyEcologySnapshots().slice(-MULTI_ZONE_TAINTED_STREAK);
    if (
      recent.length < MULTI_ZONE_TAINTED_STREAK ||
      !recent.every(
        (snapshot) =>
          snapshot.zones.filter((zone) => variantCount(zone, "tainted") > TAINTED_THRESHOLD)
            .length >= 2,
      ) ||
      !this.canNarrate("multi_zone_tainted", tick)
    ) {
      return null;
    }

    return {
      scope: "broadcast",
      style: "perception",
      text: "天地真元中有某种杂质在蔓延。枯藤上有紫斑，但此并非普通枯腐。",
    };
  }

  private maybeNarrateThunderSpike(
    worldModel: WorldModel,
    zone: BotanyZoneEcologyV1,
    tick: number,
  ): Narration | null {
    const thunderCount = variantCount(zone, "thunder");
    if (thunderCount <= THUNDER_THRESHOLD) {
      return null;
    }

    const previous = worldModel.getZoneAnomalyWindow(zone.zone).slice(0, -1);
    const previousThunderAverage =
      previous.reduce((sum, entry) => sum + entry.thunderCount, 0) / Math.max(previous.length, 1);
    if (
      previousThunderAverage <= 0 ||
      thunderCount / previousThunderAverage < THUNDER_SPIKE_RATIO ||
      !this.canNarrate(`thunder:${zone.zone}`, tick)
    ) {
      return null;
    }

    return {
      scope: "zone",
      target: zone.zone,
      style: "perception",
      text: "那片区域最近雷声频繁，草木都学会了蓄势。",
    };
  }

  private canNarrate(key: string, tick: number): boolean {
    const lastTick = this.lastNarrationTickByKey.get(key);
    if (lastTick !== undefined && tick - lastTick < NARRATION_COOLDOWN_TICKS) {
      return false;
    }

    this.lastNarrationTickByKey.set(key, tick);
    return true;
  }
}

function totalPlantCount(zone: BotanyZoneEcologyV1): number {
  return zone.plant_counts.reduce((total, entry) => total + entry.count, 0);
}

function variantCount(zone: BotanyZoneEcologyV1, variant: "tainted" | "thunder"): number {
  return zone.variant_counts
    .filter((entry) => entry.variant === variant)
    .reduce((total, entry) => total + entry.count, 0);
}
