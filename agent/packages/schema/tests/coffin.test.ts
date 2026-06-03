import { describe, expect, it } from "vitest";

import { ClientRequestV1 } from "../src/client-request.js";
import { CoffinGradeV1, CoffinStateV1, ServerDataV1 } from "../src/server-data.js";
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

  it("accepts coffin state server_data payload without coffin_grade (legacy compat)", () => {
    // 旧 payload 不含 coffin_grade → 仍合法（optional field）
    expect(
      validate(ServerDataV1, {
        v: 1,
        type: "coffin_state",
        in_coffin: true,
        lifespan_rate_multiplier: 0.9,
      }).ok,
    ).toBe(true);
  });

  it("accepts coffin state payload with coffin_grade for all four grades", () => {
    const grades: CoffinGradeV1[] = ["mundane", "jade", "stone", "bronze"];
    const multipliers: Record<CoffinGradeV1, number> = {
      mundane: 0.9,
      jade: 0.7,
      stone: 0.5,
      bronze: 0.3,
    };
    for (const grade of grades) {
      const payload = {
        v: 1,
        type: "coffin_state",
        in_coffin: true,
        lifespan_rate_multiplier: multipliers[grade],
        coffin_grade: grade,
      };
      const result = validate(ServerDataV1, payload);
      expect(result.ok, `grade=${grade} should be valid, errors: ${JSON.stringify(result)}`).toBe(
        true,
      );
    }
  });

  it("rejects coffin state with unknown coffin_grade value", () => {
    const result = validate(ServerDataV1, {
      v: 1,
      type: "coffin_state",
      in_coffin: true,
      lifespan_rate_multiplier: 0.9,
      coffin_grade: "diamond",
    });
    expect(result.ok, "unknown grade 'diamond' should be rejected").toBe(false);
  });

  it("CoffinStateV1 standalone: missing coffin_grade parses as undefined (optional)", () => {
    const result = validate(CoffinStateV1, {
      in_coffin: false,
      lifespan_rate_multiplier: 1.0,
    });
    expect(result.ok).toBe(true);
  });

  it("CoffinStateV1 standalone: coffin_grade=mundane roundtrip", () => {
    const obj = {
      in_coffin: true,
      lifespan_rate_multiplier: 0.9,
      coffin_grade: "mundane" as CoffinGradeV1,
    };
    expect(validate(CoffinStateV1, obj).ok).toBe(true);
  });
});
