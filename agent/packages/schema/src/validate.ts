import { Value } from "@sinclair/typebox/value";
import type { TSchema } from "@sinclair/typebox";

export interface ValidationResult {
  ok: boolean;
  errors: string[];
}

/**
 * 校验 data 是否符合 schema。
 * 轻量封装 TypeBox Value.Check，统一错误格式。
 */
export function validate<T extends TSchema>(schema: T, data: unknown): ValidationResult {
  const errors = [...Value.Errors(schema, data)];
  if (errors.length === 0) {
    return { ok: true, errors: [] };
  }
  return {
    ok: false,
    errors: errors.map((e) => `${e.path}: ${e.message}`),
  };
}
