import { describe, expect, it } from "vitest";

import {
  SPAWN_TUTORIAL_HOOK_KEYS,
  SPAWN_TUTORIAL_NARRATION_BASELINES,
} from "../src/narration/spawn-tutorial-narration.js";

describe("spawn tutorial narration baselines", () => {
  it("pins five silent-guidance baseline lines", () => {
    expect(SPAWN_TUTORIAL_NARRATION_BASELINES).toHaveLength(5);
    expect(SPAWN_TUTORIAL_NARRATION_BASELINES.map((entry) => entry.hook)).toEqual(
      [...SPAWN_TUTORIAL_HOOK_KEYS],
    );
  });

  it("keeps baseline lines free of modern tutorial vocabulary", () => {
    const forbidden = /UI|任务|教程|进度|点击|下一步|progress|quest/i;
    for (const entry of SPAWN_TUTORIAL_NARRATION_BASELINES) {
      expect(entry.text).not.toMatch(forbidden);
    }
  });
});
