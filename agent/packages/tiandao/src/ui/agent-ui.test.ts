/**
 * plan-agent-ui-data-v1 P2 — xmlTemplates / uiRenderer / uiResponseConsumer 饱和测试
 *
 * 测试覆盖：
 * 1. xmlEscape: happy path / null-undefined→"" / 所有 5 个转义字符 / 链式转义顺序
 * 2. interpolate: happy path / 必填缺失抛 Error / null 值→"" / 多 key / 嵌套 key 不存在
 * 3. renderTemplate: 各模板类型 / 缺失必填参数抛 Error / 特殊字符转义
 * 4. realmStringToRank: 6 个境界值 / 未知值→0 / null/undefined→0
 * 5. TEMPLATE_REALM_GATE: 四种面板的 realm_gate 值锁定
 * 6. extractButtonIds: happy path / 超过 16 个截断 / 无 button / 单引号属性
 * 7. UiRenderer: happy path / blur 降级 / realm 足够时清晰版 / 失败抛错
 * 8. UiResponseConsumer: connect/disconnect 生命周期 /
 *    button_click→onButtonClick / dismissed→onSessionEnd / timeout→onSessionEnd /
 *    replaced→静默 / parse_error→warn / realm_gate_rejected→narration /
 *    player_offline→warn / invalid_button_id→warn / 无效 JSON / schema 不符 /
 *    stats 计数器全覆盖
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";
import type { AgentUiResponsePayloadV1 } from "@bong/schema";

import {
  xmlEscape,
  interpolate,
  renderTemplate,
  realmStringToRank,
  TEMPLATE_REALM_GATE,
  extractButtonIds,
} from "./xmlTemplates.js";
import { UiRenderer } from "./uiRenderer.js";
import { UiResponseConsumer, REALM_GATE_NARRATION_TEXT } from "./uiResponseConsumer.js";

const { AGENT_UI_RESPONSE, AGENT_NARRATE, AGENT_UI_CMD } = CHANNELS;

// ─── mock helpers ─────────────────────────────────────────────────────────────

function makeMockClient() {
  const listeners: Map<string, ((ch: string, msg: string) => void)[]> = new Map();

  return {
    subscribe: vi.fn(async (_channel: string) => {}),
    on: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      arr.push(listener);
      listeners.set(event, arr);
    }),
    off: vi.fn((event: string, listener: (ch: string, msg: string) => void) => {
      const arr = listeners.get(event) ?? [];
      const idx = arr.indexOf(listener);
      if (idx !== -1) arr.splice(idx, 1);
      listeners.set(event, arr);
    }),
    unsubscribe: vi.fn(async () => {}),
    disconnect: vi.fn(() => {}),
    publish: vi.fn(async (_channel: string, _message: string) => 1),

    emit(channel: string, message: string) {
      (listeners.get("message") ?? []).forEach((l) => l(channel, message));
    },
  };
}

function makeResponsePayload(
  action: AgentUiResponsePayloadV1["action"],
  params: Record<string, string> = {},
): AgentUiResponsePayloadV1 {
  return {
    request_id: "test-req-001",
    action,
    params,
  };
}

// ─── 1. xmlEscape ─────────────────────────────────────────────────────────────

describe("xmlEscape", () => {
  it("passes through plain text untouched", () => {
    expect(xmlEscape("hello world")).toBe("hello world");
  });

  it("escapes & first to avoid double-escaping", () => {
    expect(xmlEscape("a & b")).toBe("a &amp; b");
  });

  it("escapes < and >", () => {
    expect(xmlEscape("<tag>")).toBe("&lt;tag&gt;");
  });

  it("escapes double quote", () => {
    expect(xmlEscape('"quoted"')).toBe("&quot;quoted&quot;");
  });

  it("escapes single quote", () => {
    expect(xmlEscape("it's")).toBe("it&apos;s");
  });

  it("escapes all 5 characters together", () => {
    const input = `< > " ' &`;
    const expected = "&lt; &gt; &quot; &apos; &amp;";
    expect(xmlEscape(input)).toBe(expected);
  });

  it("returns empty string for null", () => {
    expect(xmlEscape(null)).toBe("");
  });

  it("returns empty string for undefined", () => {
    expect(xmlEscape(undefined)).toBe("");
  });

  it("converts numbers to string", () => {
    expect(xmlEscape(42)).toBe("42");
  });

  it("avoids double-escaping: & → &amp; not &amp;amp;", () => {
    // If & is not escaped first, "a & b" → "a &amp; b" → "a &amp;amp; b" (wrong)
    const result = xmlEscape("a & b");
    expect(result).toBe("a &amp; b");
    // Verify it does NOT contain &amp;amp;
    expect(result).not.toContain("&amp;amp;");
  });
});

// ─── 2. interpolate ───────────────────────────────────────────────────────────

describe("interpolate", () => {
  it("replaces a single placeholder", () => {
    const result = interpolate("hello {{name}}", { name: "world" });
    expect(result).toBe("hello world");
  });

  it("replaces multiple placeholders", () => {
    const result = interpolate("{{a}} + {{b}} = {{c}}", { a: "1", b: "2", c: "3" });
    expect(result).toBe("1 + 2 = 3");
  });

  it("XML-escapes values before insertion", () => {
    const result = interpolate("{{text}}", { text: "<script>alert('xss')</script>" });
    expect(result).not.toContain("<script>");
    expect(result).toContain("&lt;script&gt;");
  });

  it("throws Error for missing required param", () => {
    expect(() => interpolate("hello {{name}}", {})).toThrow(
      /必填参数缺失.*name/,
    );
  });

  it("converts null value to empty string (no error)", () => {
    const result = interpolate("{{val}}", { val: null });
    expect(result).toBe("");
  });

  it("converts undefined value to empty string (no error)", () => {
    const result = interpolate("{{val}}", { val: undefined });
    expect(result).toBe("");
  });

  it("handles template with no placeholders", () => {
    const result = interpolate("no placeholders here", {});
    expect(result).toBe("no placeholders here");
  });

  it("replaces same placeholder multiple times", () => {
    const result = interpolate("{{x}} and {{x}}", { x: "y" });
    expect(result).toBe("y and y");
  });
});

// ─── 3. renderTemplate ────────────────────────────────────────────────────────

describe("renderTemplate", () => {
  it("renders tsy_discovery template with required params", () => {
    const xml = renderTemplate({
      kind: "tsy_discovery",
      params: { zone_name: "活坍缩渊", spirit_qi_display: "0.85" },
    });
    expect(xml).toContain("活坍缩渊");
    expect(xml).toContain("0.85");
    expect(xml).toContain("enter_realm");
    expect(xml).toContain("dismiss");
    expect(xml).toContain("<owo-ui>");
    expect(xml).toContain("</owo-ui>");
  });

  it("renders elder_legacy template with required params", () => {
    const xml = renderTemplate({
      kind: "elder_legacy",
      params: { elder_name: "张三大能", qi_cost_display: "500" },
    });
    expect(xml).toContain("张三大能");
    expect(xml).toContain("500");
    expect(xml).toContain("accept_legacy");
    expect(xml).toContain("refuse_legacy");
  });

  it("renders tiandao_revelation template with required params", () => {
    const xml = renderTemplate({
      kind: "tiandao_revelation",
      params: { revelation_text: "天道示机，气运将至" },
    });
    expect(xml).toContain("天道示机");
    expect(xml).toContain("气运将至");
    expect(xml).toContain("acknowledge");
  });

  it("renders blur_insufficient template with required params", () => {
    const xml = renderTemplate({
      kind: "blur_insufficient",
      params: { blur_hint: "境界不足以感知" },
    });
    expect(xml).toContain("境界不足以感知");
    expect(xml).toContain("dismiss");
  });

  it("XML-escapes dangerous characters in params", () => {
    const xml = renderTemplate({
      kind: "tiandao_revelation",
      params: { revelation_text: '<script>alert("xss")</script>' },
    });
    expect(xml).not.toContain("<script>");
    expect(xml).toContain("&lt;script&gt;");
  });

  it("throws for missing required param in tsy_discovery", () => {
    expect(() =>
      renderTemplate({
        kind: "tsy_discovery",
        params: { zone_name: "test" }, // 缺少 spirit_qi_display
      }),
    ).toThrow(/必填参数缺失.*spirit_qi_display/);
  });
});

// ─── 4. realmStringToRank ─────────────────────────────────────────────────────

describe("realmStringToRank", () => {
  const CASES: [string, number][] = [
    ["Awaken",   1],
    ["Induce",   2],
    ["Condense", 3],
    ["Solidify", 4],
    ["Spirit",   5],
    ["Void",     6],
  ];

  for (const [realm, expectedRank] of CASES) {
    it(`${realm} → ${expectedRank}`, () => {
      expect(realmStringToRank(realm)).toBe(expectedRank);
    });
  }

  it("unknown string → 0", () => {
    expect(realmStringToRank("Unknown")).toBe(0);
  });

  it("empty string → 0", () => {
    expect(realmStringToRank("")).toBe(0);
  });

  it("null → 0", () => {
    expect(realmStringToRank(null)).toBe(0);
  });

  it("undefined → 0", () => {
    expect(realmStringToRank(undefined)).toBe(0);
  });

  it("matches server Realm::rank() ordering: Awaken(1) < Spirit(5) < Void(6)", () => {
    expect(realmStringToRank("Awaken")).toBeLessThan(realmStringToRank("Spirit"));
    expect(realmStringToRank("Spirit")).toBeLessThan(realmStringToRank("Void"));
  });
});

// ─── 5. TEMPLATE_REALM_GATE ───────────────────────────────────────────────────

describe("TEMPLATE_REALM_GATE", () => {
  it("tsy_discovery realm_gate = 3（凝脉境+）", () => {
    expect(TEMPLATE_REALM_GATE.tsy_discovery).toBe(3);
  });

  it("tiandao_revelation realm_gate = 5（通灵境+）", () => {
    expect(TEMPLATE_REALM_GATE.tiandao_revelation).toBe(5);
  });

  it("elder_legacy realm_gate = 0（不门控）", () => {
    expect(TEMPLATE_REALM_GATE.elder_legacy).toBe(0);
  });

  it("blur_insufficient realm_gate = 0（内容已模糊）", () => {
    expect(TEMPLATE_REALM_GATE.blur_insufficient).toBe(0);
  });
});

// ─── 6. extractButtonIds ──────────────────────────────────────────────────────

describe("extractButtonIds", () => {
  it("extracts button ids from double-quote attributes", () => {
    const xml = `<button id="enter_realm">踏入</button><button id="dismiss">离开</button>`;
    expect(extractButtonIds(xml)).toEqual(["enter_realm", "dismiss"]);
  });

  it("extracts button ids from single-quote attributes", () => {
    const xml = `<button id='btn1'>A</button><button id='btn2'>B</button>`;
    expect(extractButtonIds(xml)).toEqual(["btn1", "btn2"]);
  });

  it("returns empty array when no buttons", () => {
    expect(extractButtonIds("<label>text</label>")).toEqual([]);
  });

  it("truncates at 16 buttons", () => {
    const buttons = Array.from({ length: 20 }, (_, i) => `<button id="btn${i}">X</button>`).join("");
    const ids = extractButtonIds(buttons);
    expect(ids).toHaveLength(16);
  });

  it("handles buttons with additional attributes", () => {
    const xml = `<button id="ok" style="color:red">OK</button>`;
    expect(extractButtonIds(xml)).toEqual(["ok"]);
  });
});

// ─── 7. UiRenderer ────────────────────────────────────────────────────────────

describe("UiRenderer", () => {
  let pub: ReturnType<typeof makeMockClient>;
  let renderer: UiRenderer;
  const fixedRequestId = "fixed-request-id-001";

  beforeEach(() => {
    pub = makeMockClient();
    renderer = new UiRenderer({
      pub,
      generateRequestId: () => fixedRequestId,
      now: () => 1_000_000,
    });
  });

  const basePlayer = {
    uuid: "player-uuid-001",
    name: "TestPlayer",
    realm: "Condense", // 凝脉境 rank=3
    composite_power: 0.5,
    breakdown: { combat: 0.5, wealth: 0.5, social: 0.5, karma: 0.0, territory: 0.5 },
    trend: "stable" as const,
    active_hours: 10,
    zone: "test_zone",
    pos: [0, 64, 0] as [number, number, number],
    recent_kills: 0,
    recent_deaths: 0,
  };

  it("publishes AgentUiRequestCommandV1 to AGENT_UI_CMD for tsy_discovery (realm sufficient)", async () => {
    const result = await renderer.renderUi({
      scenario: "tsy_discovery",
      targetPlayer: { ...basePlayer, realm: "Condense" }, // rank=3 == realm_gate=3
      params: { zone_name: "活坍缩渊", spirit_qi_display: "0.85" },
    });

    expect(result.requestId).toBe(fixedRequestId);
    expect(result.sentBlurVersion).toBe(false);
    expect(pub.publish).toHaveBeenCalledOnce();

    const [channel, raw] = pub.publish.mock.calls[0];
    expect(channel).toBe(AGENT_UI_CMD);

    const cmd = JSON.parse(raw as string);
    expect(cmd.request_id).toBe(fixedRequestId);
    expect(cmd.target_player).toBe("player-uuid-001");
    expect(cmd.realm_gate).toBe(3);
    expect(cmd.xml).toContain("活坍缩渊");
    expect(cmd.xml).toContain("<owo-ui>");
    expect(cmd.timeout_ticks).toBe(600);
    expect(cmd.allowed_button_ids).toContain("enter_realm");
    expect(cmd.allowed_button_ids).toContain("dismiss");
    // realm_gate / allowed_button_ids 仅在 command 中（server 端安全字段）
  });

  it("sends blur version when player realm < realm_gate (Awaken vs tsy_discovery gate=3)", async () => {
    const result = await renderer.renderUi({
      scenario: "tsy_discovery",
      targetPlayer: { ...basePlayer, realm: "Awaken" }, // rank=1 < 3
      params: { zone_name: "活坍缩渊", spirit_qi_display: "0.85" },
    });

    expect(result.sentBlurVersion).toBe(true);
    expect(pub.publish).toHaveBeenCalledOnce();

    const [channel, raw] = pub.publish.mock.calls[0];
    expect(channel).toBe(AGENT_UI_CMD);

    const cmd = JSON.parse(raw as string);
    // 模糊版 realm_gate=0（不门控，内容已模糊）
    expect(cmd.realm_gate).toBe(0);
    expect(cmd.xml).toContain("有什么正在靠近");
    expect(cmd.allowed_button_ids).toEqual(["dismiss"]);
  });

  it("sends blur version for tiandao_revelation with realm < 5 (Spirit=5 required)", async () => {
    const result = await renderer.renderUi({
      scenario: "tiandao_revelation",
      targetPlayer: { ...basePlayer, realm: "Solidify" }, // rank=4 < 5
      params: { revelation_text: "天机" },
    });
    expect(result.sentBlurVersion).toBe(true);
  });

  it("uses clear version for tiandao_revelation with Spirit realm (rank=5)", async () => {
    const result = await renderer.renderUi({
      scenario: "tiandao_revelation",
      targetPlayer: { ...basePlayer, realm: "Spirit" }, // rank=5 == realm_gate=5
      params: { revelation_text: "气运将至" },
    });
    expect(result.sentBlurVersion).toBe(false);
    const cmd = JSON.parse((pub.publish.mock.calls[0][1]) as string);
    expect(cmd.realm_gate).toBe(5);
    expect(cmd.xml).toContain("气运将至");
  });

  it("applies custom timeoutTicks", async () => {
    await renderer.renderUi({
      scenario: "elder_legacy",
      targetPlayer: { ...basePlayer, realm: "Awaken" },
      params: { elder_name: "老者", qi_cost_display: "100" },
      timeoutTicks: 1200,
    });
    const cmd = JSON.parse((pub.publish.mock.calls[0][1]) as string);
    expect(cmd.timeout_ticks).toBe(1200);
  });

  it("re-throws if publish fails", async () => {
    pub.publish.mockRejectedValue(new Error("redis down"));
    await expect(
      renderer.renderUi({
        scenario: "elder_legacy",
        targetPlayer: basePlayer,
        params: { elder_name: "老者", qi_cost_display: "100" },
      }),
    ).rejects.toThrow("redis down");
  });
});

// ─── 8. UiResponseConsumer ────────────────────────────────────────────────────

describe("UiResponseConsumer", () => {
  let sub: ReturnType<typeof makeMockClient>;
  let pub: ReturnType<typeof makeMockClient>;
  let onButtonClick: ReturnType<typeof vi.fn>;
  let onSessionEnd: ReturnType<typeof vi.fn>;
  let consumer: UiResponseConsumer;
  let warnSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    sub = makeMockClient();
    pub = makeMockClient();
    onButtonClick = vi.fn();
    onSessionEnd = vi.fn();
    warnSpy = vi.fn();

    consumer = new UiResponseConsumer({
      sub,
      pub,
      onButtonClick,
      onSessionEnd,
      logger: { info: vi.fn(), warn: warnSpy },
    });
  });

  // ── 生命周期 ──────────────────────────────────────────────────────────────

  describe("lifecycle", () => {
    it("subscribes to AGENT_UI_RESPONSE on connect", async () => {
      await consumer.connect();
      expect(sub.subscribe).toHaveBeenCalledWith(AGENT_UI_RESPONSE);
      expect(sub.on).toHaveBeenCalledWith("message", expect.any(Function));
    });

    it("idempotent connect: second call does not subscribe again", async () => {
      await consumer.connect();
      await consumer.connect();
      expect(sub.subscribe).toHaveBeenCalledTimes(1);
    });

    it("disconnects cleanly", async () => {
      await consumer.connect();
      await consumer.disconnect();
      expect(sub.unsubscribe).toHaveBeenCalled();
      expect(sub.disconnect).toHaveBeenCalled();
      expect(pub.disconnect).toHaveBeenCalled();
    });
  });

  // ── 初始 stats ────────────────────────────────────────────────────────────

  it("starts with all stats at 0", () => {
    expect(consumer.stats).toMatchObject({
      received: 0,
      buttonClick: 0,
      dismissed: 0,
      timeout: 0,
      replaced: 0,
      realmGateRejected: 0,
      parseError: 0,
      otherError: 0,
      rejectedContract: 0,
      narrationPublished: 0,
    });
  });

  // ── 辅助函数：发送消息 ─────────────────────────────────────────────────────

  async function sendMessage(payload: unknown): Promise<void> {
    await consumer.connect();
    sub.emit(AGENT_UI_RESPONSE, JSON.stringify(payload));
    // handlePayload is async via void; give microtasks a chance to run
    await Promise.resolve();
    await Promise.resolve();
  }

  // ── button_click ──────────────────────────────────────────────────────────

  it("button_click → calls onButtonClick and increments stats.buttonClick", async () => {
    await sendMessage(makeResponsePayload("button_click", { button_id: "enter_realm" }));
    expect(onButtonClick).toHaveBeenCalledOnce();
    const arg = onButtonClick.mock.calls[0][0] as AgentUiResponsePayloadV1;
    expect(arg.action).toBe("button_click");
    expect(arg.params["button_id"]).toBe("enter_realm");
    expect(consumer.stats.buttonClick).toBe(1);
    expect(consumer.stats.received).toBe(1);
  });

  // ── dismissed ─────────────────────────────────────────────────────────────

  it("dismissed → calls onSessionEnd and increments stats.dismissed", async () => {
    await sendMessage(makeResponsePayload("dismissed"));
    expect(onSessionEnd).toHaveBeenCalledOnce();
    expect(consumer.stats.dismissed).toBe(1);
    expect(consumer.stats.received).toBe(1);
  });

  // ── timeout ───────────────────────────────────────────────────────────────

  it("timeout → calls onSessionEnd and increments stats.timeout", async () => {
    await sendMessage(makeResponsePayload("timeout"));
    expect(onSessionEnd).toHaveBeenCalledOnce();
    expect(consumer.stats.timeout).toBe(1);
  });

  // ── replaced ──────────────────────────────────────────────────────────────

  it("replaced → silent (no callback), increments stats.replaced", async () => {
    await sendMessage(makeResponsePayload("replaced"));
    expect(onButtonClick).not.toHaveBeenCalled();
    expect(onSessionEnd).not.toHaveBeenCalled();
    expect(pub.publish).not.toHaveBeenCalled();
    expect(consumer.stats.replaced).toBe(1);
  });

  // ── parse_error ───────────────────────────────────────────────────────────

  it("parse_error → warns and increments stats.parseError", async () => {
    await sendMessage(makeResponsePayload("parse_error"));
    expect(warnSpy).toHaveBeenCalled();
    expect(consumer.stats.parseError).toBe(1);
  });

  // ── error: realm_gate_rejected ────────────────────────────────────────────

  it("error+realm_gate_rejected → emits narration to AGENT_NARRATE", async () => {
    await sendMessage(
      makeResponsePayload("error", {
        reason: "realm_gate_rejected",
        player_realm: "2",
        required_realm: "3",
      }),
    );
    expect(pub.publish).toHaveBeenCalledOnce();
    const [channel, raw] = pub.publish.mock.calls[0];
    expect(channel).toBe(AGENT_NARRATE);
    const narrationMsg = JSON.parse(raw as string);
    expect(narrationMsg.v).toBe(1);
    expect(narrationMsg.narrations).toHaveLength(1);
    expect(narrationMsg.narrations[0].text).toBe(REALM_GATE_NARRATION_TEXT);
    expect(narrationMsg.narrations[0].style).toBe("system_warning");
    expect(consumer.stats.realmGateRejected).toBe(1);
    expect(consumer.stats.narrationPublished).toBe(1);
  });

  // ── error: player_offline ─────────────────────────────────────────────────

  it("error+player_offline → warns and increments stats.otherError", async () => {
    await sendMessage(makeResponsePayload("error", { reason: "player_offline" }));
    expect(warnSpy).toHaveBeenCalled();
    expect(pub.publish).not.toHaveBeenCalled();
    expect(consumer.stats.otherError).toBe(1);
  });

  // ── error: invalid_button_id ─────────────────────────────────────────────

  it("error+invalid_button_id → warns and increments stats.otherError", async () => {
    await sendMessage(makeResponsePayload("error", { reason: "invalid_button_id" }));
    expect(warnSpy).toHaveBeenCalled();
    expect(pub.publish).not.toHaveBeenCalled();
    expect(consumer.stats.otherError).toBe(1);
  });

  // ── error: unknown reason ─────────────────────────────────────────────────

  it("error+unknown_reason → warns and increments stats.otherError", async () => {
    await sendMessage(makeResponsePayload("error", { reason: "some_future_reason" }));
    expect(warnSpy).toHaveBeenCalled();
    expect(consumer.stats.otherError).toBe(1);
  });

  // ── 无效 JSON ─────────────────────────────────────────────────────────────

  it("non-JSON message → warns and increments stats.rejectedContract", async () => {
    await consumer.connect();
    sub.emit(AGENT_UI_RESPONSE, "this is not json");
    await Promise.resolve();
    await Promise.resolve();
    expect(warnSpy).toHaveBeenCalled();
    expect(consumer.stats.rejectedContract).toBe(1);
    expect(consumer.stats.received).toBe(0);
  });

  // ── schema 不符 ───────────────────────────────────────────────────────────

  it("schema-invalid message → warns and increments stats.rejectedContract", async () => {
    await consumer.connect();
    sub.emit(AGENT_UI_RESPONSE, JSON.stringify({ request_id: "x", action: "unknown_action", params: {} }));
    await Promise.resolve();
    await Promise.resolve();
    expect(warnSpy).toHaveBeenCalled();
    expect(consumer.stats.rejectedContract).toBe(1);
  });

  // ── 消息来自其他 channel 时忽略 ───────────────────────────────────────────

  it("ignores messages from other channels", async () => {
    await consumer.connect();
    sub.emit("bong:other_channel", JSON.stringify(makeResponsePayload("button_click")));
    await Promise.resolve();
    await Promise.resolve();
    expect(onButtonClick).not.toHaveBeenCalled();
    expect(consumer.stats.received).toBe(0);
  });

  // ── realm_gate_rejected: publish 失败时记录 warn ─────────────────────────

  it("realm_gate_rejected: publish failure → warns, does not throw", async () => {
    pub.publish.mockRejectedValue(new Error("redis down"));
    await sendMessage(
      makeResponsePayload("error", { reason: "realm_gate_rejected" }),
    );
    expect(warnSpy).toHaveBeenCalled();
    expect(consumer.stats.realmGateRejected).toBe(1);
    // narrationPublished should NOT be incremented when publish fails
    expect(consumer.stats.narrationPublished).toBe(0);
  });

  // ── 多轮 stats 累加 ───────────────────────────────────────────────────────

  it("stats accumulate correctly across multiple messages", async () => {
    await consumer.connect();
    const send = (p: unknown) => {
      sub.emit(AGENT_UI_RESPONSE, JSON.stringify(p));
    };

    send(makeResponsePayload("button_click", { button_id: "a" }));
    send(makeResponsePayload("button_click", { button_id: "b" }));
    send(makeResponsePayload("dismissed"));
    send(makeResponsePayload("timeout"));
    send(makeResponsePayload("replaced"));
    // Give all async handlers time to complete
    await new Promise((r) => setTimeout(r, 10));

    expect(consumer.stats.buttonClick).toBe(2);
    expect(consumer.stats.dismissed).toBe(1);
    expect(consumer.stats.timeout).toBe(1);
    expect(consumer.stats.replaced).toBe(1);
    expect(consumer.stats.received).toBe(5);
  });
});
