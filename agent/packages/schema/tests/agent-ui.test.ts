/**
 * plan-agent-ui-data-v1 P0 — AgentUi schema 三个 TypeBox schema 的 roundtrip + 正负样本测试。
 */

import { describe, expect, it } from "vitest";
import { Value } from "@sinclair/typebox/value";

import {
  AgentUiRequestCommandV1,
  AgentUiRequestPayloadV1,
  AgentUiResponsePayloadV1,
  AgentUiClosePayloadV1,
  AgentUiActionType,
} from "../src/payloads/agent-ui.js";

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

  it("负样本: realm_gate=6 → 超出 maximum:5", () => {
    expect(
      Value.Check(AgentUiRequestCommandV1, { ...validCommand, realm_gate: 6 }),
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
