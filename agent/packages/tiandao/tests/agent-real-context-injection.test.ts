import { describe, expect, it } from "vitest";
import type { AgentUiResponsePayloadV1, ChatSignal, NpcDeathV1 } from "@bong/schema";
import { TiandaoAgent } from "../src/agent.js";
import { CALAMITY_RECIPE, MUTATION_RECIPE } from "../src/context.js";
import type { LlmClient } from "../src/llm.js";
import { createTestWorldState } from "./support/fakes.js";

type CapturedMessage = { role: string; content: string };

function makeCaptureClient() {
  const capturedMessages: CapturedMessage[][] = [];
  const client = {
    async chat(
      _model: string,
      messages: CapturedMessage[],
    ): Promise<{ content: string; durationMs: number; requestId: string; model: string }> {
      capturedMessages.push(messages);
      return {
        content: JSON.stringify({ commands: [], narrations: [], reasoning: "test" }),
        durationMs: 1,
        requestId: "test-req",
        model: "test-model",
      };
    },
  } as unknown as LlmClient;
  return { client, capturedMessages };
}

function capturedUserPrompt(messages: CapturedMessage[][]): string {
  const userMessage = messages[0]?.find((message) => message.role === "user");
  expect(userMessage, "真 TiandaoAgent.tick 必须向 LLM 发送 user message").toBeTruthy();
  return userMessage?.content ?? "";
}

function makeAgent(recipe = CALAMITY_RECIPE): TiandaoAgent {
  return new TiandaoAgent({
    name: recipe.agentName,
    skillFile: recipe.agentName === "mutation" ? "mutation.md" : "calamity.md",
    recipe,
    intervalMs: 0,
    now: () => 30_000,
    tools: [],
  });
}

describe("TiandaoAgent 真 impl 上下文注入守护", () => {
  it("setChatSignals 注入后，玩家聊天内容进入 LLM user message", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = makeAgent();
    const signals: ChatSignal[] = [
      {
        player: "offline:Kiz",
        raw: "灵气太少了，洞府外又抢不到药草",
        sentiment: -0.7,
        intent: "complaint",
        influence_weight: 0.8,
      },
    ];

    agent.setChatSignals(signals);
    await agent.tick(client, "test-model", createTestWorldState());

    const prompt = capturedUserPrompt(capturedMessages);
    expect(prompt, "prompt 必须包含 chat_signals header，而不是只测 fake agent 接线").toContain(
      "## 近期民意",
    );
    expect(prompt).toContain("offline:Kiz");
    expect(prompt).toContain("灵气太少了，洞府外又抢不到药草");
    expect(prompt).toContain("intent=complaint");
  });

  it("setNpcDeathEvents 注入后，离屏战死汇总进入 LLM user message", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = makeAgent(MUTATION_RECIPE);
    const deaths: NpcDeathV1[] = [
      {
        v: 1,
        kind: "npc_death",
        npc_id: "dormant:combat:p3",
        archetype: "rogue",
        cause: "combat",
        faction_id: "attack",
        age_ticks: 10_000,
        max_age_ticks: 200_000,
        at_tick: 84_000,
        from_dormant_combat: true,
        pos: [12, 64, -30],
      },
    ];

    agent.setNpcDeathEvents(deaths);
    await agent.tick(client, "test-model", createTestWorldState());

    const prompt = capturedUserPrompt(capturedMessages);
    expect(prompt, "prompt 必须包含 offscreenWarBlock，而不是只测 DeathAwareFakeAgent").toContain(
      "## 离屏散修消长（远方战事感知）",
    );
    expect(prompt).toContain("近窗共 1 名散修于争脉互殴中横尸");
    expect(prompt).toContain("一方散修势力折损 1 人");
  });

  it("setButtonClickEvents 注入后，button_id 进入 LLM user message", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = makeAgent();
    const clicks: AgentUiResponsePayloadV1[] = [
      {
        request_id: "p3-click",
        action: "button_click",
        params: { button_id: "enter_realm" },
      },
    ];

    agent.setButtonClickEvents(clicks);
    await agent.tick(client, "test-model", createTestWorldState());

    const prompt = capturedUserPrompt(capturedMessages);
    expect(prompt, "prompt 必须包含真实 button_click context").toContain("## 玩家天道面板交互");
    expect(prompt).toContain("request_id=p3-click");
    expect(prompt).toContain("button_id=enter_realm");
  });
});
