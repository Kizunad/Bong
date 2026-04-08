import type { Command, Narration, WorldStateV1 } from "@bong/schema";
import type { LlmClient } from "../../src/llm.js";

export class FakeLlmClient implements LlmClient {
  constructor(private readonly response: string) {}

  async chat(): Promise<string> {
    return this.response;
  }
}

export interface FakeAgentDecision {
  commands: Command[];
  narrations: Narration[];
  reasoning: string;
}

export class FakeAgent {
  constructor(
    public readonly name: string,
    private readonly decision: FakeAgentDecision | null,
  ) {}

  async tick(
    _client: LlmClient,
    _model: string,
    _state: WorldStateV1,
  ): Promise<FakeAgentDecision | null> {
    return this.decision;
  }
}

export function createTestWorldState(): WorldStateV1 {
  return {
    v: 1,
    ts: 1_712_345_678,
    tick: 123,
    players: [
      {
        uuid: "offline:test-player",
        name: "TestPlayer",
        realm: "qi_refining_1",
        composite_power: 0.2,
        breakdown: {
          combat: 0.2,
          wealth: 0.2,
          social: 0.2,
          karma: 0,
          territory: 0.1,
        },
        trend: "stable",
        active_hours: 1,
        zone: "starter_zone",
        pos: [0, 64, 0],
        recent_kills: 0,
        recent_deaths: 0,
      },
    ],
    npcs: [],
    zones: [
      {
        name: "starter_zone",
        spirit_qi: 0.5,
        danger_level: 1,
        active_events: [],
        player_count: 1,
      },
    ],
    recent_events: [],
  };
}
