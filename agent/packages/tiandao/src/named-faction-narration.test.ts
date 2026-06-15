import { beforeEach, describe, expect, it, vi } from "vitest";

import { CHANNELS, type NamedFactionStateV1 } from "@bong/schema";

import { NamedFactionNarrationRuntime } from "./named-faction-narration.js";

const { AGENT_NARRATE, NAMED_FACTION_STATE } = CHANNELS;

type Status = NamedFactionStateV1["named_factions"][number]["status"];
type NamedFactionEntry = NamedFactionStateV1["named_factions"][number];

function makeMockClient() {
  const listeners: Map<string, ((channel: string, message: string) => void)[]> = new Map();

  return {
    subscribe: vi.fn(async (_channel: string) => {}),
    on: vi.fn((event: string, listener: (channel: string, message: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    }),
    off: vi.fn((event: string, listener: (channel: string, message: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const index = arr.indexOf(listener);
      if (index !== -1) arr.splice(index, 1);
      listeners.set(event, arr);
    }),
    unsubscribe: vi.fn(async () => {}),
    disconnect: vi.fn(() => {}),
    publish: vi.fn(async (_channel: string, _message: string) => 1),
    emit(channel: string, message: string) {
      for (const listener of listeners.get("message") ?? []) listener(channel, message);
    },
  };
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function namedFactionEntry(
  id: string,
  displayName: string,
  zoneAnchor: string,
  status: Status,
  activeCount: number,
): NamedFactionEntry {
  switch (status) {
    case "active":
      return {
        id,
        display_name: displayName,
        zone_anchor: zoneAnchor,
        current_npc_count: activeCount,
        status,
        is_active: true,
      };
    case "headless":
      return {
        id,
        display_name: displayName,
        zone_anchor: zoneAnchor,
        current_npc_count: activeCount,
        status,
        is_active: true,
      };
    case "decayed":
      return {
        id,
        display_name: displayName,
        zone_anchor: zoneAnchor,
        current_npc_count: 0,
        status,
        is_active: false,
      };
  }
}

function payload(overrides: Partial<Record<"qingyun" | "cangyuan" | "north", Status>> = {}): string {
  const qingyun = overrides.qingyun ?? "active";
  const cangyuan = overrides.cangyuan ?? "active";
  const north = overrides.north ?? "headless";
  return JSON.stringify({
    v: 1,
    kind: "named_faction_state",
    named_factions: [
      namedFactionEntry("qingyun_hunters", "青云猎盟", "qingyun_peaks", qingyun, 2),
      namedFactionEntry("cangyuan_merchants", "沧渊商会", "blood_valley", cangyuan, 1),
      namedFactionEntry("north_waste_drifters", "北荒漂流者", "north_wastes", north, 1),
    ],
    relation_matrix: [],
    at_tick: 1,
  } satisfies NamedFactionStateV1);
}

describe("NamedFactionNarrationRuntime", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let runtime: NamedFactionNarrationRuntime;

  beforeEach(async () => {
    sub = makeMockClient();
    pub = makeMockClient();
    runtime = new NamedFactionNarrationRuntime({ sub, pub });
    await runtime.connect();
  });

  it("connect 订阅 CHANNELS.NAMED_FACTION_STATE", () => {
    expect(sub.subscribe).toHaveBeenCalledWith(NAMED_FACTION_STATE);
  });

  it("初始 active/headless 快照只建基线，不发布叙事", async () => {
    await runtime.handlePayload(payload({ qingyun: "active", north: "headless" }));
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.ignored).toBe(1);
  });

  it("active → headless 发布领袖陨落广播", async () => {
    await runtime.handlePayload(payload({ qingyun: "active" }));
    await runtime.handlePayload(payload({ qingyun: "headless" }));

    expect(pub.publish).toHaveBeenCalledOnce();
    const [channel, message] = pub.publish.mock.calls[0] as [string, string];
    expect(channel).toBe(AGENT_NARRATE);
    const envelope = JSON.parse(message) as { narrations: Array<{ scope: string; text: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.text).toContain("盟主死在血谷口");
  });

  it("active → decayed 发布势力消亡广播", async () => {
    await runtime.handlePayload(payload({ qingyun: "active" }));
    await runtime.handlePayload(payload({ qingyun: "decayed" }));

    expect(pub.publish).toHaveBeenCalledOnce();
    const [, message] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(message) as { narrations: Array<{ scope: string; text: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.text).toContain("最后一支队伍");
    expect(runtime.stats.published).toBe(1);
  });

  it("初始 headless 不倒推领袖陨落叙事", async () => {
    await runtime.handlePayload(payload({ north: "headless" }));
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.received).toBe(1);
  });

  it("schema 不符时 rejectedContract++ 且不发布", async () => {
    await runtime.handlePayload(JSON.stringify({ v: 1, kind: "named_faction_state" }));
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("非 JSON 载荷 rejectedContract++ 且不发布", async () => {
    await runtime.handlePayload("{not-json");
    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.rejectedContract).toBe(1);
  });

  it("headless → decayed 发布势力消亡广播", async () => {
    await runtime.handlePayload(payload({ north: "headless" }));
    await runtime.handlePayload(payload({ north: "decayed" }));

    expect(pub.publish).toHaveBeenCalledOnce();
    const [, message] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(message) as { narrations: Array<{ scope: string; text: string }> };
    expect(envelope.narrations[0]?.scope).toBe("broadcast");
    expect(envelope.narrations[0]?.text).toContain("北荒漂流者");
    expect(envelope.narrations[0]?.text).toContain("最后一支队伍");
  });

  it("同一快照内多势力转换聚合成一次广播", async () => {
    await runtime.handlePayload(payload({ qingyun: "active", cangyuan: "active" }));
    await runtime.handlePayload(payload({ qingyun: "headless", cangyuan: "decayed" }));

    expect(pub.publish).toHaveBeenCalledOnce();
    const [, message] = pub.publish.mock.calls[0] as [string, string];
    const envelope = JSON.parse(message) as { narrations: Array<{ text: string }> };
    expect(envelope.narrations).toHaveLength(2);
    expect(envelope.narrations.map((entry) => entry.text).join("\n")).toContain("盟主死在血谷口");
    expect(envelope.narrations.map((entry) => entry.text).join("\n")).toContain("沧渊商会");
  });

  it("disconnect 后移除订阅回调，不再消费 Redis message", async () => {
    await runtime.handlePayload(payload({ qingyun: "active" }));
    await runtime.disconnect();

    sub.emit(NAMED_FACTION_STATE, payload({ qingyun: "headless" }));
    await flushMicrotasks();

    expect(pub.publish).not.toHaveBeenCalled();
    expect(runtime.stats.received).toBe(1);
  });
});
