import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import { CHANNELS, REDIS_V1_CHANNELS } from "../src/channels.js";
import { ServerDataV1 } from "../src/server-data.js";
import {
  validateSpiritTreasureDialogueRequestV1Contract,
  validateSpiritTreasureDialogueV1Contract,
} from "../src/spirit-treasure.js";
import { validate } from "../src/validate.js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8"));
}

describe("spirit treasure schema", () => {
  it("freezes dialogue Redis channels", () => {
    expect(CHANNELS.SPIRIT_TREASURE_DIALOGUE_REQUEST).toBe(
      "bong:spirit_treasure_dialogue_request",
    );
    expect(CHANNELS.SPIRIT_TREASURE_DIALOGUE).toBe("bong:spirit_treasure_dialogue");
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SPIRIT_TREASURE_DIALOGUE_REQUEST);
    expect(REDIS_V1_CHANNELS).toContain(CHANNELS.SPIRIT_TREASURE_DIALOGUE);
  });

  it("accepts request and response samples", () => {
    expect(
      validateSpiritTreasureDialogueRequestV1Contract(
        loadSample("spirit-treasure-dialogue-request.sample.json"),
      ).ok,
    ).toBe(true);
    expect(
      validateSpiritTreasureDialogueV1Contract(
        loadSample("spirit-treasure-dialogue.sample.json"),
      ).ok,
    ).toBe(true);
  });

  it("accepts server data state and dialogue samples", () => {
    for (const sample of [
      "server-data.spirit-treasure-state.sample.json",
      "server-data.spirit-treasure-dialogue.sample.json",
    ]) {
      expect(validate(ServerDataV1, loadSample(sample)).ok).toBe(true);
    }
  });

  it("rejects unknown request fields", () => {
    const sample = loadSample("spirit-treasure-dialogue-request.sample.json") as Record<
      string,
      unknown
    >;

    expect(
      validateSpiritTreasureDialogueRequestV1Contract({
        ...sample,
        model: "unexpected",
      }).ok,
    ).toBe(false);
  });
});
