import { describe, expect, it } from "vitest";
import type { BotanyEcologySnapshotV1, FaunaEcologySnapshotV1 } from "@bong/schema";
import { EcologyAnalyzer } from "../src/ecology-analyzer.js";
import { WorldModel } from "../src/world-model.js";

function zone(args: {
  name: string;
  spiritQi: number;
  plants: number;
  tainted?: number;
  thunder?: number;
}): BotanyEcologySnapshotV1["zones"][number] {
  return {
    zone: args.name,
    spirit_qi: args.spiritQi,
    plant_counts: [{ kind: "ning_mai_cao", count: args.plants }],
    variant_counts: [
      { variant: "none", count: Math.max(0, args.plants - (args.tainted ?? 0) - (args.thunder ?? 0)) },
      { variant: "tainted", count: args.tainted ?? 0 },
      { variant: "thunder", count: args.thunder ?? 0 },
    ],
  };
}

function snapshot(tick: number, zones: BotanyEcologySnapshotV1["zones"]): BotanyEcologySnapshotV1 {
  return { v: 1, tick, zones };
}

describe("EcologyAnalyzer", () => {
  it("emits qi reallocation narration only after five depleted snapshots with a rich target zone", () => {
    const model = new WorldModel();
    const analyzer = new EcologyAnalyzer();
    const narrations = [];

    for (let i = 1; i <= 4; i += 1) {
      narrations.push(
        ...analyzer.ingestBotanyEcology(
          model,
          snapshot(i * 600, [
            zone({ name: "hungry_zone", spiritQi: 0.1, plants: 14 }),
            zone({ name: "empty_rich_zone", spiritQi: 0.9, plants: 1 }),
          ]),
        ),
      );
    }

    expect(narrations).toHaveLength(0);
    expect(
      analyzer.ingestBotanyEcology(
        model,
        snapshot(3_000, [
          zone({ name: "hungry_zone", spiritQi: 0.1, plants: 14 }),
          zone({ name: "empty_rich_zone", spiritQi: 0.9, plants: 1 }),
        ]),
      ),
    ).toEqual([
      expect.objectContaining({
        scope: "broadcast",
        text: expect.stringContaining("某处灵脉已瘦"),
      }),
    ]);
  });

  it("emits multi-zone tainted and thunder-spike narrations with cooldown", () => {
    const model = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    analyzer.ingestBotanyEcology(
      model,
      snapshot(600, [
        zone({ name: "left", spiritQi: 0.5, plants: 10, tainted: 4, thunder: 2 }),
        zone({ name: "right", spiritQi: 0.5, plants: 10, tainted: 4 }),
      ]),
    );
    analyzer.ingestBotanyEcology(
      model,
      snapshot(1_200, [
        zone({ name: "left", spiritQi: 0.5, plants: 10, tainted: 4, thunder: 2 }),
        zone({ name: "right", spiritQi: 0.5, plants: 10, tainted: 4 }),
      ]),
    );

    const first = analyzer.ingestBotanyEcology(
      model,
      snapshot(1_800, [
        zone({ name: "left", spiritQi: 0.5, plants: 14, tainted: 4, thunder: 7 }),
        zone({ name: "right", spiritQi: 0.5, plants: 10, tainted: 4 }),
      ]),
    );
    expect(first).toEqual([
      expect.objectContaining({ text: expect.stringContaining("杂质在蔓延") }),
      expect.objectContaining({ target: "left", text: expect.stringContaining("雷声频繁") }),
    ]);

    const cooledDown = analyzer.ingestBotanyEcology(
      model,
      snapshot(2_400, [
        zone({ name: "left", spiritQi: 0.5, plants: 14, tainted: 4, thunder: 22 }),
        zone({ name: "right", spiritQi: 0.5, plants: 10, tainted: 4 }),
      ]),
    );
    expect(cooledDown).toHaveLength(0);
  });

  it("combines high lingtian pressure with tainted low-qi botany state", () => {
    const model = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    analyzer.ingestBotanyEcology(
      model,
      snapshot(600, [
        zone({ name: "starter_zone", spiritQi: 0.12, plants: 12, tainted: 3 }),
      ]),
    );

    expect(
      analyzer.ingestZonePressureCrossed(model, {
        v: 1,
        kind: "zone_pressure_crossed",
        zone: "starter_zone",
        level: "high",
        raw_pressure: 1.2,
        at_tick: 700,
      }),
    ).toEqual([
      expect.objectContaining({
        scope: "zone",
        target: "starter_zone",
        text: expect.stringContaining("灵田压过土息"),
      }),
    ]);
  });
});

// plan-mundane-fauna-v1 P3：凡兽生态 narration（绝迹 / 迁徙 / 空 snapshot / 冷却）。
function faunaZone(args: {
  name: string;
  spiritQi: number;
  counts: Record<string, number>;
}): FaunaEcologySnapshotV1["zones"][number] {
  return {
    zone: args.name,
    spirit_qi: args.spiritQi,
    species_counts: Object.entries(args.counts).map(([kind, count]) => ({ kind, count })),
  };
}

function faunaSnapshot(
  tick: number,
  zones: FaunaEcologySnapshotV1["zones"],
): FaunaEcologySnapshotV1 {
  return { v: 1, tick, zones };
}

describe("EcologyAnalyzer fauna ecology", () => {
  it("emits a vanish narration once fauna disappears from a low-qi zone over the streak", () => {
    const worldModel = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    // streak 前两帧不产 narration（history < 3）。
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(100, [faunaZone({ name: "north", spiritQi: 0.1, counts: { rabbit: 5 } })]),
      ),
    ).toHaveLength(0);
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(200, [faunaZone({ name: "north", spiritQi: 0.1, counts: { rabbit: 2 } })]),
      ),
    ).toHaveLength(0);

    // 第三帧归零 + 低灵气 → 绝迹 narration（perception, zone scope）。
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(300, [faunaZone({ name: "north", spiritQi: 0.1, counts: {} })]),
      ),
    ).toEqual([
      expect.objectContaining({
        scope: "zone",
        target: "north",
        style: "perception",
        text: expect.stringContaining("鸟兽声稀"),
      }),
    ]);
  });

  it("suppresses the vanish narration when the zone qi is still healthy", () => {
    const worldModel = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    analyzer.ingestFaunaEcology(
      worldModel,
      faunaSnapshot(100, [faunaZone({ name: "grove", spiritQi: 0.6, counts: { rabbit: 5 } })]),
    );
    analyzer.ingestFaunaEcology(
      worldModel,
      faunaSnapshot(200, [faunaZone({ name: "grove", spiritQi: 0.6, counts: { rabbit: 2 } })]),
    );
    // qi 0.6 >= FAUNA_LOW_QI(0.2) → 归零也不产 narration（消失是别的原因，不归因灵气）。
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(300, [faunaZone({ name: "grove", spiritQi: 0.6, counts: {} })]),
      ),
    ).toHaveLength(0);
  });

  it("emits a herd migration narration when a herd flees but the zone is not empty", () => {
    const worldModel = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    analyzer.ingestFaunaEcology(
      worldModel,
      faunaSnapshot(100, [
        faunaZone({ name: "north_slope", spiritQi: 0.1, counts: { goat: 6, chicken: 2 } }),
      ]),
    );
    // 山羊群 6 → 1（残量），鸡仍在 → 迁徙而非绝迹。
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(200, [
          faunaZone({ name: "north_slope", spiritQi: 0.1, counts: { goat: 1, chicken: 2 } }),
        ]),
      ),
    ).toEqual([
      expect.objectContaining({
        scope: "zone",
        target: "north_slope",
        style: "perception",
        text: expect.stringContaining("弃地而走"),
      }),
    ]);
  });

  it("does not emit fauna narrations for an empty snapshot", () => {
    const worldModel = new WorldModel();
    const analyzer = new EcologyAnalyzer();
    expect(analyzer.ingestFaunaEcology(worldModel, faunaSnapshot(500, []))).toHaveLength(0);
  });

  it("respects the narration cooldown for repeated migrations in the same zone", () => {
    const worldModel = new WorldModel();
    const analyzer = new EcologyAnalyzer();

    analyzer.ingestFaunaEcology(
      worldModel,
      faunaSnapshot(100, [faunaZone({ name: "ridge", spiritQi: 0.1, counts: { goat: 6 } })]),
    );
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(200, [faunaZone({ name: "ridge", spiritQi: 0.1, counts: { goat: 1 } })]),
      ),
    ).toHaveLength(1);

    // 冷却窗内（200→400 << 12000）第二次迁徙被压制。
    analyzer.ingestFaunaEcology(
      worldModel,
      faunaSnapshot(300, [faunaZone({ name: "ridge", spiritQi: 0.1, counts: { goat: 6 } })]),
    );
    expect(
      analyzer.ingestFaunaEcology(
        worldModel,
        faunaSnapshot(400, [faunaZone({ name: "ridge", spiritQi: 0.1, counts: { goat: 1 } })]),
      ),
    ).toHaveLength(0);
  });
});
