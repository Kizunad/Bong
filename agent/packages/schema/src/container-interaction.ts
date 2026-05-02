// plan-tsy-container-v1 §5.1 — TSY 容器搜刮 IPC schema。
//
// 服务端 → 客户端：SearchStartedV1 / SearchProgressV1（每 5 tick） /
//                  SearchCompletedV1 / SearchAbortedV1 / ContainerStateV1
// 客户端 → 服务端：StartSearchRequestV1 / CancelSearchRequestV1（client-request 路径）
//
// `ContainerKindV1` / `KeyKindV1` / `SearchAbortReasonV1` 是字面量联合，
// 与 server `ContainerKind` / `KeyKind` / `SearchAbortReason` 严格对齐。

import { type Static, Type } from "@sinclair/typebox";

import { type ValidationResult, validate } from "./validate.js";

export const ContainerKindV1 = Type.Union([
  Type.Literal("dry_corpse"),
  Type.Literal("skeleton"),
  Type.Literal("storage_pouch"),
  Type.Literal("stone_casket"),
  Type.Literal("relic_core"),
]);
export type ContainerKindV1 = Static<typeof ContainerKindV1>;

export const KeyKindV1 = Type.Union([
  Type.Literal("stone_casket_key"),
  Type.Literal("jade_coffin_seal"),
  Type.Literal("array_core_sigil"),
]);
export type KeyKindV1 = Static<typeof KeyKindV1>;

export const SearchAbortReasonV1 = Type.Union([
  Type.Literal("moved"),
  Type.Literal("combat"),
  Type.Literal("damaged"),
  Type.Literal("cancelled"),
]);
export type SearchAbortReasonV1 = Static<typeof SearchAbortReasonV1>;

export const ContainerStateV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: Type.Integer({ minimum: 0 }),
    kind: ContainerKindV1,
    family_id: Type.String({ minLength: 1 }),
    world_pos: Type.Tuple([Type.Number(), Type.Number(), Type.Number()]),
    locked: Type.Optional(KeyKindV1),
    depleted: Type.Boolean(),
    searched_by_player_id: Type.Optional(Type.String({ minLength: 1 })),
  },
  { additionalProperties: false },
);
export type ContainerStateV1 = Static<typeof ContainerStateV1>;

export function validateContainerStateV1Contract(data: unknown): ValidationResult {
  return validate(ContainerStateV1, data);
}

export const SearchStartedV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: Type.String({ minLength: 1 }),
    container_entity_id: Type.Integer({ minimum: 0 }),
    required_ticks: Type.Integer({ minimum: 1 }),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SearchStartedV1 = Static<typeof SearchStartedV1>;

export function validateSearchStartedV1Contract(data: unknown): ValidationResult {
  return validate(SearchStartedV1, data);
}

export const SearchProgressV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: Type.String({ minLength: 1 }),
    container_entity_id: Type.Integer({ minimum: 0 }),
    elapsed_ticks: Type.Integer({ minimum: 0 }),
    required_ticks: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type SearchProgressV1 = Static<typeof SearchProgressV1>;

export function validateSearchProgressV1Contract(data: unknown): ValidationResult {
  return validate(SearchProgressV1, data);
}

export const LootPreviewItemV1 = Type.Object(
  {
    template_id: Type.String({ minLength: 1 }),
    display_name: Type.String({ minLength: 1 }),
    stack_count: Type.Integer({ minimum: 1 }),
  },
  { additionalProperties: false },
);
export type LootPreviewItemV1 = Static<typeof LootPreviewItemV1>;

export const SearchCompletedV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: Type.String({ minLength: 1 }),
    container_entity_id: Type.Integer({ minimum: 0 }),
    family_id: Type.String({ minLength: 1 }),
    loot_preview: Type.Array(LootPreviewItemV1),
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SearchCompletedV1 = Static<typeof SearchCompletedV1>;

export function validateSearchCompletedV1Contract(data: unknown): ValidationResult {
  return validate(SearchCompletedV1, data);
}

export const SearchAbortedV1 = Type.Object(
  {
    v: Type.Literal(1),
    player_id: Type.String({ minLength: 1 }),
    container_entity_id: Type.Integer({ minimum: 0 }),
    reason: SearchAbortReasonV1,
    at_tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type SearchAbortedV1 = Static<typeof SearchAbortedV1>;

export function validateSearchAbortedV1Contract(data: unknown): ValidationResult {
  return validate(SearchAbortedV1, data);
}

// client → server：玩家点容器请求开搜
export const StartSearchRequestV1 = Type.Object(
  {
    type: Type.Literal("start_search"),
    v: Type.Literal(1),
    container_entity_id: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type StartSearchRequestV1 = Static<typeof StartSearchRequestV1>;

export function validateStartSearchRequestV1Contract(data: unknown): ValidationResult {
  return validate(StartSearchRequestV1, data);
}

// client → server：玩家按 ESC / 切武器主动取消
export const CancelSearchRequestV1 = Type.Object(
  {
    type: Type.Literal("cancel_search"),
    v: Type.Literal(1),
  },
  { additionalProperties: false },
);
export type CancelSearchRequestV1 = Static<typeof CancelSearchRequestV1>;

export function validateCancelSearchRequestV1Contract(data: unknown): ValidationResult {
  return validate(CancelSearchRequestV1, data);
}
