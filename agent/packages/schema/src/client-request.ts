/**
 * 客户端 → 服务端请求 schema（plan-cultivation-v1 §P1 剩余 IPC schema）。
 * 覆盖三种交互：
 *   - set_meridian_target：选择下一条要打通的经脉
 *   - breakthrough_request：申请境界突破
 *   - forge_request：请求淬炼某条经脉的 rate 或 capacity
 *
 * 传输层由 Fabric 客户端通过 Minecraft CustomPayload 通道发送，服务端
 * 在 network::mod 中反序列化为对应 Bevy Event。
 */
import { Type, type Static } from "@sinclair/typebox";

import { BotanyHarvestModeV1 } from "./botany.js";
import { ForgeAxis } from "./forge-event.js";
import { MeridianId } from "./cultivation.js";

export const SetMeridianTargetRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("set_meridian_target"),
    meridian: MeridianId,
  },
  { additionalProperties: false },
);
export type SetMeridianTargetRequestV1 = Static<typeof SetMeridianTargetRequestV1>;

export const BreakthroughRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("breakthrough_request"),
  },
  { additionalProperties: false },
);
export type BreakthroughRequestV1 = Static<typeof BreakthroughRequestV1>;

export const ForgeRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("forge_request"),
    meridian: MeridianId,
    axis: ForgeAxis,
  },
  { additionalProperties: false },
);
export type ForgeRequestV1 = Static<typeof ForgeRequestV1>;

export const InsightDecisionRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("insight_decision"),
    trigger_id: Type.String(),
    // null = 拒绝 / 超时（服务端等价处理）；非 null = 选中第 n 个候选。
    choice_idx: Type.Union([Type.Integer({ minimum: 0 }), Type.Null()]),
  },
  { additionalProperties: false },
);
export type InsightDecisionRequestV1 = Static<typeof InsightDecisionRequestV1>;

export const BotanyHarvestRequestV1 = Type.Object(
  {
    v: Type.Literal(1),
    type: Type.Literal("botany_harvest_request"),
    session_id: Type.String({ minLength: 1 }),
    mode: BotanyHarvestModeV1,
  },
  { additionalProperties: false },
);
export type BotanyHarvestRequestV1 = Static<typeof BotanyHarvestRequestV1>;

export const ClientRequestV1 = Type.Union([
  SetMeridianTargetRequestV1,
  BreakthroughRequestV1,
  ForgeRequestV1,
  InsightDecisionRequestV1,
  BotanyHarvestRequestV1,
]);
export type ClientRequestV1 = Static<typeof ClientRequestV1>;
