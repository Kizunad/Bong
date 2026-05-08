/**
 * plan-identity-v1 P5 IPC schema —— 仅 server → agent / server → client。
 *
 * - {@link WantedPlayerEventV1}：玩家 active identity 反应分级跌入 Wanted (<-75)
 *   后由 server 发到 `bong:wanted_player`，agent 用作通缉令 narration 触发器。
 * - {@link IdentityPanelStateV1}：server → client CustomPayload，灵龛 GUI 同步
 *   当前 identity 列表 + active id（玩家自己看，NPC 反应分级 NPC 视角不下发）。
 */

import { Type, type Static } from "@sinclair/typebox";

import { validate, type ValidationResult } from "./validate.js";

/** RevealedTagKind 全枚举（必须与 server `RevealedTagKind` enum 对齐）。 */
export const RevealedTagKindV1 = Type.Union([
  Type.Literal("dugu_revealed"),
  Type.Literal("anqi_master"),
  Type.Literal("zhenfa_master"),
  Type.Literal("baomai_user"),
  Type.Literal("tuike_user"),
  Type.Literal("woliu_master"),
  Type.Literal("zhenmai_user"),
  Type.Literal("sword_master"),
  Type.Literal("forge_master"),
  Type.Literal("alchemy_master"),
]);
export type RevealedTagKindV1 = Static<typeof RevealedTagKindV1>;

/** ReactionTier 4 档（对齐 server `ReactionTier`）。 */
export const ReactionTierV1 = Type.Union([
  Type.Literal("high"),
  Type.Literal("normal"),
  Type.Literal("low"),
  Type.Literal("wanted"),
]);
export type ReactionTierV1 = Static<typeof ReactionTierV1>;

/** Server → Agent：玩家被通缉事件（仅 Wanted 档触发）。 */
export const WantedPlayerEventV1 = Type.Object(
  {
    event: Type.Literal("wanted_player"),
    player_uuid: Type.String(),
    char_id: Type.String(),
    identity_display_name: Type.String(),
    identity_id: Type.Integer({ minimum: 0 }),
    reputation_score: Type.Integer(),
    primary_tag: RevealedTagKindV1,
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type WantedPlayerEventV1 = Static<typeof WantedPlayerEventV1>;

/** identity-list 单条 entry（client 面板用）。 */
export const IdentityPanelEntryV1 = Type.Object(
  {
    identity_id: Type.Integer({ minimum: 0 }),
    display_name: Type.String(),
    reputation_score: Type.Integer(),
    frozen: Type.Boolean(),
    revealed_tag_kinds: Type.Array(RevealedTagKindV1),
  },
  { additionalProperties: false },
);
export type IdentityPanelEntryV1 = Static<typeof IdentityPanelEntryV1>;

/** Server → Client CustomPayload `bong:identity_panel_state`：身份面板状态。 */
export const IdentityPanelStateV1 = Type.Object(
  {
    active_identity_id: Type.Integer({ minimum: 0 }),
    last_switch_tick: Type.Integer({ minimum: 0 }),
    cooldown_remaining_ticks: Type.Integer({ minimum: 0 }),
    identities: Type.Array(IdentityPanelEntryV1),
  },
  { additionalProperties: false },
);
export type IdentityPanelStateV1 = Static<typeof IdentityPanelStateV1>;

export function validateWantedPlayerEventV1(payload: unknown): ValidationResult {
  return validate(WantedPlayerEventV1, payload);
}

export function validateIdentityPanelStateV1(payload: unknown): ValidationResult {
  return validate(IdentityPanelStateV1, payload);
}
