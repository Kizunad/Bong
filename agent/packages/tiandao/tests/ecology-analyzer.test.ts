import { describe, expect, it } from "vitest";
import type { BotanyEcologySnapshotV1 } from "@bong/schema";
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
      analyzer.ingestLingtianZonePressure(model, {
        v: 1,
        zone: "starter_zone",
        level: "high",
        raw_pressure: 1.2,
        tick: 700,
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
