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

describe("AgentUiResponsePayloadV1.target_player Unicode contract", () => {
  const payload = (targetPlayer: string) => ({
    request_id: "response-payload",
    action: "error",
    target_player: targetPlayer,
    params: { reason: "realm_gate_rejected" },
  });
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

  it("只对新增 server→agent target_player 锁定 1..=128 code points", () => {
    for (const testCase of boundaryCases) {
      expect(
        Value.Check(AgentUiResponsePayloadV1, payload(testCase.value)),
        `target_player 对 ${testCase.name} 应判定为 ${testCase.valid}`,
      ).toBe(testCase.valid);
    }
  });

  it("保留既有 request_id 的 minLength/maxLength 接受集合", () => {
    expect(
      Value.Check(AgentUiResponsePayloadV1, {
        request_id: "😀".repeat(65),
        action: "dismissed",
        params: {},
      }),
      "既有 request_id 仍由 TypeBox UTF-16 maxLength=128 约束，不在本 PR 迁移",
    ).toBe(false);
  });
});

// ─── AgentUiRequestCommandV1（agent → server Redis）──────────────────────────

describe("AgentUiRequestCommandV1", () => {
  const validCommand = {
    request_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    target_player: "b2c3d4e5-f6a7-8901-bcde-f01234567891",
    xml: '<flow-layout><button id="enter_realm">踏入</button></flow-layout>',
    timeout_ticks: 600,
    realm_gate: 3,
    allowed_button_ids: ["enter_realm", "dismiss"],
  };

  it("happy path: 正样本通过校验", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, validCommand),
      "完整合法的 AgentUiRequestCommandV1 应通过校验",
    ).toBe(true);
  });

  it("happy path: realm_gate=0 任意境界可见", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 0 }),
      "realm_gate 下界 0 应表示无门控并通过校验",
    ).toBe(true);
  });

  it("happy path: realm_gate=5 通灵+ 专属", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 5 }),
      "realm_gate=5 应是合法的通灵境门槛",
    ).toBe(true);
  });

  it("happy path: realm_gate=6 化虚+ 专属（上限值，最高境界）", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 6 }),
      "realm_gate 上界 6 应是合法的化虚境门槛",
    ).toBe(true);
  });

  it("happy path: allowed_button_ids 空数组（不门控任何按钮）", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, {
        ...validCommand,
        allowed_button_ids: [],
      }),
      "allowed_button_ids 空数组应表示没有可点击按钮并通过校验",
    ).toBe(true);
  });

  it("happy path: allowed_button_ids 16 条（边界最大值）", () => {
    const ids = Array.from({ length: 16 }, (_, i) => `btn_${i}`);
    expect(
      Value.Check(AgentUiRequestCommandV1, {
        ...validCommand,
        allowed_button_ids: ids,
      }),
      "allowed_button_ids 上界 16 条应通过校验",
    ).toBe(true);
  });

  it("负样本: allowed_button_ids 第 17 条 → 超出 maxItems:16", () => {
    const ids = Array.from({ length: 17 }, (_, i) => `btn_${i}`);
    expect(
      Value.Check(AgentUiRequestCommandV1, {
        ...validCommand,
        allowed_button_ids: ids,
      }),
      "allowed_button_ids 17 条超过 maxItems=16 应被拒绝",
    ).toBe(false);
  });

  it("负样本: realm_gate=7 → 超出 maximum:6", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 7 }),
      "realm_gate=7 超过最高境界门槛 6 应被拒绝",
    ).toBe(false);
  });

  it("负样本: realm_gate=-1 → 低于 minimum:0", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: -1 }),
      "realm_gate=-1 低于无门控下界 0 应被拒绝",
    ).toBe(false);
  });

  it("负样本: timeout_ticks=19 → 低于 minimum:20", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, {
        ...validCommand,
        timeout_ticks: 19,
      }),
      "timeout_ticks=19 低于最短 20 ticks 应被拒绝",
    ).toBe(false);
  });

  it("负样本: timeout_ticks=2401 → 超出 maximum:2400", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, {
        ...validCommand,
        timeout_ticks: 2401,
      }),
      "timeout_ticks=2401 超过最长 2400 ticks 应被拒绝",
    ).toBe(false);
  });

  it("负样本: 缺失 request_id", () => {
    const { request_id: _, ...missing } = validCommand;
    expect(
      Value.Check(AgentUiRequestCommandV1, missing),
      "AgentUiRequestCommandV1 缺失必填 request_id 应被拒绝",
    ).toBe(false);
  });

  it("负样本: 缺失 allowed_button_ids", () => {
    const { allowed_button_ids: _, ...missing } = validCommand;
    expect(
      Value.Check(AgentUiRequestCommandV1, missing),
      "AgentUiRequestCommandV1 缺失必填 allowed_button_ids 应被拒绝",
    ).toBe(false);
  });

  it("负样本: xml 超 8192 字节", () => {
    const bigXml = "<label>" + "x".repeat(8200) + "</label>";
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, xml: bigXml }),
      "AgentUiRequestCommandV1 的 xml 超过 8192 字符上限应被拒绝",
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
      Value.Check(AgentUiRequestPayloadV1, {
        ...validPayload,
        timeout_ticks: 19,
      }),
    ).toBe(false);
  });
});

// ─── AgentUiResponsePayloadV1（C2S CustomPayload + server→agent Redis）───────

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
      request_id: "legacy-response",
      action: "error",
      params: { reason: "realm_gate_rejected" },
    };
    expect(Value.Check(AgentUiResponsePayloadV1, payload)).toBe(true);
  });

  it("负样本: target_player 显式 null 不得冒充 legacy 缺字段", () => {
    const payload = {
      request_id: "null-target",
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
    expect(Value.Check(AgentUiClientResponsePayloadV1, clientPayload)).toBe(
      true,
    );
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
    expect(Value.Check(AgentUiClientResponsePayloadV1, spoofedPayload)).toBe(
      false,
    );
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
  it("符合 AgentUiResponsePayloadV1 schema（button_click action）", () => {
    const sample = loadSample("client-request.agent-ui-response.sample.json");
    // sample 含 v/type 额外字段（C2S CustomPayload 格式），提取 agent-facing 字段做校验
    const asRecord = sample as Record<string, unknown>;
    const agentPayload = {
      request_id: asRecord["request_id"],
      action: asRecord["action"],
      params: asRecord["params"],
    };
    const result = Value.Check(AgentUiResponsePayloadV1, agentPayload);
    expect(
      result,
      `client-request.agent-ui-response.sample.json 的 agent-facing 字段应通过 AgentUiResponsePayloadV1 校验`,
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
      ...(asRecord["reason"] !== undefined
        ? { reason: asRecord["reason"] }
        : {}),
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
      expect(Value.Check(AgentUiClosePayloadV1, payload), entry.name).toBe(
        true,
      );
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
