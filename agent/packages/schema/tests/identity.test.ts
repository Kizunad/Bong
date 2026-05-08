import { describe, expect, it } from "vitest";

import {
  validateIdentityPanelStateV1,
  validateWantedPlayerEventV1,
} from "../src/identity.js";
import { CHANNELS } from "../src/channels.js";

describe("WantedPlayerEventV1", () => {
  const valid = {
    event: "wanted_player",
    player_uuid: "11111111-1111-1111-1111-111111111111",
    char_id: "offline:kiz",
    identity_display_name: "毒蛊师小李",
    identity_id: 0,
    reputation_score: -100,
    primary_tag: "dugu_revealed",
    tick: 24_000,
  };

  it("accepts a complete wanted-player event", () => {
    const result = validateWantedPlayerEventV1(valid);
    expect(result.ok).toBe(true);
    expect(result.errors).toEqual([]);
  });

  it("accepts each RevealedTagKindV1 variant as primary_tag", () => {
    const kinds = [
      "dugu_revealed",
      "anqi_master",
      "zhenfa_master",
      "baomai_user",
      "tuike_user",
      "woliu_master",
      "zhenmai_user",
      "sword_master",
      "forge_master",
      "alchemy_master",
    ];
    for (const kind of kinds) {
      const result = validateWantedPlayerEventV1({ ...valid, primary_tag: kind });
      expect(result.ok).toBe(true);
    }
  });

  it("rejects negative tick", () => {
    const result = validateWantedPlayerEventV1({ ...valid, tick: -1 });
    expect(result.ok).toBe(false);
  });

  it("rejects negative identity_id", () => {
    const result = validateWantedPlayerEventV1({ ...valid, identity_id: -1 });
    expect(result.ok).toBe(false);
  });

  it("rejects unknown primary_tag", () => {
    const result = validateWantedPlayerEventV1({ ...valid, primary_tag: "mystery_master" });
    expect(result.ok).toBe(false);
  });

  it("rejects wrong event literal", () => {
    const result = validateWantedPlayerEventV1({ ...valid, event: "low_player" });
    expect(result.ok).toBe(false);
  });

  it("rejects extra fields", () => {
    const result = validateWantedPlayerEventV1({ ...valid, extra: 42 });
    expect(result.ok).toBe(false);
  });
});

describe("IdentityPanelStateV1", () => {
  const valid = {
    active_identity_id: 0,
    last_switch_tick: 0,
    cooldown_remaining_ticks: 0,
    identities: [
      {
        identity_id: 0,
        display_name: "kiz",
        reputation_score: 0,
        frozen: false,
        revealed_tag_kinds: [],
      },
    ],
  };

  it("accepts a single-identity panel", () => {
    const result = validateIdentityPanelStateV1(valid);
    expect(result.ok).toBe(true);
  });

  it("accepts multi-identity panel with revealed tags", () => {
    const payload = {
      ...valid,
      identities: [
        ...valid.identities,
        {
          identity_id: 1,
          display_name: "alt",
          reputation_score: -50,
          frozen: true,
          revealed_tag_kinds: ["dugu_revealed"],
        },
      ],
    };
    const result = validateIdentityPanelStateV1(payload);
    expect(result.ok).toBe(true);
  });

  it("rejects negative cooldown_remaining_ticks", () => {
    const result = validateIdentityPanelStateV1({
      ...valid,
      cooldown_remaining_ticks: -1,
    });
    expect(result.ok).toBe(false);
  });

  it("rejects unknown revealed_tag_kind in entry", () => {
    const payload = {
      ...valid,
      identities: [
        {
          ...valid.identities[0],
          revealed_tag_kinds: ["mystery_master"],
        },
      ],
    };
    const result = validateIdentityPanelStateV1(payload);
    expect(result.ok).toBe(false);
  });

  it("rejects extra fields in entry", () => {
    const payload = {
      ...valid,
      identities: [
        {
          ...valid.identities[0],
          extra: 1,
        },
      ],
    };
    const result = validateIdentityPanelStateV1(payload);
    expect(result.ok).toBe(false);
  });
});

describe("CHANNELS.WANTED_PLAYER", () => {
  it("matches the documented channel name", () => {
    expect(CHANNELS.WANTED_PLAYER).toBe("bong:wanted_player");
  });
});
