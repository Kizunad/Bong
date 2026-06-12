/**
 * plan-agent-ui-data-v1 P2 — button_click 真注入 TiandaoAgent LLM 推演上下文
 *
 * 验收目标（plan §3 P2 line123/220）：
 *   玩家点击天道面板按钮 → agent 下一轮推演感知该 button_id → emit agent_cmd。
 *
 * 测试策略：
 *   - buttonClickBlock 单元测试：验证渲染逻辑（happy path / 边界 / 空）
 *   - recipe 接线测试：三路 recipe 均含 button_click_signals block
 *   - TiandaoAgent 集成测试：真 TiandaoAgent 实例，setButtonClickEvents → tick → LLM prompt 含 button_id
 *     （不用 ButtonClickAwareAgent mock——"只换 impl 不改测试"原则）
 */

import { describe, expect, it, vi } from "vitest";
import type { AgentUiResponsePayloadV1 } from "@bong/schema";
import {
  CALAMITY_RECIPE,
  ERA_RECIPE,
  MUTATION_RECIPE,
  assembleContext,
  buttonClickBlock,
  createContextInput,
} from "../src/context.js";
import { TiandaoAgent } from "../src/agent.js";
import type { LlmClient } from "../src/llm.js";
import { createTestWorldState } from "./support/fakes.js";

// ─── helpers ───────────────────────────────────────────────────────────────────

function makeClick(buttonId: string, requestId = "req-001"): AgentUiResponsePayloadV1 {
  return {
    request_id: requestId,
    action: "button_click",
    params: { button_id: buttonId },
  };
}

// ─── buttonClickBlock 单元 ──────────────────────────────────────────────────────

describe("buttonClickBlock — 渲染玩家天道面板 button_click 意图信号", () => {
  it("returns empty string when buttonClickEvents is undefined", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {});
    expect(buttonClickBlock.render(input)).toBe("");
  });

  it("returns empty string when buttonClickEvents is empty array", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      buttonClickEvents: [],
    });
    expect(buttonClickBlock.render(input)).toBe("");
  });

  it("renders button_id and request_id for a single click", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      buttonClickEvents: [makeClick("enter_realm", "req-tsy-001")],
    });
    const text = buttonClickBlock.render(input);
    expect(text, "should contain header").toContain("玩家天道面板交互");
    expect(text, "should contain button_id").toContain("button_id=enter_realm");
    expect(text, "should contain request_id").toContain("request_id=req-tsy-001");
  });

  it("renders all button_ids for multiple clicks", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      buttonClickEvents: [
        makeClick("enter_realm", "r1"),
        makeClick("observe_only", "r2"),
        makeClick("dismiss", "r3"),
      ],
    });
    const text = buttonClickBlock.render(input);
    expect(text).toContain("button_id=enter_realm");
    expect(text).toContain("button_id=observe_only");
    expect(text).toContain("button_id=dismiss");
    expect(text).toContain("request_id=r1");
    expect(text).toContain("request_id=r2");
    expect(text).toContain("request_id=r3");
  });

  it("renders (unknown) for click with no button_id param", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      buttonClickEvents: [
        { request_id: "req-empty", action: "button_click", params: {} },
      ],
    });
    const text = buttonClickBlock.render(input);
    expect(text).toContain("(unknown)");
  });

  it("block name is button_click_signals", () => {
    expect(buttonClickBlock.name).toBe("button_click_signals");
  });
});

// ─── recipe 接线 ────────────────────────────────────────────────────────────────

describe("recipe wiring — button_click_signals block 进全三路推演", () => {
  it("CALAMITY_RECIPE includes button_click_signals block", () => {
    expect(
      CALAMITY_RECIPE.blocks.some((b) => b.name === "button_click_signals"),
      "灾劫时代 recipe 须含 button_click_signals block",
    ).toBe(true);
  });

  it("MUTATION_RECIPE includes button_click_signals block", () => {
    expect(
      MUTATION_RECIPE.blocks.some((b) => b.name === "button_click_signals"),
      "变化时代 recipe 须含 button_click_signals block",
    ).toBe(true);
  });

  it("ERA_RECIPE includes button_click_signals block", () => {
    expect(
      ERA_RECIPE.blocks.some((b) => b.name === "button_click_signals"),
      "演绎时代 recipe 须含 button_click_signals block",
    ).toBe(true);
  });

  it("assembleContext for calamity surfaces button_id when click present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "calamity",
      buttonClickEvents: [makeClick("enter_realm")],
    });
    const assembled = assembleContext(CALAMITY_RECIPE, input);
    expect(assembled, "calamity context must include button_id=enter_realm").toContain(
      "button_id=enter_realm",
    );
  });

  it("assembleContext for mutation surfaces button_id when click present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "mutation",
      buttonClickEvents: [makeClick("observe_only")],
    });
    const assembled = assembleContext(MUTATION_RECIPE, input);
    expect(assembled, "mutation context must include button_id=observe_only").toContain(
      "button_id=observe_only",
    );
  });

  it("assembleContext for era surfaces button_id when click present", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "era",
      buttonClickEvents: [makeClick("dismiss")],
    });
    const assembled = assembleContext(ERA_RECIPE, input);
    expect(assembled, "era context must include button_id=dismiss").toContain(
      "button_id=dismiss",
    );
  });

  it("assembleContext omits button_click_signals block entirely when no clicks", () => {
    const input = createContextInput(createTestWorldState(), [], undefined, {
      agentName: "calamity",
      buttonClickEvents: [],
    });
    const assembled = assembleContext(CALAMITY_RECIPE, input);
    expect(assembled).not.toContain("玩家天道面板交互");
  });
});

// ─── TiandaoAgent 集成：真注入 LLM prompt ──────────────────────────────────────

describe("TiandaoAgent.setButtonClickEvents — button_id 真正进入 LLM 推演上下文", () => {
  /**
   * 构造一个 real TiandaoAgent（不用 ButtonClickAwareAgent），
   * spy LLM client 拿到 prompt，断言 button_id 出现在发给 LLM 的 user message 中。
   * 这是 plan §3 P2 line123/220 的最终验收：玩家点击 → agent 感知 → 推演含意图。
   */
  function makeCaptureClient() {
    const capturedMessages: Array<Array<{ role: string; content: string }>> = [];
    const client = {
      async chat(
        _model: string,
        messages: Array<{ role: string; content: string }>,
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

  it("setButtonClickEvents → button_id 出现在 tick() 发给 LLM 的 user message（灾劫时代）", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
      now: () => 30_000,
    });

    // 注入 button_click
    agent.setButtonClickEvents([makeClick("enter_realm", "req-inject-calamity")]);

    const world = createTestWorldState();
    await agent.tick(client, "test-model", world);

    expect(capturedMessages.length, "LLM client must be called once").toBe(1);
    const userMsg = capturedMessages[0]?.find((m) => m.role === "user");
    expect(userMsg, "must have a user message").toBeTruthy();
    expect(
      userMsg?.content,
      "LLM context must contain button_id=enter_realm (真注入验收：玩家意图进推演)",
    ).toContain("button_id=enter_realm");
    expect(
      userMsg?.content,
      "LLM context must contain request_id of the click event",
    ).toContain("req-inject-calamity");
  });

  it("setButtonClickEvents → button_id 出现在 tick() 发给 LLM 的 user message（变化时代）", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = new TiandaoAgent({
      name: "mutation",
      skillFile: "mutation.md",
      recipe: MUTATION_RECIPE,
      intervalMs: 0,
      now: () => 30_000,
    });

    agent.setButtonClickEvents([makeClick("observe_only", "req-inject-mutation")]);

    const world = createTestWorldState();
    await agent.tick(client, "test-model", world);

    const userMsg = capturedMessages[0]?.find((m) => m.role === "user");
    expect(
      userMsg?.content,
      "mutation LLM context must contain button_id=observe_only",
    ).toContain("button_id=observe_only");
  });

  it("setButtonClickEvents → button_id 出现在 tick() 发给 LLM 的 user message（演绎时代）", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = new TiandaoAgent({
      name: "era",
      skillFile: "era.md",
      recipe: ERA_RECIPE,
      intervalMs: 0,
      now: () => 30_000,
    });

    agent.setButtonClickEvents([makeClick("dismiss", "req-inject-era")]);

    const world = createTestWorldState();
    await agent.tick(client, "test-model", world);

    const userMsg = capturedMessages[0]?.find((m) => m.role === "user");
    expect(
      userMsg?.content,
      "era LLM context must contain button_id=dismiss",
    ).toContain("button_id=dismiss");
  });

  it("多个 button_click 事件全部进入 LLM 上下文", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
      now: () => 30_000,
    });

    agent.setButtonClickEvents([
      makeClick("enter_realm", "r1"),
      makeClick("observe_only", "r2"),
    ]);

    const world = createTestWorldState();
    await agent.tick(client, "test-model", world);

    const userMsg = capturedMessages[0]?.find((m) => m.role === "user");
    expect(userMsg?.content).toContain("button_id=enter_realm");
    expect(userMsg?.content).toContain("button_id=observe_only");
    expect(userMsg?.content).toContain("request_id=r1");
    expect(userMsg?.content).toContain("request_id=r2");
  });

  it("无 button_click 时 LLM 上下文不含面板交互块（无噪声）", async () => {
    const { client, capturedMessages } = makeCaptureClient();
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
      now: () => 30_000,
    });

    // 不调 setButtonClickEvents，默认空
    const world = createTestWorldState();
    await agent.tick(client, "test-model", world);

    const userMsg = capturedMessages[0]?.find((m) => m.role === "user");
    expect(userMsg?.content).not.toContain("玩家天道面板交互");
  });

  it("setButtonClickEvents 后再次 tick 依然带上 button_click（状态保持）", async () => {
    const capturedMessages: Array<Array<{ role: string; content: string }>> = [];
    let callCount = 0;
    const client = {
      async chat(
        _model: string,
        messages: Array<{ role: string; content: string }>,
      ): Promise<{ content: string; durationMs: number; requestId: string; model: string }> {
        capturedMessages.push(messages);
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "test" }),
          durationMs: 1,
          requestId: `req-${++callCount}`,
          model: "test-model",
        };
      },
    } as unknown as LlmClient;

    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
      now: () => {
        // 每次返回递增时间，确保 shouldRun 通过
        return callCount * 60_000 + 30_000;
      },
    });

    agent.setButtonClickEvents([makeClick("enter_realm", "persistent-click")]);

    const world = createTestWorldState();
    // 第一次 tick
    await agent.tick(client, "test-model", world);
    const msg1 = capturedMessages[0]?.find((m) => m.role === "user");
    expect(msg1?.content).toContain("button_id=enter_realm");

    // 第二次 tick（latestButtonClickEvents 依然保持，直到 setButtonClickEvents 重置）
    await agent.tick(client, "test-model", world);
    const msg2 = capturedMessages[1]?.find((m) => m.role === "user");
    expect(msg2?.content).toContain("button_id=enter_realm");
  });

  it("空 setButtonClickEvents([]) 覆盖先前的 click（reset 语义）", async () => {
    const capturedMessages: Array<Array<{ role: string; content: string }>> = [];
    let callCount = 0;
    const client = {
      async chat(
        _model: string,
        messages: Array<{ role: string; content: string }>,
      ): Promise<{ content: string; durationMs: number; requestId: string; model: string }> {
        capturedMessages.push(messages);
        return {
          content: JSON.stringify({ commands: [], narrations: [], reasoning: "test" }),
          durationMs: 1,
          requestId: `req-${++callCount}`,
          model: "test-model",
        };
      },
    } as unknown as LlmClient;

    const nowSeq = [30_000, 90_000];
    let idx = 0;
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
      now: () => nowSeq[idx++] ?? 150_000,
    });

    const world = createTestWorldState();

    // tick1: 有 click
    agent.setButtonClickEvents([makeClick("enter_realm", "tick1-click")]);
    await agent.tick(client, "test-model", world);
    const msg1 = capturedMessages[0]?.find((m) => m.role === "user");
    expect(msg1?.content).toContain("button_id=enter_realm");

    // tick2: 清空 click → context 不含面板块
    agent.setButtonClickEvents([]);
    await agent.tick(client, "test-model", world);
    const msg2 = capturedMessages[1]?.find((m) => m.role === "user");
    expect(msg2?.content).not.toContain("玩家天道面板交互");
  });

  it("runtime applyButtonClickEventsToAgents 通过真 TiandaoAgent.setButtonClickEvents 接线（确认非 mock 路径）", () => {
    // 单独断言 TiandaoAgent 有 setButtonClickEvents 方法（非子类、非 mock 覆盖）
    // 这是"只换 impl 不改测试"的锁：真 TiandaoAgent 实现接口后此 test 才绿。
    const agent = new TiandaoAgent({
      name: "calamity",
      skillFile: "calamity.md",
      recipe: CALAMITY_RECIPE,
      intervalMs: 0,
    });
    expect(
      typeof agent.setButtonClickEvents,
      "真 TiandaoAgent 必须实现 setButtonClickEvents（非可选，runtime applyButtonClickEventsToAgents 调此方法）",
    ).toBe("function");

    // 调用不应抛出
    expect(() => agent.setButtonClickEvents([makeClick("test_button")])).not.toThrow();
  });
});
