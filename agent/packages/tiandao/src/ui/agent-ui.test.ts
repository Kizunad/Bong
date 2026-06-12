/**
 * plan-agent-ui-data-v1 P2/P3 — xmlTemplates / uiRenderer / uiResponseConsumer / agentUiRuntime 饱和测试
 *
 * 测试覆盖：
 * 1. xmlEscape: happy path / null-undefined→"" / 所有 5 个转义字符 / 链式转义顺序
 * 2. interpolate: happy path / 必填缺失抛 Error / null 值→"" / 多 key / 嵌套 key 不存在
 * 3. renderTemplate: P3 §4 三标准模板（TSY/elder/tiandao）/ question_0 可选按钮省略 / 缺失必填参数抛 Error / 特殊字符转义
 * 4. realmStringToRank: 6 个境界值 / 未知值→0 / null/undefined→0
 * 5. TEMPLATE_REALM_GATE: 四种面板的 realm_gate 值锁定（elder_legacy=3，P3 §4）
 * 6. extractButtonIds: happy path / 超过 16 个截断 / 无 button / 单引号属性
 * 7. UiRenderer: happy path / blur 降级 / realm 足够时清晰版 / 失败抛错 / P3 elder_legacy realm_gate=3
 * 8. UiResponseConsumer: connect/disconnect 生命周期 /
 *    button_click→onButtonClick / dismissed→onSessionEnd / timeout→onSessionEnd /
 *    replaced→静默 / parse_error→warn / realm_gate_rejected→narration /
 *    player_offline→warn / invalid_button_id→warn / 无效 JSON / schema 不符 /
 *    stats 计数器全覆盖
 * 9. AgentUiRuntime e2e（P2 验收）：
 *    mock 推演:生成活坍缩渊面板→emit AgentUiRequestCommandV1→点进入→收 button_click→
 *    drainPendingButtonClicks 注入推演；realm_gate_rejected→emit narration；
 *    session_end(dismissed/timeout)→drainPendingSessionEnds
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { CHANNELS } from "@bong/schema";
import type { AgentUiResponsePayloadV1 } from "@bong/schema";

import {
  xmlEscape,
  interpolate,
  renderTemplate,
  renderElderLegacyTemplate,
  realmStringToRank,
  TEMPLATE_REALM_GATE,
  extractButtonIds,
} from "./xmlTemplates.js";
import { UiRenderer } from "./uiRenderer.js";
import { UiResponseConsumer, REALM_GATE_NARRATION_TEXT } from "./uiResponseConsumer.js";
import { AgentUiRuntime } from "./agentUiRuntime.js";

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
  // P3 §4 TSY 秘境发现：必填 zone_name / spirit_qi_display / danger_tier / agent_narrative
  it("renders tsy_discovery template with all required params (P3 §4)", () => {
    const xml = renderTemplate({
      kind: "tsy_discovery",
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.85",
        danger_tier: "极危",
        agent_narrative: "天道感知到此处有一道裂缝正在收拢",
      },
    });
    expect(xml).toContain("活坍缩渊");
    expect(xml).toContain("0.85");
    expect(xml).toContain("极危");
    expect(xml).toContain("天道感知到此处有一道裂缝正在收拢");
    expect(xml).toContain('id="enter_realm"');
    expect(xml).toContain('id="observe_only"');
    expect(xml).toContain('id="dismiss"');
    expect(xml).toContain("<owo-ui>");
    expect(xml).toContain("</owo-ui>");
    expect(xml).toContain("【活坍缩渊】");
  });

  // P3 §4 垂死大能传承：必填 elder_title / elder_narration / qi_cost；question_0 可选
  it("renders elder_legacy template with all required params, question_0 present (P3 §4)", () => {
    const xml = renderTemplate({
      kind: "elder_legacy",
      params: {
        elder_title: "张三大能",
        elder_narration: "我的传承等待了数百年……",
        qi_cost: "500",
        question_0: "你是谁？",
      },
    });
    expect(xml).toContain("张三大能");
    expect(xml).toContain("我的传承等待了数百年……");
    expect(xml).toContain("500");
    expect(xml).toContain("你是谁？");
    expect(xml).toContain('id="accept_legacy"');
    expect(xml).toContain('id="ask_question_0"');
    expect(xml).toContain('id="dismiss"');
    expect(xml).toContain("天道见证");
    expect(xml).toContain("<owo-ui>");
  });

  it("renders elder_legacy template with question_0 absent → ask_question_0 button omitted (P3 §4)", () => {
    const xml = renderTemplate({
      kind: "elder_legacy",
      params: {
        elder_title: "李四大能",
        elder_narration: "时间不多了……",
        qi_cost: "200",
        // question_0 intentionally absent
      },
    });
    expect(xml).toContain('id="accept_legacy"');
    expect(xml).toContain('id="dismiss"');
    expect(xml).not.toContain('id="ask_question_0"');
    expect(xml).not.toContain("{{question_0}}");
    expect(xml).not.toContain("{{OPTIONAL_QUESTION_BUTTON}}");
  });

  it("renders elder_legacy template with question_0=null → ask_question_0 button omitted", () => {
    const xml = renderTemplate({
      kind: "elder_legacy",
      params: {
        elder_title: "王五大能",
        elder_narration: "传承在此",
        qi_cost: "300",
        question_0: null,
      },
    });
    expect(xml).not.toContain('id="ask_question_0"');
  });

  // P3 §4 天道启示：必填 tiandao_message；realm_gate=5；only dismiss button
  it("renders tiandao_revelation template with tiandao_message (P3 §4)", () => {
    const xml = renderTemplate({
      kind: "tiandao_revelation",
      params: { tiandao_message: "气运将至，天机不可泄露" },
    });
    expect(xml).toContain("气运将至，天机不可泄露");
    expect(xml).toContain('id="dismiss"');
    // P3 spec: only dismiss button (no acknowledge)
    expect(xml).not.toContain('id="acknowledge"');
    expect(xml).toContain("天意");
    expect(xml).toContain("天道不等你");
    expect(xml).toContain("<owo-ui>");
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
      params: { tiandao_message: '<script>alert("xss")</script>' },
    });
    expect(xml).not.toContain("<script>");
    expect(xml).toContain("&lt;script&gt;");
  });

  it("XML-escapes dangerous characters in elder_legacy params", () => {
    const xml = renderTemplate({
      kind: "elder_legacy",
      params: {
        elder_title: "<evil>",
        elder_narration: 'say "hello" & goodbye',
        qi_cost: "100",
      },
    });
    expect(xml).not.toContain("<evil>");
    expect(xml).toContain("&lt;evil&gt;");
    expect(xml).toContain("&quot;hello&quot;");
    expect(xml).toContain("&amp;");
  });

  it("throws for missing required param in tsy_discovery (zone_name)", () => {
    expect(() =>
      renderTemplate({
        kind: "tsy_discovery",
        params: {
          spirit_qi_display: "0.85",
          danger_tier: "中危",
          agent_narrative: "裂缝正在收拢",
          // zone_name absent
        },
      }),
    ).toThrow(/必填参数缺失.*zone_name/);
  });

  it("throws for missing required param in tsy_discovery (danger_tier)", () => {
    expect(() =>
      renderTemplate({
        kind: "tsy_discovery",
        params: {
          zone_name: "test",
          spirit_qi_display: "0.85",
          agent_narrative: "裂缝正在收拢",
          // danger_tier absent
        },
      }),
    ).toThrow(/必填参数缺失.*danger_tier/);
  });

  it("throws for missing required param in elder_legacy (elder_title)", () => {
    expect(() =>
      renderTemplate({
        kind: "elder_legacy",
        params: { elder_narration: "传承", qi_cost: "100" },
      }),
    ).toThrow(/必填参数缺失.*elder_title/);
  });

  it("throws for missing required param in tiandao_revelation (tiandao_message)", () => {
    expect(() =>
      renderTemplate({
        kind: "tiandao_revelation",
        params: {},
      }),
    ).toThrow(/必填参数缺失.*tiandao_message/);
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

  it("elder_legacy realm_gate = 3（凝脉境+，plan §4）", () => {
    expect(TEMPLATE_REALM_GATE.elder_legacy).toBe(3);
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
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.85",
        danger_tier: "极危",
        agent_narrative: "天道感知到此处有一道裂缝正在收拢",
      },
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
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.85",
        danger_tier: "极危",
        agent_narrative: "裂缝收拢",
      },
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
      params: { tiandao_message: "天机" },
    });
    expect(result.sentBlurVersion).toBe(true);
  });

  it("uses clear version for tiandao_revelation with Spirit realm (rank=5)", async () => {
    const result = await renderer.renderUi({
      scenario: "tiandao_revelation",
      targetPlayer: { ...basePlayer, realm: "Spirit" }, // rank=5 == realm_gate=5
      params: { tiandao_message: "气运将至" },
    });
    expect(result.sentBlurVersion).toBe(false);
    const cmd = JSON.parse((pub.publish.mock.calls[0][1]) as string);
    expect(cmd.realm_gate).toBe(5);
    expect(cmd.xml).toContain("气运将至");
    // P3: tiandao_revelation only has dismiss button
    expect(cmd.allowed_button_ids).toEqual(["dismiss"]);
  });

  it("applies custom timeoutTicks for elder_legacy (P3 §4 params)", async () => {
    await renderer.renderUi({
      scenario: "elder_legacy",
      targetPlayer: { ...basePlayer, realm: "Condense" }, // realm_gate=3, rank=3 sufficient
      params: { elder_title: "老者", elder_narration: "传承在此", qi_cost: "100" },
      timeoutTicks: 1200,
    });
    const cmd = JSON.parse((pub.publish.mock.calls[0][1]) as string);
    expect(cmd.timeout_ticks).toBe(1200);
    // elder_legacy realm_gate=3
    expect(cmd.realm_gate).toBe(3);
    expect(cmd.allowed_button_ids).toContain("accept_legacy");
    expect(cmd.allowed_button_ids).toContain("dismiss");
  });

  it("elder_legacy sends blur when player realm < 3 (凝脉境)", async () => {
    // elder_legacy realm_gate=3 (plan §4), Awaken rank=1 < 3
    const result = await renderer.renderUi({
      scenario: "elder_legacy",
      targetPlayer: { ...basePlayer, realm: "Awaken" }, // rank=1 < realm_gate=3
      params: { elder_title: "老者", elder_narration: "传承在此", qi_cost: "100" },
    });
    expect(result.sentBlurVersion).toBe(true);
    const cmd = JSON.parse((pub.publish.mock.calls[0][1]) as string);
    expect(cmd.realm_gate).toBe(0); // blur version has no gate
    expect(cmd.xml).not.toContain("accept_legacy");
  });

  it("re-throws if publish fails", async () => {
    pub.publish.mockRejectedValue(new Error("redis down"));
    await expect(
      renderer.renderUi({
        scenario: "elder_legacy",
        targetPlayer: { ...basePlayer, realm: "Condense" },
        params: { elder_title: "老者", elder_narration: "传承", qi_cost: "100" },
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

// ─── 9. AgentUiRuntime e2e（P2 验收）────────────────────────────────────────
//
// plan §3 P2 验收：
//   mock 推演:生成活坍缩渊面板→emit AgentUiRequestCommandV1 到 bong:agent_ui_cmd
//            →玩家点进入→收 button_click→drainPendingButtonClicks 注入推演
//   realm_gate_rejected→emit narration 到 bong:agent_narrate
//   session_end(dismissed/timeout)→drainPendingSessionEnds

describe("AgentUiRuntime e2e (P2 验收)", () => {
  /** 构造一个完整 mock client，同时支持 pub（publish）和 sub（subscribe/on/off/unsubscribe/disconnect）接口 */
  function makeFullMockClient() {
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
      /** 模拟服务器向 bong:agent_ui_response 发布消息 */
      emitUiResponse(payload: unknown) {
        (listeners.get("message") ?? []).forEach((l) =>
          l(AGENT_UI_RESPONSE, JSON.stringify(payload)),
        );
      },
    };
  }

  const tsyPlayer = {
    uuid: "player-uuid-e2e-001",
    name: "E2EPlayer",
    realm: "Condense", // rank=3 == tsy_discovery realm_gate=3
    composite_power: 0.6,
    breakdown: { combat: 0.6, wealth: 0.5, social: 0.5, karma: 0.0, territory: 0.5 },
    trend: "rising" as const,
    active_hours: 20,
    zone: "tsy_zone",
    pos: [100, 64, 200] as [number, number, number],
    recent_kills: 0,
    recent_deaths: 0,
  };

  it("e2e: 生成活坍缩渊面板 → emit AgentUiRequestCommandV1 到 bong:agent_ui_cmd（P2 §3）", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    const result = await runtime.triggerUi({
      scenario: "tsy_discovery",
      targetPlayer: tsyPlayer,
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.72",
        danger_tier: "高危",
        agent_narrative: "天道感知到此处有一道裂缝正在收拢",
      },
    });

    // 验证 AgentUiRequestCommandV1 已发布到 bong:agent_ui_cmd
    expect(pub.publish).toHaveBeenCalledOnce();
    const [channel, raw] = pub.publish.mock.calls[0];
    expect(channel, "应发布到 AGENT_UI_CMD").toBe(AGENT_UI_CMD);

    const cmd = JSON.parse(raw as string);
    expect(cmd.request_id, "request_id 应与 result 一致").toBe(result.requestId);
    expect(cmd.target_player, "目标玩家 uuid 正确").toBe(tsyPlayer.uuid);
    expect(cmd.realm_gate, "凝脉境+ realm_gate=3").toBe(3);
    expect(cmd.xml, "XML 含面板内容").toContain("活坍缩渊");
    expect(cmd.xml, "XML 含 owo-ui 根元素").toContain("<owo-ui>");
    expect(cmd.allowed_button_ids, "含 enter_realm 按钮").toContain("enter_realm");
    expect(cmd.allowed_button_ids, "含 dismiss 按钮").toContain("dismiss");
    expect(result.sentBlurVersion, "realm 足够不发模糊版").toBe(false);

    await runtime.disconnect();
  });

  it("e2e: 玩家点击进入按钮 → button_click 进入 pendingButtonClicks 队列 → drain 后注入推演（P2 §3）", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    // 触发面板
    const result = await runtime.triggerUi({
      scenario: "tsy_discovery",
      targetPlayer: tsyPlayer,
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.72",
        danger_tier: "高危",
        agent_narrative: "裂缝收拢中",
      },
    });

    // 模拟 server 向 bong:agent_ui_response 发布 button_click（玩家点击"踏入探寻"）
    sub.emitUiResponse({
      request_id: result.requestId,
      action: "button_click",
      params: { button_id: "enter_realm" },
    });

    // 给异步 handler 跑完
    await Promise.resolve();
    await Promise.resolve();

    // 验证 button_click 已进队列
    const clicks = runtime.drainPendingButtonClicks();
    expect(clicks, "应有 1 个 button_click").toHaveLength(1);
    expect(clicks[0].action, "action 为 button_click").toBe("button_click");
    expect(clicks[0].params["button_id"], "button_id 为 enter_realm").toBe("enter_realm");
    expect(clicks[0].request_id, "request_id 与面板一致").toBe(result.requestId);

    // drain 后队列清空
    expect(runtime.drainPendingButtonClicks(), "drain 后队列为空").toHaveLength(0);

    // stats 正确
    expect(runtime.stats.buttonClick, "stats.buttonClick=1").toBe(1);

    await runtime.disconnect();
  });

  it("e2e: realm_gate_rejected → emit narration 到 bong:agent_narrate（P2 §3）", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    // 触发面板
    const result = await runtime.triggerUi({
      scenario: "tsy_discovery",
      targetPlayer: tsyPlayer,
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.72",
        danger_tier: "高危",
        agent_narrative: "裂缝收拢中",
      },
    });

    // 清除 pub.publish 调用（已有 triggerUi 的调用）
    pub.publish.mockClear();

    // 模拟 server 向 bong:agent_ui_response 发布 realm_gate_rejected error
    sub.emitUiResponse({
      request_id: result.requestId,
      action: "error",
      params: {
        reason: "realm_gate_rejected",
        player_realm: "1",
        required_realm: "3",
      },
    });

    await Promise.resolve();
    await Promise.resolve();

    // 验证 narration 已发布到 bong:agent_narrate
    expect(pub.publish, "应 emit 叙事").toHaveBeenCalledOnce();
    const [narChannel, narRaw] = pub.publish.mock.calls[0];
    expect(narChannel, "应发布到 AGENT_NARRATE").toBe(AGENT_NARRATE);

    const narMsg = JSON.parse(narRaw as string);
    expect(narMsg.v, "narration 包版本 v=1").toBe(1);
    expect(narMsg.narrations, "有 narrations 数组").toHaveLength(1);
    expect(narMsg.narrations[0].text, "叙事文本正确").toBe(REALM_GATE_NARRATION_TEXT);
    expect(narMsg.narrations[0].style, "style=system_warning").toBe("system_warning");

    // stats 正确
    expect(runtime.stats.realmGateRejected, "stats.realmGateRejected=1").toBe(1);
    expect(runtime.stats.narrationPublished, "stats.narrationPublished=1").toBe(1);

    await runtime.disconnect();
  });

  it("e2e: dismissed → drainPendingSessionEnds 队列收到 session 结束事件", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    const result = await runtime.triggerUi({
      scenario: "elder_legacy",
      targetPlayer: tsyPlayer,
      params: { elder_title: "老者", elder_narration: "传承在此", qi_cost: "100" },
    });

    sub.emitUiResponse({
      request_id: result.requestId,
      action: "dismissed",
      params: {},
    });

    await Promise.resolve();
    await Promise.resolve();

    const ends = runtime.drainPendingSessionEnds();
    expect(ends, "dismissed 进入 sessionEnds 队列").toHaveLength(1);
    expect(ends[0].action).toBe("dismissed");
    expect(runtime.stats.dismissed, "stats.dismissed=1").toBe(1);

    await runtime.disconnect();
  });

  it("e2e: timeout → drainPendingSessionEnds 队列收到 session 结束事件", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    const result = await runtime.triggerUi({
      scenario: "tiandao_revelation",
      targetPlayer: { ...tsyPlayer, realm: "Spirit" }, // rank=5 == realm_gate=5
      params: { tiandao_message: "气运将至" },
    });

    sub.emitUiResponse({
      request_id: result.requestId,
      action: "timeout",
      params: {},
    });

    await Promise.resolve();
    await Promise.resolve();

    const ends = runtime.drainPendingSessionEnds();
    expect(ends, "timeout 进入 sessionEnds 队列").toHaveLength(1);
    expect(ends[0].action).toBe("timeout");
    expect(runtime.stats.timeout, "stats.timeout=1").toBe(1);

    await runtime.disconnect();
  });

  it("e2e: realm 不足时发布模糊版（不门控），blur=true，realm_gate=0", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });
    await runtime.connect();

    const result = await runtime.triggerUi({
      scenario: "tsy_discovery",
      targetPlayer: { ...tsyPlayer, realm: "Awaken" }, // rank=1 < realm_gate=3
      params: {
        zone_name: "活坍缩渊",
        spirit_qi_display: "0.5",
        danger_tier: "中危",
        agent_narrative: "有所感应",
      },
    });

    expect(result.sentBlurVersion, "realm 不足发模糊版").toBe(true);

    const [, raw] = pub.publish.mock.calls[0];
    const cmd = JSON.parse(raw as string);
    expect(cmd.realm_gate, "模糊版 realm_gate=0").toBe(0);
    expect(cmd.allowed_button_ids, "模糊版只有 dismiss").toEqual(["dismiss"]);

    await runtime.disconnect();
  });

  it("e2e: connect 后生命周期正常 disconnect，不抛错", async () => {
    const pub = makeFullMockClient();
    const sub = makeFullMockClient();
    const runtime = new AgentUiRuntime({ pub, sub });

    await runtime.connect();
    await expect(runtime.disconnect()).resolves.not.toThrow();
    expect(sub.unsubscribe, "disconnect 调用 unsubscribe").toHaveBeenCalled();
    expect(sub.disconnect, "disconnect 调用 sub.disconnect").toHaveBeenCalled();
  });
});
