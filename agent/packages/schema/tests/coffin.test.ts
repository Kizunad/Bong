import { describe, expect, it } from "vitest";

import { ClientRequestV1 } from "../src/client-request.js";
import { ServerDataV1 } from "../src/server-data.js";
import { validate } from "../src/validate.js";

describe("coffin schema", () => {
  it("accepts coffin lifecycle client requests", () => {
    for (const payload of [
      {
        v: 1,
        type: "coffin_place",
        x: 4,
        y: 65,
        z: -9,
        item_instance_id: 4242,
      },
      {
        v: 1,
        type: "coffin_enter",
        x: 4,
        y: 65,
        z: -9,
      },
      {
        v: 1,
        type: "coffin_leave",
      },
    ]) {
      expect(validate(ClientRequestV1, payload).ok).toBe(true);
    }
  });

  it("accepts coffin state server_data payload", () => {
    expect(
      validate(ServerDataV1, {
        v: 1,
        type: "coffin_state",
        in_coffin: true,
        lifespan_rate_multiplier: 0.9,
      }).ok,
    ).toBe(true);
  });
});
