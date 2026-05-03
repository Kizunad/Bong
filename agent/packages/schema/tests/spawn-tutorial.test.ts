import { describe, expect, it } from "vitest";

import { validateBiographyEntryV1Contract } from "../src/biography.js";
import { ClientRequestV1 } from "../src/client-request.js";
import {
  validateCoffinOpenedV1Contract,
  validateTutorialHookEventV1Contract,
} from "../src/spawn-tutorial.js";
import { validate } from "../src/validate.js";

describe("spawn tutorial schema", () => {
  it("accepts coffin_open client request", () => {
    expect(
      validate(ClientRequestV1, {
        v: 1,
        type: "coffin_open",
        x: 0,
        y: 69,
        z: 0,
      }).ok,
    ).toBe(true);
  });

  it("accepts hook and coffin-opened events", () => {
    expect(
      validateTutorialHookEventV1Contract({
        v: 1,
        type: "tutorial_hook_event",
        player_id: "offline:Azure",
        hook: "first_meridian_opened",
        tick: 1200,
      }).ok,
    ).toBe(true);
    expect(
      validateCoffinOpenedV1Contract({
        v: 1,
        type: "coffin_opened",
        player_id: "offline:Azure",
        coffin_pos: [0, 69, 0],
        granted_item_id: "spirit_niche_stone",
        tick: 91,
      }).ok,
    ).toBe(true);
  });

  it("accepts spawn tutorial completion biography entry", () => {
    expect(
      validateBiographyEntryV1Contract({
        SpawnTutorialCompleted: {
          minutes_since_spawn: 28,
          tick: 33600,
        },
      }).ok,
    ).toBe(true);
  });
});
