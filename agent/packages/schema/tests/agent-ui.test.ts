/**
 * plan-agent-ui-data-v1 P0 — AgentUi schema 三个 TypeBox schema 的 roundtrip + 正负样本测试。
 *
 * sample roundtrip（项目惯例双端对拍）：
 *   4 个新 sample 文件均做 readFileSync→TypeBox.Check 对拍，确保 sample 与 schema 保持一致。
 */

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { Value } from "@sinclair/typebox/value";

const __dirname = dirname(fileURLToPath(import.meta.url));
const samplesDir = join(__dirname, "..", "samples");

function loadSample(name: string): unknown {
  return JSON.parse(readFileSync(join(samplesDir, name), "utf-8"));
}

import {
  AgentUiRequestCommandV1,
  AgentUiRequestPayloadV1,
  AgentUiClientResponsePayloadV1,
  AgentUiResponsePayloadV1,
  AgentUiClosePayloadV1,
  AgentUiActionType,
  type AgentUiErrorReasonV1,
} from "../src/payloads/agent-ui.js";
import {
  AgentUiResponseRequestV1,
  ClientRequestV1,
} from "../src/client-request.js";

describe("Agent UI ID Unicode code-point contract", () => {
  const schemaFields = [
    {
      name: "AgentUiRequestCommandV1.request_id",
      schema: AgentUiRequestCommandV1,
      payload: (value: string) => ({
        request_id: value,
        target_player: "offline:Target",
        xml: "<flow-layout />",
        timeout_ticks: 600,
        realm_gate: 0,
        allowed_button_ids: [],
      }),
    },
    {
      name: "AgentUiRequestCommandV1.target_player",
      schema: AgentUiRequestCommandV1,
      payload: (value: string) => ({
        request_id: "request-command",
        target_player: value,
        xml: "<flow-layout />",
        timeout_ticks: 600,
        realm_gate: 0,
        allowed_button_ids: [],
      }),
    },
    {
      name: "AgentUiRequestPayloadV1.request_id",
      schema: AgentUiRequestPayloadV1,
      payload: (value: string) => ({
        request_id: value,
        target_player: "offline:Target",
        xml: "<flow-layout />",
        timeout_ticks: 600,
      }),
    },
    {
      name: "AgentUiRequestPayloadV1.target_player",
      schema: AgentUiRequestPayloadV1,
      payload: (value: string) => ({
        request_id: "request-payload",
        target_player: value,
        xml: "<flow-layout />",
        timeout_ticks: 600,
      }),
    },
    {
      name: "AgentUiClientResponsePayloadV1.request_id",
      schema: AgentUiClientResponsePayloadV1,
      payload: (value: string) => ({ request_id: value, action: "dismissed", params: {} }),
    },
    {
      name: "AgentUiResponsePayloadV1.request_id",
      schema: AgentUiResponsePayloadV1,
      payload: (value: string) => ({ request_id: value, action: "dismissed", params: {} }),
    },
    {
      name: "AgentUiResponsePayloadV1.target_player",
      schema: AgentUiResponsePayloadV1,
      payload: (value: string) => ({
        request_id: "response-payload",
        action: "error",
        target_player: value,
        params: { reason: "realm_gate_rejected" },
      }),
    },
    {
      name: "AgentUiClosePayloadV1.request_id",
      schema: AgentUiClosePayloadV1,
      payload: (value: string) => ({ request_id: value }),
    },
  ] as const;
  const boundaryCases = [
    { name: "empty", value: "", valid: false },
    { name: "65 emoji", value: "😀".repeat(65), valid: true },
    { name: "128 emoji", value: "😀".repeat(128), valid: true },
    { name: "129 emoji", value: "😀".repeat(129), valid: false },
    { name: "127 BMP + 1 astral", value: `${"a".repeat(127)}😀`, valid: true },
    { name: "128 BMP + 1 astral", value: `${"a".repeat(128)}😀`, valid: false },
    { name: "128 BMP", value: "界".repeat(128), valid: true },
    { name: "129 BMP", value: "界".repeat(129), valid: false },
    { name: "lone high surrogate", value: "\ud800", valid: false },
    { name: "lone low surrogate", value: "\udc00", valid: false },
    { name: "embedded lone surrogate", value: "a\ud800b", valid: false },
  ];

  it("pins all five TypeBox schemas and every Agent UI ID field", () => {
    for (const field of schemaFields) {
      for (const testCase of boundaryCases) {
        expect(
          Value.Check(field.schema, field.payload(testCase.value)),
          `${field.name} 对 ${testCase.name} 应按 1..=128 Unicode code points 判定为 ${testCase.valid}`,
        ).toBe(testCase.valid);
      }
    }
  });
});

// ─── AgentUiRequestCommandV1（agent → server Redis）──────────────────────────

describe("AgentUiRequestCommandV1", () => {
  const validCommand = {
    request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    target_player: "b2c3d4e5-f6a7-8901-bcde-f01234567891",
    xml: "<flow-layout><button id=\"enter_realm\">踏入</button></flow-layout>",
    timeout_ticks: 600,
    realm_gate: 3,
    allowed_button_ids: ["enter_realm", "dismiss"],
  };

  it("happy path: 正样本通过校验", () => {
    expect(Value.Check(AgentUiRequestCommandV1, validCommand)).toBe(true);
  });

  it("happy path: realm_gate=0 任意境界可见", () => {
    expect(Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 0 })).toBe(
      true,
    );
  });

  it("happy path: realm_gate=5 通灵+ 专属", () => {
    expect(Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 5 })).toBe(
      true,
    );
  });

  it("happy path: realm_gate=6 化虚+ 专属（上限值，最高境界）", () => {
    expect(Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 6 })).toBe(
      true,
    );
  });

  it("happy path: allowed_button_ids 空数组（不门控任何按钮）", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, allowed_button_ids: [] }),
    ).toBe(true);
  });

  it("happy path: allowed_button_ids 16 条（边界最大值）", () => {
    const ids = Array.from({ length: 16 }, (_, i) => `btn_${i}`);
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, allowed_button_ids: ids }),
    ).toBe(true);
  });

  it("负样本: allowed_button_ids 第 17 条 → 超出 maxItems:16", () => {
    const ids = Array.from({ length: 17 }, (_, i) => `btn_${i}`);
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, allowed_button_ids: ids }),
    ).toBe(false);
  });

  it("负样本: realm_gate=7 → 超出 maximum:6", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 7 }),
    ).toBe(false);
  });

  it("负样本: realm_gate=-1 → 低于 minimum:0", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: -1 }),
    ).toBe(false);
  });

  it("负样本: timeout_ticks=19 → 低于 minimum:20", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, timeout_ticks: 19 }),
    ).toBe(false);
  });

  it("负样本: timeout_ticks=2401 → 超出 maximum:2400", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, timeout_ticks: 2401 }),
    ).toBe(false);
  });

  it("负样本: 缺失 request_id", () => {
    const { request_id: _, ...missing } = validCommand;
    expect(Value.Check(AgentUiRequestCommandV1, missing)).toBe(false);
  });

  it("负样本: 缺失 allowed_button_ids", () => {
    const { allowed_button_ids: _, ...missing } = validCommand;
    expect(Value.Check(AgentUiRequestCommandV1, missing)).toBe(false);
  });

  it("负样本: xml 超 8192 字节", () => {
    const bigXml = "<label>" + "x".repeat(8200) + "</label>";
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, xml: bigXml }),
    ).toBe(false);
  });
});

// ─── AgentUiRequestPayloadV1（server → client via server_data）──────────────

describe("AgentUiRequestPayloadV1", () => {
  const validPayload = {
    request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    target_player: "b2c3d4e5-f6a7-8901-bcde-f01234567891",
    xml: "<flow-layout><label>test</label></flow-layout>",
    timeout_ticks: 400,
  };

  it("happy path: 正样本通过校验", () => {
    expect(Value.Check(AgentUiRequestPayloadV1, validPayload)).toBe(true);
  });

  it("不含 realm_gate / allowed_button_ids 字段", () => {
    // 确认这两个安全字段不在此 schema 中
    const withSafeFields = {
      ...validPayload,
      realm_gate: 3,
      allowed_button_ids: ["btn"],
    };
    // additionalProperties: false 应拒绝多余字段
    expect(Value.Check(AgentUiRequestPayloadV1, withSafeFields)).toBe(false);
  });

  it("负样本: 缺失 xml", () => {
    const { xml: _, ...missing } = validPayload;
    expect(Value.Check(AgentUiRequestPayloadV1, missing)).toBe(false);
  });

  it("负样本: timeout_ticks=19 → 低于 minimum:20", () => {
    expect(
      Value.Check(AgentUiRequestPayloadV1, { ...validPayload, timeout_ticks: 19 }),
    ).toBe(false);
  });
});

// ─── AgentUiResponsePayloadV1（server→agent Redis）────────────────────────────

describe("AgentUiResponsePayloadV1", () => {
  it("happy path: button_click 正样本", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "button_click",
      params: { button_id: "enter_realm" },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("happy path: dismissed（空 params）", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "dismissed",
      params: {},
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("happy path: timeout", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "timeout",
      params: {},
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("happy path: replaced", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "replaced",
      params: {},
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("happy path: error（realm_gate_rejected）", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "error",
      target_player: "offline:TestPlayer",
      params: {
        reason: "realm_gate_rejected",
        player_realm: "2",
        required_realm: "3",
      },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("兼容旧 payload: target_player 缺省仍通过校验", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "error",
      params: {
        reason: "realm_gate_rejected",
        player_realm: "2",
        required_realm: "3",
      },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("负样本: target_player 显式 null 不得冒充 legacy 缺字段", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "error",
      target_player: null,
      params: { reason: "realm_gate_rejected" },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(false);
  });

  it("AgentUiErrorReasonV1 覆盖 server 实发 error reason", () => {
    const reasons: AgentUiErrorReasonV1[] = [
      "realm_gate_rejected",
      "invalid_button_id",
      "player_offline",
      "invalid_command",
      "xml_sanitize_failed",
    ];
    expect(reasons).toHaveLength(5);
    expect(reasons).toContain("invalid_command");
    expect(reasons).toContain("xml_sanitize_failed");
  });

  it("happy path: parse_error", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "parse_error",
      params: { error_code: "xml_parse_failed" },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("负样本: 非法 action 字面量", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "unknown_action",
      params: {},
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(false);
  });

  it("负样本: 缺失 request_id", () => {
    const payload = {
      action: "dismissed",
      params: {},
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(false);
  });

  it("负样本: params 包含非 string 值", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "button_click",
      params: { button_id: 123 }, // number 不符合 Record<string, string>
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(false);
  });

  it("负样本: target_player 为空字符串", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "error",
      target_player: "",
      params: { reason: "realm_gate_rejected" },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(false);
  });

  it("边界: target_player 按 Unicode code points 校验 1..=128", () => {
    const base = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      action: "error",
      params: { reason: "realm_gate_rejected" },
    };
    const cases = [
      { name: "64 emoji", value: "😀".repeat(64), utf16: 128, codePoints: 64, valid: true },
      { name: "65 emoji", value: "😀".repeat(65), utf16: 130, codePoints: 65, valid: true },
      { name: "128 emoji", value: "😀".repeat(128), utf16: 256, codePoints: 128, valid: true },
      { name: "129 emoji", value: "😀".repeat(129), utf16: 258, codePoints: 129, valid: false },
      {
        name: "127 BMP + 1 astral",
        value: "a".repeat(127) + "😀",
        utf16: 129,
        codePoints: 128,
        valid: true,
      },
      {
        name: "128 BMP + 1 astral",
        value: "a".repeat(128) + "😀",
        utf16: 130,
        codePoints: 129,
        valid: false,
      },
      { name: "128 BMP", value: "界".repeat(128), utf16: 128, codePoints: 128, valid: true },
      { name: "129 BMP", value: "界".repeat(129), utf16: 129, codePoints: 129, valid: false },
    ];

    for (const testCase of cases) {
      expect(testCase.value.length, `${testCase.name} 的 JavaScript String.length`).toBe(
        testCase.utf16,
      );
      expect([...testCase.value], `${testCase.name} 的 Unicode scalar 数`).toHaveLength(
        testCase.codePoints,
      );
      expect(
        Value.Check(AgentUiResponsePayloadV1, {
          ...base,
          target_player: testCase.value,
        }),
        `${testCase.name} 应按 ${testCase.codePoints} 个 Unicode code points 判定为 ${testCase.valid}`,
      ).toBe(testCase.valid);
    }
  });

  it("wire 边界: raw JSON lone surrogate 拒绝，合法 surrogate pair 通过", () => {
    const parseTarget = (escapedTarget: string) =>
      JSON.parse(
        `{"request_id":"target-surrogate","action":"error","target_player":"${escapedTarget}","params":{"reason":"realm_gate_rejected"}}`,
      ) as { target_player: string };

    for (const [name, escapedTarget] of [
      ["lone high surrogate", "\\ud800"],
      ["lone low surrogate", "\\udc00"],
    ] as const) {
      const payload = parseTarget(escapedTarget);
      expect(payload.target_player.length, `${name} 在 JavaScript 中是 1 UTF-16 unit`).toBe(1);
      expect(
        Value.Check(AgentUiResponsePayloadV1, payload),
        `${name} 不得通过 server→agent wire schema`,
      ).toBe(false);
    }

    const validPair = parseTarget("\\ud83d\\ude00");
    expect(validPair.target_player).toBe("😀");
    expect(validPair.target_player.length).toBe(2);
    expect(Value.Check(AgentUiResponsePayloadV1, validPair)).toBe(true);

    const loneSurrogateRequestId = JSON.parse(
      '{"request_id":"\\ud800","action":"dismissed","params":{}}',
    );
    expect(
      Value.Check(AgentUiResponsePayloadV1, loneSurrogateRequestId),
      "request_id 与 target_player 必须共用同一 well-formed UTF-16 ID 契约",
    ).toBe(false);
  });

  it("wire pattern 同时兼容无 flag 与 Unicode-aware ECMA-262 解释", () => {
    const targetSchema = AgentUiResponsePayloadV1.properties.target_player as {
      pattern: string;
    };
    const legacyPattern = new RegExp(targetSchema.pattern);
    const unicodePattern = new RegExp(targetSchema.pattern, "u");
    const cases = [
      { name: "BMP", value: "界", valid: true },
      { name: "astral emoji", value: "😀", valid: true },
      { name: "mixed BMP and astral", value: "残😀界", valid: true },
      { name: "65 emoji", value: "😀".repeat(65), valid: true },
      { name: "128 emoji boundary", value: "😀".repeat(128), valid: true },
      { name: "129 emoji overflow", value: "😀".repeat(129), valid: false },
      { name: "127 BMP + 1 astral boundary", value: "a".repeat(127) + "😀", valid: true },
      { name: "128 BMP + 1 astral overflow", value: "a".repeat(128) + "😀", valid: false },
      { name: "128 BMP boundary", value: "界".repeat(128), valid: true },
      { name: "129 BMP overflow", value: "界".repeat(129), valid: false },
      { name: "empty", value: "", valid: false },
      { name: "lone high surrogate", value: "\ud800", valid: false },
      { name: "lone low surrogate", value: "\udc00", valid: false },
      { name: "embedded lone high surrogate", value: "a\ud800b", valid: false },
      { name: "embedded lone low surrogate", value: "a\udc00b", valid: false },
    ];

    for (const testCase of cases) {
      expect(
        legacyPattern.test(testCase.value),
        `${testCase.name} 的无 flag ECMA-262 结果应为 ${testCase.valid}`,
      ).toBe(testCase.valid);
      expect(
        unicodePattern.test(testCase.value),
        `${testCase.name} 的 Unicode-aware ECMA-262 结果应为 ${testCase.valid}`,
      ).toBe(testCase.valid);
    }
  });
});

describe("AgentUiClientResponsePayloadV1 / AgentUiResponseRequestV1", () => {
  const clientPayload = {
    request_id: "req-c2s-real-producer",
    action: "button_click" as const,
    params: { button_id: "enter_realm" },
  };
  const clientRequest = {
    v: 1 as const,
    type: "agent_ui_response" as const,
    ...clientPayload,
  };

  it("接受真实 Fabric producer 的 request_id/action/params 形状", () => {
    expect(Value.Check(AgentUiClientResponsePayloadV1, clientPayload)).toBe(true);
    expect(Value.Check(AgentUiResponseRequestV1, clientRequest)).toBe(true);
    expect(Value.Check(ClientRequestV1, clientRequest)).toBe(true);
  });

  it("拒绝 C2S 伪造 target_player，该字段只属于 server→agent 权威响应", () => {
    const spoofedPayload = {
      ...clientPayload,
      target_player: "offline:Bystander",
    };
    const spoofedRequest = {
      ...clientRequest,
      target_player: "offline:Bystander",
    };
    expect(Value.Check(AgentUiClientResponsePayloadV1, spoofedPayload)).toBe(false);
    expect(Value.Check(AgentUiResponseRequestV1, spoofedRequest)).toBe(false);
    expect(Value.Check(ClientRequestV1, spoofedRequest)).toBe(false);
  });
});

// ─── AgentUiActionType 字面联合 ──────────────────────────────────────────────

describe("AgentUiActionType", () => {
  const validActions = [
    "button_click",
    "dismissed",
    "timeout",
    "replaced",
    "error",
    "parse_error",
  ];

  for (const action of validActions) {
    it(`valid action: "${action}"`, () => {
      expect(Value.Check(AgentUiActionType, action)).toBe(true);
    });
  }

  it("负样本: 非法 action", () => {
    expect(Value.Check(AgentUiActionType, "click")).toBe(false);
  });

  it("负样本: 空字符串", () => {
    expect(Value.Check(AgentUiActionType, "")).toBe(false);
  });
});

// ─── AgentUiClosePayloadV1（server → client close 信号）─────────────────────

describe("AgentUiClosePayloadV1", () => {
  it("happy path: 带 reason", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      reason: "invalid_button_id",
    };
    expect(Value.Check(AgentUiClosePayloadV1, payload)).toBe(true);
  });

  it("happy path: reason 省略（Replaced 语义）", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    };
    expect(Value.Check(AgentUiClosePayloadV1, payload)).toBe(true);
  });

  it("happy path: session_expired reason", () => {
    const payload = {
      request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
      reason: "session_expired",
    };
    expect(Value.Check(AgentUiClosePayloadV1, payload)).toBe(true);
  });

  it("负样本: 缺失 request_id", () => {
    const payload = { reason: "replaced" };
    expect(Value.Check(AgentUiClosePayloadV1, payload)).toBe(false);
  });
});

// ─── sample roundtrip（项目惯例双端 sample 对拍）─────────────────────────────
//
// 4 个 plan-agent-ui-data-v1 新增 sample 文件的 TypeBox roundtrip 验证。
// 确保 sample.json ↔ TypeBox schema 双端对齐（Rust include_str! 由 server 侧负责）。

describe("sample roundtrip: agent_ui_request_command.sample.json", () => {
  it("符合 AgentUiRequestCommandV1 schema", () => {
    const sample = loadSample("agent_ui_request_command.sample.json");
    const result = Value.Check(AgentUiRequestCommandV1, sample);
    expect(
      result,
      `agent_ui_request_command.sample.json 应通过 AgentUiRequestCommandV1 校验`,
    ).toBe(true);
  });
});

describe("sample roundtrip: client-request.agent-ui-response.sample.json", () => {
  it("符合真实 C2S AgentUiResponseRequestV1 schema（button_click action）", () => {
    const sample = loadSample("client-request.agent-ui-response.sample.json");
    expect(
      Value.Check(AgentUiResponseRequestV1, sample),
      `client-request.agent-ui-response.sample.json 应通过不含 target_player 的 C2S schema`,
    ).toBe(true);

    // 去掉 v/type 后与真实 Fabric producer payload 完全一致。
    const asRecord = sample as Record<string, unknown>;
    const clientPayload = {
      request_id: asRecord["request_id"],
      action: asRecord["action"],
      params: asRecord["params"],
    };
    const result = Value.Check(AgentUiClientResponsePayloadV1, clientPayload);
    expect(
      result,
      `client-request.agent-ui-response.sample.json 的业务字段应通过 AgentUiClientResponsePayloadV1 校验`,
    ).toBe(true);
  });
});

describe("sample roundtrip: server-data.agent-ui-close.sample.json", () => {
  it("符合 AgentUiClosePayloadV1 schema", () => {
    const sample = loadSample("server-data.agent-ui-close.sample.json");
    // sample 含 v/type 额外字段，提取 close-facing 字段
    const asRecord = sample as Record<string, unknown>;
    const closePayload = {
      request_id: asRecord["request_id"],
      ...(asRecord["reason"] !== undefined ? { reason: asRecord["reason"] } : {}),
    };
    const result = Value.Check(AgentUiClosePayloadV1, closePayload);
    expect(
      result,
      `server-data.agent-ui-close.sample.json 的 close 字段应通过 AgentUiClosePayloadV1 校验`,
    ).toBe(true);
  });
});

describe("shared wire fixture: agent-ui-close.channel-wire.sample.json", () => {
  it("锁定专属 channel 与三种 server 实际裸 JSON payload", () => {
    const fixture = loadSample("agent-ui-close.channel-wire.sample.json") as {
      channel: string;
      cases: Array<{
        name: string;
        request_id: string;
        reason?: string;
        payload_utf8: string;
      }>;
    };

    expect(fixture.channel).toBe("bong:agent_ui_close");
    expect(fixture.cases.map((entry) => entry.name)).toEqual([
      "replaced",
      "session_expired",
      "invalid_button_id",
    ]);

    for (const entry of fixture.cases) {
      const payload = JSON.parse(entry.payload_utf8) as unknown;
      expect(Value.Check(AgentUiClosePayloadV1, payload), entry.name).toBe(true);
      expect(payload).toEqual({
        request_id: entry.request_id,
        ...(entry.reason !== undefined ? { reason: entry.reason } : {}),
      });
      expect(JSON.stringify(payload)).toBe(entry.payload_utf8);
    }
  });
});

describe("sample roundtrip: server-data.agent-ui-request.sample.json", () => {
  it("符合 AgentUiRequestPayloadV1 schema（server→client server_data）", () => {
    const sample = loadSample("server-data.agent-ui-request.sample.json");
    // sample 含 v/type 额外字段，提取 payload 字段做校验
    const asRecord = sample as Record<string, unknown>;
    const requestPayload = {
      request_id: asRecord["request_id"],
      target_player: asRecord["target_player"],
      xml: asRecord["xml"],
      timeout_ticks: asRecord["timeout_ticks"],
    };
    const result = Value.Check(AgentUiRequestPayloadV1, requestPayload);
    expect(
      result,
      `server-data.agent-ui-request.sample.json 的 payload 字段应通过 AgentUiRequestPayloadV1 校验`,
    ).toBe(true);
  });
});
