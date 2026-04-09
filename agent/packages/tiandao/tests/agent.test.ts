import { describe, expect, it } from "vitest";

import { TiandaoAgent } from "../src/agent.js";
import { CALAMITY_RECIPE } from "../src/context.js";
import { createMockClient } from "../src/llm.js";
import { createMockWorldState } from "../src/mock-state.js";

describe("TiandaoAgent fake clock seam", () => {
  it("respects interval via injected now()", async () => {
    const times = [30_000, 35_000, 61_000];
    let index = 0;
    const now = () => {
      const value = times[index] ?? times[times.length - 1];
      index += 1;
      return value;
    };

    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 30_000,
      now,
    });

    const world = createMockWorldState();
    const llm = createMockClient();

    const first = await agent.tick(llm, "mock-model", world);
    const second = await agent.tick(llm, "mock-model", world);
    const third = await agent.tick(llm, "mock-model", world);

    expect(first).not.toBeNull();
    expect(second).toBeNull();
    expect(third).not.toBeNull();
  });
});
