import { Type, type Static } from "@sinclair/typebox";
import { ChatIntent } from "./common.js";

// ─── 原始聊天消息 (Server → Agent) ─────────────────────

export const ChatMessageV1 = Type.Object({
  v: Type.Literal(1),
  ts: Type.Integer({ description: "Unix timestamp (seconds)" }),
  player: Type.String(),
  raw: Type.String({ maxLength: 256 }),
  zone: Type.String(),
});
export type ChatMessageV1 = Static<typeof ChatMessageV1>;

// ─── 预处理后的聊天信号 (Agent 内部使用) ────────────────

export const ChatSignal = Type.Object({
  player: Type.String(),
  raw: Type.String(),
  sentiment: Type.Number({ minimum: -1, maximum: 1 }),
  intent: ChatIntent,
  mentions_mechanic: Type.Optional(Type.String()),
  influence_weight: Type.Number({ minimum: 0, maximum: 1 }),
});
export type ChatSignal = Static<typeof ChatSignal>;
