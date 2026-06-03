/**
 * plan-combat-skill-feedback-bridges-v1 P1 — 爆脉 v4 反馈整桥 TypeBox schema。
 *
 * 5 个 Redis payload 变体（crack_reading 无 Redis，走 S2C CustomPayload）：
 *   - BaomaiV4ScarCircuitFormedV1   → bong:baomai_v4/scar_circuit_formed
 *   - BaomaiV4ScarCircuitBrokenV1   → bong:baomai_v4/scar_circuit_broken
 *   - BaomaiV4IronCocoonStageUpV1   → bong:baomai_v4/iron_cocoon_stage_up
 *   - BaomaiV4ResonanceLockV1       → bong:baomai_v4/resonance_lock
 *   - BaomaiV4ResonanceLockEndV1    → bong:baomai_v4/resonance_lock_end
 */

import { type Static, Type } from "@sinclair/typebox";
import { type ValidationResult, validate } from "./validate.js";

// ── ScarCircuitKind enum ────────────────────────────────────────────────────

export const ScarCircuitKind = Type.Union([
  Type.Literal("tiger_mouth"),
  Type.Literal("triple_yang"),
  Type.Literal("heart_lung"),
  Type.Literal("liver_kidney"),
  Type.Literal("gall_stomach"),
  Type.Literal("spleen_kidney"),
  Type.Literal("ren_du_bridge"),
]);
export type ScarCircuitKind = Static<typeof ScarCircuitKind>;

// ── CircuitBreakReason enum ─────────────────────────────────────────────────

export const CircuitBreakReason = Type.Union([
  Type.Literal("healed"),
  Type.Literal("deepened"),
  Type.Literal("severed"),
  Type.Literal("closed"),
]);
export type CircuitBreakReason = Static<typeof CircuitBreakReason>;

// ── IronCocoonStage enum ────────────────────────────────────────────────────

export const IronCocoonStage = Type.Union([
  Type.Literal("none"),
  Type.Literal("tough_skin"),
  Type.Literal("iron_bone"),
  Type.Literal("dull_flesh"),
  Type.Literal("scar_forged"),
]);
export type IronCocoonStage = Static<typeof IronCocoonStage>;

// ── LockEndReason (tagged union) ────────────────────────────────────────────

export const LockEndReasonExpired = Type.Object(
  { type: Type.Literal("expired") },
  { additionalProperties: false },
);

export const LockEndReasonRetreated = Type.Object(
  {
    type: Type.Literal("retreated"),
    entity_id: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);

export const LockEndReasonDeath = Type.Object(
  {
    type: Type.Literal("death"),
    entity_id: Type.String({ minLength: 1 }),
  },
  { additionalProperties: false },
);

export const LockEndReason = Type.Union([
  LockEndReasonExpired,
  LockEndReasonRetreated,
  LockEndReasonDeath,
]);
export type LockEndReason = Static<typeof LockEndReason>;

// ── BaomaiV4ScarCircuitFormedV1 ─────────────────────────────────────────────

export const BaomaiV4ScarCircuitFormedV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: Type.String({ minLength: 1 }),
    circuit: ScarCircuitKind,
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV4ScarCircuitFormedV1 = Static<typeof BaomaiV4ScarCircuitFormedV1>;

export function validateBaomaiV4ScarCircuitFormedV1(data: unknown): ValidationResult {
  return validate(BaomaiV4ScarCircuitFormedV1, data);
}

// ── BaomaiV4ScarCircuitBrokenV1 ─────────────────────────────────────────────

export const BaomaiV4ScarCircuitBrokenV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: Type.String({ minLength: 1 }),
    circuit: ScarCircuitKind,
    reason: CircuitBreakReason,
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV4ScarCircuitBrokenV1 = Static<typeof BaomaiV4ScarCircuitBrokenV1>;

export function validateBaomaiV4ScarCircuitBrokenV1(data: unknown): ValidationResult {
  return validate(BaomaiV4ScarCircuitBrokenV1, data);
}

// ── BaomaiV4IronCocoonStageUpV1 ─────────────────────────────────────────────

export const BaomaiV4IronCocoonStageUpV1 = Type.Object(
  {
    v: Type.Literal(1),
    entity_id: Type.String({ minLength: 1 }),
    from: IronCocoonStage,
    to: IronCocoonStage,
    total_overloads: Type.Integer({ minimum: 0 }),
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV4IronCocoonStageUpV1 = Static<typeof BaomaiV4IronCocoonStageUpV1>;

export function validateBaomaiV4IronCocoonStageUpV1(data: unknown): ValidationResult {
  return validate(BaomaiV4IronCocoonStageUpV1, data);
}

// ── BaomaiV4ResonanceLockV1 ─────────────────────────────────────────────────

export const BaomaiV4ResonanceLockV1 = Type.Object(
  {
    v: Type.Literal(1),
    fighter_a_id: Type.String({ minLength: 1 }),
    fighter_b_id: Type.String({ minLength: 1 }),
    started_at: Type.Integer({ minimum: 0 }),
    ends_at: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV4ResonanceLockV1 = Static<typeof BaomaiV4ResonanceLockV1>;

export function validateBaomaiV4ResonanceLockV1(data: unknown): ValidationResult {
  return validate(BaomaiV4ResonanceLockV1, data);
}

// ── BaomaiV4ResonanceLockEndV1 ──────────────────────────────────────────────

export const BaomaiV4ResonanceLockEndV1 = Type.Object(
  {
    v: Type.Literal(1),
    fighter_a_id: Type.String({ minLength: 1 }),
    fighter_b_id: Type.String({ minLength: 1 }),
    reason: LockEndReason,
    tick: Type.Integer({ minimum: 0 }),
  },
  { additionalProperties: false },
);
export type BaomaiV4ResonanceLockEndV1 = Static<typeof BaomaiV4ResonanceLockEndV1>;

export function validateBaomaiV4ResonanceLockEndV1(data: unknown): ValidationResult {
  return validate(BaomaiV4ResonanceLockEndV1, data);
}
